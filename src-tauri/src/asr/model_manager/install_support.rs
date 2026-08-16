use super::fs_support::*;
use super::install::{MarkerInstalledFile, StructuralMarker};
use super::*;

pub(super) fn create_parents(root: &Path, dst: &Path) -> Result<(), ManagerError> {
    let rel = dst
        .parent()
        .ok_or_else(|| ManagerError::structural("destination missing parent"))?
        .strip_prefix(root)
        .map_err(|_| ManagerError::structural("destination escaped staging"))?;
    let mut cur = root.to_path_buf();
    for c in rel.components() {
        cur.push(c);
        match fs::symlink_metadata(&cur) {
            Ok(m) if m.file_type().is_symlink() || !m.is_dir() => {
                return Err(ManagerError::structural("unsafe staging ancestor"));
            }
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => fs::create_dir(&cur)?,
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}
pub(super) fn copy_new(src: &Path, dst: &Path) -> Result<(), ManagerError> {
    let mut i = File::open(src)?;
    let mut o = OpenOptions::new().create_new(true).write(true).open(dst)?;
    io::copy(&mut i, &mut o)?;
    o.sync_all()?;
    Ok(())
}
pub(super) fn installed_records(root: &Path) -> Result<Vec<MarkerInstalledFile>, ManagerError> {
    fn visit(
        root: &Path,
        dir: &Path,
        out: &mut Vec<MarkerInstalledFile>,
    ) -> Result<(), ManagerError> {
        for e in fs::read_dir(dir)? {
            let p = e?.path();
            let m = fs::symlink_metadata(&p)?;
            if m.file_type().is_symlink() {
                return Err(ManagerError::structural("symlink in installed bundle"));
            }
            if m.file_type().is_dir() {
                visit(root, &p, out)?;
            } else if !m.file_type().is_file() {
                return Err(ManagerError::structural("special file in installed bundle"));
            } else if !matches!(
                p.file_name().and_then(|n| n.to_str()),
                Some(STRUCTURAL_MARKER)
                    | Some(DELETE_MARKER)
                    | Some(crate::asr::runtime_qualifier::RUNTIME_QUALIFICATION_MARKER)
            ) {
                out.push(MarkerInstalledFile {
                    relative_path: p
                        .strip_prefix(root)
                        .map_err(|_| ManagerError::structural("installed path escaped root"))?
                        .to_str()
                        .ok_or_else(|| ManagerError::structural("non-UTF-8 installed path"))?
                        .to_owned(),
                    bytes: m.len(),
                    sha256: sha256_file(&p)?,
                });
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    visit(root, root, &mut out)?;
    out.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok(out)
}
pub(super) fn validate_inventory(
    p: &ModelInstallPlan,
    files: &[MarkerInstalledFile],
) -> Result<(), ManagerError> {
    if files.len() != p.install_contract.required_files().len() {
        return Err(ManagerError::integrity(
            "installed inventory count differs from manifest",
        ));
    }
    for f in p.install_contract.required_files() {
        if !files.iter().any(|i| {
            (i.relative_path.as_str(), i.bytes, i.sha256.as_str())
                == (f.path.as_str(), f.bytes, f.sha256.as_str())
        }) {
            return Err(ManagerError::integrity(
                "installed inventory differs from manifest",
            ));
        }
    }
    Ok(())
}
#[cfg(test)]
pub(crate) fn validate_required_inventory_for_test(
    p: &ModelInstallPlan,
    files: &[RequiredInstalledFile],
) -> Result<(), ManagerError> {
    validate_inventory(
        p,
        &files
            .iter()
            .map(|f| MarkerInstalledFile {
                relative_path: f.path.clone(),
                bytes: f.bytes,
                sha256: f.sha256.clone(),
            })
            .collect::<Vec<_>>(),
    )
}
pub(super) fn validate_structure(
    p: &ModelInstallPlan,
    root: &Path,
    observed: Option<&FullSherpaRuntimeIdentity>,
) -> Result<(), ManagerError> {
    for f in p.install_contract.required_files() {
        if !fs::symlink_metadata(root.join(safe_relative_path(&f.path)?))
            .map(|m| m.file_type().is_file())
            .unwrap_or(false)
        {
            return Err(ManagerError::structural("required path missing"));
        }
    }
    match p.qualification_policy {
        QualificationPolicy::StructuralWithPinnedRuntime => {
            if !matches!((p.sherpa_runtime.as_ref(),observed),(Some(a),Some(b))if a.matches(b)) {
                return Err(ManagerError::new(
                    "model_runtime_identity_mismatch",
                    "pinned sherpa identity missing",
                ));
            }
        }
        QualificationPolicy::RuntimeSmokeRequired => validate_qwen(root)?,
    }
    Ok(())
}
pub(super) fn validate_structure_for_reconcile(
    p: &ModelInstallPlan,
    root: &Path,
    observed: Option<&FullSherpaRuntimeIdentity>,
) -> Result<(), ManagerError> {
    validate_structure(p, root, observed)
}
fn validate_qwen(root: &Path) -> Result<(), ManagerError> {
    let c: serde_json::Value = serde_json::from_slice(&fs::read(root.join("config.json"))?)
        .map_err(|_| ManagerError::structural("invalid Qwen config"))?;
    if !c
        .get("thinker_config")
        .is_some_and(serde_json::Value::is_object)
    {
        return Err(ManagerError::structural("Qwen thinker_config missing"));
    }
    let idx: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("model.safetensors.index.json"))?)
            .map_err(|_| ManagerError::structural("invalid shard index"))?;
    let map = idx
        .get("weight_map")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| ManagerError::structural("shard weight_map missing"))?;
    let refs = map
        .values()
        .filter_map(serde_json::Value::as_str)
        .collect::<HashSet<_>>();
    if refs
        != [
            "model-00001-of-00002.safetensors",
            "model-00002-of-00002.safetensors",
        ]
        .into_iter()
        .collect()
    {
        return Err(ManagerError::structural("unexpected shard index coverage"));
    }
    let t: serde_json::Value = serde_json::from_slice(&fs::read(root.join("tokenizer.json"))?)
        .map_err(|_| ManagerError::structural("invalid tokenizer JSON"))?;
    let vocab = t
        .get("model")
        .and_then(|v| v.get("vocab"))
        .and_then(serde_json::Value::as_object)
        .is_some_and(|v| !v.is_empty());
    let tokens = t
        .get("added_tokens")
        .and_then(serde_json::Value::as_array)
        .map(|v| {
            v.iter()
                .filter_map(|x| x.get("content").and_then(serde_json::Value::as_str))
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default();
    if !vocab
        || !["<|endoftext|>", "<|im_start|>", "<|im_end|>"]
            .iter()
            .all(|v| tokens.contains(v))
    {
        return Err(ManagerError::structural("invalid tokenizer contract"));
    }
    Ok(())
}
pub(super) fn sync_tree(root: &Path) -> Result<(), ManagerError> {
    for e in fs::read_dir(root)? {
        let p = e?.path();
        let m = fs::symlink_metadata(&p)?;
        if m.file_type().is_dir() {
            sync_tree(&p)?;
            sync_dir(&p)?;
        } else if m.file_type().is_file() {
            File::open(&p)?.sync_all()?;
        } else {
            return Err(ManagerError::structural("special file in staging"));
        }
    }
    Ok(())
}
pub(super) fn write_marker(root: &Path, m: &StructuralMarker) -> Result<(), ManagerError> {
    super::delete::write_json_marker(root, STRUCTURAL_MARKER, m, None)
}
