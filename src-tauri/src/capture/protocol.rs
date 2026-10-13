use std::collections::{HashMap, HashSet};
use std::fmt;
use std::io::Read;

use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u16 = 2;
pub const MAX_HEADER_BYTES: usize = 64 * 1024;
pub const MAX_PAYLOAD_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_MICROPHONE_DEVICES: usize = 32;

#[derive(Clone, Copy, Debug, Deserialize, Hash, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Microphone,
    SystemAudio,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CaptureCommand {
    Start {
        protocol_version: u16,
        requested_sources: Vec<Source>,
        staging_root: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        microphone_device_uid: Option<String>,
    },
    ListMicrophones {
        protocol_version: u16,
    },
    ProbeMicrophone {
        protocol_version: u16,
        microphone_device_uid: String,
    },
    GetPermissionState {
        protocol_version: u16,
    },
    Pause {
        protocol_version: u16,
    },
    Resume {
        protocol_version: u16,
    },
    Stop {
        protocol_version: u16,
    },
    Shutdown {
        protocol_version: u16,
    },
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum RawCaptureCommand {
    Start {
        protocol_version: u16,
        requested_sources: Vec<Source>,
        staging_root: String,
        microphone_device_uid: Option<String>,
    },
    ListMicrophones {
        protocol_version: u16,
    },
    ProbeMicrophone {
        protocol_version: u16,
        microphone_device_uid: String,
    },
    GetPermissionState {
        protocol_version: u16,
    },
    Pause {
        protocol_version: u16,
    },
    Resume {
        protocol_version: u16,
    },
    Stop {
        protocol_version: u16,
    },
    Shutdown {
        protocol_version: u16,
    },
}

impl<'de> Deserialize<'de> for CaptureCommand {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = RawCaptureCommand::deserialize(deserializer)?;
        Self::try_from(raw).map_err(serde::de::Error::custom)
    }
}

impl TryFrom<RawCaptureCommand> for CaptureCommand {
    type Error = CaptureProtocolError;

