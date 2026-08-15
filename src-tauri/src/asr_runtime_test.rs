#[cfg(not(feature = "asr-runtime"))]
#[test]
fn asr_module_boundary_is_available_without_native_runtime() {
    assert!(!crate::asr::native_runtime_enabled());
}

#[cfg(feature = "asr-runtime")]
#[test]
fn sherpa_runtime_reports_the_pinned_build() {
    assert_eq!(crate::asr::runtime_version(), "1.13.5");
    assert_eq!(
        crate::asr::runtime_git_sha1(),
        "3dc7c569f31ca2cd4a20ed6f7db780327e6714c5"
    );
}
