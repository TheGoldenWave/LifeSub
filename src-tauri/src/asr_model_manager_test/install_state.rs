#[test]
fn matching_structural_runtime_publishes_runtime_qualified() {
    let body = b"model";
    let artifact = direct_artifact("model", "model.onnx", body);
    let plan = ModelInstallPlan {
        model_id: "sense-voice".to_owned(),
        provider: "sense_voice".to_owned(),
        manifest_version: "1".to_owned(),
        bundle_identity: "bundle".to_owned(),
        device: DeviceRequirement::AnyDesktop,
        qualification_policy: QualificationPolicy::StructuralWithPinnedRuntime,
        sherpa_runtime: Some(sherpa_identity()),
        install_contract: direct_contract(std::slice::from_ref(&artifact)),
        artifacts: vec![artifact.clone()],
    };
    let transport = ScriptedTransport::with(vec![response(&artifact.url, body)]);
    let catalog = MemoryCatalog::default();
    let root = TempDir::new().unwrap();
    let manager = ModelManager::new(root.path(), transport, catalog.clone())
        .with_sherpa_runtime_identity(sherpa_identity());

    let installation = manager
        .download_and_install(&plan, &compatible_device(), || false)
        .unwrap();

    assert_eq!(installation.state, "runtime_qualified");
    assert_eq!(catalog.installations.lock().unwrap().len(), 1);
}

#[test]
fn install_rechecks_disk_after_download_before_creating_staging() {
    let body = b"model";
    let plan = simple_sherpa_plan("disk-changed", body);
    let catalog = MemoryCatalog::default();
    let root = TempDir::new().unwrap();
    let manager = ModelManager::new(
        root.path(),
        ScriptedTransport::with(vec![response(&plan.artifacts[0].url, body)]),
        catalog.clone(),
    )
    .with_sherpa_runtime_identity(sherpa_identity())
    .with_available_space_sequence_for_test(vec![u64::MAX, 0]);

    let error = manager
        .download_and_install(&plan, &compatible_device(), || false)
        .unwrap_err();

    assert_eq!(error.code(), "insufficient_disk_space");
    assert_eq!(
        fs::read_dir(root.path().join("staging")).unwrap().count(),
        0
    );
    assert_eq!(
        catalog.download_states.lock().unwrap().last(),
        Some(&(
            "failed".to_owned(),
            Some("insufficient_disk_space".to_owned())
        ))
    );
    let retry = ModelManager::new(root.path(), ScriptedTransport::default(), catalog.clone())
        .with_sherpa_runtime_identity(sherpa_identity())
        .with_available_space_for_test(u64::MAX);
    assert!(retry.retry_install(&plan, "download-1").is_ok());
}

fn assert_pre_rename_install_failure_releases_active_and_retries(fault: InstallFault) {
    let body = b"model";
    let plan = simple_sherpa_plan("retry-install", body);
    let catalog = MemoryCatalog::default();
    let root = TempDir::new().unwrap();
    let manager = ModelManager::new(
        root.path(),
        ScriptedTransport::with(vec![response(&plan.artifacts[0].url, body)]),
        catalog.clone(),
    )
    .with_sherpa_runtime_identity(sherpa_identity())
    .with_install_fault_for_test(fault);

    let error = manager
        .download_and_install(&plan, &compatible_device(), || false)
        .unwrap_err();
    assert_eq!(error.code(), "model_install_failed");
    assert_eq!(
        catalog.download_states.lock().unwrap().last(),
        Some(&("failed".to_owned(), Some("model_install_failed".to_owned())))
    );
    assert!(
        !root
            .path()
            .join("models/asr/whisper/retry-install/1-bundle")
            .exists()
    );

    let retry = ModelManager::new(root.path(), ScriptedTransport::default(), catalog.clone())
        .with_sherpa_runtime_identity(sherpa_identity());
    let installation = retry.retry_install(&plan, "download-1").unwrap();
    assert_eq!(installation.state, "runtime_qualified");
    assert_eq!(
        catalog.download_states.lock().unwrap().last(),
        Some(&("succeeded".to_owned(), None))
    );
}

#[test]
fn assembly_failure_releases_active_download_and_retries_without_network() {
    assert_pre_rename_install_failure_releases_active_and_retries(InstallFault::Assembly);
}

#[test]
fn rename_failure_releases_active_download_and_retries_without_network() {
    assert_pre_rename_install_failure_releases_active_and_retries(InstallFault::Rename);
}

