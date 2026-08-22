use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FullSherpaRuntimeIdentity {
    pub version: String,
    pub git_commit: String,
    pub native_archive_sha256: String,
    pub build_id: String,
}

impl FullSherpaRuntimeIdentity {
    pub fn matches(&self, observed: &Self) -> bool {
        self == observed
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceProfile {
    pub os: String,
    pub arch: String,
    pub macos_major: u16,
    pub memory_gib: u16,
    pub metal_available: bool,
    pub chip: String,
}

impl DeviceProfile {
    pub fn current() -> Self {
        Self {
            os: std::env::consts::OS.to_owned(),
            arch: std::env::consts::ARCH.to_owned(),
            macos_major: current_macos_major(),
            memory_gib: current_memory_gib(),
            metal_available: cfg!(all(target_os = "macos", target_arch = "aarch64")),
            chip: current_chip(),
        }
    }
}

#[cfg(target_os = "macos")]
fn current_macos_major() -> u16 {
    std::process::Command::new("/usr/bin/sw_vers")
        .arg("-productVersion")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|version| version.trim().split('.').next()?.parse().ok())
        .unwrap_or(0)
}

#[cfg(not(target_os = "macos"))]
const fn current_macos_major() -> u16 {
    0
}

#[cfg(target_os = "macos")]
fn current_memory_gib() -> u16 {
    let mut bytes = 0_u64;
    let mut length = std::mem::size_of::<u64>();
    let result = unsafe {
        libc::sysctlbyname(
            c"hw.memsize".as_ptr(),
            (&mut bytes as *mut u64).cast(),
            &mut length,
            std::ptr::null_mut(),
            0,
        )
    };
    if result == 0 {
        u16::try_from(bytes / (1024 * 1024 * 1024)).unwrap_or(u16::MAX)
    } else {
        0
    }
}

#[cfg(not(target_os = "macos"))]
const fn current_memory_gib() -> u16 {
    0
}

#[cfg(target_os = "macos")]
fn current_chip() -> String {
    sysctl_string("machdep.cpu.brand_string")
        .and_then(|brand| {
            brand
                .split_whitespace()
                .find(|part| part.starts_with('M'))
                .map(str::to_owned)
        })
        .unwrap_or_default()
}

#[cfg(not(target_os = "macos"))]
fn current_chip() -> String {
    String::new()
}

#[cfg(target_os = "macos")]
fn sysctl_string(name: &str) -> Option<String> {
    let name = std::ffi::CString::new(name).ok()?;
    let mut length = 0_usize;
    if unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            std::ptr::null_mut(),
            &mut length,
            std::ptr::null_mut(),
            0,
        )
    } != 0
        || length == 0
    {
        return None;
    }
    let mut bytes = vec![0_u8; length];
    if unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            bytes.as_mut_ptr().cast(),
            &mut length,
            std::ptr::null_mut(),
            0,
        )
    } != 0
    {
        return None;
    }
    bytes.truncate(length);
    if bytes.last() == Some(&0) {
        bytes.pop();
    }
    String::from_utf8(bytes).ok()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeviceRequirement {
    AnyDesktop,
    AppleSiliconMetal {
        minimum_macos_major: u16,
        minimum_memory_gib: u16,
        chip: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualificationPolicy {
    StructuralWithPinnedRuntime,
    RuntimeSmokeRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstallMode {
    Direct,
    ExtractTarBz2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactPlan {
    pub artifact_id: String,
    pub source_repository: String,
    pub source_model: String,
    pub url: String,
    pub revision: String,
    pub expected_bytes: u64,
    pub expected_sha256: String,
    pub required_path: String,
    pub install_mode: InstallMode,
    pub redirect_hosts: Vec<String>,
    pub license_spdx: String,
    pub provenance: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelInstallPlan {
    pub model_id: String,
    pub provider: String,
    pub manifest_version: String,
    pub bundle_identity: String,
    pub device: DeviceRequirement,
    pub qualification_policy: QualificationPolicy,
    pub sherpa_runtime: Option<FullSherpaRuntimeIdentity>,
    pub artifacts: Vec<ArtifactPlan>,
    pub install_contract: InstallContract,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RequiredInstalledFile {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum InstallContract {
    Archive {
        archive_root: String,
        max_scanned_entries: u64,
        max_written_file_bytes: u64,
        max_total_written_bytes: u64,
        required_files: Vec<RequiredInstalledFile>,
    },
    Direct {
        max_written_file_bytes: u64,
        max_total_written_bytes: u64,
        required_files: Vec<RequiredInstalledFile>,
    },
}

impl InstallContract {
    pub(crate) fn required_files(&self) -> &[RequiredInstalledFile] {
        match self {
            Self::Archive { required_files, .. } | Self::Direct { required_files, .. } => {
                required_files
            }
        }
    }
    pub(super) fn max_total_written_bytes(&self) -> u64 {
        match self {
            Self::Archive {
                max_total_written_bytes,
                ..
            }
            | Self::Direct {
                max_total_written_bytes,
                ..
            } => *max_total_written_bytes,
        }
    }
    pub(super) fn max_written_file_bytes(&self) -> u64 {
        match self {
            Self::Archive {
                max_written_file_bytes,
                ..
            }
            | Self::Direct {
                max_written_file_bytes,
                ..
            } => *max_written_file_bytes,
        }
    }
    #[cfg(test)]
    pub(crate) fn required_files_mut(&mut self) -> &mut Vec<RequiredInstalledFile> {
        match self {
            Self::Archive { required_files, .. } | Self::Direct { required_files, .. } => {
                required_files
            }
        }
    }
}

impl ModelInstallPlan {
    pub(crate) fn from_manifest(manifest: &ModelManifest) -> Self {
        super::fs_support::plan_from_manifest(manifest)
    }
    pub(crate) fn from_vad_manifest(manifest: &VadManifest) -> Self {
        super::fs_support::plan_from_vad_manifest(manifest)
    }
    pub(crate) fn resolve(
        model_id: &str,
        manifest_version: &str,
        bundle_identity: &str,
    ) -> Result<Self, ManagerError> {
        let plan = super::fs_support::resolve_current_plan(model_id)?;
        if plan.manifest_version != manifest_version || plan.bundle_identity != bundle_identity {
            return Err(ManagerError::structural("model manifest identity mismatch"));
        }
        Ok(plan)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactCheckpoint {
    pub artifact_id: String,
    pub source_identity: String,
    pub downloaded_bytes: u64,
    pub expected_bytes: u64,
    pub temp_path: PathBuf,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub verified_sha256: Option<String>,
    pub state: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredInstallation {
    pub model_id: String,
    pub provider: String,
    pub manifest_version: String,
    pub bundle_identity: String,
    pub install_dir: PathBuf,
    pub state: String,
    pub runtime_identity_json: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DeletionLease {
    pub model_id: String,
    pub install_dir: PathBuf,
    pub prior_state: String,
    pub prior_runtime_identity_json: Option<String>,
    pub prior_qualified_at: Option<String>,
    pub prior_last_error_code: Option<String>,
}

pub trait ModelCatalog: Send + Sync + 'static {
    fn begin_download(&self, plan: &ModelInstallPlan) -> Result<String, ManagerError>;
    fn checkpoint(
        &self,
        download_id: &str,
        artifact_id: &str,
    ) -> Result<Option<ArtifactCheckpoint>, ManagerError>;
    fn save_checkpoint(
        &self,
        download_id: &str,
        checkpoint: &ArtifactCheckpoint,
    ) -> Result<(), ManagerError>;
    fn set_download_state(
        &self,
        download_id: &str,
        state: &str,
        error_code: Option<&str>,
    ) -> Result<(), ManagerError>;
    fn publish_installation(&self, installation: &StoredInstallation) -> Result<(), ManagerError>;
    fn record_installation_recovery(
        &self,
        model_id: &str,
        error_code: &str,
    ) -> Result<(), ManagerError>;
    fn begin_delete(&self, model_id: &str) -> Result<Option<DeletionLease>, ManagerError>;
    fn finish_delete(&self, lease: &DeletionLease) -> Result<(), ManagerError>;
    fn abort_delete(&self, lease: &DeletionLease) -> Result<(), ManagerError>;
}

impl<T: ModelCatalog> ModelCatalog for std::sync::Arc<T> {
    fn begin_download(&self, plan: &ModelInstallPlan) -> Result<String, ManagerError> {
        (**self).begin_download(plan)
    }

    fn checkpoint(
        &self,
        download_id: &str,
        artifact_id: &str,
    ) -> Result<Option<ArtifactCheckpoint>, ManagerError> {
        (**self).checkpoint(download_id, artifact_id)
    }

    fn save_checkpoint(
        &self,
        download_id: &str,
        checkpoint: &ArtifactCheckpoint,
    ) -> Result<(), ManagerError> {
        (**self).save_checkpoint(download_id, checkpoint)
    }

    fn set_download_state(
        &self,
        download_id: &str,
        state: &str,
        error_code: Option<&str>,
    ) -> Result<(), ManagerError> {
        (**self).set_download_state(download_id, state, error_code)
    }

    fn publish_installation(&self, installation: &StoredInstallation) -> Result<(), ManagerError> {
        (**self).publish_installation(installation)
    }

    fn record_installation_recovery(
        &self,
        model_id: &str,
        error_code: &str,
    ) -> Result<(), ManagerError> {
        (**self).record_installation_recovery(model_id, error_code)
    }

    fn begin_delete(&self, model_id: &str) -> Result<Option<DeletionLease>, ManagerError> {
        (**self).begin_delete(model_id)
    }

    fn finish_delete(&self, lease: &DeletionLease) -> Result<(), ManagerError> {
        (**self).finish_delete(lease)
    }

    fn abort_delete(&self, lease: &DeletionLease) -> Result<(), ManagerError> {
        (**self).abort_delete(lease)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DownloadRequest {
    pub url: String,
    pub range_start: Option<u64>,
    pub if_range: Option<String>,
    pub redirect_hosts: Vec<String>,
}
pub struct DownloadResponse {
    pub status: u16,
    pub final_url: String,
    pub headers: BTreeMap<String, String>,
    pub body: Box<dyn Read + Send>,
}
pub trait HttpTransport: Clone + Send + Sync + 'static {
    fn execute(&self, request: &DownloadRequest) -> Result<DownloadResponse, ManagerError>;
}

#[derive(Clone)]
pub struct ReqwestTransport {
    pub(super) client: reqwest::blocking::Client,
}
impl ReqwestTransport {
    pub fn new() -> Result<Self, ManagerError> {
        Ok(Self {
            client: reqwest::blocking::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(HTTP_CONNECT_TIMEOUT)
                .timeout(HTTP_READ_TIMEOUT)
                .build()
                .map_err(|e| ManagerError::network(e.to_string()))?,
        })
    }
}
impl HttpTransport for ReqwestTransport {
    fn execute(&self, request: &DownloadRequest) -> Result<DownloadResponse, ManagerError> {
        super::download::execute_reqwest(self, request)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagerError {
    pub(super) code: &'static str,
    pub(super) detail: String,
}
impl ManagerError {
    pub fn code(&self) -> &'static str {
        self.code
    }
    pub(crate) fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }
    pub(crate) fn ownership_lost(detail: impl Into<String>) -> Self {
        Self::new("runtime_ownership_lost", detail)
    }
    pub(super) fn network(detail: impl Into<String>) -> Self {
        Self::new("model_download_failed", detail)
    }
    pub(super) fn invalid_source(detail: impl Into<String>) -> Self {
        Self::new("model_source_rejected", detail)
    }
    pub(super) fn integrity(detail: impl Into<String>) -> Self {
        Self::new("model_integrity_failed", detail)
    }
    pub(super) fn structural(detail: impl Into<String>) -> Self {
        Self::new("model_structural_incompatible", detail)
    }
    pub(crate) fn catalog(detail: impl Into<String>) -> Self {
        Self::new("model_catalog_failed", detail)
    }
}
impl std::fmt::Display for ManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.detail)
    }
}
impl std::error::Error for ManagerError {}
impl From<io::Error> for ManagerError {
    fn from(e: io::Error) -> Self {
        Self::new("model_io_failed", e.to_string())
    }
}
impl From<rusqlite::Error> for ManagerError {
    fn from(e: rusqlite::Error) -> Self {
        Self::catalog(e.to_string())
    }
}

pub(super) enum ReconcileOutcome {
    Recovered(StoredInstallation),
    RejectedDurably(ManagerError),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) enum DeleteMarkerFault {
    Write,
    Sync,
    Rename,
}
#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InstallFault {
    Assembly,
    Rename,
}

#[derive(Clone)]
pub struct ModelManager<T, C> {
    pub(super) storage: super::storage::ModelStorage,
    pub(super) transport: T,
    pub(super) catalog: C,
    pub(super) observed_sherpa_runtime: Option<FullSherpaRuntimeIdentity>,
    pub(super) execution_leases:
        std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, usize>>>,
    pub(super) runtime_ownership: Option<super::ModelRuntimeOwnership>,
    #[cfg(test)]
    pub(super) available_space_override: Option<u64>,
    #[cfg(test)]
    pub(super) delete_marker_fault: Option<DeleteMarkerFault>,
    #[cfg(test)]
    pub(super) install_fault: Option<InstallFault>,
    #[cfg(test)]
    pub(super) available_space_sequence: Option<std::sync::Arc<std::sync::Mutex<Vec<u64>>>>,
}
