use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::thread::JoinHandle;

use crate::capture::StreamingCapture;
use crate::service::CoreRuntime;

pub trait DesktopRuntimeFactory {
    const USES_NATIVE_CAPTURE: bool;
    const USES_NATIVE_ASR: bool;

    fn create_capture() -> StreamingCapture;
    fn spawn_worker(runtime: Arc<CoreRuntime>) -> (Arc<AtomicBool>, Option<JoinHandle<()>>);
}

pub struct FailClosedDesktopRuntimeFactory;

impl DesktopRuntimeFactory for FailClosedDesktopRuntimeFactory {
    const USES_NATIVE_CAPTURE: bool = false;
    const USES_NATIVE_ASR: bool = false;

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
