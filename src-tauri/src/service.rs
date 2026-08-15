use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::catalog::Catalog;
use crate::domain::{AudioChunk, AudioSource, CaptureSession, TranscriptRevision};

#[derive(Debug)]
pub enum ServiceError {
    Io(std::io::Error),
    Catalog(rusqlite::Error),
    InvalidEvidenceUri,
}

impl From<std::io::Error> for ServiceError {
    fn from(value: std::io::Error) -> Self { Self::Io(value) }
}

impl From<rusqlite::Error> for ServiceError {
    fn from(value: rusqlite::Error) -> Self { Self::Catalog(value) }
}

#[derive(Debug, Eq, PartialEq)]
pub enum EvidenceTarget {
    Record { id: String },
    Segment { id: String, revision: Option<i64> },
    Audio { id: String, start_seconds: Option<i64>, end_seconds: Option<i64> },
}

pub struct EvidenceService {
    catalog: Catalog,
    data_dir: PathBuf,
}

impl EvidenceService {
    pub fn new(catalog: Catalog, data_dir: impl AsRef<Path>) -> Self {
        Self { catalog, data_dir: data_dir.as_ref().to_path_buf() }
    }

    pub fn import_audio(&self, session: &CaptureSession, source_path: impl AsRef<Path>) -> Result<AudioChunk, ServiceError> {
        let source_path = source_path.as_ref();
        self.catalog.insert_session(session)?;
        fs::create_dir_all(self.data_dir.join("audio"))?;
        let bytes = fs::read(source_path)?;
        let digest = hex::encode(Sha256::digest(&bytes));
        let extension = source_path.extension().and_then(|value| value.to_str()).unwrap_or("audio");
        let id = format!("chk_{}", Uuid::new_v4().simple());
        let relative_path = PathBuf::from("audio").join(format!("{id}.{extension}"));
        fs::write(self.data_dir.join(&relative_path), &bytes)?;
        let chunk = AudioChunk {
            id,
            session_id: session.id.clone(),
            source: AudioSource::Imported,
            path: relative_path.to_string_lossy().into_owned(),
            sha256: digest,
            byte_length: bytes.len() as u64,
        };
        self.catalog.insert_chunk(&chunk)?;
        Ok(chunk)
    }

    pub fn render_markdown(&self, session: &CaptureSession, revision: &TranscriptRevision) -> String {
        let mut output = format!(
            "---\nrecord_id: {}\nevidence_uri: {}\nstarted_at: {}\ntranscript_revision: {}\nasr_provider: {}\n---\n\n# {}\n",
            session.id,
            session.evidence_uri(),
            session.started_at.to_rfc3339(),
            revision.number,
            revision.provider,
            session.title,
        );
        for segment in &revision.segments {
            let minutes = segment.start_ms / 60_000;
            let seconds = (segment.start_ms / 1_000) % 60;
            output.push_str(&format!("\n## {minutes:02}:{seconds:02}\n\n[{}] {}\n", source_label(segment.source), segment.text));
        }
        output
    }
}

pub fn parse_evidence_uri(uri: &str) -> Result<EvidenceTarget, ServiceError> {
    let body = uri.strip_prefix("lifesub://").ok_or(ServiceError::InvalidEvidenceUri)?;
    if let Some(id) = body.strip_prefix("record/") {
        return non_empty(id).map(|id| EvidenceTarget::Record { id });
    }
    if let Some(body) = body.strip_prefix("segment/") {
        let (id, query) = body.split_once('?').unwrap_or((body, ""));
        let revision = query.strip_prefix("revision=").and_then(|value| value.parse().ok());
        return non_empty(id).map(|id| EvidenceTarget::Segment { id, revision });
    }
    if let Some(body) = body.strip_prefix("audio/") {
        let (id, fragment) = body.split_once('#').unwrap_or((body, ""));
        let range = fragment.strip_prefix("t=").and_then(|value| value.split_once(','));
        let start_seconds = range.and_then(|(start, _)| start.parse().ok());
        let end_seconds = range.and_then(|(_, end)| end.parse().ok());
        return non_empty(id).map(|id| EvidenceTarget::Audio { id, start_seconds, end_seconds });
    }
    Err(ServiceError::InvalidEvidenceUri)
}

fn non_empty(value: &str) -> Result<String, ServiceError> {
    if value.is_empty() { Err(ServiceError::InvalidEvidenceUri) } else { Ok(value.to_owned()) }
}

fn source_label(source: AudioSource) -> &'static str {
    match source { AudioSource::Microphone => "麦克风", AudioSource::SystemAudio => "系统音频", AudioSource::Imported => "导入音频" }
}
