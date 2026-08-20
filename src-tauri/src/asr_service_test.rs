//! Atomic publication tests for ASR Receipts and Revisions.
//!
//! These tests verify that the publish_asr_revision transaction:
//! - Inserts Receipt, Revision, revision_receipts, Segments, and FTS rows
//!   in a single BEGIN IMMEDIATE transaction.
//! - Enforces compatible start_ms/end_ms (session-relative mirror).
//! - Rolls back completely on any partial failure.
//! - Respects fencing tokens (claimed_by + claim_generation).
//! - Rejects publication when cancel_requested_at is set.
//! - Preserves a committed revision even if cancellation arrives after commit.

use chrono::Utc;

use crate::asr::settings::AsrProviderKind;
use crate::catalog::Catalog;
use crate::domain::{
    AsrJobState, AudioChunk, AudioSource, CaptureSession, DataDestination, ProviderOutcome,
    ProviderReceipt, ProvenanceStatus, TranscriptSegment,
};

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

/// Insert a minimal ASR job into the catalog for test setup.
///
/// The job is inserted in `transcribing` state with the given claimed_by
/// and claim_generation, ready for the publication step.
fn insert_test_job(
    catalog: &Catalog,
    session_id: &str,
    chunk_id: &str,
    claimed_by: &str,
    claim_generation: i64,
) -> (String, String, i64) {
    let job_id = format!("job_{}", uuid::Uuid::new_v4().simple());
    let now = Utc::now();
    catalog
        .insert_asr_job(
            &job_id,
            session_id,
            chunk_id,
            AsrProviderKind::SenseVoice,
            "sense-voice-small-int8-2024-07-17",
            "1.0.0",
            "abc123",
            r#"{"files":[]}"#,
            r#"{"source":"test"}"#,
            None,
            None,
            None,
            None,
            r#"{}"#,
            "sha256:test",
            "fp:test",
            AsrJobState::Transcribing,
            1,
            claim_generation,
            3,
            now,
            Some(claimed_by),
            Some(now),
            None,
        )
        .unwrap();
    (job_id, claimed_by.to_string(), claim_generation)
}

/// Build a minimal ProviderReceipt for test assertions.
fn test_receipt(job_id: &str, chunk_id: &str) -> ProviderReceipt {
    ProviderReceipt {
        job_id: job_id.to_string(),
        chunk_id: chunk_id.to_string(),
        provider: AsrProviderKind::SenseVoice,
        model_id: "sense-voice-small-int8-2024-07-17".to_string(),
        manifest_version: "1.0.0".to_string(),
        archive_sha256: "abc123".to_string(),
        required_file_hashes_json: r#"{"files":[]}"#.to_string(),
        model_source_json: r#"{"source":"test"}"#.to_string(),
        vad_model_id: None,
        vad_manifest_version: None,
        vad_archive_sha256: None,
        vad_required_file_hashes_json: None,
        runtime_version: "1.13.5".to_string(),
        runtime_build_id: "3dc7c569".to_string(),
        parameters_json: r#"{}"#.to_string(),
        input_sha256: "sha256:test".to_string(),
        started_at: Utc::now(),
        finished_at: Utc::now(),
        data_destination: DataDestination::LocalDevice,
        outcome: ProviderOutcome::Succeeded,
    }
}

/// Build a test segment with chunk provenance.
fn test_segment(
    chunk_id: &str,
    chunk_start_ms: i64,
    chunk_end_ms: i64,
    session_offset_ms: i64,
    text: &str,
) -> TranscriptSegment {
    TranscriptSegment::new(
        session_offset_ms + chunk_start_ms,
        session_offset_ms + chunk_end_ms,
        AudioSource::Imported,
        text,
    )
    .with_chunk_provenance(chunk_id, chunk_start_ms, chunk_end_ms, session_offset_ms)
}

// ---------------------------------------------------------------------------
// Happy-path: atomic publication succeeds
// ---------------------------------------------------------------------------

