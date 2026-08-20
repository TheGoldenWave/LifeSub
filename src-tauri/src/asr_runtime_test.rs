/// Runtime version tests verify that the pinned sherpa-onnx 1.13.5 static build
/// reports the correct version and git commit SHA-1 at runtime.
/// These tests are gated behind the `asr-runtime` feature because they depend on
/// the native sherpa-onnx library being linked.
///
/// Real-model fixture tests load the Gate manifest, decode audio, run
/// SenseVoice and Whisper providers, and compute CER/WER/key-phrase/boundary
/// metrics. They require `LIFESUB_ASR_MODEL_DIR` to point at installed models
/// and the WAV fixtures to exist at the paths declared in the manifest.

#[cfg(feature = "asr-runtime")]
#[test]
fn sherpa_runtime_reports_the_pinned_build() {
    assert_eq!(crate::asr::runtime_version(), "1.13.5");
    assert_eq!(
        crate::asr::runtime_git_sha1(),
        "3dc7c569f31ca2cd4a20ed6f7db780327e6714c5"
    );
}

// ---------------------------------------------------------------------------
// Real-model fixture tests — gated behind asr-runtime
// ---------------------------------------------------------------------------

#[cfg(feature = "asr-runtime")]
mod real_model_tests {
    use std::path::{Path, PathBuf};

    use crate::asr::gate_metrics::{
        self, compute_metrics, GateFixture, GateFixtureSegment, GateManifest,
        PredictedSegment, MAX_BOUNDARY_ERROR_MAX_MS, MEDIAN_BOUNDARY_ERROR_MAX_MS,
    };
    use crate::asr::provider::{AsrProvider, AsrRequest, AudioSlice, CancellationToken};
    use crate::asr::settings::{AsrLanguage, AsrProviderKind, AsrProviderOptions, WhisperTask};

