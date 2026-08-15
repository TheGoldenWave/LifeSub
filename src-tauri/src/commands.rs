use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::catalog::Catalog;
use crate::domain::{
    AudioChunk, CaptureSession, CaptureState, TranscriptRevision, TranscriptSegment,
};
use crate::service::{
    parse_evidence_uri, CoreRuntime, EvidenceService, RuntimeOwnershipGuard,
};

pub struct AppState {
    pub catalog: Catalog,
    pub data_dir: PathBuf,
    runtime_ownership: RuntimeOwnershipGuard,
}

#[derive(Serialize)]
pub struct EvidenceResolution {
    pub kind: String,
    pub id: String,
    pub start_seconds: Option<i64>,
    pub end_seconds: Option<i64>,
    pub revision: Option<i64>,
}

impl AppState {
    pub fn initialize(app: &AppHandle) -> Result<Self, String> {
        let data_dir = app
            .path()
            .app_data_dir()
            .map_err(|error| error.to_string())?;
        Self::initialize_at(data_dir)
    }

    fn initialize_at(data_dir: PathBuf) -> Result<Self, String> {
        let runtime = CoreRuntime::initialize(&data_dir).map_err(|error| format!("{error:?}"))?;
        let (catalog, runtime_ownership) = runtime.into_parts();
        // Task 11 replaces secondary failure with socket connection and structured Tauri errors.
        Ok(Self {
            catalog,
            data_dir,
            runtime_ownership,
        })
    }

    fn with_current_catalog<T>(
        &self,
        mutation: impl FnOnce(&Catalog) -> Result<T, String>,
    ) -> Result<T, String> {
        self.runtime_ownership
            .ensure_current()
            .map_err(|error| format!("{error:?}"))?;
        mutation(&self.catalog)
    }

    fn with_current_service<T>(
        &self,
        mutation: impl FnOnce(&Catalog, &std::path::Path) -> Result<T, String>,
    ) -> Result<T, String> {
        self.runtime_ownership
            .ensure_current()
            .map_err(|error| format!("{error:?}"))?;
        mutation(&self.catalog, &self.data_dir)
    }

    fn create_capture_session(&self, title: String) -> Result<CaptureSession, String> {
        let session = CaptureSession::new(title);
        self.with_current_catalog(|catalog| {
            catalog
                .insert_session(&session)
                .map_err(|error| error.to_string())?;
            Ok(session)
        })
    }

    fn transition_capture_session(
        &self,
        session: CaptureSession,
        target: CaptureState,
    ) -> Result<CaptureSession, String> {
        let session = session
            .transition(target)
            .map_err(|error| format!("{error:?}"))?;
        self.with_current_catalog(|catalog| {
            catalog
                .update_session(&session)
                .map_err(|error| error.to_string())?;
            Ok(session)
        })
    }

    fn import_audio_file(
        &self,
        session: CaptureSession,
        path: String,
    ) -> Result<AudioChunk, String> {
        self.with_current_service(|catalog, data_dir| {
            EvidenceService::import_audio_with_existing_catalog(catalog, data_dir, &session, path)
                .map_err(|error| format!("{error:?}"))
        })
    }

    fn append_transcript_revision(
        &self,
        session_id: String,
        provider: String,
        segments: Vec<TranscriptSegment>,
    ) -> Result<TranscriptRevision, String> {
        self.with_current_catalog(|catalog| {
            catalog
                .append_revision(&session_id, &provider, segments)
                .map_err(|error| error.to_string())
        })
    }
}

#[tauri::command]
pub fn create_capture_session(
    title: String,
    state: State<'_, AppState>,
) -> Result<CaptureSession, String> {
    state.create_capture_session(title)
}

#[tauri::command]
pub fn transition_capture_session(
    session: CaptureSession,
    target: CaptureState,
    state: State<'_, AppState>,
) -> Result<CaptureSession, String> {
    state.transition_capture_session(session, target)
}

