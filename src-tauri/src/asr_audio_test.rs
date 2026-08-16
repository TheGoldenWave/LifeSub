use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::asr::audio::{
    AudioPreparationError, SampleRange, WORK_SAMPLE_RATE_HZ, checked_sample_index,
    decode_to_working_audio, resample_mono, sample_range_to_millis, sanitize_and_downmix,
    work_range_to_original_frames,
};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/asr")
        .join(name)
}

#[test]
fn fixture_manifest_freezes_source_license_hash_bytes_and_frames() {
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture("fixtures.json")).unwrap()).unwrap();
    assert_eq!(manifest["license_spdx"], "CC0-1.0");
    assert!(manifest["source"].as_str().unwrap().contains("Synthetic"));

    let formats: Vec<_> = manifest["fixtures"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|row| row["format"].as_str())
        .collect();
    for format in ["wav", "mp3", "m4a", "aac", "flac", "ogg"] {
        assert!(formats.contains(&format), "missing {format} fixture");
    }

    for row in manifest["fixtures"].as_array().unwrap() {
        let bytes = fs::read(fixture(row["file"].as_str().unwrap())).unwrap();
        assert_eq!(bytes.len() as u64, row["bytes"].as_u64().unwrap());
        assert_eq!(hex::encode(Sha256::digest(bytes)), row["sha256"]);
        assert!(row["nominal_frames"].as_u64().unwrap() > 0);
    }
}

#[test]
fn every_declared_import_format_decodes_real_frames_to_finite_16khz_pcm() {
    for name in [
        "tone.wav",
        "tone.mp3",
        "tone.m4a",
        "tone.aac",
        "tone.flac",
        "tone.ogg",
    ] {
        let audio = decode_to_working_audio(&fixture(name)).unwrap();
        assert_eq!(audio.sample_rate_hz, WORK_SAMPLE_RATE_HZ, "{name}");
        assert_eq!(audio.source_sample_rate_hz, 48_000, "{name}");
        assert!(audio.source_frames >= 24_000, "{name}");
        assert!((500..=550).contains(&audio.duration_ms), "{name}");
        assert!(!audio.samples.is_empty(), "{name}");
        let expected_work_frames =
            (audio.source_frames * u64::from(WORK_SAMPLE_RATE_HZ)).div_ceil(48_000);
        assert_eq!(audio.samples.len() as u64, expected_work_frames, "{name}");
        assert_eq!(
            audio.duration_ms,
            (audio.source_frames * 1_000).div_ceil(48_000)
        );
        assert!(audio.samples.iter().all(|sample| sample.is_finite()));
        assert!(
            audio
                .samples
                .iter()
                .all(|sample| (-1.0..=1.0).contains(sample))
        );
    }
}

#[test]
fn multichannel_downmix_is_an_arithmetic_mean() {
    let audio = decode_to_working_audio(&fixture("multichannel-3ch.wav")).unwrap();
    assert_eq!(audio.source_channels, 3);
    assert_eq!(audio.samples.len(), 1_600);
    let center = audio.samples[audio.samples.len() / 2];
    assert!((center - 0.2).abs() < 0.001, "center={center}");
}

#[test]
fn sanitization_replaces_non_finite_values_then_clamps_the_mean() {
    let mixed = sanitize_and_downmix(&[2.0, f32::NAN, -2.0, f32::INFINITY, 0.75, 0.75], 2).unwrap();
    assert_eq!(mixed, vec![1.0, -1.0, 0.75]);
}

#[test]
fn process_all_resampling_compensates_delay_and_preserves_impulse_position() {
    let audio = decode_to_working_audio(&fixture("impulse-48k.wav")).unwrap();
    assert_eq!(audio.samples.len(), 16_000);
    let peak = audio
        .samples
        .iter()
        .enumerate()
        .max_by(|left, right| left.1.abs().total_cmp(&right.1.abs()))
        .unwrap();
    assert!((3_999..=4_001).contains(&peak.0), "peak={}", peak.0);
    assert!(
        audio.samples[..3_900]
            .iter()
            .all(|value| value.abs() < 0.001)
    );

    let unchanged = resample_mono(&[0.25, -0.25], 16_000).unwrap();
    assert_eq!(unchanged, vec![0.25, -0.25]);
}

#[test]
fn half_open_time_mapping_uses_checked_floor_start_and_ceil_end() {
    let millis = sample_range_to_millis(SampleRange::new(1, 45).unwrap(), 44_100, 45).unwrap();
    assert_eq!(millis.start_ms, 0);
    assert_eq!(millis.end_ms, 2);

    let original =
        work_range_to_original_frames(SampleRange::new(1, 2).unwrap(), 44_100, 44_100).unwrap();
    assert_eq!(original, SampleRange::new(2, 6).unwrap());

    assert_eq!(
        sample_range_to_millis(SampleRange::new(1, 2).unwrap(), 0, 2),
        Err(AudioPreparationError::InvalidSampleRate)
    );
    assert_eq!(
        sample_range_to_millis(SampleRange::new(1, 3).unwrap(), 16_000, 2),
        Err(AudioPreparationError::InvalidRange)
    );
    assert_eq!(
        sample_range_to_millis(
            SampleRange::new(u64::MAX - 1, u64::MAX).unwrap(),
            1,
            u64::MAX
        ),
        Err(AudioPreparationError::ArithmeticOverflow)
    );
    assert_eq!(
        SampleRange { start: 10, end: 10 }.checked_len(),
        Err(AudioPreparationError::InvalidRange)
    );
    assert_eq!(
        checked_sample_index(6, 5),
        Err(AudioPreparationError::IndexOutOfRange)
    );
}
