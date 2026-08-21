use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use chrono::{DateTime, Utc};

use crate::asr::job::{Clock, ExecutionSnapshot, ExecutionStage, JobCoordinator, JobError};
use crate::catalog::PublicationError;
use crate::domain::{AsrErrorCode, ProviderReceipt, TranscriptSegmentPublication};

pub const DEFAULT_IDLE_POLL: Duration = Duration::from_millis(250);
pub const DEFAULT_RUN_POLL: Duration = Duration::from_millis(200);

/// Desktop fallback executor: queued work is failed with an explicit runtime
/// error until a verified native provider is available. It never fabricates a
/// transcript or leaves work queued forever.
#[derive(Clone, Copy, Default)]
pub struct FailClosedEngine;

pub struct FailClosedRun;

impl<C: Clock> ExecutionEngine<C> for FailClosedEngine {
    type Prepared = ();
    type Run = FailClosedRun;

    fn prepare(&self, _snapshot: &ExecutionSnapshot) -> Result<Self::Prepared, ExecutionFailure> {
        Err(ExecutionFailure {
            code: AsrErrorCode::ProviderInitializationFailed,
            summary: "verified native ASR provider is not available".to_owned(),
        })
    }

    fn start(
        &self,
        _prepared: Self::Prepared,
        _started_at: DateTime<Utc>,
    ) -> Result<Self::Run, ExecutionFailure> {
        unreachable!("fail-closed engine never starts an inference run")
    }
}

impl ExecutionRun for FailClosedRun {
    fn recv_timeout(
        &mut self,
        _timeout: Duration,
    ) -> Result<Result<ExecutionSuccess, ExecutionFailure>, RunPollResult> {
        unreachable!("fail-closed engine never starts an inference run")
    }

    fn cancel(&self) {}
}

