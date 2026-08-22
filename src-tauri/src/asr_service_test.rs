use std::sync::Condvar;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, Mutex};
use std::time::Duration;

use chrono::{DateTime, Utc};
use rusqlite::Connection;
use tempfile::TempDir;

use crate::asr::job::{ClaimToken, Clock};
use crate::asr::manifest::model_registry;
use crate::asr::model_lookup::{ModelCapabilities, ModelLookup};
use crate::asr::provider::{ProviderOptions, ProviderSelection};
use crate::asr::service::{AsrEnqueueRequest, AsrService, EnqueueProviderFactory};
use crate::asr::settings::{AsrProviderOptions, AsrSettings, WhisperTask};
use crate::catalog::{PublicationError, PublicationFailurePoint};
use crate::domain::{
    AsrProviderKind, AudioSource, DataDestination, ProviderOutcome, ProviderReceipt,
    ProviderReceiptDraft, TranscriptSegmentPublication,
};
use crate::service::{CoreRuntime, RuntimeOwnershipError};

const NOW: &str = "2026-08-16T08:00:00.000Z";
const STARTED: &str = "2026-08-16T07:59:50.000Z";
const FINISHED: &str = "2026-08-16T07:59:59.000Z";
const SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const BUNDLE: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

#[derive(Clone)]
struct TestClock(Arc<Mutex<DateTime<Utc>>>);

impl TestClock {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(parse_time(NOW))))
    }
}

impl Clock for TestClock {
    fn now(&self) -> DateTime<Utc> {
        *self.0.lock().unwrap()
    }
}

struct Fixture {
    _temp: TempDir,
    data_dir: std::path::PathBuf,
    db_path: std::path::PathBuf,
    runtime: CoreRuntime,
    token: ClaimToken,
    clock: TestClock,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let data_dir = temp.path().join("data");
        let db_path = data_dir.join("lifesub.sqlite3");
        let runtime = CoreRuntime::initialize_with_boot_id(&data_dir, "boot-a").unwrap();
        let token = ClaimToken {
            job_id: "job-1".to_owned(),
            claimed_by: r#"{"boot_id":"boot-a","worker_id":"worker-1"}"#.to_owned(),
            claim_generation: 7,
        };
        let claimed_by = token.claimed_by.replace('\'', "''");
        runtime
            .job_repository_with_clock(TestClock::new())
            .execute_fixture_sql(&format!(
                "INSERT INTO sessions(id, title, state, started_at)
                 VALUES('session-1', 'atomic publication', 'stopped', '{NOW}');
                 INSERT INTO chunks(
                   id, session_id, source, path, sha256, byte_length,
                   session_offset_ms, duration_ms, integrity_state
                 ) VALUES('chunk-1', 'session-1', 'imported', 'audio.wav', '{SHA}', 10,
                          12000, 8000, 'available');
                 INSERT INTO asr_jobs(
                   id, session_id, chunk_id, provider, model_id, manifest_version,
                   archive_sha256, required_file_hashes_json, model_source_json,
                   parameters_json, input_sha256, fingerprint, state, attempt_count,
                   claim_generation, max_attempts, available_at, claimed_by,
                   lease_expires_at, created_at, updated_at
                 ) VALUES(
                   'job-1', 'session-1', 'chunk-1', 'whisper', 'whisper-small', '1',
                   '{BUNDLE}', '{{}}', '{{}}', '{{}}', '{SHA}', 'fingerprint-1',
                   'transcribing', 1, 7, 3, '{NOW}', '{claimed_by}',
                   '2026-08-16T08:00:30.000Z', '{NOW}', '{NOW}'
                 );"
            ))
            .unwrap();
        Self {
            _temp: temp,
            data_dir,
            db_path,
            runtime,
            token,
            clock: TestClock::new(),
        }
    }

    fn publish(
        &self,
    ) -> Result<crate::domain::TranscriptRevision, crate::catalog::PublicationError> {
        self.runtime
            .job_repository_with_clock(self.clock.clone())
            .publish(
                &self.token,
                &receipt(),
                &[TranscriptSegmentPublication {
                    id: "segment-1".to_owned(),
                    chunk_start_ms: 250,
                    chunk_end_ms: 1750,
                    source: AudioSource::Imported,
                    text: "atomic searchable transcript".to_owned(),
                }],
            )
    }

    fn counts(&self) -> (i64, i64, i64, i64, i64) {
        let connection = Connection::open(&self.db_path).unwrap();
        (
            count(&connection, "provider_receipts"),
            count(&connection, "revisions"),
            count(&connection, "revision_receipts"),
            count(&connection, "segments"),
            count(&connection, "segment_search"),
        )
    }

    fn job_state(&self) -> String {
        Connection::open(&self.db_path)
            .unwrap()
            .query_row("SELECT state FROM asr_jobs WHERE id = 'job-1'", [], |row| {
                row.get(0)
            })
            .unwrap()
    }
}

