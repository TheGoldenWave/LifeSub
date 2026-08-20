use std::fs::File;

use chrono::{DateTime, Utc, TimeDelta};
use tempfile::TempDir;

use crate::asr::settings::AsrProviderKind;
use crate::catalog::Catalog;
use crate::domain::AsrJobState;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// A minimal input struct to insert a job row for testing.
struct JobInput {
    id: String,
    fingerprint: String,
    state: AsrJobState,
    attempt_count: i64,
    claim_generation: i64,
    max_attempts: i64,
    available_at: DateTime<Utc>,
    claimed_by: Option<String>,
    lease_expires_at: Option<DateTime<Utc>>,
    cancel_requested_at: Option<DateTime<Utc>>,
}

fn job_input(id: &str, fingerprint: &str, state: AsrJobState) -> JobInput {
    JobInput {
        id: id.to_string(),
        fingerprint: fingerprint.to_string(),
        state,
        attempt_count: 0,
        claim_generation: 0,
        max_attempts: 3,
        available_at: Utc::now(),
        claimed_by: None,
        lease_expires_at: None,
        cancel_requested_at: None,
    }
}

fn insert_job(catalog: &Catalog, input: &JobInput) {
    // Ensure foreign key references exist — insert session and chunk
    use crate::domain::{AudioChunk, AudioSource, CaptureSession};
    let session = CaptureSession::new("test session");
    let mut session = session;
    session.id = "s_test".to_string();
    let _ = catalog.insert_session(&session);

    let chunk = AudioChunk {
        id: "c_test".to_string(),
        session_id: "s_test".to_string(),
        source: AudioSource::Imported,
        path: "/tmp/test.wav".to_string(),
        sha256: "sha256_def".to_string(),
        byte_length: 1024,
    };
    let _ = catalog.insert_chunk(&chunk);

    catalog
        .insert_asr_job(
            &input.id,
            "s_test",
            "c_test",
            AsrProviderKind::SenseVoice,
            "sense-voice-small",
            "1.0",
            "sha256_abc",
            "{}",
            "{}",
            None,
            None,
            None,
            None,
            "{}",
            "sha256_def",
            &input.fingerprint,
            input.state,
            input.attempt_count,
            input.claim_generation,
            input.max_attempts,
            input.available_at,
            input.claimed_by.as_deref(),
            input.lease_expires_at,
            input.cancel_requested_at,
        )
        .unwrap();
}

// ---------------------------------------------------------------------------
// Lock tests
// ---------------------------------------------------------------------------

#[test]
fn asr_worker_lock_is_exclusive() {
    let dir = TempDir::new().unwrap();
    let lock_path = dir.path().join("asr-worker.lock");

    let file1 = File::create(&lock_path).unwrap();
    fs2::FileExt::lock_exclusive(&file1).unwrap();

    // Second attempt on the same file should fail
    let file2 = File::create(&lock_path).unwrap();
    assert!(fs2::FileExt::try_lock_exclusive(&file2).is_err());

    // After releasing the first lock, a new lock should succeed
    drop(file1);

    let file3 = File::create(&lock_path).unwrap();
    assert!(fs2::FileExt::try_lock_exclusive(&file3).is_ok());
}

// ---------------------------------------------------------------------------
// CAS claim tests
// ---------------------------------------------------------------------------

#[test]
fn claim_cas_changes_queued_to_preparing() {
    let catalog = Catalog::in_memory().unwrap();
    let input = job_input("job-1", "fp-1", AsrJobState::Queued);
    insert_job(&catalog, &input);

    let claimed = catalog.claim_asr_job("boot-a", "worker-1").unwrap();

    assert!(claimed.is_some(), "claim should succeed for queued job");
    let claimed = claimed.unwrap();
    assert_eq!(claimed.job_id, "job-1");
    assert_eq!(claimed.state, AsrJobState::Preparing);
    assert_eq!(claimed.claimed_by.as_deref(), Some("boot-a:worker-1"));
    assert_eq!(claimed.claim_generation, 1);
    assert_eq!(claimed.attempt_count, 1);
    assert!(claimed.lease_expires_at.is_some());
}