#[test]
fn publish_inserts_receipt_revision_segments_and_fts_atomically() {
    let catalog = Catalog::in_memory().unwrap();
    let session = CaptureSession::new("原子发布测试");
    catalog.insert_session(&session).unwrap();

    let chunk = AudioChunk {
        id: "chk_test".to_string(),
        session_id: session.id.clone(),
        source: AudioSource::Imported,
        path: "audio/test.wav".to_string(),
        sha256: "sha256:test".to_string(),
        byte_length: 1024,
    };
    catalog.insert_chunk(&chunk).unwrap();

    let (job_id, claimed_by, claim_generation) =
        insert_test_job(&catalog, &session.id, &chunk.id, "boot-a:worker-1", 1);

    let receipt = test_receipt(&job_id, &chunk.id);
    let segments = vec![
        test_segment(&chunk.id, 0, 2000, 0, "第一段文本"),
        test_segment(&chunk.id, 2500, 5000, 0, "第二段文本"),
    ];

    let revision = catalog
        .publish_asr_revision(
            &job_id,
            &claimed_by,
            claim_generation,
            &session.id,
            "sense_voice",
            &receipt,
            &segments,
        )
        .expect("publish should succeed");

    // ── Revision exists ──
    assert_eq!(revision.session_id, session.id);
    assert_eq!(revision.provider, "sense_voice");
    assert_eq!(
        revision.provenance_status,
        ProvenanceStatus::VerifiedLocalAsr
    );
    assert_eq!(revision.segments.len(), 2);

    // ── Segments are readable and FTS-indexed ──
    let revisions = catalog.list_revisions(&session.id).unwrap();
    assert_eq!(revisions.len(), 1);
    let search_results = catalog.search_segments("第一段").unwrap();
    assert_eq!(search_results.len(), 1);
    assert_eq!(search_results[0].text, "第一段文本");

    // ── Time provenance: session_start_ms == start_ms, session_end_ms == end_ms ──
    for seg in &revision.segments {
        assert_eq!(
            seg.start_ms,
            seg.session_start_ms.unwrap(),
            "start_ms must equal session_start_ms"
        );
        assert_eq!(
            seg.end_ms,
            seg.session_end_ms.unwrap(),
            "end_ms must equal session_end_ms"
        );
        assert_eq!(
            seg.session_start_ms.unwrap(),
            0 + seg.chunk_start_ms.unwrap(),
            "session_start_ms = session_offset_ms + chunk_start_ms"
        );
        assert_eq!(
            seg.session_end_ms.unwrap(),
            0 + seg.chunk_end_ms.unwrap(),
            "session_end_ms = session_offset_ms + chunk_end_ms"
        );
    }
}

// ---------------------------------------------------------------------------
// Partial failure: no partial Evidence is visible
// ---------------------------------------------------------------------------

#[test]
fn publish_rolls_back_on_duplicate_job_id() {
    let catalog = Catalog::in_memory().unwrap();
    let session = CaptureSession::new("重复发布测试");
    catalog.insert_session(&session).unwrap();

    let chunk = AudioChunk {
        id: "chk_dup".to_string(),
        session_id: session.id.clone(),
        source: AudioSource::Imported,
        path: "audio/dup.wav".to_string(),
        sha256: "sha256:test".to_string(),
        byte_length: 1024,
    };
    catalog.insert_chunk(&chunk).unwrap();

    let (job_id, claimed_by, claim_generation) =
        insert_test_job(&catalog, &session.id, &chunk.id, "boot-a:worker-1", 1);

    let receipt = test_receipt(&job_id, &chunk.id);
    let segments = vec![test_segment(&chunk.id, 0, 1000, 0, "文本")];

    // First publish succeeds
    catalog
        .publish_asr_revision(
            &job_id,
            &claimed_by,
            claim_generation,
            &session.id,
            "sense_voice",
            &receipt,
            &segments,
        )
        .unwrap();

    // Second publish with the same job_id MUST fail (job already succeeded)
    // Re-insert a new job with same id but transcribing state for the test
    // Actually, the first publish already set the job to succeeded. The second
    // publish will fail the fencing check because state != 'transcribing'.
    let result = catalog.publish_asr_revision(
        &job_id,
        &claimed_by,
        claim_generation,
        &session.id,
        "sense_voice",
        &receipt,
        &segments,
    );
    assert!(result.is_err(), "fencing must fail after first publish");

    // Only one revision exists — no partial second revision
    let revisions = catalog.list_revisions(&session.id).unwrap();
    assert_eq!(revisions.len(), 1);
}