    /// Resolve the project root from the manifest directory.
    ///
    /// In tests, `CARGO_MANIFEST_DIR` points to `src-tauri/`, so we go up one
    /// level to reach the project root where `tests/` lives.
    fn project_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("CARGO_MANIFEST_DIR has no parent")
            .to_path_buf()
    }

    /// Resolve the model directory from the `LIFESUB_ASR_MODEL_DIR` env var.
    fn model_dir() -> Option<PathBuf> {
        std::env::var("LIFESUB_ASR_MODEL_DIR").ok().map(PathBuf::from)
    }

    /// Load the Gate fixture manifest from the version-controlled JSON file.
    fn load_manifest() -> GateManifest {
        let manifest_path = project_root().join("tests/fixtures/asr/fixture-manifest.json");
        let data = std::fs::read_to_string(&manifest_path)
            .unwrap_or_else(|e| panic!("failed to read fixture manifest at {}: {e}", manifest_path.display()));
        serde_json::from_str(&data)
            .unwrap_or_else(|e| panic!("failed to parse fixture manifest: {e}"))
    }

    /// Read and hash a fixture audio file.
    fn read_fixture_audio(fixture_path: &str) -> Vec<u8> {
        let full_path = project_root().join(fixture_path);
        std::fs::read(&full_path)
            .unwrap_or_else(|e| panic!("failed to read fixture audio at {}: {e}", full_path.display()))
    }

    /// Build a SenseVoice provider from the model directory.
    fn build_sense_voice(model_dir: &Path) -> Box<dyn AsrProvider> {
        let sense_voice_dir = model_dir.join("asr").join("sense_voice")
            .join("sense-voice-small-int8-2024-07-17");
        // Find the actual install directory (versioned path)
        let install_dir = find_installed_model_dir(&sense_voice_dir);
        crate::asr::sense_voice::build_sense_voice_provider(&install_dir, 4)
            .expect("failed to build SenseVoice provider")
    }

    /// Build a Whisper Tiny provider from the model directory.
    fn build_whisper_tiny(model_dir: &Path) -> Box<dyn AsrProvider> {
        let whisper_dir = model_dir.join("asr").join("whisper")
            .join("whisper-tiny");
        let install_dir = find_installed_model_dir(&whisper_dir);
        crate::asr::whisper::build_whisper_provider(
            &install_dir,
            "tiny-encoder.onnx",
            "tiny-decoder.onnx",
            "tiny-tokens.txt",
            "whisper-tiny",
            4,
        )
        .expect("failed to build Whisper Tiny provider")
    }

    /// Find the versioned install directory inside a model directory.
    ///
    /// Model directories are versioned as `<manifest-version>-<archive-hash>`.
    /// This function finds the first subdirectory that matches this pattern.
    fn find_installed_model_dir(base: &Path) -> PathBuf {
        if base.exists() && base.join("model.int8.onnx").exists() {
            return base.to_path_buf();
        }
        if base.exists() && base.join("tiny-encoder.onnx").exists() {
            return base.to_path_buf();
        }
        // Look for versioned subdirectories
        if let Ok(entries) = std::fs::read_dir(base) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    return path;
                }
            }
        }
        panic!("no installed model found at {}", base.display());
    }

    /// Transcribe audio using the given provider.
    fn transcribe_audio(
        provider: &dyn AsrProvider,
        audio_data: &[u8],
        language: AsrLanguage,
        options: AsrProviderOptions,
    ) -> Vec<PredictedSegment> {
        let decoded = crate::asr::audio::decode_audio(audio_data)
            .expect("failed to decode fixture audio");

        let audio_slice = AudioSlice {
            samples: &decoded.samples,
            sample_rate: decoded.sample_rate,
        };

        let request = AsrRequest {
            language,
            options,
            num_threads: 4,
        };

        let cancellation = CancellationToken::new();

        let result = provider
            .transcribe(audio_slice, &request, &cancellation)
            .expect("transcription failed");

        // Without VAD, produce a single segment covering the full duration
        vec![PredictedSegment {
            text: result.text,
            start_ms: 0,
            end_ms: decoded.duration_ms,
        }]
    }

    /// Verify that the fixture audio file exists and is readable.
    fn assert_fixture_exists(fixture: &GateFixture) {
        let full_path = project_root().join(&fixture.path);
        assert!(
            full_path.exists(),
            "fixture audio file missing: {} (run the fixture generation script first)",
            full_path.display()
        );
    }

    // -------------------------------------------------------------------
    // Test: fixture manifest is valid and parseable
    // -------------------------------------------------------------------

    #[test]
    fn fixture_manifest_is_valid() {
        let manifest = load_manifest();
        assert!(
            !manifest.gate_fixtures.is_empty(),
            "gate_fixtures must contain at least one real speech fixture"
        );

        for fixture in &manifest.gate_fixtures {
            assert!(!fixture.id.is_empty(), "fixture id must not be empty");
            assert!(!fixture.path.is_empty(), "fixture path must not be empty");
            assert!(
                !fixture.expected_transcript.is_empty(),
                "fixture {} must have expected_transcript",
                fixture.id
            );
            assert!(
                !fixture.expected_segments.is_empty(),
                "fixture {} must have expected_segments",
                fixture.id
            );
            assert!(
                !fixture.test_providers.is_empty(),
                "fixture {} must have test_providers",
                fixture.id
            );
        }
    }

    // -------------------------------------------------------------------
    // Test: metric computation with empty/synthetic data
    // -------------------------------------------------------------------

    #[test]
    fn compute_metrics_synthetic_fixture() {
        let fixture = GateFixture {
            id: "test".to_string(),
            path: "test.wav".to_string(),
            sha256: None,
            language: "zh".to_string(),
            expected_transcript: "你好世界".to_string(),
            expected_segments: vec![GateFixtureSegment {
                text: "你好世界".to_string(),
                start_ms: 0,
                end_ms: 2000,
            }],
            key_phrases: vec!["你好".to_string()],
            test_providers: vec!["sense_voice".to_string()],
            cer_max: Some(0.20),
            wer_max: Some(0.20),
        };

        let predicted = vec![PredictedSegment {
            text: "你好世界".to_string(),
            start_ms: 100,
            end_ms: 1800,
        }];

        let metrics = compute_metrics(&fixture, &predicted, "sense_voice", "test-model");
        assert!(metrics.all_pass);
        assert!(metrics.cer_pass);
        assert!(metrics.key_phrase_pass);
        assert!(metrics.boundary_pass);
        assert_eq!(metrics.cer, 0.0);
    }

    // -------------------------------------------------------------------
    // Test: CER computation with Chinese text
    // -------------------------------------------------------------------

    #[test]
    fn cer_computation_chinese() {
        let reference = "你好世界这是一个测试";
        let hypothesis = "你好世界这是一个测试";
        let cer = gate_metrics::compute_cer(reference, hypothesis);
        assert_eq!(cer, 0.0);

        // One character error
        let cer = gate_metrics::compute_cer("你好世界", "你好地球");
        assert!((cer - 0.5).abs() < 0.01); // 1 error / 2 chars after removing punct
    }

    // -------------------------------------------------------------------
    // Test: WER computation with English text
    // -------------------------------------------------------------------

    #[test]
    fn wer_computation_english() {
        let reference = "this is a test";
        let hypothesis = "this is a test";
        let wer = gate_metrics::compute_wer(reference, hypothesis);
        assert_eq!(wer, 0.0);

        // One word substitution
        let wer = gate_metrics::compute_wer("this is a test", "this is a demo");
        assert!((wer - 0.25).abs() < 0.01); // 1 error / 4 words
    }

    // -------------------------------------------------------------------
    // Test: key phrase detection in mixed-language text
    // -------------------------------------------------------------------

    #[test]
    fn key_phrase_detection_zh_en() {
        let hypothesis = "今天的会议我们将讨论 machine learning 的最新进展";
        let phrases = vec![
            "machine learning".to_string(),
            "最新进展".to_string(),
        ];
        let (found, missing) = gate_metrics::check_key_phrases(hypothesis, &phrases);
        assert_eq!(found.len(), 2);
        assert!(missing.is_empty());
    }

    // -------------------------------------------------------------------
    // Real model tests — require LIFESUB_ASR_MODEL_DIR and fixture WAVs
    // -------------------------------------------------------------------

    /// SenseVoice CER gate: transcribe zh.wav, verify CER ≤ 20%.
    #[test]
    fn sense_voice_zh_cer_gate() {
        let manifest = load_manifest();
        let zh_fixture = manifest
            .gate_fixtures
            .iter()
            .find(|f| f.id == "zh-mandarin")
            .expect("zh-mandarin fixture not found in manifest");

        assert_fixture_exists(zh_fixture);

        let model_dir = model_dir().expect("LIFESUB_ASR_MODEL_DIR must be set for real model tests");
        let provider = build_sense_voice(&model_dir);

        let audio_data = read_fixture_audio(&zh_fixture.path);
        let segments = transcribe_audio(
            provider.as_ref(),
            &audio_data,
            AsrLanguage::Zh,
            AsrProviderOptions::SenseVoice { use_itn: true },
        );

        let metrics = compute_metrics(zh_fixture, &segments, "sense_voice", "sense-voice-small-int8-2024-07-17");

        assert!(
            metrics.cer_pass,
            "SenseVoice CER {:.4} exceeds threshold: expected ≤ {:.4}",
            metrics.cer,
            zh_fixture.cer_max.unwrap_or(0.20)
        );
        assert!(
            metrics.key_phrase_pass,
            "SenseVoice key phrases missing: {:?}",
            metrics.key_phrases_missing
        );
        assert!(
            metrics.boundary_pass,
            "SenseVoice boundary errors: median={:?}ms, max={:?}ms (thresholds: median≤{}ms, max≤{}ms)",
            metrics.median_boundary_error_ms,
            metrics.max_boundary_error_ms,
            MEDIAN_BOUNDARY_ERROR_MAX_MS,
            MAX_BOUNDARY_ERROR_MAX_MS,
        );
    }

    /// Whisper WER gate: transcribe en.wav, verify WER ≤ 20%.
    #[test]
    fn whisper_en_wer_gate() {
        let manifest = load_manifest();
        let en_fixture = manifest
            .gate_fixtures
            .iter()
            .find(|f| f.id == "en-english")
            .expect("en-english fixture not found in manifest");

        assert_fixture_exists(en_fixture);

        let model_dir = model_dir().expect("LIFESUB_ASR_MODEL_DIR must be set for real model tests");
        let provider = build_whisper_tiny(&model_dir);

        let audio_data = read_fixture_audio(&en_fixture.path);
        let segments = transcribe_audio(
            provider.as_ref(),
            &audio_data,
            AsrLanguage::En,
            AsrProviderOptions::Whisper {
                task: WhisperTask::Transcribe,
            },
        );

        let metrics = compute_metrics(en_fixture, &segments, "whisper", "whisper-tiny");

        assert!(
            metrics.wer_pass,
            "Whisper WER {:.4} exceeds threshold: expected ≤ {:.4}",
            metrics.wer,
            en_fixture.wer_max.unwrap_or(0.20)
        );
        assert!(
            metrics.key_phrase_pass,
            "Whisper key phrases missing: {:?}",
            metrics.key_phrases_missing
        );
        assert!(
            metrics.boundary_pass,
            "Whisper boundary errors: median={:?}ms, max={:?}ms (thresholds: median≤{}ms, max≤{}ms)",
            metrics.median_boundary_error_ms,
            metrics.max_boundary_error_ms,
            MEDIAN_BOUNDARY_ERROR_MAX_MS,
            MAX_BOUNDARY_ERROR_MAX_MS,
        );
    }

    /// Whisper mixed-language key phrase gate: transcribe zh-en.wav, verify
    /// all key phrases (both Chinese and English) are present.
    #[test]
    fn whisper_zh_en_key_phrase_gate() {
        let manifest = load_manifest();
        let zh_en_fixture = manifest
            .gate_fixtures
            .iter()
            .find(|f| f.id == "zh-en-mixed")
            .expect("zh-en-mixed fixture not found in manifest");

        assert_fixture_exists(zh_en_fixture);

        let model_dir = model_dir().expect("LIFESUB_ASR_MODEL_DIR must be set for real model tests");
        let provider = build_whisper_tiny(&model_dir);

        let audio_data = read_fixture_audio(&zh_en_fixture.path);
        let segments = transcribe_audio(
            provider.as_ref(),
            &audio_data,
            AsrLanguage::Zh,
            AsrProviderOptions::Whisper {
                task: WhisperTask::Transcribe,
            },
        );

        let metrics = compute_metrics(zh_en_fixture, &segments, "whisper", "whisper-tiny");

        assert!(
            metrics.all_key_phrases_present,
            "Whisper mixed-language key phrases missing: {:?} (found: {:?})",
            metrics.key_phrases_missing,
            metrics.key_phrases_found
        );
        assert!(
            metrics.boundary_pass,
            "Whisper mixed boundary errors: median={:?}ms, max={:?}ms",
            metrics.median_boundary_error_ms,
            metrics.max_boundary_error_ms,
        );
    }
}