use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::asr::manifest::model_registry;
use crate::asr::model_manager::{ModelCatalog, ModelManager, ReqwestTransport, StoredInstallation};
use crate::asr::provider::DeviceIdentity;
use crate::asr::runtime_qualifier::{
    MarkerFault, QualificationCatalog, QualificationHandle, QualificationRecord,
    QualifiedRuntimeIdentity, QualifierError, RUNTIME_QUALIFICATION_MARKER, RuntimeQualifier,
    RuntimeSmoke, load_qualification_speech_fixture,
};
use crate::catalog::Catalog;

const MODEL_ID: &str = "qwen3-asr-1.7b";

struct FakeSmoke {
    calls: AtomicUsize,
    result: Result<QualifiedRuntimeIdentity, &'static str>,
}

impl RuntimeSmoke for FakeSmoke {
    fn smoke(&self, _handle: &QualificationHandle) -> Result<QualifiedRuntimeIdentity, String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.result.clone().map_err(str::to_owned)
    }
}

fn identity() -> QualifiedRuntimeIdentity {
    QualifiedRuntimeIdentity {
        crate_name: "qwen3-asr".to_owned(),
        crate_version: "0.2.2".to_owned(),
        git_commit: "c5ef09646af6278d2ba8b8ceaf543ffb32d1a5dc".to_owned(),
        candle_version: "0.9.2".to_owned(),
        backend: "metal".to_owned(),
        target_os: "macos".to_owned(),
        target_arch: "aarch64".to_owned(),
        device_index: 0,
        device_name: "Apple M4".to_owned(),
        smoke_fixture_sha256: crate::asr::runtime_qualifier::QUALIFICATION_SMOKE_FIXTURE_SHA256
            .to_owned(),
        qualification_contract_sha256: crate::asr::runtime_qualifier::QUALIFICATION_CONTRACT_SHA256
            .to_owned(),
    }
}

fn setup() -> (tempfile::TempDir, Arc<Catalog>, QualificationHandle) {
    let directory = tempfile::tempdir().unwrap();
    let catalog = Arc::new(Catalog::in_memory().unwrap());
    let manifest = model_registry().model(MODEL_ID).unwrap();
    let handle = QualificationHandle::from_manifest(
        manifest,
        directory.path(),
        DeviceIdentity {
            os: "macos".to_owned(),
            arch: "aarch64".to_owned(),
            backend: "metal".to_owned(),
            device_index: 0,
            macos_major: 14,
            memory_gib: 24,
            chip: "M4".to_owned(),
        },
    );
    catalog
        .publish_installation(&StoredInstallation {
            model_id: MODEL_ID.to_owned(),
            provider: "qwen3_asr".to_owned(),
            manifest_version: manifest.manifest_version.to_owned(),
            bundle_identity: manifest.bundle.identity_sha256.to_owned(),
            install_dir: directory.path().to_path_buf(),
            state: "installed_unqualified".to_owned(),
            runtime_identity_json: None,
        })
        .unwrap();
    (directory, catalog, handle)
}

