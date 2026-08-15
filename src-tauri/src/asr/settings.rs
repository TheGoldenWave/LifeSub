use serde::{Deserialize, Serialize};

use crate::asr::model_lookup::ModelLookup;
use crate::domain::{AsrErrorCode, AsrLanguage, AsrProviderKind};

const DEFAULT_LANGUAGE: &str = "auto";
const DEFAULT_MAX_THREADS: usize = 4;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WhisperTask {
    Transcribe,
    Translate,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "provider", rename_all = "snake_case")]
pub enum AsrProviderOptions {
    SenseVoice { use_itn: bool },
    Whisper { task: WhisperTask },
    Qwen3Asr,
}

impl AsrProviderOptions {
    fn matches_provider(&self, provider: AsrProviderKind) -> bool {
        matches!(
            (self, provider),
            (
                AsrProviderOptions::SenseVoice { .. },
                AsrProviderKind::SenseVoice
            ) | (AsrProviderOptions::Whisper { .. }, AsrProviderKind::Whisper)
                | (AsrProviderOptions::Qwen3Asr, AsrProviderKind::Qwen3Asr)
        )
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AsrSettings {
    pub provider: AsrProviderKind,
    pub model_id: String,
    pub language: AsrLanguage,
    pub num_threads: u16,
    pub vad_enabled: bool,
    pub auto_transcribe_imports: bool,
    pub options: AsrProviderOptions,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AsrSettingsError {
    UnknownModel,
    ModelProviderMismatch,
    UnsupportedLanguage,
    ProviderOptionsMismatch,
    ModelCapabilityUnavailable,
    InvalidThreadCount,
}

impl AsrSettings {
    pub fn sense_voice(model_id: impl Into<String>) -> Self {
        Self::new(
            AsrProviderKind::SenseVoice,
            model_id,
            AsrProviderOptions::SenseVoice { use_itn: true },
        )
    }

    pub fn whisper(model_id: impl Into<String>) -> Self {
        Self::new(
            AsrProviderKind::Whisper,
            model_id,
            AsrProviderOptions::Whisper {
                task: WhisperTask::Transcribe,
            },
        )
    }

    pub fn qwen3_asr(model_id: impl Into<String>) -> Self {
        Self::new(
            AsrProviderKind::Qwen3Asr,
            model_id,
            AsrProviderOptions::Qwen3Asr,
        )
    }

    pub fn with_options(mut self, options: AsrProviderOptions) -> Self {
        self.options = options;
        self
    }

    pub fn with_language(mut self, language: AsrLanguage) -> Self {
        self.language = language;
        self
    }

    pub fn with_num_threads(mut self, num_threads: u16) -> Self {
        self.num_threads = num_threads;
        self
    }

    pub fn validate<M>(&self, models: &M) -> Result<(), AsrSettingsError>
    where
        M: ModelLookup + ?Sized,
    {
        let capabilities = models
            .lookup(&self.model_id)
            .ok_or(AsrSettingsError::UnknownModel)?;
        if capabilities.provider != self.provider {
            return Err(AsrSettingsError::ModelProviderMismatch);
        }
        if !capabilities.supports_language(&self.language) {
            return Err(AsrSettingsError::UnsupportedLanguage);
        }
        if !self.options.matches_provider(self.provider) {
            return Err(AsrSettingsError::ProviderOptionsMismatch);
        }
        if !capabilities.executable {
            return Err(AsrSettingsError::ModelCapabilityUnavailable);
        }
        if self.num_threads == 0 || usize::from(self.num_threads) > logical_cpu_count() {
            return Err(AsrSettingsError::InvalidThreadCount);
        }
        Ok(())
    }

    fn new(
        provider: AsrProviderKind,
        model_id: impl Into<String>,
        options: AsrProviderOptions,
    ) -> Self {
        Self {
            provider,
            model_id: model_id.into(),
            language: AsrLanguage::new(DEFAULT_LANGUAGE)
                .expect("the built-in default ASR language must be valid"),
            num_threads: default_num_threads(),
            vad_enabled: true,
            auto_transcribe_imports: true,
            options,
        }
    }
}

impl From<AsrSettingsError> for AsrErrorCode {
    fn from(error: AsrSettingsError) -> Self {
        match error {
            AsrSettingsError::UnknownModel => Self::ModelNotInstalled,
            AsrSettingsError::ModelCapabilityUnavailable => Self::ModelCapabilityUnavailable,
            AsrSettingsError::ModelProviderMismatch
            | AsrSettingsError::UnsupportedLanguage
            | AsrSettingsError::ProviderOptionsMismatch
            | AsrSettingsError::InvalidThreadCount => Self::InvalidProviderParameter,
        }
    }
}

fn logical_cpu_count() -> usize {
    std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1)
}

fn default_num_threads() -> u16 {
    u16::try_from(logical_cpu_count().min(DEFAULT_MAX_THREADS)).unwrap_or(1)
}
