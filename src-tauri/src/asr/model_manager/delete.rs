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
