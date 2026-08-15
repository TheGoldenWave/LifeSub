use std::collections::BTreeSet;

use rusqlite::{Connection, Error, TransactionBehavior, ffi};

const CURRENT_VERSION: i64 = 2;
const FTS_TABLE: &str = "segment_search";
const FTS_SHADOWS: [&str; 5] = [
    "segment_search_config",
    "segment_search_content",
    "segment_search_data",
    "segment_search_docsize",
    "segment_search_idx",
];
const V1_TABLES: [&str; 5] = [
    "chunks",
    "revisions",
    "segment_search",
    "segments",
    "sessions",
];
const V2_TABLES: [&str; 11] = [
    "asr_jobs",
    "asr_settings",
    "chunks",
    "model_downloads",
    "model_installations",
    "provider_receipts",
    "revision_receipts",
    "revisions",
    "segment_search",
    "segments",
    "sessions",
];

const LEGACY_SCHEMA: &str = "
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
);";

const FRESH_BASE_SCHEMA: &str = "
CREATE TABLE sessions (
  id TEXT PRIMARY KEY, title TEXT NOT NULL, state TEXT NOT NULL,
  started_at TEXT NOT NULL, ended_at TEXT
);
CREATE TABLE revisions (
  id TEXT PRIMARY KEY, session_id TEXT NOT NULL, number INTEGER NOT NULL,
  provider TEXT NOT NULL, created_at TEXT NOT NULL,
  provenance_status TEXT NOT NULL DEFAULT 'legacy_unverified'
    CHECK(provenance_status IN ('legacy_unverified', 'verified_local_asr', 'manual')),
  UNIQUE(session_id, number), FOREIGN KEY(session_id) REFERENCES sessions(id)
);
CREATE TABLE segments (
  id TEXT NOT NULL, revision_id TEXT NOT NULL, start_ms INTEGER NOT NULL,
  end_ms INTEGER NOT NULL, source TEXT NOT NULL, text TEXT NOT NULL,
  chunk_id TEXT REFERENCES chunks(id), chunk_start_ms INTEGER, chunk_end_ms INTEGER,
  session_start_ms INTEGER, session_end_ms INTEGER,
  PRIMARY KEY(id, revision_id), FOREIGN KEY(revision_id) REFERENCES revisions(id)
);
CREATE TABLE chunks (
  id TEXT PRIMARY KEY, session_id TEXT NOT NULL, source TEXT NOT NULL,
  path TEXT NOT NULL, sha256 TEXT NOT NULL, byte_length INTEGER NOT NULL,
  session_offset_ms INTEGER NOT NULL DEFAULT 0, duration_ms INTEGER,
  integrity_state TEXT NOT NULL DEFAULT 'available'
    CHECK(integrity_state IN ('available', 'corrupted', 'missing')),
  last_error_code TEXT, last_error_at TEXT,
  FOREIGN KEY(session_id) REFERENCES sessions(id)
);
CREATE VIRTUAL TABLE segment_search USING fts5(
  segment_id UNINDEXED, revision_id UNINDEXED, text, tokenize='trigram'
);";

const LEGACY_ALTERS: &str = "
ALTER TABLE chunks ADD COLUMN session_offset_ms INTEGER NOT NULL DEFAULT 0;
ALTER TABLE chunks ADD COLUMN duration_ms INTEGER;
ALTER TABLE chunks ADD COLUMN integrity_state TEXT NOT NULL DEFAULT 'available'
  CHECK(integrity_state IN ('available', 'corrupted', 'missing'));
ALTER TABLE chunks ADD COLUMN last_error_code TEXT;
ALTER TABLE chunks ADD COLUMN last_error_at TEXT;
ALTER TABLE revisions ADD COLUMN provenance_status TEXT NOT NULL DEFAULT 'legacy_unverified'
  CHECK(provenance_status IN ('legacy_unverified', 'verified_local_asr', 'manual'));
ALTER TABLE segments ADD COLUMN chunk_id TEXT REFERENCES chunks(id);
ALTER TABLE segments ADD COLUMN chunk_start_ms INTEGER;
ALTER TABLE segments ADD COLUMN chunk_end_ms INTEGER;
ALTER TABLE segments ADD COLUMN session_start_ms INTEGER;
ALTER TABLE segments ADD COLUMN session_end_ms INTEGER;";

