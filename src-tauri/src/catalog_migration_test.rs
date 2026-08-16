use std::collections::BTreeSet;
use std::fs;
use std::time::Duration;
use std::{sync::mpsc, thread};

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
const V2_FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../tests/fixtures/catalog/lifesub-v0.2.sqlite3"
);
const V3_FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../tests/fixtures/catalog/lifesub-v0.3.sqlite3"
);
const V2_FIXTURE_SHA256: &str = "e2956f8a5c0531e8b444519c8e11e2de5952f6b4b4ec391c3321e9f60e6e4639";
const V3_FIXTURE_SHA256: &str = "79f8ec380b1555691e9bc4fd79bd743213b275270d35a61e791c0f278d970de2";

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

#[test]
fn fresh_catalog_uses_v3_model_install_contract() {
    let mut connection = Connection::open_in_memory().unwrap();

    migrations::migrate(&mut connection).unwrap();

    assert_eq!(user_version(&connection), 3);
    assert_columns(
        &connection,
        "model_download_artifacts",
        &[
            ("download_id", "TEXT", true, None, 1),
            ("artifact_id", "TEXT", true, None, 2),
            ("source_repository", "TEXT", true, None, 0),
            ("source_model", "TEXT", true, None, 0),
            ("source_url", "TEXT", true, None, 0),
            ("source_revision", "TEXT", true, None, 0),
            ("expected_bytes", "INTEGER", true, None, 0),
            ("downloaded_bytes", "INTEGER", true, Some("0"), 0),
            ("expected_sha256", "TEXT", true, None, 0),
            ("verified_sha256", "TEXT", false, None, 0),
            ("required_path", "TEXT", true, None, 0),
            ("temp_path", "TEXT", false, None, 0),
            ("etag", "TEXT", false, None, 0),
            ("last_modified", "TEXT", false, None, 0),
            ("checkpointed_at", "TEXT", false, None, 0),
            ("verified_at", "TEXT", false, None, 0),
            ("state", "TEXT", true, None, 0),
            ("error_code", "TEXT", false, None, 0),
            ("error_summary", "TEXT", false, None, 0),
            ("created_at", "TEXT", true, None, 0),
            ("updated_at", "TEXT", true, None, 0),
        ],
    );
    let installation_sql: String = connection
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE name = 'model_installations'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let normalized = migrations::normalize_sql_for_test(&installation_sql);
    assert!(normalized.contains("'installed_unqualified','runtime_qualified','deleting'"));
    assert!(normalized.contains("runtime_identity_json text"));
    assert!(normalized.contains("qualified_at text"));
}

#[test]
fn migrates_immutable_v2_without_blind_runtime_qualification() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("catalog.sqlite3");
    fs::copy(V2_FIXTURE, &path).unwrap();
    let mut connection = Connection::open(&path).unwrap();

    migrations::migrate(&mut connection).unwrap();

    assert_eq!(user_version(&connection), 3);
    let state: String = connection
        .query_row(
            "SELECT state FROM model_installations WHERE model_id = 'legacy-ready-model'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(state, "installed_unqualified");
    assert_eq!(
        migrations::classify(&mut connection).unwrap(),
        SchemaKind::CurrentV3
    );
}

#[test]
fn immutable_v3_fixture_reopens_idempotently() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("catalog.sqlite3");
    fs::copy(V3_FIXTURE, &path).unwrap();
    let before = fs::read(&path).unwrap();

    let mut connection = Connection::open(&path).unwrap();
    migrations::migrate(&mut connection).unwrap();
    assert_eq!(user_version(&connection), 3);
    assert_eq!(
        migrations::classify(&mut connection).unwrap(),
        SchemaKind::CurrentV3
    );
    drop(connection);

    assert_eq!(fs::read(path).unwrap(), before);
}

#[test]
fn catalog_fixtures_have_frozen_bytes() {
    use sha2::{Digest, Sha256};

    assert_eq!(
        hex::encode(Sha256::digest(fs::read(V2_FIXTURE).unwrap())),
        V2_FIXTURE_SHA256
    );
    assert_eq!(
        hex::encode(Sha256::digest(fs::read(V3_FIXTURE).unwrap())),
        V3_FIXTURE_SHA256
    );
}

#[test]
fn v2_to_v3_failure_rolls_back_original_bytes_and_version() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("catalog.sqlite3");
    fs::copy(V2_FIXTURE, &path).unwrap();
    let before = fs::read(&path).unwrap();
    let mut connection = Connection::open(&path).unwrap();

    let error = migrations::migrate_with_hook(&mut connection, || {
        Err(Error::SqliteFailure(
            ffi::Error::new(ffi::SQLITE_ABORT),
            Some("forced v3 failure".to_owned()),
        ))
    })
    .unwrap_err();

    assert!(error.to_string().contains("forced v3 failure"));
    assert_eq!(user_version(&connection), 2);
    drop(connection);
    assert_eq!(fs::read(path).unwrap(), before);
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

fn rewrite_schema_literal(connection: &Connection, table: &str, from: &str, to: &str) {
    connection
        .pragma_update(None, "writable_schema", true)
        .unwrap();
    let changed = connection
        .execute(
            "UPDATE sqlite_schema SET sql = replace(sql, ?2, ?3) WHERE name = ?1",
            [table, from, to],
        )
        .unwrap();
    connection
        .pragma_update(None, "writable_schema", false)
        .unwrap();
    assert_eq!(changed, 1);
}

mod classification;
mod contract;
mod rollback_concurrency;
