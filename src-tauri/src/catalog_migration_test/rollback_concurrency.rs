use super::*;

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
fn migration_entrypoint_overrides_zero_busy_timeout_and_waits() {
    let (_directory, path, connection) = fresh_file_connection();
    drop(connection);
    let mut lock_connection = Connection::open(&path).unwrap();
    let lock = lock_connection
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .unwrap();
    let (started_tx, started_rx) = mpsc::channel();
    let migration_path = path.clone();
    let worker = thread::spawn(move || {
        let mut connection = Connection::open(migration_path).unwrap();
        connection.busy_timeout(Duration::ZERO).unwrap();
        started_tx.send(()).unwrap();
        migrations::migrate(&mut connection)
    });
    started_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    thread::sleep(Duration::from_millis(100));
    lock.commit().unwrap();

    worker.join().unwrap().unwrap();
}

#[test]
fn catalog_open_waits_for_brief_immediate_writer() {
    let (_directory, path, connection) = fresh_file_connection();
    drop(connection);
    let mut lock_connection = Connection::open(&path).unwrap();
    let lock = lock_connection
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .unwrap();
    let (result_tx, result_rx) = mpsc::channel();
    let catalog_path = path.clone();
    thread::spawn(move || {
        result_tx.send(Catalog::open(catalog_path).is_ok()).unwrap();
    });
    thread::sleep(Duration::from_millis(100));
    assert!(matches!(
        result_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    lock.commit().unwrap();

    assert!(result_rx.recv_timeout(Duration::from_secs(2)).unwrap());
}
