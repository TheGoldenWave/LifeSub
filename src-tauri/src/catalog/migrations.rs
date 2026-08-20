use rusqlite::Connection;

/// Classification of a database schema for migration decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaKind {
    /// Empty database with no user tables — create v2 from scratch.
    Fresh,
    /// V0.1 database with exact expected fingerprint — migrate to v2.
    LegacyV1,
    /// Unknown or corrupt schema — refuse to operate.
    Unknown,
}

// ── V0.1 fingerprint constants ─────────────────────────────────────────

/// V0.1 expected tables and their columns in declaration order.
const V1_TABLES: &[(&str, &[&str])] = &[
    (
        "sessions",
        &["id", "title", "state", "started_at", "ended_at"],
    ),
    (
        "revisions",
        &["id", "session_id", "number", "provider", "created_at"],
    ),
    (
        "segments",
        &["id", "revision_id", "start_ms", "end_ms", "source", "text"],
    ),
    (
        "chunks",
        &[
            "id",
            "session_id",
            "source",
            "path",
            "sha256",
            "byte_length",
        ],
    ),
];

// ── DDL fragments ──────────────────────────────────────────────────────

/// Base tables that exist in both v1 and v2 (with v2 columns).
const V2_BASE_DDL: &str = "
CREATE TABLE IF NOT EXISTS sessions (
  id TEXT PRIMARY KEY, title TEXT NOT NULL, state TEXT NOT NULL,
  started_at TEXT NOT NULL, ended_at TEXT
);

CREATE TABLE IF NOT EXISTS revisions (
  id TEXT PRIMARY KEY, session_id TEXT NOT NULL, number INTEGER NOT NULL,
  provider TEXT NOT NULL, created_at TEXT NOT NULL,
  provenance_status TEXT NOT NULL DEFAULT 'legacy_unverified',
  UNIQUE(session_id, number), FOREIGN KEY(session_id) REFERENCES sessions(id)
);

CREATE TABLE IF NOT EXISTS segments (
  id TEXT NOT NULL, revision_id TEXT NOT NULL, start_ms INTEGER NOT NULL,
  end_ms INTEGER NOT NULL, source TEXT NOT NULL, text TEXT NOT NULL,
  chunk_id TEXT REFERENCES chunks(id),
  chunk_start_ms INTEGER,
  chunk_end_ms INTEGER,
  session_start_ms INTEGER,
  session_end_ms INTEGER,
  PRIMARY KEY(id, revision_id), FOREIGN KEY(revision_id) REFERENCES revisions(id)
);

CREATE TABLE IF NOT EXISTS chunks (
  id TEXT PRIMARY KEY, session_id TEXT NOT NULL, source TEXT NOT NULL,
  path TEXT NOT NULL, sha256 TEXT NOT NULL, byte_length INTEGER NOT NULL,
  session_offset_ms INTEGER NOT NULL DEFAULT 0,
  duration_ms INTEGER,
  integrity_state TEXT NOT NULL DEFAULT 'available',
  last_error_code TEXT,
  last_error_at TEXT,
  FOREIGN KEY(session_id) REFERENCES sessions(id)
);

CREATE VIRTUAL TABLE IF NOT EXISTS segment_search USING fts5(
  segment_id UNINDEXED, revision_id UNINDEXED, text, tokenize='trigram'
);
";

