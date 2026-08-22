#[test]
fn incompatible_qwen_device_rejects_before_db_or_network() {
    let transport = ScriptedTransport::default();
    let catalog = MemoryCatalog::default();
    let root = TempDir::new().unwrap();
    let manager = ModelManager::new(root.path(), transport.clone(), catalog.clone());
    let mut device = compatible_device();
    device.memory_gib = 16;

    let error = manager
        .download_and_install(&qwen_plan(b"{}", "00"), &device, || false)
        .unwrap_err();

    assert_eq!(error.code(), "model_device_unsupported");
    assert_eq!(*catalog.begins.lock().unwrap(), 0);
    assert!(transport.requests.lock().unwrap().is_empty());
}

#[test]
fn interrupted_download_resumes_with_range_and_if_range() {
    let bytes = b"abcdef";
    let sha = hex::encode(sha2::Sha256::digest(bytes));
    let catalog = MemoryCatalog::default();
    let root = TempDir::new().unwrap();
    let part = root.path().join("downloads/download-1/config.part");
    fs::create_dir_all(part.parent().unwrap()).unwrap();
    fs::write(&part, b"abc").unwrap();
    catalog
        .checkpoints
        .lock()
        .unwrap()
        .push(ArtifactCheckpoint {
            artifact_id: "config".to_owned(),
            source_identity: config_source_identity(&sha),
            downloaded_bytes: 3,
            expected_bytes: 6,
            temp_path: part,
            etag: Some("etag-1".to_owned()),
            last_modified: None,
            verified_sha256: None,
            state: "downloading".to_owned(),
        });
    let transport = ScriptedTransport::with(vec![Ok(DownloadResponse {
        status: 206,
        final_url: "http://127.0.0.1/config".to_owned(),
        headers: [
            ("content-length".to_owned(), "3".to_owned()),
            ("content-range".to_owned(), "bytes 3-5/6".to_owned()),
            ("etag".to_owned(), "etag-1".to_owned()),
        ]
        .into_iter()
        .collect(),
        body: Box::new(std::io::Cursor::new(b"def".to_vec())),
    })]);
    let manager = ModelManager::new(root.path(), transport.clone(), catalog);

    let result = manager.download_only(&qwen_plan(bytes, &sha), &compatible_device(), || false);

    assert!(result.is_ok());
    let request = &transport.requests.lock().unwrap()[0];
    assert_eq!(request.range_start, Some(3));
    assert_eq!(request.if_range.as_deref(), Some("etag-1"));
    assert_eq!(
        fs::read(root.path().join("downloads/download-1/config.part")).unwrap(),
        bytes
    );
}

#[test]
fn ignored_range_restarts_instead_of_appending() {
    let bytes = b"abcdef";
    let sha = hex::encode(sha2::Sha256::digest(bytes));
    let catalog = MemoryCatalog::default();
    let root = TempDir::new().unwrap();
    let part = root.path().join("downloads/download-1/config.part");
    fs::create_dir_all(part.parent().unwrap()).unwrap();
    fs::write(&part, b"abc").unwrap();
    catalog
        .checkpoints
        .lock()
        .unwrap()
        .push(ArtifactCheckpoint {
            artifact_id: "config".to_owned(),
            source_identity: config_source_identity(&sha),
            downloaded_bytes: 3,
            expected_bytes: 6,
            temp_path: part.clone(),
            etag: Some("etag-1".to_owned()),
            last_modified: None,
            verified_sha256: None,
            state: "downloading".to_owned(),
        });
    let transport = ScriptedTransport::with(vec![Ok(DownloadResponse {
        status: 200,
        final_url: "http://127.0.0.1/config".to_owned(),
        headers: [("content-length".to_owned(), "6".to_owned())]
            .into_iter()
            .collect(),
        body: Box::new(std::io::Cursor::new(bytes.to_vec())),
    })]);
    let manager = ModelManager::new(root.path(), transport, catalog);

    manager
        .download_only(&qwen_plan(bytes, &sha), &compatible_device(), || false)
        .unwrap();

    assert_eq!(fs::read(part).unwrap(), bytes);
}

#[test]
fn additional_space_calculation_is_checked_and_not_double_counted() {
    assert_eq!(checked_required_additional_free(100, 120, 30), Some(250));
    assert_eq!(checked_required_additional_free(u64::MAX, 1, 0), None);
}

#[test]
fn qwen17_preflight_uses_exact_incremental_space_formula() {
    let plan = ModelInstallPlan::from_manifest(model_registry().model("qwen3-asr-1.7b").unwrap());
    let root = TempDir::new().unwrap();
    let catalog = MemoryCatalog::default();
    let manager = ModelManager::new(root.path(), ScriptedTransport::default(), catalog.clone());

    assert_eq!(
        manager
            .required_additional_free_for_test(&plan, None)
            .unwrap(),
        9_956_915_272
    );

    for artifact in &plan.artifacts {
        let part = root
            .path()
            .join("downloads/download-1")
            .join(format!("{}.part", artifact.artifact_id));
        fs::create_dir_all(part.parent().unwrap()).unwrap();
        let file = fs::File::create(&part).unwrap();
        file.set_len(artifact.expected_bytes).unwrap();
        catalog
            .checkpoints
            .lock()
            .unwrap()
            .push(ArtifactCheckpoint {
                artifact_id: artifact.artifact_id.clone(),
                source_identity: format!(
                    "{}\n{}\n{}\n{}\n{}\n{}",
                    artifact.source_repository,
                    artifact.source_model,
                    artifact.revision,
                    artifact.url,
                    artifact.expected_sha256,
                    artifact.required_path,
                ),
                downloaded_bytes: artifact.expected_bytes,
                expected_bytes: artifact.expected_bytes,
                temp_path: part,
                etag: None,
                last_modified: None,
                verified_sha256: Some(artifact.expected_sha256.clone()),
                state: "verified".to_owned(),
            });
    }
    assert_eq!(
        manager
            .required_additional_free_for_test(&plan, Some("download-1"))
            .unwrap(),
        5_246_893_092
    );
}

