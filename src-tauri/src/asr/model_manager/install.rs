use super::fs_support::*;
use super::install_support::*;
use super::*;

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(super) struct StructuralMarker {
    schema: String,
    model_id: String,
    manifest_version: String,
    bundle_identity: String,
    qualification_policy: QualificationPolicy,
    runtime_identity_json: Option<String>,
    compatibility_contract: String,
    install_contract: InstallContract,
    artifacts: Vec<MarkerArtifact>,
    pub(super) installed_files: Vec<MarkerInstalledFile>,
}
#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
struct MarkerArtifact {
    artifact_id: String,
    source_repository: String,
    source_model: String,
    revision: String,
    sha256: String,
    required_path: String,
    license_spdx: String,
    provenance: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(super) struct MarkerInstalledFile {
    pub(super) relative_path: String,
    pub(super) bytes: u64,
    pub(super) sha256: String,
}
impl StructuralMarker {
    fn from_plan(p: &ModelInstallPlan, r: Option<String>, files: Vec<MarkerInstalledFile>) -> Self {
        Self {
            schema: "lifesub.model-structural.v1".to_owned(),
            model_id: p.model_id.clone(),
            manifest_version: p.manifest_version.clone(),
            bundle_identity: p.bundle_identity.clone(),
            qualification_policy: p.qualification_policy,
            runtime_identity_json: r,
            compatibility_contract: match p.qualification_policy {
                QualificationPolicy::StructuralWithPinnedRuntime => "pinned_sherpa_structural",
                QualificationPolicy::RuntimeSmokeRequired => {
                    "original_thinker_config_plus_hf_tokenizer"
                }
            }
            .to_owned(),
            install_contract: p.install_contract.clone(),
            artifacts: p
                .artifacts
                .iter()
                .map(|a| MarkerArtifact {
                    artifact_id: a.artifact_id.clone(),
                    source_repository: a.source_repository.clone(),
                    source_model: a.source_model.clone(),
                    revision: a.revision.clone(),
                    sha256: a.expected_sha256.clone(),
                    required_path: a.required_path.clone(),
                    license_spdx: a.license_spdx.clone(),
                    provenance: a.provenance.clone(),
                })
                .collect(),
            installed_files: files,
        }
    }
}
pub(super) fn matches_marker(m: &StructuralMarker, p: &ModelInstallPlan, r: Option<&str>) -> bool {
    m.schema == "lifesub.model-structural.v1"
        && m.model_id == p.model_id
        && m.manifest_version == p.manifest_version
        && m.bundle_identity == p.bundle_identity
        && m.qualification_policy == p.qualification_policy
        && m.runtime_identity_json.as_deref() == r
        && m.compatibility_contract
            == match p.qualification_policy {
                QualificationPolicy::StructuralWithPinnedRuntime => "pinned_sherpa_structural",
                QualificationPolicy::RuntimeSmokeRequired => {
                    "original_thinker_config_plus_hf_tokenizer"
                }
            }
        && m.install_contract == p.install_contract
        && m.artifacts
            == p.artifacts
                .iter()
                .map(|a| MarkerArtifact {
                    artifact_id: a.artifact_id.clone(),
                    source_repository: a.source_repository.clone(),
                    source_model: a.source_model.clone(),
                    revision: a.revision.clone(),
                    sha256: a.expected_sha256.clone(),
                    required_path: a.required_path.clone(),
                    license_spdx: a.license_spdx.clone(),
                    provenance: a.provenance.clone(),
                })
                .collect::<Vec<_>>()
}

impl<T: HttpTransport, C: ModelCatalog> ModelManager<T, C> {
    pub(crate) fn download_and_install<F: Fn() -> bool>(
        &self,
        p: &ModelInstallPlan,
        d: &DeviceProfile,
        c: F,
    ) -> Result<StoredInstallation, ManagerError> {
        self.ensure_runtime_current()?;
        let id = self.download_only(p, d, &c)?;
        if c() {
            self.catalog
                .set_download_state(&id, "cancelled", Some("model_download_cancelled"))?;
            return Err(ManagerError::new(
                "model_download_cancelled",
                "download cancelled",
            ));
        }
        self.complete_install(p, &id)
    }
    pub(crate) fn retry_install(
        &self,
        p: &ModelInstallPlan,
        id: &str,
    ) -> Result<StoredInstallation, ManagerError> {
        self.ensure_runtime_current()?;
        validate_component("download_id", id)?;
        validate_plan(p)?;
        self.complete_install(p, id)
    }
    pub fn retry_model_install(
        &self,
        m: &str,
        id: &str,
    ) -> Result<StoredInstallation, ManagerError> {
        self.ensure_runtime_current()?;
        validate_component("model_id", m)?;
        self.retry_install(&resolve_current_plan(m)?, id)
    }
    fn complete_install(
        &self,
        p: &ModelInstallPlan,
        id: &str,
    ) -> Result<StoredInstallation, ManagerError> {
        self.catalog.set_download_state(id, "installing", None)?;
        if let Err(e) = self.preflight_disk(p, Some(id)) {
            self.catalog
                .set_download_state(id, "failed", Some(e.code()))?;
            return Err(e);
        }
        let i = match self.install_verified_download(p, id) {
            Ok(v) => v,
            Err(e) => {
                if self.storage.real_directory(&self.install_dir(p))? {
                    return self.recover_post_rename(p, id, e);
                }
                self.catalog
                    .set_download_state(id, "failed", Some(e.code()))?;
                return Err(e);
            }
        };
        if let Err(e) = self.catalog.publish_installation(&i) {
            return self.recover_post_rename(p, id, e);
        }
        if let Err(e) = self.catalog.set_download_state(id, "succeeded", None) {
            return self.recover_post_rename(p, id, e);
        }
        Ok(i)
    }
    fn recover_post_rename(
        &self,
        p: &ModelInstallPlan,
        id: &str,
        original: ManagerError,
    ) -> Result<StoredInstallation, ManagerError> {
        match self.reconcile_installation_outcome(p) {
            Ok(types::ReconcileOutcome::Recovered(i)) => {
                if let Err(e) = self.catalog.set_download_state(id, "succeeded", None) {
                    self.catalog
                        .set_download_state(id, "failed", Some("recovery_required"))?;
                    Err(e)
                } else {
                    Ok(i)
                }
            }
            Ok(types::ReconcileOutcome::RejectedDurably(e)) => {
                self.catalog
                    .set_download_state(id, "failed", Some("recovery_required"))?;
                Err(e)
            }
            Err(_) => {
                self.catalog
                    .set_download_state(id, "failed", Some("recovery_required"))?;
                Err(original)
            }
        }
    }
    pub(crate) fn install_verified_download(
        &self,
        p: &ModelInstallPlan,
        id: &str,
    ) -> Result<StoredInstallation, ManagerError> {
        validate_component("download_id", id)?;
        validate_plan(p)?;
        if let Some(fs) = self.storage.anchored_fs() {
            return self.install_verified_anchored(fs.as_ref(), p, id);
        }
        for a in &p.artifacts {
            let path = self
                .download_dir(id)
                .join(format!("{}.part", a.artifact_id));
            let cp = self
                .catalog
                .checkpoint(id, &a.artifact_id)?
                .ok_or_else(|| ManagerError::integrity("verified checkpoint missing"))?;
            if cp.source_identity != source_identity(a)
                || cp.expected_bytes != a.expected_bytes
                || cp.downloaded_bytes != a.expected_bytes
                || cp.temp_path != path
                || cp.verified_sha256.as_deref() != Some(&a.expected_sha256)
                || cp.state != "verified"
            {
                return Err(ManagerError::integrity(
                    "verified artifact source identity mismatch",
                ));
            }
            let m = fs::symlink_metadata(&path)?;
            if !m.file_type().is_file()
                || m.len() != a.expected_bytes
                || sha256_file(&path)? != a.expected_sha256
            {
                return Err(ManagerError::integrity(
                    "verified artifact changed before install",
                ));
            }
        }
        let staging = self
            .storage
            .nominal_root()
            .join("staging")
            .join(format!("{}-{}-{id}", p.model_id, p.manifest_version));
        if staging.exists() {
            fs::remove_dir_all(&staging)?;
        }
        fs::create_dir_all(&staging)?;
        #[cfg(test)]
        if self.install_fault == Some(InstallFault::Assembly) {
            fs::remove_dir_all(&staging)?;
            return Err(ManagerError::new(
                "model_install_failed",
                "injected assembly failure",
            ));
        }
        match &p.install_contract {
            InstallContract::Direct { .. } => {
                for a in &p.artifacts {
                    let src = self
                        .download_dir(id)
                        .join(format!("{}.part", a.artifact_id));
                    let dst = staging.join(safe_relative_path(&a.required_path)?);
                    create_parents(&staging, &dst)?;
                    copy_new(&src, &dst)?;
                }
            }
            InstallContract::Archive { .. } => {
                extract_tar_bz2_safely(
                    &self
                        .download_dir(id)
                        .join(format!("{}.part", p.artifacts[0].artifact_id)),
                    &staging,
                    &p.install_contract,
                )?;
            }
        }
        let files = installed_records(&staging)?;
        validate_inventory(p, &files)?;
        if let Err(e) = validate_structure(p, &staging, self.observed_sherpa_runtime.as_ref()) {
            if e.code() == "model_runtime_identity_mismatch" {
                self.quarantine(&staging, p, "runtime-identity-mismatch")?;
            }
            return Err(e);
        }
        let runtime = p
            .sherpa_runtime
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| ManagerError::structural(e.to_string()))?;
        write_marker(
            &staging,
            &StructuralMarker::from_plan(p, runtime.clone(), files),
        )?;
        sync_tree(&staging)?;
        sync_dir(&staging)?;
        let final_dir = self.install_dir(p);
        fs::create_dir_all(final_dir.parent().unwrap())?;
        if final_dir.exists() {
            return Err(ManagerError::new(
                "model_install_conflict",
                "immutable install directory already exists",
            ));
        }
        #[cfg(test)]
        if self.install_fault == Some(InstallFault::Rename) {
            fs::remove_dir_all(&staging)?;
            return Err(ManagerError::new(
                "model_install_failed",
                "injected rename failure",
            ));
        }
        fs::rename(&staging, &final_dir)?;
        sync_dir(final_dir.parent().unwrap())?;
        Ok(StoredInstallation {
            model_id: p.model_id.clone(),
            provider: p.provider.clone(),
            manifest_version: p.manifest_version.clone(),
            bundle_identity: p.bundle_identity.clone(),
            install_dir: final_dir,
            state: match p.qualification_policy {
                QualificationPolicy::StructuralWithPinnedRuntime => "runtime_qualified",
                QualificationPolicy::RuntimeSmokeRequired => "installed_unqualified",
            }
            .to_owned(),
            runtime_identity_json: runtime,
        })
    }

