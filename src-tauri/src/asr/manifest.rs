use std::collections::HashSet;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::asr::model_lookup::{ModelCapabilities, ModelLookup};
use crate::domain::AsrProviderKind;

const SHERPA_REPOSITORY: &str = "https://github.com/k2-fsa/sherpa-onnx";
const SHERPA_RELEASE_API: &str =
    "https://api.github.com/repos/k2-fsa/sherpa-onnx/releases/tags/asr-models";
const GITHUB_REDIRECT_HOSTS: &[&str] = &[
    "api.github.com",
    "github.com",
    "release-assets.githubusercontent.com",
];
const HUGGING_FACE_REDIRECT_HOSTS: &[&str] = &[
    "cas-bridge.xethub.hf.co",
    "cdn-lfs-us-1.hf.co",
    "huggingface.co",
];
const MODELSCOPE_DIRECT_HOSTS: &[&str] = &["www.modelscope.cn"];
const MODELSCOPE_LFS_HOSTS: &[&str] = &["cdn-lfs-cn-1.modelscope.cn", "www.modelscope.cn"];
const MODELSCOPE_REVISION: &str = "d69410f1c275f2b0fa60cbb9960edfcdb0ae0aec";
const HUGGING_FACE_REVISION: &str = "bcd2b5b7f32b480ab5790554cfa8347f246a14f3";
const QWEN17_IDENTITY: &str = "8a5c16d08be3c49e638689b6438a9a3be9d5d732e49f904d2c0666d5229c995a";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactInstallMode {
    Direct,
    ExtractTarBz2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactFile {
    pub artifact_id: &'static str,
    pub source_repository: &'static str,
    pub source_model: &'static str,
    pub source_endpoint: &'static str,
    pub resolved_url: &'static str,
    pub revision: &'static str,
    pub bytes: u64,
    pub sha256: &'static str,
    pub required_path: &'static str,
    pub required: bool,
    pub install_mode: ArtifactInstallMode,
    pub license_spdx: &'static str,
    pub provenance: &'static str,
    pub redirect_hosts: &'static [&'static str],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactBundle {
    pub artifacts: &'static [ArtifactFile],
    pub required_paths: &'static [&'static str],
    pub identity_sha256: &'static str,
    pub install_constraints: InstallConstraints,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RequiredInstallFile {
    pub path: &'static str,
    pub bytes: u64,
    pub sha256: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchiveInstallConstraints {
    pub max_scanned_entries: u64,
    pub max_written_file_bytes: u64,
    pub max_total_written_bytes: u64,
    pub required_files: &'static [RequiredInstallFile],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectInstallConstraints {
    pub max_written_file_bytes: u64,
    pub max_total_written_bytes: u64,
    pub required_files: &'static [RequiredInstallFile],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstallConstraints {
    Archive(ArchiveInstallConstraints),
    Direct(DirectInstallConstraints),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeRequirement {
    SherpaOnnx {
        crate_version: &'static str,
        git_commit: &'static str,
        native_archive_sha256: &'static str,
        build_id: &'static str,
        cargo_feature: &'static str,
    },
    QwenCandleMetal {
        crate_name: &'static str,
        crate_version: &'static str,
        git_url: &'static str,
        git_commit: &'static str,
        cargo_feature: &'static str,
        target_os: &'static str,
        target_arch: &'static str,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceRequirement {
    AnyDesktop,
    AppleSiliconMetal {
        minimum_macos_major: u16,
        minimum_memory_gib: u16,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QualificationPolicy {
    StructuralWithPinnedRuntime,
    RuntimeSmokeRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModelSource {
    pub repository_url: &'static str,
    pub model_card_url: &'static str,
    pub license_spdx: &'static str,
    pub provenance: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModelManifest {
    pub id: &'static str,
    pub manifest_version: &'static str,
    pub display_name: &'static str,
    pub provider: AsrProviderKind,
    pub supported_languages: &'static [&'static str],
    pub bundle: ArtifactBundle,
    pub runtime: RuntimeRequirement,
    pub device: DeviceRequirement,
    pub qualification_policy: QualificationPolicy,
    pub source: ModelSource,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VadManifest {
    pub id: &'static str,
    pub manifest_version: &'static str,
    pub bundle: ArtifactBundle,
    pub runtime: RuntimeRequirement,
    pub qualification_policy: QualificationPolicy,
    pub source: ModelSource,
    pub sherpa_onnx_version: &'static str,
    pub sherpa_onnx_commit: &'static str,
    pub silero_config_source_header: &'static str,
    pub vad_config_source_header: &'static str,
    pub threshold: f32,
    pub min_silence_duration_seconds: f32,
    pub min_speech_duration_seconds: f32,
    pub max_speech_duration_seconds: f32,
    pub window_size_samples: i32,
    pub sample_rate_hz: i32,
    pub num_threads: i32,
    pub provider: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModelRegistry {
    models: &'static [ModelManifest],
}

impl ModelRegistry {
    pub const fn new(models: &'static [ModelManifest]) -> Self {
        Self { models }
    }

    pub const fn models(&self) -> &'static [ModelManifest] {
        self.models
    }

    pub fn model(&self, model_id: &str) -> Option<&'static ModelManifest> {
        self.models.iter().find(|model| model.id == model_id)
    }
}

impl ModelLookup for ModelRegistry {
    fn lookup(&self, model_id: &str) -> Option<ModelCapabilities> {
        let model = self.model(model_id)?;
        Some(
            ModelCapabilities::new(
                model.provider,
                model.supported_languages,
                true,
                false,
                false,
            )
            .with_reason_code("model_context_required"),
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegistryValidationError {
    EmptyRegistry,
    DuplicateModelId,
    InvalidManifestField,
    InvalidArtifact,
    DuplicateArtifactId,
    InvalidRequiredPath,
    OverlappingRequiredPath,
    InvalidBundleIdentity,
    InvalidInstallConstraints,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigCompatibilityError {
    InvalidJson,
    MissingThinkerConfig,
}

pub fn model_registry() -> &'static ModelRegistry {
    &REGISTRY
}

pub fn vad_manifest() -> &'static VadManifest {
    &VAD
}

pub fn validate_qwen_config_shape(bytes: &[u8]) -> Result<(), ConfigCompatibilityError> {
    let value: Value =
        serde_json::from_slice(bytes).map_err(|_| ConfigCompatibilityError::InvalidJson)?;
    match value.get("thinker_config") {
        Some(Value::Object(_)) => Ok(()),
        _ => Err(ConfigCompatibilityError::MissingThinkerConfig),
    }
}

pub fn canonical_bundle_payload(model: &ModelManifest) -> serde_json::Result<String> {
    let mut artifacts = model.bundle.artifacts.iter().collect::<Vec<_>>();
    artifacts.sort_by(|left, right| {
        left.artifact_id
            .as_bytes()
            .cmp(right.artifact_id.as_bytes())
    });
    let artifacts = artifacts
        .into_iter()
        .map(artifact_payload)
        .collect::<Vec<_>>();
    let value = json!({
        "artifacts": artifacts,
        "compatibility_contract": compatibility_payload(model),
        "device_requirement": device_payload(model.device),
        "manifest_version": model.manifest_version,
        "model_id": model.id,
        "provider": provider_name(model.provider),
        "qualification_policy": qualification_name(model.qualification_policy),
        "required_paths": model.bundle.required_paths,
        "runtime_requirement": runtime_payload(model.runtime),
        "schema": "lifesub.model-bundle.v1",
    });
    serde_json_canonicalizer::to_string(&value)
}

pub fn validate_registry(
    registry: &ModelRegistry,
    vad: &VadManifest,
) -> Result<(), RegistryValidationError> {
    if registry.models.is_empty() {
        return Err(RegistryValidationError::EmptyRegistry);
    }
    let mut model_ids = HashSet::new();
    for model in registry.models {
        if !model_ids.insert(model.id) {
            return Err(RegistryValidationError::DuplicateModelId);
        }
        validate_model_contract(model)?;
        if model.bundle.artifacts.len() > 1 {
            let canonical = canonical_bundle_payload(model)
                .map_err(|_| RegistryValidationError::InvalidBundleIdentity)?;
            if sha256_hex(canonical.as_bytes()) != model.bundle.identity_sha256 {
                return Err(RegistryValidationError::InvalidBundleIdentity);
            }
        } else if model.bundle.identity_sha256 != model.bundle.artifacts[0].sha256 {
            return Err(RegistryValidationError::InvalidBundleIdentity);
        }
        let expected = MODELS
            .iter()
            .find(|expected| expected.id == model.id)
            .ok_or(RegistryValidationError::InvalidManifestField)?;
        if expected != model {
            return Err(RegistryValidationError::InvalidManifestField);
        }
    }
    validate_vad_contract(vad)
}

fn validate_model_contract(model: &ModelManifest) -> Result<(), RegistryValidationError> {
    if any_blank(&[
        model.id,
        model.manifest_version,
        model.display_name,
        model.source.repository_url,
        model.source.model_card_url,
        model.source.license_spdx,
        model.source.provenance,
    ]) || !valid_https_url(model.source.repository_url)
        || !valid_https_url(model.source.model_card_url)
        || !valid_languages(model.supported_languages)
    {
        return Err(RegistryValidationError::InvalidManifestField);
    }
    let expected = if model.id == "qwen3-asr-1.7b" {
        (
            QWEN_RUNTIME,
            DeviceRequirement::AppleSiliconMetal {
                minimum_macos_major: 14,
                minimum_memory_gib: 24,
            },
            QualificationPolicy::RuntimeSmokeRequired,
        )
    } else {
        (
            SHERPA_RUNTIME,
            DeviceRequirement::AnyDesktop,
            QualificationPolicy::StructuralWithPinnedRuntime,
        )
    };
    if (model.runtime, model.device, model.qualification_policy) != expected {
        return Err(RegistryValidationError::InvalidManifestField);
    }
    if expected_required_paths(model.id) != Some(model.bundle.required_paths) {
        return Err(RegistryValidationError::InvalidRequiredPath);
    }
    validate_bundle(&model.bundle)
}

fn validate_vad_contract(vad: &VadManifest) -> Result<(), RegistryValidationError> {
    if vad.id != "silero-vad-2024-01-17"
        || vad.manifest_version != "1"
        || any_blank(&[
            vad.id,
            vad.manifest_version,
            vad.source.repository_url,
            vad.source.model_card_url,
            vad.source.license_spdx,
            vad.source.provenance,
        ])
        || !valid_https_url(vad.source.repository_url)
        || !valid_https_url(vad.source.model_card_url)
        || vad.runtime != SHERPA_RUNTIME
        || vad.qualification_policy != QualificationPolicy::StructuralWithPinnedRuntime
        || vad.bundle.artifacts.len() != 1
        || vad.bundle.artifacts[0].install_mode != ArtifactInstallMode::Direct
        || vad.bundle.identity_sha256 != vad.bundle.artifacts[0].sha256
        || vad.bundle.install_constraints != VAD_INSTALL_CONSTRAINTS
        || vad.source.license_spdx != vad.bundle.artifacts[0].license_spdx
        || !valid_vad_parameters(vad)
    {
        return Err(RegistryValidationError::InvalidManifestField);
    }
    validate_bundle(&vad.bundle)
}

fn valid_vad_parameters(vad: &VadManifest) -> bool {
    let artifact = &vad.bundle.artifacts[0];
    let numeric_values = [
        vad.threshold,
        vad.min_silence_duration_seconds,
        vad.min_speech_duration_seconds,
        vad.max_speech_duration_seconds,
    ];
    numeric_values.iter().all(|value| value.is_finite())
        && vad.threshold > 0.0
        && vad.threshold <= 1.0
        && vad.min_silence_duration_seconds > 0.0
        && vad.min_speech_duration_seconds > 0.0
        && vad.max_speech_duration_seconds >= vad.min_speech_duration_seconds
        && vad.window_size_samples > 0
        && vad.sample_rate_hz > 0
        && vad.num_threads > 0
        && vad.sherpa_onnx_version == PINNED_SHERPA_RUNTIME.version
        && vad.sherpa_onnx_commit == PINNED_SHERPA_RUNTIME.git_commit
        && vad.silero_config_source_header == VAD_SILERO_SOURCE_HEADER
        && vad.vad_config_source_header == VAD_SOURCE_HEADER
        && vad.threshold.to_bits() == VAD_THRESHOLD.to_bits()
        && vad.min_silence_duration_seconds.to_bits() == VAD_MIN_SILENCE_SECONDS.to_bits()
        && vad.min_speech_duration_seconds.to_bits() == VAD_MIN_SPEECH_SECONDS.to_bits()
        && vad.max_speech_duration_seconds.to_bits() == VAD_MAX_SPEECH_SECONDS.to_bits()
        && vad.window_size_samples == VAD_WINDOW_SIZE_SAMPLES
        && vad.sample_rate_hz == VAD_SAMPLE_RATE_HZ
        && vad.num_threads == VAD_NUM_THREADS
        && vad.provider == VAD_PROVIDER
        && artifact.resolved_url
            == "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/silero_vad.onnx"
        && artifact.bytes == 643_854
        && artifact.sha256 == "9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6"
        && artifact.required_path == "silero_vad.onnx"
        && vad.bundle.required_paths == ["silero_vad.onnx"]
}

fn expected_required_paths(model_id: &str) -> Option<&'static [&'static str]> {
    match model_id {
        "sense-voice-small-int8-2024-07-17" => Some(SENSE_PATHS),
        "whisper-tiny" => Some(WHISPER_TINY_PATHS),
        "whisper-base" => Some(WHISPER_BASE_PATHS),
        "whisper-small" => Some(WHISPER_SMALL_PATHS),
        "qwen3-asr-0.6b-int8-2026-03-25" => Some(QWEN06_PATHS),
        "qwen3-asr-1.7b" => Some(QWEN17_PATHS),
        _ => None,
    }
}

fn validate_bundle(bundle: &ArtifactBundle) -> Result<(), RegistryValidationError> {
    if bundle.artifacts.is_empty()
        || bundle.required_paths.is_empty()
        || !is_sha256(bundle.identity_sha256)
    {
        return Err(RegistryValidationError::InvalidBundleIdentity);
    }
    let mut artifact_ids = HashSet::new();
    for artifact in bundle.artifacts {
        if !artifact_ids.insert(artifact.artifact_id) {
            return Err(RegistryValidationError::DuplicateArtifactId);
        }
        if !valid_artifact(artifact) {
            return Err(RegistryValidationError::InvalidArtifact);
        }
    }
    for (index, path) in bundle.required_paths.iter().enumerate() {
        if !valid_relative_path(path) {
            return Err(RegistryValidationError::InvalidRequiredPath);
        }
        if bundle.required_paths[index + 1..]
            .iter()
            .any(|other| paths_overlap(path, other))
        {
            return Err(RegistryValidationError::OverlappingRequiredPath);
        }
    }
    validate_install_constraints(bundle)?;
    Ok(())
}

fn validate_install_constraints(bundle: &ArtifactBundle) -> Result<(), RegistryValidationError> {
    let (max_scanned_entries, max_written_file_bytes, max_total_written_bytes, required_files) =
        match bundle.install_constraints {
            InstallConstraints::Archive(constraints) => (
                Some(constraints.max_scanned_entries),
                constraints.max_written_file_bytes,
                constraints.max_total_written_bytes,
                constraints.required_files,
            ),
            InstallConstraints::Direct(constraints) => (
                None,
                constraints.max_written_file_bytes,
                constraints.max_total_written_bytes,
                constraints.required_files,
            ),
        };
    if required_files.is_empty() {
        return Err(RegistryValidationError::InvalidInstallConstraints);
    }
    let mut paths = HashSet::new();
    let mut total = 0_u64;
    let mut max_file = 0_u64;
    for file in required_files {
        if !valid_relative_path(file.path)
            || file.bytes == 0
            || !is_sha256(file.sha256)
            || !paths.insert(file.path)
        {
            return Err(RegistryValidationError::InvalidInstallConstraints);
        }
        total = total
            .checked_add(file.bytes)
            .ok_or(RegistryValidationError::InvalidInstallConstraints)?;
        max_file = max_file.max(file.bytes);
    }
    if max_file != max_written_file_bytes
        || total != max_total_written_bytes
        || bundle.required_paths.len() != required_files.len()
        || !bundle
            .required_paths
            .iter()
            .all(|path| paths.contains(path))
    {
        return Err(RegistryValidationError::InvalidInstallConstraints);
    }
    match bundle.install_constraints {
        InstallConstraints::Archive(_) => {
            if max_scanned_entries.is_none_or(|entries| {
                entries < u64::try_from(required_files.len()).unwrap_or(u64::MAX)
            }) || bundle
                .artifacts
                .iter()
                .any(|artifact| artifact.install_mode != ArtifactInstallMode::ExtractTarBz2)
            {
                return Err(RegistryValidationError::InvalidInstallConstraints);
            }
        }
        InstallConstraints::Direct(_) => {
            if bundle
                .artifacts
                .iter()
                .any(|artifact| artifact.install_mode != ArtifactInstallMode::Direct)
                || bundle.artifacts.len() != required_files.len()
                || bundle.artifacts.iter().any(|artifact| {
                    !required_files.iter().any(|file| {
                        (file.path, file.bytes, file.sha256)
                            == (artifact.required_path, artifact.bytes, artifact.sha256)
                    })
                })
                || required_files.iter().any(|file| {
                    bundle
                        .artifacts
                        .iter()
                        .filter(|artifact| {
                            (file.path, file.bytes, file.sha256)
                                == (artifact.required_path, artifact.bytes, artifact.sha256)
                        })
                        .count()
                        != 1
                })
            {
                return Err(RegistryValidationError::InvalidInstallConstraints);
            }
        }
    }
    Ok(())
}

fn valid_artifact(artifact: &ArtifactFile) -> bool {
    !any_blank(&[
        artifact.artifact_id,
        artifact.source_repository,
        artifact.source_model,
        artifact.source_endpoint,
        artifact.resolved_url,
        artifact.revision,
        artifact.required_path,
        artifact.license_spdx,
        artifact.provenance,
    ]) && valid_https_url(artifact.source_repository)
        && valid_https_url(artifact.source_endpoint)
        && valid_https_url(artifact.resolved_url)
        && (artifact.revision.starts_with("github-release-asset:")
            || artifact.resolved_url.contains(artifact.revision))
        && artifact.bytes > 0
        && is_sha256(artifact.sha256)
        && valid_relative_path(artifact.required_path)
        && artifact.required
        && valid_redirect_allowlist(artifact)
        && artifact
            .redirect_hosts
            .windows(2)
            .all(|pair| pair[0] < pair[1])
}

fn valid_redirect_allowlist(artifact: &ArtifactFile) -> bool {
    let Some(download_host) = reqwest::Url::parse(artifact.resolved_url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
    else {
        return false;
    };
    let expected = if artifact.source_repository.contains("github.com/k2-fsa/") {
        GITHUB_REDIRECT_HOSTS
    } else if artifact
        .source_repository
        .starts_with("https://www.modelscope.cn/")
    {
        if artifact.bytes > 20_000_000 {
            MODELSCOPE_LFS_HOSTS
        } else {
            MODELSCOPE_DIRECT_HOSTS
        }
    } else if artifact
        .source_repository
        .starts_with("https://huggingface.co/")
    {
        HUGGING_FACE_REDIRECT_HOSTS
    } else {
        return false;
    };
    artifact.redirect_hosts == expected
        && artifact.redirect_hosts.contains(&download_host.as_str())
        && artifact
            .redirect_hosts
            .iter()
            .all(|host| valid_hostname(host))
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

fn valid_languages(languages: &[&str]) -> bool {
    if languages.is_empty() || !languages.contains(&"auto") {
        return false;
    }
    let mut seen = HashSet::new();
    languages.iter().all(|language| {
        !language.is_empty()
            && language.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || byte == b'_'
            })
            && seen.insert(*language)
    })
}

fn valid_https_url(value: &str) -> bool {
    let Some(authority) = value
        .strip_prefix("https://")
        .and_then(|rest| rest.split('/').next())
    else {
        return false;
    };
    if authority.is_empty()
        || authority.contains('@')
        || authority.ends_with(":443")
        || authority.bytes().any(|byte| byte.is_ascii_uppercase())
    {
        return false;
    }
    let Ok(url) = reqwest::Url::parse(value) else {
        return false;
    };
    url.scheme() == "https"
        && url.host_str().is_some()
        && url.port_or_known_default() == Some(443)
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none()
}

fn any_blank(values: &[&str]) -> bool {
    values.iter().any(|value| {
        value.trim().is_empty()
            || value.contains("TODO")
            || value.contains("PLACEHOLDER")
            || value == &"null"
    })
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && value != "0000000000000000000000000000000000000000000000000000000000000000"
}

fn valid_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.ends_with('/')
        && !path.contains('\\')
        && !path.chars().any(char::is_control)
        && path
            .split('/')
            .all(|component| !component.is_empty() && component != "." && component != "..")
}

fn paths_overlap(left: &str, right: &str) -> bool {
    left == right
        || right
            .strip_prefix(left)
            .is_some_and(|suffix| suffix.starts_with('/'))
        || left
            .strip_prefix(right)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn artifact_payload(artifact: &ArtifactFile) -> Value {
    json!({
        "artifact_id": artifact.artifact_id,
        "bytes": artifact.bytes,
        "install_mode": match artifact.install_mode {
            ArtifactInstallMode::Direct => "direct",
            ArtifactInstallMode::ExtractTarBz2 => "extract_tar_bz2",
        },
        "license_spdx": artifact.license_spdx,
        "provenance": artifact.provenance,
        "redirect_hosts": artifact.redirect_hosts,
        "required": artifact.required,
        "required_path": artifact.required_path,
        "resolved_url": artifact.resolved_url,
        "revision": artifact.revision,
        "sha256": artifact.sha256,
        "source_endpoint": artifact.source_endpoint,
        "source_model": artifact.source_model,
        "source_repository": artifact.source_repository,
    })
}

fn compatibility_payload(model: &ModelManifest) -> Value {
    if model.id == "qwen3-asr-1.7b" {
        json!({
            "config_shape": "top_level_thinker_config",
            "conversion": "none",
            "tokenizer_source": "official_qwen3_asr_1.7b_hf",
        })
    } else {
        json!({"archive_contract": "sherpa_release_required_paths_v1"})
    }
}

fn runtime_payload(runtime: RuntimeRequirement) -> Value {
    match runtime {
        RuntimeRequirement::SherpaOnnx {
            crate_version,
            git_commit,
            native_archive_sha256,
            build_id,
            cargo_feature,
        } => json!({
            "build_id": build_id,
            "cargo_feature": cargo_feature,
            "crate": "sherpa-onnx",
            "git_commit": git_commit,
            "native_archive_sha256": native_archive_sha256,
            "version": crate_version,
        }),
        RuntimeRequirement::QwenCandleMetal {
            crate_name,
            crate_version,
            git_url,
            git_commit,
            cargo_feature,
            target_os,
            target_arch,
        } => json!({
            "backend": "candle_metal",
            "cargo_feature": cargo_feature,
            "crate": crate_name,
            "git_commit": git_commit,
            "git_url": git_url,
            "target_arch": target_arch,
            "target_os": target_os,
            "version": crate_version,
        }),
    }
}

fn device_payload(device: DeviceRequirement) -> Value {
    match device {
        DeviceRequirement::AnyDesktop => json!({"kind": "any_desktop"}),
        DeviceRequirement::AppleSiliconMetal {
            minimum_macos_major,
            minimum_memory_gib,
        } => json!({
            "kind": "apple_silicon_metal",
            "minimum_macos_major": minimum_macos_major,
            "minimum_memory_gib": minimum_memory_gib,
        }),
    }
}

fn provider_name(provider: AsrProviderKind) -> &'static str {
    match provider {
        AsrProviderKind::SenseVoice => "sense_voice",
        AsrProviderKind::Whisper => "whisper",
        AsrProviderKind::Qwen3Asr => "qwen3_asr",
    }
}

fn qualification_name(policy: QualificationPolicy) -> &'static str {
    match policy {
        QualificationPolicy::StructuralWithPinnedRuntime => "structural_with_pinned_runtime",
        QualificationPolicy::RuntimeSmokeRequired => "runtime_smoke_required",
    }
}

const PINNED_SHERPA_RUNTIME: crate::asr::PinnedSherpaRuntimeIdentity =
    crate::asr::pinned_sherpa_runtime_identity();
const SHERPA_RUNTIME: RuntimeRequirement = RuntimeRequirement::SherpaOnnx {
    crate_version: PINNED_SHERPA_RUNTIME.version,
    git_commit: PINNED_SHERPA_RUNTIME.git_commit,
    native_archive_sha256: PINNED_SHERPA_RUNTIME.native_archive_sha256,
    build_id: PINNED_SHERPA_RUNTIME.build_id,
    cargo_feature: "static",
};

const QWEN_RUNTIME: RuntimeRequirement = RuntimeRequirement::QwenCandleMetal {
    crate_name: "qwen3-asr",
    crate_version: "0.2.2",
    git_url: "https://github.com/alan890104/qwen3-asr-rs.git",
    git_commit: "c5ef09646af6278d2ba8b8ceaf543ffb32d1a5dc",
    cargo_feature: "metal",
    target_os: "macos",
    target_arch: "aarch64",
};
const VAD_SILERO_SOURCE_HEADER: &str = "sherpa-onnx/csrc/silero-vad-model-config.h";
const VAD_SOURCE_HEADER: &str = "sherpa-onnx/csrc/vad-model-config.h";
const VAD_THRESHOLD: f32 = 0.5;
const VAD_MIN_SILENCE_SECONDS: f32 = 0.5;
const VAD_MIN_SPEECH_SECONDS: f32 = 0.25;
const VAD_MAX_SPEECH_SECONDS: f32 = 20.0;
const VAD_WINDOW_SIZE_SAMPLES: i32 = 512;
const VAD_SAMPLE_RATE_HZ: i32 = 16_000;
const VAD_NUM_THREADS: i32 = 1;
const VAD_PROVIDER: &str = "cpu";

const SENSE_INSTALL_FILES: &[RequiredInstallFile] = &[
    RequiredInstallFile {
        path: "model.int8.onnx",
        bytes: 239_233_841,
        sha256: "c71f0ce00bec95b07744e116345e33d8cbbe08cef896382cf907bf4b51a2cd51",
    },
    RequiredInstallFile {
        path: "test_wavs/en.wav",
        bytes: 228_908,
        sha256: "eb1eb008904465b74c304aad8342e8c7d3c6e61ffe9f66adcaca9cf0f76a93f4",
    },
    RequiredInstallFile {
        path: "test_wavs/ja.wav",
        bytes: 230_444,
        sha256: "460bd8dccb0d2a5f4e29c628f837be4082d13defc64c3fc21dd1b6bb0e119095",
    },
    RequiredInstallFile {
        path: "test_wavs/ko.wav",
        bytes: 147_500,
        sha256: "0dc797a5c81ed30fc339d91f3da718ab02854e17ffa37cb93c4c039ac5c6bb9c",
    },
    RequiredInstallFile {
        path: "test_wavs/yue.wav",
        bytes: 164_780,
        sha256: "0960b2db54ae202071d250e6462fbf74a3c863f0e3e7f01273e4939c996875a0",
    },
    RequiredInstallFile {
        path: "test_wavs/zh.wav",
        bytes: 178_988,
        sha256: "b77f1794fe374a0ba1ee1dc458bfaf9349496cbbfc32780c50ba3c5a7ad8e373",
    },
    RequiredInstallFile {
        path: "tokens.txt",
        bytes: 315_894,
        sha256: "f449eb28dc567533d7fa59be34e2abca8784f771850c78a47fb731a31429a1dc",
    },
];
const TINY_INSTALL_FILES: &[RequiredInstallFile] = &[
    RequiredInstallFile {
        path: "test_wavs/0.wav",
        bytes: 212_044,
        sha256: "6bc58a4efdf20daac252b6b1502632601a71efe0308f6757dc1eda34891a7e4f",
    },
    RequiredInstallFile {
        path: "test_wavs/1.wav",
        bytes: 534_924,
        sha256: "5143a6ba93c4b274e2c4ac22deb75c2c48936c853f0519add1de828b6c79cc5a",
    },
    RequiredInstallFile {
        path: "test_wavs/8k.wav",
        bytes: 77_244,
        sha256: "f6f3c8b33e2534cdc154fe773ad2750f1f6a2ca5096179cdf037ae782456613e",
    },
    RequiredInstallFile {
        path: "test_wavs/trans.txt",
        bytes: 449,
        sha256: "b9ac44e7b794abb1a2d5faf0005e98a665971e7ac2ed15435832cc34edaa9100",
    },
    RequiredInstallFile {
        path: "tiny-decoder.onnx",
        bytes: 114_505_801,
        sha256: "e144c07dc6b55cece24392811f2d934b97013811f5e677d1315d341a0a74a25d",
    },
    RequiredInstallFile {
        path: "tiny-encoder.onnx",
        bytes: 37_647_080,
        sha256: "42c1d4cbf889632ba21ab6f0d4064c80209755f265ce5cd630db4a6793e7089c",
    },
    RequiredInstallFile {
        path: "tiny-tokens.txt",
        bytes: 816_730,
        sha256: "b34b360dbb493e781e479794586d661700670d65564001f23024971d1f2fa126",
    },
];
const BASE_INSTALL_FILES: &[RequiredInstallFile] = &[
    RequiredInstallFile {
        path: "base-decoder.onnx",
        bytes: 196_548_998,
        sha256: "8a12c3f6ad65bb5b86d7e6eccc302378f20f9fb2df6cb10747c62895da7ac194",
    },
    RequiredInstallFile {
        path: "base-encoder.onnx",
        bytes: 95_087_154,
        sha256: "5a6b87cb313993f6c9fefec9e7027556f6cb30becddf49655bee36c50ecc12d7",
    },
    RequiredInstallFile {
        path: "base-tokens.txt",
        bytes: 816_730,
        sha256: "b34b360dbb493e781e479794586d661700670d65564001f23024971d1f2fa126",
    },
    RequiredInstallFile {
        path: "test_wavs/0.wav",
        bytes: 212_044,
        sha256: "6bc58a4efdf20daac252b6b1502632601a71efe0308f6757dc1eda34891a7e4f",
    },
    RequiredInstallFile {
        path: "test_wavs/1.wav",
        bytes: 534_924,
        sha256: "5143a6ba93c4b274e2c4ac22deb75c2c48936c853f0519add1de828b6c79cc5a",
    },
    RequiredInstallFile {
        path: "test_wavs/8k.wav",
        bytes: 77_244,
        sha256: "f6f3c8b33e2534cdc154fe773ad2750f1f6a2ca5096179cdf037ae782456613e",
    },
    RequiredInstallFile {
        path: "test_wavs/trans.txt",
        bytes: 449,
        sha256: "b9ac44e7b794abb1a2d5faf0005e98a665971e7ac2ed15435832cc34edaa9100",
    },
];
const SMALL_INSTALL_FILES: &[RequiredInstallFile] = &[
    RequiredInstallFile {
        path: "small-decoder.onnx",
        bytes: 559_127_829,
        sha256: "a4165cca5c77e381938c0e111032a384901b1e434ae2ad948859035392d21d2c",
    },
    RequiredInstallFile {
        path: "small-encoder.onnx",
        bytes: 409_528_992,
        sha256: "119bd1e8ba0524baee1687f6b22bf0abd2fe539549cd000734edbca81c66751e",
    },
    RequiredInstallFile {
        path: "small-tokens.txt",
        bytes: 816_730,
        sha256: "b34b360dbb493e781e479794586d661700670d65564001f23024971d1f2fa126",
    },
    RequiredInstallFile {
        path: "test_wavs/0.wav",
        bytes: 212_044,
        sha256: "6bc58a4efdf20daac252b6b1502632601a71efe0308f6757dc1eda34891a7e4f",
    },
    RequiredInstallFile {
        path: "test_wavs/1.wav",
        bytes: 534_924,
        sha256: "5143a6ba93c4b274e2c4ac22deb75c2c48936c853f0519add1de828b6c79cc5a",
    },
    RequiredInstallFile {
        path: "test_wavs/8k.wav",
        bytes: 77_244,
        sha256: "f6f3c8b33e2534cdc154fe773ad2750f1f6a2ca5096179cdf037ae782456613e",
    },
    RequiredInstallFile {
        path: "test_wavs/trans.txt",
        bytes: 449,
        sha256: "b9ac44e7b794abb1a2d5faf0005e98a665971e7ac2ed15435832cc34edaa9100",
    },
];
const QWEN06_INSTALL_FILES: &[RequiredInstallFile] = &[
    RequiredInstallFile {
        path: "conv_frontend.onnx",
        bytes: 44_148_281,
        sha256: "d22dc4423e0940e49884e903d2ea2f7e5567c14fc1aed97e4e26d6b8f208ef9e",
    },
    RequiredInstallFile {
        path: "decoder.int8.onnx",
        bytes: 755_914_231,
        sha256: "4f6885be5959ae26af3089d38ee7972c5fafbeeb1cf8d5e76eab6d8b61ca5771",
    },
    RequiredInstallFile {
        path: "encoder.int8.onnx",
        bytes: 182_491_662,
        sha256: "60748d3e6744a57c9c91e1b17424a6c2990567e8adceb0783940c03ed98fa9d9",
    },
    RequiredInstallFile {
        path: "test_wavs/ar1.wav",
        bytes: 168_044,
        sha256: "700b3c274f2fedffbb6016f03c574adaad7aa0291acc1a1ba72f07112051073f",
    },
    RequiredInstallFile {
        path: "test_wavs/cantonese.wav",
        bytes: 526_444,
        sha256: "ec832e035c13c670e0cf68dee0ca5dfae38bf2c583aab31e587441cb3eba3f3f",
    },
    RequiredInstallFile {
        path: "test_wavs/codeswitch.wav",
        bytes: 549_550,
        sha256: "2def7fa41004d0a7d148d4afbf4c467c9d112d8b373996123e9a4c43d94957c7",
    },
    RequiredInstallFile {
        path: "test_wavs/de.wav",
        bytes: 215_084,
        sha256: "80bb10c44085a7ce01a17abaf6a2095ed37e1695fca41cc0ea9733f1f24a749c",
    },
    RequiredInstallFile {
        path: "test_wavs/es1.wav",
        bytes: 164_844,
        sha256: "4543f94738445a38306fb80bb0329ef5ca6d81ab1b6c3f15af1de1c3382f4b31",
    },
    RequiredInstallFile {
        path: "test_wavs/f1_noise.wav",
        bytes: 1_677_606,
        sha256: "7ae35f5d8f038e518f3abdeda5f78d71cb2f67c9ca29cb9a49a0b4d0702909bd",
    },
    RequiredInstallFile {
        path: "test_wavs/fast1.wav",
        bytes: 1_003_794,
        sha256: "b43bbd0bd982c3cc88081f64389bf29fe9e9a01287d44f0b15887bc49c2b352a",
    },
    RequiredInstallFile {
        path: "test_wavs/fr1.wav",
        bytes: 191_916,
        sha256: "c6421b34feccbe7fdfaa8b641b8ecb7bcd7b9f2c237c7b82712c860be524db4e",
    },
    RequiredInstallFile {
        path: "test_wavs/ja1.wav",
        bytes: 448_100,
        sha256: "d926ed0159a2d750d1ae7835e60a5cb5f8737629f7bb3de6cd111a3614d5dc67",
    },
    RequiredInstallFile {
        path: "test_wavs/noise1-en.wav",
        bytes: 2_831_516,
        sha256: "3664ef02fa664da93d94a1afc271bda31c0f8d07a9f3c74ac6cd1e5aabe8572c",
    },
    RequiredInstallFile {
        path: "test_wavs/noise2.wav",
        bytes: 741_186,
        sha256: "33f85268f7fbad6b3152b9ab051edab1a85082fde66bccff61d7f5ef7b437e58",
    },
    RequiredInstallFile {
        path: "test_wavs/qiqiu1.wav",
        bytes: 1_631_150,
        sha256: "1b69a0fce35936979824c1751a11c285559a635aeb91160d1da3b00118321495",
    },
    RequiredInstallFile {
        path: "test_wavs/raokouling.wav",
        bytes: 1_831_074,
        sha256: "3cc59ec494f71135ff5761717e20597f0559b43f793dd72ae4924b86c5e038d8",
    },
    RequiredInstallFile {
        path: "test_wavs/rap1.wav",
        bytes: 935_868,
        sha256: "ac6186d732b59c664776f84238f586d3e6c97adbc8b9f66e939ddcab5773cf3c",
    },
    RequiredInstallFile {
        path: "test_wavs/ru1.wav",
        bytes: 152_364,
        sha256: "e48b22f32d4d1c38f0a94a58acfc43bb8f5b7fc3b0ac01ea49372040ca831acf",
    },
    RequiredInstallFile {
        path: "test_wavs/transcript.txt",
        bytes: 5_386,
        sha256: "9cab82a507e1e5a7743336f2e40fabdaa1eb6181818d7a3768925abc03effd24",
    },
    RequiredInstallFile {
        path: "tokenizer/merges.txt",
        bytes: 1_671_853,
        sha256: "8831e4f1a044471340f7c0a83d7bd71306a5b867e95fd870f74d0c5308a904d5",
    },
    RequiredInstallFile {
        path: "tokenizer/tokenizer_config.json",
        bytes: 12_487,
        sha256: "4942d005604266809309cabc9f4e9cb89ce855d59b14681fdc0e1cc62ea26c4c",
    },
    RequiredInstallFile {
        path: "tokenizer/vocab.json",
        bytes: 2_776_833,
        sha256: "ca10d7e9fb3ed18575dd1e277a2579c16d108e32f27439684afa0e10b1440910",
    },
];

const SENSE_INSTALL_CONSTRAINTS: InstallConstraints =
    InstallConstraints::Archive(ArchiveInstallConstraints {
        max_scanned_entries: 12,
        max_written_file_bytes: 239_233_841,
        max_total_written_bytes: 240_500_355,
        required_files: SENSE_INSTALL_FILES,
    });
const TINY_INSTALL_CONSTRAINTS: InstallConstraints =
    InstallConstraints::Archive(ArchiveInstallConstraints {
        max_scanned_entries: 11,
        max_written_file_bytes: 114_505_801,
        max_total_written_bytes: 153_794_272,
        required_files: TINY_INSTALL_FILES,
    });
const BASE_INSTALL_CONSTRAINTS: InstallConstraints =
    InstallConstraints::Archive(ArchiveInstallConstraints {
        max_scanned_entries: 11,
        max_written_file_bytes: 196_548_998,
        max_total_written_bytes: 293_277_543,
        required_files: BASE_INSTALL_FILES,
    });
const SMALL_INSTALL_CONSTRAINTS: InstallConstraints =
    InstallConstraints::Archive(ArchiveInstallConstraints {
        max_scanned_entries: 11,
        max_written_file_bytes: 559_127_829,
        max_total_written_bytes: 970_298_212,
        required_files: SMALL_INSTALL_FILES,
    });
const QWEN06_INSTALL_CONSTRAINTS: InstallConstraints =
    InstallConstraints::Archive(ArchiveInstallConstraints {
        max_scanned_entries: 27,
        max_written_file_bytes: 755_914_231,
        max_total_written_bytes: 1_000_089_273,
        required_files: QWEN06_INSTALL_FILES,
    });

const SENSE_PATHS: &[&str] = &[
    "model.int8.onnx",
    "tokens.txt",
    "test_wavs/en.wav",
    "test_wavs/ja.wav",
    "test_wavs/ko.wav",
    "test_wavs/yue.wav",
    "test_wavs/zh.wav",
];
const WHISPER_TINY_PATHS: &[&str] = &[
    "test_wavs/0.wav",
    "test_wavs/1.wav",
    "test_wavs/8k.wav",
    "test_wavs/trans.txt",
    "tiny-decoder.onnx",
    "tiny-encoder.onnx",
    "tiny-tokens.txt",
];
const WHISPER_BASE_PATHS: &[&str] = &[
    "base-decoder.onnx",
    "base-encoder.onnx",
    "base-tokens.txt",
    "test_wavs/0.wav",
    "test_wavs/1.wav",
    "test_wavs/8k.wav",
    "test_wavs/trans.txt",
];
const WHISPER_SMALL_PATHS: &[&str] = &[
    "small-decoder.onnx",
    "small-encoder.onnx",
    "small-tokens.txt",
    "test_wavs/0.wav",
    "test_wavs/1.wav",
    "test_wavs/8k.wav",
    "test_wavs/trans.txt",
];
const QWEN06_PATHS: &[&str] = &[
    "conv_frontend.onnx",
    "decoder.int8.onnx",
    "encoder.int8.onnx",
    "test_wavs/ar1.wav",
    "test_wavs/cantonese.wav",
    "test_wavs/codeswitch.wav",
    "test_wavs/de.wav",
    "test_wavs/es1.wav",
    "test_wavs/f1_noise.wav",
    "test_wavs/fast1.wav",
    "test_wavs/fr1.wav",
    "test_wavs/ja1.wav",
    "test_wavs/noise1-en.wav",
    "test_wavs/noise2.wav",
    "test_wavs/qiqiu1.wav",
    "test_wavs/raokouling.wav",
    "test_wavs/rap1.wav",
    "test_wavs/ru1.wav",
    "test_wavs/transcript.txt",
    "tokenizer/merges.txt",
    "tokenizer/tokenizer_config.json",
    "tokenizer/vocab.json",
];
const QWEN17_PATHS: &[&str] = &[
    "config.json",
    "model-00001-of-00002.safetensors",
    "model-00002-of-00002.safetensors",
    "model.safetensors.index.json",
    "tokenizer.json",
];

const SENSE_ARTIFACTS: &[ArtifactFile] = &[github_archive(
    "sense-voice-archive",
    "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17.tar.bz2",
    "github-release-asset:288366523",
    "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17.tar.bz2",
    163_002_883,
    "7d1efa2138a65b0b488df37f8b89e3d91a60676e416f515b952358d83dfd347e",
    "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17",
    "LicenseRef-FunASR-Model-1.1",
    "Official sherpa-onnx conversion of FunAudioLLM SenseVoiceSmall INT8; archive LICENSE points to the FunASR model license.",
)];
const WHISPER_TINY_ARTIFACTS: &[ArtifactFile] = &[github_archive(
    "whisper-tiny-archive",
    "sherpa-onnx-whisper-tiny.tar.bz2",
    "github-release-asset:179373699",
    "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-tiny.tar.bz2",
    116_204_861,
    "c46116994e539aa165266d96b325252728429c12535eb9d8b6a2b10f129e66b1",
    "sherpa-onnx-whisper-tiny",
    "MIT",
    "Official sherpa-onnx ONNX export of OpenAI Whisper Tiny.",
)];
const WHISPER_BASE_ARTIFACTS: &[ArtifactFile] = &[github_archive(
    "whisper-base-archive",
    "sherpa-onnx-whisper-base.tar.bz2",
    "github-release-asset:196350768",
    "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-base.tar.bz2",
    207_557_382,
    "911b2083efd7c0dca2ac3b358b75222660dc09fb716d64fbfc417ba6c99ff3de",
    "sherpa-onnx-whisper-base",
    "MIT",
    "Official sherpa-onnx ONNX export of OpenAI Whisper Base.",
)];
const WHISPER_SMALL_ARTIFACTS: &[ArtifactFile] = &[github_archive(
    "whisper-small-archive",
    "sherpa-onnx-whisper-small.tar.bz2",
    "github-release-asset:179373989",
    "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-small.tar.bz2",
    639_387_718,
    "486a46afbb7ba798507190ffe02fea2dd726049af212e774537efac6afb210a6",
    "sherpa-onnx-whisper-small",
    "MIT",
    "Official sherpa-onnx ONNX export of OpenAI Whisper Small.",
)];
const QWEN06_ARTIFACTS: &[ArtifactFile] = &[github_archive(
    "qwen06-archive",
    "sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25.tar.bz2",
    "github-release-asset:390698077",
    "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25.tar.bz2",
    878_702_423,
    "393f8a14e2f5fb96746aaab342997a40641001fbd5bf9592a080a8329178ee96",
    "sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25",
    "Apache-2.0",
    "Sherpa release built from official Qwen3-ASR 0.6B using the linked Wasser1462 ONNX conversion.",
)];

#[allow(clippy::too_many_arguments)]
const fn github_archive(
    artifact_id: &'static str,
    filename: &'static str,
    revision: &'static str,
    resolved_url: &'static str,
    bytes: u64,
    sha256: &'static str,
    required_path: &'static str,
    license_spdx: &'static str,
    provenance: &'static str,
) -> ArtifactFile {
    ArtifactFile {
        artifact_id,
        source_repository: SHERPA_REPOSITORY,
        source_model: filename,
        source_endpoint: SHERPA_RELEASE_API,
        resolved_url,
        revision,
        bytes,
        sha256,
        required_path,
        required: true,
        install_mode: ArtifactInstallMode::ExtractTarBz2,
        license_spdx,
        provenance,
        redirect_hosts: GITHUB_REDIRECT_HOSTS,
    }
}

const QWEN17_ARTIFACTS: &[ArtifactFile] = &[
    modelscope_file(
        "qwen17-config",
        "config.json",
        "https://www.modelscope.cn/models/Qwen/Qwen3-ASR-1.7B/resolve/d69410f1c275f2b0fa60cbb9960edfcdb0ae0aec/config.json",
        MODELSCOPE_DIRECT_HOSTS,
        6_194,
        "2e74a751548b8ad7d7526d29365ad8144c345d8b412b1152d25dc6698452712f",
        "Official original config with top-level thinker_config; conversion none.",
    ),
    modelscope_file(
        "qwen17-index",
        "model.safetensors.index.json",
        "https://www.modelscope.cn/models/Qwen/Qwen3-ASR-1.7B/resolve/d69410f1c275f2b0fa60cbb9960edfcdb0ae0aec/model.safetensors.index.json",
        MODELSCOPE_DIRECT_HOSTS,
        64_821,
        "f994739fe38e5210b9e3e8ce6c6307315e2ceac3cb630e7b7414d69dce520f60",
        "Official original safetensors shard index; conversion none.",
    ),
    ArtifactFile {
        artifact_id: "qwen17-tokenizer",
        source_repository: "https://huggingface.co/Qwen/Qwen3-ASR-1.7B-hf",
        source_model: "Qwen/Qwen3-ASR-1.7B-hf",
        source_endpoint: "https://huggingface.co/api/models/Qwen/Qwen3-ASR-1.7B-hf/revision/bcd2b5b7f32b480ab5790554cfa8347f246a14f3",
        resolved_url: "https://huggingface.co/Qwen/Qwen3-ASR-1.7B-hf/resolve/bcd2b5b7f32b480ab5790554cfa8347f246a14f3/tokenizer.json",
        revision: HUGGING_FACE_REVISION,
        bytes: 11_429_653,
        sha256: "fe1fad59be22a41ee293363fcf95fdedbc7c93f3b49270b1d2e18bd1399a7a05",
        required_path: "tokenizer.json",
        required: true,
        install_mode: ArtifactInstallMode::Direct,
        license_spdx: "Apache-2.0",
        provenance: "Official -hf tokenizer mixed with official original config and weights; conversion none.",
        redirect_hosts: HUGGING_FACE_REDIRECT_HOSTS,
    },
    modelscope_file(
        "qwen17-weights-00001",
        "model-00001-of-00002.safetensors",
        "https://www.modelscope.cn/models/Qwen/Qwen3-ASR-1.7B/resolve/d69410f1c275f2b0fa60cbb9960edfcdb0ae0aec/model-00001-of-00002.safetensors",
        MODELSCOPE_LFS_HOSTS,
        4_220_320_824,
        "a4cd1f1a04d90b757dc7f7dd26254e69a013b19e80efe590a83c6a3bde8608d6",
        "Official original safetensors weight shard 1; conversion none.",
    ),
    modelscope_file(
        "qwen17-weights-00002",
        "model-00002-of-00002.safetensors",
        "https://www.modelscope.cn/models/Qwen/Qwen3-ASR-1.7B/resolve/d69410f1c275f2b0fa60cbb9960edfcdb0ae0aec/model-00002-of-00002.safetensors",
        MODELSCOPE_LFS_HOSTS,
        478_200_688,
        "6e0b9d9e09e2e0238e7ef3cc8a484ab387e91b90f1900bedf88bc92d7929ccfc",
        "Official original safetensors weight shard 2; conversion none.",
    ),
];
const QWEN17_INSTALL_FILES: &[RequiredInstallFile] = &[
    RequiredInstallFile {
        path: "config.json",
        bytes: 6_194,
        sha256: "2e74a751548b8ad7d7526d29365ad8144c345d8b412b1152d25dc6698452712f",
    },
    RequiredInstallFile {
        path: "model.safetensors.index.json",
        bytes: 64_821,
        sha256: "f994739fe38e5210b9e3e8ce6c6307315e2ceac3cb630e7b7414d69dce520f60",
    },
    RequiredInstallFile {
        path: "tokenizer.json",
        bytes: 11_429_653,
        sha256: "fe1fad59be22a41ee293363fcf95fdedbc7c93f3b49270b1d2e18bd1399a7a05",
    },
    RequiredInstallFile {
        path: "model-00001-of-00002.safetensors",
        bytes: 4_220_320_824,
        sha256: "a4cd1f1a04d90b757dc7f7dd26254e69a013b19e80efe590a83c6a3bde8608d6",
    },
    RequiredInstallFile {
        path: "model-00002-of-00002.safetensors",
        bytes: 478_200_688,
        sha256: "6e0b9d9e09e2e0238e7ef3cc8a484ab387e91b90f1900bedf88bc92d7929ccfc",
    },
];
const QWEN17_INSTALL_CONSTRAINTS: InstallConstraints =
    InstallConstraints::Direct(DirectInstallConstraints {
        max_written_file_bytes: 4_220_320_824,
        max_total_written_bytes: 4_710_022_180,
        required_files: QWEN17_INSTALL_FILES,
    });

const fn modelscope_file(
    artifact_id: &'static str,
    path: &'static str,
    resolved_url: &'static str,
    redirect_hosts: &'static [&'static str],
    bytes: u64,
    sha256: &'static str,
    provenance: &'static str,
) -> ArtifactFile {
    ArtifactFile {
        artifact_id,
        source_repository: "https://www.modelscope.cn/models/Qwen/Qwen3-ASR-1.7B",
        source_model: "Qwen/Qwen3-ASR-1.7B",
        source_endpoint: "https://www.modelscope.cn/api/v1/models/Qwen/Qwen3-ASR-1.7B/repo/files?Revision=d69410f1c275f2b0fa60cbb9960edfcdb0ae0aec&Recursive=true",
        resolved_url,
        revision: MODELSCOPE_REVISION,
        bytes,
        sha256,
        required_path: path,
        required: true,
        install_mode: ArtifactInstallMode::Direct,
        license_spdx: "Apache-2.0",
        provenance,
        redirect_hosts,
    }
}

const LANG_SENSE: &[&str] = &["auto", "zh", "en", "ja", "ko", "yue"];
const LANG_WHISPER: &[&str] = &["auto", "zh", "en", "ja", "ko", "yue"];
const LANG_QWEN06: &[&str] = &["auto"];
const LANG_QWEN17: &[&str] = &[
    "auto", "zh", "en", "yue", "ar", "de", "fr", "es", "pt", "id", "it", "ko", "ru", "th", "vi",
    "ja", "tr", "hi", "ms", "nl", "sv", "da", "fi", "pl", "cs", "fil", "fa", "el", "hu", "mk",
    "ro",
];

const MODELS: &[ModelManifest] = &[
    ModelManifest {
        id: "sense-voice-small-int8-2024-07-17",
        manifest_version: "1",
        display_name: "SenseVoiceSmall INT8",
        provider: AsrProviderKind::SenseVoice,
        supported_languages: LANG_SENSE,
        bundle: ArtifactBundle {
            artifacts: SENSE_ARTIFACTS,
            required_paths: SENSE_PATHS,
            identity_sha256: "7d1efa2138a65b0b488df37f8b89e3d91a60676e416f515b952358d83dfd347e",
            install_constraints: SENSE_INSTALL_CONSTRAINTS,
        },
        runtime: SHERPA_RUNTIME,
        device: DeviceRequirement::AnyDesktop,
        qualification_policy: QualificationPolicy::StructuralWithPinnedRuntime,
        source: ModelSource {
            repository_url: "https://github.com/FunAudioLLM/SenseVoice",
            model_card_url: "https://github.com/FunAudioLLM/SenseVoice",
            license_spdx: "LicenseRef-FunASR-Model-1.1",
            provenance: "SenseVoiceSmall converted and distributed by sherpa-onnx.",
        },
    },
    whisper_model(
        "whisper-tiny",
        "Whisper Tiny",
        WHISPER_TINY_ARTIFACTS,
        WHISPER_TINY_PATHS,
        "c46116994e539aa165266d96b325252728429c12535eb9d8b6a2b10f129e66b1",
        TINY_INSTALL_CONSTRAINTS,
    ),
    whisper_model(
        "whisper-base",
        "Whisper Base",
        WHISPER_BASE_ARTIFACTS,
        WHISPER_BASE_PATHS,
        "911b2083efd7c0dca2ac3b358b75222660dc09fb716d64fbfc417ba6c99ff3de",
        BASE_INSTALL_CONSTRAINTS,
    ),
    whisper_model(
        "whisper-small",
        "Whisper Small",
        WHISPER_SMALL_ARTIFACTS,
        WHISPER_SMALL_PATHS,
        "486a46afbb7ba798507190ffe02fea2dd726049af212e774537efac6afb210a6",
        SMALL_INSTALL_CONSTRAINTS,
    ),
    ModelManifest {
        id: "qwen3-asr-0.6b-int8-2026-03-25",
        manifest_version: "1",
        display_name: "Qwen3-ASR 0.6B INT8",
        provider: AsrProviderKind::Qwen3Asr,
        supported_languages: LANG_QWEN06,
        bundle: ArtifactBundle {
            artifacts: QWEN06_ARTIFACTS,
            required_paths: QWEN06_PATHS,
            identity_sha256: "393f8a14e2f5fb96746aaab342997a40641001fbd5bf9592a080a8329178ee96",
            install_constraints: QWEN06_INSTALL_CONSTRAINTS,
        },
        runtime: SHERPA_RUNTIME,
        device: DeviceRequirement::AnyDesktop,
        qualification_policy: QualificationPolicy::StructuralWithPinnedRuntime,
        source: ModelSource {
            repository_url: "https://github.com/QwenLM/Qwen3-ASR",
            model_card_url: "https://www.modelscope.cn/models/Qwen/Qwen3-ASR-0.6B",
            license_spdx: "Apache-2.0",
            provenance: "Official Qwen model converted to sherpa ONNX INT8 by the upstream release process.",
        },
    },
    ModelManifest {
        id: "qwen3-asr-1.7b",
        manifest_version: "2",
        display_name: "Qwen3-ASR 1.7B",
        provider: AsrProviderKind::Qwen3Asr,
        supported_languages: LANG_QWEN17,
        bundle: ArtifactBundle {
            artifacts: QWEN17_ARTIFACTS,
            required_paths: QWEN17_PATHS,
            identity_sha256: QWEN17_IDENTITY,
            install_constraints: QWEN17_INSTALL_CONSTRAINTS,
        },
        runtime: QWEN_RUNTIME,
        device: DeviceRequirement::AppleSiliconMetal {
            minimum_macos_major: 14,
            minimum_memory_gib: 24,
        },
        qualification_policy: QualificationPolicy::RuntimeSmokeRequired,
        source: ModelSource {
            repository_url: "https://github.com/QwenLM/Qwen3-ASR",
            model_card_url: "https://www.modelscope.cn/models/Qwen/Qwen3-ASR-1.7B",
            license_spdx: "Apache-2.0",
            provenance: "Official original ModelScope config/weights plus official Hugging Face -hf tokenizer; conversion none.",
        },
    },
];

const fn whisper_model(
    id: &'static str,
    display_name: &'static str,
    artifacts: &'static [ArtifactFile],
    required_paths: &'static [&'static str],
    identity_sha256: &'static str,
    install_constraints: InstallConstraints,
) -> ModelManifest {
    ModelManifest {
        id,
        manifest_version: "1",
        display_name,
        provider: AsrProviderKind::Whisper,
        supported_languages: LANG_WHISPER,
        bundle: ArtifactBundle {
            artifacts,
            required_paths,
            identity_sha256,
            install_constraints,
        },
        runtime: SHERPA_RUNTIME,
        device: DeviceRequirement::AnyDesktop,
        qualification_policy: QualificationPolicy::StructuralWithPinnedRuntime,
        source: ModelSource {
            repository_url: "https://github.com/openai/whisper",
            model_card_url: "https://github.com/openai/whisper",
            license_spdx: "MIT",
            provenance: "OpenAI Whisper model exported and distributed by sherpa-onnx.",
        },
    }
}

const VAD_ARTIFACTS: &[ArtifactFile] = &[ArtifactFile {
    artifact_id: "silero-vad-onnx",
    source_repository: SHERPA_REPOSITORY,
    source_model: "silero_vad.onnx",
    source_endpoint: SHERPA_RELEASE_API,
    resolved_url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/silero_vad.onnx",
    revision: "github-release-asset:271935959",
    bytes: 643_854,
    sha256: "9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6",
    required_path: "silero_vad.onnx",
    required: true,
    install_mode: ArtifactInstallMode::Direct,
    license_spdx: "MIT",
    provenance: "Silero VAD ONNX model distributed by sherpa-onnx; detector defaults are frozen from sherpa-onnx 1.13.5 source headers at commit 3dc7c569f31ca2cd4a20ed6f7db780327e6714c5.",
    redirect_hosts: GITHUB_REDIRECT_HOSTS,
}];
const VAD_INSTALL_FILES: &[RequiredInstallFile] = &[RequiredInstallFile {
    path: "silero_vad.onnx",
    bytes: 643_854,
    sha256: "9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6",
}];
const VAD_INSTALL_CONSTRAINTS: InstallConstraints =
    InstallConstraints::Direct(DirectInstallConstraints {
        max_written_file_bytes: 643_854,
        max_total_written_bytes: 643_854,
        required_files: VAD_INSTALL_FILES,
    });

static REGISTRY: ModelRegistry = ModelRegistry::new(MODELS);
static VAD: VadManifest = VadManifest {
    id: "silero-vad-2024-01-17",
    manifest_version: "1",
    bundle: ArtifactBundle {
        artifacts: VAD_ARTIFACTS,
        required_paths: &["silero_vad.onnx"],
        identity_sha256: "9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6",
        install_constraints: VAD_INSTALL_CONSTRAINTS,
    },
    runtime: SHERPA_RUNTIME,
    qualification_policy: QualificationPolicy::StructuralWithPinnedRuntime,
    source: ModelSource {
        repository_url: "https://github.com/snakers4/silero-vad",
        model_card_url: "https://github.com/snakers4/silero-vad",
        license_spdx: "MIT",
        provenance: "Official Silero VAD ONNX asset redistributed by sherpa-onnx.",
    },
    sherpa_onnx_version: PINNED_SHERPA_RUNTIME.version,
    sherpa_onnx_commit: PINNED_SHERPA_RUNTIME.git_commit,
    silero_config_source_header: VAD_SILERO_SOURCE_HEADER,
    vad_config_source_header: VAD_SOURCE_HEADER,
    threshold: VAD_THRESHOLD,
    min_silence_duration_seconds: VAD_MIN_SILENCE_SECONDS,
    min_speech_duration_seconds: VAD_MIN_SPEECH_SECONDS,
    max_speech_duration_seconds: VAD_MAX_SPEECH_SECONDS,
    window_size_samples: VAD_WINDOW_SIZE_SAMPLES,
    sample_rate_hz: VAD_SAMPLE_RATE_HZ,
    num_threads: VAD_NUM_THREADS,
    provider: VAD_PROVIDER,
};
