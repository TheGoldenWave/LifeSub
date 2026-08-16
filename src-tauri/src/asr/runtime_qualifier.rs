use std::fs;
#[cfg(any(
    test,
    all(
        feature = "asr-qwen17-runtime",
        target_os = "macos",
        target_arch = "aarch64"
    )
))]
use std::fs::File;
#[cfg(any(
    test,
    all(
        feature = "asr-qwen17-runtime",
        target_os = "macos",
        target_arch = "aarch64"
    )
))]
use std::fs::OpenOptions;
#[cfg(any(
    test,
    all(
        feature = "asr-qwen17-runtime",
        target_os = "macos",
        target_arch = "aarch64"
    )
))]
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[cfg(any(
    test,
    all(
        feature = "asr-qwen17-runtime",
        target_os = "macos",
        target_arch = "aarch64"
    )
))]
#[cfg(target_os = "macos")]
use std::ffi::CString;
#[cfg(any(
    test,
    all(
        feature = "asr-qwen17-runtime",
        target_os = "macos",
        target_arch = "aarch64"
    )
))]
#[cfg(target_os = "macos")]
use std::os::unix::ffi::OsStrExt;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[cfg(any(
    test,
    all(
        feature = "asr-qwen17-runtime",
        target_os = "macos",
        target_arch = "aarch64"
    )
))]
use unicode_normalization::UnicodeNormalization;

use crate::asr::manifest::ModelManifest;
use crate::asr::provider::DeviceIdentity;

pub const RUNTIME_QUALIFICATION_MARKER: &str = ".lifesub-runtime-qualified.json";
pub const QUALIFICATION_SMOKE_FIXTURE_SHA256: &str =
    "9ea167a7ec7959cfad379f8edd6732098177d57d29b50737aa1f3f0ec84e819a";
pub const QUALIFICATION_CONTRACT_SHA256: &str =
    "b96f1f2f268ae54694e4d2a6a036e3ac8a94759db389e47e1005387239147006";
const QUALIFICATION_SPEECH_FILE: &str = "qwen06-codeswitch.wav";
const QUALIFICATION_AUDIO_FILE_SHA256: &str =
    "2def7fa41004d0a7d148d4afbf4c467c9d112d8b373996123e9a4c43d94957c7";
const QUALIFICATION_SOURCE_ARCHIVE_SHA256: &str =
    "393f8a14e2f5fb96746aaab342997a40641001fbd5bf9592a080a8329178ee96";
const QUALIFICATION_SOURCE_ARCHIVE_PATH: &str =
    "sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25.tar.bz2::test_wavs/codeswitch.wav";
const QUALIFICATION_LICENSE_SPDX: &str = "Apache-2.0";
const QUALIFICATION_PROVENANCE: &str = "Official sherpa-onnx Qwen3-ASR 0.6B release test_wavs/codeswitch.wav; copied from verified archive cache.";
const QUALIFICATION_EXPECTED_TEXT: &str =
    "I'm alone, all by myself. Je suis tout seul. Sono tutto. Estoy solo.";
const QUALIFICATION_EXPECTED_PHRASES: [&str; 4] =
    ["I'm alone", "Je suis tout seul", "Sono tutto", "Estoy solo"];
const QUALIFICATION_MINIMUM_PHRASE_MATCHES: usize = 2;
const QUALIFICATION_NORMALIZATION: &str = "unicode-nfkc-alphanumeric-lowercase-v1";
const QUALIFICATION_CONTRACT_SCHEMA: &str = "lifesub.qwen17-qualification-contract.v1";
const QUALIFICATION_FAILED: &str = "model_runtime_qualification_failed";
const QUALIFICATION_RECOVERY_REQUIRED: &str = "model_runtime_qualification_recovery_required";

#[derive(Clone, Debug, PartialEq)]
pub struct QualificationSpeechFixture {
    pub samples: Vec<f32>,
    pub sample_rate_hz: u32,
    pub expected_text: String,
    pub expected_phrases: Vec<String>,
    pub minimum_phrase_matches: usize,
    pub normalization: String,
    pub source_sha256: String,
    pub canonical_sample_sha256: String,
    pub contract_sha256: String,
}

