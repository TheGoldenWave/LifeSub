#[test]
fn changed_etag_discards_partial_and_restarts_without_range() {
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
            etag: Some("etag-old".to_owned()),
            last_modified: None,
            verified_sha256: None,
            state: "downloading".to_owned(),
        });
    let transport = ScriptedTransport::with(vec![
        Ok(DownloadResponse {
            status: 206,
            final_url: "http://127.0.0.1/config".to_owned(),
            headers: [
                ("content-length".to_owned(), "3".to_owned()),
                ("content-range".to_owned(), "bytes 3-5/6".to_owned()),
                ("etag".to_owned(), "etag-new".to_owned()),
            ]
            .into_iter()
            .collect(),
            body: Box::new(std::io::Cursor::new(b"def".to_vec())),
        }),
        Ok(DownloadResponse {
            status: 200,
            final_url: "http://127.0.0.1/config".to_owned(),
            headers: [
                ("content-length".to_owned(), "6".to_owned()),
                ("etag".to_owned(), "etag-new".to_owned()),
            ]
            .into_iter()
            .collect(),
            body: Box::new(std::io::Cursor::new(bytes.to_vec())),
        }),
    ]);
    let manager = ModelManager::new(root.path(), transport.clone(), catalog);

    manager
        .download_only(&qwen_plan(bytes, &sha), &compatible_device(), || false)
        .unwrap();

    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests[0].range_start, Some(3));
    assert_eq!(requests[1].range_start, None);
    assert_eq!(fs::read(part).unwrap(), bytes);
}

#[test]
fn changed_last_modified_discards_partial_when_etag_is_absent() {
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
            etag: None,
            last_modified: Some("old".to_owned()),
            verified_sha256: None,
            state: "downloading".to_owned(),
        });
    let transport = ScriptedTransport::with(vec![
        Ok(DownloadResponse {
            status: 206,
            final_url: "http://127.0.0.1/config".to_owned(),
            headers: [
                ("content-length".to_owned(), "3".to_owned()),
                ("content-range".to_owned(), "bytes 3-5/6".to_owned()),
                ("last-modified".to_owned(), "new".to_owned()),
            ]
            .into_iter()
            .collect(),
            body: Box::new(std::io::Cursor::new(b"def".to_vec())),
        }),
        Ok(DownloadResponse {
            status: 200,
            final_url: "http://127.0.0.1/config".to_owned(),
            headers: [
                ("content-length".to_owned(), "6".to_owned()),
                ("last-modified".to_owned(), "new".to_owned()),
            ]
            .into_iter()
            .collect(),
            body: Box::new(std::io::Cursor::new(bytes.to_vec())),
        }),
    ]);
    let manager = ModelManager::new(root.path(), transport.clone(), catalog);

    manager
        .retry_download(
            &qwen_plan(bytes, &sha),
            "download-1",
            &compatible_device(),
            || false,
        )
        .unwrap();

    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests[0].range_start, Some(3));
    assert_eq!(requests[0].if_range.as_deref(), Some("old"));
    assert_eq!(requests[1].range_start, None);
}

#[test]
fn explicit_retry_reuses_verified_artifact_only_with_matching_source_identity() {
    let bytes = b"abcdef";
    let sha = hex::encode(sha2::Sha256::digest(bytes));
    let catalog = MemoryCatalog::default();
    let root = TempDir::new().unwrap();
    let part = root.path().join("downloads/download-1/config.part");
    fs::create_dir_all(part.parent().unwrap()).unwrap();
    fs::write(&part, bytes).unwrap();
    catalog
        .checkpoints
        .lock()
        .unwrap()
        .push(ArtifactCheckpoint {
            artifact_id: "config".to_owned(),
            source_identity: config_source_identity(&sha),
            downloaded_bytes: bytes.len() as u64,
            expected_bytes: bytes.len() as u64,
            temp_path: part,
            etag: Some("etag".to_owned()),
            last_modified: None,
            verified_sha256: Some(sha.clone()),
            state: "verified".to_owned(),
        });
    let transport = ScriptedTransport::default();
    let manager = ModelManager::new(root.path(), transport.clone(), catalog);

    manager
        .retry_download(
            &qwen_plan(bytes, &sha),
            "download-1",
            &compatible_device(),
            || false,
        )
        .unwrap();

    assert!(transport.requests.lock().unwrap().is_empty());
}

