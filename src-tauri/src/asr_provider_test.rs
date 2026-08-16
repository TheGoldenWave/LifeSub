use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::{fs, path::Path};

use crate::asr::provider::{
    AsrProvider, AudioSlice, BackendKind, CancellationToken, DeviceIdentity, NativeBackend,
    NativeBackendFactory, NativeRequest, ProviderError, ProviderFactory, ProviderOptions,
    ProviderRequest, ProviderSelection, QualificationEvidence, RuntimeFamily,
};
use crate::asr::settings::WhisperTask;
use crate::domain::{AsrErrorCode, AsrProviderKind};

#[cfg(all(
    feature = "asr-qwen17-runtime",
    target_os = "macos",
    target_arch = "aarch64"
))]
use crate::asr::provider::{Qwen17RuntimeBoundary, create_qwen17_backend_with_runtime};

const SENSE: &str = "sense-voice-small-int8-2024-07-17";
const TINY: &str = "whisper-tiny";
const BASE: &str = "whisper-base";
const SMALL: &str = "whisper-small";
const QWEN06: &str = "qwen3-asr-0.6b-int8-2026-03-25";
const QWEN17: &str = "qwen3-asr-1.7b";
type RequestMutation = Box<dyn Fn(&mut ProviderRequest)>;

#[derive(Clone, Default)]
struct FakeBackendFactory {
    constructed: Arc<Mutex<Vec<NativeRequest>>>,
    calls: Arc<Mutex<usize>>,
    create_failure: Option<AsrErrorCode>,
    transcribe_failure: Option<AsrErrorCode>,
    cancel_during_call: Option<CancellationToken>,
}

impl NativeBackendFactory for FakeBackendFactory {
    fn create(&self, request: &NativeRequest) -> Result<Box<dyn NativeBackend>, ProviderError> {
        self.constructed.lock().unwrap().push(request.clone());
        if let Some(code) = self.create_failure {
            return Err(ProviderError::new(
                code,
                "fake native initialization failure",
            ));
        }
        Ok(Box::new(FakeBackend {
            calls: self.calls.clone(),
            transcribe_failure: self.transcribe_failure,
            cancel_during_call: self.cancel_during_call.clone(),
        }))
    }
}

struct FakeBackend {
    calls: Arc<Mutex<usize>>,
    transcribe_failure: Option<AsrErrorCode>,
    cancel_during_call: Option<CancellationToken>,
}

impl NativeBackend for FakeBackend {
    fn transcribe(&mut self, _audio: AudioSlice<'_>) -> Result<String, ProviderError> {
        *self.calls.lock().unwrap() += 1;
        if let Some(token) = &self.cancel_during_call {
            token.cancel();
        }
        if let Some(code) = self.transcribe_failure {
            Err(ProviderError::new(code, "fake native inference failure"))
        } else {
            Ok("transcript".to_owned())
        }
    }
}

fn request(model_id: &str) -> ProviderRequest {
    let manifest = crate::asr::manifest::model_registry()
        .model(model_id)
        .unwrap();
    ProviderRequest {
        provider: manifest.provider,
        model_id: model_id.to_owned(),
        manifest_version: manifest.manifest_version.to_owned(),
        bundle_identity: manifest.bundle.identity_sha256.to_owned(),
        install_dir: PathBuf::from("/qualified/model"),
        language: "auto".to_owned(),
        num_threads: 2,
        options: match manifest.provider {
            AsrProviderKind::SenseVoice => ProviderOptions::SenseVoice { use_itn: true },
            AsrProviderKind::Whisper => ProviderOptions::Whisper {
                task: WhisperTask::Transcribe,
            },
            AsrProviderKind::Qwen3Asr => ProviderOptions::Qwen3Asr,
        },
        qualification: QualificationEvidence::matching(manifest, DeviceIdentity::current()),
    }
}

