use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::catalog::Catalog;
use crate::domain::{
    AudioChunk, AudioSource, CaptureSession, ChunkIntegrityState, TranscriptRevision,
};

#[derive(Debug)]
pub enum ServiceError {
    Io(std::io::Error),
    Catalog(rusqlite::Error),
    InvalidEvidenceUri,
    InputUnavailable,
    InputIntegrityFailed,
}

impl From<std::io::Error> for ServiceError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<rusqlite::Error> for ServiceError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Catalog(value)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum EvidenceTarget {
    Record {
        id: String,
    },
    Segment {
        id: String,
        revision: Option<i64>,
    },
    Audio {
        id: String,
        start_seconds: Option<i64>,
        end_seconds: Option<i64>,
    },
}

pub struct EvidenceService {
    catalog: Catalog,
    data_dir: PathBuf,
}

impl EvidenceService {
    pub fn new(catalog: Catalog, data_dir: impl AsRef<Path>) -> Self {
        Self {
            catalog,
            data_dir: data_dir.as_ref().to_path_buf(),
        }
    }

    /// Returns a shared reference to the underlying Catalog for test assertions
    /// and callers that need to inspect chunk integrity state after service operations.
    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    const AUDIO_DIR: &str = "audio";

    pub fn import_audio(
        &self,
        session: &CaptureSession,
        source_path: impl AsRef<Path>,
    ) -> Result<AudioChunk, ServiceError> {
        let source_path = source_path.as_ref();
        self.catalog.insert_session(session)?;
        let audio_dir = self.data_dir.join(Self::AUDIO_DIR);
        fs::create_dir_all(&audio_dir)?;
        let bytes = fs::read(source_path)?;
        let digest = hex::encode(Sha256::digest(&bytes));
        let extension = source_path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("audio");
        let id = format!("chk_{}", Uuid::new_v4().simple());
        let relative_path = PathBuf::from(Self::AUDIO_DIR).join(format!("{id}.{extension}"));
        let final_path = self.data_dir.join(&relative_path);

        // 写入同目录临时文件，写入过程中计算 hash
        let temp_path = audio_dir.join(format!(".{id}.tmp"));
        {
            let mut temp_file = fs::File::create(&temp_path)?;
            temp_file.write_all(&bytes)?;
            // 强制刷新到磁盘，缩小崩溃窗口
            temp_file.sync_all()?;
        }

        // 原子 rename 到最终路径
        fs::rename(&temp_path, &final_path)?;
        // fsync 父目录，确保 rename 被持久化
        let parent_dir = final_path.parent().unwrap();
        fs::File::open(parent_dir)?.sync_all()?;

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

    /// 在执行 ASR 之前重新计算文件 hash，与 chunk 记录的 hash 比较。
    /// 不一致或文件缺失时更新 chunk 的 integrity state 并返回错误。
    pub fn verify_chunk(&self, chunk_id: &str) -> Result<(), ServiceError> {
        let chunks = self.catalog.list_chunks()?;
        let chunk = chunks
            .iter()
            .find(|c| c.id == chunk_id)
            .ok_or(ServiceError::InputUnavailable)?;
        let file_path = self.data_dir.join(&chunk.path);
        if !file_path.exists() {
            self.catalog.update_chunk_integrity(
                chunk_id,
                ChunkIntegrityState::Missing,
                Some("input_unavailable"),
            )?;
            return Err(ServiceError::InputUnavailable);
        }
        let bytes = fs::read(&file_path)?;
        let actual_digest = hex::encode(Sha256::digest(&bytes));
        if actual_digest != chunk.sha256 {
            self.catalog.update_chunk_integrity(
                chunk_id,
                ChunkIntegrityState::Corrupted,
                Some("input_integrity_failed"),
            )?;
            return Err(ServiceError::InputIntegrityFailed);
        }
        Ok(())
    }

    /// 启动时 reconciliation：清理孤儿临时文件，检测 chunk 文件状态。
    /// 返回每个 chunk 的当前 integrity 状态。
    pub fn reconcile_chunks(&self) -> Result<Vec<(String, ChunkIntegrityState)>, ServiceError> {
        let audio_dir = self.data_dir.join(Self::AUDIO_DIR);
        // 清理孤儿临时文件（未被 catalog 引用的 .tmp 文件）
        if audio_dir.exists() {
            for entry in fs::read_dir(&audio_dir)? {
                let entry = entry?;
                let file_name = entry.file_name();
                let name = file_name.to_string_lossy();
                if name.ends_with(".tmp") {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }

        let chunks = self.catalog.list_chunks()?;
        let mut results = Vec::with_capacity(chunks.len());
        for chunk in &chunks {
            let file_path = self.data_dir.join(&chunk.path);
            if !file_path.exists() {
                self.catalog.update_chunk_integrity(
                    &chunk.id,
                    ChunkIntegrityState::Missing,
                    Some("input_unavailable"),
                )?;
                results.push((chunk.id.clone(), ChunkIntegrityState::Missing));
                continue;
            }
            let bytes = match fs::read(&file_path) {
                Ok(b) => b,
                Err(_) => {
                    self.catalog.update_chunk_integrity(
                        &chunk.id,
                        ChunkIntegrityState::Corrupted,
                        Some("input_integrity_failed"),
                    )?;
                    results.push((chunk.id.clone(), ChunkIntegrityState::Corrupted));
                    continue;
                }
            };
            let actual_digest = hex::encode(Sha256::digest(&bytes));
            if actual_digest != chunk.sha256 {
                self.catalog.update_chunk_integrity(
                    &chunk.id,
                    ChunkIntegrityState::Corrupted,
                    Some("input_integrity_failed"),
                )?;
                results.push((chunk.id.clone(), ChunkIntegrityState::Corrupted));
            } else {
                results.push((chunk.id.clone(), ChunkIntegrityState::Available));
            }
        }
        Ok(results)
    }

    /// 查询 chunk 的 integrity 状态，委托给 Catalog。
    pub fn chunk_integrity(&self, chunk_id: &str) -> Result<ChunkIntegrityState, ServiceError> {
        Ok(self.catalog.chunk_integrity(chunk_id)?)
    }

    pub fn render_markdown(
        &self,
        session: &CaptureSession,
        revision: &TranscriptRevision,
    ) -> String {
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
            output.push_str(&format!(
                "\n## {minutes:02}:{seconds:02}\n\n[{}] {}\n",
                source_label(segment.source),
                segment.text
            ));
        }
        output
    }
}

pub fn parse_evidence_uri(uri: &str) -> Result<EvidenceTarget, ServiceError> {
    let body = uri
        .strip_prefix("lifesub://")
        .ok_or(ServiceError::InvalidEvidenceUri)?;
    if let Some(id) = body.strip_prefix("record/") {
        return non_empty(id).map(|id| EvidenceTarget::Record { id });
    }
    if let Some(body) = body.strip_prefix("segment/") {
        let (id, query) = body.split_once('?').unwrap_or((body, ""));
        let revision = query
            .strip_prefix("revision=")
            .and_then(|value| value.parse().ok());
        return non_empty(id).map(|id| EvidenceTarget::Segment { id, revision });
    }
    if let Some(body) = body.strip_prefix("audio/") {
        let (id, fragment) = body.split_once('#').unwrap_or((body, ""));
        let range = fragment
            .strip_prefix("t=")
            .and_then(|value| value.split_once(','));
        let start_seconds = range.and_then(|(start, _)| start.parse().ok());
        let end_seconds = range.and_then(|(_, end)| end.parse().ok());
        return non_empty(id).map(|id| EvidenceTarget::Audio {
            id,
            start_seconds,
            end_seconds,
        });
    }
    Err(ServiceError::InvalidEvidenceUri)
}

fn non_empty(value: &str) -> Result<String, ServiceError> {
    if value.is_empty() {
        Err(ServiceError::InvalidEvidenceUri)
    } else {
        Ok(value.to_owned())
    }
}

fn source_label(source: AudioSource) -> &'static str {
    match source {
        AudioSource::Microphone => "麦克风",
        AudioSource::SystemAudio => "系统音频",
        AudioSource::Imported => "导入音频",
    }
}
