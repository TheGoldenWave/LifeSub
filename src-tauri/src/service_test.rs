use std::fs;

use tempfile::tempdir;

use crate::catalog::Catalog;
use crate::domain::{AudioSource, CaptureSession, ChunkIntegrityState, TranscriptSegment};
use crate::service::{EvidenceService, EvidenceTarget, parse_evidence_uri};

#[test]
fn imported_audio_is_copied_and_hashed_without_touching_the_source() {
    let source_dir = tempdir().unwrap();
    let data_dir = tempdir().unwrap();
    let source = source_dir.path().join("sample.wav");
    fs::write(&source, b"lifesub audio fixture").unwrap();
    let catalog = Catalog::in_memory().unwrap();
    let service = EvidenceService::new(catalog, data_dir.path());
    let session = CaptureSession::new("导入测试");

    let chunk = service.import_audio(&session, &source).unwrap();

    assert_eq!(fs::read(&source).unwrap(), b"lifesub audio fixture");
    assert_eq!(chunk.source, AudioSource::Imported);
    assert_eq!(chunk.byte_length, 21);
    assert!(data_dir.path().join(&chunk.path).exists());
    assert_eq!(chunk.sha256.len(), 64);
}

#[test]
fn markdown_export_contains_revision_and_stable_evidence() {
    let data_dir = tempdir().unwrap();
    let catalog = Catalog::in_memory().unwrap();
    let session = CaptureSession::new("首版讨论");
    catalog.insert_session(&session).unwrap();
    let revision = catalog
        .append_revision(
            &session.id,
            "demo-local",
            vec![TranscriptSegment::new(
                1200,
                4400,
                AudioSource::Microphone,
                "原始音频先持久化",
            )],
        )
        .unwrap();
    let service = EvidenceService::new(catalog, data_dir.path());

    let markdown = service.render_markdown(&session, &revision);

    assert!(markdown.contains(&session.evidence_uri()));
    assert!(markdown.contains("transcript_revision: 1"));
    assert!(markdown.contains("原始音频先持久化"));
}

#[test]
fn evidence_uri_parser_keeps_audio_time_ranges() {
    assert_eq!(
        parse_evidence_uri("lifesub://audio/chk_123#t=120,165").unwrap(),
        EvidenceTarget::Audio {
            id: "chk_123".into(),
            start_seconds: Some(120),
            end_seconds: Some(165)
        }
    );
}

#[test]
fn imported_audio_uses_atomic_temp_write_and_leaves_no_orphan_temp_files() {
    let source_dir = tempdir().unwrap();
    let data_dir = tempdir().unwrap();
    let source = source_dir.path().join("sample.wav");
    fs::write(&source, b"lifesub audio fixture").unwrap();
    let catalog = Catalog::in_memory().unwrap();
    let service = EvidenceService::new(catalog, data_dir.path());
    let session = CaptureSession::new("原子写入测试");

    let chunk = service.import_audio(&session, &source).unwrap();

    // 源文件未被修改
    assert_eq!(fs::read(&source).unwrap(), b"lifesub audio fixture");
    // 最终文件存在且内容正确
    let final_path = data_dir.path().join(&chunk.path);
    assert!(final_path.exists());
    assert_eq!(fs::read(&final_path).unwrap(), b"lifesub audio fixture");
    // 音频目录中不存在任何 .tmp 临时文件
    let audio_dir = data_dir.path().join("audio");
    let orphan_count = fs::read_dir(&audio_dir)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
        .count();
    assert_eq!(
        orphan_count, 0,
        "audio directory must contain no orphan .tmp files"
    );
}

#[test]
fn reconcile_cleans_orphan_temp_files() {
    let data_dir = tempdir().unwrap();
    let audio_dir = data_dir.path().join("audio");
    fs::create_dir_all(&audio_dir).unwrap();
    // 手动创建模拟的孤儿临时文件
    let orphan_path = audio_dir.join("orphan_test.tmp");
    fs::write(&orphan_path, b"orphan data").unwrap();
    assert!(orphan_path.exists());

    let catalog = Catalog::in_memory().unwrap();
    let service = EvidenceService::new(catalog, data_dir.path());

    service.reconcile_chunks().unwrap();

    // 孤儿临时文件已被清理
    assert!(!orphan_path.exists());
}

