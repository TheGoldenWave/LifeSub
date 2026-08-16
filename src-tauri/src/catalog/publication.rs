#[cfg(test)]
use std::sync::atomic::Ordering;

use chrono::{SecondsFormat, Utc};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};
use serde::Deserialize;

use crate::asr::job::ClaimToken;
use crate::domain::{
    AsrProviderKind, AudioSource, ProviderReceipt, TranscriptRevision, TranscriptSegment,
    TranscriptSegmentPublication,
};
use crate::service::RuntimeOwnershipError;

use super::Catalog;

#[derive(Debug)]
pub enum PublicationError {
    Ownership(RuntimeOwnershipError),
    Cancelled,
    OwnershipLost,
    InvalidResult(&'static str),
    Catalog(String),
}

impl PartialEq for PublicationError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Ownership(left), Self::Ownership(right)) => {
                ownership_error_name(left) == ownership_error_name(right)
            }
            (Self::Cancelled, Self::Cancelled) | (Self::OwnershipLost, Self::OwnershipLost) => true,
            (Self::InvalidResult(left), Self::InvalidResult(right)) => left == right,
            (Self::Catalog(left), Self::Catalog(right)) => left == right,
            _ => false,
        }
    }
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum PublicationFailurePoint {
    Receipt = 1,
    Revision = 2,
    ReceiptLink = 3,
    Segment = 4,
    Search = 5,
    Succeed = 6,
}

#[cfg(test)]
impl PublicationFailurePoint {
    pub(crate) const ALL: [Self; 6] = [
        Self::Receipt,
        Self::Revision,
        Self::ReceiptLink,
        Self::Segment,
        Self::Search,
        Self::Succeed,
    ];
}

#[derive(Deserialize)]
struct ReceiptRow {
    job_id: String,
    chunk_id: String,
    provider: AsrProviderKind,
    model_id: String,
    manifest_version: String,
    archive_sha256: String,
    required_file_hashes_json: String,
    model_source_json: String,
    vad_model_id: Option<String>,
    vad_manifest_version: Option<String>,
    vad_archive_sha256: Option<String>,
    vad_required_file_hashes_json: Option<String>,
    runtime_version: String,
    runtime_build_id: String,
    parameters_json: String,
    input_sha256: String,
    started_at: chrono::DateTime<Utc>,
    finished_at: chrono::DateTime<Utc>,
}

struct PublicationContext {
    session_id: String,
    chunk_id: String,
    chunk_offset_ms: i64,
    chunk_duration_ms: Option<i64>,
    chunk_source: String,
    provider: String,
    model_id: String,
    manifest_version: String,
    archive_sha256: String,
    required_file_hashes_json: String,
    model_source_json: String,
    vad_model_id: Option<String>,
    vad_manifest_version: Option<String>,
    vad_archive_sha256: Option<String>,
    vad_required_file_hashes_json: Option<String>,
    parameters_json: String,
    input_sha256: String,
}

