#[test]
fn real_catalog_persists_five_artifact_checkpoints() {
    let bodies = [b"a".as_slice(), b"b", b"c", b"d", b"e"];
    let artifacts = bodies
        .iter()
        .enumerate()
        .map(|(index, body)| direct_artifact(&format!("a{index}"), &format!("a{index}.bin"), body))
        .collect::<Vec<_>>();
    let responses = artifacts
        .iter()
        .zip(bodies.iter())
        .map(|(artifact, body)| response(&artifact.url, body))
        .collect();
    let plan = ModelInstallPlan {
        model_id: "fixture-five".to_owned(),
        provider: "whisper".to_owned(),
        manifest_version: "1".to_owned(),
        bundle_identity: "bundle".to_owned(),
        device: DeviceRequirement::AnyDesktop,
        qualification_policy: QualificationPolicy::StructuralWithPinnedRuntime,
        sherpa_runtime: Some(sherpa_identity()),
        install_contract: direct_contract(&artifacts),
        artifacts,
    };
    let catalog = Catalog::in_memory().unwrap();
    let root = TempDir::new().unwrap();
    let manager = ModelManager::new(root.path(), ScriptedTransport::with(responses), catalog);

    let download_id = manager
        .download_only(&plan, &compatible_device(), || false)
        .unwrap();

    assert_eq!(
        manager
            .catalog()
            .model_artifact_count(&download_id)
            .unwrap(),
        5
    );
}

#[test]
fn delete_cas_has_one_winner_and_active_lease_blocks_it() {
    let catalog = Arc::new(Catalog::in_memory().unwrap());
    let installation = StoredInstallation {
        model_id: "delete-model".to_owned(),
        provider: "whisper".to_owned(),
        manifest_version: "1".to_owned(),
        bundle_identity: "bundle".to_owned(),
        install_dir: PathBuf::from("models/delete-model"),
        state: "runtime_qualified".to_owned(),
        runtime_identity_json: Some("{}".to_owned()),
    };
    catalog.publish_installation(&installation).unwrap();
    let barrier = Arc::new(std::sync::Barrier::new(3));
    let workers = (0..2)
        .map(|_| {
            let catalog = catalog.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                catalog.begin_delete("delete-model").unwrap()
            })
        })
        .collect::<Vec<_>>();
    barrier.wait();
    let leases = workers
        .into_iter()
        .filter_map(|worker| worker.join().unwrap())
        .collect::<Vec<_>>();
    let winners = leases.len();
    assert_eq!(winners, 1);
    catalog.abort_delete(&leases[0]).unwrap();
    catalog.insert_test_model_lease("delete-model").unwrap();

    assert!(catalog.begin_delete("delete-model").unwrap().is_none());
}

#[test]
fn qualified_delete_db_failure_restores_directory_and_exact_prior_state() {
    let root = TempDir::new().unwrap();
    let install_dir = root.path().join("models/asr/whisper/qualified/1-bundle");
    fs::create_dir_all(&install_dir).unwrap();
    fs::write(install_dir.join("model.onnx"), b"model").unwrap();
    let catalog = MemoryCatalog::default();
    catalog
        .installations
        .lock()
        .unwrap()
        .push(StoredInstallation {
            model_id: "qualified".to_owned(),
            provider: "whisper".to_owned(),
            manifest_version: "1".to_owned(),
            bundle_identity: "bundle".to_owned(),
            install_dir: install_dir.clone(),
            state: "runtime_qualified".to_owned(),
            runtime_identity_json: Some("{\"runtime\":\"pinned\"}".to_owned()),
        });
    *catalog.finish_delete_failures.lock().unwrap() = 1;
    let manager = ModelManager::new(root.path(), ScriptedTransport::default(), catalog.clone());

    let error = manager.delete("qualified", &install_dir).unwrap_err();

    assert_eq!(error.code(), "model_catalog_failed");
    assert!(install_dir.join("model.onnx").is_file());
    let restored = catalog.restored_deletions.lock().unwrap();
    assert_eq!(restored.len(), 1);
    assert_eq!(restored[0].prior_state, "runtime_qualified");
    assert_eq!(
        restored[0].prior_runtime_identity_json.as_deref(),
        Some("{\"runtime\":\"pinned\"}")
    );
    assert_eq!(
        restored[0].prior_qualified_at.as_deref(),
        Some("qualified-at")
    );
}

