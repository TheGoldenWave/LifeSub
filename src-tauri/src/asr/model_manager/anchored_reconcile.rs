use std::ffi::{CStr, CString, OsStr, OsString};
use std::fs::File;
use std::io::{self, Read};
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::fs_support::{source_identity, validate_component};
use super::install::{MarkerInstalledFile, StructuralMarker, matches_marker};
use super::install_support::validate_inventory;
use super::types::ReconcileOutcome;
use super::*;

#[cfg(test)]
thread_local! {
    static BEFORE_REMOVE_RENAME: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
        std::cell::RefCell::new(None);
}

struct AnchoredFs {
    root: std::sync::Arc<File>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct EntryStat {
    device: u64,
    inode: u64,
    mode: libc::mode_t,
    len: u64,
}

impl EntryStat {
    fn is_dir(self) -> bool {
        self.mode & libc::S_IFMT == libc::S_IFDIR
    }

    fn is_file(self) -> bool {
        self.mode & libc::S_IFMT == libc::S_IFREG
    }
}

impl AnchoredFs {
    fn new(root: std::sync::Arc<File>) -> Self {
        Self { root }
    }

    fn open_dir(&self, relative: &Path) -> Result<Option<File>, ManagerError> {
        open_dir_from(self.root.as_raw_fd(), relative)
    }

    fn ensure_dir(&self, relative: &Path) -> Result<File, ManagerError> {
        let mut current = self.root.try_clone()?;
        for component in relative.components() {
            let name = component_name(component.as_os_str())?;
            match open_dir_at(current.as_raw_fd(), &name) {
                Ok(next) => current = next,
                Err(error) if error.raw_os_error() == Some(libc::ENOENT) => {
                    let result =
                        unsafe { libc::mkdirat(current.as_raw_fd(), name.as_ptr(), 0o700) };
                    if result != 0
                        && io::Error::last_os_error().raw_os_error() != Some(libc::EEXIST)
                    {
                        return Err(io::Error::last_os_error().into());
                    }
                    current = open_dir_at(current.as_raw_fd(), &name).map_err(map_unsafe_entry)?;
                }
                Err(error) => return Err(map_unsafe_entry(error)),
            }
        }
        Ok(current)
    }

    fn remove_tree(&self, relative: &Path) -> Result<(), ManagerError> {
        let Some((parent, name)) = self.parent_and_name(relative)? else {
            return Err(ManagerError::structural("cannot remove anchored root"));
        };
        remove_entry_at(parent.as_raw_fd(), &name)
    }

    fn remove_file(&self, relative: &Path) -> Result<(), ManagerError> {
        let Some((parent, name)) = self.parent_and_name(relative)? else {
            return Err(ManagerError::structural("cannot remove anchored root"));
        };
        let stat = stat_at(parent.as_raw_fd(), &name)?;
        if !stat.is_file() {
            return Err(ManagerError::structural("expected regular file"));
        }
        unlink_at(parent.as_raw_fd(), &name, 0)
    }

    fn read_regular(&self, relative: &Path) -> Result<Vec<u8>, ManagerError> {
        let mut file = self.open_regular(relative, false)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        Ok(bytes)
    }

