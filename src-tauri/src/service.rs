use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use uuid::Uuid;

use crate::catalog::Catalog;
use crate::domain::{
    AudioChunk, AudioSource, CaptureSession, ChunkIntegrityState, TranscriptRevision,
};

use self::audio_store::{AudioStore, ValidationError};

mod audio_store;
mod error;
mod evidence_uri;

pub use error::{ImportFault, ServiceError};
pub use evidence_uri::{parse_evidence_uri, EvidenceTarget};

pub struct EvidenceService {
    catalog: Catalog,
    data_dir: PathBuf,
    #[cfg(test)]
    import_fault: Option<ImportFault>,
}

impl EvidenceService {
    pub fn new(catalog: Catalog, data_dir: impl AsRef<Path>) -> Self {
        Self {
            catalog,
            data_dir: data_dir.as_ref().to_path_buf(),
            #[cfg(test)]
            import_fault: None,
        }
    }

    #[cfg(test)]
    pub fn with_import_fault(
        catalog: Catalog,
        data_dir: impl AsRef<Path>,
        import_fault: ImportFault,
    ) -> Self {
        Self {
            catalog,
            data_dir: data_dir.as_ref().to_path_buf(),
            import_fault: Some(import_fault),
        }
    }

    #[cfg(test)]
    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    pub fn import_audio(
        &self,
        session: &CaptureSession,
        source_path: impl AsRef<Path>,
    ) -> Result<AudioChunk, ServiceError> {
        let source_path = source_path.as_ref();
        self.catalog.insert_session(session)?;
        let store = AudioStore::new(&self.data_dir);
        let id = format!("chk_{}", Uuid::new_v4().simple());
        let pending = store.write_temp(source_path, &id)?;
        self.fail_import_at(ImportFault::AfterTempSync)?;
        if self.has_import_fault(ImportFault::RenameIo) {
            store.discard_pending(&pending);
            return Err(injected_io_error("rename").into());
        }

        let extension = source_path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("audio");
        let stored = store.rename_to_final(&pending, &id, extension)?;
        self.fail_import_at(ImportFault::AfterFinalRename)?;
        if self.has_import_fault(ImportFault::ParentSyncIo) {
            store.discard_stored(&stored);
            return Err(injected_io_error("parent sync").into());
        }
        store.sync_final(&stored)?;

        let chunk = AudioChunk {
            id,
            session_id: session.id.clone(),
            source: AudioSource::Imported,
            path: stored.relative_path.to_string_lossy().into_owned(),
            sha256: pending.digest,
            byte_length: pending.byte_length,
        };
        self.catalog.insert_chunk(&chunk)?;
        Ok(chunk)
    }

    pub fn reconcile_audio(&self) -> Result<(), ServiceError> {
        self.reconcile_audio_before(SystemTime::now())
    }

    pub fn reconcile_audio_before(&self, stale_before: SystemTime) -> Result<(), ServiceError> {
        let chunks = self.catalog.list_chunks()?;
        let referenced_paths = chunks
            .iter()
            .map(|chunk| chunk.path.as_str())
            .collect::<HashSet<_>>();
        AudioStore::new(&self.data_dir).reconcile_orphans(&referenced_paths, stale_before)?;

        for chunk in chunks {
            let (state, error_code) = match self.validate_chunk_bytes(&chunk) {
                Ok(()) => (ChunkIntegrityState::Available, None),
                Err(ChunkValidationError::Missing) => {
                    (ChunkIntegrityState::Missing, Some("input_unavailable"))
                }
                Err(ChunkValidationError::Corrupted) => (
                    ChunkIntegrityState::Corrupted,
                    Some("input_integrity_failed"),
                ),
            };
            self.catalog
                .update_chunk_integrity(&chunk.id, state, error_code)?;
        }
        Ok(())
    }

    pub fn verify_chunk(&self, chunk_id: &str) -> Result<AudioChunk, ServiceError> {
        let chunk = self
            .catalog
            .chunk(chunk_id)?
            .ok_or(ServiceError::InputUnavailable)?;
        match self.validate_chunk_bytes(&chunk) {
            Ok(()) => {
                self.catalog.update_chunk_integrity(
                    chunk_id,
                    ChunkIntegrityState::Available,
                    None,
                )?;
                Ok(chunk)
            }
            Err(ChunkValidationError::Missing) => {
                self.catalog.update_chunk_integrity(
                    chunk_id,
                    ChunkIntegrityState::Missing,
                    Some("input_unavailable"),
                )?;
                Err(ServiceError::InputUnavailable)
            }
            Err(ChunkValidationError::Corrupted) => {
                self.catalog.update_chunk_integrity(
                    chunk_id,
                    ChunkIntegrityState::Corrupted,
                    Some("input_integrity_failed"),
                )?;
                Err(ServiceError::InputIntegrityFailed)
            }
        }
    }

    pub fn chunk_integrity(&self, chunk_id: &str) -> Result<ChunkIntegrityState, ServiceError> {
        self.catalog
            .chunk_integrity(chunk_id)?
            .ok_or(ServiceError::InputUnavailable)
    }

    fn validate_chunk_bytes(&self, chunk: &AudioChunk) -> Result<(), ChunkValidationError> {
        match AudioStore::new(&self.data_dir).validate(chunk) {
            Ok(()) => Ok(()),
            Err(ValidationError::Missing) => Err(ChunkValidationError::Missing),
            Err(ValidationError::Corrupted) => Err(ChunkValidationError::Corrupted),
        }
    }

    #[cfg(test)]
    fn fail_import_at(&self, point: ImportFault) -> Result<(), ServiceError> {
        if self.import_fault == Some(point) {
            Err(ServiceError::InjectedCrash(point))
        } else {
            Ok(())
        }
    }

    #[cfg(not(test))]
    fn fail_import_at(&self, _point: ImportFault) -> Result<(), ServiceError> {
        Ok(())
    }

    #[cfg(test)]
    fn has_import_fault(&self, point: ImportFault) -> bool {
        self.import_fault == Some(point)
    }

    #[cfg(not(test))]
    fn has_import_fault(&self, _point: ImportFault) -> bool {
        false
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

#[derive(Clone, Copy)]
enum ChunkValidationError {
    Missing,
    Corrupted,
}

fn injected_io_error(operation: &str) -> std::io::Error {
    std::io::Error::other(format!("injected {operation} failure"))
}

fn source_label(source: AudioSource) -> &'static str {
    match source {
        AudioSource::Microphone => "麦克风",
        AudioSource::SystemAudio => "系统音频",
        AudioSource::Imported => "导入音频",
    }
}
