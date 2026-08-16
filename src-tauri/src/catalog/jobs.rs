use chrono::DateTime;
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};
use serde::Deserialize;

use crate::asr::job::ClaimToken;
use crate::catalog::PublicationError;
#[cfg(test)]
use crate::catalog::PublicationFailurePoint;
use crate::domain::{ProviderReceipt, TranscriptRevision, TranscriptSegmentPublication};
use crate::service::{JobOwnershipCapability, RuntimeOwnershipError};

use super::Catalog;

#[derive(Debug)]
pub(crate) enum JobCatalogError {
    Ownership(RuntimeOwnershipError),
    Catalog(rusqlite::Error),
}

pub(crate) struct JobCatalog<'a> {
    catalog: &'a Catalog,
    capability: JobOwnershipCapability<'a>,
}

impl<'a> JobCatalog<'a> {
    pub(crate) fn new(catalog: &'a Catalog, capability: JobOwnershipCapability<'a>) -> Self {
        Self {
            catalog,
            capability,
        }
    }

    pub(crate) fn claim(
        &self,
        claimed_by: &str,
        now: &str,
        lease_expires_at: &str,
    ) -> Result<Option<ClaimedJobRow>, JobCatalogError> {
        self.ensure()?;
        self.catalog
            .claim_asr_job(claimed_by, now, lease_expires_at)
            .map_err(JobCatalogError::Catalog)
    }

    pub(crate) fn renew(
        &self,
        id: &str,
        claimed_by: &str,
        generation: i64,
        now: &str,
        lease_expires_at: &str,
    ) -> Result<OwnedMutationResult, JobCatalogError> {
        self.ensure()?;
        self.catalog
            .renew_asr_job(id, claimed_by, generation, now, lease_expires_at)
            .map_err(JobCatalogError::Catalog)
    }

    pub(crate) fn mark_transcribing(
        &self,
        id: &str,
        claimed_by: &str,
        generation: i64,
        now: &str,
    ) -> Result<OwnedMutationResult, JobCatalogError> {
        self.ensure()?;
        self.catalog
            .mark_asr_job_transcribing(id, claimed_by, generation, now)
            .map_err(JobCatalogError::Catalog)
    }

    pub(crate) fn fail(
        &self,
        id: &str,
        claimed_by: &str,
        generation: i64,
        now: &str,
        error_code: &str,
        error_summary: &str,
    ) -> Result<OwnedMutationResult, JobCatalogError> {
        self.ensure()?;
        self.catalog
            .fail_asr_job(id, claimed_by, generation, now, error_code, error_summary)
            .map_err(JobCatalogError::Catalog)
    }

    pub(crate) fn request_cancel(
        &self,
        id: &str,
        now: &str,
    ) -> Result<CancelResult, JobCatalogError> {
        self.ensure()?;
        self.catalog
            .request_asr_job_cancel(id, now)
            .map_err(JobCatalogError::Catalog)
    }

    pub(crate) fn acknowledge_cancel(
        &self,
        id: &str,
        claimed_by: &str,
        generation: i64,
        now: &str,
    ) -> Result<usize, JobCatalogError> {
        self.ensure()?;
        self.catalog
            .acknowledge_asr_job_cancel(id, claimed_by, generation, now)
            .map_err(JobCatalogError::Catalog)
    }

    pub(crate) fn recover(
        &self,
        current_boot_id: &str,
        now: &str,
        retry_at: impl Fn(i64) -> String,
    ) -> Result<RecoveryCounts, JobCatalogError> {
        self.ensure()?;
        self.catalog
            .recover_asr_jobs(current_boot_id, now, retry_at)
            .map_err(JobCatalogError::Catalog)
    }

    pub(crate) fn owns_running(
        &self,
        id: &str,
        claimed_by: &str,
        generation: i64,
    ) -> Result<bool, JobCatalogError> {
        self.ensure()?;
        self.catalog
            .owns_running_asr_job(id, claimed_by, generation)
            .map_err(JobCatalogError::Catalog)
    }

    pub(crate) fn model_ready(
        &self,
        id: &str,
        model: &ReadyModel<'_>,
    ) -> Result<bool, JobCatalogError> {
        self.ensure()?;
        self.catalog
            .asr_job_model_ready(id, model)
            .map_err(JobCatalogError::Catalog)
    }