#[test]
fn publish_rolls_back_when_segment_has_invalid_chunk_id() {
    let catalog = Catalog::in_memory().unwrap();
    let session = CaptureSession::new("无效chunk测试");
    catalog.insert_session(&session).unwrap();

    let chunk = AudioChunk {
        id: "chk_valid".to_string(),
        session_id: session.id.clone(),
        source: AudioSource::Imported,
        path: "audio/valid.wav".to_string(),
        sha256: "sha256:test".to_string(),
        byte_length: 1024,
    };
    catalog.insert_chunk(&chunk).unwrap();

    let (job_id, claimed_by, claim_generation) =
        insert_test_job(&catalog, &session.id, &chunk.id, "boot-a:worker-1", 1);

    let receipt = test_receipt(&job_id, &chunk.id);

    // Segment references a non-existent chunk_id — FOREIGN KEY violation
    let segments = vec![test_segment("nonexistent_chunk", 0, 1000, 0, "文本")];

    let result = catalog.publish_asr_revision(
        &job_id,
        &claimed_by,
        claim_generation,
        &session.id,
        "sense_voice",
        &receipt,
        &segments,
    );
    assert!(result.is_err(), "invalid chunk_id must fail");

    // No revision was created
    let revisions = catalog.list_revisions(&session.id).unwrap();
    assert_eq!(revisions.len(), 0, "no revision should exist after rollback");
}

// ---------------------------------------------------------------------------
// Cancellation race: cancel before transaction = no revision
// ---------------------------------------------------------------------------

#[test]
fn cancel_before_publish_prevents_revision() {
    let catalog = Catalog::in_memory().unwrap();
    let session = CaptureSession::new("取消测试");
    catalog.insert_session(&session).unwrap();

    let chunk = AudioChunk {
        id: "chk_cancel".to_string(),
        session_id: session.id.clone(),
        source: AudioSource::Imported,
        path: "audio/cancel.wav".to_string(),
        sha256: "sha256:test".to_string(),
        byte_length: 1024,
    };
    catalog.insert_chunk(&chunk).unwrap();

    let (job_id, claimed_by, claim_generation) =
        insert_test_job(&catalog, &session.id, &chunk.id, "boot-a:worker-1", 1);

    // Set cancel_requested_at BEFORE the publish transaction
    catalog.request_cancel(&job_id).unwrap();

    let receipt = test_receipt(&job_id, &chunk.id);
    let segments = vec![test_segment(&chunk.id, 0, 1000, 0, "文本")];

    let result = catalog.publish_asr_revision(
        &job_id,
        &claimed_by,
        claim_generation,
        &session.id,
        "sense_voice",
        &receipt,
        &segments,
    );
    assert!(
        result.is_err(),
        "publish must fail when cancel_requested_at is set"
    );

    // No revision was created
    let revisions = catalog.list_revisions(&session.id).unwrap();
    assert_eq!(revisions.len(), 0);
}

// ---------------------------------------------------------------------------
// Cancellation race: cancel after commit = revision remains
// ---------------------------------------------------------------------------

#[test]
fn cancel_after_commit_keeps_revision() {
    let catalog = Catalog::in_memory().unwrap();
    let session = CaptureSession::new("后取消测试");
    catalog.insert_session(&session).unwrap();

    let chunk = AudioChunk {
        id: "chk_post".to_string(),
        session_id: session.id.clone(),
        source: AudioSource::Imported,
        path: "audio/post.wav".to_string(),
        sha256: "sha256:test".to_string(),
        byte_length: 1024,
    };
    catalog.insert_chunk(&chunk).unwrap();

    let (job_id, claimed_by, claim_generation) =
        insert_test_job(&catalog, &session.id, &chunk.id, "boot-a:worker-1", 1);

    let receipt = test_receipt(&job_id, &chunk.id);
    let segments = vec![test_segment(&chunk.id, 0, 1000, 0, "文本")];

    // Publish succeeds
    let revision = catalog
        .publish_asr_revision(
            &job_id,
            &claimed_by,
            claim_generation,
            &session.id,
            "sense_voice",
            &receipt,
            &segments,
        )
        .unwrap();

    // Cancel arrives AFTER commit — the revision must remain
    catalog.request_cancel(&job_id).unwrap();

    // Revision still exists
    let revisions = catalog.list_revisions(&session.id).unwrap();
    assert_eq!(revisions.len(), 1);
    assert_eq!(revisions[0].id, revision.id);
}