const ASR_SCHEMA: &str = "
CREATE TABLE asr_settings (
  singleton_id INTEGER PRIMARY KEY CHECK(singleton_id = 1),
  provider TEXT NOT NULL CHECK(provider IN ('sense_voice', 'whisper')),
  model_id TEXT NOT NULL,
  language TEXT NOT NULL,
  num_threads INTEGER NOT NULL CHECK(num_threads >= 1),
  vad_enabled INTEGER NOT NULL CHECK(vad_enabled IN (0, 1)),
  auto_transcribe_imports INTEGER NOT NULL CHECK(auto_transcribe_imports IN (0, 1)),
  provider_options_json TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE model_installations (
  model_id TEXT PRIMARY KEY,
  provider TEXT NOT NULL CHECK(provider IN ('sense_voice', 'whisper', 'vad')),
  manifest_version TEXT NOT NULL,
  archive_sha256 TEXT NOT NULL,
  install_dir TEXT NOT NULL UNIQUE,
  state TEXT NOT NULL CHECK(state IN ('ready', 'corrupt', 'deleting')),
  installed_at TEXT NOT NULL,
  last_error_code TEXT
);
CREATE TABLE model_downloads (
  id TEXT PRIMARY KEY,
  model_id TEXT NOT NULL,
  manifest_version TEXT NOT NULL,
  archive_sha256 TEXT NOT NULL,
  state TEXT NOT NULL CHECK(state IN
    ('queued', 'downloading', 'verifying', 'installing', 'succeeded', 'failed', 'cancelled')),
  downloaded_bytes INTEGER NOT NULL DEFAULT 0,
  expected_bytes INTEGER NOT NULL,
  temp_path TEXT,
  error_code TEXT,
  error_summary TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE UNIQUE INDEX model_downloads_one_active_model
ON model_downloads(model_id)
WHERE state IN ('queued', 'downloading', 'verifying', 'installing');
CREATE TABLE asr_jobs (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL REFERENCES sessions(id),
  chunk_id TEXT NOT NULL REFERENCES chunks(id),
  provider TEXT NOT NULL CHECK(provider IN ('sense_voice', 'whisper')),
  model_id TEXT NOT NULL,
  manifest_version TEXT NOT NULL,
  archive_sha256 TEXT NOT NULL,
  required_file_hashes_json TEXT NOT NULL,
  model_source_json TEXT NOT NULL,
  vad_model_id TEXT,
  vad_manifest_version TEXT,
  vad_archive_sha256 TEXT,
  vad_required_file_hashes_json TEXT,
  parameters_json TEXT NOT NULL,
  input_sha256 TEXT NOT NULL,
  fingerprint TEXT NOT NULL,
  state TEXT NOT NULL CHECK(state IN
    ('queued', 'blocked_model', 'preparing', 'transcribing', 'succeeded', 'failed', 'cancelled')),
  attempt_count INTEGER NOT NULL DEFAULT 0,
  claim_generation INTEGER NOT NULL DEFAULT 0,
  max_attempts INTEGER NOT NULL DEFAULT 3 CHECK(max_attempts BETWEEN 1 AND 10),
  available_at TEXT NOT NULL,
  claimed_by TEXT,
  lease_expires_at TEXT,
  cancel_requested_at TEXT,
  error_code TEXT,
  error_summary TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE UNIQUE INDEX asr_jobs_one_active_fingerprint
ON asr_jobs(fingerprint)
WHERE state IN ('queued', 'blocked_model', 'preparing', 'transcribing');
CREATE INDEX asr_jobs_claimable
ON asr_jobs(state, available_at, lease_expires_at);
CREATE TABLE provider_receipts (
  id TEXT PRIMARY KEY,
  job_id TEXT NOT NULL UNIQUE REFERENCES asr_jobs(id),
  chunk_id TEXT NOT NULL REFERENCES chunks(id),
  provider TEXT NOT NULL,
  model_id TEXT NOT NULL,
  manifest_version TEXT NOT NULL,
  archive_sha256 TEXT NOT NULL,
  required_file_hashes_json TEXT NOT NULL,
  model_source_json TEXT NOT NULL,
  vad_model_id TEXT,
  vad_manifest_version TEXT,
  vad_archive_sha256 TEXT,
  vad_required_file_hashes_json TEXT,
  runtime_version TEXT NOT NULL,
  runtime_build_id TEXT NOT NULL,
  parameters_json TEXT NOT NULL,
  input_sha256 TEXT NOT NULL,
  started_at TEXT NOT NULL,
  finished_at TEXT NOT NULL,
  data_destination TEXT NOT NULL CHECK(data_destination = 'local_device'),
  outcome TEXT NOT NULL CHECK(outcome = 'succeeded')
);
CREATE TABLE revision_receipts (
  revision_id TEXT NOT NULL REFERENCES revisions(id),
  receipt_id TEXT NOT NULL UNIQUE REFERENCES provider_receipts(id),
  PRIMARY KEY(revision_id, receipt_id)
);";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SchemaKind {
    Fresh,
    LegacyV1,
    CurrentV2,
    Unknown,
}

#[derive(Debug, PartialEq, Eq)]
struct Column {
    name: String,
    data_type: String,
    not_null: bool,
    default_value: Option<String>,
    primary_key_position: i64,
}

pub(crate) fn classify(connection: &Connection) -> rusqlite::Result<SchemaKind> {
    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    match version {
        0 if user_tables(connection)?.is_empty()
            && named_indexes(connection)?.is_empty()
            && auxiliary_objects(connection)?.is_empty() =>
        {
            Ok(SchemaKind::Fresh)
        }
        0 if is_v1(connection)? => Ok(SchemaKind::LegacyV1),
        0 => Ok(SchemaKind::Unknown),
        CURRENT_VERSION if is_v2(connection)? => Ok(SchemaKind::CurrentV2),
        CURRENT_VERSION => Err(migration_error("corrupt v2 catalog schema")),
        other => Err(migration_error(&format!(
            "incompatible catalog version {other}"
        ))),
    }
}

pub(crate) fn migrate(connection: &mut Connection) -> rusqlite::Result<()> {
    migrate_with_hook(connection, || Ok(()))
}

pub(crate) fn migrate_with_hook<F>(connection: &mut Connection, hook: F) -> rusqlite::Result<()>
where
    F: FnOnce() -> rusqlite::Result<()>,
{
    connection.pragma_update(None, "foreign_keys", true)?;
    let kind = classify(connection)?;
    match kind {
        SchemaKind::CurrentV2 => return Ok(()),
        SchemaKind::Unknown => return Err(migration_error("unknown or corrupt catalog schema")),
        SchemaKind::Fresh | SchemaKind::LegacyV1 => {}
    }

    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    match kind {
        SchemaKind::Fresh => transaction.execute_batch(FRESH_BASE_SCHEMA)?,
        SchemaKind::LegacyV1 => transaction.execute_batch(LEGACY_ALTERS)?,
        SchemaKind::CurrentV2 | SchemaKind::Unknown => unreachable!(),
    }
    transaction.execute_batch(ASR_SCHEMA)?;
    hook()?;
    if !is_v2(&transaction)? {
        return Err(migration_error(
            "migration produced invalid v2 catalog schema",
        ));
    }
    transaction.pragma_update(None, "user_version", CURRENT_VERSION)?;
    transaction.commit()
}

fn is_v1(connection: &Connection) -> rusqlite::Result<bool> {
    if user_tables(connection)? != names(&V1_TABLES)
        || !fts_is_trigram(connection)?
        || !fts_shadows_are_complete(connection)?
        || !named_indexes(connection)?.is_empty()
        || !auxiliary_objects(connection)?.is_empty()
    {
        return Ok(false);
    }
    for table in V1_TABLES {
        let expected = statement_named(LEGACY_SCHEMA, table)?;
        if compact_sql(&schema_sql(connection, table)?) != compact_sql(&expected) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn is_v2(connection: &Connection) -> rusqlite::Result<bool> {
    let expected_indexes = names(&[
        "asr_jobs_claimable",
        "asr_jobs_one_active_fingerprint",
        "model_downloads_one_active_model",
    ]);
    if user_tables(connection)? != names(&V2_TABLES)
        || !fts_is_trigram(connection)?
        || !fts_shadows_are_complete(connection)?
        || named_indexes(connection)? != expected_indexes
        || !auxiliary_objects(connection)?.is_empty()
    {
        return Ok(false);
    }
    if !columns_match(
        connection,
        "sessions",
        &[
            ("id", "TEXT", false, None, 1),
            ("title", "TEXT", true, None, 0),
            ("state", "TEXT", true, None, 0),
            ("started_at", "TEXT", true, None, 0),
            ("ended_at", "TEXT", false, None, 0),
        ],
    )? || !columns_match(
        connection,
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
    )? || !columns_match(
        connection,
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
    )? || !columns_match(
        connection,
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
    )? {
        return Ok(false);
    }

    for (table, expected) in [
        (
            "asr_settings",
            columns_from_create(ASR_SCHEMA, "asr_settings")?,
        ),
        (
            "model_installations",
            columns_from_create(ASR_SCHEMA, "model_installations")?,
        ),
        (
            "model_downloads",
            columns_from_create(ASR_SCHEMA, "model_downloads")?,
        ),
        ("asr_jobs", columns_from_create(ASR_SCHEMA, "asr_jobs")?),
        (
            "provider_receipts",
            columns_from_create(ASR_SCHEMA, "provider_receipts")?,
        ),
        (
            "revision_receipts",
            columns_from_create(ASR_SCHEMA, "revision_receipts")?,
        ),
    ] {
        if compact_sql(&schema_sql(connection, table)?) != compact_sql(&expected) {
            return Ok(false);
        }
    }

    for index in [
        "model_downloads_one_active_model",
        "asr_jobs_one_active_fingerprint",
        "asr_jobs_claimable",
    ] {
        let expected = statement_named(ASR_SCHEMA, index)?;
        let Some(actual) = schema_sql_optional(connection, index)? else {
            return Ok(false);
        };
        if compact_sql(&actual) != compact_sql(&expected) {
            return Ok(false);
        }
    }
    base_constraints_match(connection)
}

fn base_constraints_match(connection: &Connection) -> rusqlite::Result<bool> {
    let revisions = compact_sql(&schema_sql(connection, "revisions")?);
    let chunks = compact_sql(&schema_sql(connection, "chunks")?);
    let segments = compact_sql(&schema_sql(connection, "segments")?);
    Ok(revisions.contains("unique(session_id,number)")
        && revisions.contains("foreignkey(session_id)referencessessions(id)")
        && revisions.contains(
            "check(provenance_statusin('legacy_unverified','verified_local_asr','manual'))",
        )
        && chunks.contains("foreignkey(session_id)referencessessions(id)")
        && chunks.contains("check(integrity_statein('available','corrupted','missing'))")
        && segments.contains("primarykey(id,revision_id)")
        && segments.contains("foreignkey(revision_id)referencesrevisions(id)")
        && foreign_keys(connection, "revisions")?
            == [("session_id", "sessions", "id")]
                .into_iter()
                .map(owned_foreign_key)
                .collect()
        && foreign_keys(connection, "chunks")?
            == [("session_id", "sessions", "id")]
                .into_iter()
                .map(owned_foreign_key)
                .collect()
        && foreign_keys(connection, "segments")?
            == [
                ("chunk_id", "chunks", "id"),
                ("revision_id", "revisions", "id"),
            ]
            .into_iter()
            .map(owned_foreign_key)
            .collect())
}

fn columns_match(
    connection: &Connection,
    table: &str,
    expected: &[(&str, &str, bool, Option<&str>, i64)],
) -> rusqlite::Result<bool> {
    let actual = table_columns(connection, table)?;
    let expected = expected
        .iter()
        .map(|column| Column {
            name: column.0.to_owned(),
            data_type: column.1.to_owned(),
            not_null: column.2,
            default_value: column.3.map(str::to_owned),
            primary_key_position: column.4,
        })
        .collect::<Vec<_>>();
    Ok(actual == expected)
}

fn table_columns(connection: &Connection, table: &str) -> rusqlite::Result<Vec<Column>> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info('{table}')"))?;
    statement
        .query_map([], |row| {
            Ok(Column {
                name: row.get(1)?,
                data_type: row.get(2)?,
                not_null: row.get::<_, i64>(3)? != 0,
                default_value: row.get(4)?,
                primary_key_position: row.get(5)?,
            })
        })?
        .collect()
}

fn foreign_keys(
    connection: &Connection,
    table: &str,
) -> rusqlite::Result<BTreeSet<(String, String, String)>> {
    let mut statement = connection.prepare(&format!("PRAGMA foreign_key_list('{table}')"))?;
    statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(3)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(4)?,
            ))
        })?
        .collect()
}

