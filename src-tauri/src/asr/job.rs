use chrono::{DateTime, Duration, SecondsFormat, Utc};
use serde::Serialize;

use crate::catalog::Catalog;
use crate::catalog::jobs::{CancelResult, ReadyModel, RetryResult};
use crate::domain::AsrErrorCode;
use crate::service::{RuntimeOwnershipError, RuntimeOwnershipGuard};

const LEASE_SECONDS: i64 = 30;
const RENEW_SECONDS: i64 = 5;

pub trait Clock: Clone {
    fn now(&self) -> DateTime<Utc>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimToken {
    pub job_id: String,
    pub claimed_by: String,
    pub claim_generation: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimedJob {
    pub id: String,
    pub attempt_count: i64,
    pub renew_at: DateTime<Utc>,
    pub lease_expires_at: DateTime<Utc>,
    pub token: ClaimToken,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelReadiness {
    pub provider: String,
    pub model_id: String,
    pub manifest_version: String,
    pub bundle_identity: String,
    pub runtime_identity_json: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CancellationOutcome {
    Cancelled,
    Requested,
    Unchanged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecoveryOutcome {
    pub requeued: usize,
    pub cancelled: usize,
    pub exhausted: usize,
}

#[derive(Debug)]
pub enum JobError {
    Ownership(RuntimeOwnershipError),
    Catalog(rusqlite::Error),
    OwnershipLost,
    InvalidTransition,
    ModelNotReady,
}

pub struct JobRepository<'a, C = SystemClock> {
    catalog: &'a Catalog,
    ownership: &'a RuntimeOwnershipGuard,
    boot_id: String,
    clock: C,
}

#[derive(Serialize)]
struct ClaimedBy<'a> {
    boot_id: &'a str,
    worker_id: &'a str,
}

impl<'a, C: Clock> JobRepository<'a, C> {
    pub fn new(
        catalog: &'a Catalog,
        ownership: &'a RuntimeOwnershipGuard,
        boot_id: impl Into<String>,
        clock: C,
    ) -> Self {
        Self {
            catalog,
            ownership,
            boot_id: boot_id.into(),
            clock,
        }
    }

    pub fn claim(&self, worker_id: &str) -> Result<Option<ClaimedJob>, JobError> {
        self.ensure_owner()?;
        let now = self.clock.now();
        let lease_expires_at = now + Duration::seconds(LEASE_SECONDS);
        let claimed_by = serde_json::to_string(&ClaimedBy {
            boot_id: &self.boot_id,
            worker_id,
        })
        .expect("claimed-by serialization is infallible");
        self.catalog
            .claim_asr_job(
                &claimed_by,
                &canonical_time(now),
                &canonical_time(lease_expires_at),
            )
            .map(|row| {
                row.map(|row| ClaimedJob {
                    id: row.id.clone(),
                    attempt_count: row.attempt_count,
                    renew_at: now + Duration::seconds(RENEW_SECONDS),
                    lease_expires_at,
                    token: ClaimToken {
                        job_id: row.id,
                        claimed_by,
                        claim_generation: row.claim_generation,
                    },
                })
            })
            .map_err(JobError::Catalog)
    }

    pub fn renew(&self, token: &ClaimToken) -> Result<(), JobError> {
        self.ensure_owner()?;
        let now = self.clock.now();
        let changed = self
            .catalog
            .renew_asr_job(
                &token.job_id,
                &token.claimed_by,
                token.claim_generation,
                &canonical_time(now),
                &canonical_time(now + Duration::seconds(LEASE_SECONDS)),
            )
            .map_err(JobError::Catalog)?;
        owned(changed)
    }

    pub fn mark_transcribing(&self, token: &ClaimToken) -> Result<(), JobError> {
        self.ensure_owner()?;
        let changed = self
            .catalog
            .mark_asr_job_transcribing(
                &token.job_id,
                &token.claimed_by,
                token.claim_generation,
                &canonical_time(self.clock.now()),
            )
            .map_err(JobError::Catalog)?;
        owned(changed)
    }

    pub fn fail(
        &self,
        token: &ClaimToken,
        error_code: AsrErrorCode,
        error_summary: &str,
    ) -> Result<(), JobError> {
        self.ensure_owner()?;
        let changed = self
            .catalog
            .fail_asr_job(
                &token.job_id,
                &token.claimed_by,
                token.claim_generation,
                &canonical_time(self.clock.now()),
                error_name(error_code),
                error_summary,
            )
            .map_err(JobError::Catalog)?;
        owned(changed)
    }

    pub fn request_cancel(&self, job_id: &str) -> Result<CancellationOutcome, JobError> {
        self.ensure_owner()?;
        self.catalog
            .request_asr_job_cancel(job_id, &canonical_time(self.clock.now()))
            .map(|result| match result {
                CancelResult::Cancelled => CancellationOutcome::Cancelled,
                CancelResult::Requested => CancellationOutcome::Requested,
                CancelResult::Unchanged => CancellationOutcome::Unchanged,
            })
            .map_err(JobError::Catalog)
    }

    pub fn acknowledge_cancel(&self, token: &ClaimToken) -> Result<(), JobError> {
        self.ensure_owner()?;
        let changed = self
            .catalog
            .acknowledge_asr_job_cancel(
                &token.job_id,
                &token.claimed_by,
                token.claim_generation,
                &canonical_time(self.clock.now()),
            )
            .map_err(JobError::Catalog)?;
        owned(changed)
    }

    pub fn recover(&self) -> Result<RecoveryOutcome, JobError> {
        self.ensure_owner()?;
        let now = self.clock.now();
        let counts = self
            .catalog
            .recover_asr_jobs(&self.boot_id, &canonical_time(now), |attempt| {
                let delay = if attempt <= 1 { 5 } else { 30 };
                canonical_time(now + Duration::seconds(delay))
            })
            .map_err(JobError::Catalog)?;
        Ok(RecoveryOutcome {
            requeued: counts.requeued,
            cancelled: counts.cancelled,
            exhausted: counts.exhausted,
        })
    }

    pub fn is_ready_to_retry(
        &self,
        job_id: &str,
        readiness: &ModelReadiness,
    ) -> Result<bool, JobError> {
        self.ensure_owner()?;
        self.catalog
            .asr_job_model_ready(job_id, &ready_model(readiness))
            .map_err(JobError::Catalog)
    }

    pub fn retry(&self, job_id: &str, readiness: &ModelReadiness) -> Result<i64, JobError> {
        self.ensure_owner()?;
        match self
            .catalog
            .retry_asr_job(
                job_id,
                &ready_model(readiness),
                &canonical_time(self.clock.now()),
            )
            .map_err(JobError::Catalog)?
        {
            RetryResult::Retried(generation) => Ok(generation),
            RetryResult::InvalidTransition => Err(JobError::InvalidTransition),
            RetryResult::ModelNotReady => Err(JobError::ModelNotReady),
        }
    }

    fn ensure_owner(&self) -> Result<(), JobError> {
        self.ownership.ensure_current().map_err(JobError::Ownership)
    }
}

fn ready_model(readiness: &ModelReadiness) -> ReadyModel<'_> {
    ReadyModel {
        provider: &readiness.provider,
        model_id: &readiness.model_id,
        manifest_version: &readiness.manifest_version,
        bundle_identity: &readiness.bundle_identity,
        runtime_identity_json: &readiness.runtime_identity_json,
    }
}

fn canonical_time(time: DateTime<Utc>) -> String {
    time.to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn owned(changed: usize) -> Result<(), JobError> {
    if changed == 1 {
        Ok(())
    } else {
        Err(JobError::OwnershipLost)
    }
}

fn error_name(code: AsrErrorCode) -> &'static str {
    match code {
        AsrErrorCode::ModelNotInstalled => "model_not_installed",
        AsrErrorCode::ModelCapabilityUnavailable => "model_capability_unavailable",
        AsrErrorCode::ModelDownloadFailed => "model_download_failed",
        AsrErrorCode::ModelIntegrityFailed => "model_integrity_failed",
        AsrErrorCode::InsufficientDiskSpace => "insufficient_disk_space",
        AsrErrorCode::UnsupportedOrCorruptAudio => "unsupported_or_corrupt_audio",
        AsrErrorCode::InputIntegrityFailed => "input_integrity_failed",
        AsrErrorCode::InputUnavailable => "input_unavailable",
        AsrErrorCode::InvalidProviderParameter => "invalid_provider_parameter",
        AsrErrorCode::ProviderInitializationFailed => "provider_initialization_failed",
        AsrErrorCode::TranscriptionFailed => "transcription_failed",
        AsrErrorCode::Cancelled => "cancelled",
        AsrErrorCode::RecoveryRequired => "recovery_required",
        AsrErrorCode::RecoveryRetryExhausted => "recovery_retry_exhausted",
        AsrErrorCode::ReceiptInvalid => "receipt_invalid",
    }
}
