use std::ffi::{CString, OsStr};
use std::fs::File;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::sync::atomic::AtomicBool;

use fs2::FileExt;
use sha2::{Digest, Sha256};

use crate::asr::job::{
    Clock, JobControl, JobCoordinator, JobCoordinatorReservation, JobError, JobRepository,
    SystemClock,
};
#[cfg(feature = "asr-runtime")]
use crate::asr::model_manager::FullSherpaRuntimeIdentity;
use crate::asr::model_manager::{ManagerError, ModelManager, ReqwestTransport};
use crate::catalog::Catalog;

use super::{EvidenceService, ServiceError};

const CATALOG_FILE: &str = "lifesub.sqlite3";
const RUNTIME_LOCK_FILE: &str = "asr-worker.lock";
const CORE_LOCK_PREFIX: &str = ".lifesub-core-";
const CORE_LOCK_SUFFIX: &str = ".lock";

#[derive(Debug)]
pub enum RuntimeOwnershipError {
    AlreadyOwned,
    CatalogMismatch,
    UnsafePath,
    Io(io::Error),
}

pub struct RuntimeOwnershipGuard {
    parent_container: File,
    parent_name: CString,
    parent_identity: FileIdentity,
    core_lock: File,
    core_lock_name: CString,
    core_lock_identity: FileIdentity,
    lock: File,
    data_dir: File,
    parent: File,
    data_dir_name: CString,
    data_dir_identity: FileIdentity,
    lock_name: CString,
    lock_identity: FileIdentity,
}

#[derive(Clone)]
pub(crate) struct DataDirectoryCapability(std::sync::Arc<File>);

impl DataDirectoryCapability {
    pub(super) fn into_file(self) -> std::sync::Arc<File> {
        self.0
    }
}

pub struct CoreRuntime {
    catalog: Catalog,
    ownership: RuntimeOwnershipGuard,
    boot_id: String,
    coordinator_reserved: AtomicBool,
}

pub(crate) struct JobOwnershipCapability<'a> {
    catalog_instance_id: uuid::Uuid,
    ownership: &'a RuntimeOwnershipGuard,
}

impl JobOwnershipCapability<'_> {
    pub(crate) fn ensure_for(&self, catalog: &Catalog) -> Result<(), RuntimeOwnershipError> {
        if catalog.instance_id() != self.catalog_instance_id {
            return Err(RuntimeOwnershipError::CatalogMismatch);
        }
        self.ownership.ensure_current()
    }
}

#[derive(Debug)]
pub enum CoreRuntimeError {
    Ownership(RuntimeOwnershipError),
    Catalog(rusqlite::Error),
    Service(ServiceError),
    ModelManager(ManagerError),
    Io(io::Error),
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct FileIdentity {
    device: u64,
    inode: u64,
}

struct ExternalOwnershipAnchor {
    parent_container: File,
    parent_name: CString,
    parent_identity: FileIdentity,
    parent: File,
    lock: File,
    lock_name: CString,
    lock_identity: FileIdentity,
}

impl ExternalOwnershipAnchor {
    fn acquire(data_dir: &Path) -> Result<Self, RuntimeOwnershipError> {
        let parent_path = data_dir.parent().ok_or(RuntimeOwnershipError::UnsafePath)?;
        let canonical_parent = parent_path
            .canonicalize()
            .map_err(RuntimeOwnershipError::Io)?;
        let parent_name = c_string(
            canonical_parent
                .file_name()
                .ok_or(RuntimeOwnershipError::UnsafePath)?,
        )?;
        let parent_container_path = canonical_parent
            .parent()
            .ok_or(RuntimeOwnershipError::UnsafePath)?;
        let parent_container = open_directory_path(parent_container_path)?;
        let parent = open_directory_at(&parent_container, &parent_name)?;
        let parent_identity = file_identity(&parent)?;
        ensure_entry_identity(
            &parent_container,
            &parent_name,
            parent_identity,
            libc::S_IFDIR,
        )?;
        let data_dir_name = data_dir
            .file_name()
            .ok_or(RuntimeOwnershipError::UnsafePath)?;
        // LifeSub has one desktop data root per canonical parent. The parent inode is the only
        // coordination object that cannot be replaced independently from entries inside it.
        match parent.try_lock_exclusive() {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                return Err(RuntimeOwnershipError::AlreadyOwned);
            }
            Err(error) => return Err(RuntimeOwnershipError::Io(error)),
        }
        let target_identity = canonical_parent.join(data_dir_name);
        let digest = hex::encode(Sha256::digest(target_identity.as_os_str().as_bytes()));
        let lock_name = c_string(OsStr::new(&format!(
            "{CORE_LOCK_PREFIX}{digest}{CORE_LOCK_SUFFIX}"
        )))?;
        let lock = open_lock_at(&parent, &lock_name)?;
        let lock_identity = file_identity(&lock)?;
        match lock.try_lock_exclusive() {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                return Err(RuntimeOwnershipError::AlreadyOwned);
            }
            Err(error) => return Err(RuntimeOwnershipError::Io(error)),
        }
        ensure_entry_identity(&parent, &lock_name, lock_identity, libc::S_IFREG)?;
        Ok(Self {
            parent_container,
            parent_name,
            parent_identity,
            parent,
            lock,
            lock_name,
            lock_identity,
        })
    }
}

