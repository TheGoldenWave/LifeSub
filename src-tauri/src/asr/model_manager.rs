//! Model download, verification, safe extraction, and versioned installation.
//!
//! The model manager handles the full lifecycle of model artifacts:
//! downloading, SHA-256 verification, safe archive extraction, versioned
//! install activation, startup reconciliation, and deletion.
//!
//! ## Download pipeline
//!
//! ```text
//! enqueue → downloading → verifying → installing → succeeded
//!                   ↓            ↓            ↓
//!               cancelled     failed       failed
//! ```
//!
//! ## Safe extraction rules
//!
//! - Reject: absolute paths, `..` components, symlinks, hardlinks
//! - Enforce: max file count (100), max single file size (500 MB),
//!   max total expanded size (2 GB)
//! - Write immutable marker `.lifesub-model-install` after extraction
//! - fsync and atomic rename into versioned install directory
//! - Activate in SQLite transaction

use std::fs;
use std::io::{BufReader, Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use sha2::{Digest, Sha256};

use crate::catalog::{Catalog, ModelDownloadRow, ModelInstallationRow};

use super::manifest::{self, ModelManifest};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum total expanded size of an extracted archive (2 GB).
const MAX_EXPANDED_SIZE: u64 = 2_000_000_000;

/// Maximum number of files allowed in an archive.
const MAX_FILE_COUNT: usize = 100;

/// Maximum size of a single file after extraction (500 MB).
const MAX_SINGLE_FILE_SIZE: u64 = 500_000_000;

/// Name of the immutable install marker written to each install directory.
const IMMUTABLE_MARKER: &str = ".lifesub-model-install";

/// Default download timeout.
const DOWNLOAD_TIMEOUT_SECS: u64 = 3600;

/// Progress reporting interval (bytes) — persist to DB every N bytes.
const PROGRESS_INTERVAL_BYTES: u64 = 1_048_576; // 1 MiB

// ---------------------------------------------------------------------------
// Model download state (mirrors model_downloads table)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadState {
    Queued,
    Downloading,
    Verifying,
    Installing,
    Succeeded,
    Failed,
    Cancelled,
}

/// A model download record, as persisted in and read from model_downloads.
#[derive(Clone, Debug, serde::Serialize)]
pub struct ModelDownload {
    pub id: String,
    pub model_id: String,
    pub manifest_version: String,
    pub archive_sha256: String,
    pub state: DownloadState,
    pub downloaded_bytes: u64,
    pub expected_bytes: u64,
    pub temp_path: Option<String>,
    pub error_code: Option<String>,
    pub error_summary: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

// ---------------------------------------------------------------------------
// Model installation state (mirrors model_installations table)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallState {
    Ready,
    Corrupt,
    Deleting,
}

/// A model installation record.
#[derive(Clone, Debug, serde::Serialize)]
pub struct ModelInstallation {
    pub model_id: String,
    pub provider: String,
    pub manifest_version: String,
    pub archive_sha256: String,
    pub install_dir: String,
    pub state: InstallState,
    pub installed_at: String,
    pub last_error_code: Option<String>,
}

// ---------------------------------------------------------------------------
// Model manager errors
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModelManagerError {
    ManifestNotFound(String),
    InsufficientDiskSpace {
        needed: u64,
        available: u64,
    },
    DownloadFailed(String),
    IntegrityCheckFailed(String),
    ExtractionFailed(String),
    InstallFailed(String),
    ModelInUse(String),
    IoError(String),
    CatalogError(String),
    ActiveDownloadExists(String),
    InvalidArchive(String),
    RedirectDisallowed(String),
}

impl std::fmt::Display for ModelManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ManifestNotFound(m) => write!(f, "manifest not found: {}", m),
            Self::InsufficientDiskSpace { needed, available } => write!(f, "insufficient disk space: need {}, have {}", needed, available),
            Self::DownloadFailed(m) => write!(f, "download failed: {}", m),
            Self::IntegrityCheckFailed(m) => write!(f, "integrity check failed: {}", m),
            Self::ExtractionFailed(m) => write!(f, "extraction failed: {}", m),
            Self::InstallFailed(m) => write!(f, "install failed: {}", m),
            Self::ModelInUse(m) => write!(f, "model in use: {}", m),
            Self::IoError(m) => write!(f, "io error: {}", m),
            Self::CatalogError(m) => write!(f, "catalog error: {}", m),
            Self::ActiveDownloadExists(m) => write!(f, "active download exists for: {}", m),
            Self::InvalidArchive(m) => write!(f, "invalid archive: {}", m),
            Self::RedirectDisallowed(m) => write!(f, "redirect disallowed: {}", m),
        }
    }
}

