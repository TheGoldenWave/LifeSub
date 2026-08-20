use super::settings::{AsrLanguage, AsrProviderKind};

/// Minimal trait for model lookup used by settings validation.
///
/// Task 3 tests use a stub implementation; Task 5's static model manifest
/// provides the real implementation.
pub trait ModelLookup {
    /// The ASR provider this model belongs to.
    fn provider(&self) -> AsrProviderKind;

    /// The stable model identifier.
    fn model_id(&self) -> &str;

    /// Whether the model supports the given language for transcription.
    fn supports_language(&self, language: &AsrLanguage) -> bool;
}