    fn file_len(&self, relative: &Path) -> Result<Option<u64>, ManagerError> {
        let Some((parent, name)) = self.parent_and_name(relative)? else {
            return Ok(None);
        };
        match stat_at(parent.as_raw_fd(), &name) {
            Ok(stat) if stat.is_file() => Ok(Some(stat.len)),
            Ok(_) => Err(ManagerError::structural("expected regular file")),
            Err(error) if is_not_found(&error) => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn truncate_and_sync(&self, relative: &Path, len: u64) -> Result<(), ManagerError> {
        let file = self.open_regular(relative, true)?;
        file.set_len(len)?;
        file.sync_all()?;
        Ok(())
    }

    fn sha256(&self, relative: &Path) -> Result<String, ManagerError> {
        let mut file = self.open_regular(relative, false)?;
        let mut digest = Sha256::new();
        let mut buffer = [0u8; COPY_BUFFER_BYTES];
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            digest.update(&buffer[..read]);
        }
        Ok(hex::encode(digest.finalize()))
    }

    fn open_regular(&self, relative: &Path, writable: bool) -> Result<File, ManagerError> {
        let Some((parent, name)) = self.parent_and_name(relative)? else {
            return Err(ManagerError::structural("expected regular file"));
        };
        let flags = if writable {
            libc::O_RDWR
        } else {
            libc::O_RDONLY
        };
        let fd = unsafe {
            libc::openat(
                parent.as_raw_fd(),
                name.as_ptr(),
                flags | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(map_unsafe_entry(io::Error::last_os_error()));
        }
        let file = unsafe { File::from_raw_fd(fd) };
        let stat = file_stat(file.as_raw_fd())?;
        if !stat.is_file() {
            return Err(ManagerError::structural("expected regular file"));
        }
        Ok(file)
    }

    fn entries(&self, relative: &Path) -> Result<Option<Vec<OsString>>, ManagerError> {
        let Some(dir) = self.open_dir(relative)? else {
            return Ok(None);
        };
        Ok(Some(read_dir_names(&dir)?))
    }

    fn entry_stat(&self, relative: &Path) -> Result<EntryStat, ManagerError> {
        let Some((parent, name)) = self.parent_and_name(relative)? else {
            return file_stat(self.root.as_raw_fd());
        };
        stat_at(parent.as_raw_fd(), &name)
    }

    fn rename(&self, source: &Path, destination: &Path) -> Result<(), ManagerError> {
        let Some((source_parent, source_name)) = self.parent_and_name(source)? else {
            return Err(ManagerError::structural("cannot rename anchored root"));
        };
        let Some((destination_parent, destination_name)) = self.parent_and_name(destination)?
        else {
            return Err(ManagerError::structural("cannot rename to anchored root"));
        };
        let result = unsafe {
            libc::renameat(
                source_parent.as_raw_fd(),
                source_name.as_ptr(),
                destination_parent.as_raw_fd(),
                destination_name.as_ptr(),
            )
        };
        if result != 0 {
            return Err(io::Error::last_os_error().into());
        }
        destination_parent.sync_all()?;
        source_parent.sync_all()?;
        Ok(())
    }

    fn sync_dir(&self, relative: &Path) -> Result<(), ManagerError> {
        if let Some(dir) = self.open_dir(relative)? {
            dir.sync_all()?;
        }
        Ok(())
    }

    fn parent_and_name(&self, relative: &Path) -> Result<Option<(File, CString)>, ManagerError> {
        let Some(name) = relative.file_name() else {
            return Ok(None);
        };
        let parent = relative.parent().unwrap_or_else(|| Path::new(""));
        let Some(parent) = self.open_dir(parent)? else {
            return Err(io::Error::from(io::ErrorKind::NotFound).into());
        };
        Ok(Some((parent, component_name(name)?)))
    }
}

impl<T: HttpTransport, C: ModelCatalog> ModelManager<T, C> {
    fn anchored_fs(&self) -> Result<AnchoredFs, ManagerError> {
        self.anchored_root
            .clone()
            .map(AnchoredFs::new)
            .ok_or_else(|| ManagerError::structural("anchored data directory is missing"))
    }
}

impl<T: HttpTransport> ModelManager<T, Catalog> {
    pub fn reconcile_all_anchored(&self) -> Result<(), ManagerError> {
        let fs = self.anchored_fs()?;
        self.reconcile_trash_anchored(&fs)?;
        self.reconcile_downloads_anchored(&fs)?;
        clear_directory(&fs, Path::new("staging"), "staging")?;
        self.reconcile_installations_anchored(&fs)?;
        self.reconcile_unrecorded_anchored(&fs)
    }

    fn reconcile_downloads_anchored(&self, fs: &AnchoredFs) -> Result<(), ManagerError> {
        let records = self.catalog.model_download_records()?;
        let known = records
            .iter()
            .map(|record| record.id.as_str())
            .collect::<HashSet<_>>();
        for record in &records {
            if validate_component("download_id", &record.id).is_err() {
                self.catalog.mark_download_recovery_required(&record.id)?;
                continue;
            }
            let plan = match ModelInstallPlan::resolve(
                &record.model_id,
                &record.manifest_version,
                &record.bundle_identity,
            ) {
                Ok(plan) => plan,
                Err(_) => {
                    self.catalog.mark_download_recovery_required(&record.id)?;
                    continue;
                }
            };
            let artifact_ids = self
                .catalog
                .model_download_artifact_ids(&record.id)?
                .into_iter()
                .collect::<HashSet<_>>();
            for artifact in &plan.artifacts {
                let Some(checkpoint) =
                    self.catalog.checkpoint(&record.id, &artifact.artifact_id)?
                else {
                    continue;
                };
                let relative = PathBuf::from("downloads")
                    .join(&record.id)
                    .join(format!("{}.part", artifact.artifact_id));
                let nominal = self.root.join(&relative);
                let source_matches = checkpoint.source_identity == source_identity(artifact)
                    && checkpoint.expected_bytes == artifact.expected_bytes
                    && checkpoint.temp_path == nominal;
                if !source_matches {
                    if fs.file_len(&relative)?.is_some() {
                        fs.remove_file(&relative)?;
                    }
                    self.catalog.save_checkpoint(
                        &record.id,
                        &ArtifactCheckpoint {
                            artifact_id: artifact.artifact_id.clone(),
                            source_identity: source_identity(artifact),
                            downloaded_bytes: 0,
                            expected_bytes: artifact.expected_bytes,
                            temp_path: nominal,
                            etag: None,
                            last_modified: None,
                            verified_sha256: None,
                            state: "pending".to_owned(),
                        },
                    )?;
                    continue;
                }
                let file_len = fs.file_len(&relative)?.unwrap_or(0);
                let durable = checkpoint.downloaded_bytes.min(file_len);
                if file_len > durable {
                    fs.truncate_and_sync(&relative, durable)?;
                }
                let verified = durable == artifact.expected_bytes
                    && checkpoint.verified_sha256.as_deref() == Some(&artifact.expected_sha256)
                    && fs.sha256(&relative)? == artifact.expected_sha256;
                self.catalog.save_checkpoint(
                    &record.id,
                    &ArtifactCheckpoint {
                        artifact_id: artifact.artifact_id.clone(),
                        source_identity: checkpoint.source_identity,
                        downloaded_bytes: durable,
                        expected_bytes: artifact.expected_bytes,
                        temp_path: nominal,
                        etag: checkpoint.etag,
                        last_modified: checkpoint.last_modified,
                        verified_sha256: verified.then(|| artifact.expected_sha256.clone()),
                        state: if verified { "verified" } else { "downloading" }.to_owned(),
                    },
                )?;
            }
            let download_dir = PathBuf::from("downloads").join(&record.id);
            if let Some(entries) = fs.entries(&download_dir)? {
                for name in entries {
                    let Some(name_utf8) = name.to_str() else {
                        return Err(ManagerError::structural("non-UTF-8 download entry"));
                    };
                    if let Some(artifact_id) = name_utf8.strip_suffix(".part")
                        && !artifact_ids.contains(artifact_id)
                    {
                        fs.remove_file(&download_dir.join(name))?;
                    }
                }
                fs.sync_dir(&download_dir)?;
            }
            if matches!(
                record.state.as_str(),
                "queued" | "downloading" | "verifying" | "installing"
            ) {
                self.catalog.mark_download_recovery_required(&record.id)?;
            }
        }
        if let Some(entries) = fs.entries(Path::new("downloads"))? {
            for name in entries {
                let relative = PathBuf::from("downloads").join(&name);
                if !fs.entry_stat(&relative)?.is_dir() {
                    return Err(ManagerError::structural(
                        "downloads root contains a non-directory entry",
                    ));
                }
                if !name.to_str().is_some_and(|name| known.contains(name)) {
                    fs.remove_tree(&relative)?;
                }
            }
            fs.sync_dir(Path::new("downloads"))?;
        }
        Ok(())
    }

    fn reconcile_trash_anchored(&self, fs: &AnchoredFs) -> Result<(), ManagerError> {
        let Some(entries) = fs.entries(Path::new("trash"))? else {
            return Ok(());
        };
        for name in entries {
            let relative = PathBuf::from("trash").join(name);
            let stat = fs.entry_stat(&relative)?;
            if stat.is_file() {
                fs.remove_file(&relative)?;
                continue;
            }
            if !stat.is_dir() {
                return Err(ManagerError::structural(
                    "trash contains a link or special entry",
                ));
            }
            let marker = relative.join(DELETE_MARKER);
            if fs.file_len(&marker)?.is_some() {
                let lease: DeletionLease = serde_json::from_slice(&fs.read_regular(&marker)?)
                    .map_err(|_| ManagerError::integrity("invalid deletion marker"))?;
                if self.catalog.model_deletion_lease(&lease.model_id)? != Some(lease.clone()) {
                    return Err(ManagerError::integrity(
                        "trash deletion marker does not match current Catalog lease",
                    ));
                }
                self.catalog.complete_deletion_recovery(&lease)?;
            }
            fs.remove_tree(&relative)?;
        }
        fs.sync_dir(Path::new("trash"))
    }

    fn reconcile_installations_anchored(&self, fs: &AnchoredFs) -> Result<(), ManagerError> {
        for record in self.catalog.model_installation_records()? {
            let plan = match ModelInstallPlan::resolve(
                &record.model_id,
                &record.manifest_version,
                &record.bundle_identity,
            ) {
                Ok(plan) => plan,
                Err(_) => {
                    self.catalog.record_installation_recovery(
                        &record.model_id,
                        "model_manifest_identity_mismatch",
                    )?;
                    continue;
                }
            };
            if record.install_dir != self.install_dir(&plan) {
                self.catalog
                    .record_installation_recovery(&record.model_id, "model_integrity_failed")?;
                continue;
            }
            let relative = install_relative(&plan);
            if record.state == "deleting" {
                self.reconcile_deleting_anchored(fs, &relative, &record)?;
                continue;
            }
            match self.reconcile_installation_anchored(fs, &relative, &plan)? {
                ReconcileOutcome::Recovered(_) | ReconcileOutcome::RejectedDurably(_) => {}
            }
        }
        Ok(())
    }

    fn reconcile_deleting_anchored(
        &self,
        fs: &AnchoredFs,
        relative: &Path,
        record: &ModelInstallationRecord,
    ) -> Result<(), ManagerError> {
        if fs.open_dir(relative)?.is_none() {
            self.catalog
                .complete_deletion_recovery(&lease_from_record_anchored(record))?;
            return Ok(());
        }
        let marker = relative.join(DELETE_MARKER);
        if fs.file_len(&marker)?.is_some() {
            let lease: DeletionLease = serde_json::from_slice(&fs.read_regular(&marker)?)
                .map_err(|_| ManagerError::integrity("invalid deletion marker"))?;
            if lease != lease_from_record_anchored(record) {
                return Err(ManagerError::integrity(
                    "deletion marker does not match current Catalog lease",
                ));
            }
            fs.ensure_dir(Path::new("trash"))?;
            let trash = PathBuf::from("trash").join(format!(
                "{}-recovery-{}",
                record.model_id,
                uuid::Uuid::new_v4().simple()
            ));
            fs.rename(relative, &trash)?;
            self.catalog.complete_deletion_recovery(&lease)?;
            fs.remove_tree(&trash)?;
            fs.sync_dir(Path::new("trash"))?;
        } else {
            let temporary = relative.join(format!("{DELETE_MARKER}.tmp"));
            if fs.file_len(&temporary)?.is_some() {
                fs.remove_file(&temporary)?;
            }
            self.catalog
                .abort_delete(&lease_from_record_anchored(record))?;
        }
        Ok(())
    }

    fn reconcile_unrecorded_anchored(&self, fs: &AnchoredFs) -> Result<(), ManagerError> {
        let root = Path::new("models/asr");
        let Some(providers) = fs.entries(root)? else {
            return Ok(());
        };
        let recorded = self
            .catalog
            .model_installation_records()?
            .into_iter()
            .map(|r| r.install_dir)
            .collect::<HashSet<_>>();
        for provider in providers {
            let provider_path = root.join(&provider);
            require_directory(
                fs,
                &provider_path,
                "models provider entry is not a real directory",
            )?;
            for model in fs.entries(&provider_path)?.unwrap_or_default() {
                let model_path = provider_path.join(&model);
                require_directory(
                    fs,
                    &model_path,
                    "models model entry is not a real directory",
                )?;
                let Some(model_id) = model.to_str() else {
                    return Err(ManagerError::structural("non-UTF-8 model directory"));
                };
                for final_name in fs.entries(&model_path)?.unwrap_or_default() {
                    let final_path = model_path.join(&final_name);
                    require_directory(
                        fs,
                        &final_path,
                        "models final entry is not a real directory",
                    )?;
                    if recorded.contains(&self.root.join(&final_path)) {
                        continue;
                    }
                    let Some(identity) = final_name.to_str() else {
                        self.quarantine_unknown_anchored(fs, &final_path, "unknown-model")?;
                        continue;
                    };
                    let Some(plan) = model_registry()
                        .model(model_id)
                        .map(ModelInstallPlan::from_manifest)
                        .or_else(|| {
                            (vad_manifest().id == model_id)
                                .then(|| ModelInstallPlan::from_vad_manifest(vad_manifest()))
                        })
                    else {
                        self.quarantine_unknown_anchored(fs, &final_path, "unknown-model")?;
                        continue;
                    };
                    if identity != format!("{}-{}", plan.manifest_version, plan.bundle_identity) {
                        self.quarantine_unknown_anchored(fs, &final_path, "identity-mismatch")?;
                        continue;
                    }
                    match self.reconcile_installation_anchored(fs, &final_path, &plan)? {
                        ReconcileOutcome::Recovered(_) | ReconcileOutcome::RejectedDurably(_) => {}
                    }
                }
            }
        }
        Ok(())
    }

    fn reconcile_installation_anchored(
        &self,
        fs: &AnchoredFs,
        relative: &Path,
        plan: &ModelInstallPlan,
    ) -> Result<ReconcileOutcome, ManagerError> {
        if fs.open_dir(relative)?.is_none() {
            self.catalog
                .record_installation_recovery(&plan.model_id, "model_integrity_failed")?;
            return Ok(ReconcileOutcome::RejectedDurably(ManagerError::new(
                "model_install_missing",
                "install directory is missing",
            )));
        }
        let validation = validate_installation_anchored(
            fs,
            relative,
            plan,
            self.observed_sherpa_runtime.as_ref(),
        );
        if let Err(error) = validation {
            let recovery_code = recovery_code(&error);
            self.catalog
                .record_installation_recovery(&plan.model_id, recovery_code)?;
            self.quarantine_unknown_anchored(fs, relative, "reconcile-rejected")?;
            return Ok(ReconcileOutcome::RejectedDurably(error));
        }
        let runtime_identity_json = plan
            .sherpa_runtime
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| ManagerError::structural(e.to_string()))?;
        let installation = StoredInstallation {
            model_id: plan.model_id.clone(),
            provider: plan.provider.clone(),
            manifest_version: plan.manifest_version.clone(),
            bundle_identity: plan.bundle_identity.clone(),
            install_dir: self.root.join(relative),
            state: match plan.qualification_policy {
                QualificationPolicy::StructuralWithPinnedRuntime => "runtime_qualified",
                QualificationPolicy::RuntimeSmokeRequired => "installed_unqualified",
            }
            .to_owned(),
            runtime_identity_json,
        };
        self.catalog.publish_installation(&installation)?;
        Ok(ReconcileOutcome::Recovered(installation))
    }

    fn quarantine_unknown_anchored(
        &self,
        fs: &AnchoredFs,
        source: &Path,
        reason: &str,
    ) -> Result<(), ManagerError> {
        fs.ensure_dir(Path::new("quarantine"))?;
        let destination =
            PathBuf::from("quarantine").join(format!("{reason}-{}", uuid::Uuid::new_v4().simple()));
        fs.rename(source, &destination)?;
        fs.sync_dir(Path::new("quarantine"))
    }
}

fn recovery_code(error: &ManagerError) -> &'static str {
    if error.code() == "model_runtime_identity_mismatch" {
        "model_runtime_identity_mismatch"
    } else {
        "model_runtime_qualification_recovery_required"
    }
}

fn validate_installation_anchored(
    fs: &AnchoredFs,
    relative: &Path,
    plan: &ModelInstallPlan,
    observed: Option<&FullSherpaRuntimeIdentity>,
) -> Result<(), ManagerError> {
    let marker: StructuralMarker =
        serde_json::from_slice(&fs.read_regular(&relative.join(STRUCTURAL_MARKER))?)
            .map_err(|_| ManagerError::structural("invalid structural marker"))?;
    let runtime = plan
        .sherpa_runtime
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|e| ManagerError::structural(e.to_string()))?;
    if !matches_marker(&marker, plan, runtime.as_deref()) {
        return Err(ManagerError::structural("structural marker mismatch"));
    }
    let actual = installed_records_anchored(fs, relative)?;
    if marker.installed_files != actual {
        return Err(ManagerError::integrity(
            "installed file inventory or hash mismatch",
        ));
    }
    validate_inventory(plan, &actual)?;
    for required in plan.install_contract.required_files() {
        fs.file_len(&relative.join(safe_relative_path(&required.path)?))?
            .ok_or_else(|| ManagerError::structural("required path missing"))?;
    }
    match plan.qualification_policy {
        QualificationPolicy::StructuralWithPinnedRuntime => {
            if !matches!((plan.sherpa_runtime.as_ref(), observed), (Some(expected), Some(actual)) if expected.matches(actual))
            {
                return Err(ManagerError::new(
                    "model_runtime_identity_mismatch",
                    "pinned sherpa identity missing",
                ));
            }
        }
        QualificationPolicy::RuntimeSmokeRequired => validate_qwen_anchored(fs, relative)?,
    }
    Ok(())
}

