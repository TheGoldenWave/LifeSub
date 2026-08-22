use std::collections::HashSet;
use std::path::{Component, Path};

use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{OptionalExtension, params};
use sha2::{Digest, Sha256};

use crate::asr::job::{
    ChunkExecutionSnapshot, ClaimToken, ExecutionParametersSnapshot, ExecutionSnapshot,
    ExecutionStage, ModelExecutionSnapshot, RequiredFileSnapshot, SnapshotError,
    VadExecutionSnapshot,
};
use crate::asr::settings::{AsrProviderOptions, AsrSettings};
use crate::domain::{AsrProviderKind, AudioSource};

use super::Catalog;

const SHA256_HEX_LENGTH: usize = 64;

struct SnapshotRow {
    owned: bool,
    expected_stage: bool,
    cancelled: bool,
    lease_expires_at: Option<String>,
    input_available: bool,
    session_id: String,
    chunk_id: String,
    chunk_source: String,
    chunk_path: String,
    chunk_sha256: String,
    chunk_byte_length: i64,
    session_offset_ms: i64,
    duration_ms: Option<i64>,
    provider: String,
    model_id: String,
    manifest_version: String,
    bundle_identity: String,
    required_file_hashes_json: String,
    model_source_json: String,
    parameters_json: String,
    vad_model_id: Option<String>,
    vad_manifest_version: Option<String>,
    vad_bundle_identity: Option<String>,
    vad_required_file_hashes_json: Option<String>,
}

impl Catalog {
    pub(super) fn load_execution_snapshot(
        &self,
        token: &ClaimToken,
        stage: ExecutionStage,
        now: &str,
    ) -> Result<ExecutionSnapshot, SnapshotError> {
        let row = self
            .connection
            .lock()
            .unwrap()
            .query_row(
                "SELECT
                   COALESCE(j.claimed_by = ?2 AND j.claim_generation = ?3, 0),
                   j.state = ?4,
                   j.cancel_requested_at IS NOT NULL,
                   j.lease_expires_at,
                   j.session_id = c.session_id AND c.integrity_state = 'available'
                     AND j.input_sha256 = c.sha256,
                   j.session_id, j.chunk_id, c.source, c.path, c.sha256, c.byte_length,
                   c.session_offset_ms, c.duration_ms, j.provider, j.model_id,
                   j.manifest_version, j.archive_sha256, j.required_file_hashes_json,
                   j.model_source_json, j.parameters_json, j.vad_model_id,
                   j.vad_manifest_version, j.vad_archive_sha256,
                   j.vad_required_file_hashes_json
                 FROM asr_jobs j
                 JOIN chunks c ON c.id = j.chunk_id
                 WHERE j.id = ?1",
                params![
                    token.job_id,
                    token.claimed_by,
                    token.claim_generation,
                    stage.as_str(),
                ],
                |row| {
                    Ok(SnapshotRow {
                        owned: row.get(0)?,
                        expected_stage: row.get(1)?,
                        cancelled: row.get(2)?,
                        lease_expires_at: row.get(3)?,
                        input_available: row.get(4)?,
                        session_id: row.get(5)?,
                        chunk_id: row.get(6)?,
                        chunk_source: row.get(7)?,
                        chunk_path: row.get(8)?,
                        chunk_sha256: row.get(9)?,
                        chunk_byte_length: row.get(10)?,
                        session_offset_ms: row.get(11)?,
                        duration_ms: row.get(12)?,
                        provider: row.get(13)?,
                        model_id: row.get(14)?,
                        manifest_version: row.get(15)?,
                        bundle_identity: row.get(16)?,
                        required_file_hashes_json: row.get(17)?,
                        model_source_json: row.get(18)?,
                        parameters_json: row.get(19)?,
                        vad_model_id: row.get(20)?,
                        vad_manifest_version: row.get(21)?,
                        vad_bundle_identity: row.get(22)?,
                        vad_required_file_hashes_json: row.get(23)?,
                    })
                },
            )
            .optional()
            .map_err(SnapshotError::Catalog)?
            .ok_or(SnapshotError::OwnershipLost)?;
        row.into_snapshot(&token.job_id, now)
    }
}

