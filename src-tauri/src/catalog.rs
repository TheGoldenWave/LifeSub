pub mod migrations;

use std::sync::Mutex;

use chrono::{DateTime, TimeDelta, Utc};
use rusqlite::{Connection, OptionalExtension, params, TransactionBehavior};

use crate::asr::settings::AsrProviderKind;
use crate::domain::{
    AsrJobState, AudioChunk, AudioSource, CaptureSession, CaptureState, ChunkIntegrityState,
    ProviderReceipt, ProvenanceStatus, TranscriptRevision, TranscriptSegment,
};

#[derive(Clone, Debug)]
pub struct AsrJobRow {
    pub id: String, pub state: AsrJobState, pub attempt_count: i64, pub claim_generation: i64,
    pub max_attempts: i64, pub available_at: DateTime<Utc>, pub claimed_by: Option<String>,
    pub lease_expires_at: Option<DateTime<Utc>>, pub cancel_requested_at: Option<DateTime<Utc>>,
    pub error_code: Option<String>, pub error_summary: Option<String>,
}

pub struct Catalog { connection: Mutex<Connection> }

impl Catalog {
    pub fn in_memory() -> rusqlite::Result<Self> {
        let catalog = Self { connection: Mutex::new(Connection::open_in_memory()?) };
        catalog.migrate()?;
        Ok(catalog)
    }
    pub fn open(path: impl AsRef<std::path::Path>) -> rusqlite::Result<Self> {
        let catalog = Self { connection: Mutex::new(Connection::open(path)?) };
        catalog.migrate()?;
        Ok(catalog)
    }
    fn migrate(&self) -> rusqlite::Result<()> { let conn = self.connection.lock().unwrap(); migrations::migrate(&conn) }

