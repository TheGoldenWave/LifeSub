use super::fs_support::*;
use super::*;

pub(crate) fn extract_tar_bz2_safely(
    archive_path: &Path,
    destination: &Path,
    contract: &InstallContract,
) -> Result<u64, ManagerError> {
    let InstallContract::Archive {
        archive_root,
        max_scanned_entries,
        max_written_file_bytes,
        max_total_written_bytes,
        required_files,
    } = contract
    else {
        return Err(ManagerError::structural(
            "archive extractor received direct contract",
        ));
    };
    let root = safe_relative_path(archive_root)?;
    let required = required_files
        .iter()
        .map(|f| (PathBuf::from(&f.path), f))
        .collect::<BTreeMap<_, _>>();
    let mut archive = tar::Archive::new(BzDecoder::new(File::open(archive_path)?));
    let mut seen = HashSet::new();
    let mut written_paths = HashSet::new();
    let mut scanned = 0u64;
    let mut total = 0u64;
    for entry in archive.entries().map_err(ManagerError::from)? {
        let mut entry = entry.map_err(ManagerError::from)?;
        scanned = scanned
            .checked_add(1)
            .ok_or_else(|| ManagerError::structural("archive entry count overflow"))?;
        if scanned > *max_scanned_entries {
            return Err(ManagerError::structural(
                "archive entry count exceeds contract",
            ));
        }
        let kind = entry.header().entry_type();
        if !(kind.is_file() || kind.is_dir()) {
            return Err(ManagerError::structural(
                "archive link or special entry rejected",
            ));
        }
        let p = safe_relative_path(
            entry
                .path()
                .map_err(ManagerError::from)?
                .to_str()
                .ok_or_else(|| ManagerError::structural("non-UTF-8 archive path"))?,
        )?;
        if !seen.insert(p.clone()) {
            return Err(ManagerError::structural("duplicate archive path"));
        }
        let rel = p
            .strip_prefix(&root)
            .map_err(|_| ManagerError::structural("archive has unexpected top-level root"))?;
        if rel.as_os_str().is_empty() {
            if !kind.is_dir() {
                return Err(ManagerError::structural("archive root is not a directory"));
            }
            continue;
        }
        if kind.is_dir() {
            continue;
        }
        let Some(req) = required.get(rel) else {
            continue;
        };
        let size = entry.size();
        if size > *max_written_file_bytes {
            return Err(ManagerError::structural(
                "archive required file exceeds per-file limit",
            ));
        }
        if size != req.bytes {
            return Err(ManagerError::integrity(
                "archive required file size differs from manifest",
            ));
        }
        total = total
            .checked_add(size)
            .ok_or_else(|| ManagerError::structural("archive written bytes overflow"))?;
        if total > *max_total_written_bytes {
            return Err(ManagerError::structural(
                "archive written bytes exceed contract",
            ));
        }
        if !written_paths.insert(rel.to_path_buf()) {
            return Err(ManagerError::structural(
                "duplicate normalized required path",
            ));
        }
        let output = destination.join(rel);
        create_parents(destination, &output)?;
        let mut f = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&output)?;
        if io::copy(&mut entry, &mut f)? != size {
            return Err(ManagerError::integrity("archive entry length mismatch"));
        }
        f.sync_all()?;
        if sha256_file(&output)? != req.sha256 {
            return Err(ManagerError::integrity(
                "archive required file hash differs from manifest",
            ));
        }
    }
    if scanned != *max_scanned_entries
        || total != *max_total_written_bytes
        || written_paths.len() != required.len()
        || required.keys().any(|p| !written_paths.contains(p))
    {
        return Err(ManagerError::structural(
            "archive inventory does not match manifest contract",
        ));
    }
    Ok(total)
}
fn create_parents(root: &Path, dst: &Path) -> Result<(), ManagerError> {
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
