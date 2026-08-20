//! Whisper provider adapter for sherpa-onnx.
//!
//! Wraps the sherpa-onnx `OfflineRecognizer` with Whisper model config to
//! implement the `AsrProvider` trait. Supports transcribe and translate tasks,
//! and the languages declared in the Whisper manifest.

use std::path::Path;

use sherpa_onnx::{
    OfflineRecognizer, OfflineRecognizerConfig, OfflineWhisperModelConfig,
};

use super::provider::{
    AsrError, AsrProvider, AsrRequest, AsrText, AudioSlice, CancellationToken, ProviderIdentity,
};
use super::settings::{AsrLanguage, AsrProviderKind, AsrProviderOptions, WhisperTask};

/// Builds a Whisper provider from the sherpa-onnx runtime.
///
/// # Arguments
///
/// * `model_dir` - The directory containing the encoder/decoder ONNX files
///   and the tokens file. The exact filenames depend on the model size
///   (e.g. `base-encoder.onnx`, `base-decoder.onnx`, `base-tokens.txt`).
/// * `encoder_name` - The filename of the encoder ONNX model (e.g. "base-encoder.onnx").
/// * `decoder_name` - The filename of the decoder ONNX model (e.g. "base-decoder.onnx").
/// * `tokens_name` - The filename of the tokens file (e.g. "base-tokens.txt").
/// * `model_id` - The stable model identifier from the manifest.
/// * `num_threads` - Number of threads for the ONNX runtime.
///
/// # Errors
///
/// Returns `AsrError::ProviderInitializationFailed` if the model files cannot
/// be loaded or the ONNX session cannot be created.
pub fn build_whisper_provider(
    model_dir: &Path,
    encoder_name: &str,
    decoder_name: &str,
    tokens_name: &str,
    model_id: &str,
    num_threads: u16,
) -> Result<Box<dyn AsrProvider>, AsrError> {
    let encoder_path = model_dir.join(encoder_name);
    let decoder_path = model_dir.join(decoder_name);
    let tokens_path = model_dir.join(tokens_name);

    if !encoder_path.exists() {
        return Err(AsrError::ModelIntegrityFailed {
            model_id: model_id.to_string(),
            reason: format!("encoder file not found: {}", encoder_path.display()),
        });
    }
    if !decoder_path.exists() {
        return Err(AsrError::ModelIntegrityFailed {
            model_id: model_id.to_string(),
            reason: format!("decoder file not found: {}", decoder_path.display()),
        });
    }
    if !tokens_path.exists() {
        return Err(AsrError::ModelIntegrityFailed {
            model_id: model_id.to_string(),
            reason: format!("tokens file not found: {}", tokens_path.display()),
        });
    }

    let encoder_path_str = encoder_path
        .to_str()
        .ok_or_else(|| AsrError::ProviderInitializationFailed {
            reason: "encoder path is not valid UTF-8".to_string(),
        })?
        .to_string();
    let decoder_path_str = decoder_path
        .to_str()
        .ok_or_else(|| AsrError::ProviderInitializationFailed {
            reason: "decoder path is not valid UTF-8".to_string(),
        })?
        .to_string();
    let tokens_path_str = tokens_path
        .to_str()
        .ok_or_else(|| AsrError::ProviderInitializationFailed {
            reason: "tokens path is not valid UTF-8".to_string(),
        })?
        .to_string();

    let mut config = OfflineRecognizerConfig::default();
    config.model_config.whisper = OfflineWhisperModelConfig {
        encoder: Some(encoder_path_str),
        decoder: Some(decoder_path_str),
        language: None, // language is set per-request via stream options
        task: None,     // task is set per-request via stream options
        tail_paddings: -1,
        enable_token_timestamps: false,
        enable_segment_timestamps: false,
    };
    config.model_config.tokens = Some(tokens_path_str);
    config.model_config.num_threads = num_threads as i32;
    config.model_config.model_type = Some("whisper".to_string());

    let recognizer = OfflineRecognizer::create(&config).ok_or_else(|| {
        AsrError::ProviderInitializationFailed {
            reason: "failed to initialize Whisper model: OfflineRecognizer::create returned None".to_string(),
        }
    })?;

    let identity = ProviderIdentity {
        kind: AsrProviderKind::Whisper,
        model_id: model_id.to_string(),
        runtime_version: crate::asr::runtime_version().to_string(),
        runtime_build_id: crate::asr::runtime_git_sha1().to_string(),
    };

    Ok(Box::new(WhisperProvider {
        recognizer,
        identity,
    }))
}

// ---------------------------------------------------------------------------
// Provider implementation
// ---------------------------------------------------------------------------

struct WhisperProvider {
    recognizer: OfflineRecognizer,
    identity: ProviderIdentity,
}

impl AsrProvider for WhisperProvider {
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
        let task = match &request.options {
            AsrProviderOptions::Whisper { task } => *task,
            _ => {
                return Err(AsrError::InvalidProviderParameter {
                    reason: "expected Whisper options".to_string(),
                });
            }
        };

        let language = whisper_language_code(&request.language);
        let task_str = match task {
            WhisperTask::Transcribe => "transcribe",
            WhisperTask::Translate => "translate",
        };
        let sample_rate = audio.sample_rate as i32;

        let stream = self.recognizer.create_stream();

        // Set per-stream language and task options.
        // Whisper uses these stream options to control behavior per request.
        stream.set_option("language", language);
        stream.set_option("task", task_str);

        // Accept waveform.
        stream.accept_waveform(sample_rate, audio.samples);

        // Run inference. This is synchronous and blocks the calling thread.
        // The cancellation token is checked before this call; mid-inference
        // cancellation is not supported by sherpa-onnx.
        self.recognizer.decode(&stream);

        let result = stream.get_result().ok_or(AsrError::TranscriptionFailed {
            reason: "Whisper model returned no result".to_string(),
        })?;

        let trimmed = result.text.trim().to_string();
        if trimmed.is_empty() {
            return Err(AsrError::EmptyOutput);
        }

        Ok(AsrText { text: trimmed })
    }
}

/// Maps a LifeSub `AsrLanguage` to the sherpa-onnx Whisper language code.
///
/// Whisper uses language codes compatible with OpenAI's Whisper.
/// `Auto` maps to empty string for auto-detection.
fn whisper_language_code(language: &AsrLanguage) -> &'static str {
    match language {
        AsrLanguage::Auto => "",
        AsrLanguage::Zh => "zh",
        AsrLanguage::En => "en",
        AsrLanguage::Ja => "ja",
        AsrLanguage::Ko => "ko",
        AsrLanguage::De => "de",
        AsrLanguage::Fr => "fr",
        AsrLanguage::Es => "es",
        AsrLanguage::It => "it",
        AsrLanguage::Pt => "pt",
        AsrLanguage::Ru => "ru",
        AsrLanguage::Nl => "nl",
        AsrLanguage::Pl => "pl",
        AsrLanguage::Tr => "tr",
        AsrLanguage::Ar => "ar",
        AsrLanguage::Hi => "hi",
        AsrLanguage::Vi => "vi",
        AsrLanguage::Th => "th",
        AsrLanguage::Uk => "uk",
        AsrLanguage::Yue => "zh", // Whisper doesn't natively support Yue; fall back to zh
    }
}