// ---------------------------------------------------------------------------
// Model manager
// ---------------------------------------------------------------------------

/// Manages model downloads, installation, reconciliation, and deletion.
pub struct ModelManager {
    catalog: Arc<Catalog>,
    models_dir: PathBuf,
    downloads_dir: PathBuf,
    staging_dir: PathBuf,
}

impl ModelManager {
    /// Create a new model manager.
    ///
    /// `data_dir` is the app data directory (e.g. `~/.lifesub`).
    pub fn new(catalog: Arc<Catalog>, data_dir: &Path) -> Self {
        Self {
            catalog,
            models_dir: data_dir.join("models").join("asr"),
            downloads_dir: data_dir.join("downloads"),
            staging_dir: data_dir.join("models").join(".staging"),
        }
    }

    /// Ensure required directories exist.
    pub fn ensure_dirs(&self) -> Result<(), ModelManagerError> {
        fs::create_dir_all(&self.models_dir)
            .map_err(|e| ModelManagerError::IoError(e.to_string()))?;
        fs::create_dir_all(&self.downloads_dir)
            .map_err(|e| ModelManagerError::IoError(e.to_string()))?;
        fs::create_dir_all(&self.staging_dir)
            .map_err(|e| ModelManagerError::IoError(e.to_string()))?;
        Ok(())
    }

    // -- Directory accessors (used by tests) --

    pub fn models_dir(&self) -> &Path {
        &self.models_dir
    }

    pub fn downloads_dir(&self) -> &Path {
        &self.downloads_dir
    }

    pub fn staging_dir(&self) -> &Path {
        &self.staging_dir
    }

    // -----------------------------------------------------------------------
    // Download operations
    // -----------------------------------------------------------------------

    /// Enqueue a model download.
    ///
    /// Returns the download ID on success. Fails if there is already an active
    /// download (queued, downloading, verifying, or installing) for the same model.
    pub fn enqueue_download(
        &self,
        model_id: &str,
        manifest_version: &str,
        archive_sha256: &str,
        url: &str,
        expected_bytes: u64,
    ) -> Result<String, ModelManagerError> {
        // Check for existing active download
        let active = self
            .catalog
            .list_active_downloads_for_model(model_id)
            .map_err(|e| ModelManagerError::CatalogError(e.to_string()))?;
        if !active.is_empty() {
            return Err(ModelManagerError::ActiveDownloadExists(model_id.to_string()));
        }

        let id = format!("mdl_{}", uuid::Uuid::new_v4().simple());
        let now = Utc::now().to_rfc3339();

        self.catalog
            .insert_model_download(
                &id,
                model_id,
                manifest_version,
                archive_sha256,
                "queued",
                expected_bytes,
                &now,
            )
            .map_err(|e| ModelManagerError::CatalogError(e.to_string()))?;

        Ok(id)
    }

