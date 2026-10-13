use sha2::Digest;
use std::collections::HashMap;
use std::io::Write;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::asr::job::canonical_time;
use crate::asr::manifest::{QualificationPolicy, RuntimeRequirement, model_registry, vad_manifest};
use crate::asr::model_lookup::{
    DeviceSupport, InstallationQualification, ModelCapabilities, ModelLookup, ModelLookupContext,
};
use crate::asr::model_manager::DeviceProfile;
use crate::asr::provider::{ProviderOptions, ProviderSelection};
use crate::asr::service::{AsrEnqueueRequest, DEFAULT_VAD_MODEL_ID, EnqueueProviderFactory};
use crate::asr::settings::{AsrProviderOptions, AsrSettings, WhisperTask};
use crate::capture::helper::{
    MicrophoneDiscoveryError, list_microphones_one_shot_with_cancel,
    permission_state_one_shot_with_cancel, probe_microphone_one_shot_with_cancel,
};
use crate::capture::protocol::{
    MicrophoneDevice, MicrophoneProbeMessage, MicrophoneProbeResult, MicrophoneTransport,
    PermissionState, PermissionStateMessage,
};
use crate::capture::session::NativeCaptureSession;
use crate::catalog::{Catalog, TimelineChunk, TimelineJobSummary};
use crate::desktop_runtime::{DesktopRuntimeFactory, ProductionDesktopRuntimeFactory};
use crate::diagnostics::{
    DiagnosticError, DiagnosticIdentities, DiagnosticMicrophoneProbe, DiagnosticModel,
    DiagnosticPermissions, DiagnosticSelectedMicrophone, DiagnosticsSnapshot, sha256_file,
    sha256_text, storage_snapshot,
};
use crate::domain::{
    AsrErrorCode, AsrLanguage, AsrProviderKind, AudioChunk, AudioSource, CaptureNote,
    CaptureSession, CaptureState, ChunkIntegrityState, DictionaryCategory, DictionaryEntry,
    ImportAsrDisposition, ImportModelReadiness, StatsSnapshot, TranscriptRevision,
    TranscriptSegment, Voiceprint,
};
use crate::quick_input::QuickInput;
use crate::service::{CoreRuntime, EvidenceService, parse_evidence_uri, render_record_markdown};

pub struct AppState {
    runtime: Arc<CoreRuntime>,
    data_dir: PathBuf,
    streaming: Mutex<NativeCaptureSession>,
    last_microphone_probe: Mutex<Option<MicrophoneProbeMessage>>,
    last_capture_error: Mutex<Option<DiagnosticError>>,
    model_operations: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    worker_shutdown: Arc<AtomicBool>,
    worker_handle: Mutex<Option<std::thread::JoinHandle<()>>>,
    pub quick_input: QuickInput,
}

pub(crate) enum StartupDisposition<T> {
    Ready(T),
    Exit { reason: String },
}