#[test]
fn complete_length_with_wrong_hash_restarts_from_zero() {
    let expected = b"abcdef";
    let sha = hex::encode(sha2::Sha256::digest(expected));
    let catalog = MemoryCatalog::default();
    let root = TempDir::new().unwrap();
    let part = root.path().join("downloads/download-1/config.part");
    fs::create_dir_all(part.parent().unwrap()).unwrap();
    fs::write(&part, b"xxxxxx").unwrap();
    catalog
        .checkpoints
        .lock()
        .unwrap()
        .push(ArtifactCheckpoint {
            artifact_id: "config".to_owned(),
            source_identity: config_source_identity(&sha),
            downloaded_bytes: 6,
            expected_bytes: 6,
            temp_path: part.clone(),
            etag: Some("etag".to_owned()),
            last_modified: None,
            verified_sha256: None,
            state: "downloaded".to_owned(),
        });
    let transport = ScriptedTransport::with(vec![response("http://127.0.0.1/config", expected)]);
    let manager = ModelManager::new(root.path(), transport.clone(), catalog);

    manager
        .retry_download(
            &qwen_plan(expected, &sha),
            "download-1",
            &compatible_device(),
            || false,
        )
        .unwrap();

    assert_eq!(transport.requests.lock().unwrap()[0].range_start, None);
    assert_eq!(fs::read(part).unwrap(), expected);
}

#[test]
fn checkpoint_file_mismatch_uses_durable_db_upper_bound() {
    let bytes = b"abcdef";
    let sha = hex::encode(sha2::Sha256::digest(bytes));
    let catalog = MemoryCatalog::default();
    let root = TempDir::new().unwrap();
    let part = root.path().join("downloads/download-1/config.part");
    fs::create_dir_all(part.parent().unwrap()).unwrap();
    fs::write(&part, b"abcde").unwrap();
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
            etag: Some("etag".to_owned()),
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
            ("etag".to_owned(), "etag".to_owned()),
        ]
        .into_iter()
        .collect(),
        body: Box::new(std::io::Cursor::new(b"def".to_vec())),
    })]);
    let manager = ModelManager::new(root.path(), transport.clone(), catalog);

    manager
        .retry_download(
            &qwen_plan(bytes, &sha),
            "download-1",
            &compatible_device(),
            || false,
        )
        .unwrap();

    assert_eq!(transport.requests.lock().unwrap()[0].range_start, Some(3));
    assert_eq!(fs::read(part).unwrap(), bytes);
}

#[test]
fn incorrect_content_length_fails_without_publishing_verified_checkpoint() {
    let bytes = b"abcdef";
    let sha = hex::encode(sha2::Sha256::digest(bytes));
    let catalog = MemoryCatalog::default();
    let transport = ScriptedTransport::with(vec![Ok(DownloadResponse {
        status: 200,
        final_url: "http://127.0.0.1/config".to_owned(),
        headers: [("content-length".to_owned(), "5".to_owned())]
            .into_iter()
            .collect(),
        body: Box::new(std::io::Cursor::new(bytes.to_vec())),
    })]);
    let root = TempDir::new().unwrap();
    let manager = ModelManager::new(root.path(), transport, catalog.clone());

    let error = manager
        .download_only(&qwen_plan(bytes, &sha), &compatible_device(), || false)
        .unwrap_err();

    assert_eq!(error.code(), "model_download_failed");
    assert!(
        catalog
            .checkpoints
            .lock()
            .unwrap()
            .iter()
            .all(|checkpoint| checkpoint.state != "verified")
    );
}

#[test]
fn overlong_response_is_rejected_before_extra_bytes_are_written() {
    let expected = b"abcdef";
    let sha = hex::encode(sha2::Sha256::digest(expected));
    let catalog = MemoryCatalog::default();
    let transport = ScriptedTransport::with(vec![Ok(DownloadResponse {
        status: 200,
        final_url: "http://127.0.0.1/config".to_owned(),
        headers: [("content-length".to_owned(), expected.len().to_string())]
            .into_iter()
            .collect(),
        body: Box::new(std::io::Cursor::new(b"abcdefX".to_vec())),
    })]);
    let root = TempDir::new().unwrap();
    let manager = ModelManager::new(root.path(), transport, catalog.clone());

    let error = manager
        .download_only(&qwen_plan(expected, &sha), &compatible_device(), || false)
        .unwrap_err();

    assert_eq!(error.code(), "model_download_failed");
    assert_eq!(
        fs::metadata(root.path().join("downloads/download-1/config.part"))
            .unwrap()
            .len(),
        0
    );
    assert_eq!(catalog.checkpoints.lock().unwrap()[0].downloaded_bytes, 0);
}

