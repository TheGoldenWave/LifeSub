#[cfg(test)]
use std::sync::atomic::Ordering;

use chrono::{DateTime, Utc};
use rusqlite::{OptionalExtension, params};

use crate::domain::{AsrErrorCode, AudioChunk, CaptureSession, ChunkIntegrityState};

use super::{Catalog, parse_source, parse_time, source_name, state_name};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChunkDiagnostics {
    pub integrity_state: ChunkIntegrityState,
    pub error_code: Option<AsrErrorCode>,
    pub error_at: Option<DateTime<Utc>>,
}

pub(crate) enum ImportedChunkInsertError<E> {
    Catalog(rusqlite::Error),
    Ensure(E),
}

impl Catalog {
    pub fn insert_chunk(&self, chunk: &AudioChunk) -> rusqlite::Result<()> {
        #[cfg(test)]
        if self.fail_next_chunk_insert.swap(false, Ordering::SeqCst) {
            return Err(rusqlite::Error::ExecuteReturnedResults);
        }
        self.connection.lock().unwrap().execute(
            "INSERT INTO chunks(id, session_id, source, path, sha256, byte_length) VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
            params![chunk.id, chunk.session_id, source_name(chunk.source), chunk.path, chunk.sha256, chunk.byte_length],
        )?;
        Ok(())
    }

    pub(crate) fn insert_imported_chunk<E>(
        &self,
        session: &CaptureSession,
        chunk: &AudioChunk,
        ensure_current: impl FnOnce() -> Result<(), E>,
    ) -> Result<(), ImportedChunkInsertError<E>> {
        let mut connection = self.connection.lock().unwrap();
        let transaction = connection
            .transaction()
            .map_err(ImportedChunkInsertError::Catalog)?;
        transaction
            .execute(
            "INSERT OR IGNORE INTO sessions(id, title, state, started_at, ended_at) VALUES(?1, ?2, ?3, ?4, ?5)",
            params![session.id, session.title, state_name(session.state), session.started_at.to_rfc3339(), session.ended_at.map(|value| value.to_rfc3339())],
        )
            .map_err(ImportedChunkInsertError::Catalog)?;
        #[cfg(test)]
        if self.fail_next_chunk_insert.swap(false, Ordering::SeqCst) {
            return Err(ImportedChunkInsertError::Catalog(
                rusqlite::Error::ExecuteReturnedResults,
            ));
        }
        transaction
            .execute(
            "INSERT INTO chunks(id, session_id, source, path, sha256, byte_length) VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
            params![chunk.id, chunk.session_id, source_name(chunk.source), chunk.path, chunk.sha256, chunk.byte_length],
        )
            .map_err(ImportedChunkInsertError::Catalog)?;
        ensure_current().map_err(ImportedChunkInsertError::Ensure)?;
        transaction
            .commit()
            .map_err(ImportedChunkInsertError::Catalog)
    }

    pub fn chunk(&self, id: &str) -> rusqlite::Result<Option<AudioChunk>> {
        self.connection
            .lock()
            .unwrap()
            .query_row(
                "SELECT id, session_id, source, path, sha256, byte_length FROM chunks WHERE id = ?1",
                [id],
                chunk_from_row,
            )
            .optional()
    }

    pub fn list_chunks(&self) -> rusqlite::Result<Vec<AudioChunk>> {
        let connection = self.connection.lock().unwrap();
        let mut statement = connection.prepare(
            "SELECT id, session_id, source, path, sha256, byte_length FROM chunks ORDER BY id",
        )?;
        statement.query_map([], chunk_from_row)?.collect()
    }

    pub fn chunk_integrity(&self, id: &str) -> rusqlite::Result<Option<ChunkIntegrityState>> {
        self.connection
            .lock()
            .unwrap()
            .query_row(
                "SELECT integrity_state FROM chunks WHERE id = ?1",
                [id],
                |row| parse_integrity(&row.get::<_, String>(0)?),
            )
            .optional()
    }

