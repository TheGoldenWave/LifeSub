//! Audio decoding, downmix, resampling, and time conversion.
//!
//! Decodes supported formats (WAV, MP3, M4A, AAC, FLAC, OGG) via Symphonia 0.6,
//! downmixes multi-channel audio to f32 mono using arithmetic mean,
//! resamples to 16 kHz with Rubato 5, and provides authoritative frame-index
//! to millisecond time conversion.

use rubato::audioadapter_buffers::owned::InterleavedOwned;
use rubato::{Fft, FixedSync, Resampler};
use symphonia::core::audio::sample::Sample;
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Standard ASR working sample rate (Hz).
const TARGET_SAMPLE_RATE: usize = 16_000;

/// Resampler chunk size — process audio in blocks of this many input frames.
const RESAMPLER_CHUNK_SIZE: usize = 1024;

/// Maximum number of audio channels we accept before rejecting.
const MAX_CHANNELS: usize = 16;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Decoded audio ready for ASR processing.
///
/// Always 16 kHz f32 mono, clamped to [-1, 1].
#[derive(Clone, Debug, PartialEq)]
pub struct DecodedAudio {
    /// f32 mono samples at 16 kHz.
    pub samples: Vec<f32>,
    /// Always 16_000 after decode + resample.
    pub sample_rate: u32,
    /// Total duration in milliseconds, derived from original frame count.
    pub duration_ms: u64,
    /// Human-readable description of the original format.
    pub original_format: String,
}

/// Errors that can occur during audio decoding or processing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AudioError {
    /// The input format is not supported by any registered Symphonia codec.
    UnsupportedFormat,
    /// The codec reported an error during decode.
    DecodeFailed(String),
    /// The resampler could not be created or failed during processing.
    ResampleFailed(String),
    /// The audio has more channels than supported.
    TooManyChannels(usize),
    /// The input is empty.
    EmptyInput,
    /// The audio stream ended without producing any samples.
    NoAudioData,
}

