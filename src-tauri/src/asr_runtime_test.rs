/// Runtime version tests verify that the pinned sherpa-onnx 1.13.5 static build
/// reports the correct version and git commit SHA-1 at runtime.
/// These tests are gated behind the `asr-runtime` feature because they depend on
/// the native sherpa-onnx library being linked.

#[cfg(feature = "asr-runtime")]
#[test]
fn sherpa_runtime_reports_the_pinned_build() {
    assert_eq!(crate::asr::runtime_version(), "1.13.5");
    assert_eq!(
        crate::asr::runtime_git_sha1(),
        "3dc7c569f31ca2cd4a20ed6f7db780327e6714c5"
    );
}
