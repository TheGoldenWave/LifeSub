use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::asr::settings::AsrProviderKind;

// ---------------------------------------------------------------------------
// Capture domain
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureState {
    Idle,
    Recording,
    Paused,
    Stopped,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioSource {
    Microphone,
    SystemAudio,
    Imported,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CaptureSession {
    pub id: String,
    pub title: String,
    pub state: CaptureState,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
}

// ---------------------------------------------------------------------------
// Transcript domain
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TranscriptSegment {
    pub id: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub source: AudioSource,
    pub text: String,
    pub chunk_id: Option<String>,
    pub chunk_start_ms: Option<i64>,
    pub chunk_end_ms: Option<i64>,
    pub session_start_ms: Option<i64>,
    pub session_end_ms: Option<i64>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceStatus {
    /// V0.1 revision produced before ASR receipts existed.
    #[default]
    LegacyUnverified,
    /// Revision produced by a verified local ASR provider with a Receipt.
    VerifiedLocalAsr,
    /// Revision text was entered or edited manually.
    Manual,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TranscriptRevision {
    pub id: String,
    pub session_id: String,
    pub number: i64,
    /// Legacy provider string — kept readable for V0.1 compatibility.
    pub provider: String,
    #[serde(default)]
    pub provenance_status: ProvenanceStatus,
    pub created_at: DateTime<Utc>,
    pub segments: Vec<TranscriptSegment>,
}

// ---------------------------------------------------------------------------
// Audio chunk domain
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AudioChunk {
    pub id: String,
    pub session_id: String,
    pub source: AudioSource,
    pub path: String,
    pub sha256: String,
    pub byte_length: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChunkIntegrityState {
    Available,
    Corrupted,
    Missing,
}

// ---------------------------------------------------------------------------
// ASR job domain — persisted as snake_case strings
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AsrJobState {
    Queued,
    BlockedModel,
    Preparing,
    Transcribing,
    Succeeded,
    Failed,
    Cancelled,
}

// ---------------------------------------------------------------------------
// Stable ASR error codes — persisted as snake_case, never Debug output
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AsrErrorCode {
    ModelNotInstalled,
    ModelIntegrityFailed,
    ModelDownloadFailed,
    InsufficientDiskSpace,
    UnsupportedOrCorruptAudio,
    InputIntegrityFailed,
    InputUnavailable,
    InvalidProviderParameter,
    ProviderInitializationFailed,
    TranscriptionFailed,
    Cancelled,
    RecoveryRequired,
}

// ---------------------------------------------------------------------------
// Provider Receipt — immutable evidence of a completed ASR job
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataDestination {
    LocalDevice,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderOutcome {
    Succeeded,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderReceipt {
    pub job_id: String,
    pub chunk_id: String,
    pub provider: AsrProviderKind,
    pub model_id: String,
    pub manifest_version: String,
    pub archive_sha256: String,
    pub required_file_hashes_json: String,
    pub model_source_json: String,
    pub vad_model_id: Option<String>,
    pub vad_manifest_version: Option<String>,
    pub vad_archive_sha256: Option<String>,
    pub vad_required_file_hashes_json: Option<String>,
    pub runtime_version: String,
    pub runtime_build_id: String,
    pub parameters_json: String,
    pub input_sha256: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub data_destination: DataDestination,
    pub outcome: ProviderOutcome,
}

// ---------------------------------------------------------------------------
// Domain errors
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DomainError {
    InvalidCaptureTransition {
        from: CaptureState,
        to: CaptureState,
    },
}

impl CaptureSession {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            id: format!("rec_{}", Uuid::new_v4().simple()),
            title: title.into(),
            state: CaptureState::Idle,
            started_at: Utc::now(),
            ended_at: None,
        }
    }

    pub fn transition(mut self, target: CaptureState) -> Result<Self, DomainError> {
        let valid = matches!(
            (self.state, target),
            (CaptureState::Idle, CaptureState::Recording)
                | (CaptureState::Recording, CaptureState::Paused)
                | (CaptureState::Recording, CaptureState::Stopped)
                | (CaptureState::Paused, CaptureState::Recording)
                | (CaptureState::Paused, CaptureState::Stopped)
        );
        if !valid {
            return Err(DomainError::InvalidCaptureTransition {
                from: self.state,
                to: target,
            });
        }
        self.state = target;
        if target == CaptureState::Stopped {
            self.ended_at = Some(Utc::now());
        }
        Ok(self)
    }

    pub fn evidence_uri(&self) -> String {
        format!("lifesub://record/{}", self.id)
    }
}

impl TranscriptSegment {
    pub fn new(start_ms: i64, end_ms: i64, source: AudioSource, text: impl Into<String>) -> Self {
        Self {
            id: format!("seg_{}", Uuid::new_v4().simple()),
            start_ms,
            end_ms,
            source,
            text: text.into(),
            chunk_id: None, chunk_start_ms: None, chunk_end_ms: None,
            session_start_ms: None, session_end_ms: None,
        }
    }

    pub fn with_chunk_provenance(mut self, chunk_id: impl Into<String>, chunk_start_ms: i64, chunk_end_ms: i64, session_offset_ms: i64) -> Self {
        self.chunk_id = Some(chunk_id.into());
        self.chunk_start_ms = Some(chunk_start_ms);
        self.chunk_end_ms = Some(chunk_end_ms);
        self.session_start_ms = Some(session_offset_ms + chunk_start_ms);
        self.session_end_ms = Some(session_offset_ms + chunk_end_ms);
        self.start_ms = self.session_start_ms.unwrap();
        self.end_ms = self.session_end_ms.unwrap();
        self
    }

    pub fn evidence_uri(&self, revision: i64) -> String {
        format!("lifesub://segment/{}?revision={revision}", self.id)
    }
}
