#[cfg(feature = "asr-runtime")]
use std::path::PathBuf;

use crate::asr::audio::{AudioPreparationError, SampleRange};
use crate::asr::manifest::vad_manifest;
use crate::asr::vad::{
    ENERGY_FRAME, ENERGY_HALF_FRAME, FakeSpeechDetector, MAX_PROVIDER_WINDOW, PADDING,
    SPLIT_SEARCH_RADIUS, SpeechDetector, VadRuntimeConfig, partition_detector_cores,
    partition_without_vad, select_boundary, select_split_boundary,
};

const TOTAL: u64 = 500_000;

fn silence() -> Vec<f32> {
    vec![0.0; TOTAL as usize]
}

#[test]
fn orchestration_constants_are_frozen_and_vad_config_is_canonical() {
    assert_eq!(PADDING, 3_200);
    assert_eq!(MAX_PROVIDER_WINDOW, 400_000);
    assert_eq!(ENERGY_FRAME, 320);
    assert_eq!(ENERGY_HALF_FRAME, 160);
    assert_eq!(SPLIT_SEARCH_RADIUS, 32_000);

    let config = VadRuntimeConfig::canonical();
    assert_eq!(config.threshold, 0.5);
    assert_eq!(config.min_silence_duration_seconds, 0.5);
    assert_eq!(config.min_speech_duration_seconds, 0.25);
    assert_eq!(config.max_speech_duration_seconds, 20.0);
    assert_eq!(config.window_size_samples, 512);
    assert_eq!(config.sample_rate_hz, 16_000);
    assert_eq!(config.num_threads, 1);
    assert_eq!(config.provider, "cpu");
    assert_eq!(
        VadRuntimeConfig::from_manifest(vad_manifest()).unwrap(),
        config
    );

    let mut changed = *vad_manifest();
    changed.threshold = 0.6;
    assert_eq!(
        VadRuntimeConfig::from_manifest(&changed),
        Err(AudioPreparationError::InvalidVadConfig)
    );
}

#[test]
fn exact_padding_fit_does_not_split_but_one_more_sample_does() {
    let audio = silence();
    let exact = partition_detector_cores(&audio, &[SampleRange::new(0, 396_800).unwrap()]).unwrap();
    assert_eq!(exact[0].windows.len(), 1);
    assert_eq!(
        exact[0].windows[0].inference,
        SampleRange::new(0, 400_000).unwrap()
    );

    let over = partition_detector_cores(&audio, &[SampleRange::new(0, 396_801).unwrap()]).unwrap();
    assert!(over[0].windows.len() > 1);

    let nonzero =
        partition_detector_cores(&audio, &[SampleRange::new(10_000, 403_600).unwrap()]).unwrap();
    assert_eq!(nonzero[0].windows.len(), 1);
    assert_eq!(
        nonzero[0].windows[0].inference,
        SampleRange::new(6_800, 406_800).unwrap()
    );
}

#[test]
fn energy_split_uses_absolute_grid_mean_square_and_earliest_tie() {
    let core = SampleRange::new(0, 450_000).unwrap();
    let mut audio = vec![1.0; TOTAL as usize];
    audio[380_640..380_960].fill(0.1);
    assert_eq!(
        select_split_boundary(&audio, core, 0, 396_800, None).unwrap(),
        380_800
    );

    audio[380_960..381_280].fill(0.1);
    assert_eq!(
        select_split_boundary(&audio, core, 0, 396_800, None).unwrap(),
        380_800
    );
    assert_eq!(select_boundary(396_800, &[]), 396_800);
    assert_eq!(
        select_split_boundary(&audio, core, 390_000, 396_800, Some(&[0, 320])).unwrap(),
        396_800
    );
}

#[test]
fn long_cores_cover_exactly_and_only_context_windows_overlap() {
    let total = 1_200_000_u64;
    let audio = vec![0.5; total as usize];
    let core = SampleRange::new(10_000, 1_190_000).unwrap();
    let utterances = partition_detector_cores(&audio, &[core]).unwrap();
    let windows = &utterances[0].windows;
    assert_eq!(utterances[0].evidence, core);
    assert_eq!(windows.first().unwrap().core.start, core.start);
    assert_eq!(windows.last().unwrap().core.end, core.end);
    assert!(windows.iter().all(|window| {
        window.core.start < window.core.end
            && window.inference.checked_len().unwrap() <= MAX_PROVIDER_WINDOW
    }));
    assert!(windows.windows(2).all(|pair| {
        pair[0].core.end == pair[1].core.start && pair[0].inference.end >= pair[1].inference.start
    }));
}