#[test]
fn stalled_response_observes_cancellation_after_bounded_read_timeout() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel();
    let (accepted_tx, accepted_rx) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        let started = Instant::now();
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if shutdown_rx.try_recv().is_ok() || started.elapsed() >= Duration::from_secs(2)
                    {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("loopback accept failed: {error}"),
            }
        };
        accepted_tx.send(()).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let mut request = [0_u8; 2048];
        let _ = stream.read(&mut request).unwrap();
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: 4\r\nETag: stalled\r\nConnection: close\r\n\r\n"
        )
        .unwrap();
        let _ = shutdown_rx.recv_timeout(Duration::from_secs(2));
    });
    let body = b"data";
    let sha = hex::encode(sha2::Sha256::digest(body));
    let mut plan = qwen_plan(body, &sha);
    plan.artifacts[0].url = format!("http://{address}/artifact");
    let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let cancellation = cancelled.clone();
    std::thread::spawn(move || {
        if accepted_rx.recv_timeout(Duration::from_secs(1)).is_ok() {
            std::thread::sleep(Duration::from_millis(50));
            cancellation.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    });
    let root = TempDir::new().unwrap();
    let manager = ModelManager::new(
        root.path(),
        LoopbackTransport::with_read_timeout(Duration::from_millis(100)),
        MemoryCatalog::default(),
    );
    let started = Instant::now();

    let result = manager
        .download_only(&plan, &compatible_device(), || {
            cancelled.load(std::sync::atomic::Ordering::SeqCst)
        });
    let elapsed = started.elapsed();
    let _ = shutdown_tx.send(());
    server.join().unwrap();
    let error = result.unwrap_err();

    assert_eq!(error.code(), "model_download_cancelled");
    assert!(elapsed < Duration::from_secs(2));
}

#[test]
fn final_redirect_host_is_checked_against_current_artifact() {
    let bytes = b"abcdef";
    let sha = hex::encode(sha2::Sha256::digest(bytes));
    let transport = ScriptedTransport::with(vec![Ok(DownloadResponse {
        status: 200,
        final_url: "https://disallowed.example/config".to_owned(),
        headers: [("content-length".to_owned(), "6".to_owned())]
            .into_iter()
            .collect(),
        body: Box::new(std::io::Cursor::new(bytes.to_vec())),
    })]);
    let root = TempDir::new().unwrap();
    let manager = ModelManager::new(root.path(), transport, MemoryCatalog::default());

    let error = manager
        .download_only(&qwen_plan(bytes, &sha), &compatible_device(), || false)
        .unwrap_err();

    assert_eq!(error.code(), "model_source_rejected");
}

#[test]
fn corrupt_artifact_is_never_marked_verified() {
    let expected = b"abcdef";
    let sha = hex::encode(sha2::Sha256::digest(expected));
    let catalog = MemoryCatalog::default();
    let transport = ScriptedTransport::with(vec![Ok(DownloadResponse {
        status: 200,
        final_url: "http://127.0.0.1/config".to_owned(),
        headers: [("content-length".to_owned(), "6".to_owned())]
            .into_iter()
            .collect(),
        body: Box::new(std::io::Cursor::new(b"xxxxxx".to_vec())),
    })]);
    let root = TempDir::new().unwrap();
    let manager = ModelManager::new(root.path(), transport, catalog.clone());

    let error = manager
        .download_only(&qwen_plan(expected, &sha), &compatible_device(), || false)
        .unwrap_err();

    assert_eq!(error.code(), "model_integrity_failed");
    assert!(
        catalog
            .checkpoints
            .lock()
            .unwrap()
            .iter()
            .all(|checkpoint| checkpoint.state != "verified")
    );
}

#[test]
fn tamper_between_verify_and_install_is_rejected_before_staging() {
    let body = b"model";
    let artifact = direct_artifact("model", "model.onnx", body);
    let plan = ModelInstallPlan {
        model_id: "tamper-model".to_owned(),
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
    let catalog = MemoryCatalog::default();
    let manager = ModelManager::new(
        root.path(),
        ScriptedTransport::with(vec![response(&artifact.url, body)]),
        catalog.clone(),
    )
    .with_sherpa_runtime_identity(sherpa_identity());
    let download_id = manager
        .download_only(&plan, &compatible_device(), || false)
        .unwrap();
    fs::write(
        root.path().join("downloads/download-1/model.part"),
        b"xxxxx",
    )
    .unwrap();

    let error = manager
        .install_verified_download(&plan, &download_id)
        .unwrap_err();

    assert_eq!(error.code(), "model_integrity_failed");
    assert_eq!(
        fs::read_dir(root.path().join("staging")).unwrap().count(),
        0
    );
    assert!(catalog.installations.lock().unwrap().is_empty());
}