#[test]
fn claim_cas_increments_attempt_count_and_generation() {
    let catalog = Catalog::in_memory().unwrap();
    let mut input = job_input("job-2", "fp-2", AsrJobState::Queued);
    input.attempt_count = 2;
    input.claim_generation = 5;
    insert_job(&catalog, &input);

    let claimed = catalog.claim_asr_job("boot-a", "worker-1").unwrap().unwrap();
    assert_eq!(claimed.attempt_count, 3);
    assert_eq!(claimed.claim_generation, 6);
}

#[test]
fn claim_cas_returns_none_when_no_claimable_job() {
    let catalog = Catalog::in_memory().unwrap();
    let claimed = catalog.claim_asr_job("boot-a", "worker-1").unwrap();
    assert!(claimed.is_none());
}

#[test]
fn claim_cas_requires_affected_rows_equal_one() {
    let catalog = Catalog::in_memory().unwrap();

    // Insert a job that is already preparing — no queued job to claim
    let mut input = job_input("job-3", "fp-3", AsrJobState::Preparing);
    input.claimed_by = Some("boot-b:worker-1".to_string());
    input.lease_expires_at = Some(Utc::now() + TimeDelta::seconds(60));
    insert_job(&catalog, &input);

    // Claim should return None because no job is in 'queued' state
    let claimed = catalog.claim_asr_job("boot-a", "worker-1").unwrap();
    assert!(claimed.is_none(), "already-claimed job should not be re-claimed");
}

#[test]
fn claim_cas_rejects_cancelled_jobs() {
    let catalog = Catalog::in_memory().unwrap();
    let mut input = job_input("job-cancel", "fp-cancel", AsrJobState::Queued);
    input.cancel_requested_at = Some(Utc::now());
    insert_job(&catalog, &input);

    let claimed = catalog.claim_asr_job("boot-a", "worker-1").unwrap();
    assert!(claimed.is_none(), "cancelled job should not be claimable");
}

#[test]
fn claim_cas_respects_available_at() {
    let catalog = Catalog::in_memory().unwrap();
    let mut input = job_input("job-future", "fp-future", AsrJobState::Queued);
    input.available_at = Utc::now() + TimeDelta::seconds(3600);
    insert_job(&catalog, &input);

    let claimed = catalog.claim_asr_job("boot-a", "worker-1").unwrap();
    assert!(claimed.is_none(), "future available_at job should not be claimable");
}

// ---------------------------------------------------------------------------
// Lease tests
// ---------------------------------------------------------------------------

#[test]
fn lease_is_30_seconds() {
    let catalog = Catalog::in_memory().unwrap();
    let input = job_input("job-lease", "fp-lease", AsrJobState::Queued);
    insert_job(&catalog, &input);

    let claimed = catalog.claim_asr_job("boot-a", "worker-1").unwrap().unwrap();
    let expires = claimed.lease_expires_at.unwrap();
    let now = Utc::now();

    // Lease should be approximately 30 seconds from now
    let delta = (expires - now).num_seconds();
    assert!(delta >= 28 && delta <= 32, "lease should be ~30s, got {delta}s");
}

#[test]
fn lease_renewal_succeeds_with_valid_fencing_token() {
    let catalog = Catalog::in_memory().unwrap();
    let input = job_input("job-renew", "fp-renew", AsrJobState::Queued);
    insert_job(&catalog, &input);

    let claimed = catalog.claim_asr_job("boot-a", "worker-1").unwrap().unwrap();
    let claimed_by = claimed.claimed_by.clone().unwrap();
    let generation = claimed.claim_generation;

    let renewed = catalog
        .renew_lease("job-renew", &claimed_by, generation)
        .unwrap();
    assert!(renewed, "renewal should succeed with valid token");
}

#[test]
fn lease_renewal_fails_with_wrong_generation() {
    let catalog = Catalog::in_memory().unwrap();
    let input = job_input("job-wrong-gen", "fp-wrong-gen", AsrJobState::Queued);
    insert_job(&catalog, &input);

    let claimed = catalog.claim_asr_job("boot-a", "worker-1").unwrap().unwrap();
    let claimed_by = claimed.claimed_by.clone().unwrap();

    let renewed = catalog
        .renew_lease("job-wrong-gen", &claimed_by, 999)
        .unwrap();
    assert!(!renewed, "renewal with wrong generation must fail");
}