    /// Execute a download (blocking).
    ///
    /// Downloads the archive, verifies SHA-256, extracts it safely, and
    /// activates the installation. The download row is updated through each
    /// state transition.
    pub fn download(&self, download_id: &str) -> Result<(), ModelManagerError> {
        // Load the download record
        let dl = self
            .catalog
            .get_model_download(download_id)
            .map_err(|e| ModelManagerError::CatalogError(e.to_string()))?
            .ok_or_else(|| {
                ModelManagerError::CatalogError(format!("download not found: {}", download_id))
            })?;

        if dl.state != "queued" {
            return Err(ModelManagerError::CatalogError(format!(
                "download {} is not queued (state: {})",
                download_id, dl.state
            )));
        }

        let model_id = dl.model_id.clone();
        let manifest_version = dl.manifest_version.clone();
        let expected_hash = dl.archive_sha256.clone();
        let expected_bytes = dl.expected_bytes;

        // Find the manifest to get the download URL and provider
        let manifest = manifest::find_by_id(&model_id).ok_or_else(|| {
            ModelManagerError::ManifestNotFound(model_id.clone())
        })?;

        let provider = snake_case_provider(
            manifest
                .provider
                .unwrap_or(crate::asr::settings::AsrProviderKind::SenseVoice),
        );
        let url = manifest.source.download_url;

        // Transition to downloading
        self.transition_download(download_id, DownloadState::Downloading)?;

        // Download to temp file
        let temp_path = self.downloads_dir.join(format!("{}.part", download_id));

        let result = self.download_to_file(url, &temp_path, expected_bytes, download_id);

        match result {
            Ok(()) => {}
            Err(e) => {
                // Clean up partial file
                let _ = fs::remove_file(&temp_path);
                self.fail_download(download_id, "model_download_failed", &e.to_string())?;
                return Err(e);
            }
        }

        // Transition to verifying
        self.transition_download(download_id, DownloadState::Verifying)?;

        // Read the downloaded file and verify SHA-256
        let archive_bytes = fs::read(&temp_path)
            .map_err(|e| ModelManagerError::IoError(e.to_string()))?;

        let actual_hash = hex::encode(Sha256::digest(&archive_bytes));
        if actual_hash != expected_hash {
            let _ = fs::remove_file(&temp_path);
            let msg = format!(
                "SHA-256 mismatch: expected {}, got {}",
                expected_hash, actual_hash
            );
            self.fail_download(download_id, "model_integrity_failed", &msg)?;
            return Err(ModelManagerError::IntegrityCheckFailed(msg));
        }

        // Transition to installing
        self.transition_download(download_id, DownloadState::Installing)?;

        // Extract and install
        let install_result = self.extract_and_verify(
            &archive_bytes,
            &expected_hash,
            &provider,
            &model_id,
            &manifest_version,
        );

        // Clean up temp file regardless of outcome
        let _ = fs::remove_file(&temp_path);

        match install_result {
            Ok(install_dir) => {
                // Activate in SQLite transaction
                let now = Utc::now().to_rfc3339();
                self.catalog
                    .upsert_model_installation(
                        &model_id,
                        &provider,
                        &manifest_version,
                        &expected_hash,
                        &install_dir,
                        "ready",
                        &now,
                        None,
                    )
                    .map_err(|e| ModelManagerError::CatalogError(e.to_string()))?;

                // Mark download as succeeded
                self.transition_download(download_id, DownloadState::Succeeded)?;
                Ok(())
            }
            Err(e) => {
                self.fail_download(download_id, "model_install_failed", &e.to_string())?;
                Err(e)
            }
        }
    }

    /// Cancel a download.
    ///
    /// Only downloads in `queued`, `downloading`, `verifying`, or `installing`
    /// state can be cancelled. Already finished or failed downloads are
    /// unaffected.
    pub fn cancel_download(&self, download_id: &str) -> Result<(), ModelManagerError> {
        self.catalog
            .cancel_model_download(download_id)
            .map_err(|e| ModelManagerError::CatalogError(e.to_string()))
    }

    // -----------------------------------------------------------------------
    // Safe extraction
    // -----------------------------------------------------------------------

