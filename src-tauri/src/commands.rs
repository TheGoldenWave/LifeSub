use sha2::Digest;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::asr::job::canonical_time;
use crate::asr::manifest::{QualificationPolicy, RuntimeRequirement, model_registry, vad_manifest};
use crate::asr::model_lookup::{
    DeviceSupport, InstallationQualification, ModelCapabilities, ModelLookup, ModelLookupContext,
};
use crate::asr::model_manager::DeviceProfile;
use crate::asr::provider::{ProviderOptions, ProviderSelection};
use crate::asr::service::{AsrEnqueueRequest, DEFAULT_VAD_MODEL_ID, EnqueueProviderFactory};
use crate::asr::settings::{AsrProviderOptions, AsrSettings, WhisperTask};
use crate::capture::StreamingCapture;
use crate::catalog::{Catalog, TimelineChunk, TimelineJobSummary};
use crate::domain::{
    AsrErrorCode, AsrLanguage, AsrProviderKind, AudioChunk, CaptureNote, CaptureSession,
    CaptureState, ChunkIntegrityState, DictionaryCategory, DictionaryEntry, ImportAsrDisposition,
    ImportModelReadiness, StatsSnapshot, TranscriptRevision, TranscriptSegment, Voiceprint,
};
use crate::desktop_runtime::{DesktopRuntimeFactory, ProductionDesktopRuntimeFactory};
use crate::quick_input::QuickInput;
use crate::service::{CoreRuntime, EvidenceService, parse_evidence_uri};

pub struct AppState {
    runtime: Arc<CoreRuntime>,
    data_dir: PathBuf,
    streaming: Mutex<StreamingCapture>,
    worker_shutdown: Arc<AtomicBool>,
    worker_handle: Mutex<Option<std::thread::JoinHandle<()>>>,
    pub quick_input: QuickInput,
}

#[derive(Serialize)]
pub struct EvidenceResolution {
    pub kind: String,
    pub id: String,
    pub start_seconds: Option<i64>,
    pub end_seconds: Option<i64>,
    pub revision: Option<i64>,
}

#[derive(Serialize)]
pub struct TimelineRecordPayload {
    pub session: CaptureSession,
    pub chunks: Vec<TimelineChunkPayload>,
    pub latest_job: Option<TimelineJobSummaryPayload>,
    pub revisions: Vec<TranscriptRevision>,
    pub notes: Vec<CaptureNote>,
}

