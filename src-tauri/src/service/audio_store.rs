use std::collections::HashSet;
use std::ffi::{CStr, CString, OsStr, OsString};
use std::fs::File;
#[cfg(test)]
use std::fs;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime};
#[cfg(test)]
use std::sync::{Arc, Barrier};

use sha2::{Digest, Sha256};

use crate::domain::AudioChunk;

use super::ImportFault;

const AUDIO_DIRECTORY: &str = "audio";
const IMPORT_TEMP_PREFIX: &str = ".lifesub-import-";
const IMPORT_TEMP_SUFFIX: &str = ".tmp";
const DEFAULT_AUDIO_EXTENSION: &str = "audio";
const MAX_AUDIO_EXTENSION_BYTES: usize = 16;
const COPY_BUFFER_BYTES: usize = 64 * 1024;

pub(super) struct AudioStore {
    data_dir: PathBuf,
    #[cfg(test)]
    import_fault: Option<ImportFault>,
    #[cfg(test)]
    audio_directory_swap_target: Option<PathBuf>,
    #[cfg(test)]
    first_audio_create_barrier: Option<Arc<Barrier>>,
}

pub(super) struct PendingAudio {
    pub(super) digest: String,
    pub(super) byte_length: u64,
    directory: AnchoredDirectory,
    temp_name: OsString,
}

pub(super) struct StoredAudio {
    pub(super) relative_path: PathBuf,
    directory: AnchoredDirectory,
    final_name: OsString,
}

struct DirectoryHandle(File);

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
    pub(super) fn new(data_dir: &Path) -> Self {
        Self {
            data_dir: data_dir.to_path_buf(),
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
            data_dir: data_dir.to_path_buf(),
            import_fault,
            audio_directory_swap_target: audio_directory_swap_target.map(Path::to_path_buf),
            first_audio_create_barrier,
        }
    }

    pub(super) fn write_temp(&self, source_path: &Path, id: &str) -> io::Result<PendingAudio> {
        let directory = self.prepare_directory()?;
        let temp_name = OsString::from(format!("{IMPORT_TEMP_PREFIX}{id}{IMPORT_TEMP_SUFFIX}"));
        match write_hashed_temp(source_path, &directory.directory, &temp_name) {
            Ok((digest, byte_length)) => Ok(PendingAudio {
                digest,
                byte_length,
                directory,
                temp_name,
            }),
            Err(error) => {
                if directory.ensure_current().is_ok() {
                    directory.directory.unlink_if_present(&temp_name);
                }
                Err(error)
            }
        }
    }

    pub(super) fn rename_to_final(
        &self,
        pending: &PendingAudio,
        id: &str,
        extension: &str,
    ) -> io::Result<StoredAudio> {
        let final_name = OsString::from(format!("{}-{id}.{extension}", pending.digest));
        let relative_path = PathBuf::from(AUDIO_DIRECTORY).join(&final_name);
        pending.directory.ensure_current()?;
        let stored_directory = pending.directory.try_clone()?;
        if let Err(error) = self.rename(&pending.directory.directory, &pending.temp_name, &final_name) {
            if pending.directory.ensure_current().is_ok() {
                pending.directory.directory.unlink_if_present(&pending.temp_name);
            }
            return Err(error);
        }
        pending.directory.ensure_current()?;
        Ok(StoredAudio {
            relative_path,
            directory: stored_directory,
            final_name,
        })
    }

    pub(super) fn sync_final(&self, stored: &StoredAudio) -> io::Result<()> {
        stored.directory.ensure_current()?;
        if let Err(error) = self.sync_directory(&stored.directory.directory, ImportFault::ParentSyncIo) {
            if stored.directory.ensure_current().is_ok() {
                stored.directory.directory.unlink_if_present(&stored.final_name);
            }
            return Err(error);
        }
        stored.directory.ensure_current()?;
        Ok(())
    }

    pub(super) fn ensure_stored_current(&self, stored: &StoredAudio) -> io::Result<()> {
        stored.directory.ensure_current()
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
                audio_dir.directory.unlink(&name)?;
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
        self.data_dir.join(AUDIO_DIRECTORY)
    }

    fn prepare_directory(&self) -> io::Result<AnchoredDirectory> {
        let data_dir = DirectoryHandle::open_path(&self.data_dir)?;
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
        let data_dir = DirectoryHandle::open_path(&self.data_dir)?;
        match data_dir.open_directory(OsStr::new(AUDIO_DIRECTORY)) {
            Ok(audio_dir) => {
                self.swap_audio_directory_for_test()?;
                Ok(Some(AnchoredDirectory::new(data_dir, audio_dir)?))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
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
        fs::rename(&audio_dir, self.data_dir.join("audio-held"))?;
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
        descriptor_result(descriptor).map(|descriptor| unsafe {
            Self(File::from_raw_fd(descriptor))
        })
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
        descriptor_result(descriptor).map(|descriptor| unsafe {
            Self(File::from_raw_fd(descriptor))
        })
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
                libc::O_WRONLY
                    | libc::O_CREAT
                    | libc::O_EXCL
                    | libc::O_CLOEXEC
                    | libc::O_NOFOLLOW,
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

    fn unlink(&self, name: &OsStr) -> io::Result<()> {
        let name = c_string(name)?;
        syscall_result(unsafe { libc::unlinkat(self.0.as_raw_fd(), name.as_ptr(), 0) })
    }

    fn unlink_if_present(&self, name: &OsStr) {
        let _ = self.unlink(name);
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
        })
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
        self.0.try_clone().map(Self)
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
        && extension.bytes().all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
}

pub(super) fn canonical_extension(path: &Path) -> String {
    path.extension()
        .and_then(OsStr::to_str)
        .map(str::to_ascii_lowercase)
        .filter(|extension| is_canonical_extension(extension))
        .unwrap_or_else(|| DEFAULT_AUDIO_EXTENSION.to_owned())
}

fn unsafe_storage_error() -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, "unsafe audio storage layout")
}

fn write_hashed_temp(
    source_path: &Path,
    directory: &DirectoryHandle,
    temp_name: &OsStr,
) -> io::Result<(String, u64)> {
    let source = File::open(source_path)?;
    let temp = directory.create_file(temp_name)?;
    let mut reader = BufReader::new(source);
    let mut writer = BufWriter::new(temp);
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
    Ok((hex::encode(hasher.finalize()), byte_length))
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
