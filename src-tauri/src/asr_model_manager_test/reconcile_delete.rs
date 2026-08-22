#[test]
fn global_reconcile_completes_crash_left_trash_deletion() {
    let root = TempDir::new().unwrap();
    let install_dir = root.path().join("models/asr/whisper/trash-model/1-bundle");
    fs::create_dir_all(&install_dir).unwrap();
    fs::write(install_dir.join("model.onnx"), b"model").unwrap();
    let catalog = Catalog::in_memory().unwrap();
    catalog
        .publish_installation(&StoredInstallation {
            model_id: "trash-model".to_owned(),
            provider: "whisper".to_owned(),
            manifest_version: "1".to_owned(),
            bundle_identity: "bundle".to_owned(),
            install_dir: install_dir.clone(),
            state: "runtime_qualified".to_owned(),
            runtime_identity_json: Some("{}".to_owned()),
        })
        .unwrap();
    let lease = catalog.begin_delete("trash-model").unwrap().unwrap();
    fs::write(
        install_dir.join(".lifesub-delete.json"),
        serde_json::to_vec(&lease).unwrap(),
    )
    .unwrap();
    let trash = root.path().join("trash/trash-model-crash");
    fs::create_dir_all(trash.parent().unwrap()).unwrap();
    fs::rename(&install_dir, &trash).unwrap();
    let manager = ModelManager::new(root.path(), ScriptedTransport::default(), catalog);

    manager.reconcile_all().unwrap();

    assert!(!trash.exists());
    assert!(
        manager
            .catalog()
            .model_installation_records()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn reconcile_rejects_delete_marker_that_does_not_match_current_lease() {
    let root = TempDir::new().unwrap();
    let install_dir = root
        .path()
        .join("models/asr/whisper/mismatch-delete/1-bundle");
    fs::create_dir_all(&install_dir).unwrap();
    fs::write(install_dir.join("model.onnx"), b"model").unwrap();
    let catalog = Catalog::in_memory().unwrap();
    catalog
        .publish_installation(&StoredInstallation {
            model_id: "mismatch-delete".to_owned(),
            provider: "whisper".to_owned(),
            manifest_version: "1".to_owned(),
            bundle_identity: "bundle".to_owned(),
            install_dir: install_dir.clone(),
            state: "runtime_qualified".to_owned(),
            runtime_identity_json: Some("{}".to_owned()),
        })
        .unwrap();
    let mut lease = catalog.begin_delete("mismatch-delete").unwrap().unwrap();
    lease.prior_state = "installed_unqualified".to_owned();
    fs::write(
        install_dir.join(".lifesub-delete.json"),
        serde_json::to_vec(&lease).unwrap(),
    )
    .unwrap();
    let manager = ModelManager::new(root.path(), ScriptedTransport::default(), catalog);

    let error = manager.reconcile_all().unwrap_err();

    assert_eq!(error.code(), "model_integrity_failed");
    assert!(install_dir.join("model.onnx").is_file());
    assert_eq!(
        manager
            .catalog()
            .model_installation_state("mismatch-delete")
            .unwrap()
            .0,
        "deleting"
    );
}

#[test]
fn reconcile_rejects_stale_trash_marker_without_catalog_lease() {
    let root = TempDir::new().unwrap();
    let trash = root.path().join("trash/stale-delete");
    fs::create_dir_all(&trash).unwrap();
    let lease = DeletionLease {
        model_id: "stale-delete".to_owned(),
        install_dir: root.path().join("models/asr/whisper/stale-delete/1-bundle"),
        prior_state: "runtime_qualified".to_owned(),
        prior_runtime_identity_json: Some("{}".to_owned()),
        prior_qualified_at: Some("qualified-at".to_owned()),
        prior_last_error_code: None,
    };
    fs::write(
        trash.join(".lifesub-delete.json"),
        serde_json::to_vec(&lease).unwrap(),
    )
    .unwrap();
    let manager = ModelManager::new(
        root.path(),
        ScriptedTransport::default(),
        Catalog::in_memory().unwrap(),
    );

    let error = manager.reconcile_all().unwrap_err();

    assert_eq!(error.code(), "model_integrity_failed");
    assert!(trash.is_dir());
}

#[test]
fn global_reconcile_restores_delete_crash_before_marker() {
    let root = TempDir::new().unwrap();
    let install_dir = root.path().join("models/asr/whisper/pre-marker/1-bundle");
    fs::create_dir_all(&install_dir).unwrap();
    fs::write(install_dir.join("model.onnx"), b"model").unwrap();
    let catalog = Catalog::in_memory().unwrap();
    catalog
        .publish_installation(&StoredInstallation {
            model_id: "pre-marker".to_owned(),
            provider: "whisper".to_owned(),
            manifest_version: "1".to_owned(),
            bundle_identity: "bundle".to_owned(),
            install_dir: install_dir.clone(),
            state: "runtime_qualified".to_owned(),
            runtime_identity_json: Some("{\"runtime\":\"pinned\"}".to_owned()),
        })
        .unwrap();
    assert!(catalog.begin_delete("pre-marker").unwrap().is_some());
    let manager = ModelManager::new(root.path(), ScriptedTransport::default(), catalog);

    manager.reconcile_all().unwrap();

    assert!(install_dir.join("model.onnx").is_file());
    assert_eq!(
        manager
            .catalog()
            .model_installation_state("pre-marker")
            .unwrap()
            .0,
        "runtime_qualified"
    );
}

#[test]
fn global_reconcile_removes_incomplete_delete_marker_before_rollback() {
    let root = TempDir::new().unwrap();
    let install_dir = root.path().join("models/asr/whisper/tmp-marker/1-bundle");
    fs::create_dir_all(&install_dir).unwrap();
    fs::write(install_dir.join("model.onnx"), b"model").unwrap();
    let catalog = Catalog::in_memory().unwrap();
    catalog
        .publish_installation(&StoredInstallation {
            model_id: "tmp-marker".to_owned(),
            provider: "whisper".to_owned(),
            manifest_version: "1".to_owned(),
            bundle_identity: "bundle".to_owned(),
            install_dir: install_dir.clone(),
            state: "runtime_qualified".to_owned(),
            runtime_identity_json: Some("{}".to_owned()),
        })
        .unwrap();
    assert!(catalog.begin_delete("tmp-marker").unwrap().is_some());
    fs::write(install_dir.join(".lifesub-delete.json.tmp"), b"incomplete").unwrap();
    let manager = ModelManager::new(root.path(), ScriptedTransport::default(), catalog);

    manager.reconcile_all().unwrap();

    assert!(!install_dir.join(".lifesub-delete.json.tmp").exists());
    assert!(!install_dir.join(".lifesub-delete.json").exists());
    manager.delete("tmp-marker", &install_dir).unwrap();
    assert!(!install_dir.exists());
}

#[test]
fn core_runtime_startup_fails_closed_when_quarantine_cannot_be_published() {
    let parent = TempDir::new().unwrap();
    let data_dir = parent.path().join("data");
    fs::create_dir(&data_dir).unwrap();
    let catalog = Catalog::open(data_dir.join("lifesub.sqlite3")).unwrap();
    let plan = ModelInstallPlan::from_manifest(model_registry().model("whisper-tiny").unwrap());
    let install_dir = data_dir
        .join("models/asr")
        .join(&plan.provider)
        .join(&plan.model_id)
        .join(format!(
            "{}-{}",
            plan.manifest_version, plan.bundle_identity
        ));
    fs::create_dir_all(&install_dir).unwrap();
    fs::write(install_dir.join("corrupt"), b"corrupt").unwrap();
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
    drop(catalog);
    fs::write(data_dir.join("quarantine"), b"not-a-directory").unwrap();

    assert!(CoreRuntime::initialize(&data_dir).is_err());
}

#[test]
fn core_runtime_startup_fails_closed_when_catalog_recovery_publish_fails() {
    let parent = TempDir::new().unwrap();
    let data_dir = parent.path().join("data");
    fs::create_dir(&data_dir).unwrap();
    let catalog_path = data_dir.join("lifesub.sqlite3");
    let catalog = Catalog::open(&catalog_path).unwrap();
    let plan = ModelInstallPlan::from_manifest(model_registry().model("whisper-base").unwrap());
    let install_dir = data_dir
        .join("models/asr")
        .join(&plan.provider)
        .join(&plan.model_id)
        .join(format!(
            "{}-{}",
            plan.manifest_version, plan.bundle_identity
        ));
    fs::create_dir_all(&install_dir).unwrap();
    fs::write(install_dir.join("corrupt"), b"corrupt").unwrap();
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
    drop(catalog);
    rusqlite::Connection::open(&catalog_path)
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER fail_model_recovery BEFORE UPDATE ON model_installations
             BEGIN SELECT RAISE(FAIL, 'injected model recovery failure'); END;",
        )
        .unwrap();

    assert!(CoreRuntime::initialize(&data_dir).is_err());
}

#[test]
fn global_reconcile_quarantines_unknown_final_directory() {
    let root = TempDir::new().unwrap();
    let unknown = root
        .path()
        .join("models/asr/whisper/unknown-model/1-unknown");
    fs::create_dir_all(&unknown).unwrap();
    fs::write(unknown.join("model.onnx"), b"model").unwrap();
    let manager = ModelManager::new(
        root.path(),
        ScriptedTransport::default(),
        Catalog::in_memory().unwrap(),
    );

    manager.reconcile_all().unwrap();

    assert!(!unknown.exists());
    assert_eq!(
        fs::read_dir(root.path().join("quarantine"))
            .unwrap()
            .count(),
        1
    );
}