// ---------------------------------------------------------------------------
// Stale generation: fencing token prevents publishing
// ---------------------------------------------------------------------------

#[test]
fn stale_generation_cannot_publish() {
    let catalog = Catalog::in_memory().unwrap();
    let session = CaptureSession::new("stale测试");
    catalog.insert_session(&session).unwrap();

    let chunk = AudioChunk {
        id: "chk_stale".to_string(),
        session_id: session.id.clone(),
        source: AudioSource::Imported,
        path: "audio/stale.wav".to_string(),
        sha256: "sha256:test".to_string(),
        byte_length: 1024,
    };
    catalog.insert_chunk(&chunk).unwrap();

    let (job_id, _claimed_by, _claim_generation) =
        insert_test_job(&catalog, &session.id, &chunk.id, "boot-a:worker-1", 1);

    let receipt = test_receipt(&job_id, &chunk.id);
    let segments = vec![test_segment(&chunk.id, 0, 1000, 0, "文本")];

    // Try to publish with a DIFFERENT claim_generation (stale)
    let result = catalog.publish_asr_revision(
        &job_id,
        "boot-a:worker-1", // same claimed_by
        99,                // wrong claim_generation
        &session.id,
        "sense_voice",
        &receipt,
        &segments,
    );
    assert!(
        result.is_err(),
        "stale claim_generation must fail to publish"
    );

    // No revision was created
    let revisions = catalog.list_revisions(&session.id).unwrap();
    assert_eq!(revisions.len(), 0);
}

#[test]
fn stale_claimed_by_cannot_publish() {
    let catalog = Catalog::in_memory().unwrap();
    let session = CaptureSession::new("stale claimed_by测试");
    catalog.insert_session(&session).unwrap();

    let chunk = AudioChunk {
        id: "chk_stale2".to_string(),
        session_id: session.id.clone(),
        source: AudioSource::Imported,
        path: "audio/stale2.wav".to_string(),
        sha256: "sha256:test".to_string(),
        byte_length: 1024,
    };
    catalog.insert_chunk(&chunk).unwrap();

    let (job_id, _claimed_by, claim_generation) =
        insert_test_job(&catalog, &session.id, &chunk.id, "boot-a:worker-1", 1);

    let receipt = test_receipt(&job_id, &chunk.id);
    let segments = vec![test_segment(&chunk.id, 0, 1000, 0, "文本")];

    // Try to publish with a DIFFERENT claimed_by (stale / different worker)
    let result = catalog.publish_asr_revision(
        &job_id,
        "boot-b:worker-2", // different claimed_by
        claim_generation,
        &session.id,
        "sense_voice",
        &receipt,
        &segments,
    );
    assert!(result.is_err(), "stale claimed_by must fail to publish");

    // No revision was created
    let revisions = catalog.list_revisions(&session.id).unwrap();
    assert_eq!(revisions.len(), 0);
}

// ---------------------------------------------------------------------------
// Time provenance: session_start_ms = chunk.session_offset_ms + chunk_start_ms
// ---------------------------------------------------------------------------

