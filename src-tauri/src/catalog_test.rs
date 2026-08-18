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
            vec![TranscriptSegment::new(
                0,
                4200,
                AudioSource::Microphone,
                "证据链必须保留原始转写",
            )],
        )
        .unwrap();
    let correction = catalog
        .append_revision(
            &session.id,
            "manual",
            vec![TranscriptSegment::new(
                0,
                4200,
                AudioSource::Microphone,
                "证据链必须保留原始转写和修订",
            )],
        )
        .unwrap();

    assert_eq!(original.number, 1);
    assert_eq!(correction.number, 2);
    assert_eq!(catalog.list_revisions(&session.id).unwrap().len(), 2);
    assert_eq!(catalog.search_segments("证据链").unwrap().len(), 2);
}
