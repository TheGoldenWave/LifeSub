use std::collections::HashSet;
use std::sync::{Condvar, Mutex};

use sha2::{Digest, Sha256};

use crate::asr::job::{Clock, EnqueueJob, InputValidation, JobRepository};
use crate::asr::manifest::InstallConstraints;
use crate::asr::manifest::{ModelManifest, canonical_bundle_payload, model_registry, vad_manifest};
use crate::asr::model_lookup::ModelLookup;
use crate::asr::provider::ProviderSelection;
use crate::asr::settings::AsrSettings;
use crate::domain::{AsrErrorCode, AsrProviderKind};

pub const DEFAULT_VAD_MODEL_ID: &str = "silero-vad-2024-01-17";

/// Cheap enqueue-time dispatch validation. Implementations must not load model weights or hold an
/// execution lease, and must not panic; the claimed worker constructs the real provider. The
/// single-flight guard still releases during unwinding if an implementation violates this boundary.
pub trait EnqueueProviderFactory {
    fn validate_constructible(
        &self,
        settings: &AsrSettings,
        selection: &ProviderSelection,
    ) -> Result<(), AsrErrorCode>;
}

#[derive(Default)]
pub(crate) struct EnqueueReservations {
    active: Mutex<HashSet<String>>,
    changed: Condvar,
}

struct EnqueueReservation<'a> {
    reservations: &'a EnqueueReservations,
    fingerprint: String,
}

impl EnqueueReservations {
    fn reserve(&self, fingerprint: &str) -> EnqueueReservation<'_> {
        let mut active = self
            .active
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        while active.contains(fingerprint) {
            active = self
                .changed
                .wait(active)
                .unwrap_or_else(|error| error.into_inner());
        }
        active.insert(fingerprint.to_owned());
        EnqueueReservation {
            reservations: self,
            fingerprint: fingerprint.to_owned(),
        }
    }
}

