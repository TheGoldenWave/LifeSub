use std::fs::File;
#[cfg(test)]
use std::io::Read;
use std::io::{Seek, SeekFrom};
use std::os::fd::AsRawFd;
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use sha2::{Digest, Sha256};

use super::ManagerError;
use super::anchored_reconcile::{AnchoredFs, EntryStat};

#[derive(Clone)]
pub(super) enum ModelStorage {
    #[cfg(test)]
    TestPath(PathBuf),
    Anchored(Arc<AnchoredFs>),
}

impl ModelStorage {
    #[cfg(test)]
    pub(super) fn test_path(path: PathBuf) -> Self {
        Self::TestPath(path)
    }

    pub(super) fn anchored(nominal_root: PathBuf, root: File) -> Self {
        Self::Anchored(Arc::new(AnchoredFs::new(nominal_root, root)))
    }

    pub(super) fn nominal_root(&self) -> &Path {
        match self {
            #[cfg(test)]
            Self::TestPath(path) => path,
            Self::Anchored(storage) => storage.nominal_root(),
        }
    }

    pub(super) fn anchored_fs(&self) -> Option<Arc<AnchoredFs>> {
        match self {
            #[cfg(test)]
            Self::TestPath(_) => None,
            Self::Anchored(storage) => Some(storage.clone()),
        }
    }

    pub(super) fn relative_install_path(&self, nominal: &Path) -> Result<PathBuf, ManagerError> {
        nominal
            .strip_prefix(self.nominal_root())
            .map(Path::to_path_buf)
            .map_err(|_| ManagerError::integrity("installation path mismatch"))
    }

    pub(super) fn open_download_part(
        &self,
        relative: PathBuf,
    ) -> Result<DownloadPart, ManagerError> {
        let nominal_path = self.nominal_root().join(&relative);
        match self {
            #[cfg(test)]
            Self::TestPath(_) => {
                let file = std::fs::OpenOptions::new()
                    .create(true)
                    .read(true)
                    .write(true)
                    .truncate(false)
                    .open(&nominal_path)?;
                Ok(DownloadPart {
                    nominal_path,
                    file,
                    anchored: None,
                })
            }
            Self::Anchored(root) => {
                root.ensure_dir(relative.parent().unwrap_or_else(|| Path::new("")))?;
                let file = root.open_regular_create(&relative)?;
                let identity = EntryStat::from_fd(file.as_raw_fd())?;
                Ok(DownloadPart {
                    nominal_path,
                    file,
                    anchored: Some(AnchoredPart {
                        root: root.clone(),
                        relative,
                        identity,
                    }),
                })
            }
        }
    }

    pub(super) fn checkpoint_bytes(&self, nominal: &Path) -> Result<u64, ManagerError> {
        match self {
            #[cfg(test)]
            Self::TestPath(_) => super::fs_support::checkpoint_bytes(nominal),
            Self::Anchored(root) => {
                let relative = nominal
                    .strip_prefix(root.nominal_root())
                    .map_err(|_| ManagerError::integrity("checkpoint path mismatch"))?;
                root.file_len(relative).map(|length| length.unwrap_or(0))
            }
        }
    }

    pub(super) fn available_space(&self) -> Result<u64, ManagerError> {
        match self {
            #[cfg(test)]
            Self::TestPath(path) => Ok(fs2::available_space(path)?),
            Self::Anchored(root) => root.available_space(),
        }
    }

    pub(super) fn ensure_assembly_roots(&self) -> Result<(), ManagerError> {
        match self {
            #[cfg(test)]
            Self::TestPath(root) => {
                let staging = root.join("staging");
                let final_root = root.join("models/asr");
                std::fs::create_dir_all(&staging)?;
                std::fs::create_dir_all(&final_root)?;
                super::fs_support::same_volume(&staging, &final_root)
            }
            Self::Anchored(root) => {
                root.ensure_dir(Path::new("staging"))?;
                root.ensure_dir(Path::new("models/asr"))?;
                root.same_device(Path::new("staging"), Path::new("models/asr"))
            }
        }
    }

    pub(super) fn hold_verified_download_part(
        &self,
        nominal: &Path,
        expected_len: u64,
        expected_sha256: &str,
    ) -> Result<VerifiedDownloadPart, ManagerError> {
        match self {
            #[cfg(test)]
            Self::TestPath(_) => VerifiedDownloadPart::capture_path(
                nominal,
                expected_len,
                expected_sha256.to_owned(),
            ),
            Self::Anchored(root) => {
                let relative = nominal
                    .strip_prefix(root.nominal_root())
                    .map_err(|_| ManagerError::integrity("download part path mismatch"))?;
                VerifiedDownloadPart::capture_anchored(
                    root.clone(),
                    relative.to_path_buf(),
                    expected_len,
                    expected_sha256.to_owned(),
                )
            }
        }
    }