#[test]
fn shipping_models_dispatch_to_exact_backend_once() {
    let cases = [
        (SENSE, BackendKind::SenseVoiceSherpa),
        (TINY, BackendKind::WhisperSherpa),
        (BASE, BackendKind::WhisperSherpa),
        (SMALL, BackendKind::WhisperSherpa),
        (QWEN06, BackendKind::Qwen06Sherpa),
        (QWEN17, BackendKind::Qwen17CandleMetal),
    ];
    for (model_id, expected) in cases {
        let backend = FakeBackendFactory::default();
        let provider = ProviderFactory::new(backend.clone())
            .create(request(model_id))
            .unwrap();
        assert_eq!(provider.identity().backend, expected);
        match expected {
            BackendKind::Qwen17CandleMetal => {
                assert_eq!(provider.identity().execution.backend, "metal");
                assert_eq!(
                    provider.identity().execution.version,
                    "qwen3-asr/0.2.2;candle-core/0.9.2"
                );
                assert_eq!(provider.identity().execution.device_index, Some(0));
            }
            _ => {
                assert_eq!(provider.identity().execution.runtime_name, "sherpa-onnx");
                assert_eq!(provider.identity().execution.version, "1.13.5");
                assert!(
                    provider
                        .identity()
                        .execution
                        .native_archive_sha256
                        .is_some()
                );
            }
        }
        assert_eq!(backend.constructed.lock().unwrap().len(), 1);
    }
}

#[test]
fn invalid_identity_or_qualification_never_constructs_a_backend() {
    let mutations: Vec<RequestMutation> = vec![
        Box::new(|value| value.provider = AsrProviderKind::Whisper),
        Box::new(|value| value.bundle_identity = "wrong-bundle".to_owned()),
        Box::new(|value| value.manifest_version = "wrong-version".to_owned()),
        Box::new(|value| value.qualification.marker_identity = "wrong-marker".to_owned()),
        Box::new(|value| value.qualification.device.arch = "x86_64".to_owned()),
    ];
    for mutate in mutations {
        let backend = FakeBackendFactory::default();
        let mut provider_request = request(QWEN17);
        mutate(&mut provider_request);
        assert!(
            ProviderFactory::new(backend.clone())
                .create(provider_request)
                .is_err()
        );
        assert!(backend.constructed.lock().unwrap().is_empty());
    }
}

#[test]
fn backend_initialization_failure_preserves_code_without_fallback() {
    let backend = FakeBackendFactory {
        create_failure: Some(AsrErrorCode::ProviderInitializationFailed),
        ..Default::default()
    };

    let error = ProviderFactory::new(backend.clone())
        .create(request(QWEN17))
        .unwrap_err();

    assert_eq!(error.code(), AsrErrorCode::ProviderInitializationFailed);
    let constructed = backend.constructed.lock().unwrap();
    assert_eq!(constructed.len(), 1);
    assert_eq!(constructed[0].runtime, RuntimeFamily::QwenCandleMetal);
    assert_eq!(*backend.calls.lock().unwrap(), 0);
}

#[cfg(all(
    feature = "asr-qwen17-runtime",
    target_os = "macos",
    target_arch = "aarch64"
))]
#[test]
fn production_qwen17_metal_construction_failure_never_loads_or_falls_back_to_cpu() {
    #[derive(Clone, Default)]
    struct FailingMetalRuntime {
        device_indices: Arc<Mutex<Vec<usize>>>,
        loads: Arc<Mutex<usize>>,
    }

    impl Qwen17RuntimeBoundary for FailingMetalRuntime {
        type Device = ();

        fn create_metal_device(&self, device_index: usize) -> Result<Self::Device, String> {
            self.device_indices.lock().unwrap().push(device_index);
            Err("simulated Metal construction failure".to_owned())
        }

        fn load(
            &self,
            _request: &NativeRequest,
            _device: Self::Device,
        ) -> Result<Box<dyn NativeBackend>, String> {
            *self.loads.lock().unwrap() += 1;
            unreachable!("model load must not run after Metal construction fails")
        }
    }

    let runtime = FailingMetalRuntime::default();
    let native = crate::asr::qwen3_asr::native_request(
        &request(QWEN17),
        crate::asr::manifest::model_registry()
            .model(QWEN17)
            .unwrap(),
    )
    .unwrap();

    let Err(error) = create_qwen17_backend_with_runtime(&native, &runtime) else {
        panic!("Metal construction failure must fail provider initialization");
    };

    assert_eq!(error.code(), AsrErrorCode::ProviderInitializationFailed);
    assert_eq!(*runtime.device_indices.lock().unwrap(), vec![0]);
    assert_eq!(*runtime.loads.lock().unwrap(), 0);
}

