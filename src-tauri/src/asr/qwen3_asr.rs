use crate::asr::manifest::ModelManifest;
use crate::asr::provider::{
    BackendKind, NativeRequest, ProviderError, ProviderOptions, ProviderRequest,
    RuntimeExecutionIdentity, RuntimeFamily,
};
use crate::domain::AsrErrorCode;

#[cfg(all(
    feature = "asr-qwen17-runtime",
    target_os = "macos",
    target_arch = "aarch64"
))]
pub struct Qwen17RuntimeSmoke {
    device_name: String,
}

#[cfg(all(
    feature = "asr-qwen17-runtime",
    target_os = "macos",
    target_arch = "aarch64"
))]
impl Qwen17RuntimeSmoke {
    pub fn new(device_name: impl Into<String>) -> Self {
        Self {
            device_name: device_name.into(),
        }
    }
}

#[cfg(all(
    feature = "asr-qwen17-runtime",
    target_os = "macos",
    target_arch = "aarch64"
))]
impl crate::asr::runtime_qualifier::RuntimeSmoke for Qwen17RuntimeSmoke {
    fn smoke(
        &self,
        handle: &crate::asr::runtime_qualifier::QualificationHandle,
    ) -> Result<crate::asr::runtime_qualifier::QualifiedRuntimeIdentity, String> {
        let fixture = crate::asr::runtime_qualifier::load_qualification_speech_fixture()
            .map_err(|error| error.to_string())?;
        let device = create_metal_device().map_err(|error| error.to_string())?;
        let inference = qwen3_asr::AsrInference::load(&handle.install_dir, device)
            .map_err(|error| error.to_string())?;
        let result = inference
            .transcribe_samples(&fixture.samples, qwen3_asr::TranscribeOptions::default())
            .map_err(|error| error.to_string())?;
        if result.text.trim().is_empty() {
            return Err("Qwen 1.7B qualification smoke returned empty text".to_owned());
        }
        let matched = fixture
            .expected_phrases
            .iter()
            .filter(|phrase| {
                crate::asr::runtime_qualifier::normalize_qualification_text(&result.text)
                    .contains(&crate::asr::runtime_qualifier::normalize_qualification_text(phrase))
            })
            .count();
        if matched < fixture.minimum_phrase_matches {
            return Err(format!(
                "Qwen 1.7B qualification smoke matched only {matched}/{} expected phrases",
                fixture.expected_phrases.len()
            ));
        }
        Ok(crate::asr::runtime_qualifier::QualifiedRuntimeIdentity {
            crate_name: "qwen3-asr".to_owned(),
            crate_version: "0.2.2".to_owned(),
            git_commit: "c5ef09646af6278d2ba8b8ceaf543ffb32d1a5dc".to_owned(),
            candle_version: "0.9.2".to_owned(),
            backend: "metal".to_owned(),
            target_os: std::env::consts::OS.to_owned(),
            target_arch: std::env::consts::ARCH.to_owned(),
            device_index: 0,
            device_name: self.device_name.clone(),
            smoke_fixture_sha256: crate::asr::runtime_qualifier::QUALIFICATION_SMOKE_FIXTURE_SHA256
                .to_owned(),
            qualification_contract_sha256:
                crate::asr::runtime_qualifier::QUALIFICATION_CONTRACT_SHA256.to_owned(),
        })
    }
}

pub(crate) fn native_request(
    request: &ProviderRequest,
    manifest: &ModelManifest,
) -> Result<NativeRequest, ProviderError> {
    if request.options != ProviderOptions::Qwen3Asr {
        return Err(ProviderError::new(
            AsrErrorCode::InvalidProviderParameter,
            "Qwen3-ASR options required",
        ));
    }
    match request.model_id.as_str() {
        "qwen3-asr-0.6b-int8-2026-03-25" => qwen06(request),
        "qwen3-asr-1.7b" => qwen17(request, manifest),
        _ => Err(ProviderError::new(
            AsrErrorCode::InvalidProviderParameter,
            "unknown Qwen model",
        )),
    }
}

