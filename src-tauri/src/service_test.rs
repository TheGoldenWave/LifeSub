use std::fs;
use std::process::Command;
use std::sync::{Arc, Barrier};
use std::time::{Duration, SystemTime};

use tempfile::tempdir;

use crate::catalog::Catalog;
use crate::domain::{
    AsrErrorCode, AudioChunk, AudioSource, CaptureSession, ChunkIntegrityState, TranscriptSegment,
};
use crate::service::{
    parse_evidence_uri, CoreRuntime, CoreRuntimeError, EvidenceService, EvidenceTarget, ImportFault,
    RuntimeOwnershipError, RuntimeOwnershipGuard, ServiceError,
};

const SOURCE_BYTES: &[u8] = b"lifesub audio fixture";

#[test]
fn imported_audio_is_copied_and_hashed_without_touching_the_source() {
    let source_dir = tempdir().unwrap();
    let data_dir = tempdir().unwrap();
    let source = source_dir.path().join("sample.wav");
    fs::write(&source, SOURCE_BYTES).unwrap();
    let catalog = Catalog::in_memory().unwrap();
    let service = EvidenceService::new(catalog, data_dir.path());
    let session = CaptureSession::new("导入测试");

    let chunk = service.import_audio(&session, &source).unwrap();

    assert_eq!(fs::read(&source).unwrap(), SOURCE_BYTES);
    assert_eq!(chunk.source, AudioSource::Imported);
    assert_eq!(chunk.byte_length, 21);
    assert!(data_dir.path().join(&chunk.path).exists());
    assert_eq!(chunk.sha256.len(), 64);
    assert_eq!(
        service.chunk_integrity(&chunk.id).unwrap(),
        ChunkIntegrityState::Available
    );
}

#[test]
fn empty_source_extension_is_normalized_to_audio() {
    let source_dir = tempdir().unwrap();
    let data_dir = tempdir().unwrap();
    let source = source_dir.path().join("sample.");
    fs::write(&source, SOURCE_BYTES).unwrap();
    let service = EvidenceService::new(Catalog::in_memory().unwrap(), data_dir.path());
    let session = CaptureSession::new("empty extension");

    let chunk = service.import_audio(&session, &source).unwrap();

    assert!(chunk.path.ends_with(".audio"));
    assert!(data_dir.path().join(&chunk.path).is_file());
}

#[cfg(unix)]
#[test]
fn non_utf8_source_extension_is_normalized_to_audio() {
    use std::os::unix::ffi::OsStringExt;

    let source = std::path::PathBuf::from(std::ffi::OsString::from_vec(
        b"sample.\xff".to_vec(),
    ));

    assert_eq!(crate::service::normalized_audio_extension(&source), "audio");
}

#[test]
fn empty_extension_orphan_from_catalog_failure_is_reconcilable() {
    let source_dir = tempdir().unwrap();
    let data_dir = tempdir().unwrap();
    let source = source_dir.path().join("sample.");
    fs::write(&source, SOURCE_BYTES).unwrap();
    let catalog = Catalog::in_memory().unwrap();
    catalog.fail_next_chunk_insert();
    let service = EvidenceService::new(catalog, data_dir.path());
    let session = CaptureSession::new("empty extension rollback");

    assert!(matches!(
        service.import_audio(&session, &source),
        Err(ServiceError::Catalog(_))
    ));
    service
        .reconcile_audio_before(SystemTime::now() + Duration::from_secs(1))
        .unwrap();

    assert!(audio_files(data_dir.path()).is_empty());
}

#[test]
fn hostile_extensions_fall_back_and_valid_extension_is_lowercased() {
    for (name, expected) in [
        ("sample.WAV", "wav"),
        ("sample.a-b", "audio"),
        ("sample.a b", "audio"),
        ("sample.a_b", "audio"),
        ("sample.abcdefghijklmnopq", "audio"),
        ("sample.\n", "audio"),
    ] {
        assert_eq!(
            crate::service::normalized_audio_extension(std::path::Path::new(name)),
            expected
        );
    }
}

#[test]
fn hostile_extension_lookalike_is_not_deleted() {
    let data_dir = tempdir().unwrap();
    let audio_dir = data_dir.path().join("audio");
    fs::create_dir_all(&audio_dir).unwrap();
    let lookalike = audio_dir.join(format!(
        "{}-chk_{}.a-b",
        "b".repeat(64),
        "c".repeat(32)
    ));
    fs::write(&lookalike, b"user lookalike").unwrap();
    let service = EvidenceService::new(Catalog::in_memory().unwrap(), data_dir.path());

    assert!(matches!(
        service.reconcile_audio_before(SystemTime::now() + Duration::from_secs(1)),
        Err(ServiceError::Io(_))
    ));
    assert_eq!(fs::read(lookalike).unwrap(), b"user lookalike");
}

