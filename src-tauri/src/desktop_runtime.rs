use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::thread::JoinHandle;

use crate::capture::StreamingCapture;
use crate::service::CoreRuntime;

mod sealed {
    pub trait DesktopRuntimeFactory {}
    pub trait NativeCapture {}
    pub trait NativeAsr {}
}

pub trait DesktopRuntimeFactory: sealed::DesktopRuntimeFactory {
    fn create_capture() -> StreamingCapture;
    fn spawn_worker(runtime: Arc<CoreRuntime>) -> (Arc<AtomicBool>, Option<JoinHandle<()>>);
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

pub struct FailClosedDesktopRuntimeFactory;

impl sealed::DesktopRuntimeFactory for FailClosedDesktopRuntimeFactory {}

impl DesktopRuntimeFactory for FailClosedDesktopRuntimeFactory {
    fn create_capture() -> StreamingCapture {
        StreamingCapture::default()
    }

    fn spawn_worker(runtime: Arc<CoreRuntime>) -> (Arc<AtomicBool>, Option<JoinHandle<()>>) {
        #[cfg(not(test))]
        {
            let (shutdown, handle) = crate::asr::worker::spawn_fail_closed_worker(runtime);
            (shutdown, Some(handle))
        }
        #[cfg(test)]
        {
            let _ = runtime;
            (Arc::new(AtomicBool::new(false)), None)
        }
    }
}

pub type ProductionDesktopRuntimeFactory = FailClosedDesktopRuntimeFactory;
