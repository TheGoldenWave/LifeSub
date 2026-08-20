//! Command contract tests for the ASR desktop API.
//!
//! These tests verify that each handler in `desktop_api.rs` has the correct
//! contracts: expected inputs, outputs, error handling, and validation.

use crate::asr::manifest;
use crate::asr::settings::{
    AsrLanguage, AsrProviderKind, AsrProviderOptions, AsrSettings, WhisperTask,
};
use crate::catalog::Catalog;
use crate::desktop_api;

fn setup() -> (Catalog, tempfile::TempDir) {
    let data_dir = tempfile::tempdir().unwrap();
    let catalog = Catalog::open(data_dir.path().join("test.sqlite3")).unwrap();
    (catalog, data_dir)
}

#[test]
fn get_asr_settings_returns_defaults() {
    let (catalog, _data_dir) = setup();
    let settings = desktop_api::get_asr_settings(&catalog).unwrap();
    assert_eq!(settings.provider, AsrProviderKind::SenseVoice);
    assert_eq!(settings.model_id, "sense-voice-small-int8-2024-07-17");
    assert_eq!(settings.language, AsrLanguage::Zh);
    assert!(settings.vad_enabled);
}

#[test]
fn save_asr_settings_validates_provider_options_mismatch() {
    let (catalog, _data_dir) = setup();
    let invalid = AsrSettings::whisper("whisper-base")
        .with_options(AsrProviderOptions::SenseVoice { use_itn: true });
    let result = desktop_api::save_asr_settings(&catalog, &invalid);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("ProviderOptionsMismatch"));
}

#[test]
fn save_asr_settings_validates_model_not_found() {
    let (catalog, _data_dir) = setup();
    let result = desktop_api::save_asr_settings(&catalog, &AsrSettings::sense_voice("nonexistent"));
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("model not found"));
}

#[test]
fn save_asr_settings_validates_model_provider_mismatch() {
    let (catalog, _data_dir) = setup();
    let result = desktop_api::save_asr_settings(&catalog, &AsrSettings::sense_voice("whisper-base"));
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("ModelProviderMismatch"));
}

#[test]
fn save_asr_settings_accepts_valid_config() {
    let (catalog, _data_dir) = setup();
    let settings = AsrSettings::sense_voice("sense-voice-small-int8-2024-07-17");
    assert!(desktop_api::save_asr_settings(&catalog, &settings).is_ok());
}

#[test]
fn list_asr_models_returns_all_registered_models() {
    let (catalog, _data_dir) = setup();
    let models = desktop_api::list_asr_models(&catalog).unwrap();
    assert!(models.len() >= 4, "expected at least 4 models, got {}", models.len());
    for model in &models {
        assert!(!model.model_id.is_empty());
        assert!(!model.display_name.is_empty());
        assert!(!model.provider.is_empty());
        assert!(model.archive_size_bytes > 0);
        assert!(!model.license.is_empty());
    }
    let sv = models.iter().find(|m| m.model_id == "sense-voice-small-int8-2024-07-17").expect("SenseVoice model must be present");
    assert_eq!(sv.provider, "sense_voice");
    assert!(sv.recommended);
}

#[test]
fn download_asr_model_fails_for_nonexistent_model() {
    let (catalog, _data_dir) = setup();
    let result = desktop_api::download_asr_model(&catalog, "nonexistent");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("model not found"));
}

#[test]
fn download_asr_model_returns_stub_for_valid_model() {
    let (catalog, _data_dir) = setup();
    let result = desktop_api::download_asr_model(&catalog, "sense-voice-small-int8-2024-07-17");
    assert!(result.is_err()); // stub returns error
}

#[test]
fn cancel_model_download_with_bogus_id_returns_error() {
    let (catalog, _data_dir) = setup();
    assert!(desktop_api::cancel_model_download(&catalog, "bogus").is_err());
}

#[test]
fn delete_asr_model_fails_for_nonexistent_model() {
    let (catalog, _data_dir) = setup();
    assert!(desktop_api::delete_asr_model(&catalog, "nonexistent").is_err());
}

