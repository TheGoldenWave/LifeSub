//! Tests for ASR model manager: downloads, safe extraction, versioned activation,
//! and startup reconciliation.
//!
//! Covers the contract from the V0.2 design spec §9 (模型安装事务).

use std::fs;
use std::io::{Cursor, Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use tempfile::TempDir;

use crate::asr::model_manager::{
    ModelManager, ModelManagerError,
};
use crate::catalog::Catalog;

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

/// Create a valid tar.bz2 archive with the given entries.
/// Each entry is (relative_path, content_bytes).
fn build_tar_bz2(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut tar_buf = Vec::new();
    {
        let mut ar = tar::Builder::new(&mut tar_buf);
        for (path, content) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_path(path).unwrap();
            header.set_size(content.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            ar.append(&header, Cursor::new(content)).unwrap();
        }
        ar.finish().unwrap();
    }
    // Compress with bzip2
    let mut compressed = Vec::new();
    {
        let mut writer = bzip2::write::BzEncoder::new(&mut compressed, bzip2::Compression::default());
        writer.write_all(&tar_buf).unwrap();
        writer.finish().unwrap();
    }
    compressed
}

/// SHA-256 hex digest of bytes.
fn sha256_hex(data: &[u8]) -> String {
    use sha2::Digest;
    hex::encode(sha2::Sha256::digest(data))
}

/// Create a test Catalog backed by an in-memory database.
fn test_catalog() -> Arc<Catalog> {
    Arc::new(Catalog::in_memory().expect("create in-memory catalog"))
}

/// Create a model manager for testing with a temp directory.
fn test_manager() -> (ModelManager, Arc<Catalog>, TempDir) {
    let temp = TempDir::new().expect("create temp dir");
    let catalog = test_catalog();
    let mgr = ModelManager::new(Arc::clone(&catalog), temp.path());
    fs::create_dir_all(mgr.downloads_dir()).expect("create downloads dir");
    fs::create_dir_all(mgr.staging_dir()).expect("create staging dir");
    fs::create_dir_all(mgr.models_dir()).expect("create models dir");
    (mgr, catalog, temp)
}

/// A tiny HTTP fixture server that serves a fixed response.
///
/// Provide `content` bytes and an optional `Content-Length` override.
/// Set `close_early` to true to simulate an interrupted download.
struct HttpFixture {
    addr: String,
    content: Vec<u8>,
    content_length: Option<u64>,
    close_early: bool,
    stop: Arc<AtomicBool>,
}

impl HttpFixture {
    fn serve(content: Vec<u8>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = format!("http://{}", listener.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let stop_clone = Arc::clone(&stop);
        let content_clone = content.clone();
        let content_len = content.len() as u64;

        thread::spawn(move || {
            listener
                .set_nonblocking(true)
                .expect("set nonblocking");
            while !stop_clone.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream.set_read_timeout(Some(Duration::from_millis(100)));
                        let mut buf = [0u8; 4096];
                        let _ = stream.read(&mut buf);
                        // Send HTTP response
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            content_len
                        );
                        let _ = stream.write_all(response.as_bytes());
                        let _ = stream.write_all(&content_clone);
                        let _ = stream.flush();
                        break; // serve one request then stop
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });

        HttpFixture {
            addr,
            content,
            content_length: None,
            close_early: false,
            stop,
        }
    }

    /// Serve with a Content-Length header that does not match the actual body.
    fn serve_with_wrong_length(content: Vec<u8>, claimed_length: u64) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = format!("http://{}", listener.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let stop_clone = Arc::clone(&stop);
        let content_clone = content.clone();

        thread::spawn(move || {
            listener
                .set_nonblocking(true)
                .expect("set nonblocking");
            while !stop_clone.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream.set_read_timeout(Some(Duration::from_millis(100)));
                        let mut buf = [0u8; 4096];
                        let _ = stream.read(&mut buf);
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            claimed_length
                        );
                        let _ = stream.write_all(response.as_bytes());
                        let _ = stream.write_all(&content_clone);
                        let _ = stream.flush();
                        break;
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });

        HttpFixture {
            addr,
            content,
            content_length: Some(claimed_length),
            close_early: false,
            stop,
        }
    }

    /// Serve with a redirect (302) to the given URL.
    fn serve_redirect(redirect_to: &str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = format!("http://{}", listener.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let stop_clone = Arc::clone(&stop);
        let redirect = redirect_to.to_string();

        thread::spawn(move || {
            listener
                .set_nonblocking(true)
                .expect("set nonblocking");
            while !stop_clone.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream.set_read_timeout(Some(Duration::from_millis(100)));
                        let mut buf = [0u8; 4096];
                        let _ = stream.read(&mut buf);
                        let response = format!(
                            "HTTP/1.1 302 Found\r\nLocation: {}\r\nConnection: close\r\n\r\n",
                            redirect
                        );
                        let _ = stream.write_all(response.as_bytes());
                        let _ = stream.flush();
                        break;
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });

        HttpFixture {
            addr,
            content: Vec::new(),
            content_length: None,
            close_early: false,
            stop,
        }
    }

    /// Serve with the connection deliberately closed early (interrupted).
    fn serve_interrupted(content: Vec<u8>, send_bytes: usize) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = format!("http://{}", listener.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let stop_clone = Arc::clone(&stop);
        let content_len = content.len() as u64;
        let content_for_thread = content.clone();

        thread::spawn(move || {
            listener
                .set_nonblocking(true)
                .expect("set nonblocking");
            while !stop_clone.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream.set_read_timeout(Some(Duration::from_millis(100)));
                        let mut buf = [0u8; 4096];
                        let _ = stream.read(&mut buf);
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            content_len
                        );
                        let _ = stream.write_all(response.as_bytes());
                        // Only send partial content
                        let _ = stream.write_all(&content_for_thread[..send_bytes.min(content_for_thread.len())]);
                        let _ = stream.flush();
                        // Close the connection immediately
                        drop(stream);
                        break;
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });

        HttpFixture {
            addr,
            content,
            content_length: None,
            close_early: true,
            stop,
        }
    }

    fn url(&self) -> String {
        self.addr.clone()
    }
}