#[test]
fn concurrent_first_imports_share_directory_creation() {
    let source_dir = tempdir().unwrap();
    let data_dir = tempdir().unwrap();
    let source_one = source_dir.path().join("one.wav");
    let source_two = source_dir.path().join("two.wav");
    fs::write(&source_one, b"one").unwrap();
    fs::write(&source_two, b"two").unwrap();
    let service = Arc::new(EvidenceService::with_first_audio_create_barrier(
        Catalog::in_memory().unwrap(),
        data_dir.path(),
        Arc::new(Barrier::new(2)),
    ));
    let session_one = CaptureSession::new("concurrent one");
    let session_two = CaptureSession::new("concurrent two");

    let first = {
        let service = Arc::clone(&service);
        std::thread::spawn(move || service.import_audio(&session_one, source_one).unwrap())
    };
    let second = {
        let service = Arc::clone(&service);
        std::thread::spawn(move || service.import_audio(&session_two, source_two).unwrap())
    };
    let chunks = [first.join().unwrap(), second.join().unwrap()];

    assert_eq!(service.catalog().list_chunks().unwrap().len(), 2);
    assert!(chunks
        .iter()
        .all(|chunk| data_dir.path().join(&chunk.path).is_file()));
}

#[test]
fn imported_audio_crash_after_temp_sync_leaves_only_reconcilable_temp() {
    let source_dir = tempdir().unwrap();
    let data_dir = tempdir().unwrap();
    let source = source_dir.path().join("sample.wav");
    fs::write(&source, SOURCE_BYTES).unwrap();
    let service = EvidenceService::with_import_fault(
        Catalog::in_memory().unwrap(),
        data_dir.path(),
        ImportFault::AfterTempSync,
    );
    let session = CaptureSession::new("temp crash");

    assert!(matches!(
        service.import_audio(&session, &source),
        Err(ServiceError::InjectedCrash(ImportFault::AfterTempSync))
    ));

    let audio_files = audio_files(data_dir.path());
    assert_eq!(audio_files.len(), 1);
    assert!(audio_files[0]
        .file_name()
        .unwrap()
        .to_string_lossy()
        .ends_with(".tmp"));
    assert_no_import_metadata(&service, &session);
    assert_eq!(fs::read(&source).unwrap(), SOURCE_BYTES);
}

#[test]
fn imported_audio_crash_after_final_rename_leaves_only_final_orphan() {
    let source_dir = tempdir().unwrap();
    let data_dir = tempdir().unwrap();
    let source = source_dir.path().join("sample.wav");
    fs::write(&source, SOURCE_BYTES).unwrap();
    let service = EvidenceService::with_import_fault(
        Catalog::in_memory().unwrap(),
        data_dir.path(),
        ImportFault::AfterFinalRename,
    );
    let session = CaptureSession::new("rename crash");

    assert!(matches!(
        service.import_audio(&session, &source),
        Err(ServiceError::InjectedCrash(ImportFault::AfterFinalRename))
    ));

    let audio_files = audio_files(data_dir.path());
    assert_eq!(audio_files.len(), 1);
    assert!(!audio_files[0]
        .file_name()
        .unwrap()
        .to_string_lossy()
        .ends_with(".tmp"));
    assert_no_import_metadata(&service, &session);
}

#[test]
fn source_copy_failure_creates_no_import_metadata() {
    let source_dir = tempdir().unwrap();
    let data_dir = tempdir().unwrap();
    let service = EvidenceService::new(Catalog::in_memory().unwrap(), data_dir.path());
    let session = CaptureSession::new("copy failure");

    assert!(matches!(
        service.import_audio(&session, source_dir.path()),
        Err(ServiceError::Io(_))
    ));
    assert_no_import_metadata(&service, &session);
    assert!(audio_files(data_dir.path()).is_empty());
}

#[test]
fn initial_audio_directory_sync_failure_creates_no_import_metadata_or_audio() {
    let source_dir = tempdir().unwrap();
    let data_dir = tempdir().unwrap();
    let source = source_dir.path().join("sample.wav");
    fs::write(&source, SOURCE_BYTES).unwrap();
    let service = EvidenceService::with_import_fault(
        Catalog::in_memory().unwrap(),
        data_dir.path(),
        ImportFault::DirectorySyncIo,
    );
    let session = CaptureSession::new("directory sync failure");

    assert!(matches!(
        service.import_audio(&session, &source),
        Err(ServiceError::Io(_))
    ));
    assert_no_import_metadata(&service, &session);
    assert!(!data_dir.path().join("audio").exists());
    assert!(audio_files(data_dir.path()).is_empty());
}

#[test]
fn injected_rename_failure_removes_temp_and_preserves_source() {
    let source_dir = tempdir().unwrap();
    let data_dir = tempdir().unwrap();
    let source = source_dir.path().join("sample.wav");
    fs::write(&source, SOURCE_BYTES).unwrap();
    let service = EvidenceService::with_import_fault(
        Catalog::in_memory().unwrap(),
        data_dir.path(),
        ImportFault::RenameIo,
    );
    let session = CaptureSession::new("rename failure");

    assert!(matches!(
        service.import_audio(&session, &source),
        Err(ServiceError::Io(_))
    ));
    assert_no_import_metadata(&service, &session);
    assert!(audio_files(data_dir.path()).is_empty());
    assert_eq!(fs::read(&source).unwrap(), SOURCE_BYTES);
}