#[test]
fn qualifier_publishes_marker_before_catalog_cas() {
    let (directory, catalog, handle) = setup();
    let smoke = Arc::new(FakeSmoke {
        calls: AtomicUsize::new(0),
        result: Ok(identity()),
    });
    RuntimeQualifier::new(catalog.clone(), smoke.clone())
        .qualify(&handle)
        .unwrap();
    let record = catalog.model_installation_records().unwrap().pop().unwrap();
    assert_eq!(record.state, "runtime_qualified");
    assert_eq!(record.last_error_code, None);
    assert!(
        directory
            .path()
            .join(RUNTIME_QUALIFICATION_MARKER)
            .is_file()
    );
    assert_eq!(smoke.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn smoke_failure_remains_unqualified_without_corrupting_files() {
    let (directory, catalog, handle) = setup();
    let smoke = Arc::new(FakeSmoke {
        calls: AtomicUsize::new(0),
        result: Err("smoke failed"),
    });
    assert!(
        RuntimeQualifier::new(catalog.clone(), smoke)
            .qualify(&handle)
            .is_err()
    );
    let record = catalog.model_installation_records().unwrap().pop().unwrap();
    assert_eq!(record.state, "installed_unqualified");
    assert_eq!(
        record.last_error_code.as_deref(),
        Some("model_runtime_qualification_failed")
    );
    assert!(!directory.path().join(RUNTIME_QUALIFICATION_MARKER).exists());
}

#[test]
fn marker_write_sync_and_rename_failures_remain_unqualified() {
    for fault in [MarkerFault::Write, MarkerFault::Sync, MarkerFault::Rename] {
        let (directory, catalog, handle) = setup();
        let smoke = Arc::new(FakeSmoke {
            calls: AtomicUsize::new(0),
            result: Ok(identity()),
        });
        assert!(
            RuntimeQualifier::new(catalog.clone(), smoke)
                .with_marker_fault(fault)
                .qualify(&handle)
                .is_err()
        );
        let record = catalog.model_installation_records().unwrap().pop().unwrap();
        assert_eq!(record.state, "installed_unqualified");
        assert_eq!(
            record.last_error_code.as_deref(),
            Some("model_runtime_qualification_recovery_required")
        );
        assert!(!directory.path().join(RUNTIME_QUALIFICATION_MARKER).exists());
    }
}

#[test]
fn durable_marker_after_crash_is_reconciled_without_repeating_smoke() {
    let (_directory, catalog, handle) = setup();
    let first = Arc::new(FakeSmoke {
        calls: AtomicUsize::new(0),
        result: Ok(identity()),
    });
    let error = RuntimeQualifier::new(catalog.clone(), first)
        .with_marker_fault(MarkerFault::AfterDurableRename)
        .qualify(&handle)
        .unwrap_err();
    assert_eq!(
        error.code(),
        "model_runtime_qualification_recovery_required"
    );
    let recovery = Arc::new(FakeSmoke {
        calls: AtomicUsize::new(0),
        result: Err("must not run"),
    });
    RuntimeQualifier::new(catalog.clone(), recovery.clone())
        .qualify(&handle)
        .unwrap();
    assert_eq!(recovery.calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        catalog.model_installation_records().unwrap()[0].state,
        "runtime_qualified"
    );
}

#[test]
fn missing_marker_demotes_qualified_database_state() {
    let (directory, catalog, handle) = setup();
    let smoke = Arc::new(FakeSmoke {
        calls: AtomicUsize::new(0),
        result: Ok(identity()),
    });
    let qualifier = RuntimeQualifier::new(catalog.clone(), smoke);
    qualifier.qualify(&handle).unwrap();
    std::fs::remove_file(directory.path().join(RUNTIME_QUALIFICATION_MARKER)).unwrap();
    qualifier.reconcile(&handle).unwrap();
    let record = catalog.model_installation_records().unwrap().pop().unwrap();
    assert_eq!(record.state, "installed_unqualified");
    assert_eq!(
        record.last_error_code.as_deref(),
        Some("model_runtime_qualification_recovery_required")
    );
}

#[test]
fn mismatched_marker_demotes_qualified_database_state() {
    let (directory, catalog, handle) = setup();
    let smoke = Arc::new(FakeSmoke {
        calls: AtomicUsize::new(0),
        result: Ok(identity()),
    });
    let qualifier = RuntimeQualifier::new(catalog.clone(), smoke);
    qualifier.qualify(&handle).unwrap();
    let marker_path = directory.path().join(RUNTIME_QUALIFICATION_MARKER);
    let bytes = std::fs::read(&marker_path).unwrap();
    let mut marker: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    marker["bundle_identity"] = serde_json::Value::String("conflicting-bundle".to_owned());
    std::fs::write(&marker_path, serde_json::to_vec(&marker).unwrap()).unwrap();
    qualifier.reconcile(&handle).unwrap();
    let record = catalog.model_installation_records().unwrap().pop().unwrap();
    assert_eq!(record.state, "installed_unqualified");
    assert_eq!(
        record.last_error_code.as_deref(),
        Some("model_runtime_qualification_recovery_required")
    );
    assert!(!marker_path.exists());
}

#[test]
fn concurrent_and_repeated_qualification_is_idempotent_for_same_identity() {
    let (_directory, catalog, handle) = setup();
    let smoke = Arc::new(FakeSmoke {
        calls: AtomicUsize::new(0),
        result: Ok(identity()),
    });
    let qualifier = Arc::new(RuntimeQualifier::new(catalog.clone(), smoke));
    let first = {
        let qualifier = qualifier.clone();
        let handle = handle.clone();
        std::thread::spawn(move || qualifier.qualify(&handle))
    };
    let second = {
        let qualifier = qualifier.clone();
        let handle = handle.clone();
        std::thread::spawn(move || qualifier.qualify(&handle))
    };
    first.join().unwrap().unwrap();
    second.join().unwrap().unwrap();
    qualifier.qualify(&handle).unwrap();
    assert_eq!(
        catalog.model_installation_records().unwrap()[0].state,
        "runtime_qualified"
    );
}

#[test]
fn model_manager_invokes_qualifier_through_its_existing_catalog_gateway() {
    let directory = tempfile::tempdir().unwrap();
    let catalog = Catalog::in_memory().unwrap();
    let manifest = model_registry().model(MODEL_ID).unwrap();
    let handle = QualificationHandle::from_manifest(
        manifest,
        directory.path(),
        DeviceIdentity {
            os: "macos".to_owned(),
            arch: "aarch64".to_owned(),
            backend: "metal".to_owned(),
            device_index: 0,
            macos_major: 14,
            memory_gib: 24,
            chip: "M4".to_owned(),
        },
    );
    catalog
        .publish_installation(&StoredInstallation {
            model_id: MODEL_ID.to_owned(),
            provider: "qwen3_asr".to_owned(),
            manifest_version: manifest.manifest_version.to_owned(),
            bundle_identity: manifest.bundle.identity_sha256.to_owned(),
            install_dir: directory.path().to_path_buf(),
            state: "installed_unqualified".to_owned(),
            runtime_identity_json: None,
        })
        .unwrap();
    let manager = ModelManager::new(
        directory.path().join("downloads"),
        ReqwestTransport::new().unwrap(),
        catalog,
    );
    manager
        .runtime_qualifier_for_test(FakeSmoke {
            calls: AtomicUsize::new(0),
            result: Ok(identity()),
        })
        .qualify(&handle)
        .unwrap();
    assert_eq!(
        manager.catalog().model_installation_records().unwrap()[0].state,
        "runtime_qualified"
    );
}

struct CasLoserCatalog {
    after_cas: QualificationRecord,
    reads: AtomicUsize,
    missing_after_cas: bool,
}

impl QualificationCatalog for CasLoserCatalog {
    fn qualification_record(
        &self,
        _model_id: &str,
    ) -> Result<Option<QualificationRecord>, QualifierError> {
        let read = self.reads.fetch_add(1, Ordering::SeqCst);
        if read == 0 {
            let mut initial = self.after_cas.clone();
            initial.state = "installed_unqualified".to_owned();
            initial.runtime_identity_json = None;
            Ok(Some(initial))
        } else {
            Ok((!self.missing_after_cas).then(|| self.after_cas.clone()))
        }
    }

    fn cas_runtime_qualified(
        &self,
        _handle: &QualificationHandle,
        _runtime_identity_json: &str,
    ) -> Result<bool, QualifierError> {
        Ok(false)
    }

    fn demote_runtime_qualification(
        &self,
        _handle: &QualificationHandle,
        _error_code: &str,
    ) -> Result<bool, QualifierError> {
        Ok(false)
    }

    fn record_qualification_error(
        &self,
        _handle: &QualificationHandle,
        _error_code: &str,
    ) -> Result<(), QualifierError> {
        Ok(())
    }
}

fn durable_marker_handle() -> (tempfile::TempDir, QualificationHandle) {
    let (directory, catalog, handle) = setup();
    let smoke = Arc::new(FakeSmoke {
        calls: AtomicUsize::new(0),
        result: Ok(identity()),
    });
    RuntimeQualifier::new(catalog, smoke)
        .with_marker_fault(MarkerFault::AfterDurableRename)
        .qualify(&handle)
        .unwrap_err();
    (directory, handle)
}

#[test]
fn reconcile_cas_loser_accepts_only_the_exact_same_qualified_winner() {
    let (_directory, handle) = durable_marker_handle();
    let catalog = CasLoserCatalog {
        after_cas: QualificationRecord {
            state: "runtime_qualified".to_owned(),
            manifest_version: handle.manifest_version.clone(),
            bundle_identity: handle.bundle_identity.clone(),
            install_dir: handle.install_dir.clone(),
            runtime_identity_json: Some(serde_json::to_string(&identity()).unwrap()),
        },
        reads: AtomicUsize::new(0),
        missing_after_cas: false,
    };
    let smoke = FakeSmoke {
        calls: AtomicUsize::new(0),
        result: Err("must not run"),
    };
    RuntimeQualifier::new(catalog, smoke)
        .reconcile(&handle)
        .unwrap();
}

#[test]
fn reconcile_cas_loser_rejects_conflicting_or_nonqualified_winner() {
    for (state, runtime_identity_json) in [
        ("runtime_qualified", Some("{}".to_owned())),
        ("deleting", None),
    ] {
        let (_directory, handle) = durable_marker_handle();
        let catalog = CasLoserCatalog {
            after_cas: QualificationRecord {
                state: state.to_owned(),
                manifest_version: handle.manifest_version.clone(),
                bundle_identity: handle.bundle_identity.clone(),
                install_dir: handle.install_dir.clone(),
                runtime_identity_json,
            },
            reads: AtomicUsize::new(0),
            missing_after_cas: false,
        };
        let smoke = FakeSmoke {
            calls: AtomicUsize::new(0),
            result: Err("must not run"),
        };
        assert_eq!(
            RuntimeQualifier::new(catalog, smoke)
                .reconcile(&handle)
                .unwrap_err()
                .code(),
            "model_runtime_qualification_recovery_required"
        );
    }

    let (_directory, handle) = durable_marker_handle();
    let catalog = CasLoserCatalog {
        after_cas: QualificationRecord {
            state: "runtime_qualified".to_owned(),
            manifest_version: handle.manifest_version.clone(),
            bundle_identity: handle.bundle_identity.clone(),
            install_dir: handle.install_dir.clone(),
            runtime_identity_json: Some(serde_json::to_string(&identity()).unwrap()),
        },
        reads: AtomicUsize::new(0),
        missing_after_cas: true,
    };
    let smoke = FakeSmoke {
        calls: AtomicUsize::new(0),
        result: Err("must not run"),
    };
    assert_eq!(
        RuntimeQualifier::new(catalog, smoke)
            .reconcile(&handle)
            .unwrap_err()
            .code(),
        "model_runtime_qualification_recovery_required"
    );
}

#[test]
fn qualification_speech_fixture_is_verified_decoded_and_frozen() {
    let fixture = load_qualification_speech_fixture().unwrap();
    assert_eq!(fixture.sample_rate_hz, 16_000);
    assert!(!fixture.samples.is_empty());
    assert!(fixture.samples.len() <= 25 * 16_000);
    assert_eq!(
        fixture.expected_text,
        "I'm alone, all by myself. Je suis tout seul. Sono tutto. Estoy solo."
    );
    assert_eq!(
        fixture.expected_phrases,
        ["I'm alone", "Je suis tout seul", "Sono tutto", "Estoy solo"]
    );
    assert_eq!(fixture.minimum_phrase_matches, 2);
    assert_eq!(
        fixture.normalization,
        "unicode-nfkc-alphanumeric-lowercase-v1"
    );
    assert_eq!(
        fixture.source_sha256,
        "2def7fa41004d0a7d148d4afbf4c467c9d112d8b373996123e9a4c43d94957c7"
    );
    assert_eq!(
        fixture.canonical_sample_sha256,
        "9ea167a7ec7959cfad379f8edd6732098177d57d29b50737aa1f3f0ec84e819a"
    );
    assert_eq!(
        fixture.contract_sha256,
        crate::asr::runtime_qualifier::QUALIFICATION_CONTRACT_SHA256
    );
}

#[test]
fn qualification_text_normalization_is_nfkc_alphanumeric_and_lowercase() {
    assert_eq!(
        crate::asr::runtime_qualifier::normalize_qualification_text("Ｉ’M, Alone！"),
        "imalone"
    );
}

#[test]
fn mutated_qualification_speech_audio_fails_before_smoke_or_catalog_cas() {
    let source_root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/asr");
    let directory = tempfile::tempdir().unwrap();
    std::fs::copy(
        source_root.join("fixtures.json"),
        directory.path().join("fixtures.json"),
    )
    .unwrap();
    let mut bytes = std::fs::read(source_root.join("qwen06-codeswitch.wav")).unwrap();
    let index = bytes.len() - 1;
    bytes[index] ^= 0x01;
    std::fs::write(directory.path().join("qwen06-codeswitch.wav"), bytes).unwrap();
    assert!(
        crate::asr::runtime_qualifier::load_qualification_speech_fixture_from(directory.path())
            .is_err()
    );

    struct MutationSmoke(std::path::PathBuf);
    impl RuntimeSmoke for MutationSmoke {
        fn smoke(&self, _handle: &QualificationHandle) -> Result<QualifiedRuntimeIdentity, String> {
            crate::asr::runtime_qualifier::load_qualification_speech_fixture_from(&self.0)
                .map(|_| identity())
                .map_err(|error| error.to_string())
        }
    }
    struct MutationCatalog {
        record: QualificationRecord,
        cas_calls: AtomicUsize,
    }
    impl QualificationCatalog for MutationCatalog {
        fn qualification_record(
            &self,
            _model_id: &str,
        ) -> Result<Option<QualificationRecord>, QualifierError> {
            Ok(Some(self.record.clone()))
        }
        fn cas_runtime_qualified(
            &self,
            _handle: &QualificationHandle,
            _runtime_identity_json: &str,
        ) -> Result<bool, QualifierError> {
            self.cas_calls.fetch_add(1, Ordering::SeqCst);
            Ok(true)
        }
        fn demote_runtime_qualification(
            &self,
            _handle: &QualificationHandle,
            _error_code: &str,
        ) -> Result<bool, QualifierError> {
            Ok(false)
        }
        fn record_qualification_error(
            &self,
            _handle: &QualificationHandle,
            _error_code: &str,
        ) -> Result<(), QualifierError> {
            Ok(())
        }
    }
    let manifest = model_registry().model(MODEL_ID).unwrap();
    let handle = QualificationHandle::from_manifest(
        manifest,
        directory.path(),
        DeviceIdentity {
            os: "macos".to_owned(),
            arch: "aarch64".to_owned(),
            backend: "metal".to_owned(),
            device_index: 0,
            macos_major: 14,
            memory_gib: 24,
            chip: "M4".to_owned(),
        },
    );
    let catalog = MutationCatalog {
        record: QualificationRecord {
            state: "installed_unqualified".to_owned(),
            manifest_version: handle.manifest_version.clone(),
            bundle_identity: handle.bundle_identity.clone(),
            install_dir: handle.install_dir.clone(),
            runtime_identity_json: None,
        },
        cas_calls: AtomicUsize::new(0),
    };
    assert!(
        RuntimeQualifier::new(&catalog, MutationSmoke(directory.path().to_path_buf()))
            .qualify(&handle)
            .is_err()
    );
    assert_eq!(catalog.cas_calls.load(Ordering::SeqCst), 0);
}

#[test]
fn mutated_qualification_contract_metadata_is_rejected() {
    let source_root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/asr");
    for field in [
        "expected_text",
        "expected_phrases",
        "minimum_phrase_matches",
        "normalization",
        "sha256",
        "canonical_sample_sha256",
        "source_archive_sha256",
        "source_archive_path",
        "license_spdx",
        "provenance",
        "qualification_contract_sha256",
    ] {
        let directory = tempfile::tempdir().unwrap();
        std::fs::copy(
            source_root.join("qwen06-codeswitch.wav"),
            directory.path().join("qwen06-codeswitch.wav"),
        )
        .unwrap();
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(source_root.join("fixtures.json")).unwrap())
                .unwrap();
        let row = manifest["fixtures"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|row| row["file"] == "qwen06-codeswitch.wav")
            .unwrap();
        row[field] = match &row[field] {
            serde_json::Value::Array(values) => {
                let mut values = values.clone();
                values.push(serde_json::Value::String("mutated".to_owned()));
                serde_json::Value::Array(values)
            }
            serde_json::Value::Number(number) => {
                serde_json::Value::Number((number.as_u64().unwrap() + 1).into())
            }
            _ => serde_json::Value::String("mutated".to_owned()),
        };
        std::fs::write(
            directory.path().join("fixtures.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        assert!(
            crate::asr::runtime_qualifier::load_qualification_speech_fixture_from(directory.path())
                .is_err(),
            "mutating {field} must invalidate the frozen qualification contract"
        );
    }
}

#[test]
fn reconciliation_removes_stale_uuid_qualification_marker_temps() {
    let (directory, catalog, handle) = setup();
    let stale = directory.path().join(format!(
        "{}.{}.tmp",
        RUNTIME_QUALIFICATION_MARKER,
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::write(&stale, b"partial marker").unwrap();

    RuntimeQualifier::new(
        catalog,
        FakeSmoke {
            calls: AtomicUsize::new(0),
            result: Err("must not run"),
        },
    )
    .reconcile(&handle)
    .unwrap();

    assert!(!stale.exists());
}

#[cfg(all(
    feature = "asr-qwen17-runtime",
    target_os = "macos",
    target_arch = "aarch64"
))]
#[test]
#[ignore = "set LIFESUB_RUN_QWEN17_SMOKE=1 and LIFESUB_QWEN17_MODEL_DIR to run real Metal qualification"]
fn real_qwen17_speech_smoke_revalidates_bundle_then_publishes_marker_and_cas() {
    assert_eq!(
        std::env::var("LIFESUB_RUN_QWEN17_SMOKE").as_deref(),
        Ok("1"),
        "selected real Qwen qualification gate requires LIFESUB_RUN_QWEN17_SMOKE=1"
    );
    let model_dir = std::env::var_os("LIFESUB_QWEN17_MODEL_DIR")
        .expect("selected real Qwen qualification gate requires LIFESUB_QWEN17_MODEL_DIR");
    let model_dir = std::path::PathBuf::from(model_dir);
    let manifest = model_registry().model(MODEL_ID).unwrap();
    let catalog = Arc::new(Catalog::in_memory().unwrap());
    catalog
        .publish_installation(&StoredInstallation {
            model_id: MODEL_ID.to_owned(),
            provider: "qwen3_asr".to_owned(),
            manifest_version: manifest.manifest_version.to_owned(),
            bundle_identity: manifest.bundle.identity_sha256.to_owned(),
            install_dir: model_dir.clone(),
            state: "installed_unqualified".to_owned(),
            runtime_identity_json: None,
        })
        .unwrap();
    let manager = crate::asr::model_manager::ModelManager::new(
        model_dir.parent().unwrap(),
        crate::asr::model_manager::ReqwestTransport::new().unwrap(),
        catalog.clone(),
    );
    manager.qualify_qwen17_current_device(&model_dir).unwrap();
    assert_eq!(
        catalog.model_installation_records().unwrap()[0].state,
        "runtime_qualified"
    );
    assert!(model_dir.join(RUNTIME_QUALIFICATION_MARKER).is_file());
}
