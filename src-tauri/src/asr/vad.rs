use crate::asr::audio::{AudioPreparationError, SampleRange, checked_sample_index};
use crate::asr::manifest::VadManifest;
use crate::asr::model_manager::ExecutableInstallationLease;

use std::fs::File;
use std::path::{Path, PathBuf};

pub const PADDING: u64 = 3_200;
pub const MAX_PROVIDER_WINDOW: u64 = 400_000;
pub const ENERGY_FRAME: u64 = 320;
pub const ENERGY_HALF_FRAME: u64 = 160;
pub const SPLIT_SEARCH_RADIUS: u64 = 32_000;

#[derive(Clone, Debug, PartialEq)]
pub struct ProviderWindow {
    pub core: SampleRange,
    pub inference: SampleRange,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EvidenceUtterance {
    pub evidence: SampleRange,
    pub windows: Vec<ProviderWindow>,
}

pub struct VerifiedVadModel {
    path: PathBuf,
    _file: File,
    _lease: ExecutableInstallationLease,
}

impl VerifiedVadModel {
    pub fn new(lease: ExecutableInstallationLease) -> Result<Self, AudioPreparationError> {
        if lease.model_id() != crate::asr::manifest::vad_manifest().id || !lease.is_fd_anchored() {
            return Err(AudioPreparationError::InvalidVadConfig);
        }
        lease
            .revalidate_execution_boundary()
            .map_err(|_| AudioPreparationError::DetectorFailed)?;
        let (file, path) = lease
            .open_execution_path(Path::new("silero_vad.onnx"))
            .map_err(|_| AudioPreparationError::DetectorFailed)?;
        lease
            .revalidate_execution_boundary()
            .map_err(|_| AudioPreparationError::DetectorFailed)?;
        Ok(Self {
            path,
            _file: file,
            _lease: lease,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    #[cfg(feature = "asr-runtime")]
    fn revalidate(&self) -> Result<(), AudioPreparationError> {
        self._lease
            .revalidate_execution_boundary()
            .map_err(|_| AudioPreparationError::DetectorFailed)
    }

    #[cfg(test)]
    pub(crate) fn validation_count_for_test(&self) -> usize {
        self._lease.validation_count()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct VadRuntimeConfig {
    pub threshold: f32,
    pub min_silence_duration_seconds: f32,
    pub min_speech_duration_seconds: f32,
    pub max_speech_duration_seconds: f32,
    pub window_size_samples: i32,
    pub sample_rate_hz: i32,
    pub num_threads: i32,
    pub provider: String,
}

impl VadRuntimeConfig {
    pub fn canonical() -> Self {
        Self {
            threshold: 0.5,
            min_silence_duration_seconds: 0.5,
            min_speech_duration_seconds: 0.25,
            max_speech_duration_seconds: 20.0,
            window_size_samples: 512,
            sample_rate_hz: 16_000,
            num_threads: 1,
            provider: "cpu".to_owned(),
        }
    }

    pub fn from_manifest(manifest: &VadManifest) -> Result<Self, AudioPreparationError> {
        let config = Self {
            threshold: manifest.threshold,
            min_silence_duration_seconds: manifest.min_silence_duration_seconds,
            min_speech_duration_seconds: manifest.min_speech_duration_seconds,
            max_speech_duration_seconds: manifest.max_speech_duration_seconds,
            window_size_samples: manifest.window_size_samples,
            sample_rate_hz: manifest.sample_rate_hz,
            num_threads: manifest.num_threads,
            provider: manifest.provider.to_owned(),
        };
        if config != Self::canonical() {
            return Err(AudioPreparationError::InvalidVadConfig);
        }
        Ok(config)
    }

    #[cfg(feature = "asr-runtime")]
    pub fn to_sherpa_config(
        &self,
        model_path: &std::path::Path,
    ) -> Result<sherpa_onnx::VadModelConfig, AudioPreparationError> {
        let model = model_path
            .to_str()
            .ok_or(AudioPreparationError::DetectorFailed)?
            .to_owned();
        Ok(sherpa_onnx::VadModelConfig {
            silero_vad: sherpa_onnx::SileroVadModelConfig {
                model: Some(model),
                threshold: self.threshold,
                min_silence_duration: self.min_silence_duration_seconds,
                min_speech_duration: self.min_speech_duration_seconds,
                window_size: self.window_size_samples,
                max_speech_duration: self.max_speech_duration_seconds,
            },
            ten_vad: sherpa_onnx::TenVadModelConfig {
                model: None,
                threshold: self.threshold,
                min_silence_duration: self.min_silence_duration_seconds,
                min_speech_duration: self.min_speech_duration_seconds,
                window_size: self.window_size_samples,
                max_speech_duration: self.max_speech_duration_seconds,
            },
            sample_rate: self.sample_rate_hz,
            num_threads: self.num_threads,
            provider: Some(self.provider.clone()),
            debug: false,
        })
    }
}

pub trait SpeechDetector {
    fn detect(&mut self, samples: &[f32]) -> Result<Vec<SampleRange>, AudioPreparationError>;
}

#[cfg(feature = "asr-runtime")]
pub struct SherpaSpeechDetector {
    detector: sherpa_onnx::VoiceActivityDetector,
    window_size_samples: usize,
    _model: VerifiedVadModel,
}

#[cfg(feature = "asr-runtime")]
impl SherpaSpeechDetector {
    pub fn new(
        verified_model: VerifiedVadModel,
        config: VadRuntimeConfig,
    ) -> Result<Self, AudioPreparationError> {
        verified_model.revalidate()?;
        let native = config.to_sherpa_config(verified_model.path())?;
        let detector = sherpa_onnx::VoiceActivityDetector::create(
            &native,
            config.max_speech_duration_seconds + config.min_silence_duration_seconds + 1.0,
        )
        .ok_or(AudioPreparationError::DetectorFailed)?;
        verified_model.revalidate()?;
        let window_size_samples = usize::try_from(config.window_size_samples)
            .map_err(|_| AudioPreparationError::InvalidVadConfig)?;
        if window_size_samples == 0 {
            return Err(AudioPreparationError::InvalidVadConfig);
        }
        Ok(Self {
            detector,
            window_size_samples,
            _model: verified_model,
        })
    }

    fn drain(&self, ranges: &mut Vec<SampleRange>) -> Result<(), AudioPreparationError> {
        while let Some(segment) = self.detector.front() {
            let start = u64::try_from(segment.start())
                .map_err(|_| AudioPreparationError::DetectorFailed)?;
            let length =
                u64::try_from(segment.n()).map_err(|_| AudioPreparationError::DetectorFailed)?;
            let end = start
                .checked_add(length)
                .ok_or(AudioPreparationError::ArithmeticOverflow)?;
            ranges.push(SampleRange::new(start, end)?);
            self.detector.pop();
        }
        Ok(())
    }
}

#[cfg(feature = "asr-runtime")]
impl SpeechDetector for SherpaSpeechDetector {
    fn detect(&mut self, samples: &[f32]) -> Result<Vec<SampleRange>, AudioPreparationError> {
        self.detector.reset();
        let mut ranges = Vec::new();
        for window in samples.chunks(self.window_size_samples) {
            self.detector.accept_waveform(window);
            self.drain(&mut ranges)?;
        }
        self.detector.flush();
        self.drain(&mut ranges)?;
        Ok(ranges)
    }
}

pub struct FakeSpeechDetector {
    cores: Vec<SampleRange>,
}

impl FakeSpeechDetector {
    pub fn new(cores: Vec<SampleRange>) -> Self {
        Self { cores }
    }
}

impl SpeechDetector for FakeSpeechDetector {
    fn detect(&mut self, _samples: &[f32]) -> Result<Vec<SampleRange>, AudioPreparationError> {
        Ok(self.cores.clone())
    }
}

pub fn select_boundary(latest_safe: u64, candidates: &[(u64, f64)]) -> u64 {
    candidates
        .iter()
        .min_by(|left, right| left.1.total_cmp(&right.1).then(left.0.cmp(&right.0)))
        .map(|candidate| candidate.0)
        .unwrap_or(latest_safe)
}

pub fn select_split_boundary(
    samples: &[f32],
    core: SampleRange,
    cursor: u64,
    latest_safe: u64,
    injected_candidates: Option<&[u64]>,
) -> Result<u64, AudioPreparationError> {
    let total_samples =
        u64::try_from(samples.len()).map_err(|_| AudioPreparationError::IndexOutOfRange)?;
    if core.start > cursor
        || cursor >= latest_safe
        || latest_safe >= core.end
        || core.end > total_samples
    {
        return Err(AudioPreparationError::InvalidRange);
    }
    let search_start = checked_sub_or_zero(latest_safe, SPLIT_SEARCH_RADIUS);
    let search_end = latest_safe
        .checked_add(SPLIT_SEARCH_RADIUS)
        .ok_or(AudioPreparationError::ArithmeticOverflow)?;
    let generated;
    let candidates = if let Some(candidates) = injected_candidates {
        candidates
    } else {
        generated = absolute_grid_candidates(search_start, latest_safe)?;
        &generated
    };
    let mut scored = Vec::new();
    for candidate in candidates.iter().copied() {
        let frame_start = match candidate.checked_sub(ENERGY_HALF_FRAME) {
            Some(value) => value,
            None => continue,
        };
        let frame_end = candidate
            .checked_add(ENERGY_HALF_FRAME)
            .ok_or(AudioPreparationError::ArithmeticOverflow)?;
        if candidate % ENERGY_FRAME != 0
            || candidate <= cursor
            || candidate > latest_safe
            || candidate < core.start
            || candidate >= core.end
            || candidate < search_start
            || candidate > search_end
            || frame_start < core.start
            || frame_end > core.end
            || frame_end > total_samples
        {
            continue;
        }
        let start = checked_sample_index(frame_start, samples.len())?;
        let end = usize::try_from(frame_end).map_err(|_| AudioPreparationError::IndexOutOfRange)?;
        if end > samples.len() {
            return Err(AudioPreparationError::IndexOutOfRange);
        }
        let sum = samples[start..end].iter().fold(0.0_f64, |sum, sample| {
            let value = f64::from(*sample);
            sum + value * value
        });
        scored.push((candidate, sum / ENERGY_FRAME as f64));
    }
    Ok(select_boundary(latest_safe, &scored))
}

pub fn partition_detector_cores(
    samples: &[f32],
    cores: &[SampleRange],
) -> Result<Vec<EvidenceUtterance>, AudioPreparationError> {
    validate_detector_cores(samples, cores)?;
    cores
        .iter()
        .copied()
        .map(|core| {
            Ok(EvidenceUtterance {
                evidence: core,
                windows: partition_core(samples, core)?,
            })
        })
        .collect()
}

pub fn partition_without_vad(samples: &[f32]) -> Result<EvidenceUtterance, AudioPreparationError> {
    let total_samples =
        u64::try_from(samples.len()).map_err(|_| AudioPreparationError::IndexOutOfRange)?;
    let evidence = SampleRange::new(0, total_samples)?;
    Ok(EvidenceUtterance {
        evidence,
        windows: partition_core(samples, evidence)?,
    })
}

fn validate_detector_cores(
    samples: &[f32],
    cores: &[SampleRange],
) -> Result<(), AudioPreparationError> {
    if cores.is_empty() {
        return Err(AudioPreparationError::InvalidDetectorCores);
    }
    let total_samples =
        u64::try_from(samples.len()).map_err(|_| AudioPreparationError::IndexOutOfRange)?;
    let mut previous: Option<SampleRange> = None;
    for core in cores {
        if core.start >= core.end || core.end > total_samples {
            return Err(AudioPreparationError::InvalidDetectorCores);
        }
        if let Some(previous_core) = previous
            && (core.start <= previous_core.start || core.start < previous_core.end)
        {
            return Err(AudioPreparationError::InvalidDetectorCores);
        }
        previous = Some(*core);
    }
    Ok(())
}

fn partition_core(
    samples: &[f32],
    core: SampleRange,
) -> Result<Vec<ProviderWindow>, AudioPreparationError> {
    let total_samples =
        u64::try_from(samples.len()).map_err(|_| AudioPreparationError::IndexOutOfRange)?;
    if core.start >= core.end || core.end > total_samples {
        return Err(AudioPreparationError::InvalidRange);
    }
    let mut windows = Vec::new();
    let mut cursor = core.start;
    while cursor < core.end {
        let inference_start = checked_sub_or_zero(cursor, PADDING);
        let padded_core_end = core
            .end
            .checked_add(PADDING)
            .ok_or(AudioPreparationError::ArithmeticOverflow)?
            .min(total_samples);
        let padded_len = padded_core_end
            .checked_sub(inference_start)
            .ok_or(AudioPreparationError::ArithmeticOverflow)?;
        if padded_len <= MAX_PROVIDER_WINDOW {
            windows.push(ProviderWindow {
                core: SampleRange::new(cursor, core.end)?,
                inference: SampleRange::new(inference_start, padded_core_end)?,
            });
            break;
        }

        let latest_safe = inference_start
            .checked_add(MAX_PROVIDER_WINDOW)
            .and_then(|value| value.checked_sub(PADDING))
            .ok_or(AudioPreparationError::ArithmeticOverflow)?;
        if latest_safe <= cursor || latest_safe >= core.end {
            return Err(AudioPreparationError::InvalidRange);
        }
        let boundary = select_split_boundary(samples, core, cursor, latest_safe, None)?;
        if boundary <= cursor || boundary > latest_safe {
            return Err(AudioPreparationError::InvalidRange);
        }
        let inference_end = boundary
            .checked_add(PADDING)
            .ok_or(AudioPreparationError::ArithmeticOverflow)?;
        let inference = SampleRange::new(inference_start, inference_end)?;
        if inference.checked_len()? > MAX_PROVIDER_WINDOW {
            return Err(AudioPreparationError::InvalidRange);
        }
        windows.push(ProviderWindow {
            core: SampleRange::new(cursor, boundary)?,
            inference,
        });
        cursor = boundary;
    }
    Ok(windows)
}

fn absolute_grid_candidates(start: u64, end: u64) -> Result<Vec<u64>, AudioPreparationError> {
    if start > end {
        return Ok(Vec::new());
    }
    let remainder = start % ENERGY_FRAME;
    let first = if remainder == 0 {
        start
    } else {
        start
            .checked_add(ENERGY_FRAME - remainder)
            .ok_or(AudioPreparationError::ArithmeticOverflow)?
    };
    let mut candidates = Vec::new();
    let mut candidate = first;
    while candidate <= end {
        candidates.push(candidate);
        candidate = match candidate.checked_add(ENERGY_FRAME) {
            Some(next) => next,
            None => break,
        };
    }
    Ok(candidates)
}

#[allow(clippy::manual_saturating_arithmetic)]
fn checked_sub_or_zero(value: u64, amount: u64) -> u64 {
    // The frozen contract requires checked arithmetic followed by an explicit zero clamp.
    value.checked_sub(amount).unwrap_or(0)
}
