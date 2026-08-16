use std::collections::HashSet;
use std::ffi::{CStr, CString, OsStr, OsString};
#[cfg(test)]
use std::fs;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Component, Path, PathBuf};
#[cfg(test)]
use std::sync::{Arc, Barrier};
use std::time::{Duration, SystemTime};

use sha2::{Digest, Sha256};

use crate::domain::AudioChunk;

use super::ImportFault;
use super::runtime_lock::DataDirectoryCapability;

const AUDIO_DIRECTORY: &str = "audio";
const IMPORT_TEMP_PREFIX: &str = ".lifesub-import-";
const IMPORT_TEMP_SUFFIX: &str = ".tmp";
const DEFAULT_AUDIO_EXTENSION: &str = "audio";
const MAX_AUDIO_EXTENSION_BYTES: usize = 16;
const COPY_BUFFER_BYTES: usize = 64 * 1024;

#[cfg(test)]
thread_local! {
    static AFTER_CLEANUP_IDENTITY: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
        std::cell::RefCell::new(None);
}

#[cfg(test)]
pub(super) fn set_cleanup_identity_hook(hook: impl FnOnce() + 'static) {
    AFTER_CLEANUP_IDENTITY.with(|slot| *slot.borrow_mut() = Some(Box::new(hook)));
}

pub(super) struct AudioStore {
    data_dir: DataDirectory,
    #[cfg(test)]
    import_fault: Option<ImportFault>,
    #[cfg(test)]
    audio_directory_swap_target: Option<PathBuf>,
    #[cfg(test)]
    first_audio_create_barrier: Option<Arc<Barrier>>,
}

enum DataDirectory {
    Path(PathBuf),
    Anchored(DirectoryHandle),
}

pub(super) struct PendingAudio {
    pub(super) digest: String,
    pub(super) byte_length: u64,
    directory: AnchoredDirectory,
    temp_name: OsString,
    identity: FileIdentity,
}

pub(super) struct StoredAudio {
    pub(super) relative_path: PathBuf,
    directory: AnchoredDirectory,
    final_name: OsString,
    identity: FileIdentity,
}

struct DirectoryHandle(std::sync::Arc<File>);

struct AnchoredDirectory {
    parent: DirectoryHandle,
    directory: DirectoryHandle,
    identity: FileIdentity,
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct FileIdentity {
    device: u64,
    inode: u64,
}

#[derive(Clone, Copy)]
pub(super) enum ValidationError {
    Missing,
    Corrupted,
}

impl AudioStore {
    #[cfg_attr(test, allow(dead_code))]
    pub(super) fn new(data_dir: &Path) -> Self {
        Self {
            data_dir: DataDirectory::Path(data_dir.to_path_buf()),
            #[cfg(test)]
            import_fault: None,
            #[cfg(test)]
            audio_directory_swap_target: None,
            #[cfg(test)]
            first_audio_create_barrier: None,
        }
    }

    pub(super) fn anchored(data_dir: DataDirectoryCapability) -> Self {
        Self {
            data_dir: DataDirectory::Anchored(DirectoryHandle(data_dir.into_file())),
            #[cfg(test)]
            import_fault: None,
            #[cfg(test)]
            audio_directory_swap_target: None,
            #[cfg(test)]
            first_audio_create_barrier: None,
        }
    }

    #[cfg(test)]
    pub(super) fn with_fault(
        data_dir: &Path,
        import_fault: Option<ImportFault>,
        audio_directory_swap_target: Option<&Path>,
        first_audio_create_barrier: Option<Arc<Barrier>>,
    ) -> Self {
        Self {
            data_dir: DataDirectory::Path(data_dir.to_path_buf()),
            import_fault,
            audio_directory_swap_target: audio_directory_swap_target.map(Path::to_path_buf),
            first_audio_create_barrier,
        }
    }

    pub(super) fn write_temp(&self, source_path: &Path, id: &str) -> io::Result<PendingAudio> {
        let directory = self.prepare_directory()?;
        let temp_name = OsString::from(format!("{IMPORT_TEMP_PREFIX}{id}{IMPORT_TEMP_SUFFIX}"));
        let (digest, byte_length, identity) =
            write_hashed_temp(source_path, &directory.directory, &temp_name)?;
        Ok(PendingAudio {
            digest,
            byte_length,
            directory,
            temp_name,
            identity,
        })
    }