fn installed_records_anchored(
    fs: &AnchoredFs,
    root: &Path,
) -> Result<Vec<MarkerInstalledFile>, ManagerError> {
    fn visit(
        fs: &AnchoredFs,
        root: &Path,
        current: &Path,
        output: &mut Vec<MarkerInstalledFile>,
    ) -> Result<(), ManagerError> {
        for name in fs.entries(current)?.unwrap_or_default() {
            let path = current.join(&name);
            let stat = fs.entry_stat(&path)?;
            if stat.is_dir() {
                visit(fs, root, &path, output)?;
            } else if !stat.is_file() {
                return Err(ManagerError::structural("special file in installed bundle"));
            } else if !matches!(
                name.to_str(),
                Some(STRUCTURAL_MARKER)
                    | Some(DELETE_MARKER)
                    | Some(crate::asr::runtime_qualifier::RUNTIME_QUALIFICATION_MARKER)
            ) {
                let relative_path = path
                    .strip_prefix(root)
                    .map_err(|_| ManagerError::structural("installed path escaped root"))?
                    .to_str()
                    .ok_or_else(|| ManagerError::structural("non-UTF-8 installed path"))?
                    .to_owned();
                output.push(MarkerInstalledFile {
                    relative_path,
                    bytes: stat.len,
                    sha256: fs.sha256(&path)?,
                });
            }
        }
        Ok(())
    }
    let mut output = Vec::new();
    visit(fs, root, root, &mut output)?;
    output.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(output)
}

