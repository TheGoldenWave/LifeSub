use std::path::PathBuf;

use chrono::Utc;
use rusqlite::{OptionalExtension, params};

use crate::asr::model_manager::{
    ArtifactCheckpoint, DeletionLease, ExecutionInstallationRecord, ManagerError, ModelCatalog,
    ModelInstallPlan, StoredInstallation,
};
use crate::asr::runtime_qualifier::{
    QualificationCatalog, QualificationHandle, QualificationRecord, QualifierError,
};

use super::Catalog;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ModelDownloadRecord {
    pub id: String,
    pub model_id: String,
    pub manifest_version: String,
    pub bundle_identity: String,
    pub state: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ModelInstallationRecord {
    pub model_id: String,
    pub manifest_version: String,
    pub bundle_identity: String,
    pub install_dir: PathBuf,
    pub state: String,
    pub runtime_identity_json: Option<String>,
    pub qualified_at: Option<String>,
    pub last_error_code: Option<String>,
}

impl ModelCatalog for Catalog {
    fn begin_download(&self, plan: &ModelInstallPlan) -> Result<String, ManagerError> {
        let mut connection = self.connection.lock().unwrap();
        let transaction = connection.transaction()?;
        let download_id = format!("mdl_{}", uuid::Uuid::new_v4().simple());
        let now = Utc::now().to_rfc3339();
        let expected_bytes = plan
            .artifacts
            .iter()
            .map(|artifact| artifact.expected_bytes)
            .sum::<u64>();
        transaction.execute(
            "INSERT INTO model_downloads(
               id, model_id, manifest_version, archive_sha256, state,
               downloaded_bytes, expected_bytes, created_at, updated_at
             ) VALUES(?1, ?2, ?3, ?4, 'queued', 0, ?5, ?6, ?6)",
            params![
                download_id,
                plan.model_id,
                plan.manifest_version,
                plan.bundle_identity,
                expected_bytes,
                now,
            ],
        )?;
        for artifact in &plan.artifacts {
            transaction.execute(
                "INSERT INTO model_download_artifacts(
                   download_id, artifact_id, source_repository, source_model, source_url,
                   source_revision, expected_bytes, expected_sha256, required_path, state,
                   created_at, updated_at
                 ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'pending', ?10, ?10)",
                params![
                    download_id,
                    artifact.artifact_id,
                    artifact.source_repository,
                    artifact.source_model,
                    artifact.url,
                    artifact.revision,
                    artifact.expected_bytes,
                    artifact.expected_sha256,
                    artifact.required_path,
                    now,
                ],
            )?;
        }
        transaction.commit()?;
        Ok(download_id)
    }

    fn checkpoint(
        &self,
        download_id: &str,
        artifact_id: &str,
    ) -> Result<Option<ArtifactCheckpoint>, ManagerError> {
        let connection = self.connection.lock().unwrap();
        connection
            .query_row(
                "SELECT artifact_id, source_repository, source_model, source_revision, source_url,
                        expected_sha256, required_path, downloaded_bytes, expected_bytes, temp_path,
                        etag, last_modified, verified_sha256, state
                 FROM model_download_artifacts
                 WHERE download_id = ?1 AND artifact_id = ?2",
                params![download_id, artifact_id],
                |row| {
                    let repository: String = row.get(1)?;
                    let model: String = row.get(2)?;
                    let revision: String = row.get(3)?;
                    let url: String = row.get(4)?;
                    let expected_sha256: String = row.get(5)?;
                    let required_path: String = row.get(6)?;
                    let downloaded_bytes: i64 = row.get(7)?;
                    let expected_bytes: i64 = row.get(8)?;
                    let temp_path: Option<String> = row.get(9)?;
                    let artifact_id: String = row.get(0)?;
                    let etag: Option<String> = row.get(10)?;
                    let last_modified: Option<String> = row.get(11)?;
                    let verified_sha256: Option<String> = row.get(12)?;
                    let state: String = row.get(13)?;
                    Ok(temp_path.map(|temp_path| ArtifactCheckpoint {
                        artifact_id,
                        source_identity: format!(
                            "{repository}\n{model}\n{revision}\n{url}\n{expected_sha256}\n{required_path}"
                        ),
                        downloaded_bytes: downloaded_bytes as u64,
                        expected_bytes: expected_bytes as u64,
                        temp_path: PathBuf::from(temp_path),
                        etag,
                        last_modified,
                        verified_sha256,
                        state,
                    }))
                },
            )
            .optional()
            .map(|value| value.flatten())
            .map_err(Into::into)
    }

    fn save_checkpoint(
        &self,
        download_id: &str,
        checkpoint: &ArtifactCheckpoint,
    ) -> Result<(), ManagerError> {
        let mut connection = self.connection.lock().unwrap();
        let transaction = connection.transaction()?;
        let now = Utc::now().to_rfc3339();
        let verified_at = checkpoint.verified_sha256.as_ref().map(|_| now.as_str());
        let changed = transaction.execute(
            "UPDATE model_download_artifacts
             SET downloaded_bytes = ?3, temp_path = ?4, etag = ?5, last_modified = ?6,
                 verified_sha256 = ?7, checkpointed_at = ?8, verified_at = ?9,
                 state = ?10, error_code = NULL, error_summary = NULL, updated_at = ?8
             WHERE download_id = ?1 AND artifact_id = ?2",
            params![
                download_id,
                checkpoint.artifact_id,
                checkpoint.downloaded_bytes,
                checkpoint.temp_path.to_string_lossy(),
                checkpoint.etag,
                checkpoint.last_modified,
                checkpoint.verified_sha256,
                now,
                verified_at,
                checkpoint.state,
            ],
        )?;
        if changed != 1 {
            return Err(ManagerError::catalog("artifact checkpoint row missing"));
        }
        let changed = transaction.execute(
            "UPDATE model_downloads
             SET downloaded_bytes = (
                 SELECT COALESCE(SUM(downloaded_bytes), 0)
                 FROM model_download_artifacts WHERE download_id = ?1
             ), updated_at = ?2 WHERE id = ?1",
            params![download_id, now],
        )?;
        if changed != 1 {
            return Err(ManagerError::catalog("model download row missing"));
        }
        transaction.commit()?;
        Ok(())
    }

    fn set_download_state(
        &self,
        download_id: &str,
        state: &str,
        error_code: Option<&str>,
    ) -> Result<(), ManagerError> {
        let connection = self.connection.lock().unwrap();
        let changed = connection.execute(
            "UPDATE model_downloads
             SET state = ?2, error_code = ?3, updated_at = ?4 WHERE id = ?1",
            params![download_id, state, error_code, Utc::now().to_rfc3339()],
        )?;
        if changed == 1 {
            Ok(())
        } else {
            Err(ManagerError::catalog("model download row missing"))
        }
    }

    fn publish_installation(&self, installation: &StoredInstallation) -> Result<(), ManagerError> {
        let connection = self.connection.lock().unwrap();
        connection.execute(
            "INSERT INTO model_installations(
               model_id, provider, manifest_version, archive_sha256, install_dir, state,
               installed_at, runtime_identity_json, qualified_at, last_error_code
             ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
               CASE WHEN ?6 = 'runtime_qualified' THEN ?7 ELSE NULL END, NULL)
             ON CONFLICT(model_id) DO UPDATE SET
               provider = excluded.provider,
               manifest_version = excluded.manifest_version,
               archive_sha256 = excluded.archive_sha256,
               install_dir = excluded.install_dir,
               state = excluded.state,
               installed_at = excluded.installed_at,
               runtime_identity_json = excluded.runtime_identity_json,
               qualified_at = excluded.qualified_at,
               last_error_code = NULL",
            params![
                installation.model_id,
                installation.provider,
                installation.manifest_version,
                installation.bundle_identity,
                installation.install_dir.to_string_lossy(),
                installation.state,
                Utc::now().to_rfc3339(),
                installation.runtime_identity_json,
            ],
        )?;
        Ok(())
    }

    fn record_installation_recovery(
        &self,
        model_id: &str,
        error_code: &str,
    ) -> Result<(), ManagerError> {
        self.connection.lock().unwrap().execute(
            "UPDATE model_installations
             SET state = 'installed_unqualified', runtime_identity_json = NULL,
                 qualified_at = NULL, last_error_code = ?2
             WHERE model_id = ?1",
            params![model_id, error_code],
        )?;
        Ok(())
    }

    fn begin_delete(&self, model_id: &str) -> Result<Option<DeletionLease>, ManagerError> {
        let mut connection = self.connection.lock().unwrap();
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let now = Utc::now().to_rfc3339();
        let lease = transaction
            .query_row(
                "SELECT install_dir, state, runtime_identity_json, qualified_at, last_error_code
                 FROM model_installations
                 WHERE model_id = ?1 AND state <> 'deleting'
                   AND NOT EXISTS (
                     SELECT 1 FROM asr_jobs
                     WHERE model_id = ?1 AND state IN ('preparing', 'transcribing')
                       AND claimed_by IS NOT NULL
                       AND (lease_expires_at IS NULL OR lease_expires_at > ?2)
                   )",
                params![model_id, now],
                |row| {
                    Ok(DeletionLease {
                        model_id: model_id.to_owned(),
                        install_dir: PathBuf::from(row.get::<_, String>(0)?),
                        prior_state: row.get(1)?,
                        prior_runtime_identity_json: row.get(2)?,
                        prior_qualified_at: row.get(3)?,
                        prior_last_error_code: row.get(4)?,
                    })
                },
            )
            .optional()?;
        let Some(lease) = lease else {
            transaction.commit()?;
            return Ok(None);
        };
        let changed = transaction.execute(
            "UPDATE model_installations
             SET state = 'deleting'
             WHERE model_id = ?1 AND state = ?2",
            params![model_id, lease.prior_state],
        )?;
        if changed != 1 {
            return Err(ManagerError::catalog("deletion ownership lost"));
        }
        transaction.commit()?;
        Ok(Some(lease))
    }

    fn finish_delete(&self, lease: &DeletionLease) -> Result<(), ManagerError> {
        let connection = self.connection.lock().unwrap();
        let changed = connection.execute(
            "DELETE FROM model_installations
             WHERE model_id = ?1 AND state = 'deleting' AND install_dir = ?2",
            params![lease.model_id, lease.install_dir.to_string_lossy()],
        )?;
        if changed == 1 {
            Ok(())
        } else {
            Err(ManagerError::catalog("deletion ownership lost"))
        }
    }

    fn abort_delete(&self, lease: &DeletionLease) -> Result<(), ManagerError> {
        let connection = self.connection.lock().unwrap();
        let changed = connection.execute(
            "UPDATE model_installations
             SET state = ?2, runtime_identity_json = ?3, qualified_at = ?4,
                 last_error_code = ?5
             WHERE model_id = ?1 AND state = 'deleting'",
            params![
                lease.model_id,
                lease.prior_state,
                lease.prior_runtime_identity_json,
                lease.prior_qualified_at,
                lease.prior_last_error_code,
            ],
        )?;
        if changed == 1 {
            Ok(())
        } else {
            Err(ManagerError::catalog("deletion rollback ownership lost"))
        }
    }
}

