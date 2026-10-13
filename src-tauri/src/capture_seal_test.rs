use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use sha2::{Digest, Sha256};
use tempfile::TempDir;

use crate::capture::protocol::{ChunkSealed, Source};
use crate::capture::seal::{
    NativeChunkSealError, NativeChunkSealRequest, NativeChunkSealer, SealFault,
};
use crate::capture::{
    committer::CatalogCaptureCommitter, coordinator::NativeCaptureChunkCommitter,
};
use crate::catalog::Catalog;
use crate::domain::{
    AudioSource, CaptureChunkTiming, CaptureSession, CaptureSourceRecord, CaptureState,
};

struct SealFixture {
    _temporary: TempDir,
    staging: PathBuf,
    final_root: PathBuf,
    catalog: Catalog,
    session: CaptureSession,
    source: CaptureSourceRecord,
}

impl SealFixture {
    fn new() -> Self {
        let temporary = tempfile::tempdir().unwrap();
        let staging = temporary.path().join("staging");
        let final_root = temporary.path().join("data");
        std::fs::create_dir(&staging).unwrap();
        std::fs::create_dir(&final_root).unwrap();

        let catalog = Catalog::in_memory().unwrap();
        let mut session = CaptureSession::new("native seal");
        session.state = CaptureState::Recording;
        catalog.insert_session(&session).unwrap();
        let source = CaptureSourceRecord {
            id: "src_native_seal".into(),
            session_id: session.id.clone(),
            source: AudioSource::Microphone,
            device_uid: "BuiltInMicrophoneDevice".into(),
            device_name: "MacBook Pro Microphone".into(),
            channel_count: 1,
            sample_rate_hz: 48_000,
            created_at: chrono::Utc::now(),
        };
        catalog.insert_capture_source(&source).unwrap();

        Self {
            _temporary: temporary,
            staging,
            final_root,
            catalog,
            session,
            source,
        }
    }

    fn write_partial(&self, name: &str, bytes: &[u8]) {
        std::fs::write(self.staging.join(name), bytes).unwrap();
    }

    fn request(&self, name: impl Into<PathBuf>, bytes: &[u8]) -> NativeChunkSealRequest {
        let now = chrono::Utc::now();
        NativeChunkSealRequest {
            relative_staging_path: name.into(),
            chunk_id: "chk_native_seal".into(),
            session_id: self.session.id.clone(),
            source: AudioSource::Microphone,
            expected_sha256: hex::encode(Sha256::digest(bytes)),
            expected_byte_length: bytes.len() as u64,
            timing: CaptureChunkTiming {
                chunk_id: "chk_native_seal".into(),
                capture_source_id: self.source.id.clone(),
                source_sequence: 4,
                source_sample_start: 48_000,
                source_sample_end: 120_000,
                session_start_ms: 1_000,
                session_end_ms: 2_500,
                monotonic_start_ns: 10_000_000,
                monotonic_end_ns: 25_000_000,
                utc_start_at: now,
                utc_end_at: now + chrono::Duration::milliseconds(1_500),
                discontinuity_count: 0,
                dropped_frame_count: 0,
                created_at: now,
            },
        }
    }

