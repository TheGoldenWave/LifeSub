#[cfg(not(feature = "asr-runtime"))]
#[test]
fn asr_module_boundary_is_available_without_native_runtime() {
    assert!(!crate::asr::native_runtime_enabled());
}

#[cfg(feature = "asr-runtime")]
#[test]
fn sherpa_runtime_reports_the_pinned_build() {
    assert_eq!(crate::asr::runtime_version(), "1.13.5");
    assert_eq!(crate::asr::runtime_git_sha1(), "3dc7c569");
    assert_eq!(
        crate::asr::pinned_runtime_git_sha1(),
        "3dc7c569f31ca2cd4a20ed6f7db780327e6714c5"
    );

    let identity = crate::asr::verify_runtime_identity().unwrap();
    assert_eq!(identity.version, "1.13.5");
    assert_eq!(identity.observed_git_sha1, "3dc7c569");
    assert_eq!(
        identity.pinned_git_sha1,
        "3dc7c569f31ca2cd4a20ed6f7db780327e6714c5"
    );
}

#[cfg(feature = "asr-runtime")]
#[test]
fn runtime_identity_rejects_version_mismatch() {
    assert_eq!(
        crate::asr::verify_runtime_identity_values("1.13.4", "3dc7c569"),
        Err(crate::asr::RuntimeIdentityError::VersionMismatch {
            observed: "1.13.4".to_owned(),
            pinned: "1.13.5",
        })
    );
}

#[cfg(feature = "asr-runtime")]
#[test]
fn runtime_identity_rejects_noncanonical_git_sha1s() {
    for observed in ["3", "3d", "deadbeef"] {
        assert_eq!(
            crate::asr::verify_runtime_identity_values("1.13.5", observed),
            Err(crate::asr::RuntimeIdentityError::GitSha1Mismatch {
                observed: observed.to_owned(),
                pinned: "3dc7c569f31ca2cd4a20ed6f7db780327e6714c5",
            })
        );
    }
}