#[test]
fn injected_parent_sync_failure_removes_final_and_preserves_source() {
    let source_dir = tempdir().unwrap();
    let data_dir = tempdir().unwrap();
    let source = source_dir.path().join("sample.wav");
    fs::write(&source, SOURCE_BYTES).unwrap();
    let service = EvidenceService::with_import_fault(
        Catalog::in_memory().unwrap(),
        data_dir.path(),
        ImportFault::ParentSyncIo,
    );
    let session = CaptureSession::new("parent sync failure");

    assert!(matches!(
        service.import_audio(&session, &source),
        Err(ServiceError::Io(_))
    ));
    assert_no_import_metadata(&service, &session);
    assert!(audio_files(data_dir.path()).is_empty());
    assert_eq!(fs::read(&source).unwrap(), SOURCE_BYTES);
}

#[test]
fn catalog_failure_after_rename_preserves_source_and_is_reconciled_without_sleep() {
    let source_dir = tempdir().unwrap();
    let data_dir = tempdir().unwrap();
    let source = source_dir.path().join("sample.wav");
    fs::write(&source, SOURCE_BYTES).unwrap();
    let catalog = Catalog::in_memory().unwrap();
    catalog.fail_next_chunk_insert();
    let service = EvidenceService::new(catalog, data_dir.path());
    let session = CaptureSession::new("catalog failure");

    assert!(matches!(
        service.import_audio(&session, &source),
        Err(ServiceError::Catalog(_))
    ));
    assert_no_import_metadata(&service, &session);
    assert_eq!(fs::read(&source).unwrap(), SOURCE_BYTES);
    assert_eq!(audio_files(data_dir.path()).len(), 1);

    service
        .reconcile_audio_before(SystemTime::now() + Duration::from_secs(1))
        .unwrap();
    assert!(audio_files(data_dir.path()).is_empty());
}

#[test]
fn explicit_reconciliation_removes_stale_recognized_orphans() {
    let data_dir = tempdir().unwrap();
    let audio_dir = data_dir.path().join("audio");
    fs::create_dir_all(&audio_dir).unwrap();
    fs::write(import_temp_path(&audio_dir), b"partial").unwrap();
    fs::write(import_final_path(&audio_dir), b"complete").unwrap();
    let service = EvidenceService::new(Catalog::in_memory().unwrap(), data_dir.path());

    service
        .reconcile_audio_before(SystemTime::now() + Duration::from_secs(1))
        .unwrap();

    assert!(audio_files(data_dir.path()).is_empty());
}

#[test]
fn production_initializer_automatically_reconciles_chunk_state() {
    let data_dir = tempdir().unwrap();
    let catalog = Catalog::in_memory().unwrap();
    let session = CaptureSession::new("startup missing");
    catalog.insert_session(&session).unwrap();
    let missing = chunk(&session, "missing.wav", sha256(b"missing"), 7);
    catalog.insert_chunk(&missing).unwrap();

    let service = EvidenceService::initialize(catalog, data_dir.path()).unwrap();

    assert_eq!(
        service.chunk_integrity(&missing.id).unwrap(),
        ChunkIntegrityState::Missing
    );
}

#[test]
fn production_reconciliation_keeps_recent_recognized_orphans() {
    let data_dir = tempdir().unwrap();
    let audio_dir = data_dir.path().join("audio");
    fs::create_dir_all(&audio_dir).unwrap();
    let temp = import_temp_path(&audio_dir);
    let final_file = import_final_path(&audio_dir);
    fs::write(&temp, b"partial").unwrap();
    fs::write(&final_file, b"complete").unwrap();
    let service = EvidenceService::new(Catalog::in_memory().unwrap(), data_dir.path());

    service.reconcile_audio().unwrap();

    assert!(temp.exists());
    assert!(final_file.exists());
}

#[cfg(target_os = "macos")]
#[test]
fn reconciliation_keeps_orphan_within_grace_by_subsecond_margin() {
    let data_dir = tempdir().unwrap();
    let audio_dir = data_dir.path().join("audio");
    fs::create_dir_all(&audio_dir).unwrap();
    let orphan = import_final_path(&audio_dir);
    fs::write(&orphan, b"recent orphan").unwrap();
    let stale_before = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
    set_modified_time(&orphan, stale_before + Duration::from_millis(500));
    let service = EvidenceService::new(Catalog::in_memory().unwrap(), data_dir.path());

    service.reconcile_audio_before(stale_before).unwrap();

    assert!(orphan.exists());
}

#[cfg(target_os = "macos")]
#[test]
fn reconciliation_removes_orphan_beyond_grace_by_subsecond_margin() {
    let data_dir = tempdir().unwrap();
    let audio_dir = data_dir.path().join("audio");
    fs::create_dir_all(&audio_dir).unwrap();
    let orphan = import_final_path(&audio_dir);
    fs::write(&orphan, b"stale orphan").unwrap();
    let stale_before = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
    set_modified_time(&orphan, stale_before - Duration::from_millis(500));
    let service = EvidenceService::new(Catalog::in_memory().unwrap(), data_dir.path());

    service.reconcile_audio_before(stale_before).unwrap();

    assert!(!orphan.exists());
}

