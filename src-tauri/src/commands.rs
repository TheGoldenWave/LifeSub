use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::catalog::Catalog;
use crate::domain::{
    AudioChunk, CaptureSession, CaptureState, TranscriptRevision, TranscriptSegment,
};
use crate::service::{CoreRuntime, EvidenceService, parse_evidence_uri};

pub struct AppState {
    runtime: CoreRuntime,
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
        // Task 11 replaces secondary failure with socket connection and structured Tauri errors.
        Ok(Self { runtime })
    }

    fn with_current_catalog<T>(
        &self,
        mutation: impl FnOnce(&Catalog) -> Result<T, String>,
    ) -> Result<T, String> {
        self.runtime
            .ensure_current()
            .map_err(|error| format!("{error:?}"))?;
        mutation(self.runtime.catalog_ref())
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
        let data_dir = self
            .runtime
            .try_clone_data_directory()
            .map_err(|error| format!("{error:?}"))?;
        EvidenceService::import_audio_with_existing_catalog(
            self.runtime.catalog_ref(),
            data_dir,
            &session,
            path,
            || self.ensure_import_current(),
        )
        .map_err(|error| format!("{error:?}"))
    }

    fn ensure_import_current(&self) -> Result<(), crate::service::ServiceError> {
        self.runtime.ensure_current().map_err(|error| {
            crate::service::ServiceError::Io(std::io::Error::other(format!("{error:?}")))
        })
    }

    #[cfg(test)]
    fn import_audio_file_with_open_barriers(
        &self,
        session: CaptureSession,
        path: String,
        checked: &std::sync::Barrier,
        resume: &std::sync::Barrier,
    ) -> Result<AudioChunk, String> {
        let data_dir = self
            .runtime
            .try_clone_data_directory()
            .map_err(|error| format!("{error:?}"))?;
        checked.wait();
        resume.wait();
        EvidenceService::import_audio_with_existing_catalog(
            self.runtime.catalog_ref(),
            data_dir,
            &session,
            path,
            || self.ensure_import_current(),
        )
        .map_err(|error| format!("{error:?}"))
    }

    #[cfg(test)]
    fn import_audio_file_with_commit_barriers(
        &self,
        session: CaptureSession,
        path: String,
        checked: &std::sync::Barrier,
        resume: &std::sync::Barrier,
    ) -> Result<AudioChunk, String> {
        let data_dir = self
            .runtime
            .try_clone_data_directory()
            .map_err(|error| format!("{error:?}"))?;
        EvidenceService::import_audio_with_existing_catalog_and_commit_barriers(
            self.runtime.catalog_ref(),
            data_dir,
            &session,
            path,
            || self.ensure_import_current(),
            checked,
            resume,
        )
        .map_err(|error| format!("{error:?}"))
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
        .runtime
        .catalog_ref()
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
    use std::sync::{Arc, Barrier};

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
        let baseline_sessions = state.runtime.catalog_ref().session_count().unwrap();
        let lock = data_dir.join("asr-worker.lock");
        fs::rename(&lock, data_dir.join("old-lock")).unwrap();
        fs::write(&lock, b"replacement").unwrap();

        assert!(
            state
                .create_capture_session("blocked create".into())
                .is_err()
        );
        assert!(
            state
                .transition_capture_session(session.clone(), CaptureState::Recording)
                .is_err()
        );
        assert!(
            state
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
                .is_err()
        );
        assert!(
            state
                .import_audio_file(session.clone(), source.to_string_lossy().into_owned())
                .is_err()
        );

        assert_eq!(
            state.runtime.catalog_ref().session_count().unwrap(),
            baseline_sessions
        );
        assert_eq!(
            state
                .runtime
                .catalog_ref()
                .persisted_session_state(&session.id)
                .unwrap(),
            "idle"
        );
        assert!(
            state
                .runtime
                .catalog_ref()
                .list_revisions(&session.id)
                .unwrap()
                .is_empty()
        );
        assert!(
            state
                .runtime
                .catalog_ref()
                .list_chunks()
                .unwrap()
                .is_empty()
        );
        assert!(
            state
                .runtime
                .catalog_ref()
                .search_segments("blocked")
                .is_ok()
        );
    }

    #[test]
    fn guarded_import_uses_existing_catalog_connection_without_reopening_path() {
        let parent = tempdir().unwrap();
        let data_dir = parent.path().join("data");
        let runtime = CoreRuntime::initialize(&data_dir).unwrap();
        let state = AppState { runtime };
        let session = state
            .create_capture_session("existing catalog import".into())
            .unwrap();
        let source = parent.path().join("sample.wav");
        fs::write(&source, b"audio").unwrap();

        let chunk = state
            .import_audio_file(session, source.to_string_lossy().into_owned())
            .unwrap();

        assert_eq!(
            state.runtime.catalog_ref().list_chunks().unwrap(),
            vec![chunk]
        );
    }

    #[test]
    fn guarded_import_rejects_data_root_swap_between_check_and_audio_open() {
        let parent = tempdir().unwrap();
        let data_dir = parent.path().join("data");
        let state = AppState::initialize_at(data_dir.clone()).unwrap();
        let session = CaptureSession::new("guarded import race");
        let source = parent.path().join("sample.wav");
        fs::write(&source, b"audio").unwrap();
        let checked = Arc::new(Barrier::new(2));
        let resume = Arc::new(Barrier::new(2));
        let import_checked = Arc::clone(&checked);
        let import_resume = Arc::clone(&resume);
        let import = std::thread::spawn(move || {
            let result = state.import_audio_file_with_open_barriers(
                session,
                source.to_string_lossy().into_owned(),
                &import_checked,
                &import_resume,
            );
            (state, result)
        });

        checked.wait();
        let anchored_data_dir = parent.path().join("anchored-data");
        fs::rename(&data_dir, &anchored_data_dir).unwrap();
        fs::create_dir(&data_dir).unwrap();
        resume.wait();

        let (state, result) = import.join().unwrap();
        assert!(result.is_err());
        assert_eq!(state.runtime.catalog_ref().session_count().unwrap(), 0);
        assert!(
            state
                .runtime
                .catalog_ref()
                .list_chunks()
                .unwrap()
                .is_empty()
        );
        assert!(audio_files(&data_dir).is_empty());
        assert!(audio_files(&anchored_data_dir).is_empty());
    }

    #[test]
    fn guarded_import_rolls_back_swap_after_storage_check_before_commit() {
        let parent = tempdir().unwrap();
        let data_dir = parent.path().join("data");
        let state = AppState::initialize_at(data_dir.clone()).unwrap();
        let session = CaptureSession::new("guarded commit race");
        let source = parent.path().join("sample.wav");
        fs::write(&source, b"audio").unwrap();
        let checked = Arc::new(Barrier::new(2));
        let resume = Arc::new(Barrier::new(2));
        let import_checked = Arc::clone(&checked);
        let import_resume = Arc::clone(&resume);
        let import = std::thread::spawn(move || {
            let result = state.import_audio_file_with_commit_barriers(
                session,
                source.to_string_lossy().into_owned(),
                &import_checked,
                &import_resume,
            );
            (state, result)
        });

        checked.wait();
        let anchored_data_dir = parent.path().join("anchored-data");
        fs::rename(&data_dir, &anchored_data_dir).unwrap();
        fs::create_dir(&data_dir).unwrap();
        resume.wait();

        let (state, result) = import.join().unwrap();
        assert!(result.is_err());
        assert_eq!(state.runtime.catalog_ref().session_count().unwrap(), 0);
        assert!(
            state
                .runtime
                .catalog_ref()
                .list_chunks()
                .unwrap()
                .is_empty()
        );
        assert!(audio_files(&data_dir).is_empty());
        assert!(audio_files(&anchored_data_dir).is_empty());
    }

    fn audio_files(data_dir: &std::path::Path) -> Vec<std::path::PathBuf> {
        let audio_dir = data_dir.join("audio");
        if !audio_dir.exists() {
            return Vec::new();
        }
        fs::read_dir(audio_dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect()
    }
}