#[derive(Deserialize, Serialize)]
struct QualificationContract {
    audio_file_sha256: String,
    canonical_sample_sha256: String,
    expected_phrases: Vec<String>,
    expected_text: String,
    license_spdx: String,
    minimum_phrase_matches: usize,
    normalization: String,
    provenance: String,
    require_nonempty: bool,
    schema: String,
    source_archive_path: String,
    source_archive_sha256: String,
}

pub fn load_qualification_speech_fixture() -> Result<QualificationSpeechFixture, QualifierError> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/asr");
    load_qualification_speech_fixture_from(&root)
}

pub(crate) fn load_qualification_speech_fixture_from(
    root: &Path,
) -> Result<QualificationSpeechFixture, QualifierError> {
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("fixtures.json")).map_err(fixture_error)?)
            .map_err(fixture_error)?;
    let row = manifest["fixtures"]
        .as_array()
        .and_then(|rows| {
            rows.iter()
                .find(|row| row["file"].as_str() == Some(QUALIFICATION_SPEECH_FILE))
        })
        .ok_or_else(|| fixture_error("qualification speech fixture manifest row is missing"))?;
    let contract = QualificationContract {
        audio_file_sha256: required_string(row, "sha256")?,
        canonical_sample_sha256: required_string(row, "canonical_sample_sha256")?,
        expected_phrases: required_strings(row, "expected_phrases")?,
        expected_text: required_string(row, "expected_text")?,
        license_spdx: required_string(row, "license_spdx")?,
        minimum_phrase_matches: row["minimum_phrase_matches"]
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| fixture_error("qualification minimum phrase matches is invalid"))?,
        normalization: required_string(row, "normalization")?,
        provenance: required_string(row, "provenance")?,
        require_nonempty: row["require_nonempty"]
            .as_bool()
            .ok_or_else(|| fixture_error("qualification non-empty contract is missing"))?,
        schema: QUALIFICATION_CONTRACT_SCHEMA.to_owned(),
        source_archive_path: required_string(row, "source_archive_path")?,
        source_archive_sha256: required_string(row, "source_archive_sha256")?,
    };
    validate_frozen_contract(&contract, row)?;
    let path = root.join(QUALIFICATION_SPEECH_FILE);
    let bytes = fs::read(&path).map_err(fixture_error)?;
    let source_sha256 = hex::encode(Sha256::digest(&bytes));
    if contract.audio_file_sha256 != source_sha256 {
        return Err(fixture_error("qualification speech fixture hash mismatch"));
    }
    let audio = crate::asr::audio::decode_to_working_audio(&path)
        .map_err(|error| fixture_error(format!("qualification speech decode failed: {error:?}")))?;
    if audio.sample_rate_hz != 16_000
        || audio.samples.is_empty()
        || audio.samples.len() > 25 * 16_000
    {
        return Err(fixture_error(
            "qualification speech fixture must decode to non-empty <=25s 16k PCM",
        ));
    }
    let canonical_sample_sha256 = canonical_sample_hash(&audio.samples);
    let expected_canonical = &contract.canonical_sample_sha256;
    if expected_canonical != &canonical_sample_sha256
        || expected_canonical != QUALIFICATION_SMOKE_FIXTURE_SHA256
    {
        return Err(fixture_error(format!(
            "qualification canonical sample hash mismatch: observed {canonical_sample_sha256}"
        )));
    }
    Ok(QualificationSpeechFixture {
        samples: audio.samples,
        sample_rate_hz: audio.sample_rate_hz,
        expected_text: contract.expected_text,
        expected_phrases: contract.expected_phrases,
        minimum_phrase_matches: contract.minimum_phrase_matches,
        normalization: contract.normalization,
        source_sha256,
        canonical_sample_sha256,
        contract_sha256: QUALIFICATION_CONTRACT_SHA256.to_owned(),
    })
}

