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
