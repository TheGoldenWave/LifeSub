//! Fake-provider contract tests for the ASR provider trait.
//!
//! These tests validate the provider interface without loading native models:
//! - Provider/model identity
//! - Language mapping
//! - SenseVoice ITN (inverse text normalization)
//! - Whisper task (transcribe vs translate)
//! - Empty-output rejection
//! - Cancellation between windows
//! - Error mapping
//!
//! Fake providers return deterministic text so tests are fast and repeatable.

use crate::asr::provider::{
    AsrError, AsrProvider, AsrRequest, AsrText, AudioSlice, CancellationToken, ProviderIdentity,
};
use crate::asr::settings::{AsrLanguage, AsrProviderKind, AsrProviderOptions, WhisperTask};

// ---------------------------------------------------------------------------
// Fake providers for contract testing
// ---------------------------------------------------------------------------

/// A fake SenseVoice provider that returns preset text.
/// Respects the ITN setting: when ITN is on, the output includes "[ITN]" marker.
struct FakeSenseVoiceProvider {
    identity: ProviderIdentity,
}

impl FakeSenseVoiceProvider {
    fn new() -> Self {
        Self {
            identity: ProviderIdentity {
                kind: AsrProviderKind::SenseVoice,
                model_id: "sense-voice-small-int8-2024-07-17".to_string(),
                runtime_version: "1.13.5".to_string(),
                runtime_build_id: "3dc7c569f31ca2cd4a20ed6f7db780327e6714c5".to_string(),
            },
        }
    }
}

impl AsrProvider for FakeSenseVoiceProvider {
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

        let use_itn = match &request.options {
            AsrProviderOptions::SenseVoice { use_itn } => *use_itn,
            _ => {
                return Err(AsrError::InvalidProviderParameter {
                    reason: "expected SenseVoice options".to_string(),
                })
            }
        };

        let text = if use_itn {
            // ITN transforms numeric text: "一百二十三" -> "123"
            format!("[ITN] 今天气温二十三点五度，采样率{}", audio.sample_rate)
        } else {
            format!("今天气温二十三点五度，采样率{}", audio.sample_rate)
        };

        Ok(AsrText { text })
    }
}

/// A fake Whisper provider that returns preset text.
/// Respects the task setting: translate produces English output.
struct FakeWhisperProvider {
    identity: ProviderIdentity,
}

impl FakeWhisperProvider {
    fn new() -> Self {
        Self {
            identity: ProviderIdentity {
                kind: AsrProviderKind::Whisper,
                model_id: "whisper-base".to_string(),
                runtime_version: "1.13.5".to_string(),
                runtime_build_id: "3dc7c569f31ca2cd4a20ed6f7db780327e6714c5".to_string(),
            },
        }
    }
}

impl AsrProvider for FakeWhisperProvider {
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

        let task = match &request.options {
            AsrProviderOptions::Whisper { task } => *task,
            _ => {
                return Err(AsrError::InvalidProviderParameter {
                    reason: "expected Whisper options".to_string(),
                })
            }
        };

        let text = match task {
            WhisperTask::Transcribe => {
                format!(
                    "This is a transcription of audio at {} Hz",
                    audio.sample_rate
                )
            }
            WhisperTask::Translate => {
                format!(
                    "[TRANSLATE] This is a translation at {} Hz",
                    audio.sample_rate
                )
            }
        };

        Ok(AsrText { text })
    }
}

// ---------------------------------------------------------------------------
// Helper to create a non-empty audio slice for tests
// ---------------------------------------------------------------------------

fn audio_slice(samples: &[f32]) -> AudioSlice<'_> {
    AudioSlice {
        samples,
        sample_rate: 16000,
    }
}

fn non_empty_audio() -> Vec<f32> {
    vec![0.0_f32; 16000] // 1 second of silence at 16 kHz
}

// ---------------------------------------------------------------------------
// 1. Provider/model identity
// ---------------------------------------------------------------------------

#[test]
fn sense_voice_provider_reports_correct_identity() {
    let provider = FakeSenseVoiceProvider::new();
    let id = provider.identity();
    assert_eq!(id.kind, AsrProviderKind::SenseVoice);
    assert_eq!(id.model_id, "sense-voice-small-int8-2024-07-17");
    assert_eq!(id.runtime_version, "1.13.5");
    assert_eq!(
        id.runtime_build_id,
        "3dc7c569f31ca2cd4a20ed6f7db780327e6714c5"
    );
}