impl SnapshotRow {
    fn into_snapshot(self, job_id: &str, now: &str) -> Result<ExecutionSnapshot, SnapshotError> {
        if !self.owned {
            return Err(SnapshotError::OwnershipLost);
        }
        if !self.expected_stage {
            return Err(SnapshotError::StageMismatch);
        }
        if self.cancelled {
            return Err(SnapshotError::CancelRequested);
        }
        if !lease_is_current(self.lease_expires_at.as_deref(), now) {
            return Err(SnapshotError::LeaseExpired);
        }
        if !self.input_available {
            return Err(SnapshotError::InputUnavailable);
        }
        validate_nonempty(job_id, "job.id")?;
        validate_nonempty(&self.session_id, "job.session_id")?;
        validate_nonempty(&self.chunk_id, "chunk.id")?;
        validate_relative_path(&self.chunk_path, "chunk.path")?;
        validate_sha256(&self.chunk_sha256, "chunk.sha256")?;
        let byte_length = u64::try_from(self.chunk_byte_length)
            .ok()
            .filter(|length| *length > 0)
            .ok_or(SnapshotError::InvalidSnapshot("chunk.byte_length"))?;
        if self.session_offset_ms < 0 {
            return Err(SnapshotError::InvalidSnapshot("chunk.session_offset_ms"));
        }
        if self.duration_ms.is_some_and(|duration| duration <= 0) {
            return Err(SnapshotError::InvalidSnapshot("chunk.duration_ms"));
        }
        let source =
            serde_json::from_value::<AudioSource>(serde_json::Value::String(self.chunk_source))
                .map_err(|_| SnapshotError::InvalidSnapshot("chunk.source"))?;
        let provider =
            serde_json::from_value::<AsrProviderKind>(serde_json::Value::String(self.provider))
                .map_err(|_| SnapshotError::InvalidSnapshot("job.provider"))?;
        validate_nonempty(&self.model_id, "job.model_id")?;
        validate_nonempty(&self.manifest_version, "job.manifest_version")?;
        validate_sha256(&self.bundle_identity, "job.bundle_identity")?;
        let required_files =
            required_files(&self.required_file_hashes_json, "required_file_hashes_json")?;
        let model_source = validate_model_source(
            &self.model_source_json,
            provider,
            &self.model_id,
            &self.manifest_version,
            &self.bundle_identity,
            &required_files,
        )?;
        let parameters_value: serde_json::Value = serde_json::from_str(&self.parameters_json)
            .map_err(|_| SnapshotError::InvalidSnapshot("parameters_json"))?;
        let parameters_object = parameters_value
            .as_object()
            .ok_or(SnapshotError::InvalidSnapshot("parameters_json"))?;
        validate_exact_keys(
            parameters_object,
            &[
                "provider",
                "model_id",
                "language",
                "num_threads",
                "vad_enabled",
                "auto_transcribe_imports",
                "options",
            ],
            "parameters_json",
        )?;
        let settings: AsrSettings = serde_json::from_value(parameters_value)
            .map_err(|_| SnapshotError::InvalidSnapshot("parameters_json"))?;
        if settings.provider != provider
            || settings.model_id != self.model_id
            || settings.num_threads == 0
            || usize::from(settings.num_threads) > logical_cpu_count()
            || !language_supported(&self.model_id, settings.language.as_str())
            || !options_match(provider, &settings.options)
            || !options_schema_valid(&self.parameters_json, provider)
        {
            return Err(SnapshotError::InvalidSnapshot("parameters_json"));
        }
        let vad = vad_snapshot(
            self.vad_model_id,
            self.vad_manifest_version,
            self.vad_bundle_identity,
            self.vad_required_file_hashes_json,
        )?;
        if settings.vad_enabled != vad.is_some() {
            return Err(SnapshotError::InvalidSnapshot("parameters_json"));
        }
        Ok(ExecutionSnapshot {
            job_id: job_id.to_owned(),
            session_id: self.session_id,
            chunk: ChunkExecutionSnapshot {
                id: self.chunk_id,
                source,
                relative_path: self.chunk_path,
                sha256: self.chunk_sha256,
                byte_length,
                session_offset_ms: self.session_offset_ms,
                duration_ms: self.duration_ms,
            },
            model: ModelExecutionSnapshot {
                provider,
                model_id: self.model_id,
                manifest_version: self.manifest_version,
                bundle_identity: self.bundle_identity,
                required_file_hashes_json: self.required_file_hashes_json,
                required_files,
                model_source_json: self.model_source_json,
                source: model_source,
            },
            parameters: ExecutionParametersSnapshot::from_settings(self.parameters_json, settings),
            vad,
        })
    }
}

