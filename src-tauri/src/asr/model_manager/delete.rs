use super::fs_support::*;
use super::*;

impl<T: HttpTransport, C: ModelCatalog> ModelManager<T, C> {
    pub fn delete_model(&self, id: &str) -> Result<(), ManagerError> {
        validate_component("model_id", id)?;
        let p = resolve_current_plan(id)?;
        self.delete(id, &self.install_dir(&p))
    }
    pub(crate) fn delete(&self, id: &str, install: &Path) -> Result<(), ManagerError> {
        validate_component("model_id", id)?;
        if self.execution_lease_count(id) != 0 {
            return Err(ManagerError::new(
                "model_in_use",
                "active provider execution lease prevents deletion",
            ));
        }
        let Some(lease) = self.catalog.begin_delete(id)? else {
            return Err(ManagerError::new(
                "model_in_use",
                "active ASR lease prevents deletion",
            ));
        };
        if lease.install_dir != install {
            self.catalog.abort_delete(&lease)?;
            return Err(ManagerError::integrity("installation path mismatch"));
        }
        if !real_dir(install)? {
            self.catalog.abort_delete(&lease)?;
            return Err(ManagerError::integrity("installation directory missing"));
        }
        let trash_root = self.root.join("trash");
        let trash = trash_root.join(format!("{}-{}", id, uuid::Uuid::new_v4().simple()));
        let prepared = (|| {
            #[cfg(test)]
            let fault = self.delete_marker_fault;
            #[cfg(not(test))]
            let fault = None;
            write_json_marker(install, DELETE_MARKER, &lease, fault)?;
            fs::create_dir_all(&trash_root)?;
            fs::rename(install, &trash)?;
            sync_dir(install.parent().unwrap_or(&self.root))?;
            sync_dir(&trash_root)
        })();
        if let Err(e) = prepared {
            restore(install, &trash, &trash_root, &self.root)?;
            self.catalog.abort_delete(&lease)?;
            return Err(e);
        }
        if let Err(e) = self.catalog.finish_delete(&lease) {
            restore(install, &trash, &trash_root, &self.root)?;
            self.catalog.abort_delete(&lease)?;
            return Err(e);
        }
        if trash.exists() {
            fs::remove_dir_all(&trash)?;
            sync_dir(&trash_root)?;
        }
        Ok(())
    }
}

pub(super) fn write_json_marker<T: Serialize>(
    dir: &Path,
    name: &str,
    value: &T,
    fault: Option<DeleteMarkerFault>,
) -> Result<(), ManagerError> {
    #[cfg(not(test))]
    let _ = fault;
    let tmp = dir.join(format!("{name}.tmp"));
    let final_path = dir.join(name);
    let bytes = serde_json::to_vec(value).map_err(|e| ManagerError::structural(e.to_string()))?;
    let result = (|| {
        let mut f = OpenOptions::new().create_new(true).write(true).open(&tmp)?;
        #[cfg(test)]
        if fault == Some(DeleteMarkerFault::Write) {
            return Err(ManagerError::new(
                "model_io_failed",
                "injected delete marker write failure",
            ));
        }
        f.write_all(&bytes)?;
        #[cfg(test)]
        if fault == Some(DeleteMarkerFault::Sync) {
            return Err(ManagerError::new(
                "model_io_failed",
                "injected delete marker sync failure",
            ));
        }
        f.sync_all()?;
        drop(f);
        #[cfg(test)]
        if fault == Some(DeleteMarkerFault::Rename) {
            return Err(ManagerError::new(
                "model_io_failed",
                "injected delete marker rename failure",
            ));
        }
        fs::rename(&tmp, &final_path)?;
        sync_dir(dir)
    })();
    if let Err(e) = result {
        cleanup_markers(dir, &tmp, &final_path)?;
        return Err(e);
    }
    Ok(())
}
pub(super) fn cleanup_markers(
    dir: &Path,
    tmp: &Path,
    final_path: &Path,
) -> Result<(), ManagerError> {
    remove_file(tmp)?;
    remove_file(final_path)?;
    sync_dir(dir)
}
fn restore(
    install: &Path,
    trash: &Path,
    trash_root: &Path,
    root: &Path,
) -> Result<(), ManagerError> {
    if trash.exists() {
        fs::create_dir_all(install.parent().unwrap_or(root))?;
        fs::rename(trash, install)?;
        sync_dir(install.parent().unwrap_or(root))?;
        if real_dir(trash_root)? {
            sync_dir(trash_root)?;
        }
    }
    if real_dir(install)? {
        cleanup_markers(
            install,
            &install.join(format!("{DELETE_MARKER}.tmp")),
            &install.join(DELETE_MARKER),
        )?;
    }
    Ok(())
}
