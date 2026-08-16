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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeRequirement {
    SherpaOnnx {
        crate_version: &'static str,
        git_commit: &'static str,
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
        && vad.sherpa_onnx_version == VAD_SHERPA_VERSION
        && vad.sherpa_onnx_commit == VAD_SHERPA_COMMIT
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
            cargo_feature,
        } => json!({
            "cargo_feature": cargo_feature,
            "crate": "sherpa-onnx",
            "git_commit": git_commit,
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

const SHERPA_RUNTIME: RuntimeRequirement = RuntimeRequirement::SherpaOnnx {
    crate_version: "1.13.5",
    git_commit: "3dc7c569f31ca2cd4a20ed6f7db780327e6714c5",
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
const VAD_SHERPA_VERSION: &str = "1.13.5";
const VAD_SHERPA_COMMIT: &str = "3dc7c569f31ca2cd4a20ed6f7db780327e6714c5";
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
const LANG_WHISPER: &[&str] = &["auto", "zh", "en", "ja", "ko", "yue", "multilingual"];
const LANG_QWEN: &[&str] = &[
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
    ),
    whisper_model(
        "whisper-base",
        "Whisper Base",
        WHISPER_BASE_ARTIFACTS,
        WHISPER_BASE_PATHS,
        "911b2083efd7c0dca2ac3b358b75222660dc09fb716d64fbfc417ba6c99ff3de",
    ),
    whisper_model(
        "whisper-small",
        "Whisper Small",
        WHISPER_SMALL_ARTIFACTS,
        WHISPER_SMALL_PATHS,
        "486a46afbb7ba798507190ffe02fea2dd726049af212e774537efac6afb210a6",
    ),
    ModelManifest {
        id: "qwen3-asr-0.6b-int8-2026-03-25",
        manifest_version: "1",
        display_name: "Qwen3-ASR 0.6B INT8",
        provider: AsrProviderKind::Qwen3Asr,
        supported_languages: LANG_QWEN,
        bundle: ArtifactBundle {
            artifacts: QWEN06_ARTIFACTS,
            required_paths: QWEN06_PATHS,
            identity_sha256: "393f8a14e2f5fb96746aaab342997a40641001fbd5bf9592a080a8329178ee96",
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
        supported_languages: LANG_QWEN,
        bundle: ArtifactBundle {
            artifacts: QWEN17_ARTIFACTS,
            required_paths: QWEN17_PATHS,
            identity_sha256: QWEN17_IDENTITY,
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

static REGISTRY: ModelRegistry = ModelRegistry::new(MODELS);
static VAD: VadManifest = VadManifest {
    id: "silero-vad-2024-01-17",
    manifest_version: "1",
    bundle: ArtifactBundle {
        artifacts: VAD_ARTIFACTS,
        required_paths: &["silero_vad.onnx"],
        identity_sha256: "9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6",
    },
    runtime: SHERPA_RUNTIME,
    qualification_policy: QualificationPolicy::StructuralWithPinnedRuntime,
    source: ModelSource {
        repository_url: "https://github.com/snakers4/silero-vad",
        model_card_url: "https://github.com/snakers4/silero-vad",
        license_spdx: "MIT",
        provenance: "Official Silero VAD ONNX asset redistributed by sherpa-onnx.",
    },
    sherpa_onnx_version: VAD_SHERPA_VERSION,
    sherpa_onnx_commit: VAD_SHERPA_COMMIT,
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