fn vad_snapshot(
    model_id: Option<String>,
    manifest_version: Option<String>,
    bundle_identity: Option<String>,
    required_file_hashes_json: Option<String>,
) -> Result<Option<VadExecutionSnapshot>, SnapshotError> {
    match (
        model_id,
        manifest_version,
        bundle_identity,
        required_file_hashes_json,
    ) {
        (None, None, None, None) => Ok(None),
        (Some(model_id), Some(manifest_version), Some(bundle_identity), Some(json)) => {
            validate_nonempty(&model_id, "vad.model_id")?;
            validate_nonempty(&manifest_version, "vad.manifest_version")?;
            validate_sha256(&bundle_identity, "vad.bundle_identity")?;
            let required_files = required_files(&json, "vad_required_file_hashes_json")?;
            if required_files.len() != 1
                || required_files[0].sha256 != bundle_identity
                || required_files[0].path != "silero_vad.onnx"
            {
                return Err(SnapshotError::InvalidSnapshot(
                    "vad_required_file_hashes_json",
                ));
            }
            Ok(Some(VadExecutionSnapshot {
                model_id,
                manifest_version,
                bundle_identity,
                required_file_hashes_json: json,
                required_files,
            }))
        }
        _ => Err(SnapshotError::InvalidSnapshot("vad_identity")),
    }
}

fn required_files(
    json: &str,
    field: &'static str,
) -> Result<Vec<RequiredFileSnapshot>, SnapshotError> {
    let files: Vec<RequiredFileSnapshot> =
        serde_json::from_str(json).map_err(|_| SnapshotError::InvalidSnapshot(field))?;
    if files.is_empty() {
        return Err(SnapshotError::InvalidSnapshot(field));
    }
    let mut paths = HashSet::with_capacity(files.len());
    let mut total = 0_u64;
    for file in &files {
        validate_relative_path(&file.path, field)?;
        validate_sha256(&file.sha256, field)?;
        if file.byte_length == 0 || !paths.insert(file.path.as_str()) {
            return Err(SnapshotError::InvalidSnapshot(field));
        }
        total = total
            .checked_add(file.byte_length)
            .ok_or(SnapshotError::InvalidSnapshot(field))?;
    }
    Ok(files)
}

fn options_match(provider: AsrProviderKind, options: &AsrProviderOptions) -> bool {
    matches!(
        (provider, options),
        (
            AsrProviderKind::SenseVoice,
            AsrProviderOptions::SenseVoice { .. }
        ) | (AsrProviderKind::Whisper, AsrProviderOptions::Whisper { .. })
            | (AsrProviderKind::Qwen3Asr, AsrProviderOptions::Qwen3Asr)
    )
}

fn options_schema_valid(json: &str, provider: AsrProviderKind) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return false;
    };
    let Some(options) = value.get("options").and_then(serde_json::Value::as_object) else {
        return false;
    };
    let expected: &[&str] = match provider {
        AsrProviderKind::SenseVoice => &["provider", "use_itn"],
        AsrProviderKind::Whisper => &["provider", "task"],
        AsrProviderKind::Qwen3Asr => &["provider"],
    };
    exact_keys(options, expected)
}

