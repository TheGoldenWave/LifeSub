#[test]
fn shipping_manifests_map_to_explicit_task6_policies() {
    for manifest in model_registry().models() {
        let plan = ModelInstallPlan::from_manifest(manifest);
        if manifest.id == "qwen3-asr-1.7b" {
            assert_eq!(
                plan.qualification_policy,
                QualificationPolicy::RuntimeSmokeRequired
            );
            assert!(plan.sherpa_runtime.is_none());
        } else {
            assert_eq!(
                plan.qualification_policy,
                QualificationPolicy::StructuralWithPinnedRuntime
            );
            assert_eq!(plan.sherpa_runtime, Some(sherpa_identity()));
        }
        let required = plan
            .install_contract
            .required_files()
            .iter()
            .map(|file| file.path.as_str())
            .collect::<HashSet<_>>();
        let essentials: &[&str] = match manifest.id {
            "sense-voice-small-int8-2024-07-17" => &["model.int8.onnx", "tokens.txt"],
            "whisper-tiny" => &["tiny-encoder.onnx", "tiny-decoder.onnx", "tiny-tokens.txt"],
            "whisper-base" => &["base-encoder.onnx", "base-decoder.onnx", "base-tokens.txt"],
            "whisper-small" => &[
                "small-encoder.onnx",
                "small-decoder.onnx",
                "small-tokens.txt",
            ],
            "qwen3-asr-0.6b-int8-2026-03-25" => &[
                "conv_frontend.onnx",
                "encoder.int8.onnx",
                "decoder.int8.onnx",
                "tokenizer/merges.txt",
                "tokenizer/tokenizer_config.json",
                "tokenizer/vocab.json",
            ],
            "qwen3-asr-1.7b" => &[
                "config.json",
                "model-00001-of-00002.safetensors",
                "model-00002-of-00002.safetensors",
                "model.safetensors.index.json",
                "tokenizer.json",
            ],
            other => panic!("unhandled shipping model {other}"),
        };
        assert!(essentials.iter().all(|path| required.contains(path)));
    }
    let vad = ModelInstallPlan::from_vad_manifest(vad_manifest());
    assert_eq!(
        vad.qualification_policy,
        QualificationPolicy::StructuralWithPinnedRuntime
    );
    assert_eq!(vad.sherpa_runtime, Some(sherpa_identity()));
    assert_eq!(vad.install_contract.required_files().len(), 1);
    assert_eq!(
        vad.install_contract.required_files()[0].path,
        "silero_vad.onnx"
    );
}

#[test]
fn every_shipping_required_file_is_mandatory_for_structural_publication() {
    let plans = model_registry()
        .models()
        .iter()
        .map(ModelInstallPlan::from_manifest)
        .chain(std::iter::once(ModelInstallPlan::from_vad_manifest(
            vad_manifest(),
        )))
        .collect::<Vec<_>>();

    for plan in plans {
        let exact = plan.install_contract.required_files().to_vec();
        validate_required_inventory_for_test(&plan, &exact).unwrap();
        for missing in 0..exact.len() {
            let mut incomplete = exact.clone();
            let removed = incomplete.remove(missing);
            assert!(
                validate_required_inventory_for_test(&plan, &incomplete).is_err(),
                "{} accepted missing {}",
                plan.model_id,
                removed.path
            );
        }
    }
}

#[cfg(feature = "asr-runtime")]
#[test]
fn trusted_sherpa_runtime_matches_full_task6_identity() {
    let observed = crate::asr::verify_runtime_identity().unwrap();
    let full = FullSherpaRuntimeIdentity {
        version: observed.version,
        git_commit: observed.pinned_git_sha1.to_owned(),
        native_archive_sha256: observed.native_archive_sha256.to_owned(),
        build_id: observed.build_id.to_owned(),
    };

    assert_eq!(full, sherpa_identity());
}

#[test]
fn injected_loopback_transport_downloads_five_artifacts() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let bodies = [b"one".as_slice(), b"two", b"three", b"four", b"five"];
    let server_bodies = bodies.map(<[u8]>::to_vec);
    let server = std::thread::spawn(move || {
        for body in server_bodies {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 2048];
            let _ = stream.read(&mut request).unwrap();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            stream.write_all(&body).unwrap();
        }
    });
    let artifacts = bodies
        .iter()
        .enumerate()
        .map(|(index, body)| {
            let mut artifact = direct_artifact(
                &format!("loopback-{index}"),
                &format!("loopback-{index}.bin"),
                body,
            );
            artifact.url = format!("http://{address}/artifact-{index}");
            artifact
        })
        .collect::<Vec<_>>();
    let plan = ModelInstallPlan {
        model_id: "loopback-five".to_owned(),
        provider: "whisper".to_owned(),
        manifest_version: "1".to_owned(),
        bundle_identity: "bundle".to_owned(),
        device: DeviceRequirement::AnyDesktop,
        qualification_policy: QualificationPolicy::StructuralWithPinnedRuntime,
        sherpa_runtime: Some(sherpa_identity()),
        install_contract: direct_contract(&artifacts),
        artifacts,
    };
    let root = TempDir::new().unwrap();
    let manager = ModelManager::new(
        root.path(),
        LoopbackTransport::new(),
        MemoryCatalog::default(),
    );

    manager
        .download_only(&plan, &compatible_device(), || false)
        .unwrap();
    server.join().unwrap();
}