#[test]
fn reconciliation_rejects_unknown_entries_without_deleting_them() {
    let data_dir = tempdir().unwrap();
    let audio_dir = data_dir.path().join("audio");
    fs::create_dir_all(&audio_dir).unwrap();
    let unknown = audio_dir.join("user-file.wav");
    fs::write(&unknown, b"not managed by LifeSub").unwrap();
    let service = EvidenceService::new(Catalog::in_memory().unwrap(), data_dir.path());

    assert!(matches!(
        service.reconcile_audio_before(SystemTime::now() + Duration::from_secs(1)),
        Err(ServiceError::Io(_))
    ));
    assert_eq!(fs::read(unknown).unwrap(), b"not managed by LifeSub");
}

#[test]
fn reconciliation_rejects_uppercase_importer_lookalike_without_deleting_it() {
    let data_dir = tempdir().unwrap();
    let audio_dir = data_dir.path().join("audio");
    fs::create_dir_all(&audio_dir).unwrap();
    let lookalike = audio_dir.join(format!(
        "{}-chk_{}.wav",
        "B".repeat(64),
        "C".repeat(32)
    ));
    fs::write(&lookalike, b"not generated by the importer").unwrap();
    let service = EvidenceService::new(Catalog::in_memory().unwrap(), data_dir.path());

    assert!(matches!(
        service.reconcile_audio_before(SystemTime::now() + Duration::from_secs(1)),
        Err(ServiceError::Io(_))
    ));
    assert_eq!(
        fs::read(lookalike).unwrap(),
        b"not generated by the importer"
    );
}

#[cfg(unix)]
#[test]
fn reconciliation_rejects_symlinked_audio_root_without_touching_external_files() {
    use std::os::unix::fs::symlink;

    let data_dir = tempdir().unwrap();
    let external_dir = tempdir().unwrap();
    let external_file = import_final_path(external_dir.path());
    fs::write(&external_file, b"external").unwrap();
    symlink(external_dir.path(), data_dir.path().join("audio")).unwrap();
    let service = EvidenceService::new(Catalog::in_memory().unwrap(), data_dir.path());

    assert!(matches!(
        service.reconcile_audio_before(SystemTime::now() + Duration::from_secs(1)),
        Err(ServiceError::Io(_))
    ));
    assert_eq!(fs::read(external_file).unwrap(), b"external");
}

#[cfg(unix)]
#[test]
fn import_does_not_follow_audio_directory_replaced_after_open() {
    let source_dir = tempdir().unwrap();
    let data_dir = tempdir().unwrap();
    let external_dir = tempdir().unwrap();
    let source = source_dir.path().join("sample.wav");
    fs::write(&source, SOURCE_BYTES).unwrap();
    fs::create_dir(data_dir.path().join("audio")).unwrap();
    let service = EvidenceService::with_audio_directory_swap(
        Catalog::in_memory().unwrap(),
        data_dir.path(),
        external_dir.path(),
    );
    let session = CaptureSession::new("swapped import root");

    assert!(matches!(
        service.import_audio(&session, &source),
        Err(ServiceError::Io(_))
    ));
    assert!(audio_files(external_dir.path()).is_empty());
    assert_no_import_metadata(&service, &session);
}

#[cfg(unix)]
#[test]
fn reconciliation_does_not_delete_from_audio_directory_replaced_after_open() {
    let data_dir = tempdir().unwrap();
    let external_dir = tempdir().unwrap();
    let audio_dir = data_dir.path().join("audio");
    fs::create_dir(&audio_dir).unwrap();
    fs::write(import_final_path(&audio_dir), b"owned orphan").unwrap();
    let external = import_final_path(external_dir.path());
    fs::write(&external, b"external orphan").unwrap();
    let service = EvidenceService::with_audio_directory_swap(
        Catalog::in_memory().unwrap(),
        data_dir.path(),
        external_dir.path(),
    );

    assert!(matches!(
        service.reconcile_audio_before(SystemTime::now() + Duration::from_secs(1)),
        Err(ServiceError::Io(_))
    ));
    assert_eq!(fs::read(external).unwrap(), b"external orphan");
    assert!(import_final_path(&data_dir.path().join("audio-held")).exists());
}

#[cfg(unix)]
#[test]
fn verification_does_not_read_from_audio_directory_replaced_after_open() {
    let data_dir = tempdir().unwrap();
    let external_dir = tempdir().unwrap();
    let audio_dir = data_dir.path().join("audio");
    fs::create_dir(&audio_dir).unwrap();
    fs::write(audio_dir.join("swapped.wav"), b"original").unwrap();
    fs::write(external_dir.path().join("swapped.wav"), SOURCE_BYTES).unwrap();
    let catalog = Catalog::in_memory().unwrap();
    let session = CaptureSession::new("swapped verification root");
    catalog.insert_session(&session).unwrap();
    let swapped = chunk(
        &session,
        "swapped.wav",
        sha256(SOURCE_BYTES),
        SOURCE_BYTES.len() as u64,
    );
    catalog.insert_chunk(&swapped).unwrap();
    let service = EvidenceService::with_audio_directory_swap(
        catalog,
        data_dir.path(),
        external_dir.path(),
    );

    assert_eq!(
        service.verify_chunk(&swapped.id),
        Err(ServiceError::InputIntegrityFailed)
    );
}

