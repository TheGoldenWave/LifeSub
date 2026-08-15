pub(super) const CURRENT_VERSION: i64 = 2;
pub(super) const FTS_TABLE: &str = "segment_search";
pub(super) const FTS_SHADOWS: [&str; 5] = [
    "segment_search_config",
    "segment_search_content",
    "segment_search_data",
    "segment_search_docsize",
    "segment_search_idx",
];
pub(super) const V1_TABLES: [&str; 5] = [
    "chunks",
    "revisions",
    "segment_search",
    "segments",
    "sessions",
];
pub(super) const V2_TABLES: [&str; 11] = [
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

pub(super) const LEGACY_SCHEMA: &str = "
CREATE TABLE sessions (id TEXT PRIMARY KEY, title TEXT NOT NULL, state TEXT NOT NULL, started_at TEXT NOT NULL, ended_at TEXT);
CREATE TABLE revisions (id TEXT PRIMARY KEY, session_id TEXT NOT NULL, number INTEGER NOT NULL, provider TEXT NOT NULL, created_at TEXT NOT NULL, UNIQUE(session_id, number), FOREIGN KEY(session_id) REFERENCES sessions(id));
CREATE TABLE segments (id TEXT NOT NULL, revision_id TEXT NOT NULL, start_ms INTEGER NOT NULL, end_ms INTEGER NOT NULL, source TEXT NOT NULL, text TEXT NOT NULL, PRIMARY KEY(id, revision_id), FOREIGN KEY(revision_id) REFERENCES revisions(id));
CREATE TABLE chunks (id TEXT PRIMARY KEY, session_id TEXT NOT NULL, source TEXT NOT NULL, path TEXT NOT NULL, sha256 TEXT NOT NULL, byte_length INTEGER NOT NULL, FOREIGN KEY(session_id) REFERENCES sessions(id));
CREATE VIRTUAL TABLE segment_search USING fts5(segment_id UNINDEXED, revision_id UNINDEXED, text, tokenize='trigram');";

pub(super) const FRESH_BASE_SCHEMA: &str = "
CREATE TABLE sessions (id TEXT PRIMARY KEY, title TEXT NOT NULL, state TEXT NOT NULL, started_at TEXT NOT NULL, ended_at TEXT);
CREATE TABLE revisions (id TEXT PRIMARY KEY, session_id TEXT NOT NULL, number INTEGER NOT NULL, provider TEXT NOT NULL, created_at TEXT NOT NULL, provenance_status TEXT NOT NULL DEFAULT 'legacy_unverified' CHECK(provenance_status IN ('legacy_unverified', 'verified_local_asr', 'manual')), UNIQUE(session_id, number), FOREIGN KEY(session_id) REFERENCES sessions(id));
CREATE TABLE segments (id TEXT NOT NULL, revision_id TEXT NOT NULL, start_ms INTEGER NOT NULL, end_ms INTEGER NOT NULL, source TEXT NOT NULL, text TEXT NOT NULL, chunk_id TEXT REFERENCES chunks(id), chunk_start_ms INTEGER, chunk_end_ms INTEGER, session_start_ms INTEGER, session_end_ms INTEGER, PRIMARY KEY(id, revision_id), FOREIGN KEY(revision_id) REFERENCES revisions(id));
CREATE TABLE chunks (id TEXT PRIMARY KEY, session_id TEXT NOT NULL, source TEXT NOT NULL, path TEXT NOT NULL, sha256 TEXT NOT NULL, byte_length INTEGER NOT NULL, session_offset_ms INTEGER NOT NULL DEFAULT 0, duration_ms INTEGER, integrity_state TEXT NOT NULL DEFAULT 'available' CHECK(integrity_state IN ('available', 'corrupted', 'missing')), last_error_code TEXT, last_error_at TEXT, FOREIGN KEY(session_id) REFERENCES sessions(id));
CREATE VIRTUAL TABLE segment_search USING fts5(segment_id UNINDEXED, revision_id UNINDEXED, text, tokenize='trigram');";

pub(super) const FTS_SHADOW_SCHEMA: [(&str, &str); 5] = [
    (
        "segment_search_config",
        "CREATE TABLE 'segment_search_config'(k PRIMARY KEY, v) WITHOUT ROWID",
    ),
    (
        "segment_search_content",
        "CREATE TABLE 'segment_search_content'(id INTEGER PRIMARY KEY, c0, c1, c2)",
    ),
    (
        "segment_search_data",
        "CREATE TABLE 'segment_search_data'(id INTEGER PRIMARY KEY, block BLOB)",
    ),
    (
        "segment_search_docsize",
        "CREATE TABLE 'segment_search_docsize'(id INTEGER PRIMARY KEY, sz BLOB)",
    ),
    (
        "segment_search_idx",
        "CREATE TABLE 'segment_search_idx'(segid, term, pgno, PRIMARY KEY(segid, term)) WITHOUT ROWID",
    ),
];

pub(super) const LEGACY_ALTERS: &str = "
ALTER TABLE chunks ADD COLUMN session_offset_ms INTEGER NOT NULL DEFAULT 0;
ALTER TABLE chunks ADD COLUMN duration_ms INTEGER;
ALTER TABLE chunks ADD COLUMN integrity_state TEXT NOT NULL DEFAULT 'available' CHECK(integrity_state IN ('available', 'corrupted', 'missing'));
ALTER TABLE chunks ADD COLUMN last_error_code TEXT;
ALTER TABLE chunks ADD COLUMN last_error_at TEXT;
ALTER TABLE revisions ADD COLUMN provenance_status TEXT NOT NULL DEFAULT 'legacy_unverified' CHECK(provenance_status IN ('legacy_unverified', 'verified_local_asr', 'manual'));
ALTER TABLE segments ADD COLUMN chunk_id TEXT REFERENCES chunks(id);
ALTER TABLE segments ADD COLUMN chunk_start_ms INTEGER;
ALTER TABLE segments ADD COLUMN chunk_end_ms INTEGER;
ALTER TABLE segments ADD COLUMN session_start_ms INTEGER;
ALTER TABLE segments ADD COLUMN session_end_ms INTEGER;";

pub(super) const ASR_SCHEMA: &str = "
CREATE TABLE asr_settings (singleton_id INTEGER PRIMARY KEY CHECK(singleton_id = 1), provider TEXT NOT NULL CHECK(provider IN ('sense_voice', 'whisper')), model_id TEXT NOT NULL, language TEXT NOT NULL, num_threads INTEGER NOT NULL CHECK(num_threads >= 1), vad_enabled INTEGER NOT NULL CHECK(vad_enabled IN (0, 1)), auto_transcribe_imports INTEGER NOT NULL CHECK(auto_transcribe_imports IN (0, 1)), provider_options_json TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE model_installations (model_id TEXT PRIMARY KEY, provider TEXT NOT NULL CHECK(provider IN ('sense_voice', 'whisper', 'vad')), manifest_version TEXT NOT NULL, archive_sha256 TEXT NOT NULL, install_dir TEXT NOT NULL UNIQUE, state TEXT NOT NULL CHECK(state IN ('ready', 'corrupt', 'deleting')), installed_at TEXT NOT NULL, last_error_code TEXT);
CREATE TABLE model_downloads (id TEXT PRIMARY KEY, model_id TEXT NOT NULL, manifest_version TEXT NOT NULL, archive_sha256 TEXT NOT NULL, state TEXT NOT NULL CHECK(state IN ('queued', 'downloading', 'verifying', 'installing', 'succeeded', 'failed', 'cancelled')), downloaded_bytes INTEGER NOT NULL DEFAULT 0, expected_bytes INTEGER NOT NULL, temp_path TEXT, error_code TEXT, error_summary TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE UNIQUE INDEX model_downloads_one_active_model ON model_downloads(model_id) WHERE state IN ('queued', 'downloading', 'verifying', 'installing');
CREATE TABLE asr_jobs (id TEXT PRIMARY KEY, session_id TEXT NOT NULL REFERENCES sessions(id), chunk_id TEXT NOT NULL REFERENCES chunks(id), provider TEXT NOT NULL CHECK(provider IN ('sense_voice', 'whisper')), model_id TEXT NOT NULL, manifest_version TEXT NOT NULL, archive_sha256 TEXT NOT NULL, required_file_hashes_json TEXT NOT NULL, model_source_json TEXT NOT NULL, vad_model_id TEXT, vad_manifest_version TEXT, vad_archive_sha256 TEXT, vad_required_file_hashes_json TEXT, parameters_json TEXT NOT NULL, input_sha256 TEXT NOT NULL, fingerprint TEXT NOT NULL, state TEXT NOT NULL CHECK(state IN ('queued', 'blocked_model', 'preparing', 'transcribing', 'succeeded', 'failed', 'cancelled')), attempt_count INTEGER NOT NULL DEFAULT 0, claim_generation INTEGER NOT NULL DEFAULT 0, max_attempts INTEGER NOT NULL DEFAULT 3 CHECK(max_attempts BETWEEN 1 AND 10), available_at TEXT NOT NULL, claimed_by TEXT, lease_expires_at TEXT, cancel_requested_at TEXT, error_code TEXT, error_summary TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE UNIQUE INDEX asr_jobs_one_active_fingerprint ON asr_jobs(fingerprint) WHERE state IN ('queued', 'blocked_model', 'preparing', 'transcribing');
CREATE INDEX asr_jobs_claimable ON asr_jobs(state, available_at, lease_expires_at);
CREATE TABLE provider_receipts (id TEXT PRIMARY KEY, job_id TEXT NOT NULL UNIQUE REFERENCES asr_jobs(id), chunk_id TEXT NOT NULL REFERENCES chunks(id), provider TEXT NOT NULL, model_id TEXT NOT NULL, manifest_version TEXT NOT NULL, archive_sha256 TEXT NOT NULL, required_file_hashes_json TEXT NOT NULL, model_source_json TEXT NOT NULL, vad_model_id TEXT, vad_manifest_version TEXT, vad_archive_sha256 TEXT, vad_required_file_hashes_json TEXT, runtime_version TEXT NOT NULL, runtime_build_id TEXT NOT NULL, parameters_json TEXT NOT NULL, input_sha256 TEXT NOT NULL, started_at TEXT NOT NULL, finished_at TEXT NOT NULL, data_destination TEXT NOT NULL CHECK(data_destination = 'local_device'), outcome TEXT NOT NULL CHECK(outcome = 'succeeded'));
CREATE TABLE revision_receipts (revision_id TEXT NOT NULL REFERENCES revisions(id), receipt_id TEXT NOT NULL UNIQUE REFERENCES provider_receipts(id), PRIMARY KEY(revision_id, receipt_id));";
