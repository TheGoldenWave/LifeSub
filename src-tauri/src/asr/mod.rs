const PINNED_RUNTIME_GIT_SHA1_ABBREVIATED: &str = "3dc7c569";
const PINNED_RUNTIME_GIT_SHA1: &str = "3dc7c569f31ca2cd4a20ed6f7db780327e6714c5";

#[cfg(feature = "asr-runtime")]
pub fn runtime_version() -> &'static str {
    sherpa_onnx::version()
}

#[cfg(feature = "asr-runtime")]
pub fn runtime_git_sha1() -> &'static str {
    let runtime_git_sha1 = sherpa_onnx::git_sha1();
    assert_eq!(
        runtime_git_sha1, PINNED_RUNTIME_GIT_SHA1_ABBREVIATED,
        "sherpa-onnx runtime build does not match the pinned Git SHA1"
    );
    PINNED_RUNTIME_GIT_SHA1
}
