use chrono::{DateTime, Utc};
use serde::de::Error as _;
use serde::{Deserialize, Serialize};

use crate::domain::AsrProviderKind;

const SHA256_HEX_LENGTH: usize = 64;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DataDestination {
    LocalDevice,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderOutcome {
    Succeeded,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct ProviderReceiptDraft {
    pub job_id: String,
    pub chunk_id: String,
    pub provider: AsrProviderKind,
    pub model_id: String,
    pub manifest_version: String,
    pub archive_sha256: String,
    pub required_file_hashes_json: String,
    pub model_source_json: String,
    pub vad_model_id: Option<String>,
    pub vad_manifest_version: Option<String>,
    pub vad_archive_sha256: Option<String>,
    pub vad_required_file_hashes_json: Option<String>,
    pub runtime_version: String,
    pub runtime_build_id: String,
    pub parameters_json: String,
    pub input_sha256: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub data_destination: DataDestination,
    pub outcome: ProviderOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProviderReceipt {
    job_id: String,
    chunk_id: String,
    provider: AsrProviderKind,
    model_id: String,
    manifest_version: String,
    archive_sha256: String,
    required_file_hashes_json: String,
    model_source_json: String,
    vad_model_id: Option<String>,
    vad_manifest_version: Option<String>,
    vad_archive_sha256: Option<String>,
    vad_required_file_hashes_json: Option<String>,
    runtime_version: String,
    runtime_build_id: String,
    parameters_json: String,
    input_sha256: String,
    started_at: DateTime<Utc>,
    finished_at: DateTime<Utc>,
    data_destination: DataDestination,
    outcome: ProviderOutcome,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderReceiptError {
    EmptyField(&'static str),
    InvalidSha256(&'static str),
    InvalidJson(&'static str),
    ExpectedJsonObject(&'static str),
    PartialVadIdentity,
    FinishedBeforeStarted,
}

impl TryFrom<ProviderReceiptDraft> for ProviderReceipt {
    type Error = ProviderReceiptError;

    fn try_from(draft: ProviderReceiptDraft) -> Result<Self, Self::Error> {
        validate_draft(&draft)?;
        Ok(Self {
            job_id: draft.job_id,
            chunk_id: draft.chunk_id,
            provider: draft.provider,
            model_id: draft.model_id,
            manifest_version: draft.manifest_version,
            archive_sha256: draft.archive_sha256,
            required_file_hashes_json: draft.required_file_hashes_json,
            model_source_json: draft.model_source_json,
            vad_model_id: draft.vad_model_id,
            vad_manifest_version: draft.vad_manifest_version,
            vad_archive_sha256: draft.vad_archive_sha256,
            vad_required_file_hashes_json: draft.vad_required_file_hashes_json,
            runtime_version: draft.runtime_version,
            runtime_build_id: draft.runtime_build_id,
            parameters_json: draft.parameters_json,
            input_sha256: draft.input_sha256,
            started_at: draft.started_at,
            finished_at: draft.finished_at,
            data_destination: draft.data_destination,
            outcome: draft.outcome,
        })
    }
}

impl<'de> Deserialize<'de> for ProviderReceipt {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let draft = ProviderReceiptDraft::deserialize(deserializer)?;
        Self::try_from(draft).map_err(D::Error::custom)
    }
}

impl ProviderReceipt {
    pub fn job_id(&self) -> &str {
        &self.job_id
    }

    pub fn chunk_id(&self) -> &str {
        &self.chunk_id
    }

    pub const fn provider(&self) -> AsrProviderKind {
        self.provider
    }

    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    pub const fn outcome(&self) -> ProviderOutcome {
        self.outcome
    }
}

impl std::fmt::Display for ProviderReceiptError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyField(field) => write!(formatter, "receipt field {field} must not be empty"),
            Self::InvalidSha256(field) => write!(formatter, "receipt field {field} is not SHA-256"),
            Self::InvalidJson(field) => write!(formatter, "receipt field {field} is invalid JSON"),
            Self::ExpectedJsonObject(field) => {
                write!(formatter, "receipt field {field} must be a JSON object")
            }
            Self::PartialVadIdentity => formatter.write_str("receipt VAD identity is incomplete"),
            Self::FinishedBeforeStarted => {
                formatter.write_str("receipt finished before it started")
            }
        }
    }
}

fn validate_draft(draft: &ProviderReceiptDraft) -> Result<(), ProviderReceiptError> {
    for (field, value) in [
        ("job_id", draft.job_id.as_str()),
        ("chunk_id", draft.chunk_id.as_str()),
        ("model_id", draft.model_id.as_str()),
        ("manifest_version", draft.manifest_version.as_str()),
        ("runtime_version", draft.runtime_version.as_str()),
        ("runtime_build_id", draft.runtime_build_id.as_str()),
    ] {
        validate_non_empty(field, value)?;
    }
    validate_sha256("archive_sha256", &draft.archive_sha256)?;
    validate_sha256("input_sha256", &draft.input_sha256)?;
    validate_json_object(
        "required_file_hashes_json",
        &draft.required_file_hashes_json,
    )?;
    validate_json_object("model_source_json", &draft.model_source_json)?;
    validate_json("parameters_json", &draft.parameters_json)?;
    validate_vad_identity(draft)?;
    if draft.finished_at < draft.started_at {
        return Err(ProviderReceiptError::FinishedBeforeStarted);
    }
    Ok(())
}

fn validate_vad_identity(draft: &ProviderReceiptDraft) -> Result<(), ProviderReceiptError> {
    match (
        draft.vad_model_id.as_deref(),
        draft.vad_manifest_version.as_deref(),
        draft.vad_archive_sha256.as_deref(),
        draft.vad_required_file_hashes_json.as_deref(),
    ) {
        (None, None, None, None) => Ok(()),
        (Some(model), Some(manifest), Some(hash), Some(file_hashes)) => {
            validate_non_empty("vad_model_id", model)?;
            validate_non_empty("vad_manifest_version", manifest)?;
            validate_sha256("vad_archive_sha256", hash)?;
            validate_json_object("vad_required_file_hashes_json", file_hashes)
        }
        _ => Err(ProviderReceiptError::PartialVadIdentity),
    }
}

fn validate_non_empty(field: &'static str, value: &str) -> Result<(), ProviderReceiptError> {
    if value.trim().is_empty() {
        return Err(ProviderReceiptError::EmptyField(field));
    }
    Ok(())
}

fn validate_sha256(field: &'static str, value: &str) -> Result<(), ProviderReceiptError> {
    if value.len() != SHA256_HEX_LENGTH || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ProviderReceiptError::InvalidSha256(field));
    }
    Ok(())
}

fn validate_json(
    field: &'static str,
    value: &str,
) -> Result<serde_json::Value, ProviderReceiptError> {
    serde_json::from_str(value).map_err(|_| ProviderReceiptError::InvalidJson(field))
}

fn validate_json_object(field: &'static str, value: &str) -> Result<(), ProviderReceiptError> {
    if !validate_json(field, value)?.is_object() {
        return Err(ProviderReceiptError::ExpectedJsonObject(field));
    }
    Ok(())
}