#[test]
fn insufficient_preflight_rejects_before_catalog_or_network() {
    let plan = ModelInstallPlan::from_manifest(model_registry().model("qwen3-asr-1.7b").unwrap());
    let root = TempDir::new().unwrap();
    let catalog = MemoryCatalog::default();
    let transport = ScriptedTransport::default();
    let manager = ModelManager::new(root.path(), transport.clone(), catalog.clone())
        .with_available_space_for_test(9_956_915_271);

    let error = manager
        .download_only(&plan, &compatible_device(), || false)
        .unwrap_err();

    assert_eq!(error.code(), "insufficient_disk_space");
    assert_eq!(*catalog.begins.lock().unwrap(), 0);
    assert!(transport.requests.lock().unwrap().is_empty());
}

#[test]
fn archive_preflight_uses_manifest_written_total_not_compressed_multiplier() {
    let artifact = ArtifactPlan {
        artifact_id: "high-ratio".to_owned(),
        source_repository: "repo".to_owned(),
        source_model: "archive".to_owned(),
        url: "http://127.0.0.1/archive".to_owned(),
        revision: "revision".to_owned(),
        expected_bytes: 1,
        expected_sha256: "a".repeat(64),
        required_path: "root".to_owned(),
        install_mode: InstallMode::ExtractTarBz2,
        redirect_hosts: vec!["127.0.0.1".to_owned()],
        license_spdx: "MIT".to_owned(),
        provenance: "fixture".to_owned(),
    };
    let plan = ModelInstallPlan {
        model_id: "high-ratio".to_owned(),
        provider: "whisper".to_owned(),
        manifest_version: "1".to_owned(),
        bundle_identity: "bundle".to_owned(),
        device: DeviceRequirement::AnyDesktop,
        qualification_policy: QualificationPolicy::StructuralWithPinnedRuntime,
        sherpa_runtime: Some(sherpa_identity()),
        artifacts: vec![artifact],
        install_contract: InstallContract::Archive {
            archive_root: "root".to_owned(),
            max_scanned_entries: 1,
            max_written_file_bytes: 1_000_000_000,
            max_total_written_bytes: 1_000_000_000,
            required_files: vec![RequiredInstalledFile {
                path: "model.onnx".to_owned(),
                bytes: 1_000_000_000,
                sha256: "b".repeat(64),
            }],
        },
    };
    let root = TempDir::new().unwrap();
    let catalog = MemoryCatalog::default();
    let transport = ScriptedTransport::default();
    let manager = ModelManager::new(root.path(), transport.clone(), catalog.clone())
        .with_available_space_for_test(600_000_000);

    let error = manager
        .download_only(&plan, &compatible_device(), || false)
        .unwrap_err();

    assert_eq!(error.code(), "insufficient_disk_space");
    assert_eq!(*catalog.begins.lock().unwrap(), 0);
    assert!(transport.requests.lock().unwrap().is_empty());
}

#[test]
fn preflight_overflow_and_checkpoint_metadata_failure_fail_closed() {
    let mut plan = qwen_plan(b"data", &hex::encode(sha2::Sha256::digest(b"data")));
    if let InstallContract::Direct {
        max_total_written_bytes,
        ..
    } = &mut plan.install_contract
    {
        *max_total_written_bytes = u64::MAX;
    }
    let root = TempDir::new().unwrap();
    let catalog = MemoryCatalog::default();
    let manager = ModelManager::new(root.path(), ScriptedTransport::default(), catalog.clone());
    let error = manager
        .required_additional_free_for_test(&plan, None)
        .unwrap_err();
    assert_eq!(error.code(), "insufficient_disk_space");

    let directory_checkpoint = root.path().join("downloads/download-1/config.part");
    fs::create_dir_all(&directory_checkpoint).unwrap();
    catalog
        .checkpoints
        .lock()
        .unwrap()
        .push(ArtifactCheckpoint {
            artifact_id: "config".to_owned(),
            source_identity: config_source_identity(&plan.artifacts[0].expected_sha256),
            downloaded_bytes: 1,
            expected_bytes: 4,
            temp_path: directory_checkpoint,
            etag: None,
            last_modified: None,
            verified_sha256: None,
            state: "downloading".to_owned(),
        });
    let error = manager
        .required_additional_free_for_test(
            &qwen_plan(b"data", &plan.artifacts[0].expected_sha256),
            Some("download-1"),
        )
        .unwrap_err();
    assert_eq!(error.code(), "insufficient_disk_space");
}

#[test]
fn pinned_runtime_requires_all_four_identity_fields() {
    let expected = FullSherpaRuntimeIdentity {
        version: "1.13.5".to_owned(),
        git_commit: "commit".to_owned(),
        native_archive_sha256: "archive".to_owned(),
        build_id: "build".to_owned(),
    };
    let mut observed = expected.clone();
    observed.build_id = "other".to_owned();

    assert!(!expected.matches(&observed));
}