fn validate_qwen_anchored(fs: &AnchoredFs, root: &Path) -> Result<(), ManagerError> {
    let config: serde_json::Value =
        serde_json::from_slice(&fs.read_regular(&root.join("config.json"))?)
            .map_err(|_| ManagerError::structural("invalid Qwen config"))?;
    if !config
        .get("thinker_config")
        .is_some_and(serde_json::Value::is_object)
    {
        return Err(ManagerError::structural("Qwen thinker_config missing"));
    }
    let index: serde_json::Value =
        serde_json::from_slice(&fs.read_regular(&root.join("model.safetensors.index.json"))?)
            .map_err(|_| ManagerError::structural("invalid shard index"))?;
    let references = index
        .get("weight_map")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| ManagerError::structural("shard weight_map missing"))?
        .values()
        .filter_map(serde_json::Value::as_str)
        .collect::<HashSet<_>>();
    if references
        != [
            "model-00001-of-00002.safetensors",
            "model-00002-of-00002.safetensors",
        ]
        .into_iter()
        .collect()
    {
        return Err(ManagerError::structural("unexpected shard index coverage"));
    }
    let tokenizer: serde_json::Value =
        serde_json::from_slice(&fs.read_regular(&root.join("tokenizer.json"))?)
            .map_err(|_| ManagerError::structural("invalid tokenizer JSON"))?;
    let vocab = tokenizer
        .get("model")
        .and_then(|value| value.get("vocab"))
        .and_then(serde_json::Value::as_object)
        .is_some_and(|value| !value.is_empty());
    let tokens = tokenizer
        .get("added_tokens")
        .and_then(serde_json::Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.get("content").and_then(serde_json::Value::as_str))
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default();
    if !vocab
        || !["<|endoftext|>", "<|im_start|>", "<|im_end|>"]
            .iter()
            .all(|token| tokens.contains(token))
    {
        return Err(ManagerError::structural("invalid tokenizer contract"));
    }
    Ok(())
}

