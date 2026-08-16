use chrono::{DateTime, Duration, SecondsFormat, Utc};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::catalog::Catalog;
use crate::catalog::PublicationError;
#[cfg(test)]
use crate::catalog::PublicationFailurePoint;
use crate::catalog::jobs::OwnedMutationResult;
use crate::catalog::jobs::{CancelResult, JobCatalog, JobCatalogError, ReadyModel, RetryResult};
use crate::domain::AsrErrorCode;
use crate::domain::{ProviderReceipt, TranscriptRevision, TranscriptSegmentPublication};
use crate::service::{JobOwnershipCapability, RuntimeOwnershipError};

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
    CancelRequested,
    CoordinatorAlreadyActive,
    InvalidTransition,
    ModelNotReady,
}

pub(crate) struct JobRepository<'a, C = SystemClock> {
    jobs: JobCatalog<'a>,
    boot_id: String,
    clock: C,
}

pub struct JobCoordinator<'a, C = SystemClock> {
    repository: JobRepository<'a, C>,
    worker_id: String,
    active: Option<ClaimedJob>,
    _reservation: JobCoordinatorReservation<'a>,
}

pub struct JobControl<'a, C = SystemClock> {
    repository: JobRepository<'a, C>,
}

pub(crate) struct JobCoordinatorReservation<'a> {
    reserved: &'a AtomicBool,
}

impl<'a> JobCoordinatorReservation<'a> {
    pub(crate) fn try_reserve(reserved: &'a AtomicBool) -> Result<Self, JobError> {
        reserved
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| JobError::CoordinatorAlreadyActive)?;
        Ok(Self { reserved })
    }
}

impl Drop for JobCoordinatorReservation<'_> {
    fn drop(&mut self) {
        self.reserved.store(false, Ordering::Release);
    }
}

#[derive(Serialize)]
struct ClaimedBy<'a> {
    boot_id: &'a str,
    worker_id: &'a str,
}

impl<'a, C: Clock> JobRepository<'a, C> {
    pub(crate) fn from_core(
        catalog: &'a Catalog,
        capability: JobOwnershipCapability<'a>,
        boot_id: impl Into<String>,
        clock: C,
    ) -> Self {
        Self {
            jobs: JobCatalog::new(catalog, capability),
            boot_id: boot_id.into(),
            clock,
        }
    }

