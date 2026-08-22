#[test]
fn qwen_five_file_structural_install_remains_unqualified() {
    let bodies: [(&str, &str, &[u8]); 5] = [
        ("config", "config.json", br#"{"thinker_config":{}}"#),
        (
            "index",
            "model.safetensors.index.json",
            br#"{"weight_map":{"a":"model-00001-of-00002.safetensors","b":"model-00002-of-00002.safetensors"}}"#,
        ),
        ("shard1", "model-00001-of-00002.safetensors", b"one"),
        ("shard2", "model-00002-of-00002.safetensors", b"two"),
        (
            "tokenizer",
            "tokenizer.json",
            br#"{"model":{"vocab":{"a":0}},"added_tokens":[{"content":"<|endoftext|>"},{"content":"<|im_start|>"},{"content":"<|im_end|>"}]}"#,
        ),
    ];
    let artifacts = bodies
        .iter()
        .map(|(id, path, body)| direct_artifact(id, path, body))
        .collect::<Vec<_>>();
    let responses = artifacts
        .iter()
        .zip(bodies.iter())
        .map(|(artifact, (_, _, body))| response(&artifact.url, body))
        .collect::<Vec<_>>();
    let plan = ModelInstallPlan {
        model_id: "qwen3-asr-1.7b".to_owned(),
        provider: "qwen3_asr".to_owned(),
        manifest_version: "2".to_owned(),
        bundle_identity: "bundle".to_owned(),
        device: DeviceRequirement::AppleSiliconMetal {
            minimum_macos_major: 14,
            minimum_memory_gib: 24,
            chip: "M4".to_owned(),
        },
        qualification_policy: QualificationPolicy::RuntimeSmokeRequired,
        sherpa_runtime: None,
        install_contract: direct_contract(&artifacts),
        artifacts,
    };
    let catalog = MemoryCatalog::default();
    *catalog.publish_failures.lock().unwrap() = 2;
    let root = TempDir::new().unwrap();
    let manager = ModelManager::new(
        root.path(),
        ScriptedTransport::with(responses),
        catalog.clone(),
    );

    assert!(
        manager
            .download_and_install(&plan, &compatible_device(), || false)
            .is_err()
    );
    let installation = manager.reconcile_installation(&plan).unwrap();

    assert_eq!(installation.state, "installed_unqualified");
    assert_eq!(
        catalog.installations.lock().unwrap()[0].state,
        "installed_unqualified"
    );
}

#[test]
fn rename_before_db_crash_reconciles_structural_policy() {
    let body = b"model";
    let artifact = direct_artifact("model", "model.onnx", body);
    let plan = ModelInstallPlan {
        model_id: "whisper-base".to_owned(),
        provider: "whisper".to_owned(),
        manifest_version: "1".to_owned(),
        bundle_identity: "bundle".to_owned(),
        device: DeviceRequirement::AnyDesktop,
        qualification_policy: QualificationPolicy::StructuralWithPinnedRuntime,
        sherpa_runtime: Some(sherpa_identity()),
        install_contract: direct_contract(std::slice::from_ref(&artifact)),
        artifacts: vec![artifact.clone()],
    };
    let catalog = MemoryCatalog::default();
    *catalog.publish_failures.lock().unwrap() = 2;
    let root = TempDir::new().unwrap();
    let manager = ModelManager::new(
        root.path(),
        ScriptedTransport::with(vec![response(&artifact.url, body)]),
        catalog.clone(),
    )
    .with_sherpa_runtime_identity(sherpa_identity());

    assert!(
        manager
            .download_and_install(&plan, &compatible_device(), || false)
            .is_err()
    );
    let installation = manager.reconcile_installation(&plan).unwrap();

    assert_eq!(installation.state, "runtime_qualified");
    assert_eq!(catalog.installations.lock().unwrap().len(), 1);
}

#[test]
fn reconciliation_rejects_extra_or_mutated_installed_files() {
    let body = b"model";
    let artifact = direct_artifact("model", "model.onnx", body);
    let plan = ModelInstallPlan {
        model_id: "inventory-model".to_owned(),
        provider: "whisper".to_owned(),
        manifest_version: "1".to_owned(),
        bundle_identity: "bundle".to_owned(),
        device: DeviceRequirement::AnyDesktop,
        qualification_policy: QualificationPolicy::StructuralWithPinnedRuntime,
        sherpa_runtime: Some(sherpa_identity()),
        install_contract: direct_contract(std::slice::from_ref(&artifact)),
        artifacts: vec![artifact.clone()],
    };
    let root = TempDir::new().unwrap();
    let manager = ModelManager::new(
        root.path(),
        ScriptedTransport::with(vec![response(&artifact.url, body)]),
        MemoryCatalog::default(),
    )
    .with_sherpa_runtime_identity(sherpa_identity());
    let installation = manager
        .download_and_install(&plan, &compatible_device(), || false)
        .unwrap();
    fs::write(installation.install_dir.join("unexpected.bin"), b"extra").unwrap();

    let error = manager.reconcile_installation(&plan).unwrap_err();

    assert_eq!(error.code(), "model_integrity_failed");
    assert_eq!(
        fs::read_dir(root.path().join("quarantine"))
            .unwrap()
            .count(),
        1
    );
}
#[test]
fn reconciliation_rejects_fifo_without_opening_or_hashing_it() {
    let body = b"model";
    let plan = simple_sherpa_plan("fifo-model", body);
    let root = TempDir::new().unwrap();
    let manager = ModelManager::new(
        root.path(),
        ScriptedTransport::with(vec![response(&plan.artifacts[0].url, body)]),
        MemoryCatalog::default(),
    )
    .with_sherpa_runtime_identity(sherpa_identity());
    let installation = manager
        .download_and_install(&plan, &compatible_device(), || false)
        .unwrap();
    let fifo = installation.install_dir.join("unexpected-fifo");
    let fifo_c = CString::new(fifo.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(fifo_c.as_ptr(), 0o600) }, 0);

    let started = Instant::now();
    let error = manager.reconcile_installation(&plan).unwrap_err();

    assert_eq!(error.code(), "model_structural_incompatible");
    assert!(started.elapsed() < Duration::from_secs(1));
    assert!(!installation.install_dir.exists());
}

