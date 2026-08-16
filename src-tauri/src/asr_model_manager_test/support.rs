use std::collections::{HashSet, VecDeque};
use std::ffi::CString;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use sha2::Digest;
use tempfile::TempDir;

use crate::asr::manifest::{model_registry, vad_manifest};
use crate::asr::model_manager::{
    ArtifactCheckpoint, ArtifactPlan, DeleteMarkerFault, DeletionLease, DeviceProfile,
    DeviceRequirement, DownloadRequest, DownloadResponse, FullSherpaRuntimeIdentity, HttpTransport,
    InstallContract, InstallFault, InstallMode, ManagerError, ModelCatalog, ModelInstallPlan,
    ModelManager, QualificationPolicy, RequiredInstalledFile, ReqwestTransport, StoredInstallation,
    checked_required_additional_free, extract_tar_bz2_safely, validate_required_inventory_for_test,
};
use crate::catalog::Catalog;
use crate::service::CoreRuntime;

#[derive(Clone, Default)]
struct ScriptedTransport {
    requests: Arc<Mutex<Vec<DownloadRequest>>>,
    responses: Arc<Mutex<VecDeque<Result<DownloadResponse, ManagerError>>>>,
}

#[derive(Clone)]
struct LoopbackTransport {
    client: reqwest::blocking::Client,
}

impl LoopbackTransport {
    fn new() -> Self {
        Self {
            client: reqwest::blocking::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap(),
        }
    }

    fn with_read_timeout(timeout: Duration) -> Self {
        Self {
            client: reqwest::blocking::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(timeout)
                .timeout(timeout)
                .build()
                .unwrap(),
        }
    }
}

impl HttpTransport for LoopbackTransport {
    fn execute(&self, request: &DownloadRequest) -> Result<DownloadResponse, ManagerError> {
        let mut builder = self.client.get(&request.url);
        if let Some(start) = request.range_start {
            builder = builder.header(reqwest::header::RANGE, format!("bytes={start}-"));
        }
        if let Some(value) = &request.if_range {
            builder = builder.header(reqwest::header::IF_RANGE, value);
        }
        let response = builder
            .send()
            .map_err(|error| ManagerError::catalog(error.to_string()))?;
        let status = response.status().as_u16();
        let final_url = response.url().to_string();
        let headers = response
            .headers()
            .iter()
            .map(|(name, value)| (name.as_str().to_owned(), value.to_str().unwrap().to_owned()))
            .collect();
        Ok(DownloadResponse {
            status,
            final_url,
            headers,
            body: Box::new(response),
        })
    }
}

impl ScriptedTransport {
    fn with(responses: Vec<Result<DownloadResponse, ManagerError>>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(responses.into())),
            ..Self::default()
        }
    }
}

impl HttpTransport for ScriptedTransport {
    fn execute(&self, request: &DownloadRequest) -> Result<DownloadResponse, ManagerError> {
        self.requests.lock().unwrap().push(request.clone());
        self.responses.lock().unwrap().pop_front().unwrap()
    }
}

#[derive(Clone, Default)]
struct MemoryCatalog {
    begins: Arc<Mutex<usize>>,
    checkpoints: Arc<Mutex<Vec<ArtifactCheckpoint>>>,
    installations: Arc<Mutex<Vec<StoredInstallation>>>,
    publish_failures: Arc<Mutex<usize>>,
    finish_delete_failures: Arc<Mutex<usize>>,
    restored_deletions: Arc<Mutex<Vec<DeletionLease>>>,
    download_states: DownloadStateLog,
    state_failure: Arc<Mutex<Option<String>>>,
}

type DownloadStateLog = Arc<Mutex<Vec<(String, Option<String>)>>>;

impl ModelCatalog for MemoryCatalog {
    fn begin_download(&self, _: &ModelInstallPlan) -> Result<String, ManagerError> {
        *self.begins.lock().unwrap() += 1;
        Ok("download-1".to_owned())
    }

    fn checkpoint(
        &self,
        _: &str,
        artifact_id: &str,
    ) -> Result<Option<ArtifactCheckpoint>, ManagerError> {
        Ok(self
            .checkpoints
            .lock()
            .unwrap()
            .iter()
            .find(|item| item.artifact_id == artifact_id)
            .cloned())
    }

    fn save_checkpoint(
        &self,
        _: &str,
        checkpoint: &ArtifactCheckpoint,
    ) -> Result<(), ManagerError> {
        let mut checkpoints = self.checkpoints.lock().unwrap();
        checkpoints.retain(|item| item.artifact_id != checkpoint.artifact_id);
        checkpoints.push(checkpoint.clone());
        Ok(())
    }