#[cfg(all(
    feature = "asr-qwen17-runtime",
    target_os = "macos",
    target_arch = "aarch64"
))]
#[test]
fn production_qwen17_inference_failure_does_not_reload_or_reconstruct_backend() {
    #[derive(Clone, Default)]
    struct FailingInferenceRuntime {
        device_indices: Arc<Mutex<Vec<usize>>>,
        loads: Arc<Mutex<usize>>,
        calls: Arc<Mutex<usize>>,
    }

    struct FailingInferenceBackend {
        calls: Arc<Mutex<usize>>,
    }

    impl NativeBackend for FailingInferenceBackend {
        fn transcribe(&mut self, _audio: AudioSlice<'_>) -> Result<String, ProviderError> {
            *self.calls.lock().unwrap() += 1;
            Err(ProviderError::new(
                AsrErrorCode::TranscriptionFailed,
                "simulated inference failure",
            ))
        }
    }

    impl Qwen17RuntimeBoundary for FailingInferenceRuntime {
        type Device = ();

        fn create_metal_device(&self, device_index: usize) -> Result<Self::Device, String> {
            self.device_indices.lock().unwrap().push(device_index);
            Ok(())
        }

        fn load(
            &self,
            _request: &NativeRequest,
            _device: Self::Device,
        ) -> Result<Box<dyn NativeBackend>, String> {
            *self.loads.lock().unwrap() += 1;
            Ok(Box::new(FailingInferenceBackend {
                calls: self.calls.clone(),
            }))
        }
    }

    let runtime = FailingInferenceRuntime::default();
    let native = crate::asr::qwen3_asr::native_request(
        &request(QWEN17),
        crate::asr::manifest::model_registry()
            .model(QWEN17)
            .unwrap(),
    )
    .unwrap();
    let mut backend = create_qwen17_backend_with_runtime(&native, &runtime).unwrap();

    let error = backend
        .transcribe(AudioSlice::new(&[0.1; 16_000], 16_000).unwrap())
        .unwrap_err();

    assert_eq!(error.code(), AsrErrorCode::TranscriptionFailed);
    assert_eq!(*runtime.device_indices.lock().unwrap(), vec![0]);
    assert_eq!(*runtime.loads.lock().unwrap(), 1);
    assert_eq!(*runtime.calls.lock().unwrap(), 1);
}

#[test]
fn backend_inference_failure_preserves_code_without_fallback() {
    let backend = FakeBackendFactory {
        transcribe_failure: Some(AsrErrorCode::TranscriptionFailed),
        ..Default::default()
    };
    let mut provider = ProviderFactory::new(backend.clone())
        .create(request(QWEN06))
        .unwrap();

    let error = provider
        .transcribe(
            AudioSlice::new(&[0.1; 16_000], 16_000).unwrap(),
            &CancellationToken::new(),
        )
        .unwrap_err();

    assert_eq!(error.code(), AsrErrorCode::TranscriptionFailed);
    let constructed = backend.constructed.lock().unwrap();
    assert_eq!(constructed.len(), 1);
    assert_eq!(constructed[0].runtime, RuntimeFamily::SherpaOnnx);
    assert_eq!(*backend.calls.lock().unwrap(), 1);
}

#[test]
fn qwen_models_never_fallback_across_runtime_families() {
    let qwen06 = FakeBackendFactory {
        transcribe_failure: Some(AsrErrorCode::TranscriptionFailed),
        ..Default::default()
    };
    let mut provider = ProviderFactory::new(qwen06.clone())
        .create(request(QWEN06))
        .unwrap();
    assert!(
        provider
            .transcribe(
                AudioSlice::new(&[0.1; 16_000], 16_000).unwrap(),
                &CancellationToken::new()
            )
            .is_err()
    );
    assert_eq!(
        qwen06.constructed.lock().unwrap()[0].runtime,
        RuntimeFamily::SherpaOnnx
    );
    assert_eq!(qwen06.constructed.lock().unwrap().len(), 1);

    let qwen17 = FakeBackendFactory {
        transcribe_failure: Some(AsrErrorCode::TranscriptionFailed),
        ..Default::default()
    };
    let mut provider = ProviderFactory::new(qwen17.clone())
        .create(request(QWEN17))
        .unwrap();
    assert!(
        provider
            .transcribe(
                AudioSlice::new(&[0.1; 16_000], 16_000).unwrap(),
                &CancellationToken::new()
            )
            .is_err()
    );
    assert_eq!(
        qwen17.constructed.lock().unwrap()[0].runtime,
        RuntimeFamily::QwenCandleMetal
    );
    assert_eq!(qwen17.constructed.lock().unwrap().len(), 1);
}