impl QualificationCatalog for Catalog {
    fn qualification_record(
        &self,
        model_id: &str,
    ) -> Result<Option<QualificationRecord>, QualifierError> {
        self.connection
            .lock()
            .unwrap()
            .query_row(
                "SELECT state, manifest_version, archive_sha256, install_dir, runtime_identity_json
                 FROM model_installations WHERE model_id = ?1",
                [model_id],
                |row| {
                    Ok(QualificationRecord {
                        state: row.get(0)?,
                        manifest_version: row.get(1)?,
                        bundle_identity: row.get(2)?,
                        install_dir: PathBuf::from(row.get::<_, String>(3)?),
                        runtime_identity_json: row.get(4)?,
                    })
                },
            )
            .optional()
            .map_err(|error| QualifierError::new("model_catalog_failed", error.to_string()))
    }

    fn cas_runtime_qualified(
        &self,
        handle: &QualificationHandle,
        runtime_identity_json: &str,
    ) -> Result<bool, QualifierError> {
        let changed = self
            .connection
            .lock()
            .unwrap()
            .execute(
                "UPDATE model_installations
             SET state = 'runtime_qualified', runtime_identity_json = ?5,
                 qualified_at = ?6, last_error_code = NULL
             WHERE model_id = ?1 AND state = 'installed_unqualified'
               AND manifest_version = ?2 AND archive_sha256 = ?3 AND install_dir = ?4",
                params![
                    handle.model_id,
                    handle.manifest_version,
                    handle.bundle_identity,
                    handle.install_dir.to_string_lossy(),
                    runtime_identity_json,
                    Utc::now().to_rfc3339(),
                ],
            )
            .map_err(|error| QualifierError::new("model_catalog_failed", error.to_string()))?;
        Ok(changed == 1)
    }

