//! Voice Activity Detection and audio partitioning.
//!
//! Provides a VAD detector trait, a deterministic fake detector for tests,
//! and audio partitioning into non-overlapping windows with 200 ms speech
//! padding and 25-second maximum duration. Windows exceeding 25 seconds are
//! split at minimum-energy points or hard-split at the boundary.

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// VAD speech padding in milliseconds (applied to each side of core interval).
const SPEECH_PADDING_MS: u64 = 200;

/// Maximum continuous speech window duration in milliseconds.
const MAX_WINDOW_MS: u64 = 25_000;

/// Energy search window for split-point detection (ms on each side).
const ENERGY_SEARCH_WINDOW_MS: u64 = 2_000;

/// Standard ASR sample rate for time/sample conversion.
#[allow(dead_code)]
const TARGET_SAMPLE_RATE: u32 = 16_000;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A core speech interval detected by VAD (no padding applied).
///
/// These are the non-overlapping evidence ranges. Padded intervals used for
/// inference context are derived during partitioning.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpeechSegment {
    /// Start time in milliseconds (chunk-relative, core interval).
    pub start_ms: u64,
    /// End time in milliseconds (chunk-relative, core interval).
    pub end_ms: u64,
}

/// A partitioned audio window ready for inference.
///
/// The core interval is the non-overlapping evidence range. The padded
/// interval extends by 200 ms on each side to provide inference context.
/// The sample range indexes into the full decoded audio buffer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PartitionedWindow {
    /// Core interval start in milliseconds (non-overlapping, for evidence).
    pub core_start_ms: u64,
    /// Core interval end in milliseconds.
    pub core_end_ms: u64,
    /// Padded interval start (core_start_ms - 200 ms, clamped to 0).
    pub padded_start_ms: u64,
    /// Padded interval end (core_end_ms + 200 ms, clamped to duration).
    pub padded_end_ms: u64,
    /// Starting sample index in the full decoded audio buffer.
    pub sample_offset: usize,
    /// Number of samples in this window (padded range).
    pub sample_count: usize,
}

/// Trait for voice activity detection.
///
/// Implementations include the real Silero VAD (`asr-runtime` feature) and
/// a deterministic fake detector for fast tests.
pub trait VadDetector: Send {
    /// Detect speech segments in the given audio samples.
    ///
    /// Returns core (non-padded) speech intervals in chronological order.
    fn detect(&self, samples: &[f32], sample_rate: u32) -> Vec<SpeechSegment>;
}

/// A deterministic fake VAD detector for tests.
///
/// Returns pre-configured segments regardless of the audio content.
pub struct FakeVadDetector {
    segments: Vec<SpeechSegment>,
}

impl FakeVadDetector {
    /// Create a fake detector that always returns the given segments.
    pub fn new(segments: Vec<SpeechSegment>) -> Self {
        Self { segments }
    }
}

