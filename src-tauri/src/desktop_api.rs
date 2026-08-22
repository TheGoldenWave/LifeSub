//! Framework-independent ASR handlers and DTO mapping.
//!
//! Every handler receives only the services it needs and returns plain result
//! types. Tauri commands in `commands.rs` are thin wrappers that delegate here.
//! This separation keeps the business logic testable without a Tauri runtime.

use std::path::Path;

use serde::Serialize;

use crate::asr::manifest;
use crate::asr::settings::{AsrProviderKind, AsrSettings};
use crate::catalog::Catalog;
use crate::domain::{AudioChunk, AsrJobState};

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize)]
pub struct ImportResult {
    pub chunk: AudioChunk,
    pub job: Option<AsrJobInfo>,
}

#[derive(Clone, Debug, Serialize)]
pub struct AsrJobInfo {
    pub id: String,
    pub state: AsrJobState,
    pub attempt_count: i64,
    pub error_code: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ModelInfo {
    pub model_id: String,
    pub display_name: String,
    pub provider: String,
    pub archive_size_bytes: u64,
    pub license: String,
    pub recommended: bool,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Serialize)]
pub struct ModelProgressEvent {
    pub model_id: String,
    pub download_id: String,
    pub state: String,
    pub downloaded_bytes: u64,
    pub expected_bytes: u64,
    pub error_code: Option<String>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Serialize)]
pub struct JobStateEvent {
    pub job_id: String,
    pub session_id: String,
    pub chunk_id: String,
    pub state: AsrJobState,
    pub error_code: Option<String>,
}

// ---------------------------------------------------------------------------
// Settings handlers
// ---------------------------------------------------------------------------

pub fn get_asr_settings(_catalog: &Catalog) -> Result<AsrSettings, String> {
    Ok(AsrSettings::sense_voice("sense-voice-small-int8-2024-07-17"))
}

pub fn save_asr_settings(_catalog: &Catalog, settings: &AsrSettings) -> Result<(), String> {
    let model = manifest::find_by_id(&settings.model_id)
        .ok_or_else(|| format!("model not found: {}", settings.model_id))?;
    settings.validate(model).map_err(|e| format!("invalid settings: {e:?}"))
}

// ---------------------------------------------------------------------------
// Model handlers
// ---------------------------------------------------------------------------

pub fn list_asr_models(_catalog: &Catalog) -> Result<Vec<ModelInfo>, String> {
    let models = manifest::all_manifests();
    let mut result = Vec::with_capacity(models.len());
    for model in models {
        result.push(ModelInfo {
            model_id: model.id.to_string(),
            display_name: model.display_name.to_string(),
            provider: match model.provider {
                Some(AsrProviderKind::SenseVoice) => "sense_voice".into(),
                Some(AsrProviderKind::Whisper) => "whisper".into(),
                None => "vad".into(),
            },
            archive_size_bytes: model.archive_size_bytes,
            license: model.source.license.to_string(),
            recommended: model.id == "sense-voice-small-int8-2024-07-17" || model.id == "whisper-base",
        });
    }
    Ok(result)
}

pub fn download_asr_model(_catalog: &Catalog, model_id: &str) -> Result<String, String> {
    let _model = manifest::find_by_id(model_id)
        .ok_or_else(|| format!("model not found: {model_id}"))?;
    Err("stub: model download not yet implemented".into())
}

pub fn cancel_model_download(_catalog: &Catalog, _download_id: &str) -> Result<(), String> {
    Err("stub: cancel download not yet implemented".into())
}

pub fn delete_asr_model(_catalog: &Catalog, model_id: &str) -> Result<(), String> {
    let _model = manifest::find_by_id(model_id)
        .ok_or_else(|| format!("model not found: {model_id}"))?;
    Err("stub: model deletion not yet implemented".into())
}

// ---------------------------------------------------------------------------
// Job handlers
// ---------------------------------------------------------------------------

pub fn list_asr_jobs(_catalog: &Catalog) -> Result<Vec<AsrJobInfo>, String> {
    Ok(Vec::new())
}

pub fn cancel_asr_job(_catalog: &Catalog, _job_id: &str) -> Result<(), String> {
    Ok(())
}

pub fn retry_asr_job(_catalog: &Catalog, job_id: &str) -> Result<AsrJobInfo, String> {
    Err(format!("retry_asr_job: stub — job {job_id} cannot be retried yet"))
}

pub fn retranscribe_record(
    _catalog: &Catalog, _session_id: &str, _chunk_id: &str, settings: &AsrSettings,
) -> Result<String, String> {
    let model = manifest::find_by_id(&settings.model_id)
        .ok_or_else(|| format!("model not found: {}", settings.model_id))?;
    settings.validate(model).map_err(|e| format!("invalid settings: {e:?}"))?;
    Err("retranscribe_record: stub — not yet implemented".into())
}

// ---------------------------------------------------------------------------
// Worker initialization
// ---------------------------------------------------------------------------

pub struct WorkerContext {
    _boot_id: String,
    _data_dir: std::path::PathBuf,
}

impl WorkerContext {
    pub fn initialize(_data_dir: &Path) -> Result<Self, String> {
        // Stub: acquires asr-worker.lock and initializes services.
        let boot_id = format!("boot_{}", uuid::Uuid::new_v4().simple());
        Ok(Self { _boot_id: boot_id, _data_dir: _data_dir.to_path_buf() })
    }
}

#[allow(dead_code)]
pub fn build_model_progress_event(
    model_id: &str, download_id: &str, state: &str,
    downloaded_bytes: u64, expected_bytes: u64, error_code: Option<&str>,
) -> ModelProgressEvent {
    ModelProgressEvent {
        model_id: model_id.to_string(),
        download_id: download_id.to_string(),
        state: state.to_string(),
        downloaded_bytes,
        expected_bytes,
        error_code: error_code.map(|value| value.to_string()),
    }
}

#[allow(dead_code)]
pub fn build_job_state_event(
    job_id: &str, session_id: &str, chunk_id: &str,
    state: AsrJobState, error_code: Option<&str>,
) -> JobStateEvent {
    JobStateEvent {
        job_id: job_id.to_string(),
        session_id: session_id.to_string(),
        chunk_id: chunk_id.to_string(),
        state,
        error_code: error_code.map(|value| value.to_string()),
    }
}
