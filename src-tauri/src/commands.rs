use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::catalog::Catalog;
use crate::domain::{
    AudioChunk, CaptureNote, CaptureSession, CaptureState, DictionaryCategory, DictionaryEntry,
    StatsSnapshot, TranscriptRevision, TranscriptSegment, Voiceprint,
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

    fn create_note(
        &self,
        session_id: String,
        content: String,
        timestamp_ms: i64,
        tag: String,
        segment_id: Option<String>,
    ) -> Result<CaptureNote, String> {
        let note = CaptureNote::new(session_id, content, timestamp_ms, tag, segment_id);
        self.with_current_catalog(|catalog| {
            catalog.insert_note(&note).map_err(|e| e.to_string())?;
            Ok(note.clone())
        })
    }

    fn list_notes(&self, session_id: String) -> Result<Vec<CaptureNote>, String> {
        self.with_current_catalog(|catalog| {
            catalog.list_notes(&session_id).map_err(|e| e.to_string())
        })
    }

    fn update_note(&self, note_id: String, content: String, tag: String) -> Result<(), String> {
        self.with_current_catalog(|catalog| {
            catalog
                .update_note(&note_id, &content, &tag)
                .map_err(|e| e.to_string())
        })
    }

    fn delete_note(&self, note_id: String) -> Result<(), String> {
        self.with_current_catalog(|catalog| {
            catalog.delete_note(&note_id).map_err(|e| e.to_string())
        })
    }

    fn create_category(&self, name: String, scope: String) -> Result<DictionaryCategory, String> {
        let cat = DictionaryCategory::new(name, scope);
        self.with_current_catalog(|catalog| {
            catalog.insert_category(&cat).map_err(|e| e.to_string())?;
            Ok(cat.clone())
        })
    }

    fn list_categories(
        &self,
        scope: Option<String>,
    ) -> Result<Vec<DictionaryCategory>, String> {
        self.with_current_catalog(|catalog| {
            catalog
                .list_categories(scope.as_deref())
                .map_err(|e| e.to_string())
        })
    }

    fn delete_category(&self, category_id: String) -> Result<(), String> {
        self.with_current_catalog(|catalog| {
            catalog
                .delete_category(&category_id)
                .map_err(|e| e.to_string())
        })
    }

    fn create_entry(
        &self,
        category_id: String,
        term: String,
        pinyin: String,
        aliases: String,
        note: String,
    ) -> Result<DictionaryEntry, String> {
        let entry = DictionaryEntry::new(category_id, term, pinyin, aliases, note);
        self.with_current_catalog(|catalog| {
            catalog.insert_entry(&entry).map_err(|e| e.to_string())?;
            Ok(entry.clone())
        })
    }

    fn list_entries(
        &self,
        category_id: String,
        query: Option<String>,
    ) -> Result<Vec<DictionaryEntry>, String> {
        self.with_current_catalog(|catalog| {
            catalog
                .list_entries(&category_id, query.as_deref())
                .map_err(|e| e.to_string())
        })
    }

    fn update_entry(
        &self,
        entry_id: String,
        term: String,
        pinyin: String,
        aliases: String,
        note: String,
    ) -> Result<(), String> {
        self.with_current_catalog(|catalog| {
            catalog
                .update_entry(&entry_id, &term, &pinyin, &aliases, &note)
                .map_err(|e| e.to_string())
        })
    }

    fn toggle_entry(&self, entry_id: String, enabled: bool) -> Result<(), String> {
        self.with_current_catalog(|catalog| {
            catalog
                .toggle_entry(&entry_id, enabled)
                .map_err(|e| e.to_string())
        })
    }

    fn delete_entry(&self, entry_id: String) -> Result<(), String> {
        self.with_current_catalog(|catalog| {
            catalog.delete_entry(&entry_id).map_err(|e| e.to_string())
        })
    }

    fn list_voiceprints(&self) -> Result<Vec<Voiceprint>, String> {
        self.with_current_catalog(|catalog| {
            catalog.list_voiceprints().map_err(|e| e.to_string())
        })
    }

    fn register_voiceprint(
        &self,
        name: String,
        embedding_path: String,
        dictionary_entry_id: Option<String>,
    ) -> Result<Voiceprint, String> {
        let vp = Voiceprint::new(name, embedding_path, dictionary_entry_id);
        self.with_current_catalog(|catalog| {
            catalog.insert_voiceprint(&vp).map_err(|e| e.to_string())?;
            Ok(vp.clone())
        })
    }

    fn rename_voiceprint(&self, voiceprint_id: String, name: String) -> Result<(), String> {
        self.with_current_catalog(|catalog| {
            catalog
                .rename_voiceprint(&voiceprint_id, &name)
                .map_err(|e| e.to_string())
        })
    }

    fn delete_voiceprint(&self, voiceprint_id: String) -> Result<(), String> {
        self.with_current_catalog(|catalog| {
            catalog
                .delete_voiceprint(&voiceprint_id)
                .map_err(|e| e.to_string())
        })
    }

    fn link_voiceprint_to_entry(&self, voiceprint_id: String, entry_id: String) -> Result<(), String> {
        self.with_current_catalog(|catalog| {
            catalog
                .link_voiceprint_to_entry(&voiceprint_id, &entry_id)
                .map_err(|e| e.to_string())
        })
    }

    fn get_stats_snapshot(&self, date: Option<String>) -> Result<StatsSnapshot, String> {
        self.with_current_catalog(|catalog| {
            catalog
                .get_stats_snapshot(date.as_deref())
                .map_err(|e| e.to_string())
        })
    }

    fn get_asr_config(&self) -> Result<crate::domain::AsrConfig, String> {
        self.with_current_catalog(|catalog| {
            let json = catalog
                .get_setting("asr_config")
                .map_err(|e| e.to_string())?
                .unwrap_or_else(|| {
                    serde_json::to_string(&crate::domain::AsrConfig {
                        provider: "sense_voice".into(),
                        language: "zh".into(),
                        auto_transcribe: true,
                        threads: 4,
                        vad_enabled: true,
                        vad_min_speech_ms: 300,
                        vad_silence_ms: 800,
                        itn_enabled: true,
                    })
                    .unwrap()
                });
            serde_json::from_str(&json).map_err(|e| e.to_string())
        })
    }

    fn set_asr_config(&self, config: crate::domain::AsrConfig) -> Result<(), String> {
        let json = serde_json::to_string(&config).map_err(|e| e.to_string())?;
        self.with_current_catalog(|catalog| {
            catalog.set_setting("asr_config", &json).map_err(|e| e.to_string())
        })
    }

    fn get_recording_config(&self) -> Result<crate::domain::RecordingConfig, String> {
        self.with_current_catalog(|catalog| {
            let json = catalog
                .get_setting("recording_config")
                .map_err(|e| e.to_string())?
                .unwrap_or_else(|| {
                    serde_json::to_string(&crate::domain::RecordingConfig {
                        capture_mode: "smart".into(),
                        im_detection_enabled: true,
                        im_apps: vec![
                            "wechat".into(), "dingtalk".into(), "feishu".into(),
                            "teams".into(), "zoom".into(), "qq".into(),
                        ],
                        detection_delay_secs: 3,
                        recovery_delay_secs: 5,
                        sample_rate: 16000,
                        storage_path: "~/.lifesub/recordings/".into(),
                    })
                    .unwrap()
                });
            serde_json::from_str(&json).map_err(|e| e.to_string())
        })
    }

    fn set_recording_config(&self, config: crate::domain::RecordingConfig) -> Result<(), String> {
        let json = serde_json::to_string(&config).map_err(|e| e.to_string())?;
        self.with_current_catalog(|catalog| {
            catalog.set_setting("recording_config", &json).map_err(|e| e.to_string())
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

// ── Task 13.5 commands ───────────────────────────────────────────────────

#[tauri::command]
pub fn create_note(
    session_id: String,
    content: String,
    timestamp_ms: i64,
    tag: String,
    segment_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<CaptureNote, String> {
    state.create_note(session_id, content, timestamp_ms, tag, segment_id)
}

#[tauri::command]
pub fn list_notes(session_id: String, state: State<'_, AppState>) -> Result<Vec<CaptureNote>, String> {
    state.list_notes(session_id)
}

#[tauri::command]
pub fn update_note(note_id: String, content: String, tag: String, state: State<'_, AppState>) -> Result<(), String> {
    state.update_note(note_id, content, tag)
}

#[tauri::command]
pub fn delete_note(note_id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.delete_note(note_id)
}

#[tauri::command]
pub fn create_category(name: String, scope: String, state: State<'_, AppState>) -> Result<DictionaryCategory, String> {
    state.create_category(name, scope)
}

#[tauri::command]
pub fn list_categories(scope: Option<String>, state: State<'_, AppState>) -> Result<Vec<DictionaryCategory>, String> {
    state.list_categories(scope)
}

#[tauri::command]
pub fn delete_category(category_id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.delete_category(category_id)
}

#[tauri::command]
pub fn create_entry(
    category_id: String,
    term: String,
    pinyin: String,
    aliases: String,
    note: String,
    state: State<'_, AppState>,
) -> Result<DictionaryEntry, String> {
    state.create_entry(category_id, term, pinyin, aliases, note)
}

#[tauri::command]
pub fn list_entries(
    category_id: String,
    query: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<DictionaryEntry>, String> {
    state.list_entries(category_id, query)
}

#[tauri::command]
pub fn update_entry(
    entry_id: String,
    term: String,
    pinyin: String,
    aliases: String,
    note: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.update_entry(entry_id, term, pinyin, aliases, note)
}

#[tauri::command]
pub fn toggle_entry(entry_id: String, enabled: bool, state: State<'_, AppState>) -> Result<(), String> {
    state.toggle_entry(entry_id, enabled)
}

#[tauri::command]
pub fn delete_entry(entry_id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.delete_entry(entry_id)
}

#[tauri::command]
pub fn list_voiceprints(state: State<'_, AppState>) -> Result<Vec<Voiceprint>, String> {
    state.list_voiceprints()
}

#[tauri::command]
pub fn register_voiceprint(
    name: String,
    embedding_path: String,
    dictionary_entry_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<Voiceprint, String> {
    state.register_voiceprint(name, embedding_path, dictionary_entry_id)
}

#[tauri::command]
pub fn rename_voiceprint(voiceprint_id: String, name: String, state: State<'_, AppState>) -> Result<(), String> {
    state.rename_voiceprint(voiceprint_id, name)
}

#[tauri::command]
pub fn delete_voiceprint(voiceprint_id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.delete_voiceprint(voiceprint_id)
}

#[tauri::command]
pub fn link_voiceprint_to_entry(
    voiceprint_id: String,
    entry_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.link_voiceprint_to_entry(voiceprint_id, entry_id)
}

#[tauri::command]
pub fn get_stats_snapshot(date: Option<String>, state: State<'_, AppState>) -> Result<StatsSnapshot, String> {
    state.get_stats_snapshot(date)
}

#[tauri::command]
pub fn get_asr_config(state: State<'_, AppState>) -> Result<crate::domain::AsrConfig, String> {
    state.get_asr_config()
}

#[tauri::command]
pub fn set_asr_config(config: crate::domain::AsrConfig, state: State<'_, AppState>) -> Result<(), String> {
    state.set_asr_config(config)
}

#[tauri::command]
pub fn get_recording_config(state: State<'_, AppState>) -> Result<crate::domain::RecordingConfig, String> {
    state.get_recording_config()
}

#[tauri::command]
pub fn set_recording_config(config: crate::domain::RecordingConfig, state: State<'_, AppState>) -> Result<(), String> {
    state.set_recording_config(config)
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
