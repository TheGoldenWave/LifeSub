use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::catalog::Catalog;
use crate::domain::{AudioChunk, CaptureSession, CaptureState, TranscriptRevision, TranscriptSegment};
use crate::service::{parse_evidence_uri, EvidenceService};

pub struct AppState {
    pub catalog: Catalog,
    pub data_dir: PathBuf,
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
        let data_dir = app.path().app_data_dir().map_err(|error| error.to_string())?;
        std::fs::create_dir_all(&data_dir).map_err(|error| error.to_string())?;
        let catalog = Catalog::open(data_dir.join("lifesub.sqlite3")).map_err(|error| error.to_string())?;
        Ok(Self { catalog, data_dir })
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
pub fn import_audio_file(session: CaptureSession, path: String, state: State<'_, AppState>) -> Result<AudioChunk, String> {
    EvidenceService::new(Catalog::open(state.data_dir.join("lifesub.sqlite3")).map_err(|error| error.to_string())?, &state.data_dir)
        .import_audio(&session, path)
        .map_err(|error| format!("{error:?}"))
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
