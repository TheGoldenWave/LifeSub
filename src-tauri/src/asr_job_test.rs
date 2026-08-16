use std::fs;
use std::sync::{Arc, Barrier, Mutex};
use std::thread;

use chrono::{DateTime, Duration, Utc};
use rusqlite::{Connection, params};
use tempfile::TempDir;

use crate::asr::job::{
    CancellationOutcome, Clock, JobError, JobRepository, ModelReadiness, RecoveryOutcome,
};
use crate::catalog::Catalog;
use crate::domain::AsrErrorCode;
use crate::service::{RuntimeOwnershipError, RuntimeOwnershipGuard};

const NOW: &str = "2026-08-16T08:00:00.000Z";
const SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const BUNDLE: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const RUNTIME: &str = r#"{"build_id":"runtime-1","version":"1.13.5"}"#;

#[derive(Clone)]
struct TestClock(Arc<Mutex<DateTime<Utc>>>);

impl TestClock {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(
            DateTime::parse_from_rfc3339(NOW)
                .unwrap()
                .with_timezone(&Utc),
        )))
    }

    fn advance(&self, duration: Duration) {
        let mut now = self.0.lock().unwrap();
        *now += duration;
    }
}

impl Clock for TestClock {
    fn now(&self) -> DateTime<Utc> {
        *self.0.lock().unwrap()
    }
}

struct Fixture {
    _parent: TempDir,
    data_dir: std::path::PathBuf,
    db_path: std::path::PathBuf,
    catalog: Catalog,
    ownership: RuntimeOwnershipGuard,
    clock: TestClock,
}

impl Fixture {
    fn new() -> Self {
        let parent = tempfile::tempdir().unwrap();
        let data_dir = parent.path().join("data");
        fs::create_dir(&data_dir).unwrap();
        let ownership = RuntimeOwnershipGuard::acquire(&data_dir).unwrap();
        let db_path = data_dir.join("lifesub.sqlite3");
        let catalog = Catalog::open(&db_path).unwrap();
        Self {
            _parent: parent,
            data_dir,
            db_path,
            catalog,
            ownership,
            clock: TestClock::new(),
        }
    }

    fn repository(&self, boot_id: &str) -> JobRepository<'_, TestClock> {
        JobRepository::new(&self.catalog, &self.ownership, boot_id, self.clock.clone())
    }

    fn insert_job(&self, id: &str, state: &str, integrity: &str, available_at: &str) {
        let connection = Connection::open(&self.db_path).unwrap();
        connection
            .pragma_update(None, "foreign_keys", true)
            .unwrap();
        connection
            .execute(
                "INSERT OR IGNORE INTO sessions(id, title, state, started_at)
                 VALUES('session-1', 'jobs', 'stopped', ?1)",
                [NOW],
            )
            .unwrap();
        connection
            .execute(
                "INSERT OR IGNORE INTO chunks(
                   id, session_id, source, path, sha256, byte_length, integrity_state
                 ) VALUES(?1, 'session-1', 'imported', ?2, ?3, 8, ?4)",
                params![format!("chunk-{id}"), format!("{id}.wav"), SHA, integrity],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO asr_jobs(
                   id, session_id, chunk_id, provider, model_id, manifest_version,
                   archive_sha256, required_file_hashes_json, model_source_json,
                   parameters_json, input_sha256, fingerprint, state, max_attempts,
                   available_at, created_at, updated_at
                 ) VALUES(
                   ?1, 'session-1', ?2, 'whisper', 'whisper-small', '1', ?3,
                   '{}', '{}', '{}', ?4, ?5, ?6, 3, ?7, ?8, ?8
                 )",
                params![
                    id,
                    format!("chunk-{id}"),
                    BUNDLE,
                    SHA,
                    format!("fingerprint-{id}"),
                    state,
                    available_at,
                    NOW,
                ],
            )
            .unwrap();
    }

    fn row(&self, id: &str) -> JobRow {
        Connection::open(&self.db_path)
            .unwrap()
            .query_row(
                "SELECT state, attempt_count, claim_generation, available_at, claimed_by,
                        lease_expires_at, cancel_requested_at, error_code, fingerprint,
                        parameters_json, updated_at
                 FROM asr_jobs WHERE id = ?1",
                [id],
                |row| {
                    Ok(JobRow {
                        state: row.get(0)?,
                        attempt_count: row.get(1)?,
                        claim_generation: row.get(2)?,
                        available_at: row.get(3)?,
                        claimed_by: row.get(4)?,
                        lease_expires_at: row.get(5)?,
                        cancel_requested_at: row.get(6)?,
                        error_code: row.get(7)?,
                        fingerprint: row.get(8)?,
                        parameters_json: row.get(9)?,
                        updated_at: row.get(10)?,
                    })
                },
            )
            .unwrap()
    }

    fn install_ready_model(&self) {
        Connection::open(&self.db_path)
            .unwrap()
            .execute(
                "INSERT INTO model_installations(
                   model_id, provider, manifest_version, archive_sha256, install_dir, state,
                   installed_at, runtime_identity_json, qualified_at
                 ) VALUES(
                   'whisper-small', 'whisper', '1', ?1, '/models/whisper-small',
                   'runtime_qualified', ?2, ?3, ?2
                 )",
                params![BUNDLE, NOW, RUNTIME],
            )
            .unwrap();
    }
}