#[test]
fn catalog_publish_ambiguity_reconciles_synchronously() {
    let body = b"model";
    let plan = simple_sherpa_plan("publish-ambiguity", body);
    let catalog = MemoryCatalog::default();
    *catalog.publish_failures.lock().unwrap() = 1;
    let root = TempDir::new().unwrap();
    let manager = ModelManager::new(
        root.path(),
        ScriptedTransport::with(vec![response(&plan.artifacts[0].url, body)]),
        catalog.clone(),
    )
    .with_sherpa_runtime_identity(sherpa_identity());

    let installation = manager
        .download_and_install(&plan, &compatible_device(), || false)
        .unwrap();

    assert_eq!(installation.state, "runtime_qualified");
    assert_eq!(catalog.installations.lock().unwrap().len(), 1);
    assert_eq!(
        catalog.download_states.lock().unwrap().last(),
        Some(&("succeeded".to_owned(), None))
    );
}

#[test]
fn final_succeeded_state_failure_reconciles_and_retries_transition() {
    let body = b"model";
    let plan = simple_sherpa_plan("state-ambiguity", body);
    let catalog = MemoryCatalog::default();
    *catalog.state_failure.lock().unwrap() = Some("succeeded".to_owned());
    let root = TempDir::new().unwrap();
    let manager = ModelManager::new(
        root.path(),
        ScriptedTransport::with(vec![response(&plan.artifacts[0].url, body)]),
        catalog.clone(),
    )
    .with_sherpa_runtime_identity(sherpa_identity());

    let installation = manager
        .download_and_install(&plan, &compatible_device(), || false)
        .unwrap();

    assert_eq!(installation.state, "runtime_qualified");
    assert_eq!(
        catalog.download_states.lock().unwrap().last(),
        Some(&("succeeded".to_owned(), None))
    );
}

#[test]
fn persistent_catalog_publish_failure_marks_recovery_required_and_retries() {
    let body = b"model";
    let plan = simple_sherpa_plan("publish-retry", body);
    let catalog = MemoryCatalog::default();
    *catalog.publish_failures.lock().unwrap() = 2;
    let root = TempDir::new().unwrap();
    let manager = ModelManager::new(
        root.path(),
        ScriptedTransport::with(vec![response(&plan.artifacts[0].url, body)]),
        catalog.clone(),
    )
    .with_sherpa_runtime_identity(sherpa_identity());

    assert!(
        manager
            .download_and_install(&plan, &compatible_device(), || false)
            .is_err()
    );
    assert_eq!(
        catalog.download_states.lock().unwrap().last(),
        Some(&("failed".to_owned(), Some("recovery_required".to_owned())))
    );

    let retry = ModelManager::new(root.path(), ScriptedTransport::default(), catalog.clone())
        .with_sherpa_runtime_identity(sherpa_identity());
    assert!(retry.retry_install(&plan, "download-1").is_ok());
    assert_eq!(
        catalog.download_states.lock().unwrap().last(),
        Some(&("succeeded".to_owned(), None))
    );
}

#[test]
fn mismatched_structural_runtime_publishes_no_installation() {
    let body = b"model";
    let artifact = direct_artifact("model", "model.onnx", body);
    let plan = ModelInstallPlan {
        model_id: "whisper-tiny".to_owned(),
        provider: "whisper".to_owned(),
        manifest_version: "1".to_owned(),
        bundle_identity: "bundle".to_owned(),
        device: DeviceRequirement::AnyDesktop,
        qualification_policy: QualificationPolicy::StructuralWithPinnedRuntime,
        sherpa_runtime: Some(sherpa_identity()),
        install_contract: direct_contract(std::slice::from_ref(&artifact)),
        artifacts: vec![artifact.clone()],
    };
    let transport = ScriptedTransport::with(vec![response(&artifact.url, body)]);
    let catalog = MemoryCatalog::default();
    let root = TempDir::new().unwrap();
    let mut wrong = sherpa_identity();
    wrong.build_id = "wrong".to_owned();
    let manager = ModelManager::new(root.path(), transport, catalog.clone())
        .with_sherpa_runtime_identity(wrong);

    let error = manager
        .download_and_install(&plan, &compatible_device(), || false)
        .unwrap_err();

    assert_eq!(error.code(), "model_runtime_identity_mismatch");
    assert!(catalog.installations.lock().unwrap().is_empty());
    assert_eq!(
        fs::read_dir(root.path().join("quarantine"))
            .unwrap()
            .count(),
        1
    );
    assert_eq!(
        catalog.download_states.lock().unwrap().last(),
        Some(&(
            "failed".to_owned(),
            Some("model_runtime_identity_mismatch".to_owned())
        ))
    );
    let retry = ModelManager::new(root.path(), ScriptedTransport::default(), catalog.clone())
        .with_sherpa_runtime_identity(sherpa_identity());
    assert!(retry.retry_install(&plan, "download-1").is_ok());
}
