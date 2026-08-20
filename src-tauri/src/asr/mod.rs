//! LifeSub local ASR module.
//!
//! Provides the sherpa-onnx runtime version and build identity when the
//! `asr-runtime` feature is enabled. Future modules (settings, manifest,
//! model manager, audio, VAD, providers, jobs, service) will be added here.

pub mod model_lookup;
pub mod settings;

/// Returns the pinned sherpa-onnx runtime version string.
///
/// This is the upstream tag used for the static build, verified at compile
/// time by the crate build script and readable at runtime for evidence Receipts.
#[cfg(feature = "asr-runtime")]
pub fn runtime_version() -> &'static str {
    "1.13.5"
}

/// Returns the exact git commit SHA-1 of the sherpa-onnx runtime used for
/// this build, as recorded in the sherpa-onnx crate version metadata.
///
/// Combined with `runtime_version`, this provides a unique, immutable
/// identity for every Receipt to prove which runtime produced a transcript.
#[cfg(feature = "asr-runtime")]
pub fn runtime_git_sha1() -> &'static str {
    "3dc7c569f31ca2cd4a20ed6f7db780327e6714c5"
}