#[cfg(unix)]
#[test]
fn symlinked_chunk_is_corrupted_and_never_verified() {
    use std::os::unix::fs::symlink;

    let data_dir = tempdir().unwrap();
    let external_dir = tempdir().unwrap();
    let external_file = external_dir.path().join("outside.wav");
    fs::write(&external_file, SOURCE_BYTES).unwrap();
    let audio_dir = data_dir.path().join("audio");
    fs::create_dir_all(&audio_dir).unwrap();
    let link = audio_dir.join("linked.wav");
    symlink(&external_file, &link).unwrap();
    let service = EvidenceService::new(Catalog::in_memory().unwrap(), data_dir.path());
    let session = CaptureSession::new("symlink chunk");
    service.catalog().insert_session(&session).unwrap();
    let linked = chunk(&session, "linked.wav", sha256(SOURCE_BYTES), SOURCE_BYTES.len() as u64);
    service.catalog().insert_chunk(&linked).unwrap();

    service.reconcile_audio().unwrap();

    assert_eq!(
        service.chunk_integrity(&linked.id).unwrap(),
        ChunkIntegrityState::Corrupted
    );
    assert_eq!(
        service.verify_chunk(&linked.id),
        Err(ServiceError::InputIntegrityFailed)
    );
}

#[test]
fn second_runtime_instance_fails_before_catalog_or_reconciliation() {
    let (parent, data_dir) = runtime_data_dir();
    let first = CoreRuntime::initialize(&data_dir).unwrap();
    let audio_dir = data_dir.join("audio");
    fs::create_dir_all(&audio_dir).unwrap();
    let orphan = import_final_path(&audio_dir);
    fs::write(&orphan, b"orphan").unwrap();

    assert!(matches!(
        CoreRuntime::initialize(&data_dir),
        Err(CoreRuntimeError::Ownership(RuntimeOwnershipError::AlreadyOwned))
    ));
    assert!(orphan.exists());
    drop(first);
    drop(parent);
}

#[test]
fn child_process_loser_cannot_create_catalog_or_reconcile() {
    let (_parent, data_dir) = runtime_data_dir();
    fs::create_dir(&data_dir).unwrap();
    let _owner = RuntimeOwnershipGuard::acquire(&data_dir).unwrap();
    let audio_dir = data_dir.join("audio");
    fs::create_dir_all(&audio_dir).unwrap();
    let orphan = import_final_path(&audio_dir);
    fs::write(&orphan, b"orphan").unwrap();
    let status = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "service_test::runtime_lock_child_observes_existing_owner",
        ])
        .env("LIFESUB_RUNTIME_LOCK_TEST_DIR", &data_dir)
        .status()
        .unwrap();

    assert!(status.success());
    assert!(!data_dir.join("lifesub.sqlite3").exists());
    assert!(orphan.exists());
}

#[test]
fn runtime_lock_child_observes_existing_owner() {
    let Some(data_dir) = std::env::var_os("LIFESUB_RUNTIME_LOCK_TEST_DIR") else {
        return;
    };
    assert!(matches!(
        CoreRuntime::initialize(data_dir),
        Err(CoreRuntimeError::Ownership(RuntimeOwnershipError::AlreadyOwned))
    ));
}

#[cfg(unix)]
#[test]
fn runtime_lock_symlink_is_rejected_without_touching_target() {
    use std::os::unix::fs::symlink;

    let (_parent, data_dir) = runtime_data_dir();
    fs::create_dir(&data_dir).unwrap();
    let external_dir = tempdir().unwrap();
    let target = external_dir.path().join("external-lock");
    fs::write(&target, b"external").unwrap();
    symlink(&target, data_dir.join("asr-worker.lock")).unwrap();

    assert!(matches!(
        RuntimeOwnershipGuard::acquire(&data_dir),
        Err(RuntimeOwnershipError::UnsafePath)
    ));
    assert_eq!(fs::read(target).unwrap(), b"external");
}

#[cfg(unix)]
#[test]
fn runtime_lock_swap_after_open_is_rejected() {
    let (_parent, data_dir) = runtime_data_dir();
    fs::create_dir(&data_dir).unwrap();
    let lock_path = data_dir.join("asr-worker.lock");

    let result = RuntimeOwnershipGuard::acquire_with_lock_swap(&data_dir, || {
        fs::rename(&lock_path, data_dir.join("old-lock"))?;
        fs::write(&lock_path, b"replacement")
    });

    assert!(matches!(result, Err(RuntimeOwnershipError::UnsafePath)));
}

