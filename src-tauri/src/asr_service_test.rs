use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use rusqlite::Connection;
use tempfile::TempDir;

use crate::asr::job::{ClaimToken, Clock};
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