#[derive(Serialize)]
pub struct ImportAudioOutcomePayload {
    pub session: CaptureSession,
    pub chunk: AudioChunk,
    pub job: Option<TimelineJobSummaryPayload>,
    pub asr_warning: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TimelineChunkPayload {
    pub id: String,
    pub source: String,
    pub audio_path: String,
    pub integrity_state: String,
    pub error_code: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TimelineJobSummaryPayload {
    pub id: String,
    pub chunk_id: String,
    pub state: String,
    pub error_code: Option<String>,
    pub error_summary: Option<String>,
}

#[derive(Serialize)]
pub struct AppRuntimeInfo {
    pub app_version: String,
    pub tauri_version: String,
    pub frontend_stack: String,
    pub asr_runtime: String,
}

#[derive(Serialize)]
pub struct ModelDownloadPayload {
    pub state: String,
}

#[derive(Serialize)]
pub struct AsrModelPayload {
    pub model_id: String,
    pub display_name: String,
    pub provider: String,
    pub manifest_version: String,
    pub bundle_identity: String,
    pub supported_languages: Vec<String>,
    pub qualification_policy: String,
    pub runtime_family: String,
    pub runtime_version: String,
    pub artifact_count: usize,
    pub total_bytes: u64,
    pub license_spdx: String,
    pub installation_state: String,
    pub selectable: bool,
    pub installable: bool,
    pub executable: bool,
    pub reason_code: Option<String>,
    pub last_error_code: Option<String>,
    pub download: Option<ModelDownloadPayload>,
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
        let runtime =
            Arc::new(CoreRuntime::initialize(&data_dir).map_err(|error| format!("{error:?}"))?);
        let (worker_shutdown, worker_handle) =
            ProductionDesktopRuntimeFactory::spawn_worker(runtime.clone());
        Ok(Self {
            runtime,
            data_dir,
            streaming: Mutex::new(ProductionDesktopRuntimeFactory::create_capture()),
            worker_shutdown,
            worker_handle: Mutex::new(worker_handle),
            quick_input: QuickInput::default(),
        })
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

    #[cfg(test)]
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

    fn create_manual_revision(
        &self,
        session_id: String,
        segments: Vec<TranscriptSegment>,
    ) -> Result<TranscriptRevision, String> {
        self.with_current_catalog(|catalog| {
            catalog.append_manual_revision_from_latest(&session_id, segments)
        })
    }

    fn import_audio_record(
        &self,
        path: String,
        title: String,
    ) -> Result<ImportAudioOutcomePayload, String> {
        let mut session = CaptureSession::new(title);
        session.state = CaptureState::Stopped;
        session.ended_at = Some(chrono::Utc::now());
        let chunk = self.import_audio_file(session.clone(), path)?;
        let (job, asr_warning) = match self.enqueue_import_job(&session, &chunk) {
            Ok(job) => (job, None),
            Err(error) => {
                let failed = self.persist_import_preflight_failure(&session, &chunk, &error)?;
                (Some(failed), Some(error))
            }
        };
        Ok(ImportAudioOutcomePayload {
            session,
            chunk,
            job,
            asr_warning,
        })
    }

    fn persist_import_preflight_failure(
        &self,
        session: &CaptureSession,
        chunk: &AudioChunk,
        summary: &str,
    ) -> Result<TimelineJobSummaryPayload, String> {
        let config = self.get_asr_config().ok();
        let provider = config
            .as_ref()
            .map(|value| value.provider.as_str())
            .filter(|value| matches!(*value, "sense_voice" | "whisper" | "qwen3_asr"))
            .unwrap_or("sense_voice");
        let model_id = config
            .as_ref()
            .map(|value| value.model_id.as_str())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("unresolved");
        let now = canonical_time(chrono::Utc::now());
        let job = crate::asr::job::EnqueueJob {
            id: format!("asr_{}", uuid::Uuid::new_v4().simple()),
            session_id: session.id.clone(),
            chunk_id: chunk.id.clone(),
            provider: provider.into(),
            model_id: model_id.into(),
            manifest_version: "unresolved".into(),
            archive_sha256: "0".repeat(64),
            required_file_hashes_json: "{}".into(),
            model_source_json: "{}".into(),
            vad_model_id: None,
            vad_manifest_version: None,
            vad_archive_sha256: None,
            vad_required_file_hashes_json: None,
            parameters_json: serde_json::json!({ "preflight_error": summary }).to_string(),
            input_sha256: chunk.sha256.clone(),
            fingerprint: hex::encode(sha2::Sha256::digest(
                format!("{}\0{}\0preflight", session.id, chunk.id).as_bytes(),
            )),
            available_at: now.clone(),
            created_at: now,
        };
        self.runtime
            .catalog_ref()
            .insert_failed_asr_job(
                &job,
                asr_error_name(AsrErrorCode::InvalidProviderParameter),
                summary,
            )
            .map_err(|error| format!("{error:?}"))?;
        Ok(TimelineJobSummaryPayload {
            id: job.id,
            chunk_id: chunk.id.clone(),
            state: "failed".into(),
            error_code: Some(asr_error_name(AsrErrorCode::InvalidProviderParameter).into()),
            error_summary: Some(summary.to_owned()),
        })
    }

    fn list_timeline_records(&self) -> Result<Vec<TimelineRecordPayload>, String> {
        let data_dir = self.data_dir.clone();
        self.with_current_catalog(|catalog| {
            let sessions = catalog.list_sessions().map_err(|error| error.to_string())?;
            sessions
                .into_iter()
                .map(|session| {
                    let chunks = catalog
                        .list_chunks_for_session(&session.id)
                        .map_err(|error| error.to_string())?
                        .into_iter()
                        .map(|chunk| map_timeline_chunk(chunk, &data_dir))
                        .collect::<Vec<_>>();
                    let latest_job = catalog
                        .latest_job_for_session(&session.id)
                        .map_err(|error| error.to_string())?
                        .map(map_timeline_job);
                    let revisions = catalog
                        .list_revisions_with_segments(&session.id)
                        .map_err(|error| error.to_string())?;
                    let notes = catalog
                        .list_notes(&session.id)
                        .map_err(|error| error.to_string())?;
                    Ok(TimelineRecordPayload {
                        session,
                        chunks,
                        latest_job,
                        revisions,
                        notes,
                    })
                })
                .collect()
        })
    }

    fn enqueue_import_job(
        &self,
        session: &CaptureSession,
        chunk: &AudioChunk,
    ) -> Result<Option<TimelineJobSummaryPayload>, String> {
        let config = self.get_asr_config()?;
        if !config.auto_transcribe {
            return Ok(None);
        }

        config.validate_for_persistence()?;
        let selection = resolve_import_model(self, &config)?;
        let settings = asr_settings_from_config(&config, &selection.model_id)?;
        let provider_selection = provider_selection_from_settings(&settings);
        let manifest = model_registry()
            .model(&selection.model_id)
            .ok_or_else(|| "selected model manifest missing".to_owned())?;
        let fingerprint = import_fingerprint(
            &session.id,
            &chunk.id,
            &chunk.sha256,
            manifest.id,
            manifest.bundle.identity_sha256,
            &settings,
        )?;
        let job_id = format!("asr_{}", uuid::Uuid::new_v4().simple());
        let now = canonical_time(chrono::Utc::now());
        let job = crate::asr::job::EnqueueJob {
            id: job_id.clone(),
            session_id: session.id.clone(),
            chunk_id: chunk.id.clone(),
            provider: provider_name(settings.provider).to_owned(),
            model_id: manifest.id.to_owned(),
            manifest_version: manifest.manifest_version.to_owned(),
            archive_sha256: manifest.bundle.identity_sha256.to_owned(),
            required_file_hashes_json: import_required_file_hashes(manifest)?,
            model_source_json: import_model_source(manifest)?,
            vad_model_id: if settings.vad_enabled {
                Some(DEFAULT_VAD_MODEL_ID.to_owned())
            } else {
                None
            },
            vad_manifest_version: if settings.vad_enabled {
                Some(vad_manifest().manifest_version.to_owned())
            } else {
                None
            },
            vad_archive_sha256: if settings.vad_enabled {
                Some(vad_manifest().bundle.identity_sha256.to_owned())
            } else {
                None
            },
            vad_required_file_hashes_json: if settings.vad_enabled {
                Some(import_vad_required_file_hashes()?)
            } else {
                None
            },
            parameters_json: serde_json::to_string(&settings).map_err(|error| error.to_string())?,
            input_sha256: chunk.sha256.clone(),
            fingerprint,
            available_at: now.clone(),
            created_at: now,
        };

        let vad_ready =
            !settings.vad_enabled || default_vad_is_executable(self.runtime.catalog_ref())?;

        if selection.executable && vad_ready {
            let lookup = ImportModelLookup {
                selected_model_id: selection.model_id.clone(),
                provider: selection.provider,
                supported_languages: manifest
                    .supported_languages
                    .iter()
                    .map(|value| (*value).to_owned())
                    .collect(),
                model_executable: true,
                vad_executable: vad_ready,
            };
            let service = self
                .runtime
                .asr_service(&lookup, &NoopEnqueueProviderFactory);
            match service.enqueue(AsrEnqueueRequest {
                session_id: session.id.clone(),
                chunk_id: chunk.id.clone(),
                input_sha256: chunk.sha256.clone(),
                settings,
                selection: provider_selection,
                vad_model_id: if job.vad_model_id.is_some() {
                    Some(DEFAULT_VAD_MODEL_ID.to_owned())
                } else {
                    None
                },
            }) {
                Ok(outcome) => {
                    return Ok(Some(TimelineJobSummaryPayload {
                        id: outcome.job_id,
                        chunk_id: chunk.id.clone(),
                        state: "queued".into(),
                        error_code: None,
                        error_summary: None,
                    }));
                }
                Err(error) => {
                    let (state, summary) = match ImportAsrDisposition::classify(
                        true,
                        ImportModelReadiness::Ready,
                        Err(error),
                    ) {
                        ImportAsrDisposition::BlockedModel(code) => {
                            let summary =
                                format!("auto-transcribe blocked: {}", asr_error_name(code));
                            self.runtime
                                .catalog_ref()
                                .insert_blocked_asr_job(&job, asr_error_name(code), &summary)
                                .map_err(|error| format!("{error:?}"))?;
                            ("blocked_model", summary)
                        }
                        ImportAsrDisposition::Failed(code) => {
                            let summary =
                                format!("auto-transcribe failed: {}", asr_error_name(code));
                            self.runtime
                                .catalog_ref()
                                .insert_failed_asr_job(&job, asr_error_name(code), &summary)
                                .map_err(|error| format!("{error:?}"))?;
                            ("failed", summary)
                        }
                        _ => unreachable!("ready enqueue errors classify as blocked or failed"),
                    };
                    return Ok(Some(TimelineJobSummaryPayload {
                        id: job.id,
                        chunk_id: chunk.id.clone(),
                        state: state.into(),
                        error_code: Some(asr_error_name(error).into()),
                        error_summary: Some(summary),
                    }));
                }
            }
        }

        let error = if !vad_ready {
            AsrErrorCode::ModelCapabilityUnavailable
        } else if selection.reason_code.as_deref() == Some("model_not_installed") {
            AsrErrorCode::ModelNotInstalled
        } else {
            AsrErrorCode::ModelCapabilityUnavailable
        };
        let error_code = asr_error_name(error).to_owned();
        self.runtime
            .catalog_ref()
            .insert_blocked_asr_job(
                &job,
                &error_code,
                &format!("auto-transcribe blocked: {error_code}"),
            )
            .map_err(|error| format!("{error:?}"))?;
        Ok(Some(TimelineJobSummaryPayload {
            id: job.id,
            chunk_id: chunk.id.clone(),
            state: "blocked_model".into(),
            error_code: Some(error_code.clone()),
            error_summary: Some(format!("auto-transcribe blocked: {error_code}")),
        }))
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

    fn list_categories(&self, scope: Option<String>) -> Result<Vec<DictionaryCategory>, String> {
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
        self.with_current_catalog(|catalog| catalog.list_voiceprints().map_err(|e| e.to_string()))
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

    fn link_voiceprint_to_entry(
        &self,
        voiceprint_id: String,
        entry_id: String,
    ) -> Result<(), String> {
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
                        model_id: "sense-voice-small-int8-2024-07-17".into(),
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
        config.validate_for_persistence()?;
        let json = serde_json::to_string(&config).map_err(|e| e.to_string())?;
        self.with_current_catalog(|catalog| {
            catalog
                .set_setting("asr_config", &json)
                .map_err(|e| e.to_string())
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
                            "wechat".into(),
                            "dingtalk".into(),
                            "feishu".into(),
                            "teams".into(),
                            "zoom".into(),
                            "qq".into(),
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
            catalog
                .set_setting("recording_config", &json)
                .map_err(|e| e.to_string())
        })
    }

    fn list_asr_models(&self) -> Result<Vec<AsrModelPayload>, String> {
        let installations = self
            .runtime
            .catalog_ref()
            .model_installation_records()
            .map_err(|error| error.to_string())?;
        let downloads = self
            .runtime
            .catalog_ref()
            .model_download_records()
            .map_err(|error| error.to_string())?;
        let installations = installations
            .into_iter()
            .map(|record| (record.model_id.clone(), record))
            .collect::<HashMap<_, _>>();
        let device = DeviceProfile::current();

        Ok(model_registry()
            .models()
            .iter()
            .map(|manifest| {
                let installation = installations.get(manifest.id).filter(|record| {
                    record.manifest_version == manifest.manifest_version
                        && record.bundle_identity == manifest.bundle.identity_sha256
                });
                let installation_state = installation
                    .map(|record| record.state.as_str())
                    .unwrap_or("not_installed");
                let qualification = match installation_state {
                    "runtime_qualified" => InstallationQualification::RuntimeQualified,
                    "installed_unqualified" => InstallationQualification::InstalledUnqualified,
                    _ => InstallationQualification::NotInstalled,
                };
                let capabilities = model_registry()
                    .lookup_with_context(
                        manifest.id,
                        ModelLookupContext::new(device_support(manifest, &device), qualification),
                    )
                    .expect("manifest registry lookup must succeed");

                let (runtime_family, runtime_version) = runtime_descriptor(manifest.runtime);

                AsrModelPayload {
                    model_id: manifest.id.to_owned(),
                    display_name: manifest.display_name.to_owned(),
                    provider: provider_name(manifest.provider).to_owned(),
                    manifest_version: manifest.manifest_version.to_owned(),
                    bundle_identity: manifest.bundle.identity_sha256.to_owned(),
                    supported_languages: manifest
                        .supported_languages
                        .iter()
                        .map(|language| (*language).to_owned())
                        .collect(),
                    qualification_policy: qualification_name(manifest.qualification_policy)
                        .to_owned(),
                    runtime_family: runtime_family.to_owned(),
                    runtime_version: runtime_version.to_owned(),
                    artifact_count: manifest.bundle.artifacts.len(),
                    total_bytes: manifest
                        .bundle
                        .artifacts
                        .iter()
                        .map(|artifact| artifact.bytes)
                        .sum(),
                    license_spdx: manifest.source.license_spdx.to_owned(),
                    installation_state: installation_state.to_owned(),
                    selectable: capabilities.selectable,
                    installable: capabilities.installable,
                    executable: capabilities.executable,
                    reason_code: capabilities.reason_code,
                    last_error_code: installation.and_then(|record| record.last_error_code.clone()),
                    download: downloads
                        .iter()
                        .find(|record| {
                            record.model_id == manifest.id
                                && record.manifest_version == manifest.manifest_version
                                && record.bundle_identity == manifest.bundle.identity_sha256
                        })
                        .map(|record| ModelDownloadPayload {
                            state: record.state.clone(),
                        }),
                }
            })
            .collect())
    }
}

impl Drop for AppState {
    fn drop(&mut self) {
        self.worker_shutdown.store(true, Ordering::Release);
        if let Some(handle) = self.worker_handle.lock().unwrap().take() {
            let _ = handle.join();
        }
    }
}

struct NoopEnqueueProviderFactory;

impl EnqueueProviderFactory for NoopEnqueueProviderFactory {
    fn validate_constructible(
        &self,
        _settings: &AsrSettings,
        _selection: &ProviderSelection,
    ) -> Result<(), AsrErrorCode> {
        Ok(())
    }
}

struct ImportModelSelection {
    model_id: String,
    executable: bool,
    reason_code: Option<String>,
    provider: AsrProviderKind,
}

struct ImportModelLookup {
    selected_model_id: String,
    provider: AsrProviderKind,
    supported_languages: Vec<String>,
    model_executable: bool,
    vad_executable: bool,
}

impl ModelLookup for ImportModelLookup {
    fn lookup(&self, model_id: &str) -> Option<ModelCapabilities> {
        if model_id == self.selected_model_id {
            return Some(ModelCapabilities::new(
                self.provider,
                self.supported_languages.iter().map(String::as_str),
                true,
                self.model_executable,
                self.model_executable,
            ));
        }
        if model_id == DEFAULT_VAD_MODEL_ID {
            return Some(ModelCapabilities::new(
                AsrProviderKind::SenseVoice,
                ["auto"],
                true,
                self.vad_executable,
                self.vad_executable,
            ));
        }
        None
    }
}

fn provider_name(provider: crate::domain::AsrProviderKind) -> &'static str {
    match provider {
        crate::domain::AsrProviderKind::SenseVoice => "sense_voice",
        crate::domain::AsrProviderKind::Whisper => "whisper",
        crate::domain::AsrProviderKind::Qwen3Asr => "qwen3_asr",
    }
}

fn qualification_name(policy: QualificationPolicy) -> &'static str {
    match policy {
        QualificationPolicy::StructuralWithPinnedRuntime => "structural_with_pinned_runtime",
        QualificationPolicy::RuntimeSmokeRequired => "runtime_smoke_required",
    }
}

fn runtime_descriptor(runtime: RuntimeRequirement) -> (&'static str, &'static str) {
    match runtime {
        RuntimeRequirement::SherpaOnnx { crate_version, .. } => ("sherpa_onnx", crate_version),
        RuntimeRequirement::QwenCandleMetal { crate_version, .. } => {
            ("qwen_candle_metal", crate_version)
        }
    }
}

fn device_support(
    manifest: &crate::asr::manifest::ModelManifest,
    device: &DeviceProfile,
) -> DeviceSupport {
    match manifest.device {
        crate::asr::manifest::DeviceRequirement::AnyDesktop => DeviceSupport::Compatible,
        crate::asr::manifest::DeviceRequirement::AppleSiliconMetal {
            minimum_macos_major,
            minimum_memory_gib,
        } => {
            if device.os == "macos"
                && device.arch == "aarch64"
                && device.metal_available
                && device.macos_major >= minimum_macos_major
                && device.memory_gib >= minimum_memory_gib
            {
                DeviceSupport::Compatible
            } else {
                DeviceSupport::Unsupported
            }
        }
    }
}

fn map_timeline_chunk(chunk: TimelineChunk, data_dir: &std::path::Path) -> TimelineChunkPayload {
    TimelineChunkPayload {
        id: chunk.chunk.id,
        source: source_name(chunk.chunk.source).to_owned(),
        audio_path: data_dir
            .join(chunk.chunk.path)
            .to_string_lossy()
            .into_owned(),
        integrity_state: chunk_integrity_name(chunk.integrity_state).to_owned(),
        error_code: chunk.error_code,
    }
}

fn map_timeline_job(job: TimelineJobSummary) -> TimelineJobSummaryPayload {
    TimelineJobSummaryPayload {
        id: job.id,
        chunk_id: job.chunk_id,
        state: job.state,
        error_code: job.error_code,
        error_summary: job.error_summary,
    }
}

fn resolve_import_model(
    state: &AppState,
    config: &crate::domain::AsrConfig,
) -> Result<ImportModelSelection, String> {
    let provider = parse_provider_kind(&config.provider)?;
    let chosen = state
        .list_asr_models()?
        .into_iter()
        .find(|model| model.model_id == config.model_id)
        .ok_or_else(|| {
            format!(
                "configured ASR model {} is not in the manifest",
                config.model_id
            )
        })?;
    if chosen.provider != provider_name(provider) {
        return Err(format!(
            "configured ASR model {} does not belong to provider {}",
            config.model_id, config.provider
        ));
    }
    Ok(ImportModelSelection {
        model_id: chosen.model_id,
        executable: chosen.executable,
        reason_code: chosen.reason_code,
        provider,
    })
}

fn asr_settings_from_config(
    config: &crate::domain::AsrConfig,
    model_id: &str,
) -> Result<AsrSettings, String> {
    let provider = parse_provider_kind(&config.provider)?;
    let language =
        AsrLanguage::new(config.language.clone()).map_err(|_| "invalid ASR language".to_owned())?;
    let num_threads =
        u16::try_from(config.threads).map_err(|_| "invalid ASR thread count".to_owned())?;
    let options = match provider {
        AsrProviderKind::SenseVoice => AsrProviderOptions::SenseVoice {
            use_itn: config.itn_enabled,
        },
        AsrProviderKind::Whisper => AsrProviderOptions::Whisper {
            task: WhisperTask::Transcribe,
        },
        AsrProviderKind::Qwen3Asr => AsrProviderOptions::Qwen3Asr,
    };
    Ok(AsrSettings {
        provider,
        model_id: model_id.to_owned(),
        language,
        num_threads,
        vad_enabled: config.vad_enabled,
        auto_transcribe_imports: config.auto_transcribe,
        options,
    })
}

fn provider_selection_from_settings(settings: &AsrSettings) -> ProviderSelection {
    let options = match &settings.options {
        AsrProviderOptions::SenseVoice { use_itn } => {
            ProviderOptions::SenseVoice { use_itn: *use_itn }
        }
        AsrProviderOptions::Whisper { task } => ProviderOptions::Whisper { task: *task },
        AsrProviderOptions::Qwen3Asr => ProviderOptions::Qwen3Asr,
    };
    ProviderSelection::new(settings.language.as_str(), settings.num_threads, options)
}

fn parse_provider_kind(value: &str) -> Result<AsrProviderKind, String> {
    match value {
        "sense_voice" => Ok(AsrProviderKind::SenseVoice),
        "whisper" => Ok(AsrProviderKind::Whisper),
        "qwen3_asr" => Ok(AsrProviderKind::Qwen3Asr),
        _ => Err(format!("unknown provider {value}")),
    }
}

fn asr_error_name(code: AsrErrorCode) -> &'static str {
    match code {
        AsrErrorCode::ModelNotInstalled => "model_not_installed",
        AsrErrorCode::ModelCapabilityUnavailable => "model_capability_unavailable",
        AsrErrorCode::ModelDownloadFailed => "model_download_failed",
        AsrErrorCode::ModelIntegrityFailed => "model_integrity_failed",
        AsrErrorCode::InsufficientDiskSpace => "insufficient_disk_space",
        AsrErrorCode::UnsupportedOrCorruptAudio => "unsupported_or_corrupt_audio",
        AsrErrorCode::InputIntegrityFailed => "input_integrity_failed",
        AsrErrorCode::InputUnavailable => "input_unavailable",
        AsrErrorCode::InvalidProviderParameter => "invalid_provider_parameter",
        AsrErrorCode::ProviderInitializationFailed => "provider_initialization_failed",
        AsrErrorCode::TranscriptionFailed => "transcription_failed",
        AsrErrorCode::Cancelled => "cancelled",
        AsrErrorCode::RecoveryRequired => "recovery_required",
        AsrErrorCode::RecoveryRetryExhausted => "recovery_retry_exhausted",
        AsrErrorCode::ReceiptInvalid => "receipt_invalid",
    }
}

fn chunk_integrity_name(state: ChunkIntegrityState) -> &'static str {
    match state {
        ChunkIntegrityState::Available => "available",
        ChunkIntegrityState::Corrupted => "corrupted",
        ChunkIntegrityState::Missing => "missing",
    }
}

