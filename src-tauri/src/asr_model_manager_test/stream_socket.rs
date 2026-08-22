#[test]
fn mid_body_disconnect_persists_actual_checkpoint_and_retry_uses_range() {
    const TOTAL_BYTES: usize = 6 * 1024 * 1024;
    const FIRST_BYTES: usize = 5 * 1024 * 1024;

    let bytes = vec![b'x'; TOTAL_BYTES];
    let sha = hex::encode(sha2::Sha256::digest(&bytes));
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (request_tx, request_rx) = std::sync::mpsc::channel();
    let server_bytes = bytes.clone();
    let server = std::thread::spawn(move || {
        let (mut first, _) = listener.accept().unwrap();
        let mut request = vec![0_u8; 4096];
        let read = first.read(&mut request).unwrap();
        request_tx
            .send(String::from_utf8_lossy(&request[..read]).into_owned())
            .unwrap();
        write!(
            first,
            "HTTP/1.1 200 OK\r\nContent-Length: {TOTAL_BYTES}\r\nETag: checkpoint-etag\r\nConnection: close\r\n\r\n"
        )
        .unwrap();
        first.write_all(&server_bytes[..FIRST_BYTES]).unwrap();
        drop(first);

        let (mut second, _) = listener.accept().unwrap();
        let mut request = vec![0_u8; 4096];
        let read = second.read(&mut request).unwrap();
        let request = String::from_utf8_lossy(&request[..read]).into_owned();
        let range_start = request
            .lines()
            .find_map(|line| {
                line.to_ascii_lowercase()
                    .strip_prefix("range: bytes=")
                    .and_then(|value| value.strip_suffix('-'))
                    .map(str::to_owned)
            })
            .unwrap()
            .parse::<usize>()
            .unwrap();
        request_tx.send(request).unwrap();
        write!(
            second,
            "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {range_start}-{}/{TOTAL_BYTES}\r\nETag: checkpoint-etag\r\nConnection: close\r\n\r\n",
            TOTAL_BYTES - range_start,
            TOTAL_BYTES - 1,
        )
        .unwrap();
        second.write_all(&server_bytes[range_start..]).unwrap();
    });
    let mut plan = qwen_plan(&bytes, &sha);
    plan.artifacts[0].url = format!("http://{address}/artifact");
    let root = TempDir::new().unwrap();
    let catalog = MemoryCatalog::default();
    let manager = ModelManager::new(root.path(), LoopbackTransport::new(), catalog.clone());

    let first_error = manager
        .download_only(&plan, &compatible_device(), || false)
        .unwrap_err();
    assert_eq!(first_error.code(), "model_download_failed");
    let persisted_bytes = catalog.checkpoints.lock().unwrap()[0].downloaded_bytes;
    assert!(persisted_bytes > 0);
    assert!(persisted_bytes <= FIRST_BYTES as u64);
    manager
        .retry_download(&plan, "download-1", &compatible_device(), || false)
        .unwrap();
    server.join().unwrap();

    let first_request = request_rx.recv().unwrap();
    let second_request = request_rx.recv().unwrap();
    assert!(!first_request.to_ascii_lowercase().contains("range:"));
    let second_request = second_request.to_ascii_lowercase();
    assert!(second_request.contains(&format!("range: bytes={persisted_bytes}-")));
    assert!(second_request.contains("if-range: checkpoint-etag"));
    let checkpoint = catalog.checkpoints.lock().unwrap()[0].clone();
    assert_eq!(checkpoint.downloaded_bytes, TOTAL_BYTES as u64);
    assert_eq!(checkpoint.state, "verified");
}