impl Drop for HttpFixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

// ---------------------------------------------------------------------------
// 1. Interrupted download
// ---------------------------------------------------------------------------

#[test]
fn download_interrupted_mid_stream() {
    let (mgr, catalog, _temp) = test_manager();
    let archive = build_tar_bz2(&[("model.onnx", b"fake model data")]);
    let archive_hash = sha256_hex(&archive);

    let fixture = HttpFixture::serve_interrupted(archive.clone(), 100);

    let download_id = mgr
        .enqueue_download(
            "test-model",
            "v1",
            &archive_hash,
            &fixture.url(),
            archive.len() as u64,
        )
        .expect("enqueue");

    let result = mgr.download(&download_id);
    assert!(result.is_err(), "interrupted download should fail");

    // Verify the download is marked as failed
    let download = catalog
        .get_model_download(&download_id)
        .expect("get download")
        .expect("download exists");
    assert_eq!(
        download.state,
        "failed",
        "download should be in failed state"
    );
}

// ---------------------------------------------------------------------------
// 2. Wrong content length
// ---------------------------------------------------------------------------

#[test]
fn wrong_content_length_header() {
    let (mgr, _catalog, _temp) = test_manager();
    let archive = build_tar_bz2(&[("model.onnx", b"fake model data")]);
    let archive_hash = sha256_hex(&archive);

    // Claim the content is much larger than reality
    let fixture =
        HttpFixture::serve_with_wrong_length(archive.clone(), archive.len() as u64 + 1000);

    let download_id = mgr
        .enqueue_download(
            "test-model",
            "v1",
            &archive_hash,
            &fixture.url(),
            archive.len() as u64 + 1000,
        )
        .expect("enqueue");

    let result = mgr.download(&download_id);
    // Should detect the discrepancy (either Content-Length mismatch or premature EOF)
    assert!(result.is_err(), "wrong content length should cause failure");
}

