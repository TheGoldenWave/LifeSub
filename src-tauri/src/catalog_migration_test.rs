use std::fs;

use rusqlite::{Connection, Error, ffi};
use tempfile::TempDir;

use crate::catalog::{
    Catalog,
    migrations::{self, SchemaKind},
};

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../tests/fixtures/catalog/lifesub-v0.1.sqlite3"
);

fn fixture_copy() -> (TempDir, std::path::PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("catalog.sqlite3");
    fs::copy(FIXTURE, &path).unwrap();
    (directory, path)
}

fn user_version(connection: &Connection) -> i64 {
    connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap()
}

fn user_objects(connection: &Connection) -> Vec<String> {
    let mut statement = connection
        .prepare(
            "SELECT name FROM sqlite_schema
             WHERE name NOT LIKE 'sqlite_%' AND name NOT LIKE 'segment_search_%'
             ORDER BY name",
        )
        .unwrap();
    statement
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

#[test]
fn classifies_fresh_legacy_and_current_catalogs() {
    let fresh = Connection::open_in_memory().unwrap();
    assert_eq!(migrations::classify(&fresh).unwrap(), SchemaKind::Fresh);

    let (_directory, legacy_path) = fixture_copy();
    let legacy = Connection::open(legacy_path).unwrap();
    assert_eq!(user_version(&legacy), 0);
    assert_eq!(migrations::classify(&legacy).unwrap(), SchemaKind::LegacyV1);

    let mut current = Connection::open_in_memory().unwrap();
    migrations::migrate(&mut current).unwrap();
    assert_eq!(
        migrations::classify(&current).unwrap(),
        SchemaKind::CurrentV2
    );
}

#[test]
fn rejects_unknown_version_zero_schema_without_changing_it() {
    let mut connection = Connection::open_in_memory().unwrap();
    connection
        .execute_batch(
            "CREATE TABLE sessions(id TEXT PRIMARY KEY); CREATE TABLE surprise(id TEXT);",
        )
        .unwrap();
    let before = user_objects(&connection);

    assert_eq!(
        migrations::classify(&connection).unwrap(),
        SchemaKind::Unknown
    );
    let error = migrations::migrate(&mut connection).unwrap_err();

    assert!(
        error
            .to_string()
            .contains("unknown or corrupt catalog schema")
    );
    assert_eq!(user_version(&connection), 0);
    assert_eq!(user_objects(&connection), before);
}

#[test]
fn rejects_version_zero_database_with_unknown_view() {
    let mut connection = Connection::open_in_memory().unwrap();
    connection
        .execute("CREATE VIEW unknown_catalog_view AS SELECT 1 AS id", [])
        .unwrap();

    assert_eq!(
        migrations::classify(&connection).unwrap(),
        SchemaKind::Unknown
    );
    let error = migrations::migrate(&mut connection).unwrap_err();

    assert!(
        error
            .to_string()
            .contains("unknown or corrupt catalog schema")
    );
    assert_eq!(user_version(&connection), 0);
}

#[test]
fn rejects_v1_lookalike_with_wrong_fts_tokenizer() {
    let connection = Connection::open_in_memory().unwrap();
    connection.execute_batch(
        "CREATE TABLE sessions (id TEXT PRIMARY KEY, title TEXT NOT NULL, state TEXT NOT NULL, started_at TEXT NOT NULL, ended_at TEXT);
         CREATE TABLE revisions (id TEXT PRIMARY KEY, session_id TEXT NOT NULL, number INTEGER NOT NULL, provider TEXT NOT NULL, created_at TEXT NOT NULL, UNIQUE(session_id, number), FOREIGN KEY(session_id) REFERENCES sessions(id));
         CREATE TABLE segments (id TEXT NOT NULL, revision_id TEXT NOT NULL, start_ms INTEGER NOT NULL, end_ms INTEGER NOT NULL, source TEXT NOT NULL, text TEXT NOT NULL, PRIMARY KEY(id, revision_id), FOREIGN KEY(revision_id) REFERENCES revisions(id));
         CREATE TABLE chunks (id TEXT PRIMARY KEY, session_id TEXT NOT NULL, source TEXT NOT NULL, path TEXT NOT NULL, sha256 TEXT NOT NULL, byte_length INTEGER NOT NULL, FOREIGN KEY(session_id) REFERENCES sessions(id));
         CREATE VIRTUAL TABLE segment_search USING fts5(segment_id UNINDEXED, revision_id UNINDEXED, text, tokenize='unicode61');",
    ).unwrap();

    assert_eq!(
        migrations::classify(&connection).unwrap(),
        SchemaKind::Unknown
    );
}

#[test]
fn migrates_real_v1_fixture_and_preserves_legacy_evidence() {
    let (_directory, path) = fixture_copy();
    let catalog = Catalog::open(&path).unwrap();

    let revisions = catalog.list_revisions("session_fixture_01").unwrap();
    let matches = catalog.search_segments("searchable").unwrap();
    assert_eq!(revisions.len(), 1);
    assert_eq!(revisions[0].provider, "demo-local");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].start_ms, 1250);
    assert_eq!(matches[0].end_ms, 4750);
    drop(catalog);

    let connection = Connection::open(path).unwrap();
    assert_eq!(user_version(&connection), 2);
    assert_eq!(
        migrations::classify(&connection).unwrap(),
        SchemaKind::CurrentV2
    );
    let provenance: String = connection
        .query_row(
            "SELECT provenance_status FROM revisions WHERE id = 'revision_fixture_01'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let receipt_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM provider_receipts", [], |row| {
            row.get(0)
        })
        .unwrap();
    let chunk: (i64, Option<i64>, String) = connection.query_row(
        "SELECT session_offset_ms, duration_ms, integrity_state FROM chunks WHERE id = 'chunk_fixture_01'",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).unwrap();
    assert_eq!(provenance, "legacy_unverified");
    assert_eq!(receipt_count, 0);
    assert_eq!(chunk, (0, None, "available".to_owned()));
}