impl std::fmt::Display for AudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedFormat => write!(f, "unsupported audio format"),
            Self::DecodeFailed(msg) => write!(f, "decode failed: {msg}"),
            Self::ResampleFailed(msg) => write!(f, "resample failed: {msg}"),
            Self::TooManyChannels(n) => write!(f, "too many channels: {n}"),
            Self::EmptyInput => write!(f, "empty input"),
            Self::NoAudioData => write!(f, "no audio data in stream"),
        }
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Decode an audio buffer into 16 kHz f32 mono PCM.
///
/// Format is auto-detected from the byte stream. Multi-channel audio is
/// downmixed to mono via arithmetic mean. The output is resampled to 16 kHz
/// and clamped to [-1, 1].
pub fn decode_audio(data: &[u8]) -> Result<DecodedAudio, AudioError> {
    if data.is_empty() {
        return Err(AudioError::EmptyInput);
    }

    let mss = MediaSourceStream::new(
        Box::new(std::io::Cursor::new(data.to_vec())),
        Default::default(),
    );

    let hint = Hint::new();
    let fmt_opts: FormatOptions = Default::default();
    let meta_opts: MetadataOptions = Default::default();
    let dec_opts: AudioDecoderOptions = Default::default();

    let mut format = symphonia::default::get_probe()
        .probe(&hint, mss, fmt_opts, meta_opts)
        .map_err(|_| AudioError::UnsupportedFormat)?;

    let track = format
        .default_track(TrackType::Audio)
        .ok_or(AudioError::UnsupportedFormat)?;

    let track_id = track.id;
    let codec_params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or(AudioError::UnsupportedFormat)?;

    let sample_rate = codec_params.sample_rate.unwrap_or(0);
    if sample_rate == 0 {
        return Err(AudioError::DecodeFailed(
            "unknown sample rate".to_string(),
        ));
    }

    let channels = codec_params.channels.clone().map(|c| c.count()).unwrap_or(1);
    if channels > MAX_CHANNELS {
        return Err(AudioError::TooManyChannels(channels));
    }

    let original_format = format!(
        "{} {}Hz {}ch",
        codec_params.codec, sample_rate, channels
    );

    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(codec_params, &dec_opts)
        .map_err(|e| AudioError::DecodeFailed(e.to_string()))?;

    let mut raw_samples: Vec<f32> = Vec::new();
    let mut total_frames: u64 = 0;

    loop {
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(Error::IoError(ref e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(AudioError::DecodeFailed(e.to_string())),
        };

        if packet.track_id != track_id {
            continue;
        }

        let audio_buf = match decoder.decode(&packet) {
            Ok(buf) => buf,
            Err(Error::DecodeError(_)) => continue,
            Err(e) => return Err(AudioError::DecodeFailed(e.to_string())),
        };

        let num_frames = audio_buf.frames();
        total_frames += num_frames as u64;

        let sample_count = audio_buf.samples_interleaved();
        let start = raw_samples.len();
        raw_samples.resize(start + sample_count, f32::MID);
        audio_buf.copy_to_slice_interleaved(&mut raw_samples[start..]);
    }

    if raw_samples.is_empty() {
        return Err(AudioError::NoAudioData);
    }

    // Downmix to mono (arithmetic mean)
    let mono_samples = downmix_to_mono(&raw_samples, channels as usize);

    // Resample to 16 kHz
    let resampled =
        resample_to_target(&mono_samples, sample_rate as usize, TARGET_SAMPLE_RATE)
            .map_err(AudioError::ResampleFailed)?;

    // Clamp to [-1, 1]
    let clamped: Vec<f32> = resampled.into_iter().map(|s| s.clamp(-1.0, 1.0)).collect();

    // Duration from original frame count at original sample rate
    let duration_ms = (total_frames * 1000) / sample_rate as u64;

    Ok(DecodedAudio {
        samples: clamped,
        sample_rate: TARGET_SAMPLE_RATE as u32,
        duration_ms,
        original_format,
    })
}

/// Convert a frame index at the given sample rate to milliseconds (floor).
pub fn frame_index_to_ms(frame_index: usize, sample_rate: u32) -> u64 {
    (frame_index as u64 * 1000) / sample_rate as u64
}

/// Convert milliseconds to the nearest frame index at the given sample rate.
pub fn ms_to_frame_index(ms: u64, sample_rate: u32) -> usize {
    ((ms as u64 * sample_rate as u64) / 1000) as usize
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Downmix interleaved multi-channel samples to mono via arithmetic mean.
fn downmix_to_mono(interleaved: &[f32], channels: usize) -> Vec<f32> {
    if channels == 1 {
        return interleaved.to_vec();
    }

    let num_frames = interleaved.len() / channels;
    let mut mono = Vec::with_capacity(num_frames);

    for frame_idx in 0..num_frames {
        let start = frame_idx * channels;
        let sum: f32 = interleaved[start..start + channels].iter().sum();
        mono.push(sum / channels as f32);
    }

    mono
}

/// Resample f32 mono audio from `input_rate` to `output_rate` Hz.
///
/// Uses Rubato 5 `Fft` synchronous resampler with fixed input size.
/// Resampler delay is compensated so the output timing aligns with the input.
fn resample_to_target(
    samples: &[f32],
    input_rate: usize,
    output_rate: usize,
) -> Result<Vec<f32>, String> {
    if input_rate == output_rate {
        return Ok(samples.to_vec());
    }

    let chunk_size = RESAMPLER_CHUNK_SIZE;

    let mut resampler = Fft::<f32>::new(
        input_rate,
        output_rate,
        chunk_size,
        1, // single channel
        FixedSync::Input,
    )
    .map_err(|e| format!("failed to create resampler: {e}"))?;

    let mut output = Vec::with_capacity(
        (samples.len() as f64 * output_rate as f64 / input_rate as f64).ceil() as usize,
    );

    let mut input_offset = 0;

    while input_offset < samples.len() {
        let remaining = samples.len() - input_offset;
        let take = chunk_size.min(remaining);

        let chunk: Vec<f32> = if take == chunk_size {
            samples[input_offset..input_offset + take].to_vec()
        } else {
            let mut padded = vec![0.0f32; chunk_size];
            padded[..take].copy_from_slice(&samples[input_offset..input_offset + take]);
            padded
        };

        let chunk_len = chunk.len();
        let input_buffer = InterleavedOwned::new_from(chunk, 1, chunk_len)
            .map_err(|e| format!("failed to create input buffer: {e:?}"))?;

        let result = resampler
            .process(&input_buffer, None)
            .map_err(|e| format!("resampling error: {e}"))?;

        output.extend_from_slice(&result.take_data());
        input_offset += take;
    }

    // Flush the resampler's internal buffer
    let flush_input = vec![0.0f32; chunk_size];
    let flush_len = flush_input.len();
    let flush_buffer = InterleavedOwned::new_from(flush_input, 1, flush_len)
        .map_err(|e| format!("failed to create flush buffer: {e:?}"))?;

    let flush_output = resampler
        .process(&flush_buffer, None)
        .map_err(|e| format!("resampling flush error: {e}"))?;

    output.extend_from_slice(&flush_output.take_data());

    // Compensate for resampler delay: skip startup ramp, keep tail
    let delay = resampler.output_delay();
    if delay > 0 && output.len() > delay {
        output.drain(..delay);
    }

    // Trim trailing near-silence from padding/flush
    while output.len() > delay
        && output.last().map_or(false, |&s| s.abs() < 1e-7)
    {
        output.pop();
    }

    Ok(output)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_index_to_ms_basics() {
        assert_eq!(frame_index_to_ms(0, 16_000), 0);
        assert_eq!(frame_index_to_ms(16_000, 16_000), 1000);
        assert_eq!(frame_index_to_ms(8_000, 16_000), 500);
    }

    #[test]
    fn ms_to_frame_index_basics() {
        assert_eq!(ms_to_frame_index(0, 16_000), 0);
        assert_eq!(ms_to_frame_index(1000, 16_000), 16_000);
        assert_eq!(ms_to_frame_index(500, 16_000), 8_000);
    }

    #[test]
    fn downmix_stereo_is_arithmetic_mean() {
        let interleaved = vec![
            1.0, 0.0,  // frame 0
            0.5, 0.5,  // frame 1
            -1.0, 0.0, // frame 2
        ];
        let mono = downmix_to_mono(&interleaved, 2);
        assert_eq!(mono.len(), 3);
        assert!((mono[0] - 0.5).abs() < 1e-6);
        assert!((mono[1] - 0.5).abs() < 1e-6);
        assert!((mono[2] - (-0.5)).abs() < 1e-6);
    }

    #[test]
    fn downmix_mono_is_identity() {
        let interleaved = vec![0.5, -0.3, 0.8];
        let mono = downmix_to_mono(&interleaved, 1);
        assert_eq!(mono, interleaved);
    }

    #[test]
    fn resample_passthrough() {
        let samples: Vec<f32> = (0..1024).map(|i| (i as f32 * 0.01).sin()).collect();
        let result = resample_to_target(&samples, 16_000, 16_000).unwrap();
        assert_eq!(result.len(), samples.len());
    }

    #[test]
    fn resample_downsample_length() {
        let samples: Vec<f32> = (0..48_000).map(|i| (i as f32 * 0.001).sin()).collect();
        let result = resample_to_target(&samples, 48_000, 16_000).unwrap();
        let expected = samples.len() * 16_000 / 48_000;
        let diff = (result.len() as i64 - expected as i64).unsigned_abs();
        assert!(diff < 1000);
    }

    #[test]
    fn decode_empty_errors() {
        assert_eq!(decode_audio(&[]), Err(AudioError::EmptyInput));
    }

    #[test]
    fn decode_garbage_errors() {
        let result = decode_audio(&[0xDE, 0xAD, 0xBE, 0xEF]);
        assert!(result.is_err());
    }
}