#[test]
fn publishes_receipt_revision_segments_search_and_success_in_one_transaction() {
    let fixture = Fixture::new();

    let revision = fixture.publish().unwrap();

    assert_eq!(revision.session_id, "session-1");
    assert_eq!(revision.number, 1);
    assert_eq!(revision.provider, "whisper");
    assert_eq!(revision.created_at, parse_time(NOW));
    assert_eq!(fixture.counts(), (1, 1, 1, 1, 1));
    assert_eq!(fixture.job_state(), "succeeded");
    let connection = Connection::open(&fixture.db_path).unwrap();
    let provenance = connection
        .query_row(
            "SELECT s.start_ms, s.end_ms, s.chunk_id, s.chunk_start_ms, s.chunk_end_ms,
                    s.session_start_ms, s.session_end_ms, r.created_at, j.updated_at,
                    p.started_at, p.finished_at
             FROM segments s
             JOIN revisions r ON r.id = s.revision_id
             JOIN revision_receipts rr ON rr.revision_id = r.id
             JOIN provider_receipts p ON p.id = rr.receipt_id
             JOIN asr_jobs j ON j.id = p.job_id",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, String>(10)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(
        provenance,
        (
            12250,
            13750,
            "chunk-1".to_owned(),
            250,
            1750,
            12250,
            13750,
            NOW.to_owned(),
            NOW.to_owned(),
            STARTED.to_owned(),
            FINISHED.to_owned(),
        )
    );
    assert_eq!(
        Connection::open(&fixture.db_path)
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM segment_search WHERE segment_search MATCH 'searchable'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        1
    );
}

#[test]
fn every_publication_write_failure_rolls_back_all_evidence_and_job_state() {
    for point in PublicationFailurePoint::ALL {
        let fixture = Fixture::new();
        fixture
            .runtime
            .job_repository_with_clock(fixture.clock.clone())
            .fail_publication_at(point);

        assert!(
            fixture.publish().is_err(),
            "{point:?} unexpectedly succeeded"
        );
        assert_eq!(
            fixture.counts(),
            (0, 0, 0, 0, 0),
            "partial data at {point:?}"
        );
        assert_eq!(
            fixture.job_state(),
            "transcribing",
            "job changed at {point:?}"
        );
    }
}

#[test]
fn cancellation_before_publication_produces_no_revision() {
    let fixture = Fixture::new();
    fixture
        .runtime
        .job_repository_with_clock(fixture.clock.clone())
        .execute_fixture_sql(&format!(
            "UPDATE asr_jobs SET cancel_requested_at = '{NOW}' WHERE id = 'job-1'"
        ))
        .unwrap();

    assert_eq!(
        fixture.publish(),
        Err(crate::catalog::PublicationError::Cancelled)
    );
    assert_eq!(fixture.counts(), (0, 0, 0, 0, 0));
    assert_eq!(fixture.job_state(), "transcribing");
}

#[test]
fn cancellation_after_commit_keeps_succeeded_revision() {
    let fixture = Fixture::new();
    let revision = fixture.publish().unwrap();

    fixture
        .runtime
        .job_repository_with_clock(fixture.clock.clone())
        .execute_fixture_sql(&format!(
            "UPDATE asr_jobs SET cancel_requested_at = '{NOW}'
             WHERE id = 'job-1' AND state IN ('queued', 'blocked_model', 'preparing', 'transcribing')"
        ))
        .unwrap();

    assert_eq!(fixture.job_state(), "succeeded");
    assert_eq!(fixture.counts(), (1, 1, 1, 1, 1));
    assert_eq!(revision.number, 1);
}

#[test]
fn stale_claim_generation_cannot_publish_receipt_or_revision() {
    let fixture = Fixture::new();
    let mut stale = fixture.token.clone();
    stale.claim_generation -= 1;

    assert_eq!(
        fixture
            .runtime
            .job_repository_with_clock(fixture.clock.clone())
            .publish(
                &stale,
                &receipt(),
                &[TranscriptSegmentPublication {
                    id: "segment-1".to_owned(),
                    chunk_start_ms: 0,
                    chunk_end_ms: 100,
                    source: AudioSource::Imported,
                    text: "stale".to_owned(),
                }],
            ),
        Err(crate::catalog::PublicationError::OwnershipLost)
    );
    assert_eq!(fixture.counts(), (0, 0, 0, 0, 0));
    assert_eq!(fixture.job_state(), "transcribing");
}

