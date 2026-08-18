use std::sync::Mutex;

#[cfg(test)]
use std::sync::atomic::{AtomicBool, AtomicU8};

use chrono::{DateTime, Utc};
use rusqlite::{Connection, params};

use crate::domain::{
    AudioSource, CaptureNote, CaptureSession, CaptureState, DictionaryCategory, DictionaryEntry,
    HourlySlot, StatsSnapshot, TranscriptRevision, TranscriptSegment, Voiceprint,
};

mod anchored_vfs;
mod chunks;
mod job_snapshot;
pub(crate) mod jobs;
pub(crate) mod migrations;
mod models;
mod publication;

pub use chunks::ChunkDiagnostics;
pub(crate) use chunks::ImportedChunkInsertError;
pub(crate) use models::ModelInstallationRecord;
pub use publication::PublicationError;
#[cfg(test)]
pub(crate) use publication::PublicationFailurePoint;

pub struct Catalog {
    instance_id: uuid::Uuid,
    connection: Mutex<Connection>,
    _anchored_vfs: Option<anchored_vfs::AnchoredVfs>,
    #[cfg(test)]
    fail_next_chunk_insert: AtomicBool,
    #[cfg(test)]
    fail_publication_at: AtomicU8,
}

pub(crate) struct AnchoredCatalogOpen {
    anchored_vfs: anchored_vfs::AnchoredVfs,
}

impl Catalog {
    pub fn in_memory() -> rusqlite::Result<Self> {
        let catalog = Self {
            instance_id: uuid::Uuid::new_v4(),
            connection: Mutex::new(Connection::open_in_memory()?),
            _anchored_vfs: None,
            #[cfg(test)]
            fail_next_chunk_insert: AtomicBool::new(false),
            #[cfg(test)]
            fail_publication_at: AtomicU8::new(0),
        };
        migrations::migrate(&mut catalog.connection.lock().unwrap())?;
        Ok(catalog)
    }

    pub fn open(path: impl AsRef<std::path::Path>) -> rusqlite::Result<Self> {
        let catalog = Self {
            instance_id: uuid::Uuid::new_v4(),
            connection: Mutex::new(Connection::open(path)?),
            _anchored_vfs: None,
            #[cfg(test)]
            fail_next_chunk_insert: AtomicBool::new(false),
            #[cfg(test)]
            fail_publication_at: AtomicU8::new(0),
        };
        migrations::migrate(&mut catalog.connection.lock().unwrap())?;
        Ok(catalog)
    }

    pub(crate) fn prepare_anchored(
        directory: &std::fs::File,
        database_name: &str,
    ) -> rusqlite::Result<AnchoredCatalogOpen> {
        let anchored_vfs = anchored_vfs::AnchoredVfs::register(directory, database_name)?;
        Ok(AnchoredCatalogOpen { anchored_vfs })
    }

    pub(crate) const fn instance_id(&self) -> uuid::Uuid {
        self.instance_id
    }
}

impl AnchoredCatalogOpen {
    pub(crate) fn open(self) -> rusqlite::Result<Catalog> {
        let flags = rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE
            | rusqlite::OpenFlags::SQLITE_OPEN_CREATE
            | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let connection = Connection::open_with_flags_and_vfs(
            self.anchored_vfs.database_path(),
            flags,
            self.anchored_vfs.name(),
        )?;
        connection.pragma_update(None, "temp_store", "MEMORY")?;
        let catalog = Catalog {
            instance_id: uuid::Uuid::new_v4(),
            connection: Mutex::new(connection),
            _anchored_vfs: Some(self.anchored_vfs),
            #[cfg(test)]
            fail_next_chunk_insert: AtomicBool::new(false),
            #[cfg(test)]
            fail_publication_at: AtomicU8::new(0),
        };
        migrations::migrate(&mut catalog.connection.lock().unwrap())?;
        Ok(catalog)
    }
}

impl Catalog {
    pub(crate) fn connection(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.connection.lock().unwrap()
    }