fn qwen06(request: &ProviderRequest) -> Result<NativeRequest, ProviderError> {
    if request.language != "auto" {
        return Err(ProviderError::new(
            AsrErrorCode::InvalidProviderParameter,
            "Qwen 0.6B only supports auto language",
        ));
    }
    Ok(NativeRequest {
        backend: BackendKind::Qwen06Sherpa,
        runtime: RuntimeFamily::SherpaOnnx,
        install_dir: request.install_dir.clone(),
        required_files: vec![
            request.install_dir.join("conv_frontend.onnx"),
            request.install_dir.join("encoder.int8.onnx"),
            request.install_dir.join("decoder.int8.onnx"),
            request.install_dir.join("tokenizer"),
        ],
        language: None,
        use_itn: None,
        whisper_task: None,
        num_threads: request.num_threads,
        device: request.qualification.device.clone(),
        runtime_identity: RuntimeExecutionIdentity::sherpa(),
    })
}

fn qwen17(
    request: &ProviderRequest,
    manifest: &ModelManifest,
) -> Result<NativeRequest, ProviderError> {
    if !manifest
        .supported_languages
        .contains(&request.language.as_str())
    {
        return Err(ProviderError::new(
            AsrErrorCode::InvalidProviderParameter,
            "unsupported Qwen 1.7B language",
        ));
    }
    let device = request.qualification.device.clone();
    Ok(NativeRequest {
        backend: BackendKind::Qwen17CandleMetal,
        runtime: RuntimeFamily::QwenCandleMetal,
        install_dir: request.install_dir.clone(),
        required_files: manifest
            .bundle
            .required_paths
            .iter()
            .map(|path| request.install_dir.join(path))
            .collect(),
        language: qwen17_language(&request.language).map(str::to_owned),
        use_itn: None,
        whisper_task: None,
        num_threads: request.num_threads,
        runtime_identity: RuntimeExecutionIdentity::qwen17(&device),
        device,
    })
}

fn qwen17_language(code: &str) -> Option<&'static str> {
    match code {
        "auto" => None,
        "zh" => Some("chinese"),
        "en" => Some("english"),
        "yue" => Some("cantonese"),
        "ar" => Some("arabic"),
        "de" => Some("german"),
        "fr" => Some("french"),
        "es" => Some("spanish"),
        "pt" => Some("portuguese"),
        "id" => Some("indonesian"),
        "it" => Some("italian"),
        "ko" => Some("korean"),
        "ru" => Some("russian"),
        "th" => Some("thai"),
        "vi" => Some("vietnamese"),
        "ja" => Some("japanese"),
        "tr" => Some("turkish"),
        "hi" => Some("hindi"),
        "ms" => Some("malay"),
        "nl" => Some("dutch"),
        "sv" => Some("swedish"),
        "da" => Some("danish"),
        "fi" => Some("finnish"),
        "pl" => Some("polish"),
        "cs" => Some("czech"),
        "fil" => Some("filipino"),
        "fa" => Some("persian"),
        "el" => Some("greek"),
        "hu" => Some("hungarian"),
        "mk" => Some("macedonian"),
        "ro" => Some("romanian"),
        _ => None,
    }
}

#[cfg(feature = "asr-runtime")]
pub(crate) fn sherpa_config(request: &NativeRequest) -> sherpa_onnx::OfflineRecognizerConfig {
    let mut config = sherpa_onnx::OfflineRecognizerConfig::default();
    config.model_config.qwen3_asr = sherpa_onnx::OfflineQwen3ASRModelConfig {
        conv_frontend: request
            .required_files
            .first()
            .map(|path| path.to_string_lossy().into_owned()),
        encoder: request
            .required_files
            .get(1)
            .map(|path| path.to_string_lossy().into_owned()),
        decoder: request
            .required_files
            .get(2)
            .map(|path| path.to_string_lossy().into_owned()),
        tokenizer: request
            .required_files
            .get(3)
            .map(|path| path.to_string_lossy().into_owned()),
        hotwords: None,
        ..Default::default()
    };
    config.model_config.num_threads = i32::from(request.num_threads);
    config.model_config.provider = Some("cpu".to_owned());
    config
}

#[cfg(all(
    feature = "asr-qwen17-runtime",
    target_os = "macos",
    target_arch = "aarch64"
))]
pub(crate) fn create_metal_device() -> Result<candle_core::Device, ProviderError> {
    let device = candle_core::Device::new_metal(0).map_err(|error| {
        ProviderError::new(
            AsrErrorCode::ProviderInitializationFailed,
            error.to_string(),
        )
    })?;
    if !device.is_metal() {
        return Err(ProviderError::new(
            AsrErrorCode::ProviderInitializationFailed,
            "Candle device is not Metal",
        ));
    }
    Ok(device)
}