impl Catalog {
    pub(crate) fn publish_asr_result(
        &self,
        token: &ClaimToken,
        receipt: &ProviderReceipt,
        segments: &[TranscriptSegmentPublication],
        now: chrono::DateTime<Utc>,
    ) -> Result<TranscriptRevision, PublicationError> {
        validate_segments(segments)?;
        let receipt: ReceiptRow = serde_json::from_value(
            serde_json::to_value(receipt)
                .map_err(|error| PublicationError::Catalog(error.to_string()))?,
        )
        .map_err(|error| PublicationError::Catalog(error.to_string()))?;
        if receipt.job_id != token.job_id {
            return Err(PublicationError::InvalidResult("receipt job mismatch"));
        }

        let mut connection = self.connection.lock().unwrap();
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(catalog_error)?;
        let now_text = canonical_time(now);
        let context = publication_context(&transaction, token, &now_text)?;
        validate_receipt(&receipt, &context)?;
        validate_segment_bounds(segments, context.chunk_duration_ms)?;

        let receipt_id = format!("receipt_{}", uuid::Uuid::new_v4().simple());
        transaction
            .execute(
                "INSERT INTO provider_receipts(
                   id, job_id, chunk_id, provider, model_id, manifest_version,
                   archive_sha256, required_file_hashes_json, model_source_json,
                   vad_model_id, vad_manifest_version, vad_archive_sha256,
                   vad_required_file_hashes_json, runtime_version, runtime_build_id,
                   parameters_json, input_sha256, started_at, finished_at,
                   data_destination, outcome
                 ) VALUES(
                   ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                   ?14, ?15, ?16, ?17, ?18, ?19, 'local_device', 'succeeded'
                 )",
                params![
                    receipt_id,
                    receipt.job_id,
                    receipt.chunk_id,
                    provider_name(receipt.provider),
                    receipt.model_id,
                    receipt.manifest_version,
                    receipt.archive_sha256,
                    receipt.required_file_hashes_json,
                    receipt.model_source_json,
                    receipt.vad_model_id,
                    receipt.vad_manifest_version,
                    receipt.vad_archive_sha256,
                    receipt.vad_required_file_hashes_json,
                    receipt.runtime_version,
                    receipt.runtime_build_id,
                    receipt.parameters_json,
                    receipt.input_sha256,
                    canonical_time(receipt.started_at),
                    canonical_time(receipt.finished_at),
                ],
            )
            .map_err(catalog_error)?;
        self.fail_if_requested(PublicationFailurePoint::Receipt)?;

        let revision_id = format!("tr_{}", uuid::Uuid::new_v4().simple());
        let number = transaction
            .query_row(
                "SELECT COALESCE(MAX(number), 0) + 1 FROM revisions WHERE session_id = ?1",
                [&context.session_id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(catalog_error)?;
        let created_at = now;
        transaction
            .execute(
                "INSERT INTO revisions(
                   id, session_id, number, provider, created_at, provenance_status
                 ) VALUES(?1, ?2, ?3, ?4, ?5, 'verified_local_asr')",
                params![
                    revision_id,
                    context.session_id,
                    number,
                    context.provider,
                    now_text,
                ],
            )
            .map_err(catalog_error)?;
        self.fail_if_requested(PublicationFailurePoint::Revision)?;
        transaction
            .execute(
                "INSERT INTO revision_receipts(revision_id, receipt_id) VALUES(?1, ?2)",
                params![revision_id, receipt_id],
            )
            .map_err(catalog_error)?;
        self.fail_if_requested(PublicationFailurePoint::ReceiptLink)?;

        let mut published_segments = Vec::with_capacity(segments.len());
        for segment in segments {
            if source_name(segment.source) != context.chunk_source {
                return Err(PublicationError::InvalidResult("segment source mismatch"));
            }
            let session_start_ms = context
                .chunk_offset_ms
                .checked_add(segment.chunk_start_ms)
                .ok_or(PublicationError::InvalidResult(
                    "segment timestamp overflow",
                ))?;
            let session_end_ms = context
                .chunk_offset_ms
                .checked_add(segment.chunk_end_ms)
                .ok_or(PublicationError::InvalidResult(
                    "segment timestamp overflow",
                ))?;
            transaction
                .execute(
                    "INSERT INTO segments(
                       id, revision_id, start_ms, end_ms, source, text, chunk_id,
                       chunk_start_ms, chunk_end_ms, session_start_ms, session_end_ms
                     ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?3, ?4)",
                    params![
                        segment.id,
                        revision_id,
                        session_start_ms,
                        session_end_ms,
                        source_name(segment.source),
                        segment.text,
                        context.chunk_id,
                        segment.chunk_start_ms,
                        segment.chunk_end_ms,
                    ],
                )
                .map_err(catalog_error)?;
            self.fail_if_requested(PublicationFailurePoint::Segment)?;
            transaction
                .execute(
                    "INSERT INTO segment_search(segment_id, revision_id, text)
                     VALUES(?1, ?2, ?3)",
                    params![segment.id, revision_id, segment.text],
                )
                .map_err(catalog_error)?;
            self.fail_if_requested(PublicationFailurePoint::Search)?;
            published_segments.push(TranscriptSegment {
                id: segment.id.clone(),
                start_ms: session_start_ms,
                end_ms: session_end_ms,
                source: segment.source,
                text: segment.text.clone(),
            });
        }