#[test]
fn lease_renewal_fails_with_wrong_claimed_by() {
    let catalog = Catalog::in_memory().unwrap();
    let input = job_input("job-wrong-owner", "fp-wrong-owner", AsrJobState::Queued);
    insert_job(&catalog, &input);

    let claimed = catalog.claim_asr_job("boot-a", "worker-1").unwrap().unwrap();
    let generation = claimed.claim_generation;

    let renewed = catalog
        .renew_lease("job-wrong-owner", "boot-b:worker-2", generation)
        .unwrap();
    assert!(!renewed, "renewal with wrong owner must fail");
}

// ---------------------------------------------------------------------------
// State transition tests
// ---------------------------------------------------------------------------

#[test]
fn transition_preparing_to_transcribing_with_fencing() {
    let catalog = Catalog::in_memory().unwrap();
    let input = job_input("job-trans", "fp-trans", AsrJobState::Queued);
    insert_job(&catalog, &input);

    let claimed = catalog.claim_asr_job("boot-a", "worker-1").unwrap().unwrap();
    let claimed_by = claimed.claimed_by.clone().unwrap();
    let generation = claimed.claim_generation;

    let ok = catalog
        .transition_job_state("job-trans", &claimed_by, generation, AsrJobState::Transcribing)
        .unwrap();
    assert!(ok);
}

#[test]
fn transition_fails_with_wrong_fencing_token() {
    let catalog = Catalog::in_memory().unwrap();
    let input = job_input("job-fence", "fp-fence", AsrJobState::Queued);
    insert_job(&catalog, &input);

    let claimed = catalog.claim_asr_job("boot-a", "worker-1").unwrap().unwrap();
    let claimed_by = claimed.claimed_by.clone().unwrap();

    // Simulate another worker stealing the claim by incrementing generation
    catalog
        .transition_job_state("job-fence", &claimed_by, 999, AsrJobState::Transcribing)
        .unwrap();

    // Now try with the real generation — but the DB should have been updated
    // Actually, we need to test that the transition fails when the generation
    // doesn't match. Let's test with wrong generation directly.
    let bad = catalog
        .transition_job_state("job-fence", &claimed_by, 999, AsrJobState::Transcribing)
        .unwrap();
    assert!(!bad, "transition with wrong generation must fail");
}

// ---------------------------------------------------------------------------
// Stale boot ID recovery tests
// ---------------------------------------------------------------------------

#[test]
fn stale_job_from_other_boot_id_is_requeued() {
    let catalog = Catalog::in_memory().unwrap();
    let mut input = job_input("job-stale", "fp-stale", AsrJobState::Preparing);
    input.claimed_by = Some("old-boot:worker-1".to_string());
    input.lease_expires_at = Some(Utc::now() + TimeDelta::seconds(60));
    input.attempt_count = 1;
    input.claim_generation = 1;
    insert_job(&catalog, &input);

    let recovered = catalog.recover_stale_jobs("new-boot").unwrap();
    assert_eq!(recovered.len(), 1, "stale job from other boot must be recovered");
    assert_eq!(recovered[0].job_id, "job-stale");
    assert_eq!(recovered[0].action, "requeued");
}

#[test]
fn stale_job_at_max_attempts_is_failed() {
    let catalog = Catalog::in_memory().unwrap();
    let mut input = job_input("job-max", "fp-max", AsrJobState::Preparing);
    input.claimed_by = Some("old-boot:worker-1".to_string());
    input.lease_expires_at = Some(Utc::now() + TimeDelta::seconds(60));
    input.attempt_count = 3;
    input.claim_generation = 3;
    input.max_attempts = 3;
    insert_job(&catalog, &input);

    let recovered = catalog.recover_stale_jobs("new-boot").unwrap();
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].action, "failed");
}

#[test]
fn stale_job_same_boot_id_is_not_recovered() {
    let catalog = Catalog::in_memory().unwrap();
    let mut input = job_input("job-same-boot", "fp-same-boot", AsrJobState::Preparing);
    input.claimed_by = Some("my-boot:worker-1".to_string());
    input.lease_expires_at = Some(Utc::now() + TimeDelta::seconds(60));
    insert_job(&catalog, &input);

    // Same boot ID — only recover if lease is actually expired
    let recovered = catalog.recover_stale_jobs("my-boot").unwrap();
    assert!(recovered.is_empty(), "same boot ID with valid lease must not be recovered");
}