#[test]
fn tail_padding_clamps_without_an_empty_trailing_window() {
    let audio = silence();
    let utterances =
        partition_detector_cores(&audio, &[SampleRange::new(100_000, TOTAL).unwrap()]).unwrap();
    let last = utterances[0].windows.last().unwrap();
    assert_eq!(last.core.end, TOTAL);
    assert_eq!(last.inference.end, TOTAL);
    assert!(last.core.start < last.core.end);
}

#[test]
fn detector_cores_are_strictly_validated_without_merging_gaps_or_adjacency() {
    let audio = silence();
    let invalid = [
        vec![],
        vec![SampleRange { start: 10, end: 10 }],
        vec![SampleRange {
            start: 0,
            end: TOTAL + 1,
        }],
        vec![
            SampleRange::new(20, 30).unwrap(),
            SampleRange::new(10, 15).unwrap(),
        ],
        vec![
            SampleRange::new(10, 30).unwrap(),
            SampleRange::new(20, 40).unwrap(),
        ],
    ];
    for cores in invalid {
        assert_eq!(
            partition_detector_cores(&audio, &cores),
            Err(AudioPreparationError::InvalidDetectorCores)
        );
    }

    let cores = [
        SampleRange::new(10, 20).unwrap(),
        SampleRange::new(20, 30).unwrap(),
        SampleRange::new(40, 50).unwrap(),
    ];
    let utterances = partition_detector_cores(&audio, &cores).unwrap();
    assert_eq!(utterances.len(), 3);
    assert_eq!(
        utterances
            .iter()
            .map(|item| item.evidence)
            .collect::<Vec<_>>(),
        cores
    );
}

#[test]
fn vad_off_exposes_one_evidence_utterance_with_internal_windows() {
    let audio = vec![0.0; 1_000_000];
    let utterance = partition_without_vad(&audio).unwrap();
    assert_eq!(utterance.evidence, SampleRange::new(0, 1_000_000).unwrap());
    assert!(utterance.windows.len() > 1);
    assert_eq!(utterance.windows.first().unwrap().core.start, 0);
    assert_eq!(utterance.windows.last().unwrap().core.end, 1_000_000);
}

#[test]
fn fake_detector_is_trait_driven_and_errors_are_stable() {
    let expected = vec![SampleRange::new(100, 200).unwrap()];
    let mut detector = FakeSpeechDetector::new(expected.clone());
    let trait_object: &mut dyn SpeechDetector = &mut detector;
    assert_eq!(trait_object.detect(&[0.0; 320]).unwrap(), expected);
    assert_eq!(
        AudioPreparationError::InvalidDetectorCores.code(),
        "invalid_detector_cores"
    );
    assert_eq!(
        AudioPreparationError::IndexOutOfRange.code(),
        "audio_index_out_of_range"
    );
}

#[cfg(feature = "asr-runtime")]
#[test]
fn native_config_maps_every_explicit_field_without_lifesub_window_constants() {
    let model = PathBuf::from("/verified/models/silero_vad.onnx");
    let config = VadRuntimeConfig::canonical();
    let native = config.to_sherpa_config(&model).unwrap();
    assert_eq!(native.silero_vad.model.as_deref(), model.to_str());
    assert_eq!(native.silero_vad.threshold, 0.5);
    assert_eq!(native.silero_vad.min_silence_duration, 0.5);
    assert_eq!(native.silero_vad.min_speech_duration, 0.25);
    assert_eq!(native.silero_vad.max_speech_duration, 20.0);
    assert_eq!(native.silero_vad.window_size, 512);
    assert_eq!(native.sample_rate, 16_000);
    assert_eq!(native.num_threads, 1);
    assert_eq!(native.provider.as_deref(), Some("cpu"));
    assert!(!native.debug);
}