    pub(crate) fn retry(
        &self,
        id: &str,
        model: &ReadyModel<'_>,
        now: &str,
    ) -> Result<RetryResult, JobCatalogError> {
        self.ensure()?;
        self.catalog
            .retry_asr_job(id, model, now)
            .map_err(JobCatalogError::Catalog)
    }

    pub(crate) fn publish(
        &self,
        token: &ClaimToken,
        receipt: &ProviderReceipt,
        segments: &[TranscriptSegmentPublication],
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<TranscriptRevision, PublicationError> {
        self.ensure().map_err(|error| match error {
            JobCatalogError::Ownership(error) => PublicationError::Ownership(error),
            JobCatalogError::Catalog(error) => PublicationError::Catalog(error.to_string()),
        })?;
        self.catalog
            .publish_asr_result(token, receipt, segments, now)
    }

    #[cfg(test)]
    pub(crate) fn fail_publication_at(&self, point: PublicationFailurePoint) {
        self.catalog.fail_publication_at(point);
    }

    #[cfg(test)]
    pub(crate) fn execute_fixture_sql(&self, sql: &str) -> rusqlite::Result<()> {
        self.catalog.connection.lock().unwrap().execute_batch(sql)
    }

    fn ensure(&self) -> Result<(), JobCatalogError> {
        self.capability
            .ensure_for(self.catalog)
            .map_err(JobCatalogError::Ownership)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClaimedJobRow {
    pub id: String,
    pub attempt_count: i64,
    pub claim_generation: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RecoveryCounts {
    pub requeued: usize,
    pub cancelled: usize,
    pub exhausted: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RecoveryCandidate {
    id: String,
    claimed_by: String,
    claim_generation: i64,
    attempt_count: i64,
    max_attempts: i64,
    cancel_requested: bool,
    lease_expires_at: Option<String>,
}

#[derive(Deserialize)]
struct StoredClaimOwner {
    boot_id: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CancelResult {
    Cancelled,
    Requested,
    Unchanged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RetryResult {
    Retried(i64),
    InvalidTransition,
    ModelNotReady,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OwnedMutationResult {
    Updated,
    CancelRequested,
    OwnershipLost,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReadyModel<'a> {
    pub provider: &'a str,
    pub model_id: &'a str,
    pub manifest_version: &'a str,
    pub bundle_identity: &'a str,
    pub runtime_identity_json: &'a str,
}

impl Catalog {
    fn claim_asr_job(
        &self,
        claimed_by: &str,
        now: &str,
        lease_expires_at: &str,
    ) -> rusqlite::Result<Option<ClaimedJobRow>> {
        let mut connection = self.connection.lock().unwrap();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let candidate = transaction
            .query_row(
                "SELECT j.id
                 FROM asr_jobs j
                 JOIN chunks c ON c.id = j.chunk_id
                 WHERE j.state = 'queued'
                   AND j.cancel_requested_at IS NULL
                   AND j.available_at <= ?1
                   AND (j.lease_expires_at IS NULL OR j.lease_expires_at <= ?1)
                   AND c.integrity_state = 'available'
                   AND NOT EXISTS (
                     SELECT 1 FROM asr_jobs active
                     WHERE active.state IN ('preparing', 'transcribing')
                       AND active.claimed_by IS NOT NULL
                   )
                 ORDER BY j.available_at, j.created_at, j.id
                 LIMIT 1",
                [now],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let Some(id) = candidate else {
            transaction.commit()?;
            return Ok(None);
        };
        let changed = transaction.execute(
            "UPDATE asr_jobs
             SET state = 'preparing', claimed_by = ?2, lease_expires_at = ?3,
                 attempt_count = attempt_count + 1,
                 claim_generation = claim_generation + 1,
                 updated_at = ?1
             WHERE id = ?4
               AND state = 'queued'
               AND cancel_requested_at IS NULL
               AND available_at <= ?1
               AND (lease_expires_at IS NULL OR lease_expires_at <= ?1)
               AND EXISTS (
                 SELECT 1 FROM chunks c
                 WHERE c.id = asr_jobs.chunk_id AND c.integrity_state = 'available'
               )
               AND NOT EXISTS (
                 SELECT 1 FROM asr_jobs active
                 WHERE active.state IN ('preparing', 'transcribing')
                   AND active.claimed_by IS NOT NULL
               )",
            params![now, claimed_by, lease_expires_at, id],
        )?;
        if changed != 1 {
            transaction.commit()?;
            return Ok(None);
        }
        let claimed = transaction.query_row(
            "SELECT id, attempt_count, claim_generation FROM asr_jobs WHERE id = ?1",
            [id],
            |row| {
                Ok(ClaimedJobRow {
                    id: row.get(0)?,
                    attempt_count: row.get(1)?,
                    claim_generation: row.get(2)?,
                })
            },
        )?;
        transaction.commit()?;
        Ok(Some(claimed))
    }

    fn renew_asr_job(
        &self,
        id: &str,
        claimed_by: &str,
        generation: i64,
        now: &str,
        lease_expires_at: &str,
    ) -> rusqlite::Result<OwnedMutationResult> {
        let mut connection = self.connection.lock().unwrap();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = transaction.execute(
            "UPDATE asr_jobs
             SET lease_expires_at = ?5, updated_at = ?4
             WHERE id = ?1 AND claimed_by = ?2 AND claim_generation = ?3
               AND state IN ('preparing', 'transcribing')
               AND cancel_requested_at IS NULL
               AND lease_expires_at > ?4",
            params![id, claimed_by, generation, now, lease_expires_at],
        )?;
        let result = owned_mutation_result(&transaction, id, claimed_by, generation, now, changed)?;
        transaction.commit()?;
        Ok(result)
    }

    fn mark_asr_job_transcribing(
        &self,
        id: &str,
        claimed_by: &str,
        generation: i64,
        now: &str,
    ) -> rusqlite::Result<OwnedMutationResult> {
        let mut connection = self.connection.lock().unwrap();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = transaction.execute(
            "UPDATE asr_jobs SET state = 'transcribing', updated_at = ?4
             WHERE id = ?1 AND claimed_by = ?2 AND claim_generation = ?3
               AND state = 'preparing' AND cancel_requested_at IS NULL
               AND lease_expires_at > ?4",
            params![id, claimed_by, generation, now],
        )?;
        let result = owned_mutation_result(&transaction, id, claimed_by, generation, now, changed)?;
        transaction.commit()?;
        Ok(result)
    }

    fn fail_asr_job(
        &self,
        id: &str,
        claimed_by: &str,
        generation: i64,
        now: &str,
        error_code: &str,
        error_summary: &str,
    ) -> rusqlite::Result<OwnedMutationResult> {
        let mut connection = self.connection.lock().unwrap();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = transaction.execute(
            "UPDATE asr_jobs
             SET state = 'failed', claimed_by = NULL, lease_expires_at = NULL,
                 error_code = ?5, error_summary = ?6, updated_at = ?4
             WHERE id = ?1 AND claimed_by = ?2 AND claim_generation = ?3
               AND state IN ('preparing', 'transcribing')
               AND cancel_requested_at IS NULL AND lease_expires_at > ?4",
            params![id, claimed_by, generation, now, error_code, error_summary],
        )?;
        let result = owned_mutation_result(&transaction, id, claimed_by, generation, now, changed)?;
        transaction.commit()?;
        Ok(result)
    }

    fn request_asr_job_cancel(&self, id: &str, now: &str) -> rusqlite::Result<CancelResult> {
        let mut connection = self.connection.lock().unwrap();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let cancelled = transaction.execute(
            "UPDATE asr_jobs
             SET state = 'cancelled', cancel_requested_at = ?2, error_code = 'cancelled',
                 error_summary = NULL, claimed_by = NULL, lease_expires_at = NULL,
                 updated_at = ?2
             WHERE id = ?1 AND state IN ('queued', 'blocked_model')",
            params![id, now],
        )?;
        if cancelled == 1 {
            transaction.commit()?;
            return Ok(CancelResult::Cancelled);
        }
        let requested = transaction.execute(
            "UPDATE asr_jobs SET cancel_requested_at = ?2, updated_at = ?2
             WHERE id = ?1 AND state IN ('preparing', 'transcribing')
               AND cancel_requested_at IS NULL",
            params![id, now],
        )?;
        transaction.commit()?;
        Ok(if requested == 1 {
            CancelResult::Requested
        } else {
            CancelResult::Unchanged
        })
    }

    fn acknowledge_asr_job_cancel(
        &self,
        id: &str,
        claimed_by: &str,
        generation: i64,
        now: &str,
    ) -> rusqlite::Result<usize> {
        self.connection.lock().unwrap().execute(
            "UPDATE asr_jobs
             SET state = 'cancelled', claimed_by = NULL, lease_expires_at = NULL,
                 error_code = 'cancelled', error_summary = NULL, updated_at = ?4
             WHERE id = ?1 AND claimed_by = ?2 AND claim_generation = ?3
               AND state IN ('preparing', 'transcribing')
               AND cancel_requested_at IS NOT NULL AND lease_expires_at > ?4",
            params![id, claimed_by, generation, now],
        )
    }

    fn recover_asr_jobs(
        &self,
        current_boot_id: &str,
        now: &str,
        retry_at: impl Fn(i64) -> String,
    ) -> rusqlite::Result<RecoveryCounts> {
        let mut connection = self.connection.lock().unwrap();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let candidates = {
            let mut statement = transaction.prepare(
                "SELECT id, claimed_by, claim_generation, attempt_count, max_attempts,
                        cancel_requested_at IS NOT NULL, lease_expires_at
                 FROM asr_jobs
                 WHERE state IN ('preparing', 'transcribing')
                   AND claimed_by IS NOT NULL
                 ORDER BY created_at, id",
            )?;
            statement
                .query_map([], |row| {
                    Ok(RecoveryCandidate {
                        id: row.get(0)?,
                        claimed_by: row.get(1)?,
                        claim_generation: row.get(2)?,
                        attempt_count: row.get(3)?,
                        max_attempts: row.get(4)?,
                        cancel_requested: row.get(5)?,
                        lease_expires_at: row.get(6)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        let now_time = DateTime::parse_from_rfc3339(now).expect("canonical job clock");
        let mut counts = RecoveryCounts {
            requeued: 0,
            cancelled: 0,
            exhausted: 0,
        };
        for candidate in candidates {
            let same_boot = serde_json::from_str::<StoredClaimOwner>(&candidate.claimed_by)
                .map(|owner| owner.boot_id == current_boot_id)
                .unwrap_or(false);
            let lease_expired = candidate
                .lease_expires_at
                .as_deref()
                .and_then(|lease| DateTime::parse_from_rfc3339(lease).ok())
                .map(|lease| lease <= now_time)
                .unwrap_or(true);
            if same_boot && !lease_expired {
                continue;
            }
            let (state, available_at, error_code, error_summary) = if candidate.cancel_requested {
                counts.cancelled += 1;
                ("cancelled", now.to_owned(), Some("cancelled"), None)
            } else if candidate.attempt_count >= candidate.max_attempts {
                counts.exhausted += 1;
                (
                    "failed",
                    now.to_owned(),
                    Some("recovery_retry_exhausted"),
                    Some("ASR recovery claim limit exhausted"),
                )
            } else {
                counts.requeued += 1;
                ("queued", retry_at(candidate.attempt_count), None, None)
            };
            let changed = transaction.execute(
                "UPDATE asr_jobs
                 SET state = ?4, available_at = ?5, claimed_by = NULL,
                     lease_expires_at = NULL, error_code = ?6, error_summary = ?7,
                     updated_at = ?8
                 WHERE id = ?1 AND claimed_by = ?2 AND claim_generation = ?3
                   AND state IN ('preparing', 'transcribing')",
                params![
                    candidate.id,
                    candidate.claimed_by,
                    candidate.claim_generation,
                    state,
                    available_at,
                    error_code,
                    error_summary,
                    now,
                ],
            )?;
            if changed != 1 {
                return Err(rusqlite::Error::QueryReturnedNoRows);
            }
        }
        transaction.commit()?;
        Ok(counts)
    }

    fn owns_running_asr_job(
        &self,
        id: &str,
        claimed_by: &str,
        generation: i64,
    ) -> rusqlite::Result<bool> {
        self.connection.lock().unwrap().query_row(
            "SELECT EXISTS(
               SELECT 1 FROM asr_jobs
               WHERE id = ?1 AND claimed_by = ?2 AND claim_generation = ?3
                 AND state IN ('preparing', 'transcribing')
             )",
            params![id, claimed_by, generation],
            |row| row.get(0),
        )
    }

    fn asr_job_model_ready(&self, id: &str, model: &ReadyModel<'_>) -> rusqlite::Result<bool> {
        self.connection.lock().unwrap().query_row(
            "SELECT EXISTS(
               SELECT 1
               FROM asr_jobs j
               JOIN model_installations m ON m.model_id = j.model_id
               JOIN chunks c ON c.id = j.chunk_id
               WHERE j.id = ?1
                 AND j.provider = ?2 AND j.model_id = ?3
                 AND j.manifest_version = ?4 AND j.archive_sha256 = ?5
                 AND c.integrity_state = 'available'
                 AND m.provider = ?2 AND m.model_id = ?3
                 AND m.manifest_version = ?4 AND m.archive_sha256 = ?5
                 AND m.state = 'runtime_qualified'
                 AND m.runtime_identity_json = ?6
             )",
            params![
                id,
                model.provider,
                model.model_id,
                model.manifest_version,
                model.bundle_identity,
                model.runtime_identity_json,
            ],
            |row| row.get(0),
        )
    }

    fn retry_asr_job(
        &self,
        id: &str,
        model: &ReadyModel<'_>,
        now: &str,
    ) -> rusqlite::Result<RetryResult> {
        let mut connection = self.connection.lock().unwrap();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let state = transaction
            .query_row("SELECT state FROM asr_jobs WHERE id = ?1", [id], |row| {
                row.get::<_, String>(0)
            })
            .optional()?;
        let Some(state) = state else {
            transaction.commit()?;
            return Ok(RetryResult::InvalidTransition);
        };
        if !matches!(state.as_str(), "blocked_model" | "failed") {
            transaction.commit()?;
            return Ok(RetryResult::InvalidTransition);
        }
        let changed = transaction.execute(
            "UPDATE asr_jobs
             SET state = 'queued', attempt_count = 0,
                 claim_generation = claim_generation + 1,
                 available_at = ?7, claimed_by = NULL, lease_expires_at = NULL,
                 cancel_requested_at = NULL, error_code = NULL, error_summary = NULL,
                 updated_at = ?7
             WHERE id = ?1 AND state = ?8
               AND provider = ?2 AND model_id = ?3
               AND manifest_version = ?4 AND archive_sha256 = ?5
               AND EXISTS (
                 SELECT 1 FROM chunks c
                 WHERE c.id = asr_jobs.chunk_id AND c.integrity_state = 'available'
               )
               AND EXISTS (
                 SELECT 1 FROM model_installations m
                 WHERE m.provider = ?2 AND m.model_id = ?3
                   AND m.manifest_version = ?4 AND m.archive_sha256 = ?5
                   AND m.state = 'runtime_qualified'
                   AND m.runtime_identity_json = ?6
               )",
            params![
                id,
                model.provider,
                model.model_id,
                model.manifest_version,
                model.bundle_identity,
                model.runtime_identity_json,
                now,
                state,
            ],
        )?;
        if changed != 1 {
            transaction.commit()?;
            return Ok(RetryResult::ModelNotReady);
        }
        let generation = transaction.query_row(
            "SELECT claim_generation FROM asr_jobs WHERE id = ?1",
            [id],
            |row| row.get(0),
        )?;
        transaction.commit()?;
        Ok(RetryResult::Retried(generation))
    }
}

fn owned_mutation_result(
    transaction: &Transaction<'_>,
    id: &str,
    claimed_by: &str,
    generation: i64,
    now: &str,
    changed: usize,
) -> rusqlite::Result<OwnedMutationResult> {
    if changed == 1 {
        return Ok(OwnedMutationResult::Updated);
    }
    let cancel_requested = transaction.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM asr_jobs
           WHERE id = ?1 AND claimed_by = ?2 AND claim_generation = ?3
             AND state IN ('preparing', 'transcribing')
             AND cancel_requested_at IS NOT NULL AND lease_expires_at > ?4
         )",
        params![id, claimed_by, generation, now],
        |row| row.get(0),
    )?;
    Ok(if cancel_requested {
        OwnedMutationResult::CancelRequested
    } else {
        OwnedMutationResult::OwnershipLost
    })
}