fn clear_directory(fs: &AnchoredFs, root: &Path, label: &str) -> Result<(), ManagerError> {
    let Some(entries) = fs.entries(root)? else {
        return Ok(());
    };
    for name in entries {
        let path = root.join(name);
        let stat = fs.entry_stat(&path)?;
        if !stat.is_dir() && !stat.is_file() {
            return Err(ManagerError::structural(format!(
                "{label} root contains a link or special entry"
            )));
        }
        fs.remove_tree(&path)?;
    }
    fs.sync_dir(root)
}

fn require_directory(fs: &AnchoredFs, path: &Path, detail: &str) -> Result<(), ManagerError> {
    if fs.entry_stat(path)?.is_dir() {
        Ok(())
    } else {
        Err(ManagerError::structural(detail))
    }
}

fn install_relative(plan: &ModelInstallPlan) -> PathBuf {
    PathBuf::from("models/asr")
        .join(&plan.provider)
        .join(&plan.model_id)
        .join(format!(
            "{}-{}",
            plan.manifest_version, plan.bundle_identity
        ))
}

fn lease_from_record_anchored(record: &ModelInstallationRecord) -> DeletionLease {
    DeletionLease {
        model_id: record.model_id.clone(),
        install_dir: record.install_dir.clone(),
        prior_state: if record.qualified_at.is_some() {
            "runtime_qualified"
        } else {
            "installed_unqualified"
        }
        .to_owned(),
        prior_runtime_identity_json: record.runtime_identity_json.clone(),
        prior_qualified_at: record.qualified_at.clone(),
        prior_last_error_code: record.last_error_code.clone(),
    }
}