    pub(super) fn real_directory(&self, nominal: &Path) -> Result<bool, ManagerError> {
        match self {
            #[cfg(test)]
            Self::TestPath(_) => super::fs_support::real_dir(nominal),
            Self::Anchored(root) => {
                let relative = nominal
                    .strip_prefix(root.nominal_root())
                    .map_err(|_| ManagerError::integrity("installation path mismatch"))?;
                Ok(root.open_dir(relative)?.is_some())
            }
        }
    }
}

pub(super) struct VerifiedDownloadPart {
    file: File,
    expected_len: u64,
    expected_sha256: String,
    anchored: Option<AnchoredVerifiedPart>,
}

struct AnchoredVerifiedPart {
    root: Arc<AnchoredFs>,
    relative: PathBuf,
    identity: EntryStat,
}

impl VerifiedDownloadPart {
    #[cfg(test)]
    fn capture_path(
        path: &Path,
        expected_len: u64,
        expected_sha256: String,
    ) -> Result<Self, ManagerError> {
        let file = File::open(path)?;
        let held = Self {
            file,
            expected_len,
            expected_sha256,
            anchored: None,
        };
        held.validate_contents()?;
        Ok(held)
    }

    fn capture_anchored(
        root: Arc<AnchoredFs>,
        relative: PathBuf,
        expected_len: u64,
        expected_sha256: String,
    ) -> Result<Self, ManagerError> {
        let file = root.open_regular(&relative, false)?;
        let identity = EntryStat::from_fd(file.as_raw_fd())?;
        let held = Self {
            file,
            expected_len,
            expected_sha256,
            anchored: Some(AnchoredVerifiedPart {
                root,
                relative,
                identity,
            }),
        };
        held.revalidate()?;
        Ok(held)
    }

    pub(super) fn copy_to(&self, output: &mut File) -> Result<(), ManagerError> {
        self.revalidate()?;
        let mut offset = 0u64;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let read = self.file.read_at(&mut buffer, offset)?;
            if read == 0 {
                break;
            }
            std::io::Write::write_all(output, &buffer[..read])?;
            offset += read as u64;
        }
        output.sync_all()?;
        self.revalidate()
    }

    pub(super) fn extract_tar_bz2_to(
        &self,
        storage: &AnchoredFs,
        staging: &Path,
        contract: &super::InstallContract,
    ) -> Result<u64, ManagerError> {
        self.revalidate()?;
        let staging = storage.reanchor(staging)?;
        let reader = self.file.try_clone()?;
        let written = super::archive::extract_tar_bz2_from_held_files(reader, &staging, contract)?;
        self.revalidate()?;
        Ok(written)
    }

    fn revalidate(&self) -> Result<(), ManagerError> {
        if let Some(anchored) = &self.anchored {
            if EntryStat::from_fd(self.file.as_raw_fd())? != anchored.identity {
                return Err(ManagerError::integrity(
                    "verified download part identity changed",
                ));
            }
            let current = anchored.root.open_regular(&anchored.relative, false)?;
            if EntryStat::from_fd(current.as_raw_fd())? != anchored.identity {
                return Err(ManagerError::integrity(
                    "verified download part path no longer names the held inode",
                ));
            }
        }
        self.validate_contents()
    }

    fn validate_contents(&self) -> Result<(), ManagerError> {
        if EntryStat::from_fd(self.file.as_raw_fd())?.len != self.expected_len {
            return Err(ManagerError::integrity(
                "verified artifact changed before install",
            ));
        }
        let mut digest = Sha256::new();
        let mut offset = 0u64;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let read = self.file.read_at(&mut buffer, offset)?;
            if read == 0 {
                break;
            }
            digest.update(&buffer[..read]);
            offset += read as u64;
        }
        if hex::encode(digest.finalize()) != self.expected_sha256 {
            return Err(ManagerError::integrity(
                "verified artifact changed before install",
            ));
        }
        Ok(())
    }
}

pub(super) struct DownloadPart {
    nominal_path: PathBuf,
    file: File,
    anchored: Option<AnchoredPart>,
}

struct AnchoredPart {
    root: Arc<AnchoredFs>,
    relative: PathBuf,
    identity: EntryStat,
}

impl DownloadPart {
    pub(super) fn nominal_path(&self) -> &Path {
        &self.nominal_path
    }

