use std::collections::BTreeSet;
use std::fs;
use std::time::Duration;

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

type ColumnContract<'a> = (&'a str, &'a str, bool, Option<&'a str>, i64);
type ForeignKeyContract = (String, String, String, String, String, String);
type UniqueContract = (String, bool, Vec<String>);
type SegmentSnapshot = (
    String,
    String,
    i64,
    i64,
    String,
    String,
    Option<String>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
);
type ChunkSnapshot = (
    String,
    String,
    String,
    String,
    String,
    i64,
    i64,
    Option<i64>,
    String,
    Option<String>,
    Option<String>,
);

fn fixture_copy() -> (TempDir, std::path::PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("catalog.sqlite3");
    fs::copy(FIXTURE, &path).unwrap();
    (directory, path)
}

fn fresh_file_connection() -> (TempDir, std::path::PathBuf, Connection) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("catalog.sqlite3");
    let mut connection = Connection::open(&path).unwrap();
    migrations::migrate(&mut connection).unwrap();
    (directory, path, connection)
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

fn assert_columns(connection: &Connection, table: &str, expected: &[ColumnContract<'_>]) {
    let mut statement = connection
        .prepare(&format!("PRAGMA table_info('{table}')"))
        .unwrap();
    let actual = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)? != 0,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let expected = expected
        .iter()
        .map(|column| {
            (
                column.0.to_owned(),
                column.1.to_owned(),
                column.2,
                column.3.map(str::to_owned),
                column.4,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected, "column contract changed for {table}");
}

fn foreign_keys(connection: &Connection, table: &str) -> BTreeSet<ForeignKeyContract> {
    let mut statement = connection
        .prepare(&format!("PRAGMA foreign_key_list('{table}')"))
        .unwrap();
    statement
        .query_map([], |row| {
            Ok((
                row.get(3)?,
                row.get(2)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
            ))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn fk(from: &str, table: &str, to: &str) -> ForeignKeyContract {
    (
        from.to_owned(),
        table.to_owned(),
        to.to_owned(),
        "NO ACTION".to_owned(),
        "NO ACTION".to_owned(),
        "NONE".to_owned(),
    )
}

fn unique_contracts(connection: &Connection, table: &str) -> BTreeSet<UniqueContract> {
    let mut indexes = connection
        .prepare(&format!("PRAGMA index_list('{table}')"))
        .unwrap();
    indexes
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)? != 0,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)? != 0,
            ))
        })
        .unwrap()
        .map(|index| {
            let (name, unique, origin, partial) = index.unwrap();
            let mut columns = connection
                .prepare(&format!("PRAGMA index_info('{name}')"))
                .unwrap();
            let columns = columns
                .query_map([], |row| row.get(2))
                .unwrap()
                .collect::<Result<Vec<String>, _>>()
                .unwrap();
            (origin, unique && partial, columns)
        })
        .filter(|(origin, partial_unique, _)| origin != "c" || *partial_unique)
        .collect()
}

fn compact_sql(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_ascii_whitespace())
        .collect()
}

fn schema_sql(connection: &Connection, name: &str) -> String {
    connection
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE name = ?1",
            [name],
            |row| row.get(0),
        )
        .unwrap()
}