    pub fn insert_chunk(&self, chunk: &AudioChunk) -> rusqlite::Result<()> {
        self.connection.lock().unwrap().execute("INSERT INTO chunks(id, session_id, source, path, sha256, byte_length) VALUES(?1, ?2, ?3, ?4, ?5, ?6)", params![chunk.id, chunk.session_id, source_name(chunk.source), chunk.path, chunk.sha256, chunk.byte_length])?;
        Ok(())
    }
    pub fn chunk_integrity(&self, chunk_id: &str) -> rusqlite::Result<ChunkIntegrityState> {
        self.connection.lock().unwrap().query_row("SELECT integrity_state FROM chunks WHERE id = ?1", [chunk_id], |row| { let s: String = row.get(0)?; Ok(parse_integrity(&s)) })
    }
    pub fn update_chunk_integrity(&self, chunk_id: &str, state: ChunkIntegrityState, error_code: Option<&str>) -> rusqlite::Result<()> {
        let now = Utc::now().to_rfc3339();
        self.connection.lock().unwrap().execute("UPDATE chunks SET integrity_state = ?2, last_error_code = ?3, last_error_at = ?4 WHERE id = ?1", params![chunk_id, integrity_name(state), error_code, now])?;
        Ok(())
    }
    pub fn list_chunks(&self) -> rusqlite::Result<Vec<AudioChunk>> {
        let c = self.connection.lock().unwrap();
        let mut s = c.prepare("SELECT id, session_id, source, path, sha256, byte_length FROM chunks")?;
        s.query_map([], |row| { let src: String = row.get(2)?; Ok(AudioChunk { id: row.get(0)?, session_id: row.get(1)?, source: parse_source(&src), path: row.get(3)?, sha256: row.get(4)?, byte_length: row.get(5)? }) })?.collect()
    }
    pub fn insert_session(&self, session: &CaptureSession) -> rusqlite::Result<()> {
        self.connection.lock().unwrap().execute("INSERT OR IGNORE INTO sessions(id, title, state, started_at, ended_at) VALUES(?1, ?2, ?3, ?4, ?5)", params![session.id, session.title, state_name(session.state), session.started_at.to_rfc3339(), session.ended_at.map(|v| v.to_rfc3339())])?;
        Ok(())
    }
    pub fn update_session(&self, session: &CaptureSession) -> rusqlite::Result<()> {
        self.connection.lock().unwrap().execute("UPDATE sessions SET title = ?2, state = ?3, ended_at = ?4 WHERE id = ?1", params![session.id, session.title, state_name(session.state), session.ended_at.map(|v| v.to_rfc3339())])?;
        Ok(())
    }
    pub fn append_revision(&self, session_id: &str, provider: &str, segments: Vec<TranscriptSegment>) -> rusqlite::Result<TranscriptRevision> {
        let mut c = self.connection.lock().unwrap();
        let tx = c.transaction()?;
        let number: i64 = tx.query_row("SELECT COALESCE(MAX(number), 0) + 1 FROM revisions WHERE session_id = ?1", [session_id], |row| row.get(0))?;
        let rev = TranscriptRevision { id: format!("tr_{}", uuid::Uuid::new_v4().simple()), session_id: session_id.to_owned(), number, provider: provider.to_owned(), provenance_status: ProvenanceStatus::LegacyUnverified, created_at: Utc::now(), segments };
        tx.execute("INSERT INTO revisions(id, session_id, number, provider, created_at) VALUES(?1, ?2, ?3, ?4, ?5)", params![rev.id, rev.session_id, rev.number, rev.provider, rev.created_at.to_rfc3339()])?;
        for seg in &rev.segments {
            tx.execute("INSERT INTO segments(id, revision_id, start_ms, end_ms, source, text) VALUES(?1, ?2, ?3, ?4, ?5, ?6)", params![seg.id, rev.id, seg.start_ms, seg.end_ms, source_name(seg.source), seg.text])?;
            tx.execute("INSERT INTO segment_search(segment_id, revision_id, text) VALUES(?1, ?2, ?3)", params![seg.id, rev.id, seg.text])?;
        }
        tx.commit()?;
        Ok(rev)
    }
    pub fn list_revisions(&self, session_id: &str) -> rusqlite::Result<Vec<TranscriptRevision>> {
        let c = self.connection.lock().unwrap();
        let mut s = c.prepare("SELECT id, number, provider, created_at FROM revisions WHERE session_id = ?1 ORDER BY number")?;
        s.query_map([session_id], |row| { let ca: String = row.get(3)?; Ok(TranscriptRevision { id: row.get(0)?, session_id: session_id.to_owned(), number: row.get(1)?, provider: row.get(2)?, provenance_status: ProvenanceStatus::LegacyUnverified, created_at: parse_time(&ca)?, segments: Vec::new() }) })?.collect::<rusqlite::Result<Vec<_>>>()
    }
    pub fn search_segments(&self, query: &str) -> rusqlite::Result<Vec<TranscriptSegment>> {
        let c = self.connection.lock().unwrap();
        let mut s = c.prepare("SELECT s.id, s.start_ms, s.end_ms, s.source, s.text FROM segment_search f JOIN segments s ON s.id = f.segment_id AND s.revision_id = f.revision_id WHERE segment_search MATCH ?1 ORDER BY rank")?;
        s.query_map([query], |row| { let src: String = row.get(3)?; Ok(TranscriptSegment { id: row.get(0)?, start_ms: row.get(1)?, end_ms: row.get(2)?, source: parse_source(&src), text: row.get(4)?, chunk_id: None, chunk_start_ms: None, chunk_end_ms: None, session_start_ms: None, session_end_ms: None }) })?.collect()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn insert_asr_job(&self, id: &str, session_id: &str, chunk_id: &str, provider: AsrProviderKind, model_id: &str, manifest_version: &str, archive_sha256: &str, required_file_hashes_json: &str, model_source_json: &str, vad_model_id: Option<&str>, vad_manifest_version: Option<&str>, vad_archive_sha256: Option<&str>, vad_required_file_hashes_json: Option<&str>, parameters_json: &str, input_sha256: &str, fingerprint: &str, state: AsrJobState, attempt_count: i64, claim_generation: i64, max_attempts: i64, available_at: DateTime<Utc>, claimed_by: Option<&str>, lease_expires_at: Option<DateTime<Utc>>, cancel_requested_at: Option<DateTime<Utc>>) -> rusqlite::Result<()> {
        let now = Utc::now().to_rfc3339();
        let p = provider_name(provider); let st = job_state_name(state);
        let av = available_at.to_rfc3339(); let le = lease_expires_at.map(|t| t.to_rfc3339()); let ca = cancel_requested_at.map(|t| t.to_rfc3339());
        self.connection.lock().unwrap().execute("INSERT INTO asr_jobs(id,session_id,chunk_id,provider,model_id,manifest_version,archive_sha256,required_file_hashes_json,model_source_json,vad_model_id,vad_manifest_version,vad_archive_sha256,vad_required_file_hashes_json,parameters_json,input_sha256,fingerprint,state,attempt_count,claim_generation,max_attempts,available_at,claimed_by,lease_expires_at,cancel_requested_at,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26)", params![id, session_id, chunk_id, p, model_id, manifest_version, archive_sha256, required_file_hashes_json, model_source_json, vad_model_id, vad_manifest_version, vad_archive_sha256, vad_required_file_hashes_json, parameters_json, input_sha256, fingerprint, st, attempt_count, claim_generation, max_attempts, av, claimed_by, le, ca, now, now])?;
        Ok(())
    }

    pub fn get_job(&self, job_id: &str) -> rusqlite::Result<Option<AsrJobRow>> {
        self.connection.lock().unwrap().query_row("SELECT id, state, attempt_count, claim_generation, max_attempts, available_at, claimed_by, lease_expires_at, cancel_requested_at, error_code, error_summary FROM asr_jobs WHERE id = ?1", params![job_id], |row| {
            let st: String = row.get(1)?; let av: String = row.get(5)?; let le: Option<String> = row.get(7)?; let ca: Option<String> = row.get(8)?;
            Ok(AsrJobRow { id: row.get(0)?, state: parse_job_state(&st), attempt_count: row.get(2)?, claim_generation: row.get(3)?, max_attempts: row.get(4)?, available_at: parse_time(&av)?, claimed_by: row.get(6)?, lease_expires_at: le.and_then(|s| parse_time_opt(&s)), cancel_requested_at: ca.and_then(|s| parse_time_opt(&s)), error_code: row.get(9)?, error_summary: row.get(10)? })
        }).optional()
    }

    pub fn request_cancel(&self, job_id: &str) -> rusqlite::Result<bool> {
        let now = Utc::now().to_rfc3339();
        let aff = self.connection.lock().unwrap().execute("UPDATE asr_jobs SET cancel_requested_at = ?2, updated_at = ?3 WHERE id = ?1 AND cancel_requested_at IS NULL", params![job_id, now, now])?;
        Ok(aff > 0 || self.job_exists(job_id)?)
    }
    fn job_exists(&self, job_id: &str) -> rusqlite::Result<bool> {
        self.connection.lock().unwrap().query_row("SELECT COUNT(*) > 0 FROM asr_jobs WHERE id = ?1", params![job_id], |row| row.get(0))
    }

    pub fn set_chunk_session_offset(&self, chunk_id: &str, offset_ms: i64) -> rusqlite::Result<()> {
        self.connection.lock().unwrap().execute("UPDATE chunks SET session_offset_ms = ?2 WHERE id = ?1", params![chunk_id, offset_ms])?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn publish_asr_revision(&self, job_id: &str, claimed_by: &str, claim_generation: i64, session_id: &str, provider: &str, receipt: &ProviderReceipt, segments: &[TranscriptSegment]) -> rusqlite::Result<TranscriptRevision> {
        let mut c = self.connection.lock().unwrap();
        let tx = c.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let fenced: bool = tx.query_row("SELECT COUNT(*) > 0 FROM asr_jobs WHERE id = ?1 AND claimed_by = ?2 AND claim_generation = ?3 AND state = 'transcribing' AND cancel_requested_at IS NULL", params![job_id, claimed_by, claim_generation], |row| row.get(0))?;
        if !fenced { return Err(rusqlite::Error::InvalidParameterName("fencing token mismatch".into())); }
        let rid = format!("rcpt_{}", uuid::Uuid::new_v4().simple());
        tx.execute("INSERT INTO provider_receipts(id, job_id, chunk_id, provider, model_id, manifest_version, archive_sha256, required_file_hashes_json, model_source_json, vad_model_id, vad_manifest_version, vad_archive_sha256, vad_required_file_hashes_json, runtime_version, runtime_build_id, parameters_json, input_sha256, started_at, finished_at, data_destination, outcome) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21)", params![rid, receipt.job_id, receipt.chunk_id, provider_name(receipt.provider), receipt.model_id, receipt.manifest_version, receipt.archive_sha256, receipt.required_file_hashes_json, receipt.model_source_json, receipt.vad_model_id, receipt.vad_manifest_version, receipt.vad_archive_sha256, receipt.vad_required_file_hashes_json, receipt.runtime_version, receipt.runtime_build_id, receipt.parameters_json, receipt.input_sha256, receipt.started_at.to_rfc3339(), receipt.finished_at.to_rfc3339(), "local_device", "succeeded"])?;
        let rev_id = format!("tr_{}", uuid::Uuid::new_v4().simple());
        let number: i64 = tx.query_row("SELECT COALESCE(MAX(number), 0) + 1 FROM revisions WHERE session_id = ?1", [session_id], |row| row.get(0))?;
        let ca = Utc::now();
        tx.execute("INSERT INTO revisions(id, session_id, number, provider, provenance_status, created_at) VALUES(?1,?2,?3,?4,?5,?6)", params![rev_id, session_id, number, provider, "verified_local_asr", ca.to_rfc3339()])?;
        tx.execute("INSERT INTO revision_receipts(revision_id, receipt_id) VALUES(?1,?2)", params![rev_id, rid])?;
        let mut pub_segs = Vec::with_capacity(segments.len());
        for seg in segments {
            tx.execute("INSERT INTO segments(id, revision_id, start_ms, end_ms, source, text, chunk_id, chunk_start_ms, chunk_end_ms, session_start_ms, session_end_ms) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)", params![seg.id, rev_id, seg.start_ms, seg.end_ms, source_name(seg.source), seg.text, seg.chunk_id, seg.chunk_start_ms, seg.chunk_end_ms, seg.session_start_ms, seg.session_end_ms])?;
            tx.execute("INSERT INTO segment_search(segment_id, revision_id, text) VALUES(?1,?2,?3)", params![seg.id, rev_id, seg.text])?;
            pub_segs.push(seg.clone());
        }
        let now = Utc::now().to_rfc3339();
        tx.execute("UPDATE asr_jobs SET state = 'succeeded', updated_at = ?2 WHERE id = ?1", params![job_id, now])?;
        tx.commit()?;
        Ok(TranscriptRevision { id: rev_id, session_id: session_id.to_owned(), number, provider: provider.to_owned(), provenance_status: ProvenanceStatus::VerifiedLocalAsr, created_at: ca, segments: pub_segs })
    }
}

fn state_name(state: CaptureState) -> &'static str { match state { CaptureState::Idle => "idle", CaptureState::Recording => "recording", CaptureState::Paused => "paused", CaptureState::Stopped => "stopped" } }
fn source_name(source: AudioSource) -> &'static str { match source { AudioSource::Microphone => "microphone", AudioSource::SystemAudio => "system_audio", AudioSource::Imported => "imported" } }
fn parse_source(value: &str) -> AudioSource { match value { "system_audio" => AudioSource::SystemAudio, "imported" => AudioSource::Imported, _ => AudioSource::Microphone } }
fn parse_time(value: &str) -> rusqlite::Result<DateTime<Utc>> { DateTime::parse_from_rfc3339(value).map(|t| t.with_timezone(&Utc)).map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))) }
fn parse_time_opt(value: &str) -> Option<DateTime<Utc>> { DateTime::parse_from_rfc3339(value).map(|t| t.with_timezone(&Utc)).ok() }
fn integrity_name(state: ChunkIntegrityState) -> &'static str { match state { ChunkIntegrityState::Available => "available", ChunkIntegrityState::Corrupted => "corrupted", ChunkIntegrityState::Missing => "missing" } }
fn parse_integrity(value: &str) -> ChunkIntegrityState { match value { "corrupted" => ChunkIntegrityState::Corrupted, "missing" => ChunkIntegrityState::Missing, _ => ChunkIntegrityState::Available } }
fn provider_name(provider: AsrProviderKind) -> &'static str { match provider { AsrProviderKind::SenseVoice => "sense_voice", AsrProviderKind::Whisper => "whisper" } }
fn parse_provider(value: &str) -> AsrProviderKind { match value { "whisper" => AsrProviderKind::Whisper, _ => AsrProviderKind::SenseVoice } }
fn job_state_name(state: AsrJobState) -> &'static str { match state { AsrJobState::Queued => "queued", AsrJobState::BlockedModel => "blocked_model", AsrJobState::Preparing => "preparing", AsrJobState::Transcribing => "transcribing", AsrJobState::Succeeded => "succeeded", AsrJobState::Failed => "failed", AsrJobState::Cancelled => "cancelled" } }
fn parse_job_state(value: &str) -> AsrJobState { match value { "blocked_model" => AsrJobState::BlockedModel, "preparing" => AsrJobState::Preparing, "transcribing" => AsrJobState::Transcribing, "succeeded" => AsrJobState::Succeeded, "failed" => AsrJobState::Failed, "cancelled" => AsrJobState::Cancelled, _ => AsrJobState::Queued } }
