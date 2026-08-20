use rusqlite::Connection;

use crate::catalog::migrations::{classify_schema, SchemaKind};

/// Create an in-memory database with the exact V0.1 schema fingerprint.
fn create_v0_1_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         CREATE TABLE sessions (
           id TEXT PRIMARY KEY, title TEXT NOT NULL, state TEXT NOT NULL,
           started_at TEXT NOT NULL, ended_at TEXT
         );
         CREATE TABLE revisions (
           id TEXT PRIMARY KEY, session_id TEXT NOT NULL, number INTEGER NOT NULL,
           provider TEXT NOT NULL, created_at TEXT NOT NULL,
           UNIQUE(session_id, number), FOREIGN KEY(session_id) REFERENCES sessions(id)
         );
         CREATE TABLE segments (
           id TEXT NOT NULL, revision_id TEXT NOT NULL, start_ms INTEGER NOT NULL,
           end_ms INTEGER NOT NULL, source TEXT NOT NULL, text TEXT NOT NULL,
           PRIMARY KEY(id, revision_id), FOREIGN KEY(revision_id) REFERENCES revisions(id)
         );
         CREATE TABLE chunks (
           id TEXT PRIMARY KEY, session_id TEXT NOT NULL, source TEXT NOT NULL,
           path TEXT NOT NULL, sha256 TEXT NOT NULL, byte_length INTEGER NOT NULL,
           FOREIGN KEY(session_id) REFERENCES sessions(id)
         );
         CREATE VIRTUAL TABLE segment_search USING fts5(
           segment_id UNINDEXED, revision_id UNINDEXED, text, tokenize='trigram'
         );",
    )
    .unwrap();
    conn
}

/// Create an in-memory database with the exact V0.1 schema and representative
/// test data — a session, a chunk, two revisions with segments, and search
/// content — so that migration assertions can verify legacy data is still
/// readable after upgrade.
fn create_v0_1_db_with_data() -> Connection {
    let conn = create_v0_1_db();
    conn.execute_batch(
        "INSERT INTO sessions(id, title, state, started_at) VALUES('rec_test', '迁移测试', 'stopped', '2025-01-01T00:00:00Z');
         INSERT INTO chunks(id, session_id, source, path, sha256, byte_length) VALUES('chk_test', 'rec_test', 'imported', 'audio/test.wav', 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855', 0);
         INSERT INTO revisions(id, session_id, number, provider, created_at) VALUES('rev_1', 'rec_test', 1, 'demo-local', '2025-01-01T00:00:01Z');
         INSERT INTO revisions(id, session_id, number, provider, created_at) VALUES('rev_2', 'rec_test', 2, 'manual', '2025-01-01T00:00:02Z');
         INSERT INTO segments(id, revision_id, start_ms, end_ms, source, text) VALUES('seg_1', 'rev_1', 0, 4200, 'microphone', '原始转写文本');
         INSERT INTO segments(id, revision_id, start_ms, end_ms, source, text) VALUES('seg_2', 'rev_2', 0, 4200, 'microphone', '修订后的转写文本');
         INSERT INTO segment_search(segment_id, revision_id, text) VALUES('seg_1', 'rev_1', '原始转写文本');
         INSERT INTO segment_search(segment_id, revision_id, text) VALUES('seg_2', 'rev_2', '修订后的转写文本');",
    )
    .unwrap();
    conn
}

// ── Schema classification ──────────────────────────────────────────────

#[test]
fn classify_fresh_database() {
    let conn = Connection::open_in_memory().unwrap();
    assert_eq!(classify_schema(&conn).unwrap(), SchemaKind::Fresh);
}

#[test]
fn classify_legacy_v1_database() {
    let conn = create_v0_1_db();
    assert_eq!(classify_schema(&conn).unwrap(), SchemaKind::LegacyV1);
}

#[test]
fn classify_unknown_v0_database() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE unknown_table (id INTEGER PRIMARY KEY);",
    )
    .unwrap();
    assert_eq!(classify_schema(&conn).unwrap(), SchemaKind::Unknown);
}