// ---------------------------------------------------------------------------
// 3. Disallowed redirect
// ---------------------------------------------------------------------------

#[test]
fn disallowed_redirect_host() {
    let (mgr, _catalog, _temp) = test_manager();
    let archive = build_tar_bz2(&[("model.onnx", b"fake model data")]);
    let archive_hash = sha256_hex(&archive);

    // Redirect to a host that is not in the allowlist
    let fixture = HttpFixture::serve_redirect("http://evil.example.com/model.tar.bz2");

    let download_id = mgr
        .enqueue_download(
            "test-model",
            "v1",
            &archive_hash,
            &fixture.url(),
            archive.len() as u64,
        )
        .expect("enqueue");

    let result = mgr.download(&download_id);
    assert!(
        result.is_err(),
        "redirect to disallowed host should fail"
    );
}

// ---------------------------------------------------------------------------
// 4. Wrong SHA-256
// ---------------------------------------------------------------------------

#[test]
fn archive_sha256_mismatch() {
    let (mgr, _catalog, _temp) = test_manager();
    let archive = build_tar_bz2(&[("model.onnx", b"fake model data")]);
    let real_hash = sha256_hex(&archive);

    let fixture = HttpFixture::serve(archive.clone());

    // Enqueue with a deliberately wrong hash
    let wrong_hash = "0".repeat(64);
    let download_id = mgr
        .enqueue_download(
            "test-model",
            "v1",
            &wrong_hash,
            &fixture.url(),
            archive.len() as u64,
        )
        .expect("enqueue");

    let result = mgr.download(&download_id);
    assert!(
        result.is_err(),
        "wrong SHA-256 should fail verification"
    );
    // But the download itself should succeed — only verification fails
    // The download row should be failed with model_integrity_failed
}

// ---------------------------------------------------------------------------
// 5. Path traversal in archive
// ---------------------------------------------------------------------------

#[test]
fn rejects_absolute_path_in_archive() {
    let (mgr, _catalog, _temp) = test_manager();
    let archive = build_tar_bz2(&[("/etc/passwd", b"evil"), ("model.onnx", b"ok")]);
    let archive_hash = sha256_hex(&archive);

    let result = mgr.extract_and_verify(
        &archive,
        &archive_hash,
        "test-provider",
        "test-model",
        "v1",
    );
    assert!(
        result.is_err(),
        "absolute path in archive should be rejected"
    );
}

#[test]
fn rejects_parent_dir_traversal_in_archive() {
    let (mgr, _catalog, _temp) = test_manager();
    let archive = build_tar_bz2(&[("../escape.txt", b"evil"), ("model.onnx", b"ok")]);
    let archive_hash = sha256_hex(&archive);

    let result = mgr.extract_and_verify(
        &archive,
        &archive_hash,
        "test-provider",
        "test-model",
        "v1",
    );
    assert!(
        result.is_err(),
        ".. path traversal in archive should be rejected"
    );
}

// ---------------------------------------------------------------------------
// 6. Symlink/hardlink rejection
// ---------------------------------------------------------------------------

#[test]
fn rejects_symlink_in_archive() {
    let (mgr, _catalog, _temp) = test_manager();

    // Build a tar with a symlink entry
    let mut tar_buf = Vec::new();
    {
        let mut ar = tar::Builder::new(&mut tar_buf);
        // Add a symlink
        let mut header = tar::Header::new_gnu();
        header.set_path("link_to_etc").unwrap();
        header.set_entry_type(tar::EntryType::Symlink);
        header.set_link_name("/etc/passwd").unwrap();
        header.set_size(0);
        header.set_cksum();
        ar.append(&header, Cursor::new(&[] as &[u8])).unwrap();

        // Add a valid file
        let mut header2 = tar::Header::new_gnu();
        header2.set_path("model.onnx").unwrap();
        header2.set_size(3);
        header2.set_mode(0o644);
        header2.set_cksum();
        ar.append(&header2, Cursor::new(b"ok")).unwrap();

        ar.finish().unwrap();
    }
    let mut compressed = Vec::new();
    {
        let mut writer =
            bzip2::write::BzEncoder::new(&mut compressed, bzip2::Compression::Default);
        writer.write_all(&tar_buf).unwrap();
        writer.finish().unwrap();
    }
    let archive_hash = sha256_hex(&compressed);

    let result = mgr.extract_and_verify(
        &compressed,
        &archive_hash,
        "test-provider",
        "test-model",
        "v1",
    );
    assert!(
        result.is_err(),
        "symlink in archive should be rejected"
    );
}

