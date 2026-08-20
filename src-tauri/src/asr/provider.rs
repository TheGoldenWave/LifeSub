//! ASR provider trait and shared types.
//!
//! Providers receive validated PCM audio slices and return transcript text.
//! They do not read settings, select fallback models, write SQLite, or assign
//! revision numbers. The trait is object-safe and `Send` so it can be used
//! across threads.

use std::sync::atomic::{AtomicBool, Ordering};

use super::settings::{AsrLanguage, AsrProviderKind, AsrProviderOptions};

// ---------------------------------------------------------------------------
// Audio slice — the input to a provider
// ---------------------------------------------------------------------------

/// A slice of `f32` mono PCM audio at the provider's expected sample rate.
///
/// The audio has already been decoded, downmixed to mono, and resampled by
/// `asr::audio`. The provider does not perform further audio processing.
#[derive(Clone, Copy)]
pub struct AudioSlice<'a> {
    /// Mono `f32` PCM samples in `[-1.0, 1.0]`.
    pub samples: &'a [f32],
    /// Sample rate in Hz (e.g. 16000).
    pub sample_rate: u32,
}

// ---------------------------------------------------------------------------
// ASR request — validated parameters for a single transcription
// ---------------------------------------------------------------------------

/// A validated transcription request.
///
/// The language and options have already been validated against the model
/// manifest by the caller. The provider should trust these values.
#[derive(Clone, Debug)]
pub struct AsrRequest {
    /// The language to transcribe in.
    pub language: AsrLanguage,
    /// Provider-specific options (ITN for SenseVoice, task for Whisper).
    pub options: AsrProviderOptions,
    /// Number of threads for the inference engine.
    pub num_threads: u16,
}

// ---------------------------------------------------------------------------
// ASR text result
// ---------------------------------------------------------------------------

/// The result of a successful transcription.
///
/// The text is the raw output from the model. Upstream consumers may apply
/// post-processing (e.g. ITN for SenseVoice) before persisting.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AsrText {
    /// The transcribed text.
    pub text: String,
}

// ---------------------------------------------------------------------------
// Provider identity
// ---------------------------------------------------------------------------

/// Immutable identity of a provider instance.
///
/// This is recorded in every `ProviderReceipt` to establish exactly which
/// model and runtime version produced the transcript.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderIdentity {
    /// The provider kind (SenseVoice or Whisper).
    pub kind: AsrProviderKind,
    /// The stable model identifier from the manifest.
    pub model_id: String,
    /// The sherpa-onnx runtime version (e.g. "1.13.5").
    pub runtime_version: String,
    /// The sherpa-onnx runtime build identity (git SHA-1).
    pub runtime_build_id: String,
}

// ---------------------------------------------------------------------------
// Cancellation token
// ---------------------------------------------------------------------------

/// A lightweight cancellation token for synchronous inference.
///
/// The provider checks this token before processing each audio window.
/// The caller sets the token from another thread to request cancellation.
/// Since native inference is synchronous, cancellation is only checked
/// between windows, not mid-inference.
#[derive(Debug)]
pub struct CancellationToken {
    cancelled: AtomicBool,
}

impl CancellationToken {
    /// Creates a new token in the non-cancelled state.
    pub fn new() -> Self {
        Self {
            cancelled: AtomicBool::new(false),
        }
    }

    /// Signals cancellation. Safe to call from any thread.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    /// Returns whether cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

// Safety: CancellationToken is safe to share across threads.
// The inner AtomicBool provides the necessary synchronization.
unsafe impl Send for CancellationToken {}
unsafe impl Sync for CancellationToken {}

// ---------------------------------------------------------------------------
// ASR error
// ---------------------------------------------------------------------------

/// ASR provider errors with stable error codes.
///
/// Each variant maps to a stable `AsrErrorCode` for persistence.
/// The error messages are for diagnostics and should not be shown
/// directly to users without mapping.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AsrError {
    /// The requested model is not installed.
    ModelNotInstalled {
        model_id: String,
    },
    /// The installed model files are corrupt or incomplete.
    ModelIntegrityFailed {
        model_id: String,
        reason: String,
    },
    /// A provider parameter is invalid (e.g. wrong options variant).
    InvalidProviderParameter {
        reason: String,
    },
    /// The provider failed to initialize (e.g. ONNX model load error).
    ProviderInitializationFailed {
        reason: String,
    },
    /// The transcription engine returned an error.
    TranscriptionFailed {
        reason: String,
    },
    /// The user requested cancellation.
    Cancelled,
    /// The model returned empty or whitespace-only output.
    EmptyOutput,
}

// ---------------------------------------------------------------------------
// ASR provider trait
// ---------------------------------------------------------------------------

/// The ASR provider trait.
///
/// Implementations wrap a specific model (SenseVoice or Whisper) and produce
/// transcript text from audio slices. Providers are `Send` so they can be
/// moved to a background thread for inference.
///
/// # Contract
///
/// - `identity()` MUST return the same reference for the lifetime of the
///   provider. The caller may rely on pointer equality.
/// - `transcribe()` MUST check the cancellation token before processing
///   each audio window and return `AsrError::Cancelled` if it is set.
/// - `transcribe()` MUST return `AsrError::EmptyOutput` if the model
///   produces an empty or whitespace-only text result.
/// - Providers MUST NOT read settings, select fallback models, write to
///   SQLite, or assign revision numbers.
pub trait AsrProvider: Send {
    /// Returns the immutable identity of this provider.
    fn identity(&self) -> &ProviderIdentity;

    /// Transcribes the given audio slice.
    ///
    /// # Arguments
    ///
    /// * `audio` - The decoded, mono, resampled PCM audio.
    /// * `request` - The validated transcription parameters.
    /// * `cancellation` - A token checked before each window.
    ///
    /// # Errors
    ///
    /// Returns `AsrError::Cancelled` if cancellation was requested.
    /// Returns `AsrError::EmptyOutput` if the model produced no text.
    fn transcribe(
        &self,
        audio: AudioSlice<'_>,
        request: &AsrRequest,
        cancellation: &CancellationToken,
    ) -> Result<AsrText, AsrError>;
}