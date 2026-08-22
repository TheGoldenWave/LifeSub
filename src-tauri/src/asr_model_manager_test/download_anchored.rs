fn anchored_download_manager(
    root: &std::path::Path,
    transport: ScriptedTransport,
    catalog: MemoryCatalog,
) -> ModelManager<ScriptedTransport, MemoryCatalog> {
    ModelManager::new_anchored(root, std::fs::File::open(root).unwrap(), transport, catalog)
}

#[test]
fn production_download_stays_on_held_root_when_nominal_data_root_is_swapped() {
    let bytes = b"abcdef";
    let sha = hex::encode(sha2::Sha256::digest(bytes));
    let parent = TempDir::new().unwrap();
    let root = parent.path().join("data");
    let held = parent.path().join("held");
    fs::create_dir(&root).unwrap();
    let transport = ScriptedTransport::with(vec![response("http://127.0.0.1/config", bytes)]);
    let manager = anchored_download_manager(&root, transport, MemoryCatalog::default());
    let swapped = std::cell::Cell::new(false);

    manager
        .download_only(&qwen_plan(bytes, &sha), &compatible_device(), || {
            if !swapped.replace(true) {
                fs::rename(&root, &held).unwrap();
                fs::create_dir(&root).unwrap();
            }
            false
        })
        .unwrap();

    assert_eq!(
        fs::read(held.join("downloads/download-1/config.part")).unwrap(),
        bytes
    );
    assert_eq!(fs::read_dir(root).unwrap().count(), 0);
}

#[test]
fn production_download_rejects_symlink_part_without_touching_target() {
    let bytes = b"abcdef";
    let sha = hex::encode(sha2::Sha256::digest(bytes));
    let root = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    fs::create_dir_all(root.path().join("downloads/download-1")).unwrap();
    let target = outside.path().join("target");
    fs::write(&target, b"outside").unwrap();
    std::os::unix::fs::symlink(
        &target,
        root.path().join("downloads/download-1/config.part"),
    )
    .unwrap();
    let transport = ScriptedTransport::with(vec![response("http://127.0.0.1/config", bytes)]);
    let manager = anchored_download_manager(root.path(), transport, MemoryCatalog::default());

    let error = manager
        .download_only(&qwen_plan(bytes, &sha), &compatible_device(), || false)
        .unwrap_err();

    assert_eq!(error.code(), "model_structural_incompatible");
    assert_eq!(fs::read(target).unwrap(), b"outside");
}

#[test]
fn production_download_rejects_fifo_part_without_blocking() {
    let bytes = b"abcdef";
    let sha = hex::encode(sha2::Sha256::digest(bytes));
    let root = TempDir::new().unwrap();
    fs::create_dir_all(root.path().join("downloads/download-1")).unwrap();
    let fifo = root.path().join("downloads/download-1/config.part");
    let fifo_name = CString::new(fifo.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(fifo_name.as_ptr(), 0o600) }, 0);
    let transport = ScriptedTransport::with(vec![response("http://127.0.0.1/config", bytes)]);
    let manager = anchored_download_manager(root.path(), transport, MemoryCatalog::default());

    let error = manager
        .download_only(&qwen_plan(bytes, &sha), &compatible_device(), || false)
        .unwrap_err();

    assert_eq!(error.code(), "model_structural_incompatible");
}

#[test]
fn production_download_fails_closed_when_part_identity_changes_before_checkpoint() {
    let bytes = vec![b'x'; (4 * 1024 * 1024) + 1];
    let sha = hex::encode(sha2::Sha256::digest(&bytes));
    let root = TempDir::new().unwrap();
    let part = root.path().join("downloads/download-1/config.part");
    let replacement = root.path().join("downloads/download-1/replacement");
    let transport = ScriptedTransport::with(vec![response("http://127.0.0.1/config", &bytes)]);
    let manager = anchored_download_manager(root.path(), transport, MemoryCatalog::default());
    let calls = std::cell::Cell::new(0usize);

    let error = manager
        .download_only(&qwen_plan(&bytes, &sha), &compatible_device(), || {
            let call = calls.get() + 1;
            calls.set(call);
            if call == 3 {
                fs::rename(&part, &replacement).unwrap();
                fs::write(&part, b"replacement").unwrap();
            }
            false
        })
        .unwrap_err();

    assert_eq!(error.code(), "model_integrity_failed");
    assert_eq!(fs::read(part).unwrap(), b"replacement");
}