    pub(crate) fn claim(&self, worker_id: &str) -> Result<Option<ClaimedJob>, JobError> {
        let now = self.clock.now();
        let lease_expires_at = now + Duration::seconds(LEASE_SECONDS);
        let claimed_by = serde_json::to_string(&ClaimedBy {
            boot_id: &self.boot_id,
            worker_id,
        })
        .expect("claimed-by serialization is infallible");
        self.jobs
            .claim(
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
            .map_err(map_catalog_error)
    }

    pub(crate) fn renew(&self, token: &ClaimToken) -> Result<(), JobError> {
        let now = self.clock.now();
        let result = self
            .jobs
            .renew(
                &token.job_id,
                &token.claimed_by,
                token.claim_generation,
                &canonical_time(now),
                &canonical_time(now + Duration::seconds(LEASE_SECONDS)),
            )
            .map_err(map_catalog_error)?;
        owned_mutation(result)
    }

    pub(crate) fn mark_transcribing(&self, token: &ClaimToken) -> Result<(), JobError> {
        let result = self
            .jobs
            .mark_transcribing(
                &token.job_id,
                &token.claimed_by,
                token.claim_generation,
                &canonical_time(self.clock.now()),
            )
            .map_err(map_catalog_error)?;
        owned_mutation(result)
    }

    pub(crate) fn fail(
        &self,
        token: &ClaimToken,
        error_code: AsrErrorCode,
        error_summary: &str,
    ) -> Result<(), JobError> {
        let result = self
            .jobs
            .fail(
                &token.job_id,
                &token.claimed_by,
                token.claim_generation,
                &canonical_time(self.clock.now()),
                error_name(error_code),
                error_summary,
            )
            .map_err(map_catalog_error)?;
        owned_mutation(result)
    }

    pub(crate) fn request_cancel(&self, job_id: &str) -> Result<CancellationOutcome, JobError> {
        self.jobs
            .request_cancel(job_id, &canonical_time(self.clock.now()))
            .map(|result| match result {
                CancelResult::Cancelled => CancellationOutcome::Cancelled,
                CancelResult::Requested => CancellationOutcome::Requested,
                CancelResult::Unchanged => CancellationOutcome::Unchanged,
            })
            .map_err(map_catalog_error)
    }

    pub(crate) fn acknowledge_cancel(&self, token: &ClaimToken) -> Result<(), JobError> {
        let changed = self
            .jobs
            .acknowledge_cancel(
                &token.job_id,
                &token.claimed_by,
                token.claim_generation,
                &canonical_time(self.clock.now()),
            )
            .map_err(map_catalog_error)?;
        owned(changed)
    }

    pub(crate) fn recover(&self) -> Result<RecoveryOutcome, JobError> {
        let now = self.clock.now();
        let counts = self
            .jobs
            .recover(&self.boot_id, &canonical_time(now), |attempt| {
                let delay = if attempt <= 1 { 5 } else { 30 };
                canonical_time(now + Duration::seconds(delay))
            })
            .map_err(map_catalog_error)?;
        Ok(RecoveryOutcome {
            requeued: counts.requeued,
            cancelled: counts.cancelled,
            exhausted: counts.exhausted,
        })
    }

    pub(crate) fn publish(
        &self,
        token: &ClaimToken,
        receipt: &ProviderReceipt,
        segments: &[TranscriptSegmentPublication],
    ) -> Result<TranscriptRevision, PublicationError> {
        self.jobs
            .publish(token, receipt, segments, self.clock.now())
    }

    #[cfg(test)]
    pub(crate) fn fail_publication_at(&self, point: PublicationFailurePoint) {
        self.jobs.fail_publication_at(point);
    }

    #[cfg(test)]
    pub(crate) fn execute_fixture_sql(&self, sql: &str) -> rusqlite::Result<()> {
        self.jobs.execute_fixture_sql(sql)
    }

    fn owns_running(&self, token: &ClaimToken) -> Result<bool, JobError> {
        self.jobs
            .owns_running(&token.job_id, &token.claimed_by, token.claim_generation)
            .map_err(map_catalog_error)
    }

    pub(crate) fn is_ready_to_retry(
        &self,
        job_id: &str,
        readiness: &ModelReadiness,
    ) -> Result<bool, JobError> {
        self.jobs
            .model_ready(job_id, &ready_model(readiness))
            .map_err(map_catalog_error)
    }

    pub(crate) fn retry(&self, job_id: &str, readiness: &ModelReadiness) -> Result<i64, JobError> {
        match self
            .jobs
            .retry(
                job_id,
                &ready_model(readiness),
                &canonical_time(self.clock.now()),
            )
            .map_err(map_catalog_error)?
        {
            RetryResult::Retried(generation) => Ok(generation),
            RetryResult::InvalidTransition => Err(JobError::InvalidTransition),
            RetryResult::ModelNotReady => Err(JobError::ModelNotReady),
        }
    }
}

impl<C: Clock> JobCoordinator<'_, C> {
    pub fn publish(
        &self,
        token: &ClaimToken,
        receipt: &ProviderReceipt,
        segments: &[TranscriptSegmentPublication],
    ) -> Result<TranscriptRevision, PublicationError> {
        self.repository.publish(token, receipt, segments)
    }
}

impl<'a, C: Clock> JobControl<'a, C> {
    pub(crate) const fn from_core(repository: JobRepository<'a, C>) -> Self {
        Self { repository }
    }

    pub fn request_cancel(&self, job_id: &str) -> Result<CancellationOutcome, JobError> {
        self.repository.request_cancel(job_id)
    }

    pub fn is_ready_to_retry(
        &self,
        job_id: &str,
        readiness: &ModelReadiness,
    ) -> Result<bool, JobError> {
        self.repository.is_ready_to_retry(job_id, readiness)
    }

    pub fn retry(&self, job_id: &str, readiness: &ModelReadiness) -> Result<i64, JobError> {
        self.repository.retry(job_id, readiness)
    }
}

impl<'a, C: Clock> JobCoordinator<'a, C> {
    pub(crate) fn from_core(
        repository: JobRepository<'a, C>,
        worker_id: impl Into<String>,
        reservation: JobCoordinatorReservation<'a>,
    ) -> Self {
        Self {
            repository,
            worker_id: worker_id.into(),
            active: None,
            _reservation: reservation,
        }
    }