fn source_name(source: crate::domain::AudioSource) -> &'static str {
    match source {
        crate::domain::AudioSource::Microphone => "microphone",
        crate::domain::AudioSource::SystemAudio => "system_audio",
        crate::domain::AudioSource::Imported => "imported",
    }
}

fn default_vad_is_executable(catalog: &Catalog) -> Result<bool, String> {
    let installation = catalog
        .model_installation_records()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|record| record.model_id == DEFAULT_VAD_MODEL_ID);
    Ok(installation.is_some_and(|record| {
        record.manifest_version == vad_manifest().manifest_version
            && record.bundle_identity == vad_manifest().bundle.identity_sha256
            && record.state == "runtime_qualified"
    }))
}

fn import_required_file_hashes(
    manifest: &crate::asr::manifest::ModelManifest,
) -> Result<String, String> {
    use crate::asr::manifest::InstallConstraints;
    let required_files = match manifest.bundle.install_constraints {
        InstallConstraints::Archive(constraints) => constraints.required_files,
        InstallConstraints::Direct(constraints) => constraints.required_files,
    };
    let mut required_files = required_files.iter().collect::<Vec<_>>();
    required_files.sort_by(|left, right| left.path.as_bytes().cmp(right.path.as_bytes()));
    let value = serde_json::Value::Array(
        required_files
            .into_iter()
            .map(|file| {
                serde_json::json!({
                    "path": file.path,
                    "bytes": file.bytes,
                    "sha256": file.sha256,
                })
            })
            .collect(),
    );
    serde_json_canonicalizer::to_string(&value).map_err(|error| error.to_string())
}

