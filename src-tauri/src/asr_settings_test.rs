//! Tagged-settings validation tests for ASR provider-specific options.
//!
//! These tests validate that AsrSettings correctly rejects:
//! - Provider/options variant mismatches (e.g. Whisper + SenseVoice options)
//! - Invalid thread counts
//! - Unsupported languages
//! - Model/provider ownership mismatches
//! - Missing models
//!
//! And that valid combinations pass validation.

use crate::asr::model_lookup::ModelLookup;
use crate::asr::settings::{
    AsrLanguage, AsrProviderKind, AsrProviderOptions, AsrSettings, AsrSettingsError, WhisperTask,
};

// ---------------------------------------------------------------------------
// Stub model for tests — Task 5's static manifest will provide the real impl
// ---------------------------------------------------------------------------

struct StubModel {
    provider: AsrProviderKind,
    model_id: String,
    supported_languages: Vec<AsrLanguage>,
}

impl ModelLookup for StubModel {
    fn provider(&self) -> AsrProviderKind {
        self.provider
    }

    fn model_id(&self) -> &str {
        &self.model_id
    }

    fn supports_language(&self, language: &AsrLanguage) -> bool {
        self.supported_languages.contains(language)
    }
}

fn whisper_stub() -> StubModel {
    StubModel {
        provider: AsrProviderKind::Whisper,
        model_id: "whisper-base".to_string(),
        supported_languages: vec![
            AsrLanguage::Auto,
            AsrLanguage::En,
            AsrLanguage::Zh,
            AsrLanguage::Ja,
        ],
    }
}

fn sense_voice_stub() -> StubModel {
    StubModel {
        provider: AsrProviderKind::SenseVoice,
        model_id: "sense-voice-small-int8-2024-07-17".to_string(),
        supported_languages: vec![
            AsrLanguage::Zh,
            AsrLanguage::En,
            AsrLanguage::Ja,
            AsrLanguage::Ko,
            AsrLanguage::Yue,
        ],
    }
}

// ---------------------------------------------------------------------------
// RED-phase tests — these will fail until validation is implemented
// ---------------------------------------------------------------------------

#[test]
fn provider_options_mismatch_whisper_with_sense_voice_options() {
    let settings = AsrSettings::whisper("whisper-base")
        .with_options(AsrProviderOptions::SenseVoice { use_itn: true });
    assert_eq!(
        settings.validate(&whisper_stub()),
        Err(AsrSettingsError::ProviderOptionsMismatch)
    );
}

#[test]
fn provider_options_mismatch_sense_voice_with_whisper_options() {
    let settings = AsrSettings::sense_voice("sense-voice-small-int8-2024-07-17").with_options(
        AsrProviderOptions::Whisper {
            task: WhisperTask::Transcribe,
        },
    );
    assert_eq!(
        settings.validate(&sense_voice_stub()),
        Err(AsrSettingsError::ProviderOptionsMismatch)
    );
}

#[test]
fn thread_count_zero_rejected() {
    let settings = AsrSettings::whisper("whisper-base").with_threads(0);
    assert_eq!(
        settings.validate(&whisper_stub()),
        Err(AsrSettingsError::InvalidThreadCount(0))
    );
}

#[test]
fn thread_count_one_accepted() {
    let settings = AsrSettings::whisper("whisper-base").with_threads(1);
    assert!(settings.validate(&whisper_stub()).is_ok());
}

#[test]
fn thread_count_max_u16_accepted() {
    let settings = AsrSettings::whisper("whisper-base").with_threads(u16::MAX);
    assert!(settings.validate(&whisper_stub()).is_ok());
}

#[test]
fn language_not_supported_by_model() {
    let settings = AsrSettings::whisper("whisper-base").with_language(AsrLanguage::Ko);
    assert_eq!(
        settings.validate(&whisper_stub()),
        Err(AsrSettingsError::LanguageNotSupported {
            language: AsrLanguage::Ko,
            model_id: "whisper-base".to_string(),
        })
    );
}

#[test]
fn language_supported_accepted() {
    let settings = AsrSettings::sense_voice("sense-voice-small-int8-2024-07-17")
        .with_language(AsrLanguage::Zh);
    assert!(settings.validate(&sense_voice_stub()).is_ok());
}

#[test]
fn model_not_found_by_id() {
    let settings = AsrSettings::whisper("nonexistent-model");
    let stub = whisper_stub(); // model_id == "whisper-base", not "nonexistent-model"
    assert_eq!(
        settings.validate(&stub),
        Err(AsrSettingsError::ModelNotFound(
            "nonexistent-model".to_string()
        ))
    );
}