    pub(super) fn len(&self) -> Result<u64, ManagerError> {
        Ok(EntryStat::from_fd(self.file.as_raw_fd())?.len)
    }

    pub(super) fn truncate_and_sync(&mut self, len: u64) -> Result<(), ManagerError> {
        self.file.set_len(len)?;
        self.file.seek(SeekFrom::Start(len))?;
        self.file.sync_all()?;
        self.revalidate()
    }

    pub(super) fn prepare_write(&mut self, offset: u64) -> Result<(), ManagerError> {
        self.file.set_len(offset)?;
        self.file.seek(SeekFrom::Start(offset))?;
        self.revalidate()
    }

    pub(super) fn writer(&mut self) -> &mut File {
        &mut self.file
    }

    pub(super) fn sha256(&self) -> Result<String, ManagerError> {
        let mut digest = Sha256::new();
        let mut offset = 0u64;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let read = self.file.read_at(&mut buffer, offset)?;
            if read == 0 {
                break;
            }
            digest.update(&buffer[..read]);
            offset += read as u64;
        }
        Ok(hex::encode(digest.finalize()))
    }

    pub(super) fn revalidate(&self) -> Result<(), ManagerError> {
        let Some(anchored) = &self.anchored else {
            return Ok(());
        };
        if !EntryStat::from_fd(self.file.as_raw_fd())?.same_object(anchored.identity) {
            return Err(ManagerError::integrity("download part identity changed"));
        }
        let current = anchored.root.open_regular(&anchored.relative, false)?;
        if !EntryStat::from_fd(current.as_raw_fd())?.same_object(anchored.identity) {
            return Err(ManagerError::integrity(
                "download part path no longer names the held inode",
            ));
        }
        Ok(())
    }
}

#[derive(Debug)]
pub(super) enum InstallationStorage {
    #[cfg(test)]
    TestPath,
    Anchored(AnchoredInstallation),
}

impl InstallationStorage {
    pub(super) fn is_anchored(&self) -> bool {
        matches!(self, Self::Anchored(_))
    }

    pub(super) fn revalidate(&self) -> Result<(), ManagerError> {
        match self {
            #[cfg(test)]
            Self::TestPath => Ok(()),
            Self::Anchored(installation) => installation.revalidate(),
        }
    }

    pub(super) fn open_required(&self, relative: &Path) -> Result<Option<File>, ManagerError> {
        match self {
            #[cfg(test)]
            Self::TestPath => Ok(None),
            Self::Anchored(installation) => installation.open_required(relative).map(Some),
        }
    }

    pub(super) fn open_directory(&self, relative: &Path) -> Result<Option<File>, ManagerError> {
        match self {
            #[cfg(test)]
            Self::TestPath => Ok(None),
            Self::Anchored(installation) => installation.open_directory(relative).map(Some),
        }
    }
}

#[derive(Debug)]
pub(super) struct AnchoredInstallation {
    root: Arc<AnchoredFs>,
    root_identity: EntryStat,
    files: Vec<HeldFile>,
}

#[derive(Debug)]
struct HeldFile {
    relative: PathBuf,
    file: File,
    identity: EntryStat,
    expected_len: u64,
    expected_sha256: String,
}

impl AnchoredInstallation {
    pub(super) fn capture(
        storage: &AnchoredFs,
        relative: &Path,
        required: &[(PathBuf, u64, String)],
    ) -> Result<Self, ManagerError> {
        let root = Arc::new(storage.reanchor(relative)?);
        let root_identity = root.root_identity()?;
        let files = required
            .iter()
            .map(|(path, bytes, sha256)| {
                HeldFile::capture(root.as_ref(), path, *bytes, sha256.clone())
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            root,
            root_identity,
            files,
        })
    }

    pub(super) fn revalidate(&self) -> Result<(), ManagerError> {
        if self.root.root_identity()? != self.root_identity {
            return Err(ManagerError::integrity(
                "installation directory identity changed",
            ));
        }
        for file in &self.files {
            file.revalidate(self.root.as_ref())?;
        }
        Ok(())
    }

    pub(super) fn root(&self) -> &AnchoredFs {
        &self.root
    }

    fn open_required(&self, relative: &Path) -> Result<File, ManagerError> {
        let held = self
            .files
            .iter()
            .find(|file| file.relative == relative)
            .ok_or_else(|| ManagerError::structural("file is not part of the execution lease"))?;
        held.validate_contents()?;
        held.file.try_clone().map_err(Into::into)
    }