#[test]
fn provider_options_map_without_pseudo_languages() {
    let sense_backend = FakeBackendFactory::default();
    let mut sense = request(SENSE);
    sense.language = "yue".to_owned();
    sense.options = ProviderOptions::SenseVoice { use_itn: false };
    ProviderFactory::new(sense_backend.clone())
        .create(sense)
        .unwrap();
    let sense_spec = &sense_backend.constructed.lock().unwrap()[0];
    assert_eq!(sense_spec.language.as_deref(), Some("yue"));
    assert_eq!(sense_spec.use_itn, Some(false));

    let whisper_backend = FakeBackendFactory::default();
    let mut whisper = request(TINY);
    whisper.language = "en".to_owned();
    whisper.options = ProviderOptions::Whisper {
        task: WhisperTask::Translate,
    };
    ProviderFactory::new(whisper_backend.clone())
        .create(whisper)
        .unwrap();
    let whisper_spec = &whisper_backend.constructed.lock().unwrap()[0];
    assert_eq!(whisper_spec.language.as_deref(), Some("en"));
    assert_eq!(whisper_spec.whisper_task.as_deref(), Some("translate"));

    let backend = FakeBackendFactory::default();
    let mut invalid = request(TINY);
    invalid.language = "multilingual".to_owned();
    let error = ProviderFactory::new(backend.clone())
        .create(invalid)
        .unwrap_err();
    assert_eq!(error.code(), AsrErrorCode::InvalidProviderParameter);
    assert!(backend.constructed.lock().unwrap().is_empty());
}

#[test]
fn qwen_language_contracts_are_runtime_specific() {
    let backend = FakeBackendFactory::default();
    let mut qwen06 = request(QWEN06);
    qwen06.language = "en".to_owned();
    assert_eq!(
        ProviderFactory::new(backend.clone())
            .create(qwen06)
            .unwrap_err()
            .code(),
        AsrErrorCode::InvalidProviderParameter
    );
    assert!(backend.constructed.lock().unwrap().is_empty());

    let backend = FakeBackendFactory::default();
    let mut qwen17 = request(QWEN17);
    qwen17.language = "zh".to_owned();
    ProviderFactory::new(backend.clone())
        .create(qwen17)
        .unwrap();
    assert_eq!(
        backend.constructed.lock().unwrap()[0].language.as_deref(),
        Some("chinese")
    );

    let backend = FakeBackendFactory::default();
    let mut invalid = request(QWEN17);
    invalid.language = "xx".to_owned();
    assert!(
        ProviderFactory::new(backend.clone())
            .create(invalid)
            .is_err()
    );
    assert!(backend.constructed.lock().unwrap().is_empty());
}

#[test]
fn audio_slice_and_cancellation_bound_the_native_call() {
    assert!(AudioSlice::new(&[], 16_000).is_err());
    assert!(AudioSlice::new(&[0.0; 16_000], 8_000).is_err());
    assert!(AudioSlice::new(&[0.0; 400_001], 16_000).is_err());

    let before = FakeBackendFactory::default();
    let mut provider = ProviderFactory::new(before.clone())
        .create(request(SENSE))
        .unwrap();
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert_eq!(
        provider
            .transcribe(AudioSlice::new(&[0.0; 16_000], 16_000).unwrap(), &cancelled)
            .unwrap_err()
            .code(),
        AsrErrorCode::Cancelled
    );
    assert_eq!(*before.calls.lock().unwrap(), 0);

    let during = CancellationToken::new();
    let backend = FakeBackendFactory {
        cancel_during_call: Some(during.clone()),
        ..Default::default()
    };
    let mut provider = ProviderFactory::new(backend.clone())
        .create(request(SENSE))
        .unwrap();
    assert_eq!(
        provider
            .transcribe(AudioSlice::new(&[0.0; 16_000], 16_000).unwrap(), &during)
            .unwrap_err()
            .code(),
        AsrErrorCode::Cancelled
    );
    assert_eq!(*backend.calls.lock().unwrap(), 1);
}

