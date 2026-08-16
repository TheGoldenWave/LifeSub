use crate::asr::manifest::ModelManifest;
use crate::asr::provider::{
    BackendKind, NativeRequest, ProviderError, ProviderOptions, ProviderRequest,
    RuntimeExecutionIdentity, RuntimeFamily,
};

pub(crate) fn native_request(
    request: &ProviderRequest,
    manifest: &ModelManifest,
) -> Result<NativeRequest, ProviderError> {
    let ProviderOptions::SenseVoice { use_itn } = request.options else {
        return Err(ProviderError::new(
            crate::domain::AsrErrorCode::InvalidProviderParameter,
            "SenseVoice options required",
        ));
    };
    if !manifest
        .supported_languages
        .contains(&request.language.as_str())
    {
        return Err(ProviderError::new(
            crate::domain::AsrErrorCode::InvalidProviderParameter,
            "unsupported SenseVoice language",
        ));
    }
    Ok(NativeRequest {
        backend: BackendKind::SenseVoiceSherpa,
        runtime: RuntimeFamily::SherpaOnnx,
        install_dir: request.install_dir.clone(),
        required_files: vec![
            request.install_dir.join("model.int8.onnx"),
            request.install_dir.join("tokens.txt"),
        ],
        language: Some(request.language.clone()),
        use_itn: Some(use_itn),
        whisper_task: None,
        num_threads: request.num_threads,
        device: request.qualification.device.clone(),
        runtime_identity: RuntimeExecutionIdentity::sherpa(),
    })
}

#[cfg(feature = "asr-runtime")]
pub(crate) fn sherpa_config(request: &NativeRequest) -> sherpa_onnx::OfflineRecognizerConfig {
    let mut config = sherpa_onnx::OfflineRecognizerConfig::default();
    config.model_config.sense_voice = sherpa_onnx::OfflineSenseVoiceModelConfig {
        model: request
            .required_files
            .first()
            .map(|path| path.to_string_lossy().into_owned()),
        language: request.language.clone(),
        use_itn: request.use_itn.unwrap_or(true),
    };
    config.model_config.tokens = request
        .required_files
        .get(1)
        .map(|path| path.to_string_lossy().into_owned());
    config.model_config.num_threads = i32::from(request.num_threads);
    config.model_config.provider = Some("cpu".to_owned());
    config
}