fn required_string(row: &serde_json::Value, field: &str) -> Result<String, QualifierError> {
    row[field]
        .as_str()
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| {
            fixture_error(format!(
                "qualification speech fixture field {field} is missing"
            ))
        })
}

fn required_strings(row: &serde_json::Value, field: &str) -> Result<Vec<String>, QualifierError> {
    row[field]
        .as_array()
        .ok_or_else(|| {
            fixture_error(format!(
                "qualification speech fixture field {field} is missing"
            ))
        })?
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| {
                    fixture_error(format!(
                        "qualification speech fixture field {field} is invalid"
                    ))
                })
        })
        .collect()
}

fn validate_frozen_contract(
    contract: &QualificationContract,
    row: &serde_json::Value,
) -> Result<(), QualifierError> {
    let expected_phrases = QUALIFICATION_EXPECTED_PHRASES.map(str::to_owned).to_vec();
    if contract.audio_file_sha256 != QUALIFICATION_AUDIO_FILE_SHA256
        || contract.canonical_sample_sha256 != QUALIFICATION_SMOKE_FIXTURE_SHA256
        || contract.expected_phrases != expected_phrases
        || contract.expected_text != QUALIFICATION_EXPECTED_TEXT
        || contract.license_spdx != QUALIFICATION_LICENSE_SPDX
        || contract.minimum_phrase_matches != QUALIFICATION_MINIMUM_PHRASE_MATCHES
        || contract.normalization != QUALIFICATION_NORMALIZATION
        || contract.provenance != QUALIFICATION_PROVENANCE
        || !contract.require_nonempty
        || contract.source_archive_path != QUALIFICATION_SOURCE_ARCHIVE_PATH
        || contract.source_archive_sha256 != QUALIFICATION_SOURCE_ARCHIVE_SHA256
    {
        return Err(fixture_error(
            "qualification speech contract differs from the frozen contract",
        ));
    }
    let bytes = serde_json_canonicalizer::to_vec(contract).map_err(fixture_error)?;
    let digest = hex::encode(Sha256::digest(bytes));
    if digest != QUALIFICATION_CONTRACT_SHA256
        || row["qualification_contract_sha256"].as_str() != Some(digest.as_str())
    {
        return Err(fixture_error(
            "qualification speech contract digest mismatch",
        ));
    }
    Ok(())
}

#[cfg(any(
    test,
    all(
        feature = "asr-qwen17-runtime",
        target_os = "macos",
        target_arch = "aarch64"
    )
))]
pub(crate) fn normalize_qualification_text(value: &str) -> String {
    value
        .nfkc()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn canonical_sample_hash(samples: &[f32]) -> String {
    let mut hasher = Sha256::new();
    for sample in samples {
        hasher.update(sample.to_le_bytes());
    }
    hex::encode(hasher.finalize())
}

fn fixture_error(detail: impl std::fmt::Display) -> QualifierError {
    QualifierError::new(QUALIFICATION_FAILED, detail.to_string())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QualificationHandle {
    pub model_id: String,
    pub manifest_version: String,
    pub bundle_identity: String,
    pub install_dir: PathBuf,
    pub device: DeviceIdentity,
}

impl QualificationHandle {
    pub fn from_manifest(
        manifest: &ModelManifest,
        install_dir: impl AsRef<Path>,
        device: DeviceIdentity,
    ) -> Self {
        Self {
            model_id: manifest.id.to_owned(),
            manifest_version: manifest.manifest_version.to_owned(),
            bundle_identity: manifest.bundle.identity_sha256.to_owned(),
            install_dir: install_dir.as_ref().to_path_buf(),
            device,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct QualifiedRuntimeIdentity {
    pub crate_name: String,
    pub crate_version: String,
    pub git_commit: String,
    pub candle_version: String,
    pub backend: String,
    pub target_os: String,
    pub target_arch: String,
    pub device_index: u16,
    pub device_name: String,
    pub smoke_fixture_sha256: String,
    pub qualification_contract_sha256: String,
}

#[cfg(any(
    test,
    all(
        feature = "asr-qwen17-runtime",
        target_os = "macos",
        target_arch = "aarch64"
    )
))]
pub(crate) trait RuntimeSmoke: Send + Sync {
    fn smoke(&self, handle: &QualificationHandle) -> Result<QualifiedRuntimeIdentity, String>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QualificationRecord {
    pub state: String,
    pub manifest_version: String,
    pub bundle_identity: String,
    pub install_dir: PathBuf,
    pub runtime_identity_json: Option<String>,
}

pub trait QualificationCatalog: Send + Sync {
    fn qualification_record(
        &self,
        model_id: &str,
    ) -> Result<Option<QualificationRecord>, QualifierError>;
    fn cas_runtime_qualified(
        &self,
        handle: &QualificationHandle,
        runtime_identity_json: &str,
    ) -> Result<bool, QualifierError>;
    fn demote_runtime_qualification(
        &self,
        handle: &QualificationHandle,
        error_code: &str,
    ) -> Result<bool, QualifierError>;
    fn record_qualification_error(
        &self,
        handle: &QualificationHandle,
        error_code: &str,
    ) -> Result<(), QualifierError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QualifierError {
    code: &'static str,
    detail: String,
}

impl QualifierError {
    pub fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }

    pub const fn code(&self) -> &'static str {
        self.code
    }
}

impl std::fmt::Display for QualifierError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.detail)
    }
}

