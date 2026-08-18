use std::sync::Mutex;

use chrono::{DateTime, Utc};
use rusqlite::{Connection, params};

use crate::domain::{
    AudioChunk, AudioSource, CaptureSession, CaptureState, TranscriptRevision, TranscriptSegment,
};

pub struct Catalog {
    connection: Mutex<Connection>,
}

impl Catalog {
    pub fn in_memory() -> rusqlite::Result<Self> {
        let catalog = Self {
            connection: Mutex::new(Connection::open_in_memory()?),
        };
        catalog.migrate()?;
        Ok(catalog)
    }

    pub fn open(path: impl AsRef<std::path::Path>) -> rusqlite::Result<Self> {
        let catalog = Self {
            connection: Mutex::new(Connection::open(path)?),
        };
        catalog.migrate()?;
        Ok(catalog)
    }

    fn migrate(&self) -> rusqlite::Result<()> {
        self.connection.lock().unwrap().execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS sessions (
               id TEXT PRIMARY KEY, title TEXT NOT NULL, state TEXT NOT NULL,
               started_at TEXT NOT NULL, ended_at TEXT
             );
             CREATE TABLE IF NOT EXISTS revisions (
               id TEXT PRIMARY KEY, session_id TEXT NOT NULL, number INTEGER NOT NULL,
               provider TEXT NOT NULL, created_at TEXT NOT NULL,
               UNIQUE(session_id, number), FOREIGN KEY(session_id) REFERENCES sessions(id)
             );
             CREATE TABLE IF NOT EXISTS segments (
               id TEXT NOT NULL, revision_id TEXT NOT NULL, start_ms INTEGER NOT NULL,
               end_ms INTEGER NOT NULL, source TEXT NOT NULL, text TEXT NOT NULL,
               PRIMARY KEY(id, revision_id), FOREIGN KEY(revision_id) REFERENCES revisions(id)
             );
             CREATE TABLE IF NOT EXISTS chunks (
               id TEXT PRIMARY KEY, session_id TEXT NOT NULL, source TEXT NOT NULL,
               path TEXT NOT NULL, sha256 TEXT NOT NULL, byte_length INTEGER NOT NULL,
               FOREIGN KEY(session_id) REFERENCES sessions(id)
             );
             CREATE VIRTUAL TABLE IF NOT EXISTS segment_search USING fts5(
               segment_id UNINDEXED, revision_id UNINDEXED, text, tokenize='trigram'
             );",
        )
    }

    pub fn insert_chunk(&self, chunk: &AudioChunk) -> rusqlite::Result<()> {
        self.connection.lock().unwrap().execute(
            "INSERT INTO chunks(id, session_id, source, path, sha256, byte_length) VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
            params![chunk.id, chunk.session_id, source_name(chunk.source), chunk.path, chunk.sha256, chunk.byte_length],
        )?;
        Ok(())
    }

    pub fn insert_session(&self, session: &CaptureSession) -> rusqlite::Result<()> {
        self.connection.lock().unwrap().execute(
            "INSERT OR IGNORE INTO sessions(id, title, state, started_at, ended_at) VALUES(?1, ?2, ?3, ?4, ?5)",
            params![session.id, session.title, state_name(session.state), session.started_at.to_rfc3339(), session.ended_at.map(|value| value.to_rfc3339())],
        )?;
        Ok(())
    }

    pub fn update_session(&self, session: &CaptureSession) -> rusqlite::Result<()> {
        self.connection.lock().unwrap().execute(
            "UPDATE sessions SET title = ?2, state = ?3, ended_at = ?4 WHERE id = ?1",
            params![
                session.id,
                session.title,
                state_name(session.state),
                session.ended_at.map(|value| value.to_rfc3339())
            ],
        )?;
        Ok(())
    }

    pub fn append_revision(
        &self,
        session_id: &str,
        provider: &str,
        segments: Vec<TranscriptSegment>,
    ) -> rusqlite::Result<TranscriptRevision> {
        let mut connection = self.connection.lock().unwrap();
        let transaction = connection.transaction()?;
        let number: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(number), 0) + 1 FROM revisions WHERE session_id = ?1",
            [session_id],
            |row| row.get(0),
        )?;
        let revision = TranscriptRevision {
            id: format!("tr_{}", uuid::Uuid::new_v4().simple()),
            session_id: session_id.to_owned(),
            number,
            provider: provider.to_owned(),
            created_at: Utc::now(),
            segments,
        };
        transaction.execute(
            "INSERT INTO revisions(id, session_id, number, provider, created_at) VALUES(?1, ?2, ?3, ?4, ?5)",
            params![revision.id, revision.session_id, revision.number, revision.provider, revision.created_at.to_rfc3339()],
        )?;
        for segment in &revision.segments {
            transaction.execute(
                "INSERT INTO segments(id, revision_id, start_ms, end_ms, source, text) VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
                params![segment.id, revision.id, segment.start_ms, segment.end_ms, source_name(segment.source), segment.text],
            )?;
            transaction.execute(
                "INSERT INTO segment_search(segment_id, revision_id, text) VALUES(?1, ?2, ?3)",
                params![segment.id, revision.id, segment.text],
            )?;
        }
        transaction.commit()?;
        Ok(revision)
    }

    pub fn list_revisions(&self, session_id: &str) -> rusqlite::Result<Vec<TranscriptRevision>> {
        let connection = self.connection.lock().unwrap();
        let mut statement = connection.prepare(
            "SELECT id, number, provider, created_at FROM revisions WHERE session_id = ?1 ORDER BY number",
        )?;
        let revisions = statement
            .query_map([session_id], |row| {
                let created_at: String = row.get(3)?;
                Ok(TranscriptRevision {
                    id: row.get(0)?,
                    session_id: session_id.to_owned(),
                    number: row.get(1)?,
                    provider: row.get(2)?,
                    created_at: parse_time(&created_at)?,
                    segments: Vec::new(),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(revisions)
    }

    pub fn search_segments(&self, query: &str) -> rusqlite::Result<Vec<TranscriptSegment>> {
        let connection = self.connection.lock().unwrap();
        let mut statement = connection.prepare(
            "SELECT s.id, s.start_ms, s.end_ms, s.source, s.text
             FROM segment_search f JOIN segments s ON s.id = f.segment_id AND s.revision_id = f.revision_id
             WHERE segment_search MATCH ?1 ORDER BY rank",
        )?;
        statement
            .query_map([query], |row| {
                let source: String = row.get(3)?;
                Ok(TranscriptSegment {
                    id: row.get(0)?,
                    start_ms: row.get(1)?,
                    end_ms: row.get(2)?,
                    source: parse_source(&source),
                    text: row.get(4)?,
                })
            })?
            .collect()
    }
}

fn state_name(state: CaptureState) -> &'static str {
    match state {
        CaptureState::Idle => "idle",
        CaptureState::Recording => "recording",
        CaptureState::Paused => "paused",
        CaptureState::Stopped => "stopped",
    }
}

fn source_name(source: AudioSource) -> &'static str {
    match source {
        AudioSource::Microphone => "microphone",
        AudioSource::SystemAudio => "system_audio",
        AudioSource::Imported => "imported",
    }
}

fn parse_source(value: &str) -> AudioSource {
    match value {
        "system_audio" => AudioSource::SystemAudio,
        "imported" => AudioSource::Imported,
        _ => AudioSource::Microphone,
    }
}

fn parse_time(value: &str) -> rusqlite::Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|time| time.with_timezone(&Utc))
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })
}