#[test]
fn model_provider_mismatch() {
    // Whisper settings pointing to a SenseVoice model
    let settings = AsrSettings::whisper("sense-voice-small-int8-2024-07-17");
    assert_eq!(
        settings.validate(&sense_voice_stub()),
        Err(AsrSettingsError::ModelProviderMismatch {
            model_id: "sense-voice-small-int8-2024-07-17".to_string(),
            expected: AsrProviderKind::Whisper,
            actual: AsrProviderKind::SenseVoice,
        })
    );
}

#[test]
fn valid_whisper_transcribe_settings() {
    let settings = AsrSettings::whisper("whisper-base")
        .with_options(AsrProviderOptions::Whisper {
            task: WhisperTask::Transcribe,
        })
        .with_language(AsrLanguage::En)
        .with_threads(2);
    assert!(settings.validate(&whisper_stub()).is_ok());
}

#[test]
fn valid_whisper_translate_task() {
    let settings = AsrSettings::whisper("whisper-base").with_options(AsrProviderOptions::Whisper {
        task: WhisperTask::Translate,
    });
    assert!(settings.validate(&whisper_stub()).is_ok());
}

#[test]
fn valid_sense_voice_with_itn() {
    let settings = AsrSettings::sense_voice("sense-voice-small-int8-2024-07-17")
        .with_options(AsrProviderOptions::SenseVoice { use_itn: true });
    assert!(settings.validate(&sense_voice_stub()).is_ok());
}

#[test]
fn valid_sense_voice_without_itn() {
    let settings = AsrSettings::sense_voice("sense-voice-small-int8-2024-07-17")
        .with_options(AsrProviderOptions::SenseVoice { use_itn: false });
    assert!(settings.validate(&sense_voice_stub()).is_ok());
}

#[test]
fn auto_language_for_whisper() {
    let settings = AsrSettings::whisper("whisper-base").with_language(AsrLanguage::Auto);
    assert!(settings.validate(&whisper_stub()).is_ok());
}

#[test]
fn sense_voice_all_supported_languages() {
    for lang in &[
        AsrLanguage::Zh,
        AsrLanguage::En,
        AsrLanguage::Ja,
        AsrLanguage::Ko,
        AsrLanguage::Yue,
    ] {
        let settings = AsrSettings::sense_voice("sense-voice-small-int8-2024-07-17")
            .with_language(lang.clone());
        assert!(
            settings.validate(&sense_voice_stub()).is_ok(),
            "SenseVoice should support {:?}",
            lang
        );
    }
}

// ---------------------------------------------------------------------------
// Serde contract tests — enums MUST serialize as snake_case strings
// ---------------------------------------------------------------------------

#[test]
fn serde_asr_provider_kind_snake_case() {
    assert_eq!(
        serde_json::to_string(&AsrProviderKind::SenseVoice).unwrap(),
        "\"sense_voice\""
    );
    assert_eq!(
        serde_json::to_string(&AsrProviderKind::Whisper).unwrap(),
        "\"whisper\""
    );
}

#[test]
fn serde_asr_provider_kind_roundtrip() {
    let json = serde_json::to_string(&AsrProviderKind::SenseVoice).unwrap();
    let parsed: AsrProviderKind = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, AsrProviderKind::SenseVoice);
}

#[test]
fn serde_whisper_task_snake_case() {
    assert_eq!(
        serde_json::to_string(&WhisperTask::Transcribe).unwrap(),
        "\"transcribe\""
    );
    assert_eq!(
        serde_json::to_string(&WhisperTask::Translate).unwrap(),
        "\"translate\""
    );
}

#[test]
fn serde_asr_provider_options_sense_voice() {
    let opts = AsrProviderOptions::SenseVoice { use_itn: true };
    let json = serde_json::to_string(&opts).unwrap();
    let parsed: AsrProviderOptions = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, opts);
}

#[test]
fn serde_asr_provider_options_whisper() {
    let opts = AsrProviderOptions::Whisper {
        task: WhisperTask::Translate,
    };
    let json = serde_json::to_string(&opts).unwrap();
    let parsed: AsrProviderOptions = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, opts);
}

#[test]
fn serde_asr_language_snake_case() {
    assert_eq!(serde_json::to_string(&AsrLanguage::Zh).unwrap(), "\"zh\"");
    assert_eq!(
        serde_json::to_string(&AsrLanguage::Auto).unwrap(),
        "\"auto\""
    );
}

#[test]
fn serde_asr_settings_roundtrip() {
    let settings = AsrSettings::whisper("whisper-base")
        .with_language(AsrLanguage::En)
        .with_threads(2);
    let json = serde_json::to_string(&settings).unwrap();
    let parsed: AsrSettings = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, settings);
}
