use chrono::Utc;
use std::path::PathBuf;
use std::sync::{Arc, Barrier, Mutex, mpsc};
use std::time::Duration;

use crate::asr::audio::WorkingAudio;
use crate::asr::job::{
    ChunkExecutionSnapshot, ExecutionParametersSnapshot, ExecutionSnapshot, ModelExecutionSnapshot,
    SystemClock, VadExecutionSnapshot,
};
use crate::asr::manifest::model_registry;
use crate::asr::native_engine::{NativePrepared, PreparedNativeAsr, ProductionNativeAsrEngine};
use crate::asr::provider::{
    AsrProvider, AudioSlice, BackendKind, CancellationToken, DeviceIdentity, NativeBackend,
    NativeBackendFactory, NativeRequest, Provider, ProviderError, ProviderFactory, ProviderOptions,
    ProviderRequest, QualificationEvidence,
};
use crate::asr::settings::AsrSettings;
use crate::asr::vad::{MAX_PROVIDER_WINDOW, partition_without_vad};
use crate::asr::worker::{ExecutionEngine, ExecutionRun};
use crate::domain::{AsrLanguage, AsrProviderKind, AudioSource};
use crate::service::CoreRuntime;

const MODEL_ID: &str = "sense-voice-small-int8-2024-07-17";

#[derive(Clone, Default)]
struct RecordingBackendFactory {
    calls: Arc<Mutex<usize>>,
    first_call_barriers: Option<Arc<(Barrier, Barrier)>>,
    dropped: Option<mpsc::Sender<()>>,
}

impl NativeBackendFactory for RecordingBackendFactory {
    fn create(&self, _request: &NativeRequest) -> Result<Box<dyn NativeBackend>, ProviderError> {
        Ok(Box::new(RecordingBackend {
            calls: self.calls.clone(),
            first_call_barriers: self.first_call_barriers.clone(),
            dropped: self.dropped.clone(),
        }))
    }
}

struct RecordingBackend {
    calls: Arc<Mutex<usize>>,
    first_call_barriers: Option<Arc<(Barrier, Barrier)>>,
    dropped: Option<mpsc::Sender<()>>,
}

impl NativeBackend for RecordingBackend {
    fn transcribe(&mut self, _audio: AudioSlice<'_>) -> Result<String, ProviderError> {
        let call = {
            let mut calls = self.calls.lock().unwrap();
            *calls += 1;
            *calls
        };
        if call == 1
            && let Some(barriers) = &self.first_call_barriers
        {
            barriers.0.wait();
            barriers.1.wait();
        }
        Ok(format!("window {call}"))
    }
}

impl Drop for RecordingBackend {
    fn drop(&mut self) {
        if let Some(dropped) = &self.dropped {
            let _ = dropped.send(());
        }
    }
}

fn provider(factory: RecordingBackendFactory) -> Provider {
    let manifest = model_registry().model(MODEL_ID).unwrap();
    ProviderFactory::new(factory)
        .create(ProviderRequest {
            provider: AsrProviderKind::SenseVoice,
            model_id: MODEL_ID.to_owned(),
            manifest_version: manifest.manifest_version.to_owned(),
            bundle_identity: manifest.bundle.identity_sha256.to_owned(),
            install_dir: PathBuf::from("/qualified/model"),
            language: "auto".to_owned(),
            num_threads: 2,
            options: ProviderOptions::SenseVoice { use_itn: true },
            qualification: QualificationEvidence::matching(manifest, DeviceIdentity::current()),
        })
        .unwrap()
}

fn working_audio(sample_count: usize) -> WorkingAudio {
    WorkingAudio {
        samples: vec![0.1; sample_count],
        sample_rate_hz: 16_000,
        source_sample_rate_hz: 16_000,
        source_channels: 1,
        source_frames: sample_count as u64,
        duration_ms: (sample_count as u64).div_ceil(16),
    }
}

fn snapshot(with_vad: bool) -> ExecutionSnapshot {
    let manifest = model_registry().model(MODEL_ID).unwrap();
    let mut settings = AsrSettings::sense_voice(MODEL_ID)
        .with_language(AsrLanguage::new("auto").unwrap())
        .with_num_threads(2);
    settings.vad_enabled = with_vad;
    let parameters_json = serde_json::to_string(&settings).unwrap();
    ExecutionSnapshot {
        job_id: "asr_test".to_owned(),
        session_id: "rec_test".to_owned(),
        chunk: ChunkExecutionSnapshot {
            id: "chk_test".to_owned(),
            source: AudioSource::Imported,
            relative_path: "audio/test.wav".to_owned(),
            sha256: "a".repeat(64),
            byte_length: 128,
            session_offset_ms: 0,
            duration_ms: Some(31_250),
        },
        model: ModelExecutionSnapshot {
            provider: manifest.provider,
            model_id: manifest.id.to_owned(),
            manifest_version: manifest.manifest_version.to_owned(),
            bundle_identity: manifest.bundle.identity_sha256.to_owned(),
            required_file_hashes_json: "[]".to_owned(),
            required_files: Vec::new(),
            model_source_json: "{}".to_owned(),
            source: serde_json::json!({}),
        },
        parameters: ExecutionParametersSnapshot::from_settings(parameters_json, settings),
        vad: with_vad.then(|| {
            let vad = crate::asr::manifest::vad_manifest();
            VadExecutionSnapshot {
                model_id: vad.id.to_owned(),
                manifest_version: vad.manifest_version.to_owned(),
                bundle_identity: vad.bundle.identity_sha256.to_owned(),
                required_file_hashes_json: "[]".to_owned(),
                required_files: Vec::new(),
            }
        }),
    }
}