struct JobRow {
    state: String,
    attempt_count: i64,
    claim_generation: i64,
    available_at: String,
    claimed_by: Option<String>,
    lease_expires_at: Option<String>,
    cancel_requested_at: Option<String>,
    error_code: Option<String>,
    fingerprint: String,
    parameters_json: String,
    updated_at: String,
}

fn ready_model() -> ModelReadiness {
    ModelReadiness {
        provider: "whisper".to_owned(),
        model_id: "whisper-small".to_owned(),
        manifest_version: "1".to_owned(),
        bundle_identity: BUNDLE.to_owned(),
        runtime_identity_json: RUNTIME.to_owned(),
    }
}

fn assert_millis(timestamp: &str) {
    assert!(
        timestamp.ends_with(".000Z"),
        "non-canonical timestamp: {timestamp}"
    );
    DateTime::parse_from_rfc3339(timestamp).unwrap();
}

#[test]
fn claim_is_atomic_fenced_ordered_and_requires_available_input() {
    let fixture = Fixture::new();
    fixture.insert_job("future", "queued", "available", "2026-08-16T08:00:01.000Z");
    fixture.insert_job("missing", "queued", "missing", "2026-08-16T07:59:00.000Z");
    fixture.insert_job(
        "corrupted",
        "queued",
        "corrupted",
        "2026-08-16T07:59:15.000Z",
    );
    fixture.insert_job("first", "queued", "available", "2026-08-16T07:59:30.000Z");

    let claim = fixture
        .repository("boot-a")
        .claim("worker-1")
        .unwrap()
        .unwrap();

    assert_eq!(claim.id, "first");
    assert_eq!(claim.attempt_count, 1);
    assert_eq!(claim.token.claim_generation, 1);
    assert_eq!(
        claim
            .renew_at
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        "2026-08-16T08:00:05.000Z"
    );
    assert_eq!(
        claim
            .lease_expires_at
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        "2026-08-16T08:00:30.000Z"
    );
    let row = fixture.row("first");
    assert_eq!(row.state, "preparing");
    assert_eq!(row.attempt_count, 1);
    assert_eq!(row.claim_generation, 1);
    assert_eq!(
        row.claimed_by.as_deref(),
        Some(claim.token.claimed_by.as_str())
    );
    assert_millis(row.lease_expires_at.as_deref().unwrap());
    assert_millis(&row.updated_at);
    assert!(
        fixture
            .repository("boot-a")
            .claim("worker-2")
            .unwrap()
            .is_none()
    );
}

#[test]
fn two_catalog_connections_cannot_claim_the_same_job() {
    let fixture = Fixture::new();
    fixture.insert_job("only", "queued", "available", NOW);
    let second_catalog = Catalog::open(&fixture.db_path).unwrap();
    let barrier = Barrier::new(2);

    let claimed = thread::scope(|scope| {
        let first = JobRepository::new(
            &fixture.catalog,
            &fixture.ownership,
            "boot-a",
            fixture.clock.clone(),
        );
        let second = JobRepository::new(
            &second_catalog,
            &fixture.ownership,
            "boot-b",
            fixture.clock.clone(),
        );
        let first_barrier = &barrier;
        let second_barrier = &barrier;
        let one = scope.spawn(move || {
            first_barrier.wait();
            first.claim("worker-1").unwrap()
        });
        let two = scope.spawn(move || {
            second_barrier.wait();
            second.claim("worker-2").unwrap()
        });
        [one.join().unwrap(), two.join().unwrap()]
    });

    assert_eq!(claimed.iter().filter(|claim| claim.is_some()).count(), 1);
    assert_eq!(fixture.row("only").attempt_count, 1);
}