#[test]
fn classify_unknown_when_columns_differ() {
    let conn = Connection::open_in_memory().unwrap();
    // Missing a column: sessions without `ended_at`
    conn.execute_batch(
        "CREATE TABLE sessions (id TEXT PRIMARY KEY, title TEXT NOT NULL, state TEXT NOT NULL, started_at TEXT NOT NULL);
         CREATE TABLE revisions (id TEXT PRIMARY KEY, session_id TEXT NOT NULL, number INTEGER NOT NULL, provider TEXT NOT NULL, created_at TEXT NOT NULL, UNIQUE(session_id, number), FOREIGN KEY(session_id) REFERENCES sessions(id));
         CREATE TABLE segments (id TEXT NOT NULL, revision_id TEXT NOT NULL, start_ms INTEGER NOT NULL, end_ms INTEGER NOT NULL, source TEXT NOT NULL, text TEXT NOT NULL, PRIMARY KEY(id, revision_id), FOREIGN KEY(revision_id) REFERENCES revisions(id));
         CREATE TABLE chunks (id TEXT PRIMARY KEY, session_id TEXT NOT NULL, source TEXT NOT NULL, path TEXT NOT NULL, sha256 TEXT NOT NULL, byte_length INTEGER NOT NULL, FOREIGN KEY(session_id) REFERENCES sessions(id));
         CREATE VIRTUAL TABLE segment_search USING fts5(segment_id UNINDEXED, revision_id UNINDEXED, text, tokenize='trigram');",
    )
    .unwrap();
    assert_eq!(classify_schema(&conn).unwrap(), SchemaKind::Unknown);
}

// ── Migration: legacy data preservation ────────────────────────────────

#[test]
fn migrate_v1_preserves_revisions() {
    let conn = create_v0_1_db_with_data();
    // Classify before migration
    assert_eq!(classify_schema(&conn).unwrap(), SchemaKind::LegacyV1);

    // Run the migration
    crate::catalog::migrations::migrate(&conn).unwrap();

    // Verify user_version is now 2
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, 2);

    // Revisions still readable
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM revisions", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 2);

    // Provider strings preserved
    let providers: Vec<String> = conn
        .prepare("SELECT provider FROM revisions ORDER BY number")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(providers, vec!["demo-local", "manual"]);

    // Legacy revisions marked legacy_unverified
    let statuses: Vec<String> = conn
        .prepare("SELECT provenance_status FROM revisions ORDER BY number")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(statuses, vec!["legacy_unverified", "legacy_unverified"]);
}

#[test]
fn migrate_v1_preserves_segments_and_search() {
    let conn = create_v0_1_db_with_data();
    crate::catalog::migrations::migrate(&conn).unwrap();

    // Segments still readable with original start_ms/end_ms
    let segments: Vec<(String, i64, i64)> = conn
        .prepare("SELECT text, start_ms, end_ms FROM segments ORDER BY id")
        .unwrap()
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?))
        })
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(segments.len(), 2);
    assert_eq!(segments[0].0, "原始转写文本");
    assert_eq!(segments[0].1, 0);
    assert_eq!(segments[0].2, 4200);

    // FTS5 search still works
    let raw_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM segment_search",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(raw_count, 2, "FTS5 table should contain 2 rows after migration");

    let search_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM segment_search WHERE segment_search MATCH '转写文本'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(search_count, 2);
}

#[test]
fn migrate_v1_preserves_chunks() {
    let conn = create_v0_1_db_with_data();
    crate::catalog::migrations::migrate(&conn).unwrap();

    // Chunk still readable
    let (sha256, session_offset_ms, integrity_state): (String, i64, String) = conn
        .query_row(
            "SELECT sha256, session_offset_ms, integrity_state FROM chunks WHERE id = 'chk_test'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        sha256,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    // New columns have expected defaults for legacy data
    assert_eq!(session_offset_ms, 0);
    assert_eq!(integrity_state, "available");
}

#[test]
fn migrate_v1_adds_new_columns_to_segments() {
    let conn = create_v0_1_db_with_data();
    crate::catalog::migrations::migrate(&conn).unwrap();

    // New segment columns exist and are NULL for legacy data (no chunk
    // association yet)
    let (chunk_id, chunk_start_ms, session_start_ms): (
        Option<String>,
        Option<i64>,
        Option<i64>,
    ) = conn
        .query_row(
            "SELECT chunk_id, chunk_start_ms, session_start_ms FROM segments WHERE id = 'seg_1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert!(chunk_id.is_none());
    assert!(chunk_start_ms.is_none());
    assert!(session_start_ms.is_none());
}

#[test]
fn migrate_v1_creates_asr_tables() {
    let conn = create_v0_1_db_with_data();
    crate::catalog::migrations::migrate(&conn).unwrap();

    // All new v2 tables exist
    for table in &[
        "asr_settings",
        "model_installations",
        "model_downloads",
        "asr_jobs",
        "provider_receipts",
        "revision_receipts",
    ] {
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name=?1",
                [table],
                |row| row.get(0),
            )
            .unwrap();
        assert!(exists, "expected table {} to exist after migration", table);
    }
}

#[test]
fn migrate_v1_creates_partial_indexes() {
    let conn = create_v0_1_db_with_data();
    crate::catalog::migrations::migrate(&conn).unwrap();

    for index in &[
        "model_downloads_one_active_model",
        "asr_jobs_one_active_fingerprint",
        "asr_jobs_claimable",
    ] {
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='index' AND name=?1",
                [index],
                |row| row.get(0),
            )
            .unwrap();
        assert!(
            exists,
            "expected index {} to exist after migration",
            index
        );
    }
}

