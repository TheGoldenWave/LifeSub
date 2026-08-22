#[test]
fn unsafe_required_path_rejects_before_db_or_network() {
    let transport = ScriptedTransport::default();
    let catalog = MemoryCatalog::default();
    let root = TempDir::new().unwrap();
    let manager = ModelManager::new(root.path(), transport.clone(), catalog.clone());
    let mut plan = qwen_plan(b"{}", &"0".repeat(64));
    plan.artifacts[0].required_path = "../escape".to_owned();

    let error = manager
        .download_only(&plan, &compatible_device(), || false)
        .unwrap_err();

    assert_eq!(error.code(), "model_structural_incompatible");
    assert_eq!(*catalog.begins.lock().unwrap(), 0);
    assert!(transport.requests.lock().unwrap().is_empty());
}

#[test]
fn unsafe_path_components_reject_before_db_network_or_filesystem_escape() {
    let root = TempDir::new().unwrap();
    let outside = root.path().parent().unwrap().join("lifesub-path-escape");
    let transport = ScriptedTransport::default();
    let catalog = MemoryCatalog::default();
    let manager = ModelManager::new(root.path(), transport.clone(), catalog.clone());
    let base = simple_sherpa_plan("safe-model", b"model");
    let mut plans = Vec::new();
    for field in ["model", "provider", "version", "bundle", "artifact"] {
        let mut plan = base.clone();
        match field {
            "model" => plan.model_id = "../../../outside".to_owned(),
            "provider" => plan.provider = "../../../outside".to_owned(),
            "version" => plan.manifest_version = "../../../outside".to_owned(),
            "bundle" => plan.bundle_identity = "../../../outside".to_owned(),
            "artifact" => plan.artifacts[0].artifact_id = "../../../outside".to_owned(),
            _ => unreachable!(),
        }
        plans.push(plan);
    }

    for plan in plans {
        let error = manager
            .download_only(&plan, &compatible_device(), || false)
            .unwrap_err();
        assert_eq!(error.code(), "model_structural_incompatible");
    }
    let error = manager
        .retry_download(&base, "../../../outside", &compatible_device(), || false)
        .unwrap_err();
    assert_eq!(error.code(), "model_structural_incompatible");
    let error = manager
        .download_model("../../../outside", &compatible_device(), || false)
        .unwrap_err();
    assert_eq!(error.code(), "model_structural_incompatible");
    assert_eq!(*catalog.begins.lock().unwrap(), 0);
    assert!(transport.requests.lock().unwrap().is_empty());
    assert!(!outside.exists());
}

#[test]
fn canonical_manifest_install_constraints_reject_drift_before_db_or_network() {
    let transport = ScriptedTransport::default();
    let catalog = MemoryCatalog::default();
    let root = TempDir::new().unwrap();
    let manager = ModelManager::new(root.path(), transport.clone(), catalog.clone());
    let mut plan = ModelInstallPlan::from_manifest(model_registry().model("whisper-tiny").unwrap());
    let InstallContract::Archive {
        max_scanned_entries,
        ..
    } = &mut plan.install_contract
    else {
        panic!("whisper-tiny must use archive install constraints");
    };
    *max_scanned_entries += 1;

    let error = manager
        .download_only(&plan, &compatible_device(), || false)
        .unwrap_err();

    assert_eq!(error.code(), "model_structural_incompatible");
    assert_eq!(*catalog.begins.lock().unwrap(), 0);
    assert!(transport.requests.lock().unwrap().is_empty());
}

#[test]
fn cancellation_stops_before_transport_request() {
    let transport = ScriptedTransport::default();
    let root = TempDir::new().unwrap();
    let manager = ModelManager::new(root.path(), transport.clone(), MemoryCatalog::default());

    let error = manager
        .download_only(
            &qwen_plan(b"{}", &"0".repeat(64)),
            &compatible_device(),
            || true,
        )
        .unwrap_err();

    assert_eq!(error.code(), "model_download_cancelled");
    assert!(transport.requests.lock().unwrap().is_empty());
}

#[test]
fn mid_stream_cancellation_persists_actual_bytes_and_validator() {
    let bytes = vec![b'x'; 5 * 1024 * 1024];
    let sha = hex::encode(sha2::Sha256::digest(&bytes));
    let catalog = MemoryCatalog::default();
    let transport = ScriptedTransport::with(vec![Ok(DownloadResponse {
        status: 200,
        final_url: "http://127.0.0.1/config".to_owned(),
        headers: [
            ("content-length".to_owned(), bytes.len().to_string()),
            ("etag".to_owned(), "cancel-etag".to_owned()),
        ]
        .into_iter()
        .collect(),
        body: Box::new(std::io::Cursor::new(bytes.clone())),
    })]);
    let root = TempDir::new().unwrap();
    let manager = ModelManager::new(root.path(), transport, catalog.clone());
    let polls = std::sync::atomic::AtomicUsize::new(0);

    let error = manager
        .download_only(&qwen_plan(&bytes, &sha), &compatible_device(), || {
            polls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) > 70
        })
        .unwrap_err();

    assert_eq!(error.code(), "model_download_cancelled");
    let checkpoint = catalog.checkpoints.lock().unwrap()[0].clone();
    assert!(checkpoint.downloaded_bytes > 0);
    assert!(checkpoint.downloaded_bytes < bytes.len() as u64);
    assert_eq!(checkpoint.etag.as_deref(), Some("cancel-etag"));
    assert_eq!(checkpoint.state, "cancelled");
}