fn validate_model_source(
    json: &str,
    provider: AsrProviderKind,
    model_id: &str,
    manifest_version: &str,
    bundle_identity: &str,
    required_files: &[RequiredFileSnapshot],
) -> Result<serde_json::Value, SnapshotError> {
    let value: serde_json::Value = serde_json::from_str(json).map_err(|_| invalid_source())?;
    let root = value.as_object().ok_or_else(invalid_source)?;
    validate_exact_keys(
        root,
        &[
            "bundle",
            "repository_url",
            "model_card_url",
            "license_spdx",
            "provenance",
            "source_contract_sha256",
        ],
        "model_source_json",
    )?;
    validate_https_string(root.get("repository_url"), "model_source_json")?;
    validate_https_string(root.get("model_card_url"), "model_source_json")?;
    validate_text(root.get("license_spdx"), "model_source_json")?;
    validate_text(root.get("provenance"), "model_source_json")?;
    let source_contract_sha256 = root
        .get("source_contract_sha256")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(invalid_source)?;
    validate_sha256(source_contract_sha256, "model_source_json")?;
    let mut contract = root.clone();
    contract.remove("source_contract_sha256");
    let canonical_contract =
        serde_json_canonicalizer::to_string(&serde_json::Value::Object(contract))
            .map_err(|_| invalid_source())?;
    if hex::encode(Sha256::digest(canonical_contract.as_bytes())) != source_contract_sha256 {
        return Err(invalid_source());
    }
    let bundle = root
        .get("bundle")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(invalid_source)?;
    validate_exact_keys(
        bundle,
        &[
            "artifacts",
            "compatibility_contract",
            "device_requirement",
            "manifest_version",
            "model_id",
            "provider",
            "qualification_policy",
            "required_paths",
            "runtime_requirement",
            "schema",
        ],
        "model_source_json",
    )?;
    if string_field(bundle, "schema")? != "lifesub.model-bundle.v1"
        || string_field(bundle, "model_id")? != model_id
        || string_field(bundle, "manifest_version")? != manifest_version
        || string_field(bundle, "provider")? != provider_name(provider)
    {
        return Err(invalid_source());
    }
    validate_compatibility(bundle.get("compatibility_contract"), model_id)?;
    validate_device(bundle.get("device_requirement"))?;
    validate_runtime(bundle.get("runtime_requirement"))?;
    if !matches!(
        string_field(bundle, "qualification_policy")?,
        "structural_with_pinned_runtime" | "runtime_smoke_required"
    ) {
        return Err(invalid_source());
    }
    let required_paths = string_array(bundle.get("required_paths"))?;
    let artifacts = validate_artifacts(bundle.get("artifacts"))?;
    if required_paths.is_empty() || artifacts.is_empty() {
        return Err(invalid_source());
    }
    validate_required_files_against_bundle(required_files, &required_paths, &artifacts)?;
    let canonical = serde_json_canonicalizer::to_string(&serde_json::Value::Object(bundle.clone()))
        .map_err(|_| invalid_source())?;
    let calculated = if artifacts.len() == 1 {
        artifacts[0].sha256.clone()
    } else {
        hex::encode(Sha256::digest(canonical.as_bytes()))
    };
    if calculated != bundle_identity {
        return Err(invalid_source());
    }
    Ok(value)
}

#[derive(Clone)]
struct ArtifactIdentity {
    required_path: String,
    byte_length: u64,
    sha256: String,
    install_mode: String,
}