    fn demote_runtime_qualification(
        &self,
        handle: &QualificationHandle,
        error_code: &str,
    ) -> Result<bool, QualifierError> {
        let changed = self
            .connection
            .lock()
            .unwrap()
            .execute(
                "UPDATE model_installations
             SET state = 'installed_unqualified', runtime_identity_json = NULL,
                 qualified_at = NULL, last_error_code = ?5
             WHERE model_id = ?1 AND state = 'runtime_qualified'
               AND manifest_version = ?2 AND archive_sha256 = ?3 AND install_dir = ?4",
                params![
                    handle.model_id,
                    handle.manifest_version,
                    handle.bundle_identity,
                    handle.install_dir.to_string_lossy(),
                    error_code,
                ],
            )
            .map_err(|error| QualifierError::new("model_catalog_failed", error.to_string()))?;
        Ok(changed == 1)
    }

    fn record_qualification_error(
        &self,
        handle: &QualificationHandle,
        error_code: &str,
    ) -> Result<(), QualifierError> {
        self.connection
            .lock()
            .unwrap()
            .execute(
                "UPDATE model_installations SET last_error_code = ?5
             WHERE model_id = ?1 AND state = 'installed_unqualified'
               AND manifest_version = ?2 AND archive_sha256 = ?3 AND install_dir = ?4",
                params![
                    handle.model_id,
                    handle.manifest_version,
                    handle.bundle_identity,
                    handle.install_dir.to_string_lossy(),
                    error_code,
                ],
            )
            .map_err(|error| QualifierError::new("model_catalog_failed", error.to_string()))?;
        Ok(())
    }
}