    fn open_directory(&self, relative: &Path) -> Result<File, ManagerError> {
        let contains_required_file = self
            .files
            .iter()
            .any(|file| file.relative.starts_with(relative) && file.relative != relative);
        if !contains_required_file {
            return Err(ManagerError::structural(
                "directory is not part of the execution lease",
            ));
        }
        self.root
            .open_dir(relative)?
            .ok_or_else(|| ManagerError::integrity("leased directory is missing"))
    }

    #[cfg(test)]
    fn read_required(&self, relative: &Path) -> Result<Vec<u8>, ManagerError> {
        let mut file = self.open_required(relative)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        Ok(bytes)
    }
}

impl HeldFile {
    fn capture(
        root: &AnchoredFs,
        relative: &Path,
        expected_len: u64,
        expected_sha256: String,
    ) -> Result<Self, ManagerError> {
        let file = root.open_regular(relative, false)?;
        let identity = EntryStat::from_fd(file.as_raw_fd())?;
        let held = Self {
            relative: relative.to_path_buf(),
            file,
            identity,
            expected_len,
            expected_sha256,
        };
        held.validate_contents()?;
        Ok(held)
    }

    fn revalidate(&self, root: &AnchoredFs) -> Result<(), ManagerError> {
        if EntryStat::from_fd(self.file.as_raw_fd())? != self.identity {
            return Err(ManagerError::integrity("leased file identity changed"));
        }
        self.open_current(root)?;
        self.validate_contents()
    }

    fn open_current(&self, root: &AnchoredFs) -> Result<File, ManagerError> {
        let current = root.open_regular(&self.relative, false)?;
        if EntryStat::from_fd(current.as_raw_fd())? != self.identity {
            return Err(ManagerError::integrity(
                "leased file path no longer names the held inode",
            ));
        }
        Ok(current)
    }

    fn validate_contents(&self) -> Result<(), ManagerError> {
        if self.identity.len != self.expected_len {
            return Err(ManagerError::integrity("leased file length mismatch"));
        }
        let mut digest = Sha256::new();
        let mut offset = 0u64;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let read = self.file.read_at(&mut buffer, offset)?;
            if read == 0 {
                break;
            }
            digest.update(&buffer[..read]);
            offset += read as u64;
        }
        if hex::encode(digest.finalize()) != self.expected_sha256 {
            return Err(ManagerError::integrity("leased file hash mismatch"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs::{self, File};
    use std::path::{Path, PathBuf};

    use sha2::{Digest, Sha256};
    use tempfile::TempDir;

    use super::{AnchoredFs, AnchoredInstallation};

    #[test]
    fn anchored_installation_stays_on_held_inode_after_root_entry_swap() {
        let parent = TempDir::new().unwrap();
        let root = parent.path().join("data");
        let held = parent.path().join("held");
        let relative = Path::new("models/asr/provider/model/1-bundle");
        fs::create_dir_all(root.join(relative)).unwrap();
        fs::write(root.join(relative).join("model.bin"), b"held-model").unwrap();
        let root_dir = File::open(&root).unwrap();
        let storage = AnchoredFs::new(root.clone(), root_dir);
        let expected = hex::encode(Sha256::digest(b"held-model"));
        let installation = AnchoredInstallation::capture(
            &storage,
            relative,
            &[(PathBuf::from("model.bin"), 10, expected)],
        )
        .unwrap();

        fs::rename(&root, &held).unwrap();
        fs::create_dir_all(root.join(relative)).unwrap();
        fs::write(root.join(relative).join("model.bin"), b"replacement").unwrap();

        installation.revalidate().unwrap();
        assert_eq!(
            installation.read_required(Path::new("model.bin")).unwrap(),
            b"held-model"
        );
    }

    #[test]
    fn anchored_installation_rejects_required_file_inode_replacement() {
        let root = TempDir::new().unwrap();
        let relative = Path::new("models/asr/provider/model/1-bundle");
        fs::create_dir_all(root.path().join(relative)).unwrap();
        let model = root.path().join(relative).join("model.bin");
        fs::write(&model, b"held-model").unwrap();
        let storage = AnchoredFs::new(root.path().to_path_buf(), File::open(root.path()).unwrap());
        let expected = hex::encode(Sha256::digest(b"held-model"));
        let installation = AnchoredInstallation::capture(
            &storage,
            relative,
            &[(PathBuf::from("model.bin"), 10, expected)],
        )
        .unwrap();

        fs::rename(&model, model.with_extension("old")).unwrap();
        fs::write(&model, b"held-model").unwrap();

        assert_eq!(
            installation.revalidate().unwrap_err().code(),
            "model_integrity_failed"
        );
        assert_eq!(
            installation.read_required(Path::new("model.bin")).unwrap(),
            b"held-model"
        );
    }
}