#[test]
fn lease_and_running_transitions_reject_stale_generations() {
    let fixture = Fixture::new();
    fixture.insert_job("fenced", "queued", "available", NOW);
    let jobs = fixture.repository("boot-a");
    let first = jobs.claim("worker-1").unwrap().unwrap();
    fixture.clock.advance(Duration::seconds(31));
    assert_eq!(
        jobs.recover().unwrap(),
        RecoveryOutcome {
            requeued: 1,
            cancelled: 0,
            exhausted: 0
        }
    );
    fixture.clock.advance(Duration::seconds(5));
    let second = jobs.claim("worker-2").unwrap().unwrap();

    assert!(matches!(
        jobs.renew(&first.token),
        Err(JobError::OwnershipLost)
    ));
    assert!(matches!(
        jobs.mark_transcribing(&first.token),
        Err(JobError::OwnershipLost)
    ));
    assert!(matches!(
        jobs.fail(&first.token, AsrErrorCode::TranscriptionFailed, "stale"),
        Err(JobError::OwnershipLost)
    ));
    jobs.mark_transcribing(&second.token).unwrap();
    fixture.clock.advance(Duration::seconds(5));
    jobs.renew(&second.token).unwrap();
    assert_eq!(fixture.row("fenced").state, "transcribing");
}

#[test]
fn recovery_distinguishes_boot_ids_applies_backoff_and_exhausts_third_claim() {
    let fixture = Fixture::new();
    fixture.insert_job("recover", "queued", "available", NOW);
    let boot_a = fixture.repository("boot-a");
    let first = boot_a.claim("worker-1").unwrap().unwrap();
    assert_eq!(first.attempt_count, 1);
    assert_eq!(
        boot_a.recover().unwrap(),
        RecoveryOutcome {
            requeued: 0,
            cancelled: 0,
            exhausted: 0
        }
    );

    let boot_b = fixture.repository("boot-b");
    assert_eq!(boot_b.recover().unwrap().requeued, 1);
    assert_eq!(
        fixture.row("recover").available_at,
        "2026-08-16T08:00:05.000Z"
    );
    fixture.clock.advance(Duration::seconds(5));
    let second = boot_b.claim("worker-2").unwrap().unwrap();
    fixture.clock.advance(Duration::seconds(31));
    assert_eq!(boot_b.recover().unwrap().requeued, 1);
    assert_eq!(
        fixture.row("recover").available_at,
        "2026-08-16T08:01:06.000Z"
    );
    fixture.clock.advance(Duration::seconds(30));
    let third = boot_b.claim("worker-3").unwrap().unwrap();
    fixture.clock.advance(Duration::seconds(31));
    assert_eq!(boot_b.recover().unwrap().exhausted, 1);

    assert!(third.token.claim_generation > second.token.claim_generation);
    let row = fixture.row("recover");
    assert_eq!(row.state, "failed");
    assert_eq!(row.attempt_count, 3);
    assert_eq!(row.error_code.as_deref(), Some("recovery_retry_exhausted"));
}

#[test]
fn cancellation_is_immediate_before_claim_and_marker_only_while_running() {
    let fixture = Fixture::new();
    fixture.insert_job("running", "queued", "available", NOW);
    let jobs = fixture.repository("boot-a");
    let running = jobs.claim("worker-1").unwrap().unwrap();
    fixture.insert_job("queued", "queued", "available", NOW);
    fixture.insert_job("blocked", "blocked_model", "available", NOW);

    assert_eq!(
        jobs.request_cancel("queued").unwrap(),
        CancellationOutcome::Cancelled
    );
    assert_eq!(
        jobs.request_cancel("blocked").unwrap(),
        CancellationOutcome::Cancelled
    );
    assert_eq!(
        jobs.request_cancel(&running.id).unwrap(),
        CancellationOutcome::Requested
    );
    assert_eq!(fixture.row(&running.id).state, "preparing");
    assert!(fixture.row(&running.id).cancel_requested_at.is_some());
    jobs.acknowledge_cancel(&running.token).unwrap();
    assert_eq!(fixture.row(&running.id).state, "cancelled");
    assert!(matches!(
        jobs.acknowledge_cancel(&running.token),
        Err(JobError::OwnershipLost)
    ));
}

