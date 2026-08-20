use std::path::PathBuf;
use std::sync::Arc;

use tauri::{AppHandle, Manager, State};

use crate::asr::settings::AsrSettings;
use crate::catalog::Catalog;
use crate::desktop_api::{self, AsrJobInfo, ImportResult, ModelInfo};
use crate::domain::{
    AudioChunk, CaptureSession, CaptureState, TranscriptRevision, TranscriptSegment,
};
use crate::service::{EvidenceService, parse_evidence_uri};

pub struct AppState {
    pub catalog: Arc<Catalog>,
    pub data_dir: PathBuf,
    pub worker_ctx: Option<desktop_api::WorkerContext>,
}

#[derive(serde::Serialize)]
pub struct EvidenceResolution {
    pub kind: String,
    pub id: String,
    pub start_seconds: Option<i64>,
    pub end_seconds: Option<i64>,
    pub revision: Option<i64>,
}

impl AppState {
    pub fn initialize(app: &AppHandle) -> Result<Self, String> {
        let data_dir = app.path().app_data_dir().map_err(|error| error.to_string())?;
        std::fs::create_dir_all(&data_dir).map_err(|error| error.to_string())?;
        let catalog = Catalog::open(data_dir.join("lifesub.sqlite3")).map_err(|error| error.to_string())?;
        let catalog = Arc::new(catalog);
        let worker_ctx = desktop_api::WorkerContext::initialize(&data_dir).ok();
        Ok(Self { catalog, data_dir, worker_ctx })
    }
}

#[tauri::command]
pub fn create_capture_session(title: String, state: State<'_, AppState>) -> Result<CaptureSession, String> {
    let session = CaptureSession::new(title);
    state.catalog.insert_session(&session).map_err(|error| error.to_string())?;
    Ok(session)
}

#[tauri::command]
pub fn transition_capture_session(session: CaptureSession, target: CaptureState, state: State<'_, AppState>) -> Result<CaptureSession, String> {
    let session = session.transition(target).map_err(|error| format!("{error:?}"))?;
    state.catalog.update_session(&session).map_err(|error| error.to_string())?;
    Ok(session)
}

#[tauri::command]
pub fn import_audio_file(session: CaptureSession, path: String, state: State<'_, AppState>) -> Result<ImportResult, String> {
    let service = EvidenceService::new(
        Catalog::open(state.data_dir.join("lifesub.sqlite3")).map_err(|error| error.to_string())?,
        &state.data_dir,
    );
    let chunk = service.import_audio(&session, path).map_err(|error| format!("{error:?}"))?;
    Ok(ImportResult { chunk, job: None })
}

#[tauri::command]
pub fn append_transcript_revision(session_id: String, provider: String, segments: Vec<TranscriptSegment>, state: State<'_, AppState>) -> Result<TranscriptRevision, String> {
    state.catalog.append_revision(&session_id, &provider, segments).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn search_transcripts(query: String, state: State<'_, AppState>) -> Result<Vec<TranscriptSegment>, String> {
    state.catalog.search_segments(&query).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn resolve_evidence(uri: String) -> Result<EvidenceResolution, String> {
    use crate::service::EvidenceTarget;
    match parse_evidence_uri(&uri).map_err(|error| format!("{error:?}"))? {
        EvidenceTarget::Record { id } => Ok(EvidenceResolution { kind: "record".into(), id, start_seconds: None, end_seconds: None, revision: None }),
        EvidenceTarget::Segment { id, revision } => Ok(EvidenceResolution { kind: "segment".into(), id, start_seconds: None, end_seconds: None, revision }),
        EvidenceTarget::Audio { id, start_seconds, end_seconds } => Ok(EvidenceResolution { kind: "audio".into(), id, start_seconds, end_seconds, revision: None }),
    }
}

#[tauri::command]
pub fn get_asr_settings(state: State<'_, AppState>) -> Result<AsrSettings, String> {
    desktop_api::get_asr_settings(&state.catalog)
}

#[tauri::command]
pub fn save_asr_settings(settings: AsrSettings, state: State<'_, AppState>) -> Result<(), String> {
    desktop_api::save_asr_settings(&state.catalog, &settings)
}

#[tauri::command]
pub fn list_asr_models(state: State<'_, AppState>) -> Result<Vec<ModelInfo>, String> {
    desktop_api::list_asr_models(&state.catalog)
}

#[tauri::command]
pub fn download_asr_model(model_id: String, state: State<'_, AppState>) -> Result<String, String> {
    desktop_api::download_asr_model(&state.catalog, &model_id)
}

#[tauri::command]
pub fn cancel_model_download(download_id: String, state: State<'_, AppState>) -> Result<(), String> {
    desktop_api::cancel_model_download(&state.catalog, &download_id)
}

#[tauri::command]
pub fn delete_asr_model(model_id: String, state: State<'_, AppState>) -> Result<(), String> {
    desktop_api::delete_asr_model(&state.catalog, &model_id)
}

#[tauri::command]
pub fn list_asr_jobs(state: State<'_, AppState>) -> Result<Vec<AsrJobInfo>, String> {
    desktop_api::list_asr_jobs(&state.catalog)
}

#[tauri::command]
pub fn cancel_asr_job(job_id: String, state: State<'_, AppState>) -> Result<(), String> {
    desktop_api::cancel_asr_job(&state.catalog, &job_id)
}

#[tauri::command]
pub fn retry_asr_job(job_id: String, state: State<'_, AppState>) -> Result<AsrJobInfo, String> {
    desktop_api::retry_asr_job(&state.catalog, &job_id)
}

#[tauri::command]
pub fn retranscribe_record(session_id: String, chunk_id: String, settings: AsrSettings, state: State<'_, AppState>) -> Result<String, String> {
    desktop_api::retranscribe_record(&state.catalog, &session_id, &chunk_id, &settings)
}
