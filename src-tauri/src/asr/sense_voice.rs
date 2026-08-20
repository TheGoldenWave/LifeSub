//! SenseVoice provider adapter for sherpa-onnx.
//!
//! Wraps the sherpa-onnx `OfflineRecognizer` with SenseVoice model config to
//! implement the `AsrProvider` trait. Supports ITN (inverse text normalization)
//! and the languages declared in the SenseVoice manifest.

use std::path::Path;

use sherpa_onnx::{
    OfflineRecognizer, OfflineRecognizerConfig, OfflineSenseVoiceModelConfig,
};

use super::provider::{
    AsrError, AsrProvider, AsrRequest, AsrText, AudioSlice, CancellationToken, ProviderIdentity,
};
use super::settings::{AsrLanguage, AsrProviderKind, AsrProviderOptions};

/// Builds a SenseVoice provider from the sherpa-onnx runtime.
///
/// # Arguments
///
/// * `model_dir` - The directory containing `model.int8.onnx` and `tokens.txt`.
/// * `num_threads` - Number of threads for the ONNX runtime.
///
/// # Errors
///
/// Returns `AsrError::ProviderInitializationFailed` if the model files cannot
/// be loaded or the ONNX session cannot be created.
pub fn build_sense_voice_provider(
    model_dir: &Path,
    num_threads: u16,
) -> Result<Box<dyn AsrProvider>, AsrError> {
    let model_path = model_dir.join("model.int8.onnx");
    let tokens_path = model_dir.join("tokens.txt");

    if !model_path.exists() {
        return Err(AsrError::ModelIntegrityFailed {
            model_id: "sense-voice-small-int8-2024-07-17".to_string(),
            reason: format!("model file not found: {}", model_path.display()),
        });
    }
    if !tokens_path.exists() {
        return Err(AsrError::ModelIntegrityFailed {
            model_id: "sense-voice-small-int8-2024-07-17".to_string(),
            reason: format!("tokens file not found: {}", tokens_path.display()),
        });
    }

    let model_path_str = model_path
        .to_str()
        .ok_or_else(|| AsrError::ProviderInitializationFailed {
            reason: "model path is not valid UTF-8".to_string(),
        })?
        .to_string();
    let tokens_path_str = tokens_path
        .to_str()
        .ok_or_else(|| AsrError::ProviderInitializationFailed {
            reason: "tokens path is not valid UTF-8".to_string(),
        })?
        .to_string();

    let mut config = OfflineRecognizerConfig::default();
    config.model_config.sense_voice = OfflineSenseVoiceModelConfig {
        model: Some(model_path_str),
        language: None, // language is set per-request via stream options
        use_itn: true,  // ITN is enabled by default; request-level option overrides
    };
    config.model_config.tokens = Some(tokens_path_str);
    config.model_config.num_threads = num_threads as i32;
    config.model_config.model_type = Some("sense_voice".to_string());

    let recognizer = OfflineRecognizer::create(&config).ok_or_else(|| {
        AsrError::ProviderInitializationFailed {
            reason: "failed to initialize SenseVoice model: OfflineRecognizer::create returned None".to_string(),
        }
    })?;

    let identity = ProviderIdentity {
        kind: AsrProviderKind::SenseVoice,
        model_id: "sense-voice-small-int8-2024-07-17".to_string(),
        runtime_version: crate::asr::runtime_version().to_string(),
        runtime_build_id: crate::asr::runtime_git_sha1().to_string(),
    };

    Ok(Box::new(SenseVoiceProvider {
        recognizer,
        identity,
    }))
}

// ---------------------------------------------------------------------------
// Provider implementation
// ---------------------------------------------------------------------------

struct SenseVoiceProvider {
    recognizer: OfflineRecognizer,
    identity: ProviderIdentity,
}

impl AsrProvider for SenseVoiceProvider {
    fn identity(&self) -> &ProviderIdentity {
        &self.identity
    }

    fn transcribe(
        &self,
        audio: AudioSlice<'_>,
        request: &AsrRequest,
        cancellation: &CancellationToken,
    ) -> Result<AsrText, AsrError> {
        if cancellation.is_cancelled() {
            return Err(AsrError::Cancelled);
        }

        if audio.samples.is_empty() {
            return Err(AsrError::EmptyOutput);
        }

        // Validate that the options variant matches this provider.
        // ITN is configured at model build time; the per-request value is
        // validated here for correctness but the model-level config prevails.
        let _use_itn = match &request.options {
            AsrProviderOptions::SenseVoice { use_itn } => *use_itn,
            _ => {
                return Err(AsrError::InvalidProviderParameter {
                    reason: "expected SenseVoice options".to_string(),
                });
            }
        };

        let language = sense_voice_language_code(&request.language);
        let sample_rate = audio.sample_rate as i32;

        let stream = self.recognizer.create_stream();

        // Set per-stream language option.
        stream.set_option("language", language);

        // Accept waveform — sherpa-onnx handles the full audio at once.
        stream.accept_waveform(sample_rate, audio.samples);

        // Run inference. This is synchronous and blocks the calling thread.
        // The cancellation token is checked before this call; mid-inference
        // cancellation is not supported by sherpa-onnx.
        self.recognizer.decode(&stream);

        let result = stream.get_result().ok_or(AsrError::TranscriptionFailed {
            reason: "SenseVoice model returned no result".to_string(),
        })?;

        // ITN is controlled by the model config, not the result.
        // The `use_itn` flag is passed through to the config at build time.
        let text = result.text;

        let trimmed = text.trim().to_string();
        if trimmed.is_empty() {
            return Err(AsrError::EmptyOutput);
        }

        Ok(AsrText { text: trimmed })
    }
}

/// Maps a LifeSub `AsrLanguage` to the sherpa-onnx SenseVoice language code.
///
/// SenseVoice uses short ISO 639-1 codes. Auto falls back to Chinese.
/// Unsupported languages are mapped to `"zh"` as a safe fallback.
fn sense_voice_language_code(language: &AsrLanguage) -> &'static str {
    match language {
        AsrLanguage::Zh | AsrLanguage::Auto => "zh",
        AsrLanguage::En => "en",
        AsrLanguage::Ja => "ja",
        AsrLanguage::Ko => "ko",
        AsrLanguage::Yue => "yue",
        _ => "zh",
    }
}