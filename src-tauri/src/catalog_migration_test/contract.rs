use super::*;

#[test]
fn creates_complete_v3_schema_for_fresh_catalog() {
    let mut connection = Connection::open_in_memory().unwrap();
    migrations::migrate(&mut connection).unwrap();

    assert_eq!(user_version(&connection), 4);
    for table in [
        "asr_settings",
        "model_installations",
        "model_downloads",
        "model_download_artifacts",
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
        "model_download_artifacts_state",
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
            ("runtime_identity_json", "TEXT", false, None, 0),
            ("qualified_at", "TEXT", false, None, 0),
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
        foreign_keys(&connection, "model_download_artifacts"),
        [(
            "download_id".to_owned(),
            "model_downloads".to_owned(),
            "id".to_owned(),
            "NO ACTION".to_owned(),
            "CASCADE".to_owned(),
            "NONE".to_owned(),
        )]
        .into_iter()
        .collect()
    );
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
        unique_contracts(&connection, "model_download_artifacts"),
        [
            contract("pk", false, &["download_id", "artifact_id"]),
            contract("u", false, &["download_id", "required_path"]),
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
        (
            "model_download_artifacts_state",
            "CREATE INDEX model_download_artifacts_state ON model_download_artifacts(download_id, state)",
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
                "CHECK(provider IN ('sense_voice', 'whisper', 'qwen3_asr'))",
                "CHECK(num_threads >= 1)",
                "CHECK(vad_enabled IN (0, 1))",
                "CHECK(auto_transcribe_imports IN (0, 1))",
            ][..],
        ),
        (
            "model_installations",
            &[
                "CHECK(provider IN ('sense_voice', 'whisper', 'qwen3_asr', 'vad'))",
                "CHECK(state IN ('installed_unqualified', 'runtime_qualified', 'deleting'))",
            ][..],
        ),
        (
            "model_download_artifacts",
            &[
                "CHECK(expected_bytes >= 0)",
                "CHECK(downloaded_bytes >= 0 AND downloaded_bytes <= expected_bytes)",
                "CHECK(length(expected_sha256) = 64)",
                "CHECK(verified_sha256 IS NULL OR length(verified_sha256) = 64)",
                "CHECK(state IN ('pending', 'downloading', 'downloaded', 'verifying', 'verified', 'failed', 'cancelled'))",
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
                "CHECK(provider IN ('sense_voice', 'whisper', 'qwen3_asr'))",
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
fn fresh_v3_accepts_qwen3_asr_settings() {
    let mut connection = Connection::open_in_memory().unwrap();
    migrations::migrate(&mut connection).unwrap();

    connection
        .execute(
            "INSERT INTO asr_settings(
           singleton_id, provider, model_id, language, num_threads, vad_enabled,
           auto_transcribe_imports, provider_options_json, updated_at
         ) VALUES(1, 'qwen3_asr', 'qwen3-asr-0.6b', 'auto', 2, 1, 1, '{}', '2026-08-15T00:00:00Z')",
            [],
        )
        .unwrap();
}
