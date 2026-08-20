use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChunkIntegrityState {
    Available,
    Corrupted,
    Missing,
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
