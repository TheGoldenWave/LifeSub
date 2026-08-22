use super::anchored_reconcile::AnchoredFs;
use super::fs_support::*;
use super::*;

impl<T: HttpTransport, C: ModelCatalog> ModelManager<T, C> {
    pub fn delete_model(&self, id: &str) -> Result<(), ManagerError> {
        self.ensure_runtime_current()?;
        validate_component("model_id", id)?;
        let p = resolve_current_plan(id)?;
        if let Some(fs) = self.storage.anchored_fs() {
            let relative = self.storage.relative_install_path(&self.install_dir(&p))?;
            return self.delete_anchored(id, fs.as_ref(), &relative);
        }
        self.delete(id, &self.install_dir(&p))
    }

    fn delete_anchored(
        &self,
        id: &str,
        fs: &AnchoredFs,
        relative: &Path,
    ) -> Result<(), ManagerError> {
        self.ensure_runtime_current()?;
        validate_component("model_id", id)?;
        let mut registry = self.execution_leases.lock().unwrap();
        if registry.get(id).copied().unwrap_or(0) != 0 {
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
        registry.insert(id.to_owned(), DELETE_RESERVED);
        drop(registry);
        let _reservation = DeletionReservationGuard {
            model_id: id.to_owned(),
            registry: self.execution_leases.clone(),
        };
        if fs.open_dir(relative)?.is_none() {
            self.catalog.abort_delete(&lease)?;
            return Err(ManagerError::integrity("installation directory missing"));
        }
        let trash =
            PathBuf::from("trash").join(format!("{}-{}", id, uuid::Uuid::new_v4().simple()));
        let prepared = (|| {
            let marker_bytes =
                serde_json::to_vec(&lease).map_err(|e| ManagerError::structural(e.to_string()))?;
            let marker_tmp = relative.join(format!("{DELETE_MARKER}.tmp"));
            fs.write_new_synced(&marker_tmp, &marker_bytes)?;
            fs.rename_noreplace(&marker_tmp, &relative.join(DELETE_MARKER))?;
            fs.sync_dir(relative)?;
            fs.ensure_dir(Path::new("trash"))?;
            fs.rename(relative, &trash)?;
            fs.sync_dir(relative.parent().unwrap_or_else(|| Path::new("")))?;
            fs.sync_dir(Path::new("trash"))
        })();
        if let Err(e) = prepared {
            restore_anchored(fs, relative, &trash)?;
            self.catalog.abort_delete(&lease)?;
            return Err(e);
        }
        if let Err(e) = self.catalog.finish_delete(&lease) {
            restore_anchored(fs, relative, &trash)?;
            self.catalog.abort_delete(&lease)?;
            return Err(e);
        }
        if fs.open_dir(&trash)?.is_some() {
            fs.remove_tree(&trash)?;
            fs.sync_dir(Path::new("trash"))?;
        }
        Ok(())
    }

    pub(crate) fn delete(&self, id: &str, install: &Path) -> Result<(), ManagerError> {
        if let Some(fs) = self.storage.anchored_fs() {
            let relative = self.storage.relative_install_path(install)?;
            return self.delete_anchored(id, fs.as_ref(), &relative);
        }
        self.delete_path(id, install)
    }

    fn delete_path(&self, id: &str, install: &Path) -> Result<(), ManagerError> {
        self.ensure_runtime_current()?;
        validate_component("model_id", id)?;
        let mut registry = self.execution_leases.lock().unwrap();
        if registry.get(id).copied().unwrap_or(0) != 0 {
            return Err(ManagerError::new(
                "model_in_use",
                "active provider execution lease prevents deletion",
            ));
        }
        #[cfg(test)]
        let barriers = take_delete_reservation_barriers_for_test(install);
        #[cfg(test)]
        if let Some(barriers) = &barriers {
            barriers.0.wait();
            barriers.1.wait();
        }
        let Some(lease) = self.catalog.begin_delete(id)? else {
            return Err(ManagerError::new(
                "model_in_use",
                "active ASR lease prevents deletion",
            ));
        };
        registry.insert(id.to_owned(), DELETE_RESERVED);
        drop(registry);
        let _reservation = DeletionReservationGuard {
            model_id: id.to_owned(),
            registry: self.execution_leases.clone(),
        };
        #[cfg(test)]
        if let Some(barriers) = &barriers {
            barriers.2.wait();
        }
        if lease.install_dir != install {
            self.catalog.abort_delete(&lease)?;
            return Err(ManagerError::integrity("installation path mismatch"));
        }
        if !real_dir(install)? {
            self.catalog.abort_delete(&lease)?;
            return Err(ManagerError::integrity("installation directory missing"));
        }
        let trash_root = self.storage.nominal_root().join("trash");
        let trash = trash_root.join(format!("{}-{}", id, uuid::Uuid::new_v4().simple()));
        let prepared = (|| {
            #[cfg(test)]
            let fault = self.delete_marker_fault;
            #[cfg(not(test))]
            let fault = None;
            write_json_marker(install, DELETE_MARKER, &lease, fault)?;
            fs::create_dir_all(&trash_root)?;
            fs::rename(install, &trash)?;
            sync_dir(install.parent().unwrap_or(self.storage.nominal_root()))?;
            sync_dir(&trash_root)
        })();
        if let Err(e) = prepared {
            restore(install, &trash, &trash_root, self.storage.nominal_root())?;
            self.catalog.abort_delete(&lease)?;
            return Err(e);
        }
        if let Err(e) = self.catalog.finish_delete(&lease) {
            restore(install, &trash, &trash_root, self.storage.nominal_root())?;
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

struct DeletionReservationGuard {
    model_id: String,
    registry: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, usize>>>,
}

impl Drop for DeletionReservationGuard {
    fn drop(&mut self) {
        let mut registry = self.registry.lock().unwrap();
        if registry.get(&self.model_id) == Some(&DELETE_RESERVED) {
            registry.remove(&self.model_id);
        }
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

fn restore_anchored(fs: &AnchoredFs, relative: &Path, trash: &Path) -> Result<(), ManagerError> {
    if fs.open_dir(trash)?.is_some() {
        fs.ensure_dir(relative.parent().unwrap_or_else(|| Path::new("")))?;
        fs.rename(trash, relative)?;
        fs.sync_dir(relative.parent().unwrap_or_else(|| Path::new("")))?;
        if fs.open_dir(Path::new("trash"))?.is_some() {
            fs.sync_dir(Path::new("trash"))?;
        }
    }
    if fs.open_dir(relative)?.is_some() {
        let tmp = relative.join(format!("{DELETE_MARKER}.tmp"));
        let final_marker = relative.join(DELETE_MARKER);
        if fs.file_len(&tmp)?.is_some() {
            let _ = fs.remove_file(&tmp);
        }
        if fs.file_len(&final_marker)?.is_some() {
            let _ = fs.remove_file(&final_marker);
        }
        fs.sync_dir(relative)?;
    }
    Ok(())
}