impl RuntimeOwnershipGuard {
    pub fn acquire(data_dir: impl AsRef<Path>) -> Result<Self, RuntimeOwnershipError> {
        let data_dir = data_dir.as_ref();
        let anchor = ExternalOwnershipAnchor::acquire(data_dir)?;
        Self::acquire_with_anchor_and_hook(data_dir, anchor, || Ok(()))
    }

    fn acquire_with_anchor_and_hook(
        data_dir: &Path,
        anchor: ExternalOwnershipAnchor,
        hook: impl FnOnce() -> io::Result<()>,
    ) -> Result<Self, RuntimeOwnershipError> {
        let data_dir_name = c_string(
            data_dir
                .file_name()
                .ok_or(RuntimeOwnershipError::UnsafePath)?,
        )?;
        let data_dir_file = open_directory_at(&anchor.parent, &data_dir_name)?;
        let data_dir_identity = file_identity(&data_dir_file)?;
        ensure_entry_identity(
            &anchor.parent,
            &data_dir_name,
            data_dir_identity,
            libc::S_IFDIR,
        )?;
        match data_dir_file.try_lock_exclusive() {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                return Err(RuntimeOwnershipError::AlreadyOwned);
            }
            Err(error) => return Err(RuntimeOwnershipError::Io(error)),
        }
        let lock_name = c_string(OsStr::new(RUNTIME_LOCK_FILE))?;
        let lock = open_lock_at(&data_dir_file, &lock_name)?;
        let lock_identity = file_identity(&lock)?;
        hook().map_err(RuntimeOwnershipError::Io)?;
        match lock.try_lock_exclusive() {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                return Err(RuntimeOwnershipError::AlreadyOwned);
            }
            Err(error) => return Err(RuntimeOwnershipError::Io(error)),
        }
        ensure_entry_identity(&data_dir_file, &lock_name, lock_identity, libc::S_IFREG)?;
        ensure_entry_identity(
            &anchor.parent,
            &data_dir_name,
            data_dir_identity,
            libc::S_IFDIR,
        )?;
        ensure_entry_identity(
            &anchor.parent_container,
            &anchor.parent_name,
            anchor.parent_identity,
            libc::S_IFDIR,
        )?;
        Ok(Self {
            parent_container: anchor.parent_container,
            parent_name: anchor.parent_name,
            parent_identity: anchor.parent_identity,
            core_lock: anchor.lock,
            core_lock_name: anchor.lock_name,
            core_lock_identity: anchor.lock_identity,
            lock,
            data_dir: data_dir_file,
            parent: anchor.parent,
            data_dir_name,
            data_dir_identity,
            lock_name,
            lock_identity,
        })
    }

    pub fn ensure_current(&self) -> Result<(), RuntimeOwnershipError> {
        ensure_file_identity(&self.parent, self.parent_identity, libc::S_IFDIR)?;
        ensure_entry_identity(
            &self.parent_container,
            &self.parent_name,
            self.parent_identity,
            libc::S_IFDIR,
        )?;
        ensure_file_identity(&self.data_dir, self.data_dir_identity, libc::S_IFDIR)?;
        ensure_file_identity(&self.core_lock, self.core_lock_identity, libc::S_IFREG)?;
        ensure_entry_identity(
            &self.parent,
            &self.core_lock_name,
            self.core_lock_identity,
            libc::S_IFREG,
        )?;
        ensure_entry_identity(
            &self.parent,
            &self.data_dir_name,
            self.data_dir_identity,
            libc::S_IFDIR,
        )?;
        ensure_file_identity(&self.lock, self.lock_identity, libc::S_IFREG)?;
        ensure_entry_identity(
            &self.data_dir,
            &self.lock_name,
            self.lock_identity,
            libc::S_IFREG,
        )
    }

    pub(crate) fn try_clone_data_directory(
        &self,
    ) -> Result<DataDirectoryCapability, RuntimeOwnershipError> {
        self.ensure_current()?;
        let data_dir = self
            .data_dir
            .try_clone()
            .map_err(RuntimeOwnershipError::Io)?;
        self.ensure_current()?;
        Ok(DataDirectoryCapability(std::sync::Arc::new(data_dir)))
    }

    #[cfg(test)]
    pub fn acquire_with_lock_swap(
        data_dir: &Path,
        swap: impl FnOnce() -> io::Result<()>,
    ) -> Result<Self, RuntimeOwnershipError> {
        let anchor = ExternalOwnershipAnchor::acquire(data_dir)?;
        Self::acquire_with_anchor_and_hook(data_dir, anchor, swap)
    }
}