    pub(super) fn rename_to_final(
        &self,
        pending: &PendingAudio,
        id: &str,
        extension: &str,
    ) -> io::Result<StoredAudio> {
        let final_name = OsString::from(format!("{}-{id}.{extension}", pending.digest));
        let relative_path = PathBuf::from(AUDIO_DIRECTORY).join(&final_name);
        if let Err(error) = pending.directory.ensure_current() {
            pending
                .directory
                .directory
                .cleanup_regular_file(&pending.temp_name, pending.identity)?;
            return Err(error);
        }
        let stored_directory = match pending.directory.try_clone() {
            Ok(directory) => directory,
            Err(error) => {
                pending
                    .directory
                    .directory
                    .cleanup_regular_file(&pending.temp_name, pending.identity)?;
                return Err(error);
            }
        };
        if let Err(error) = self.rename(
            &pending.directory.directory,
            &pending.temp_name,
            &final_name,
        ) {
            pending
                .directory
                .directory
                .cleanup_regular_file(&pending.temp_name, pending.identity)?;
            return Err(error);
        }
        let stored = StoredAudio {
            relative_path,
            directory: stored_directory,
            final_name,
            identity: pending.identity,
        };
        if let Err(error) = pending.directory.ensure_current() {
            stored
                .directory
                .directory
                .cleanup_regular_file(&stored.final_name, stored.identity)?;
            return Err(error);
        }
        Ok(stored)
    }

    pub(super) fn sync_final(&self, stored: &StoredAudio) -> io::Result<()> {
        if let Err(error) = stored.directory.ensure_current() {
            stored
                .directory
                .directory
                .cleanup_regular_file(&stored.final_name, stored.identity)?;
            return Err(error);
        }
        if let Err(error) =
            self.sync_directory(&stored.directory.directory, ImportFault::ParentSyncIo)
        {
            stored
                .directory
                .directory
                .cleanup_regular_file(&stored.final_name, stored.identity)?;
            return Err(error);
        }
        if let Err(error) = stored.directory.ensure_current() {
            stored
                .directory
                .directory
                .cleanup_regular_file(&stored.final_name, stored.identity)?;
            return Err(error);
        }
        Ok(())
    }

    pub(super) fn ensure_stored_current(&self, stored: &StoredAudio) -> io::Result<()> {
        stored.directory.ensure_current()?;
        if stored
            .directory
            .directory
            .regular_file_identity(&stored.final_name)?
            == Some(stored.identity)
        {
            Ok(())
        } else {
            Err(unsafe_storage_error())
        }
    }

    pub(super) fn discard_stored(&self, stored: &StoredAudio) -> io::Result<()> {
        stored
            .directory
            .directory
            .cleanup_regular_file(&stored.final_name, stored.identity)
    }

    pub(super) fn reconcile_orphans(
        &self,
        referenced_paths: &HashSet<&str>,
        stale_before: SystemTime,
    ) -> io::Result<()> {
        let Some(audio_dir) = self.existing_audio_dir()? else {
            return Ok(());
        };
        for name in audio_dir.entries()? {
            audio_dir.ensure_current()?;
            let Some(name_str) = name.to_str() else {
                return Err(unsafe_storage_error());
            };
            let relative = format!("{AUDIO_DIRECTORY}/{name_str}");
            let is_referenced = referenced_paths.contains(relative.as_str());
            if is_referenced {
                continue;
            }
            let metadata = audio_dir.directory.metadata(&name)?;
            if !metadata.is_regular || !is_recognized_importer_name(name_str) {
                return Err(unsafe_storage_error());
            }
            if metadata.modified <= stale_before {
                audio_dir.ensure_current()?;
                audio_dir
                    .directory
                    .cleanup_regular_file(&name, metadata.identity)?;
            }
        }
        audio_dir.ensure_current()?;
        self.sync_directory(&audio_dir.directory, ImportFault::ParentSyncIo)
    }

    pub(super) fn validate(&self, chunk: &AudioChunk) -> Result<(), ValidationError> {
        let directory = match self.existing_audio_dir() {
            Ok(Some(directory)) => directory,
            Ok(None) => return Err(ValidationError::Missing),
            Err(_) => return Err(ValidationError::Corrupted),
        };
        let name = self.safe_chunk_name(&chunk.path)?;
        let result = directory.directory.open_file(&name).and_then(hash_file);
        if directory.ensure_current().is_err() {
            return Err(ValidationError::Corrupted);
        }
        match result {
            Ok((digest, byte_length))
                if digest == chunk.sha256 && byte_length == chunk.byte_length =>
            {
                Ok(())
            }
            Ok(_) => Err(ValidationError::Corrupted),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Err(ValidationError::Missing),
            Err(_) => Err(ValidationError::Corrupted),
        }
    }