fn open_dir_from(root_fd: RawFd, relative: &Path) -> Result<Option<File>, ManagerError> {
    let duplicated = unsafe { libc::fcntl(root_fd, libc::F_DUPFD_CLOEXEC, 0) };
    if duplicated < 0 {
        return Err(io::Error::last_os_error().into());
    }
    let mut current = unsafe { File::from_raw_fd(duplicated) };
    for component in relative.components() {
        let name = component_name(component.as_os_str())?;
        match open_dir_at(current.as_raw_fd(), &name) {
            Ok(next) => current = next,
            Err(error) if error.raw_os_error() == Some(libc::ENOENT) => return Ok(None),
            Err(error) => return Err(map_unsafe_entry(error)),
        }
    }
    Ok(Some(current))
}

fn open_dir_at(parent: RawFd, name: &CStr) -> io::Result<File> {
    let fd = unsafe {
        libc::openat(
            parent,
            name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(unsafe { File::from_raw_fd(fd) })
    }
}

fn read_dir_names(dir: &File) -> Result<Vec<OsString>, ManagerError> {
    let duplicated = unsafe { libc::fcntl(dir.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 0) };
    if duplicated < 0 {
        return Err(io::Error::last_os_error().into());
    }
    let stream = unsafe { libc::fdopendir(duplicated) };
    if stream.is_null() {
        unsafe { libc::close(duplicated) };
        return Err(io::Error::last_os_error().into());
    }
    let mut names = Vec::new();
    loop {
        let entry = unsafe { libc::readdir(stream) };
        if entry.is_null() {
            break;
        }
        let bytes = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
        if bytes != b"." && bytes != b".." {
            names.push(OsString::from_vec(bytes.to_vec()));
        }
    }
    unsafe { libc::closedir(stream) };
    Ok(names)
}

fn remove_entry_at(parent: RawFd, name: &CStr) -> Result<(), ManagerError> {
    let expected = stat_at(parent, name)?;
    #[cfg(test)]
    BEFORE_REMOVE_RENAME.with(|hook| {
        if let Some(hook) = hook.borrow_mut().take() {
            hook();
        }
    });
    let tombstone = component_name(OsStr::new(&format!(
        ".lifesub-remove-{}",
        uuid::Uuid::new_v4().simple()
    )))?;
    rename_noreplace(parent, name, &tombstone)?;
    let stat = stat_at(parent, &tombstone)?;
    if stat != expected {
        let _ = rename_noreplace(parent, &tombstone, name);
        return Err(ManagerError::structural(
            "anchored entry changed before removal",
        ));
    }
    if stat.is_file() {
        return unlink_at(parent, &tombstone, 0);
    }
    if !stat.is_dir() {
        return Err(ManagerError::structural(
            "refusing to remove link or special entry",
        ));
    }
    let dir = open_dir_at(parent, &tombstone).map_err(map_unsafe_entry)?;
    for child in read_dir_names(&dir)? {
        let child = component_name(&child)?;
        remove_entry_at(dir.as_raw_fd(), &child)?;
    }
    dir.sync_all()?;
    drop(dir);
    unlink_at(parent, &tombstone, libc::AT_REMOVEDIR)
}

fn rename_noreplace(parent: RawFd, source: &CStr, destination: &CStr) -> Result<(), ManagerError> {
    #[cfg(target_os = "macos")]
    let result = unsafe {
        libc::renameatx_np(
            parent,
            source.as_ptr(),
            parent,
            destination.as_ptr(),
            libc::RENAME_EXCL,
        )
    };
    #[cfg(not(target_os = "macos"))]
    let result = unsafe { libc::renameat(parent, source.as_ptr(), parent, destination.as_ptr()) };
    if result != 0 {
        Err(io::Error::last_os_error().into())
    } else {
        Ok(())
    }
}

fn unlink_at(parent: RawFd, name: &CStr, flags: i32) -> Result<(), ManagerError> {
    if unsafe { libc::unlinkat(parent, name.as_ptr(), flags) } != 0 {
        Err(io::Error::last_os_error().into())
    } else {
        Ok(())
    }
}

fn stat_at(parent: RawFd, name: &CStr) -> Result<EntryStat, ManagerError> {
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    if unsafe {
        libc::fstatat(
            parent,
            name.as_ptr(),
            stat.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } != 0
    {
        return Err(io::Error::last_os_error().into());
    }
    let stat = unsafe { stat.assume_init() };
    Ok(EntryStat {
        device: stat.st_dev as u64,
        inode: stat.st_ino,
        mode: stat.st_mode,
        len: stat.st_size as u64,
    })
}

fn file_stat(fd: RawFd) -> Result<EntryStat, ManagerError> {
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    if unsafe { libc::fstat(fd, stat.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error().into());
    }
    let stat = unsafe { stat.assume_init() };
    Ok(EntryStat {
        device: stat.st_dev as u64,
        inode: stat.st_ino,
        mode: stat.st_mode,
        len: stat.st_size as u64,
    })
}

fn component_name(value: &OsStr) -> Result<CString, ManagerError> {
    if value.is_empty() || value.as_bytes().contains(&b'/') {
        return Err(ManagerError::structural("unsafe anchored path component"));
    }
    CString::new(value.as_bytes())
        .map_err(|_| ManagerError::structural("NUL in anchored path component"))
}

fn map_unsafe_entry(error: io::Error) -> ManagerError {
    match error.raw_os_error() {
        Some(libc::ELOOP | libc::ENOTDIR) => {
            ManagerError::structural("anchored path contains a link or non-directory")
        }
        _ => error.into(),
    }
}

fn is_not_found(error: &ManagerError) -> bool {
    error.code() == "model_io_failed" && error.to_string().contains("No such file or directory")
}

#[cfg(test)]
mod tests {
    use std::fs::{self, File};

    use tempfile::TempDir;

    use super::super::*;
    use super::{BEFORE_REMOVE_RENAME, recovery_code};

    #[test]
    fn anchored_reconcile_stays_on_held_root_after_entry_swap() {
        let parent = TempDir::new().unwrap();
        let root = parent.path().join("data");
        let held = parent.path().join("held");
        fs::create_dir_all(root.join("downloads/orphan-download")).unwrap();
        fs::write(root.join("downloads/orphan-download/partial"), b"held").unwrap();
        fs::create_dir_all(root.join("staging/stale-install")).unwrap();
        fs::write(root.join("staging/stale-install/partial"), b"held").unwrap();
        fs::create_dir_all(root.join("trash")).unwrap();
        fs::write(root.join("trash/stale-marker"), b"held").unwrap();
        fs::create_dir_all(root.join("models/asr/unknown/model/identity")).unwrap();
        fs::write(
            root.join("models/asr/unknown/model/identity/model.bin"),
            b"held",
        )
        .unwrap();
        let root_dir = File::open(&root).unwrap();

        fs::rename(&root, &held).unwrap();
        let replacement_sentinels = [
            "downloads/replacement-download/sentinel",
            "staging/replacement-install/sentinel",
            "trash/replacement-trash/sentinel",
            "models/asr/replacement/model/identity/sentinel",
            "quarantine/replacement-quarantine/sentinel",
        ];
        for relative in replacement_sentinels {
            let path = root.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, b"replacement").unwrap();
        }

        let manager = ModelManager::new_anchored(
            &root,
            root_dir,
            ReqwestTransport::new().unwrap(),
            Catalog::in_memory().unwrap(),
        );
        manager.reconcile_all_anchored().unwrap();

        assert_eq!(fs::read_dir(held.join("downloads")).unwrap().count(), 0);
        assert_eq!(fs::read_dir(held.join("staging")).unwrap().count(), 0);
        assert_eq!(fs::read_dir(held.join("trash")).unwrap().count(), 0);
        assert!(!held.join("models/asr/unknown/model/identity").exists());
        assert_eq!(fs::read_dir(held.join("quarantine")).unwrap().count(), 1);
        for relative in replacement_sentinels {
            assert_eq!(fs::read(root.join(relative)).unwrap(), b"replacement");
        }
    }

    #[test]
    fn anchored_reconcile_rejects_symlink_without_touching_target() {
        let root = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        fs::create_dir(root.path().join("staging")).unwrap();
        fs::write(outside.path().join("sentinel"), b"outside").unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join("staging/link")).unwrap();
        let root_dir = File::open(root.path()).unwrap();
        let manager = ModelManager::new_anchored(
            root.path(),
            root_dir,
            ReqwestTransport::new().unwrap(),
            Catalog::in_memory().unwrap(),
        );

        let error = manager.reconcile_all_anchored().unwrap_err();

        assert_eq!(error.code(), "model_structural_incompatible");
        assert_eq!(
            fs::read(outside.path().join("sentinel")).unwrap(),
            b"outside"
        );
        assert!(
            root.path()
                .join("staging/link")
                .symlink_metadata()
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }

    #[test]
    fn anchored_cleanup_does_not_delete_same_name_replacement() {
        let root = TempDir::new().unwrap();
        let staging = root.path().join("staging");
        let original = staging.join("stale");
        let held = staging.join("held-stale");
        fs::create_dir_all(&original).unwrap();
        fs::write(original.join("original"), b"original").unwrap();
        let original_for_hook = original.clone();
        let held_for_hook = held.clone();
        BEFORE_REMOVE_RENAME.with(|hook| {
            *hook.borrow_mut() = Some(Box::new(move || {
                fs::rename(&original_for_hook, &held_for_hook).unwrap();
                fs::create_dir(&original_for_hook).unwrap();
                fs::write(original_for_hook.join("replacement"), b"replacement").unwrap();
            }));
        });
        let manager = ModelManager::new_anchored(
            root.path(),
            File::open(root.path()).unwrap(),
            ReqwestTransport::new().unwrap(),
            Catalog::in_memory().unwrap(),
        );

        assert!(manager.reconcile_all_anchored().is_err());
        assert_eq!(
            fs::read(original.join("replacement")).unwrap(),
            b"replacement"
        );
        assert_eq!(fs::read(held.join("original")).unwrap(), b"original");
    }

    #[test]
    fn anchored_recovery_error_mapping_matches_legacy_contract() {
        assert_eq!(
            recovery_code(&ManagerError::new(
                "model_runtime_identity_mismatch",
                "runtime mismatch"
            )),
            "model_runtime_identity_mismatch"
        );
        assert_eq!(
            recovery_code(&ManagerError::integrity("bad inventory")),
            "model_runtime_qualification_recovery_required"
        );
    }
}
