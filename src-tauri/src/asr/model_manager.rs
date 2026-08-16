use std::collections::{BTreeMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use bzip2::read::BzDecoder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::asr::manifest::{
    ArtifactFile, ArtifactInstallMode, DeviceRequirement as ManifestDeviceRequirement,
    InstallConstraints as ManifestInstallConstraints, ModelManifest,
    QualificationPolicy as ManifestQualificationPolicy, RuntimeRequirement, VadManifest,
    model_registry, vad_manifest,
};
use crate::catalog::{Catalog, ModelInstallationRecord};
use crate::domain::AsrProviderKind;

const COPY_BUFFER_BYTES: usize = 64 * 1024;
const CHECKPOINT_INTERVAL_BYTES: u64 = 4 * 1024 * 1024;
const MAX_REDIRECTS: usize = 10;
const STRUCTURAL_MARKER: &str = ".lifesub-structural.json";
const DELETE_MARKER: &str = ".lifesub-delete.json";
const DISK_SAFETY_MARGIN_BYTES: u64 = 512 * 1024 * 1024;
const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const HTTP_READ_TIMEOUT: Duration = Duration::from_secs(2);

mod archive;
mod delete;
mod download;
mod fs_support;
mod install;
mod install_support;
mod reconcile;
mod types;

pub(crate) use types::DeleteMarkerFault;
#[cfg(test)]
pub(crate) use types::InstallFault;
pub use types::{
    ArtifactCheckpoint, ArtifactPlan, DeletionLease, DeviceProfile, DeviceRequirement,
    DownloadRequest, DownloadResponse, FullSherpaRuntimeIdentity, HttpTransport, InstallContract,
    InstallMode, ManagerError, ModelCatalog, ModelInstallPlan, ModelManager, QualificationPolicy,
    RequiredInstalledFile, ReqwestTransport, StoredInstallation,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExecutionInstallationRecord {
    pub model_id: String,
    pub manifest_version: String,
    pub bundle_identity: String,
    pub install_dir: PathBuf,
    pub state: String,
    pub runtime_identity_json: Option<String>,
}

#[derive(Debug)]
pub struct ExecutableInstallationLease {
    plan: ModelInstallPlan,
    install_dir: PathBuf,
    runtime_identity_json: Option<String>,
    device: DeviceProfile,
    observed_sherpa_runtime: Option<FullSherpaRuntimeIdentity>,
    _guard: ExecutionLeaseGuard,
    validation_count: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

#[derive(Debug)]
struct ExecutionLeaseGuard {
    model_id: String,
    registry: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, usize>>>,
}

impl Drop for ExecutionLeaseGuard {
    fn drop(&mut self) {
        let mut registry = self.registry.lock().unwrap();
        if let Some(count) = registry.get_mut(&self.model_id) {
            *count -= 1;
            if *count == 0 {
                registry.remove(&self.model_id);
            }
        }
    }
}

impl ExecutableInstallationLease {
    pub fn model_id(&self) -> &str {
        &self.plan.model_id
    }

    pub(crate) fn plan(&self) -> &ModelInstallPlan {
        &self.plan
    }

    pub(crate) fn install_dir(&self) -> &Path {
        &self.install_dir
    }

    pub(crate) fn runtime_identity_json(&self) -> Option<&str> {
        self.runtime_identity_json.as_deref()
    }

    pub(crate) fn device(&self) -> &DeviceProfile {
        &self.device
    }

    #[cfg(test)]
    pub(crate) fn validation_count(&self) -> usize {
        self.validation_count
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    pub(crate) fn revalidate(&self) -> Result<(), ManagerError> {
        self.validation_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        validate_executable_installation(
            &self.plan,
            &self.install_dir,
            self.observed_sherpa_runtime.as_ref(),
        )
    }

    #[cfg(test)]
    pub(crate) fn for_test(
        model_id: &str,
        install_dir: impl AsRef<Path>,
        device: crate::asr::provider::DeviceIdentity,
    ) -> Result<Self, ManagerError> {
        let manifest = model_registry()
            .model(model_id)
            .ok_or_else(|| ManagerError::structural("unknown model"))?;
        let plan = ModelInstallPlan::from_manifest(manifest);
        let registry = std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::from(
            [(model_id.to_owned(), 1)],
        )));
        Ok(Self {
            observed_sherpa_runtime: plan.sherpa_runtime.clone(),
            plan,
            install_dir: install_dir.as_ref().to_path_buf(),
            runtime_identity_json: Some("{}".to_owned()),
            device: DeviceProfile {
                os: device.os,
                arch: device.arch,
                macos_major: device.macos_major,
                memory_gib: device.memory_gib,
                metal_available: device.backend == "metal",
                chip: device.chip,
            },
            _guard: ExecutionLeaseGuard {
                model_id: model_id.to_owned(),
                registry,
            },
            validation_count: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        })
    }
}

impl<T: HttpTransport> ModelManager<T, Catalog> {
    pub fn executable_installation(
        &self,
        model_id: &str,
    ) -> Result<ExecutableInstallationLease, ManagerError> {
        let guard = self.acquire_execution_lease(model_id)?;
        let device = DeviceProfile::current();
        let record = self
            .catalog
            .execution_installation(model_id)?
            .ok_or_else(|| ManagerError::new("model_not_installed", "installation is missing"))?;
        if record.state != "runtime_qualified" {
            return Err(ManagerError::new(
                "model_runtime_unqualified",
                "installation is not runtime qualified",
            ));
        }
        let plan = ModelInstallPlan::resolve(
            &record.model_id,
            &record.manifest_version,
            &record.bundle_identity,
        )?;
        fs_support::validate_device(&plan.device, &device)?;
        let expected_dir = self.install_dir(&plan);
        if record.install_dir != expected_dir {
            return Err(ManagerError::integrity("installation path mismatch"));
        }
        let expected_runtime = plan
            .sherpa_runtime
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| ManagerError::structural(error.to_string()))?;
        match plan.qualification_policy {
            QualificationPolicy::StructuralWithPinnedRuntime
                if record.runtime_identity_json != expected_runtime =>
            {
                return Err(ManagerError::new(
                    "model_runtime_identity_mismatch",
                    "Catalog runtime identity does not match pinned sherpa runtime",
                ));
            }
            QualificationPolicy::RuntimeSmokeRequired if record.runtime_identity_json.is_none() => {
                return Err(ManagerError::new(
                    "model_runtime_unqualified",
                    "qualified runtime identity is missing",
                ));
            }
            _ => {}
        }
        let lease = ExecutableInstallationLease {
            plan,
            install_dir: record.install_dir,
            runtime_identity_json: record.runtime_identity_json,
            device,
            observed_sherpa_runtime: self.observed_sherpa_runtime.clone(),
            _guard: guard,
            validation_count: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        };
        lease.revalidate()?;
        Ok(lease)
    }
}

impl<T: HttpTransport, C: ModelCatalog> ModelManager<T, C> {
    fn acquire_execution_lease(&self, model_id: &str) -> Result<ExecutionLeaseGuard, ManagerError> {
        fs_support::validate_component("model_id", model_id)?;
        let mut registry = self.execution_leases.lock().unwrap();
        let count = registry.entry(model_id.to_owned()).or_insert(0);
        *count = count
            .checked_add(1)
            .ok_or_else(|| ManagerError::new("model_in_use", "execution lease overflow"))?;
        Ok(ExecutionLeaseGuard {
            model_id: model_id.to_owned(),
            registry: self.execution_leases.clone(),
        })
    }

    pub(crate) fn execution_lease_count(&self, model_id: &str) -> usize {
        self.execution_leases
            .lock()
            .unwrap()
            .get(model_id)
            .copied()
            .unwrap_or(0)
    }

    #[cfg(test)]
    pub(crate) fn hold_execution_lease_for_test(
        &self,
        model_id: &str,
        install_dir: impl AsRef<Path>,
    ) -> Result<ExecutableInstallationLease, ManagerError> {
        let manifest = model_registry()
            .model(model_id)
            .ok_or_else(|| ManagerError::structural("unknown model"))?;
        let plan = ModelInstallPlan::from_manifest(manifest);
        Ok(ExecutableInstallationLease {
            observed_sherpa_runtime: plan.sherpa_runtime.clone(),
            plan,
            install_dir: install_dir.as_ref().to_path_buf(),
            runtime_identity_json: Some("{}".to_owned()),
            device: DeviceProfile::current(),
            _guard: self.acquire_execution_lease(model_id)?,
            validation_count: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(1)),
        })
    }
}