impl std::error::Error for QualifierError {}

impl From<std::io::Error> for QualifierError {
    fn from(error: std::io::Error) -> Self {
        Self::new(QUALIFICATION_RECOVERY_REQUIRED, error.to_string())
    }
}

impl From<serde_json::Error> for QualifierError {
    fn from(error: serde_json::Error) -> Self {
        Self::new(QUALIFICATION_RECOVERY_REQUIRED, error.to_string())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarkerFault {
    Write,
    Sync,
    Rename,
    AfterDurableRename,
}

#[derive(Clone)]
#[cfg(any(
    test,
    all(
        feature = "asr-qwen17-runtime",
        target_os = "macos",
        target_arch = "aarch64"
    )
))]
pub(crate) struct RuntimeQualifier<C, S> {
    catalog: C,
    smoke: S,
    marker_fault: Option<MarkerFault>,
}

#[cfg(any(
    test,
    all(
        feature = "asr-qwen17-runtime",
        target_os = "macos",
        target_arch = "aarch64"
    )
))]
impl<C: QualificationCatalog, S: RuntimeSmoke> RuntimeQualifier<C, S> {
    pub fn new(catalog: C, smoke: S) -> Self {
        Self {
            catalog,
            smoke,
            marker_fault: None,
        }
    }

    #[cfg(test)]
    pub fn with_marker_fault(mut self, fault: MarkerFault) -> Self {
        self.marker_fault = Some(fault);
        self
    }

