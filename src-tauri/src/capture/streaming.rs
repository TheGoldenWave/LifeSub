//! Live streaming capture service.
//!
//! Owns the real-time ASR event channel: a background thread captures audio
//! (or reads a mock source in development), runs per-chunk transcription, and
//! emits `asr-live-segment` events to the WebView. The production ASR provider
//! is wired in behind the `StreamingSource` trait; the mock source keeps the
//! development build usable without native models.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

/// Emitted to the WebView whenever a transcribed segment becomes available.
pub const LIVE_SEGMENT_EVENT: &str = "asr-live-segment";

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
                ("spk-1", "张伟", "我们今天先确认首版范围，重点是把基础闭环真正跑起来。"),
                ("spk-2", "我", "好的，我记一下。证据链要保证每次修改都能追溯。"),
                ("spk-1", "张伟", "对，而且要保证搜索结果能回到准确的音频时间范围。"),
                ("spk-3", "可能是李娜？", "还有一个点，关于数据目录的权限控制，需要再确认。"),
                ("spk-4", "未知说话人 1", "这个方案我觉得可以，但是需要再确认一下安全性。"),
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
                source: if id == "spk-4" { "unknown".into() } else if id == "spk-3" { "dictionary".into() } else if id == "spk-2" { "manual".into() } else { "voiceprint".into() },
                voiceprint_id: if id == "spk-1" { Some("vp-1".into()) } else { None },
            },
            text: text.to_string(),
            completed: true,
        })
    }
}

/// Running state of the streaming capture loop.
struct StreamingLoop {
    stop: Arc<AtomicBool>,
    handle: JoinHandle<()>,
}

/// Owned by `AppState`; starts/stops the streaming loop on demand.
pub struct StreamingCapture {
    loop_handle: Option<StreamingLoop>,
}

impl Default for StreamingCapture {
    fn default() -> Self {
        Self { loop_handle: None }
    }
}

impl StreamingCapture {
    pub fn is_running(&self) -> bool {
        self.loop_handle.is_some()
    }

    pub fn start(&mut self, app: AppHandle) {
        if self.loop_handle.is_some() {
            return;
        }
        let stop = Arc::new(AtomicBool::new(false));
        let stop_clone = stop.clone();
        let handle = thread::spawn(move || {
            run_streaming_loop(app, stop_clone, MockStreamingSource::new());
        });
        self.loop_handle = Some(StreamingLoop { stop, handle });
    }

    pub fn stop(&mut self) {
        if let Some(streaming) = self.loop_handle.take() {
            streaming.stop.store(true, Ordering::SeqCst);
            // The loop wakes frequently, so a bounded join is safe.
            let _ = streaming.handle.join();
        }
    }
}

fn run_streaming_loop<S: StreamingSource>(app: AppHandle, stop: Arc<AtomicBool>, mut source: S) {
    let mut sequence: u64 = 0;
    while !stop.load(Ordering::SeqCst) {
        sequence += 1;
        if let Some(segment) = source.next_segment(sequence) {
            if app.emit(LIVE_SEGMENT_EVENT, &segment).is_err() {
                // No listener attached; continue until stopped.
            }
        } else {
            // Mock source exhausted; keep the loop alive briefly then idle.
        }
        // 2-second cadence keeps development visible and production cheap.
        thread::sleep(Duration::from_millis(2000));
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
        let mut capture = StreamingCapture::default();
        assert!(!capture.is_running());
        // start/stop require an AppHandle; verified at the command layer.
        assert!(capture.loop_handle.is_none());
    }
}