    /// Extract and verify an archive, returning the versioned install directory.
    ///
    /// This is the core safe extraction routine. It:
    /// 1. Decompresses the bzip2 archive
    /// 2. Iterates tar entries, rejecting unsafe paths
    /// 3. Enforces size limits
    /// 4. Extracts to staging directory
    /// 5. Writes immutable marker
    /// 6. Fsyncs and atomically renames to the versioned install directory
    pub fn extract_and_verify(
        &self,
        archive_bytes: &[u8],
        archive_hash: &str,
        provider: &str,
        model_id: &str,
        manifest_version: &str,
    ) -> Result<String, ModelManagerError> {
        // Build the versioned install path
        let install_dir_name = format!("{}-{}", manifest_version, archive_hash);
        let install_dir = self
            .models_dir
            .join(provider)
            .join(model_id)
            .join(&install_dir_name);

        // Create staging directory
        let staging_id = uuid::Uuid::new_v4().simple().to_string();
        let staging_dir = self.staging_dir.join(&staging_id);

        fs::create_dir_all(&staging_dir)
            .map_err(|e| ModelManagerError::IoError(e.to_string()))?;

        // Decompress and extract
        let result = self.extract_tar_bz2(archive_bytes, &staging_dir);

        match result {
            Ok(()) => {
                // Write immutable marker
                let marker_content = format!(
                    "{}\n{}\n{}\n{}\n",
                    provider, model_id, manifest_version, archive_hash
                );
                let marker_path = staging_dir.join(IMMUTABLE_MARKER);
                fs::write(&marker_path, &marker_content)
                    .map_err(|e| ModelManagerError::IoError(e.to_string()))?;

                // Fsync marker and staging directory
                Self::fsync_file(&marker_path)?;
                Self::fsync_dir(&staging_dir)?;

                // Create parent directories for the install target
                if let Some(parent) = install_dir.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|e| ModelManagerError::IoError(e.to_string()))?;
                }

                // Atomic rename from staging to install directory
                fs::rename(&staging_dir, &install_dir).map_err(|e| {
                    ModelManagerError::IoError(format!(
                        "rename staging to install dir failed: {}",
                        e
                    ))
                })?;

                // Fsync the parent directory to ensure the rename is durable
                if let Some(parent) = install_dir.parent() {
                    Self::fsync_dir(parent)?;
                }

                Ok(install_dir.to_string_lossy().to_string())
            }
            Err(e) => {
                // Clean up staging on failure
                let _ = fs::remove_dir_all(&staging_dir);
                Err(e)
            }
        }
    }

    /// Extract a tar.bz2 archive to the staging directory with safety checks.
    fn extract_tar_bz2(&self, archive_bytes: &[u8], dest: &Path) -> Result<(), ModelManagerError> {
        let cursor = std::io::Cursor::new(archive_bytes);
        let decoder = bzip2::read::BzDecoder::new(cursor);
        let mut archive = tar::Archive::new(decoder);

        let mut total_size: u64 = 0;
        let mut file_count: usize = 0;

        for entry_result in archive.entries().map_err(|e| {
            ModelManagerError::ExtractionFailed(format!("cannot read archive entries: {}", e))
        })? {
            let mut entry = entry_result.map_err(|e| {
                ModelManagerError::ExtractionFailed(format!("cannot read entry: {}", e))
            })?;

            let path = entry
                .path()
                .map_err(|e| {
                    ModelManagerError::ExtractionFailed(format!("cannot read entry path: {}", e))
                })?
                .to_path_buf();

            // -- Safety checks --

            // Reject absolute paths
            if path.is_absolute() {
                return Err(ModelManagerError::InvalidArchive(format!(
                    "absolute path forbidden: {}",
                    path.display()
                )));
            }

            // Reject parent directory traversal
            if path
                .components()
                .any(|c| c == std::path::Component::ParentDir)
            {
                return Err(ModelManagerError::InvalidArchive(format!(
                    "parent directory traversal forbidden: {}",
                    path.display()
                )));
            }

            // Reject symlinks and hardlinks
            let entry_type = entry.header().entry_type();
            if entry_type.is_symlink() || entry_type.is_hard_link() {
                return Err(ModelManagerError::InvalidArchive(format!(
                    "symlink or hardlink forbidden: {}",
                    path.display()
                )));
            }

            // Only process regular files and directories
            if entry_type.is_dir() {
                let target = dest.join(&path);
                fs::create_dir_all(&target)
                    .map_err(|e| ModelManagerError::IoError(e.to_string()))?;
                continue;
            }

            if !entry_type.is_file() {
                continue; // Skip other entry types
            }

            // Check file count
            file_count += 1;
            if file_count > MAX_FILE_COUNT {
                return Err(ModelManagerError::InvalidArchive(format!(
                    "too many files: {} (max {})",
                    file_count, MAX_FILE_COUNT
                )));
            }

            // Check single file size
            let size = entry.size();
            if size > MAX_SINGLE_FILE_SIZE {
                return Err(ModelManagerError::InvalidArchive(format!(
                    "file too large: {} bytes (max {})",
                    size, MAX_SINGLE_FILE_SIZE
                )));
            }

            // Check total expanded size
            total_size += size;
            if total_size > MAX_EXPANDED_SIZE {
                return Err(ModelManagerError::InvalidArchive(format!(
                    "total expanded size exceeds limit: {} bytes (max {})",
                    total_size, MAX_EXPANDED_SIZE
                )));
            }

            // Check that the resolved path is within dest
            let target = dest.join(&path);
            let canonical_dest = dest
                .canonicalize()
                .map_err(|e| ModelManagerError::IoError(e.to_string()))?;
            // The target may not exist yet, so canonicalize its parent
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| ModelManagerError::IoError(e.to_string()))?;
            }
            let canonical_target = target
                .canonicalize()
                .unwrap_or_else(|_| target.clone());
            if !canonical_target.starts_with(&canonical_dest) {
                return Err(ModelManagerError::InvalidArchive(format!(
                    "path escapes extraction directory: {}",
                    path.display()
                )));
            }

            // Extract the file
            entry.unpack(&target).map_err(|e| {
                ModelManagerError::ExtractionFailed(format!(
                    "cannot extract {}: {}",
                    path.display(),
                    e
                ))
            })?;
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Installation management
    // -----------------------------------------------------------------------

    /// List all model installations.
    pub fn list_installations(&self) -> Result<Vec<ModelInstallation>, ModelManagerError> {
        let rows = self
            .catalog
            .list_model_installations()
            .map_err(|e| ModelManagerError::CatalogError(e.to_string()))?;
        Ok(rows.into_iter().map(|r| row_to_installation(&r)).collect())
    }

    /// Get a single model installation by model ID.
    pub fn get_installation(
        &self,
        model_id: &str,
    ) -> Result<Option<ModelInstallation>, ModelManagerError> {
        let row = self
            .catalog
            .get_model_installation(model_id)
            .map_err(|e| ModelManagerError::CatalogError(e.to_string()))?;
        Ok(row.map(|r| row_to_installation(&r)))
    }

    /// Delete a model installation.
    ///
    /// Removes the install directory from disk and the record from the database.
    pub fn delete_model(&self, model_id: &str) -> Result<(), ModelManagerError> {
        // Mark as deleting first
        self.catalog
            .update_installation_state(model_id, "deleting", None)
            .map_err(|e| ModelManagerError::CatalogError(e.to_string()))?;

        let install = self
            .catalog
            .get_model_installation(model_id)
            .map_err(|e| ModelManagerError::CatalogError(e.to_string()))?
            .ok_or_else(|| {
                ModelManagerError::CatalogError(format!("model not installed: {}", model_id))
            })?;

        // Remove the install directory
        let install_path = Path::new(&install.install_dir);
        if install_path.exists() {
            fs::remove_dir_all(install_path)
                .map_err(|e| ModelManagerError::IoError(e.to_string()))?;
        }

        // Remove the database record
        self.catalog
            .delete_model_installation(model_id)
            .map_err(|e| ModelManagerError::CatalogError(e.to_string()))?;

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Startup reconciliation
    // -----------------------------------------------------------------------

    /// Reconcile model installations at startup.
    ///
    /// Performs the following cleanup:
    /// 1. Remove stale `.part` files from downloads directory
    /// 2. Remove stale staging directories
    /// 3. Detect unrecorded installs (orphan directories with markers) → mark corrupt
    /// 4. Detect missing active directories (DB record exists but dir missing) → mark corrupt
    /// 5. Mark any active downloads from previous sessions as failed/recovery_required
    pub fn reconcile(&self) -> Result<(), ModelManagerError> {
        self.ensure_dirs()?;

        // 1. Clean stale .part files
        self.clean_stale_part_files()?;

        // 2. Clean stale staging directories
        self.clean_stale_staging()?;

        // 3. Mark active downloads from previous sessions as failed
        self.fail_stale_downloads()?;

        // 4. Reconcile installations: detect orphans and missing dirs
        self.reconcile_installations()?;

        Ok(())
    }

    /// Remove all `.part` files from the downloads directory.
    fn clean_stale_part_files(&self) -> Result<(), ModelManagerError> {
        if !self.downloads_dir.exists() {
            return Ok(());
        }

        let entries = fs::read_dir(&self.downloads_dir)
            .map_err(|e| ModelManagerError::IoError(e.to_string()))?;

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "part") {
                let _ = fs::remove_file(&path);
            }
        }
        Ok(())
    }

    /// Remove all staging directories.
    fn clean_stale_staging(&self) -> Result<(), ModelManagerError> {
        if !self.staging_dir.exists() {
            return Ok(());
        }

        let entries = fs::read_dir(&self.staging_dir)
            .map_err(|e| ModelManagerError::IoError(e.to_string()))?;

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            let path = entry.path();
            if path.is_dir() {
                let _ = fs::remove_dir_all(&path);
            }
        }
        Ok(())
    }

    /// Mark any queued/downloading/verifying/installing downloads as failed.
    fn fail_stale_downloads(&self) -> Result<(), ModelManagerError> {
        let active = self
            .catalog
            .list_active_downloads()
            .map_err(|e| ModelManagerError::CatalogError(e.to_string()))?;

        for dl in active {
            self.catalog
                .fail_download_with_code(
                    &dl.id,
                    "recovery_required",
                    "Download was interrupted by previous session termination",
                )
                .map_err(|e| ModelManagerError::CatalogError(e.to_string()))?;
        }
        Ok(())
    }

    /// Reconcile installations: detect orphan directories and missing directories.
    fn reconcile_installations(&self) -> Result<(), ModelManagerError> {
        // Scan for orphan directories (directories with markers but no DB record)
        self.detect_orphan_installations()?;

        // Check DB records for missing directories
        self.detect_missing_installations()?;

        Ok(())
    }

    /// Find directories with immutable markers that aren't in the database and mark
    /// them as corrupt installations.
    fn detect_orphan_installations(&self) -> Result<(), ModelManagerError> {
        if !self.models_dir.exists() {
            return Ok(());
        }

        // Walk models/asr/<provider>/<model-id>/<version-hash>/
        let provider_dirs = match fs::read_dir(&self.models_dir) {
            Ok(dirs) => dirs,
            Err(_) => return Ok(()),
        };

        for provider_entry in provider_dirs {
            let provider_entry = match provider_entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            let provider_path = provider_entry.path();
            if !provider_path.is_dir() {
                continue;
            }
            let provider = provider_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            let model_dirs = match fs::read_dir(&provider_path) {
                Ok(dirs) => dirs,
                Err(_) => continue,
            };

            for model_entry in model_dirs {
                let model_entry = match model_entry {
                    Ok(e) => e,
                    Err(_) => continue,
                };
                let model_path = model_entry.path();
                if !model_path.is_dir() {
                    continue;
                }
                let model_id = model_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                let version_dirs = match fs::read_dir(&model_path) {
                    Ok(dirs) => dirs,
                    Err(_) => continue,
                };

                for version_entry in version_dirs {
                    let version_entry = match version_entry {
                        Ok(e) => e,
                        Err(_) => continue,
                    };
                    let version_path = version_entry.path();
                    if !version_path.is_dir() {
                        continue;
                    }

                    let marker_path = version_path.join(IMMUTABLE_MARKER);
                    if !marker_path.exists() {
                        continue;
                    }

                    // Check if there's a DB record for this install dir
                    let install_dir_str = version_path.to_string_lossy().to_string();
                    let existing = self
                        .catalog
                        .find_installation_by_dir(&install_dir_str)
                        .map_err(|e| ModelManagerError::CatalogError(e.to_string()))?;

                    if existing.is_none() {
                        // Orphan: insert as corrupt
                        let marker_content = fs::read_to_string(&marker_path).unwrap_or_default();
                        let lines: Vec<&str> = marker_content.lines().collect();
                        let manifest_version = lines.get(2).unwrap_or(&"unknown").to_string();
                        let archive_hash = lines.get(3).unwrap_or(&"unknown").to_string();
                        let now = Utc::now().to_rfc3339();

                        let _ = self.catalog.upsert_model_installation(
                            &model_id,
                            &provider,
                            &manifest_version,
                            &archive_hash,
                            &install_dir_str,
                            "corrupt",
                            &now,
                            Some("unrecorded_install"),
                        );
                    }
                }
            }
        }

        Ok(())
    }

    /// Find DB installation records whose directories don't exist on disk.
    fn detect_missing_installations(&self) -> Result<(), ModelManagerError> {
        let installations = self
            .catalog
            .list_model_installations()
            .map_err(|e| ModelManagerError::CatalogError(e.to_string()))?;

        for install in installations {
            if install.state == "corrupt"
                || install.state == "deleting"
            {
                continue;
            }

            let install_path = Path::new(&install.install_dir);
            let marker_path = install_path.join(IMMUTABLE_MARKER);

            if !install_path.exists() || !marker_path.exists() {
                self.catalog
                    .update_installation_state(
                        &install.model_id,
                        "corrupt",
                        Some("missing_install_dir"),
                    )
                    .map_err(|e| ModelManagerError::CatalogError(e.to_string()))?;
            }
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    fn transition_download(
        &self,
        download_id: &str,
        state: DownloadState,
    ) -> Result<(), ModelManagerError> {
        let state_str = download_state_name(&state);
        self.catalog
            .update_download_state(download_id, state_str)
            .map_err(|e| ModelManagerError::CatalogError(e.to_string()))
    }

    fn fail_download(
        &self,
        download_id: &str,
        error_code: &str,
        error_summary: &str,
    ) -> Result<(), ModelManagerError> {
        self.catalog
            .fail_download_with_code(download_id, error_code, error_summary)
            .map_err(|e| ModelManagerError::CatalogError(e.to_string()))
    }

    fn download_to_file(
        &self,
        url: &str,
        dest: &Path,
        expected_bytes: u64,
        download_id: &str,
    ) -> Result<(), ModelManagerError> {
        let client = reqwest::blocking::Client::builder()
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                // Only allow redirects to known hosts
                if attempt.previous().len() >= 5 {
                    return attempt.error("too many redirects");
                }
                let url = attempt.url();
                let host = url.host_str().unwrap_or("");
                // Check against the manifest allowlist
                if !is_allowed_redirect_host(host) {
                    return attempt.error("redirect to disallowed host");
                }
                attempt.follow()
            }))
            .timeout(Duration::from_secs(DOWNLOAD_TIMEOUT_SECS))
            .build()
            .map_err(|e| ModelManagerError::DownloadFailed(e.to_string()))?;

        let response = client
            .get(url)
            .send()
            .map_err(|e| ModelManagerError::DownloadFailed(format!("HTTP request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(ModelManagerError::DownloadFailed(format!(
                "HTTP {}",
                response.status()
            )));
        }

        // Verify Content-Length matches expected
        if let Some(content_length) = response.content_length() {
            if content_length != expected_bytes {
                return Err(ModelManagerError::DownloadFailed(format!(
                    "Content-Length mismatch: header says {}, expected {}",
                    content_length, expected_bytes
                )));
            }
        }

        // Write to temp file
        let mut file = fs::File::create(dest)
            .map_err(|e| ModelManagerError::IoError(e.to_string()))?;

        let mut reader = response;
        let mut buffer = [0u8; 8192];
        let mut total_read: u64 = 0;
        let mut last_progress: u64 = 0;

        loop {
            let bytes_read = reader
                .read(&mut buffer)
                .map_err(|e| ModelManagerError::DownloadFailed(format!("read error: {}", e)))?;
            if bytes_read == 0 {
                break;
            }
            file.write_all(&buffer[..bytes_read])
                .map_err(|e| ModelManagerError::IoError(e.to_string()))?;
            total_read += bytes_read as u64;

            // Persist progress at bounded intervals
            if total_read - last_progress >= PROGRESS_INTERVAL_BYTES {
                last_progress = total_read;
                let _ = self
                    .catalog
                    .update_download_progress(download_id, total_read);
            }
        }

        file.flush()
            .map_err(|e| ModelManagerError::IoError(e.to_string()))?;
        file.sync_all()
            .map_err(|e| ModelManagerError::IoError(e.to_string()))?;

        // Final progress update
        let _ = self
            .catalog
            .update_download_progress(download_id, total_read);

        Ok(())
    }

    fn fsync_file(path: &Path) -> Result<(), ModelManagerError> {
        let file = fs::File::open(path).map_err(|e| ModelManagerError::IoError(e.to_string()))?;
        file.sync_all()
            .map_err(|e| ModelManagerError::IoError(e.to_string()))
    }

    fn fsync_dir(path: &Path) -> Result<(), ModelManagerError> {
        let dir = fs::File::open(path).map_err(|e| ModelManagerError::IoError(e.to_string()))?;
        dir.sync_all()
            .map_err(|e| ModelManagerError::IoError(e.to_string()))
    }
}