    pub fn qualify(
        &self,
        handle: &QualificationHandle,
    ) -> Result<QualifiedRuntimeIdentity, QualifierError> {
        let record = self.current_record(handle)?;
        if let Some(marker) = read_marker(handle)? {
            if marker.matches_handle(handle) {
                let runtime_json = serde_json::to_string(&marker.runtime)?;
                if record.state == "runtime_qualified"
                    && record.runtime_identity_json.as_deref() == Some(runtime_json.as_str())
                {
                    return Ok(marker.runtime);
                }
                if self.catalog.cas_runtime_qualified(handle, &runtime_json)? {
                    return Ok(marker.runtime);
                }
                return self.accept_concurrent_winner(handle, &marker.runtime);
            }
            self.catalog
                .record_qualification_error(handle, QUALIFICATION_RECOVERY_REQUIRED)?;
            return Err(QualifierError::new(
                QUALIFICATION_RECOVERY_REQUIRED,
                "qualification marker identity mismatch",
            ));
        }

        if record.state == "runtime_qualified" {
            self.catalog
                .demote_runtime_qualification(handle, QUALIFICATION_RECOVERY_REQUIRED)?;
        }
        let runtime = self.smoke.smoke(handle).map_err(|detail| {
            let _ = self
                .catalog
                .record_qualification_error(handle, QUALIFICATION_FAILED);
            QualifierError::new(QUALIFICATION_FAILED, detail)
        })?;
        validate_runtime_identity(handle, &runtime).inspect_err(|_error| {
            let _ = self
                .catalog
                .record_qualification_error(handle, QUALIFICATION_FAILED);
        })?;
        let marker = QualificationMarker::new(handle, runtime.clone());
        if let Err(error) = publish_marker(handle, &marker, self.marker_fault) {
            self.catalog
                .record_qualification_error(handle, QUALIFICATION_RECOVERY_REQUIRED)?;
            return Err(error);
        }
        if self.marker_fault == Some(MarkerFault::AfterDurableRename) {
            return Err(QualifierError::new(
                QUALIFICATION_RECOVERY_REQUIRED,
                "simulated crash after durable marker",
            ));
        }
        let runtime_json = serde_json::to_string(&runtime)?;
        if self.catalog.cas_runtime_qualified(handle, &runtime_json)? {
            Ok(runtime)
        } else {
            self.accept_concurrent_winner(handle, &runtime)
        }
    }

    pub fn reconcile(&self, handle: &QualificationHandle) -> Result<(), QualifierError> {
        cleanup_stale_marker_temps(handle)?;
        let record = self.current_record(handle)?;
        let marker = read_marker(handle)?;
        match (record.state.as_str(), marker) {
            ("runtime_qualified", Some(marker)) if marker.matches_handle(handle) => {
                let runtime_json = serde_json::to_string(&marker.runtime)?;
                if record.runtime_identity_json.as_deref() == Some(runtime_json.as_str()) {
                    Ok(())
                } else {
                    quarantine_marker(handle)?;
                    self.catalog
                        .demote_runtime_qualification(handle, QUALIFICATION_RECOVERY_REQUIRED)?;
                    Err(QualifierError::new(
                        QUALIFICATION_RECOVERY_REQUIRED,
                        "database and marker runtime identities differ",
                    ))
                }
            }
            ("runtime_qualified", Some(_)) => {
                quarantine_marker(handle)?;
                self.catalog
                    .demote_runtime_qualification(handle, QUALIFICATION_RECOVERY_REQUIRED)?;
                Ok(())
            }
            ("runtime_qualified", None) => {
                self.catalog
                    .demote_runtime_qualification(handle, QUALIFICATION_RECOVERY_REQUIRED)?;
                Ok(())
            }
            ("installed_unqualified", Some(marker)) if marker.matches_handle(handle) => {
                let runtime_json = serde_json::to_string(&marker.runtime)?;
                if self.catalog.cas_runtime_qualified(handle, &runtime_json)? {
                    Ok(())
                } else {
                    self.accept_concurrent_winner(handle, &marker.runtime)
                        .map(|_| ())
                }
            }
            ("installed_unqualified", Some(_)) => {
                quarantine_marker(handle)?;
                self.catalog
                    .record_qualification_error(handle, QUALIFICATION_RECOVERY_REQUIRED)
            }
            ("installed_unqualified", None) => Ok(()),
            _ => Err(QualifierError::new(
                QUALIFICATION_RECOVERY_REQUIRED,
                "installation is not qualifiable",
            )),
        }
    }

    fn current_record(
        &self,
        handle: &QualificationHandle,
    ) -> Result<QualificationRecord, QualifierError> {
        let record = self
            .catalog
            .qualification_record(&handle.model_id)?
            .ok_or_else(|| {
                QualifierError::new(QUALIFICATION_RECOVERY_REQUIRED, "installation is missing")
            })?;
        if record.manifest_version != handle.manifest_version
            || record.bundle_identity != handle.bundle_identity
            || record.install_dir != handle.install_dir
        {
            return Err(QualifierError::new(
                QUALIFICATION_RECOVERY_REQUIRED,
                "installation handle identity mismatch",
            ));
        }
        Ok(record)
    }

