use std::collections::HashSet;
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::sync::{Arc, Barrier};
use std::time::{Duration, SystemTime};

use uuid::Uuid;

use crate::catalog::{Catalog, ImportedChunkInsertError};
use crate::domain::{
    AsrErrorCode, AudioChunk, AudioSource, CaptureSession, ChunkIntegrityState, TranscriptRevision,
};

use self::audio_store::{AudioStore, ValidationError, canonical_extension};
use self::runtime_lock::DataDirectoryCapability;

mod audio_store;
mod error;
mod evidence_uri;
mod runtime_lock;

pub use error::{ImportFault, ServiceError};
pub use evidence_uri::{EvidenceTarget, parse_evidence_uri};
pub(crate) use runtime_lock::JobOwnershipCapability;
pub use runtime_lock::{
    CoreRuntime, CoreRuntimeError, RuntimeOwnershipError, RuntimeOwnershipGuard,
};

const ORPHAN_GRACE: Duration = Duration::from_secs(10 * 60);

#[cfg(test)]
pub(crate) fn normalized_audio_extension(path: &Path) -> String {
    canonical_extension(path)
}

pub struct EvidenceService {
    catalog: Catalog,
    data_dir: PathBuf,
    data_dir_capability: Option<DataDirectoryCapability>,
    #[cfg(test)]
    import_fault: Option<ImportFault>,
    #[cfg(test)]
    audio_directory_swap_target: Option<PathBuf>,
    #[cfg(test)]
    first_audio_create_barrier: Option<Arc<Barrier>>,
}

impl EvidenceService {
    pub fn new(catalog: Catalog, data_dir: impl AsRef<Path>) -> Self {
        Self {
            catalog,
            data_dir: data_dir.as_ref().to_path_buf(),
            data_dir_capability: None,
            #[cfg(test)]
            import_fault: None,
            #[cfg(test)]
            audio_directory_swap_target: None,
            #[cfg(test)]
            first_audio_create_barrier: None,
        }
    }

    pub fn initialize(catalog: Catalog, data_dir: impl AsRef<Path>) -> Result<Self, ServiceError> {
        let service = Self::new(catalog, data_dir);
        service.reconcile_audio()?;
        Ok(service)
    }

    pub(crate) fn initialize_anchored(
        catalog: Catalog,
        data_dir: DataDirectoryCapability,
    ) -> Result<Self, ServiceError> {
        let service = Self {
            catalog,
            data_dir: PathBuf::new(),
            data_dir_capability: Some(data_dir),
            #[cfg(test)]
            import_fault: None,
            #[cfg(test)]
            audio_directory_swap_target: None,
            #[cfg(test)]
            first_audio_create_barrier: None,
        };
        service.reconcile_audio()?;
        Ok(service)
    }