#[test]
fn classifies_fresh_legacy_and_current_catalogs() {
    let mut fresh = Connection::open_in_memory().unwrap();
    assert_eq!(migrations::classify(&mut fresh).unwrap(), SchemaKind::Fresh);

    let (_directory, legacy_path) = fixture_copy();
    let mut legacy = Connection::open(legacy_path).unwrap();
    assert_eq!(user_version(&legacy), 0);
    assert_eq!(
        migrations::classify(&mut legacy).unwrap(),
        SchemaKind::LegacyV1
    );

    let mut current = Connection::open_in_memory().unwrap();
    migrations::migrate(&mut current).unwrap();
    assert_eq!(
        migrations::classify(&mut current).unwrap(),
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
        migrations::classify(&mut connection).unwrap(),
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
        migrations::classify(&mut connection).unwrap(),
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
    let mut connection = Connection::open_in_memory().unwrap();
    connection.execute_batch(
        "CREATE TABLE sessions (id TEXT PRIMARY KEY, title TEXT NOT NULL, state TEXT NOT NULL, started_at TEXT NOT NULL, ended_at TEXT);
         CREATE TABLE revisions (id TEXT PRIMARY KEY, session_id TEXT NOT NULL, number INTEGER NOT NULL, provider TEXT NOT NULL, created_at TEXT NOT NULL, UNIQUE(session_id, number), FOREIGN KEY(session_id) REFERENCES sessions(id));
         CREATE TABLE segments (id TEXT NOT NULL, revision_id TEXT NOT NULL, start_ms INTEGER NOT NULL, end_ms INTEGER NOT NULL, source TEXT NOT NULL, text TEXT NOT NULL, PRIMARY KEY(id, revision_id), FOREIGN KEY(revision_id) REFERENCES revisions(id));
         CREATE TABLE chunks (id TEXT PRIMARY KEY, session_id TEXT NOT NULL, source TEXT NOT NULL, path TEXT NOT NULL, sha256 TEXT NOT NULL, byte_length INTEGER NOT NULL, FOREIGN KEY(session_id) REFERENCES sessions(id));
         CREATE VIRTUAL TABLE segment_search USING fts5(segment_id UNINDEXED, revision_id UNINDEXED, text, tokenize='unicode61');",
    ).unwrap();

    assert_eq!(
        migrations::classify(&mut connection).unwrap(),
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

    let mut connection = Connection::open(path).unwrap();
    assert_eq!(user_version(&connection), 2);
    assert_eq!(
        migrations::classify(&mut connection).unwrap(),
        SchemaKind::CurrentV2
    );
    let session: (String, String, String, String, Option<String>) = connection
        .query_row(
            "SELECT id, title, state, started_at, ended_at FROM sessions",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .unwrap();
    let revision: (String, String, i64, String, String, String) = connection
        .query_row(
            "SELECT id, session_id, number, provider, created_at, provenance_status FROM revisions",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .unwrap();
    let segment: SegmentSnapshot = connection
        .query_row(
            "SELECT id, revision_id, start_ms, end_ms, source, text, chunk_id,
                chunk_start_ms, chunk_end_ms, session_start_ms, session_end_ms FROM segments",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                    row.get(10)?,
                ))
            },
        )
        .unwrap();
    let chunk: ChunkSnapshot = connection
        .query_row(
            "SELECT id, session_id, source, path, sha256, byte_length, session_offset_ms,
                duration_ms, integrity_state, last_error_code, last_error_at FROM chunks",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                    row.get(10)?,
                ))
            },
        )
        .unwrap();
    let receipt_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM provider_receipts", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(
        session,
        (
            "session_fixture_01".to_owned(),
            "Migration fixture".to_owned(),
            "stopped".to_owned(),
            "2026-01-01T00:00:00+00:00".to_owned(),
            Some("2026-01-01T00:01:00+00:00".to_owned()),
        )
    );
    assert_eq!(
        revision,
        (
            "revision_fixture_01".to_owned(),
            "session_fixture_01".to_owned(),
            1,
            "demo-local".to_owned(),
            "2026-01-01T00:01:00+00:00".to_owned(),
            "legacy_unverified".to_owned(),
        )
    );
    assert_eq!(
        segment,
        (
            "segment_fixture_01".to_owned(),
            "revision_fixture_01".to_owned(),
            1250,
            4750,
            "imported".to_owned(),
            "migration fixture searchable transcript".to_owned(),
            None,
            None,
            None,
            None,
            None,
        )
    );
    assert_eq!(
        chunk,
        (
            "chunk_fixture_01".to_owned(),
            "session_fixture_01".to_owned(),
            "imported".to_owned(),
            "fixtures/audio/sample.wav".to_owned(),
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
            4096,
            0,
            None,
            "available".to_owned(),
            None,
            None,
        )
    );
    assert_eq!(receipt_count, 0);
    for id in [&session.0, &revision.0, &segment.0, &chunk.0] {
        assert!(!id.contains(['/', '?', '#']));
    }
    assert_eq!(
        format!("lifesub://record/{}", session.0),
        "lifesub://record/session_fixture_01"
    );
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

    assert_columns(
        &connection,
        "sessions",
        &[
            ("id", "TEXT", false, None, 1),
            ("title", "TEXT", true, None, 0),
            ("state", "TEXT", true, None, 0),
            ("started_at", "TEXT", true, None, 0),
            ("ended_at", "TEXT", false, None, 0),
        ],
    );
    assert_columns(
        &connection,
        "revisions",
        &[
            ("id", "TEXT", false, None, 1),
            ("session_id", "TEXT", true, None, 0),
            ("number", "INTEGER", true, None, 0),
            ("provider", "TEXT", true, None, 0),
            ("created_at", "TEXT", true, None, 0),
            (
                "provenance_status",
                "TEXT",
                true,
                Some("'legacy_unverified'"),
                0,
            ),
        ],
    );
    assert_columns(
        &connection,
        "segments",
        &[
            ("id", "TEXT", true, None, 1),
            ("revision_id", "TEXT", true, None, 2),
            ("start_ms", "INTEGER", true, None, 0),
            ("end_ms", "INTEGER", true, None, 0),
            ("source", "TEXT", true, None, 0),
            ("text", "TEXT", true, None, 0),
            ("chunk_id", "TEXT", false, None, 0),
            ("chunk_start_ms", "INTEGER", false, None, 0),
            ("chunk_end_ms", "INTEGER", false, None, 0),
            ("session_start_ms", "INTEGER", false, None, 0),
            ("session_end_ms", "INTEGER", false, None, 0),
        ],
    );
    assert_columns(
        &connection,
        "chunks",
        &[
            ("id", "TEXT", false, None, 1),
            ("session_id", "TEXT", true, None, 0),
            ("source", "TEXT", true, None, 0),
            ("path", "TEXT", true, None, 0),
            ("sha256", "TEXT", true, None, 0),
            ("byte_length", "INTEGER", true, None, 0),
            ("session_offset_ms", "INTEGER", true, Some("0"), 0),
            ("duration_ms", "INTEGER", false, None, 0),
            ("integrity_state", "TEXT", true, Some("'available'"), 0),
            ("last_error_code", "TEXT", false, None, 0),
            ("last_error_at", "TEXT", false, None, 0),
        ],
    );
    assert_columns(
        &connection,
        "asr_settings",
        &[
            ("singleton_id", "INTEGER", false, None, 1),
            ("provider", "TEXT", true, None, 0),
            ("model_id", "TEXT", true, None, 0),
            ("language", "TEXT", true, None, 0),
            ("num_threads", "INTEGER", true, None, 0),
            ("vad_enabled", "INTEGER", true, None, 0),
            ("auto_transcribe_imports", "INTEGER", true, None, 0),
            ("provider_options_json", "TEXT", true, None, 0),
            ("updated_at", "TEXT", true, None, 0),
        ],
    );
    assert_columns(
        &connection,
        "model_installations",
        &[
            ("model_id", "TEXT", false, None, 1),
            ("provider", "TEXT", true, None, 0),
            ("manifest_version", "TEXT", true, None, 0),
            ("archive_sha256", "TEXT", true, None, 0),
            ("install_dir", "TEXT", true, None, 0),
            ("state", "TEXT", true, None, 0),
            ("installed_at", "TEXT", true, None, 0),
            ("last_error_code", "TEXT", false, None, 0),
        ],
    );
    assert_columns(
        &connection,
        "model_downloads",
        &[
            ("id", "TEXT", false, None, 1),
            ("model_id", "TEXT", true, None, 0),
            ("manifest_version", "TEXT", true, None, 0),
            ("archive_sha256", "TEXT", true, None, 0),
            ("state", "TEXT", true, None, 0),
            ("downloaded_bytes", "INTEGER", true, Some("0"), 0),
            ("expected_bytes", "INTEGER", true, None, 0),
            ("temp_path", "TEXT", false, None, 0),
            ("error_code", "TEXT", false, None, 0),
            ("error_summary", "TEXT", false, None, 0),
            ("created_at", "TEXT", true, None, 0),
            ("updated_at", "TEXT", true, None, 0),
        ],
    );
    assert_columns(
        &connection,
        "asr_jobs",
        &[
            ("id", "TEXT", false, None, 1),
            ("session_id", "TEXT", true, None, 0),
            ("chunk_id", "TEXT", true, None, 0),
            ("provider", "TEXT", true, None, 0),
            ("model_id", "TEXT", true, None, 0),
            ("manifest_version", "TEXT", true, None, 0),
            ("archive_sha256", "TEXT", true, None, 0),
            ("required_file_hashes_json", "TEXT", true, None, 0),
            ("model_source_json", "TEXT", true, None, 0),
            ("vad_model_id", "TEXT", false, None, 0),
            ("vad_manifest_version", "TEXT", false, None, 0),
            ("vad_archive_sha256", "TEXT", false, None, 0),
            ("vad_required_file_hashes_json", "TEXT", false, None, 0),
            ("parameters_json", "TEXT", true, None, 0),
            ("input_sha256", "TEXT", true, None, 0),
            ("fingerprint", "TEXT", true, None, 0),
            ("state", "TEXT", true, None, 0),
            ("attempt_count", "INTEGER", true, Some("0"), 0),
            ("claim_generation", "INTEGER", true, Some("0"), 0),
            ("max_attempts", "INTEGER", true, Some("3"), 0),
            ("available_at", "TEXT", true, None, 0),
            ("claimed_by", "TEXT", false, None, 0),
            ("lease_expires_at", "TEXT", false, None, 0),
            ("cancel_requested_at", "TEXT", false, None, 0),
            ("error_code", "TEXT", false, None, 0),
            ("error_summary", "TEXT", false, None, 0),
            ("created_at", "TEXT", true, None, 0),
            ("updated_at", "TEXT", true, None, 0),
        ],
    );
    assert_columns(
        &connection,
        "provider_receipts",
        &[
            ("id", "TEXT", false, None, 1),
            ("job_id", "TEXT", true, None, 0),
            ("chunk_id", "TEXT", true, None, 0),
            ("provider", "TEXT", true, None, 0),
            ("model_id", "TEXT", true, None, 0),
            ("manifest_version", "TEXT", true, None, 0),
            ("archive_sha256", "TEXT", true, None, 0),
            ("required_file_hashes_json", "TEXT", true, None, 0),
            ("model_source_json", "TEXT", true, None, 0),
            ("vad_model_id", "TEXT", false, None, 0),
            ("vad_manifest_version", "TEXT", false, None, 0),
            ("vad_archive_sha256", "TEXT", false, None, 0),
            ("vad_required_file_hashes_json", "TEXT", false, None, 0),
            ("runtime_version", "TEXT", true, None, 0),
            ("runtime_build_id", "TEXT", true, None, 0),
            ("parameters_json", "TEXT", true, None, 0),
            ("input_sha256", "TEXT", true, None, 0),
            ("started_at", "TEXT", true, None, 0),
            ("finished_at", "TEXT", true, None, 0),
            ("data_destination", "TEXT", true, None, 0),
            ("outcome", "TEXT", true, None, 0),
        ],
    );
    assert_columns(
        &connection,
        "revision_receipts",
        &[
            ("revision_id", "TEXT", true, None, 1),
            ("receipt_id", "TEXT", true, None, 2),
        ],
    );
    assert_columns(
        &connection,
        "segment_search",
        &[
            ("segment_id", "", false, None, 0),
            ("revision_id", "", false, None, 0),
            ("text", "", false, None, 0),
        ],
    );
    assert_columns(
        &connection,
        "segment_search_config",
        &[("k", "", true, None, 1), ("v", "", false, None, 0)],
    );
    assert_columns(
        &connection,
        "segment_search_content",
        &[
            ("id", "INTEGER", false, None, 1),
            ("c0", "", false, None, 0),
            ("c1", "", false, None, 0),
            ("c2", "", false, None, 0),
        ],
    );
    assert_columns(
        &connection,
        "segment_search_data",
        &[
            ("id", "INTEGER", false, None, 1),
            ("block", "BLOB", false, None, 0),
        ],
    );
    assert_columns(
        &connection,
        "segment_search_docsize",
        &[
            ("id", "INTEGER", false, None, 1),
            ("sz", "BLOB", false, None, 0),
        ],
    );
    assert_columns(
        &connection,
        "segment_search_idx",
        &[
            ("segid", "", true, None, 1),
            ("term", "", true, None, 2),
            ("pgno", "", false, None, 0),
        ],
    );

    let no_fks = BTreeSet::new();
    for table in [
        "sessions",
        "asr_settings",
        "model_installations",
        "model_downloads",
        "segment_search",
        "segment_search_config",
        "segment_search_content",
        "segment_search_data",
        "segment_search_docsize",
        "segment_search_idx",
    ] {
        assert_eq!(
            foreign_keys(&connection, table),
            no_fks,
            "unexpected FK on {table}"
        );
    }
    assert_eq!(
        foreign_keys(&connection, "revisions"),
        [fk("session_id", "sessions", "id")].into_iter().collect()
    );
    assert_eq!(
        foreign_keys(&connection, "chunks"),
        [fk("session_id", "sessions", "id")].into_iter().collect()
    );
    assert_eq!(
        foreign_keys(&connection, "segments"),
        [
            fk("chunk_id", "chunks", "id"),
            fk("revision_id", "revisions", "id"),
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(
        foreign_keys(&connection, "asr_jobs"),
        [
            fk("chunk_id", "chunks", "id"),
            fk("session_id", "sessions", "id"),
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(
        foreign_keys(&connection, "provider_receipts"),
        [
            fk("chunk_id", "chunks", "id"),
            fk("job_id", "asr_jobs", "id"),
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(
        foreign_keys(&connection, "revision_receipts"),
        [
            fk("receipt_id", "provider_receipts", "id"),
            fk("revision_id", "revisions", "id"),
        ]
        .into_iter()
        .collect()
    );

    let contract = |origin: &str, partial: bool, columns: &[&str]| {
        (
            origin.to_owned(),
            partial,
            columns.iter().map(|value| (*value).to_owned()).collect(),
        )
    };
    assert_eq!(
        unique_contracts(&connection, "sessions"),
        [contract("pk", false, &["id"])].into_iter().collect()
    );
    assert_eq!(
        unique_contracts(&connection, "revisions"),
        [
            contract("pk", false, &["id"]),
            contract("u", false, &["session_id", "number"]),
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(
        unique_contracts(&connection, "segments"),
        [contract("pk", false, &["id", "revision_id"])]
            .into_iter()
            .collect()
    );
    assert_eq!(
        unique_contracts(&connection, "chunks"),
        [contract("pk", false, &["id"])].into_iter().collect()
    );
    assert!(unique_contracts(&connection, "asr_settings").is_empty());
    assert_eq!(
        unique_contracts(&connection, "segment_search_config"),
        [contract("pk", false, &["k"])].into_iter().collect()
    );
    assert_eq!(
        unique_contracts(&connection, "segment_search_idx"),
        [contract("pk", false, &["segid", "term"])]
            .into_iter()
            .collect()
    );
    assert_eq!(
        unique_contracts(&connection, "model_installations"),
        [
            contract("pk", false, &["model_id"]),
            contract("u", false, &["install_dir"]),
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(
        unique_contracts(&connection, "model_downloads"),
        [
            contract("pk", false, &["id"]),
            contract("c", true, &["model_id"]),
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(
        unique_contracts(&connection, "asr_jobs"),
        [
            contract("pk", false, &["id"]),
            contract("c", true, &["fingerprint"]),
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(
        unique_contracts(&connection, "provider_receipts"),
        [
            contract("pk", false, &["id"]),
            contract("u", false, &["job_id"]),
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(
        unique_contracts(&connection, "revision_receipts"),
        [
            contract("pk", false, &["revision_id", "receipt_id"]),
            contract("u", false, &["receipt_id"]),
        ]
        .into_iter()
        .collect()
    );

    for (name, expected) in [
        (
            "model_downloads_one_active_model",
            "CREATE UNIQUE INDEX model_downloads_one_active_model ON model_downloads(model_id) WHERE state IN ('queued', 'downloading', 'verifying', 'installing')",
        ),
        (
            "asr_jobs_one_active_fingerprint",
            "CREATE UNIQUE INDEX asr_jobs_one_active_fingerprint ON asr_jobs(fingerprint) WHERE state IN ('queued', 'blocked_model', 'preparing', 'transcribing')",
        ),
        (
            "asr_jobs_claimable",
            "CREATE INDEX asr_jobs_claimable ON asr_jobs(state, available_at, lease_expires_at)",
        ),
    ] {
        assert_eq!(
            compact_sql(&schema_sql(&connection, name)),
            compact_sql(expected)
        );
    }

    for (table, checks) in [
        (
            "revisions",
            &["CHECK(provenance_status IN ('legacy_unverified', 'verified_local_asr', 'manual'))"]
                [..],
        ),
        (
            "chunks",
            &["CHECK(integrity_state IN ('available', 'corrupted', 'missing'))"][..],
        ),
        (
            "asr_settings",
            &[
                "CHECK(singleton_id = 1)",
                "CHECK(provider IN ('sense_voice', 'whisper'))",
                "CHECK(num_threads >= 1)",
                "CHECK(vad_enabled IN (0, 1))",
                "CHECK(auto_transcribe_imports IN (0, 1))",
            ][..],
        ),
        (
            "model_installations",
            &[
                "CHECK(provider IN ('sense_voice', 'whisper', 'vad'))",
                "CHECK(state IN ('ready', 'corrupt', 'deleting'))",
            ][..],
        ),
        (
            "model_downloads",
            &[
                "CHECK(state IN ('queued', 'downloading', 'verifying', 'installing', 'succeeded', 'failed', 'cancelled'))",
            ][..],
        ),
        (
            "asr_jobs",
            &[
                "CHECK(provider IN ('sense_voice', 'whisper'))",
                "CHECK(state IN ('queued', 'blocked_model', 'preparing', 'transcribing', 'succeeded', 'failed', 'cancelled'))",
                "CHECK(max_attempts BETWEEN 1 AND 10)",
            ][..],
        ),
        (
            "provider_receipts",
            &[
                "CHECK(data_destination = 'local_device')",
                "CHECK(outcome = 'succeeded')",
            ][..],
        ),
    ] {
        let sql = compact_sql(&schema_sql(&connection, table));
        for expected in checks {
            assert!(
                sql.contains(&compact_sql(expected)),
                "missing {expected} on {table}"
            );
        }
    }
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
fn migration_holds_immediate_lock_from_classification_through_commit() {
    let (_directory, path) = fixture_copy();
    let mut migration_connection = Connection::open(&path).unwrap();
    let racing_connection = Connection::open(&path).unwrap();
    racing_connection.busy_timeout(Duration::ZERO).unwrap();

    migrations::migrate_with_classification_hook(&mut migration_connection, || {
        let error = racing_connection
            .execute("CREATE TABLE raced_schema_change(id TEXT)", [])
            .unwrap_err();
        match error {
            Error::SqliteFailure(details, _) => assert!(matches!(
                details.code,
                rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
            )),
            other => panic!("unexpected racing schema error: {other:?}"),
        }
        Ok(())
    })
    .unwrap();

    let raced: i64 = migration_connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_schema WHERE name = 'raced_schema_change'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(raced, 0);
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

#[test]
fn rejects_malformed_v1_fts_shadow_tables() {
    for shadow in [
        "segment_search_config",
        "segment_search_content",
        "segment_search_data",
        "segment_search_docsize",
        "segment_search_idx",
    ] {
        let (_directory, path) = fixture_copy();
        let mut connection = Connection::open(path).unwrap();
        connection
            .execute_batch(&format!(
                "DROP TABLE {shadow}; CREATE TABLE {shadow}(malformed TEXT);"
            ))
            .unwrap();

        assert_eq!(
            migrations::classify(&mut connection).unwrap(),
            SchemaKind::Unknown,
            "accepted malformed {shadow}"
        );
    }
}

#[test]
fn rejects_v2_base_table_with_unapproved_unique_title() {
    let (_directory, path, connection) = fresh_file_connection();
    connection
        .execute_batch(
            "DROP TABLE sessions;
         CREATE TABLE sessions (
           id TEXT PRIMARY KEY, title TEXT NOT NULL UNIQUE, state TEXT NOT NULL,
           started_at TEXT NOT NULL, ended_at TEXT
         );",
        )
        .unwrap();
    drop(connection);

    let mut reopened = Connection::open(path).unwrap();
    let error = migrations::migrate(&mut reopened).unwrap_err();
    assert!(error.to_string().contains("corrupt v2 catalog schema"));
}

#[test]
fn rejects_v2_base_table_with_changed_foreign_key_target_and_action() {
    let (_directory, path, connection) = fresh_file_connection();
    connection
        .execute_batch(
            "DROP TABLE chunks;
         CREATE TABLE chunks (
           id TEXT PRIMARY KEY, session_id TEXT NOT NULL, source TEXT NOT NULL,
           path TEXT NOT NULL, sha256 TEXT NOT NULL, byte_length INTEGER NOT NULL,
           session_offset_ms INTEGER NOT NULL DEFAULT 0, duration_ms INTEGER,
           integrity_state TEXT NOT NULL DEFAULT 'available'
             CHECK(integrity_state IN ('available', 'corrupted', 'missing')),
           last_error_code TEXT, last_error_at TEXT,
           FOREIGN KEY(session_id) REFERENCES revisions(id) ON DELETE CASCADE
         );",
        )
        .unwrap();
    drop(connection);

    let mut reopened = Connection::open(path).unwrap();
    let error = migrations::migrate(&mut reopened).unwrap_err();
    assert!(error.to_string().contains("corrupt v2 catalog schema"));
}

#[test]
fn rejects_v2_base_table_with_extra_check() {
    let (_directory, path, connection) = fresh_file_connection();
    connection
        .execute_batch(
            "DROP TABLE revisions;
         CREATE TABLE revisions (
           id TEXT PRIMARY KEY, session_id TEXT NOT NULL, number INTEGER NOT NULL,
           provider TEXT NOT NULL CHECK(provider <> ''), created_at TEXT NOT NULL,
           provenance_status TEXT NOT NULL DEFAULT 'legacy_unverified'
             CHECK(provenance_status IN ('legacy_unverified', 'verified_local_asr', 'manual')),
           UNIQUE(session_id, number), FOREIGN KEY(session_id) REFERENCES sessions(id)
         );",
        )
        .unwrap();
    drop(connection);

    let mut reopened = Connection::open(path).unwrap();
    let error = migrations::migrate(&mut reopened).unwrap_err();
    assert!(error.to_string().contains("corrupt v2 catalog schema"));
}

#[test]
fn rejects_v2_base_table_with_extra_autoindex_constraint() {
    let (_directory, path, connection) = fresh_file_connection();
    connection
        .execute_batch(
            "DROP TABLE segments;
         CREATE TABLE segments (
           id TEXT NOT NULL, revision_id TEXT NOT NULL, start_ms INTEGER NOT NULL,
           end_ms INTEGER NOT NULL, source TEXT NOT NULL, text TEXT NOT NULL UNIQUE,
           chunk_id TEXT REFERENCES chunks(id), chunk_start_ms INTEGER, chunk_end_ms INTEGER,
           session_start_ms INTEGER, session_end_ms INTEGER,
           PRIMARY KEY(id, revision_id), FOREIGN KEY(revision_id) REFERENCES revisions(id)
         );",
        )
        .unwrap();
    drop(connection);

    let mut reopened = Connection::open(path).unwrap();
    let error = migrations::migrate(&mut reopened).unwrap_err();
    assert!(error.to_string().contains("corrupt v2 catalog schema"));
}