impl VadDetector for FakeVadDetector {
    fn detect(&self, _samples: &[f32], _sample_rate: u32) -> Vec<SpeechSegment> {
        self.segments.clone()
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Partition decoded audio into non-overlapping windows with padding.
///
/// Each `SpeechSegment` is expanded by 200 ms on each side for inference
/// context. Segments (or groups of segments) exceeding 25 seconds are split
/// at minimum-energy points; if no clear energy dip is found, they are
/// hard-split at the 25-second boundary. Core intervals remain monotonic
/// and non-overlapping.
///
/// Evidence ranges must use `core_start_ms`/`core_end_ms`, not the padded
/// inference context.
pub fn partition_audio(
    samples: &[f32],
    sample_rate: u32,
    segments: &[SpeechSegment],
    duration_ms: u64,
) -> Vec<PartitionedWindow> {
    if segments.is_empty() {
        return Vec::new();
    }

    let mut windows = Vec::new();

    for segment in segments {
        let seg_start = segment.start_ms;
        let seg_end = segment.end_ms;

        if seg_end <= seg_start || seg_start >= duration_ms {
            continue;
        }

        let clamped_end = seg_end.min(duration_ms);

        // Split segments longer than MAX_WINDOW_MS
        let sub_segments = split_long_segment(samples, sample_rate, seg_start, clamped_end);

        for (sub_start, sub_end) in sub_segments {
            let padded_start = sub_start.saturating_sub(SPEECH_PADDING_MS);
            let padded_end = (sub_end + SPEECH_PADDING_MS).min(duration_ms);

            let sample_offset = ms_to_sample_index(padded_start, sample_rate);
            let sample_end = ms_to_sample_index(padded_end, sample_rate);
            let sample_count = sample_end.saturating_sub(sample_offset);

            windows.push(PartitionedWindow {
                core_start_ms: sub_start,
                core_end_ms: sub_end,
                padded_start_ms: padded_start,
                padded_end_ms: padded_end,
                sample_offset,
                sample_count,
            });
        }
    }

    windows
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Split a long segment (>25s) into sub-segments at minimum-energy points.
///
/// Returns a list of (start_ms, end_ms) pairs each ≤ MAX_WINDOW_MS.
fn split_long_segment(
    samples: &[f32],
    sample_rate: u32,
    start_ms: u64,
    end_ms: u64,
) -> Vec<(u64, u64)> {
    let total_duration = end_ms - start_ms;

    if total_duration <= MAX_WINDOW_MS {
        return vec![(start_ms, end_ms)];
    }

    let mut splits = Vec::new();
    let mut cursor = start_ms;

    while cursor < end_ms {
        let remaining = end_ms - cursor;
        if remaining <= MAX_WINDOW_MS {
            splits.push((cursor, end_ms));
            break;
        }

        // Try to find a minimum-energy split point within the last
        // ENERGY_SEARCH_WINDOW_MS before the MAX_WINDOW_MS boundary.
        let boundary = cursor + MAX_WINDOW_MS;
        let search_start = boundary.saturating_sub(ENERGY_SEARCH_WINDOW_MS);
        let search_end = boundary;

        let split_point = find_min_energy_split(
            samples,
            sample_rate,
            search_start,
            search_end,
            cursor,
            end_ms,
        );

        splits.push((cursor, split_point));
        cursor = split_point;
    }

    splits
}

/// Find the minimum-energy point in a search window.
///
/// Returns the split point in milliseconds. If no clear energy dip is found,
/// falls back to a hard split at the boundary.
fn find_min_energy_split(
    samples: &[f32],
    sample_rate: u32,
    search_start_ms: u64,
    search_end_ms: u64,
    clip_start_ms: u64,
    clip_end_ms: u64,
) -> u64 {
    let boundary_ms = search_end_ms;

    let search_start_sample = ms_to_sample_index(search_start_ms, sample_rate);
    let search_end_sample = ms_to_sample_index(search_end_ms, sample_rate).min(samples.len());

    if search_start_sample >= search_end_sample || search_end_sample - search_start_sample < 100 {
        // Search window too small — hard split at boundary
        return boundary_ms;
    }

    // Compute energy in overlapping windows of ~20ms
    let window_size = ms_to_sample_index(20, sample_rate);
    let hop_size = window_size / 2;

    let mut min_energy = f32::MAX;
    let mut min_energy_sample = search_start_sample;

    let mut pos = search_start_sample;
    while pos + window_size <= search_end_sample {
        let energy: f32 = samples[pos..pos + window_size]
            .iter()
            .map(|&s| s * s)
            .sum();
        if energy < min_energy {
            min_energy = energy;
            min_energy_sample = pos + window_size / 2;
        }
        pos += hop_size;
    }

    // Convert sample index back to ms
    let min_energy_ms = (min_energy_sample as u64 * 1000) / sample_rate as u64;

    // Ensure the split point is within the valid range
    if min_energy_ms > clip_start_ms && min_energy_ms < clip_end_ms {
        min_energy_ms
    } else {
        boundary_ms
    }
}

/// Convert milliseconds to sample index at the given sample rate.
fn ms_to_sample_index(ms: u64, sample_rate: u32) -> usize {
    // Use ceil: (ms * sample_rate + 999) / 1000
    ((ms as u64 * sample_rate as u64 + 999) / 1000) as usize
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_detector_returns_provided_segments() {
        let segments = vec![
            SpeechSegment {
                start_ms: 100,
                end_ms: 500,
            },
            SpeechSegment {
                start_ms: 1000,
                end_ms: 2000,
            },
        ];
        let detector = FakeVadDetector::new(segments.clone());
        let result = detector.detect(&[0.0f32; 100], TARGET_SAMPLE_RATE);
        assert_eq!(result, segments);
    }

    #[test]
    fn fake_detector_empty() {
        let detector = FakeVadDetector::new(vec![]);
        let result = detector.detect(&[0.0f32; 100], TARGET_SAMPLE_RATE);
        assert!(result.is_empty());
    }

    #[test]
    fn partition_empty_segments() {
        let samples = vec![0.0f32; 160_000];
        let windows = partition_audio(&samples, TARGET_SAMPLE_RATE, &[], 10_000);
        assert!(windows.is_empty());
    }

    #[test]
    fn partition_single_short_segment() {
        let segments = vec![SpeechSegment {
            start_ms: 1000,
            end_ms: 2000,
        }];
        let samples = vec![0.0f32; 160_000];
        let windows = partition_audio(&samples, TARGET_SAMPLE_RATE, &segments, 10_000);
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].core_start_ms, 1000);
        assert_eq!(windows[0].core_end_ms, 2000);
        assert_eq!(windows[0].padded_start_ms, 800); // 1000 - 200
        assert_eq!(windows[0].padded_end_ms, 2200); // 2000 + 200
    }

    #[test]
    fn padding_clamped_to_zero() {
        let segments = vec![SpeechSegment {
            start_ms: 50,
            end_ms: 500,
        }];
        let samples = vec![0.0f32; 160_000];
        let windows = partition_audio(&samples, TARGET_SAMPLE_RATE, &segments, 10_000);
        assert_eq!(windows[0].padded_start_ms, 0);
    }

    #[test]
    fn padding_clamped_to_duration() {
        let segments = vec![SpeechSegment {
            start_ms: 9500,
            end_ms: 10_000,
        }];
        let samples = vec![0.0f32; 160_000];
        let windows = partition_audio(&samples, TARGET_SAMPLE_RATE, &segments, 10_000);
        assert_eq!(windows[0].padded_end_ms, 10_000);
    }

    #[test]
    fn windows_are_monotonic_and_non_overlapping() {
        let segments = vec![
            SpeechSegment {
                start_ms: 0,
                end_ms: 1000,
            },
            SpeechSegment {
                start_ms: 2000,
                end_ms: 3000,
            },
            SpeechSegment {
                start_ms: 5000,
                end_ms: 7000,
            },
        ];
        let samples = vec![0.0f32; 160_000];
        let windows = partition_audio(&samples, TARGET_SAMPLE_RATE, &segments, 10_000);

        for pair in windows.windows(2) {
            assert!(pair[0].core_end_ms <= pair[1].core_start_ms);
        }
        for w in &windows {
            assert!(w.core_start_ms < w.core_end_ms);
            assert!(w.core_end_ms <= 10_000);
        }
    }

    #[test]
    fn long_segment_splits_at_25s() {
        let segments = vec![SpeechSegment {
            start_ms: 0,
            end_ms: 55_000,
        }];
        let samples = vec![0.5f32; 880_000]; // 55s @ 16kHz
        let windows = partition_audio(&samples, TARGET_SAMPLE_RATE, &segments, 55_000);

        assert!(windows.len() >= 3);
        for w in &windows {
            assert!(w.core_end_ms - w.core_start_ms <= MAX_WINDOW_MS);
        }
    }

    #[test]
    fn min_energy_split_finds_dip() {
        let segments = vec![SpeechSegment {
            start_ms: 0,
            end_ms: 30_000,
        }];
        let mut samples = vec![0.5f32; 480_000];
        // Create energy dip at ~15 seconds
        let dip_center = 240_000;
        let dip_width = 1600;
        for i in dip_center - dip_width..dip_center + dip_width {
            if i < samples.len() {
                samples[i] = 0.0;
            }
        }

        let windows = partition_audio(&samples, TARGET_SAMPLE_RATE, &segments, 30_000);
        assert!(windows.len() >= 2);

        let has_split_near_dip = windows.windows(2).any(|pair| {
            let split = pair[0].core_end_ms;
            split >= 10_000 && split <= 20_000
        });
        assert!(has_split_near_dip);
    }

    #[test]
    fn window_sample_indices_are_valid() {
        let segments = vec![
            SpeechSegment {
                start_ms: 1000,
                end_ms: 2000,
            },
            SpeechSegment {
                start_ms: 3000,
                end_ms: 4000,
            },
        ];
        let samples = vec![0.5f32; 160_000];
        let windows = partition_audio(&samples, TARGET_SAMPLE_RATE, &segments, 10_000);

        for w in &windows {
            assert!(w.sample_offset + w.sample_count <= samples.len());
            assert!(w.sample_count > 0);
        }
    }
}