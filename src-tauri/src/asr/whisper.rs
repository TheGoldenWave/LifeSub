use crate::asr::manifest::ModelManifest;
use crate::asr::provider::{
    BackendKind, NativeRequest, ProviderError, ProviderOptions, ProviderRequest,
    RuntimeExecutionIdentity, RuntimeFamily,
};
use crate::asr::settings::WhisperTask;
use crate::domain::AsrErrorCode;

pub(crate) fn native_request(
    request: &ProviderRequest,
    manifest: &ModelManifest,
) -> Result<NativeRequest, ProviderError> {
    let ProviderOptions::Whisper { task } = request.options else {
        return Err(ProviderError::new(
            AsrErrorCode::InvalidProviderParameter,
            "Whisper options required",
        ));
    };
    if request.language == "multilingual"
        || !manifest
            .supported_languages
            .contains(&request.language.as_str())
    {
        return Err(ProviderError::new(
            AsrErrorCode::InvalidProviderParameter,
            "unsupported Whisper runtime language",
        ));
    }
    let prefix = request.model_id.strip_prefix("whisper-").ok_or_else(|| {
        ProviderError::new(
            AsrErrorCode::InvalidProviderParameter,
            "invalid Whisper model id",
        )
    })?;
    Ok(NativeRequest {
        backend: BackendKind::WhisperSherpa,
        runtime: RuntimeFamily::SherpaOnnx,
        install_dir: request.install_dir.clone(),
        required_files: vec![
            request.install_dir.join(format!("{prefix}-encoder.onnx")),
            request.install_dir.join(format!("{prefix}-decoder.onnx")),
            request.install_dir.join(format!("{prefix}-tokens.txt")),
        ],
        language: Some(request.language.clone()),
        use_itn: None,
        whisper_task: Some(
            match task {
                WhisperTask::Transcribe => "transcribe",
                WhisperTask::Translate => "translate",
            }
            .to_owned(),
        ),
        num_threads: request.num_threads,
        device: request.qualification.device.clone(),
        runtime_identity: RuntimeExecutionIdentity::sherpa(),
    })
}

#[cfg(feature = "asr-runtime")]
pub(crate) fn sherpa_config(request: &NativeRequest) -> sherpa_onnx::OfflineRecognizerConfig {
    let mut config = sherpa_onnx::OfflineRecognizerConfig::default();
    config.model_config.whisper = sherpa_onnx::OfflineWhisperModelConfig {
        encoder: request
            .required_files
            .first()
            .map(|path| path.to_string_lossy().into_owned()),
        decoder: request
            .required_files
            .get(1)
            .map(|path| path.to_string_lossy().into_owned()),
        language: request.language.clone(),
        task: request.whisper_task.clone(),
        tail_paddings: -1,
        enable_token_timestamps: true,
        enable_segment_timestamps: true,
    };
    config.model_config.tokens = request
        .required_files
        .get(2)
        .map(|path| path.to_string_lossy().into_owned());
    config.model_config.num_threads = i32::from(request.num_threads);
    config.model_config.provider = Some("cpu".to_owned());
    config
}