fn validate_executable_installation(
    plan: &ModelInstallPlan,
    install_dir: &Path,
    observed_sherpa_runtime: Option<&FullSherpaRuntimeIdentity>,
) -> Result<(), ManagerError> {
    use install::{StructuralMarker, matches_marker};
    use install_support::{installed_records, validate_inventory};

    if !fs_support::real_dir(install_dir)? {
        return Err(ManagerError::integrity("installation directory is missing"));
    }
    let marker: StructuralMarker = serde_json::from_slice(&fs_support::read_regular(
        &install_dir.join(STRUCTURAL_MARKER),
    )?)
    .map_err(|_| ManagerError::structural("invalid structural marker"))?;
    let structural_runtime = plan
        .sherpa_runtime
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|error| ManagerError::structural(error.to_string()))?;
    if !matches_marker(&marker, plan, structural_runtime.as_deref()) {
        return Err(ManagerError::structural("structural marker mismatch"));
    }
    let actual = installed_records(install_dir)?;
    if marker.installed_files != actual {
        return Err(ManagerError::integrity(
            "installed file inventory or hash mismatch",
        ));
    }
    validate_inventory(plan, &actual)?;
    install_support::validate_structure_for_reconcile(plan, install_dir, observed_sherpa_runtime)
}

pub const fn checked_required_additional_free(
    remaining_parts: u64,
    peak_additional_assembly: u64,
    safety_margin: u64,
) -> Option<u64> {
    match remaining_parts.checked_add(peak_additional_assembly) {
        Some(value) => value.checked_add(safety_margin),
        None => None,
    }
}

