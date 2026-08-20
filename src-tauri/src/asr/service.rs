//! ASR service orchestration.
//!
//! The `AsrService` coordinates chunk verification, audio decoding,
//! transcription, and atomic publication of Receipts and Revisions.
//! It delegates persistence to the Catalog, which enforces fencing
//! tokens and transaction atomicity.
//!
//! Full execution pipeline (chunk hash → model resolution → audio decode →
//! transcription → atomic publish) will be completed in later tasks as
//! the provider and audio modules are implemented. Task 10 delivers the
//! atomic publication contract.

use crate::catalog::Catalog;
use crate::domain::{ProviderReceipt, TranscriptRevision, TranscriptSegment};

// ---------------------------------------------------------------------------
// Service errors
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AsrServiceError {
    CatalogError(String),
    FencingTokenMismatch,
    Cancelled,
    ChunkUnavailable,
    InputIntegrityFailed,
    EmptyTranscription,
    ModelNotInstalled(String),
    TranscriptionFailed(String),
}

impl From<rusqlite::Error> for AsrServiceError {
    fn from(value: rusqlite::Error) -> Self {
        Self::CatalogError(value.to_string())
    }
}

// ---------------------------------------------------------------------------
// ASR service
// ---------------------------------------------------------------------------

/// Orchestrates the ASR pipeline from job claim to revision publication.
pub struct AsrService {
    catalog: Catalog,
}

impl AsrService {
    pub fn new(catalog: Catalog) -> Self {
        Self { catalog }
    }

    /// Returns a shared reference to the underlying Catalog.
    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    /// Publish a completed ASR result atomically.
    ///
    /// This is the final step of the ASR pipeline. The caller must have
    /// already verified the chunk hash, decoded the audio, run the
    /// provider, and assembled non-empty segments with correct time
    /// provenance.
    ///
    /// The publication transaction verifies the fencing token, inserts
    /// the Receipt, Revision, revision_receipts link, segments with
    /// chunk-level and session-relative time coordinates, and FTS entries,
    /// and marks the job as succeeded — all in a single `BEGIN IMMEDIATE`
    /// transaction.
    #[allow(clippy::too_many_arguments)]
    pub fn publish_result(
        &self,
        job_id: &str,
        claimed_by: &str,
        claim_generation: i64,
        session_id: &str,
        provider: &str,
        receipt: &ProviderReceipt,
        segments: &[TranscriptSegment],
    ) -> Result<TranscriptRevision, AsrServiceError> {
        if segments.is_empty() {
            return Err(AsrServiceError::EmptyTranscription);
        }

        self.catalog
            .publish_asr_revision(
                job_id,
                claimed_by,
                claim_generation,
                session_id,
                provider,
                receipt,
                segments,
            )
            .map_err(|e| {
                if let rusqlite::Error::InvalidParameterName(msg) = &e {
                    if msg.contains("fencing token mismatch") {
                        return AsrServiceError::FencingTokenMismatch;
                    }
                }
                AsrServiceError::CatalogError(e.to_string())
            })
    }
}