    #[cfg(test)]
    fn audio_dir(&self) -> PathBuf {
        match &self.data_dir {
            DataDirectory::Path(path) => path.join(AUDIO_DIRECTORY),
            DataDirectory::Anchored(_) => unreachable!("test swap requires a path-backed store"),
        }
    }

    fn prepare_directory(&self) -> io::Result<AnchoredDirectory> {
        let data_dir = self.open_data_dir()?;
        let audio_dir = match data_dir.open_directory(OsStr::new(AUDIO_DIRECTORY)) {
            Ok(directory) => directory,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                self.wait_before_first_audio_create();
                match data_dir.create_directory(OsStr::new(AUDIO_DIRECTORY)) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                    Err(error) => return Err(error),
                }
                if let Err(error) = self.sync_directory(&data_dir, ImportFault::DirectorySyncIo) {
                    data_dir.remove_directory_if_present(OsStr::new(AUDIO_DIRECTORY));
                    return Err(error);
                }
                data_dir.open_directory(OsStr::new(AUDIO_DIRECTORY))?
            }
            Err(error) => return Err(error),
        };
        self.swap_audio_directory_for_test()?;
        AnchoredDirectory::new(data_dir, audio_dir)
    }

    fn existing_audio_dir(&self) -> io::Result<Option<AnchoredDirectory>> {
        let data_dir = self.open_data_dir()?;
        match data_dir.open_directory(OsStr::new(AUDIO_DIRECTORY)) {
            Ok(audio_dir) => {
                self.swap_audio_directory_for_test()?;
                Ok(Some(AnchoredDirectory::new(data_dir, audio_dir)?))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn open_data_dir(&self) -> io::Result<DirectoryHandle> {
        match &self.data_dir {
            DataDirectory::Path(path) => DirectoryHandle::open_path(path),
            DataDirectory::Anchored(directory) => directory.try_clone(),
        }
    }

    fn rename(
        &self,
        directory: &DirectoryHandle,
        source: &OsStr,
        destination: &OsStr,
    ) -> io::Result<()> {
        self.inject_io_failure(ImportFault::RenameIo, "rename")?;
        directory.rename(source, destination)
    }

    fn sync_directory(&self, directory: &DirectoryHandle, fault: ImportFault) -> io::Result<()> {
        self.inject_io_failure(fault, "directory sync")?;
        directory.0.sync_all()
    }

    #[cfg(test)]
    fn inject_io_failure(&self, fault: ImportFault, operation: &str) -> io::Result<()> {
        if self.import_fault == Some(fault) {
            Err(io::Error::other(format!("injected {operation} failure")))
        } else {
            Ok(())
        }
    }

    #[cfg(not(test))]
    fn inject_io_failure(&self, _fault: ImportFault, _operation: &str) -> io::Result<()> {
        Ok(())
    }

    fn safe_chunk_name(&self, relative_path: &str) -> Result<OsString, ValidationError> {
        let relative_path = Path::new(relative_path);
        let mut components = relative_path.components();
        if components.next() != Some(Component::Normal(OsStr::new(AUDIO_DIRECTORY))) {
            return Err(ValidationError::Corrupted);
        }
        let Some(Component::Normal(name)) = components.next() else {
            return Err(ValidationError::Corrupted);
        };
        if components.next().is_some() {
            return Err(ValidationError::Corrupted);
        }
        Ok(name.to_os_string())
    }

    #[cfg(all(test, unix))]
    fn swap_audio_directory_for_test(&self) -> io::Result<()> {
        use std::os::unix::fs::symlink;

        let Some(target) = &self.audio_directory_swap_target else {
            return Ok(());
        };
        let audio_dir = self.audio_dir();
        let data_dir = match &self.data_dir {
            DataDirectory::Path(data_dir) => data_dir,
            DataDirectory::Anchored(_) => return Err(unsafe_storage_error()),
        };
        fs::rename(&audio_dir, data_dir.join("audio-held"))?;
        symlink(target, audio_dir)
    }

    #[cfg(test)]
    fn wait_before_first_audio_create(&self) {
        if let Some(barrier) = &self.first_audio_create_barrier {
            barrier.wait();
        }
    }

    #[cfg(not(test))]
    fn wait_before_first_audio_create(&self) {}

    #[cfg(not(all(test, unix)))]
    fn swap_audio_directory_for_test(&self) -> io::Result<()> {
        Ok(())
    }
}

impl DirectoryHandle {
    fn open_path(path: &Path) -> io::Result<Self> {
        let path = c_string(path.as_os_str())?;
        let descriptor = unsafe {
            libc::open(
                path.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            )
        };
        descriptor_result(descriptor)
            .map(|descriptor| unsafe { Self(std::sync::Arc::new(File::from_raw_fd(descriptor))) })
    }

    fn open_directory(&self, name: &OsStr) -> io::Result<Self> {
        let name = c_string(name)?;
        let descriptor = unsafe {
            libc::openat(
                self.0.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            )
        };
        descriptor_result(descriptor)
            .map(|descriptor| unsafe { Self(std::sync::Arc::new(File::from_raw_fd(descriptor))) })
    }

    fn create_directory(&self, name: &OsStr) -> io::Result<()> {
        let name = c_string(name)?;
        syscall_result(unsafe { libc::mkdirat(self.0.as_raw_fd(), name.as_ptr(), 0o700) })
    }

    fn remove_directory_if_present(&self, name: &OsStr) {
        let Ok(name) = c_string(name) else {
            return;
        };
        unsafe {
            libc::unlinkat(self.0.as_raw_fd(), name.as_ptr(), libc::AT_REMOVEDIR);
        }
    }

    fn create_file(&self, name: &OsStr) -> io::Result<File> {
        let name = c_string(name)?;
        let descriptor = unsafe {
            libc::openat(
                self.0.as_raw_fd(),
                name.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_CLOEXEC | libc::O_NOFOLLOW,
                0o600,
            )
        };
        descriptor_result(descriptor).map(|descriptor| unsafe { File::from_raw_fd(descriptor) })
    }

    fn open_file(&self, name: &OsStr) -> io::Result<File> {
        let name = c_string(name)?;
        let descriptor = unsafe {
            libc::openat(
                self.0.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            )
        };
        descriptor_result(descriptor).and_then(|descriptor| {
            let file = unsafe { File::from_raw_fd(descriptor) };
            if file.metadata()?.is_file() {
                Ok(file)
            } else {
                Err(unsafe_storage_error())
            }
        })
    }

    fn rename(&self, source: &OsStr, destination: &OsStr) -> io::Result<()> {
        let source = c_string(source)?;
        let destination = c_string(destination)?;
        syscall_result(unsafe {
            libc::renameat(
                self.0.as_raw_fd(),
                source.as_ptr(),
                self.0.as_raw_fd(),
                destination.as_ptr(),
            )
        })
    }

    fn rename_noreplace(&self, source: &OsStr, destination: &OsStr) -> io::Result<()> {
        let source = c_string(source)?;
        let destination = c_string(destination)?;
        #[cfg(target_os = "macos")]
        let result = unsafe {
            libc::renameatx_np(
                self.0.as_raw_fd(),
                source.as_ptr(),
                self.0.as_raw_fd(),
                destination.as_ptr(),
                libc::RENAME_EXCL,
            )
        };
        #[cfg(target_os = "linux")]
        let result = unsafe {
            libc::syscall(
                libc::SYS_renameat2,
                self.0.as_raw_fd(),
                source.as_ptr(),
                self.0.as_raw_fd(),
                destination.as_ptr(),
                libc::RENAME_NOREPLACE,
            ) as libc::c_int
        };
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "atomic no-replace rename is unavailable",
        ));
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        syscall_result(result)
    }

    fn unlink(&self, name: &OsStr) -> io::Result<()> {
        let name = c_string(name)?;
        syscall_result(unsafe { libc::unlinkat(self.0.as_raw_fd(), name.as_ptr(), 0) })
    }

    fn metadata(&self, name: &OsStr) -> io::Result<EntryMetadata> {
        let name = c_string(name)?;
        let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
        syscall_result(unsafe {
            libc::fstatat(
                self.0.as_raw_fd(),
                name.as_ptr(),
                stat.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        })?;
        let stat = unsafe { stat.assume_init() };
        let modified = stat_modified_time(&stat);
        Ok(EntryMetadata {
            is_regular: stat.st_mode & libc::S_IFMT == libc::S_IFREG,
            modified,
            identity: FileIdentity {
                device: stat.st_dev as u64,
                inode: stat.st_ino,
            },
        })
    }

    fn regular_file_identity(&self, name: &OsStr) -> io::Result<Option<FileIdentity>> {
        let name = c_string(name)?;
        let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
        let result = unsafe {
            libc::fstatat(
                self.0.as_raw_fd(),
                name.as_ptr(),
                stat.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        };
        if result == -1 {
            let error = io::Error::last_os_error();
            return if error.kind() == io::ErrorKind::NotFound {
                Ok(None)
            } else {
                Err(error)
            };
        }
        let stat = unsafe { stat.assume_init() };
        if stat.st_mode & libc::S_IFMT != libc::S_IFREG {
            return Ok(None);
        }
        Ok(Some(FileIdentity {
            device: stat.st_dev as u64,
            inode: stat.st_ino,
        }))
    }

    fn cleanup_regular_file(&self, name: &OsStr, identity: FileIdentity) -> io::Result<()> {
        if self.regular_file_identity(name)? != Some(identity) {
            return Ok(());
        }
        #[cfg(test)]
        AFTER_CLEANUP_IDENTITY.with(|slot| {
            if let Some(hook) = slot.borrow_mut().take() {
                hook();
            }
        });
        let tombstone = OsString::from(format!(
            "{IMPORT_TEMP_PREFIX}chk_{}{IMPORT_TEMP_SUFFIX}",
            uuid::Uuid::new_v4().simple()
        ));
        self.rename_noreplace(name, &tombstone)?;
        let tombstone_identity = self.regular_file_identity(&tombstone);
        match tombstone_identity {
            Ok(Some(current)) if current == identity => {
                self.unlink(&tombstone)?;
                self.0.sync_all()
            }
            Ok(_) => {
                self.restore_cleanup_tombstone(&tombstone, name)?;
                Err(unsafe_storage_error())
            }
            Err(error) => {
                self.restore_cleanup_tombstone(&tombstone, name)?;
                Err(error)
            }
        }
    }

    fn restore_cleanup_tombstone(&self, tombstone: &OsStr, name: &OsStr) -> io::Result<()> {
        self.rename_noreplace(tombstone, name)
            .map_err(|_| unsafe_storage_error())?;
        self.0.sync_all()
    }

    fn identity(&self) -> io::Result<FileIdentity> {
        let metadata = self.0.metadata()?;
        use std::os::unix::fs::MetadataExt;
        Ok(FileIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }

    fn entry_identity(&self, name: &OsStr) -> io::Result<FileIdentity> {
        let name = c_string(name)?;
        let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
        syscall_result(unsafe {
            // SAFETY: `name` is a valid NUL-terminated string and `stat` points to writable storage.
            libc::fstatat(
                self.0.as_raw_fd(),
                name.as_ptr(),
                stat.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        })?;
        let stat = unsafe {
            // SAFETY: successful `fstatat` initialized the entire `stat` value.
            stat.assume_init()
        };
        if stat.st_mode & libc::S_IFMT != libc::S_IFDIR {
            return Err(unsafe_storage_error());
        }
        Ok(FileIdentity {
            device: stat.st_dev as u64,
            inode: stat.st_ino,
        })
    }

    fn entries(&self) -> io::Result<Vec<OsString>> {
        let descriptor = unsafe { libc::dup(self.0.as_raw_fd()) };
        let descriptor = descriptor_result(descriptor)?;
        let directory = unsafe { libc::fdopendir(descriptor) };
        if directory.is_null() {
            unsafe {
                libc::close(descriptor);
            }
            return Err(io::Error::last_os_error());
        }
        let mut entries = Vec::new();
        loop {
            unsafe {
                *libc::__error() = 0;
            }
            let entry = unsafe { libc::readdir(directory) };
            if entry.is_null() {
                let error = io::Error::last_os_error();
                unsafe {
                    libc::closedir(directory);
                }
                return if error.raw_os_error() == Some(0) {
                    Ok(entries)
                } else {
                    Err(error)
                };
            }
            let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) };
            if name.to_bytes() != b"." && name.to_bytes() != b".." {
                entries.push(OsString::from_vec(name.to_bytes().to_vec()));
            }
        }
    }

    fn try_clone(&self) -> io::Result<Self> {
        Ok(Self(std::sync::Arc::clone(&self.0)))
    }
}

impl AnchoredDirectory {
    fn new(parent: DirectoryHandle, directory: DirectoryHandle) -> io::Result<Self> {
        let identity = directory.identity()?;
        let anchored = Self {
            parent,
            directory,
            identity,
        };
        anchored.ensure_current()?;
        Ok(anchored)
    }

    fn ensure_current(&self) -> io::Result<()> {
        if self.parent.entry_identity(OsStr::new(AUDIO_DIRECTORY))? == self.identity {
            Ok(())
        } else {
            Err(unsafe_storage_error())
        }
    }

    fn entries(&self) -> io::Result<Vec<OsString>> {
        self.ensure_current()?;
        self.directory.entries()
    }

    fn try_clone(&self) -> io::Result<Self> {
        Self::new(self.parent.try_clone()?, self.directory.try_clone()?)
    }
}

struct EntryMetadata {
    is_regular: bool,
    modified: SystemTime,
    identity: FileIdentity,
}

fn stat_modified_time(stat: &libc::stat) -> SystemTime {
    let seconds = u64::try_from(stat.st_mtime).unwrap_or(0);
    let nanoseconds = u32::try_from(stat.st_mtime_nsec).unwrap_or(0);
    SystemTime::UNIX_EPOCH + Duration::new(seconds, nanoseconds)
}

fn c_string(value: &OsStr) -> io::Result<CString> {
    CString::new(value.as_bytes()).map_err(|_| unsafe_storage_error())
}

fn descriptor_result(descriptor: RawFd) -> io::Result<RawFd> {
    if descriptor == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(descriptor)
    }
}

fn syscall_result(result: libc::c_int) -> io::Result<()> {
    if result == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn is_recognized_importer_name(name: &str) -> bool {
    is_import_temp(name) || is_import_final(name)
}

fn is_import_temp(name: &str) -> bool {
    let Some(id) = name
        .strip_prefix(IMPORT_TEMP_PREFIX)
        .and_then(|value| value.strip_suffix(IMPORT_TEMP_SUFFIX))
    else {
        return false;
    };
    is_chunk_id(id)
}

fn is_import_final(name: &str) -> bool {
    let Some((digest, remainder)) = name.split_once('-') else {
        return false;
    };
    let Some((id, extension)) = remainder.rsplit_once('.') else {
        return false;
    };
    digest.len() == 64
        && digest.bytes().all(is_lowercase_hex)
        && is_chunk_id(id)
        && is_canonical_extension(extension)
}

fn is_chunk_id(value: &str) -> bool {
    let Some(uuid) = value.strip_prefix("chk_") else {
        return false;
    };
    uuid.len() == 32 && uuid.bytes().all(is_lowercase_hex)
}

fn is_lowercase_hex(byte: u8) -> bool {
    byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
}

fn is_canonical_extension(extension: &str) -> bool {
    !extension.is_empty()
        && extension.len() <= MAX_AUDIO_EXTENSION_BYTES
        && extension
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
}

pub(super) fn canonical_extension(path: &Path) -> String {
    path.extension()
        .and_then(OsStr::to_str)
        .map(str::to_ascii_lowercase)
        .filter(|extension| is_canonical_extension(extension))
        .unwrap_or_else(|| DEFAULT_AUDIO_EXTENSION.to_owned())
}

fn unsafe_storage_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "unsafe audio storage layout",
    )
}