#[test]
fn whisper_provider_reports_correct_identity() {
    let provider = FakeWhisperProvider::new();
    let id = provider.identity();
    assert_eq!(id.kind, AsrProviderKind::Whisper);
    assert_eq!(id.model_id, "whisper-base");
    assert_eq!(id.runtime_version, "1.13.5");
    assert_eq!(
        id.runtime_build_id,
        "3dc7c569f31ca2cd4a20ed6f7db780327e6714c5"
    );
}

#[test]
fn identity_is_stable_across_transcriptions() {
    let provider = FakeSenseVoiceProvider::new();
    let id1 = provider.identity() as *const ProviderIdentity;
    let id2 = provider.identity() as *const ProviderIdentity;
    assert_eq!(id1, id2, "identity pointer must be stable");
}

// ---------------------------------------------------------------------------
// 2. Language mapping
// ---------------------------------------------------------------------------

#[test]
fn sense_voice_language_zh_passes_to_provider() {
    let provider = FakeSenseVoiceProvider::new();
    let audio = non_empty_audio();
    let request = AsrRequest {
        language: AsrLanguage::Zh,
        options: AsrProviderOptions::SenseVoice { use_itn: false },
        num_threads: 4,
    };
    let token = CancellationToken::new();
    let result = provider
        .transcribe(audio_slice(&audio), &request, &token)
        .unwrap();
    // The fake provider includes the sample rate, so we verify output is
    // non-empty and contains expected language-specific content.
    assert!(!result.text.is_empty());
    assert!(result.text.contains("采样率"));
}

#[test]
fn whisper_language_en_passes_to_provider() {
    let provider = FakeWhisperProvider::new();
    let audio = non_empty_audio();
    let request = AsrRequest {
        language: AsrLanguage::En,
        options: AsrProviderOptions::Whisper {
            task: WhisperTask::Transcribe,
        },
        num_threads: 2,
    };
    let token = CancellationToken::new();
    let result = provider
        .transcribe(audio_slice(&audio), &request, &token)
        .unwrap();
    assert!(!result.text.is_empty());
    assert!(result.text.contains("transcription"));
}

#[test]
fn auto_language_accepted_by_provider() {
    let provider = FakeWhisperProvider::new();
    let audio = non_empty_audio();
    let request = AsrRequest {
        language: AsrLanguage::Auto,
        options: AsrProviderOptions::Whisper {
            task: WhisperTask::Transcribe,
        },
        num_threads: 2,
    };
    let token = CancellationToken::new();
    let result = provider.transcribe(audio_slice(&audio), &request, &token);
    assert!(result.is_ok(), "auto language should be accepted");
}

// ---------------------------------------------------------------------------
// 3. SenseVoice ITN (inverse text normalization)
// ---------------------------------------------------------------------------

#[test]
fn sense_voice_with_itn_enabled() {
    let provider = FakeSenseVoiceProvider::new();
    let audio = non_empty_audio();
    let request = AsrRequest {
        language: AsrLanguage::Zh,
        options: AsrProviderOptions::SenseVoice { use_itn: true },
        num_threads: 4,
    };
    let token = CancellationToken::new();
    let result = provider
        .transcribe(audio_slice(&audio), &request, &token)
        .unwrap();
    // ITN output should contain the [ITN] marker (fake provider convention)
    assert!(
        result.text.contains("[ITN]"),
        "ITN output should contain ITN marker, got: {}",
        result.text
    );
}

#[test]
fn sense_voice_without_itn() {
    let provider = FakeSenseVoiceProvider::new();
    let audio = non_empty_audio();
    let request = AsrRequest {
        language: AsrLanguage::Zh,
        options: AsrProviderOptions::SenseVoice { use_itn: false },
        num_threads: 4,
    };
    let token = CancellationToken::new();
    let result = provider
        .transcribe(audio_slice(&audio), &request, &token)
        .unwrap();
    assert!(
        !result.text.contains("[ITN]"),
        "non-ITN output should not contain ITN marker, got: {}",
        result.text
    );
}

