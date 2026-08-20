use serde::{Deserialize, Serialize};

use super::model_lookup::ModelLookup;

// ---------------------------------------------------------------------------
// Provider kind — persisted as snake_case strings, never Debug output
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AsrProviderKind {
    SenseVoice,
    Whisper,
}

// ---------------------------------------------------------------------------
// Whisper task mode
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WhisperTask {
    Transcribe,
    Translate,
}

// ---------------------------------------------------------------------------
// Provider-specific options — tagged enum so the variant is self-describing
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AsrProviderOptions {
    SenseVoice { use_itn: bool },
    Whisper { task: WhisperTask },
}

// ---------------------------------------------------------------------------
// Languages supported by at least one provider
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AsrLanguage {
    Auto,
    Zh,
    En,
    Ja,
    Ko,
    Yue,
    De,
    Fr,
    Es,
    It,
    Pt,
    Ru,
    Nl,
    Pl,
    Tr,
    Ar,
    Hi,
    Vi,
    Th,
    Uk,
}

// ---------------------------------------------------------------------------
// Validated ASR settings
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AsrSettings {
    pub provider: AsrProviderKind,
    pub model_id: String,
    pub language: AsrLanguage,
    pub num_threads: u16,
    pub vad_enabled: bool,
    pub auto_transcribe_imports: bool,
    pub options: AsrProviderOptions,
}

// ---------------------------------------------------------------------------
// Settings validation errors
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AsrSettingsError {
    ProviderOptionsMismatch,
    InvalidThreadCount(u16),
    LanguageNotSupported {
        language: AsrLanguage,
        model_id: String,
    },
    ModelNotFound(String),
    ModelProviderMismatch {
        model_id: String,
        expected: AsrProviderKind,
        actual: AsrProviderKind,
    },
}

// ---------------------------------------------------------------------------
// Convenience constructors for tests
// ---------------------------------------------------------------------------

impl AsrSettings {
    pub fn sense_voice(model_id: &str) -> Self {
        Self {
            provider: AsrProviderKind::SenseVoice,
            model_id: model_id.to_string(),
            language: AsrLanguage::Zh,
            num_threads: 4,
            vad_enabled: true,
            auto_transcribe_imports: false,
            options: AsrProviderOptions::SenseVoice { use_itn: true },
        }
    }

    pub fn whisper(model_id: &str) -> Self {
        Self {
            provider: AsrProviderKind::Whisper,
            model_id: model_id.to_string(),
            language: AsrLanguage::En,
            num_threads: 4,
            vad_enabled: true,
            auto_transcribe_imports: false,
            options: AsrProviderOptions::Whisper {
                task: WhisperTask::Transcribe,
            },
        }
    }

    pub fn with_options(mut self, options: AsrProviderOptions) -> Self {
        self.options = options;
        self
    }

    pub fn with_language(mut self, language: AsrLanguage) -> Self {
        self.language = language;
        self
    }

    pub fn with_threads(mut self, num_threads: u16) -> Self {
        self.num_threads = num_threads;
        self
    }

    /// Validates that the settings are consistent with the given model.
    ///
    /// Checks are performed in order of increasing cost:
    /// 1. Model identity — the looked-up model must match `self.model_id`
    /// 2. Provider ownership — the model must belong to `self.provider`
    /// 3. Options compatibility — options variant must match the provider kind
    /// 4. Thread count — must be at least 1
    /// 5. Language support — the model must support the requested language
    pub fn validate(&self, model: &dyn ModelLookup) -> Result<(), AsrSettingsError> {
        // 1. Model identity
        if model.model_id() != self.model_id {
            return Err(AsrSettingsError::ModelNotFound(self.model_id.clone()));
        }

        // 2. Provider ownership
        let model_provider = model.provider();
        if model_provider != self.provider {
            return Err(AsrSettingsError::ModelProviderMismatch {
                model_id: self.model_id.clone(),
                expected: self.provider,
                actual: model_provider,
            });
        }

        // 3. Options variant must match provider kind
        let options_match = matches!(
            (&self.options, self.provider),
            (
                AsrProviderOptions::SenseVoice { .. },
                AsrProviderKind::SenseVoice
            ) | (AsrProviderOptions::Whisper { .. }, AsrProviderKind::Whisper)
        );
        if !options_match {
            return Err(AsrSettingsError::ProviderOptionsMismatch);
        }

        // 4. Thread count must be at least 1
        if self.num_threads < 1 {
            return Err(AsrSettingsError::InvalidThreadCount(self.num_threads));
        }

        // 5. Language support
        if !model.supports_language(&self.language) {
            return Err(AsrSettingsError::LanguageNotSupported {
                language: self.language.clone(),
                model_id: self.model_id.clone(),
            });
        }

        Ok(())
    }
}
