use std::fs::File;
use std::path::Path;

use rubato::audioadapter_buffers::owned::InterleavedOwned;
use rubato::{Fft, FixedSync, Resampler};
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

pub const WORK_SAMPLE_RATE_HZ: u32 = 16_000;
// Whole-file in-memory preparation needs a safety bound; this is not an import policy.
pub const MAX_DECODED_AUDIO_DURATION_SECONDS: u64 = 24 * 60 * 60;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SampleRange {
    pub start: u64,
    pub end: u64,
}

impl SampleRange {
    pub fn new(start: u64, end: u64) -> Result<Self, AudioPreparationError> {
        if start >= end {
            return Err(AudioPreparationError::InvalidRange);
        }
        Ok(Self { start, end })
    }

    pub fn checked_len(self) -> Result<u64, AudioPreparationError> {
        let length = self
            .end
            .checked_sub(self.start)
            .ok_or(AudioPreparationError::InvalidRange)?;
        if length == 0 {
            return Err(AudioPreparationError::InvalidRange);
        }
        Ok(length)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MillisecondRange {
    pub start_ms: u64,
    pub end_ms: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorkingAudio {
    pub samples: Vec<f32>,
    pub sample_rate_hz: u32,
    pub source_sample_rate_hz: u32,
    pub source_channels: usize,
    pub source_frames: u64,
    pub duration_ms: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AudioPreparationError {
    UnsupportedOrCorruptAudio,
    InvalidSampleRate,
    InvalidRange,
    InvalidDetectorCores,
    ArithmeticOverflow,
    IndexOutOfRange,
    ResampleFailed,
    DetectorFailed,
    InvalidVadConfig,
    ResourceLimitExceeded,
    AllocationFailed,
}

impl AudioPreparationError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedOrCorruptAudio => "unsupported_or_corrupt_audio",
            Self::InvalidSampleRate => "invalid_sample_rate",
            Self::InvalidRange => "invalid_audio_range",
            Self::InvalidDetectorCores => "invalid_detector_cores",
            Self::ArithmeticOverflow => "audio_arithmetic_overflow",
            Self::IndexOutOfRange => "audio_index_out_of_range",
            Self::ResampleFailed => "audio_resample_failed",
            Self::DetectorFailed => "vad_detector_failed",
            Self::InvalidVadConfig => "invalid_vad_runtime_config",
            Self::ResourceLimitExceeded => "audio_resource_limit_exceeded",
            Self::AllocationFailed => "audio_allocation_failed",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DecodeResourceBudget {
    channels: usize,
    max_source_frames: u64,
    max_interleaved_samples: usize,
    source_frames: u64,
    interleaved_samples: usize,
}

impl DecodeResourceBudget {
    pub(crate) fn new(sample_rate_hz: u32, channels: usize) -> Result<Self, AudioPreparationError> {
        if sample_rate_hz == 0 {
            return Err(AudioPreparationError::InvalidSampleRate);
        }
        if channels == 0 {
            return Err(AudioPreparationError::UnsupportedOrCorruptAudio);
        }
        let max_source_frames = max_source_frames(sample_rate_hz)?;
        let max_interleaved_samples = checked_interleaved_samples(max_source_frames, channels)?;
        Ok(Self {
            channels,
            max_source_frames,
            max_interleaved_samples,
            source_frames: 0,
            interleaved_samples: 0,
        })
    }

    pub(crate) fn accept_packet(
        &mut self,
        packet_frames: u64,
        packet_samples: usize,
    ) -> Result<(), AudioPreparationError> {
        if packet_frames == 0 || packet_samples == 0 {
            return Err(AudioPreparationError::UnsupportedOrCorruptAudio);
        }
        let expected_samples = checked_interleaved_samples(packet_frames, self.channels)?;
        if packet_samples != expected_samples {
            return Err(AudioPreparationError::UnsupportedOrCorruptAudio);
        }
        let source_frames = self
            .source_frames
            .checked_add(packet_frames)
            .ok_or(AudioPreparationError::ResourceLimitExceeded)?;
        let interleaved_samples = self
            .interleaved_samples
            .checked_add(packet_samples)
            .ok_or(AudioPreparationError::ResourceLimitExceeded)?;
        if source_frames > self.max_source_frames
            || interleaved_samples > self.max_interleaved_samples
        {
            return Err(AudioPreparationError::ResourceLimitExceeded);
        }
        self.source_frames = source_frames;
        self.interleaved_samples = interleaved_samples;
        Ok(())
    }
}

pub(crate) fn validate_declared_source_frames(
    declared_frames: Option<u64>,
    sample_rate_hz: u32,
) -> Result<(), AudioPreparationError> {
    let max_frames = max_source_frames(sample_rate_hz)?;
    if declared_frames.is_some_and(|frames| frames > max_frames) {
        return Err(AudioPreparationError::ResourceLimitExceeded);
    }
    Ok(())
}

pub fn decode_to_working_audio(path: &Path) -> Result<WorkingAudio, AudioPreparationError> {
    let source = File::open(path).map_err(|_| AudioPreparationError::UnsupportedOrCorruptAudio)?;
    let extension = path.extension().and_then(|value| value.to_str());
    decode_from_file(source, extension)
}

pub fn decode_to_working_audio_from_file(
    file: File,
    extension_hint: Option<&str>,
) -> Result<WorkingAudio, AudioPreparationError> {
    decode_from_file(file, extension_hint)
}

fn decode_from_file(
    source: File,
    extension_hint: Option<&str>,
) -> Result<WorkingAudio, AudioPreparationError> {
    let stream = MediaSourceStream::new(Box::new(source), Default::default());
    let mut hint = Hint::new();
    if let Some(extension) = extension_hint {
        hint.with_extension(extension);
    }
    let mut format = symphonia::default::get_probe()
        .probe(
            &hint,
            stream,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|_| AudioPreparationError::UnsupportedOrCorruptAudio)?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or(AudioPreparationError::UnsupportedOrCorruptAudio)?;
    let codec_params = track
        .codec_params
        .as_ref()
        .and_then(|params| params.audio())
        .ok_or(AudioPreparationError::UnsupportedOrCorruptAudio)?;
    if let Some(sample_rate_hz) = codec_params.sample_rate {
        validate_declared_source_frames(track.num_frames, sample_rate_hz)?;
    }
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(codec_params, &AudioDecoderOptions::default())
        .map_err(|_| AudioPreparationError::UnsupportedOrCorruptAudio)?;
    let track_id = track.id;
    let mut source_rate = None;
    let mut source_channels = None;
    let mut source_frames = 0_u64;
    let mut interleaved = Vec::new();
    let mut resource_budget = None;

    loop {
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(_) => return Err(AudioPreparationError::UnsupportedOrCorruptAudio),
        };
        if packet.track_id != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(SymphoniaError::DecodeError(_)) => {
                return Err(AudioPreparationError::UnsupportedOrCorruptAudio);
            }
            Err(_) => return Err(AudioPreparationError::UnsupportedOrCorruptAudio),
        };
        let rate = decoded.spec().rate();
        let channels = decoded.spec().channels().count();
        if rate == 0 || channels == 0 {
            return Err(AudioPreparationError::UnsupportedOrCorruptAudio);
        }
        if source_rate.is_some_and(|expected| expected != rate)
            || source_channels.is_some_and(|expected| expected != channels)
        {
            return Err(AudioPreparationError::UnsupportedOrCorruptAudio);
        }
        source_rate = Some(rate);
        source_channels = Some(channels);
        if decoded.frames() == 0 {
            if decoded.samples_interleaved() != 0 {
                return Err(AudioPreparationError::UnsupportedOrCorruptAudio);
            }
            continue;
        }
        let packet_frames = u64::try_from(decoded.frames())
            .map_err(|_| AudioPreparationError::ResourceLimitExceeded)?;
        let packet_samples = decoded
            .frames()
            .checked_mul(channels)
            .ok_or(AudioPreparationError::ResourceLimitExceeded)?;
        let budget = match resource_budget.as_mut() {
            Some(budget) => budget,
            None => resource_budget.insert(DecodeResourceBudget::new(rate, channels)?),
        };
        budget.accept_packet(packet_frames, packet_samples)?;
        source_frames = budget.source_frames;
        let old_len = interleaved.len();
        reserve_f32_capacity(&mut interleaved, packet_samples)?;
        interleaved.resize(old_len + packet_samples, 0.0);
        decoded.copy_to_slice_interleaved(&mut interleaved[old_len..]);
    }

    let source_sample_rate_hz =
        source_rate.ok_or(AudioPreparationError::UnsupportedOrCorruptAudio)?;
    let source_channels =
        source_channels.ok_or(AudioPreparationError::UnsupportedOrCorruptAudio)?;
    if source_frames == 0 {
        return Err(AudioPreparationError::UnsupportedOrCorruptAudio);
    }
    let mono = sanitize_and_downmix(&interleaved, source_channels)?;
    let samples = resample_mono(&mono, source_sample_rate_hz)?;
    let duration_ms = checked_ceil_ratio(source_frames, 1_000, u64::from(source_sample_rate_hz))?;

    Ok(WorkingAudio {
        samples,
        sample_rate_hz: WORK_SAMPLE_RATE_HZ,
        source_sample_rate_hz,
        source_channels,
        source_frames,
        duration_ms,
    })
}

pub(crate) fn sanitize_and_downmix(
    interleaved: &[f32],
    channels: usize,
) -> Result<Vec<f32>, AudioPreparationError> {
    if channels == 0 || interleaved.is_empty() || !interleaved.len().is_multiple_of(channels) {
        return Err(AudioPreparationError::UnsupportedOrCorruptAudio);
    }
    let divisor = channels as f64;
    let mut mono = Vec::new();
    reserve_f32_capacity(&mut mono, interleaved.len() / channels)?;
    for frame in interleaved.chunks_exact(channels) {
        let sum = frame.iter().fold(0.0_f64, |sum, sample| {
            sum + if sample.is_finite() {
                f64::from(*sample)
            } else {
                0.0
            }
        });
        mono.push((sum / divisor).clamp(-1.0, 1.0) as f32);
    }
    Ok(mono)
}

pub(crate) fn resample_mono(
    samples: &[f32],
    source_rate_hz: u32,
) -> Result<Vec<f32>, AudioPreparationError> {
    if source_rate_hz == 0 {
        return Err(AudioPreparationError::InvalidSampleRate);
    }
    if samples.is_empty() {
        return Err(AudioPreparationError::UnsupportedOrCorruptAudio);
    }
    let expected_output_frames = resampled_frame_count(
        u64::try_from(samples.len()).map_err(|_| AudioPreparationError::ResourceLimitExceeded)?,
        source_rate_hz,
    )?;
    if source_rate_hz == WORK_SAMPLE_RATE_HZ {
        return copy_f32_samples(samples);
    }
    let input = InterleavedOwned::new_from(copy_f32_samples(samples)?, 1, samples.len())
        .map_err(|_| AudioPreparationError::ResampleFailed)?;
    let chunk_size = samples.len().clamp(1, 1_024);
    let mut resampler = Fft::<f32>::new(
        usize::try_from(source_rate_hz).map_err(|_| AudioPreparationError::ResampleFailed)?,
        WORK_SAMPLE_RATE_HZ as usize,
        chunk_size,
        1,
        FixedSync::Input,
    )
    .map_err(|_| AudioPreparationError::ResampleFailed)?;
    let needed_output_frames = resampler.process_all_needed_output_len(samples.len());
    let mut output_samples = Vec::new();
    reserve_f32_capacity(&mut output_samples, needed_output_frames)?;
    output_samples.resize(needed_output_frames, 0.0);
    let mut output = InterleavedOwned::new_from(output_samples, 1, needed_output_frames)
        .map_err(|_| AudioPreparationError::ResampleFailed)?;
    let (_, produced) = resampler
        .process_all_into_buffer(&input, &mut output, samples.len(), None)
        .map_err(|_| AudioPreparationError::ResampleFailed)?;
    if produced != expected_output_frames {
        return Err(AudioPreparationError::ResampleFailed);
    }
    let mut output = output.take_data();
    output.truncate(produced);
    for sample in &mut output {
        *sample = if sample.is_finite() {
            sample.clamp(-1.0, 1.0)
        } else {
            0.0
        };
    }
    Ok(output)
}

pub(crate) fn resampled_frame_count(
    source_frames: u64,
    source_rate_hz: u32,
) -> Result<usize, AudioPreparationError> {
    if source_rate_hz == 0 {
        return Err(AudioPreparationError::InvalidSampleRate);
    }
    if source_frames > max_source_frames(source_rate_hz)? {
        return Err(AudioPreparationError::ResourceLimitExceeded);
    }
    let output_frames = checked_ceil_ratio(
        source_frames,
        u64::from(WORK_SAMPLE_RATE_HZ),
        u64::from(source_rate_hz),
    )?;
    let max_work_frames = u64::from(WORK_SAMPLE_RATE_HZ)
        .checked_mul(MAX_DECODED_AUDIO_DURATION_SECONDS)
        .ok_or(AudioPreparationError::ResourceLimitExceeded)?;
    if output_frames > max_work_frames {
        return Err(AudioPreparationError::ResourceLimitExceeded);
    }
    usize::try_from(output_frames).map_err(|_| AudioPreparationError::ResourceLimitExceeded)
}

pub(crate) fn reserve_f32_capacity(
    samples: &mut Vec<f32>,
    additional: usize,
) -> Result<(), AudioPreparationError> {
    samples
        .try_reserve(additional)
        .map_err(|_| AudioPreparationError::AllocationFailed)
}

pub fn sample_range_to_millis(
    range: SampleRange,
    sample_rate_hz: u32,
    total_frames: u64,
) -> Result<MillisecondRange, AudioPreparationError> {
    validate_range(range, total_frames)?;
    if sample_rate_hz == 0 {
        return Err(AudioPreparationError::InvalidSampleRate);
    }
    let denominator = u64::from(sample_rate_hz);
    let start_ms = range
        .start
        .checked_mul(1_000)
        .ok_or(AudioPreparationError::ArithmeticOverflow)?
        / denominator;
    let end_ms = checked_ceil_ratio(range.end, 1_000, denominator)?;
    if start_ms >= end_ms {
        return Err(AudioPreparationError::InvalidRange);
    }
    Ok(MillisecondRange { start_ms, end_ms })
}

pub fn work_range_to_original_frames(
    range: SampleRange,
    source_rate_hz: u32,
    source_frames: u64,
) -> Result<SampleRange, AudioPreparationError> {
    if source_rate_hz == 0 {
        return Err(AudioPreparationError::InvalidSampleRate);
    }
    let source_rate = u64::from(source_rate_hz);
    let work_rate = u64::from(WORK_SAMPLE_RATE_HZ);
    let start = range
        .start
        .checked_mul(source_rate)
        .ok_or(AudioPreparationError::ArithmeticOverflow)?
        / work_rate;
    let end = checked_ceil_ratio(range.end, source_rate, work_rate)?.min(source_frames);
    let mapped = SampleRange::new(start, end)?;
    validate_range(mapped, source_frames)?;
    Ok(mapped)
}

pub(crate) fn checked_sample_index(
    index: u64,
    sample_count: usize,
) -> Result<usize, AudioPreparationError> {
    let index = usize::try_from(index).map_err(|_| AudioPreparationError::IndexOutOfRange)?;
    if index >= sample_count {
        return Err(AudioPreparationError::IndexOutOfRange);
    }
    Ok(index)
}

fn validate_range(range: SampleRange, total_frames: u64) -> Result<(), AudioPreparationError> {
    if range.start >= range.end || range.end > total_frames {
        return Err(AudioPreparationError::InvalidRange);
    }
    Ok(())
}

fn checked_ceil_ratio(
    value: u64,
    multiplier: u64,
    denominator: u64,
) -> Result<u64, AudioPreparationError> {
    if denominator == 0 {
        return Err(AudioPreparationError::InvalidSampleRate);
    }
    let numerator = value
        .checked_mul(multiplier)
        .ok_or(AudioPreparationError::ArithmeticOverflow)?;
    let adjustment = denominator
        .checked_sub(1)
        .ok_or(AudioPreparationError::ArithmeticOverflow)?;
    numerator
        .checked_add(adjustment)
        .ok_or(AudioPreparationError::ArithmeticOverflow)
        .map(|adjusted| adjusted / denominator)
}

fn max_source_frames(sample_rate_hz: u32) -> Result<u64, AudioPreparationError> {
    if sample_rate_hz == 0 {
        return Err(AudioPreparationError::InvalidSampleRate);
    }
    u64::from(sample_rate_hz)
        .checked_mul(MAX_DECODED_AUDIO_DURATION_SECONDS)
        .ok_or(AudioPreparationError::ResourceLimitExceeded)
}

fn checked_interleaved_samples(
    frames: u64,
    channels: usize,
) -> Result<usize, AudioPreparationError> {
    let channels =
        u64::try_from(channels).map_err(|_| AudioPreparationError::ResourceLimitExceeded)?;
    let samples = frames
        .checked_mul(channels)
        .ok_or(AudioPreparationError::ResourceLimitExceeded)?;
    usize::try_from(samples).map_err(|_| AudioPreparationError::ResourceLimitExceeded)
}

fn copy_f32_samples(samples: &[f32]) -> Result<Vec<f32>, AudioPreparationError> {
    let mut copied = Vec::new();
    reserve_f32_capacity(&mut copied, samples.len())?;
    copied.extend_from_slice(samples);
    Ok(copied)
}