fn validate_artifacts(
    value: Option<&serde_json::Value>,
) -> Result<Vec<ArtifactIdentity>, SnapshotError> {
    let values = value
        .and_then(serde_json::Value::as_array)
        .ok_or_else(invalid_source)?;
    let mut ids = HashSet::new();
    let mut paths = HashSet::new();
    let mut artifacts = Vec::with_capacity(values.len());
    for value in values {
        let artifact = value.as_object().ok_or_else(invalid_source)?;
        validate_exact_keys(
            artifact,
            &[
                "artifact_id",
                "bytes",
                "install_mode",
                "license_spdx",
                "provenance",
                "redirect_hosts",
                "required",
                "required_path",
                "resolved_url",
                "revision",
                "sha256",
                "source_endpoint",
                "source_model",
                "source_repository",
            ],
            "model_source_json",
        )?;
        let artifact_id = string_field(artifact, "artifact_id")?;
        let required_path = string_field(artifact, "required_path")?;
        let source_repository = string_field(artifact, "source_repository")?;
        let source_endpoint = string_field(artifact, "source_endpoint")?;
        let resolved_url = string_field(artifact, "resolved_url")?;
        let revision = string_field(artifact, "revision")?;
        let sha256 = string_field(artifact, "sha256")?;
        let install_mode = string_field(artifact, "install_mode")?;
        validate_text(artifact.get("source_model"), "model_source_json")?;
        validate_text(artifact.get("license_spdx"), "model_source_json")?;
        validate_text(artifact.get("provenance"), "model_source_json")?;
        validate_https(source_repository)?;
        validate_https(source_endpoint)?;
        validate_https(resolved_url)?;
        validate_sha256(sha256, "model_source_json")?;
        validate_relative_path(required_path, "model_source_json")?;
        if artifact
            .get("required")
            .and_then(serde_json::Value::as_bool)
            != Some(true)
            || !matches!(install_mode, "direct" | "extract_tar_bz2")
            || (!revision.starts_with("github-release-asset:") && !resolved_url.contains(revision))
            || !ids.insert(artifact_id)
            || !paths.insert(required_path)
        {
            return Err(invalid_source());
        }
        let byte_length = artifact
            .get("bytes")
            .and_then(serde_json::Value::as_u64)
            .filter(|bytes| *bytes > 0)
            .ok_or_else(invalid_source)?;
        let redirect_hosts = string_array(artifact.get("redirect_hosts"))?;
        let resolved_host = reqwest::Url::parse(resolved_url)
            .ok()
            .and_then(|url| url.host_str().map(str::to_owned))
            .ok_or_else(invalid_source)?;
        if redirect_hosts.is_empty()
            || !redirect_hosts.iter().any(|host| host == &resolved_host)
            || redirect_hosts.windows(2).any(|pair| pair[0] >= pair[1])
            || redirect_hosts.iter().any(|host| !valid_hostname(host))
        {
            return Err(invalid_source());
        }
        artifacts.push(ArtifactIdentity {
            required_path: required_path.to_owned(),
            byte_length,
            sha256: sha256.to_owned(),
            install_mode: install_mode.to_owned(),
        });
    }
    Ok(artifacts)
}

fn validate_required_files_against_bundle(
    files: &[RequiredFileSnapshot],
    required_paths: &[String],
    artifacts: &[ArtifactIdentity],
) -> Result<(), SnapshotError> {
    if artifacts
        .iter()
        .all(|artifact| artifact.install_mode == "direct")
    {
        if files.len() != artifacts.len()
            || files.iter().any(|file| {
                !artifacts.iter().any(|artifact| {
                    artifact.required_path == file.path
                        && artifact.byte_length == file.byte_length
                        && artifact.sha256 == file.sha256
                })
            })
        {
            return Err(invalid_source());
        }
    } else {
        if artifacts
            .iter()
            .any(|artifact| artifact.install_mode != "extract_tar_bz2")
            || files.len() != required_paths.len()
            || files
                .iter()
                .any(|file| !required_paths.iter().any(|path| path == &file.path))
            || required_paths
                .iter()
                .any(|path| files.iter().filter(|file| &file.path == path).count() != 1)
        {
            return Err(invalid_source());
        }
    }
    Ok(())
}

