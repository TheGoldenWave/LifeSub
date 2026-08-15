use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

use sha2::{Digest, Sha256};

use crate::domain::AudioChunk;

use super::ImportFault;

const AUDIO_DIRECTORY: &str = "audio";
const IMPORT_TEMP_PREFIX: &str = ".lifesub-import-";
const IMPORT_TEMP_SUFFIX: &str = ".tmp";
const COPY_BUFFER_BYTES: usize = 64 * 1024;

pub(super) struct AudioStore {
    data_dir: PathBuf,
    #[cfg(test)]
    import_fault: Option<ImportFault>,
}

pub(super) struct PendingAudio {
    pub(super) digest: String,
    pub(super) byte_length: u64,
    pub(super) temp_path: PathBuf,
}

pub(super) struct StoredAudio {
    pub(super) relative_path: PathBuf,
}

#[derive(Clone, Copy)]
pub(super) enum ValidationError {
    Missing,
    Corrupted,
}

impl AudioStore {
    #[cfg(not(test))]
    pub(super) fn new(data_dir: &Path) -> Self {
        Self {
            data_dir: data_dir.to_path_buf(),
            #[cfg(test)]
            import_fault: None,
        }
    }

    #[cfg(test)]
    pub(super) fn with_fault(data_dir: &Path, import_fault: Option<ImportFault>) -> Self {
        Self {
            data_dir: data_dir.to_path_buf(),
            import_fault,
        }
    }

    pub(super) fn write_temp(&self, source_path: &Path, id: &str) -> io::Result<PendingAudio> {
        let audio_dir = self.prepare_directory()?;
        let temp_path = audio_dir.join(format!("{IMPORT_TEMP_PREFIX}{id}{IMPORT_TEMP_SUFFIX}"));
        match write_hashed_temp(source_path, &temp_path) {
            Ok((digest, byte_length)) => Ok(PendingAudio {
                digest,
                byte_length,
                temp_path,
            }),
            Err(error) => {
                remove_file_if_present(&temp_path);
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
        let relative_path =
            PathBuf::from(AUDIO_DIRECTORY).join(format!("{}-{id}.{extension}", pending.digest));
        let final_path = self.data_dir.join(&relative_path);
        if let Err(error) = self.rename(&pending.temp_path, &final_path) {
            remove_file_if_present(&pending.temp_path);
            return Err(error);
        }
        Ok(StoredAudio { relative_path })
    }

    pub(super) fn sync_final(&self, stored: &StoredAudio) -> io::Result<()> {
        if let Err(error) = self.sync_directory(&self.audio_dir(), ImportFault::ParentSyncIo) {
            remove_file_if_present(&self.data_dir.join(&stored.relative_path));
            return Err(error);
        }
        Ok(())
    }

    pub(super) fn reconcile_orphans(
        &self,
        referenced_paths: &HashSet<&str>,
        stale_before: SystemTime,
    ) -> io::Result<()> {
        let audio_dir = self.audio_dir();
        if !audio_dir.exists() {
            return Ok(());
        }
        for entry in fs::read_dir(&audio_dir)? {
            let entry = entry?;
            let path = entry.path();
            let relative = path
                .strip_prefix(&self.data_dir)
                .ok()
                .and_then(Path::to_str);
            let is_referenced = relative.is_some_and(|value| referenced_paths.contains(value));
            if !is_referenced && entry.metadata()?.modified()? <= stale_before {
                remove_entry(&path)?;
            }
        }
        self.sync_directory(&audio_dir, ImportFault::ParentSyncIo)
    }

    pub(super) fn validate(&self, chunk: &AudioChunk) -> Result<(), ValidationError> {
        let path = self.safe_chunk_path(&chunk.path)?;
        match hash_file(&path) {
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

    fn audio_dir(&self) -> PathBuf {
        self.data_dir.join(AUDIO_DIRECTORY)
    }

    fn prepare_directory(&self) -> io::Result<PathBuf> {
        let audio_dir = self.audio_dir();
        if !audio_dir.exists() {
            fs::create_dir(&audio_dir)?;
            if let Err(error) = self.sync_directory(&self.data_dir, ImportFault::DirectorySyncIo) {
                let _ = fs::remove_dir(&audio_dir);
                return Err(error);
            }
        }
        Ok(audio_dir)
    }

    fn rename(&self, source: &Path, destination: &Path) -> io::Result<()> {
        self.inject_io_failure(ImportFault::RenameIo, "rename")?;
        fs::rename(source, destination)
    }

    fn sync_directory(&self, path: &Path, fault: ImportFault) -> io::Result<()> {
        self.inject_io_failure(fault, "directory sync")?;
        File::open(path)?.sync_all()
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

    fn safe_chunk_path(&self, relative_path: &str) -> Result<PathBuf, ValidationError> {
        let relative_path = Path::new(relative_path);
        let mut components = relative_path.components();
        if components.next() != Some(Component::Normal(OsStr::new(AUDIO_DIRECTORY)))
            || components.any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(ValidationError::Corrupted);
        }
        Ok(self.data_dir.join(relative_path))
    }
}

fn write_hashed_temp(source_path: &Path, temp_path: &Path) -> io::Result<(String, u64)> {
    let source = File::open(source_path)?;
    let temp = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temp_path)?;
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

fn hash_file(path: &Path) -> io::Result<(String, u64)> {
    let mut file = BufReader::new(File::open(path)?);
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

fn remove_entry(path: &Path) -> io::Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

fn remove_file_if_present(path: &Path) {
    let _ = fs::remove_file(path);
}