#[test]
fn shipping_transport_never_relaxes_https_for_loopback_tests() {
    let transport = ReqwestTransport::new().unwrap();
    let request = DownloadRequest {
        url: "http://127.0.0.1/artifact".to_owned(),
        range_start: None,
        if_range: None,
        redirect_hosts: vec!["127.0.0.1".to_owned()],
    };

    let error = match transport.execute(&request) {
        Ok(_) => panic!("shipping transport accepted HTTP"),
        Err(error) => error,
    };

    assert_eq!(error.code(), "model_source_rejected");
}

#[test]
fn core_runtime_startup_automatically_reconciles_model_state() {
    let parent = TempDir::new().unwrap();
    let data_dir = parent.path().join("data");
    fs::create_dir(&data_dir).unwrap();
    let catalog_path = data_dir.join("lifesub.sqlite3");
    let catalog = Catalog::open(&catalog_path).unwrap();
    let plan = ModelInstallPlan::from_manifest(model_registry().model("whisper-tiny").unwrap());
    let download_id = catalog.begin_download(&plan).unwrap();
    catalog
        .set_download_state(&download_id, "downloading", None)
        .unwrap();
    let artifact = &plan.artifacts[0];
    let part = data_dir
        .join("downloads")
        .join(&download_id)
        .join(format!("{}.part", artifact.artifact_id));
    fs::create_dir_all(part.parent().unwrap()).unwrap();
    fs::write(&part, b"uncheckpointed-tail").unwrap();
    catalog
        .save_checkpoint(
            &download_id,
            &ArtifactCheckpoint {
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
                downloaded_bytes: 4,
                expected_bytes: artifact.expected_bytes,
                temp_path: part.clone(),
                etag: Some("etag".to_owned()),
                last_modified: None,
                verified_sha256: None,
                state: "downloading".to_owned(),
            },
        )
        .unwrap();
    let stale_staging = data_dir.join("staging/stale-install");
    fs::create_dir_all(&stale_staging).unwrap();
    fs::write(stale_staging.join("partial"), b"partial").unwrap();
    drop(catalog);

    let runtime = CoreRuntime::initialize(&data_dir).unwrap();
    let (catalog, _ownership) = runtime.into_parts();

    let record = catalog
        .model_download_records()
        .unwrap()
        .into_iter()
        .find(|record| record.id == download_id)
        .unwrap();
    assert_eq!(record.state, "failed");
    assert_eq!(fs::metadata(part).unwrap().len(), 4);
    assert!(!stale_staging.exists());
}

#[test]
fn core_runtime_startup_downgrades_db_installation_with_missing_directory() {
    let parent = TempDir::new().unwrap();
    let data_dir = parent.path().join("data");
    fs::create_dir(&data_dir).unwrap();
    let catalog = Catalog::open(data_dir.join("lifesub.sqlite3")).unwrap();
    let plan = ModelInstallPlan::from_manifest(model_registry().model("whisper-base").unwrap());
    catalog
        .publish_installation(&StoredInstallation {
            model_id: plan.model_id.clone(),
            provider: plan.provider.clone(),
            manifest_version: plan.manifest_version.clone(),
            bundle_identity: plan.bundle_identity.clone(),
            install_dir: data_dir
                .join("models/asr")
                .join(&plan.provider)
                .join(&plan.model_id)
                .join(format!(
                    "{}-{}",
                    plan.manifest_version, plan.bundle_identity
                )),
            state: "runtime_qualified".to_owned(),
            runtime_identity_json: Some("{}".to_owned()),
        })
        .unwrap();
    drop(catalog);

    let runtime = CoreRuntime::initialize(&data_dir).unwrap();
    let (catalog, _ownership) = runtime.into_parts();

    assert_eq!(
        catalog.model_installation_state(&plan.model_id).unwrap(),
        (
            "installed_unqualified".to_owned(),
            Some("model_integrity_failed".to_owned())
        )
    );
}

#[test]
fn core_runtime_startup_quarantines_unrecorded_incomplete_final() {
    let parent = TempDir::new().unwrap();
    let data_dir = parent.path().join("data");
    fs::create_dir(&data_dir).unwrap();
    let plan = ModelInstallPlan::from_manifest(model_registry().model("whisper-small").unwrap());
    let final_dir = data_dir
        .join("models/asr")
        .join(&plan.provider)
        .join(&plan.model_id)
        .join(format!(
            "{}-{}",
            plan.manifest_version, plan.bundle_identity
        ));
    fs::create_dir_all(&final_dir).unwrap();
    fs::write(final_dir.join("partial"), b"partial").unwrap();

    let runtime = CoreRuntime::initialize(&data_dir).unwrap();
    let (_catalog, _ownership) = runtime.into_parts();

    assert!(!final_dir.exists());
    assert_eq!(
        fs::read_dir(data_dir.join("quarantine")).unwrap().count(),
        1
    );
}