fn assert_delete_marker_fault_is_retryable(fault: DeleteMarkerFault) {
    let root = TempDir::new().unwrap();
    let install_dir = root.path().join("models/asr/whisper/delete-fault/1-bundle");
    fs::create_dir_all(&install_dir).unwrap();
    fs::write(install_dir.join("model.onnx"), b"model").unwrap();
    let catalog = MemoryCatalog::default();
    catalog
        .installations
        .lock()
        .unwrap()
        .push(StoredInstallation {
            model_id: "delete-fault".to_owned(),
            provider: "whisper".to_owned(),
            manifest_version: "1".to_owned(),
            bundle_identity: "bundle".to_owned(),
            install_dir: install_dir.clone(),
            state: "runtime_qualified".to_owned(),
            runtime_identity_json: Some("{}".to_owned()),
        });
    let manager = ModelManager::new(root.path(), ScriptedTransport::default(), catalog.clone())
        .with_delete_marker_fault_for_test(fault);

    assert!(manager.delete("delete-fault", &install_dir).is_err());
    assert!(install_dir.join("model.onnx").is_file());
    assert!(!install_dir.join(".lifesub-delete.json.tmp").exists());
    assert!(!install_dir.join(".lifesub-delete.json").exists());

    let retry = ModelManager::new(root.path(), ScriptedTransport::default(), catalog);
    retry.delete("delete-fault", &install_dir).unwrap();
    assert!(!install_dir.exists());
}

#[test]
fn delete_marker_write_failure_is_clean_and_retryable() {
    assert_delete_marker_fault_is_retryable(DeleteMarkerFault::Write);
}

#[test]
fn delete_marker_sync_failure_is_clean_and_retryable() {
    assert_delete_marker_fault_is_retryable(DeleteMarkerFault::Sync);
}

#[test]
fn delete_marker_rename_failure_is_clean_and_retryable() {
    assert_delete_marker_fault_is_retryable(DeleteMarkerFault::Rename);
}

#[test]
fn successful_delete_removes_catalog_row_before_cleaning_trash() {
    let root = TempDir::new().unwrap();
    let install_dir = root.path().join("models/asr/whisper/delete-ok/1-bundle");
    fs::create_dir_all(&install_dir).unwrap();
    fs::write(install_dir.join("model.onnx"), b"model").unwrap();
    let catalog = Catalog::in_memory().unwrap();
    catalog
        .publish_installation(&StoredInstallation {
            model_id: "delete-ok".to_owned(),
            provider: "whisper".to_owned(),
            manifest_version: "1".to_owned(),
            bundle_identity: "bundle".to_owned(),
            install_dir: install_dir.clone(),
            state: "runtime_qualified".to_owned(),
            runtime_identity_json: Some("{}".to_owned()),
        })
        .unwrap();
    let manager = ModelManager::new(root.path(), ScriptedTransport::default(), catalog);

    manager.delete("delete-ok", &install_dir).unwrap();

    assert!(!install_dir.exists());
    assert!(
        manager
            .catalog()
            .model_installation_records()
            .unwrap()
            .is_empty()
    );
    assert_eq!(fs::read_dir(root.path().join("trash")).unwrap().count(), 0);
}

#[test]
fn db_installation_with_missing_directory_is_downgraded_fail_closed() {
    let body = b"model";
    let artifact = direct_artifact("model", "model.onnx", body);
    let plan = ModelInstallPlan {
        model_id: "missing-model".to_owned(),
        provider: "whisper".to_owned(),
        manifest_version: "1".to_owned(),
        bundle_identity: "bundle".to_owned(),
        device: DeviceRequirement::AnyDesktop,
        qualification_policy: QualificationPolicy::StructuralWithPinnedRuntime,
        sherpa_runtime: Some(sherpa_identity()),
        install_contract: direct_contract(std::slice::from_ref(&artifact)),
        artifacts: vec![artifact],
    };
    let root = TempDir::new().unwrap();
    let install_dir = root
        .path()
        .join("models/asr/whisper/missing-model/1-bundle");
    let catalog = Catalog::in_memory().unwrap();
    catalog
        .publish_installation(&StoredInstallation {
            model_id: plan.model_id.clone(),
            provider: plan.provider.clone(),
            manifest_version: plan.manifest_version.clone(),
            bundle_identity: plan.bundle_identity.clone(),
            install_dir,
            state: "runtime_qualified".to_owned(),
            runtime_identity_json: Some("{}".to_owned()),
        })
        .unwrap();
    let manager = ModelManager::new(root.path(), ScriptedTransport::default(), catalog)
        .with_sherpa_runtime_identity(sherpa_identity());

    let error = manager.reconcile_installation(&plan).unwrap_err();

    assert_eq!(error.code(), "model_install_missing");
    assert_eq!(
        manager
            .catalog()
            .model_installation_state("missing-model")
            .unwrap(),
        (
            "installed_unqualified".to_owned(),
            Some("model_integrity_failed".to_owned())
        )
    );
}