#[test]
fn list_asr_jobs_returns_empty_when_no_jobs_exist() {
    let (catalog, _data_dir) = setup();
    let jobs = desktop_api::list_asr_jobs(&catalog).unwrap();
    assert!(jobs.is_empty());
}

#[test]
fn cancel_asr_job_with_nonexistent_id_is_ok() {
    let (catalog, _data_dir) = setup();
    assert!(desktop_api::cancel_asr_job(&catalog, "nonexistent").is_ok());
}

#[test]
fn retry_asr_job_with_nonexistent_id_fails() {
    let (catalog, _data_dir) = setup();
    assert!(desktop_api::retry_asr_job(&catalog, "nonexistent").is_err());
}

#[test]
fn retranscribe_record_validates_settings() {
    let (catalog, _data_dir) = setup();
    let invalid = AsrSettings::whisper("whisper-base")
        .with_options(AsrProviderOptions::SenseVoice { use_itn: true });
    let result = desktop_api::retranscribe_record(&catalog, "rec", "chk", &invalid);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("ProviderOptionsMismatch"));
}

#[test]
fn retranscribe_record_with_valid_settings_returns_stub_error() {
    let (catalog, _data_dir) = setup();
    let settings = AsrSettings::sense_voice("sense-voice-small-int8-2024-07-17");
    let result = desktop_api::retranscribe_record(&catalog, "rec", "chk", &settings);
    assert!(result.is_err()); // stub
}

#[test]
fn import_result_dto_has_expected_shape() {
    let chunk = crate::domain::AudioChunk {
        id: "chk_test".into(), session_id: "rec_test".into(),
        source: crate::domain::AudioSource::Imported,
        path: "audio/test.wav".into(),
        sha256: "a".repeat(64), byte_length: 100,
    };
    let result = desktop_api::ImportResult { chunk: chunk.clone(), job: None };
    assert_eq!(result.chunk.id, "chk_test");
    assert!(result.job.is_none());
}

#[test]
fn model_progress_event_has_stable_payload() {
    let event = desktop_api::build_model_progress_event(
        "sense-voice-small-int8-2024-07-17", "mdl_abc", "downloading", 50_000_000, 163_002_883, None,
    );
    assert_eq!(event.model_id, "sense-voice-small-int8-2024-07-17");
    assert_eq!(event.download_id, "mdl_abc");
    assert_eq!(event.state, "downloading");
    assert_eq!(event.downloaded_bytes, 50_000_000);
    assert_eq!(event.expected_bytes, 163_002_883);
    assert!(event.error_code.is_none());
}

#[test]
fn job_state_event_has_stable_payload() {
    use crate::domain::AsrJobState;
    let event = desktop_api::build_job_state_event("job_abc", "rec_test", "chk_test", AsrJobState::Transcribing, None);
    assert_eq!(event.job_id, "job_abc");
    assert_eq!(event.session_id, "rec_test");
    assert_eq!(event.chunk_id, "chk_test");
    assert_eq!(event.state, AsrJobState::Transcribing);
    assert!(event.error_code.is_none());
}

#[test]
fn job_state_event_includes_error_code_when_present() {
    use crate::domain::AsrJobState;
    let event = desktop_api::build_job_state_event("job_abc", "rec", "chk", AsrJobState::Failed, Some("model_integrity_failed"));
    assert_eq!(event.state, AsrJobState::Failed);
    assert_eq!(event.error_code, Some("model_integrity_failed".into()));
}

#[test]
fn worker_context_initialize_succeeds() {
    let data_dir = tempfile::tempdir().unwrap();
    let ctx = desktop_api::WorkerContext::initialize(data_dir.path()).unwrap();
    assert!(!ctx.boot_id.is_empty());
    assert!(ctx.boot_id.starts_with("boot_"));
}

#[test]
fn provider_kind_snake_case_matches_manifest_values() {
    let sense_voice: AsrProviderKind = serde_json::from_str(r#""sense_voice""#).unwrap();
    assert_eq!(sense_voice, AsrProviderKind::SenseVoice);
    let whisper: AsrProviderKind = serde_json::from_str(r#""whisper""#).unwrap();
    assert_eq!(whisper, AsrProviderKind::Whisper);
}