        let changed = transaction
            .execute(
                "UPDATE asr_jobs
                 SET state = 'succeeded', claimed_by = NULL, lease_expires_at = NULL,
                     error_code = NULL, error_summary = NULL, updated_at = ?4
                 WHERE id = ?1 AND claimed_by = ?2 AND claim_generation = ?3
                   AND state = 'transcribing'
                   AND cancel_requested_at IS NULL AND lease_expires_at > ?4",
                params![
                    token.job_id,
                    token.claimed_by,
                    token.claim_generation,
                    now_text,
                ],
            )
            .map_err(catalog_error)?;
        if changed != 1 {
            return Err(PublicationError::OwnershipLost);
        }
        self.fail_if_requested(PublicationFailurePoint::Succeed)?;
        transaction.commit().map_err(catalog_error)?;
        Ok(TranscriptRevision {
            id: revision_id,
            session_id: context.session_id,
            number,
            provider: context.provider,
            created_at,
            segments: published_segments,
        })
    }

    #[cfg(test)]
    pub(crate) fn fail_publication_at(&self, point: PublicationFailurePoint) {
        self.fail_publication_at
            .store(point as u8, Ordering::Release);
    }

    #[cfg(test)]
    fn fail_if_requested(&self, point: PublicationFailurePoint) -> Result<(), PublicationError> {
        if self
            .fail_publication_at
            .compare_exchange(point as u8, 0, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            return Err(PublicationError::Catalog(format!(
                "injected failure after {point:?}"
            )));
        }
        Ok(())
    }

    #[cfg(not(test))]
    fn fail_if_requested(&self, _point: PublicationFailurePoint) -> Result<(), PublicationError> {
        Ok(())
    }
}

fn publication_context(
    transaction: &Transaction<'_>,
    token: &ClaimToken,
    now: &str,
) -> Result<PublicationContext, PublicationError> {
    let context = transaction
        .query_row(
            "SELECT j.session_id, j.chunk_id, c.session_offset_ms, c.duration_ms, c.source, j.provider,
                    j.model_id, j.manifest_version, j.archive_sha256,
                    j.required_file_hashes_json, j.model_source_json,
                    j.vad_model_id, j.vad_manifest_version, j.vad_archive_sha256,
                    j.vad_required_file_hashes_json, j.parameters_json, j.input_sha256
             FROM asr_jobs j
             JOIN chunks c ON c.id = j.chunk_id
             WHERE j.id = ?1 AND j.claimed_by = ?2 AND j.claim_generation = ?3
               AND j.state = 'transcribing'
               AND j.cancel_requested_at IS NULL AND j.lease_expires_at > ?4",
            params![token.job_id, token.claimed_by, token.claim_generation, now],
            |row| {
                Ok(PublicationContext {
                    session_id: row.get(0)?,
                    chunk_id: row.get(1)?,
                    chunk_offset_ms: row.get(2)?,
                    chunk_duration_ms: row.get(3)?,
                    chunk_source: row.get(4)?,
                    provider: row.get(5)?,
                    model_id: row.get(6)?,
                    manifest_version: row.get(7)?,
                    archive_sha256: row.get(8)?,
                    required_file_hashes_json: row.get(9)?,
                    model_source_json: row.get(10)?,
                    vad_model_id: row.get(11)?,
                    vad_manifest_version: row.get(12)?,
                    vad_archive_sha256: row.get(13)?,
                    vad_required_file_hashes_json: row.get(14)?,
                    parameters_json: row.get(15)?,
                    input_sha256: row.get(16)?,
                })
            },
        )
        .optional()
        .map_err(catalog_error)?;
    if let Some(context) = context {
        return Ok(context);
    }
    let cancelled = transaction
        .query_row(
            "SELECT EXISTS(
               SELECT 1 FROM asr_jobs
               WHERE id = ?1 AND claimed_by = ?2 AND claim_generation = ?3
                 AND state = 'transcribing'
                 AND cancel_requested_at IS NOT NULL AND lease_expires_at > ?4
             )",
            params![token.job_id, token.claimed_by, token.claim_generation, now],
            |row| row.get::<_, bool>(0),
        )
        .map_err(catalog_error)?;
    Err(if cancelled {
        PublicationError::Cancelled
    } else {
        PublicationError::OwnershipLost
    })
}