    fn set_download_state(
        &self,
        _: &str,
        state: &str,
        error: Option<&str>,
    ) -> Result<(), ManagerError> {
        let mut failure = self.state_failure.lock().unwrap();
        if failure.as_deref() == Some(state) {
            *failure = None;
            return Err(ManagerError::catalog("injected state transition failure"));
        }
        self.download_states
            .lock()
            .unwrap()
            .push((state.to_owned(), error.map(str::to_owned)));
        Ok(())
    }

    fn publish_installation(&self, installation: &StoredInstallation) -> Result<(), ManagerError> {
        let mut failures = self.publish_failures.lock().unwrap();
        if *failures > 0 {
            *failures -= 1;
            return Err(ManagerError::catalog("injected publish failure"));
        }
        self.installations
            .lock()
            .unwrap()
            .push(installation.clone());
        Ok(())
    }

    fn record_installation_recovery(&self, _: &str, _: &str) -> Result<(), ManagerError> {
        Ok(())
    }

    fn begin_delete(&self, model_id: &str) -> Result<Option<DeletionLease>, ManagerError> {
        Ok(self
            .installations
            .lock()
            .unwrap()
            .iter()
            .find(|installation| installation.model_id == model_id)
            .map(|installation| DeletionLease {
                model_id: model_id.to_owned(),
                install_dir: installation.install_dir.clone(),
                prior_state: installation.state.clone(),
                prior_runtime_identity_json: installation.runtime_identity_json.clone(),
                prior_qualified_at: (installation.state == "runtime_qualified")
                    .then(|| "qualified-at".to_owned()),
                prior_last_error_code: None,
            }))
    }

    fn finish_delete(&self, _: &DeletionLease) -> Result<(), ManagerError> {
        let mut failures = self.finish_delete_failures.lock().unwrap();
        if *failures > 0 {
            *failures -= 1;
            return Err(ManagerError::catalog("injected delete failure"));
        }
        Ok(())
    }

    fn abort_delete(&self, lease: &DeletionLease) -> Result<(), ManagerError> {
        self.restored_deletions.lock().unwrap().push(lease.clone());
        Ok(())
    }
}

fn compatible_device() -> DeviceProfile {
    DeviceProfile {
        os: "macos".to_owned(),
        arch: "aarch64".to_owned(),
        macos_major: 14,
        memory_gib: 24,
        metal_available: true,
        chip: "M4".to_owned(),
    }
}

fn qwen_plan(bytes: &[u8], sha256: &str) -> ModelInstallPlan {
    let artifact = ArtifactPlan {
        artifact_id: "config".to_owned(),
        source_repository: "repo".to_owned(),
        source_model: "model".to_owned(),
        url: "http://127.0.0.1/config".to_owned(),
        revision: "revision".to_owned(),
        expected_bytes: bytes.len() as u64,
        expected_sha256: sha256.to_owned(),
        required_path: "config.json".to_owned(),
        install_mode: InstallMode::Direct,
        redirect_hosts: vec!["127.0.0.1".to_owned()],
        license_spdx: "Apache-2.0".to_owned(),
        provenance: "fixture".to_owned(),
    };
    ModelInstallPlan {
        model_id: "qwen3-asr-1.7b".to_owned(),
        provider: "qwen3_asr".to_owned(),
        manifest_version: "2".to_owned(),
        bundle_identity: "bundle".to_owned(),
        device: DeviceRequirement::AppleSiliconMetal {
            minimum_macos_major: 14,
            minimum_memory_gib: 24,
            chip: "M4".to_owned(),
        },
        qualification_policy: QualificationPolicy::RuntimeSmokeRequired,
        sherpa_runtime: None,
        install_contract: direct_contract(std::slice::from_ref(&artifact)),
        artifacts: vec![artifact],
    }
}

fn direct_contract(artifacts: &[ArtifactPlan]) -> InstallContract {
    let required_files = artifacts
        .iter()
        .map(|artifact| RequiredInstalledFile {
            path: artifact.required_path.clone(),
            bytes: artifact.expected_bytes,
            sha256: artifact.expected_sha256.clone(),
        })
        .collect::<Vec<_>>();
    InstallContract::Direct {
        max_written_file_bytes: required_files.iter().map(|file| file.bytes).max().unwrap(),
        max_total_written_bytes: required_files.iter().map(|file| file.bytes).sum(),
        required_files,
    }
}

fn config_source_identity(sha256: &str) -> String {
    format!("repo\nmodel\nrevision\nhttp://127.0.0.1/config\n{sha256}\nconfig.json")
}