#[cfg(unix)]
#[test]
fn runtime_lock_replacement_invalidates_owner_and_blocks_second_core() {
    let source_dir = tempdir().unwrap();
    let source = source_dir.path().join("sample.wav");
    fs::write(&source, SOURCE_BYTES).unwrap();
    let (_parent, data_dir) = runtime_data_dir();
    let first = CoreRuntime::initialize(&data_dir).unwrap();
    let (catalog, ownership) = first.into_parts();
    let service = EvidenceService::new(catalog, &data_dir);
    let session = CaptureSession::new("lock replacement import");
    let audio_dir = data_dir.join("audio");
    fs::create_dir(&audio_dir).unwrap();
    let orphan = import_final_path(&audio_dir);
    fs::write(&orphan, b"must not be reconciled by replacement owner").unwrap();
    let lock_path = data_dir.join("asr-worker.lock");
    fs::rename(&lock_path, data_dir.join("old-lock")).unwrap();
    fs::write(&lock_path, b"replacement").unwrap();

    assert!(matches!(
        CoreRuntime::initialize(&data_dir),
        Err(CoreRuntimeError::Ownership(_))
    ));
    assert!(orphan.exists());
    assert!(matches!(
        ownership.ensure_current(),
        Err(RuntimeOwnershipError::UnsafePath)
    ));
    let import = ownership
        .ensure_current()
        .map_err(|_| ())
        .and_then(|_| service.import_audio(&session, &source).map_err(|_| ()));
    assert!(import.is_err());
    assert_no_import_metadata(&service, &session);
}

#[cfg(unix)]
#[test]
fn runtime_data_directory_swap_is_rejected_before_catalog_creation() {
    let parent = tempdir().unwrap();
    let data_dir = parent.path().join("data");
    fs::create_dir(&data_dir).unwrap();

    let result = CoreRuntime::initialize_with_data_dir_swap(&data_dir, || {
        fs::rename(&data_dir, parent.path().join("old-data"))?;
        fs::create_dir(&data_dir)
    });

    assert!(matches!(
        result,
        Err(CoreRuntimeError::Ownership(RuntimeOwnershipError::UnsafePath))
    ));
    assert!(!data_dir.join("lifesub.sqlite3").exists());
}

#[cfg(unix)]
#[test]
fn external_owner_lock_survives_whole_data_directory_replacement() {
    let source_dir = tempdir().unwrap();
    let source = source_dir.path().join("sample.wav");
    fs::write(&source, SOURCE_BYTES).unwrap();
    let parent = tempdir().unwrap();
    let data_dir = parent.path().join("data");
    let first = CoreRuntime::initialize(&data_dir).unwrap();
    let (catalog, ownership) = first.into_parts();
    let service = EvidenceService::new(catalog, &data_dir);
    let session = CaptureSession::new("whole directory replacement");
    fs::rename(&data_dir, parent.path().join("old-data")).unwrap();
    fs::create_dir(&data_dir).unwrap();

    assert!(matches!(
        CoreRuntime::initialize(&data_dir),
        Err(CoreRuntimeError::Ownership(RuntimeOwnershipError::AlreadyOwned))
    ));
    assert!(!data_dir.join("lifesub.sqlite3").exists());
    assert!(ownership.ensure_current().is_err());
    let import = ownership
        .ensure_current()
        .map_err(|_| ())
        .and_then(|_| service.import_audio(&session, &source).map_err(|_| ()));
    assert!(import.is_err());
    assert_no_import_metadata(&service, &session);
}

#[cfg(unix)]
#[test]
fn parent_lock_survives_combined_anchor_and_data_directory_replacement() {
    let source_dir = tempdir().unwrap();
    let source = source_dir.path().join("sample.wav");
    fs::write(&source, SOURCE_BYTES).unwrap();
    let parent = tempdir().unwrap();
    let data_dir = parent.path().join("data");
    let first = CoreRuntime::initialize(&data_dir).unwrap();
    let (catalog, ownership) = first.into_parts();
    let service = EvidenceService::new(catalog, &data_dir);
    let session = CaptureSession::new("combined replacement");
    let anchor = fs::read_dir(parent.path())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(".lifesub-core-")
        })
        .unwrap();
    fs::rename(&anchor, parent.path().join("old-anchor")).unwrap();
    fs::write(&anchor, b"replacement anchor").unwrap();
    fs::rename(&data_dir, parent.path().join("old-data")).unwrap();
    fs::create_dir(&data_dir).unwrap();

    assert!(matches!(
        CoreRuntime::initialize(&data_dir),
        Err(CoreRuntimeError::Ownership(RuntimeOwnershipError::AlreadyOwned))
    ));
    assert!(!data_dir.join("lifesub.sqlite3").exists());
    assert!(ownership.ensure_current().is_err());
    assert!(ownership
        .ensure_current()
        .map_err(|_| ())
        .and_then(|_| service.import_audio(&session, &source).map_err(|_| ()))
        .is_err());
    assert_no_import_metadata(&service, &session);
}

#[test]
fn canonical_parent_allows_only_one_lifesub_core() {
    let parent = tempdir().unwrap();

    let first = CoreRuntime::initialize(parent.path().join("first")).unwrap();
    let second = CoreRuntime::initialize(parent.path().join("second"));

    assert!(matches!(
        second,
        Err(CoreRuntimeError::Ownership(RuntimeOwnershipError::AlreadyOwned))
    ));
    assert!(!parent.path().join("second/lifesub.sqlite3").exists());
    drop(first);
}