fn owned_foreign_key(values: (&str, &str, &str)) -> (String, String, String) {
    (
        values.0.to_owned(),
        values.1.to_owned(),
        values.2.to_owned(),
    )
}

fn user_tables(connection: &Connection) -> rusqlite::Result<BTreeSet<String>> {
    Ok(all_tables(connection)?
        .into_iter()
        .filter(|name| !FTS_SHADOWS.contains(&name.as_str()))
        .collect())
}

fn all_tables(connection: &Connection) -> rusqlite::Result<BTreeSet<String>> {
    schema_names(connection, "table")
}

fn named_indexes(connection: &Connection) -> rusqlite::Result<BTreeSet<String>> {
    schema_names(connection, "index")
}

fn auxiliary_objects(connection: &Connection) -> rusqlite::Result<BTreeSet<String>> {
    let mut statement = connection.prepare(
        "SELECT name FROM sqlite_schema
         WHERE type IN ('view', 'trigger') AND name NOT LIKE 'sqlite_%'",
    )?;
    statement.query_map([], |row| row.get(0))?.collect()
}

fn schema_names(connection: &Connection, object_type: &str) -> rusqlite::Result<BTreeSet<String>> {
    let mut statement = connection
        .prepare("SELECT name FROM sqlite_schema WHERE type = ?1 AND name NOT LIKE 'sqlite_%'")?;
    statement
        .query_map([object_type], |row| row.get(0))?
        .collect()
}