pub(crate) fn safe_relative_path(value: &str) -> Result<PathBuf, ManagerError> {
    if value.is_empty() || value.contains('\\') {
        return Err(ManagerError::structural("unsafe required path"));
    }
    let path = Path::new(value);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ManagerError::structural("unsafe required path"));
    }
    Ok(path.to_path_buf())
}

pub(crate) use archive::extract_tar_bz2_safely;
#[cfg(test)]
pub(crate) use install_support::validate_required_inventory_for_test;

impl<T, C> ModelManager<T, C>
where
    T: HttpTransport,
    C: ModelCatalog + crate::asr::runtime_qualifier::QualificationCatalog,
{
    #[cfg(test)]
    pub(crate) fn runtime_qualifier_for_test<S>(
        &self,
        smoke: S,
    ) -> crate::asr::runtime_qualifier::RuntimeQualifier<&C, S>
    where
        S: crate::asr::runtime_qualifier::RuntimeSmoke,
    {
        crate::asr::runtime_qualifier::RuntimeQualifier::new(&self.catalog, smoke)
    }
}

#[cfg(all(
    feature = "asr-qwen17-runtime",
    target_os = "macos",
    target_arch = "aarch64"
))]
impl<T, C> ModelManager<T, C>
where
    T: HttpTransport,
    C: ModelCatalog + crate::asr::runtime_qualifier::QualificationCatalog,
{
    pub fn qualify_qwen17_current_device(
        &self,
        install_dir: impl AsRef<Path>,
    ) -> Result<
        crate::asr::runtime_qualifier::QualifiedRuntimeIdentity,
        crate::asr::runtime_qualifier::QualifierError,
    > {
        const MODEL_ID: &str = "qwen3-asr-1.7b";
        let manifest = model_registry().model(MODEL_ID).ok_or_else(|| {
            crate::asr::runtime_qualifier::QualifierError::new(
                "model_runtime_qualification_failed",
                "Qwen 1.7B manifest is missing",
            )
        })?;
        let plan = ModelInstallPlan::from_manifest(manifest);
        let install_dir = install_dir.as_ref();
        let device = DeviceProfile::current();
        fs_support::validate_device(&plan.device, &device).map_err(|error| {
            crate::asr::runtime_qualifier::QualifierError::new(error.code(), error.to_string())
        })?;
        validate_executable_installation(&plan, install_dir, self.observed_sherpa_runtime.as_ref())
            .map_err(|error| {
                crate::asr::runtime_qualifier::QualifierError::new(error.code(), error.to_string())
            })?;
        let current_device = crate::asr::provider::DeviceIdentity::current();
        let handle = crate::asr::runtime_qualifier::QualificationHandle::from_manifest(
            manifest,
            install_dir,
            current_device.clone(),
        );
        let smoke = crate::asr::qwen3_asr::Qwen17RuntimeSmoke::new(current_device.chip);
        crate::asr::runtime_qualifier::RuntimeQualifier::new(&self.catalog, smoke).qualify(&handle)
    }

    pub fn reconcile_qwen17_current_device(
        &self,
        install_dir: impl AsRef<Path>,
    ) -> Result<(), crate::asr::runtime_qualifier::QualifierError> {
        const MODEL_ID: &str = "qwen3-asr-1.7b";
        let manifest = model_registry().model(MODEL_ID).ok_or_else(|| {
            crate::asr::runtime_qualifier::QualifierError::new(
                "model_runtime_qualification_recovery_required",
                "Qwen 1.7B manifest is missing",
            )
        })?;
        let device = crate::asr::provider::DeviceIdentity::current();
        let handle = crate::asr::runtime_qualifier::QualificationHandle::from_manifest(
            manifest,
            install_dir,
            device.clone(),
        );
        let smoke = crate::asr::qwen3_asr::Qwen17RuntimeSmoke::new(device.chip);
        crate::asr::runtime_qualifier::RuntimeQualifier::new(&self.catalog, smoke)
            .reconcile(&handle)
    }
}