// ---------------------------------------------------------------------------
// 4. Whisper task
// ---------------------------------------------------------------------------

#[test]
fn whisper_transcribe_task() {
    let provider = FakeWhisperProvider::new();
    let audio = non_empty_audio();
    let request = AsrRequest {
        language: AsrLanguage::En,
        options: AsrProviderOptions::Whisper {
            task: WhisperTask::Transcribe,
        },
        num_threads: 2,
    };
    let token = CancellationToken::new();
    let result = provider
        .transcribe(audio_slice(&audio), &request, &token)
        .unwrap();
    assert!(result.text.contains("transcription"));
    assert!(
        !result.text.contains("[TRANSLATE]"),
        "transcribe should not contain translate marker"
    );
}

#[test]
fn whisper_translate_task() {
    let provider = FakeWhisperProvider::new();
    let audio = non_empty_audio();
    let request = AsrRequest {
        language: AsrLanguage::En,
        options: AsrProviderOptions::Whisper {
            task: WhisperTask::Translate,
        },
        num_threads: 2,
    };
    let token = CancellationToken::new();
    let result = provider
        .transcribe(audio_slice(&audio), &request, &token)
        .unwrap();
    assert!(result.text.contains("[TRANSLATE]"));
    assert!(result.text.contains("translation"));
}

// ---------------------------------------------------------------------------
// 5. Empty-output rejection
// ---------------------------------------------------------------------------

#[test]
fn sense_voice_empty_audio_rejected() {
    let provider = FakeSenseVoiceProvider::new();
    let request = AsrRequest {
        language: AsrLanguage::Zh,
        options: AsrProviderOptions::SenseVoice { use_itn: true },
        num_threads: 4,
    };
    let token = CancellationToken::new();
    let result = provider.transcribe(audio_slice(&[]), &request, &token);
    assert_eq!(result, Err(AsrError::EmptyOutput));
}

#[test]
fn whisper_empty_audio_rejected() {
    let provider = FakeWhisperProvider::new();
    let request = AsrRequest {
        language: AsrLanguage::En,
        options: AsrProviderOptions::Whisper {
            task: WhisperTask::Transcribe,
        },
        num_threads: 2,
    };
    let token = CancellationToken::new();
    let result = provider.transcribe(audio_slice(&[]), &request, &token);
    assert_eq!(result, Err(AsrError::EmptyOutput));
}

#[test]
fn empty_output_after_transcription_is_rejected() {
    // The fake provider already rejects empty audio before inference,
    // but this test documents that the real provider must also reject
    // cases where the model returns an empty or whitespace-only string.
    let provider = FakeSenseVoiceProvider::new();
    let request = AsrRequest {
        language: AsrLanguage::Zh,
        options: AsrProviderOptions::SenseVoice { use_itn: true },
        num_threads: 4,
    };
    let token = CancellationToken::new();
    // Non-empty audio should produce non-empty output
    let result = provider
        .transcribe(audio_slice(&non_empty_audio()), &request, &token)
        .unwrap();
    assert!(!result.text.trim().is_empty());
}

// ---------------------------------------------------------------------------
// 6. Cancellation between windows
// ---------------------------------------------------------------------------

#[test]
fn cancellation_before_transcription_returns_cancelled() {
    let provider = FakeSenseVoiceProvider::new();
    let audio = non_empty_audio();
    let request = AsrRequest {
        language: AsrLanguage::Zh,
        options: AsrProviderOptions::SenseVoice { use_itn: true },
        num_threads: 4,
    };
    let token = CancellationToken::new();
    token.cancel();
    let result = provider.transcribe(audio_slice(&audio), &request, &token);
    assert_eq!(result, Err(AsrError::Cancelled));
}

#[test]
fn cancellation_checked_before_each_window() {
    // The fake provider checks cancellation at the start of transcribe().
    // Real providers must check before each VAD window.
    let provider = FakeWhisperProvider::new();
    let audio = non_empty_audio();
    let request = AsrRequest {
        language: AsrLanguage::En,
        options: AsrProviderOptions::Whisper {
            task: WhisperTask::Transcribe,
        },
        num_threads: 2,
    };
    let token = CancellationToken::new();
    token.cancel();
    let result = provider.transcribe(audio_slice(&audio), &request, &token);
    assert_eq!(result, Err(AsrError::Cancelled));
}

