use std::fs::File;
use std::path::Path;
use uuid::Uuid;
use crate::catalog::Catalog;

pub const LEASE_DURATION_SECS: u32 = 30;
pub const LEASE_RENEWAL_SECS: u32 = 5;
pub const MAX_ATTEMPTS: u32 = 3;
pub const RETRY_BACKOFF_FIRST_SECS: u32 = 5;
pub const RETRY_BACKOFF_SECOND_SECS: u32 = 30;

pub fn generate_boot_id() -> String { format!("boot_{}", Uuid::new_v4().simple()) }

pub fn acquire_worker_lock(data_dir: &Path) -> Result<File, std::io::Error> {
    let lock_path = data_dir.join("asr-worker.lock");
    let lock_file = File::create(&lock_path)?;
    fs2::FileExt::try_lock_exclusive(&lock_file).map_err(|_| std::io::Error::new(std::io::ErrorKind::WouldBlock, "another instance holds the ASR worker lock"))?;
    Ok(lock_file)
}

pub struct JobManager;

impl JobManager {
    pub fn new() -> Self { Self }
    pub fn recover_stale(&self, catalog: &Catalog, boot_id: &str) -> Result<Vec<crate::catalog::RecoveredJob>, rusqlite::Error> { catalog.recover_stale_jobs(boot_id) }
    pub fn claim(&self, catalog: &Catalog, boot_id: &str, worker_id: &str) -> Result<Option<crate::catalog::ClaimedJob>, rusqlite::Error> { catalog.claim_asr_job(boot_id, worker_id) }
    pub fn renew_lease(&self, catalog: &Catalog, job_id: &str, claimed_by: &str, claim_generation: i64) -> Result<bool, rusqlite::Error> { catalog.renew_lease(job_id, claimed_by, claim_generation) }
    pub fn transition_state(&self, catalog: &Catalog, job_id: &str, claimed_by: &str, claim_generation: i64, new_state: crate::domain::AsrJobState) -> Result<bool, rusqlite::Error> { catalog.transition_job_state(job_id, claimed_by, claim_generation, new_state) }
    pub fn complete(&self, catalog: &Catalog, job_id: &str, claimed_by: &str, claim_generation: i64) -> Result<bool, rusqlite::Error> { catalog.complete_job(job_id, claimed_by, claim_generation) }
    pub fn fail(&self, catalog: &Catalog, job_id: &str, claimed_by: &str, claim_generation: i64, error_code: &str, error_summary: &str) -> Result<bool, rusqlite::Error> { catalog.fail_job(job_id, claimed_by, claim_generation, error_code, error_summary) }
}

impl Default for JobManager { fn default() -> Self { Self::new() } }