#[test]
fn time_provenance_preserves_session_offset() {
    let catalog = Catalog::in_memory().unwrap();
    let session = CaptureSession::new("时间坐标测试");
    catalog.insert_session(&session).unwrap();

    // Simulate a chunk with session_offset_ms = 5000 (5 seconds into the session)
    let chunk = AudioChunk {
        id: "chk_offset".to_string(),
        session_id: session.id.clone(),
        source: AudioSource::Imported,
        path: "audio/offset.wav".to_string(),
        sha256: "sha256:offset".to_string(),
        byte_length: 2048,
    };
    catalog.insert_chunk(&chunk).unwrap();
    // Set the session_offset_ms explicitly for this test
    catalog
        .set_chunk_session_offset(&chunk.id, 5000)
        .unwrap();

    let (job_id, claimed_by, claim_generation) =
        insert_test_job(&catalog, &session.id, &chunk.id, "boot-a:worker-1", 1);

    let receipt = test_receipt(&job_id, &chunk.id);
    // chunk-relative times: 1000ms to 4000ms
    // session_offset_ms = 5000ms
    // session-relative times: 6000ms to 9000ms
    let segments = vec![test_segment(&chunk.id, 1000, 4000, 5000, "偏移文本")];

    let revision = catalog
        .publish_asr_revision(
            &job_id,
            &claimed_by,
            claim_generation,
            &session.id,
            "sense_voice",
            &receipt,
            &segments,
        )
        .unwrap();

    let seg = &revision.segments[0];
    assert_eq!(seg.chunk_start_ms.unwrap(), 1000);
    assert_eq!(seg.chunk_end_ms.unwrap(), 4000);
    assert_eq!(seg.session_start_ms.unwrap(), 6000);
    assert_eq!(seg.session_end_ms.unwrap(), 9000);
    // Compatibility mirror
    assert_eq!(seg.start_ms, 6000);
    assert_eq!(seg.end_ms, 9000);
}

// ---------------------------------------------------------------------------
// Job state transitions: publish marks job as succeeded
// ---------------------------------------------------------------------------

#[test]
fn publish_transitions_job_to_succeeded() {
    let catalog = Catalog::in_memory().unwrap();
    let session = CaptureSession::new("状态转换测试");
    catalog.insert_session(&session).unwrap();

    let chunk = AudioChunk {
        id: "chk_state".to_string(),
        session_id: session.id.clone(),
        source: AudioSource::Imported,
        path: "audio/state.wav".to_string(),
        sha256: "sha256:test".to_string(),
        byte_length: 1024,
    };
    catalog.insert_chunk(&chunk).unwrap();

    let (job_id, claimed_by, claim_generation) =
        insert_test_job(&catalog, &session.id, &chunk.id, "boot-a:worker-1", 1);

    let receipt = test_receipt(&job_id, &chunk.id);
    let segments = vec![test_segment(&chunk.id, 0, 1000, 0, "文本")];

    catalog
        .publish_asr_revision(
            &job_id,
            &claimed_by,
            claim_generation,
            &session.id,
            "sense_voice",
            &receipt,
            &segments,
        )
        .unwrap();

    // Job state was updated to succeeded
    let job_row = catalog.get_job(&job_id).unwrap().unwrap();
    assert_eq!(job_row.state, AsrJobState::Succeeded);
}

// ---------------------------------------------------------------------------
// FTS indexing: all segments are searchable after publish
// ---------------------------------------------------------------------------

#[test]
fn publish_indexes_all_segments_in_fts() {
    let catalog = Catalog::in_memory().unwrap();
    let session = CaptureSession::new("FTS测试");
    catalog.insert_session(&session).unwrap();

    let chunk = AudioChunk {
        id: "chk_fts".to_string(),
        session_id: session.id.clone(),
        source: AudioSource::Imported,
        path: "audio/fts.wav".to_string(),
        sha256: "sha256:test".to_string(),
        byte_length: 1024,
    };
    catalog.insert_chunk(&chunk).unwrap();

    let (job_id, claimed_by, claim_generation) =
        insert_test_job(&catalog, &session.id, &chunk.id, "boot-a:worker-1", 1);

    let receipt = test_receipt(&job_id, &chunk.id);
    let segments = vec![
        test_segment(&chunk.id, 0, 1000, 0, "证据链完整性"),
        test_segment(&chunk.id, 1500, 3000, 0, "原始音频归档"),
        test_segment(&chunk.id, 3500, 5000, 0, "哈希校验通过"),
    ];

    catalog
        .publish_asr_revision(
            &job_id,
            &claimed_by,
            claim_generation,
            &session.id,
            "sense_voice",
            &receipt,
            &segments,
        )
        .unwrap();

    // Each segment is independently searchable via trigram FTS
    // Use 3+ character search terms for reliable trigram matching
    assert_eq!(catalog.search_segments("证据链").unwrap().len(), 1);
    assert_eq!(catalog.search_segments("校验通过").unwrap().len(), 1);
    // "原始音频" only appears in the middle segment
    let all = catalog.search_segments("原始音频").unwrap();
    assert_eq!(all.len(), 1);
}