#[cfg(feature = "desktop")]
pub fn spawn_fail_closed_worker(
    runtime: Arc<crate::service::CoreRuntime>,
) -> (Arc<AtomicBool>, std::thread::JoinHandle<()>) {
    let shutdown = Arc::new(AtomicBool::new(false));
    let thread_shutdown = shutdown.clone();
    let handle = std::thread::spawn(move || {
        let Ok(coordinator) = runtime.job_coordinator("desktop-worker") else {
            return;
        };
        let mut worker = WorkerLoop::new(
            coordinator,
            crate::asr::job::SystemClock,
            FailClosedEngine,
            thread_shutdown,
            ThreadSleepWaiter,
        );
        let _ = worker.run_until_shutdown();
    });
    (shutdown, handle)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionFailure {
    pub code: AsrErrorCode,
    pub summary: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionSuccess {
    pub receipt: ProviderReceipt,
    pub segments: Vec<TranscriptSegmentPublication>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunPollResult {
    Pending,
}

pub trait ExecutionRun: Send {
    fn recv_timeout(
        &mut self,
        timeout: Duration,
    ) -> Result<Result<ExecutionSuccess, ExecutionFailure>, RunPollResult>;

    fn cancel(&self);
}

pub trait ExecutionEngine<C: Clock>: Send + Sync + 'static {
    type Prepared;
    type Run: ExecutionRun;

    fn prepare(&self, snapshot: &ExecutionSnapshot) -> Result<Self::Prepared, ExecutionFailure>;

    fn start(
        &self,
        prepared: Self::Prepared,
        started_at: DateTime<Utc>,
    ) -> Result<Self::Run, ExecutionFailure>;
}

pub trait IdleWaiter: Clone + Send + Sync + 'static {
    fn wait(&self, duration: Duration);
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ThreadSleepWaiter;

impl IdleWaiter for ThreadSleepWaiter {
    fn wait(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

#[derive(Debug)]
pub enum WorkerError {
    Job(JobError),
    Execution(ExecutionFailure),
    Publish(PublicationError),
}

impl From<JobError> for WorkerError {
    fn from(value: JobError) -> Self {
        Self::Job(value)
    }
}

pub struct WorkerLoop<'a, C, E, W>
where
    C: Clock,
    E: ExecutionEngine<C>,
    W: IdleWaiter,
{
    coordinator: JobCoordinator<'a, C>,
    clock: C,
    engine: E,
    shutdown: Arc<AtomicBool>,
    idle_waiter: W,
    idle_poll: Duration,
    run_poll: Duration,
    recovered: bool,
}

impl<'a, C, E, W> WorkerLoop<'a, C, E, W>
where
    C: Clock + Clone,
    E: ExecutionEngine<C>,
    W: IdleWaiter,
{
    pub fn new(
        coordinator: JobCoordinator<'a, C>,
        clock: C,
        engine: E,
        shutdown: Arc<AtomicBool>,
        idle_waiter: W,
    ) -> Self {
        Self {
            coordinator,
            clock,
            engine,
            shutdown,
            idle_waiter,
            idle_poll: DEFAULT_IDLE_POLL,
            run_poll: DEFAULT_RUN_POLL,
            recovered: false,
        }
    }

    #[cfg(test)]
    fn with_polls(mut self, idle_poll: Duration, run_poll: Duration) -> Self {
        self.idle_poll = idle_poll;
        self.run_poll = run_poll;
        self
    }

    pub fn run_until_shutdown(&mut self) -> Result<(), WorkerError> {
        while !(self.shutdown.load(Ordering::Acquire) && self.coordinator.active_claim().is_none())
        {
            self.run_once()?;
        }
        Ok(())
    }

    fn run_once(&mut self) -> Result<(), WorkerError> {
        if !self.recovered {
            self.coordinator.recover()?;
            self.recovered = true;
        }

        if self.shutdown.load(Ordering::Acquire) && self.coordinator.active_claim().is_none() {
            return Ok(());
        }

        if self.coordinator.claim_next()?.is_none() {
            self.idle_waiter.wait(self.idle_poll);
            return Ok(());
        }

        let token = self
            .coordinator
            .active_claim()
            .expect("claimed job must remain active")
            .token
            .clone();
        let snapshot = match self
            .coordinator
            .load_execution_snapshot(&token, ExecutionStage::Preparing)
        {
            Ok(snapshot) => snapshot,
            Err(error) => return self.handle_preparing_snapshot_error(error),
        };
        let prepared = match self.engine.prepare(&snapshot) {
            Ok(prepared) => prepared,
            Err(failure) => {
                self.coordinator.fail(failure.code, &failure.summary)?;
                return Ok(());
            }
        };
        self.coordinator.enter_transcribing()?;
        let started_at = self.clock.now();
        let mut run = match self.engine.start(prepared, started_at) {
            Ok(run) => run,
            Err(failure) => {
                self.coordinator.fail(failure.code, &failure.summary)?;
                return Ok(());
            }
        };

        loop {
            match run.recv_timeout(self.next_run_timeout()) {
                Ok(Ok(success)) => {
                    match self
                        .coordinator
                        .publish(&token, &success.receipt, &success.segments)
                    {
                        Ok(_) => {
                            self.coordinator.recover()?;
                            return Ok(());
                        }
                        Err(PublicationError::Cancelled) => {
                            run.cancel();
                            self.coordinator.acknowledge_cancel()?;
                            return Ok(());
                        }
                        Err(PublicationError::OwnershipLost) => {
                            run.cancel();
                            self.coordinator.recover()?;
                            return Ok(());
                        }
                        Err(error) => return Err(WorkerError::Publish(error)),
                    }
                }
                Ok(Err(failure)) => {
                    self.coordinator.fail(failure.code, &failure.summary)?;
                    return Ok(());
                }
                Err(RunPollResult::Pending) => {
                    if let Some(outcome) = self.observe_running(&token, &run)? {
                        return Ok(outcome);
                    }
                }
            }
        }
    }

    fn next_run_timeout(&self) -> Duration {
        let Some(active) = self.coordinator.active_claim() else {
            return self.run_poll;
        };
        let now = self.clock.now();
        let renew_delay = active.renew_at.signed_duration_since(now);
        if renew_delay <= chrono::Duration::zero() {
            return Duration::ZERO;
        }
        let renew_delay = renew_delay
            .to_std()
            .unwrap_or_else(|_| Duration::from_millis(1));
        renew_delay.min(self.run_poll)
    }

    fn observe_running<R: ExecutionRun>(
        &mut self,
        token: &crate::asr::job::ClaimToken,
        run: &R,
    ) -> Result<Option<()>, WorkerError> {
        let renew_due = self
            .coordinator
            .active_claim()
            .is_some_and(|claim| self.clock.now() >= claim.renew_at);
        if renew_due {
            match self.coordinator.renew_if_due() {
                Ok(_) => return Ok(None),
                Err(JobError::CancelRequested) => {
                    run.cancel();
                    return Ok(Some(()));
                }
                Err(JobError::OwnershipLost) => {
                    run.cancel();
                    self.coordinator.recover()?;
                    return Ok(Some(()));
                }
                Err(error) => return Err(WorkerError::Job(error)),
            }
        }

        match self
            .coordinator
            .load_execution_snapshot(token, ExecutionStage::Transcribing)
        {
            Ok(_) => Ok(None),
            Err(crate::asr::job::SnapshotError::CancelRequested) => {
                run.cancel();
                self.coordinator.acknowledge_cancel()?;
                Ok(Some(()))
            }
            Err(crate::asr::job::SnapshotError::OwnershipLost)
            | Err(crate::asr::job::SnapshotError::LeaseExpired)
            | Err(crate::asr::job::SnapshotError::StageMismatch) => {
                run.cancel();
                self.coordinator.recover()?;
                Ok(Some(()))
            }
            Err(crate::asr::job::SnapshotError::InputUnavailable) => {
                run.cancel();
                self.coordinator.fail(
                    AsrErrorCode::InputUnavailable,
                    "job input became unavailable while transcribing",
                )?;
                Ok(Some(()))
            }
            Err(error) => {
                run.cancel();
                let failure = map_snapshot_error(error);
                self.coordinator
                    .fail(AsrErrorCode::RecoveryRequired, &failure.summary)?;
                Ok(Some(()))
            }
        }
    }

    fn handle_preparing_snapshot_error(
        &mut self,
        error: crate::asr::job::SnapshotError,
    ) -> Result<(), WorkerError> {
        match error {
            crate::asr::job::SnapshotError::CancelRequested => {
                self.coordinator.acknowledge_cancel()?;
                Ok(())
            }
            crate::asr::job::SnapshotError::InputUnavailable => {
                self.coordinator.fail(
                    AsrErrorCode::InputUnavailable,
                    "job input became unavailable before execution",
                )?;
                Ok(())
            }
            crate::asr::job::SnapshotError::OwnershipLost
            | crate::asr::job::SnapshotError::LeaseExpired
            | crate::asr::job::SnapshotError::StageMismatch => {
                self.coordinator.recover()?;
                Ok(())
            }
            crate::asr::job::SnapshotError::Ownership(error) => {
                Err(WorkerError::Job(JobError::Ownership(error)))
            }
            crate::asr::job::SnapshotError::Catalog(error) => {
                Err(WorkerError::Job(JobError::Catalog(error)))
            }
            crate::asr::job::SnapshotError::InvalidSnapshot(_) => {
                let failure = map_snapshot_error(error);
                self.coordinator
                    .fail(AsrErrorCode::RecoveryRequired, &failure.summary)?;
                Ok(())
            }
        }
    }
}

fn map_snapshot_error(error: crate::asr::job::SnapshotError) -> ExecutionFailure {
    match error {
        crate::asr::job::SnapshotError::CancelRequested => ExecutionFailure {
            code: AsrErrorCode::Cancelled,
            summary: "job cancelled before execution".to_owned(),
        },
        crate::asr::job::SnapshotError::InputUnavailable => ExecutionFailure {
            code: AsrErrorCode::InputUnavailable,
            summary: "job input became unavailable".to_owned(),
        },
        crate::asr::job::SnapshotError::Ownership(_)
        | crate::asr::job::SnapshotError::Catalog(_)
        | crate::asr::job::SnapshotError::OwnershipLost
        | crate::asr::job::SnapshotError::StageMismatch
        | crate::asr::job::SnapshotError::LeaseExpired
        | crate::asr::job::SnapshotError::InvalidSnapshot(_) => ExecutionFailure {
            code: AsrErrorCode::RecoveryRequired,
            summary: "failed to load execution snapshot".to_owned(),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use chrono::Duration as ChronoDuration;
    use rusqlite::{Connection, params};
    use sha2::{Digest, Sha256};
    use tempfile::TempDir;

    use crate::asr::manifest::{
        InstallConstraints, canonical_bundle_payload, model_registry, vad_manifest,
    };
    use crate::domain::{AudioSource, DataDestination, ProviderOutcome, ProviderReceiptDraft};
    use crate::service::CoreRuntime;

    use super::*;

    const NOW: &str = "2026-08-19T08:00:00.000Z";
    const INPUT_SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const MODEL_ID: &str = "whisper-small";

    #[derive(Clone)]
    struct TestClock(Arc<Mutex<DateTime<Utc>>>);

    impl TestClock {
        fn new() -> Self {
            Self(Arc::new(Mutex::new(parse_time(NOW))))
        }

        fn advance(&self, duration: Duration) {
            let delta = ChronoDuration::from_std(duration).unwrap();
            *self.0.lock().unwrap() += delta;
        }
    }

    impl Clock for TestClock {
        fn now(&self) -> DateTime<Utc> {
            *self.0.lock().unwrap()
        }
    }

    struct Fixture {
        _temp: TempDir,
        db_path: std::path::PathBuf,
        runtime: CoreRuntime,
        clock: TestClock,
    }

    impl Fixture {
        fn new() -> Self {
            let temp = tempfile::tempdir().unwrap();
            let data_dir = temp.path().join("data");
            let db_path = data_dir.join("lifesub.sqlite3");
            let runtime = CoreRuntime::initialize_with_boot_id(&data_dir, "boot-worker").unwrap();
            Self {
                _temp: temp,
                db_path,
                runtime,
                clock: TestClock::new(),
            }
        }

        fn insert_job(&self, id: &str, state: &str, available_at: &str) {
            let connection = Connection::open(&self.db_path).unwrap();
            connection
                .pragma_update(None, "foreign_keys", true)
                .unwrap();
            connection
                .execute(
                    "INSERT OR IGNORE INTO sessions(id, title, state, started_at)
                     VALUES('session-worker', 'worker', 'stopped', ?1)",
                    [NOW],
                )
                .unwrap();
            connection
                .execute(
                    "INSERT OR IGNORE INTO chunks(
                       id, session_id, source, path, sha256, byte_length, session_offset_ms,
                       duration_ms, integrity_state
                     ) VALUES(
                       'chunk-worker', 'session-worker', 'imported', 'audio/worker.wav', ?1,
                       4096, 0, 8000, 'available'
                     )",
                    [INPUT_SHA],
                )
                .unwrap();
            let settings = serde_json::json!({
                "provider": "whisper",
                "model_id": MODEL_ID,
                "language": "auto",
                "num_threads": 2,
                "vad_enabled": true,
                "auto_transcribe_imports": true,
                "options": {"provider": "whisper", "task": "transcribe"}
            });
            let manifest = model_registry().model(MODEL_ID).unwrap();
            let required = required_files_json(manifest.bundle.install_constraints);
            let vad = vad_manifest();
            let vad_required = required_files_json(vad.bundle.install_constraints);
            let source = with_source_contract(serde_json::json!({
                "bundle": serde_json::from_str::<serde_json::Value>(
                    &canonical_bundle_payload(manifest).unwrap()
                ).unwrap(),
                "repository_url": manifest.source.repository_url,
                "model_card_url": manifest.source.model_card_url,
                "license_spdx": manifest.source.license_spdx,
                "provenance": manifest.source.provenance,
            }));
            connection
                .execute(
                    "INSERT INTO asr_jobs(
                       id, session_id, chunk_id, provider, model_id, manifest_version,
                       archive_sha256, required_file_hashes_json, model_source_json,
                       vad_model_id, vad_manifest_version, vad_archive_sha256,
                       vad_required_file_hashes_json, parameters_json, input_sha256,
                       fingerprint, state, max_attempts, available_at, created_at, updated_at
                     ) VALUES(
                       ?1, 'session-worker', 'chunk-worker', 'whisper', ?2, ?3, ?4, ?5, ?6,
                       'silero-vad-2024-01-17', ?7, ?8, ?9, ?10, ?11, ?12, ?13, 3, ?14, ?15, ?15
                     )",
                    params![
                        id,
                        MODEL_ID,
                        manifest.manifest_version,
                        manifest.bundle.identity_sha256,
                        required.to_string(),
                        source.to_string(),
                        vad.manifest_version,
                        vad.bundle.identity_sha256,
                        vad_required.to_string(),
                        settings.to_string(),
                        INPUT_SHA,
                        format!("fingerprint-{id}"),
                        state,
                        available_at,
                        NOW,
                    ],
                )
                .unwrap();
        }

        fn force_claim_owner(&self, job_id: &str, boot_id: &str) {
            let claimed_by =
                serde_json::json!({"boot_id": boot_id, "worker_id": "stale-worker"}).to_string();
            Connection::open(&self.db_path)
                .unwrap()
                .execute(
                    "UPDATE asr_jobs
                     SET state = 'preparing', attempt_count = 1, claim_generation = 1,
                         claimed_by = ?2, lease_expires_at = '2026-08-19T07:59:00.000Z'
                     WHERE id = ?1",
                    params![job_id, claimed_by],
                )
                .unwrap();
        }

        fn row_state(&self, job_id: &str) -> (String, Option<String>) {
            Connection::open(&self.db_path)
                .unwrap()
                .query_row(
                    "SELECT state, error_code FROM asr_jobs WHERE id = ?1",
                    [job_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap()
        }

        fn revision_count(&self) -> i64 {
            Connection::open(&self.db_path)
                .unwrap()
                .query_row("SELECT COUNT(*) FROM revisions", [], |row| row.get(0))
                .unwrap()
        }
    }

    #[derive(Clone)]
    struct ScriptedEngine {
        clock: TestClock,
        plans: Arc<Mutex<VecDeque<JobPlan>>>,
    }

    struct FakePrepared {
        snapshot: ExecutionSnapshot,
        plan: JobPlan,
    }

    #[derive(Clone)]
    struct JobPlan {
        polls_before_finish: usize,
        on_poll: Option<Arc<dyn Fn(usize) + Send + Sync>>,
        outcome: PlanOutcome,
    }

    #[derive(Clone)]
    enum PlanOutcome {
        Success {
            text: &'static str,
        },
        Failure {
            code: AsrErrorCode,
            summary: &'static str,
        },
        PrepareFailure {
            code: AsrErrorCode,
            summary: &'static str,
        },
    }

    struct FakeRun {
        clock: TestClock,
        snapshot: ExecutionSnapshot,
        started_at: DateTime<Utc>,
        polls_before_finish: usize,
        polled: usize,
        on_poll: Option<Arc<dyn Fn(usize) + Send + Sync>>,
        outcome: PlanOutcome,
        cancelled: Arc<AtomicBool>,
    }

    impl ExecutionRun for FakeRun {
        fn recv_timeout(
            &mut self,
            timeout: Duration,
        ) -> Result<Result<ExecutionSuccess, ExecutionFailure>, RunPollResult> {
            self.clock.advance(timeout);
            let current_poll = self.polled;
            self.polled += 1;
            if let Some(callback) = &self.on_poll {
                callback(current_poll);
            }
            if self.polled <= self.polls_before_finish {
                return Err(RunPollResult::Pending);
            }
            if self.cancelled.load(Ordering::Acquire) {
                return Ok(Err(ExecutionFailure {
                    code: AsrErrorCode::Cancelled,
                    summary: "cancelled".to_owned(),
                }));
            }
            match &self.outcome {
                PlanOutcome::Success { text } => Ok(Ok(success_from_snapshot(
                    &self.snapshot,
                    text,
                    self.started_at,
                    self.clock.now(),
                ))),
                PlanOutcome::Failure { code, summary } => Ok(Err(ExecutionFailure {
                    code: *code,
                    summary: (*summary).to_owned(),
                })),
                PlanOutcome::PrepareFailure { .. } => unreachable!("prepare failures never start"),
            }
        }

        fn cancel(&self) {
            self.cancelled.store(true, Ordering::Release);
        }
    }

    impl ExecutionEngine<TestClock> for ScriptedEngine {
        type Prepared = FakePrepared;
        type Run = FakeRun;

        fn prepare(
            &self,
            snapshot: &ExecutionSnapshot,
        ) -> Result<Self::Prepared, ExecutionFailure> {
            let plan = self.plans.lock().unwrap().pop_front().unwrap();
            if let PlanOutcome::PrepareFailure { code, summary } = &plan.outcome {
                return Err(ExecutionFailure {
                    code: *code,
                    summary: (*summary).to_owned(),
                });
            }
            Ok(FakePrepared {
                snapshot: snapshot.clone(),
                plan,
            })
        }

        fn start(
            &self,
            prepared: Self::Prepared,
            started_at: DateTime<Utc>,
        ) -> Result<Self::Run, ExecutionFailure> {
            Ok(FakeRun {
                clock: self.clock.clone(),
                snapshot: prepared.snapshot,
                started_at,
                polls_before_finish: prepared.plan.polls_before_finish,
                polled: 0,
                on_poll: prepared.plan.on_poll,
                outcome: prepared.plan.outcome,
                cancelled: Arc::new(AtomicBool::new(false)),
            })
        }
    }

    #[derive(Clone)]
    struct StopAfterIdle {
        clock: TestClock,
        shutdown: Arc<AtomicBool>,
        waits: Arc<AtomicUsize>,
        stop_after: usize,
    }

    impl IdleWaiter for StopAfterIdle {
        fn wait(&self, duration: Duration) {
            self.clock.advance(duration);
            let waited = self.waits.fetch_add(1, Ordering::SeqCst) + 1;
            if waited >= self.stop_after {
                self.shutdown.store(true, Ordering::Release);
            }
        }
    }

    #[test]
    fn worker_recovers_stale_jobs_on_startup_and_publishes_new_revision() {
        let fixture = Fixture::new();
        fixture.insert_job("recover-me", "queued", NOW);
        fixture.force_claim_owner("recover-me", "old-boot");
        let shutdown = Arc::new(AtomicBool::new(false));
        let idle = StopAfterIdle {
            clock: fixture.clock.clone(),
            shutdown: shutdown.clone(),
            waits: Arc::new(AtomicUsize::new(0)),
            stop_after: 2,
        };
        let engine = ScriptedEngine {
            clock: fixture.clock.clone(),
            plans: Arc::new(Mutex::new(VecDeque::from([JobPlan {
                polls_before_finish: 0,
                on_poll: None,
                outcome: PlanOutcome::Success { text: "recovered" },
            }]))),
        };
        let coordinator = fixture
            .runtime
            .job_coordinator_with_clock("worker-1", fixture.clock.clone())
            .unwrap();
        let mut worker =
            WorkerLoop::new(coordinator, fixture.clock.clone(), engine, shutdown, idle)
                .with_polls(Duration::from_secs(5), Duration::from_secs(5));

        worker.run_until_shutdown().unwrap();

        assert_eq!(fixture.row_state("recover-me").0, "succeeded");
        assert_eq!(fixture.revision_count(), 1);
    }

    #[test]
    fn worker_renews_long_running_transcription_until_publish_succeeds() {
        let fixture = Fixture::new();
        fixture.insert_job("long-run", "queued", NOW);
        let shutdown = Arc::new(AtomicBool::new(false));
        let idle = StopAfterIdle {
            clock: fixture.clock.clone(),
            shutdown: shutdown.clone(),
            waits: Arc::new(AtomicUsize::new(0)),
            stop_after: 1,
        };
        let engine = ScriptedEngine {
            clock: fixture.clock.clone(),
            plans: Arc::new(Mutex::new(VecDeque::from([JobPlan {
                polls_before_finish: 7,
                on_poll: None,
                outcome: PlanOutcome::Success {
                    text: "slow success",
                },
            }]))),
        };
        let coordinator = fixture
            .runtime
            .job_coordinator_with_clock("worker-2", fixture.clock.clone())
            .unwrap();
        let mut worker =
            WorkerLoop::new(coordinator, fixture.clock.clone(), engine, shutdown, idle)
                .with_polls(Duration::from_secs(1), Duration::from_secs(5));

        worker.run_until_shutdown().unwrap();

        assert_eq!(fixture.row_state("long-run").0, "succeeded");
        assert_eq!(fixture.revision_count(), 1);
    }

    #[test]
    fn worker_acknowledges_cancel_and_does_not_publish_revision() {
        let fixture = Fixture::new();
        fixture.insert_job("cancel-me", "queued", NOW);
        let shutdown = Arc::new(AtomicBool::new(false));
        let engine = ScriptedEngine {
            clock: fixture.clock.clone(),
            plans: Arc::new(Mutex::new(VecDeque::from([JobPlan {
                polls_before_finish: 4,
                on_poll: Some(Arc::new({
                    let db_path = fixture.db_path.clone();
                    move |poll| {
                        if poll == 0 {
                            Connection::open(&db_path)
                                .unwrap()
                                .execute(
                                    "UPDATE asr_jobs
                                     SET cancel_requested_at = '2026-08-19T08:00:01.000Z'
                                     WHERE id = 'cancel-me'",
                                    [],
                                )
                                .unwrap();
                        }
                    }
                })),
                outcome: PlanOutcome::Success {
                    text: "must not publish",
                },
            }]))),
        };
        let idle = StopAfterIdle {
            clock: fixture.clock.clone(),
            shutdown: shutdown.clone(),
            waits: Arc::new(AtomicUsize::new(0)),
            stop_after: 1,
        };
        let coordinator = fixture
            .runtime
            .job_coordinator_with_clock("worker-3", fixture.clock.clone())
            .unwrap();
        let mut worker =
            WorkerLoop::new(coordinator, fixture.clock.clone(), engine, shutdown, idle)
                .with_polls(Duration::from_secs(1), Duration::from_secs(5));

        worker.run_until_shutdown().unwrap();

        let (state, error_code) = fixture.row_state("cancel-me");
        assert_eq!(state, "cancelled");
        assert_eq!(error_code.as_deref(), Some("cancelled"));
        assert_eq!(fixture.revision_count(), 0);
    }

    #[test]
    fn worker_fails_job_when_execution_prepare_or_run_fails() {
        for outcome in [
            PlanOutcome::PrepareFailure {
                code: AsrErrorCode::UnsupportedOrCorruptAudio,
                summary: "decode failed",
            },
            PlanOutcome::Failure {
                code: AsrErrorCode::TranscriptionFailed,
                summary: "provider failed",
            },
        ] {
            let fixture = Fixture::new();
            fixture.insert_job("fail-me", "queued", NOW);
            let shutdown = Arc::new(AtomicBool::new(false));
            let idle = StopAfterIdle {
                clock: fixture.clock.clone(),
                shutdown: shutdown.clone(),
                waits: Arc::new(AtomicUsize::new(0)),
                stop_after: 1,
            };
            let engine = ScriptedEngine {
                clock: fixture.clock.clone(),
                plans: Arc::new(Mutex::new(VecDeque::from([JobPlan {
                    polls_before_finish: 0,
                    on_poll: None,
                    outcome: outcome.clone(),
                }]))),
            };
            let coordinator = fixture
                .runtime
                .job_coordinator_with_clock("worker-4", fixture.clock.clone())
                .unwrap();
            let mut worker =
                WorkerLoop::new(coordinator, fixture.clock.clone(), engine, shutdown, idle)
                    .with_polls(Duration::from_secs(1), Duration::from_secs(5));

            worker.run_until_shutdown().unwrap();

            let (state, error_code) = fixture.row_state("fail-me");
            assert_eq!(state, "failed");
            assert!(error_code.is_some());
            assert_eq!(fixture.revision_count(), 0);
        }
    }

    #[test]
    fn worker_shutdown_finishes_active_job_without_claiming_next_job() {
        let fixture = Fixture::new();
        fixture.insert_job("first", "queued", NOW);
        fixture.insert_job("second", "queued", NOW);
        let shutdown = Arc::new(AtomicBool::new(false));
        let engine = ScriptedEngine {
            clock: fixture.clock.clone(),
            plans: Arc::new(Mutex::new(VecDeque::from([JobPlan {
                polls_before_finish: 2,
                on_poll: Some(Arc::new({
                    let shutdown = shutdown.clone();
                    move |poll| {
                        if poll == 0 {
                            shutdown.store(true, Ordering::Release);
                        }
                    }
                })),
                outcome: PlanOutcome::Success {
                    text: "finish before exit",
                },
            }]))),
        };
        let idle = StopAfterIdle {
            clock: fixture.clock.clone(),
            shutdown: shutdown.clone(),
            waits: Arc::new(AtomicUsize::new(0)),
            stop_after: 1,
        };
        let coordinator = fixture
            .runtime
            .job_coordinator_with_clock("worker-5", fixture.clock.clone())
            .unwrap();
        let mut worker =
            WorkerLoop::new(coordinator, fixture.clock.clone(), engine, shutdown, idle)
                .with_polls(Duration::from_secs(1), Duration::from_secs(5));

        worker.run_until_shutdown().unwrap();

        assert_eq!(fixture.row_state("first").0, "succeeded");
        assert_eq!(fixture.row_state("second").0, "queued");
        assert_eq!(fixture.revision_count(), 1);
    }

    #[test]
    fn runtime_allows_only_one_worker_coordinator_reservation() {
        let fixture = Fixture::new();
        let _first = fixture
            .runtime
            .job_coordinator_with_clock("worker-a", fixture.clock.clone())
            .unwrap();

        assert!(matches!(
            fixture
                .runtime
                .job_coordinator_with_clock("worker-b", fixture.clock.clone()),
            Err(JobError::CoordinatorAlreadyActive)
        ));
    }

    fn required_files_json(constraints: InstallConstraints) -> serde_json::Value {
        let files = match constraints {
            InstallConstraints::Archive(value) => value.required_files,
            InstallConstraints::Direct(value) => value.required_files,
        };
        serde_json::Value::Array(
            files
                .iter()
                .map(|file| {
                    serde_json::json!({
                        "path": file.path,
                        "bytes": file.bytes,
                        "sha256": file.sha256,
                    })
                })
                .collect(),
        )
    }

    fn with_source_contract(mut source: serde_json::Value) -> serde_json::Value {
        let canonical = serde_json_canonicalizer::to_string(&source).unwrap();
        source["source_contract_sha256"] =
            serde_json::json!(hex::encode(Sha256::digest(canonical.as_bytes())));
        source
    }

    fn parse_time(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .unwrap()
            .with_timezone(&Utc)
    }

    fn success_from_snapshot(
        snapshot: &ExecutionSnapshot,
        text: &str,
        started_at: DateTime<Utc>,
        finished_at: DateTime<Utc>,
    ) -> ExecutionSuccess {
        ExecutionSuccess {
            receipt: ProviderReceipt::try_from(ProviderReceiptDraft {
                job_id: snapshot.job_id.clone(),
                chunk_id: snapshot.chunk.id.clone(),
                provider: snapshot.model.provider,
                model_id: snapshot.model.model_id.clone(),
                manifest_version: snapshot.model.manifest_version.clone(),
                archive_sha256: snapshot.model.bundle_identity.clone(),
                required_file_hashes_json: snapshot.model.required_file_hashes_json.clone(),
                model_source_json: snapshot.model.model_source_json.clone(),
                vad_model_id: snapshot.vad.as_ref().map(|value| value.model_id.clone()),
                vad_manifest_version: snapshot
                    .vad
                    .as_ref()
                    .map(|value| value.manifest_version.clone()),
                vad_archive_sha256: snapshot
                    .vad
                    .as_ref()
                    .map(|value| value.bundle_identity.clone()),
                vad_required_file_hashes_json: snapshot
                    .vad
                    .as_ref()
                    .map(|value| value.required_file_hashes_json.clone()),
                runtime_version: "runtime-1".to_owned(),
                runtime_build_id: "build-1".to_owned(),
                parameters_json: snapshot.parameters.json.clone(),
                input_sha256: snapshot.chunk.sha256.clone(),
                started_at,
                finished_at,
                data_destination: DataDestination::LocalDevice,
                outcome: ProviderOutcome::Succeeded,
            })
            .unwrap(),
            segments: vec![TranscriptSegmentPublication {
                id: "seg_worker".to_owned(),
                chunk_start_ms: 0,
                chunk_end_ms: 500,
                source: AudioSource::Imported,
                text: text.to_owned(),
            }],
        }
    }
}
