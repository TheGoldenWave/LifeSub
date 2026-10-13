use rusqlite::{OptionalExtension, params};

use crate::domain::{AudioChunk, CaptureChunkTiming, CaptureSourceRecord};

use super::{Catalog, chunks, parse_source, parse_time, source_name};

#[derive(Debug)]
#[allow(dead_code)]
pub(crate) enum NativeCaptureInsertError {
    Catalog(rusqlite::Error),
    Conflict,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub(crate) enum NativeCaptureInsertOutcome {
    Inserted,
    Existing,
}

impl Catalog {
    pub fn insert_capture_sources_atomic(
        &self,
        sources: &[CaptureSourceRecord],
    ) -> rusqlite::Result<()> {
        let mut connection = self.connection.lock().unwrap();
        let transaction = connection.transaction()?;
        for source in sources {
            transaction.execute(
                "INSERT INTO capture_sources(
                    id, session_id, source, device_uid, device_name, channel_count,
                    sample_rate_hz, created_at
                 ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    source.id,
                    source.session_id,
                    source_name(source.source),
                    source.device_uid,
                    source.device_name,
                    source.channel_count,
                    source.sample_rate_hz,
                    source.created_at.to_rfc3339(),
                ],
            )?;
        }
        transaction.commit()
    }

    pub fn insert_capture_source(&self, source: &CaptureSourceRecord) -> rusqlite::Result<()> {
        self.connection.lock().unwrap().execute(
            "INSERT INTO capture_sources(
                id, session_id, source, device_uid, device_name, channel_count, sample_rate_hz,
                created_at
             ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                source.id,
                source.session_id,
                source_name(source.source),
                source.device_uid,
                source.device_name,
                source.channel_count,
                source.sample_rate_hz,
                source.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn capture_source(&self, id: &str) -> rusqlite::Result<Option<CaptureSourceRecord>> {
        self.connection
            .lock()
            .unwrap()
            .query_row(
                "SELECT id, session_id, source, device_uid, device_name, channel_count,
                        sample_rate_hz, created_at
                 FROM capture_sources
                 WHERE id = ?1",
                [id],
                |row| {
                    let source = row.get::<_, String>(2)?;
                    let created_at = row.get::<_, String>(7)?;
                    Ok(CaptureSourceRecord {
                        id: row.get(0)?,
                        session_id: row.get(1)?,
                        source: parse_source(&source),
                        device_uid: row.get(3)?,
                        device_name: row.get(4)?,
                        channel_count: row.get(5)?,
                        sample_rate_hz: row.get(6)?,
                        created_at: parse_time(&created_at)?,
                    })
                },
            )
            .optional()
    }

    pub fn capture_chunk_timing(
        &self,
        chunk_id: &str,
    ) -> rusqlite::Result<Option<CaptureChunkTiming>> {
        self.connection
            .lock()
            .unwrap()
            .query_row(
                "SELECT chunk_id, capture_source_id, source_sequence, source_sample_start,
                        source_sample_end, session_start_ms, session_end_ms, monotonic_start_ns,
                        monotonic_end_ns, utc_start_at, utc_end_at, discontinuity_count,
                        dropped_frame_count, created_at
                 FROM capture_chunk_timing
                 WHERE chunk_id = ?1",
                [chunk_id],
                capture_chunk_timing_from_row,
            )
            .optional()
    }

    #[allow(dead_code)]
    pub(crate) fn insert_native_capture_chunk(
        &self,
        chunk: &AudioChunk,
        timing: &CaptureChunkTiming,
    ) -> Result<NativeCaptureInsertOutcome, NativeCaptureInsertError> {
        let Some((session_offset_ms, duration_ms, byte_length)) =
            validated_native_capture(chunk, timing)
        else {
            return Err(NativeCaptureInsertError::Conflict);
        };

        let mut connection = self.connection.lock().unwrap();
        let transaction = connection
            .transaction()
            .map_err(NativeCaptureInsertError::Catalog)?;
        let existing_chunk = transaction
            .query_row(
                "SELECT id, session_id, source, path, sha256, byte_length
                 FROM chunks
                 WHERE id = ?1",
                [&chunk.id],
                chunks::chunk_from_row,
            )
            .optional()
            .map_err(NativeCaptureInsertError::Catalog)?;
        let existing_timing = transaction
            .query_row(
                "SELECT chunk_id, capture_source_id, source_sequence, source_sample_start,
                        source_sample_end, session_start_ms, session_end_ms, monotonic_start_ns,
                        monotonic_end_ns, utc_start_at, utc_end_at, discontinuity_count,
                        dropped_frame_count, created_at
                 FROM capture_chunk_timing
                 WHERE chunk_id = ?1",
                [&chunk.id],
                capture_chunk_timing_from_row,
            )
            .optional()
            .map_err(NativeCaptureInsertError::Catalog)?;

        match (existing_chunk, existing_timing) {
            (Some(existing_chunk), Some(existing_timing))
                if existing_chunk == *chunk && existing_timing == *timing =>
            {
                transaction
                    .commit()
                    .map_err(NativeCaptureInsertError::Catalog)?;
                return Ok(NativeCaptureInsertOutcome::Existing);
            }
            (Some(_), _) | (_, Some(_)) => return Err(NativeCaptureInsertError::Conflict),
            (None, None) => {}
        }

        let path_owner = transaction
            .query_row(
                "SELECT id FROM chunks WHERE path = ?1 LIMIT 1",
                [&chunk.path],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(NativeCaptureInsertError::Catalog)?;
        if path_owner.is_some() {
            return Err(NativeCaptureInsertError::Conflict);
        }

        let source = transaction
            .query_row(
                "SELECT id, session_id, source, device_uid, device_name, channel_count,
                        sample_rate_hz, created_at
                 FROM capture_sources
                 WHERE id = ?1",
                [&timing.capture_source_id],
                capture_source_from_row,
            )
            .optional()
            .map_err(NativeCaptureInsertError::Catalog)?
            .ok_or(NativeCaptureInsertError::Conflict)?;
        if source.session_id != chunk.session_id || source.source != chunk.source {
            return Err(NativeCaptureInsertError::Conflict);
        }

        transaction
            .execute(
                "INSERT INTO chunks(
                    id, session_id, source, path, sha256, byte_length,
                    session_offset_ms, duration_ms
                 ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    chunk.id,
                    chunk.session_id,
                    source_name(chunk.source),
                    chunk.path,
                    chunk.sha256,
                    byte_length,
                    session_offset_ms,
                    duration_ms,
                ],
            )
            .map_err(map_insert_error)?;
        transaction
            .execute(
                "INSERT INTO capture_chunk_timing(
                    chunk_id, capture_source_id, source_sequence, source_sample_start,
                    source_sample_end, session_start_ms, session_end_ms, monotonic_start_ns,
                    monotonic_end_ns, utc_start_at, utc_end_at, discontinuity_count,
                    dropped_frame_count, created_at
                 ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                params![
                    timing.chunk_id,
                    timing.capture_source_id,
                    timing.source_sequence,
                    timing.source_sample_start,
                    timing.source_sample_end,
                    timing.session_start_ms,
                    timing.session_end_ms,
                    timing.monotonic_start_ns,
                    timing.monotonic_end_ns,
                    timing.utc_start_at.to_rfc3339(),
                    timing.utc_end_at.to_rfc3339(),
                    timing.discontinuity_count,
                    timing.dropped_frame_count,
                    timing.created_at.to_rfc3339(),
                ],
            )
            .map_err(map_insert_error)?;
        transaction
            .commit()
            .map_err(NativeCaptureInsertError::Catalog)?;
        Ok(NativeCaptureInsertOutcome::Inserted)
    }
}

fn validated_native_capture(
    chunk: &AudioChunk,
    timing: &CaptureChunkTiming,
) -> Option<(i64, i64, i64)> {
    if chunk.source == crate::domain::AudioSource::Imported
        || timing.chunk_id != chunk.id
        || !native_chunk_path_matches(chunk)
        || timing.source_sequence < 0
        || timing.source_sample_start < 0
        || timing.source_sample_end < timing.source_sample_start
        || timing.session_start_ms < 0
        || timing.session_end_ms < timing.session_start_ms
        || timing.monotonic_start_ns < 0
        || timing.monotonic_end_ns < timing.monotonic_start_ns
        || timing.discontinuity_count < 0
        || timing.dropped_frame_count < 0
    {
        return None;
    }
    let duration_ms = timing.session_end_ms.checked_sub(timing.session_start_ms)?;
    let byte_length = i64::try_from(chunk.byte_length).ok()?;
    Some((timing.session_start_ms, duration_ms, byte_length))
}

fn native_chunk_path_matches(chunk: &AudioChunk) -> bool {
    let path = std::path::Path::new(&chunk.path);
    let mut components = path.components();
    if components.next() != Some(std::path::Component::Normal(std::ffi::OsStr::new("audio"))) {
        return false;
    }
    let Some(std::path::Component::Normal(name)) = components.next() else {
        return false;
    };
    if components.next().is_some() {
        return false;
    }
    let Some(name) = name.to_str() else {
        return false;
    };
    let Some(extension) = name
        .strip_prefix(&chunk.id)
        .and_then(|suffix| suffix.strip_prefix('.'))
    else {
        return false;
    };
    !extension.is_empty()
        && extension.len() <= 16
        && extension
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
}

fn map_insert_error(error: rusqlite::Error) -> NativeCaptureInsertError {
    match &error {
        rusqlite::Error::SqliteFailure(details, _)
            if matches!(
                details.extended_code,
                rusqlite::ffi::SQLITE_CONSTRAINT_PRIMARYKEY
                    | rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE
            ) =>
        {
            NativeCaptureInsertError::Conflict
        }
        _ => NativeCaptureInsertError::Catalog(error),
    }
}

#[allow(dead_code)]
fn capture_source_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CaptureSourceRecord> {
    let source = row.get::<_, String>(2)?;
    let created_at = row.get::<_, String>(7)?;
    Ok(CaptureSourceRecord {
        id: row.get(0)?,
        session_id: row.get(1)?,
        source: parse_source(&source),
        device_uid: row.get(3)?,
        device_name: row.get(4)?,
        channel_count: row.get(5)?,
        sample_rate_hz: row.get(6)?,
        created_at: parse_time(&created_at)?,
    })
}

fn capture_chunk_timing_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CaptureChunkTiming> {
    let utc_start_at = row.get::<_, String>(9)?;
    let utc_end_at = row.get::<_, String>(10)?;
    let created_at = row.get::<_, String>(13)?;
    Ok(CaptureChunkTiming {
        chunk_id: row.get(0)?,
        capture_source_id: row.get(1)?,
        source_sequence: row.get(2)?,
        source_sample_start: row.get(3)?,
        source_sample_end: row.get(4)?,
        session_start_ms: row.get(5)?,
        session_end_ms: row.get(6)?,
        monotonic_start_ns: row.get(7)?,
        monotonic_end_ns: row.get(8)?,
        utc_start_at: parse_time(&utc_start_at)?,
        utc_end_at: parse_time(&utc_end_at)?,
        discontinuity_count: row.get(11)?,
        dropped_frame_count: row.get(12)?,
        created_at: parse_time(&created_at)?,
    })
}
