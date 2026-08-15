use crate::catalog::Catalog;
use crate::domain::{AudioSource, CaptureSession, TranscriptSegment};

#[test]
fn revisions_are_append_only_and_searchable() {
    let catalog = Catalog::in_memory().unwrap();
    let session = CaptureSession::new("首版讨论");
    catalog.insert_session(&session).unwrap();

    let original = catalog
        .append_revision(
            &session.id,
            "demo-local",
            vec![TranscriptSegment::new(0, 4200, AudioSource::Microphone, "证据链必须保留原始转写")],
        )
        .unwrap();
    let correction = catalog
        .append_revision(
            &session.id,
            "manual",
            vec![TranscriptSegment::new(0, 4200, AudioSource::Microphone, "证据链必须保留原始转写和修订")],
        )
        .unwrap();

    assert_eq!(original.number, 1);
    assert_eq!(correction.number, 2);
    assert_eq!(catalog.list_revisions(&session.id).unwrap().len(), 2);
    assert_eq!(catalog.search_segments("证据链").unwrap().len(), 2);
}

#[test]
fn unknown_persisted_chunk_integrity_is_rejected() {
    let catalog = Catalog::in_memory().unwrap();
    let session = CaptureSession::new("unknown integrity");
    catalog.insert_session(&session).unwrap();
    let chunk = crate::domain::AudioChunk {
        id: "chk_unknown_integrity".into(),
        session_id: session.id,
        source: AudioSource::Imported,
        path: "audio/unknown.wav".into(),
        sha256: "0".repeat(64),
        byte_length: 0,
    };
    catalog.insert_chunk(&chunk).unwrap();
    catalog.force_chunk_integrity(&chunk.id, "future_state").unwrap();

    assert!(catalog.chunk_integrity(&chunk.id).is_err());
    assert!(catalog.chunk_diagnostics(&chunk.id).is_err());
}
