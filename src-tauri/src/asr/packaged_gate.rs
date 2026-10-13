use std::path::Path;
use std::time::Instant;

use crate::asr::audio::decode_to_working_audio;
use crate::asr::manifest::model_registry;
use crate::asr::model_manager::DeviceProfile;
use crate::asr::provider::{
    AsrProvider, AudioSlice, CancellationToken, ProductionProviderFactory, ProviderOptions,
    ProviderSelection,
};
use crate::asr::settings::WhisperTask;
use crate::domain::AsrProviderKind;
use crate::service::CoreRuntime;

#[derive(Clone, Debug, serde::Serialize)]
pub struct PackagedInferenceResult {
    pub model_id: String,
    pub manifest_version: String,
    pub bundle_identity: String,
    pub text: String,
    pub audio_duration_ms: u64,
    pub elapsed_ms: u64,
}

pub fn ensure_installed_model(data_directory: &Path, model_id: &str) -> Result<(), String> {
    let runtime = CoreRuntime::initialize(data_directory).map_err(|error| format!("{error:?}"))?;
    runtime
        .ensure_model_installed(model_id, &DeviceProfile::current(), || false)
        .map_err(|error| error.to_string())
}

pub fn transcribe_installed_fixture(
    data_directory: &Path,
    model_id: &str,
    fixture: &Path,
) -> Result<PackagedInferenceResult, String> {
    let manifest = model_registry()
        .model(model_id)
        .ok_or_else(|| format!("unknown packaged gate model {model_id}"))?;
    let runtime = CoreRuntime::initialize(data_directory).map_err(|error| format!("{error:?}"))?;
    let installation = runtime
        .executable_installation(model_id)
        .map_err(|error| error.to_string())?;
    let options = match manifest.provider {
        AsrProviderKind::SenseVoice => ProviderOptions::SenseVoice { use_itn: true },
        AsrProviderKind::Whisper => ProviderOptions::Whisper {
            task: WhisperTask::Transcribe,
        },
        AsrProviderKind::Qwen3Asr => ProviderOptions::Qwen3Asr,
    };
    let mut provider = ProductionProviderFactory::new()
        .create(installation, ProviderSelection::new("auto", 4, options))
        .map_err(|error| error.to_string())?;
    let audio = decode_to_working_audio(fixture).map_err(|error| error.code().to_string())?;
    let slice =
        AudioSlice::new(&audio.samples, audio.sample_rate_hz).map_err(|error| error.to_string())?;
    let started = Instant::now();
    let text = provider
        .transcribe(slice, &CancellationToken::new())
        .map_err(|error| error.to_string())?;
    let elapsed_ms = started.elapsed().as_millis() as u64;

    Ok(PackagedInferenceResult {
        model_id: manifest.id.to_string(),
        manifest_version: manifest.manifest_version.to_string(),
        bundle_identity: manifest.bundle.identity_sha256.to_string(),
        text,
        audio_duration_ms: audio.duration_ms,
        elapsed_ms,
    })
}