pub(crate) fn startup_disposition<T>(result: Result<T, String>) -> StartupDisposition<T> {
    match result {
        Ok(value) => StartupDisposition::Ready(value),
        Err(reason) => StartupDisposition::Exit { reason },
    }
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

#[derive(Clone, Debug, Serialize)]
pub struct CommittedSegmentPayload {
    pub segment_id: String,
    pub chunk_id: String,
    pub capture_source_id: Option<String>,
    pub start_ms: i64,
    pub end_ms: i64,
    pub source: String,
    pub text: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct CommittedPublicationPayload {
    pub revision_id: String,
    pub session_id: String,
    pub revision_number: i64,
    pub provider: String,
    pub created_at: String,
    pub segments: Vec<CommittedSegmentPayload>,
}

#[derive(Serialize)]
pub struct AsrSessionStatePayload {
    pub latest_revision: Option<CommittedPublicationPayload>,
    pub latest_job: Option<TimelineJobSummaryPayload>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MicrophoneDevicePayload {
    pub uid: String,
    pub name: String,
    pub is_default: bool,
    pub is_connected: bool,
    pub transport: String,
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
    pub id: String,
    pub state: String,
    pub downloaded_bytes: u64,
    pub expected_bytes: u64,
    pub artifact_index: usize,
    pub artifact_count: usize,
    pub error_code: Option<String>,
    pub error_summary: Option<String>,
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
        let runtime =
            Arc::new(CoreRuntime::initialize(&data_dir).map_err(|error| format!("{error:?}"))?);
        let publication_app = app.clone();
        let publication_runtime = runtime.clone();
        let publication = Arc::new(move |revision: &TranscriptRevision| {
            if let Ok(payload) =
                committed_publication_payload(publication_runtime.catalog_ref(), revision)
            {
                let _ = publication_app.emit("asr-publication-committed", payload);
            }
        });
        let failure_app = app.clone();
        let failure = Arc::new(move |event: &crate::asr::worker::DurableJobFailure| {
            let _ = failure_app.emit("asr-job-failed", event);
        });
        Self::from_runtime(
            data_dir,
            runtime,
            crate::asr::worker::WorkerCallbacks {
                publication: Some(publication),
                failure: Some(failure),
            },
        )
    }

    #[cfg(test)]
    fn initialize_at(data_dir: PathBuf) -> Result<Self, String> {
        let runtime =
            Arc::new(CoreRuntime::initialize(&data_dir).map_err(|error| format!("{error:?}"))?);
        Self::from_runtime(
            data_dir,
            runtime,
            crate::asr::worker::WorkerCallbacks::default(),
        )
    }

    fn from_runtime(
        data_dir: PathBuf,
        runtime: Arc<CoreRuntime>,
        callbacks: crate::asr::worker::WorkerCallbacks,
    ) -> Result<Self, String> {
        let (worker_shutdown, worker_handle) =
            ProductionDesktopRuntimeFactory::spawn_worker(runtime.clone(), callbacks);
        Ok(Self {
            runtime,
            data_dir,
            streaming: Mutex::new(NativeCaptureSession::new()),
            last_microphone_probe: Mutex::new(None),
            last_capture_error: Mutex::new(None),
            model_operations: Arc::new(Mutex::new(HashMap::new())),
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

    fn write_record_markdown_export(
        &self,
        path: &std::path::Path,
        session_id: &str,
        revision_number: i64,
    ) -> Result<(), String> {
        let markdown = self.with_current_catalog(|catalog| {
            let session = catalog
                .list_sessions()
                .map_err(|error| error.to_string())?
                .into_iter()
                .find(|session| session.id == session_id)
                .ok_or_else(|| "record export session not found".to_string())?;
            let revision = catalog
                .list_revisions_with_segments(session_id)
                .map_err(|error| error.to_string())?
                .into_iter()
                .find(|revision| revision.number == revision_number)
                .ok_or_else(|| "record export revision not found".to_string())?;
            Ok(render_record_markdown(&session, &revision))
        })?;
        write_private_export(path, markdown.as_bytes(), "record export")
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

    fn asr_session_state(&self, session_id: String) -> Result<AsrSessionStatePayload, String> {
        self.with_current_catalog(|catalog| {
            let latest_revision = catalog
                .list_revisions_with_segments(&session_id)
                .map_err(|error| error.to_string())?
                .last()
                .map(|revision| committed_publication_payload(catalog, revision))
                .transpose()?;
            let latest_job = catalog
                .latest_job_for_session(&session_id)
                .map_err(|error| error.to_string())?
                .map(map_timeline_job);
            Ok(AsrSessionStatePayload {
                latest_revision,
                latest_job,
            })
        })
    }

    fn enqueue_import_job(
        &self,
        session: &CaptureSession,
        chunk: &AudioChunk,
    ) -> Result<Option<TimelineJobSummaryPayload>, String> {
        Self::enqueue_auto_transcription(&self.runtime, &session.id, chunk)
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

    pub(crate) fn enqueue_auto_transcription(
        runtime: &CoreRuntime,
        session_id: &str,
        chunk: &AudioChunk,
    ) -> Result<Option<TimelineJobSummaryPayload>, String> {
        let config = load_asr_config(runtime)?;
        if !config.auto_transcribe {
            return Ok(None);
        }

        config.validate_for_persistence()?;
        let selection = resolve_import_model(runtime, &config)?;
        let settings = asr_settings_from_config(&config, &selection.model_id)?;
        let provider_selection = provider_selection_from_settings(&settings);
        let manifest = model_registry()
            .model(&selection.model_id)
            .ok_or_else(|| "selected model manifest missing".to_owned())?;
        let fingerprint = import_fingerprint(
            session_id,
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
            session_id: session_id.to_owned(),
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

        let vad_ready = !settings.vad_enabled || default_vad_is_executable(runtime.catalog_ref())?;

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
            let service = runtime.asr_service(&lookup, &NoopEnqueueProviderFactory);
            match service.enqueue(AsrEnqueueRequest {
                session_id: session_id.to_owned(),
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
                            runtime
                                .catalog_ref()
                                .insert_blocked_asr_job(&job, asr_error_name(code), &summary)
                                .map_err(|error| format!("{error:?}"))?;
                            ("blocked_model", summary)
                        }
                        ImportAsrDisposition::Failed(code) => {
                            let summary =
                                format!("auto-transcribe failed: {}", asr_error_name(code));
                            runtime
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
        runtime
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
        load_asr_config(&self.runtime)
    }

    fn set_asr_config(&self, config: crate::domain::AsrConfig) -> Result<(), String> {
        let normalized = config.normalized_for_persistence()?;
        normalized.validate_for_persistence()?;
        let json = serde_json::to_string(&normalized).map_err(|e| e.to_string())?;
        self.with_current_catalog(|catalog| {
            catalog
                .set_setting("asr_config", &json)
                .map_err(|e| e.to_string())
        })
    }

    fn get_settings_capabilities(&self) -> Result<crate::domain::SettingsCapabilities, String> {
        Ok(settings_capabilities_for(&self.data_dir))
    }

    fn get_recording_config(&self) -> Result<crate::domain::RecordingConfig, String> {
        let capabilities = settings_capabilities_for(&self.data_dir);
        self.with_current_catalog(|catalog| {
            let json = catalog
                .get_setting("recording_config")
                .map_err(|e| e.to_string())?
                .unwrap_or_else(|| {
                    serde_json::to_string(
                        &crate::domain::RecordingConfig::default_for_capabilities(&capabilities),
                    )
                    .unwrap()
                });
            let config: crate::domain::RecordingConfig =
                serde_json::from_str(&json).map_err(|e| e.to_string())?;
            Ok(config.normalized_for_capabilities(&capabilities))
        })
    }

    fn set_recording_config(&self, config: crate::domain::RecordingConfig) -> Result<(), String> {
        let capabilities = settings_capabilities_for(&self.data_dir);
        let normalized = config.normalized_for_capabilities(&capabilities);
        normalized.validate_for_persistence()?;
        let json = serde_json::to_string(&normalized).map_err(|e| e.to_string())?;
        self.with_current_catalog(|catalog| {
            catalog
                .set_setting("recording_config", &json)
                .map_err(|e| e.to_string())
        })
    }

    fn list_microphone_devices(
        &self,
        cancelled: &AtomicBool,
    ) -> Result<Vec<MicrophoneDevicePayload>, String> {
        let devices = list_microphones_one_shot_with_cancel(
            &self.data_dir,
            std::time::Duration::from_secs(5),
            cancelled,
        )
        .map_err(map_microphone_discovery_error)?;
        Ok(devices
            .into_iter()
            .map(MicrophoneDevicePayload::from)
            .collect())
    }

    fn probe_microphone(
        &self,
        microphone_device_uid: &str,
        cancelled: &AtomicBool,
    ) -> Result<MicrophoneProbeMessage, String> {
        let result = probe_microphone_one_shot_with_cancel(
            &self.data_dir,
            microphone_device_uid,
            std::time::Duration::from_secs(5),
            cancelled,
        )
        .map_err(map_microphone_discovery_error)?;
        *self
            .last_microphone_probe
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(result.clone());
        Ok(result)
    }

    fn diagnostics_snapshot(
        &self,
        app_version: String,
        app_executable: &std::path::Path,
        permissions: PermissionStateMessage,
        devices: Vec<MicrophoneDevice>,
    ) -> Result<DiagnosticsSnapshot, String> {
        let config = self.get_recording_config()?;
        let selected = config
            .microphone_device_uid
            .as_deref()
            .and_then(|uid| devices.iter().find(|device| device.uid == uid));
        let latest_probe = self
            .last_microphone_probe
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
            .filter(|probe| Some(probe.uid.as_str()) == config.microphone_device_uid.as_deref());

        let installations = self
            .runtime
            .catalog_ref()
            .model_installation_records()
            .map_err(|error| error.to_string())?;
        let installation_by_id = installations
            .into_iter()
            .map(|record| (record.model_id.clone(), record))
            .collect::<HashMap<_, _>>();
        let models = self
            .list_asr_models()?
            .into_iter()
            .map(|model| {
                let installation = installation_by_id.get(&model.model_id).filter(|record| {
                    record.manifest_version == model.manifest_version
                        && record.bundle_identity == model.bundle_identity
                });
                DiagnosticModel {
                    model_id: model.model_id,
                    state: model.installation_state,
                    bundle_identity: model.bundle_identity,
                    runtime_identity_sha256: installation
                        .and_then(|record| record.runtime_identity_json.as_deref())
                        .map(sha256_text),
                    last_error_code: model.last_error_code,
                }
            })
            .collect();
        let runtime = crate::asr::pinned_sherpa_runtime_identity();
        let helper_code_sha256 = option_env!("LIFESUB_CAPTURE_HELPER_CODE_SHA256")
            .ok_or_else(|| "capture helper build identity unavailable".to_string())?
            .to_string();

        let active_capture_error = self
            .streaming
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .recent_error()
            .map(|error| DiagnosticError {
                code: error.code,
                summary: error.message,
            });
        let recent_error = active_capture_error.or_else(|| {
            self.last_capture_error
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .clone()
        });

        Ok(DiagnosticsSnapshot::new(
            DiagnosticPermissions {
                microphone: permission_name(permissions.microphone).to_string(),
                screen_recording: permission_name(permissions.screen_recording).to_string(),
            },
            DiagnosticSelectedMicrophone {
                uid: config.microphone_device_uid,
                name: config.microphone_device_name,
                connected: selected.is_some(),
                transport: selected
                    .map(|device| microphone_transport_name(device.transport).to_string()),
                probe_result: latest_probe
                    .as_ref()
                    .map(|probe| probe_result_name(probe.result).to_string()),
                probe: latest_probe.map(|probe| DiagnosticMicrophoneProbe {
                    uid: probe.uid,
                    name: probe.name,
                    transport: microphone_transport_name(probe.transport).to_string(),
                    input_sample_rate_hz: probe.input_sample_rate_hz,
                    input_channel_count: probe.input_channel_count,
                    read_back_uid: probe.read_back_uid,
                    first_frame_sample_count: probe.first_frame_sample_count,
                    elapsed_ms: probe.elapsed_ms,
                    result: probe_result_name(probe.result).to_string(),
                }),
            },
            DiagnosticIdentities {
                app_version,
                app_executable_sha256: sha256_file(app_executable)?,
                helper_code_sha256,
                capture_protocol_version: crate::capture::protocol::PROTOCOL_VERSION,
                runtime_version: runtime.version.to_string(),
                runtime_git_commit: runtime.git_commit.to_string(),
                runtime_archive_sha256: runtime.native_archive_sha256.to_string(),
                runtime_build_id: runtime.build_id.to_string(),
            },
            models,
            storage_snapshot(&self.data_dir)?,
            recent_error,
        ))
    }

    fn record_capture_error(&self, code: &str, summary: String) {
        *self
            .last_capture_error
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(DiagnosticError {
            code: code.to_string(),
            summary,
        });
    }

    fn request_model_cancellation(&self, download_id: String) -> Result<ModelOperationAck, String> {
        let record = self
            .runtime
            .catalog_ref()
            .model_download_records()
            .map_err(|error| error.to_string())?
            .into_iter()
            .find(|record| record.id == download_id)
            .ok_or_else(|| "model download not found".to_string())?;
        validate_current_download_identity(&record)?;
        if matches!(record.state.as_str(), "succeeded" | "failed" | "cancelled") {
            return Ok(ModelOperationAck {
                download_id,
                cancel_requested: false,
            });
        }
        let operation = self
            .model_operations
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(&record.model_id)
            .cloned()
            .ok_or_else(|| "model operation is not active".to_string())?;
        operation.store(true, Ordering::Release);
        Ok(ModelOperationAck {
            download_id,
            cancel_requested: true,
        })
    }

    fn retry_install_ids(&self, model_id: &str, download_id: &str) -> Result<[String; 2], String> {
        let install_ids = model_install_ids(model_id)?;
        let records = self
            .runtime
            .catalog_ref()
            .model_download_records()
            .map_err(|error| error.to_string())?;
        let record = records
            .iter()
            .find(|record| record.id == download_id)
            .ok_or_else(|| "model download not found".to_string())?;
        validate_current_download_identity(record)?;
        if !install_ids.iter().any(|id| id == &record.model_id)
            || !matches!(record.state.as_str(), "failed" | "cancelled")
        {
            return Err("model download is not retryable for this model".to_string());
        }
        let latest_retryable = records.iter().find(|candidate| {
            candidate.model_id == record.model_id
                && candidate.manifest_version == record.manifest_version
                && candidate.bundle_identity == record.bundle_identity
                && matches!(candidate.state.as_str(), "failed" | "cancelled")
        });
        if latest_retryable.map(|candidate| candidate.id.as_str()) != Some(download_id) {
            return Err("model download was replaced by a newer operation".to_string());
        }
        Ok(install_ids)
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
                let vad = vad_manifest();
                let vad_ready = installations.get(vad.id).is_some_and(|record| {
                    record.manifest_version == vad.manifest_version
                        && record.bundle_identity == vad.bundle.identity_sha256
                        && record.state == "runtime_qualified"
                });
                let target_download = downloads.iter().find(|record| {
                    record.model_id == manifest.id
                        && record.manifest_version == manifest.manifest_version
                        && record.bundle_identity == manifest.bundle.identity_sha256
                });
                let dependency_download = (!vad_ready)
                    .then(|| {
                        downloads.iter().find(|record| {
                            record.model_id == vad.id
                                && record.manifest_version == vad.manifest_version
                                && record.bundle_identity == vad.bundle.identity_sha256
                                && record.state != "succeeded"
                        })
                    })
                    .flatten();
                let visible_download = dependency_download.or(target_download);

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
                    download: visible_download.map(|record| ModelDownloadPayload {
                        id: record.id.clone(),
                        state: if record.state == "succeeded"
                            && installation_state == "installed_unqualified"
                        {
                            "qualifying".to_string()
                        } else {
                            record.state.clone()
                        },
                        downloaded_bytes: record.downloaded_bytes,
                        expected_bytes: record.expected_bytes,
                        artifact_index: if record.artifact_count == 0 {
                            0
                        } else if record.state == "succeeded" {
                            record.artifact_count
                        } else {
                            (record.verified_artifact_count + 1).min(record.artifact_count)
                        },
                        artifact_count: record.artifact_count,
                        error_code: record.error_code.clone(),
                        error_summary: record.error_summary.clone(),
                    }),
                }
            })
            .collect())
    }
}

fn load_asr_config(runtime: &CoreRuntime) -> Result<crate::domain::AsrConfig, String> {
    runtime
        .ensure_current()
        .map_err(|error| format!("{error:?}"))?;
    let json = runtime
        .catalog_ref()
        .get_setting("asr_config")
        .map_err(|error| error.to_string())?
        .unwrap_or_else(|| {
            serde_json::to_string(
                &crate::domain::AsrConfig::default_for_provider("sense_voice")
                    .expect("default provider must be available"),
            )
            .expect("default ASR config must serialize")
        });
    let config: crate::domain::AsrConfig =
        serde_json::from_str(&json).map_err(|error| error.to_string())?;
    Ok(config.normalized_for_projection())
}

fn model_install_ids(model_id: &str) -> Result<[String; 2], String> {
    let model = model_registry()
        .model(model_id)
        .ok_or_else(|| format!("unknown ASR model {model_id}"))?;
    Ok([
        crate::asr::service::DEFAULT_VAD_MODEL_ID.to_owned(),
        model.id.to_owned(),
    ])
}

fn settings_capabilities_for(data_dir: &std::path::Path) -> crate::domain::SettingsCapabilities {
    crate::domain::SettingsCapabilities::for_data_directory(data_dir.to_string_lossy().into_owned())
}

fn parse_whisper_task(task: Option<&str>) -> Result<WhisperTask, String> {
    match task.unwrap_or("transcribe") {
        "transcribe" => Ok(WhisperTask::Transcribe),
        "translate" => Ok(WhisperTask::Translate),
        other => Err(format!("unsupported whisper task {other}")),
    }
}

fn committed_publication_payload(
    catalog: &Catalog,
    revision: &TranscriptRevision,
) -> Result<CommittedPublicationPayload, String> {
    let segments = revision
        .segments
        .iter()
        .map(|segment| {
            let chunk_id = segment
                .chunk_id
                .clone()
                .ok_or_else(|| "committed ASR segment is missing chunk identity".to_owned())?;
            let capture_source_id = catalog
                .capture_chunk_timing(&chunk_id)
                .map_err(|error| error.to_string())?
                .map(|timing| timing.capture_source_id);
            Ok(CommittedSegmentPayload {
                segment_id: segment.id.clone(),
                chunk_id,
                capture_source_id,
                start_ms: segment.start_ms,
                end_ms: segment.end_ms,
                source: match segment.source {
                    AudioSource::Microphone => "microphone",
                    AudioSource::SystemAudio => "system_audio",
                    AudioSource::Imported => "imported",
                }
                .to_owned(),
                text: segment.text.clone(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(CommittedPublicationPayload {
        revision_id: revision.id.clone(),
        session_id: revision.session_id.clone(),
        revision_number: revision.number,
        provider: revision.provider.clone(),
        created_at: revision.created_at.to_rfc3339(),
        segments,
    })
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
    runtime: &CoreRuntime,
    config: &crate::domain::AsrConfig,
) -> Result<ImportModelSelection, String> {
    let provider = parse_provider_kind(&config.provider)?;
    let manifest = model_registry().model(&config.model_id).ok_or_else(|| {
        format!(
            "configured ASR model {} is not in the manifest",
            config.model_id
        )
    })?;
    if manifest.provider != provider {
        return Err(format!(
            "configured ASR model {} does not belong to provider {}",
            config.model_id, config.provider
        ));
    }
    let installation = runtime
        .catalog_ref()
        .model_installation_records()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|record| {
            record.model_id == manifest.id
                && record.manifest_version == manifest.manifest_version
                && record.bundle_identity == manifest.bundle.identity_sha256
        });
    let qualification = match installation.as_ref().map(|record| record.state.as_str()) {
        Some("runtime_qualified") => InstallationQualification::RuntimeQualified,
        Some("installed_unqualified") => InstallationQualification::InstalledUnqualified,
        _ => InstallationQualification::NotInstalled,
    };
    let capabilities = model_registry()
        .lookup_with_context(
            manifest.id,
            ModelLookupContext::new(
                device_support(manifest, &DeviceProfile::current()),
                qualification,
            ),
        )
        .ok_or_else(|| "selected model manifest lookup failed".to_owned())?;
    Ok(ImportModelSelection {
        model_id: manifest.id.to_owned(),
        executable: capabilities.executable,
        reason_code: capabilities.reason_code,
        provider,
    })
}

fn asr_settings_from_config(
    config: &crate::domain::AsrConfig,
    model_id: &str,
) -> Result<AsrSettings, String> {
    let normalized = config.normalized_for_persistence()?;
    let provider = parse_provider_kind(&normalized.provider)?;
    let language = AsrLanguage::new(normalized.language.clone())
        .map_err(|_| "invalid ASR language".to_owned())?;
    let num_threads =
        u16::try_from(normalized.threads).map_err(|_| "invalid ASR thread count".to_owned())?;
    let options = match provider {
        AsrProviderKind::SenseVoice => AsrProviderOptions::SenseVoice {
            use_itn: normalized.itn_enabled,
        },
        AsrProviderKind::Whisper => AsrProviderOptions::Whisper {
            task: parse_whisper_task(normalized.whisper_task.as_deref())?,
        },
        AsrProviderKind::Qwen3Asr => AsrProviderOptions::Qwen3Asr,
    };
    Ok(AsrSettings {
        provider,
        model_id: model_id.to_owned(),
        language,
        num_threads,
        vad_enabled: normalized.vad_enabled,
        auto_transcribe_imports: normalized.auto_transcribe,
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
pub fn get_asr_session_state(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<AsrSessionStatePayload, String> {
    state.asr_session_state(session_id)
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
pub fn get_settings_capabilities(
    state: State<'_, AppState>,
) -> Result<crate::domain::SettingsCapabilities, String> {
    state.get_settings_capabilities()
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
pub async fn list_microphone_devices(
    app: AppHandle,
) -> Result<Vec<MicrophoneDevicePayload>, String> {
    let cancelled = Arc::new(AtomicBool::new(false));
    let worker_cancelled = cancelled.clone();
    let mut cancellation_guard = MicrophoneDiscoveryCancellationGuard::new(cancelled);
    let result = tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        state.list_microphone_devices(&worker_cancelled)
    })
    .await;
    cancellation_guard.disarm();
    result.map_err(|error| format!("microphone device listing task failed: {error}"))?
}

#[tauri::command]
pub async fn probe_microphone(
    microphone_device_uid: String,
    app: AppHandle,
) -> Result<MicrophoneProbeMessage, String> {
    let cancelled = Arc::new(AtomicBool::new(false));
    let worker_cancelled = cancelled.clone();
    let mut cancellation_guard = MicrophoneDiscoveryCancellationGuard::new(cancelled);
    let result = tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        state.probe_microphone(&microphone_device_uid, &worker_cancelled)
    })
    .await;
    cancellation_guard.disarm();
    result.map_err(|error| format!("microphone probe task failed: {error}"))?
}

#[tauri::command]
pub async fn get_diagnostics(app: AppHandle) -> Result<DiagnosticsSnapshot, String> {
    let cancelled = Arc::new(AtomicBool::new(false));
    let worker_cancelled = cancelled.clone();
    let mut cancellation_guard = MicrophoneDiscoveryCancellationGuard::new(cancelled);
    let result = tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let permissions = permission_state_one_shot_with_cancel(
            &state.data_dir,
            std::time::Duration::from_secs(5),
            &worker_cancelled,
        )
        .map_err(map_microphone_discovery_error)?;
        let devices = list_microphones_one_shot_with_cancel(
            &state.data_dir,
            std::time::Duration::from_secs(5),
            &worker_cancelled,
        )
        .map_err(map_microphone_discovery_error)?;
        let executable = std::env::current_exe().map_err(|error| error.to_string())?;
        state.diagnostics_snapshot(
            app.package_info().version.to_string(),
            &executable,
            permissions,
            devices,
        )
    })
    .await;
    cancellation_guard.disarm();
    result.map_err(|error| format!("diagnostics task failed: {error}"))?
}

#[tauri::command]
pub fn write_diagnostics_export(path: String, snapshot: DiagnosticsSnapshot) -> Result<(), String> {
    let path = std::path::Path::new(&path);
    let json = snapshot.to_redacted_pretty_json()?;
    write_private_export(path, json.as_bytes(), "diagnostics export")
}

#[tauri::command]
pub fn write_record_markdown_export(
    path: String,
    session_id: String,
    revision_number: i64,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.write_record_markdown_export(std::path::Path::new(&path), &session_id, revision_number)
}

fn write_private_export(
    path: &std::path::Path,
    contents: &[u8],
    export_name: &str,
) -> Result<(), String> {
    write_private_export_with_before_replace(path, contents, export_name, || Ok(()))
}

fn write_private_export_with_before_replace(
    path: &std::path::Path,
    contents: &[u8],
    export_name: &str,
    before_replace: impl FnOnce() -> std::io::Result<()>,
) -> Result<(), String> {
    if !path.is_absolute() {
        return Err(format!("{export_name} path must be absolute"));
    }
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.file_type().is_file() || metadata.nlink() != 1 => {
            return Err(format!(
                "{export_name} target must be a single-link regular file"
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.to_string()),
    }
    let parent = path
        .parent()
        .ok_or_else(|| format!("{export_name} path has no parent"))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| format!("{export_name} path has no safe filename"))?;
    let temporary_path = parent.join(format!(
        ".{file_name}.lifesub-{}.tmp",
        uuid::Uuid::new_v4().simple()
    ));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&temporary_path)
            .map_err(|error| error.to_string())?;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))
            .map_err(|error| error.to_string())?;
        file.write_all(contents)
            .map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        before_replace().map_err(|error| error.to_string())?;
        std::fs::rename(&temporary_path, path).map_err(|error| error.to_string())?;
        let parent_directory = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_CLOEXEC)
            .open(parent)
            .map_err(|error| error.to_string())?;
        parent_directory
            .sync_all()
            .map_err(|error| error.to_string())?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary_path);
    }
    result
}

#[tauri::command]
pub fn open_macos_privacy_settings(target: String) -> Result<(), String> {
    let url = match target.as_str() {
        "microphone" => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"
        }
        "screen_recording" => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"
        }
        _ => return Err("unsupported macOS privacy settings target".to_string()),
    };
    let status = std::process::Command::new("/usr/bin/open")
        .arg(url)
        .status()
        .map_err(|error| error.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("failed to open macOS privacy settings".to_string())
    }
}

struct MicrophoneDiscoveryCancellationGuard {
    cancelled: Arc<AtomicBool>,
    armed: bool,
}

impl MicrophoneDiscoveryCancellationGuard {
    fn new(cancelled: Arc<AtomicBool>) -> Self {
        Self {
            cancelled,
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for MicrophoneDiscoveryCancellationGuard {
    fn drop(&mut self) {
        if self.armed {
            self.cancelled.store(true, Ordering::Release);
        }
    }
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ModelOperationAck {
    pub download_id: String,
    pub cancel_requested: bool,
}

struct ModelOperationRegistration {
    registry: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    model_ids: Vec<String>,
    cancellation: Arc<AtomicBool>,
}

impl Drop for ModelOperationRegistration {
    fn drop(&mut self) {
        let mut registry = self
            .registry
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        for model_id in &self.model_ids {
            if registry
                .get(model_id)
                .is_some_and(|value| Arc::ptr_eq(value, &self.cancellation))
            {
                registry.remove(model_id);
            }
        }
    }
}

fn register_model_operation(
    registry: &Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    model_ids: &[String],
) -> Result<(Arc<AtomicBool>, ModelOperationRegistration), String> {
    let cancellation = Arc::new(AtomicBool::new(false));
    let mut operations = registry.lock().unwrap_or_else(|error| error.into_inner());
    if model_ids
        .iter()
        .any(|model_id| operations.contains_key(model_id))
    {
        return Err("model operation already active".to_string());
    }
    for model_id in model_ids {
        operations.insert(model_id.clone(), cancellation.clone());
    }
    drop(operations);
    Ok((
        cancellation.clone(),
        ModelOperationRegistration {
            registry: registry.clone(),
            model_ids: model_ids.to_vec(),
            cancellation,
        },
    ))
}

async fn execute_model_install(
    runtime: Arc<CoreRuntime>,
    registry: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    install_ids: [String; 2],
) -> Result<(), String> {
    let model_ids = install_ids.to_vec();
    let (cancellation, registration) = register_model_operation(&registry, &model_ids)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _registration = registration;
        let device = DeviceProfile::current();
        for install_id in install_ids {
            runtime
                .ensure_model_installed(&install_id, &device, || {
                    cancellation.load(Ordering::Acquire)
                })
                .map_err(|error| error.to_string())?;
        }
        Ok::<(), String>(())
    })
    .await
    .map_err(|error| format!("model install task failed: {error}"))?
}

#[tauri::command]
pub async fn install_asr_model(
    model_id: String,
    state: State<'_, AppState>,
) -> Result<AsrModelPayload, String> {
    let install_ids = model_install_ids(&model_id)?;
    execute_model_install(
        state.runtime.clone(),
        state.model_operations.clone(),
        install_ids,
    )
    .await?;

    state
        .list_asr_models()?
        .into_iter()
        .find(|model| model.model_id == model_id)
        .ok_or_else(|| "installed model is missing from the current manifest".to_owned())
}

#[tauri::command]
pub fn cancel_model_download(
    download_id: String,
    state: State<'_, AppState>,
) -> Result<ModelOperationAck, String> {
    state.request_model_cancellation(download_id)
}

#[tauri::command]
pub async fn retry_model_download(
    model_id: String,
    download_id: String,
    state: State<'_, AppState>,
) -> Result<AsrModelPayload, String> {
    let install_ids = state.retry_install_ids(&model_id, &download_id)?;
    execute_model_install(
        state.runtime.clone(),
        state.model_operations.clone(),
        install_ids,
    )
    .await?;
    state
        .list_asr_models()?
        .into_iter()
        .find(|model| model.model_id == model_id)
        .ok_or_else(|| "installed model is missing from the current manifest".to_string())
}

fn validate_current_download_identity(
    record: &crate::catalog::ModelDownloadRecord,
) -> Result<(), String> {
    let current = if record.model_id == DEFAULT_VAD_MODEL_ID {
        let manifest = vad_manifest();
        Some((manifest.manifest_version, manifest.bundle.identity_sha256))
    } else {
        model_registry()
            .model(&record.model_id)
            .map(|manifest| (manifest.manifest_version, manifest.bundle.identity_sha256))
    }
    .ok_or_else(|| "model download manifest is no longer current".to_string())?;
    if record.manifest_version == current.0 && record.bundle_identity == current.1 {
        Ok(())
    } else {
        Err("model download identity is stale".to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::sync::{Arc, Barrier};
    use std::time::Duration;

    use tempfile::tempdir;

    use super::*;
    use crate::asr::manifest::model_registry;
    use crate::asr::model_manager::{
        ArtifactCheckpoint, ModelCatalog, ModelInstallPlan, StoredInstallation,
    };
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
            streaming: Mutex::new(NativeCaptureSession::new()),
            last_microphone_probe: Mutex::new(None),
            last_capture_error: Mutex::new(None),
            model_operations: Arc::new(Mutex::new(HashMap::new())),
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
        let download = model.download.unwrap();
        assert_eq!(download.id, second);
        assert_eq!(download.downloaded_bytes, 0);
        assert_eq!(
            download.expected_bytes,
            plan.artifacts
                .iter()
                .map(|artifact| artifact.expected_bytes)
                .sum::<u64>()
        );
        assert_eq!(download.artifact_index, 1);
        assert_eq!(download.artifact_count, plan.artifacts.len());
        assert_eq!(download.error_code, None);
        assert_eq!(download.error_summary, None);
    }

    #[test]
    fn model_cancel_requests_active_operation_without_synthesizing_catalog_state() {
        let parent = tempdir().unwrap();
        let state = AppState::initialize_at(parent.path().join("data")).unwrap();
        let manifest = model_registry()
            .model("sense-voice-small-int8-2024-07-17")
            .unwrap();
        let plan = ModelInstallPlan::from_manifest(manifest);
        let download_id = state.runtime.catalog_ref().begin_download(&plan).unwrap();
        state
            .runtime
            .catalog_ref()
            .set_download_state(&download_id, "downloading", None)
            .unwrap();
        let cancellation = Arc::new(AtomicBool::new(false));
        state
            .model_operations
            .lock()
            .unwrap()
            .insert(manifest.id.to_string(), cancellation.clone());

        let ack = state
            .request_model_cancellation(download_id.clone())
            .unwrap();

        assert_eq!(
            ack,
            ModelOperationAck {
                download_id: download_id.clone(),
                cancel_requested: true,
            }
        );
        assert!(cancellation.load(Ordering::Acquire));
        let persisted = state
            .runtime
            .catalog_ref()
            .model_download_records()
            .unwrap()
            .into_iter()
            .find(|record| record.id == download_id)
            .unwrap();
        assert_eq!(persisted.state, "downloading");
    }

    #[test]
    fn model_retry_preserves_exact_latest_download_identity() {
        let parent = tempdir().unwrap();
        let state = AppState::initialize_at(parent.path().join("data")).unwrap();
        let manifest = model_registry()
            .model("sense-voice-small-int8-2024-07-17")
            .unwrap();
        let plan = ModelInstallPlan::from_manifest(manifest);
        let first = state.runtime.catalog_ref().begin_download(&plan).unwrap();
        state
            .runtime
            .catalog_ref()
            .set_download_state(&first, "failed", Some("network"))
            .unwrap();
        std::thread::sleep(Duration::from_millis(2));
        let latest = state.runtime.catalog_ref().begin_download(&plan).unwrap();
        state
            .runtime
            .catalog_ref()
            .set_download_state(&latest, "cancelled", Some("manual_cancel"))
            .unwrap();

        assert!(state.retry_install_ids(manifest.id, &first).is_err());
        assert_eq!(
            state.retry_install_ids(manifest.id, &latest).unwrap(),
            model_install_ids(manifest.id).unwrap()
        );
    }

    #[test]
    fn model_catalog_projects_active_vad_dependency_on_the_requested_asr_model() {
        let parent = tempdir().unwrap();
        let state = AppState::initialize_at(parent.path().join("data")).unwrap();
        let vad = vad_manifest();
        let vad_plan = ModelInstallPlan::from_vad_manifest(vad);
        let download_id = state
            .runtime
            .catalog_ref()
            .begin_download(&vad_plan)
            .unwrap();
        state
            .runtime
            .catalog_ref()
            .set_download_state(&download_id, "downloading", None)
            .unwrap();

        let model = state
            .list_asr_models()
            .unwrap()
            .into_iter()
            .find(|model| model.model_id == "sense-voice-small-int8-2024-07-17")
            .unwrap();

        assert_eq!(
            model.download.as_ref().map(|download| download.id.as_str()),
            Some(download_id.as_str())
        );
        assert_eq!(
            model
                .download
                .as_ref()
                .map(|download| download.state.as_str()),
            Some("downloading")
        );
    }

    #[test]
    fn model_catalog_projects_durable_checkpoint_bytes_artifact_and_error() {
        let parent = tempdir().unwrap();
        let state = AppState::initialize_at(parent.path().join("data")).unwrap();
        let manifest = model_registry().model("whisper-base").unwrap();
        let plan = ModelInstallPlan::from_manifest(manifest);
        let download_id = state.runtime.catalog_ref().begin_download(&plan).unwrap();
        let artifact = &plan.artifacts[0];
        let checkpoint_path = parent.path().join("download.part");
        std::fs::write(&checkpoint_path, vec![0_u8; 17]).unwrap();
        state
            .runtime
            .catalog_ref()
            .save_checkpoint(
                &download_id,
                &ArtifactCheckpoint {
                    artifact_id: artifact.artifact_id.clone(),
                    source_identity: format!(
                        "{}\n{}\n{}\n{}\n{}\n{}",
                        artifact.source_repository,
                        artifact.source_model,
                        artifact.revision,
                        artifact.url,
                        artifact.expected_sha256,
                        artifact.required_path,
                    ),
                    downloaded_bytes: 17,
                    expected_bytes: artifact.expected_bytes,
                    temp_path: checkpoint_path,
                    etag: None,
                    last_modified: None,
                    verified_sha256: None,
                    state: "downloading".to_string(),
                },
            )
            .unwrap();
        state
            .runtime
            .catalog_ref()
            .set_download_state(&download_id, "downloading", None)
            .unwrap();

        let active = state
            .list_asr_models()
            .unwrap()
            .into_iter()
            .find(|model| model.model_id == manifest.id)
            .unwrap()
            .download
            .unwrap();
        assert_eq!(active.downloaded_bytes, 17);
        assert_eq!(active.artifact_index, 1);
        assert_eq!(active.artifact_count, plan.artifacts.len());

        state
            .runtime
            .catalog_ref()
            .set_download_state(&download_id, "failed", Some("model_download_failed"))
            .unwrap();
        let failed = state
            .list_asr_models()
            .unwrap()
            .into_iter()
            .find(|model| model.model_id == manifest.id)
            .unwrap()
            .download
            .unwrap();
        assert_eq!(failed.error_code.as_deref(), Some("model_download_failed"));
        assert_eq!(
            failed.error_summary.as_deref(),
            Some("model_download_failed")
        );
    }

    #[test]
    fn recording_config_persists_selected_microphone_pair_through_catalog_settings() {
        let parent = tempdir().unwrap();
        let data_dir = parent.path().join("data");
        let state = AppState::initialize_at(data_dir.clone()).unwrap();

        let default_config = state.get_recording_config().unwrap();
        assert_eq!(default_config.microphone_device_uid, None);
        assert_eq!(default_config.microphone_device_name, None);

        state
            .set_recording_config(crate::domain::RecordingConfig {
                capture_mode: "smart".to_string(),
                im_detection_enabled: true,
                im_apps: vec!["wechat".to_string(), "zoom".to_string()],
                detection_delay_secs: 3,
                recovery_delay_secs: 5,
                sample_rate: 16_000,
                storage_path: "~/.lifesub/recordings/".to_string(),
                microphone_device_uid: Some("mic-uid-1".to_string()),
                microphone_device_name: Some("Built-in Microphone".to_string()),
            })
            .unwrap();

        assert_eq!(
            state.get_recording_config().unwrap(),
            crate::domain::RecordingConfig {
                capture_mode: "smart".to_string(),
                im_detection_enabled: false,
                im_apps: Vec::new(),
                detection_delay_secs: 3,
                recovery_delay_secs: 5,
                sample_rate: 16_000,
                storage_path: data_dir.to_string_lossy().into_owned(),
                microphone_device_uid: Some("mic-uid-1".to_string()),
                microphone_device_name: Some("Built-in Microphone".to_string()),
            }
        );
    }

    #[test]
    fn recording_config_projects_truthful_production_defaults_instead_of_legacy_active_values() {
        let parent = tempdir().unwrap();
        let data_dir = parent.path().join("data");
        let state = AppState::initialize_at(data_dir.clone()).unwrap();

        state
            .runtime
            .catalog_ref()
            .set_setting(
                "recording_config",
                r#"{
                    "capture_mode":"smart",
                    "im_detection_enabled":true,
                    "im_apps":["wechat","qq"],
                    "detection_delay_secs":3,
                    "recovery_delay_secs":5,
                    "sample_rate":48000,
                    "storage_path":"/tmp/legacy-recordings"
                }"#,
            )
            .unwrap();

        let config = state.get_recording_config().unwrap();

        assert_eq!(config.sample_rate, 16_000);
        assert_eq!(config.storage_path, data_dir.to_string_lossy());
        assert!(!config.im_detection_enabled);
    }

    #[test]
    fn settings_capabilities_report_truthful_capture_constraints() {
        let parent = tempdir().unwrap();
        let data_dir = parent.path().join("data");
        let state = AppState::initialize_at(data_dir.clone()).unwrap();

        let capabilities = state.get_settings_capabilities().unwrap();
        let json = serde_json::to_value(capabilities).unwrap();

        assert_eq!(json["production_capture_output_sample_rate_hz"], 16_000);
        assert_eq!(
            json["data_directory_path"],
            data_dir.to_string_lossy().as_ref()
        );
        assert_eq!(json["im_detection_available"], false);
        assert_eq!(
            json["supported_capture_modes"],
            serde_json::json!(["smart", "mic_only", "system_only"])
        );
    }

    #[test]
    fn diagnostics_snapshot_projects_current_runtime_model_device_and_error_facts() {
        let parent = tempdir().unwrap();
        let data_dir = parent.path().join("data");
        let state = AppState::initialize_at(data_dir.clone()).unwrap();
        let mut config = state.get_recording_config().unwrap();
        config.microphone_device_uid = Some("mic-1".to_string());
        config.microphone_device_name = Some("Built-in Microphone".to_string());
        state.set_recording_config(config).unwrap();
        *state.last_microphone_probe.lock().unwrap() = Some(MicrophoneProbeMessage {
            uid: "mic-1".to_string(),
            name: "Built-in Microphone".to_string(),
            transport: MicrophoneTransport::BuiltIn,
            input_sample_rate_hz: Some(44_100),
            input_channel_count: Some(1),
            read_back_uid: Some("mic-1".to_string()),
            first_frame_sample_count: 128,
            elapsed_ms: 12,
            result: MicrophoneProbeResult::Compatible,
        });
        state.record_capture_error(
            "selected_microphone_disconnected",
            "所选麦克风已断开".to_string(),
        );
        let executable = parent.path().join("lifesub-test");
        std::fs::write(&executable, b"test executable").unwrap();

        let snapshot = state
            .diagnostics_snapshot(
                "0.2.1-test".to_string(),
                &executable,
                PermissionStateMessage {
                    microphone: PermissionState::Granted,
                    screen_recording: PermissionState::NotGranted,
                },
                vec![MicrophoneDevice {
                    uid: "mic-1".to_string(),
                    name: "Built-in Microphone".to_string(),
                    is_default: true,
                    is_connected: true,
                    transport: MicrophoneTransport::BuiltIn,
                }],
            )
            .unwrap();

        assert_eq!(snapshot.permissions.microphone, "granted");
        assert_eq!(snapshot.permissions.screen_recording, "not_granted");
        assert_eq!(snapshot.selected_microphone.uid.as_deref(), Some("mic-1"));
        assert_eq!(
            snapshot.selected_microphone.probe_result.as_deref(),
            Some("compatible")
        );
        let diagnostic_probe = snapshot.selected_microphone.probe.as_ref().unwrap();
        assert_eq!(diagnostic_probe.uid, "mic-1");
        assert_eq!(diagnostic_probe.read_back_uid.as_deref(), Some("mic-1"));
        assert_eq!(diagnostic_probe.first_frame_sample_count, 128);
        assert_eq!(diagnostic_probe.elapsed_ms, 12);
        assert_eq!(snapshot.identities.app_version, "0.2.1-test");
        assert_eq!(snapshot.identities.app_executable_sha256.len(), 64);
        assert!(
            snapshot
                .models
                .iter()
                .any(|model| model.model_id == "sense-voice-small-int8-2024-07-17")
        );
        assert_eq!(
            snapshot
                .recent_error
                .as_ref()
                .map(|error| error.code.as_str()),
            Some("selected_microphone_disconnected")
        );
        snapshot.to_redacted_pretty_json().unwrap();
    }

    #[test]
    #[ignore = "run explicitly for the 0.2.2 high-file-count diagnostics baseline"]
    fn diagnostics_snapshot_high_file_count_baseline() {
        use std::time::Instant;

        const FILES_PER_CATEGORY: usize = 2_000;
        const FILE_BYTES: usize = 16;

        let parent = tempdir().unwrap();
        let data_dir = parent.path().join("data");
        let state = AppState::initialize_at(data_dir.clone()).unwrap();
        for category in ["audio", "models"] {
            let directory = data_dir.join(category);
            std::fs::create_dir_all(&directory).unwrap();
            for index in 0..FILES_PER_CATEGORY {
                std::fs::write(
                    directory.join(format!("baseline-{index:05}.bin")),
                    [0_u8; FILE_BYTES],
                )
                .unwrap();
            }
        }
        let executable = parent.path().join("lifesub-test");
        std::fs::write(&executable, b"test executable").unwrap();
        let permissions = PermissionStateMessage {
            microphone: PermissionState::Granted,
            screen_recording: PermissionState::Granted,
        };
        for run in ["cold", "warm"] {
            let started = Instant::now();
            let snapshot = state
                .diagnostics_snapshot(
                    "0.2.2-baseline".to_string(),
                    &executable,
                    permissions.clone(),
                    Vec::new(),
                )
                .unwrap();
            let elapsed_ms = started.elapsed().as_secs_f64() * 1_000.0;
            assert_eq!(
                snapshot.storage.audio_bytes,
                (FILES_PER_CATEGORY * FILE_BYTES) as u64
            );
            assert_eq!(
                snapshot.storage.model_bytes,
                (FILES_PER_CATEGORY * FILE_BYTES) as u64
            );
            println!(
                "diagnostics_baseline run={run} files={} elapsed_ms={elapsed_ms:.3} complete=true",
                FILES_PER_CATEGORY * 2
            );
        }
    }

    #[test]
    fn diagnostics_export_rejects_relative_symlink_and_unknown_privacy_targets() {
        let snapshot = DiagnosticsSnapshot::new(
            DiagnosticPermissions {
                microphone: "granted".to_string(),
                screen_recording: "not_granted".to_string(),
            },
            DiagnosticSelectedMicrophone {
                uid: None,
                name: None,
                connected: false,
                transport: None,
                probe_result: None,
                probe: None,
            },
            DiagnosticIdentities {
                app_version: "0.2.1".to_string(),
                app_executable_sha256: "a".repeat(64),
                helper_code_sha256: "b".repeat(64),
                capture_protocol_version: 2,
                runtime_version: "1.13.5".to_string(),
                runtime_git_commit: "c".repeat(40),
                runtime_archive_sha256: "d".repeat(64),
                runtime_build_id: "build".to_string(),
            },
            Vec::new(),
            crate::diagnostics::DiagnosticStorage {
                data_directory: "/tmp/lifesub".to_string(),
                catalog_bytes: 0,
                audio_bytes: 0,
                model_bytes: 0,
            },
            None,
        );
        assert!(write_diagnostics_export("relative.json".to_string(), snapshot.clone()).is_err());

        let parent = tempdir().unwrap();
        let target = parent.path().join("target.json");
        let link = parent.path().join("link.json");
        std::fs::write(&target, b"unchanged").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(write_diagnostics_export(link.to_string_lossy().into_owned(), snapshot).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"unchanged");
        assert!(open_macos_privacy_settings("camera".to_string()).is_err());
    }

    #[test]
    fn record_markdown_export_uses_exact_catalog_revision_and_rejects_symlinks() {
        let parent = tempdir().unwrap();
        let state = AppState::initialize_at(parent.path().join("data")).unwrap();
        let session = CaptureSession::new("真实导出");
        state
            .runtime
            .catalog_ref()
            .insert_session(&session)
            .unwrap();
        let revision = state
            .runtime
            .catalog_ref()
            .append_revision(
                &session.id,
                "sense_voice",
                vec![TranscriptSegment::new(
                    1_200,
                    4_400,
                    AudioSource::Microphone,
                    "持久化后才能提示成功",
                )],
            )
            .unwrap();
        let output = parent.path().join("record.md");

        state
            .write_record_markdown_export(&output, &session.id, revision.number)
            .unwrap();

        let markdown = std::fs::read_to_string(&output).unwrap();
        assert!(markdown.contains(&session.evidence_uri()));
        assert!(markdown.contains("transcript_revision: 1"));
        assert!(markdown.contains("持久化后才能提示成功"));
        assert!(
            state
                .write_record_markdown_export(&output, &session.id, 99)
                .is_err()
        );

        let target = parent.path().join("target.md");
        let link = parent.path().join("link.md");
        std::fs::write(&target, b"unchanged").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(
            state
                .write_record_markdown_export(&link, &session.id, revision.number)
                .is_err()
        );
        assert_eq!(std::fs::read(&target).unwrap(), b"unchanged");

        let hardlink_target = parent.path().join("hardlink-target.md");
        let hardlink = parent.path().join("hardlink.md");
        std::fs::write(&hardlink_target, b"hardlink unchanged").unwrap();
        std::fs::hard_link(&hardlink_target, &hardlink).unwrap();
        assert!(
            state
                .write_record_markdown_export(&hardlink, &session.id, revision.number)
                .is_err()
        );
        assert_eq!(
            std::fs::read(&hardlink_target).unwrap(),
            b"hardlink unchanged"
        );
    }

    #[test]
    fn write_record_markdown_export_preserves_existing_file_on_failure() {
        let parent = tempdir().unwrap();
        let output = parent.path().join("record.md");
        std::fs::write(&output, b"existing export").unwrap();

        let result = write_private_export_with_before_replace(
            &output,
            b"replacement export",
            "record export",
            || Err(std::io::Error::other("injected before replace")),
        );

        assert!(result.is_err());
        assert_eq!(std::fs::read(&output).unwrap(), b"existing export");
        assert_eq!(
            std::fs::read_dir(parent.path())
                .unwrap()
                .filter_map(Result::ok)
                .filter(|entry| entry.file_name().to_string_lossy().contains(".lifesub-"))
                .count(),
            0
        );
    }

    #[test]
    fn write_private_export_atomically_replaces_with_private_permissions() {
        let parent = tempdir().unwrap();
        let output = parent.path().join("record.md");
        std::fs::write(&output, b"existing export").unwrap();
        std::fs::set_permissions(&output, std::fs::Permissions::from_mode(0o644)).unwrap();

        write_private_export(&output, b"replacement export", "record export").unwrap();

        assert_eq!(std::fs::read(&output).unwrap(), b"replacement export");
        assert_eq!(
            std::fs::metadata(&output).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[test]
    fn microphone_device_payload_serializes_with_camel_case_fields() {
        let payload = MicrophoneDevicePayload::from(MicrophoneDevice {
            uid: "mic-uid-1".to_string(),
            name: "Built-in Microphone".to_string(),
            is_default: true,
            is_connected: true,
            transport: crate::capture::protocol::MicrophoneTransport::BuiltIn,
        });

        let json = serde_json::to_value(payload).unwrap();

        assert_eq!(json["uid"], "mic-uid-1");
        assert_eq!(json["name"], "Built-in Microphone");
        assert_eq!(json["isDefault"], true);
        assert_eq!(json["isConnected"], true);
        assert_eq!(json["transport"], "built_in");
        assert!(json.get("is_default").is_none());
        assert!(json.get("is_connected").is_none());
    }

    #[test]
    fn model_install_plan_includes_vad_before_the_requested_model() {
        assert_eq!(
            model_install_ids("sense-voice-small-int8-2024-07-17").unwrap(),
            ["silero-vad-2024-01-17", "sense-voice-small-int8-2024-07-17",]
        );
        assert!(model_install_ids("retired-model").is_err());
    }

    #[test]
    fn asr_settings_from_config_maps_the_whisper_task_and_rejects_unknown_values() {
        let translate_config: crate::domain::AsrConfig = serde_json::from_str(
            r#"{
                "provider":"whisper",
                "model_id":"whisper-base",
                "language":"auto",
                "auto_transcribe":true,
                "threads":4,
                "vad_enabled":true,
                "vad_min_speech_ms":300,
                "vad_silence_ms":800,
                "itn_enabled":false,
                "whisper_task":"translate"
            }"#,
        )
        .unwrap();

        let settings = asr_settings_from_config(&translate_config, "whisper-base").unwrap();
        assert_eq!(
            settings.options,
            AsrProviderOptions::Whisper {
                task: WhisperTask::Translate
            }
        );

        let invalid = serde_json::from_str::<crate::domain::AsrConfig>(
            r#"{
                "provider":"whisper",
                "model_id":"whisper-base",
                "language":"auto",
                "auto_transcribe":true,
                "threads":4,
                "vad_enabled":true,
                "vad_min_speech_ms":300,
                "vad_silence_ms":800,
                "itn_enabled":false,
                "whisper_task":"summarize"
            }"#,
        );
        assert!(invalid.is_err());
    }

    #[test]
    fn committed_publication_payload_includes_native_chunk_and_capture_source_identity() {
        let catalog = Catalog::in_memory().unwrap();
        let mut session = CaptureSession::new("native publication");
        session.state = CaptureState::Stopped;
        session.ended_at = Some(chrono::Utc::now());
        catalog.insert_session(&session).unwrap();
        let source = crate::domain::CaptureSourceRecord {
            id: "capture-source-mic".to_owned(),
            session_id: session.id.clone(),
            source: AudioSource::Microphone,
            device_uid: "device-uid".to_owned(),
            device_name: "Built-in Microphone".to_owned(),
            channel_count: 1,
            sample_rate_hz: 16_000,
            created_at: chrono::Utc::now(),
        };
        catalog.insert_capture_source(&source).unwrap();
        let chunk = AudioChunk {
            id: "chk_native_publication".to_owned(),
            session_id: session.id.clone(),
            source: AudioSource::Microphone,
            path: "audio/chk_native_publication.caf".to_owned(),
            sha256: "a".repeat(64),
            byte_length: 32_000,
        };
        let now = chrono::Utc::now();
        catalog
            .insert_native_capture_chunk(
                &chunk,
                &crate::domain::CaptureChunkTiming {
                    chunk_id: chunk.id.clone(),
                    capture_source_id: source.id.clone(),
                    source_sequence: 0,
                    source_sample_start: 0,
                    source_sample_end: 16_000,
                    session_start_ms: 0,
                    session_end_ms: 1_000,
                    monotonic_start_ns: 1,
                    monotonic_end_ns: 1_000_000_001,
                    utc_start_at: now,
                    utc_end_at: now + chrono::Duration::seconds(1),
                    discontinuity_count: 0,
                    dropped_frame_count: 0,
                    created_at: now,
                },
            )
            .unwrap();
        let revision = catalog
            .append_revision(
                &session.id,
                "sense_voice",
                vec![TranscriptSegment {
                    id: "segment-native".to_owned(),
                    start_ms: 0,
                    end_ms: 1_000,
                    source: AudioSource::Microphone,
                    text: "persisted native text".to_owned(),
                    chunk_id: Some(chunk.id.clone()),
                    chunk_start_ms: Some(0),
                    chunk_end_ms: Some(1_000),
                }],
            )
            .unwrap();

        let payload = committed_publication_payload(&catalog, &revision).unwrap();

        assert_eq!(payload.revision_id, revision.id);
        assert_eq!(payload.segments[0].chunk_id, chunk.id);
        assert_eq!(
            payload.segments[0].capture_source_id.as_deref(),
            Some("capture-source-mic")
        );
        assert_eq!(payload.segments[0].source, "microphone");
    }

    #[test]
    fn startup_failure_requests_clean_exit_instead_of_setup_error() {
        match startup_disposition::<()>(Err("Ownership(AlreadyOwned)".to_owned())) {
            StartupDisposition::Exit { reason } => {
                assert_eq!(reason, "Ownership(AlreadyOwned)");
            }
            StartupDisposition::Ready(()) => {
                panic!("startup failure must not enter the event loop")
            }
        }
    }

    #[test]
    #[ignore = "set LIFESUB_RUN_REAL_ASR_E2E=1 and LIFESUB_REAL_MODEL_DATA_DIR to run canonical import-to-publication inference"]
    fn real_sense_voice_import_persists_nonempty_revision_segments() {
        assert_eq!(
            std::env::var("LIFESUB_RUN_REAL_ASR_E2E").as_deref(),
            Ok("1"),
            "set LIFESUB_RUN_REAL_ASR_E2E=1 to run real inference"
        );
        let data_dir = std::env::var_os("LIFESUB_REAL_MODEL_DATA_DIR")
            .map(PathBuf::from)
            .expect("set LIFESUB_REAL_MODEL_DATA_DIR to the canonical LifeSub app data directory");
        let state = AppState::initialize_at(data_dir).unwrap();
        let (real_worker_shutdown, real_worker_handle) =
            crate::asr::worker::spawn_native_worker(state.runtime.clone());
        state
            .set_asr_config(crate::domain::AsrConfig {
                provider: "sense_voice".to_owned(),
                model_id: "sense-voice-small-int8-2024-07-17".to_owned(),
                language: "auto".to_owned(),
                auto_transcribe: true,
                threads: 4,
                vad_enabled: true,
                vad_min_speech_ms: 300,
                vad_silence_ms: 800,
                itn_enabled: true,
                whisper_task: None,
            })
            .unwrap();
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/asr/qwen06-codeswitch.wav");
        let pending_session_id = state
            .list_timeline_records()
            .unwrap()
            .into_iter()
            .filter(|record| {
                record.session.title == "Real SenseVoice E2E"
                    && record.latest_job.as_ref().is_some_and(|job| {
                        !matches!(job.state.as_str(), "succeeded" | "failed" | "cancelled")
                    })
            })
            .min_by(|left, right| left.session.started_at.cmp(&right.session.started_at))
            .map(|record| record.session.id);
        let session_id = match pending_session_id {
            Some(session_id) => session_id,
            None => {
                let outcome = state
                    .import_audio_record(
                        fixture.to_string_lossy().into_owned(),
                        "Real SenseVoice E2E".to_owned(),
                    )
                    .unwrap();
                assert!(outcome.asr_warning.is_none(), "{:?}", outcome.asr_warning);
                outcome.session.id
            }
        };

        let deadline = std::time::Instant::now() + Duration::from_secs(300);
        let completed = loop {
            let job = state
                .runtime
                .catalog_ref()
                .latest_job_for_session(&session_id)
                .unwrap()
                .unwrap();
            if matches!(job.state.as_str(), "succeeded" | "failed" | "cancelled") {
                break job;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "real ASR job timed out"
            );
            std::thread::sleep(Duration::from_millis(200));
        };
        assert_eq!(
            completed.state, "succeeded",
            "real ASR failed: {:?} {:?}",
            completed.error_code, completed.error_summary
        );

        let record = state
            .list_timeline_records()
            .unwrap()
            .into_iter()
            .find(|record| record.session.id == session_id)
            .unwrap();
        let revision = record.revisions.last().expect("ASR revision missing");
        assert!(!revision.segments.is_empty());
        assert!(
            revision
                .segments
                .iter()
                .all(|segment| !segment.text.trim().is_empty())
        );
        real_worker_shutdown.store(true, Ordering::Release);
        real_worker_handle.join().unwrap();
    }
}

// ── Phase 2.1: Streaming capture ─────────────────────────────────────────

#[tauri::command]
pub async fn start_streaming_capture(
    app: AppHandle,
    session_id: String,
    capture_mode: crate::capture::session::NativeCaptureMode,
    microphone_device_uid: Option<String>,
) -> Result<crate::capture::session::NativeCaptureStartStatus, String> {
    let blocking_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = blocking_app.state::<AppState>();
        if !state
            .runtime
            .catalog_ref()
            .session_exists(&session_id)
            .map_err(|error| error.to_string())?
        {
            return Err("capture session does not exist".into());
        }
        let mut streaming = state.streaming.lock().unwrap_or_else(|e| e.into_inner());
        if streaming.is_running() {
            return Err("streaming capture already running".into());
        }
        let result = streaming
            .start(
                blocking_app.clone(),
                state.runtime.clone(),
                state.data_dir.clone(),
                session_id,
                capture_mode,
                microphone_device_uid,
            )
            .map_err(|error| error.to_string());
        if let Err(error) = &result {
            state.record_capture_error("capture_start_failed", error.clone());
        }
        result
    })
    .await
    .map_err(|error| format!("capture start task failed: {error}"))?
}

impl From<MicrophoneDevice> for MicrophoneDevicePayload {
    fn from(value: MicrophoneDevice) -> Self {
        Self {
            uid: value.uid,
            name: value.name,
            is_default: value.is_default,
            is_connected: value.is_connected,
            transport: serde_json::to_string(&value.transport)
                .unwrap()
                .trim_matches('"')
                .to_owned(),
        }
    }
}

fn map_microphone_discovery_error(error: MicrophoneDiscoveryError) -> String {
    match error {
        MicrophoneDiscoveryError::Launch(launch_error) => {
            format!("capture helper launch failed: {launch_error:?}")
        }
        MicrophoneDiscoveryError::Cancelled => {
            "capture helper microphone request cancelled".to_string()
        }
        MicrophoneDiscoveryError::Timeout => {
            "capture helper microphone request timed out".to_string()
        }
        MicrophoneDiscoveryError::Fatal { code, summary } => format!("{code}: {summary}"),
        MicrophoneDiscoveryError::UnexpectedResponse => {
            "capture helper returned an unexpected microphone response".to_string()
        }
        MicrophoneDiscoveryError::MissingShutdownAck => {
            "capture helper did not acknowledge microphone shutdown".to_string()
        }
        MicrophoneDiscoveryError::Transport(summary) => {
            format!("capture helper transport failed: {summary}")
        }
    }
}

fn permission_name(permission: PermissionState) -> &'static str {
    match permission {
        PermissionState::NotDetermined => "not_determined",
        PermissionState::NotGranted => "not_granted",
        PermissionState::Denied => "denied",
        PermissionState::Restricted => "restricted",
        PermissionState::Granted => "granted",
    }
}

fn microphone_transport_name(transport: MicrophoneTransport) -> &'static str {
    match transport {
        MicrophoneTransport::BuiltIn => "built_in",
        MicrophoneTransport::Usb => "usb",
        MicrophoneTransport::Bluetooth => "bluetooth",
        MicrophoneTransport::Aggregate => "aggregate",
        MicrophoneTransport::Virtual => "virtual",
        MicrophoneTransport::Unknown => "unknown",
    }
}