#[test]
fn production_download_fails_before_checkpoint_when_runtime_ownership_is_lost() {
    let bytes = vec![b'x'; (4 * 1024 * 1024) + 1];
    let sha = hex::encode(sha2::Sha256::digest(&bytes));
    let root = TempDir::new().unwrap();
    let transport = ScriptedTransport::with(vec![response("http://127.0.0.1/config", &bytes)]);
    let current = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let ownership_current = current.clone();
    let ownership =
        crate::asr::model_manager::ModelRuntimeOwnership::new(current.clone(), move || {
            ownership_current
                .load(std::sync::atomic::Ordering::SeqCst)
                .then_some(())
                .ok_or_else(|| ManagerError::ownership_lost("test ownership lost"))
        });
    let manager = ModelManager::new_anchored_owned(
        root.path(),
        std::fs::File::open(root.path()).unwrap(),
        transport,
        MemoryCatalog::default(),
        ownership,
    );
    let calls = std::cell::Cell::new(0usize);

    let error = manager
        .download_only(&qwen_plan(&bytes, &sha), &compatible_device(), || {
            let call = calls.get() + 1;
            calls.set(call);
            if call == 3 {
                current.store(false, std::sync::atomic::Ordering::SeqCst);
            }
            false
        })
        .unwrap_err();

    assert_eq!(error.code(), "runtime_ownership_lost");
}

#[test]
fn production_download_preflight_reads_checkpoint_from_held_root_after_swap() {
    let bytes = b"abcdef";
    let sha = hex::encode(sha2::Sha256::digest(bytes));
    let plan = qwen_plan(bytes, &sha);
    let parent = TempDir::new().unwrap();
    let root = parent.path().join("data");
    let held = parent.path().join("held");
    let nominal_part = root.join("downloads/download-1/config.part");
    fs::create_dir_all(nominal_part.parent().unwrap()).unwrap();
    fs::write(&nominal_part, b"abc").unwrap();
    let catalog = MemoryCatalog::default();
    catalog
        .checkpoints
        .lock()
        .unwrap()
        .push(ArtifactCheckpoint {
            artifact_id: "config".to_owned(),
            source_identity: config_source_identity(&sha),
            downloaded_bytes: 3,
            expected_bytes: 6,
            temp_path: nominal_part.clone(),
            etag: Some("etag".to_owned()),
            last_modified: None,
            verified_sha256: None,
            state: "downloading".to_owned(),
        });
    let manager = anchored_download_manager(&root, ScriptedTransport::default(), catalog);
    fs::rename(&root, &held).unwrap();
    fs::create_dir_all(nominal_part.parent().unwrap()).unwrap();
    fs::write(&nominal_part, b"replacement-bytes").unwrap();

    let required = manager
        .required_additional_free_for_test(&plan, Some("download-1"))
        .unwrap();

    assert_eq!(required, 536_870_921);
}

#[test]
fn production_download_preflight_creates_only_held_assembly_roots_after_swap() {
    let bytes = b"abcdef";
    let sha = hex::encode(sha2::Sha256::digest(bytes));
    let parent = TempDir::new().unwrap();
    let root = parent.path().join("data");
    let held = parent.path().join("held");
    fs::create_dir(&root).unwrap();
    let manager = anchored_download_manager(
        &root,
        ScriptedTransport::default(),
        MemoryCatalog::default(),
    );
    fs::rename(&root, &held).unwrap();
    fs::create_dir(&root).unwrap();
    fs::write(root.join("sentinel"), b"replacement").unwrap();

    manager
        .required_additional_free_for_test(&qwen_plan(bytes, &sha), None)
        .unwrap();

    assert!(held.join("staging").is_dir());
    assert!(held.join("models/asr").is_dir());
    assert_eq!(fs::read(root.join("sentinel")).unwrap(), b"replacement");
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
}