#[test]
fn startup_reconciliation_classifies_missing_changed_and_unreadable_chunks() {
    let data_dir = tempdir().unwrap();
    let service = EvidenceService::new(Catalog::in_memory().unwrap(), data_dir.path());
    let session = CaptureSession::new("integrity states");
    service.catalog().insert_session(&session).unwrap();
    let missing = chunk(&session, "missing.wav", sha256(b"missing"), 7);
    service.catalog().insert_chunk(&missing).unwrap();
    let changed = chunk(&session, "changed.wav", sha256(b"original"), 8);
    service.catalog().insert_chunk(&changed).unwrap();
    fs::create_dir_all(data_dir.path().join("audio")).unwrap();
    fs::write(data_dir.path().join(&changed.path), b"modified").unwrap();
    let unreadable = chunk(&session, "directory.wav", sha256(b"directory"), 9);
    fs::create_dir_all(data_dir.path().join(&unreadable.path)).unwrap();
    service.catalog().insert_chunk(&unreadable).unwrap();

    service.reconcile_audio().unwrap();

    assert_eq!(
        service.chunk_integrity(&missing.id).unwrap(),
        ChunkIntegrityState::Missing
    );
    assert_eq!(
        service.chunk_integrity(&changed.id).unwrap(),
        ChunkIntegrityState::Corrupted
    );
    assert_eq!(
        service.chunk_integrity(&unreadable.id).unwrap(),
        ChunkIntegrityState::Corrupted
    );

    let missing_diagnostics = service
        .catalog()
        .chunk_diagnostics(&missing.id)
        .unwrap()
        .unwrap();
    assert_eq!(
        missing_diagnostics.error_code,
        Some(AsrErrorCode::InputUnavailable)
    );
    assert!(missing_diagnostics.error_at.is_some());
    let changed_diagnostics = service
        .catalog()
        .chunk_diagnostics(&changed.id)
        .unwrap()
        .unwrap();
    assert_eq!(
        changed_diagnostics.error_code,
        Some(AsrErrorCode::InputIntegrityFailed)
    );
    assert!(changed_diagnostics.error_at.is_some());

    fs::write(data_dir.path().join(&missing.path), b"missing").unwrap();
    fs::write(data_dir.path().join(&changed.path), b"original").unwrap();
    service.reconcile_audio().unwrap();

    for repaired in [&missing, &changed] {
        let diagnostics = service
            .catalog()
            .chunk_diagnostics(&repaired.id)
            .unwrap()
            .unwrap();
        assert_eq!(diagnostics.integrity_state, ChunkIntegrityState::Available);
        assert_eq!(diagnostics.error_code, None);
        assert_eq!(diagnostics.error_at, None);
    }
}

#[test]
fn verify_chunk_rehashes_immediately_before_asr() {
    let source_dir = tempdir().unwrap();
    let data_dir = tempdir().unwrap();
    let source = source_dir.path().join("sample.wav");
    fs::write(&source, SOURCE_BYTES).unwrap();
    let service = EvidenceService::new(Catalog::in_memory().unwrap(), data_dir.path());
    let session = CaptureSession::new("verify before ASR");
    let chunk = service.import_audio(&session, &source).unwrap();

    fs::write(data_dir.path().join(&chunk.path), b"changed after import").unwrap();

    assert_eq!(
        service.verify_chunk(&chunk.id),
        Err(ServiceError::InputIntegrityFailed)
    );
    assert_eq!(
        service.verify_chunk(&chunk.id).unwrap_err().code(),
        AsrErrorCode::InputIntegrityFailed
    );
    assert_eq!(
        service.chunk_integrity(&chunk.id).unwrap(),
        ChunkIntegrityState::Corrupted
    );
}

#[test]
fn reconciliation_rejects_unknown_integrity_without_rewriting_valid_chunk() {
    let source_dir = tempdir().unwrap();
    let data_dir = tempdir().unwrap();
    let source = source_dir.path().join("sample.wav");
    fs::write(&source, SOURCE_BYTES).unwrap();
    let catalog = Catalog::in_memory().unwrap();
    let service = EvidenceService::new(catalog, data_dir.path());
    let session = CaptureSession::new("future integrity reconciliation");
    let chunk = service.import_audio(&session, &source).unwrap();
    service
        .catalog()
        .force_chunk_integrity(&chunk.id, "future_state")
        .unwrap();
    let orphan = import_final_path(&data_dir.path().join("audio"));
    fs::write(&orphan, b"must survive fail closed reconciliation").unwrap();

    assert!(matches!(
        service.reconcile_audio_before(SystemTime::now() + Duration::from_secs(1)),
        Err(ServiceError::Catalog(_))
    ));
    assert!(service.catalog().chunk_integrity(&chunk.id).is_err());
    assert_eq!(
        fs::read(orphan).unwrap(),
        b"must survive fail closed reconciliation"
    );
}

