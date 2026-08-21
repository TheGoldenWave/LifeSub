use lifesub_lib::desktop_runtime::{
    NativeAsrDesktopRuntimeFactory, NativeCaptureDesktopRuntimeFactory,
    ProductionDesktopRuntimeFactory,
};

fn assert_native_runtime_factory<T>()
where
    T: NativeCaptureDesktopRuntimeFactory + NativeAsrDesktopRuntimeFactory,
{
}

#[test]
#[ignore = "release gate"]
fn release_wiring_contract() {
    assert_native_runtime_factory::<ProductionDesktopRuntimeFactory>();
}