impl Drop for RuntimeOwnershipGuard {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.lock);
        let _ = FileExt::unlock(&self.data_dir);
        let _ = FileExt::unlock(&self.core_lock);
        let _ = FileExt::unlock(&self.parent);
    }
}

impl CoreRuntime {
    pub fn initialize(data_dir: impl AsRef<Path>) -> Result<Self, CoreRuntimeError> {
        Self::initialize_with_hooks(
            data_dir.as_ref(),
            uuid::Uuid::new_v4().to_string(),
            || Ok(()),
            || Ok(()),
            || Ok(()),
            || Ok(()),
            || Ok(()),
        )
    }

    fn initialize_with_hooks(
        data_dir: &Path,
        boot_id: String,
        parent_hook: impl FnOnce() -> io::Result<()>,
        ownership_hook: impl FnOnce() -> io::Result<()>,
        catalog_open_hook: impl FnOnce() -> io::Result<()>,
        resolved_catalog_open_hook: impl FnOnce() -> io::Result<()>,
        post_catalog_open_hook: impl FnOnce() -> io::Result<()>,
    ) -> Result<Self, CoreRuntimeError> {
        let anchor =
            ExternalOwnershipAnchor::acquire(data_dir).map_err(CoreRuntimeError::Ownership)?;
        parent_hook().map_err(CoreRuntimeError::Io)?;
        ensure_entry_identity(
            &anchor.parent_container,
            &anchor.parent_name,
            anchor.parent_identity,
            libc::S_IFDIR,
        )
        .map_err(CoreRuntimeError::Ownership)?;
        let data_dir_name = c_string(data_dir.file_name().ok_or(CoreRuntimeError::Ownership(
            RuntimeOwnershipError::UnsafePath,
        ))?)
        .map_err(CoreRuntimeError::Ownership)?;
        create_directory_at(&anchor.parent, &data_dir_name).map_err(CoreRuntimeError::Ownership)?;
        let ownership =
            RuntimeOwnershipGuard::acquire_with_anchor_and_hook(data_dir, anchor, || Ok(()))
                .map_err(CoreRuntimeError::Ownership)?;
        ownership
            .ensure_current()
            .map_err(CoreRuntimeError::Ownership)?;
        ownership_hook().map_err(CoreRuntimeError::Io)?;
        ownership
            .ensure_current()
            .map_err(CoreRuntimeError::Ownership)?;
        catalog_open_hook().map_err(CoreRuntimeError::Io)?;
        ownership
            .ensure_current()
            .map_err(CoreRuntimeError::Ownership)?;
        let catalog_open = Catalog::prepare_anchored(&ownership.data_dir, CATALOG_FILE)
            .map_err(CoreRuntimeError::Catalog)?;
        resolved_catalog_open_hook().map_err(CoreRuntimeError::Io)?;
        ownership
            .ensure_current()
            .map_err(CoreRuntimeError::Ownership)?;
        let catalog = catalog_open.open().map_err(CoreRuntimeError::Catalog)?;
        post_catalog_open_hook().map_err(CoreRuntimeError::Io)?;
        ownership
            .ensure_current()
            .map_err(CoreRuntimeError::Ownership)?;
        let data_dir_capability = ownership
            .try_clone_data_directory()
            .map_err(CoreRuntimeError::Ownership)?;
        let catalog = EvidenceService::initialize_anchored(catalog, data_dir_capability)
            .map_err(CoreRuntimeError::Service)?
            .into_catalog();
        ownership
            .ensure_current()
            .map_err(CoreRuntimeError::Ownership)?;
        let model_root = ownership
            .data_dir
            .try_clone()
            .map_err(CoreRuntimeError::Io)?;
        let manager = ModelManager::new_anchored(
            data_dir,
            model_root,
            ReqwestTransport::new().map_err(CoreRuntimeError::ModelManager)?,
            catalog,
        );
        #[cfg(feature = "asr-runtime")]
        let manager = {
            let identity = crate::asr::verify_runtime_identity().map_err(|error| {
                CoreRuntimeError::ModelManager(ManagerError::catalog(format!("{error:?}")))
            })?;
            manager.with_sherpa_runtime_identity(FullSherpaRuntimeIdentity {
                version: identity.version,
                git_commit: identity.pinned_git_sha1.to_owned(),
                native_archive_sha256: identity.native_archive_sha256.to_owned(),
                build_id: identity.build_id.to_owned(),
            })
        };
        #[cfg(not(feature = "asr-runtime"))]
        let manager = manager;
        manager
            .reconcile_all_anchored()
            .map_err(CoreRuntimeError::ModelManager)?;
        ownership
            .ensure_current()
            .map_err(CoreRuntimeError::Ownership)?;
        let catalog = manager.into_catalog();
        Ok(Self {
            catalog,
            ownership,
            boot_id,
            coordinator_reserved: AtomicBool::new(false),
        })
    }