#[test]
fn rejects_hardlink_in_archive() {
    let (mgr, _catalog, _temp) = test_manager();

    // Build a tar with a hardlink entry
    let mut tar_buf = Vec::new();
    {
        let mut ar = tar::Builder::new(&mut tar_buf);

        // Add a regular file first
        let mut header = tar::Header::new_gnu();
        header.set_path("original.txt").unwrap();
        header.set_size(4);
        header.set_mode(0o644);
        header.set_cksum();
        ar.append(&header, Cursor::new(b"data")).unwrap();

        // Add a hardlink to it
        let mut header2 = tar::Header::new_gnu();
        header2.set_path("link.txt").unwrap();
        header2.set_entry_type(tar::EntryType::Link);
        header2.set_link_name("original.txt").unwrap();
        header2.set_size(0);
        header2.set_cksum();
        ar.append(&header2, Cursor::new(&[] as &[u8])).unwrap();

        ar.finish().unwrap();
    }
    let mut compressed = Vec::new();
    {
        let mut writer =
            bzip2::write::BzEncoder::new(&mut compressed, bzip2::Compression::Default);
        writer.write_all(&tar_buf).unwrap();
        writer.finish().unwrap();
    }
    let archive_hash = sha256_hex(&compressed);

    let result = mgr.extract_and_verify(
        &compressed,
        &archive_hash,
        "test-provider",
        "test-model",
        "v1",
    );
    assert!(
        result.is_err(),
        "hardlink in archive should be rejected"
    );
}

// ---------------------------------------------------------------------------
// 7. Expanded-size limit
// ---------------------------------------------------------------------------

#[test]
fn expanded_size_exceeds_limit() {
    let (mgr, _catalog, _temp) = test_manager();

    // Create 200 entries (exceeds MAX_FILE_COUNT = 100)
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    for i in 0..150 {
        entries.push((format!("file_{}.txt", i), vec![0u8; 100]));
    }
    let entry_refs: Vec<(&str, &[u8])> = entries
        .iter()
        .map(|(name, data)| (name.as_str(), data.as_slice()))
        .collect();
    let archive = build_tar_bz2(&entry_refs);
    let archive_hash = sha256_hex(&archive);

    let result = mgr.extract_and_verify(
        &archive,
        &archive_hash,
        "test-provider",
        "test-model",
        "v1",
    );
    assert!(
        result.is_err(),
        "too many files should be rejected"
    );
}

// ---------------------------------------------------------------------------
// 8. Rename-before-DB crash (reconciliation)
// ---------------------------------------------------------------------------

#[test]
fn reconcile_orphan_install_dir() {
    let (mgr, catalog, temp) = test_manager();

    // Simulate an install directory that exists on disk but is not in the DB
    let provider_dir = temp.path().join("models/asr/test-provider/test-model");
    fs::create_dir_all(&provider_dir).expect("create provider dir");

    let orphan_dir = provider_dir.join("v1-aaaabbbbccccddddeeeeffff0000111122223333");
    fs::create_dir_all(&orphan_dir).expect("create orphan dir");
    // Write the immutable marker
    fs::write(
        orphan_dir.join(".lifesub-model-install"),
        "test-provider\ntest-model\nv1\naaaabbbbccccddddeeeeffff0000111122223333\n",
    )
    .expect("write marker");

    // Run reconciliation
    mgr.reconcile().expect("reconcile");

    // The orphan should be recorded as a corrupt installation
    let install = catalog
        .get_model_installation("test-model")
        .expect("get installation")
        .expect("installation exists");
    assert_eq!(
        install.state,
        "corrupt",
        "orphan install should be marked corrupt"
    );
}