    fn install_verified_anchored(
        &self,
        fs: &anchored_reconcile::AnchoredFs,
        p: &ModelInstallPlan,
        id: &str,
    ) -> Result<StoredInstallation, ManagerError> {
        let mut parts = Vec::with_capacity(p.artifacts.len());
        for artifact in &p.artifacts {
            let nominal = self
                .download_dir(id)
                .join(format!("{}.part", artifact.artifact_id));
            let checkpoint = self
                .catalog
                .checkpoint(id, &artifact.artifact_id)?
                .ok_or_else(|| ManagerError::integrity("verified checkpoint missing"))?;
            if checkpoint.source_identity != source_identity(artifact)
                || checkpoint.expected_bytes != artifact.expected_bytes
                || checkpoint.downloaded_bytes != artifact.expected_bytes
                || checkpoint.temp_path != nominal
                || checkpoint.verified_sha256.as_deref() != Some(&artifact.expected_sha256)
                || checkpoint.state != "verified"
            {
                return Err(ManagerError::integrity(
                    "verified artifact source identity mismatch",
                ));
            }
            parts.push(self.storage.hold_verified_download_part(
                &nominal,
                artifact.expected_bytes,
                &artifact.expected_sha256,
            )?);
        }
        let staging =
            PathBuf::from("staging").join(format!("{}-{}-{id}", p.model_id, p.manifest_version));
        if fs.open_dir(&staging)?.is_some() {
            fs.remove_tree(&staging)?;
        }
        fs.ensure_dir(&staging)?;
        let staging_identity = fs.entry_stat(&staging)?;
        let (staging_claim, staging_fs, staging_identity) =
            fs.claim_directory(&staging, staging_identity)?;
        #[cfg(test)]
        if self.install_fault == Some(InstallFault::Assembly) {
            fs.remove_tree_if_identity(&staging_claim, staging_identity)?;
            return Err(ManagerError::new(
                "model_install_failed",
                "injected assembly failure",
            ));
        }
        let result = (|| {
            match &p.install_contract {
                InstallContract::Direct { .. } => {
                    for (artifact, part) in p.artifacts.iter().zip(parts.iter()) {
                        let destination = safe_relative_path(&artifact.required_path)?;
                        staging_fs
                            .ensure_dir(destination.parent().unwrap_or_else(|| Path::new("")))?;
                        let mut output = staging_fs.open_regular_create_new(&destination)?;
                        part.copy_to(&mut output)?;
                    }
                }
                InstallContract::Archive { .. } => {
                    let archive = parts.first().ok_or_else(|| {
                        ManagerError::integrity("verified archive artifact missing")
                    })?;
                    archive.extract_tar_bz2_to(&staging_fs, Path::new(""), &p.install_contract)?;
                }
            }
            let files = anchored_reconcile::installed_records_anchored(&staging_fs, Path::new(""))?;
            validate_inventory(p, &files)?;
            match p.qualification_policy {
                QualificationPolicy::StructuralWithPinnedRuntime => {
                    if !matches!(
                        (p.sherpa_runtime.as_ref(), self.observed_sherpa_runtime.as_ref()),
                        (Some(expected), Some(actual)) if expected.matches(actual)
                    ) {
                        return Err(ManagerError::new(
                            "model_runtime_identity_mismatch",
                            "pinned sherpa identity missing",
                        ));
                    }
                }
                QualificationPolicy::RuntimeSmokeRequired => {
                    anchored_reconcile::validate_qwen_anchored(&staging_fs, Path::new(""))?;
                }
            }
            let runtime = p
                .sherpa_runtime
                .as_ref()
                .map(serde_json::to_string)
                .transpose()
                .map_err(|error| ManagerError::structural(error.to_string()))?;
            let marker = StructuralMarker::from_plan(p, runtime.clone(), files);
            let marker_bytes = serde_json::to_vec_pretty(&marker)
                .map_err(|error| ManagerError::structural(error.to_string()))?;
            let marker_tmp = PathBuf::from(format!("{STRUCTURAL_MARKER}.tmp"));
            staging_fs.write_new_synced(&marker_tmp, &marker_bytes)?;
            staging_fs.rename_noreplace(&marker_tmp, Path::new(STRUCTURAL_MARKER))?;
            staging_fs.sync_dir(Path::new(""))?;
            let final_relative = self.storage.relative_install_path(&self.install_dir(p))?;
            fs.ensure_dir(final_relative.parent().unwrap_or_else(|| Path::new("")))?;
            #[cfg(test)]
            if self.install_fault == Some(InstallFault::Rename) {
                return Err(ManagerError::new(
                    "model_install_failed",
                    "injected rename failure",
                ));
            }
            if fs.open_dir(&final_relative)?.is_some() {
                return Err(ManagerError::new(
                    "model_install_conflict",
                    "immutable install directory already exists",
                ));
            }
            fs.publish_directory_noreplace(&staging_claim, staging_identity, &final_relative)?;
            Ok(StoredInstallation {
                model_id: p.model_id.clone(),
                provider: p.provider.clone(),
                manifest_version: p.manifest_version.clone(),
                bundle_identity: p.bundle_identity.clone(),
                install_dir: self.install_dir(p),
                state: match p.qualification_policy {
                    QualificationPolicy::StructuralWithPinnedRuntime => "runtime_qualified",
                    QualificationPolicy::RuntimeSmokeRequired => "installed_unqualified",
                }
                .to_owned(),
                runtime_identity_json: runtime,
            })
        })();
        if result.is_err()
            && fs
                .entry_stat(&staging_claim)
                .is_ok_and(|actual| actual.same_object(staging_identity))
            && !result
                .as_ref()
                .is_err_and(|error| error.code() == "model_install_conflict")
        {
            if result
                .as_ref()
                .is_err_and(|error| error.code() == "model_runtime_identity_mismatch")
            {
                fs.ensure_dir(Path::new("quarantine"))?;
                fs.rename(
                    &staging_claim,
                    &PathBuf::from("quarantine").join(format!(
                        "{}-{}-runtime-identity-mismatch-{}",
                        p.model_id,
                        p.manifest_version,
                        uuid::Uuid::new_v4().simple()
                    )),
                )?;
                fs.sync_dir(Path::new("quarantine"))?;
            } else {
                fs.remove_tree_if_identity(&staging_claim, staging_identity)?;
            }
        }
        result
    }
    pub(super) fn preflight_disk(
        &self,
        p: &ModelInstallPlan,
        id: Option<&str>,
    ) -> Result<(), ManagerError> {
        let required = self.required_additional_free(p, id)?;
        #[cfg(test)]
        let available = if let Some(s) = &self.available_space_sequence {
            let mut s = s.lock().unwrap();
            if s.is_empty() {
                return Err(ManagerError::new(
                    "insufficient_disk_space",
                    "available-space test sequence exhausted",
                ));
            }
            s.remove(0)
        } else {
            self.available_space_override
                .unwrap_or(self.storage.available_space()?)
        };
        #[cfg(not(test))]
        let available = self.storage.available_space()?;
        if available < required {
            Err(ManagerError::new(
                "insufficient_disk_space",
                format!("requires {required} bytes, only {available} available"),
            ))
        } else {
            Ok(())
        }
    }
    pub(super) fn required_additional_free(
        &self,
        p: &ModelInstallPlan,
        id: Option<&str>,
    ) -> Result<u64, ManagerError> {
        let mut remaining = 0u64;
        for a in &p.artifacts {
            let cp = id
                .map(|id| self.catalog.checkpoint(id, &a.artifact_id))
                .transpose()?
                .flatten();
            let file_bytes = cp
                .as_ref()
                .map(|c| self.storage.checkpoint_bytes(&c.temp_path))
                .transpose()?
                .unwrap_or(0);
            let source = cp.as_ref().is_some_and(|c| {
                c.source_identity == source_identity(a) && c.expected_bytes == a.expected_bytes
            });
            let reusable = cp.as_ref().is_some_and(|c| {
                source
                    && c.downloaded_bytes == a.expected_bytes
                    && c.verified_sha256.as_deref() == Some(&a.expected_sha256)
                    && file_bytes == a.expected_bytes
            });
            if !reusable {
                let existing = cp
                    .filter(|c| source && file_bytes == c.downloaded_bytes)
                    .map(|c| c.downloaded_bytes)
                    .unwrap_or(0)
                    .min(a.expected_bytes);
                remaining = remaining
                    .checked_add(a.expected_bytes - existing)
                    .ok_or_else(|| {
                        ManagerError::new("insufficient_disk_space", "disk calculation overflow")
                    })?;
            }
        }
        self.storage.ensure_assembly_roots()?;
        checked_required_additional_free(
            remaining,
            p.install_contract.max_total_written_bytes(),
            DISK_SAFETY_MARGIN_BYTES,
        )
        .ok_or_else(|| ManagerError::new("insufficient_disk_space", "disk calculation overflow"))
    }
    pub(super) fn install_dir(&self, p: &ModelInstallPlan) -> PathBuf {
        self.storage
            .nominal_root()
            .join("models/asr")
            .join(&p.provider)
            .join(&p.model_id)
            .join(format!("{}-{}", p.manifest_version, p.bundle_identity))
    }
    pub(super) fn quarantine(
        &self,
        source: &Path,
        p: &ModelInstallPlan,
        reason: &str,
    ) -> Result<(), ManagerError> {
        let q = self.storage.nominal_root().join("quarantine");
        fs::create_dir_all(&q)?;
        fs::rename(
            source,
            q.join(format!(
                "{}-{}-{reason}-{}",
                p.model_id,
                p.manifest_version,
                uuid::Uuid::new_v4().simple()
            )),
        )?;
        sync_dir(&q)
    }
}