fn write_hashed_temp(
    source_path: &Path,
    directory: &DirectoryHandle,
    temp_name: &OsStr,
) -> io::Result<(String, u64, FileIdentity)> {
    let source = File::open(source_path)?;
    let temp = directory.create_file(temp_name)?;
    let metadata = temp.metadata()?;
    use std::os::unix::fs::MetadataExt;
    let identity = FileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    };
    let mut reader = BufReader::new(source);
    let mut writer = BufWriter::new(temp);
    let write_result = (|| {
        let mut hasher = Sha256::new();
        let mut byte_length = 0_u64;
        let mut buffer = [0_u8; COPY_BUFFER_BYTES];
        loop {
            let count = reader.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            writer.write_all(&buffer[..count])?;
            hasher.update(&buffer[..count]);
            byte_length += count as u64;
        }
        writer.flush()?;
        writer.get_ref().sync_all()?;
        Ok((hex::encode(hasher.finalize()), byte_length, identity))
    })();
    if write_result.is_err() {
        directory.cleanup_regular_file(temp_name, identity)?;
    }
    write_result
}

fn hash_file(file: File) -> io::Result<(String, u64)> {
    let mut file = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut byte_length = 0_u64;
    let mut buffer = [0_u8; COPY_BUFFER_BYTES];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
        byte_length += count as u64;
    }
    Ok((hex::encode(hasher.finalize()), byte_length))
}
