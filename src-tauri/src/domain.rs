use chrono::{DateTime, Utc};
use serde::de::Error as _;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use crate::asr::receipt::{
    DataDestination, ProviderOutcome, ProviderReceipt, ProviderReceiptDraft, ProviderReceiptError,
};

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

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AsrProviderKind {
    SenseVoice,
    Whisper,
    Qwen3Asr,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct AsrLanguage(String);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AsrLanguageError {
    Empty,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
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

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChunkIntegrityState {
    Available,
    Corrupted,
    Missing,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AsrErrorCode {
    ModelNotInstalled,
    ModelCapabilityUnavailable,
    ModelDownloadFailed,
    ModelIntegrityFailed,
    InsufficientDiskSpace,
    UnsupportedOrCorruptAudio,
    InputIntegrityFailed,
    InputUnavailable,
    InvalidProviderParameter,
    ProviderInitializationFailed,
    TranscriptionFailed,
    Cancelled,
    RecoveryRequired,
    RecoveryRetryExhausted,
    ReceiptInvalid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct TranscriptTimeRange {
    start_ms: i64,
    end_ms: i64,
    audio_duration_ms: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TranscriptTimeRangeError {
    NegativeStart,
    EmptyOrReversed,
    ExceedsAudioDuration,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CaptureSession {
    pub id: String,
    pub title: String,
    pub state: CaptureState,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TranscriptSegment {
    pub id: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub source: AudioSource,
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TranscriptRevision {
    pub id: String,
    pub session_id: String,
    pub number: i64,
    pub provider: String,
    pub created_at: DateTime<Utc>,
    pub segments: Vec<TranscriptSegment>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AudioChunk {
    pub id: String,
    pub session_id: String,
    pub source: AudioSource,
    pub path: String,
    pub sha256: String,
    pub byte_length: u64,
}

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
        }
    }

    pub fn evidence_uri(&self, revision: i64) -> String {
        format!("lifesub://segment/{}?revision={revision}", self.id)
    }
}

impl AsrLanguage {
    pub fn new(value: impl Into<String>) -> Result<Self, AsrLanguageError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(AsrLanguageError::Empty);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for AsrLanguage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(|_| D::Error::custom("ASR language must not be empty"))
    }
}

impl TranscriptTimeRange {
    pub fn new(
        start_ms: i64,
        end_ms: i64,
        audio_duration_ms: i64,
    ) -> Result<Self, TranscriptTimeRangeError> {
        if start_ms < 0 {
            return Err(TranscriptTimeRangeError::NegativeStart);
        }
        if end_ms <= start_ms {
            return Err(TranscriptTimeRangeError::EmptyOrReversed);
        }
        if end_ms > audio_duration_ms {
            return Err(TranscriptTimeRangeError::ExceedsAudioDuration);
        }
        Ok(Self {
            start_ms,
            end_ms,
            audio_duration_ms,
        })
    }

    pub const fn start_ms(self) -> i64 {
        self.start_ms
    }

    pub const fn end_ms(self) -> i64 {
        self.end_ms
    }

    pub const fn audio_duration_ms(self) -> i64 {
        self.audio_duration_ms
    }
}

impl<'de> Deserialize<'de> for TranscriptTimeRange {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct WireRange {
            start_ms: i64,
            end_ms: i64,
            audio_duration_ms: i64,
        }

        let wire = WireRange::deserialize(deserializer)?;
        Self::new(wire.start_ms, wire.end_ms, wire.audio_duration_ms)
            .map_err(|_| D::Error::custom("invalid transcript time range"))
    }
}
