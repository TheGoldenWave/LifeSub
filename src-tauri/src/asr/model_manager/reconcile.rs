use super::delete::cleanup_markers;
use super::fs_support::*;
use super::install::{StructuralMarker, matches_marker};
use super::install_support::{installed_records, validate_inventory};
use super::types::ReconcileOutcome;
use super::*;

impl<T: HttpTransport, C: ModelCatalog> ModelManager<T, C> {
    #[cfg(test)]
    pub(crate) fn reconcile_installation(
        &self,
        p: &ModelInstallPlan,
    ) -> Result<StoredInstallation, ManagerError> {
        match self.reconcile_installation_outcome(p)? {
            ReconcileOutcome::Recovered(i) => Ok(i),
            ReconcileOutcome::RejectedDurably(e) => Err(e),
        }
    }
    pub(super) fn reconcile_installation_outcome(
        &self,
        p: &ModelInstallPlan,
    ) -> Result<ReconcileOutcome, ManagerError> {
        let dir = self.install_dir(p);
        if !real_dir(&dir)? {
            self.catalog
                .record_installation_recovery(&p.model_id, "model_integrity_failed")?;
            return Ok(ReconcileOutcome::RejectedDurably(ManagerError::new(
                "model_install_missing",
                "install directory is missing",
            )));
        }
        let validation = (|| {
            let marker: StructuralMarker =
                serde_json::from_slice(&read_regular(&dir.join(STRUCTURAL_MARKER))?)
                    .map_err(|_| ManagerError::structural("invalid structural marker"))?;
            let runtime = p
                .sherpa_runtime
                .as_ref()
                .map(serde_json::to_string)
                .transpose()
                .map_err(|e| ManagerError::structural(e.to_string()))?;
            if !matches_marker(&marker, p, runtime.as_deref()) {
                return Err(ManagerError::structural("structural marker mismatch"));
            }
            let actual = installed_records(&dir)?;
            if marker.installed_files != actual {
                return Err(ManagerError::integrity(
                    "installed file inventory or hash mismatch",
                ));
            }
            validate_inventory(p, &marker.installed_files)?;
            super::install_support::validate_structure_for_reconcile(
                p,
                &dir,
                self.observed_sherpa_runtime.as_ref(),
            )?;
            Ok(runtime)
        })();
        let runtime = match validation {
            Ok(v) => v,
            Err(e) => {
                self.quarantine(&dir, p, "reconcile-rejected")?;
                let code = if e.code() == "model_runtime_identity_mismatch" {
                    "model_runtime_identity_mismatch"
                } else {
                    "model_runtime_qualification_recovery_required"
                };
                self.catalog
                    .record_installation_recovery(&p.model_id, code)?;
                return Ok(ReconcileOutcome::RejectedDurably(e));
            }
        };
        let i = StoredInstallation {
            model_id: p.model_id.clone(),
            provider: p.provider.clone(),
            manifest_version: p.manifest_version.clone(),
            bundle_identity: p.bundle_identity.clone(),
            install_dir: dir,
            state: match p.qualification_policy {
                QualificationPolicy::StructuralWithPinnedRuntime => "runtime_qualified",
                QualificationPolicy::RuntimeSmokeRequired => "installed_unqualified",
            }
            .to_owned(),
            runtime_identity_json: runtime,
        };
        self.catalog.publish_installation(&i)?;
        Ok(ReconcileOutcome::Recovered(i))
    }
}