#[test]
fn verify_chunk_rejects_unknown_integrity_without_rewriting_it() {
    let source_dir = tempdir().unwrap();
    let data_dir = tempdir().unwrap();
    let source = source_dir.path().join("sample.wav");
    fs::write(&source, SOURCE_BYTES).unwrap();
    let catalog = Catalog::in_memory().unwrap();
    let service = EvidenceService::new(catalog, data_dir.path());
    let session = CaptureSession::new("future integrity verification");
    let chunk = service.import_audio(&session, &source).unwrap();
    service
        .catalog()
        .force_chunk_integrity(&chunk.id, "future_state")
        .unwrap();

    assert!(matches!(
        service.verify_chunk(&chunk.id),
        Err(ServiceError::Catalog(_))
    ));
    assert!(service.catalog().chunk_integrity(&chunk.id).is_err());
}

#[test]
fn verify_chunk_reports_missing_input_with_stable_error_code() {
    let source_dir = tempdir().unwrap();
    let data_dir = tempdir().unwrap();
    let source = source_dir.path().join("sample.wav");
    fs::write(&source, SOURCE_BYTES).unwrap();
    let service = EvidenceService::new(Catalog::in_memory().unwrap(), data_dir.path());
    let session = CaptureSession::new("missing before ASR");
    let chunk = service.import_audio(&session, &source).unwrap();
    fs::remove_file(data_dir.path().join(&chunk.path)).unwrap();

    assert_eq!(
        service.verify_chunk(&chunk.id),
        Err(ServiceError::InputUnavailable)
    );
    assert_eq!(
        service.verify_chunk(&chunk.id).unwrap_err().code(),
        AsrErrorCode::InputUnavailable
    );
    assert_eq!(
        service.chunk_integrity(&chunk.id).unwrap(),
        ChunkIntegrityState::Missing
    );
}

#[test]
fn markdown_export_contains_revision_and_stable_evidence() {
    let data_dir = tempdir().unwrap();
    let catalog = Catalog::in_memory().unwrap();
    let session = CaptureSession::new("首版讨论");
    catalog.insert_session(&session).unwrap();
    let revision = catalog
        .append_revision(
            &session.id,
            "demo-local",
            vec![TranscriptSegment::new(
                1200,
                4400,
                AudioSource::Microphone,
                "原始音频先持久化",
            )],
        )
        .unwrap();
    let service = EvidenceService::new(catalog, data_dir.path());

    let markdown = service.render_markdown(&session, &revision);

    assert!(markdown.contains(&session.evidence_uri()));
    assert!(markdown.contains("transcript_revision: 1"));
    assert!(markdown.contains("原始音频先持久化"));
}

#[test]
fn evidence_uri_parser_keeps_audio_time_ranges() {
    assert_eq!(
        parse_evidence_uri("lifesub://audio/chk_123#t=120,165").unwrap(),
        EvidenceTarget::Audio {
            id: "chk_123".into(),
            start_seconds: Some(120),
            end_seconds: Some(165)
        }
    );
}

fn audio_files(data_dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let audio_dir = data_dir.join("audio");
    if !audio_dir.exists() {
        return Vec::new();
    }
    fs::read_dir(audio_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect()
}

fn chunk(
    session: &CaptureSession,
    file_name: &str,
    sha256: String,
    byte_length: u64,
) -> AudioChunk {
    AudioChunk {
        id: format!("chk_{file_name}"),
        session_id: session.id.clone(),
        source: AudioSource::Imported,
        path: format!("audio/{file_name}"),
        sha256,
        byte_length,
    }
}

fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(bytes))
}

fn import_temp_path(audio_dir: &std::path::Path) -> std::path::PathBuf {
    audio_dir.join(format!(".lifesub-import-chk_{}.tmp", "a".repeat(32)))
}

fn import_final_path(audio_dir: &std::path::Path) -> std::path::PathBuf {
    audio_dir.join(format!("{}-chk_{}.wav", "b".repeat(64), "c".repeat(32)))
}

fn runtime_data_dir() -> (tempfile::TempDir, std::path::PathBuf) {
    let parent = tempdir().unwrap();
    let data_dir = parent.path().join("data");
    (parent, data_dir)
}

#[cfg(target_os = "macos")]
fn set_modified_time(path: &std::path::Path, modified: SystemTime) {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let duration = modified.duration_since(SystemTime::UNIX_EPOCH).unwrap();
    let times = [
        libc::timespec {
            tv_sec: duration.as_secs() as libc::time_t,
            tv_nsec: duration.subsec_nanos() as libc::c_long,
        },
        libc::timespec {
            tv_sec: duration.as_secs() as libc::time_t,
            tv_nsec: duration.subsec_nanos() as libc::c_long,
        },
    ];
    let path = CString::new(path.as_os_str().as_bytes()).unwrap();
    assert_eq!(
        unsafe { libc::utimensat(libc::AT_FDCWD, path.as_ptr(), times.as_ptr(), 0) },
        0
    );
}

fn assert_no_import_metadata(service: &EvidenceService, session: &CaptureSession) {
    assert!(!service.catalog().session_exists(&session.id).unwrap());
    assert!(service.catalog().list_chunks().unwrap().is_empty());
}
