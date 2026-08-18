pub(super) const CURRENT_VERSION: i64 = 4;
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
pub(super) const V3_TABLES: [&str; 12] = [
    "asr_jobs",
    "asr_settings",
    "chunks",
    "model_download_artifacts",
    "model_downloads",
    "model_installations",
    "provider_receipts",
    "revision_receipts",
    "revisions",
    "segment_search",
    "segments",
    "sessions",
];
pub(super) const V4_TABLES: [&str; 15] = [
    "asr_jobs",
    "asr_settings",
    "chunks",
    "model_download_artifacts",
    "model_downloads",
    "model_installations",
    "open_intent_ledger",
    "operations",
    "provider_receipts",
    "revision_receipts",
    "revisions",
    "segment_search",
    "segments",
    "sessions",
    "tool_requests",
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
CREATE TABLE asr_settings (singleton_id INTEGER PRIMARY KEY CHECK(singleton_id = 1), provider TEXT NOT NULL CHECK(provider IN ('sense_voice', 'whisper', 'qwen3_asr')), model_id TEXT NOT NULL, language TEXT NOT NULL, num_threads INTEGER NOT NULL CHECK(num_threads >= 1), vad_enabled INTEGER NOT NULL CHECK(vad_enabled IN (0, 1)), auto_transcribe_imports INTEGER NOT NULL CHECK(auto_transcribe_imports IN (0, 1)), provider_options_json TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE model_installations (model_id TEXT PRIMARY KEY, provider TEXT NOT NULL CHECK(provider IN ('sense_voice', 'whisper', 'qwen3_asr', 'vad')), manifest_version TEXT NOT NULL, archive_sha256 TEXT NOT NULL, install_dir TEXT NOT NULL UNIQUE, state TEXT NOT NULL CHECK(state IN ('ready', 'corrupt', 'deleting')), installed_at TEXT NOT NULL, last_error_code TEXT);
CREATE TABLE model_downloads (id TEXT PRIMARY KEY, model_id TEXT NOT NULL, manifest_version TEXT NOT NULL, archive_sha256 TEXT NOT NULL, state TEXT NOT NULL CHECK(state IN ('queued', 'downloading', 'verifying', 'installing', 'succeeded', 'failed', 'cancelled')), downloaded_bytes INTEGER NOT NULL DEFAULT 0, expected_bytes INTEGER NOT NULL, temp_path TEXT, error_code TEXT, error_summary TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE UNIQUE INDEX model_downloads_one_active_model ON model_downloads(model_id) WHERE state IN ('queued', 'downloading', 'verifying', 'installing');
CREATE TABLE asr_jobs (id TEXT PRIMARY KEY, session_id TEXT NOT NULL REFERENCES sessions(id), chunk_id TEXT NOT NULL REFERENCES chunks(id), provider TEXT NOT NULL CHECK(provider IN ('sense_voice', 'whisper', 'qwen3_asr')), model_id TEXT NOT NULL, manifest_version TEXT NOT NULL, archive_sha256 TEXT NOT NULL, required_file_hashes_json TEXT NOT NULL, model_source_json TEXT NOT NULL, vad_model_id TEXT, vad_manifest_version TEXT, vad_archive_sha256 TEXT, vad_required_file_hashes_json TEXT, parameters_json TEXT NOT NULL, input_sha256 TEXT NOT NULL, fingerprint TEXT NOT NULL, state TEXT NOT NULL CHECK(state IN ('queued', 'blocked_model', 'preparing', 'transcribing', 'succeeded', 'failed', 'cancelled')), attempt_count INTEGER NOT NULL DEFAULT 0, claim_generation INTEGER NOT NULL DEFAULT 0, max_attempts INTEGER NOT NULL DEFAULT 3 CHECK(max_attempts BETWEEN 1 AND 10), available_at TEXT NOT NULL, claimed_by TEXT, lease_expires_at TEXT, cancel_requested_at TEXT, error_code TEXT, error_summary TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE UNIQUE INDEX asr_jobs_one_active_fingerprint ON asr_jobs(fingerprint) WHERE state IN ('queued', 'blocked_model', 'preparing', 'transcribing');
CREATE INDEX asr_jobs_claimable ON asr_jobs(state, available_at, lease_expires_at);
CREATE TABLE provider_receipts (id TEXT PRIMARY KEY, job_id TEXT NOT NULL UNIQUE REFERENCES asr_jobs(id), chunk_id TEXT NOT NULL REFERENCES chunks(id), provider TEXT NOT NULL, model_id TEXT NOT NULL, manifest_version TEXT NOT NULL, archive_sha256 TEXT NOT NULL, required_file_hashes_json TEXT NOT NULL, model_source_json TEXT NOT NULL, vad_model_id TEXT, vad_manifest_version TEXT, vad_archive_sha256 TEXT, vad_required_file_hashes_json TEXT, runtime_version TEXT NOT NULL, runtime_build_id TEXT NOT NULL, parameters_json TEXT NOT NULL, input_sha256 TEXT NOT NULL, started_at TEXT NOT NULL, finished_at TEXT NOT NULL, data_destination TEXT NOT NULL CHECK(data_destination = 'local_device'), outcome TEXT NOT NULL CHECK(outcome = 'succeeded'));
CREATE TABLE revision_receipts (revision_id TEXT NOT NULL REFERENCES revisions(id), receipt_id TEXT NOT NULL UNIQUE REFERENCES provider_receipts(id), PRIMARY KEY(revision_id, receipt_id));";

pub(super) const MODEL_MANAGER_V3_SCHEMA: &str = "
ALTER TABLE model_installations RENAME TO model_installations_v2;
CREATE TABLE model_installations (model_id TEXT PRIMARY KEY, provider TEXT NOT NULL CHECK(provider IN ('sense_voice', 'whisper', 'qwen3_asr', 'vad')), manifest_version TEXT NOT NULL, archive_sha256 TEXT NOT NULL, install_dir TEXT NOT NULL UNIQUE, state TEXT NOT NULL CHECK(state IN ('installed_unqualified', 'runtime_qualified', 'deleting')), installed_at TEXT NOT NULL, runtime_identity_json TEXT, qualified_at TEXT, last_error_code TEXT);
INSERT INTO model_installations(model_id, provider, manifest_version, archive_sha256, install_dir, state, installed_at, runtime_identity_json, qualified_at, last_error_code)
SELECT model_id, provider, manifest_version, archive_sha256, install_dir,
       CASE state WHEN 'deleting' THEN 'deleting' ELSE 'installed_unqualified' END,
       installed_at, NULL, NULL,
       CASE WHEN state = 'corrupt' THEN COALESCE(last_error_code, 'model_integrity_failed') ELSE last_error_code END
FROM model_installations_v2;
DROP TABLE model_installations_v2;
CREATE TABLE model_download_artifacts (
  download_id TEXT NOT NULL REFERENCES model_downloads(id) ON DELETE CASCADE,
  artifact_id TEXT NOT NULL,
  source_repository TEXT NOT NULL,
  source_model TEXT NOT NULL,
  source_url TEXT NOT NULL,
  source_revision TEXT NOT NULL,
  expected_bytes INTEGER NOT NULL CHECK(expected_bytes >= 0),
  downloaded_bytes INTEGER NOT NULL DEFAULT 0 CHECK(downloaded_bytes >= 0 AND downloaded_bytes <= expected_bytes),
  expected_sha256 TEXT NOT NULL CHECK(length(expected_sha256) = 64),
  verified_sha256 TEXT CHECK(verified_sha256 IS NULL OR length(verified_sha256) = 64),
  required_path TEXT NOT NULL,
  temp_path TEXT,
  etag TEXT,
  last_modified TEXT,
  checkpointed_at TEXT,
  verified_at TEXT,
  state TEXT NOT NULL CHECK(state IN ('pending', 'downloading', 'downloaded', 'verifying', 'verified', 'failed', 'cancelled')),
  error_code TEXT,
  error_summary TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY(download_id, artifact_id),
  UNIQUE(download_id, required_path)
);
CREATE INDEX model_download_artifacts_state ON model_download_artifacts(download_id, state);";

pub(super) const TOOL_API_V4_SCHEMA: &str = "
CREATE TABLE tool_requests (
  idempotency_key TEXT NOT NULL,
  contract TEXT NOT NULL,
  contract_version INTEGER NOT NULL,
  principal_id TEXT NOT NULL,
  principal_kind TEXT NOT NULL CHECK(principal_kind IN ('local_agent', 'tauri_ui', 'gateway')),
  method TEXT NOT NULL,
  request_fingerprint TEXT NOT NULL,
  state TEXT NOT NULL CHECK(state IN ('in_progress', 'succeeded', 'failed')),
  operation_id TEXT,
  response_json TEXT,
  error_code TEXT,
  error_message_key TEXT,
  created_at TEXT NOT NULL,
  committed_at TEXT NOT NULL,
  PRIMARY KEY(contract, contract_version, principal_id, idempotency_key)
);
CREATE INDEX tool_requests_operation ON tool_requests(operation_id);

CREATE TABLE operations (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL,
  state TEXT NOT NULL CHECK(state IN ('queued', 'running', 'succeeded', 'failed', 'cancelled', 'recovery_required')),
  principal_id TEXT NOT NULL,
  principal_kind TEXT NOT NULL CHECK(principal_kind IN ('local_agent', 'tauri_ui', 'gateway')),
  method TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  last_error_code TEXT,
  last_error_summary TEXT
);
CREATE INDEX operations_principal ON operations(principal_id, principal_kind, created_at);

CREATE TABLE open_intent_ledger (
  intent_id TEXT PRIMARY KEY,
  requesting_principal_id TEXT NOT NULL,
  requesting_principal_kind TEXT NOT NULL CHECK(requesting_principal_kind IN ('local_agent', 'tauri_ui', 'gateway')),
  evidence_ref_json TEXT NOT NULL,
  state TEXT NOT NULL CHECK(state IN ('pending', 'executing', 'consumed', 'uncertain', 'expired')),
  display_metadata_json TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  host_event_id TEXT,
  claim_principal_id TEXT,
  claim_request_id TEXT,
  consent_at TEXT,
  consumed_at TEXT,
  uncertain_at TEXT,
  diagnostic_id TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);";