#[test]
fn empty_native_output_is_a_stable_transcription_failure() {
    #[derive(Clone)]
    struct EmptyFactory;
    struct EmptyBackend;
    impl NativeBackendFactory for EmptyFactory {
        fn create(
            &self,
            _request: &NativeRequest,
        ) -> Result<Box<dyn NativeBackend>, ProviderError> {
            Ok(Box::new(EmptyBackend))
        }
    }
    impl NativeBackend for EmptyBackend {
        fn transcribe(&mut self, _audio: AudioSlice<'_>) -> Result<String, ProviderError> {
            Ok(" \n\t".to_owned())
        }
    }
    let mut provider = ProviderFactory::new(EmptyFactory)
        .create(request(SENSE))
        .unwrap();
    assert_eq!(
        provider
            .transcribe(
                AudioSlice::new(&[0.0; 16_000], 16_000).unwrap(),
                &CancellationToken::new()
            )
            .unwrap_err()
            .code(),
        AsrErrorCode::TranscriptionFailed
    );
}

#[test]
fn executable_lease_revalidates_every_shipping_bundle_before_backend_construction() {
    use crate::asr::model_manager::ExecutableInstallationLease;
    for model_id in [SENSE, TINY, BASE, SMALL, QWEN06, QWEN17] {
        let directory = tempfile::tempdir().unwrap();
        let backend = FakeBackendFactory::default();
        let lease = ExecutableInstallationLease::for_test(
            model_id,
            directory.path(),
            DeviceIdentity::current(),
        )
        .unwrap();
        assert!(lease.revalidate().is_err());
        assert!(backend.constructed.lock().unwrap().is_empty());
    }
}

#[test]
fn provider_factory_revalidates_held_inventory_before_and_after_native_construction() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("data");
    fs::create_dir_all(&root).unwrap();
    let lease = anchored_test_lease(SENSE, &root);
    assert_eq!(lease.validation_count(), 0);
    let backend = FakeBackendFactory::default();
    let provider = ProviderFactory::new(backend.clone())
        .create_verified(lease, ProviderSelection::for_test(SENSE))
        .unwrap();
    assert_eq!(backend.constructed.lock().unwrap().len(), 1);
    assert_eq!(provider.identity().model_id, SENSE);
    assert_eq!(provider.inventory_validation_count_for_test(), 2);
}

fn anchored_test_lease(
    model_id: &str,
    root: &Path,
) -> crate::asr::model_manager::ExecutableInstallationLease {
    use crate::asr::model_manager::ExecutableInstallationLease;

    let relative = Path::new("models/asr/test/model/bundle");
    let manifest = crate::asr::manifest::model_registry()
        .model(model_id)
        .unwrap();
    let required_files = match manifest.bundle.install_constraints {
        crate::asr::manifest::InstallConstraints::Archive(constraints) => {
            constraints.required_files
        }
        crate::asr::manifest::InstallConstraints::Direct(constraints) => constraints.required_files,
    };
    let files = required_files
        .iter()
        .map(|required| {
            let bytes = format!("held:{}:{}", model_id, required.path).into_bytes();
            let path = root.join(relative).join(required.path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, &bytes).unwrap();
            (PathBuf::from(required.path), bytes)
        })
        .collect::<Vec<_>>();
    ExecutableInstallationLease::for_anchored_test(
        model_id,
        root,
        relative,
        files,
        DeviceIdentity::current(),
    )
    .unwrap()
}