#[test]
fn missing_chunk_file_is_detected_during_reconcile() {
    let source_dir = tempdir().unwrap();
    let data_dir = tempdir().unwrap();
    let source = source_dir.path().join("sample.wav");
    fs::write(&source, b"lifesub audio fixture").unwrap();
    let catalog = Catalog::in_memory().unwrap();
    let service = EvidenceService::new(catalog, data_dir.path());
    let session = CaptureSession::new("缺失文件测试");

    let chunk = service.import_audio(&session, &source).unwrap();

    // 删除实际文件，模拟文件丢失
    fs::remove_file(data_dir.path().join(&chunk.path)).unwrap();

    service.reconcile_chunks().unwrap();

    // chunk 被标记为 missing
    let state = service.chunk_integrity(&chunk.id).unwrap();
    assert_eq!(state, ChunkIntegrityState::Missing);
}

#[test]
fn changed_chunk_bytes_are_detected_during_reconcile() {
    let source_dir = tempdir().unwrap();
    let data_dir = tempdir().unwrap();
    let source = source_dir.path().join("sample.wav");
    fs::write(&source, b"lifesub audio fixture").unwrap();
    let catalog = Catalog::in_memory().unwrap();
    let service = EvidenceService::new(catalog, data_dir.path());
    let session = CaptureSession::new("篡改检测测试");

    let chunk = service.import_audio(&session, &source).unwrap();

    // 篡改文件内容
    fs::write(data_dir.path().join(&chunk.path), b"tampered content").unwrap();

    service.reconcile_chunks().unwrap();

    // chunk 被标记为 corrupted
    let state = service.chunk_integrity(&chunk.id).unwrap();
    assert_eq!(state, ChunkIntegrityState::Corrupted);
}

#[test]
fn verify_chunk_passes_with_intact_file() {
    let source_dir = tempdir().unwrap();
    let data_dir = tempdir().unwrap();
    let source = source_dir.path().join("sample.wav");
    fs::write(&source, b"lifesub audio fixture").unwrap();
    let catalog = Catalog::in_memory().unwrap();
    let service = EvidenceService::new(catalog, data_dir.path());
    let session = CaptureSession::new("校验通过测试");

    let chunk = service.import_audio(&session, &source).unwrap();

    // 文件完整时校验通过
    service.verify_chunk(&chunk.id).unwrap();
}

#[test]
fn verify_chunk_fails_when_hash_mismatches() {
    let source_dir = tempdir().unwrap();
    let data_dir = tempdir().unwrap();
    let source = source_dir.path().join("sample.wav");
    fs::write(&source, b"lifesub audio fixture").unwrap();
    let catalog = Catalog::in_memory().unwrap();
    let service = EvidenceService::new(catalog, data_dir.path());
    let session = CaptureSession::new("校验失败测试");

    let chunk = service.import_audio(&session, &source).unwrap();

    // 篡改文件内容
    fs::write(data_dir.path().join(&chunk.path), b"tampered content").unwrap();

    // 校验失败
    let result = service.verify_chunk(&chunk.id);
    assert!(result.is_err());
}

#[test]
fn verify_chunk_marks_missing_when_file_absent() {
    let source_dir = tempdir().unwrap();
    let data_dir = tempdir().unwrap();
    let source = source_dir.path().join("sample.wav");
    fs::write(&source, b"lifesub audio fixture").unwrap();
    let catalog = Catalog::in_memory().unwrap();
    let service = EvidenceService::new(catalog, data_dir.path());
    let session = CaptureSession::new("文件缺失校验测试");

    let chunk = service.import_audio(&session, &source).unwrap();

    // 删除文件
    fs::remove_file(data_dir.path().join(&chunk.path)).unwrap();

    // 校验失败
    let result = service.verify_chunk(&chunk.id);
    assert!(result.is_err());
    // 校验后 chunk 状态被更新为 missing
    let state = service.chunk_integrity(&chunk.id).unwrap();
    assert_eq!(state, ChunkIntegrityState::Missing);
}
