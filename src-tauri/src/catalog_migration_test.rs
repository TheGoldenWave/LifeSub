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
