use super::*;

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
        SchemaKind::CurrentV5
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
fn rejects_future_and_corrupt_current_catalogs() {
    let mut future = Connection::open_in_memory().unwrap();
    future.pragma_update(None, "user_version", 6).unwrap();
    let error = migrations::migrate(&mut future).unwrap_err();
    assert!(error.to_string().contains("incompatible catalog version 6"));

    let mut current = Connection::open_in_memory().unwrap();
    migrations::migrate(&mut current).unwrap();
    current
        .execute("DROP INDEX asr_jobs_claimable", [])
        .unwrap();
    let error = migrations::migrate(&mut current).unwrap_err();
    assert!(error.to_string().contains("corrupt v5 catalog schema"));
}

#[test]
fn rejects_corrupt_v2_fixture_without_attempting_v3_ddl() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("catalog.sqlite3");
    fs::copy(V2_FIXTURE, &path).unwrap();
    let mut connection = Connection::open(path).unwrap();
    connection
        .execute("DROP INDEX model_downloads_one_active_model", [])
        .unwrap();

    let error = migrations::migrate(&mut connection).unwrap_err();

    assert!(error.to_string().contains("corrupt v2 catalog schema"));
    assert_eq!(user_version(&connection), 2);
    let artifacts: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_schema WHERE name = 'model_download_artifacts'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(artifacts, 0);
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

    assert!(error.to_string().contains("corrupt v5 catalog schema"));
}

#[test]
fn rejects_current_catalog_with_missing_fts_shadow_table() {
    let mut connection = Connection::open_in_memory().unwrap();
    migrations::migrate(&mut connection).unwrap();
    connection
        .execute("DROP TABLE segment_search_docsize", [])
        .unwrap();

    let error = migrations::migrate(&mut connection).unwrap_err();

    assert!(error.to_string().contains("corrupt v5 catalog schema"));
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
    assert!(error.to_string().contains("corrupt v5 catalog schema"));
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
    assert!(error.to_string().contains("corrupt v5 catalog schema"));
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
    assert!(error.to_string().contains("corrupt v5 catalog schema"));
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
    assert!(error.to_string().contains("corrupt v5 catalog schema"));
}

#[test]
fn rejects_changed_case_in_outcome_literal() {
    let (_directory, path, connection) = fresh_file_connection();
    rewrite_schema_literal(
        &connection,
        "provider_receipts",
        "'succeeded'",
        "'SUCCEEDED'",
    );
    drop(connection);

    let mut reopened = Connection::open(path).unwrap();
    let error = migrations::migrate(&mut reopened).unwrap_err();
    assert!(error.to_string().contains("corrupt v5 catalog schema"));
}

#[test]
fn rejects_changed_case_in_provenance_default_literal() {
    let (_directory, path, connection) = fresh_file_connection();
    rewrite_schema_literal(
        &connection,
        "revisions",
        "DEFAULT 'legacy_unverified'",
        "DEFAULT 'LEGACY_UNVERIFIED'",
    );
    drop(connection);

    let mut reopened = Connection::open(path).unwrap();
    let error = migrations::migrate(&mut reopened).unwrap_err();
    assert!(error.to_string().contains("corrupt v5 catalog schema"));
}

#[test]
fn rejects_changed_whitespace_inside_string_literal() {
    let (_directory, path, connection) = fresh_file_connection();
    rewrite_schema_literal(
        &connection,
        "provider_receipts",
        "'succeeded'",
        "'suc ceeded'",
    );
    drop(connection);

    let mut reopened = Connection::open(path).unwrap();
    let error = migrations::migrate(&mut reopened).unwrap_err();
    assert!(error.to_string().contains("corrupt v5 catalog schema"));
}

#[test]
fn sql_normalizer_preserves_escaped_string_literal_contents() {
    assert_eq!(
        migrations::normalize_sql_for_test("CHECK(value = 'It''s  A')"),
        "check(value='It''s  A')"
    );
}

#[test]
fn rejects_schema_with_missing_token_boundary() {
    let (_directory, path, connection) = fresh_file_connection();
    rewrite_schema_literal(
        &connection,
        "asr_settings",
        "provider TEXT NOT NULL",
        "provider TEXTNOT NULL",
    );
    drop(connection);

    let mut reopened = Connection::open(path).unwrap();
    let error = migrations::migrate(&mut reopened).unwrap_err();
    assert!(error.to_string().contains("corrupt v5 catalog schema"));
}