impl<T: HttpTransport> ModelManager<T, Catalog> {
    pub fn into_catalog(self) -> Catalog {
        self.catalog
    }
    pub fn reconcile_all(&self) -> Result<(), ManagerError> {
        self.reconcile_trash()?;
        self.reconcile_downloads()?;
        self.reconcile_staging()?;
        self.reconcile_installations()?;
        self.reconcile_unrecorded()
    }
    fn reconcile_downloads(&self) -> Result<(), ManagerError> {
        let records = self.catalog.model_download_records()?;
        let known = records
            .iter()
            .map(|r| r.id.as_str())
            .collect::<HashSet<_>>();
        for r in &records {
            if validate_component("download_id", &r.id).is_err() {
                self.catalog.mark_download_recovery_required(&r.id)?;
                continue;
            }
            let p = match ModelInstallPlan::resolve(
                &r.model_id,
                &r.manifest_version,
                &r.bundle_identity,
            ) {
                Ok(p) => p,
                Err(_) => {
                    self.catalog.mark_download_recovery_required(&r.id)?;
                    continue;
                }
            };
            let ids = self
                .catalog
                .model_download_artifact_ids(&r.id)?
                .into_iter()
                .collect::<HashSet<_>>();
            for a in &p.artifacts {
                let Some(cp) = self.catalog.checkpoint(&r.id, &a.artifact_id)? else {
                    continue;
                };
                let path = self
                    .download_dir(&r.id)
                    .join(format!("{}.part", a.artifact_id));
                let source = cp.source_identity == source_identity(a)
                    && cp.expected_bytes == a.expected_bytes
                    && cp.temp_path == path;
                if !source {
                    if path.exists() {
                        fs::remove_file(&path)?;
                    }
                    self.catalog.save_checkpoint(
                        &r.id,
                        &ArtifactCheckpoint {
                            artifact_id: a.artifact_id.clone(),
                            source_identity: source_identity(a),
                            downloaded_bytes: 0,
                            expected_bytes: a.expected_bytes,
                            temp_path: path,
                            etag: None,
                            last_modified: None,
                            verified_sha256: None,
                            state: "pending".to_owned(),
                        },
                    )?;
                    continue;
                }
                let file = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                let durable = cp.downloaded_bytes.min(file);
                if file > durable {
                    let f = OpenOptions::new().write(true).open(&path)?;
                    f.set_len(durable)?;
                    f.sync_all()?;
                }
                let verified = durable == a.expected_bytes
                    && cp.verified_sha256.as_deref() == Some(&a.expected_sha256)
                    && sha256_file(&path)? == a.expected_sha256;
                self.catalog.save_checkpoint(
                    &r.id,
                    &ArtifactCheckpoint {
                        artifact_id: a.artifact_id.clone(),
                        source_identity: cp.source_identity,
                        downloaded_bytes: durable,
                        expected_bytes: a.expected_bytes,
                        temp_path: path,
                        etag: cp.etag,
                        last_modified: cp.last_modified,
                        verified_sha256: verified.then(|| a.expected_sha256.clone()),
                        state: if verified { "verified" } else { "downloading" }.to_owned(),
                    },
                )?;
            }
            let dir = self.download_dir(&r.id);
            if real_dir(&dir)? {
                for e in fs::read_dir(&dir)? {
                    let p = e?.path();
                    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if let Some(id) = name.strip_suffix(".part")
                        && !ids.contains(id)
                    {
                        fs::remove_file(p)?;
                    }
                }
                sync_dir(&dir)?;
            }
            if matches!(
                r.state.as_str(),
                "queued" | "downloading" | "verifying" | "installing"
            ) {
                self.catalog.mark_download_recovery_required(&r.id)?;
            }
        }
        let root = self.root.join("downloads");
        if real_dir(&root)? {
            for e in fs::read_dir(&root)? {
                let p = e?.path();
                let m = fs::symlink_metadata(&p)?;
                if !m.file_type().is_dir() {
                    return Err(ManagerError::structural(
                        "downloads root contains a non-directory entry",
                    ));
                }
                let n = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !known.contains(n) {
                    fs::remove_dir_all(p)?;
                }
            }
            sync_dir(&root)?;
        }
        Ok(())
    }
    fn reconcile_staging(&self) -> Result<(), ManagerError> {
        let root = self.root.join("staging");
        if !real_dir(&root)? {
            return Ok(());
        }
        for e in fs::read_dir(&root)? {
            let p = e?.path();
            let m = fs::symlink_metadata(&p)?;
            if m.file_type().is_dir() {
                fs::remove_dir_all(p)?;
            } else if m.file_type().is_file() {
                fs::remove_file(p)?;
            } else {
                return Err(ManagerError::structural(
                    "staging root contains a link or special entry",
                ));
            }
        }
        sync_dir(&root)
    }
    fn reconcile_installations(&self) -> Result<(), ManagerError> {
        for r in self.catalog.model_installation_records()? {
            if r.state == "deleting" {
                self.reconcile_deleting(&r)?;
                continue;
            }
            let p = match ModelInstallPlan::resolve(
                &r.model_id,
                &r.manifest_version,
                &r.bundle_identity,
            ) {
                Ok(v) => v,
                Err(_) => {
                    self.catalog.record_installation_recovery(
                        &r.model_id,
                        "model_manifest_identity_mismatch",
                    )?;
                    continue;
                }
            };
            if r.install_dir != self.install_dir(&p) {
                self.catalog
                    .record_installation_recovery(&r.model_id, "model_integrity_failed")?;
                continue;
            }
            match self.reconcile_installation_outcome(&p)? {
                ReconcileOutcome::Recovered(_) | ReconcileOutcome::RejectedDurably(_) => {}
            }
        }
        Ok(())
    }
    fn reconcile_deleting(&self, r: &ModelInstallationRecord) -> Result<(), ManagerError> {
        if real_dir(&r.install_dir)? {
            let marker = r.install_dir.join(DELETE_MARKER);
            if regular_file(&marker)? {
                let lease: DeletionLease = serde_json::from_slice(&read_regular(&marker)?)
                    .map_err(|_| ManagerError::integrity("invalid deletion marker"))?;
                if lease != lease_from_record(r) {
                    return Err(ManagerError::integrity(
                        "deletion marker does not match current Catalog lease",
                    ));
                }
                let trash_root = self.root.join("trash");
                fs::create_dir_all(&trash_root)?;
                let trash = trash_root.join(format!(
                    "{}-recovery-{}",
                    r.model_id,
                    uuid::Uuid::new_v4().simple()
                ));
                fs::rename(&r.install_dir, &trash)?;
                sync_dir(r.install_dir.parent().unwrap_or(&self.root))?;
                sync_dir(&trash_root)?;
                self.catalog.complete_deletion_recovery(&lease)?;
                fs::remove_dir_all(trash)?;
                sync_dir(&trash_root)?;
            } else {
                cleanup_markers(
                    &r.install_dir,
                    &r.install_dir.join(format!("{DELETE_MARKER}.tmp")),
                    &marker,
                )?;
                self.catalog.abort_delete(&lease_from_record(r))?;
            }
        } else {
            self.catalog
                .complete_deletion_recovery(&lease_from_record(r))?;
        }
        Ok(())
    }
    fn reconcile_trash(&self) -> Result<(), ManagerError> {
        let root = self.root.join("trash");
        if !real_dir(&root)? {
            return Ok(());
        }
        for e in fs::read_dir(&root)? {
            let p = e?.path();
            let m = fs::symlink_metadata(&p)?;
            if m.file_type().is_file() {
                fs::remove_file(p)?;
                continue;
            }
            if !m.file_type().is_dir() {
                return Err(ManagerError::structural(
                    "trash contains a link or special entry",
                ));
            }
            let marker = p.join(DELETE_MARKER);
            if regular_file(&marker)? {
                let lease: DeletionLease = serde_json::from_slice(&read_regular(&marker)?)
                    .map_err(|_| ManagerError::integrity("invalid deletion marker"))?;
                if self.catalog.model_deletion_lease(&lease.model_id)? != Some(lease.clone()) {
                    return Err(ManagerError::integrity(
                        "trash deletion marker does not match current Catalog lease",
                    ));
                }
                self.catalog.complete_deletion_recovery(&lease)?;
            }
            fs::remove_dir_all(p)?;
        }
        sync_dir(&root)
    }
    fn reconcile_unrecorded(&self) -> Result<(), ManagerError> {
        let root = self.root.join("models/asr");
        match fs::symlink_metadata(&root) {
            Ok(m) if m.file_type().is_dir() => {}
            Ok(_) => {
                return Err(ManagerError::structural(
                    "models root is not a real directory",
                ));
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.into()),
        }
        let recorded = self
            .catalog
            .model_installation_records()?
            .into_iter()
            .map(|r| r.install_dir)
            .collect::<HashSet<_>>();
        for provider in child_dirs(&root)? {
            for model in child_dirs(&provider)? {
                let id = model.file_name().and_then(|n| n.to_str()).unwrap_or("");
                for dir in child_dirs(&model)? {
                    if recorded.contains(&dir) {
                        continue;
                    }
                    let Some(name) = dir.file_name().and_then(|n| n.to_str()) else {
                        continue;
                    };
                    let Some(p) = model_registry()
                        .model(id)
                        .map(ModelInstallPlan::from_manifest)
                        .or_else(|| {
                            (vad_manifest().id == id)
                                .then(|| ModelInstallPlan::from_vad_manifest(vad_manifest()))
                        })
                    else {
                        self.quarantine_unknown(&dir, "unknown-model")?;
                        continue;
                    };
                    if name != format!("{}-{}", p.manifest_version, p.bundle_identity) {
                        self.quarantine_unknown(&dir, "identity-mismatch")?;
                        continue;
                    }
                    match self.reconcile_installation_outcome(&p)? {
                        ReconcileOutcome::Recovered(_) | ReconcileOutcome::RejectedDurably(_) => {}
                    }
                }
            }
        }
        Ok(())
    }
    fn quarantine_unknown(&self, src: &Path, reason: &str) -> Result<(), ManagerError> {
        let q = self.root.join("quarantine");
        fs::create_dir_all(&q)?;
        fs::rename(
            src,
            q.join(format!("{reason}-{}", uuid::Uuid::new_v4().simple())),
        )?;
        sync_dir(&q)
    }
}
fn lease_from_record(r: &ModelInstallationRecord) -> DeletionLease {
    DeletionLease {
        model_id: r.model_id.clone(),
        install_dir: r.install_dir.clone(),
        prior_state: if r.qualified_at.is_some() {
            "runtime_qualified"
        } else {
            "installed_unqualified"
        }
        .to_owned(),
        prior_runtime_identity_json: r.runtime_identity_json.clone(),
        prior_qualified_at: r.qualified_at.clone(),
        prior_last_error_code: r.last_error_code.clone(),
    }
}
