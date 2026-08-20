//! LifeSub local ASR module.

pub mod audio;
pub mod job;
pub mod manifest;
pub mod model_lookup;
pub mod model_manager;
// pub mod provider;  // TODO: Task 8
// #[cfg(feature = "asr-runtime")]
// pub mod sense_voice;
pub mod service;
pub mod settings;
pub mod vad;
// #[cfg(feature = "asr-runtime")]
// pub mod whisper;

#[cfg(feature = "asr-runtime")]
pub fn runtime_version() -> &'static str { "1.13.5" }
#[cfg(feature = "asr-runtime")]
pub fn runtime_git_sha1() -> &'static str { "3dc7c569f31ca2cd4a20ed6f7db780327e6714c5" }