#[test]
fn preparing_job_cannot_publish_any_evidence() {
    let fixture = Fixture::new();
    fixture
        .runtime
        .job_repository_with_clock(fixture.clock.clone())
        .execute_fixture_sql("UPDATE asr_jobs SET state = 'preparing' WHERE id = 'job-1'")
        .unwrap();

    assert_eq!(
        fixture.publish(),
        Err(crate::catalog::PublicationError::OwnershipLost)
    );
    assert_eq!(fixture.counts(), (0, 0, 0, 0, 0));
    assert_eq!(fixture.job_state(), "preparing");
}

#[test]
fn expired_lease_cannot_publish_any_evidence() {
    let fixture = Fixture::new();
    fixture
        .runtime
        .job_repository_with_clock(fixture.clock.clone())
        .execute_fixture_sql(
            "UPDATE asr_jobs SET lease_expires_at = '2026-08-16T07:59:59.000Z'
             WHERE id = 'job-1'",
        )
        .unwrap();

    assert_eq!(fixture.publish(), Err(PublicationError::OwnershipLost));
    assert_eq!(fixture.counts(), (0, 0, 0, 0, 0));
    assert_eq!(fixture.job_state(), "transcribing");
}

#[test]
fn chunk_timestamp_overflow_rolls_back_all_evidence() {
    let fixture = Fixture::new();
    fixture
        .runtime
        .job_repository_with_clock(fixture.clock.clone())
        .execute_fixture_sql(&format!(
            "UPDATE chunks SET session_offset_ms = {} WHERE id = 'chunk-1'",
            i64::MAX
        ))
        .unwrap();

    assert_eq!(
        fixture.publish(),
        Err(PublicationError::InvalidResult(
            "segment timestamp overflow"
        ))
    );
    assert_eq!(fixture.counts(), (0, 0, 0, 0, 0));
    assert_eq!(fixture.job_state(), "transcribing");
}

#[test]
fn segment_source_mismatch_rolls_back_all_evidence() {
    let fixture = Fixture::new();
    let result = fixture
        .runtime
        .job_repository_with_clock(fixture.clock.clone())
        .publish(
            &fixture.token,
            &receipt(),
            &[TranscriptSegmentPublication {
                id: "segment-1".to_owned(),
                chunk_start_ms: 0,
                chunk_end_ms: 100,
                source: AudioSource::Microphone,
                text: "wrong source".to_owned(),
            }],
        );

    assert_eq!(
        result,
        Err(PublicationError::InvalidResult("segment source mismatch"))
    );
    assert_eq!(fixture.counts(), (0, 0, 0, 0, 0));
    assert_eq!(fixture.job_state(), "transcribing");
}

#[test]
fn foreign_catalog_capability_cannot_publish() {
    let catalog_owner = Fixture::new();
    let guard_owner = Fixture::new();
    let jobs = guard_owner
        .runtime
        .job_repository_for_foreign_core_with_clock(
            &catalog_owner.runtime,
            guard_owner.clock.clone(),
        );

    assert_eq!(
        jobs.publish(&catalog_owner.token, &receipt(), &valid_segments()),
        Err(PublicationError::Ownership(
            RuntimeOwnershipError::CatalogMismatch
        ))
    );
    assert_eq!(catalog_owner.counts(), (0, 0, 0, 0, 0));
    assert_eq!(catalog_owner.job_state(), "transcribing");
}

#[cfg(unix)]
#[test]
fn invalidated_owner_cannot_publish() {
    let fixture = Fixture::new();
    let lock_path = fixture.data_dir.join("asr-worker.lock");
    std::fs::rename(&lock_path, fixture.data_dir.join("old-lock")).unwrap();
    std::fs::write(&lock_path, b"replacement").unwrap();

    assert_eq!(
        fixture.publish(),
        Err(PublicationError::Ownership(
            RuntimeOwnershipError::UnsafePath
        ))
    );
    assert_eq!(fixture.counts(), (0, 0, 0, 0, 0));
    assert_eq!(fixture.job_state(), "transcribing");
}

fn valid_segments() -> Vec<TranscriptSegmentPublication> {
    vec![TranscriptSegmentPublication {
        id: "segment-1".to_owned(),
        chunk_start_ms: 250,
        chunk_end_ms: 1750,
        source: AudioSource::Imported,
        text: "atomic searchable transcript".to_owned(),
    }]
}

fn receipt() -> ProviderReceipt {
    ProviderReceipt::try_from(ProviderReceiptDraft {
        job_id: "job-1".to_owned(),
        chunk_id: "chunk-1".to_owned(),
        provider: AsrProviderKind::Whisper,
        model_id: "whisper-small".to_owned(),
        manifest_version: "1".to_owned(),
        archive_sha256: BUNDLE.to_owned(),
        required_file_hashes_json: "{}".to_owned(),
        model_source_json: "{}".to_owned(),
        vad_model_id: None,
        vad_manifest_version: None,
        vad_archive_sha256: None,
        vad_required_file_hashes_json: None,
        runtime_version: "1.13.5".to_owned(),
        runtime_build_id: "runtime-build-1".to_owned(),
        parameters_json: "{}".to_owned(),
        input_sha256: SHA.to_owned(),
        started_at: parse_time(STARTED),
        finished_at: parse_time(FINISHED),
        data_destination: DataDestination::LocalDevice,
        outcome: ProviderOutcome::Succeeded,
    })
    .unwrap()
}

