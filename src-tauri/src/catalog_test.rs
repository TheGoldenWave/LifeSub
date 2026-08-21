use crate::asr::job::{EnqueueJob, canonical_time};
use crate::asr::model_manager::{ArtifactCheckpoint, ModelCatalog, ModelInstallPlan};
use crate::catalog::Catalog;
use crate::domain::{AudioSource, CaptureSession, TranscriptSegment};

fn import_job_fixture(session_id: &str, chunk_id: &str, fingerprint: &str) -> EnqueueJob {
    let now = canonical_time(chrono::Utc::now());
    EnqueueJob {
        id: format!("job_{fingerprint}"),
        session_id: session_id.to_owned(),
        chunk_id: chunk_id.to_owned(),
        provider: "sense_voice".into(),
        model_id: "sense-voice-small-int8-2024-07-17".into(),
        manifest_version: "1".into(),
        archive_sha256: "a".repeat(64),
        required_file_hashes_json: "[]".into(),
        model_source_json: "{}".into(),
        vad_model_id: None,
        vad_manifest_version: None,
        vad_archive_sha256: None,
        vad_required_file_hashes_json: None,
        parameters_json: "{}".into(),
        input_sha256: "b".repeat(64),
        fingerprint: fingerprint.to_owned(),
        available_at: now.clone(),
        created_at: now,
    }
}

#[test]
fn import_jobs_persist_blocked_and_failed_outcomes_with_canonical_time() {
    let catalog = Catalog::in_memory().unwrap();
    let mut session = CaptureSession::new("import outcome");
    session.state = crate::domain::CaptureState::Stopped;
    session.ended_at = Some(chrono::Utc::now());
    catalog.insert_session(&session).unwrap();
    let chunk = crate::domain::AudioChunk {
        id: "chk_import_outcome".into(),
        session_id: session.id.clone(),
        source: AudioSource::Imported,
        path: "audio/import.wav".into(),
        sha256: "b".repeat(64),
        byte_length: 12,
    };
    catalog.insert_chunk(&chunk).unwrap();

    let blocked = import_job_fixture(&session.id, &chunk.id, "blocked");
    catalog
        .insert_blocked_asr_job(&blocked, "model_not_installed", "model missing")
        .unwrap();
    let failed = import_job_fixture(&session.id, &chunk.id, "failed");
    catalog
        .insert_failed_asr_job(&failed, "invalid_provider_parameter", "invalid settings")
        .unwrap();

    let rows = catalog
        .connection()
        .prepare("SELECT state, error_code, created_at, updated_at FROM asr_jobs ORDER BY id")
        .unwrap()
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();

    assert_eq!(rows[0].0, "blocked_model");
    assert_eq!(rows[0].1, "model_not_installed");
    assert_eq!(rows[1].0, "failed");
    assert_eq!(rows[1].1, "invalid_provider_parameter");
    for (_, _, created_at, updated_at) in rows {
        assert_eq!(created_at, updated_at);
        assert!(created_at.ends_with('Z'));
        assert_eq!(created_at.len(), "2026-08-19T00:00:00.000Z".len());
    }
}

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

#[test]
fn catalog_exposes_timeline_loading_primitives() {
    let catalog = Catalog::in_memory().unwrap();
    let session = CaptureSession::new("真实导入样本");
    catalog.insert_session(&session).unwrap();
    let chunk = crate::domain::AudioChunk {
        id: "chk_real".into(),
        session_id: session.id.clone(),
        source: AudioSource::Imported,
        path: "audio/sample.wav".into(),
        sha256: "1".repeat(64),
        byte_length: 123,
    };
    catalog.insert_chunk(&chunk).unwrap();
    catalog
        .append_revision(
            &session.id,
            "sense_voice",
            vec![TranscriptSegment::new(
                0,
                1200,
                AudioSource::Imported,
                "真实转写",
            )],
        )
        .unwrap();

    let sessions = catalog.list_sessions().unwrap();
    let latest_chunk = catalog.latest_chunk_for_session(&session.id).unwrap();
    let revisions = catalog.list_revisions_with_segments(&session.id).unwrap();

    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, session.id);
    assert_eq!(
        latest_chunk.as_ref().map(|value| value.path.as_str()),
        Some("audio/sample.wav")
    );
    assert_eq!(revisions.len(), 1);
    assert_eq!(revisions[0].segments[0].text, "真实转写");
}