impl Drop for EnqueueReservation<'_> {
    fn drop(&mut self) {
        let mut active = self
            .reservations
            .active
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        active.remove(&self.fingerprint);
        self.reservations.changed.notify_all();
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AsrEnqueueRequest {
    pub session_id: String,
    pub chunk_id: String,
    pub input_sha256: String,
    pub settings: AsrSettings,
    pub selection: ProviderSelection,
    pub vad_model_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AsrEnqueueOutcome {
    pub job_id: String,
    pub inserted: bool,
}

pub struct AsrService<'a, M, F, C> {
    jobs: JobRepository<'a, C>,
    reservations: &'a EnqueueReservations,
    models: &'a M,
    providers: &'a F,
}

impl<'a, M, F, C> AsrService<'a, M, F, C>
where
    M: ModelLookup,
    F: EnqueueProviderFactory,
    C: Clock,
{
    pub(crate) const fn new(
        jobs: JobRepository<'a, C>,
        reservations: &'a EnqueueReservations,
        models: &'a M,
        providers: &'a F,
    ) -> Self {
        Self {
            jobs,
            reservations,
            models,
            providers,
        }
    }

    pub fn enqueue(&self, request: AsrEnqueueRequest) -> Result<AsrEnqueueOutcome, AsrErrorCode> {
        request
            .settings
            .validate(self.models)
            .map_err(AsrErrorCode::from)?;
        validate_selection(&request.settings, &request.selection)?;
        let manifest = model_registry()
            .model(&request.settings.model_id)
            .ok_or(AsrErrorCode::ModelNotInstalled)?;
        if manifest.provider != request.settings.provider {
            return Err(AsrErrorCode::InvalidProviderParameter);
        }

        let vad = if request.settings.vad_enabled {
            let vad_id = request
                .vad_model_id
                .as_deref()
                .ok_or(AsrErrorCode::ModelCapabilityUnavailable)?;
            let capabilities = self
                .models
                .lookup(vad_id)
                .ok_or(AsrErrorCode::ModelCapabilityUnavailable)?;
            if !capabilities.executable || vad_id != DEFAULT_VAD_MODEL_ID {
                return Err(AsrErrorCode::ModelCapabilityUnavailable);
            }
            Some(vad_manifest())
        } else {
            None
        };

        let parameters_json = serde_json::to_string(&request.settings)
            .map_err(|_| AsrErrorCode::InvalidProviderParameter)?;
        let required_file_hashes_json = required_file_hashes(manifest)?;
        let model_source_json = model_source(manifest)?;
        let vad_required_file_hashes_json = vad
            .map(|manifest| required_file_hashes_from_bundle(&manifest.bundle))
            .transpose()?;
        let fingerprint = fingerprint(&request, manifest, &parameters_json, vad);
        match self
            .jobs
            .validate_input(
                &request.session_id,
                &request.chunk_id,
                &request.input_sha256,
            )
            .map_err(|_| AsrErrorCode::RecoveryRequired)?
        {
            InputValidation::Available => {}
            InputValidation::Missing | InputValidation::Unavailable => {
                return Err(AsrErrorCode::InputUnavailable);
            }
            InputValidation::IdentityMismatch => {
                return Err(AsrErrorCode::InputIntegrityFailed);
            }
        }
        let _reservation = self.reservations.reserve(&fingerprint);
        if let Some(job_id) = self
            .jobs
            .active_job_for_fingerprint(&fingerprint)
            .map_err(|_| AsrErrorCode::RecoveryRequired)?
        {
            return Ok(AsrEnqueueOutcome {
                job_id,
                inserted: false,
            });
        }
        self.providers
            .validate_constructible(&request.settings, &request.selection)?;
        let now = self.jobs.now();
        let outcome = self
            .jobs
            .commit_enqueue(EnqueueJob {
                id: format!("asr_{}", uuid::Uuid::new_v4().simple()),
                session_id: request.session_id,
                chunk_id: request.chunk_id,
                provider: provider_name(request.settings.provider).to_owned(),
                model_id: manifest.id.to_owned(),
                manifest_version: manifest.manifest_version.to_owned(),
                archive_sha256: manifest.bundle.identity_sha256.to_owned(),
                required_file_hashes_json,
                model_source_json,
                vad_model_id: vad.map(|value| value.id.to_owned()),
                vad_manifest_version: vad.map(|value| value.manifest_version.to_owned()),
                vad_archive_sha256: vad.map(|value| value.bundle.identity_sha256.to_owned()),
                vad_required_file_hashes_json,
                parameters_json,
                input_sha256: request.input_sha256,
                fingerprint,
                available_at: now.clone(),
                created_at: now,
            })
            .map_err(|error| match error {
                crate::asr::job::JobError::Input(code) => code,
                _ => AsrErrorCode::RecoveryRequired,
            })?;
        Ok(AsrEnqueueOutcome {
            job_id: outcome.job_id,
            inserted: outcome.inserted,
        })
    }
}

fn validate_selection(
    settings: &AsrSettings,
    selection: &ProviderSelection,
) -> Result<(), AsrErrorCode> {
    if selection.language != settings.language.as_str()
        || selection.num_threads != settings.num_threads
        || selection.options != settings.options
    {
        return Err(AsrErrorCode::InvalidProviderParameter);
    }
    Ok(())
}

fn required_file_hashes(manifest: &ModelManifest) -> Result<String, AsrErrorCode> {
    required_file_hashes_from_bundle(&manifest.bundle)
}

fn required_file_hashes_from_bundle(
    bundle: &crate::asr::manifest::ArtifactBundle,
) -> Result<String, AsrErrorCode> {
    let required_files = match bundle.install_constraints {
        InstallConstraints::Archive(constraints) => constraints.required_files,
        InstallConstraints::Direct(constraints) => constraints.required_files,
    };
    let mut required_files = required_files.iter().collect::<Vec<_>>();
    required_files.sort_by(|left, right| left.path.as_bytes().cmp(right.path.as_bytes()));
    let value = serde_json::Value::Array(
        required_files
            .into_iter()
            .map(|file| {
                serde_json::json!({
                    "path": file.path,
                    "bytes": file.bytes,
                    "sha256": file.sha256,
                })
            })
            .collect(),
    );
    serde_json_canonicalizer::to_string(&value).map_err(|_| AsrErrorCode::InvalidProviderParameter)
}

fn model_source(manifest: &ModelManifest) -> Result<String, AsrErrorCode> {
    let bundle: serde_json::Value = serde_json::from_str(
        &canonical_bundle_payload(manifest).map_err(|_| AsrErrorCode::InvalidProviderParameter)?,
    )
    .map_err(|_| AsrErrorCode::InvalidProviderParameter)?;
    let mut source = serde_json::json!({
        "bundle": bundle,
        "repository_url": manifest.source.repository_url,
        "model_card_url": manifest.source.model_card_url,
        "license_spdx": manifest.source.license_spdx,
        "provenance": manifest.source.provenance,
    });
    let canonical = serde_json_canonicalizer::to_string(&source)
        .map_err(|_| AsrErrorCode::InvalidProviderParameter)?;
    source["source_contract_sha256"] =
        serde_json::Value::String(hex::encode(Sha256::digest(canonical.as_bytes())));
    serde_json::to_string(&source).map_err(|_| AsrErrorCode::InvalidProviderParameter)
}

fn fingerprint(
    request: &AsrEnqueueRequest,
    manifest: &ModelManifest,
    parameters_json: &str,
    vad: Option<&crate::asr::manifest::VadManifest>,
) -> String {
    let payload = format!(
        "{}\0{}\0{}\0{}\0{}\0{}\0{}",
        request.session_id,
        request.chunk_id,
        request.input_sha256,
        manifest.id,
        manifest.bundle.identity_sha256,
        parameters_json,
        vad.map_or("", |value| value.bundle.identity_sha256),
    );
    hex::encode(Sha256::digest(payload.as_bytes()))
}

fn provider_name(provider: AsrProviderKind) -> &'static str {
    match provider {
        AsrProviderKind::SenseVoice => "sense_voice",
        AsrProviderKind::Whisper => "whisper",
        AsrProviderKind::Qwen3Asr => "qwen3_asr",
    }
}
