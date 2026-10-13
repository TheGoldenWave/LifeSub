use std::ffi::{CString, OsStr, OsString};
use std::fs::File;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::catalog::Catalog;
use crate::catalog::capture::{NativeCaptureInsertError, NativeCaptureInsertOutcome};
use crate::domain::{AudioChunk, AudioSource, CaptureChunkTiming};

const AUDIO_DIRECTORY: &str = "audio";
const PARTIAL_SUFFIX: &str = ".partial";
const INTENT_DIRECTORY: &str = ".lifesub-native-intents";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NativeChunkSealRequest {
    pub relative_staging_path: PathBuf,
    pub chunk_id: String,
    pub session_id: String,
    pub source: AudioSource,
    pub expected_sha256: String,
    pub expected_byte_length: u64,
    pub timing: CaptureChunkTiming,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct NativeSealIntent {
    chunk_id: String,
    final_path: String,
    expected_sha256: String,
    expected_byte_length: u64,
    session_id: String,
    source: AudioSource,
    timing: CaptureChunkTiming,
}

#[derive(Debug)]
pub(crate) enum NativeChunkSealError {
    InvalidPath,
    IntegrityMismatch,
    Conflict,
    Io(io::Error),
    Catalog(rusqlite::Error),
    RecoveryRequired(io::Error),
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SealFault {
    Rename,
    DirectorySync,
    Restore,
    IntentWrite,
    IntentSync,
    IntentFileSync,
    IntentRename,
}

pub(crate) struct NativeChunkSealer<'a> {
    catalog: &'a Catalog,
    staging: Directory,
    final_root: Directory,
    #[cfg(test)]
    fault: Option<SealFault>,
}

impl<'a> NativeChunkSealer<'a> {
    pub(crate) fn new(
        catalog: &'a Catalog,
        staging_root: &Path,
        final_root: &Path,
    ) -> io::Result<Self> {
        Ok(Self {
            catalog,
            staging: Directory::open(staging_root)?,
            final_root: Directory::open(final_root)?,
            #[cfg(test)]
            fault: None,
        })
    }

    #[cfg(test)]
    pub(crate) fn with_fault_for_test(mut self, fault: SealFault) -> Self {
        self.fault = Some(fault);
        self
    }

    pub(crate) fn seal(
        &self,
        request: &NativeChunkSealRequest,
    ) -> Result<AudioChunk, NativeChunkSealError> {
        let (staging_name, final_name) =
            validated_names(&request.relative_staging_path, &request.chunk_id)?;
        if request.source == AudioSource::Imported || request.timing.chunk_id != request.chunk_id {
            return Err(NativeChunkSealError::Conflict);
        }
        let relative_final_path = PathBuf::from(AUDIO_DIRECTORY).join(&final_name);
        let relative_final_path = relative_final_path
            .to_str()
            .ok_or(NativeChunkSealError::InvalidPath)?
            .to_owned();
        let chunk = AudioChunk {
            id: request.chunk_id.clone(),
            session_id: request.session_id.clone(),
            source: request.source,
            path: relative_final_path,
            sha256: request.expected_sha256.clone(),
            byte_length: request.expected_byte_length,
        };

        let intent = NativeSealIntent {
            chunk_id: chunk.id.clone(),
            final_path: chunk.path.clone(),
            expected_sha256: chunk.sha256.clone(),
            expected_byte_length: chunk.byte_length,
            session_id: chunk.session_id.clone(),
            source: chunk.source,
            timing: request.timing.clone(),
        };
        let intents = self
            .final_root
            .open_or_create_directory(OsStr::new(INTENT_DIRECTORY))?;
        reconcile_intent_directory(&intents)?;
        let intent_name = intent_name(&chunk.id);
        let existing_intent = read_intent(&intents, &intent_name)?;

        if let Some(existing) = self
            .catalog
            .chunk(&chunk.id)
            .map_err(NativeChunkSealError::Catalog)?
        {
            let timing = self
                .catalog
                .capture_chunk_timing(&chunk.id)
                .map_err(NativeChunkSealError::Catalog)?;
            if existing != chunk || timing.as_ref() != Some(&request.timing) {
                return Err(NativeChunkSealError::Conflict);
            }
            let audio = self
                .final_root
                .open_or_create_directory(OsStr::new(AUDIO_DIRECTORY))?;
            let final_file = audio.open_regular(&final_name)?;
            verify_file(final_file, &chunk)?;
            if existing_intent
                .as_ref()
                .is_some_and(|value| value == &intent)
            {
                remove_intent(&intents, &intent_name)?;
                intents.sync()?;
            } else if existing_intent.is_some() {
                return Err(NativeChunkSealError::Conflict);
            }
            return Ok(chunk);
        } else if self
            .catalog
            .capture_chunk_timing(&chunk.id)
            .map_err(NativeChunkSealError::Catalog)?
            .is_some()
        {
            return Err(NativeChunkSealError::Conflict);
        }

        let audio = self
            .final_root
            .open_or_create_directory(OsStr::new(AUDIO_DIRECTORY))?;
        match audio.open_regular(&final_name) {
            Ok(final_file) => {
                if existing_intent.as_ref() != Some(&intent) {
                    return Err(NativeChunkSealError::Conflict);
                }
                final_file.sync_all()?;
                match verify_file(final_file, &chunk) {
                    Ok(()) => {}
                    Err(NativeChunkSealError::IntegrityMismatch) => {
                        return Err(NativeChunkSealError::Conflict);
                    }
                    Err(error) => return Err(error),
                }
                audio.sync()?;
                return match self
                    .catalog
                    .insert_native_capture_chunk(&chunk, &request.timing)
                {
                    Ok(
                        NativeCaptureInsertOutcome::Inserted | NativeCaptureInsertOutcome::Existing,
                    ) => {
                        remove_intent(&intents, &intent_name)?;
                        intents.sync()?;
                        Ok(chunk)
                    }
                    Err(NativeCaptureInsertError::Conflict) => Err(NativeChunkSealError::Conflict),
                    Err(NativeCaptureInsertError::Catalog(error)) => {
                        Err(NativeChunkSealError::Catalog(error))
                    }
                };
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }

        let staging_file = self.staging.open_regular(staging_name)?;
        let staging_identity = FileIdentity::of(&staging_file)?;
        verify_file(staging_file.try_clone()?, &chunk)?;
        if let Some(existing_intent) = existing_intent {
            if existing_intent != intent {
                return Err(NativeChunkSealError::Conflict);
            }
        } else {
            self.write_intent(&intents, &intent_name, &intent)?;
        }
        staging_file.sync_all()?;
        if self.staging.entry_identity(staging_name)? != Some(staging_identity) {
            return Err(NativeChunkSealError::Conflict);
        }

        match self.rename_to_final(&audio, staging_name, &final_name) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                remove_intent(&intents, &intent_name)?;
                intents.sync()?;
                return Err(NativeChunkSealError::Conflict);
            }
            Err(error) => {
                remove_intent(&intents, &intent_name)?;
                intents.sync()?;
                return Err(error.into());
            }
        }

        let validation = (|| {
            let published = audio.open_regular(&final_name)?;
            if FileIdentity::of(&published)? != staging_identity {
                return Err(NativeChunkSealError::Conflict);
            }
            verify_file(published, &chunk)
        })();
        if let Err(error) = validation {
            return Err(self.restore_or_recovery(
                &audio,
                &final_name,
                staging_name,
                &intents,
                &intent_name,
                error,
            ));
        }

        if let Err(error) = self.sync_final_directory(&audio) {
            return Err(self.restore_or_recovery(
                &audio,
                &final_name,
                staging_name,
                &intents,
                &intent_name,
                error.into(),
            ));
        }

        match self
            .catalog
            .insert_native_capture_chunk(&chunk, &request.timing)
        {
            Ok(NativeCaptureInsertOutcome::Inserted | NativeCaptureInsertOutcome::Existing) => {
                remove_intent(&intents, &intent_name)?;
                intents.sync()?;
                Ok(chunk)
            }
            Err(NativeCaptureInsertError::Conflict) => Err(self.restore_or_recovery(
                &audio,
                &final_name,
                staging_name,
                &intents,
                &intent_name,
                NativeChunkSealError::Conflict,
            )),
            Err(NativeCaptureInsertError::Catalog(error)) => Err(self.restore_or_recovery(
                &audio,
                &final_name,
                staging_name,
                &intents,
                &intent_name,
                NativeChunkSealError::Catalog(error),
            )),
        }
    }

    fn write_intent(
        &self,
        intents: &Directory,
        name: &OsStr,
        intent: &NativeSealIntent,
    ) -> Result<(), NativeChunkSealError> {
        let bytes = serde_json_canonicalizer::to_vec(intent)
            .map_err(|error| NativeChunkSealError::Io(io::Error::other(error.to_string())))?;
        let temp_name = OsString::from(format!(
            ".lifesub-native-intent-{}.tmp",
            uuid::Uuid::new_v4().simple()
        ));
        let file = intents.create_file(&temp_name)?;
        let temp_identity = FileIdentity::of(&file)?;
        #[cfg(test)]
        if matches!(self.fault, Some(SealFault::IntentWrite)) {
            return Err(cleanup_after_intent_failure(
                intents,
                &temp_name,
                temp_identity,
                NativeChunkSealError::Io(io::Error::other("injected native intent write failure")),
            ));
        }
        use std::io::Write;
        if let Err(error) = (&file).write_all(&bytes) {
            return Err(cleanup_after_intent_failure(
                intents,
                &temp_name,
                temp_identity,
                error.into(),
            ));
        }
        #[cfg(test)]
        if matches!(self.fault, Some(SealFault::IntentFileSync)) {
            return Err(cleanup_after_intent_failure(
                intents,
                &temp_name,
                temp_identity,
                NativeChunkSealError::Io(io::Error::other(
                    "injected native intent file sync failure",
                )),
            ));
        }
        if let Err(error) = file.sync_all() {
            return Err(cleanup_after_intent_failure(
                intents,
                &temp_name,
                temp_identity,
                error.into(),
            ));
        }
        #[cfg(test)]
        if matches!(self.fault, Some(SealFault::IntentRename)) {
            return Err(cleanup_after_intent_failure(
                intents,
                &temp_name,
                temp_identity,
                NativeChunkSealError::Io(io::Error::other("injected native intent rename failure")),
            ));
        }
        if let Err(error) = rename_noreplace(intents, &temp_name, intents, name) {
            return Err(cleanup_after_intent_failure(
                intents,
                &temp_name,
                temp_identity,
                error.into(),
            ));
        }
        #[cfg(test)]
        if matches!(self.fault, Some(SealFault::IntentSync)) {
            let error = io::Error::other("injected native intent directory sync failure");
            return Err(cleanup_after_intent_failure(
                intents,
                name,
                temp_identity,
                NativeChunkSealError::Io(error),
            ));
        }
        if let Err(error) = intents.sync() {
            return Err(cleanup_after_intent_failure(
                intents,
                name,
                temp_identity,
                error.into(),
            ));
        }
        Ok(())
    }

    fn rename_to_final(
        &self,
        audio: &Directory,
        staging_name: &OsStr,
        final_name: &OsStr,
    ) -> io::Result<()> {
        #[cfg(test)]
        if self.fault == Some(SealFault::Rename) {
            return Err(io::Error::other("injected native chunk rename failure"));
        }
        rename_noreplace(&self.staging, staging_name, audio, final_name)
    }

    fn sync_final_directory(&self, audio: &Directory) -> io::Result<()> {
        #[cfg(test)]
        if self.fault == Some(SealFault::DirectorySync) {
            return Err(io::Error::other(
                "injected native chunk directory sync failure",
            ));
        }
        audio.sync()
    }

    fn restore_staging(
        &self,
        audio: &Directory,
        final_name: &OsStr,
        staging_name: &OsStr,
    ) -> io::Result<()> {
        #[cfg(test)]
        if self.fault == Some(SealFault::Restore) {
            return Err(io::Error::other("injected native chunk restore failure"));
        }
        rename_noreplace(audio, final_name, &self.staging, staging_name)?;
        audio.sync()?;
        self.staging.sync()
    }

    fn restore_or_recovery(
        &self,
        audio: &Directory,
        final_name: &OsStr,
        staging_name: &OsStr,
        intents: &Directory,
        intent_name: &OsStr,
        original: NativeChunkSealError,
    ) -> NativeChunkSealError {
        match self.restore_staging(audio, final_name, staging_name) {
            Ok(()) => match remove_intent(intents, intent_name).and_then(|()| intents.sync()) {
                Ok(()) => original,
                Err(error) => NativeChunkSealError::RecoveryRequired(error),
            },
            Err(error) => NativeChunkSealError::RecoveryRequired(error),
        }
    }
}