fn validate_compatibility(
    value: Option<&serde_json::Value>,
    model_id: &str,
) -> Result<(), SnapshotError> {
    let object = value
        .and_then(serde_json::Value::as_object)
        .ok_or_else(invalid_source)?;
    if model_id == "qwen3-asr-1.7b" {
        validate_exact_keys(
            object,
            &["config_shape", "conversion", "tokenizer_source"],
            "model_source_json",
        )?;
        if string_field(object, "config_shape")? != "top_level_thinker_config"
            || string_field(object, "conversion")? != "none"
            || string_field(object, "tokenizer_source")? != "official_qwen3_asr_1.7b_hf"
        {
            return Err(invalid_source());
        }
    } else {
        validate_exact_keys(object, &["archive_contract"], "model_source_json")?;
        if string_field(object, "archive_contract")? != "sherpa_release_required_paths_v1" {
            return Err(invalid_source());
        }
    }
    Ok(())
}

fn validate_device(value: Option<&serde_json::Value>) -> Result<(), SnapshotError> {
    let object = value
        .and_then(serde_json::Value::as_object)
        .ok_or_else(invalid_source)?;
    match string_field(object, "kind")? {
        "any_desktop" => validate_exact_keys(object, &["kind"], "model_source_json"),
        "apple_silicon_metal" => {
            validate_exact_keys(
                object,
                &["kind", "minimum_macos_major", "minimum_memory_gib"],
                "model_source_json",
            )?;
            if object
                .get("minimum_macos_major")
                .and_then(serde_json::Value::as_u64)
                .is_none_or(|value| value == 0 || value > u64::from(u16::MAX))
                || object
                    .get("minimum_memory_gib")
                    .and_then(serde_json::Value::as_u64)
                    .is_none_or(|value| value == 0 || value > u64::from(u16::MAX))
            {
                return Err(invalid_source());
            }
            Ok(())
        }
        _ => Err(invalid_source()),
    }
}

fn validate_runtime(value: Option<&serde_json::Value>) -> Result<(), SnapshotError> {
    let object = value
        .and_then(serde_json::Value::as_object)
        .ok_or_else(invalid_source)?;
    let fields = if object.contains_key("backend") {
        &[
            "backend",
            "cargo_feature",
            "crate",
            "git_commit",
            "git_url",
            "target_arch",
            "target_os",
            "version",
        ][..]
    } else {
        &[
            "build_id",
            "cargo_feature",
            "crate",
            "git_commit",
            "native_archive_sha256",
            "version",
        ][..]
    };
    validate_exact_keys(object, fields, "model_source_json")?;
    for field in fields {
        validate_text(object.get(*field), "model_source_json")?;
    }
    if let Some(url) = object.get("git_url").and_then(serde_json::Value::as_str) {
        validate_https(url)?;
    }
    if let Some(sha256) = object
        .get("native_archive_sha256")
        .and_then(serde_json::Value::as_str)
    {
        validate_sha256(sha256, "model_source_json")?;
    }
    Ok(())
}

fn language_supported(model_id: &str, language: &str) -> bool {
    match model_id {
        "sense-voice-small-int8-2024-07-17" | "whisper-tiny" | "whisper-base" | "whisper-small" => {
            ["auto", "zh", "en", "ja", "ko", "yue"].contains(&language)
        }
        "qwen3-asr-0.6b-int8-2026-03-25" => language == "auto",
        "qwen3-asr-1.7b" => [
            "auto", "zh", "en", "yue", "ar", "de", "fr", "es", "pt", "id", "it", "ko", "ru", "th",
            "vi", "ja", "tr", "hi", "ms", "nl", "sv", "da", "fi", "pl", "cs", "fil", "fa", "el",
            "hu", "mk", "ro",
        ]
        .contains(&language),
        _ => false,
    }
}

fn logical_cpu_count() -> usize {
    std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1)
}

fn lease_is_current(lease: Option<&str>, now: &str) -> bool {
    let Some(lease) = lease else {
        return false;
    };
    let Ok(lease_time) = DateTime::parse_from_rfc3339(lease) else {
        return false;
    };
    let Ok(now_time) = DateTime::parse_from_rfc3339(now) else {
        return false;
    };
    lease
        == lease_time
            .with_timezone(&Utc)
            .to_rfc3339_opts(SecondsFormat::Millis, true)
        && now
            == now_time
                .with_timezone(&Utc)
                .to_rfc3339_opts(SecondsFormat::Millis, true)
        && lease_time > now_time
}