    fn try_from(raw: RawCaptureCommand) -> Result<Self, Self::Error> {
        let command = match raw {
            RawCaptureCommand::Start {
                protocol_version,
                requested_sources,
                staging_root,
                microphone_device_uid,
            } => Self::Start {
                protocol_version,
                requested_sources,
                staging_root,
                microphone_device_uid,
            },
            RawCaptureCommand::ListMicrophones { protocol_version } => {
                Self::ListMicrophones { protocol_version }
            }
            RawCaptureCommand::ProbeMicrophone {
                protocol_version,
                microphone_device_uid,
            } => Self::ProbeMicrophone {
                protocol_version,
                microphone_device_uid,
            },
            RawCaptureCommand::GetPermissionState { protocol_version } => {
                Self::GetPermissionState { protocol_version }
            }
            RawCaptureCommand::Pause { protocol_version } => Self::Pause { protocol_version },
            RawCaptureCommand::Resume { protocol_version } => Self::Resume { protocol_version },
            RawCaptureCommand::Stop { protocol_version } => Self::Stop { protocol_version },
            RawCaptureCommand::Shutdown { protocol_version } => Self::Shutdown { protocol_version },
        };
        validate_command(&command)?;
        Ok(command)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PcmFormat {
    S16Le,
}

impl PcmFormat {
    pub const fn bytes_per_sample(self) -> usize {
        match self {
            Self::S16Le => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscontinuityFlag {
    Gap,
    DeviceChange,
    PermissionRevoked,
    ClockReset,
    DroppedBuffers,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionState {
    NotDetermined,
    NotGranted,
    Denied,
    Restricted,
    Granted,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HelloCapability {
    DeviceDiscovery,
    ExactMicrophoneSelection,
    MicrophoneProbe,
    PermissionDiagnostics,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MicrophoneTransport {
    BuiltIn,
    Usb,
    Bluetooth,
    Aggregate,
    Virtual,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Hello {
    pub protocol_version: u16,
    pub helper_pid: u32,
    pub launch_nonce: String,
    pub supported_sources: Vec<Source>,
    pub capabilities: Vec<HelloCapability>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawHello {
    protocol_version: u16,
    helper_pid: u32,
    launch_nonce: String,
    supported_sources: Vec<Source>,
    capabilities: Vec<HelloCapability>,
}

impl<'de> Deserialize<'de> for Hello {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = RawHello::deserialize(deserializer)?;
        let value = Self {
            protocol_version: raw.protocol_version,
            helper_pid: raw.helper_pid,
            launch_nonce: raw.launch_nonce,
            supported_sources: raw.supported_sources,
            capabilities: raw.capabilities,
        };
        validate_hello(&value).map_err(serde::de::Error::custom)?;
        Ok(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct MicrophoneDevice {
    pub uid: String,
    pub name: String,
    pub is_default: bool,
    pub is_connected: bool,
    pub transport: MicrophoneTransport,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMicrophoneDevice {
    uid: String,
    name: String,
    is_default: bool,
    is_connected: bool,
    transport: MicrophoneTransport,
}

impl<'de> Deserialize<'de> for MicrophoneDevice {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = RawMicrophoneDevice::deserialize(deserializer)?;
        let value = Self {
            uid: raw.uid,
            name: raw.name,
            is_default: raw.is_default,
            is_connected: raw.is_connected,
            transport: raw.transport,
        };
        validate_microphone_device(&value).map_err(serde::de::Error::custom)?;
        Ok(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct MicrophoneDevicesMessage {
    pub devices: Vec<MicrophoneDevice>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MicrophoneProbeResult {
    Compatible,
    PermissionDenied,
    Unavailable,
    IdentityMismatch,
    InvalidFormat,
    NoAudioFrames,
    RouteConflict,
    Timeout,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MicrophoneProbeMessage {
    pub uid: String,
    pub name: String,
    pub transport: MicrophoneTransport,
    pub input_sample_rate_hz: Option<u32>,
    pub input_channel_count: Option<u16>,
    pub read_back_uid: Option<String>,
    pub first_frame_sample_count: u64,
    pub elapsed_ms: u64,
    pub result: MicrophoneProbeResult,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMicrophoneDevicesMessage {
    devices: Vec<MicrophoneDevice>,
}

impl<'de> Deserialize<'de> for MicrophoneDevicesMessage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = RawMicrophoneDevicesMessage::deserialize(deserializer)?;
        let value = Self {
            devices: raw.devices,
        };
        validate_microphone_devices(&value).map_err(serde::de::Error::custom)?;
        Ok(value)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PermissionStateMessage {
    pub microphone: PermissionState,
    pub screen_recording: PermissionState,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceStarted {
    pub source: Source,
    pub device_id: String,
    pub device_name: String,
    pub sample_rate: u32,
    pub channel_count: u16,
    pub host_time: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AudioFrame {
    pub source: Source,
    pub sequence: u64,
    pub sample_position: u64,
    pub host_time: u64,
    pub format: PcmFormat,
    pub channel_count: u16,
    pub discontinuity_flags: Vec<DiscontinuityFlag>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Level {
    pub source: Source,
    pub rms: f32,
    pub peak: f32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChunkSealed {
    pub source: Source,
    pub relative_staging_path: String,
    pub byte_length: u64,
    pub sha256: String,
    pub duration_ms: u64,
    pub start_sample_position: u64,
    pub end_sample_position: u64,
    pub start_host_time: u64,
    pub end_host_time: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceInterrupted {
    pub source: Source,
    pub reason: String,
    pub recoverable: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRecovering {
    pub source: Source,
    pub generation: u64,
    pub reason: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRecovered {
    pub source: Source,
    pub generation: u64,
    pub device_id: String,
    pub first_host_time: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceStopped {
    pub source: Source,
    pub final_sequence: u64,
    pub start_host_time: u64,
    pub end_host_time: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FatalError {
    pub code: String,
    pub summary: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum CaptureHeader {
    Hello(Hello),
    PermissionState(PermissionStateMessage),
    SourceStarted(SourceStarted),
    MicrophoneDevices(MicrophoneDevicesMessage),
    MicrophoneProbeResult(MicrophoneProbeMessage),
    AudioFrame(AudioFrame),
    Level(Level),
    ChunkSealed(ChunkSealed),
    SourceInterrupted(SourceInterrupted),
    SourceRecovering(SourceRecovering),
    SourceRecovered(SourceRecovered),
    SourceStopped(SourceStopped),
    FatalError(FatalError),
    ShutdownAck,
}

impl CaptureHeader {
    pub fn audio_frame(
        source: Source,
        sequence: u64,
        sample_position: u64,
        host_time: u64,
        format: PcmFormat,
        channel_count: u16,
    ) -> Self {
        Self::AudioFrame(AudioFrame {
            source,
            sequence,
            sample_position,
            host_time,
            format,
            channel_count,
            discontinuity_flags: Vec::new(),
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CaptureFrame {
    pub header: CaptureHeader,
    pub payload: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureProtocolError {
    HeaderTooLarge,
    PayloadTooLarge,
    TruncatedFrame,
    FrameReadTimeout,
    InvalidHeader,
    UnexpectedPayload,
    InvalidAudioPayload,
    HelloRequired,
    HelloReplay,
    UnsupportedVersion,
    NonMonotonicSequence,
}

impl fmt::Display for CaptureProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for CaptureProtocolError {}

fn validate_version(version: u16) -> Result<(), CaptureProtocolError> {
    if version != PROTOCOL_VERSION {
        return Err(CaptureProtocolError::UnsupportedVersion);
    }
    Ok(())
}

fn validate_command(command: &CaptureCommand) -> Result<(), CaptureProtocolError> {
    let protocol_version = match command {
        CaptureCommand::Start {
            protocol_version,
            requested_sources,
            microphone_device_uid,
            ..
        } => {
            if requested_sources.is_empty() {
                return Err(CaptureProtocolError::InvalidHeader);
            }
            let microphone_requested = requested_sources.contains(&Source::Microphone);
            match (microphone_requested, microphone_device_uid.as_deref()) {
                (true, Some(uid)) if !uid.trim().is_empty() => {}
                (false, None) => {}
                _ => return Err(CaptureProtocolError::InvalidHeader),
            }
            *protocol_version
        }
        CaptureCommand::ProbeMicrophone {
            protocol_version,
            microphone_device_uid,
        } => {
            if microphone_device_uid.trim().is_empty() {
                return Err(CaptureProtocolError::InvalidHeader);
            }
            *protocol_version
        }
        CaptureCommand::GetPermissionState { protocol_version } => *protocol_version,
        CaptureCommand::ListMicrophones { protocol_version }
        | CaptureCommand::Pause { protocol_version }
        | CaptureCommand::Resume { protocol_version }
        | CaptureCommand::Stop { protocol_version }
        | CaptureCommand::Shutdown { protocol_version } => *protocol_version,
    };
    validate_version(protocol_version)
}

fn validate_hello(hello: &Hello) -> Result<(), CaptureProtocolError> {
    validate_version(hello.protocol_version)?;
    let capabilities = hello.capabilities.iter().copied().collect::<HashSet<_>>();
    if !capabilities.contains(&HelloCapability::DeviceDiscovery)
        || !capabilities.contains(&HelloCapability::ExactMicrophoneSelection)
        || !capabilities.contains(&HelloCapability::MicrophoneProbe)
        || !capabilities.contains(&HelloCapability::PermissionDiagnostics)
    {
        return Err(CaptureProtocolError::InvalidHeader);
    }
    Ok(())
}

fn validate_microphone_device(device: &MicrophoneDevice) -> Result<(), CaptureProtocolError> {
    if device.uid.trim().is_empty() || device.name.trim().is_empty() || !device.is_connected {
        return Err(CaptureProtocolError::InvalidHeader);
    }
    Ok(())
}

fn validate_microphone_devices(
    message: &MicrophoneDevicesMessage,
) -> Result<(), CaptureProtocolError> {
    if message.devices.len() > MAX_MICROPHONE_DEVICES {
        return Err(CaptureProtocolError::InvalidHeader);
    }
    let mut uids = HashSet::with_capacity(message.devices.len());
    for device in &message.devices {
        validate_microphone_device(device)?;
        if !uids.insert(device.uid.as_str()) {
            return Err(CaptureProtocolError::InvalidHeader);
        }
    }
    Ok(())
}

fn validate_header(header: &CaptureHeader) -> Result<(), CaptureProtocolError> {
    match header {
        CaptureHeader::Hello(hello) => validate_hello(hello),
        CaptureHeader::MicrophoneDevices(message) => validate_microphone_devices(message),
        CaptureHeader::MicrophoneProbeResult(message) => validate_microphone_probe(message),
        CaptureHeader::SourceRecovering(value) => {
            if value.generation == 0 || value.reason.trim().is_empty() {
                Err(CaptureProtocolError::InvalidHeader)
            } else {
                Ok(())
            }
        }
        CaptureHeader::SourceRecovered(value) => {
            if value.generation == 0
                || value.device_id.trim().is_empty()
                || value.first_host_time == 0
            {
                Err(CaptureProtocolError::InvalidHeader)
            } else {
                Ok(())
            }
        }
        _ => Ok(()),
    }
}

fn validate_microphone_probe(message: &MicrophoneProbeMessage) -> Result<(), CaptureProtocolError> {
    if message.uid.trim().is_empty() || message.name.trim().is_empty() {
        return Err(CaptureProtocolError::InvalidHeader);
    }
    let valid = match message.result {
        MicrophoneProbeResult::Compatible => {
            message.input_sample_rate_hz.is_some_and(|rate| rate > 0)
                && message
                    .input_channel_count
                    .is_some_and(|channels| channels > 0)
                && message.read_back_uid.as_deref() == Some(message.uid.as_str())
                && message.first_frame_sample_count > 0
        }
        MicrophoneProbeResult::IdentityMismatch => {
            message
                .read_back_uid
                .as_deref()
                .is_some_and(|uid| !uid.is_empty() && uid != message.uid)
                && message.first_frame_sample_count == 0
        }
        MicrophoneProbeResult::NoAudioFrames => {
            message.input_sample_rate_hz.is_some_and(|rate| rate > 0)
                && message
                    .input_channel_count
                    .is_some_and(|channels| channels > 0)
                && message.read_back_uid.as_deref() == Some(message.uid.as_str())
                && message.first_frame_sample_count == 0
        }
        _ => message.first_frame_sample_count == 0,
    };
    if valid {
        Ok(())
    } else {
        Err(CaptureProtocolError::InvalidHeader)
    }
}

pub fn encode_frame(
    header: &CaptureHeader,
    payload: &[u8],
) -> Result<Vec<u8>, CaptureProtocolError> {
    validate_header(header)?;
    let header_bytes = serde_json_canonicalizer::to_vec(header)
        .map_err(|_| CaptureProtocolError::InvalidHeader)?;
    if header_bytes.len() > MAX_HEADER_BYTES {
        return Err(CaptureProtocolError::HeaderTooLarge);
    }
    if payload.len() > MAX_PAYLOAD_BYTES {
        return Err(CaptureProtocolError::PayloadTooLarge);
    }
    if !matches!(header, CaptureHeader::AudioFrame(_)) && !payload.is_empty() {
        return Err(CaptureProtocolError::UnexpectedPayload);
    }
    validate_audio_payload(header, payload)?;

    let capacity = 8_usize
        .checked_add(header_bytes.len())
        .and_then(|size| size.checked_add(payload.len()))
        .ok_or(CaptureProtocolError::PayloadTooLarge)?;
    let mut frame = Vec::with_capacity(capacity);
    frame.extend_from_slice(&(header_bytes.len() as u32).to_be_bytes());
    frame.extend_from_slice(&header_bytes);
    frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    frame.extend_from_slice(payload);
    Ok(frame)
}

pub fn encode_command(command: &CaptureCommand) -> Result<Vec<u8>, CaptureProtocolError> {
    validate_command(command)?;
    let header = serde_json_canonicalizer::to_vec(command)
        .map_err(|_| CaptureProtocolError::InvalidHeader)?;
    if header.len() > MAX_HEADER_BYTES {
        return Err(CaptureProtocolError::HeaderTooLarge);
    }
    let mut frame = Vec::with_capacity(header.len() + 8);
    frame.extend_from_slice(&(header.len() as u32).to_be_bytes());
    frame.extend_from_slice(&header);
    frame.extend_from_slice(&0_u32.to_be_bytes());
    Ok(frame)
}

#[derive(Clone, Copy, Debug)]
pub struct FrameDecoder {
    max_header_bytes: usize,
    max_payload_bytes: usize,
}

impl Default for FrameDecoder {
    fn default() -> Self {
        Self {
            max_header_bytes: MAX_HEADER_BYTES,
            max_payload_bytes: MAX_PAYLOAD_BYTES,
        }
    }
}

impl FrameDecoder {
    pub fn decode<R: Read>(&self, reader: &mut R) -> Result<CaptureFrame, CaptureProtocolError> {
        let header_length = read_length(reader)?;
        if header_length > self.max_header_bytes {
            return Err(CaptureProtocolError::HeaderTooLarge);
        }
        let mut header_bytes = vec![0_u8; header_length];
        read_exact(reader, &mut header_bytes)?;
        let header = serde_json::from_slice::<CaptureHeader>(&header_bytes)
            .map_err(|_| CaptureProtocolError::InvalidHeader)?;
        validate_header(&header)?;

        let payload_length = read_length(reader)?;
        if payload_length > self.max_payload_bytes {
            return Err(CaptureProtocolError::PayloadTooLarge);
        }
        if !matches!(header, CaptureHeader::AudioFrame(_)) && payload_length != 0 {
            return Err(CaptureProtocolError::UnexpectedPayload);
        }
        let mut payload = vec![0_u8; payload_length];
        read_exact(reader, &mut payload)?;
        validate_audio_payload(&header, &payload)?;
        Ok(CaptureFrame { header, payload })
    }
}

fn validate_audio_payload(
    header: &CaptureHeader,
    payload: &[u8],
) -> Result<(), CaptureProtocolError> {
    let CaptureHeader::AudioFrame(frame) = header else {
        return Ok(());
    };
    let frame_bytes = frame
        .format
        .bytes_per_sample()
        .checked_mul(usize::from(frame.channel_count))
        .filter(|stride| *stride != 0)
        .ok_or(CaptureProtocolError::InvalidAudioPayload)?;
    if payload.is_empty() || !payload.len().is_multiple_of(frame_bytes) {
        return Err(CaptureProtocolError::InvalidAudioPayload);
    }
    Ok(())
}

fn read_length<R: Read>(reader: &mut R) -> Result<usize, CaptureProtocolError> {
    let mut bytes = [0_u8; 4];
    read_exact(reader, &mut bytes)?;
    Ok(u32::from_be_bytes(bytes) as usize)
}

fn read_exact<R: Read>(reader: &mut R, bytes: &mut [u8]) -> Result<(), CaptureProtocolError> {
    reader.read_exact(bytes).map_err(|error| {
        if matches!(
            error.kind(),
            std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
        ) || error
            .raw_os_error()
            .is_some_and(|code| code == libc::EAGAIN || code == libc::ETIMEDOUT)
        {
            CaptureProtocolError::FrameReadTimeout
        } else {
            CaptureProtocolError::TruncatedFrame
        }
    })
}

#[derive(Debug, Default)]
pub struct ProtocolValidator {
    hello_seen: bool,
    last_sequence_by_source: HashMap<Source, u64>,
}

impl ProtocolValidator {
    pub fn observe(&mut self, header: &CaptureHeader) -> Result<(), CaptureProtocolError> {
        match header {
            CaptureHeader::Hello(hello) => {
                if self.hello_seen {
                    return Err(CaptureProtocolError::HelloReplay);
                }
                validate_hello(hello)?;
                self.hello_seen = true;
            }
            CaptureHeader::AudioFrame(frame) => {
                if !self.hello_seen {
                    return Err(CaptureProtocolError::HelloRequired);
                }
                if self
                    .last_sequence_by_source
                    .get(&frame.source)
                    .is_some_and(|previous| frame.sequence <= *previous)
                {
                    return Err(CaptureProtocolError::NonMonotonicSequence);
                }
                self.last_sequence_by_source
                    .insert(frame.source, frame.sequence);
            }
            _ if !self.hello_seen => return Err(CaptureProtocolError::HelloRequired),
            _ => {}
        }
        Ok(())
    }
}