#[test]
fn creates_complete_v2_schema_for_fresh_catalog() {
    let mut connection = Connection::open_in_memory().unwrap();
    migrations::migrate(&mut connection).unwrap();

    assert_eq!(user_version(&connection), 2);
    for table in [
        "asr_settings",
        "model_installations",
        "model_downloads",
        "asr_jobs",
        "provider_receipts",
        "revision_receipts",
    ] {
        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_schema WHERE type = 'table' AND name = ?1",
                [table],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "missing {table}");
    }
    for index in [
        "model_downloads_one_active_model",
        "asr_jobs_one_active_fingerprint",
        "asr_jobs_claimable",
    ] {
        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_schema WHERE type = 'index' AND name = ?1",
                [index],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "missing {index}");
    }
    let generation: i64 = connection.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('asr_jobs') WHERE name = 'claim_generation' AND \"notnull\" = 1",
        [],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(generation, 1);
}

#[test]
fn migration_failure_rolls_back_every_schema_change() {
    let (_directory, path) = fixture_copy();
    let mut connection = Connection::open(path).unwrap();
    let before = user_objects(&connection);

    let error = migrations::migrate_with_hook(&mut connection, || {
        Err(Error::SqliteFailure(
            ffi::Error::new(ffi::SQLITE_ABORT),
            Some("forced migration failure".to_owned()),
        ))
    })
    .unwrap_err();

    assert!(error.to_string().contains("forced migration failure"));
    assert_eq!(user_version(&connection), 0);
    assert_eq!(user_objects(&connection), before);
    let columns: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('revisions') WHERE name = 'provenance_status'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(columns, 0);
}

#[test]
fn rejects_future_and_corrupt_current_catalogs() {
    let mut future = Connection::open_in_memory().unwrap();
    future.pragma_update(None, "user_version", 3).unwrap();
    let error = migrations::migrate(&mut future).unwrap_err();
    assert!(error.to_string().contains("incompatible catalog version 3"));

    let mut current = Connection::open_in_memory().unwrap();
    migrations::migrate(&mut current).unwrap();
    current
        .execute("DROP INDEX asr_jobs_claimable", [])
        .unwrap();
    let error = migrations::migrate(&mut current).unwrap_err();
    assert!(error.to_string().contains("corrupt v2 catalog schema"));
}

#[test]
fn rejects_current_catalog_with_unapproved_index() {
    let mut connection = Connection::open_in_memory().unwrap();
    migrations::migrate(&mut connection).unwrap();
    connection
        .execute(
            "CREATE INDEX unapproved_sessions_title ON sessions(title)",
            [],
        )
        .unwrap();

    let error = migrations::migrate(&mut connection).unwrap_err();

    assert!(error.to_string().contains("corrupt v2 catalog schema"));
}

#[test]
fn rejects_current_catalog_with_missing_fts_shadow_table() {
    let mut connection = Connection::open_in_memory().unwrap();
    migrations::migrate(&mut connection).unwrap();
    connection
        .execute("DROP TABLE segment_search_docsize", [])
        .unwrap();

    let error = migrations::migrate(&mut connection).unwrap_err();

    assert!(error.to_string().contains("corrupt v2 catalog schema"));
}
