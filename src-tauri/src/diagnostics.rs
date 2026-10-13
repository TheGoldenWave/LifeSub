use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{self, Read};
use std::path::Path;

const DIAGNOSTICS_SCHEMA: &str = "lifesub.diagnostics.v1";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DiagnosticPermissions {
    pub microphone: String,
    pub screen_recording: String,
}

impl DiagnosticPermissions {
    pub fn validate_state(value: &str) -> Result<(), String> {
        match value {
            "granted" | "denied" | "restricted" | "not_determined" | "not_granted" => Ok(()),
            _ => Err(format!("invalid diagnostic permission state {value}")),
        }
    }

    fn validate(&self) -> Result<(), String> {
        Self::validate_state(&self.microphone)?;
        Self::validate_state(&self.screen_recording)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DiagnosticSelectedMicrophone {
    pub uid: Option<String>,
    pub name: Option<String>,
    pub connected: bool,
    pub transport: Option<String>,
    pub probe_result: Option<String>,
    pub probe: Option<DiagnosticMicrophoneProbe>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DiagnosticMicrophoneProbe {
    pub uid: String,
    pub name: String,
    pub transport: String,
    pub input_sample_rate_hz: Option<u32>,
    pub input_channel_count: Option<u16>,
    pub read_back_uid: Option<String>,
    pub first_frame_sample_count: u64,
    pub elapsed_ms: u64,
    pub result: String,
}

impl DiagnosticMicrophoneProbe {
    fn validate(&self) -> Result<(), String> {
        if self.uid.trim().is_empty()
            || self.name.trim().is_empty()
            || !matches!(
                self.transport.as_str(),
                "built_in" | "usb" | "bluetooth" | "aggregate" | "virtual" | "unknown"
            )
            || !matches!(
                self.result.as_str(),
                "compatible"
                    | "permission_denied"
                    | "unavailable"
                    | "identity_mismatch"
                    | "invalid_format"
                    | "no_audio_frames"
                    | "route_conflict"
                    | "timeout"
            )
        {
            return Err("invalid diagnostic microphone probe identity".to_string());
        }
        if self.result == "compatible" {
            if self.input_sample_rate_hz.is_none_or(|value| value == 0)
                || self.input_channel_count.is_none_or(|value| value == 0)
                || self.read_back_uid.as_deref() != Some(self.uid.as_str())
                || self.first_frame_sample_count == 0
            {
                return Err("invalid compatible diagnostic microphone probe".to_string());
            }
        } else if self.first_frame_sample_count != 0 {
            return Err("failed diagnostic microphone probe claimed frames".to_string());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DiagnosticIdentities {
    pub app_version: String,
    pub app_executable_sha256: String,
    pub helper_code_sha256: String,
    pub capture_protocol_version: u16,
    pub runtime_version: String,
    pub runtime_git_commit: String,
    pub runtime_archive_sha256: String,
    pub runtime_build_id: String,
}

impl DiagnosticIdentities {
    pub fn validate_hash(value: &str) -> Result<(), String> {
        if value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            Ok(())
        } else {
            Err("diagnostic identity must be a SHA-256 hex digest".to_string())
        }
    }

    fn validate(&self) -> Result<(), String> {
        Self::validate_hash(&self.app_executable_sha256)?;
        Self::validate_hash(&self.helper_code_sha256)?;
        Self::validate_hash(&self.runtime_archive_sha256)?;
        if self.capture_protocol_version == 0
            || self.runtime_git_commit.len() != 40
            || !self
                .runtime_git_commit
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err("invalid diagnostic runtime identity".to_string());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DiagnosticModel {
    pub model_id: String,
    pub state: String,
    pub bundle_identity: String,
    pub runtime_identity_sha256: Option<String>,
    pub last_error_code: Option<String>,
}

impl DiagnosticModel {
    fn validate(&self) -> Result<(), String> {
        Self::validate_nonempty(&self.model_id)?;
        Self::validate_nonempty(&self.state)?;
        DiagnosticIdentities::validate_hash(&self.bundle_identity)?;
        if let Some(hash) = &self.runtime_identity_sha256 {
            DiagnosticIdentities::validate_hash(hash)?;
        }
        Ok(())
    }

    fn validate_nonempty(value: &str) -> Result<(), String> {
        if value.trim().is_empty() {
            Err("diagnostic model field must not be empty".to_string())
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DiagnosticStorage {
    pub data_directory: String,
    pub catalog_bytes: u64,
    pub audio_bytes: u64,
    pub model_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DiagnosticError {
    pub code: String,
    pub summary: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DiagnosticsSnapshot {
    pub schema: String,
    pub generated_at: String,
    pub permissions: DiagnosticPermissions,
    pub selected_microphone: DiagnosticSelectedMicrophone,
    pub identities: DiagnosticIdentities,
    pub models: Vec<DiagnosticModel>,
    pub storage: DiagnosticStorage,
    pub recent_error: Option<DiagnosticError>,
}

impl DiagnosticsSnapshot {
    pub fn new(
        permissions: DiagnosticPermissions,
        selected_microphone: DiagnosticSelectedMicrophone,
        identities: DiagnosticIdentities,
        models: Vec<DiagnosticModel>,
        storage: DiagnosticStorage,
        recent_error: Option<DiagnosticError>,
    ) -> Self {
        Self {
            schema: DIAGNOSTICS_SCHEMA.to_string(),
            generated_at: chrono::Utc::now().to_rfc3339(),
            permissions,
            selected_microphone,
            identities,
            models,
            storage,
            recent_error,
        }
    }

    pub fn to_redacted_pretty_json(&self) -> Result<String, String> {
        self.validate()?;
        serde_json::to_string_pretty(self).map_err(|error| error.to_string())
    }

    fn validate(&self) -> Result<(), String> {
        if self.schema != DIAGNOSTICS_SCHEMA || self.storage.data_directory.trim().is_empty() {
            return Err("invalid diagnostics snapshot".to_string());
        }
        self.permissions.validate()?;
        self.identities.validate()?;
        match (
            self.selected_microphone.probe_result.as_deref(),
            self.selected_microphone.probe.as_ref(),
        ) {
            (None, None) => {}
            (Some(result), Some(probe)) => {
                probe.validate()?;
                if self.selected_microphone.uid.as_deref() != Some(probe.uid.as_str())
                    || self.selected_microphone.name.as_deref() != Some(probe.name.as_str())
                    || self.selected_microphone.transport.as_deref()
                        != Some(probe.transport.as_str())
                    || result != probe.result
                {
                    return Err("diagnostic selected microphone probe mismatch".to_string());
                }
            }
            _ => return Err("diagnostic selected microphone probe is incomplete".to_string()),
        }
        for model in &self.models {
            model.validate()?;
        }
        if let Some(error) = &self.recent_error
            && (error.code.trim().is_empty() || error.summary.trim().is_empty())
        {
            return Err("invalid diagnostic error".to_string());
        }
        Ok(())
    }
}

pub fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|error| error.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn sha256_text(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

pub fn storage_snapshot(data_directory: &Path) -> Result<DiagnosticStorage, String> {
    Ok(DiagnosticStorage {
        data_directory: data_directory.to_string_lossy().into_owned(),
        catalog_bytes: regular_file_bytes(&data_directory.join("lifesub.sqlite3"))?,
        audio_bytes: directory_bytes(&data_directory.join("audio"))?,
        model_bytes: directory_bytes(&data_directory.join("models"))?,
    })
}

fn regular_file_bytes(path: &Path) -> Result<u64, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => Ok(metadata.len()),
        Ok(_) => Ok(0),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(0),
        Err(error) => Err(error.to_string()),
    }
}

fn directory_bytes(path: &Path) -> Result<u64, String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error.to_string()),
    };
    if !metadata.file_type().is_dir() {
        return Ok(0);
    }
    let mut total = 0_u64;
    for entry in fs::read_dir(path).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let metadata = fs::symlink_metadata(entry.path()).map_err(|error| error.to_string())?;
        let bytes = if metadata.file_type().is_file() {
            metadata.len()
        } else if metadata.file_type().is_dir() {
            directory_bytes(&entry.path())?
        } else {
            0
        };
        total = total
            .checked_add(bytes)
            .ok_or_else(|| "diagnostic storage size overflow".to_string())?;
    }
    Ok(total)
}
