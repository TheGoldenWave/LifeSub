pub mod job;
pub mod manifest;
pub mod model_lookup;
pub mod settings;

#[cfg(feature = "asr-runtime")]
pub fn runtime_version() -> &'static str { "1.13.5" }

#[cfg(feature = "asr-runtime")]
pub fn runtime_git_sha1() -> &'static str { "3dc7c569f31ca2cd4a20ed6f7db780327e6714c5" }