fn count(connection: &Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap()
}

fn parse_time(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .unwrap()
        .with_timezone(&Utc)
}

const VAD_MODEL: &str = "silero-vad-2024-01-17";

#[derive(Clone)]
struct LookupFixture {
    asr: ModelCapabilities,
    vad: ModelCapabilities,
}

impl ModelLookup for LookupFixture {
    fn lookup(&self, model_id: &str) -> Option<ModelCapabilities> {
        match model_id {
            "whisper-small" => Some(self.asr.clone()),
            VAD_MODEL => Some(self.vad.clone()),
            _ => None,
        }
    }
}

#[derive(Clone, Default)]
struct FactorySpy(Arc<AtomicUsize>);

impl FactorySpy {
    fn calls(&self) -> usize {
        self.0.load(Ordering::SeqCst)
    }
}

impl EnqueueProviderFactory for FactorySpy {
    fn validate_constructible(
        &self,
        _settings: &AsrSettings,
        _selection: &ProviderSelection,
    ) -> Result<(), crate::domain::AsrErrorCode> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

struct EnqueueFixture {
    _temp: TempDir,
    db_path: std::path::PathBuf,
    runtime: Arc<CoreRuntime>,
    clock: TestClock,
}

impl EnqueueFixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let data_dir = temp.path().join("data");
        let db_path = data_dir.join("lifesub.sqlite3");
        let runtime =
            Arc::new(CoreRuntime::initialize_with_boot_id(&data_dir, "boot-enqueue").unwrap());
        runtime
            .job_repository_with_clock(TestClock::new())
            .execute_fixture_sql(&format!(
                "INSERT INTO sessions(id, title, state, started_at)
                 VALUES('session-enqueue', 'enqueue', 'stopped', '{NOW}');
                 INSERT INTO chunks(
                   id, session_id, source, path, sha256, byte_length,
                   session_offset_ms, duration_ms, integrity_state
                 ) VALUES('chunk-enqueue', 'session-enqueue', 'imported', 'audio.wav', '{SHA}', 10,
                          0, 8000, 'available');"
            ))
            .unwrap();
        Self {
            _temp: temp,
            db_path,
            runtime,
            clock: TestClock::new(),
        }
    }

    fn job_count(&self) -> i64 {
        count(&Connection::open(&self.db_path).unwrap(), "asr_jobs")
    }

    fn execute(&self, sql: &str) {
        self.runtime
            .job_repository_with_clock(self.clock.clone())
            .execute_fixture_sql(sql)
            .unwrap();
    }

    fn service<'a>(
        &'a self,
        lookup: &'a LookupFixture,
        factory: &'a FactorySpy,
    ) -> AsrService<'a, LookupFixture, FactorySpy, TestClock> {
        self.runtime
            .asr_service_with_clock(self.clock.clone(), lookup, factory)
    }
}

fn executable_lookup() -> LookupFixture {
    LookupFixture {
        asr: ModelCapabilities::new(AsrProviderKind::Whisper, ["auto", "en"], true, true, true),
        vad: ModelCapabilities::new(AsrProviderKind::Whisper, ["auto"], true, true, true),
    }
}

fn enqueue_request() -> AsrEnqueueRequest {
    let settings = AsrSettings::whisper("whisper-small").with_num_threads(1);
    AsrEnqueueRequest {
        session_id: "session-enqueue".to_owned(),
        chunk_id: "chunk-enqueue".to_owned(),
        input_sha256: SHA.to_owned(),
        selection: ProviderSelection::new(
            "auto",
            1,
            ProviderOptions::Whisper {
                task: WhisperTask::Transcribe,
            },
        ),
        settings,
        vad_model_id: Some(VAD_MODEL.to_owned()),
    }
}

#[test]
fn enqueue_rejects_unexecutable_asr_before_job_insert_or_provider_construction() {
    let fixture = EnqueueFixture::new();
    let mut lookup = executable_lookup();
    lookup.asr.executable = false;
    lookup.asr.reason_code = Some("model_runtime_unqualified".to_owned());
    let factory = FactorySpy::default();

    let result = fixture
        .service(&lookup, &factory)
        .enqueue(enqueue_request());

    assert_eq!(
        result,
        Err(crate::domain::AsrErrorCode::ModelCapabilityUnavailable)
    );
    assert_eq!(fixture.job_count(), 0);
    assert_eq!(factory.calls(), 0);
}

