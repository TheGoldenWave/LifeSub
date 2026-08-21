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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ImportModelReadiness {
    Ready,
    #[cfg(test)]
    Blocked(AsrErrorCode),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ImportAsrDisposition {
    NoJob,
    Queued,
    BlockedModel(AsrErrorCode),
    Failed(AsrErrorCode),
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
    pub chunk_id: Option<String>,
    pub chunk_start_ms: Option<i64>,
    pub chunk_end_ms: Option<i64>,
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
pub struct TranscriptSegmentPublication {
    pub id: String,
    pub chunk_start_ms: i64,
    pub chunk_end_ms: i64,
    pub source: AudioSource,
    pub text: String,
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
            chunk_id: None,
            chunk_start_ms: None,
            chunk_end_ms: None,
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

impl<'de> Deserialize<'de> for AsrConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = AsrConfigWire::deserialize(deserializer)?;
        let model_id = match wire.model_id {
            Some(model_id) if !model_id.trim().is_empty() => model_id,
            _ => default_model_id_for_provider(&wire.provider)
                .ok_or_else(|| D::Error::custom("unknown ASR provider"))?
                .to_owned(),
        };
        Ok(Self {
            provider: wire.provider,
            model_id,
            language: wire.language,
            auto_transcribe: wire.auto_transcribe,
            threads: wire.threads,
            vad_enabled: wire.vad_enabled,
            vad_min_speech_ms: wire.vad_min_speech_ms,
            vad_silence_ms: wire.vad_silence_ms,
            itn_enabled: wire.itn_enabled,
        })
    }
}

// ── New types for Task 13.5 ──────────────────────────────────────────────

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CaptureNote {
    pub id: String,
    pub session_id: String,
    pub content: String,
    pub timestamp_ms: i64,
    pub tag: String,
    pub segment_id: Option<String>,
    pub created_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DictionaryCategory {
    pub id: String,
    pub name: String,
    pub scope: String,
    pub entry_count: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DictionaryEntry {
    pub id: String,
    pub category_id: String,
    pub term: String,
    pub pinyin: String,
    pub aliases: String,
    pub note: String,
    pub enabled: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Voiceprint {
    pub id: String,
    pub name: String,
    pub embedding_path: String,
    pub dictionary_entry_id: Option<String>,
    pub sample_count: i64,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StatsSnapshot {
    pub hourly_slots: Vec<HourlySlot>,
    pub week_sessions: i64,
    pub week_minutes: i64,
    pub month_sessions: i64,
    pub month_minutes: i64,
    pub total_sessions: i64,
    pub total_minutes: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HourlySlot {
    pub hour: i64,
    pub minutes: i64,
    pub session_id: Option<String>,
    pub title: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AsrConfig {
    pub provider: String,
    pub model_id: String,
    pub language: String,
    pub auto_transcribe: bool,
    pub threads: i64,
    pub vad_enabled: bool,
    pub vad_min_speech_ms: i64,
    pub vad_silence_ms: i64,
    pub itn_enabled: bool,
}

#[derive(Deserialize)]
struct AsrConfigWire {
    provider: String,
    model_id: Option<String>,
    language: String,
    auto_transcribe: bool,
    threads: i64,
    vad_enabled: bool,
    vad_min_speech_ms: i64,
    vad_silence_ms: i64,
    itn_enabled: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RecordingConfig {
    pub capture_mode: String,
    pub im_detection_enabled: bool,
    pub im_apps: Vec<String>,
    pub detection_delay_secs: i64,
    pub recovery_delay_secs: i64,
    pub sample_rate: i64,
    pub storage_path: String,
}

impl AsrConfig {
    pub fn default_for_provider(provider: &str) -> Option<Self> {
        Some(Self {
            provider: provider.to_owned(),
            model_id: default_model_id_for_provider(provider)?.to_owned(),
            language: "zh".to_owned(),
            auto_transcribe: true,
            threads: 4,
            vad_enabled: true,
            vad_min_speech_ms: 300,
            vad_silence_ms: 800,
            itn_enabled: provider == "sense_voice",
        })
    }

    pub fn validate_for_persistence(&self) -> Result<(), String> {
        let provider = match self.provider.as_str() {
            "sense_voice" => AsrProviderKind::SenseVoice,
            "whisper" => AsrProviderKind::Whisper,
            "qwen3_asr" => AsrProviderKind::Qwen3Asr,
            _ => return Err(format!("unknown ASR provider {}", self.provider)),
        };
        let model = crate::asr::manifest::model_registry()
            .model(&self.model_id)
            .ok_or_else(|| format!("unknown ASR model {}", self.model_id))?;
        if model.provider != provider {
            return Err(format!(
                "model {} does not belong to provider {}",
                self.model_id, self.provider
            ));
        }
        Ok(())
    }
}

impl ImportAsrDisposition {
    pub(crate) fn classify(
        auto_transcribe: bool,
        readiness: ImportModelReadiness,
        enqueue_result: Result<(), AsrErrorCode>,
    ) -> Self {
        if !auto_transcribe {
            return Self::NoJob;
        }
        match readiness {
            #[cfg(test)]
            ImportModelReadiness::Blocked(error) => Self::BlockedModel(error),
            ImportModelReadiness::Ready => match enqueue_result {
                Ok(()) => Self::Queued,
                Err(AsrErrorCode::ModelNotInstalled) => {
                    Self::BlockedModel(AsrErrorCode::ModelNotInstalled)
                }
                Err(AsrErrorCode::ModelCapabilityUnavailable) => {
                    Self::BlockedModel(AsrErrorCode::ModelCapabilityUnavailable)
                }
                Err(error) => Self::Failed(error),
            },
        }
    }
}

fn default_model_id_for_provider(provider: &str) -> Option<&'static str> {
    match provider {
        "sense_voice" => Some("sense-voice-small-int8-2024-07-17"),
        "whisper" => Some("whisper-base"),
        "qwen3_asr" => Some("qwen3-asr-0.6b-int8-2026-03-25"),
        _ => None,
    }
}

impl CaptureNote {
    pub fn new(
        session_id: String,
        content: String,
        timestamp_ms: i64,
        tag: String,
        segment_id: Option<String>,
    ) -> Self {
        Self {
            id: format!("note_{}", uuid::Uuid::new_v4().simple()),
            session_id,
            content,
            timestamp_ms,
            tag,
            segment_id,
            created_at: Utc::now().to_rfc3339(),
        }
    }
}

impl DictionaryCategory {
    pub fn new(name: String, scope: String) -> Self {
        Self {
            id: format!("dcat_{}", uuid::Uuid::new_v4().simple()),
            name,
            scope,
            entry_count: 0,
        }
    }
}

impl DictionaryEntry {
    pub fn new(
        category_id: String,
        term: String,
        pinyin: String,
        aliases: String,
        note: String,
    ) -> Self {
        Self {
            id: format!("dent_{}", uuid::Uuid::new_v4().simple()),
            category_id,
            term,
            pinyin,
            aliases,
            note,
            enabled: true,
        }
    }
}

impl Voiceprint {
    pub fn new(name: String, embedding_path: String, dictionary_entry_id: Option<String>) -> Self {
        Self {
            id: format!("vp_{}", uuid::Uuid::new_v4().simple()),
            name,
            embedding_path,
            dictionary_entry_id,
            sample_count: 1,
            updated_at: Utc::now().to_rfc3339(),
        }
    }
}