    #[cfg(test)]
    pub(crate) fn initialize_with_boot_id(
        data_dir: &Path,
        boot_id: impl Into<String>,
    ) -> Result<Self, CoreRuntimeError> {
        Self::initialize_with_hooks(
            data_dir,
            boot_id.into(),
            || Ok(()),
            || Ok(()),
            || Ok(()),
            || Ok(()),
            || Ok(()),
        )
    }

    #[cfg(test)]
    pub fn initialize_with_data_dir_swap(
        data_dir: &Path,
        swap: impl FnOnce() -> io::Result<()>,
    ) -> Result<Self, CoreRuntimeError> {
        Self::initialize_with_hooks(
            data_dir,
            uuid::Uuid::new_v4().to_string(),
            || Ok(()),
            swap,
            || Ok(()),
            || Ok(()),
            || Ok(()),
        )
    }

    #[cfg(test)]
    pub fn initialize_with_catalog_open_swap(
        data_dir: &Path,
        swap: impl FnOnce() -> io::Result<()>,
    ) -> Result<Self, CoreRuntimeError> {
        Self::initialize_with_hooks(
            data_dir,
            uuid::Uuid::new_v4().to_string(),
            || Ok(()),
            || Ok(()),
            swap,
            || Ok(()),
            || Ok(()),
        )
    }