    fn sealer(&self) -> NativeChunkSealer<'_> {
        NativeChunkSealer::new(&self.catalog, &self.staging, &self.final_root).unwrap()
    }

    fn intent_path(&self) -> PathBuf {
        self.final_root
            .join(".lifesub-native-intents/chk_native_seal.json")
    }

    fn intent_entries(&self) -> Vec<String> {
        std::fs::read_dir(self.final_root.join(".lifesub-native-intents"))
            .map(|entries| {
                entries
                    .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default()
    }
}

fn assert_invalid_path(result: Result<crate::domain::AudioChunk, NativeChunkSealError>) {
    assert!(matches!(result, Err(NativeChunkSealError::InvalidPath)));
}

#[test]
fn native_capture_committer_enqueues_only_after_durable_chunk_publication() {
    let fixture = SealFixture::new();
    let bytes = b"native capture enqueue boundary";
    let relative_path = "chk_native_enqueue.caf.partial";
    fixture.write_partial(relative_path, bytes);
    let observed = Arc::new(Mutex::new(None));
    let callback_observed = observed.clone();
    let callback_catalog = &fixture.catalog;
    let callback_final_root = &fixture.final_root;
    let mut committer = CatalogCaptureCommitter::new(
        &fixture.catalog,
        fixture.session.id.clone(),
        &fixture.staging,
        &fixture.final_root,
        std::collections::HashMap::from([(Source::Microphone, fixture.source.id.clone())]),
        10,
        move |chunk| {
            assert!(callback_final_root.join(&chunk.path).is_file());
            assert_eq!(
                callback_catalog.chunk(&chunk.id).unwrap().as_ref(),
                Some(chunk)
            );
            assert!(
                callback_catalog
                    .capture_chunk_timing(&chunk.id)
                    .unwrap()
                    .is_some()
            );
            *callback_observed.lock().unwrap() = Some(chunk.id.clone());
            Ok(())
        },
    )
    .unwrap();

    committer
        .commit(&ChunkSealed {
            source: Source::Microphone,
            relative_staging_path: relative_path.to_owned(),
            byte_length: bytes.len() as u64,
            sha256: hex::encode(Sha256::digest(bytes)),
            duration_ms: 1_000,
            start_sample_position: 0,
            end_sample_position: 16_000,
            start_host_time: 10,
            end_host_time: 1_000_000_010,
        })
        .unwrap();

    assert_eq!(
        observed.lock().unwrap().as_deref(),
        Some("chk_native_enqueue")
    );
}

#[test]
fn native_chunk_seal_rejects_unanchored_or_non_partial_paths() {
    let fixture = SealFixture::new();
    let bytes = b"native audio";

    for path in [
        PathBuf::from("/absolute/audio.caf.partial"),
        PathBuf::from("../escape.caf.partial"),
        PathBuf::from("nested/audio.caf.partial"),
        PathBuf::from("audio.caf"),
        PathBuf::new(),
    ] {
        assert_invalid_path(fixture.sealer().seal(&fixture.request(path, bytes)));
    }
}

#[test]
fn native_chunk_seal_rejects_symlinks_and_non_regular_files() {
    let fixture = SealFixture::new();
    let bytes = b"native audio";
    std::fs::write(fixture.staging.join("target.caf.partial"), bytes).unwrap();
    std::os::unix::fs::symlink(
        fixture.staging.join("target.caf.partial"),
        fixture.staging.join("link.caf.partial"),
    )
    .unwrap();
    std::fs::create_dir(fixture.staging.join("directory.caf.partial")).unwrap();

    assert!(
        fixture
            .sealer()
            .seal(&fixture.request("link.caf.partial", bytes))
            .is_err()
    );
    assert!(
        fixture
            .sealer()
            .seal(&fixture.request("directory.caf.partial", bytes))
            .is_err()
    );
    assert!(fixture.catalog.chunk("chk_native_seal").unwrap().is_none());
}

#[test]
fn native_chunk_seal_verifies_byte_length_and_sha256_before_publication() {
    let fixture = SealFixture::new();
    let bytes = b"native audio";
    fixture.write_partial("chk_native_seal.caf.partial", bytes);
    let mut wrong_length = fixture.request("chk_native_seal.caf.partial", bytes);
    wrong_length.expected_byte_length += 1;

    assert!(matches!(
        fixture.sealer().seal(&wrong_length),
        Err(NativeChunkSealError::IntegrityMismatch)
    ));
    assert!(
        fixture
            .catalog
            .chunk(&wrong_length.chunk_id)
            .unwrap()
            .is_none()
    );

    let mut wrong_hash = fixture.request("chk_native_seal.caf.partial", bytes);
    wrong_hash.expected_sha256 = "0".repeat(64);
    assert!(matches!(
        fixture.sealer().seal(&wrong_hash),
        Err(NativeChunkSealError::IntegrityMismatch)
    ));
    assert!(
        fixture
            .catalog
            .chunk(&wrong_hash.chunk_id)
            .unwrap()
            .is_none()
    );
}

#[test]
fn native_chunk_seal_publishes_file_chunk_and_timing_after_durable_storage() {
    let fixture = SealFixture::new();
    let bytes = b"durable native audio";
    fixture.write_partial("chk_native_seal.caf.partial", bytes);
    let request = fixture.request("chk_native_seal.caf.partial", bytes);

    let chunk = fixture.sealer().seal(&request).unwrap();

    assert_eq!(chunk.path, "audio/chk_native_seal.caf");
    assert_eq!(
        std::fs::read(fixture.final_root.join(&chunk.path)).unwrap(),
        bytes
    );
    assert!(!fixture.staging.join("chk_native_seal.caf.partial").exists());
    assert_eq!(fixture.catalog.chunk(&chunk.id).unwrap(), Some(chunk));
    assert_eq!(
        fixture
            .catalog
            .capture_chunk_timing(&request.chunk_id)
            .unwrap(),
        Some(request.timing)
    );
    assert!(!fixture.intent_path().exists());
}

#[test]
fn native_chunk_seal_is_idempotent_and_rejects_conflicting_replays() {
    let fixture = SealFixture::new();
    let bytes = b"idempotent native audio";
    fixture.write_partial("chk_native_seal.caf.partial", bytes);
    let request = fixture.request("chk_native_seal.caf.partial", bytes);
    let original = fixture.sealer().seal(&request).unwrap();

    assert_eq!(fixture.sealer().seal(&request).unwrap(), original);

    let mut conflicting_hash = request.clone();
    conflicting_hash.expected_sha256 = "f".repeat(64);
    assert!(matches!(
        fixture.sealer().seal(&conflicting_hash),
        Err(NativeChunkSealError::Conflict)
    ));
    let mut conflicting_path = request.clone();
    conflicting_path.relative_staging_path = PathBuf::from("other.caf.partial");
    assert!(matches!(
        fixture.sealer().seal(&conflicting_path),
        Err(NativeChunkSealError::InvalidPath | NativeChunkSealError::Conflict)
    ));
    let mut conflicting_timing = request.clone();
    conflicting_timing.timing.session_end_ms += 1;
    assert!(matches!(
        fixture.sealer().seal(&conflicting_timing),
        Err(NativeChunkSealError::Conflict)
    ));
    assert_eq!(fixture.catalog.chunk(&original.id).unwrap(), Some(original));
    assert_eq!(
        fixture
            .catalog
            .capture_chunk_timing(&request.chunk_id)
            .unwrap(),
        Some(request.timing)
    );
    assert!(!fixture.intent_path().exists());
}

#[test]
fn native_chunk_seal_never_overwrites_an_existing_final_file() {
    let fixture = SealFixture::new();
    let bytes = b"new native audio";
    fixture.write_partial("chk_native_seal.caf.partial", bytes);
    std::fs::create_dir(fixture.final_root.join("audio")).unwrap();
    let final_path = fixture.final_root.join("audio/chk_native_seal.caf");
    std::fs::write(&final_path, b"existing evidence").unwrap();
    let request = fixture.request("chk_native_seal.caf.partial", bytes);

    assert!(matches!(
        fixture.sealer().seal(&request),
        Err(NativeChunkSealError::Conflict)
    ));
    assert_eq!(std::fs::read(final_path).unwrap(), b"existing evidence");
    assert!(fixture.catalog.chunk(&request.chunk_id).unwrap().is_none());
}

#[test]
fn native_chunk_seal_rejects_reusing_a_staging_basename_for_another_chunk() {
    let fixture = SealFixture::new();
    let bytes = b"same bytes must not alias provenance";
    fixture.write_partial("chk_native_seal.caf.partial", bytes);
    let original = fixture.request("chk_native_seal.caf.partial", bytes);
    fixture.sealer().seal(&original).unwrap();
    fixture.write_partial("chk_native_seal.caf.partial", bytes);

    let mut replay = fixture.request("chk_native_seal.caf.partial", bytes);
    replay.chunk_id = "chk_other_identity".into();
    replay.timing.chunk_id = replay.chunk_id.clone();
    replay.timing.source_sequence += 1;

    assert!(matches!(
        fixture.sealer().seal(&replay),
        Err(NativeChunkSealError::InvalidPath | NativeChunkSealError::Conflict)
    ));
    assert!(fixture.catalog.chunk(&replay.chunk_id).unwrap().is_none());
    assert!(
        fixture
            .catalog
            .capture_chunk_timing(&replay.chunk_id)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        fixture
            .catalog
            .connection()
            .query_row("SELECT COUNT(*) FROM chunks", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn native_chunk_seal_recovers_exact_orphan_after_rename_before_catalog_commit() {
    let fixture = SealFixture::new();
    let bytes = b"recoverable native orphan";
    fixture.write_partial("chk_native_seal.caf.partial", bytes);
    let request = fixture.request("chk_native_seal.caf.partial", bytes);
    fixture
        .catalog
        .execute_test_sql(
            "CREATE TRIGGER fail_capture_timing_crash_window
             BEFORE INSERT ON capture_chunk_timing
             BEGIN SELECT RAISE(ABORT, 'injected crash window'); END;",
        )
        .unwrap();
    let error = fixture
        .sealer()
        .with_fault_for_test(SealFault::Restore)
        .seal(&request)
        .unwrap_err();
    assert!(matches!(error, NativeChunkSealError::RecoveryRequired(_)));
    fixture
        .catalog
        .execute_test_sql("DROP TRIGGER fail_capture_timing_crash_window")
        .unwrap();

    let chunk = fixture.sealer().seal(&request).unwrap();

    assert_eq!(chunk.path, "audio/chk_native_seal.caf");
    assert_eq!(fixture.catalog.chunk(&chunk.id).unwrap(), Some(chunk));
    assert_eq!(
        fixture
            .catalog
            .capture_chunk_timing(&request.chunk_id)
            .unwrap(),
        Some(request.timing)
    );
}

#[test]
fn native_chunk_seal_intent_faults_do_not_rename_or_publish() {
    for fault in [
        SealFault::IntentWrite,
        SealFault::IntentFileSync,
        SealFault::IntentRename,
        SealFault::IntentSync,
    ] {
        let fixture = SealFixture::new();
        let bytes = b"intent fault native audio";
        fixture.write_partial("chk_native_seal.caf.partial", bytes);
        let request = fixture.request("chk_native_seal.caf.partial", bytes);

        assert!(
            fixture
                .sealer()
                .with_fault_for_test(fault)
                .seal(&request)
                .is_err()
        );
        assert!(fixture.catalog.chunk(&request.chunk_id).unwrap().is_none());
        assert!(fixture.staging.join("chk_native_seal.caf.partial").exists());
        assert!(
            !fixture
                .final_root
                .join("audio/chk_native_seal.caf")
                .exists()
        );
        assert!(fixture.intent_entries().is_empty());
        fixture.sealer().seal(&request).unwrap();
    }
}

#[test]
fn native_chunk_seal_cleans_strict_stale_intent_temp_before_retry() {
    for bytes in [b"".as_slice(), b"partial canonical intent".as_slice()] {
        let fixture = SealFixture::new();
        let intent_dir = fixture.final_root.join(".lifesub-native-intents");
        std::fs::create_dir(&intent_dir).unwrap();
        let stale = intent_dir.join(format!(".lifesub-native-intent-{}.tmp", "a".repeat(32)));
        std::fs::write(&stale, bytes).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&stale, std::fs::Permissions::from_mode(0o600)).unwrap();
        let audio = b"retry after stale intent temp";
        fixture.write_partial("chk_native_seal.caf.partial", audio);
        let request = fixture.request("chk_native_seal.caf.partial", audio);

        fixture.sealer().seal(&request).unwrap();

        assert!(!stale.exists());
        assert!(fixture.intent_entries().is_empty());
        assert!(fixture.catalog.chunk(&request.chunk_id).unwrap().is_some());
    }
}

#[test]
fn native_chunk_seal_rejects_orphan_recovery_with_different_identity_or_hash() {
    let fixture = SealFixture::new();
    let bytes = b"authenticated native orphan";
    std::fs::create_dir(fixture.final_root.join("audio")).unwrap();
    std::fs::write(fixture.final_root.join("audio/chk_native_seal.caf"), bytes).unwrap();

    let mut wrong_hash = fixture.request("chk_native_seal.caf.partial", b"different payload");
    wrong_hash.expected_byte_length = bytes.len() as u64;
    assert!(matches!(
        fixture.sealer().seal(&wrong_hash),
        Err(NativeChunkSealError::Conflict)
    ));

    let mut wrong_identity = fixture.request("chk_native_seal.caf.partial", bytes);
    wrong_identity.chunk_id = "chk_other_identity".into();
    wrong_identity.timing.chunk_id = wrong_identity.chunk_id.clone();
    assert!(matches!(
        fixture.sealer().seal(&wrong_identity),
        Err(NativeChunkSealError::InvalidPath | NativeChunkSealError::Conflict)
    ));
    assert!(fixture.catalog.chunk("chk_native_seal").unwrap().is_none());
    assert!(
        fixture
            .catalog
            .chunk("chk_other_identity")
            .unwrap()
            .is_none()
    );
}

#[test]
fn native_chunk_seal_cannot_claim_exact_bytes_without_a_durable_intent() {
    let fixture = SealFixture::new();
    let bytes = b"unknown native orphan";
    std::fs::create_dir(fixture.final_root.join("audio")).unwrap();
    std::fs::write(fixture.final_root.join("audio/chk_native_seal.caf"), bytes).unwrap();
    let request = fixture.request("chk_native_seal.caf.partial", bytes);

    assert!(matches!(
        fixture.sealer().seal(&request),
        Err(NativeChunkSealError::Conflict)
    ));
    assert!(fixture.catalog.chunk(&request.chunk_id).unwrap().is_none());
    assert!(!fixture.intent_path().exists());
}

#[test]
fn native_chunk_seal_intent_cannot_be_claimed_by_another_valid_session_and_source() {
    let fixture = SealFixture::new();
    let bytes = b"intent provenance belongs to one capture source";
    fixture.write_partial("chk_native_seal.caf.partial", bytes);
    let original = fixture.request("chk_native_seal.caf.partial", bytes);
    fixture
        .catalog
        .execute_test_sql(
            "CREATE TRIGGER fail_capture_timing_intent_owner
             BEFORE INSERT ON capture_chunk_timing
             BEGIN SELECT RAISE(ABORT, 'injected crash window'); END;",
        )
        .unwrap();
    assert!(matches!(
        fixture
            .sealer()
            .with_fault_for_test(SealFault::Restore)
            .seal(&original),
        Err(NativeChunkSealError::RecoveryRequired(_))
    ));
    fixture
        .catalog
        .execute_test_sql("DROP TRIGGER fail_capture_timing_intent_owner")
        .unwrap();

    let mut other_session = CaptureSession::new("other valid native source");
    other_session.state = CaptureState::Recording;
    fixture.catalog.insert_session(&other_session).unwrap();
    let other_source = CaptureSourceRecord {
        id: "src_other_valid_native".into(),
        session_id: other_session.id.clone(),
        source: AudioSource::SystemAudio,
        device_uid: "screen".into(),
        device_name: "System Audio".into(),
        channel_count: 2,
        sample_rate_hz: 48_000,
        created_at: chrono::Utc::now(),
    };
    fixture
        .catalog
        .insert_capture_source(&other_source)
        .unwrap();
    let mut claimant = original.clone();
    claimant.session_id = other_session.id;
    claimant.source = AudioSource::SystemAudio;
    claimant.timing.capture_source_id = other_source.id;
    claimant.timing.source_sequence += 1;

    assert!(matches!(
        fixture.sealer().seal(&claimant),
        Err(NativeChunkSealError::Conflict)
    ));
    assert!(fixture.catalog.chunk(&claimant.chunk_id).unwrap().is_none());
    assert!(
        fixture
            .catalog
            .capture_chunk_timing(&claimant.chunk_id)
            .unwrap()
            .is_none()
    );
}

#[test]
fn native_chunk_seal_rename_and_directory_sync_faults_do_not_publish() {
    for fault in [SealFault::Rename, SealFault::DirectorySync] {
        let fixture = SealFixture::new();
        let bytes = b"faulted native audio";
        fixture.write_partial("chk_native_seal.caf.partial", bytes);
        let request = fixture.request("chk_native_seal.caf.partial", bytes);
        let sealer = fixture.sealer().with_fault_for_test(fault);

        assert!(sealer.seal(&request).is_err());
        assert!(fixture.catalog.chunk(&request.chunk_id).unwrap().is_none());
        assert!(
            fixture
                .catalog
                .capture_chunk_timing(&request.chunk_id)
                .unwrap()
                .is_none()
        );
        assert!(fixture.staging.join("chk_native_seal.caf.partial").exists());
        assert!(
            !fixture
                .final_root
                .join("audio/chk_native_seal.caf")
                .exists()
        );
    }
}

#[test]
fn native_chunk_seal_catalog_failure_rolls_back_rows_and_restores_staging_file() {
    let fixture = SealFixture::new();
    let bytes = b"catalog failure native audio";
    fixture.write_partial("chk_native_seal.caf.partial", bytes);
    let request = fixture.request("chk_native_seal.caf.partial", bytes);
    fixture
        .catalog
        .execute_test_sql(
            "CREATE TRIGGER fail_capture_timing_insert
             BEFORE INSERT ON capture_chunk_timing
             BEGIN SELECT RAISE(ABORT, 'injected timing failure'); END;",
        )
        .unwrap();

    assert!(matches!(
        fixture.sealer().seal(&request),
        Err(NativeChunkSealError::Catalog(_))
    ));
    assert!(fixture.catalog.chunk(&request.chunk_id).unwrap().is_none());
    assert!(
        fixture
            .catalog
            .capture_chunk_timing(&request.chunk_id)
            .unwrap()
            .is_none()
    );
    assert!(fixture.staging.join("chk_native_seal.caf.partial").exists());
    assert!(
        !fixture
            .final_root
            .join("audio/chk_native_seal.caf")
            .exists()
    );
    assert!(!fixture.intent_path().exists());
}

#[test]
fn native_chunk_seal_reports_recovery_required_when_restore_fails() {
    let fixture = SealFixture::new();
    let bytes = b"restore failure native audio";
    fixture.write_partial("chk_native_seal.caf.partial", bytes);
    let request = fixture.request("chk_native_seal.caf.partial", bytes);
    fixture
        .catalog
        .execute_test_sql(
            "CREATE TRIGGER fail_capture_timing_restore
             BEFORE INSERT ON capture_chunk_timing
             BEGIN SELECT RAISE(ABORT, 'injected timing failure'); END;",
        )
        .unwrap();

    let error = fixture
        .sealer()
        .with_fault_for_test(SealFault::Restore)
        .seal(&request)
        .unwrap_err();

    assert!(matches!(error, NativeChunkSealError::RecoveryRequired(_)));
    assert!(fixture.catalog.chunk(&request.chunk_id).unwrap().is_none());
    assert!(!fixture.staging.join("chk_native_seal.caf.partial").exists());
    assert!(
        fixture
            .final_root
            .join("audio/chk_native_seal.caf")
            .exists()
    );
    assert!(fixture.intent_path().exists());
}

#[test]
fn native_chunk_seal_requires_matching_source_session_and_timing_chunk() {
    let fixture = SealFixture::new();
    let bytes = b"mismatched native audio";
    fixture.write_partial("chk_native_seal.caf.partial", bytes);

    let mut wrong_chunk = fixture.request("chk_native_seal.caf.partial", bytes);
    wrong_chunk.timing.chunk_id = "different_chunk".into();
    assert!(matches!(
        fixture.sealer().seal(&wrong_chunk),
        Err(NativeChunkSealError::Conflict)
    ));

    let mut wrong_source = fixture.request("chk_native_seal.caf.partial", bytes);
    wrong_source.source = AudioSource::SystemAudio;
    assert!(matches!(
        fixture.sealer().seal(&wrong_source),
        Err(NativeChunkSealError::Conflict)
    ));
    assert!(
        fixture
            .catalog
            .chunk(&wrong_source.chunk_id)
            .unwrap()
            .is_none()
    );
}