#[test]
fn cancellation_token_is_independent_per_provider_call() {
    let provider = FakeSenseVoiceProvider::new();
    let audio = non_empty_audio();
    let request = AsrRequest {
        language: AsrLanguage::Zh,
        options: AsrProviderOptions::SenseVoice { use_itn: false },
        num_threads: 4,
    };

    // First call: not cancelled
    let token1 = CancellationToken::new();
    let result1 = provider.transcribe(audio_slice(&audio), &request, &token1);
    assert!(result1.is_ok());

    // Second call: cancelled
    let token2 = CancellationToken::new();
    token2.cancel();
    let result2 = provider.transcribe(audio_slice(&audio), &request, &token2);
    assert_eq!(result2, Err(AsrError::Cancelled));

    // Third call: new token, not cancelled
    let token3 = CancellationToken::new();
    let result3 = provider.transcribe(audio_slice(&audio), &request, &token3);
    assert!(result3.is_ok());
}

// ---------------------------------------------------------------------------
// 7. Error mapping
// ---------------------------------------------------------------------------

#[test]
fn sense_voice_wrong_options_variant_returns_error() {
    let provider = FakeSenseVoiceProvider::new();
    let audio = non_empty_audio();
    let request = AsrRequest {
        language: AsrLanguage::Zh,
        // Whisper options on a SenseVoice provider
        options: AsrProviderOptions::Whisper {
            task: WhisperTask::Transcribe,
        },
        num_threads: 4,
    };
    let token = CancellationToken::new();
    let result = provider.transcribe(audio_slice(&audio), &request, &token);
    assert!(matches!(
        result,
        Err(AsrError::InvalidProviderParameter { .. })
    ));
}

#[test]
fn whisper_wrong_options_variant_returns_error() {
    let provider = FakeWhisperProvider::new();
    let audio = non_empty_audio();
    let request = AsrRequest {
        language: AsrLanguage::En,
        // SenseVoice options on a Whisper provider
        options: AsrProviderOptions::SenseVoice { use_itn: true },
        num_threads: 2,
    };
    let token = CancellationToken::new();
    let result = provider.transcribe(audio_slice(&audio), &request, &token);
    assert!(matches!(
        result,
        Err(AsrError::InvalidProviderParameter { .. })
    ));
}

#[test]
fn asr_error_is_send() {
    // AsrError must be Send so providers can be used across threads.
    fn assert_send<T: Send>() {}
    assert_send::<AsrError>();
}

#[test]
fn asr_text_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<AsrText>();
}

#[test]
fn provider_identity_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ProviderIdentity>();
}

#[test]
fn cancellation_token_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<CancellationToken>();
}

#[test]
fn audio_slice_is_copy() {
    // AudioSlice should be Copy so it can be passed by value.
    fn assert_copy<T: Copy>() {}
    assert_copy::<AudioSlice<'_>>();
}

// ---------------------------------------------------------------------------
// 8. AsrRequest covers all required fields
// ---------------------------------------------------------------------------

#[test]
fn asr_request_carries_language_options_and_threads() {
    let request = AsrRequest {
        language: AsrLanguage::Zh,
        options: AsrProviderOptions::SenseVoice { use_itn: true },
        num_threads: 4,
    };
    assert_eq!(request.language, AsrLanguage::Zh);
    assert_eq!(request.num_threads, 4);
    assert!(matches!(
        request.options,
        AsrProviderOptions::SenseVoice { use_itn: true }
    ));
}

// ---------------------------------------------------------------------------
// 9. Real provider identity contract (when asr-runtime is available)
// ---------------------------------------------------------------------------

#[cfg(feature = "asr-runtime")]
#[test]
fn runtime_version_matches_pinned_build() {
    // When asr-runtime is enabled, the runtime version and build identity
    // must match the pinned values from mod.rs.
    assert_eq!(crate::asr::runtime_version(), "1.13.5");
    assert_eq!(
        crate::asr::runtime_git_sha1(),
        "3dc7c569f31ca2cd4a20ed6f7db780327e6714c5"
    );
}