    #[cfg(test)]
    pub fn initialize_with_resolved_catalog_open_swap(
        data_dir: &Path,
        swap: impl FnOnce() -> io::Result<()>,
    ) -> Result<Self, CoreRuntimeError> {
        Self::initialize_with_hooks(
            data_dir,
            uuid::Uuid::new_v4().to_string(),
            || Ok(()),
            || Ok(()),
            || Ok(()),
            swap,
            || Ok(()),
        )
    }

    #[cfg(test)]
    pub fn initialize_with_post_catalog_open_swap(
        data_dir: &Path,
        swap: impl FnOnce() -> io::Result<()>,
    ) -> Result<Self, CoreRuntimeError> {
        Self::initialize_with_hooks(
            data_dir,
            uuid::Uuid::new_v4().to_string(),
            || Ok(()),
            || Ok(()),
            || Ok(()),
            || Ok(()),
            swap,
        )
    }

    #[cfg(test)]
    pub fn initialize_with_parent_swap(
        data_dir: &Path,
        swap: impl FnOnce() -> io::Result<()>,
    ) -> Result<Self, CoreRuntimeError> {
        Self::initialize_with_hooks(
            data_dir,
            uuid::Uuid::new_v4().to_string(),
            swap,
            || Ok(()),
            || Ok(()),
            || Ok(()),
            || Ok(()),
        )
    }

    #[cfg(test)]
    pub fn into_parts(self) -> (Catalog, RuntimeOwnershipGuard) {
        (self.catalog, self.ownership)
    }

    #[cfg(feature = "desktop")]
    pub(crate) fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    #[cfg(feature = "desktop")]
    pub(crate) fn ensure_current(&self) -> Result<(), RuntimeOwnershipError> {
        self.ownership.ensure_current()
    }

    #[cfg(feature = "desktop")]
    pub(crate) fn try_clone_data_directory(
        &self,
    ) -> Result<DataDirectoryCapability, RuntimeOwnershipError> {
        self.ownership.try_clone_data_directory()
    }

    fn job_repository(&self) -> JobRepository<'_> {
        self.build_job_repository(&self.catalog, SystemClock)
    }

    pub fn job_coordinator(
        &self,
        worker_id: impl Into<String>,
    ) -> Result<JobCoordinator<'_>, JobError> {
        let reservation = self.reserve_job_coordinator()?;
        Ok(JobCoordinator::from_core(
            self.job_repository(),
            worker_id,
            reservation,
        ))
    }

    pub fn job_control(&self) -> JobControl<'_> {
        JobControl::from_core(self.job_repository())
    }

    #[cfg(test)]
    pub(crate) fn job_repository_with_clock<C: Clock>(&self, clock: C) -> JobRepository<'_, C> {
        self.build_job_repository(&self.catalog, clock)
    }

    #[cfg(test)]
    pub(crate) fn job_control_with_clock<C: Clock>(&self, clock: C) -> JobControl<'_, C> {
        JobControl::from_core(self.job_repository_with_clock(clock))
    }

    #[cfg(test)]
    pub(crate) fn job_coordinator_with_clock<C: Clock>(
        &self,
        worker_id: impl Into<String>,
        clock: C,
    ) -> Result<JobCoordinator<'_, C>, JobError> {
        let reservation = self.reserve_job_coordinator()?;
        Ok(JobCoordinator::from_core(
            self.job_repository_with_clock(clock),
            worker_id,
            reservation,
        ))
    }

    #[cfg(test)]
    pub(crate) fn job_repository_for_foreign_core_with_clock<'a, C: Clock>(
        &'a self,
        foreign: &'a CoreRuntime,
        clock: C,
    ) -> JobRepository<'a, C> {
        self.build_job_repository(&foreign.catalog, clock)
    }

    fn build_job_repository<'a, C: Clock>(
        &'a self,
        catalog: &'a Catalog,
        clock: C,
    ) -> JobRepository<'a, C> {
        JobRepository::from_core(
            catalog,
            JobOwnershipCapability {
                catalog_instance_id: self.catalog.instance_id(),
                ownership: &self.ownership,
            },
            self.boot_id.clone(),
            clock,
        )
    }

    fn reserve_job_coordinator(&self) -> Result<JobCoordinatorReservation<'_>, JobError> {
        JobCoordinatorReservation::try_reserve(&self.coordinator_reserved)
    }
}