/// New ASR tables introduced in v2.
const V2_ASR_DDL: &str = "
CREATE TABLE IF NOT EXISTS asr_settings (
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

CREATE TABLE IF NOT EXISTS model_installations (
  model_id TEXT PRIMARY KEY,
  provider TEXT NOT NULL CHECK(provider IN ('sense_voice', 'whisper', 'vad')),
  manifest_version TEXT NOT NULL,
  archive_sha256 TEXT NOT NULL,
  install_dir TEXT NOT NULL UNIQUE,
  state TEXT NOT NULL CHECK(state IN ('ready', 'corrupt', 'deleting')),
  installed_at TEXT NOT NULL,
  last_error_code TEXT
);

CREATE TABLE IF NOT EXISTS model_downloads (
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

CREATE UNIQUE INDEX IF NOT EXISTS model_downloads_one_active_model
ON model_downloads(model_id)
WHERE state IN ('queued', 'downloading', 'verifying', 'installing');

CREATE TABLE IF NOT EXISTS asr_jobs (
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

CREATE UNIQUE INDEX IF NOT EXISTS asr_jobs_one_active_fingerprint
ON asr_jobs(fingerprint)
WHERE state IN ('queued', 'blocked_model', 'preparing', 'transcribing');

CREATE INDEX IF NOT EXISTS asr_jobs_claimable
ON asr_jobs(state, available_at, lease_expires_at);

CREATE TABLE IF NOT EXISTS provider_receipts (
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

CREATE TABLE IF NOT EXISTS revision_receipts (
  revision_id TEXT NOT NULL REFERENCES revisions(id),
  receipt_id TEXT NOT NULL UNIQUE REFERENCES provider_receipts(id),
  PRIMARY KEY(revision_id, receipt_id)
);
";

/// ALTER TABLE statements that upgrade v1 tables to v2 in-place.
const V1_TO_V2_ALTER_DDL: &str = "
ALTER TABLE chunks ADD COLUMN session_offset_ms INTEGER NOT NULL DEFAULT 0;
ALTER TABLE chunks ADD COLUMN duration_ms INTEGER;
ALTER TABLE chunks ADD COLUMN integrity_state TEXT NOT NULL DEFAULT 'available';
ALTER TABLE chunks ADD COLUMN last_error_code TEXT;
ALTER TABLE chunks ADD COLUMN last_error_at TEXT;

ALTER TABLE revisions ADD COLUMN provenance_status TEXT NOT NULL DEFAULT 'legacy_unverified';

ALTER TABLE segments ADD COLUMN chunk_id TEXT REFERENCES chunks(id);
ALTER TABLE segments ADD COLUMN chunk_start_ms INTEGER;
ALTER TABLE segments ADD COLUMN chunk_end_ms INTEGER;
ALTER TABLE segments ADD COLUMN session_start_ms INTEGER;
ALTER TABLE segments ADD COLUMN session_end_ms INTEGER;
";

// ── Public API ─────────────────────────────────────────────────────────

/// Classify a database schema without modifying it.
///
/// Reads `PRAGMA user_version` and the schema fingerprint to determine
/// whether the database is Fresh (empty), a known LegacyV1, or Unknown.
pub fn classify_schema(conn: &Connection) -> rusqlite::Result<SchemaKind> {
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap_or(0);

    match version {
        0 => classify_v0(conn),
        2 => classify_v2(conn),
        _ => Ok(SchemaKind::Unknown),
    }
}

/// Run the migration appropriate for the current schema.
///
/// - Fresh: create the full v2 schema and set `user_version = 2`.
/// - LegacyV1: migrate v1 tables to v2 in a `BEGIN IMMEDIATE` transaction
///   and set `user_version = 2`.
/// - Unknown: return an error.
pub fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;

    let kind = classify_schema(conn)?;
    match kind {
        SchemaKind::Fresh => {
            conn.execute_batch(V2_BASE_DDL)?;
            conn.execute_batch(V2_ASR_DDL)?;
            conn.pragma_update(None, "user_version", 2)?;
            Ok(())
        }
        SchemaKind::LegacyV1 => {
            conn.execute_batch("BEGIN IMMEDIATE;")?;
            // Run ALTER TABLE and new table creation in the transaction.
            // If any statement fails, the entire transaction rolls back.
            let result = (|| {
                conn.execute_batch(V1_TO_V2_ALTER_DDL)?;
                conn.execute_batch(V2_ASR_DDL)?;
                conn.pragma_update(None, "user_version", 2)?;
                Ok(())
            })();
            match result {
                Ok(()) => {
                    conn.execute_batch("COMMIT;")?;
                    Ok(())
                }
                Err(e) => {
                    // Rollback on any error; ignore rollback errors to
                    // preserve the original error.
                    let _ = conn.execute_batch("ROLLBACK;");
                    Err(e)
                }
            }
        }
        SchemaKind::Unknown => Err(rusqlite::Error::InvalidParameterName(
            "unknown or corrupt database schema: cannot migrate".into(),
        )),
    }
}

// ── Internal helpers ───────────────────────────────────────────────────

fn classify_v0(conn: &Connection) -> rusqlite::Result<SchemaKind> {
    // Check if any user tables exist at all.
    let table_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master
         WHERE type IN ('table', 'view')
           AND name NOT LIKE 'sqlite_%'
           AND name NOT LIKE '_fts%'",
        [],
        |row| row.get(0),
    )?;

    if table_count == 0 {
        return Ok(SchemaKind::Fresh);
    }

    if fingerprint_v1(conn)? {
        return Ok(SchemaKind::LegacyV1);
    }

    Ok(SchemaKind::Unknown)
}

fn classify_v2(conn: &Connection) -> rusqlite::Result<SchemaKind> {
    // Verify that the v2 fingerprint is present.
    if fingerprint_v2(conn)? {
        Ok(SchemaKind::Fresh)
    } else {
        Ok(SchemaKind::Unknown)
    }
}

/// Return true if the database schema exactly matches the V0.1 fingerprint:
/// the expected tables, columns, and FTS5 trigram tokenizer.
fn fingerprint_v1(conn: &Connection) -> rusqlite::Result<bool> {
    for (table, expected_cols) in V1_TABLES {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
        let cols: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        if &cols != expected_cols {
            return Ok(false);
        }
    }

    // Verify the FTS5 virtual table uses the trigram tokenizer.
    let sql: String = match conn.query_row(
        "SELECT sql FROM sqlite_master WHERE type='table' AND name='segment_search'",
        [],
        |row| row.get(0),
    ) {
        Ok(sql) => sql,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(false),
        Err(e) => return Err(e),
    };

    if !sql.contains("tokenize='trigram'") {
        return Ok(false);
    }

    Ok(true)
}

/// Return true if the database contains the v2-specific tables and columns.
fn fingerprint_v2(conn: &Connection) -> rusqlite::Result<bool> {
    const V2_NEW_TABLES: &[&str] = &[
        "asr_settings",
        "model_installations",
        "model_downloads",
        "asr_jobs",
        "provider_receipts",
        "revision_receipts",
    ];

    for table in V2_NEW_TABLES {
        let exists: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name=?1",
            [table],
            |row| row.get(0),
        )?;
        if !exists {
            return Ok(false);
        }
    }

    // Verify that chunks has the session_offset_ms column (added in v2).
    let has_offset: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM pragma_table_info('chunks') WHERE name='session_offset_ms'",
        [],
        |row| row.get(0),
    )?;
    if !has_offset {
        return Ok(false);
    }

    // Verify that revisions has the provenance_status column (added in v2).
    let has_provenance: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM pragma_table_info('revisions') WHERE name='provenance_status'",
        [],
        |row| row.get(0),
    )?;
    if !has_provenance {
        return Ok(false);
    }

    Ok(true)
}