#[test]
fn enqueue_rejects_unexecutable_vad_before_job_insert_or_provider_construction() {
    let fixture = EnqueueFixture::new();
    let mut lookup = executable_lookup();
    lookup.vad.executable = false;
    let factory = FactorySpy::default();

    let result = fixture
        .service(&lookup, &factory)
        .enqueue(enqueue_request());

    assert_eq!(
        result,
        Err(crate::domain::AsrErrorCode::ModelCapabilityUnavailable)
    );
    assert_eq!(fixture.job_count(), 0);
    assert_eq!(factory.calls(), 0);
}

#[test]
fn enqueue_rejects_provider_model_options_language_and_thread_mismatches_stably() {
    let fixture = EnqueueFixture::new();
    let factory = FactorySpy::default();
    let lookup = executable_lookup();
    let mut cases = Vec::new();

    let mut provider = enqueue_request();
    provider.settings.provider = AsrProviderKind::SenseVoice;
    cases.push(provider);
    let mut options = enqueue_request();
    options.settings.options = AsrProviderOptions::SenseVoice { use_itn: true };
    cases.push(options);
    let mut language = enqueue_request();
    language.selection.language = "en".to_owned();
    cases.push(language);
    let mut threads = enqueue_request();
    threads.selection.num_threads = 2;
    cases.push(threads);
    let mut selected_options = enqueue_request();
    selected_options.selection.options = ProviderOptions::Whisper {
        task: WhisperTask::Translate,
    };
    cases.push(selected_options);

    for request in cases {
        assert_eq!(
            fixture.service(&lookup, &factory).enqueue(request),
            Err(crate::domain::AsrErrorCode::InvalidProviderParameter)
        );
    }
    assert_eq!(fixture.job_count(), 0);
    assert_eq!(factory.calls(), 0);
}

#[test]
fn enqueue_rejects_chunk_session_hash_or_integrity_mismatch_before_factory_or_insert() {
    for (mutation, expected) in [
        (
            "UPDATE chunks SET session_id = 'other-session' WHERE id = 'chunk-enqueue'",
            crate::domain::AsrErrorCode::InputIntegrityFailed,
        ),
        (
            "UPDATE chunks SET sha256 = 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb' WHERE id = 'chunk-enqueue'",
            crate::domain::AsrErrorCode::InputIntegrityFailed,
        ),
        (
            "UPDATE chunks SET integrity_state = 'missing' WHERE id = 'chunk-enqueue'",
            crate::domain::AsrErrorCode::InputUnavailable,
        ),
    ] {
        let fixture = EnqueueFixture::new();
        if mutation.contains("other-session") {
            fixture.execute(&format!(
                "INSERT INTO sessions(id, title, state, started_at)
                 VALUES('other-session', 'other', 'stopped', '{NOW}')"
            ));
        }
        fixture.execute(mutation);
        let lookup = executable_lookup();
        let factory = FactorySpy::default();

        assert_eq!(
            fixture
                .service(&lookup, &factory)
                .enqueue(enqueue_request()),
            Err(expected)
        );
        assert_eq!(fixture.job_count(), 0);
        assert_eq!(factory.calls(), 0);
    }
}

#[test]
fn enqueue_inserts_once_and_returns_existing_active_job_for_duplicate_fingerprint() {
    let fixture = EnqueueFixture::new();
    let lookup = executable_lookup();
    let factory = FactorySpy::default();
    let service = fixture.service(&lookup, &factory);

    let first = service.enqueue(enqueue_request()).unwrap();
    let duplicate = service.enqueue(enqueue_request()).unwrap();

    assert!(first.inserted);
    assert!(!duplicate.inserted);
    assert_eq!(duplicate.job_id, first.job_id);
    assert_eq!(fixture.job_count(), 1);
    assert_eq!(factory.calls(), 1);
}

#[derive(Clone, Copy)]
struct ExecutableRegistryLookup;

impl ModelLookup for ExecutableRegistryLookup {
    fn lookup(&self, model_id: &str) -> Option<ModelCapabilities> {
        if model_id == VAD_MODEL {
            return Some(ModelCapabilities::new(
                AsrProviderKind::Whisper,
                ["auto"],
                true,
                true,
                true,
            ));
        }
        let manifest = model_registry().model(model_id)?;
        Some(ModelCapabilities::new(
            manifest.provider,
            manifest.supported_languages,
            true,
            true,
            true,
        ))
    }
}

fn request_for(settings: AsrSettings) -> AsrEnqueueRequest {
    AsrEnqueueRequest {
        session_id: "session-enqueue".to_owned(),
        chunk_id: "chunk-enqueue".to_owned(),
        input_sha256: SHA.to_owned(),
        selection: ProviderSelection::new(
            settings.language.as_str(),
            settings.num_threads,
            settings.options.clone(),
        ),
        settings,
        vad_model_id: Some(VAD_MODEL.to_owned()),
    }
}