impl From<io::Error> for NativeChunkSealError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

fn validated_names<'a>(
    path: &'a Path,
    chunk_id: &str,
) -> Result<(&'a OsStr, std::ffi::OsString), NativeChunkSealError> {
    if !is_native_chunk_id(chunk_id) {
        return Err(NativeChunkSealError::InvalidPath);
    }
    let mut components = path.components();
    let Some(Component::Normal(name)) = components.next() else {
        return Err(NativeChunkSealError::InvalidPath);
    };
    if components.next().is_some() {
        return Err(NativeChunkSealError::InvalidPath);
    }
    let name_bytes = name.as_bytes();
    let Some(final_bytes) = name_bytes.strip_suffix(PARTIAL_SUFFIX.as_bytes()) else {
        return Err(NativeChunkSealError::InvalidPath);
    };
    if final_bytes.is_empty() {
        return Err(NativeChunkSealError::InvalidPath);
    }
    let Some(final_name) = std::str::from_utf8(final_bytes).ok() else {
        return Err(NativeChunkSealError::InvalidPath);
    };
    if !is_native_final_name(final_name)
        || final_name
            .strip_prefix(chunk_id)
            .and_then(|suffix| suffix.strip_prefix('.'))
            .is_none()
    {
        return Err(NativeChunkSealError::InvalidPath);
    }
    Ok((name, OsStr::from_bytes(final_bytes).to_owned()))
}

