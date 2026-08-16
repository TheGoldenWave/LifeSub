pub mod audio;
pub mod job;
pub mod manifest;
pub mod model_lookup;
pub mod model_manager;
pub mod provider;
pub mod qwen3_asr;
pub mod receipt;
pub mod runtime_qualifier;
pub mod sense_voice;
pub mod service;
pub mod settings;
pub mod vad;
pub mod whisper;

const PINNED_RUNTIME_VERSION: &str = "1.13.5";
#[cfg(feature = "asr-runtime")]
const PINNED_RUNTIME_GIT_SHA1_ABBREVIATED: &str = "3dc7c569";
const PINNED_RUNTIME_GIT_SHA1: &str = "3dc7c569f31ca2cd4a20ed6f7db780327e6714c5";
const PINNED_NATIVE_ARCHIVE_SHA256: &str =
    "339c8fc19bb4b26e118c80792bbc4546eb263040fac36ef0cc027ec29c756b44";
const PINNED_RUNTIME_BUILD_ID: &str = "sherpa-onnx-v1.13.5-osx-arm64-static-lib";
#[cfg(feature = "asr-runtime")]
const ATTESTED_NATIVE_ARCHIVE_SHA256: &str = env!("LIFESUB_SHERPA_ARCHIVE_SHA256");
#[cfg(feature = "asr-runtime")]
const ATTESTED_RUNTIME_BUILD_ID: &str = env!("LIFESUB_SHERPA_BUILD_ID");
#[cfg(feature = "asr-runtime")]
const ATTESTED_RUNTIME_VERIFIED: &str = env!("LIFESUB_SHERPA_VERIFIED");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PinnedSherpaRuntimeIdentity {
    pub version: &'static str,
    pub git_commit: &'static str,
    pub native_archive_sha256: &'static str,
    pub build_id: &'static str,
}

pub const fn pinned_sherpa_runtime_identity() -> PinnedSherpaRuntimeIdentity {
    PinnedSherpaRuntimeIdentity {
        version: PINNED_RUNTIME_VERSION,
        git_commit: PINNED_RUNTIME_GIT_SHA1,
        native_archive_sha256: PINNED_NATIVE_ARCHIVE_SHA256,
        build_id: PINNED_RUNTIME_BUILD_ID,
    }
}

#[cfg(feature = "asr-runtime")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeIdentity {
    pub(crate) version: String,
    pub(crate) observed_git_sha1: String,
    pub(crate) pinned_git_sha1: &'static str,
    pub(crate) native_archive_sha256: &'static str,
    pub(crate) build_id: &'static str,
}

#[cfg(feature = "asr-runtime")]
impl RuntimeIdentity {
    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn observed_git_sha1(&self) -> &str {
        &self.observed_git_sha1
    }

    pub const fn pinned_git_sha1(&self) -> &'static str {
        self.pinned_git_sha1
    }

    pub const fn native_archive_sha256(&self) -> &'static str {
        self.native_archive_sha256
    }

    pub const fn build_id(&self) -> &'static str {
        self.build_id
    }

    pub fn matches_pinned(&self, pinned: PinnedSherpaRuntimeIdentity) -> bool {
        self.version == pinned.version
            && (self.observed_git_sha1 == pinned.git_commit
                || self.observed_git_sha1 == PINNED_RUNTIME_GIT_SHA1_ABBREVIATED)
            && self.pinned_git_sha1 == pinned.git_commit
            && self.native_archive_sha256 == pinned.native_archive_sha256
            && self.build_id == pinned.build_id
    }
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
    NativeArchiveSha256Mismatch {
        observed: String,
        pinned: &'static str,
    },
    BuildIdMismatch {
        observed: String,
        pinned: &'static str,
    },
    BuildAttestationUnverified {
        observed: String,
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
    verify_runtime_identity_values_with_build(
        observed_version,
        observed_git_sha1,
        ATTESTED_NATIVE_ARCHIVE_SHA256,
        ATTESTED_RUNTIME_BUILD_ID,
        ATTESTED_RUNTIME_VERIFIED,
    )
}

#[cfg(feature = "asr-runtime")]
pub(crate) fn verify_runtime_identity_values_with_build(
    observed_version: &str,
    observed_git_sha1: &str,
    native_archive_sha256: &str,
    build_id: &str,
    verified: &str,
) -> Result<RuntimeIdentity, RuntimeIdentityError> {
    if verified != "1" {
        return Err(RuntimeIdentityError::BuildAttestationUnverified {
            observed: verified.to_owned(),
        });
    }
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

    if native_archive_sha256 != PINNED_NATIVE_ARCHIVE_SHA256 {
        return Err(RuntimeIdentityError::NativeArchiveSha256Mismatch {
            observed: native_archive_sha256.to_owned(),
            pinned: PINNED_NATIVE_ARCHIVE_SHA256,
        });
    }

    if build_id != PINNED_RUNTIME_BUILD_ID {
        return Err(RuntimeIdentityError::BuildIdMismatch {
            observed: build_id.to_owned(),
            pinned: PINNED_RUNTIME_BUILD_ID,
        });
    }

    Ok(RuntimeIdentity {
        version: observed_version.to_owned(),
        observed_git_sha1: observed_git_sha1.to_owned(),
        pinned_git_sha1: PINNED_RUNTIME_GIT_SHA1,
        // Task 1's trusted wrapper verifies this archive marker before a forced scoped rebuild.
        native_archive_sha256: ATTESTED_NATIVE_ARCHIVE_SHA256,
        build_id: ATTESTED_RUNTIME_BUILD_ID,
    })
}