    pub fn into_catalog(self) -> Catalog {
        self.catalog
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
            data_dir_capability: None,
            import_fault: Some(import_fault),
            audio_directory_swap_target: None,
            first_audio_create_barrier: None,
        }
    }

    #[cfg(all(test, unix))]
    pub fn with_audio_directory_swap(
        catalog: Catalog,
        data_dir: impl AsRef<Path>,
        target: impl AsRef<Path>,
    ) -> Self {
        Self {
            catalog,
            data_dir: data_dir.as_ref().to_path_buf(),
            data_dir_capability: None,
            import_fault: None,
            audio_directory_swap_target: Some(target.as_ref().to_path_buf()),
            first_audio_create_barrier: None,
        }
    }

    #[cfg(test)]
    pub fn with_first_audio_create_barrier(
        catalog: Catalog,
        data_dir: impl AsRef<Path>,
        barrier: Arc<Barrier>,
    ) -> Self {
        Self {
            catalog,
            data_dir: data_dir.as_ref().to_path_buf(),
            data_dir_capability: None,
            import_fault: None,
            audio_directory_swap_target: None,
            first_audio_create_barrier: Some(barrier),
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
        import_audio_core(
            &self.catalog,
            self.audio_store(),
            session,
            source_path.as_ref(),
            ImportHooks {
                fail_at: |point| self.fail_import_at(point),
                ensure_current: || Ok(()),
                before_stored_check: || {},
                before_commit: || {},
                before_cleanup: || {},
            },
        )
    }

    #[cfg(feature = "desktop")]
    pub(crate) fn import_audio_with_existing_catalog(
        catalog: &Catalog,
        data_dir: DataDirectoryCapability,
        session: &CaptureSession,
        source_path: impl AsRef<Path>,
        ensure_current: impl FnMut() -> Result<(), ServiceError>,
    ) -> Result<AudioChunk, ServiceError> {
        import_audio_core(
            catalog,
            AudioStore::anchored(data_dir),
            session,
            source_path.as_ref(),
            ImportHooks {
                fail_at: |_| Ok(()),
                ensure_current,
                before_stored_check: || {},
                before_commit: || {},
                before_cleanup: || {},
            },
        )
    }

    #[cfg(all(test, feature = "desktop"))]
    pub(crate) fn import_audio_with_existing_catalog_and_commit_barriers(
        catalog: &Catalog,
        data_dir: DataDirectoryCapability,
        session: &CaptureSession,
        source_path: impl AsRef<Path>,
        ensure_current: impl FnMut() -> Result<(), ServiceError>,
        checked: &Barrier,
        resume: &Barrier,
    ) -> Result<AudioChunk, ServiceError> {
        import_audio_core(
            catalog,
            AudioStore::anchored(data_dir),
            session,
            source_path.as_ref(),
            ImportHooks {
                fail_at: |_| Ok(()),
                ensure_current,
                before_stored_check: || {},
                before_commit: || {
                    checked.wait();
                    resume.wait();
                },
                before_cleanup: || {},
            },
        )
    }

    #[cfg(test)]
    pub(crate) fn import_audio_with_cleanup_barriers(
        &self,
        session: &CaptureSession,
        source_path: impl AsRef<Path>,
        cleanup_ready: &Barrier,
        cleanup_resume: &Barrier,
    ) -> Result<AudioChunk, ServiceError> {
        import_audio_core(
            &self.catalog,
            self.audio_store(),
            session,
            source_path.as_ref(),
            ImportHooks {
                fail_at: |point| self.fail_import_at(point),
                ensure_current: || Ok(()),
                before_stored_check: || {},
                before_commit: || {},
                before_cleanup: || {
                    cleanup_ready.wait();
                    cleanup_resume.wait();
                },
            },
        )
    }

    #[cfg(test)]
    pub(crate) fn import_audio_with_cleanup_identity_barriers(
        &self,
        session: &CaptureSession,
        source_path: impl AsRef<Path>,
        ready: Arc<Barrier>,
        resume: Arc<Barrier>,
    ) -> Result<AudioChunk, ServiceError> {
        audio_store::set_cleanup_identity_hook(move || {
            ready.wait();
            resume.wait();
        });
        self.import_audio(session, source_path)
    }

    #[cfg(test)]
    pub(crate) fn reconcile_audio_before_with_cleanup_identity_barriers(
        &self,
        stale_before: SystemTime,
        ready: Arc<Barrier>,
        resume: Arc<Barrier>,
    ) -> Result<(), ServiceError> {
        audio_store::set_cleanup_identity_hook(move || {
            ready.wait();
            resume.wait();
        });
        self.reconcile_audio_before(stale_before)
    }

    #[cfg(test)]
    pub(crate) fn import_audio_with_fault_barriers(
        &self,
        session: &CaptureSession,
        source_path: impl AsRef<Path>,
        barrier_point: ImportFault,
        ready: &Barrier,
        resume: &Barrier,
    ) -> Result<AudioChunk, ServiceError> {
        import_audio_core(
            &self.catalog,
            self.audio_store(),
            session,
            source_path.as_ref(),
            ImportHooks {
                fail_at: |point| {
                    if point == barrier_point {
                        ready.wait();
                        resume.wait();
                    }
                    self.fail_import_at(point)
                },
                ensure_current: || Ok(()),
                before_stored_check: || {},
                before_commit: || {},
                before_cleanup: || {},
            },
        )
    }

    #[cfg(test)]
    pub(crate) fn import_audio_with_stored_check_barriers(
        &self,
        session: &CaptureSession,
        source_path: impl AsRef<Path>,
        ready: &Barrier,
        resume: &Barrier,
    ) -> Result<AudioChunk, ServiceError> {
        import_audio_core(
            &self.catalog,
            self.audio_store(),
            session,
            source_path.as_ref(),
            ImportHooks {
                fail_at: |point| self.fail_import_at(point),
                ensure_current: || Ok(()),
                before_stored_check: || {
                    ready.wait();
                    resume.wait();
                },
                before_commit: || {},
                before_cleanup: || {},
            },
        )
    }

    pub fn reconcile_audio(&self) -> Result<(), ServiceError> {
        let stale_before = SystemTime::now()
            .checked_sub(ORPHAN_GRACE)
            .unwrap_or(SystemTime::UNIX_EPOCH);
        self.reconcile_audio_before(stale_before)
    }

    pub fn reconcile_audio_before(&self, stale_before: SystemTime) -> Result<(), ServiceError> {
        let chunks = self.catalog.list_chunks()?;
        for chunk in &chunks {
            self.catalog.chunk_integrity(&chunk.id)?;
        }
        let referenced_paths = chunks
            .iter()
            .map(|chunk| chunk.path.as_str())
            .collect::<HashSet<_>>();
        self.audio_store()
            .reconcile_orphans(&referenced_paths, stale_before)?;

        for chunk in chunks {
            let (state, error_code) = match self.validate_chunk_bytes(&chunk) {
                Ok(()) => (ChunkIntegrityState::Available, None),
                Err(ChunkValidationError::Missing) => (
                    ChunkIntegrityState::Missing,
                    Some(AsrErrorCode::InputUnavailable),
                ),
                Err(ChunkValidationError::Corrupted) => (
                    ChunkIntegrityState::Corrupted,
                    Some(AsrErrorCode::InputIntegrityFailed),
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
        self.catalog.chunk_integrity(chunk_id)?;
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
                    Some(AsrErrorCode::InputUnavailable),
                )?;
                Err(ServiceError::InputUnavailable)
            }
            Err(ChunkValidationError::Corrupted) => {
                self.catalog.update_chunk_integrity(
                    chunk_id,
                    ChunkIntegrityState::Corrupted,
                    Some(AsrErrorCode::InputIntegrityFailed),
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
        match self.audio_store().validate(chunk) {
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

    fn audio_store(&self) -> AudioStore {
        if let Some(data_dir) = &self.data_dir_capability {
            return AudioStore::anchored(data_dir.clone());
        }
        #[cfg(test)]
        {
            AudioStore::with_fault(
                &self.data_dir,
                self.import_fault,
                self.audio_directory_swap_target.as_deref(),
                self.first_audio_create_barrier.clone(),
            )
        }
        #[cfg(not(test))]
        {
            AudioStore::new(&self.data_dir)
        }
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

struct ImportHooks<FailAt, EnsureCurrent, BeforeStoredCheck, BeforeCommit, BeforeCleanup> {
    fail_at: FailAt,
    ensure_current: EnsureCurrent,
    before_stored_check: BeforeStoredCheck,
    before_commit: BeforeCommit,
    before_cleanup: BeforeCleanup,
}

fn import_audio_core<FailAt, EnsureCurrent, BeforeStoredCheck, BeforeCommit, BeforeCleanup>(
    catalog: &Catalog,
    store: AudioStore,
    session: &CaptureSession,
    source_path: &Path,
    hooks: ImportHooks<FailAt, EnsureCurrent, BeforeStoredCheck, BeforeCommit, BeforeCleanup>,
) -> Result<AudioChunk, ServiceError>
where
    FailAt: FnMut(ImportFault) -> Result<(), ServiceError>,
    EnsureCurrent: FnMut() -> Result<(), ServiceError>,
    BeforeStoredCheck: FnMut(),
    BeforeCommit: FnMut(),
    BeforeCleanup: FnMut(),
{
    let ImportHooks {
        mut fail_at,
        mut ensure_current,
        mut before_stored_check,
        mut before_commit,
        mut before_cleanup,
    } = hooks;
    let id = format!("chk_{}", Uuid::new_v4().simple());
    let pending = store.write_temp(source_path, &id)?;
    fail_at(ImportFault::AfterTempSync)?;

    let extension = canonical_extension(source_path);
    let stored = store.rename_to_final(&pending, &id, &extension)?;
    fail_at(ImportFault::AfterFinalRename)?;
    store.sync_final(&stored)?;

    let chunk = AudioChunk {
        id,
        session_id: session.id.clone(),
        source: AudioSource::Imported,
        path: stored.relative_path.to_string_lossy().into_owned(),
        sha256: pending.digest,
        byte_length: pending.byte_length,
    };
    let import_result = (|| {
        before_stored_check();
        store.ensure_stored_current(&stored)?;
        ensure_current()?;
        before_commit();
        catalog
            .insert_imported_chunk(session, &chunk, &mut ensure_current)
            .map_err(|error| match error {
                ImportedChunkInsertError::Catalog(error) => ServiceError::Catalog(error),
                ImportedChunkInsertError::Ensure(error) => error,
            })
    })();
    if let Err(error) = import_result {
        before_cleanup();
        store.discard_stored(&stored)?;
        return Err(error);
    }
    Ok(chunk)
}

#[derive(Clone, Copy)]
enum ChunkValidationError {
    Missing,
    Corrupted,
}

fn source_label(source: AudioSource) -> &'static str {
    match source {
        AudioSource::Microphone => "麦克风",
        AudioSource::SystemAudio => "系统音频",
        AudioSource::Imported => "导入音频",
    }
}
