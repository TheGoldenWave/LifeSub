use std::path::{Path, PathBuf};

use super::coordinator::{NativeCaptureTransport, NativeCaptureTransportError};
use super::helper::{AuthenticatedHelper, HelperTransportError};
use super::protocol::{CaptureCommand, CaptureHeader, PROTOCOL_VERSION, Source};

pub(crate) struct AuthenticatedCaptureTransport {
    helper: AuthenticatedHelper,
    staging_root: PathBuf,
    microphone_device_uid: Option<String>,
}

impl AuthenticatedCaptureTransport {
    pub(crate) fn new(
        helper: AuthenticatedHelper,
        staging_root: impl AsRef<Path>,
        microphone_device_uid: Option<String>,
    ) -> Self {
        Self {
            helper,
            staging_root: staging_root.as_ref().to_path_buf(),
            microphone_device_uid,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn helper_mut(&mut self) -> &mut AuthenticatedHelper {
        &mut self.helper
    }
}

impl NativeCaptureTransport for AuthenticatedCaptureTransport {
    fn preflight_source(&mut self, _source: Source) -> Result<(), NativeCaptureTransportError> {
        // Permission state is reported by the helper during start; the coordinator
        // still keeps preflight as an explicit phase before issuing start.
        Ok(())
    }

    fn start_sources(
        &mut self,
        sources: &[Source],
        microphone_device_uid: Option<&str>,
    ) -> Result<(), NativeCaptureTransportError> {
        self.helper
            .send_command(&CaptureCommand::Start {
                protocol_version: PROTOCOL_VERSION,
                requested_sources: sources.to_vec(),
                staging_root: self.staging_root.to_string_lossy().into_owned(),
                microphone_device_uid: microphone_device_uid
                    .map(ToOwned::to_owned)
                    .or_else(|| self.microphone_device_uid.clone()),
            })
            .map_err(map_transport)
    }

    fn pause(&mut self) -> Result<(), NativeCaptureTransportError> {
        self.helper
            .send_command(&CaptureCommand::Pause {
                protocol_version: PROTOCOL_VERSION,
            })
            .map_err(map_transport)
    }

    fn resume(&mut self) -> Result<(), NativeCaptureTransportError> {
        self.helper
            .send_command(&CaptureCommand::Resume {
                protocol_version: PROTOCOL_VERSION,
            })
            .map_err(map_transport)
    }

    fn stop(&mut self) -> Result<(), NativeCaptureTransportError> {
        self.helper
            .send_command(&CaptureCommand::Stop {
                protocol_version: PROTOCOL_VERSION,
            })
            .map_err(map_transport)
    }

    fn shutdown(&mut self) -> Result<(), NativeCaptureTransportError> {
        self.helper
            .send_command(&CaptureCommand::Shutdown {
                protocol_version: PROTOCOL_VERSION,
            })
            .map_err(map_transport)
    }

    fn next_event(&mut self) -> Result<Option<CaptureHeader>, NativeCaptureTransportError> {
        match self.helper.receive_header() {
            Ok(header) => Ok(Some(header)),
            Err(HelperTransportError::Timeout) => Ok(None),
            Err(error) => Err(map_transport(error)),
        }
    }
}

fn map_transport(error: HelperTransportError) -> NativeCaptureTransportError {
    match error {
        HelperTransportError::Io(_) => NativeCaptureTransportError::HelperExited,
        HelperTransportError::Timeout => NativeCaptureTransportError::ProtocolViolation,
        HelperTransportError::Protocol(_) => NativeCaptureTransportError::ProtocolViolation,
    }
}