// ---------------------------------------------------------------------------
// 9. DB-before-directory mismatch (reconciliation)
// ---------------------------------------------------------------------------

#[test]
fn reconcile_missing_active_dir() {
    let (mgr, catalog, _temp) = test_manager();

    // Insert an installation record pointing to a non-existent directory
    let now = chrono::Utc::now().to_rfc3339();
    catalog
        .upsert_model_installation(
            "test-model",
            "test-provider",
            "v1",
            "aaaabbbbccccddddeeeeffff0000111122223333",
            "/nonexistent/path/models/asr/test-provider/test-model/v1-hash",
            "ready",
            &now,
            None,
        )
        .expect("insert installation");

    // Run reconciliation
    mgr.reconcile().expect("reconcile");

    // The installation should be marked corrupt
    let install = catalog
        .get_model_installation("test-model")
        .expect("get installation")
        .expect("installation exists");
    assert_eq!(
        install.state,
        "corrupt",
        "missing dir should be marked corrupt"
    );
}

// ---------------------------------------------------------------------------
// 10. Cancellation
// ---------------------------------------------------------------------------

#[test]
fn cancel_download() {
    let (mgr, catalog, _temp) = test_manager();
    let archive = build_tar_bz2(&[("model.onnx", b"fake model data")]);
    let archive_hash = sha256_hex(&archive);

    let fixture = HttpFixture::serve(archive.clone());

    let download_id = mgr
        .enqueue_download(
            "test-model",
            "v1",
            &archive_hash,
            &fixture.url(),
            archive.len() as u64,
        )
        .expect("enqueue");

    // Cancel the download
    mgr.cancel_download(&download_id).expect("cancel");

    let download = catalog
        .get_model_download(&download_id)
        .expect("get download")
        .expect("download exists");
    assert_eq!(
        download.state,
        "cancelled",
        "download should be cancelled"
    );
}

// ---------------------------------------------------------------------------
// 11. Deletion
// ---------------------------------------------------------------------------

#[test]
fn delete_model_removes_files_and_record() {
    let (mgr, catalog, temp) = test_manager();

    // Create a fake installed model directory
    let install_dir = temp
        .path()
        .join("models/asr/test-provider/test-model/v1-aaaabbbbccccddddeeeeffff0000111122223333");
    fs::create_dir_all(&install_dir).expect("create install dir");
    fs::write(
        install_dir.join(".lifesub-model-install"),
        "test-provider\ntest-model\nv1\naaaabbbbccccddddeeeeffff0000111122223333\n",
    )
    .expect("write marker");
    fs::write(install_dir.join("model.onnx"), b"fake model").expect("write model file");

    let now = chrono::Utc::now().to_rfc3339();
    catalog
        .upsert_model_installation(
            "test-model",
            "test-provider",
            "v1",
            "aaaabbbbccccddddeeeeffff0000111122223333",
            install_dir.to_str().unwrap(),
            "ready",
            &now,
            None,
        )
        .expect("insert installation");

    // Delete
    mgr.delete_model("test-model").expect("delete model");

    // Verify directory is gone
    assert!(!install_dir.exists(), "install dir should be removed");

    // Verify DB record is gone
    let install = catalog
        .get_model_installation("test-model")
        .expect("get installation");
    assert!(install.is_none(), "installation record should be removed");
}

// ---------------------------------------------------------------------------
// 12. Full happy path: download, verify, extract, install
// ---------------------------------------------------------------------------

