use std::collections::HashMap;
use std::fmt;
use std::io::Read;

use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u16 = 1;
pub const MAX_HEADER_BYTES: usize = 64 * 1024;
pub const MAX_PAYLOAD_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Hash, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Microphone,
    SystemAudio,
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
    Denied,
    Restricted,
    Granted,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Hello {
    pub protocol_version: u16,
    pub helper_pid: u32,
    pub launch_nonce: String,
    pub supported_sources: Vec<Source>,
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
    AudioFrame(AudioFrame),
    Level(Level),
    ChunkSealed(ChunkSealed),
    SourceInterrupted(SourceInterrupted),
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
    InvalidHeader,
    UnexpectedPayload,
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

pub fn encode_frame(
    header: &CaptureHeader,
    payload: &[u8],
) -> Result<Vec<u8>, CaptureProtocolError> {
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

        let payload_length = read_length(reader)?;
        if payload_length > self.max_payload_bytes {
            return Err(CaptureProtocolError::PayloadTooLarge);
        }
        if !matches!(header, CaptureHeader::AudioFrame(_)) && payload_length != 0 {
            return Err(CaptureProtocolError::UnexpectedPayload);
        }
        let mut payload = vec![0_u8; payload_length];
        read_exact(reader, &mut payload)?;
        Ok(CaptureFrame { header, payload })
    }
}

fn read_length<R: Read>(reader: &mut R) -> Result<usize, CaptureProtocolError> {
    let mut bytes = [0_u8; 4];
    read_exact(reader, &mut bytes)?;
    Ok(u32::from_be_bytes(bytes) as usize)
}

fn read_exact<R: Read>(reader: &mut R, bytes: &mut [u8]) -> Result<(), CaptureProtocolError> {
    reader
        .read_exact(bytes)
        .map_err(|_| CaptureProtocolError::TruncatedFrame)
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
                if hello.protocol_version != PROTOCOL_VERSION {
                    return Err(CaptureProtocolError::UnsupportedVersion);
                }
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