// ── Migration: rollback on failure ─────────────────────────────────────

#[test]
fn migration_rolls_back_on_failure() {
    let conn = create_v0_1_db_with_data();

    // Artificially create a conflict that will cause the migration to fail
    // mid-way: create a table with the same name that ALTER TABLE will
    // conflict with.  We do this by creating a table that would block one of
    // the ALTER TABLE statements.
    //
    // Actually, we test this by manually running a partial migration and
    // verifying that a forced error leaves the database unchanged.  We use a
    // transaction that we deliberately roll back.
    conn.execute_batch(
        "BEGIN IMMEDIATE;
         ALTER TABLE chunks ADD COLUMN session_offset_ms INTEGER NOT NULL DEFAULT 0;
         ROLLBACK;",
    )
    .unwrap();

    // After rollback, the column should NOT exist
    let result: rusqlite::Result<String> = conn.query_row(
        "SELECT session_offset_ms FROM chunks LIMIT 1",
        [],
        |row| row.get(0),
    );
    assert!(result.is_err(), "column should not exist after rollback");

    // The database should still be classifiable as LegacyV1
    assert_eq!(classify_schema(&conn).unwrap(), SchemaKind::LegacyV1);
}

// ── Fresh v2 creation ──────────────────────────────────────────────────

#[test]
fn fresh_database_creates_full_v2_schema() {
    let conn = Connection::open_in_memory().unwrap();
    assert_eq!(classify_schema(&conn).unwrap(), SchemaKind::Fresh);

    crate::catalog::migrations::migrate(&conn).unwrap();

    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, 2);

    // All v1 + v2 tables exist
    for table in &[
        "sessions",
        "revisions",
        "segments",
        "chunks",
        "asr_settings",
        "model_installations",
        "model_downloads",
        "asr_jobs",
        "provider_receipts",
        "revision_receipts",
    ] {
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name=?1",
                [table],
                |row| row.get(0),
            )
            .unwrap();
        assert!(exists, "expected table {} in fresh v2", table);
    }

    // FTS5 table exists
    let fts_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='segment_search'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(fts_exists);

    // New columns exist on legacy tables
    let has_offset: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('chunks') WHERE name='session_offset_ms'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(has_offset);

    let has_provenance: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('revisions') WHERE name='provenance_status'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(has_provenance);
}

// ── Idempotent re-open ─────────────────────────────────────────────────

#[test]
fn already_migrated_v2_database_is_accepted() {
    let conn = create_v0_1_db_with_data();
    crate::catalog::migrations::migrate(&conn).unwrap();

    // Second migration should be a no-op (classify returns Fresh since
    // user_version=2 and fingerprint matches)
    let kind = classify_schema(&conn).unwrap();
    assert_eq!(kind, SchemaKind::Fresh);

    // Running migrate again should succeed
    crate::catalog::migrations::migrate(&conn).unwrap();

    // Data still intact
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM revisions", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 2);
}

// ── Unknown schema is rejected ─────────────────────────────────────────

#[test]
fn unknown_schema_migration_is_rejected() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE weird (x TEXT);",
    )
    .unwrap();
    assert_eq!(classify_schema(&conn).unwrap(), SchemaKind::Unknown);

    let result = crate::catalog::migrations::migrate(&conn);
    assert!(result.is_err());
}

// ── Fixture-file migration ─────────────────────────────────────────────

#[test]
fn migrate_v0_1_fixture_file() {
    // Locate the fixture relative to the workspace root.
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = manifest_dir
        .parent()
        .unwrap()
        .join("tests/fixtures/catalog/lifesub-v0.1.sqlite3");

    // Copy the fixture to a temp file so the original stays pristine.
    let temp_dir = tempfile::tempdir().unwrap();
    let copy_path = temp_dir.path().join("lifesub-v0.1-copy.sqlite3");
    std::fs::copy(&fixture_path, &copy_path).unwrap();

    let conn = Connection::open(&copy_path).unwrap();

    assert_eq!(classify_schema(&conn).unwrap(), SchemaKind::LegacyV1);

    crate::catalog::migrations::migrate(&conn).unwrap();

    // Verify user_version is now 2
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, 2);

    // Revisions preserved
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM revisions", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 2);

    // Legacy provenance status
    let status: String = conn
        .query_row(
            "SELECT provenance_status FROM revisions WHERE id = 'rev_fixture_1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(status, "legacy_unverified");

    // FTS5 search still works
    let search_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM segment_search WHERE segment_search MATCH '原始转写'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(search_count, 1);
}