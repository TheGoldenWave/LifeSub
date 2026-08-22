use super::*;

pub(super) fn provider_name(provider: AsrProviderKind) -> &'static str {
    match provider {
        AsrProviderKind::SenseVoice => "sense_voice",
        AsrProviderKind::Whisper => "whisper",
        AsrProviderKind::Qwen3Asr => "qwen3_asr",
    }
}
fn device_requirement(value: ManifestDeviceRequirement) -> DeviceRequirement {
    match value {
        ManifestDeviceRequirement::AnyDesktop => DeviceRequirement::AnyDesktop,
        ManifestDeviceRequirement::AppleSiliconMetal {
            minimum_macos_major,
            minimum_memory_gib,
        } => DeviceRequirement::AppleSiliconMetal {
            minimum_macos_major,
            minimum_memory_gib,
            chip: "M4".to_owned(),
        },
    }
}
fn qualification(value: ManifestQualificationPolicy) -> QualificationPolicy {
    match value {
        ManifestQualificationPolicy::StructuralWithPinnedRuntime => {
            QualificationPolicy::StructuralWithPinnedRuntime
        }
        ManifestQualificationPolicy::RuntimeSmokeRequired => {
            QualificationPolicy::RuntimeSmokeRequired
        }
    }
}
fn runtime(value: RuntimeRequirement) -> Option<FullSherpaRuntimeIdentity> {
    match value {
        RuntimeRequirement::SherpaOnnx {
            crate_version,
            git_commit,
            native_archive_sha256,
            build_id,
            ..
        } => Some(FullSherpaRuntimeIdentity {
            version: crate_version.to_owned(),
            git_commit: git_commit.to_owned(),
            native_archive_sha256: native_archive_sha256.to_owned(),
            build_id: build_id.to_owned(),
        }),
        RuntimeRequirement::QwenCandleMetal { .. } => None,
    }
}
fn artifact(value: &ArtifactFile) -> ArtifactPlan {
    ArtifactPlan {
        artifact_id: value.artifact_id.to_owned(),
        source_repository: value.source_repository.to_owned(),
        source_model: value.source_model.to_owned(),
        url: value.resolved_url.to_owned(),
        revision: value.revision.to_owned(),
        expected_bytes: value.bytes,
        expected_sha256: value.sha256.to_owned(),
        required_path: value.required_path.to_owned(),
        install_mode: match value.install_mode {
            ArtifactInstallMode::Direct => InstallMode::Direct,
            ArtifactInstallMode::ExtractTarBz2 => InstallMode::ExtractTarBz2,
        },
        redirect_hosts: value
            .redirect_hosts
            .iter()
            .map(|v| (*v).to_owned())
            .collect(),
        license_spdx: value.license_spdx.to_owned(),
        provenance: value.provenance.to_owned(),
    }
}
fn contract(bundle: &crate::asr::manifest::ArtifactBundle) -> InstallContract {
    let files = |items: &[crate::asr::manifest::RequiredInstallFile]| {
        items
            .iter()
            .map(|v| RequiredInstalledFile {
                path: v.path.to_owned(),
                bytes: v.bytes,
                sha256: v.sha256.to_owned(),
            })
            .collect()
    };
    match bundle.install_constraints {
        ManifestInstallConstraints::Archive(v) => InstallContract::Archive {
            archive_root: bundle.artifacts[0].required_path.to_owned(),
            max_scanned_entries: v.max_scanned_entries,
            max_written_file_bytes: v.max_written_file_bytes,
            max_total_written_bytes: v.max_total_written_bytes,
            required_files: files(v.required_files),
        },
        ManifestInstallConstraints::Direct(v) => InstallContract::Direct {
            max_written_file_bytes: v.max_written_file_bytes,
            max_total_written_bytes: v.max_total_written_bytes,
            required_files: files(v.required_files),
        },
    }
}
pub(super) fn plan_from_manifest(m: &ModelManifest) -> ModelInstallPlan {
    ModelInstallPlan {
        model_id: m.id.to_owned(),
        provider: provider_name(m.provider).to_owned(),
        manifest_version: m.manifest_version.to_owned(),
        bundle_identity: m.bundle.identity_sha256.to_owned(),
        device: device_requirement(m.device),
        qualification_policy: qualification(m.qualification_policy),
        sherpa_runtime: runtime(m.runtime),
        artifacts: m.bundle.artifacts.iter().map(artifact).collect(),
        install_contract: contract(&m.bundle),
    }
}
pub(super) fn plan_from_vad_manifest(m: &VadManifest) -> ModelInstallPlan {
    ModelInstallPlan {
        model_id: m.id.to_owned(),
        provider: "vad".to_owned(),
        manifest_version: m.manifest_version.to_owned(),
        bundle_identity: m.bundle.identity_sha256.to_owned(),
        device: DeviceRequirement::AnyDesktop,
        qualification_policy: qualification(m.qualification_policy),
        sherpa_runtime: runtime(m.runtime),
        artifacts: m.bundle.artifacts.iter().map(artifact).collect(),
        install_contract: contract(&m.bundle),
    }
}
pub(super) fn resolve_current_plan(id: &str) -> Result<ModelInstallPlan, ManagerError> {
    model_registry()
        .model(id)
        .map(plan_from_manifest)
        .or_else(|| (vad_manifest().id == id).then(|| plan_from_vad_manifest(vad_manifest())))
        .ok_or_else(|| ManagerError::structural("unknown model manifest"))
}