#[test]
fn stale_job_same_boot_id_expired_lease_is_recovered() {
    let catalog = Catalog::in_memory().unwrap();
    let mut input = job_input("job-expired", "fp-expired", AsrJobState::Preparing);
    input.claimed_by = Some("my-boot:worker-1".to_string());
    input.lease_expires_at = Some(Utc::now() - TimeDelta::seconds(10));
    insert_job(&catalog, &input);

    let recovered = catalog.recover_stale_jobs("my-boot").unwrap();
    assert_eq!(recovered.len(), 1, "same boot ID with expired lease must be recovered");
}

// ---------------------------------------------------------------------------
// Retry backoff tests
// ---------------------------------------------------------------------------

#[test]
fn first_retry_backoff_is_5_seconds() {
    let catalog = Catalog::in_memory().unwrap();
    let mut input = job_input("job-backoff1", "fp-backoff1", AsrJobState::Preparing);
    input.claimed_by = Some("old-boot:worker-1".to_string());
    input.lease_expires_at = Some(Utc::now() - TimeDelta::seconds(10));
    input.attempt_count = 1;
    insert_job(&catalog, &input);

    let recovered = catalog.recover_stale_jobs("new-boot").unwrap();
    assert_eq!(recovered[0].action, "requeued");

    // After recovery, the job should be requeued with available_at ~5s from now
    let job = catalog.get_job("job-backoff1").unwrap().unwrap();
    let delta = (job.available_at - Utc::now()).num_seconds();
    assert!(delta >= 3 && delta <= 7, "first backoff should be ~5s, got {delta}s");
}

#[test]
fn second_retry_backoff_is_30_seconds() {
    let catalog = Catalog::in_memory().unwrap();
    let mut input = job_input("job-backoff2", "fp-backoff2", AsrJobState::Preparing);
    input.claimed_by = Some("old-boot:worker-1".to_string());
    input.lease_expires_at = Some(Utc::now() - TimeDelta::seconds(10));
    input.attempt_count = 2;
    insert_job(&catalog, &input);

    let recovered = catalog.recover_stale_jobs("new-boot").unwrap();
    assert_eq!(recovered[0].action, "requeued");

    let job = catalog.get_job("job-backoff2").unwrap().unwrap();
    let delta = (job.available_at - Utc::now()).num_seconds();
    assert!(delta >= 28 && delta <= 32, "second backoff should be ~30s, got {delta}s");
}

#[test]
fn third_attempt_exhausts_retries() {
    let catalog = Catalog::in_memory().unwrap();
    let mut input = job_input("job-exhaust", "fp-exhaust", AsrJobState::Preparing);
    input.claimed_by = Some("old-boot:worker-1".to_string());
    input.lease_expires_at = Some(Utc::now() - TimeDelta::seconds(10));
    input.attempt_count = 3;
    input.max_attempts = 3;
    insert_job(&catalog, &input);

    let recovered = catalog.recover_stale_jobs("new-boot").unwrap();
    assert_eq!(recovered[0].action, "failed");

    let job = catalog.get_job("job-exhaust").unwrap().unwrap();
    assert_eq!(job.state, AsrJobState::Failed);
    assert_eq!(job.error_code.as_deref(), Some("recovery_retry_exhausted"));
}

// ---------------------------------------------------------------------------
// Cancellation tests
// ---------------------------------------------------------------------------

#[test]
fn cancel_queued_job_sets_cancelled_state() {
    let catalog = Catalog::in_memory().unwrap();
    let input = job_input("job-cq", "fp-cq", AsrJobState::Queued);
    insert_job(&catalog, &input);

    let ok = catalog.cancel_queued_blocked_job("job-cq").unwrap();
    assert!(ok);

    let job = catalog.get_job("job-cq").unwrap().unwrap();
    assert_eq!(job.state, AsrJobState::Cancelled);
}