fn validate_exact_keys(
    object: &serde_json::Map<String, serde_json::Value>,
    expected: &[&str],
    field: &'static str,
) -> Result<(), SnapshotError> {
    if !exact_keys(object, expected) {
        return Err(SnapshotError::InvalidSnapshot(field));
    }
    Ok(())
}

fn exact_keys(object: &serde_json::Map<String, serde_json::Value>, expected: &[&str]) -> bool {
    object.len() == expected.len() && expected.iter().all(|key| object.contains_key(*key))
}

fn string_field<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    field: &str,
) -> Result<&'a str, SnapshotError> {
    let value = object
        .get(field)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(invalid_source)?;
    validate_text(
        Some(&serde_json::Value::String(value.to_owned())),
        "model_source_json",
    )?;
    Ok(value)
}

fn string_array(value: Option<&serde_json::Value>) -> Result<Vec<String>, SnapshotError> {
    let values = value
        .and_then(serde_json::Value::as_array)
        .ok_or_else(invalid_source)?;
    let mut seen = HashSet::new();
    values
        .iter()
        .map(|value| {
            let value = value.as_str().ok_or_else(invalid_source)?;
            validate_text(
                Some(&serde_json::Value::String(value.to_owned())),
                "model_source_json",
            )?;
            if !seen.insert(value) {
                return Err(invalid_source());
            }
            Ok(value.to_owned())
        })
        .collect()
}

fn validate_text(
    value: Option<&serde_json::Value>,
    field: &'static str,
) -> Result<(), SnapshotError> {
    let value = value
        .and_then(serde_json::Value::as_str)
        .ok_or(SnapshotError::InvalidSnapshot(field))?;
    if value.trim().is_empty()
        || value.contains("TODO")
        || value.contains("PLACEHOLDER")
        || value == "null"
    {
        return Err(SnapshotError::InvalidSnapshot(field));
    }
    Ok(())
}

fn validate_https_string(
    value: Option<&serde_json::Value>,
    field: &'static str,
) -> Result<(), SnapshotError> {
    let value = value
        .and_then(serde_json::Value::as_str)
        .ok_or(SnapshotError::InvalidSnapshot(field))?;
    validate_https(value)
}

fn validate_https(value: &str) -> Result<(), SnapshotError> {
    let authority = value
        .strip_prefix("https://")
        .and_then(|rest| rest.split('/').next())
        .ok_or_else(invalid_source)?;
    let url = reqwest::Url::parse(value).map_err(|_| invalid_source())?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || url.port_or_known_default() != Some(443)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || authority.bytes().any(|byte| byte.is_ascii_uppercase())
    {
        return Err(invalid_source());
    }
    Ok(())
}

fn valid_hostname(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
}

fn provider_name(provider: AsrProviderKind) -> &'static str {
    match provider {
        AsrProviderKind::SenseVoice => "sense_voice",
        AsrProviderKind::Whisper => "whisper",
        AsrProviderKind::Qwen3Asr => "qwen3_asr",
    }
}

fn invalid_source() -> SnapshotError {
    SnapshotError::InvalidSnapshot("model_source_json")
}

fn validate_nonempty(value: &str, field: &'static str) -> Result<(), SnapshotError> {
    if value.trim().is_empty() {
        return Err(SnapshotError::InvalidSnapshot(field));
    }
    Ok(())
}

fn validate_sha256(value: &str, field: &'static str) -> Result<(), SnapshotError> {
    if value.len() != SHA256_HEX_LENGTH
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || value == "0000000000000000000000000000000000000000000000000000000000000000"
    {
        return Err(SnapshotError::InvalidSnapshot(field));
    }
    Ok(())
}

fn validate_relative_path(value: &str, field: &'static str) -> Result<(), SnapshotError> {
    let path = Path::new(value);
    if value.is_empty()
        || path.is_absolute()
        || !path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(SnapshotError::InvalidSnapshot(field));
    }
    Ok(())
}