    fn accept_concurrent_winner(
        &self,
        handle: &QualificationHandle,
        runtime: &QualifiedRuntimeIdentity,
    ) -> Result<QualifiedRuntimeIdentity, QualifierError> {
        let record = self.current_record(handle)?;
        let expected = serde_json::to_string(runtime)?;
        if record.state == "runtime_qualified"
            && record.runtime_identity_json.as_deref() == Some(expected.as_str())
        {
            Ok(runtime.clone())
        } else {
            Err(QualifierError::new(
                QUALIFICATION_RECOVERY_REQUIRED,
                "qualification CAS lost to conflicting identity",
            ))
        }
    }
}

#[cfg(any(
    test,
    all(
        feature = "asr-qwen17-runtime",
        target_os = "macos",
        target_arch = "aarch64"
    )
))]
impl<T: RuntimeSmoke + ?Sized> RuntimeSmoke for Arc<T> {
    fn smoke(&self, handle: &QualificationHandle) -> Result<QualifiedRuntimeIdentity, String> {
        (**self).smoke(handle)
    }
}

#[cfg(any(
    test,
    all(
        feature = "asr-qwen17-runtime",
        target_os = "macos",
        target_arch = "aarch64"
    )
))]
impl<T: RuntimeSmoke + ?Sized> RuntimeSmoke for &T {
    fn smoke(&self, handle: &QualificationHandle) -> Result<QualifiedRuntimeIdentity, String> {
        (**self).smoke(handle)
    }
}

impl<T: QualificationCatalog + ?Sized> QualificationCatalog for Arc<T> {
    fn qualification_record(
        &self,
        model_id: &str,
    ) -> Result<Option<QualificationRecord>, QualifierError> {
        (**self).qualification_record(model_id)
    }

    fn cas_runtime_qualified(
        &self,
        handle: &QualificationHandle,
        runtime_identity_json: &str,
    ) -> Result<bool, QualifierError> {
        (**self).cas_runtime_qualified(handle, runtime_identity_json)
    }

    fn demote_runtime_qualification(
        &self,
        handle: &QualificationHandle,
        error_code: &str,
    ) -> Result<bool, QualifierError> {
        (**self).demote_runtime_qualification(handle, error_code)
    }

    fn record_qualification_error(
        &self,
        handle: &QualificationHandle,
        error_code: &str,
    ) -> Result<(), QualifierError> {
        (**self).record_qualification_error(handle, error_code)
    }
}

impl<T: QualificationCatalog + ?Sized> QualificationCatalog for &T {
    fn qualification_record(
        &self,
        model_id: &str,
    ) -> Result<Option<QualificationRecord>, QualifierError> {
        (**self).qualification_record(model_id)
    }

    fn cas_runtime_qualified(
        &self,
        handle: &QualificationHandle,
        runtime_identity_json: &str,
    ) -> Result<bool, QualifierError> {
        (**self).cas_runtime_qualified(handle, runtime_identity_json)
    }

    fn demote_runtime_qualification(
        &self,
        handle: &QualificationHandle,
        error_code: &str,
    ) -> Result<bool, QualifierError> {
        (**self).demote_runtime_qualification(handle, error_code)
    }

    fn record_qualification_error(
        &self,
        handle: &QualificationHandle,
        error_code: &str,
    ) -> Result<(), QualifierError> {
        (**self).record_qualification_error(handle, error_code)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct QualificationMarker {
    schema: String,
    model_id: String,
    manifest_version: String,
    bundle_identity: String,
    device: DeviceIdentityMarker,
    runtime: QualifiedRuntimeIdentity,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct DeviceIdentityMarker {
    os: String,
    arch: String,
    backend: String,
    device_index: u16,
    macos_major: u16,
    memory_gib: u16,
    chip: String,
}

impl QualificationMarker {
    #[cfg(any(
        test,
        all(
            feature = "asr-qwen17-runtime",
            target_os = "macos",
            target_arch = "aarch64"
        )
    ))]
    fn new(handle: &QualificationHandle, runtime: QualifiedRuntimeIdentity) -> Self {
        Self {
            schema: "lifesub.runtime-qualification.v1".to_owned(),
            model_id: handle.model_id.clone(),
            manifest_version: handle.manifest_version.clone(),
            bundle_identity: handle.bundle_identity.clone(),
            device: DeviceIdentityMarker {
                os: handle.device.os.clone(),
                arch: handle.device.arch.clone(),
                backend: handle.device.backend.clone(),
                device_index: handle.device.device_index,
                macos_major: handle.device.macos_major,
                memory_gib: handle.device.memory_gib,
                chip: handle.device.chip.clone(),
            },
            runtime,
        }
    }

