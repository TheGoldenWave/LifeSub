use std::fs;

use tempfile::tempdir;

use crate::catalog::Catalog;
use crate::domain::{AudioSource, CaptureSession, TranscriptSegment};
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