fn fts_shadows_are_complete(connection: &Connection) -> rusqlite::Result<bool> {
    let tables = all_tables(connection)?;
    Ok(FTS_SHADOWS.iter().all(|name| tables.contains(*name)))
}

fn names(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn fts_is_trigram(connection: &Connection) -> rusqlite::Result<bool> {
    let expected = "CREATE VIRTUAL TABLE segment_search USING fts5(segment_id UNINDEXED, revision_id UNINDEXED, text, tokenize='trigram')";
    Ok(compact_sql(&schema_sql(connection, FTS_TABLE)?) == compact_sql(expected))
}

fn schema_sql(connection: &Connection, name: &str) -> rusqlite::Result<String> {
    connection.query_row(
        "SELECT sql FROM sqlite_schema WHERE name = ?1",
        [name],
        |row| row.get(0),
    )
}

fn schema_sql_optional(connection: &Connection, name: &str) -> rusqlite::Result<Option<String>> {
    let mut statement = connection.prepare("SELECT sql FROM sqlite_schema WHERE name = ?1")?;
    let mut rows = statement.query([name])?;
    rows.next()?.map(|row| row.get(0)).transpose()
}

fn compact_sql(sql: &str) -> String {
    sql.chars()
        .filter(|character| !character.is_ascii_whitespace())
        .flat_map(char::to_lowercase)
        .collect()
}

fn split_statements(sql: &str) -> Vec<String> {
    sql.split(';')
        .map(str::trim)
        .filter(|statement| !statement.is_empty())
        .map(str::to_owned)
        .collect()
}

fn columns_from_create(sql: &str, table: &str) -> rusqlite::Result<String> {
    statement_named(sql, table)
}

fn statement_named(sql: &str, name: &str) -> rusqlite::Result<String> {
    split_statements(sql)
        .into_iter()
        .find(|statement| {
            let normalized = compact_sql(statement);
            normalized.starts_with(&format!("createtable{name}("))
                || normalized.starts_with(&format!("createvirtualtable{name}"))
                || normalized.starts_with(&format!("createuniqueindex{name}"))
                || normalized.starts_with(&format!("createindex{name}"))
        })
        .ok_or_else(|| migration_error(&format!("missing schema contract for {name}")))
}

fn migration_error(message: &str) -> Error {
    Error::SqliteFailure(
        ffi::Error::new(ffi::SQLITE_SCHEMA),
        Some(message.to_owned()),
    )
}