#[test]
fn no_vad_partition_covers_input_with_bounded_provider_windows() {
    let samples = vec![0.1; MAX_PROVIDER_WINDOW as usize * 2 + 17];
    let utterance = partition_without_vad(&samples).unwrap();

    assert_eq!(utterance.evidence.start, 0);
    assert_eq!(utterance.evidence.end, samples.len() as u64);
    assert!(utterance.windows.len() > 1);
    assert!(
        utterance
            .windows
            .iter()
            .all(|window| window.inference.end - window.inference.start <= MAX_PROVIDER_WINDOW)
    );
}

#[test]
fn receipt_does_not_attest_vad_that_was_not_executed() {
    let audio = working_audio(16_000);
    let prepared = PreparedNativeAsr {
        provider: provider(RecordingBackendFactory::default()),
        utterances: vec![partition_without_vad(&audio.samples).unwrap()],
        audio,
        executed_vad: None,
    };

    let receipt = prepared
        .receipt(&snapshot(true), Utc::now(), Utc::now())
        .unwrap();
    let receipt = serde_json::to_value(receipt).unwrap();

    assert_eq!(receipt["vad_model_id"], serde_json::Value::Null);
}

#[test]
fn multiple_provider_windows_produce_one_text_per_utterance() {
    let audio = working_audio(500_000);
    let utterance = partition_without_vad(&audio.samples).unwrap();
    assert!(utterance.windows.len() > 1);
    let calls = Arc::new(Mutex::new(0));
    let mut prepared = PreparedNativeAsr {
        provider: provider(RecordingBackendFactory {
            calls: calls.clone(),
            ..RecordingBackendFactory::default()
        }),
        utterances: vec![utterance],
        audio,
        executed_vad: None,
    };

    let texts = prepared
        .transcribe_all_windows(&CancellationToken::new(), || Ok(()))
        .unwrap();

    assert_eq!(texts, vec!["window 1 window 2"]);
    assert_eq!(*calls.lock().unwrap(), 2);
}

#[test]
fn vad_silence_produces_a_successful_result_without_segments() {
    let audio = working_audio(16_000);
    let snapshot = snapshot(true);
    let mut prepared = PreparedNativeAsr {
        provider: provider(RecordingBackendFactory::default()),
        utterances: Vec::new(),
        audio,
        executed_vad: snapshot.vad.clone(),
    };

    let texts = prepared
        .transcribe_all_windows(&CancellationToken::new(), || Ok(()))
        .unwrap();
    let success = prepared
        .build_success(&snapshot, texts, Utc::now(), Utc::now())
        .unwrap();

    assert!(success.segments.is_empty());
}

#[test]
fn cancelling_a_native_run_stops_before_the_next_provider_window() {
    let audio = working_audio(500_000);
    let utterance = partition_without_vad(&audio.samples).unwrap();
    let barriers = Arc::new((Barrier::new(2), Barrier::new(2)));
    let calls = Arc::new(Mutex::new(0));
    let (dropped_tx, dropped_rx) = mpsc::channel();
    let prepared = PreparedNativeAsr {
        provider: provider(RecordingBackendFactory {
            calls: calls.clone(),
            first_call_barriers: Some(barriers.clone()),
            dropped: Some(dropped_tx),
        }),
        utterances: vec![utterance],
        audio,
        executed_vad: None,
    };
    let temp = tempfile::tempdir().unwrap();
    let runtime = Arc::new(CoreRuntime::initialize(temp.path().join("data")).unwrap());
    let engine = ProductionNativeAsrEngine::new(runtime);
    let mut run = <ProductionNativeAsrEngine as ExecutionEngine<SystemClock>>::start(
        &engine,
        NativePrepared {
            snapshot: snapshot(false),
            prepared: Some(prepared),
        },
        Utc::now(),
    )
    .unwrap();

    barriers.0.wait();
    run.cancel();
    barriers.1.wait();
    dropped_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let _ = run.recv_timeout(Duration::from_millis(1));

    assert_eq!(*calls.lock().unwrap(), 1);
}

#[test]
fn test_provider_uses_sense_voice_backend() {
    let provider = provider(RecordingBackendFactory::default());
    assert_eq!(provider.identity().backend, BackendKind::SenseVoiceSherpa);
}

#[test]
fn production_prepare_defers_native_provider_initialization_to_the_polled_run() {
    let temp = tempfile::tempdir().unwrap();
    let runtime = Arc::new(CoreRuntime::initialize(temp.path().join("data")).unwrap());
    let engine = ProductionNativeAsrEngine::new(runtime);

    let prepared = <ProductionNativeAsrEngine as ExecutionEngine<SystemClock>>::prepare(
        &engine,
        &snapshot(false),
    );

    assert!(prepared.is_ok());
}