fn import_vad_required_file_hashes() -> Result<String, String> {
    use crate::asr::manifest::InstallConstraints;
    let bundle = &vad_manifest().bundle;
    let required_files = match bundle.install_constraints {
        InstallConstraints::Archive(constraints) => constraints.required_files,
        InstallConstraints::Direct(constraints) => constraints.required_files,
    };
    let value = serde_json::Value::Array(
        required_files
            .iter()
            .map(|file| {
                serde_json::json!({
                    "path": file.path,
                    "bytes": file.bytes,
                    "sha256": file.sha256,
                })
            })
            .collect(),
    );
    serde_json_canonicalizer::to_string(&value).map_err(|error| error.to_string())
}

fn import_model_source(manifest: &crate::asr::manifest::ModelManifest) -> Result<String, String> {
    let bundle: serde_json::Value = serde_json::from_str(
        &crate::asr::manifest::canonical_bundle_payload(manifest)
            .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let mut source = serde_json::json!({
        "bundle": bundle,
        "repository_url": manifest.source.repository_url,
        "model_card_url": manifest.source.model_card_url,
        "license_spdx": manifest.source.license_spdx,
        "provenance": manifest.source.provenance,
    });
    let canonical =
        serde_json_canonicalizer::to_string(&source).map_err(|error| error.to_string())?;
    source["source_contract_sha256"] =
        serde_json::Value::String(hex::encode(sha2::Sha256::digest(canonical.as_bytes())));
    serde_json::to_string(&source).map_err(|error| error.to_string())
}

fn import_fingerprint(
    session_id: &str,
    chunk_id: &str,
    input_sha256: &str,
    model_id: &str,
    bundle_identity: &str,
    settings: &AsrSettings,
) -> Result<String, String> {
    let parameters_json = serde_json::to_string(settings).map_err(|error| error.to_string())?;
    let payload = format!(
        "{session_id}\0{chunk_id}\0{input_sha256}\0{model_id}\0{bundle_identity}\0{parameters_json}\0{}",
        if settings.vad_enabled {
            DEFAULT_VAD_MODEL_ID
        } else {
            ""
        }
    );
    Ok(hex::encode(sha2::Sha256::digest(payload.as_bytes())))
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
pub fn import_audio_record(
    path: String,
    title: String,
    state: State<'_, AppState>,
) -> Result<ImportAudioOutcomePayload, String> {
    state.import_audio_record(path, title)
}

#[tauri::command]
pub fn create_manual_revision(
    session_id: String,
    segments: Vec<TranscriptSegment>,
    state: State<'_, AppState>,
) -> Result<TranscriptRevision, String> {
    state.create_manual_revision(session_id, segments)
}

#[tauri::command]
pub fn list_timeline_records(
    state: State<'_, AppState>,
) -> Result<Vec<TimelineRecordPayload>, String> {
    state.list_timeline_records()
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
pub fn list_notes(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<CaptureNote>, String> {
    state.list_notes(session_id)
}

#[tauri::command]
pub fn update_note(
    note_id: String,
    content: String,
    tag: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.update_note(note_id, content, tag)
}

#[tauri::command]
pub fn delete_note(note_id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.delete_note(note_id)
}

#[tauri::command]
pub fn create_category(
    name: String,
    scope: String,
    state: State<'_, AppState>,
) -> Result<DictionaryCategory, String> {
    state.create_category(name, scope)
}

#[tauri::command]
pub fn list_categories(
    scope: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<DictionaryCategory>, String> {
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
pub fn toggle_entry(
    entry_id: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
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
pub fn rename_voiceprint(
    voiceprint_id: String,
    name: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
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
pub fn get_stats_snapshot(
    date: Option<String>,
    state: State<'_, AppState>,
) -> Result<StatsSnapshot, String> {
    state.get_stats_snapshot(date)
}

#[tauri::command]
pub fn get_asr_config(state: State<'_, AppState>) -> Result<crate::domain::AsrConfig, String> {
    state.get_asr_config()
}

#[tauri::command]
pub fn set_asr_config(
    config: crate::domain::AsrConfig,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.set_asr_config(config)
}

#[tauri::command]
pub fn get_recording_config(
    state: State<'_, AppState>,
) -> Result<crate::domain::RecordingConfig, String> {
    state.get_recording_config()
}

#[tauri::command]
pub fn set_recording_config(
    config: crate::domain::RecordingConfig,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.set_recording_config(config)
}

#[tauri::command]
pub fn get_app_runtime_info(app: AppHandle) -> Result<AppRuntimeInfo, String> {
    let sherpa = crate::asr::pinned_sherpa_runtime_identity();
    Ok(AppRuntimeInfo {
        app_version: app.package_info().version.to_string(),
        tauri_version: tauri::VERSION.to_owned(),
        frontend_stack: "React 19 + TypeScript".to_owned(),
        asr_runtime: format!("sherpa-onnx {}", sherpa.version),
    })
}

#[tauri::command]
pub fn list_asr_models(state: State<'_, AppState>) -> Result<Vec<AsrModelPayload>, String> {
    state.list_asr_models()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::{Arc, Barrier};

    use tempfile::tempdir;

    use super::*;
    use crate::asr::manifest::model_registry;
    use crate::asr::model_manager::{ModelCatalog, ModelInstallPlan, StoredInstallation};
    use crate::domain::{AudioSource, TranscriptSegment};

    #[test]
    fn import_persists_failed_job_when_configured_model_is_invalid() {
        let parent = tempdir().unwrap();
        let data_dir = parent.path().join("data");
        let source = parent.path().join("input.wav");
        fs::write(&source, b"durable audio").unwrap();
        let state = AppState::initialize_at(data_dir).unwrap();
        state
            .runtime
            .catalog_ref()
            .set_setting(
                "asr_config",
                r#"{"provider":"whisper","model_id":"retired-model","language":"auto","auto_transcribe":true,"threads":4,"vad_enabled":true,"vad_min_speech_ms":300,"vad_silence_ms":800,"itn_enabled":false}"#,
            )
            .unwrap();

        let outcome = state
            .import_audio_record(
                source.to_string_lossy().into_owned(),
                "invalid config".into(),
            )
            .unwrap();

        assert_eq!(
            outcome.job.as_ref().map(|job| job.state.as_str()),
            Some("failed")
        );
        assert_eq!(
            outcome
                .job
                .as_ref()
                .and_then(|job| job.error_code.as_deref()),
            Some("invalid_provider_parameter")
        );
        assert!(outcome.asr_warning.is_some());
        let durable = state
            .runtime
            .catalog_ref()
            .latest_job_for_session(&outcome.session.id)
            .unwrap()
            .unwrap();
        assert_eq!(durable.state, "failed");
        assert_eq!(
            durable.error_code.as_deref(),
            Some("invalid_provider_parameter")
        );
    }

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
        let state = AppState {
            runtime: Arc::new(runtime),
            data_dir,
            streaming: Mutex::new(StreamingCapture::default()),
            worker_shutdown: Arc::new(AtomicBool::new(false)),
            worker_handle: Mutex::new(None),
            quick_input: QuickInput::default(),
        };
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

    #[test]
    fn list_timeline_records_returns_resolved_audio_and_revision_history() {
        let parent = tempdir().unwrap();
        let data_dir = parent.path().join("data");
        let state = AppState::initialize_at(data_dir.clone()).unwrap();
        let session = state.create_capture_session("真实导入样本".into()).unwrap();
        let source = parent.path().join("sample.wav");
        fs::write(&source, b"audio").unwrap();

        let chunk = state
            .import_audio_file(session.clone(), source.to_string_lossy().into_owned())
            .unwrap();
        state
            .append_transcript_revision(
                session.id.clone(),
                "sense_voice".into(),
                vec![TranscriptSegment::new(
                    0,
                    1200,
                    AudioSource::Imported,
                    "真实转写",
                )],
            )
            .unwrap();
        state
            .create_note(
                session.id.clone(),
                "确认回放".into(),
                100,
                "待办".into(),
                None,
            )
            .unwrap();

        let records = state.list_timeline_records().unwrap();

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].session.id, session.id);
        assert_eq!(records[0].notes.len(), 1);
        assert_eq!(records[0].revisions.len(), 1);
        assert_eq!(records[0].revisions[0].segments[0].text, "真实转写");
        assert_eq!(
            records[0].chunks[0].audio_path,
            data_dir.join(chunk.path).to_string_lossy()
        );
    }

    #[test]
    fn list_asr_models_ignores_stale_installations_and_downloads() {
        let parent = tempdir().unwrap();
        let data_dir = parent.path().join("data");
        let state = AppState::initialize_at(data_dir).unwrap();
        let manifest = model_registry()
            .model("sense-voice-small-int8-2024-07-17")
            .unwrap();
        let plan = ModelInstallPlan::from_manifest(manifest);

        state
            .runtime
            .catalog_ref()
            .publish_installation(&StoredInstallation {
                model_id: manifest.id.to_owned(),
                provider: provider_name(manifest.provider).to_owned(),
                manifest_version: "stale-version".to_owned(),
                bundle_identity: "stale-bundle".to_owned(),
                install_dir: parent.path().join("stale-install"),
                state: "runtime_qualified".to_owned(),
                runtime_identity_json: Some("{}".to_owned()),
            })
            .unwrap();

        let stale_download = state
            .runtime
            .catalog_ref()
            .begin_download(&ModelInstallPlan {
                manifest_version: "stale-version".to_owned(),
                bundle_identity: "stale-bundle".to_owned(),
                ..plan.clone()
            })
            .unwrap();
        state
            .runtime
            .catalog_ref()
            .set_download_state(&stale_download, "failed", Some("stale_bundle"))
            .unwrap();

        let current_download = state.runtime.catalog_ref().begin_download(&plan).unwrap();
        state
            .runtime
            .catalog_ref()
            .set_download_state(&current_download, "verifying", None)
            .unwrap();

        let models = state.list_asr_models().unwrap();
        let model = models
            .into_iter()
            .find(|item| item.model_id == manifest.id)
            .unwrap();

        assert_eq!(model.installation_state, "not_installed");
        assert_eq!(
            model
                .download
                .as_ref()
                .map(|download| download.state.as_str()),
            Some("verifying")
        );
        assert!(!model.executable);
    }

    #[test]
    fn list_asr_models_uses_latest_current_bundle_download() {
        let parent = tempdir().unwrap();
        let data_dir = parent.path().join("data");
        let state = AppState::initialize_at(data_dir).unwrap();
        let manifest = model_registry()
            .model("sense-voice-small-int8-2024-07-17")
            .unwrap();
        let plan = ModelInstallPlan::from_manifest(manifest);

        let first = state.runtime.catalog_ref().begin_download(&plan).unwrap();
        state
            .runtime
            .catalog_ref()
            .set_download_state(&first, "cancelled", Some("manual_cancel"))
            .unwrap();

        let second = state.runtime.catalog_ref().begin_download(&plan).unwrap();
        state
            .runtime
            .catalog_ref()
            .set_download_state(&second, "installing", None)
            .unwrap();

        let models = state.list_asr_models().unwrap();
        let model = models
            .into_iter()
            .find(|item| item.model_id == manifest.id)
            .unwrap();

        assert_eq!(
            model
                .download
                .as_ref()
                .map(|download| download.state.as_str()),
            Some("installing")
        );
    }
}

// ── Phase 2.1: Streaming capture ─────────────────────────────────────────

#[tauri::command]
pub fn start_streaming_capture(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let mut streaming = state.streaming.lock().unwrap_or_else(|e| e.into_inner());
    if streaming.is_running() {
        return Err("streaming capture already running".into());
    }
    streaming.start(app);
    Ok(())
}

#[tauri::command]
pub fn stop_streaming_capture(state: State<'_, AppState>) -> Result<(), String> {
    let mut streaming = state.streaming.lock().unwrap_or_else(|e| e.into_inner());
    streaming.stop();
    Ok(())
}

#[tauri::command]
pub fn pause_streaming_capture(state: State<'_, AppState>) -> Result<(), String> {
    let streaming = state.streaming.lock().unwrap_or_else(|e| e.into_inner());
    if !streaming.is_running() {
        return Err("streaming capture not running".into());
    }
    streaming.pause();
    Ok(())
}

#[tauri::command]
pub fn resume_streaming_capture(state: State<'_, AppState>) -> Result<(), String> {
    let streaming = state.streaming.lock().unwrap_or_else(|e| e.into_inner());
    if !streaming.is_running() {
        return Err("streaming capture not running".into());
    }
    streaming.resume();
    Ok(())
}

// ── Phase 3: LLM polish + quick input ────────────────────────────────────

#[derive(Clone, Debug, serde::Deserialize)]
pub struct PolishRequest {
    pub text: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub app_bundle_id: Option<String>,
    #[serde(default)]
    pub preserve_raw: bool,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct PolishResponse {
    pub original: String,
    pub polished: String,
    pub provider: String,
    pub model: String,
    pub fallback: Option<String>,
    pub error: Option<String>,
}

#[tauri::command]
pub fn llm_polish(request: PolishRequest) -> Result<PolishResponse, String> {
    let model = request.model.unwrap_or_else(|| "qwen2.5:0.5b".into());
    let context = crate::llm::PolishContext {
        app_bundle_id: request.app_bundle_id,
        preserve_raw: request.preserve_raw,
    };
    let result = crate::llm::polish(&request.text, &model, &context);
    Ok(PolishResponse {
        original: result.original,
        polished: result.polished,
        provider: result.provider,
        model: result.model,
        fallback: result.fallback,
        error: result.error,
    })
}

#[tauri::command]
pub fn register_quick_input_hotkey(app: AppHandle, hotkey: Option<String>) -> Result<(), String> {
    let key = hotkey.unwrap_or_else(|| "CommandOrControl+Shift+Space".into());
    crate::quick_input::register_hotkey(&app, &key)
}

#[tauri::command]
pub fn get_frontmost_app() -> Result<Option<String>, String> {
    Ok(crate::quick_input::get_frontmost_app())
}

#[tauri::command]
pub fn paste_text_at_cursor(text: String) -> Result<(), String> {
    crate::quick_input::paste_text_at_cursor(&text)
}