    fn matches_handle(&self, handle: &QualificationHandle) -> bool {
        self.schema == "lifesub.runtime-qualification.v1"
            && self.model_id == handle.model_id
            && self.manifest_version == handle.manifest_version
            && self.bundle_identity == handle.bundle_identity
            && self.device.os == handle.device.os
            && self.device.arch == handle.device.arch
            && self.device.backend == handle.device.backend
            && self.device.device_index == handle.device.device_index
            && self.device.macos_major == handle.device.macos_major
            && self.device.memory_gib == handle.device.memory_gib
            && self.device.chip == handle.device.chip
            && validate_runtime_identity(handle, &self.runtime).is_ok()
    }
}

fn validate_runtime_identity(
    handle: &QualificationHandle,
    runtime: &QualifiedRuntimeIdentity,
) -> Result<(), QualifierError> {
    if runtime.crate_name != "qwen3-asr"
        || runtime.crate_version != "0.2.2"
        || runtime.git_commit != "c5ef09646af6278d2ba8b8ceaf543ffb32d1a5dc"
        || runtime.candle_version != "0.9.2"
        || runtime.backend != "metal"
        || runtime.target_os != handle.device.os
        || runtime.target_arch != handle.device.arch
        || runtime.device_index != handle.device.device_index
        || runtime.smoke_fixture_sha256 != QUALIFICATION_SMOKE_FIXTURE_SHA256
        || runtime.qualification_contract_sha256 != QUALIFICATION_CONTRACT_SHA256
    {
        return Err(QualifierError::new(
            QUALIFICATION_FAILED,
            "runtime smoke returned an unexpected identity",
        ));
    }
    Ok(())
}

fn marker_path(handle: &QualificationHandle) -> PathBuf {
    handle.install_dir.join(RUNTIME_QUALIFICATION_MARKER)
}

fn read_marker(
    handle: &QualificationHandle,
) -> Result<Option<QualificationMarker>, QualifierError> {
    match fs::read(marker_path(handle)) {
        Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

pub(crate) fn verify_runtime_qualification_marker(
    manifest: &ModelManifest,
    install_dir: impl AsRef<Path>,
    expected_runtime_identity_json: &str,
    device: DeviceIdentity,
) -> Result<(), QualifierError> {
    let handle = QualificationHandle::from_manifest(manifest, install_dir, device);
    let marker = read_marker(&handle)?.ok_or_else(|| {
        QualifierError::new(
            QUALIFICATION_RECOVERY_REQUIRED,
            "qualification marker is missing",
        )
    })?;
    if !marker.matches_handle(&handle)
        || serde_json::to_string(&marker.runtime)? != expected_runtime_identity_json
    {
        return Err(QualifierError::new(
            QUALIFICATION_RECOVERY_REQUIRED,
            "qualification marker runtime identity mismatch",
        ));
    }
    Ok(())
}

#[cfg(any(
    test,
    all(
        feature = "asr-qwen17-runtime",
        target_os = "macos",
        target_arch = "aarch64"
    )
))]
fn publish_marker(
    handle: &QualificationHandle,
    marker: &QualificationMarker,
    fault: Option<MarkerFault>,
) -> Result<(), QualifierError> {
    let final_path = marker_path(handle);
    let temp_path = handle.install_dir.join(format!(
        "{RUNTIME_QUALIFICATION_MARKER}.{}.tmp",
        uuid::Uuid::new_v4().simple()
    ));
    let result = (|| {
        if fault == Some(MarkerFault::Write) {
            return Err(QualifierError::new(
                QUALIFICATION_RECOVERY_REQUIRED,
                "simulated marker write failure",
            ));
        }
        let bytes = serde_json::to_vec(marker)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)?;
        file.write_all(&bytes)?;
        if fault == Some(MarkerFault::Sync) {
            return Err(QualifierError::new(
                QUALIFICATION_RECOVERY_REQUIRED,
                "simulated marker sync failure",
            ));
        }
        file.sync_all()?;
        drop(file);
        if fault == Some(MarkerFault::Rename) {
            return Err(QualifierError::new(
                QUALIFICATION_RECOVERY_REQUIRED,
                "simulated marker rename failure",
            ));
        }
        match publish_exclusive(&temp_path, &final_path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let existing: QualificationMarker =
                    serde_json::from_slice(&fs::read(&final_path)?)?;
                if existing != *marker {
                    return Err(QualifierError::new(
                        QUALIFICATION_RECOVERY_REQUIRED,
                        "conflicting immutable qualification marker",
                    ));
                }
            }
            Err(error) => return Err(error.into()),
        }
        File::open(&handle.install_dir)?.sync_all()?;
        Ok(())
    })();
    if temp_path.exists() {
        fs::remove_file(&temp_path)?;
        File::open(&handle.install_dir)?.sync_all()?;
    }
    result
}