pub(crate) fn is_native_final_name(name: &str) -> bool {
    let Some((chunk_id, extension)) = name.rsplit_once('.') else {
        return false;
    };
    is_native_chunk_id(chunk_id)
        && !extension.is_empty()
        && extension.len() <= 16
        && extension
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
}

fn is_native_chunk_id(value: &str) -> bool {
    let Some(identity) = value.strip_prefix("chk_") else {
        return false;
    };
    !identity.is_empty()
        && identity.len() <= 64
        && identity
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn verify_file(mut file: File, chunk: &AudioChunk) -> Result<(), NativeChunkSealError> {
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() != chunk.byte_length {
        return Err(NativeChunkSealError::IntegrityMismatch);
    }
    let mut digest = Sha256::new();
    let copied = io::copy(&mut file, &mut digest)?;
    if copied != chunk.byte_length || hex::encode(digest.finalize()) != chunk.sha256 {
        return Err(NativeChunkSealError::IntegrityMismatch);
    }
    Ok(())
}

struct Directory(File);

impl Directory {
    fn open(path: &Path) -> io::Result<Self> {
        let path = CString::new(path.as_os_str().as_bytes())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "directory contains NUL"))?;
        let fd = unsafe {
            libc::open(
                path.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(unsafe { File::from_raw_fd(fd) }))
    }

    fn open_or_create_directory(&self, name: &OsStr) -> io::Result<Self> {
        let name = c_name(name)?;
        let result = unsafe { libc::mkdirat(self.0.as_raw_fd(), name.as_ptr(), 0o700) };
        let created = result == 0;
        if !created {
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::AlreadyExists {
                return Err(error);
            }
        }
        let fd = unsafe {
            libc::openat(
                self.0.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let directory = Self(unsafe { File::from_raw_fd(fd) });
        if created {
            self.sync()?;
        }
        Ok(directory)
    }

    fn open_regular(&self, name: &OsStr) -> io::Result<File> {
        let name = c_name(name)?;
        let fd = unsafe {
            libc::openat(
                self.0.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let file = unsafe { File::from_raw_fd(fd) };
        if !file.metadata()?.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "native chunk staging entry is not a regular file",
            ));
        }
        Ok(file)
    }

    fn create_file(&self, name: &OsStr) -> io::Result<File> {
        let name = c_name(name)?;
        let fd = unsafe {
            libc::openat(
                self.0.as_raw_fd(),
                name.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0o600,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(unsafe { File::from_raw_fd(fd) })
    }

    fn entry_identity(&self, name: &OsStr) -> io::Result<Option<FileIdentity>> {
        match self.open_regular(name) {
            Ok(file) => FileIdentity::of(&file).map(Some),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn unlink(&self, name: &OsStr) -> io::Result<()> {
        let name = c_name(name)?;
        let result = unsafe { libc::unlinkat(self.0.as_raw_fd(), name.as_ptr(), 0) };
        if result == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    fn open_directory(&self, name: &OsStr) -> io::Result<Self> {
        let name = c_name(name)?;
        let fd = unsafe {
            libc::openat(
                self.0.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(unsafe { File::from_raw_fd(fd) }))
    }

    fn entries(&self) -> io::Result<Vec<OsString>> {
        let descriptor = unsafe { libc::dup(self.0.as_raw_fd()) };
        if descriptor < 0 {
            return Err(io::Error::last_os_error());
        }
        let directory = unsafe { libc::fdopendir(descriptor) };
        if directory.is_null() {
            unsafe { libc::close(descriptor) };
            return Err(io::Error::last_os_error());
        }
        let mut entries = Vec::new();
        loop {
            #[cfg(target_os = "macos")]
            unsafe {
                *libc::__error() = 0;
            }
            let entry = unsafe { libc::readdir(directory) };
            if entry.is_null() {
                let error = io::Error::last_os_error();
                unsafe { libc::closedir(directory) };
                return if error.raw_os_error().is_none_or(|code| code == 0) {
                    Ok(entries)
                } else {
                    Err(error)
                };
            }
            let name = unsafe { std::ffi::CStr::from_ptr((*entry).d_name.as_ptr()) };
            if name.to_bytes() != b"." && name.to_bytes() != b".." {
                entries.push(OsString::from_vec(name.to_bytes().to_vec()));
            }
        }
    }

    fn sync(&self) -> io::Result<()> {
        self.0.sync_all()
    }
}

fn intent_name(chunk_id: &str) -> std::ffi::OsString {
    std::ffi::OsString::from(format!("{chunk_id}.json"))
}

fn read_intent(
    intents: &Directory,
    name: &OsStr,
) -> Result<Option<NativeSealIntent>, NativeChunkSealError> {
    let mut file = match intents.open_regular(name) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let mut bytes = Vec::new();
    use std::io::Read;
    file.read_to_end(&mut bytes)?;
    serde_json::from_slice(&bytes).map(Some).map_err(|error| {
        NativeChunkSealError::RecoveryRequired(io::Error::other(error.to_string()))
    })
}

fn remove_intent(intents: &Directory, name: &OsStr) -> io::Result<()> {
    intents.unlink(name)
}

fn cleanup_after_intent_failure(
    intents: &Directory,
    name: &OsStr,
    identity: FileIdentity,
    original: NativeChunkSealError,
) -> NativeChunkSealError {
    match cleanup_intent_file(intents, name, identity) {
        Ok(()) => original,
        Err(error) => NativeChunkSealError::RecoveryRequired(error),
    }
}

fn cleanup_intent_file(
    intents: &Directory,
    name: &OsStr,
    identity: FileIdentity,
) -> io::Result<()> {
    let tombstone = OsString::from(format!(
        ".lifesub-native-intent-{}.tmp",
        uuid::Uuid::new_v4().simple()
    ));
    rename_noreplace(intents, name, intents, &tombstone)?;
    let current = intents.open_regular(&tombstone)?;
    if FileIdentity::of(&current)? != identity {
        return Err(io::Error::other("native intent temp identity changed"));
    }
    intents.unlink(&tombstone)?;
    intents.sync()
}

fn reconcile_intent_directory(intents: &Directory) -> io::Result<()> {
    for name in intents.entries()? {
        let Some(name_str) = name.to_str() else {
            return Err(io::Error::other("invalid native intent entry"));
        };
        let file = intents.open_regular(&name)?;
        validate_intent_file(&file)?;
        if is_intent_temp_name(name_str) {
            let identity = FileIdentity::of(&file)?;
            cleanup_intent_file(intents, &name, identity)?;
            continue;
        }
        if !is_intent_final_name(name_str) {
            return Err(io::Error::other("unknown native intent entry"));
        }
    }
    intents.sync()
}

fn validate_intent_file(file: &File) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt;
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o777 != 0o600
    {
        return Err(io::Error::other("unsafe native intent file"));
    }
    Ok(())
}

pub(crate) fn is_intent_temp_name(name: &str) -> bool {
    let Some(uuid) = name
        .strip_prefix(".lifesub-native-intent-")
        .and_then(|value| value.strip_suffix(".tmp"))
    else {
        return false;
    };
    uuid.len() == 32 && uuid.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(crate) fn is_intent_final_name(name: &str) -> bool {
    name.strip_suffix(".json").is_some_and(is_native_chunk_id)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileIdentity {
    device: libc::dev_t,
    inode: libc::ino_t,
}

impl FileIdentity {
    fn of(file: &File) -> io::Result<Self> {
        let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
        if unsafe { libc::fstat(file.as_raw_fd(), stat.as_mut_ptr()) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let stat = unsafe { stat.assume_init() };
        Ok(Self {
            device: stat.st_dev,
            inode: stat.st_ino,
        })
    }
}

fn c_name(name: &OsStr) -> io::Result<CString> {
    CString::new(name.as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "entry name contains NUL"))
}

#[cfg(target_os = "macos")]
fn rename_noreplace(
    source_dir: &Directory,
    source_name: &OsStr,
    destination_dir: &Directory,
    destination_name: &OsStr,
) -> io::Result<()> {
    let source_name = c_name(source_name)?;
    let destination_name = c_name(destination_name)?;
    let result = unsafe {
        libc::renameatx_np(
            source_dir.0.as_raw_fd(),
            source_name.as_ptr(),
            destination_dir.0.as_raw_fd(),
            destination_name.as_ptr(),
            libc::RENAME_EXCL,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(target_os = "linux")]
fn rename_noreplace(
    source_dir: &Directory,
    source_name: &OsStr,
    destination_dir: &Directory,
    destination_name: &OsStr,
) -> io::Result<()> {
    let source_name = c_name(source_name)?;
    let destination_name = c_name(destination_name)?;
    let result = unsafe {
        libc::renameat2(
            source_dir.0.as_raw_fd(),
            source_name.as_ptr(),
            destination_dir.0.as_raw_fd(),
            destination_name.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