#[test]
fn verified_sherpa_provider_consumes_held_fds_and_releases_them_on_drop() {
    for model_id in [SENSE, TINY, BASE, SMALL, QWEN06] {
        let parent = tempfile::tempdir().unwrap();
        let nominal_root = parent.path().join("data");
        fs::create_dir_all(&nominal_root).unwrap();
        let lease = anchored_test_lease(model_id, &nominal_root);
        let backend = FakeBackendFactory::default();

        let provider = ProviderFactory::new(backend.clone())
            .create_verified(lease, ProviderSelection::for_test(model_id))
            .unwrap();
        let native = backend.constructed.lock().unwrap()[0].clone();
        let direct_fd_count = if model_id == QWEN06 {
            3
        } else {
            native.required_files.len()
        };
        assert!(
            native.required_files[..direct_fd_count]
                .iter()
                .all(|path| path.starts_with("/dev/fd"))
        );
        assert!(native.required_files.iter().all(|path| path.exists()));

        drop(provider);
        assert!(native.required_files.iter().all(|path| !path.exists()));
    }
}

#[test]
fn qwen06_tokenizer_uses_a_private_alias_directory_backed_by_held_file_fds() {
    let parent = tempfile::tempdir().unwrap();
    let nominal_root = parent.path().join("data");
    fs::create_dir_all(&nominal_root).unwrap();
    let lease = anchored_test_lease(QWEN06, &nominal_root);
    let backend = FakeBackendFactory::default();

    let provider = ProviderFactory::new(backend.clone())
        .create_verified(lease, ProviderSelection::for_test(QWEN06))
        .unwrap();
    let tokenizer = backend.constructed.lock().unwrap()[0].required_files[3].clone();
    assert!(!tokenizer.starts_with("/dev/fd"));
    assert_eq!(
        fs::metadata(&tokenizer).unwrap().permissions().mode() & 0o777,
        0o700
    );
    for name in ["merges.txt", "tokenizer_config.json", "vocab.json"] {
        let bytes = fs::read(tokenizer.join(name)).unwrap();
        assert_eq!(bytes, format!("held:{QWEN06}:tokenizer/{name}").as_bytes());
    }

    drop(provider);
    assert!(!tokenizer.exists());
}