    pub fn chunk_diagnostics(&self, id: &str) -> rusqlite::Result<Option<ChunkDiagnostics>> {
        self.connection
            .lock()
            .unwrap()
            .query_row(
                "SELECT integrity_state, last_error_code, last_error_at FROM chunks WHERE id = ?1",
                [id],
                |row| {
                    let error_code = row.get::<_, Option<String>>(1)?;
                    let error_at = row.get::<_, Option<String>>(2)?;
                    Ok(ChunkDiagnostics {
                        integrity_state: parse_integrity(&row.get::<_, String>(0)?)?,
                        error_code: error_code.as_deref().map(parse_asr_error_code),
                        error_at: error_at.as_deref().map(parse_time).transpose()?,
                    })
                },
            )
            .optional()
    }

    pub fn update_chunk_integrity(
        &self,
        id: &str,
        state: ChunkIntegrityState,
        error_code: Option<AsrErrorCode>,
    ) -> rusqlite::Result<()> {
        let error_at = error_code.map(|_| Utc::now().to_rfc3339());
        self.connection.lock().unwrap().execute(
            "UPDATE chunks SET integrity_state = ?2, last_error_code = ?3, last_error_at = ?4 WHERE id = ?1",
            params![id, integrity_name(state), error_code.map(asr_error_name), error_at],
        )?;
        Ok(())
    }

    pub fn session_exists(&self, id: &str) -> rusqlite::Result<bool> {
        self.connection.lock().unwrap().query_row(
            "SELECT EXISTS(SELECT 1 FROM sessions WHERE id = ?1)",
            [id],
            |row| row.get(0),
        )
    }

    #[cfg(test)]
    pub fn fail_next_chunk_insert(&self) {
        self.fail_next_chunk_insert.store(true, Ordering::SeqCst);
    }

    #[cfg(test)]
    pub fn force_chunk_integrity(&self, id: &str, value: &str) -> rusqlite::Result<()> {
        let connection = self.connection.lock().unwrap();
        connection.execute_batch("PRAGMA ignore_check_constraints = ON")?;
        let result = connection.execute(
            "UPDATE chunks SET integrity_state = ?2 WHERE id = ?1",
            params![id, value],
        );
        connection.execute_batch("PRAGMA ignore_check_constraints = OFF")?;
        result.map(|_| ())
    }

    #[cfg(test)]
    pub fn session_count(&self) -> rusqlite::Result<i64> {
        self.connection
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))
    }

    #[cfg(test)]
    pub fn persisted_session_state(&self, id: &str) -> rusqlite::Result<String> {
        self.connection.lock().unwrap().query_row(
            "SELECT state FROM sessions WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
    }
}

pub(crate) fn chunk_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AudioChunk> {
    let source: String = row.get(2)?;
    Ok(AudioChunk {
        id: row.get(0)?,
        session_id: row.get(1)?,
        source: parse_source(&source),
        path: row.get(3)?,
        sha256: row.get(4)?,
        byte_length: row.get(5)?,
    })
}

fn integrity_name(state: ChunkIntegrityState) -> &'static str {
    match state {
        ChunkIntegrityState::Available => "available",
        ChunkIntegrityState::Corrupted => "corrupted",
        ChunkIntegrityState::Missing => "missing",
    }
}

pub(crate) fn parse_integrity(value: &str) -> rusqlite::Result<ChunkIntegrityState> {
    match value {
        "available" => Ok(ChunkIntegrityState::Available),
        "corrupted" => Ok(ChunkIntegrityState::Corrupted),
        "missing" => Ok(ChunkIntegrityState::Missing),
        _ => Err(rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "unknown chunk integrity state",
            )),
        )),
    }
}

fn asr_error_name(code: AsrErrorCode) -> &'static str {
    match code {
        AsrErrorCode::InputIntegrityFailed => "input_integrity_failed",
        AsrErrorCode::InputUnavailable => "input_unavailable",
        _ => "recovery_required",
    }
}

fn parse_asr_error_code(value: &str) -> AsrErrorCode {
    match value {
        "input_integrity_failed" => AsrErrorCode::InputIntegrityFailed,
        "input_unavailable" => AsrErrorCode::InputUnavailable,
        _ => AsrErrorCode::RecoveryRequired,
    }
}