#[test]
fn global_reconcile_never_follows_provider_model_or_final_symlinks() {
    for level in ["provider", "model", "final"] {
        let root = TempDir::new().unwrap();
        let outside = root.path().join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("sentinel"), b"outside").unwrap();
        let models = root.path().join("models/asr");
        fs::create_dir_all(&models).unwrap();
        let link = match level {
            "provider" => models.join("whisper"),
            "model" => {
                let provider = models.join("whisper");
                fs::create_dir(&provider).unwrap();
                provider.join("whisper-tiny")
            }
            "final" => {
                let model = models.join("whisper/whisper-tiny");
                fs::create_dir_all(&model).unwrap();
                model.join("1-bundle")
            }
            _ => unreachable!(),
        };
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        let manager = ModelManager::new(
            root.path(),
            ScriptedTransport::default(),
            Catalog::in_memory().unwrap(),
        );

        let error = manager.reconcile_all().unwrap_err();

        assert_eq!(error.code(), "model_structural_incompatible");
        assert_eq!(fs::read(outside.join("sentinel")).unwrap(), b"outside");
        assert!(link.symlink_metadata().unwrap().file_type().is_symlink());
    }
}

#[test]
fn reconciliation_rejects_self_consistent_direct_file_and_marker_tamper() {
    let body = b"model";
    let artifact = direct_artifact("model", "model.onnx", body);
    let plan = ModelInstallPlan {
        model_id: "self-consistent-tamper".to_owned(),
        provider: "whisper".to_owned(),
        manifest_version: "1".to_owned(),
        bundle_identity: "bundle".to_owned(),
        device: DeviceRequirement::AnyDesktop,
        qualification_policy: QualificationPolicy::StructuralWithPinnedRuntime,
        sherpa_runtime: Some(sherpa_identity()),
        install_contract: direct_contract(std::slice::from_ref(&artifact)),
        artifacts: vec![artifact.clone()],
    };
    let root = TempDir::new().unwrap();
    let manager = ModelManager::new(
        root.path(),
        ScriptedTransport::with(vec![response(&artifact.url, body)]),
        MemoryCatalog::default(),
    )
    .with_sherpa_runtime_identity(sherpa_identity());
    let installation = manager
        .download_and_install(&plan, &compatible_device(), || false)
        .unwrap();
    let tampered = b"xxxxx";
    fs::write(installation.install_dir.join("model.onnx"), tampered).unwrap();
    let marker_path = installation.install_dir.join(".lifesub-structural.json");
    let mut marker: serde_json::Value =
        serde_json::from_slice(&fs::read(&marker_path).unwrap()).unwrap();
    marker["installed_files"][0]["sha256"] =
        serde_json::Value::String(hex::encode(sha2::Sha256::digest(tampered)));
    fs::write(&marker_path, serde_json::to_vec(&marker).unwrap()).unwrap();

    let error = manager.reconcile_installation(&plan).unwrap_err();

    assert_eq!(error.code(), "model_integrity_failed");
}

#[test]
fn reconcile_with_wrong_runtime_quarantines_renamed_install() {
    let body = b"model";
    let artifact = direct_artifact("model", "model.onnx", body);
    let plan = ModelInstallPlan {
        model_id: "whisper-small".to_owned(),
        provider: "whisper".to_owned(),
        manifest_version: "1".to_owned(),
        bundle_identity: "bundle".to_owned(),
        device: DeviceRequirement::AnyDesktop,
        qualification_policy: QualificationPolicy::StructuralWithPinnedRuntime,
        sherpa_runtime: Some(sherpa_identity()),
        install_contract: direct_contract(std::slice::from_ref(&artifact)),
        artifacts: vec![artifact.clone()],
    };
    let catalog = MemoryCatalog::default();
    *catalog.publish_failures.lock().unwrap() = 2;
    let root = TempDir::new().unwrap();
    let installer = ModelManager::new(
        root.path(),
        ScriptedTransport::with(vec![response(&artifact.url, body)]),
        catalog.clone(),
    )
    .with_sherpa_runtime_identity(sherpa_identity());
    assert!(
        installer
            .download_and_install(&plan, &compatible_device(), || false)
            .is_err()
    );
    let mut wrong = sherpa_identity();
    wrong.native_archive_sha256 = "wrong".to_owned();
    let reconciler = ModelManager::new(root.path(), ScriptedTransport::default(), catalog.clone())
        .with_sherpa_runtime_identity(wrong);

    let error = reconciler.reconcile_installation(&plan).unwrap_err();

    assert_eq!(error.code(), "model_runtime_identity_mismatch");
    assert!(catalog.installations.lock().unwrap().is_empty());
    assert_eq!(
        fs::read_dir(root.path().join("quarantine"))
            .unwrap()
            .count(),
        1
    );
}