#[test]
fn enqueue_persists_canonical_required_install_files_for_archive_direct_and_vad() {
    let fixture = EnqueueFixture::new();
    let lookup = ExecutableRegistryLookup;
    let factory = FactorySpy::default();
    let service = fixture
        .runtime
        .asr_service_with_clock(fixture.clock.clone(), &lookup, &factory);

    service
        .enqueue(request_for(
            AsrSettings::sense_voice("sense-voice-small-int8-2024-07-17").with_num_threads(1),
        ))
        .unwrap();
    service
        .enqueue(request_for(
            AsrSettings::qwen3_asr("qwen3-asr-1.7b").with_num_threads(1),
        ))
        .unwrap();

    let connection = Connection::open(&fixture.db_path).unwrap();
    let archive: String = connection
        .query_row(
            "SELECT required_file_hashes_json FROM asr_jobs
             WHERE model_id = 'sense-voice-small-int8-2024-07-17'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        archive,
        r#"[{"bytes":239233841,"path":"model.int8.onnx","sha256":"c71f0ce00bec95b07744e116345e33d8cbbe08cef896382cf907bf4b51a2cd51"},{"bytes":228908,"path":"test_wavs/en.wav","sha256":"eb1eb008904465b74c304aad8342e8c7d3c6e61ffe9f66adcaca9cf0f76a93f4"},{"bytes":230444,"path":"test_wavs/ja.wav","sha256":"460bd8dccb0d2a5f4e29c628f837be4082d13defc64c3fc21dd1b6bb0e119095"},{"bytes":147500,"path":"test_wavs/ko.wav","sha256":"0dc797a5c81ed30fc339d91f3da718ab02854e17ffa37cb93c4c039ac5c6bb9c"},{"bytes":164780,"path":"test_wavs/yue.wav","sha256":"0960b2db54ae202071d250e6462fbf74a3c863f0e3e7f01273e4939c996875a0"},{"bytes":178988,"path":"test_wavs/zh.wav","sha256":"b77f1794fe374a0ba1ee1dc458bfaf9349496cbbfc32780c50ba3c5a7ad8e373"},{"bytes":315894,"path":"tokens.txt","sha256":"f449eb28dc567533d7fa59be34e2abca8784f771850c78a47fb731a31429a1dc"}]"#
    );
    let (direct, vad, source): (String, String, String) = connection
        .query_row(
            "SELECT required_file_hashes_json, vad_required_file_hashes_json, model_source_json
             FROM asr_jobs WHERE model_id = 'qwen3-asr-1.7b'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        direct,
        r#"[{"bytes":6194,"path":"config.json","sha256":"2e74a751548b8ad7d7526d29365ad8144c345d8b412b1152d25dc6698452712f"},{"bytes":4220320824,"path":"model-00001-of-00002.safetensors","sha256":"a4cd1f1a04d90b757dc7f7dd26254e69a013b19e80efe590a83c6a3bde8608d6"},{"bytes":478200688,"path":"model-00002-of-00002.safetensors","sha256":"6e0b9d9e09e2e0238e7ef3cc8a484ab387e91b90f1900bedf88bc92d7929ccfc"},{"bytes":64821,"path":"model.safetensors.index.json","sha256":"f994739fe38e5210b9e3e8ce6c6307315e2ceac3cb630e7b7414d69dce520f60"},{"bytes":11429653,"path":"tokenizer.json","sha256":"fe1fad59be22a41ee293363fcf95fdedbc7c93f3b49270b1d2e18bd1399a7a05"}]"#
    );
    assert_eq!(
        vad,
        r#"[{"bytes":643854,"path":"silero_vad.onnx","sha256":"9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6"}]"#
    );
    let source: serde_json::Value = serde_json::from_str(&source).unwrap();
    assert_eq!(source["bundle"]["artifacts"].as_array().unwrap().len(), 5);
    assert!(source["bundle"]["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|artifact| artifact["resolved_url"]
            == "https://huggingface.co/Qwen/Qwen3-ASR-1.7B-hf/resolve/bcd2b5b7f32b480ab5790554cfa8347f246a14f3/tokenizer.json"));
}

#[derive(Clone, Default)]
struct SlowFactorySpy(Arc<AtomicUsize>);

impl EnqueueProviderFactory for SlowFactorySpy {
    fn validate_constructible(
        &self,
        _settings: &AsrSettings,
        _selection: &ProviderSelection,
    ) -> Result<(), crate::domain::AsrErrorCode> {
        self.0.fetch_add(1, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(100));
        Ok(())
    }
}

