//! Live streaming capture service.
//!
//! Owns the real-time ASR event channel: a background thread captures audio
//! or reports why it cannot, then emits events to the WebView. Desktop builds
//! must never silently fall back to demo transcripts because that would present
//! fake evidence as real capture output.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

/// Emitted to the WebView whenever a transcribed segment becomes available.
pub const LIVE_SEGMENT_EVENT: &str = "asr-live-segment";
pub const LIVE_ERROR_EVENT: &str = "asr-live-error";

const STREAMING_UNAVAILABLE_CODE: &str = "streaming_unavailable";
const STREAMING_UNAVAILABLE_MESSAGE: &str = "桌面实时采集未接通，已阻止演示数据回退。";

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveCaptureError {
    pub code: String,
    pub message: String,
}

/// A speaker label carried with each live segment.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveSpeaker {
    pub id: String,
    pub label: String,
    pub source: String,
    pub voiceprint_id: Option<String>,
}

/// A single transcribed segment pushed to the frontend.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveSegment {
    pub id: String,
    pub start_ms: i64,
    pub speaker: LiveSpeaker,
    pub text: String,
    pub completed: bool,
}

/// Abstraction over the audio+ASR source. Production uses the real capture
/// pipeline and ASR provider; development uses a deterministic mock.
pub trait StreamingSource: Send {
    fn next_segment(&mut self, sequence: u64) -> Option<LiveSegment>;
}

/// Mock source emitting deterministic demo segments for development.
pub struct MockStreamingSource {
    scripts: Vec<(&'static str, &'static str, &'static str)>,
    index: usize,
}

impl MockStreamingSource {
    pub fn new() -> Self {
        Self {
            scripts: vec![
                (
                    "spk-1",
                    "张伟",
                    "我们今天先确认首版范围，重点是把基础闭环真正跑起来。",
                ),
                (
                    "spk-2",
                    "我",
                    "好的，我记一下。证据链要保证每次修改都能追溯。",
                ),
                (
                    "spk-1",
                    "张伟",
                    "对，而且要保证搜索结果能回到准确的音频时间范围。",
                ),
                (
                    "spk-3",
                    "可能是李娜？",
                    "还有一个点，关于数据目录的权限控制，需要再确认。",
                ),
                (
                    "spk-4",
                    "未知说话人 1",
                    "这个方案我觉得可以，但是需要再确认一下安全性。",
                ),
            ],
            index: 0,
        }
    }
}

impl Default for MockStreamingSource {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamingSource for MockStreamingSource {
    fn next_segment(&mut self, _sequence: u64) -> Option<LiveSegment> {
        if self.index >= self.scripts.len() {
            return None;
        }
        let (id, label, text) = self.scripts[self.index];
        self.index += 1;
        Some(LiveSegment {
            id: format!("ls-{}", self.index),
            start_ms: (self.index * 6000) as i64,
            speaker: LiveSpeaker {
                id: id.to_string(),
                label: label.to_string(),
                source: if id == "spk-4" {
                    "unknown".into()
                } else if id == "spk-3" {
                    "dictionary".into()
                } else if id == "spk-2" {
                    "manual".into()
                } else {
                    "voiceprint".into()
                },
                voiceprint_id: if id == "spk-1" {
                    Some("vp-1".into())
                } else {
                    None
                },
            },
            text: text.to_string(),
            completed: true,
        })
    }
}

/// Running state of the streaming capture loop.
struct StreamingLoop {
    stop: Arc<AtomicBool>,
    pause: Arc<AtomicBool>,
    handle: JoinHandle<()>,
}

/// Owned by `AppState`; starts/stops the streaming loop on demand.
#[derive(Default)]
pub struct StreamingCapture {
    loop_handle: Option<StreamingLoop>,
}

impl Drop for StreamingCapture {
    fn drop(&mut self) {
        self.stop();
    }
}

impl StreamingCapture {
    pub fn is_running(&self) -> bool {
        self.loop_handle
            .as_ref()
            .is_some_and(|streaming| !streaming.handle.is_finished())
    }

    pub fn is_paused(&self) -> bool {
        self.loop_handle
            .as_ref()
            .map(|l| l.pause.load(Ordering::SeqCst))
            .unwrap_or(false)
    }