fn open_directory_path(path: &Path) -> Result<File, RuntimeOwnershipError> {
    let path = c_string(path.as_os_str())?;
    let descriptor = unsafe {
        // SAFETY: path is NUL terminated and flags request a no-follow directory handle.
        libc::open(
            path.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    };
    file_from_descriptor(descriptor)
}

fn open_directory_at(parent: &File, name: &CString) -> Result<File, RuntimeOwnershipError> {
    let descriptor = unsafe {
        // SAFETY: parent is an open directory and name is a valid relative C string.
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    };
    file_from_descriptor(descriptor)
}

fn create_directory_at(parent: &File, name: &CString) -> Result<(), RuntimeOwnershipError> {
    let result = unsafe {
        // SAFETY: parent is an open directory and name is a valid relative C string.
        libc::mkdirat(parent.as_raw_fd(), name.as_ptr(), 0o700)
    };
    if result == -1 {
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::AlreadyExists {
            return Err(RuntimeOwnershipError::Io(error));
        }
    } else {
        parent.sync_all().map_err(RuntimeOwnershipError::Io)?;
    }
    Ok(())
}

fn open_lock_at(data_dir: &File, name: &CString) -> Result<File, RuntimeOwnershipError> {
    let descriptor = unsafe {
        // SAFETY: data_dir is open and creation is relative with no symlink following.
        libc::openat(
            data_dir.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDWR | libc::O_CREAT | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            0o600,
        )
    };
    file_from_descriptor(descriptor)
}

fn file_from_descriptor(descriptor: libc::c_int) -> Result<File, RuntimeOwnershipError> {
    if descriptor == -1 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ELOOP) {
            Err(RuntimeOwnershipError::UnsafePath)
        } else {
            Err(RuntimeOwnershipError::Io(error))
        }
    } else {
        Ok(unsafe {
            // SAFETY: successful open returned unique ownership of this descriptor.
            File::from_raw_fd(descriptor)
        })
    }
}

fn ensure_entry_identity(
    directory: &File,
    name: &CString,
    expected: FileIdentity,
    expected_type: libc::mode_t,
) -> Result<(), RuntimeOwnershipError> {
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    let result = unsafe {
        // SAFETY: name is NUL terminated and stat points to writable storage.
        libc::fstatat(
            directory.as_raw_fd(),
            name.as_ptr(),
            stat.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if result == -1 {
        return Err(RuntimeOwnershipError::Io(io::Error::last_os_error()));
    }
    let stat = unsafe {
        // SAFETY: successful fstatat initialized stat.
        stat.assume_init()
    };
    let actual = FileIdentity {
        device: stat.st_dev as u64,
        inode: stat.st_ino,
    };
    if stat.st_mode & libc::S_IFMT != expected_type || actual != expected {
        Err(RuntimeOwnershipError::UnsafePath)
    } else {
        Ok(())
    }
}

fn file_identity(file: &File) -> Result<FileIdentity, RuntimeOwnershipError> {
    let metadata = file.metadata().map_err(RuntimeOwnershipError::Io)?;
    Ok(FileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

fn ensure_file_identity(
    file: &File,
    expected: FileIdentity,
    expected_type: libc::mode_t,
) -> Result<(), RuntimeOwnershipError> {
    let metadata = file.metadata().map_err(RuntimeOwnershipError::Io)?;
    let actual = FileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    };
    let actual_type = metadata.mode() as libc::mode_t & libc::S_IFMT;
    if actual_type != expected_type || actual != expected {
        Err(RuntimeOwnershipError::UnsafePath)
    } else {
        Ok(())
    }
}

fn c_string(value: &OsStr) -> Result<CString, RuntimeOwnershipError> {
    CString::new(value.as_bytes()).map_err(|_| RuntimeOwnershipError::UnsafePath)
}
