pub mod model_lookup;
pub mod receipt;
pub mod settings;

#[cfg(feature = "asr-runtime")]
const PINNED_RUNTIME_VERSION: &str = "1.13.5";
#[cfg(feature = "asr-runtime")]
const PINNED_RUNTIME_GIT_SHA1_ABBREVIATED: &str = "3dc7c569";
#[cfg(feature = "asr-runtime")]
const PINNED_RUNTIME_GIT_SHA1: &str = "3dc7c569f31ca2cd4a20ed6f7db780327e6714c5";

#[cfg(feature = "asr-runtime")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeIdentity {
    pub version: String,
    pub observed_git_sha1: String,
    pub pinned_git_sha1: &'static str,
}

#[cfg(feature = "asr-runtime")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeIdentityError {
    VersionMismatch {
        observed: String,
        pinned: &'static str,
    },
    GitSha1Mismatch {
        observed: String,
        pinned: &'static str,
    },
}

pub const fn native_runtime_enabled() -> bool {
    cfg!(feature = "asr-runtime")
}

#[cfg(feature = "asr-runtime")]
pub fn runtime_version() -> &'static str {
    sherpa_onnx::version()
}

#[cfg(feature = "asr-runtime")]
pub fn runtime_git_sha1() -> &'static str {
    sherpa_onnx::git_sha1()
}

#[cfg(feature = "asr-runtime")]
pub const fn pinned_runtime_git_sha1() -> &'static str {
    PINNED_RUNTIME_GIT_SHA1
}

#[cfg(feature = "asr-runtime")]
pub fn verify_runtime_identity() -> Result<RuntimeIdentity, RuntimeIdentityError> {
    verify_runtime_identity_values(runtime_version(), runtime_git_sha1())
}

#[cfg(feature = "asr-runtime")]
pub(crate) fn verify_runtime_identity_values(
    observed_version: &str,
    observed_git_sha1: &str,
) -> Result<RuntimeIdentity, RuntimeIdentityError> {
    if observed_version != PINNED_RUNTIME_VERSION {
        return Err(RuntimeIdentityError::VersionMismatch {
            observed: observed_version.to_owned(),
            pinned: PINNED_RUNTIME_VERSION,
        });
    }

    if observed_git_sha1 != PINNED_RUNTIME_GIT_SHA1_ABBREVIATED
        && observed_git_sha1 != PINNED_RUNTIME_GIT_SHA1
    {
        return Err(RuntimeIdentityError::GitSha1Mismatch {
            observed: observed_git_sha1.to_owned(),
            pinned: PINNED_RUNTIME_GIT_SHA1,
        });
    }

    Ok(RuntimeIdentity {
        version: observed_version.to_owned(),
        observed_git_sha1: observed_git_sha1.to_owned(),
        pinned_git_sha1: PINNED_RUNTIME_GIT_SHA1,
    })
}
