use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::thread::JoinHandle;

use crate::asr::worker::WorkerCallbacks;
use crate::service::CoreRuntime;

mod sealed {
    pub trait DesktopRuntimeFactory {}
    pub trait NativeCapture {}
    pub trait NativeAsr {}
}

pub trait DesktopRuntimeFactory: sealed::DesktopRuntimeFactory {
    fn spawn_worker(
        runtime: Arc<CoreRuntime>,
        callbacks: WorkerCallbacks,
    ) -> (Arc<AtomicBool>, Option<JoinHandle<()>>);
}

pub trait NativeCaptureDesktopRuntimeFactory:
    DesktopRuntimeFactory + sealed::NativeCapture
{
}

impl<T> NativeCaptureDesktopRuntimeFactory for T where
    T: DesktopRuntimeFactory + sealed::NativeCapture
{
}

pub trait NativeAsrDesktopRuntimeFactory: DesktopRuntimeFactory + sealed::NativeAsr {}

impl<T> NativeAsrDesktopRuntimeFactory for T where T: DesktopRuntimeFactory + sealed::NativeAsr {}

pub struct NativeCaptureRuntimeFactory;

impl sealed::DesktopRuntimeFactory for NativeCaptureRuntimeFactory {}
impl sealed::NativeCapture for NativeCaptureRuntimeFactory {}
impl sealed::NativeAsr for NativeCaptureRuntimeFactory {}

impl DesktopRuntimeFactory for NativeCaptureRuntimeFactory {
    fn spawn_worker(
        runtime: Arc<CoreRuntime>,
        callbacks: WorkerCallbacks,
    ) -> (Arc<AtomicBool>, Option<JoinHandle<()>>) {
        #[cfg(not(test))]
        {
            let (shutdown, handle) =
                crate::asr::worker::spawn_native_worker_with_callbacks(runtime, callbacks);
            (shutdown, Some(handle))
        }
        #[cfg(test)]
        {
            let _ = (runtime, callbacks);
            (Arc::new(AtomicBool::new(false)), None)
        }
    }
}

pub type ProductionDesktopRuntimeFactory = NativeCaptureRuntimeFactory;