fn probe_result_name(result: MicrophoneProbeResult) -> &'static str {
    match result {
        MicrophoneProbeResult::Compatible => "compatible",
        MicrophoneProbeResult::PermissionDenied => "permission_denied",
        MicrophoneProbeResult::Unavailable => "unavailable",
        MicrophoneProbeResult::IdentityMismatch => "identity_mismatch",
        MicrophoneProbeResult::InvalidFormat => "invalid_format",
        MicrophoneProbeResult::NoAudioFrames => "no_audio_frames",
        MicrophoneProbeResult::RouteConflict => "route_conflict",
        MicrophoneProbeResult::Timeout => "timeout",
    }
}

#[tauri::command]
pub async fn stop_streaming_capture(app: AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let mut streaming = state.streaming.lock().unwrap_or_else(|e| e.into_inner());
        let result = streaming.stop().map_err(|error| error.to_string());
        if let Err(error) = &result {
            state.record_capture_error("capture_stop_failed", error.clone());
        }
        result
    })
    .await
    .map_err(|error| format!("capture stop task failed: {error}"))?
}

#[tauri::command]
pub async fn pause_streaming_capture(app: AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let streaming = state.streaming.lock().unwrap_or_else(|e| e.into_inner());
        if !streaming.is_running() {
            return Err("streaming capture not running".into());
        }
        let result = streaming.pause().map_err(|error| error.to_string());
        if let Err(error) = &result {
            state.record_capture_error("capture_pause_failed", error.clone());
        }
        result
    })
    .await
    .map_err(|error| format!("capture pause task failed: {error}"))?
}

#[tauri::command]
pub async fn resume_streaming_capture(app: AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let streaming = state.streaming.lock().unwrap_or_else(|e| e.into_inner());
        if !streaming.is_running() {
            return Err("streaming capture not running".into());
        }
        let result = streaming.resume().map_err(|error| error.to_string());
        if let Err(error) = &result {
            state.record_capture_error("capture_resume_failed", error.clone());
        }
        result
    })
    .await
    .map_err(|error| format!("capture resume task failed: {error}"))?
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