// ---------------------------------------------------------------------------
// Row conversion helpers
// ---------------------------------------------------------------------------

fn row_to_download(row: &ModelDownloadRow) -> ModelDownload {
    ModelDownload {
        id: row.id.clone(),
        model_id: row.model_id.clone(),
        manifest_version: row.manifest_version.clone(),
        archive_sha256: row.archive_sha256.clone(),
        state: parse_download_state(&row.state),
        downloaded_bytes: row.downloaded_bytes,
        expected_bytes: row.expected_bytes,
        temp_path: row.temp_path.clone(),
        error_code: row.error_code.clone(),
        error_summary: row.error_summary.clone(),
        created_at: row.created_at.clone(),
        updated_at: row.updated_at.clone(),
    }
}

fn row_to_installation(row: &ModelInstallationRow) -> ModelInstallation {
    ModelInstallation {
        model_id: row.model_id.clone(),
        provider: row.provider.clone(),
        manifest_version: row.manifest_version.clone(),
        archive_sha256: row.archive_sha256.clone(),
        install_dir: row.install_dir.clone(),
        state: parse_install_state(&row.state),
        installed_at: row.installed_at.clone(),
        last_error_code: row.last_error_code.clone(),
    }
}

fn parse_download_state(value: &str) -> DownloadState {
    match value {
        "queued" => DownloadState::Queued,
        "downloading" => DownloadState::Downloading,
        "verifying" => DownloadState::Verifying,
        "installing" => DownloadState::Installing,
        "succeeded" => DownloadState::Succeeded,
        "failed" => DownloadState::Failed,
        "cancelled" => DownloadState::Cancelled,
        _ => DownloadState::Failed,
    }
}