#[test]
fn model_readiness_is_exact_and_manual_retry_reuses_the_job_generation() {
    let fixture = Fixture::new();
    fixture.insert_job("retry", "blocked_model", "available", NOW);
    fixture.install_ready_model();
    let jobs = fixture.repository("boot-a");
    let original = fixture.row("retry");
    let mut mismatch = ready_model();
    mismatch.runtime_identity_json = r#"{"build_id":"other","version":"1.13.5"}"#.to_owned();

    assert!(!jobs.is_ready_to_retry("retry", &mismatch).unwrap());
    assert!(matches!(
        jobs.retry("retry", &mismatch),
        Err(JobError::ModelNotReady)
    ));
    assert_eq!(fixture.row("retry").state, "blocked_model");
    assert!(jobs.is_ready_to_retry("retry", &ready_model()).unwrap());
    let generation = jobs.retry("retry", &ready_model()).unwrap();

    let retried = fixture.row("retry");
    assert_eq!(generation, 1);
    assert_eq!(retried.state, "queued");
    assert_eq!(retried.attempt_count, 0);
    assert_eq!(retried.claim_generation, 1);
    assert_eq!(retried.fingerprint, original.fingerprint);
    assert_eq!(retried.parameters_json, original.parameters_json);
    assert!(retried.claimed_by.is_none());
    assert!(retried.lease_expires_at.is_none());
    assert!(retried.cancel_requested_at.is_none());
    assert!(retried.error_code.is_none());
}

#[test]
fn cancelled_jobs_cannot_retry_and_model_ready_never_autoqueues() {
    let fixture = Fixture::new();
    fixture.insert_job("cancelled", "cancelled", "available", NOW);
    fixture.insert_job("blocked", "blocked_model", "available", NOW);
    fixture.install_ready_model();
    let jobs = fixture.repository("boot-a");

    assert!(matches!(
        jobs.retry("cancelled", &ready_model()),
        Err(JobError::InvalidTransition)
    ));
    assert!(jobs.is_ready_to_retry("blocked", &ready_model()).unwrap());
    assert_eq!(fixture.row("blocked").state, "blocked_model");
}

#[test]
fn recovery_honors_running_cancel_marker_and_failed_retry_is_explicit() {
    let fixture = Fixture::new();
    fixture.insert_job("running", "queued", "available", NOW);
    let boot_a = fixture.repository("boot-a");
    let running = boot_a.claim("worker-1").unwrap().unwrap();
    assert_eq!(
        boot_a.request_cancel(&running.id).unwrap(),
        CancellationOutcome::Requested
    );

    let boot_b = fixture.repository("boot-b");
    assert_eq!(
        boot_b.recover().unwrap(),
        RecoveryOutcome {
            requeued: 0,
            cancelled: 1,
            exhausted: 0
        }
    );
    assert_eq!(fixture.row("running").state, "cancelled");

    fixture.insert_job("failed", "failed", "available", NOW);
    fixture.install_ready_model();
    assert_eq!(boot_b.retry("failed", &ready_model()).unwrap(), 1);
    assert_eq!(fixture.row("failed").state, "queued");
}

#[test]
fn recovery_treats_unparseable_claim_owner_as_stale_instead_of_aborting() {
    let fixture = Fixture::new();
    fixture.insert_job("legacy-owner", "queued", "available", NOW);
    fixture
        .repository("boot-a")
        .claim("worker-1")
        .unwrap()
        .unwrap();
    Connection::open(&fixture.db_path)
        .unwrap()
        .execute(
            "UPDATE asr_jobs SET claimed_by = 'legacy-owner' WHERE id = 'legacy-owner'",
            [],
        )
        .unwrap();

    assert_eq!(
        fixture.repository("boot-b").recover().unwrap(),
        RecoveryOutcome {
            requeued: 1,
            cancelled: 0,
            exhausted: 0,
        }
    );
}

#[test]
fn task9_repository_has_no_success_or_complete_transition() {
    let repository = include_str!("asr/job.rs");
    let catalog_jobs = include_str!("catalog/jobs.rs");

    assert!(!repository.contains("complete("));
    assert!(!catalog_jobs.contains("state = 'succeeded'"));
    assert!(!catalog_jobs.contains("state='succeeded'"));
}

#[cfg(unix)]
#[test]
fn invalidated_full_core_owner_blocks_claim_and_recovery() {
    let fixture = Fixture::new();
    fixture.insert_job("owned", "queued", "available", NOW);
    let lock_path = fixture.data_dir.join("asr-worker.lock");
    fs::rename(&lock_path, fixture.data_dir.join("old-lock")).unwrap();
    fs::write(&lock_path, b"replacement").unwrap();
    let jobs = fixture.repository("boot-a");

    assert!(matches!(
        jobs.claim("worker-1"),
        Err(JobError::Ownership(RuntimeOwnershipError::UnsafePath))
    ));
    assert!(matches!(
        jobs.recover(),
        Err(JobError::Ownership(RuntimeOwnershipError::UnsafePath))
    ));
    assert_eq!(fixture.row("owned").state, "queued");
}