impl Catalog {
    pub(crate) fn execution_installation(
        &self,
        model_id: &str,
    ) -> Result<Option<ExecutionInstallationRecord>, ManagerError> {
        self.connection
            .lock()
            .unwrap()
            .query_row(
                "SELECT model_id, manifest_version, archive_sha256, install_dir, state,
                        runtime_identity_json
                 FROM model_installations WHERE model_id = ?1",
                [model_id],
                |row| {
                    Ok(ExecutionInstallationRecord {
                        model_id: row.get(0)?,
                        manifest_version: row.get(1)?,
                        bundle_identity: row.get(2)?,
                        install_dir: PathBuf::from(row.get::<_, String>(3)?),
                        state: row.get(4)?,
                        runtime_identity_json: row.get(5)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }
}

impl Catalog {
    pub(crate) fn model_download_records(&self) -> rusqlite::Result<Vec<ModelDownloadRecord>> {
        let connection = self.connection.lock().unwrap();
        let mut statement = connection.prepare(
            "SELECT id, model_id, manifest_version, archive_sha256, state, updated_at
             FROM model_downloads ORDER BY updated_at DESC, id DESC",
        )?;
        statement
            .query_map([], |row| {
                Ok(ModelDownloadRecord {
                    id: row.get(0)?,
                    model_id: row.get(1)?,
                    manifest_version: row.get(2)?,
                    bundle_identity: row.get(3)?,
                    state: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            })?
            .collect()
    }

    pub(crate) fn model_download_artifact_ids(
        &self,
        download_id: &str,
    ) -> rusqlite::Result<Vec<String>> {
        let connection = self.connection.lock().unwrap();
        let mut statement = connection.prepare(
            "SELECT artifact_id FROM model_download_artifacts
             WHERE download_id = ?1 ORDER BY artifact_id",
        )?;
        statement
            .query_map([download_id], |row| row.get(0))?
            .collect()
    }

    pub(crate) fn mark_download_recovery_required(
        &self,
        download_id: &str,
    ) -> rusqlite::Result<()> {
        self.connection.lock().unwrap().execute(
            "UPDATE model_downloads
             SET state = 'failed', error_code = 'recovery_required', updated_at = ?2
             WHERE id = ?1 AND state IN ('queued', 'downloading', 'verifying', 'installing')",
            params![download_id, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    pub(crate) fn model_installation_records(
        &self,
    ) -> rusqlite::Result<Vec<ModelInstallationRecord>> {
        let connection = self.connection.lock().unwrap();
        let mut statement = connection.prepare(
            "SELECT model_id, manifest_version, archive_sha256, install_dir, state,
                    runtime_identity_json, qualified_at, last_error_code
             FROM model_installations ORDER BY model_id",
        )?;
        statement
            .query_map([], |row| {
                Ok(ModelInstallationRecord {
                    model_id: row.get(0)?,
                    manifest_version: row.get(1)?,
                    bundle_identity: row.get(2)?,
                    install_dir: PathBuf::from(row.get::<_, String>(3)?),
                    state: row.get(4)?,
                    runtime_identity_json: row.get(5)?,
                    qualified_at: row.get(6)?,
                    last_error_code: row.get(7)?,
                })
            })?
            .collect()
    }

    pub(crate) fn complete_deletion_recovery(&self, lease: &DeletionLease) -> rusqlite::Result<()> {
        let changed = self.connection.lock().unwrap().execute(
            "DELETE FROM model_installations
             WHERE model_id = ?1 AND state = 'deleting' AND install_dir = ?2",
            params![lease.model_id, lease.install_dir.to_string_lossy()],
        )?;
        if changed == 1 {
            Ok(())
        } else {
            Err(rusqlite::Error::QueryReturnedNoRows)
        }
    }

    pub(crate) fn model_deletion_lease(
        &self,
        model_id: &str,
    ) -> rusqlite::Result<Option<DeletionLease>> {
        self.connection
            .lock()
            .unwrap()
            .query_row(
                "SELECT install_dir, runtime_identity_json, qualified_at, last_error_code
                 FROM model_installations WHERE model_id = ?1 AND state = 'deleting'",
                [model_id],
                |row| {
                    let qualified_at: Option<String> = row.get(2)?;
                    Ok(DeletionLease {
                        model_id: model_id.to_owned(),
                        install_dir: PathBuf::from(row.get::<_, String>(0)?),
                        prior_state: if qualified_at.is_some() {
                            "runtime_qualified".to_owned()
                        } else {
                            "installed_unqualified".to_owned()
                        },
                        prior_runtime_identity_json: row.get(1)?,
                        prior_qualified_at: qualified_at,
                        prior_last_error_code: row.get(3)?,
                    })
                },
            )
            .optional()
    }

    #[cfg(test)]
    pub(crate) fn model_artifact_count(&self, download_id: &str) -> rusqlite::Result<i64> {
        self.connection.lock().unwrap().query_row(
            "SELECT COUNT(*) FROM model_download_artifacts WHERE download_id = ?1",
            [download_id],
            |row| row.get(0),
        )
    }

    #[cfg(test)]
    pub(crate) fn model_downloaded_bytes_for_test(
        &self,
        download_id: &str,
    ) -> rusqlite::Result<i64> {
        self.connection.lock().unwrap().query_row(
            "SELECT downloaded_bytes FROM model_downloads WHERE id = ?1",
            [download_id],
            |row| row.get(0),
        )
    }

    #[cfg(test)]
    pub(crate) fn insert_test_model_lease(&self, model_id: &str) -> rusqlite::Result<()> {
        let connection = self.connection.lock().unwrap();
        connection.execute_batch(
            "INSERT OR IGNORE INTO sessions(id, title, state, started_at) VALUES('lease-session', 'lease', 'stopped', '2026-08-16T00:00:00Z');
             INSERT OR IGNORE INTO chunks(id, session_id, source, path, sha256, byte_length)
             VALUES('lease-chunk', 'lease-session', 'imported', 'lease.wav', 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', 1);",
        )?;
        connection.execute(
            "INSERT INTO asr_jobs(
               id, session_id, chunk_id, provider, model_id, manifest_version, archive_sha256,
               required_file_hashes_json, model_source_json, parameters_json, input_sha256,
               fingerprint, state, attempt_count, claim_generation, max_attempts, available_at,
               claimed_by, lease_expires_at, created_at, updated_at
             ) VALUES(
               'lease-job', 'lease-session', 'lease-chunk', 'whisper', ?1, '1',
               'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
               '{}', '{}', '{}',
               'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
               'lease-fingerprint', 'transcribing', 1, 1, 3, '2026-08-16T00:00:00Z',
               'boot:worker', '2999-01-01T00:00:00Z', '2026-08-16T00:00:00Z',
               '2026-08-16T00:00:00Z')",
            [model_id],
        )?;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn model_installation_state(
        &self,
        model_id: &str,
    ) -> rusqlite::Result<(String, Option<String>)> {
        self.connection.lock().unwrap().query_row(
            "SELECT state, last_error_code FROM model_installations WHERE model_id = ?1",
            [model_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
    }
}