fn validate_segments(segments: &[TranscriptSegmentPublication]) -> Result<(), PublicationError> {
    if segments.is_empty() {
        return Err(PublicationError::InvalidResult("empty transcript"));
    }
    if segments.iter().any(|segment| {
        segment.id.trim().is_empty()
            || segment.text.trim().is_empty()
            || segment.chunk_start_ms < 0
            || segment.chunk_end_ms <= segment.chunk_start_ms
    }) {
        return Err(PublicationError::InvalidResult("invalid segment"));
    }
    Ok(())
}

fn validate_segment_bounds(
    segments: &[TranscriptSegmentPublication],
    duration_ms: Option<i64>,
) -> Result<(), PublicationError> {
    if duration_ms.is_some_and(|duration| {
        segments
            .iter()
            .any(|segment| segment.chunk_end_ms > duration)
    }) {
        return Err(PublicationError::InvalidResult("segment exceeds chunk"));
    }
    Ok(())
}

fn validate_receipt(
    receipt: &ReceiptRow,
    context: &PublicationContext,
) -> Result<(), PublicationError> {
    if receipt.chunk_id != context.chunk_id
        || provider_name(receipt.provider) != context.provider
        || receipt.model_id != context.model_id
        || receipt.manifest_version != context.manifest_version
        || receipt.archive_sha256 != context.archive_sha256
        || receipt.required_file_hashes_json != context.required_file_hashes_json
        || receipt.model_source_json != context.model_source_json
        || receipt.vad_model_id != context.vad_model_id
        || receipt.vad_manifest_version != context.vad_manifest_version
        || receipt.vad_archive_sha256 != context.vad_archive_sha256
        || receipt.vad_required_file_hashes_json != context.vad_required_file_hashes_json
        || receipt.parameters_json != context.parameters_json
        || receipt.input_sha256 != context.input_sha256
    {
        return Err(PublicationError::InvalidResult("receipt identity mismatch"));
    }
    Ok(())
}

fn provider_name(provider: AsrProviderKind) -> &'static str {
    match provider {
        AsrProviderKind::SenseVoice => "sense_voice",
        AsrProviderKind::Whisper => "whisper",
        AsrProviderKind::Qwen3Asr => "qwen3_asr",
    }
}

fn source_name(source: AudioSource) -> &'static str {
    match source {
        AudioSource::Microphone => "microphone",
        AudioSource::SystemAudio => "system_audio",
        AudioSource::Imported => "imported",
    }
}

fn canonical_time(value: chrono::DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn catalog_error(error: rusqlite::Error) -> PublicationError {
    PublicationError::Catalog(error.to_string())
}

fn ownership_error_name(error: &RuntimeOwnershipError) -> &'static str {
    match error {
        RuntimeOwnershipError::AlreadyOwned => "already_owned",
        RuntimeOwnershipError::CatalogMismatch => "catalog_mismatch",
        RuntimeOwnershipError::UnsafePath => "unsafe_path",
        RuntimeOwnershipError::Io(_) => "io",
    }
}