#[test]
fn timeline_job_and_chunk_integrity_are_exposed() {
    let catalog = Catalog::in_memory().unwrap();
    let session = CaptureSession::new("timeline dto");
    catalog.insert_session(&session).unwrap();
    let chunk = crate::domain::AudioChunk {
        id: "chk_timeline".into(),
        session_id: session.id.clone(),
        source: AudioSource::Imported,
        path: "audio/timeline.wav".into(),
        sha256: "2".repeat(64),
        byte_length: 456,
    };
    catalog.insert_chunk(&chunk).unwrap();
    catalog
        .update_chunk_integrity(
            &chunk.id,
            crate::domain::ChunkIntegrityState::Missing,
            Some(crate::domain::AsrErrorCode::InputUnavailable),
        )
        .unwrap();
    catalog
        .execute_test_sql(&format!(
            "INSERT INTO asr_jobs(
               id, session_id, chunk_id, provider, model_id, manifest_version, archive_sha256,
               required_file_hashes_json, model_source_json, vad_model_id, vad_manifest_version,
               vad_archive_sha256, vad_required_file_hashes_json, parameters_json, input_sha256,
               fingerprint, state, attempt_count, claim_generation, max_attempts, available_at,
               error_code, error_summary, created_at, updated_at
             ) VALUES(
               'asr_timeline', '{}', '{}', 'sense_voice', 'sense-voice-small-int8-2024-07-17',
               '1', 'bundle', '[]', '{{}}', NULL, NULL, NULL, NULL, '{{}}', '{}',
               'fp', 'blocked_model', 0, 0, 3, '2026-08-19T00:00:00Z',
               'model_not_installed', 'blocked by test', '2026-08-19T00:00:00Z', '2026-08-19T00:00:01Z'
             );",
            session.id, chunk.id, chunk.sha256
        ))
        .unwrap();

    let latest_job = catalog
        .latest_job_for_session(&session.id)
        .unwrap()
        .unwrap();
    let chunks = catalog.list_chunks_for_session(&session.id).unwrap();

    assert_eq!(latest_job.id, "asr_timeline");
    assert_eq!(latest_job.state, "blocked_model");
    assert_eq!(
        latest_job.error_code.as_deref(),
        Some("model_not_installed")
    );
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].chunk.id, chunk.id);
    assert_eq!(
        chunks[0].integrity_state,
        crate::domain::ChunkIntegrityState::Missing
    );
    assert_eq!(chunks[0].error_code.as_deref(), Some("input_unavailable"));
}