#[test]
fn qwen17_verified_factory_keeps_nominal_install_dir_until_candle_fd_support_exists() {
    let directory = tempfile::tempdir().unwrap();
    let mut provider_request = request(QWEN17);
    provider_request.install_dir = directory.path().to_path_buf();
    let native = crate::asr::qwen3_asr::native_request(
        &provider_request,
        crate::asr::manifest::model_registry()
            .model(QWEN17)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(native.install_dir, directory.path());
    assert!(
        native
            .required_files
            .iter()
            .all(|path| path.starts_with(directory.path()))
    );
}

#[test]
fn root_entry_swap_never_redirects_verified_provider_to_replacement_files() {
    let parent = tempfile::tempdir().unwrap();
    let nominal_root = parent.path().join("data");
    let held_root = parent.path().join("held-data");
    fs::create_dir_all(&nominal_root).unwrap();
    let lease = anchored_test_lease(SENSE, &nominal_root);
    fs::rename(&nominal_root, &held_root).unwrap();
    fs::create_dir_all(nominal_root.join("models/asr/test/model/bundle")).unwrap();
    fs::write(
        nominal_root.join("models/asr/test/model/bundle/model.int8.onnx"),
        b"replacement",
    )
    .unwrap();
    fs::write(
        nominal_root.join("models/asr/test/model/bundle/tokens.txt"),
        b"replacement",
    )
    .unwrap();
    let backend = FakeBackendFactory::default();

    let _provider = ProviderFactory::new(backend.clone())
        .create_verified(lease, ProviderSelection::for_test(SENSE))
        .unwrap();
    let native = backend.constructed.lock().unwrap()[0].clone();
    assert!(native.required_files.iter().all(|path| {
        let bytes = fs::read(path).unwrap();
        bytes.starts_with(b"held:")
    }));
}

#[test]
fn required_file_swap_during_backend_construction_fails_post_revalidation() {
    #[derive(Clone)]
    struct SwappingFactory {
        target: PathBuf,
        constructed: Arc<Mutex<usize>>,
    }

    impl NativeBackendFactory for SwappingFactory {
        fn create(
            &self,
            _request: &NativeRequest,
        ) -> Result<Box<dyn NativeBackend>, ProviderError> {
            *self.constructed.lock().unwrap() += 1;
            fs::rename(&self.target, self.target.with_extension("held")).unwrap();
            fs::write(&self.target, b"replacement").unwrap();
            Ok(Box::new(FakeBackend {
                calls: Arc::new(Mutex::new(0)),
                transcribe_failure: None,
                cancel_during_call: None,
            }))
        }
    }

    let parent = tempfile::tempdir().unwrap();
    let nominal_root = parent.path().join("data");
    fs::create_dir_all(&nominal_root).unwrap();
    let lease = anchored_test_lease(SENSE, &nominal_root);
    let constructed = Arc::new(Mutex::new(0));
    let factory = SwappingFactory {
        target: nominal_root.join("models/asr/test/model/bundle/model.int8.onnx"),
        constructed: constructed.clone(),
    };

    let error = ProviderFactory::new(factory)
        .create_verified(lease, ProviderSelection::for_test(SENSE))
        .unwrap_err();
    assert_eq!(error.code(), AsrErrorCode::ModelIntegrityFailed);
    assert_eq!(*constructed.lock().unwrap(), 1);
}

#[test]
fn model_manager_refuses_to_issue_a_lease_for_unqualified_database_state() {
    use crate::asr::model_manager::{
        ModelCatalog, ModelManager, ReqwestTransport, StoredInstallation,
    };
    use crate::catalog::Catalog;

    let directory = tempfile::tempdir().unwrap();
    let catalog = Catalog::in_memory().unwrap();
    let manifest = crate::asr::manifest::model_registry()
        .model(QWEN17)
        .unwrap();
    catalog
        .publish_installation(&StoredInstallation {
            model_id: QWEN17.to_owned(),
            provider: "qwen3_asr".to_owned(),
            manifest_version: manifest.manifest_version.to_owned(),
            bundle_identity: manifest.bundle.identity_sha256.to_owned(),
            install_dir: directory.path().to_path_buf(),
            state: "installed_unqualified".to_owned(),
            runtime_identity_json: None,
        })
        .unwrap();
    let manager = ModelManager::new(directory.path(), ReqwestTransport::new().unwrap(), catalog);
    let error = manager.executable_installation(QWEN17).unwrap_err();
    assert_eq!(error.code(), "model_runtime_unqualified");
}

#[cfg(feature = "asr-runtime")]
#[test]
fn sherpa_configs_use_exact_required_files_and_supported_runtime_fields() {
    let sense = crate::asr::sense_voice::sherpa_config(
        &crate::asr::sense_voice::native_request(
            &request(SENSE),
            crate::asr::manifest::model_registry().model(SENSE).unwrap(),
        )
        .unwrap(),
    );
    assert_eq!(
        sense.model_config.sense_voice.model.as_deref(),
        Some("/qualified/model/model.int8.onnx")
    );
    assert_eq!(
        sense.model_config.tokens.as_deref(),
        Some("/qualified/model/tokens.txt")
    );
    assert!(sense.model_config.sense_voice.use_itn);

    let whisper_request = crate::asr::whisper::native_request(
        &request(BASE),
        crate::asr::manifest::model_registry().model(BASE).unwrap(),
    )
    .unwrap();
    let whisper = crate::asr::whisper::sherpa_config(&whisper_request);
    assert_eq!(
        whisper.model_config.whisper.encoder.as_deref(),
        Some("/qualified/model/base-encoder.onnx")
    );
    assert_eq!(
        whisper.model_config.whisper.decoder.as_deref(),
        Some("/qualified/model/base-decoder.onnx")
    );
    assert_eq!(
        whisper.model_config.whisper.language.as_deref(),
        Some("auto")
    );
    assert_eq!(
        whisper.model_config.whisper.task.as_deref(),
        Some("transcribe")
    );

    let qwen_request = crate::asr::qwen3_asr::native_request(
        &request(QWEN06),
        crate::asr::manifest::model_registry()
            .model(QWEN06)
            .unwrap(),
    )
    .unwrap();
    let qwen = crate::asr::qwen3_asr::sherpa_config(&qwen_request);
    assert_eq!(
        qwen.model_config.qwen3_asr.conv_frontend.as_deref(),
        Some("/qualified/model/conv_frontend.onnx")
    );
    assert_eq!(
        qwen.model_config.qwen3_asr.tokenizer.as_deref(),
        Some("/qualified/model/tokenizer")
    );
    assert_eq!(qwen.model_config.qwen3_asr.hotwords, None);
}