    pub fn claim_next(&mut self) -> Result<Option<ClaimedJob>, JobError> {
        if let Some(claim) = &self.active {
            return Ok(Some(claim.clone()));
        }
        let claim = self.repository.claim(&self.worker_id)?;
        self.active = claim.clone();
        Ok(claim)
    }

    pub const fn active_claim(&self) -> Option<&ClaimedJob> {
        self.active.as_ref()
    }

    pub fn renew_if_due(&mut self) -> Result<bool, JobError> {
        let Some(claim) = &self.active else {
            return Ok(false);
        };
        if self.repository.clock.now() < claim.renew_at {
            return Ok(false);
        }
        self.renew_active()?;
        Ok(true)
    }

    pub fn enter_transcribing(&mut self) -> Result<(), JobError> {
        self.renew_active()?;
        let token = self.active_token()?;
        let result = self.repository.mark_transcribing(&token);
        self.handle_running_result(result)
    }

    pub fn fail(&mut self, error_code: AsrErrorCode, error_summary: &str) -> Result<(), JobError> {
        self.renew_active()?;
        self.finish_fail(error_code, error_summary)
    }

    #[cfg(test)]
    pub(crate) fn fail_with_hook(
        &mut self,
        error_code: AsrErrorCode,
        error_summary: &str,
        hook: impl FnOnce(),
    ) -> Result<(), JobError> {
        self.renew_active()?;
        hook();
        self.finish_fail(error_code, error_summary)
    }

    fn finish_fail(
        &mut self,
        error_code: AsrErrorCode,
        error_summary: &str,
    ) -> Result<(), JobError> {
        let token = self.active_token()?;
        let result = self.repository.fail(&token, error_code, error_summary);
        let result = self.handle_running_result(result);
        if result.is_ok() {
            self.active = None;
        }
        result
    }

    pub fn acknowledge_cancel(&mut self) -> Result<(), JobError> {
        let token = self.active_token()?;
        let result = self.repository.acknowledge_cancel(&token);
        let result = self.clear_if_ownership_lost(result);
        if result.is_ok() {
            self.active = None;
        }
        result
    }

    pub fn recover(&mut self) -> Result<RecoveryOutcome, JobError> {
        let outcome = self.repository.recover()?;
        let Some(token) = self.active.as_ref().map(|claim| claim.token.clone()) else {
            return Ok(outcome);
        };
        match self.repository.owns_running(&token) {
            Ok(true) => {}
            Ok(false) => self.active = None,
            Err(error) => {
                self.active = None;
                return Err(error);
            }
        }
        Ok(outcome)
    }

    fn renew_active(&mut self) -> Result<(), JobError> {
        let token = self.active_token()?;
        let result = self.repository.renew(&token);
        self.handle_running_result(result)?;
        let now = self.repository.clock.now();
        let claim = self.active.as_mut().ok_or(JobError::OwnershipLost)?;
        claim.renew_at = now + Duration::seconds(RENEW_SECONDS);
        claim.lease_expires_at = now + Duration::seconds(LEASE_SECONDS);
        Ok(())
    }

    fn active_token(&self) -> Result<ClaimToken, JobError> {
        self.active
            .as_ref()
            .map(|claim| claim.token.clone())
            .ok_or(JobError::OwnershipLost)
    }

    fn clear_if_ownership_lost<T>(&mut self, result: Result<T, JobError>) -> Result<T, JobError> {
        if matches!(result, Err(JobError::OwnershipLost)) {
            self.active = None;
        }
        result
    }

    fn handle_running_result(&mut self, result: Result<(), JobError>) -> Result<(), JobError> {
        match result {
            Err(JobError::CancelRequested) => {
                self.acknowledge_cancel()?;
                Err(JobError::CancelRequested)
            }
            other => self.clear_if_ownership_lost(other),
        }
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

fn owned_mutation(result: OwnedMutationResult) -> Result<(), JobError> {
    match result {
        OwnedMutationResult::Updated => Ok(()),
        OwnedMutationResult::CancelRequested => Err(JobError::CancelRequested),
        OwnedMutationResult::OwnershipLost => Err(JobError::OwnershipLost),
    }
}

fn map_catalog_error(error: JobCatalogError) -> JobError {
    match error {
        JobCatalogError::Ownership(error) => JobError::Ownership(error),
        JobCatalogError::Catalog(error) => JobError::Catalog(error),
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
