use lifesub_lib::desktop_runtime::{DesktopRuntimeFactory, ProductionDesktopRuntimeFactory};

#[test]
#[ignore = "release gate"]
fn release_wiring_contract() {
    assert!(
        ProductionDesktopRuntimeFactory::USES_NATIVE_CAPTURE,
        "production desktop runtime must select native capture"
    );
    assert!(
        ProductionDesktopRuntimeFactory::USES_NATIVE_ASR,
        "production desktop runtime must select native ASR"
    );
}