pub(super) fn validate_component(label: &str, value: &str) -> Result<(), ManagerError> {
    if value.is_empty()
        || value.len() > 128
        || !value.is_ascii()
        || matches!(value, "." | "..")
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        Err(ManagerError::structural(format!(
            "invalid {label} path component"
        )))
    } else {
        Ok(())
    }
}
pub(super) fn validate_plan(plan: &ModelInstallPlan) -> Result<(), ManagerError> {
    if plan.artifacts.is_empty() {
        return Err(ManagerError::structural("empty artifact bundle"));
    }
    for (l, v) in [
        ("model_id", plan.model_id.as_str()),
        ("provider", plan.provider.as_str()),
        ("manifest_version", plan.manifest_version.as_str()),
        ("bundle_identity", plan.bundle_identity.as_str()),
    ] {
        validate_component(l, v)?;
    }
    if let Ok(canonical) = ModelInstallPlan::resolve(
        &plan.model_id,
        &plan.manifest_version,
        &plan.bundle_identity,
    ) && &canonical != plan
    {
        return Err(ManagerError::structural(
            "model plan differs from canonical manifest",
        ));
    }
    let mut ids = HashSet::new();
    let mut paths = Vec::<PathBuf>::new();
    for a in &plan.artifacts {
        validate_component("artifact_id", &a.artifact_id)?;
        if !ids.insert(&a.artifact_id) {
            return Err(ManagerError::structural("duplicate artifact ID"));
        }
        if a.expected_sha256.len() != 64 {
            return Err(ManagerError::integrity("invalid artifact hash"));
        }
        let p = safe_relative_path(&a.required_path)?;
        if paths.iter().any(|x| x.starts_with(&p) || p.starts_with(x)) {
            return Err(ManagerError::structural("overlapping required paths"));
        }
        paths.push(p);
    }
    let required = plan.install_contract.required_files();
    if required.is_empty() {
        return Err(ManagerError::structural("empty required install inventory"));
    }
    let mut seen = HashSet::new();
    let mut total = 0u64;
    let mut largest = 0u64;
    for f in required {
        safe_relative_path(&f.path)?;
        if f.bytes == 0
            || f.sha256.len() != 64
            || !f.sha256.bytes().all(|b| b.is_ascii_hexdigit())
            || !seen.insert(&f.path)
        {
            return Err(ManagerError::structural("invalid required install file"));
        }
        total = total
            .checked_add(f.bytes)
            .ok_or_else(|| ManagerError::structural("install inventory overflow"))?;
        largest = largest.max(f.bytes);
    }
    if total != plan.install_contract.max_total_written_bytes()
        || largest != plan.install_contract.max_written_file_bytes()
    {
        return Err(ManagerError::structural(
            "install constraint totals mismatch",
        ));
    }
    match &plan.install_contract {
        InstallContract::Archive {
            archive_root,
            max_scanned_entries,
            ..
        } => {
            safe_relative_path(archive_root)?;
            if *max_scanned_entries < required.len() as u64
                || plan.artifacts.len() != 1
                || plan.artifacts[0].install_mode != InstallMode::ExtractTarBz2
                || plan.artifacts[0].required_path != *archive_root
            {
                return Err(ManagerError::structural(
                    "archive install contract mismatch",
                ));
            }
        }
        InstallContract::Direct { .. } => {
            if plan
                .artifacts
                .iter()
                .any(|a| a.install_mode != InstallMode::Direct)
                || plan.artifacts.len() != required.len()
                || plan.artifacts.iter().any(|a| {
                    !required.iter().any(|f| {
                        (f.path.as_str(), f.bytes, f.sha256.as_str())
                            == (
                                a.required_path.as_str(),
                                a.expected_bytes,
                                a.expected_sha256.as_str(),
                            )
                    })
                })
            {
                return Err(ManagerError::structural("direct install contract mismatch"));
            }
        }
    }
    Ok(())
}
pub(super) fn validate_device(
    r: &DeviceRequirement,
    d: &DeviceProfile,
) -> Result<(), ManagerError> {
    match r {
        DeviceRequirement::AnyDesktop => Ok(()),
        DeviceRequirement::AppleSiliconMetal {
            minimum_macos_major,
            minimum_memory_gib,
            chip,
        } if d.os == "macos"
            && d.arch == "aarch64"
            && d.macos_major >= *minimum_macos_major
            && d.memory_gib >= *minimum_memory_gib
            && d.metal_available
            && d.chip == *chip =>
        {
            Ok(())
        }
        _ => Err(ManagerError::new(
            "model_device_unsupported",
            "device does not satisfy model requirement",
        )),
    }
}
pub(super) fn source_identity(a: &ArtifactPlan) -> String {
    format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        a.source_repository, a.source_model, a.revision, a.url, a.expected_sha256, a.required_path
    )
}
pub(super) fn sha256_file(path: &Path) -> Result<String, ManagerError> {
    let mut f = File::open(path)?;
    let mut h = Sha256::new();
    io::copy(&mut f, &mut HashWriter(&mut h))?;
    Ok(hex::encode(h.finalize()))
}
struct HashWriter<'a>(&'a mut Sha256);
impl Write for HashWriter<'_> {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.0.update(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
pub(super) fn sync_dir(p: &Path) -> Result<(), ManagerError> {
    File::open(p)?.sync_all()?;
    Ok(())
}
pub(super) fn real_dir(p: &Path) -> Result<bool, ManagerError> {
    match fs::symlink_metadata(p) {
        Ok(m) => Ok(m.file_type().is_dir()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}
pub(super) fn regular_file(p: &Path) -> Result<bool, ManagerError> {
    match fs::symlink_metadata(p) {
        Ok(m) => Ok(m.file_type().is_file()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}
pub(super) fn read_regular(p: &Path) -> Result<Vec<u8>, ManagerError> {
    if !regular_file(p)? {
        return Err(ManagerError::structural(
            "marker path is not a regular file",
        ));
    }
    Ok(fs::read(p)?)
}
pub(super) fn remove_file(p: &Path) -> Result<(), ManagerError> {
    match fs::remove_file(p) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}
pub(super) fn child_dirs(root: &Path) -> Result<Vec<PathBuf>, ManagerError> {
    let mut out = Vec::new();
    for e in fs::read_dir(root)? {
        let p = e?.path();
        if !fs::symlink_metadata(&p)?.file_type().is_dir() {
            return Err(ManagerError::structural(
                "model directory tree contains a non-directory entry",
            ));
        }
        out.push(p);
    }
    Ok(out)
}
#[cfg(test)]
pub(super) fn checkpoint_bytes(p: &Path) -> Result<u64, ManagerError> {
    match fs::metadata(p) {
        Ok(m) if m.file_type().is_file() => Ok(m.len()),
        Ok(_) => Err(ManagerError::new(
            "insufficient_disk_space",
            "checkpoint path is not a regular file",
        )),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(0),
        Err(e) => Err(ManagerError::new("insufficient_disk_space", e.to_string())),
    }
}
#[cfg(test)]
pub(super) fn same_volume(a: &Path, b: &Path) -> Result<(), ManagerError> {
    #[cfg(unix)]
    {
        if fs::metadata(a)
            .map_err(|e| ManagerError::new("insufficient_disk_space", e.to_string()))?
            .dev()
            != fs::metadata(b)
                .map_err(|e| ManagerError::new("insufficient_disk_space", e.to_string()))?
                .dev()
        {
            return Err(ManagerError::new(
                "insufficient_disk_space",
                "staging and final directories are on different volumes",
            ));
        }
    }
    Ok(())
}