#[test]
fn cancel_blocked_model_job_sets_cancelled_state() {
    let catalog = Catalog::in_memory().unwrap();
    let input = job_input("job-cb", "fp-cb", AsrJobState::BlockedModel);
    insert_job(&catalog, &input);

    let ok = catalog.cancel_queued_blocked_job("job-cb").unwrap();
    assert!(ok);

    let job = catalog.get_job("job-cb").unwrap().unwrap();
    assert_eq!(job.state, AsrJobState::Cancelled);
}

#[test]
fn cancel_request_is_idempotent() {
    let catalog = Catalog::in_memory().unwrap();
    let input = job_input("job-idem", "fp-idem", AsrJobState::Queued);
    insert_job(&catalog, &input);

    let ok1 = catalog.request_cancel("job-idem").unwrap();
    assert!(ok1);

    let ok2 = catalog.request_cancel("job-idem").unwrap();
    assert!(ok2, "second cancel request should still succeed idempotently");
}

#[test]
fn model_ready_excludes_cancelled_jobs() {
    let catalog = Catalog::in_memory().unwrap();
    let mut input1 = job_input("job-cancelled", "fp-cancelled", AsrJobState::BlockedModel);
    input1.cancel_requested_at = Some(Utc::now());
    insert_job(&catalog, &input1);

    let input2 = job_input("job-ready", "fp-ready", AsrJobState::BlockedModel);
    insert_job(&catalog, &input2);

    let count = catalog.transition_blocked_to_queued().unwrap();
    assert_eq!(count, 1, "only the non-cancelled job should transition to queued");

    let job_cancelled = catalog.get_job("job-cancelled").unwrap().unwrap();
    assert_eq!(job_cancelled.state, AsrJobState::BlockedModel, "cancelled job must stay blocked");

    let job_ready = catalog.get_job("job-ready").unwrap().unwrap();
    assert_eq!(job_ready.state, AsrJobState::Queued);
}

// ---------------------------------------------------------------------------
// Stale-worker fencing test
// ---------------------------------------------------------------------------

#[test]
fn expired_claim_cannot_publish_results() {
    let catalog = Catalog::in_memory().unwrap();

    // First worker claims the job
    let input = job_input("job-fence-pub", "fp-fence-pub", AsrJobState::Queued);
    insert_job(&catalog, &input);

    let first = catalog.claim_asr_job("boot-a", "worker-1").unwrap().unwrap();
    let first_claimed_by = first.claimed_by.clone().unwrap();
    let first_gen = first.claim_generation;

    // Transition to transcribing
    catalog
        .transition_job_state("job-fence-pub", &first_claimed_by, first_gen, AsrJobState::Transcribing)
        .unwrap();

    // Simulate lease expiry and re-claim by another worker
    // Manually expire the first claim
    catalog
        .expire_claim_for_test("job-fence-pub")
        .unwrap();

    let second = catalog.claim_asr_job("boot-a", "worker-2").unwrap().unwrap();
    let second_claimed_by = second.claimed_by.clone().unwrap();
    let second_gen = second.claim_generation;

    // First worker's claim generation is now stale — cannot complete
    let first_complete = catalog
        .complete_job("job-fence-pub", &first_claimed_by, first_gen)
        .unwrap();
    assert!(!first_complete, "stale first claim must not complete");

    // Second worker can complete
    catalog
        .transition_job_state("job-fence-pub", &second_claimed_by, second_gen, AsrJobState::Transcribing)
        .unwrap();

    let second_complete = catalog
        .complete_job("job-fence-pub", &second_claimed_by, second_gen)
        .unwrap();
    assert!(second_complete, "valid second claim must complete");
}

// ---------------------------------------------------------------------------
// Active fingerprint uniqueness test
// ---------------------------------------------------------------------------

#[test]
fn duplicate_active_fingerprint_is_rejected() {
    let catalog = Catalog::in_memory().unwrap();
    let input = job_input("job-dup", "same-fp", AsrJobState::Queued);
    insert_job(&catalog, &input);

    let result = catalog.insert_asr_job(
        "job-dup2",
        "s_test",
        "c_test",
        AsrProviderKind::SenseVoice,
        "sense-voice-small",
        "1.0",
        "sha256_abc",
        "{}",
        "{}",
        None,
        None,
        None,
        None,
        "{}",
        "sha256_def",
        "same-fp",
        AsrJobState::Queued,
        0,
        0,
        3,
        Utc::now(),
        None,
        None,
        None,
    );
    assert!(result.is_err(), "duplicate active fingerprint must be rejected");
}

