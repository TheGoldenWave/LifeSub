use crate::asr::model_manager::{ArtifactCheckpoint, ModelCatalog, ModelInstallPlan};
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
    catalog
        .force_chunk_integrity(&chunk.id, "future_state")
        .unwrap();

    assert!(catalog.chunk_integrity(&chunk.id).is_err());
    assert!(catalog.chunk_diagnostics(&chunk.id).is_err());
}

#[test]
fn checkpoint_aggregate_failure_rolls_back_artifact_progress() {
    let catalog = Catalog::in_memory().unwrap();
    let plan = ModelInstallPlan::from_manifest(
        crate::asr::manifest::model_registry()
            .model("qwen3-asr-1.7b")
            .unwrap(),
    );
    let download_id = catalog.begin_download(&plan).unwrap();
    let artifact = &plan.artifacts[0];
    catalog
        .execute_test_sql(
            "CREATE TRIGGER fail_model_download_aggregate
             BEFORE UPDATE OF downloaded_bytes ON model_downloads
             BEGIN SELECT RAISE(ABORT, 'injected aggregate failure'); END;",
        )
        .unwrap();

    let error = catalog
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
                    artifact.required_path
                ),
                downloaded_bytes: 1,
                expected_bytes: artifact.expected_bytes,
                temp_path: std::path::PathBuf::from("checkpoint.part"),
                etag: None,
                last_modified: None,
                verified_sha256: None,
                state: "downloading".to_owned(),
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), "model_catalog_failed");
    assert!(
        catalog
            .checkpoint(&download_id, &artifact.artifact_id)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        catalog
            .model_downloaded_bytes_for_test(&download_id)
            .unwrap(),
        0
    );
}
