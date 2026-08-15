use std::fs;
use std::time::{Duration, SystemTime};

use tempfile::tempdir;

use crate::catalog::Catalog;
use crate::domain::{
    AsrErrorCode, AudioChunk, AudioSource, CaptureSession, ChunkIntegrityState, TranscriptSegment,
};
use crate::service::{
    parse_evidence_uri, EvidenceService, EvidenceTarget, ImportFault, ServiceError,
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
    assert!(service.catalog().list_chunks().unwrap().is_empty());
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
    assert!(service.catalog().list_chunks().unwrap().is_empty());
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
    assert_eq!(fs::read(&source).unwrap(), SOURCE_BYTES);
    assert_eq!(audio_files(data_dir.path()).len(), 1);

    service
        .reconcile_audio_before(SystemTime::now() + Duration::from_secs(1))
        .unwrap();
    assert!(audio_files(data_dir.path()).is_empty());
}

#[test]
fn startup_reconciliation_removes_expired_temp_and_final_orphans() {
    let data_dir = tempdir().unwrap();
    let audio_dir = data_dir.path().join("audio");
    fs::create_dir_all(&audio_dir).unwrap();
    fs::write(audio_dir.join(".lifesub-import-orphan.tmp"), b"partial").unwrap();
    fs::write(audio_dir.join("unreferenced.wav"), b"complete").unwrap();
    let service = EvidenceService::new(Catalog::in_memory().unwrap(), data_dir.path());

    service
        .reconcile_audio_before(SystemTime::now() + Duration::from_secs(1))
        .unwrap();

    assert!(audio_files(data_dir.path()).is_empty());
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