#[test]
fn concurrent_duplicate_enqueue_has_one_job_id_and_one_factory_winner() {
    let fixture = EnqueueFixture::new();
    let lookup = executable_lookup();
    let factory = SlowFactorySpy::default();
    let start = Barrier::new(2);

    let outcomes = std::thread::scope(|scope| {
        let left = scope.spawn(|| {
            let service =
                fixture
                    .runtime
                    .asr_service_with_clock(fixture.clock.clone(), &lookup, &factory);
            start.wait();
            service.enqueue(enqueue_request()).unwrap()
        });
        let right = scope.spawn(|| {
            let service =
                fixture
                    .runtime
                    .asr_service_with_clock(fixture.clock.clone(), &lookup, &factory);
            start.wait();
            service.enqueue(enqueue_request()).unwrap()
        });
        [left.join().unwrap(), right.join().unwrap()]
    });

    assert_eq!(outcomes[0].job_id, outcomes[1].job_id);
    assert_eq!(
        outcomes.iter().filter(|outcome| outcome.inserted).count(),
        1
    );
    assert_eq!(fixture.job_count(), 1);
    assert_eq!(factory.0.load(Ordering::SeqCst), 1);
}

#[derive(Clone, Default)]
struct FailOnceFactorySpy(Arc<AtomicUsize>);

impl EnqueueProviderFactory for FailOnceFactorySpy {
    fn validate_constructible(
        &self,
        _settings: &AsrSettings,
        _selection: &ProviderSelection,
    ) -> Result<(), crate::domain::AsrErrorCode> {
        if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
            Err(crate::domain::AsrErrorCode::ProviderInitializationFailed)
        } else {
            Ok(())
        }
    }
}

#[test]
fn failed_factory_winner_rolls_back_reservation_and_allows_retry() {
    let fixture = EnqueueFixture::new();
    let lookup = executable_lookup();
    let factory = FailOnceFactorySpy::default();
    let service = fixture
        .runtime
        .asr_service_with_clock(fixture.clock.clone(), &lookup, &factory);

    assert_eq!(
        service.enqueue(enqueue_request()),
        Err(crate::domain::AsrErrorCode::ProviderInitializationFailed)
    );
    assert_eq!(fixture.job_count(), 0);

    let retried = service.enqueue(enqueue_request()).unwrap();
    assert!(retried.inserted);
    assert_eq!(fixture.job_count(), 1);
    assert_eq!(factory.0.load(Ordering::SeqCst), 2);
}

#[derive(Clone, Default)]
struct BlockingFactory {
    state: Arc<(Mutex<(bool, bool)>, Condvar)>,
}

impl BlockingFactory {
    fn wait_until_entered(&self) {
        let (lock, changed) = &*self.state;
        let mut state = lock.lock().unwrap();
        while !state.0 {
            state = changed.wait(state).unwrap();
        }
    }

    fn release(&self) {
        let (lock, changed) = &*self.state;
        lock.lock().unwrap().1 = true;
        changed.notify_all();
    }
}

impl EnqueueProviderFactory for BlockingFactory {
    fn validate_constructible(
        &self,
        _settings: &AsrSettings,
        _selection: &ProviderSelection,
    ) -> Result<(), crate::domain::AsrErrorCode> {
        let (lock, changed) = &*self.state;
        let mut state = lock.lock().unwrap();
        state.0 = true;
        changed.notify_all();
        while !state.1 {
            state = changed.wait(state).unwrap();
        }
        Ok(())
    }
}

#[test]
fn slow_factory_does_not_hold_the_catalog_connection_lock() {
    let fixture = EnqueueFixture::new();
    fixture.execute(&format!(
        "INSERT INTO asr_jobs(
           id, session_id, chunk_id, provider, model_id, manifest_version, archive_sha256,
           required_file_hashes_json, model_source_json, parameters_json, input_sha256,
           fingerprint, state, attempt_count, claim_generation, max_attempts, available_at,
           created_at, updated_at
         ) VALUES(
           'cancel-target', 'session-enqueue', 'chunk-enqueue', 'whisper', 'whisper-small', '1',
           '{BUNDLE}', '[]', '{{}}', '{{}}', '{SHA}', 'cancel-target-fingerprint', 'queued',
           0, 0, 3, '{NOW}', '{NOW}', '{NOW}'
         )"
    ));
    let runtime = fixture.runtime.clone();
    let factory = BlockingFactory::default();
    let worker_factory = factory.clone();
    let worker_runtime = runtime.clone();
    let enqueue = std::thread::spawn(move || {
        let lookup = executable_lookup();
        worker_runtime
            .asr_service_with_clock(TestClock::new(), &lookup, &worker_factory)
            .enqueue(enqueue_request())
    });
    factory.wait_until_entered();

    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let catalog_runtime = runtime.clone();
    let catalog_access = std::thread::spawn(move || {
        let result = catalog_runtime
            .job_repository_with_clock(TestClock::new())
            .execute_fixture_sql("SELECT 1;");
        let cancel = catalog_runtime
            .job_control_with_clock(TestClock::new())
            .request_cancel("cancel-target");
        done_tx.send((result, cancel)).unwrap();
    });
    let catalog_was_available = done_rx
        .recv_timeout(Duration::from_millis(200))
        .is_ok_and(|(read, cancel)| read.is_ok() && cancel.is_ok());
    factory.release();

    enqueue.join().unwrap().unwrap();
    catalog_access.join().unwrap();
    assert!(catalog_was_available);
}

