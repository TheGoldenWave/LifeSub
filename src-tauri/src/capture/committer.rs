use std::collections::HashMap;
use std::path::Path;

use chrono::Utc;

use crate::capture::coordinator::{NativeCaptureChunkCommitter, NativeCaptureStableError};
use crate::capture::protocol::{ChunkSealed, Source};
use crate::capture::seal::{NativeChunkSealRequest, NativeChunkSealer};
use crate::catalog::Catalog;
use crate::domain::{AudioChunk, AudioSource, CaptureChunkTiming};

type AfterCommit<'a> = dyn Fn(&AudioChunk) -> Result<(), String> + Send + Sync + 'a;

pub(crate) struct CatalogCaptureCommitter<'a> {
    sealer: NativeChunkSealer<'a>,
    session_id: String,
    source_ids: HashMap<Source, String>,
    sequences: HashMap<Source, i64>,
    session_anchor_ns: u64,
    after_commit: Box<AfterCommit<'a>>,
}

impl<'a> CatalogCaptureCommitter<'a> {
    pub(crate) fn new(
        catalog: &'a Catalog,
        session_id: impl Into<String>,
        staging_root: &Path,
        data_root: &Path,
        source_ids: HashMap<Source, String>,
        session_anchor_ns: u64,
        after_commit: impl Fn(&AudioChunk) -> Result<(), String> + Send + Sync + 'a,
    ) -> std::io::Result<Self> {
        Ok(Self {
            sealer: NativeChunkSealer::new(catalog, staging_root, data_root)?,
            session_id: session_id.into(),
            source_ids,
            sequences: HashMap::new(),
            session_anchor_ns,
            after_commit: Box::new(after_commit),
        })
    }
}

impl NativeCaptureChunkCommitter for CatalogCaptureCommitter<'_> {
    fn commit(&mut self, sealed: &ChunkSealed) -> Result<(), NativeCaptureStableError> {
        let source_id = self
            .source_ids
            .get(&sealed.source)
            .ok_or(NativeCaptureStableError::ProtocolViolation)?
            .clone();
        let sequence = self.sequences.entry(sealed.source).or_insert(0);
        let current = *sequence;
        *sequence = sequence
            .checked_add(1)
            .ok_or(NativeCaptureStableError::ProtocolViolation)?;
        let source = match sealed.source {
            Source::Microphone => AudioSource::Microphone,
            Source::SystemAudio => AudioSource::SystemAudio,
        };
        let chunk_id = chunk_id_from_path(&sealed.relative_staging_path)
            .ok_or(NativeCaptureStableError::ProtocolViolation)?;
        let now = Utc::now();
        let relative_start = sealed
            .start_host_time
            .checked_sub(self.session_anchor_ns)
            .ok_or(NativeCaptureStableError::ProtocolViolation)?;
        let relative_end = sealed
            .end_host_time
            .checked_sub(self.session_anchor_ns)
            .ok_or(NativeCaptureStableError::ProtocolViolation)?;
        let timing = CaptureChunkTiming {
            chunk_id: chunk_id.clone(),
            capture_source_id: source_id,
            source_sequence: current,
            source_sample_start: sealed.start_sample_position as i64,
            source_sample_end: sealed.end_sample_position as i64,
            session_start_ms: (relative_start / 1_000_000) as i64,
            session_end_ms: (relative_end / 1_000_000) as i64,
            monotonic_start_ns: sealed.start_host_time as i64,
            monotonic_end_ns: sealed.end_host_time as i64,
            utc_start_at: now,
            utc_end_at: now + chrono::Duration::milliseconds(sealed.duration_ms as i64),
            discontinuity_count: 0,
            dropped_frame_count: 0,
            created_at: now,
        };
        let request = NativeChunkSealRequest {
            relative_staging_path: sealed.relative_staging_path.clone().into(),
            chunk_id,
            session_id: self.session_id.clone(),
            source,
            expected_sha256: sealed.sha256.clone(),
            expected_byte_length: sealed.byte_length,
            timing,
        };
        if request.expected_sha256.len() != 64
            || !request
                .expected_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(NativeCaptureStableError::ProtocolViolation);
        }
        let chunk = self
            .sealer
            .seal(&request)
            .map_err(|_| NativeCaptureStableError::CommitFailed)?;
        (self.after_commit)(&chunk).map_err(|_| NativeCaptureStableError::CommitFailed)
    }
}

fn chunk_id_from_path(path: &str) -> Option<String> {
    let name = Path::new(path).file_name()?.to_str()?;
    let stem = name.strip_suffix(".partial")?.split('.').next()?;
    (stem.starts_with("chk_") && stem.len() > 4).then(|| stem.to_string())
}