#[test]
fn append_manual_revision_from_latest_preserves_chunk_binding_and_marks_manual() {
    let catalog = Catalog::in_memory().unwrap();
    let session = CaptureSession::new("manual revision");
    catalog.insert_session(&session).unwrap();
    let chunk = crate::domain::AudioChunk {
        id: "chk_manual".into(),
        session_id: session.id.clone(),
        source: AudioSource::Imported,
        path: "audio/manual.wav".into(),
        sha256: "3".repeat(64),
        byte_length: 789,
    };
    catalog.insert_chunk(&chunk).unwrap();
    catalog
        .execute_test_sql(&format!(
            "INSERT INTO revisions(id, session_id, number, provider, created_at, provenance_status)
             VALUES('tr_base', '{}', 1, 'sense_voice', '2026-08-19T00:00:00Z', 'verified_local_asr');
             INSERT INTO segments(
               id, revision_id, start_ms, end_ms, source, text, chunk_id, chunk_start_ms, chunk_end_ms, session_start_ms, session_end_ms
             ) VALUES(
               'seg_manual', 'tr_base', 1000, 2200, 'imported', '原始文本', '{}', 100, 1300, 1000, 2200
             );
             INSERT INTO segment_search(segment_id, revision_id, text)
             VALUES('seg_manual', 'tr_base', '原始文本');",
            session.id, chunk.id
        ))
        .unwrap();

    let revision = catalog
        .append_manual_revision_from_latest(
            &session.id,
            vec![crate::domain::TranscriptSegment {
                id: "seg_manual".into(),
                start_ms: 1000,
                end_ms: 2200,
                source: AudioSource::Imported,
                text: "人工修订后".into(),
                chunk_id: None,
                chunk_start_ms: None,
                chunk_end_ms: None,
            }],
        )
        .unwrap();

    assert_eq!(revision.provider, "manual");
    assert_eq!(revision.number, 2);
    assert_eq!(revision.segments[0].chunk_id.as_deref(), Some("chk_manual"));
    assert_eq!(revision.segments[0].chunk_start_ms, Some(100));
    assert_eq!(revision.segments[0].chunk_end_ms, Some(1300));

    let provenance: String = catalog
        .connection()
        .query_row(
            "SELECT provenance_status FROM revisions WHERE id = ?1",
            [&revision.id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(provenance, "manual");
}

#[test]
fn append_manual_revision_from_latest_rejects_empty_text_and_boundary_drift() {
    let catalog = Catalog::in_memory().unwrap();
    let session = CaptureSession::new("manual reject");
    catalog.insert_session(&session).unwrap();
    catalog
        .insert_chunk(&crate::domain::AudioChunk {
            id: "chk_manual".into(),
            session_id: session.id.clone(),
            source: AudioSource::Imported,
            path: "audio/manual.wav".into(),
            sha256: "6".repeat(64),
            byte_length: 42,
        })
        .unwrap();
    catalog
        .execute_test_sql(&format!(
            "INSERT INTO revisions(id, session_id, number, provider, created_at, provenance_status)
             VALUES('tr_base', '{}', 1, 'sense_voice', '2026-08-19T00:00:00Z', 'verified_local_asr');
             INSERT INTO segments(
               id, revision_id, start_ms, end_ms, source, text, chunk_id, chunk_start_ms, chunk_end_ms, session_start_ms, session_end_ms
             ) VALUES(
               'seg_manual', 'tr_base', 1000, 2200, 'imported', '原始文本', 'chk_manual', 100, 1300, 1000, 2200
             );
             INSERT INTO segment_search(segment_id, revision_id, text)
             VALUES('seg_manual', 'tr_base', '原始文本');",
            session.id
        ))
        .unwrap();

    let empty = catalog.append_manual_revision_from_latest(
        &session.id,
        vec![crate::domain::TranscriptSegment {
            id: "seg_manual".into(),
            start_ms: 1000,
            end_ms: 2200,
            source: AudioSource::Imported,
            text: "   ".into(),
            chunk_id: None,
            chunk_start_ms: None,
            chunk_end_ms: None,
        }],
    );
    assert!(empty.is_err());

    let drift = catalog.append_manual_revision_from_latest(
        &session.id,
        vec![crate::domain::TranscriptSegment {
            id: "seg_manual".into(),
            start_ms: 999,
            end_ms: 2200,
            source: AudioSource::Imported,
            text: "人工修订后".into(),
            chunk_id: None,
            chunk_start_ms: None,
            chunk_end_ms: None,
        }],
    );
    assert!(drift.is_err());
}

#[test]
fn append_manual_revision_from_latest_uses_single_available_chunk_as_legacy_fallback() {
    let catalog = Catalog::in_memory().unwrap();
    let session = CaptureSession::new("legacy fallback");
    catalog.insert_session(&session).unwrap();
    let chunk = crate::domain::AudioChunk {
        id: "chk_legacy_single".into(),
        session_id: session.id.clone(),
        source: AudioSource::Imported,
        path: "audio/legacy.wav".into(),
        sha256: "4".repeat(64),
        byte_length: 42,
    };
    catalog.insert_chunk(&chunk).unwrap();
    catalog
        .execute_test_sql(&format!(
            "INSERT INTO revisions(id, session_id, number, provider, created_at, provenance_status)
             VALUES('tr_legacy_single', '{}', 1, 'sense_voice', '2026-08-19T00:00:00Z', 'verified_local_asr');
             INSERT INTO segments(
               id, revision_id, start_ms, end_ms, source, text, chunk_id, chunk_start_ms, chunk_end_ms, session_start_ms, session_end_ms
             ) VALUES(
               'seg_legacy_single', 'tr_legacy_single', 1000, 2200, 'imported', '原始文本', NULL, NULL, NULL, 1000, 2200
             );
             INSERT INTO segment_search(segment_id, revision_id, text)
             VALUES('seg_legacy_single', 'tr_legacy_single', '原始文本');",
            session.id
        ))
        .unwrap();

    let revision = catalog
        .append_manual_revision_from_latest(
            &session.id,
            vec![crate::domain::TranscriptSegment {
                id: "seg_legacy_single".into(),
                start_ms: 1000,
                end_ms: 2200,
                source: AudioSource::Imported,
                text: "人工修订后".into(),
                chunk_id: None,
                chunk_start_ms: None,
                chunk_end_ms: None,
            }],
        )
        .unwrap();

    assert_eq!(
        revision.segments[0].chunk_id.as_deref(),
        Some("chk_legacy_single")
    );
    assert_eq!(revision.segments[0].chunk_start_ms, Some(1000));
    assert_eq!(revision.segments[0].chunk_end_ms, Some(2200));
}

#[test]
fn append_manual_revision_from_latest_rejects_multi_chunk_legacy_binding() {
    let catalog = Catalog::in_memory().unwrap();
    let session = CaptureSession::new("legacy multi chunk");
    catalog.insert_session(&session).unwrap();
    for suffix in ["a", "b"] {
        catalog
            .insert_chunk(&crate::domain::AudioChunk {
                id: format!("chk_legacy_{suffix}"),
                session_id: session.id.clone(),
                source: AudioSource::Imported,
                path: format!("audio/{suffix}.wav"),
                sha256: suffix.repeat(64),
                byte_length: 10,
            })
            .unwrap();
    }
    catalog
        .execute_test_sql(&format!(
            "INSERT INTO revisions(id, session_id, number, provider, created_at, provenance_status)
             VALUES('tr_legacy_multi', '{}', 1, 'sense_voice', '2026-08-19T00:00:00Z', 'verified_local_asr');
             INSERT INTO segments(
               id, revision_id, start_ms, end_ms, source, text, chunk_id, chunk_start_ms, chunk_end_ms, session_start_ms, session_end_ms
             ) VALUES(
               'seg_legacy_multi', 'tr_legacy_multi', 1000, 2200, 'imported', '原始文本', NULL, NULL, NULL, 1000, 2200
             );
             INSERT INTO segment_search(segment_id, revision_id, text)
             VALUES('seg_legacy_multi', 'tr_legacy_multi', '原始文本');",
            session.id
        ))
        .unwrap();

    let error = catalog
        .append_manual_revision_from_latest(
            &session.id,
            vec![crate::domain::TranscriptSegment {
                id: "seg_legacy_multi".into(),
                start_ms: 1000,
                end_ms: 2200,
                source: AudioSource::Imported,
                text: "人工修订后".into(),
                chunk_id: None,
                chunk_start_ms: None,
                chunk_end_ms: None,
            }],
        )
        .unwrap_err();

    assert_eq!(
        error,
        "manual revision requires explicit chunk bindings when legacy segments span multiple chunks"
    );
}

#[test]
fn append_manual_revision_from_latest_rejects_unavailable_single_chunk_legacy_binding() {
    for state in [
        crate::domain::ChunkIntegrityState::Missing,
        crate::domain::ChunkIntegrityState::Corrupted,
    ] {
        let catalog = Catalog::in_memory().unwrap();
        let session = CaptureSession::new("legacy unavailable");
        catalog.insert_session(&session).unwrap();
        let chunk = crate::domain::AudioChunk {
            id: format!("chk_legacy_{state:?}").to_lowercase(),
            session_id: session.id.clone(),
            source: AudioSource::Imported,
            path: "audio/legacy-unavailable.wav".into(),
            sha256: "5".repeat(64),
            byte_length: 42,
        };
        catalog.insert_chunk(&chunk).unwrap();
        catalog
            .update_chunk_integrity(&chunk.id, state, None)
            .unwrap();
        catalog
            .execute_test_sql(&format!(
                "INSERT INTO revisions(id, session_id, number, provider, created_at, provenance_status)
                 VALUES('tr_legacy_unavailable', '{}', 1, 'sense_voice', '2026-08-19T00:00:00Z', 'verified_local_asr');
                 INSERT INTO segments(
                   id, revision_id, start_ms, end_ms, source, text, chunk_id, chunk_start_ms, chunk_end_ms, session_start_ms, session_end_ms
                 ) VALUES(
                   'seg_legacy_unavailable', 'tr_legacy_unavailable', 1000, 2200, 'imported', '原始文本', NULL, NULL, NULL, 1000, 2200
                 );
                 INSERT INTO segment_search(segment_id, revision_id, text)
                 VALUES('seg_legacy_unavailable', 'tr_legacy_unavailable', '原始文本');",
                session.id
            ))
            .unwrap();

        let error = catalog
            .append_manual_revision_from_latest(
                &session.id,
                vec![crate::domain::TranscriptSegment {
                    id: "seg_legacy_unavailable".into(),
                    start_ms: 1000,
                    end_ms: 2200,
                    source: AudioSource::Imported,
                    text: "人工修订后".into(),
                    chunk_id: None,
                    chunk_start_ms: None,
                    chunk_end_ms: None,
                }],
            )
            .unwrap_err();

        assert_eq!(
            error,
            "manual revision requires an available chunk before applying legacy chunk fallback"
        );
    }
}

#[test]
fn append_manual_revision_from_latest_rejects_partial_chunk_bindings() {
    let catalog = Catalog::in_memory().unwrap();
    let session = CaptureSession::new("partial binding");
    catalog.insert_session(&session).unwrap();
    let chunk = crate::domain::AudioChunk {
        id: "chk_partial".into(),
        session_id: session.id.clone(),
        source: AudioSource::Imported,
        path: "audio/partial.wav".into(),
        sha256: "7".repeat(64),
        byte_length: 42,
    };
    catalog.insert_chunk(&chunk).unwrap();
    catalog.execute_test_sql(&format!(
        "INSERT INTO revisions(id, session_id, number, provider, created_at, provenance_status)
         VALUES('tr_partial', '{}', 1, 'sense_voice', '2026-08-19T00:00:00Z', 'verified_local_asr');
         INSERT INTO segments(id, revision_id, start_ms, end_ms, source, text, chunk_id, chunk_start_ms, chunk_end_ms, session_start_ms, session_end_ms)
         VALUES
           ('seg_bound', 'tr_partial', 0, 1000, 'imported', '一', '{}', 0, 1000, 0, 1000),
           ('seg_unbound', 'tr_partial', 1000, 2000, 'imported', '二', NULL, NULL, NULL, 1000, 2000);",
        session.id, chunk.id
    )).unwrap();

    let before = catalog.list_revisions(&session.id).unwrap().len();
    let error = catalog
        .append_manual_revision_from_latest(
            &session.id,
            vec![
                crate::domain::TranscriptSegment {
                    id: "seg_bound".into(),
                    start_ms: 0,
                    end_ms: 1000,
                    source: AudioSource::Imported,
                    text: "一修订".into(),
                    chunk_id: None,
                    chunk_start_ms: None,
                    chunk_end_ms: None,
                },
                crate::domain::TranscriptSegment {
                    id: "seg_unbound".into(),
                    start_ms: 1000,
                    end_ms: 2000,
                    source: AudioSource::Imported,
                    text: "二修订".into(),
                    chunk_id: None,
                    chunk_start_ms: None,
                    chunk_end_ms: None,
                },
            ],
        )
        .unwrap_err();

    assert_eq!(
        error,
        "manual revision rejects partial or mixed chunk bindings"
    );
    assert_eq!(catalog.list_revisions(&session.id).unwrap().len(), before);
}
