pub mod migrations;

use std::sync::Mutex;
use chrono::{DateTime, TimeDelta, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use crate::asr::settings::AsrProviderKind;
use crate::domain::{AsrJobState, AudioChunk, AudioSource, CaptureSession, CaptureState, ChunkIntegrityState, ProvenanceStatus, TranscriptRevision, TranscriptSegment};

#[derive(Clone, Debug, serde::Serialize)]
pub struct ClaimedJob { pub job_id: String, pub chunk_id: String, pub session_id: String, pub provider: AsrProviderKind, pub model_id: String, pub state: AsrJobState, pub claimed_by: Option<String>, pub claim_generation: i64, pub attempt_count: i64, pub lease_expires_at: Option<DateTime<Utc>> }
#[derive(Clone, Debug, serde::Serialize)]
pub struct AsrJobRow { pub id: String, pub state: AsrJobState, pub attempt_count: i64, pub claim_generation: i64, pub max_attempts: i64, pub available_at: DateTime<Utc>, pub claimed_by: Option<String>, pub lease_expires_at: Option<DateTime<Utc>>, pub cancel_requested_at: Option<DateTime<Utc>>, pub error_code: Option<String>, pub error_summary: Option<String> }
#[derive(Clone, Debug, serde::Serialize)]
pub struct RecoveredJob { pub job_id: String, pub action: String }

const LEASE_DURATION_SECS: i64 = 30;
const BACKOFF_FIRST_SECS: i64 = 5;
const BACKOFF_SECOND_SECS: i64 = 30;

pub struct Catalog { connection: Mutex<Connection> }

impl Catalog {
    pub fn in_memory() -> rusqlite::Result<Self> { let c = Self { connection: Mutex::new(Connection::open_in_memory()?) }; c.migrate()?; Ok(c) }
    pub fn open(p: impl AsRef<std::path::Path>) -> rusqlite::Result<Self> { let c = Self { connection: Mutex::new(Connection::open(p)?) }; c.migrate()?; Ok(c) }
    fn migrate(&self) -> rusqlite::Result<()> { let conn = self.connection.lock().unwrap(); migrations::migrate(&conn) }

    pub fn insert_chunk(&self, chunk: &AudioChunk) -> rusqlite::Result<()> { self.connection.lock().unwrap().execute("INSERT INTO chunks(id,session_id,source,path,sha256,byte_length) VALUES(?1,?2,?3,?4,?5,?6)", params![chunk.id, chunk.session_id, source_name(chunk.source), chunk.path, chunk.sha256, chunk.byte_length])?; Ok(()) }
    pub fn chunk_integrity(&self, chunk_id: &str) -> rusqlite::Result<ChunkIntegrityState> { self.connection.lock().unwrap().query_row("SELECT integrity_state FROM chunks WHERE id=?1", [chunk_id], |row| { let s: String = row.get(0)?; Ok(parse_integrity(&s)) }) }
    pub fn update_chunk_integrity(&self, chunk_id: &str, state: ChunkIntegrityState, error_code: Option<&str>) -> rusqlite::Result<()> { let now = Utc::now().to_rfc3339(); self.connection.lock().unwrap().execute("UPDATE chunks SET integrity_state=?2,last_error_code=?3,last_error_at=?4 WHERE id=?1", params![chunk_id, integrity_name(state), error_code, now])?; Ok(()) }
    pub fn list_chunks(&self) -> rusqlite::Result<Vec<AudioChunk>> { let conn = self.connection.lock().unwrap(); let mut s = conn.prepare("SELECT id,session_id,source,path,sha256,byte_length FROM chunks")?; s.query_map([], |row| { let src: String = row.get(2)?; Ok(AudioChunk { id: row.get(0)?, session_id: row.get(1)?, source: parse_source(&src), path: row.get(3)?, sha256: row.get(4)?, byte_length: row.get(5)? }) })?.collect() }
    pub fn insert_session(&self, session: &CaptureSession) -> rusqlite::Result<()> { self.connection.lock().unwrap().execute("INSERT OR IGNORE INTO sessions(id,title,state,started_at,ended_at) VALUES(?1,?2,?3,?4,?5)", params![session.id, session.title, state_name(session.state), session.started_at.to_rfc3339(), session.ended_at.map(|v| v.to_rfc3339())])?; Ok(()) }
    pub fn update_session(&self, session: &CaptureSession) -> rusqlite::Result<()> { self.connection.lock().unwrap().execute("UPDATE sessions SET title=?2,state=?3,ended_at=?4 WHERE id=?1", params![session.id, session.title, state_name(session.state), session.ended_at.map(|v| v.to_rfc3339())])?; Ok(()) }
    pub fn append_revision(&self, session_id: &str, provider: &str, segments: Vec<TranscriptSegment>) -> rusqlite::Result<TranscriptRevision> {
        let mut conn = self.connection.lock().unwrap(); let tx = conn.transaction()?;
        let n: i64 = tx.query_row("SELECT COALESCE(MAX(number),0)+1 FROM revisions WHERE session_id=?1", [session_id], |row| row.get(0))?;
        let rev = TranscriptRevision { id: format!("tr_{}", uuid::Uuid::new_v4().simple()), session_id: session_id.to_owned(), number: n, provider: provider.to_owned(), provenance_status: ProvenanceStatus::LegacyUnverified, created_at: Utc::now(), segments };
        tx.execute("INSERT INTO revisions(id,session_id,number,provider,created_at) VALUES(?1,?2,?3,?4,?5)", params![rev.id, rev.session_id, rev.number, rev.provider, rev.created_at.to_rfc3339()])?;
        for seg in &rev.segments { tx.execute("INSERT INTO segments(id,revision_id,start_ms,end_ms,source,text) VALUES(?1,?2,?3,?4,?5,?6)", params![seg.id, rev.id, seg.start_ms, seg.end_ms, source_name(seg.source), seg.text])?; tx.execute("INSERT INTO segment_search(segment_id,revision_id,text) VALUES(?1,?2,?3)", params![seg.id, rev.id, seg.text])?; }
        tx.commit()?; Ok(rev)
    }
    pub fn list_revisions(&self, session_id: &str) -> rusqlite::Result<Vec<TranscriptRevision>> { let conn = self.connection.lock().unwrap(); let mut s = conn.prepare("SELECT id,number,provider,created_at FROM revisions WHERE session_id=?1 ORDER BY number")?; s.query_map([session_id], |row| { let ca: String = row.get(3)?; Ok(TranscriptRevision { id: row.get(0)?, session_id: session_id.to_owned(), number: row.get(1)?, provider: row.get(2)?, provenance_status: ProvenanceStatus::LegacyUnverified, created_at: parse_time(&ca)?, segments: Vec::new() }) })?.collect() }
    pub fn search_segments(&self, query: &str) -> rusqlite::Result<Vec<TranscriptSegment>> { let conn = self.connection.lock().unwrap(); let mut s = conn.prepare("SELECT s.id,s.start_ms,s.end_ms,s.source,s.text FROM segment_search f JOIN segments s ON s.id=f.segment_id AND s.revision_id=f.revision_id WHERE segment_search MATCH ?1 ORDER BY rank")?; s.query_map([query], |row| { let src: String = row.get(3)?; Ok(TranscriptSegment { id: row.get(0)?, start_ms: row.get(1)?, end_ms: row.get(2)?, source: parse_source(&src), text: row.get(4)? }) })?.collect() }

    #[allow(clippy::too_many_arguments)]
    pub fn insert_asr_job(&self, id: &str, session_id: &str, chunk_id: &str, provider: AsrProviderKind, model_id: &str, manifest_version: &str, archive_sha256: &str, required_file_hashes_json: &str, model_source_json: &str, vad_model_id: Option<&str>, vad_manifest_version: Option<&str>, vad_archive_sha256: Option<&str>, vad_required_file_hashes_json: Option<&str>, parameters_json: &str, input_sha256: &str, fingerprint: &str, state: AsrJobState, attempt_count: i64, claim_generation: i64, max_attempts: i64, available_at: DateTime<Utc>, claimed_by: Option<&str>, lease_expires_at: Option<DateTime<Utc>>, cancel_requested_at: Option<DateTime<Utc>>) -> rusqlite::Result<()> {
        let now = Utc::now().to_rfc3339();
        self.connection.lock().unwrap().execute("INSERT INTO asr_jobs(id,session_id,chunk_id,provider,model_id,manifest_version,archive_sha256,required_file_hashes_json,model_source_json,vad_model_id,vad_manifest_version,vad_archive_sha256,vad_required_file_hashes_json,parameters_json,input_sha256,fingerprint,state,attempt_count,claim_generation,max_attempts,available_at,claimed_by,lease_expires_at,cancel_requested_at,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?25)", params![id,session_id,chunk_id,provider_name(provider),model_id,manifest_version,archive_sha256,required_file_hashes_json,model_source_json,vad_model_id,vad_manifest_version,vad_archive_sha256,vad_required_file_hashes_json,parameters_json,input_sha256,fingerprint,job_state_name(state),attempt_count,claim_generation,max_attempts,available_at.to_rfc3339(),claimed_by,lease_expires_at.map(|t|t.to_rfc3339()),cancel_requested_at.map(|t|t.to_rfc3339()),now])?;
        Ok(())
    }

    pub fn claim_asr_job(&self, boot_id: &str, worker_id: &str) -> rusqlite::Result<Option<ClaimedJob>> {
        let conn = self.connection.lock().unwrap(); let now = Utc::now(); let ns = now.to_rfc3339();
        let le = (now + TimeDelta::seconds(LEASE_DURATION_SECS)).to_rfc3339(); let cb = format!("{boot_id}:{worker_id}");
        let jid: Option<String> = conn.query_row("SELECT id FROM asr_jobs WHERE state='queued' AND cancel_requested_at IS NULL AND available_at<=?1 AND (lease_expires_at IS NULL OR lease_expires_at<=?1) ORDER BY available_at ASC LIMIT 1", params![ns], |row| row.get(0)).optional()?;
        let Some(jid) = jid else { return Ok(None) };
        let aff = conn.execute("UPDATE asr_jobs SET state='preparing',claimed_by=?1,lease_expires_at=?2,attempt_count=attempt_count+1,claim_generation=claim_generation+1,updated_at=?3 WHERE id=?4 AND state='queued' AND cancel_requested_at IS NULL AND available_at<=?5 AND (lease_expires_at IS NULL OR lease_expires_at<=?5)", params![cb, le, ns, jid, ns])?;
        if aff != 1 { return Ok(None) }
        conn.query_row("SELECT id,chunk_id,session_id,provider,model_id,state,claim_generation,attempt_count,claimed_by,lease_expires_at FROM asr_jobs WHERE id=?1", params![jid], |row| { let ss: String = row.get(5)?; let ls: Option<String> = row.get(9)?; Ok(Some(ClaimedJob { job_id: row.get(0)?, chunk_id: row.get(1)?, session_id: row.get(2)?, provider: parse_provider(&row.get::<_,String>(3)?), model_id: row.get(4)?, state: parse_job_state(&ss), claim_generation: row.get(6)?, attempt_count: row.get(7)?, claimed_by: row.get(8)?, lease_expires_at: ls.and_then(|s| parse_time_opt(&s)) })) })
    }

    pub fn renew_lease(&self, job_id: &str, claimed_by: &str, claim_generation: i64) -> rusqlite::Result<bool> { let now = Utc::now(); let ns = now.to_rfc3339(); let nl = (now + TimeDelta::seconds(LEASE_DURATION_SECS)).to_rfc3339(); let aff = self.connection.lock().unwrap().execute("UPDATE asr_jobs SET lease_expires_at=?1,updated_at=?2 WHERE id=?3 AND claimed_by=?4 AND claim_generation=?5 AND lease_expires_at>?6", params![nl, ns, job_id, claimed_by, claim_generation, ns])?; Ok(aff == 1) }
    pub fn transition_job_state(&self, job_id: &str, claimed_by: &str, claim_generation: i64, new_state: AsrJobState) -> rusqlite::Result<bool> { let now = Utc::now().to_rfc3339(); let aff = self.connection.lock().unwrap().execute("UPDATE asr_jobs SET state=?1,updated_at=?2 WHERE id=?3 AND claimed_by=?4 AND claim_generation=?5", params![job_state_name(new_state), now, job_id, claimed_by, claim_generation])?; Ok(aff == 1) }

    pub fn recover_stale_jobs(&self, current_boot_id: &str) -> rusqlite::Result<Vec<RecoveredJob>> {
        let conn = self.connection.lock().unwrap(); let now = Utc::now(); let ns = now.to_rfc3339();
        let mut s = conn.prepare("SELECT id,state,attempt_count,max_attempts,claimed_by,lease_expires_at FROM asr_jobs WHERE state IN ('preparing','transcribing') AND claimed_by IS NOT NULL")?;
        let rows: Vec<(String,String,i64,i64,String,Option<String>)> = s.query_map([], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get::<_,String>(4)?,row.get(5)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        let mut recovered = Vec::new();
        for (job_id, _state, attempt_count, max_attempts, claimed_by, lease_str) in rows {
            let is_stale = match claimed_by.split(':').next() { Some(bid) if bid != current_boot_id => true, Some(_) => lease_str.and_then(|s| parse_time_opt(&s)).map_or(true, |lease| lease < now), None => true };
            if !is_stale { continue }
            if attempt_count >= max_attempts { conn.execute("UPDATE asr_jobs SET state='failed',error_code='recovery_retry_exhausted',claimed_by=NULL,lease_expires_at=NULL,updated_at=?1 WHERE id=?2", params![ns, job_id])?; recovered.push(RecoveredJob { job_id, action: "failed".to_string() }); }
            else { let bo = if attempt_count == 1 { BACKOFF_FIRST_SECS } else { BACKOFF_SECOND_SECS }; let aa = (now + TimeDelta::seconds(bo)).to_rfc3339(); conn.execute("UPDATE asr_jobs SET state='queued',claimed_by=NULL,lease_expires_at=NULL,available_at=?1,updated_at=?2 WHERE id=?3", params![aa, ns, job_id])?; recovered.push(RecoveredJob { job_id, action: "requeued".to_string() }); }
        }
        Ok(recovered)
    }

    pub fn get_job(&self, job_id: &str) -> rusqlite::Result<Option<AsrJobRow>> { self.connection.lock().unwrap().query_row("SELECT id,state,attempt_count,claim_generation,max_attempts,available_at,claimed_by,lease_expires_at,cancel_requested_at,error_code,error_summary FROM asr_jobs WHERE id=?1", params![job_id], |row| { let ss: String = row.get(1)?; let av: String = row.get(5)?; let ls: Option<String> = row.get(7)?; let cs: Option<String> = row.get(8)?; Ok(AsrJobRow { id: row.get(0)?, state: parse_job_state(&ss), attempt_count: row.get(2)?, claim_generation: row.get(3)?, max_attempts: row.get(4)?, available_at: parse_time(&av)?, claimed_by: row.get(6)?, lease_expires_at: ls.and_then(|s| parse_time_opt(&s)), cancel_requested_at: cs.and_then(|s| parse_time_opt(&s)), error_code: row.get(9)?, error_summary: row.get(10)? }) }).optional() }
    pub fn cancel_queued_blocked_job(&self, job_id: &str) -> rusqlite::Result<bool> { let now = Utc::now().to_rfc3339(); let aff = self.connection.lock().unwrap().execute("UPDATE asr_jobs SET state='cancelled',updated_at=?1 WHERE id=?2 AND state IN ('queued','blocked_model')", params![now, job_id])?; Ok(aff == 1) }
    pub fn request_cancel(&self, job_id: &str) -> rusqlite::Result<bool> { let now = Utc::now().to_rfc3339(); let aff = self.connection.lock().unwrap().execute("UPDATE asr_jobs SET cancel_requested_at=?1,updated_at=?1 WHERE id=?2 AND cancel_requested_at IS NULL", params![now, job_id])?; Ok(aff > 0 || self.job_exists(job_id)?) }
    fn job_exists(&self, job_id: &str) -> rusqlite::Result<bool> { self.connection.lock().unwrap().query_row("SELECT COUNT(*)>0 FROM asr_jobs WHERE id=?1", params![job_id], |row| row.get(0)) }
    pub fn transition_blocked_to_queued(&self) -> rusqlite::Result<usize> { let now = Utc::now().to_rfc3339(); let aff = self.connection.lock().unwrap().execute("UPDATE asr_jobs SET state='queued',updated_at=?1 WHERE state='blocked_model' AND cancel_requested_at IS NULL", params![now])?; Ok(aff) }
    pub fn complete_job(&self, job_id: &str, claimed_by: &str, claim_generation: i64) -> rusqlite::Result<bool> { let now = Utc::now().to_rfc3339(); let aff = self.connection.lock().unwrap().execute("UPDATE asr_jobs SET state='succeeded',updated_at=?1 WHERE id=?2 AND claimed_by=?3 AND claim_generation=?4 AND state='transcribing' AND cancel_requested_at IS NULL", params![now, job_id, claimed_by, claim_generation])?; Ok(aff == 1) }
    pub fn fail_job(&self, job_id: &str, claimed_by: &str, claim_generation: i64, error_code: &str, error_summary: &str) -> rusqlite::Result<bool> { let now = Utc::now().to_rfc3339(); let aff = self.connection.lock().unwrap().execute("UPDATE asr_jobs SET state='failed',error_code=?1,error_summary=?2,updated_at=?3 WHERE id=?4 AND claimed_by=?5 AND claim_generation=?6", params![error_code, error_summary, now, job_id, claimed_by, claim_generation])?; Ok(aff == 1) }
    pub fn expire_claim_for_test(&self, job_id: &str) -> rusqlite::Result<()> { let past = (Utc::now() - TimeDelta::seconds(3600)).to_rfc3339(); self.connection.lock().unwrap().execute("UPDATE asr_jobs SET lease_expires_at=?1,state='queued',claimed_by=NULL WHERE id=?2", params![past, job_id])?; Ok(()) }
}

fn state_name(s: CaptureState) -> &'static str { match s { CaptureState::Idle => "idle", CaptureState::Recording => "recording", CaptureState::Paused => "paused", CaptureState::Stopped => "stopped" } }
fn source_name(s: AudioSource) -> &'static str { match s { AudioSource::Microphone => "microphone", AudioSource::SystemAudio => "system_audio", AudioSource::Imported => "imported" } }
fn parse_source(v: &str) -> AudioSource { match v { "system_audio" => AudioSource::SystemAudio, "imported" => AudioSource::Imported, _ => AudioSource::Microphone } }
fn parse_time(v: &str) -> rusqlite::Result<DateTime<Utc>> { DateTime::parse_from_rfc3339(v).map(|t| t.with_timezone(&Utc)).map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))) }
fn parse_time_opt(v: &str) -> Option<DateTime<Utc>> { DateTime::parse_from_rfc3339(v).map(|t| t.with_timezone(&Utc)).ok() }
fn integrity_name(s: ChunkIntegrityState) -> &'static str { match s { ChunkIntegrityState::Available => "available", ChunkIntegrityState::Corrupted => "corrupted", ChunkIntegrityState::Missing => "missing" } }
fn parse_integrity(v: &str) -> ChunkIntegrityState { match v { "corrupted" => ChunkIntegrityState::Corrupted, "missing" => ChunkIntegrityState::Missing, _ => ChunkIntegrityState::Available } }
fn provider_name(p: AsrProviderKind) -> &'static str { match p { AsrProviderKind::SenseVoice => "sense_voice", AsrProviderKind::Whisper => "whisper" } }
fn parse_provider(v: &str) -> AsrProviderKind { match v { "whisper" => AsrProviderKind::Whisper, _ => AsrProviderKind::SenseVoice } }
fn job_state_name(s: AsrJobState) -> &'static str { match s { AsrJobState::Queued => "queued", AsrJobState::BlockedModel => "blocked_model", AsrJobState::Preparing => "preparing", AsrJobState::Transcribing => "transcribing", AsrJobState::Succeeded => "succeeded", AsrJobState::Failed => "failed", AsrJobState::Cancelled => "cancelled" } }
fn parse_job_state(v: &str) -> AsrJobState { match v { "blocked_model" => AsrJobState::BlockedModel, "preparing" => AsrJobState::Preparing, "transcribing" => AsrJobState::Transcribing, "succeeded" => AsrJobState::Succeeded, "failed" => AsrJobState::Failed, "cancelled" => AsrJobState::Cancelled, _ => AsrJobState::Queued } }