#[test]
fn full_download_extract_install_cycle() {
    let (mgr, catalog, _temp) = test_manager();

    let archive = build_tar_bz2(&[
        ("model.onnx", b"fake onnx model"),
        ("tokens.txt", b"hello\nworld\n"),
    ]);
    let archive_hash = sha256_hex(&archive);

    let fixture = HttpFixture::serve(archive.clone());

    let download_id = mgr
        .enqueue_download(
            "test-model",
            "v1",
            &archive_hash,
            &fixture.url(),
            archive.len() as u64,
        )
        .expect("enqueue");

    // Download
    mgr.download(&download_id).expect("download should succeed");

    // Verify the download state
    let download = catalog
        .get_model_download(&download_id)
        .expect("get download")
        .expect("download exists");
    assert_eq!(
        download.state,
        "succeeded",
        "download should be succeeded"
    );

    // Verify the installation
    let install = catalog
        .get_model_installation("test-model")
        .expect("get installation")
        .expect("installation exists");
    assert_eq!(
        install.state,
        "ready",
        "installation should be ready"
    );
    assert!(Path::new(&install.install_dir).exists(), "install dir exists");
    assert!(
        Path::new(&install.install_dir)
            .join(".lifesub-model-install")
            .exists(),
        "immutable marker exists"
    );
}

// ---------------------------------------------------------------------------
// 13. Single active download per model
// ---------------------------------------------------------------------------

#[test]
fn only_one_active_download_per_model() {
    let (mgr, _catalog, _temp) = test_manager();
    let archive = build_tar_bz2(&[("model.onnx", b"fake")]);
    let archive_hash = sha256_hex(&archive);

    let fixture = HttpFixture::serve(archive.clone());

    // Enqueue first download
    mgr.enqueue_download(
        "test-model",
        "v1",
        &archive_hash,
        &fixture.url(),
        archive.len() as u64,
    )
    .expect("first enqueue");

    // Enqueue second download for same model — should fail
    let result = mgr.enqueue_download(
        "test-model",
        "v2",
        &archive_hash,
        &fixture.url(),
        archive.len() as u64,
    );
    assert!(
        result.is_err(),
        "enqueue second download for same model should fail"
    );
}

// ---------------------------------------------------------------------------
// 14. Cleanup of .part and staging on reconciliation
// ---------------------------------------------------------------------------

#[test]
fn reconcile_cleans_stale_part_files() {
    let (mgr, _catalog, temp) = test_manager();

    // Create a stale .part file in downloads
    let part_path = temp.path().join("downloads/stale.part");
    fs::write(&part_path, b"partial download").expect("write part file");
    assert!(part_path.exists());

    // Run reconciliation
    mgr.reconcile().expect("reconcile");

    // The .part file should be cleaned up
    assert!(
        !part_path.exists(),
        "stale .part file should be cleaned up"
    );
}

#[test]
fn reconcile_cleans_staging_dirs() {
    let (mgr, _catalog, temp) = test_manager();

    // Create a stale staging directory
    let staging = temp.path().join("models/.staging/old-uuid");
    fs::create_dir_all(&staging).expect("create staging dir");
    fs::write(staging.join("model.onnx"), b"data").expect("write staging file");
    assert!(staging.exists());

    // Run reconciliation
    mgr.reconcile().expect("reconcile");

    // The staging directory should be cleaned up
    assert!(
        !staging.exists(),
        "stale staging dir should be cleaned up"
    );
}

// ---------------------------------------------------------------------------
// 15. Versioned install path is correct
// ---------------------------------------------------------------------------

#[test]
fn install_path_uses_versioned_directory() {
    let (mgr, catalog, _temp) = test_manager();

    let archive = build_tar_bz2(&[("model.onnx", b"fake model")]);
    let archive_hash = sha256_hex(&archive);

    let fixture = HttpFixture::serve(archive.clone());

    let download_id = mgr
        .enqueue_download(
            "test-model",
            "v1",
            &archive_hash,
            &fixture.url(),
            archive.len() as u64,
        )
        .expect("enqueue");

    mgr.download(&download_id).expect("download");

    let install = catalog
        .get_model_installation("test-model")
        .expect("get installation")
        .expect("installation exists");

    // The install dir should follow the versioned pattern:
    // models/asr/<provider>/<model-id>/<manifest-version>-<archive-hash>/
    let expected_suffix = format!("v1-{}", &archive_hash);
    assert!(
        install.install_dir.ends_with(&expected_suffix),
        "install dir should end with version-hash: got {}",
        install.install_dir
    );
    assert!(
        install.install_dir.contains("test-provider/test-model"),
        "install dir should contain provider/model path"
    );
}