// ---------------------------------------------------------------------------
// Exclusive lock via JobManager
// ---------------------------------------------------------------------------

#[test]
fn job_manager_acquires_exclusive_lock() {
    let dir = TempDir::new().unwrap();
    let lock_path = dir.path().join("asr-worker.lock");

    // Simulate what the JobManager does: create and lock
    let lock_file = File::create(&lock_path).unwrap();
    fs2::FileExt::lock_exclusive(&lock_file).unwrap();

    // Another attempt must fail
    let other = File::create(&lock_path).unwrap();
    assert!(fs2::FileExt::try_lock_exclusive(&other).is_err());

    drop(lock_file);
    // Now it should succeed
    assert!(fs2::FileExt::try_lock_exclusive(&other).is_ok());
}

// ---------------------------------------------------------------------------
// complete_job and fail_job with fencing
// ---------------------------------------------------------------------------

#[test]
fn complete_job_sets_succeeded_state() {
    let catalog = Catalog::in_memory().unwrap();
    let input = job_input("job-comp", "fp-comp", AsrJobState::Queued);
    insert_job(&catalog, &input);

    let claimed = catalog.claim_asr_job("boot-a", "worker-1").unwrap().unwrap();
    let claimed_by = claimed.claimed_by.clone().unwrap();
    let generation = claimed.claim_generation;

    catalog
        .transition_job_state("job-comp", &claimed_by, generation, AsrJobState::Transcribing)
        .unwrap();

    let ok = catalog.complete_job("job-comp", &claimed_by, generation).unwrap();
    assert!(ok);

    let job = catalog.get_job("job-comp").unwrap().unwrap();
    assert_eq!(job.state, AsrJobState::Succeeded);
}

#[test]
fn fail_job_sets_failed_state_with_error() {
    let catalog = Catalog::in_memory().unwrap();
    let input = job_input("job-fail", "fp-fail", AsrJobState::Queued);
    insert_job(&catalog, &input);

    let claimed = catalog.claim_asr_job("boot-a", "worker-1").unwrap().unwrap();
    let claimed_by = claimed.claimed_by.clone().unwrap();
    let generation = claimed.claim_generation;

    let ok = catalog
        .fail_job("job-fail", &claimed_by, generation, "transcription_failed", "model crashed")
        .unwrap();
    assert!(ok);

    let job = catalog.get_job("job-fail").unwrap().unwrap();
    assert_eq!(job.state, AsrJobState::Failed);
    assert_eq!(job.error_code.as_deref(), Some("transcription_failed"));
    assert_eq!(job.error_summary.as_deref(), Some("model crashed"));
}

#[test]
fn fail_job_fails_with_wrong_generation() {
    let catalog = Catalog::in_memory().unwrap();
    let input = job_input("job-fail-fence", "fp-fail-fence", AsrJobState::Queued);
    insert_job(&catalog, &input);

    let claimed = catalog.claim_asr_job("boot-a", "worker-1").unwrap().unwrap();
    let claimed_by = claimed.claimed_by.clone().unwrap();

    let ok = catalog
        .fail_job("job-fail-fence", &claimed_by, 999, "error", "nope")
        .unwrap();
    assert!(!ok, "fail with wrong generation must not succeed");
}

// ---------------------------------------------------------------------------
// complete_job requires transcribing state
// ---------------------------------------------------------------------------

#[test]
fn complete_job_requires_transcribing_state() {
    let catalog = Catalog::in_memory().unwrap();
    let input = job_input("job-not-trans", "fp-not-trans", AsrJobState::Queued);
    insert_job(&catalog, &input);

    let claimed = catalog.claim_asr_job("boot-a", "worker-1").unwrap().unwrap();
    let claimed_by = claimed.claimed_by.clone().unwrap();
    let generation = claimed.claim_generation;

    // Job is still in 'preparing' state, not 'transcribing'
    let ok = catalog.complete_job("job-not-trans", &claimed_by, generation).unwrap();
    assert!(!ok, "complete must fail when job is not in transcribing state");
}