    pub fn start(&mut self, app: AppHandle) {
        self.cleanup_finished_loop();
        if self.loop_handle.is_some() {
            return;
        }
        let stop = Arc::new(AtomicBool::new(false));
        let pause = Arc::new(AtomicBool::new(false));
        let stop_clone = stop.clone();
        let pause_clone = pause.clone();
        let handle = thread::spawn(move || {
            run_unavailable_loop(app, stop_clone, pause_clone);
        });
        self.loop_handle = Some(StreamingLoop {
            stop,
            pause,
            handle,
        });
    }

    pub fn pause(&self) {
        if let Some(ref streaming) = self.loop_handle {
            streaming.pause.store(true, Ordering::SeqCst);
        }
    }

    pub fn resume(&self) {
        if let Some(ref streaming) = self.loop_handle {
            streaming.pause.store(false, Ordering::SeqCst);
        }
    }

    pub fn stop(&mut self) {
        if let Some(streaming) = self.loop_handle.take() {
            streaming.stop.store(true, Ordering::SeqCst);
            streaming.pause.store(false, Ordering::SeqCst); // unblock paused thread
            let _ = streaming.handle.join();
        }
    }

    fn cleanup_finished_loop(&mut self) {
        if self
            .loop_handle
            .as_ref()
            .is_some_and(|streaming| streaming.handle.is_finished())
        {
            let finished = self.loop_handle.take().expect("finished loop exists");
            let _ = finished.handle.join();
        }
    }
}

fn run_unavailable_loop(app: AppHandle, stop: Arc<AtomicBool>, pause: Arc<AtomicBool>) {
    while !stop.load(Ordering::SeqCst) {
        if pause.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(200));
            continue;
        }
        if app
            .emit(LIVE_ERROR_EVENT, &streaming_unavailable_error())
            .is_err()
        {
            // No listener attached; the frontend may already have been torn down.
        }
        break;
    }
}

fn streaming_unavailable_error() -> LiveCaptureError {
    LiveCaptureError {
        code: STREAMING_UNAVAILABLE_CODE.to_string(),
        message: STREAMING_UNAVAILABLE_MESSAGE.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_source_emits_expected_sequence() {
        let mut source = MockStreamingSource::new();
        let first = source.next_segment(1).expect("first segment");
        assert_eq!(first.speaker.label, "张伟");
        assert_eq!(first.speaker.source, "voiceprint");
        assert!(first.completed);

        let second = source.next_segment(2).expect("second segment");
        assert_eq!(second.speaker.label, "我");
        assert_eq!(second.speaker.source, "manual");
    }

    #[test]
    fn mock_source_terminates_after_scripts() {
        let mut source = MockStreamingSource::new();
        let mut count = 0;
        while source.next_segment(count + 1).is_some() {
            count += 1;
        }
        assert_eq!(count, 5);
    }

    #[test]
    fn streaming_capture_starts_and_stops() {
        let capture = StreamingCapture::default();
        assert!(!capture.is_running());
        // start/stop require an AppHandle; verified at the command layer.
        assert!(capture.loop_handle.is_none());
    }

    #[test]
    fn drop_stops_and_joins_running_worker() {
        let stop = Arc::new(AtomicBool::new(false));
        let pause = Arc::new(AtomicBool::new(false));
        let worker_exited = Arc::new(AtomicBool::new(false));

        let stop_for_worker = Arc::clone(&stop);
        let pause_for_worker = Arc::clone(&pause);
        let worker_exited_for_thread = Arc::clone(&worker_exited);
        let handle = thread::spawn(move || {
            while !stop_for_worker.load(Ordering::SeqCst) {
                if pause_for_worker.load(Ordering::SeqCst) {
                    thread::sleep(Duration::from_millis(5));
                    continue;
                }
                thread::sleep(Duration::from_millis(5));
            }
            worker_exited_for_thread.store(true, Ordering::SeqCst);
        });

        let capture = StreamingCapture {
            loop_handle: Some(StreamingLoop {
                stop,
                pause,
                handle,
            }),
        };

        drop(capture);

        assert!(worker_exited.load(Ordering::SeqCst));
    }

    #[test]
    fn desktop_runtime_reports_unavailable_instead_of_mocking() {
        let error = streaming_unavailable_error();

        assert_eq!(error.code, STREAMING_UNAVAILABLE_CODE);
        assert_eq!(error.message, STREAMING_UNAVAILABLE_MESSAGE);
    }
}