    #[cfg(test)]
    pub(crate) fn connection_mut(&mut self) -> &mut Connection {
        self.connection.get_mut().unwrap()
    }

    #[cfg(test)]
    pub(crate) fn execute_test_sql(&self, sql: &str) -> rusqlite::Result<()> {
        self.connection.lock().unwrap().execute_batch(sql)
    }

    #[cfg(test)]
    pub(crate) fn test_temp_store(&self) -> rusqlite::Result<i64> {
        self.connection
            .lock()
            .unwrap()
            .query_row("PRAGMA temp_store", [], |row| row.get(0))
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

    // ── Notes ──────────────────────────────────────────────────────────

    pub fn insert_note(&self, note: &CaptureNote) -> rusqlite::Result<()> {
        self.connection.lock().unwrap().execute(
            "INSERT INTO notes(id, session_id, content, timestamp_ms, tag, segment_id, created_at) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![note.id, note.session_id, note.content, note.timestamp_ms, note.tag, note.segment_id, note.created_at],
        )?;
        Ok(())
    }

    pub fn list_notes(&self, session_id: &str) -> rusqlite::Result<Vec<CaptureNote>> {
        let connection = self.connection.lock().unwrap();
        let mut statement = connection.prepare(
            "SELECT id, session_id, content, timestamp_ms, tag, segment_id, created_at FROM notes WHERE session_id = ?1 ORDER BY timestamp_ms",
        )?;
        statement
            .query_map([session_id], |row| {
                Ok(CaptureNote {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    content: row.get(2)?,
                    timestamp_ms: row.get(3)?,
                    tag: row.get(4)?,
                    segment_id: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })?
            .collect()
    }

    pub fn update_note(&self, note_id: &str, content: &str, tag: &str) -> rusqlite::Result<()> {
        self.connection.lock().unwrap().execute(
            "UPDATE notes SET content = ?2, tag = ?3 WHERE id = ?1",
            params![note_id, content, tag],
        )?;
        Ok(())
    }

    pub fn delete_note(&self, note_id: &str) -> rusqlite::Result<()> {
        self.connection.lock().unwrap().execute(
            "DELETE FROM notes WHERE id = ?1",
            params![note_id],
        )?;
        Ok(())
    }

    // ── Dictionary Categories ──────────────────────────────────────────

    pub fn insert_category(&self, category: &DictionaryCategory) -> rusqlite::Result<()> {
        self.connection.lock().unwrap().execute(
            "INSERT INTO dictionary_categories(id, name, scope, entry_count) VALUES(?1, ?2, ?3, ?4)",
            params![category.id, category.name, category.scope, category.entry_count],
        )?;
        Ok(())
    }

    pub fn list_categories(&self, scope: Option<&str>) -> rusqlite::Result<Vec<DictionaryCategory>> {
        let connection = self.connection.lock().unwrap();
        let (sql, params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = if let Some(s) = scope {
            (
                "SELECT id, name, scope, entry_count FROM dictionary_categories WHERE scope = ?1 ORDER BY name".into(),
                vec![Box::new(s.to_owned())],
            )
        } else {
            (
                "SELECT id, name, scope, entry_count FROM dictionary_categories ORDER BY name".into(),
                vec![],
            )
        };
        let mut statement = connection.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        statement
            .query_map(param_refs.as_slice(), |row| {
                Ok(DictionaryCategory {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    scope: row.get(2)?,
                    entry_count: row.get(3)?,
                })
            })?
            .collect()
    }

    pub fn delete_category(&self, category_id: &str) -> rusqlite::Result<()> {
        self.connection.lock().unwrap().execute(
            "DELETE FROM dictionary_categories WHERE id = ?1",
            params![category_id],
        )?;
        Ok(())
    }

    // ── Dictionary Entries ─────────────────────────────────────────────

    pub fn insert_entry(&self, entry: &DictionaryEntry) -> rusqlite::Result<()> {
        self.connection.lock().unwrap().execute(
            "INSERT INTO dictionary_entries(id, category_id, term, pinyin, aliases, note, enabled) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![entry.id, entry.category_id, entry.term, entry.pinyin, entry.aliases, entry.note, entry.enabled as i32],
        )?;
        Ok(())
    }

    pub fn list_entries(
        &self,
        category_id: &str,
        query: Option<&str>,
    ) -> rusqlite::Result<Vec<DictionaryEntry>> {
        let connection = self.connection.lock().unwrap();
        let (sql, params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = if let Some(q) = query {
            let like = format!("%{}%", q);
            (
                "SELECT id, category_id, term, pinyin, aliases, note, enabled FROM dictionary_entries WHERE category_id = ?1 AND (term LIKE ?2 OR pinyin LIKE ?2 OR aliases LIKE ?2) ORDER BY term".into(),
                vec![Box::new(category_id.to_owned()), Box::new(like)],
            )
        } else {
            (
                "SELECT id, category_id, term, pinyin, aliases, note, enabled FROM dictionary_entries WHERE category_id = ?1 ORDER BY term".into(),
                vec![Box::new(category_id.to_owned())],
            )
        };
        let mut statement = connection.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        statement
            .query_map(param_refs.as_slice(), |row| {
                let enabled: i32 = row.get(6)?;
                Ok(DictionaryEntry {
                    id: row.get(0)?,
                    category_id: row.get(1)?,
                    term: row.get(2)?,
                    pinyin: row.get(3)?,
                    aliases: row.get(4)?,
                    note: row.get(5)?,
                    enabled: enabled != 0,
                })
            })?
            .collect()
    }

    pub fn update_entry(
        &self,
        entry_id: &str,
        term: &str,
        pinyin: &str,
        aliases: &str,
        note: &str,
    ) -> rusqlite::Result<()> {
        self.connection.lock().unwrap().execute(
            "UPDATE dictionary_entries SET term = ?2, pinyin = ?3, aliases = ?4, note = ?5 WHERE id = ?1",
            params![entry_id, term, pinyin, aliases, note],
        )?;
        Ok(())
    }

    pub fn toggle_entry(&self, entry_id: &str, enabled: bool) -> rusqlite::Result<()> {
        self.connection.lock().unwrap().execute(
            "UPDATE dictionary_entries SET enabled = ?2 WHERE id = ?1",
            params![entry_id, enabled as i32],
        )?;
        Ok(())
    }

    pub fn delete_entry(&self, entry_id: &str) -> rusqlite::Result<()> {
        self.connection.lock().unwrap().execute(
            "DELETE FROM dictionary_entries WHERE id = ?1",
            params![entry_id],
        )?;
        Ok(())
    }

    // ── Voiceprints ────────────────────────────────────────────────────

    pub fn insert_voiceprint(&self, vp: &Voiceprint) -> rusqlite::Result<()> {
        self.connection.lock().unwrap().execute(
            "INSERT INTO voiceprints(id, name, embedding_path, dictionary_entry_id, sample_count, updated_at) VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
            params![vp.id, vp.name, vp.embedding_path, vp.dictionary_entry_id, vp.sample_count, vp.updated_at],
        )?;
        Ok(())
    }

    pub fn list_voiceprints(&self) -> rusqlite::Result<Vec<Voiceprint>> {
        let connection = self.connection.lock().unwrap();
        let mut statement = connection.prepare(
            "SELECT id, name, embedding_path, dictionary_entry_id, sample_count, updated_at FROM voiceprints ORDER BY name",
        )?;
        statement
            .query_map([], |row| {
                Ok(Voiceprint {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    embedding_path: row.get(2)?,
                    dictionary_entry_id: row.get(3)?,
                    sample_count: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            })?
            .collect()
    }

    pub fn rename_voiceprint(&self, vp_id: &str, name: &str) -> rusqlite::Result<()> {
        self.connection.lock().unwrap().execute(
            "UPDATE voiceprints SET name = ?2, updated_at = ?3 WHERE id = ?1",
            params![vp_id, name, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn delete_voiceprint(&self, vp_id: &str) -> rusqlite::Result<()> {
        self.connection.lock().unwrap().execute(
            "DELETE FROM voiceprints WHERE id = ?1",
            params![vp_id],
        )?;
        Ok(())
    }

    pub fn link_voiceprint_to_entry(&self, vp_id: &str, entry_id: &str) -> rusqlite::Result<()> {
        self.connection.lock().unwrap().execute(
            "UPDATE voiceprints SET dictionary_entry_id = ?2, updated_at = ?3 WHERE id = ?1",
            params![vp_id, entry_id, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    // ── Settings ───────────────────────────────────────────────────────

    pub fn get_setting(&self, key: &str) -> rusqlite::Result<Option<String>> {
        let connection = self.connection.lock().unwrap();
        let mut statement = connection.prepare(
            "SELECT value_json FROM settings WHERE key = ?1",
        )?;
        let mut rows = statement.query([key])?;
        rows.next()?.map(|row| row.get(0)).transpose()
    }

    pub fn set_setting(&self, key: &str, value_json: &str) -> rusqlite::Result<()> {
        self.connection.lock().unwrap().execute(
            "INSERT INTO settings(key, value_json, updated_at) VALUES(?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
            params![key, value_json, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    // ── Stats ──────────────────────────────────────────────────────────

    pub fn get_stats_snapshot(&self, date: Option<&str>) -> rusqlite::Result<StatsSnapshot> {
        let connection = self.connection.lock().unwrap();
        let date_filter = date.unwrap_or("");
        let mut statement = connection.prepare(
            "SELECT
                CAST(strftime('%H', started_at) AS INTEGER) AS hour,
                COALESCE(SUM(
                    CASE WHEN ended_at IS NOT NULL
                    THEN (julianday(ended_at) - julianday(started_at)) * 24 * 60
                    ELSE 0 END
                ), 0) AS minutes,
                id AS session_id,
                title
             FROM sessions
             WHERE (?1 = '' OR date(started_at) = ?1)
             GROUP BY hour, id
             ORDER BY hour",
        )?;
        let mut hourly_slots: Vec<HourlySlot> = (0..24).map(|h| HourlySlot {
            hour: h,
            minutes: 0,
            session_id: None,
            title: None,
        }).collect();
        let rows = statement.query_map([date_filter], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, f64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        for row in rows {
            let (hour, minutes, sid, title) = row?;
            if let Some(slot) = hourly_slots.get_mut(hour as usize) {
                slot.minutes += minutes.round() as i64;
                if slot.session_id.is_none() {
                    slot.session_id = Some(sid);
                    slot.title = Some(title);
                }
            }
        }
        let week_count = connection.query_row(
            "SELECT COUNT(*), COALESCE(SUM(
                CASE WHEN ended_at IS NOT NULL
                THEN (julianday(ended_at) - julianday(started_at)) * 24 * 60
                ELSE 0 END
            ), 0) FROM sessions WHERE started_at >= date('now', '-7 days')",
            [],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, f64>(1)?)),
        )?;
        let month_count = connection.query_row(
            "SELECT COUNT(*), COALESCE(SUM(
                CASE WHEN ended_at IS NOT NULL
                THEN (julianday(ended_at) - julianday(started_at)) * 24 * 60
                ELSE 0 END
            ), 0) FROM sessions WHERE started_at >= date('now', '-30 days')",
            [],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, f64>(1)?)),
        )?;
        let total_count = connection.query_row(
            "SELECT COUNT(*), COALESCE(SUM(
                CASE WHEN ended_at IS NOT NULL
                THEN (julianday(ended_at) - julianday(started_at)) * 24 * 60
                ELSE 0 END
            ), 0) FROM sessions",
            [],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, f64>(1)?)),
        )?;
        Ok(StatsSnapshot {
            hourly_slots,
            week_sessions: week_count.0,
            week_minutes: week_count.1.round() as i64,
            month_sessions: month_count.0,
            month_minutes: month_count.1.round() as i64,
            total_sessions: total_count.0,
            total_minutes: total_count.1.round() as i64,
        })
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