#[cfg(any(
    test,
    all(
        feature = "asr-qwen17-runtime",
        target_os = "macos",
        target_arch = "aarch64"
    )
))]
fn cleanup_stale_marker_temps(handle: &QualificationHandle) -> Result<(), QualifierError> {
    let prefix = format!("{RUNTIME_QUALIFICATION_MARKER}.");
    let mut removed = false;
    for entry in fs::read_dir(&handle.install_dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some(uuid) = name
            .strip_prefix(&prefix)
            .and_then(|name| name.strip_suffix(".tmp"))
        else {
            continue;
        };
        if uuid.len() == 32 && uuid.chars().all(|character| character.is_ascii_hexdigit()) {
            let metadata = entry.file_type()?;
            if metadata.is_file() {
                fs::remove_file(entry.path())?;
                removed = true;
            }
        }
    }
    if removed {
        File::open(&handle.install_dir)?.sync_all()?;
    }
    Ok(())
}

#[cfg(any(
    test,
    all(
        feature = "asr-qwen17-runtime",
        target_os = "macos",
        target_arch = "aarch64"
    )
))]
#[cfg(target_os = "macos")]
fn publish_exclusive(source: &Path, destination: &Path) -> std::io::Result<()> {
    let source = CString::new(source.as_os_str().as_bytes()).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "marker temp path contains NUL",
        )
    })?;
    let destination = CString::new(destination.as_os_str().as_bytes()).map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "marker path contains NUL")
    })?;
    // RENAME_EXCL is the macOS atomic no-replace form required for immutable markers.
    let result = unsafe {
        libc::renameatx_np(
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
            destination.as_ptr(),
            libc::RENAME_EXCL,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(any(
    test,
    all(
        feature = "asr-qwen17-runtime",
        target_os = "macos",
        target_arch = "aarch64"
    )
))]
#[cfg(not(target_os = "macos"))]
fn publish_exclusive(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::hard_link(source, destination)?;
    fs::remove_file(source)
}

#[cfg(any(
    test,
    all(
        feature = "asr-qwen17-runtime",
        target_os = "macos",
        target_arch = "aarch64"
    )
))]
fn quarantine_marker(handle: &QualificationHandle) -> Result<(), QualifierError> {
    let marker = marker_path(handle);
    if marker.exists() {
        let quarantine = handle.install_dir.join(format!(
            "{RUNTIME_QUALIFICATION_MARKER}.invalid.{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::rename(marker, quarantine)?;
        File::open(&handle.install_dir)?.sync_all()?;
    }
    Ok(())
}