fn parse_install_state(value: &str) -> InstallState {
    match value {
        "ready" => InstallState::Ready,
        "corrupt" => InstallState::Corrupt,
        "deleting" => InstallState::Deleting,
        _ => InstallState::Corrupt,
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn download_state_name(state: &DownloadState) -> &'static str {
    match state {
        DownloadState::Queued => "queued",
        DownloadState::Downloading => "downloading",
        DownloadState::Verifying => "verifying",
        DownloadState::Installing => "installing",
        DownloadState::Succeeded => "succeeded",
        DownloadState::Failed => "failed",
        DownloadState::Cancelled => "cancelled",
    }
}

fn snake_case_provider(kind: crate::asr::settings::AsrProviderKind) -> String {
    match kind {
        crate::asr::settings::AsrProviderKind::SenseVoice => "sense_voice".to_string(),
        crate::asr::settings::AsrProviderKind::Whisper => "whisper".to_string(),
    }
}

/// Check if a redirect host is in the global allowlist.
fn is_allowed_redirect_host(host: &str) -> bool {
    // All models in the manifest share the same set of redirect hosts.
    // We check against the known list.
    const ALLOWED_HOSTS: &[&str] = &["github.com", "objects.githubusercontent.com"];
    ALLOWED_HOSTS.contains(&host)
}

// ---------------------------------------------------------------------------
// Public reconciliation entry point (used by service)
// ---------------------------------------------------------------------------

/// Reconcile model installations at startup.
///
/// Checks for orphan .part files, unrecorded installs, missing active
/// directories, and corrupt files. Returns a list of reconciliation actions.
pub fn reconcile_models(
    manager: &ModelManager,
    catalog: &crate::catalog::Catalog,
) -> Result<Vec<String>, ModelManagerError> {
    manager.reconcile()?;
    // Return list of actions (for logging/monitoring)
    let actions = catalog
        .list_model_installations()
        .map_err(|e| ModelManagerError::CatalogError(e.to_string()))?
        .into_iter()
        .filter(|i| i.state == "corrupt")
        .map(|i| format!("model {} marked corrupt", i.model_id))
        .collect();
    Ok(actions)
}