#[tauri::command]
pub fn import_audio_file(
    session: CaptureSession,
    path: String,
    state: State<'_, AppState>,
) -> Result<AudioChunk, String> {
    state.import_audio_file(session, path)
}

#[tauri::command]
pub fn append_transcript_revision(
    session_id: String,
    provider: String,
    segments: Vec<TranscriptSegment>,
    state: State<'_, AppState>,
) -> Result<TranscriptRevision, String> {
    state.append_transcript_revision(session_id, provider, segments)
}

#[tauri::command]
pub fn search_transcripts(
    query: String,
    state: State<'_, AppState>,
) -> Result<Vec<TranscriptSegment>, String> {
    state
        .catalog
        .search_segments(&query)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn resolve_evidence(uri: String) -> Result<EvidenceResolution, String> {
    use crate::service::EvidenceTarget;
    match parse_evidence_uri(&uri).map_err(|error| format!("{error:?}"))? {
        EvidenceTarget::Record { id } => Ok(EvidenceResolution {
            kind: "record".into(),
            id,
            start_seconds: None,
            end_seconds: None,
            revision: None,
        }),
        EvidenceTarget::Segment { id, revision } => Ok(EvidenceResolution {
            kind: "segment".into(),
            id,
            start_seconds: None,
            end_seconds: None,
            revision,
        }),
        EvidenceTarget::Audio {
            id,
            start_seconds,
            end_seconds,
        } => Ok(EvidenceResolution {
            kind: "audio".into(),
            id,
            start_seconds,
            end_seconds,
            revision: None,
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;
    use crate::domain::{AudioSource, TranscriptSegment};

    #[test]
    fn invalidated_ownership_blocks_every_write_class_without_db_changes() {
        let parent = tempdir().unwrap();
        let data_dir = parent.path().join("data");
        let state = AppState::initialize_at(data_dir.clone()).unwrap();
        let session = state
            .create_capture_session("guarded baseline".into())
            .unwrap();
        let source = parent.path().join("sample.wav");
        fs::write(&source, b"audio").unwrap();
        let baseline_sessions = state.catalog.session_count().unwrap();
        let lock = data_dir.join("asr-worker.lock");
        fs::rename(&lock, data_dir.join("old-lock")).unwrap();
        fs::write(&lock, b"replacement").unwrap();

        assert!(state.create_capture_session("blocked create".into()).is_err());
        assert!(state
            .transition_capture_session(session.clone(), CaptureState::Recording)
            .is_err());
        assert!(state
            .append_transcript_revision(
                session.id.clone(),
                "blocked".into(),
                vec![TranscriptSegment::new(
                    0,
                    1,
                    AudioSource::Imported,
                    "blocked",
                )],
            )
            .is_err());
        assert!(state
            .import_audio_file(session.clone(), source.to_string_lossy().into_owned())
            .is_err());

        assert_eq!(state.catalog.session_count().unwrap(), baseline_sessions);
        assert_eq!(
            state.catalog.persisted_session_state(&session.id).unwrap(),
            "idle"
        );
        assert!(state.catalog.list_revisions(&session.id).unwrap().is_empty());
        assert!(state.catalog.list_chunks().unwrap().is_empty());
        assert!(state.catalog.search_segments("blocked").is_ok());
    }

    #[test]
    fn guarded_import_uses_existing_catalog_connection_without_reopening_path() {
        let parent = tempdir().unwrap();
        let data_dir = parent.path().join("data");
        let runtime = CoreRuntime::initialize(&data_dir).unwrap();
        let (_disk_catalog, runtime_ownership) = runtime.into_parts();
        let state = AppState {
            catalog: Catalog::in_memory().unwrap(),
            data_dir: data_dir.clone(),
            runtime_ownership,
        };
        let session = state
            .create_capture_session("existing catalog import".into())
            .unwrap();
        let source = parent.path().join("sample.wav");
        fs::write(&source, b"audio").unwrap();

        let chunk = state
            .import_audio_file(session, source.to_string_lossy().into_owned())
            .unwrap();

        assert_eq!(state.catalog.list_chunks().unwrap(), vec![chunk]);
    }
}