#[test]
fn enqueue_transaction_revalidates_chunk_after_factory_before_insert() {
    for (mutation_sql, expected) in [
        (
            "UPDATE chunks SET integrity_state = 'missing' WHERE id = 'chunk-enqueue'",
            crate::domain::AsrErrorCode::InputUnavailable,
        ),
        (
            "UPDATE chunks SET integrity_state = 'corrupted' WHERE id = 'chunk-enqueue'",
            crate::domain::AsrErrorCode::InputUnavailable,
        ),
        (
            "UPDATE chunks SET sha256 = 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb' WHERE id = 'chunk-enqueue'",
            crate::domain::AsrErrorCode::InputIntegrityFailed,
        ),
        (
            "UPDATE chunks SET session_id = 'other-session' WHERE id = 'chunk-enqueue'",
            crate::domain::AsrErrorCode::InputIntegrityFailed,
        ),
    ] {
        let fixture = EnqueueFixture::new();
        if mutation_sql.contains("other-session") {
            fixture.execute(&format!(
                "INSERT INTO sessions(id, title, state, started_at)
                 VALUES('other-session', 'other', 'stopped', '{NOW}')"
            ));
        }
        let runtime = fixture.runtime.clone();
        let factory = BlockingFactory::default();
        let worker_factory = factory.clone();
        let worker_runtime = runtime.clone();
        let enqueue = std::thread::spawn(move || {
            let lookup = executable_lookup();
            worker_runtime
                .asr_service_with_clock(TestClock::new(), &lookup, &worker_factory)
                .enqueue(enqueue_request())
        });
        factory.wait_until_entered();

        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let mutation_runtime = runtime.clone();
        let mutation = std::thread::spawn(move || {
            mutation_runtime
                .job_repository_with_clock(TestClock::new())
                .execute_fixture_sql(mutation_sql)
                .unwrap();
            done_tx.send(()).unwrap();
        });
        let mutation_completed_before_release =
            done_rx.recv_timeout(Duration::from_millis(200)).is_ok();
        factory.release();

        let result = enqueue.join().unwrap();
        mutation.join().unwrap();
        assert!(mutation_completed_before_release);
        assert_eq!(result, Err(expected));
        assert_eq!(fixture.job_count(), 0);
    }
}

#[derive(Clone)]
struct CatalogTouchFactory {
    runtime: Arc<CoreRuntime>,
}

impl EnqueueProviderFactory for CatalogTouchFactory {
    fn validate_constructible(
        &self,
        _settings: &AsrSettings,
        _selection: &ProviderSelection,
    ) -> Result<(), crate::domain::AsrErrorCode> {
        self.runtime
            .job_repository_with_clock(TestClock::new())
            .execute_fixture_sql("SELECT 1;")
            .map_err(|_| crate::domain::AsrErrorCode::RecoveryRequired)
    }
}

#[test]
fn factory_can_reenter_catalog_without_deadlocking() {
    let fixture = EnqueueFixture::new();
    let lookup = executable_lookup();
    let factory = CatalogTouchFactory {
        runtime: fixture.runtime.clone(),
    };
    let service = fixture
        .runtime
        .asr_service_with_clock(fixture.clock.clone(), &lookup, &factory);
    assert!(service.enqueue(enqueue_request()).is_ok());
}

#[derive(Clone, Default)]
struct PanicOnceFactory(Arc<AtomicUsize>);

impl EnqueueProviderFactory for PanicOnceFactory {
    fn validate_constructible(
        &self,
        _settings: &AsrSettings,
        _selection: &ProviderSelection,
    ) -> Result<(), crate::domain::AsrErrorCode> {
        if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
            panic!("factory panic fixture");
        }
        Ok(())
    }
}

#[test]
fn panicking_factory_releases_single_flight_reservation_without_poisoning() {
    let fixture = EnqueueFixture::new();
    let lookup = executable_lookup();
    let factory = PanicOnceFactory::default();
    let service = fixture
        .runtime
        .asr_service_with_clock(fixture.clock.clone(), &lookup, &factory);

    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        service.enqueue(enqueue_request())
    }));
    assert!(panic.is_err());
    assert_eq!(fixture.job_count(), 0);

    assert!(service.enqueue(enqueue_request()).unwrap().inserted);
    assert_eq!(fixture.job_count(), 1);
}
