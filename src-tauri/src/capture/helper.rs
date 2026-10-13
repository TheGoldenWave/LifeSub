use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{self, Write};
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
use std::os::unix::fs::MetadataExt;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use super::helper_auth::ExecutableFileIdentity;
use super::helper_auth::{HandshakeError, HandshakeVerifier, PeerIdentity};
use super::protocol::{
    CaptureCommand, CaptureHeader, FrameDecoder, MicrophoneDevice, MicrophoneProbeMessage,
    PROTOCOL_VERSION, PermissionStateMessage, ProtocolValidator, encode_command,
};
use sha2::{Digest, Sha256};

const BOOTSTRAP_FD: RawFd = 3;
const CAPTURE_HELPER_IDENTIFIER: &str = "lifesub-capture-helper";
const TAURI_SIDECAR_IDENTIFIER_SUFFIX_LENGTH: usize = 40;

struct SensitiveNonce([u8; 32]);

impl Drop for SensitiveNonce {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}

#[derive(Debug)]
pub struct PrivateCaptureEndpoint {
    directory: PathBuf,
    socket_path: PathBuf,
    listener: UnixListener,
}

impl PrivateCaptureEndpoint {
    pub fn bind(parent: &Path) -> io::Result<Self> {
        let parent = parent.canonicalize()?;
        let token = uuid::Uuid::new_v4().simple().to_string();
        let directory = parent.join(format!("lc-{}", &token[..8]));
        fs::DirBuilder::new().mode(0o700).create(&directory)?;
        let socket_path = directory.join("s");
        let listener = match UnixListener::bind(&socket_path) {
            Ok(listener) => listener,
            Err(error) => {
                let _ = fs::remove_dir(&directory);
                return Err(error);
            }
        };
        fs::set_permissions(&socket_path, fs::Permissions::from_mode(0o600))?;
        Ok(Self {
            directory,
            socket_path,
            listener,
        })
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    pub fn listener(&self) -> &UnixListener {
        &self.listener
    }
}

impl Drop for PrivateCaptureEndpoint {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.socket_path);
        let _ = fs::remove_dir(&self.directory);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HelperLaunchError {
    Io,
    InvalidExecutable,
    SignatureInvalid,
    HandshakeTimeout,
    HelperExitedEarly,
    PeerIdentityUnavailable,
    Handshake(HandshakeError),
    MalformedHello,
}

#[derive(Debug)]
pub struct AuthenticatedHelper {
    child: Option<Child>,
    stream: UnixStream,
    endpoint: PrivateCaptureEndpoint,
    validator: ProtocolValidator,
}

impl AuthenticatedHelper {
    pub fn stream(&self) -> &UnixStream {
        &self.stream
    }

    pub fn endpoint_directory(&self) -> &Path {
        self.endpoint.directory()
    }

    pub fn child_id(&self) -> u32 {
        self.child.as_ref().map_or(0, Child::id)
    }

    pub fn validator_mut(&mut self) -> &mut ProtocolValidator {
        &mut self.validator
    }

    pub fn send_command(&mut self, command: &CaptureCommand) -> Result<(), HelperTransportError> {
        let frame = encode_command(command).map_err(HelperTransportError::Protocol)?;
        self.stream
            .write_all(&frame)
            .map_err(HelperTransportError::Io)
    }

    pub fn receive_header(&mut self) -> Result<CaptureHeader, HelperTransportError> {
        let frame =
            FrameDecoder::default()
                .decode(&mut self.stream)
                .map_err(|error| match error {
                    super::protocol::CaptureProtocolError::FrameReadTimeout => {
                        HelperTransportError::Timeout
                    }
                    error => HelperTransportError::Protocol(error),
                })?;
        self.validator
            .observe(&frame.header)
            .map_err(HelperTransportError::Protocol)?;
        Ok(frame.header)
    }

    pub fn terminate_and_wait(&mut self) {
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
        if let Some(mut child) = self.child.take() {
            terminate_child(&mut child);
        }
    }

    pub fn wait_for_exit(&mut self) {
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
        if let Some(mut child) = self.child.take() {
            let _ = child.wait();
        }
    }
}

#[derive(Debug)]
pub enum HelperTransportError {
    Io(io::Error),
    Timeout,
    Protocol(super::protocol::CaptureProtocolError),
}

impl Drop for AuthenticatedHelper {
    fn drop(&mut self) {
        self.terminate_and_wait();
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MicrophoneDiscoveryError {
    Launch(HelperLaunchError),
    Cancelled,
    Timeout,
    Fatal { code: String, summary: String },
    UnexpectedResponse,
    MissingShutdownAck,
    Transport(String),
}

pub(crate) trait DiscoveryHelperSession {
    fn send_command(&mut self, command: &CaptureCommand) -> Result<(), HelperTransportError>;
    fn receive_header(&mut self) -> Result<CaptureHeader, HelperTransportError>;
    fn cleanup(&mut self, orderly: bool);
}

impl DiscoveryHelperSession for AuthenticatedHelper {
    fn send_command(&mut self, command: &CaptureCommand) -> Result<(), HelperTransportError> {
        Self::send_command(self, command)
    }

    fn receive_header(&mut self) -> Result<CaptureHeader, HelperTransportError> {
        Self::receive_header(self)
    }

    fn cleanup(&mut self, orderly: bool) {
        if orderly {
            self.wait_for_exit();
        } else {
            self.terminate_and_wait();
        }
    }
}

pub fn list_microphones_one_shot(
    runtime_parent: &Path,
    timeout: Duration,
) -> Result<Vec<MicrophoneDevice>, MicrophoneDiscoveryError> {
    let cancelled = AtomicBool::new(false);
    list_microphones_one_shot_with_cancel(runtime_parent, timeout, &cancelled)
}

pub fn list_microphones_one_shot_with_cancel(
    runtime_parent: &Path,
    timeout: Duration,
    cancelled: &AtomicBool,
) -> Result<Vec<MicrophoneDevice>, MicrophoneDiscoveryError> {
    list_microphones_one_shot_with_launcher_and_cancel_for_test(
        timeout,
        |remaining| launch_packaged_helper(runtime_parent, remaining),
        cancelled,
    )
}

pub fn probe_microphone_one_shot_with_cancel(
    runtime_parent: &Path,
    microphone_device_uid: &str,
    timeout: Duration,
    cancelled: &AtomicBool,
) -> Result<MicrophoneProbeMessage, MicrophoneDiscoveryError> {
    if cancelled.load(Ordering::Acquire) {
        return Err(MicrophoneDiscoveryError::Cancelled);
    }
    let deadline = Instant::now() + timeout;
    let helper = launch_packaged_helper(runtime_parent, timeout)
        .map_err(MicrophoneDiscoveryError::Launch)?;
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        let mut helper = helper;
        helper.cleanup(false);
        return Err(MicrophoneDiscoveryError::Timeout);
    }
    probe_microphone_with_helper_and_cancel_for_test(
        helper,
        microphone_device_uid,
        remaining,
        cancelled,
    )
}

pub fn permission_state_one_shot_with_cancel(
    runtime_parent: &Path,
    timeout: Duration,
    cancelled: &AtomicBool,
) -> Result<PermissionStateMessage, MicrophoneDiscoveryError> {
    if cancelled.load(Ordering::Acquire) {
        return Err(MicrophoneDiscoveryError::Cancelled);
    }
    let deadline = Instant::now() + timeout;
    let helper = launch_packaged_helper(runtime_parent, timeout)
        .map_err(MicrophoneDiscoveryError::Launch)?;
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        let mut helper = helper;
        helper.cleanup(false);
        return Err(MicrophoneDiscoveryError::Timeout);
    }
    permission_state_with_helper_and_cancel(helper, remaining, cancelled)
}

#[cfg(test)]
pub(crate) fn permission_state_with_helper_for_test<H: DiscoveryHelperSession>(
    helper: H,
    timeout: Duration,
) -> Result<PermissionStateMessage, MicrophoneDiscoveryError> {
    let cancelled = AtomicBool::new(false);
    permission_state_with_helper_and_cancel(helper, timeout, &cancelled)
}

fn permission_state_with_helper_and_cancel<H: DiscoveryHelperSession>(
    mut helper: H,
    timeout: Duration,
    cancelled: &AtomicBool,
) -> Result<PermissionStateMessage, MicrophoneDiscoveryError> {
    let result = (|| {
        let deadline = Instant::now() + timeout;
        if cancelled.load(Ordering::Acquire) {
            return Err(MicrophoneDiscoveryError::Cancelled);
        }
        helper
            .send_command(&CaptureCommand::GetPermissionState {
                protocol_version: PROTOCOL_VERSION,
            })
            .map_err(map_discovery_transport_error)?;

        let permissions = loop {
            if cancelled.load(Ordering::Acquire) {
                return Err(MicrophoneDiscoveryError::Cancelled);
            }
            if Instant::now() >= deadline {
                return Err(MicrophoneDiscoveryError::Timeout);
            }
            match helper.receive_header() {
                Ok(CaptureHeader::PermissionState(message)) => break message,
                Ok(CaptureHeader::FatalError(error)) => {
                    return Err(MicrophoneDiscoveryError::Fatal {
                        code: error.code,
                        summary: error.summary,
                    });
                }
                Ok(_) => return Err(MicrophoneDiscoveryError::UnexpectedResponse),
                Err(HelperTransportError::Timeout) => continue,
                Err(error) => return Err(map_discovery_transport_error(error)),
            }
        };

        helper
            .send_command(&CaptureCommand::Shutdown {
                protocol_version: PROTOCOL_VERSION,
            })
            .map_err(map_discovery_transport_error)?;
        loop {
            if cancelled.load(Ordering::Acquire) {
                return Err(MicrophoneDiscoveryError::Cancelled);
            }
            if Instant::now() >= deadline {
                return Err(MicrophoneDiscoveryError::MissingShutdownAck);
            }
            match helper.receive_header() {
                Ok(CaptureHeader::ShutdownAck) => return Ok(permissions),
                Ok(CaptureHeader::FatalError(error)) => {
                    return Err(MicrophoneDiscoveryError::Fatal {
                        code: error.code,
                        summary: error.summary,
                    });
                }
                Ok(_) => return Err(MicrophoneDiscoveryError::UnexpectedResponse),
                Err(HelperTransportError::Timeout) => continue,
                Err(error) => return Err(map_discovery_transport_error(error)),
            }
        }
    })();
    helper.cleanup(result.is_ok());
    result
}

#[cfg(test)]
pub(crate) fn probe_microphone_with_helper_for_test<H: DiscoveryHelperSession>(
    helper: H,
    microphone_device_uid: &str,
    timeout: Duration,
) -> Result<MicrophoneProbeMessage, MicrophoneDiscoveryError> {
    let cancelled = AtomicBool::new(false);
    probe_microphone_with_helper_and_cancel_for_test(
        helper,
        microphone_device_uid,
        timeout,
        &cancelled,
    )
}

pub(crate) fn probe_microphone_with_helper_and_cancel_for_test<H: DiscoveryHelperSession>(
    mut helper: H,
    microphone_device_uid: &str,
    timeout: Duration,
    cancelled: &AtomicBool,
) -> Result<MicrophoneProbeMessage, MicrophoneDiscoveryError> {
    let result = (|| {
        if microphone_device_uid.trim().is_empty() {
            return Err(MicrophoneDiscoveryError::UnexpectedResponse);
        }
        if cancelled.load(Ordering::Acquire) {
            return Err(MicrophoneDiscoveryError::Cancelled);
        }
        let deadline = Instant::now() + timeout;
        helper
            .send_command(&CaptureCommand::ProbeMicrophone {
                protocol_version: PROTOCOL_VERSION,
                microphone_device_uid: microphone_device_uid.to_owned(),
            })
            .map_err(map_discovery_transport_error)?;

        let probe = loop {
            if cancelled.load(Ordering::Acquire) {
                return Err(MicrophoneDiscoveryError::Cancelled);
            }
            if Instant::now() >= deadline {
                return Err(MicrophoneDiscoveryError::Timeout);
            }
            match helper.receive_header() {
                Ok(CaptureHeader::MicrophoneProbeResult(message))
                    if message.uid == microphone_device_uid =>
                {
                    break message;
                }
                Ok(CaptureHeader::FatalError(error)) => {
                    return Err(MicrophoneDiscoveryError::Fatal {
                        code: error.code,
                        summary: error.summary,
                    });
                }
                Ok(_) => return Err(MicrophoneDiscoveryError::UnexpectedResponse),
                Err(HelperTransportError::Timeout) => continue,
                Err(error) => return Err(map_discovery_transport_error(error)),
            }
        };

        helper
            .send_command(&CaptureCommand::Shutdown {
                protocol_version: PROTOCOL_VERSION,
            })
            .map_err(map_discovery_transport_error)?;
        loop {
            if cancelled.load(Ordering::Acquire) {
                return Err(MicrophoneDiscoveryError::Cancelled);
            }
            if Instant::now() >= deadline {
                return Err(MicrophoneDiscoveryError::MissingShutdownAck);
            }
            match helper.receive_header() {
                Ok(CaptureHeader::ShutdownAck) => return Ok(probe),
                Ok(CaptureHeader::FatalError(error)) => {
                    return Err(MicrophoneDiscoveryError::Fatal {
                        code: error.code,
                        summary: error.summary,
                    });
                }
                Ok(_) => return Err(MicrophoneDiscoveryError::UnexpectedResponse),
                Err(HelperTransportError::Timeout) => continue,
                Err(error) => return Err(map_discovery_transport_error(error)),
            }
        }
    })();
    helper.cleanup(result.is_ok());
    result
}

#[cfg(test)]
pub(crate) fn list_microphones_one_shot_with_launcher_for_test<H, F>(
    timeout: Duration,
    launcher: F,
) -> Result<Vec<MicrophoneDevice>, MicrophoneDiscoveryError>
where
    H: DiscoveryHelperSession,
    F: FnOnce(Duration) -> Result<H, HelperLaunchError>,
{
    let cancelled = AtomicBool::new(false);
    list_microphones_one_shot_with_launcher_and_cancel_for_test(timeout, launcher, &cancelled)
}

fn list_microphones_one_shot_with_launcher_and_cancel_for_test<H, F>(
    timeout: Duration,
    launcher: F,
    cancelled: &AtomicBool,
) -> Result<Vec<MicrophoneDevice>, MicrophoneDiscoveryError>
where
    H: DiscoveryHelperSession,
    F: FnOnce(Duration) -> Result<H, HelperLaunchError>,
{
    if cancelled.load(Ordering::Acquire) {
        return Err(MicrophoneDiscoveryError::Cancelled);
    }
    let deadline = Instant::now() + timeout;
    let mut helper = launcher(timeout).map_err(MicrophoneDiscoveryError::Launch)?;
    if cancelled.load(Ordering::Acquire) {
        helper.cleanup(false);
        return Err(MicrophoneDiscoveryError::Cancelled);
    }
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        helper.cleanup(false);
        return Err(MicrophoneDiscoveryError::Timeout);
    }
    list_microphones_with_helper_and_cancel_for_test(helper, remaining, cancelled)
}

#[cfg(test)]
pub(crate) fn list_microphones_with_helper_for_test<H: DiscoveryHelperSession>(
    helper: H,
    timeout: Duration,
) -> Result<Vec<MicrophoneDevice>, MicrophoneDiscoveryError> {
    let cancelled = AtomicBool::new(false);
    list_microphones_with_helper_and_cancel_for_test(helper, timeout, &cancelled)
}

pub(crate) fn list_microphones_with_helper_and_cancel_for_test<H: DiscoveryHelperSession>(
    mut helper: H,
    timeout: Duration,
    cancelled: &AtomicBool,
) -> Result<Vec<MicrophoneDevice>, MicrophoneDiscoveryError> {
    let result = (|| {
        let deadline = Instant::now() + timeout;
        if cancelled.load(Ordering::Acquire) {
            return Err(MicrophoneDiscoveryError::Cancelled);
        }
        helper
            .send_command(&CaptureCommand::ListMicrophones {
                protocol_version: PROTOCOL_VERSION,
            })
            .map_err(map_discovery_transport_error)?;

        let devices = loop {
            if cancelled.load(Ordering::Acquire) {
                return Err(MicrophoneDiscoveryError::Cancelled);
            }
            if Instant::now() >= deadline {
                return Err(MicrophoneDiscoveryError::Timeout);
            }
            match helper.receive_header() {
                Ok(CaptureHeader::MicrophoneDevices(message)) => break message.devices,
                Ok(CaptureHeader::FatalError(error)) => {
                    return Err(MicrophoneDiscoveryError::Fatal {
                        code: error.code,
                        summary: error.summary,
                    });
                }
                Ok(_) => return Err(MicrophoneDiscoveryError::UnexpectedResponse),
                Err(HelperTransportError::Timeout) => continue,
                Err(error) => return Err(map_discovery_transport_error(error)),
            }
        };

        if cancelled.load(Ordering::Acquire) {
            return Err(MicrophoneDiscoveryError::Cancelled);
        }
        helper
            .send_command(&CaptureCommand::Shutdown {
                protocol_version: PROTOCOL_VERSION,
            })
            .map_err(map_discovery_transport_error)?;

        loop {
            if cancelled.load(Ordering::Acquire) {
                return Err(MicrophoneDiscoveryError::Cancelled);
            }
            if Instant::now() >= deadline {
                return Err(MicrophoneDiscoveryError::MissingShutdownAck);
            }
            match helper.receive_header() {
                Ok(CaptureHeader::ShutdownAck) => return Ok(devices),
                Ok(CaptureHeader::FatalError(error)) => {
                    return Err(MicrophoneDiscoveryError::Fatal {
                        code: error.code,
                        summary: error.summary,
                    });
                }
                Ok(_) => return Err(MicrophoneDiscoveryError::UnexpectedResponse),
                Err(HelperTransportError::Timeout) => continue,
                Err(error) => return Err(map_discovery_transport_error(error)),
            }
        }
    })();

    helper.cleanup(result.is_ok());
    result
}

fn map_discovery_transport_error(error: HelperTransportError) -> MicrophoneDiscoveryError {
    match error {
        HelperTransportError::Timeout => MicrophoneDiscoveryError::Timeout,
        HelperTransportError::Io(io_error) => {
            MicrophoneDiscoveryError::Transport(format!("io: {io_error}"))
        }
        HelperTransportError::Protocol(protocol_error) => {
            MicrophoneDiscoveryError::Transport(format!("protocol: {protocol_error:?}"))
        }
    }
}

#[derive(Debug)]
struct LaunchSpec {
    executable: PathBuf,
    args: Vec<OsString>,
    env: Vec<(OsString, OsString)>,
    require_signature: bool,
}

struct ValidatedExecutable {
    path: PathBuf,
    identity: ExecutableFileIdentity,
}

pub fn launch_packaged_helper(
    runtime_parent: &Path,
    timeout: Duration,
) -> Result<AuthenticatedHelper, HelperLaunchError> {
    launch(
        runtime_parent,
        timeout,
        LaunchSpec {
            executable: discover_helper_path()?,
            args: Vec::new(),
            env: Vec::new(),
            require_signature: true,
        },
        None,
    )
}

pub fn discover_helper_path() -> Result<PathBuf, HelperLaunchError> {
    #[cfg(debug_assertions)]
    if let Some(path) = std::env::var_os("LIFESUB_CAPTURE_HELPER_DEV_PATH") {
        return Ok(validate_executable(Path::new(&path), false)?.path);
    }
    packaged_helper_path()
}

#[cfg(test)]
pub(crate) fn launch_test_helper(
    runtime_parent: &Path,
    executable: PathBuf,
    args: Vec<OsString>,
    env: Vec<(OsString, OsString)>,
    timeout: Duration,
    nonce: [u8; 32],
) -> Result<AuthenticatedHelper, HelperLaunchError> {
    launch(
        runtime_parent,
        timeout,
        LaunchSpec {
            executable,
            args,
            env,
            require_signature: false,
        },
        Some(nonce),
    )
}

fn launch(
    runtime_parent: &Path,
    timeout: Duration,
    spec: LaunchSpec,
    nonce_override: Option<[u8; 32]>,
) -> Result<AuthenticatedHelper, HelperLaunchError> {
    let validated = validate_executable(&spec.executable, spec.require_signature)?;
    let executable = validated.path;
    let endpoint =
        PrivateCaptureEndpoint::bind(runtime_parent).map_err(|_| HelperLaunchError::Io)?;
    endpoint
        .listener()
        .set_nonblocking(true)
        .map_err(|_| HelperLaunchError::Io)?;
    let (read_fd, write_fd) = create_bootstrap_pipe()?;
    let mut command = Command::new(&executable);
    command
        .args(&spec.args)
        .envs(spec.env)
        .env("LIFESUB_CAPTURE_SOCKET", endpoint.socket_path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    unsafe {
        command.pre_exec(move || {
            if libc::dup2(read_fd, BOOTSTRAP_FD) < 0 {
                return Err(io::Error::last_os_error());
            }
            if libc::fcntl(BOOTSTRAP_FD, libc::F_SETFD, 0) < 0 {
                return Err(io::Error::last_os_error());
            }
            if read_fd != BOOTSTRAP_FD {
                libc::close(read_fd);
            }
            libc::close(write_fd);
            Ok(())
        });
    }
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(_) => {
            close_fd(read_fd);
            close_fd(write_fd);
            return Err(HelperLaunchError::Io);
        }
    };
    close_fd(read_fd);

    let nonce = SensitiveNonce(if let Some(value) = nonce_override {
        value
    } else {
        let mut value = [0_u8; 32];
        unsafe { libc::arc4random_buf(value.as_mut_ptr().cast(), value.len()) };
        value
    });
    if unsafe { fs::File::from_raw_fd(write_fd) }
        .write_all(&nonce.0)
        .is_err()
    {
        terminate_child(&mut child);
        return Err(HelperLaunchError::Io);
    }

    let deadline = Instant::now() + timeout;
    let stream = loop {
        match endpoint.listener().accept() {
            Ok((stream, _)) => break stream,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if child
                    .try_wait()
                    .map_err(|_| HelperLaunchError::Io)?
                    .is_some()
                {
                    return Err(HelperLaunchError::HelperExitedEarly);
                }
                if Instant::now() >= deadline {
                    terminate_child(&mut child);
                    return Err(HelperLaunchError::HandshakeTimeout);
                }
                thread::sleep(Duration::from_millis(5));
            }
            Err(_) => {
                terminate_child(&mut child);
                return Err(HelperLaunchError::Io);
            }
        }
    };

    if stream.set_nonblocking(false).is_err() {
        terminate_child(&mut child);
        return Err(HelperLaunchError::Io);
    }
    if stream
        .set_read_timeout(Some(Duration::from_millis(50)))
        .is_err()
    {
        terminate_child(&mut child);
        return Err(HelperLaunchError::Io);
    }
    let peer = match peer_identity(&stream) {
        Ok(peer) => peer,
        Err(error) => {
            terminate_child(&mut child);
            return Err(error);
        }
    };
    if Instant::now() >= deadline {
        terminate_child(&mut child);
        return Err(HelperLaunchError::HandshakeTimeout);
    }
    let mut verifier =
        HandshakeVerifier::new(nonce.0, child.id(), unsafe { libc::getuid() }, &executable)
            .with_file_identity(validated.identity);
    let reader_stream = match stream.try_clone() {
        Ok(stream) => stream,
        Err(_) => {
            terminate_child(&mut child);
            return Err(HelperLaunchError::Io);
        }
    };
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let reader = thread::spawn(move || {
        let result = FrameDecoder::default().decode(&mut &reader_stream);
        let _ = sender.send(result);
    });
    let remaining = deadline.saturating_duration_since(Instant::now());
    let frame = match receiver.recv_timeout(remaining.max(Duration::from_millis(1))) {
        Ok(Ok(frame)) => frame,
        Ok(Err(super::protocol::CaptureProtocolError::FrameReadTimeout))
        | Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            let _ = stream.shutdown(std::net::Shutdown::Both);
            terminate_child(&mut child);
            let _ = reader.join();
            return Err(HelperLaunchError::HandshakeTimeout);
        }
        Ok(Err(_)) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            terminate_child(&mut child);
            let _ = reader.join();
            return Err(HelperLaunchError::MalformedHello);
        }
    };
    let _ = reader.join();
    if let Err(error) = verifier.verify(&peer, &frame.header) {
        terminate_child(&mut child);
        return Err(HelperLaunchError::Handshake(error));
    }
    let mut validator = ProtocolValidator::default();
    if validator.observe(&frame.header).is_err() {
        terminate_child(&mut child);
        return Err(HelperLaunchError::MalformedHello);
    }
    Ok(AuthenticatedHelper {
        child: Some(child),
        stream,
        endpoint,
        validator,
    })
}

fn validate_executable(
    path: &Path,
    require_signature: bool,
) -> Result<ValidatedExecutable, HelperLaunchError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| HelperLaunchError::InvalidExecutable)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(HelperLaunchError::InvalidExecutable);
    }
    let canonical = path
        .canonicalize()
        .map_err(|_| HelperLaunchError::InvalidExecutable)?;
    let identity = executable_file_identity(&canonical)?;
    if require_signature {
        let output = Command::new("/usr/bin/codesign")
            .args([OsStr::new("--verify"), OsStr::new("--strict")])
            .arg(&canonical)
            .output()
            .map_err(|_| HelperLaunchError::SignatureInvalid)?;
        if !output.status.success() {
            return Err(HelperLaunchError::SignatureInvalid);
        }
        let details = Command::new("/usr/bin/codesign")
            .args([OsStr::new("-d"), OsStr::new("--verbose=4")])
            .arg(&canonical)
            .output()
            .map_err(|_| HelperLaunchError::SignatureInvalid)?;
        let mut description = details.stdout;
        description.extend_from_slice(&details.stderr);
        let description = String::from_utf8_lossy(&description);
        let identifier = description
            .lines()
            .find_map(|line| line.strip_prefix("Identifier="));
        if !details.status.success() || !identifier.is_some_and(helper_identifier_is_allowed) {
            return Err(HelperLaunchError::SignatureInvalid);
        }
        let expected_hash = option_env!("LIFESUB_CAPTURE_HELPER_CODE_SHA256")
            .ok_or(HelperLaunchError::SignatureInvalid)?;
        if canonical_executable_sha256(&canonical)? != expected_hash {
            return Err(HelperLaunchError::SignatureInvalid);
        }
    }
    Ok(ValidatedExecutable {
        path: canonical,
        identity,
    })
}

fn helper_identifier_is_allowed(identifier: &str) -> bool {
    if identifier == CAPTURE_HELPER_IDENTIFIER {
        return true;
    }
    let Some(suffix) = identifier.strip_prefix(&format!("{CAPTURE_HELPER_IDENTIFIER}-")) else {
        return false;
    };
    suffix.len() == TAURI_SIDECAR_IDENTIFIER_SUFFIX_LENGTH
        && suffix
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn canonical_executable_sha256(path: &Path) -> Result<String, HelperLaunchError> {
    let temporary_root = std::env::temp_dir().join(format!(
        "lifesub-helper-code-hash-{}",
        uuid::Uuid::new_v4().simple()
    ));
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&temporary_root)
        .map_err(|_| HelperLaunchError::SignatureInvalid)?;
    let temporary = temporary_root.join(CAPTURE_HELPER_IDENTIFIER);
    let result = (|| {
        let mut source = fs::File::open(path).map_err(|_| HelperLaunchError::SignatureInvalid)?;
        let mut destination = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)
            .map_err(|_| HelperLaunchError::SignatureInvalid)?;
        io::copy(&mut source, &mut destination).map_err(|_| HelperLaunchError::SignatureInvalid)?;
        destination
            .sync_all()
            .map_err(|_| HelperLaunchError::SignatureInvalid)?;
        drop(destination);

        let output = Command::new("/usr/bin/codesign")
            .arg("--remove-signature")
            .arg(&temporary)
            .output()
            .map_err(|_| HelperLaunchError::SignatureInvalid)?;
        if !output.status.success() {
            return Err(HelperLaunchError::SignatureInvalid);
        }
        sha256_path(&temporary).map_err(|_| HelperLaunchError::SignatureInvalid)
    })();
    let _ = fs::remove_file(&temporary);
    let _ = fs::remove_dir(&temporary_root);
    result
}

#[cfg(test)]
pub(crate) fn canonical_executable_sha256_for_test(
    path: &Path,
) -> Result<String, HelperLaunchError> {
    canonical_executable_sha256(path)
}

#[cfg(test)]
pub(crate) fn helper_identifier_is_allowed_for_test(identifier: &str) -> bool {
    helper_identifier_is_allowed(identifier)
}

fn packaged_helper_path() -> Result<PathBuf, HelperLaunchError> {
    let executable = std::env::current_exe().map_err(|_| HelperLaunchError::InvalidExecutable)?;
    let directory = executable
        .parent()
        .ok_or(HelperLaunchError::InvalidExecutable)?;
    Ok(validate_executable(&directory.join("lifesub-capture-helper"), true)?.path)
}

fn create_bootstrap_pipe() -> Result<(RawFd, RawFd), HelperLaunchError> {
    let mut fds = [-1; 2];
    if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
        return Err(HelperLaunchError::Io);
    }
    for fd in fds {
        if unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
            close_fd(fds[0]);
            close_fd(fds[1]);
            return Err(HelperLaunchError::Io);
        }
    }
    Ok((fds[0], fds[1]))
}

fn peer_identity(stream: &UnixStream) -> Result<PeerIdentity, HelperLaunchError> {
    let fd = stream.as_raw_fd();
    let mut uid: libc::uid_t = 0;
    let mut gid: libc::gid_t = 0;
    if unsafe { libc::getpeereid(fd, &mut uid, &mut gid) } != 0 {
        return Err(HelperLaunchError::PeerIdentityUnavailable);
    }
    let mut pid: libc::pid_t = 0;
    let mut length = std::mem::size_of::<libc::pid_t>() as libc::socklen_t;
    if unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_LOCAL,
            libc::LOCAL_PEERPID,
            (&mut pid as *mut libc::pid_t).cast(),
            &mut length,
        )
    } != 0
    {
        return Err(HelperLaunchError::PeerIdentityUnavailable);
    }
    let executable = executable_for_pid(pid)?;
    Ok(PeerIdentity {
        uid,
        pid: pid as u32,
        file_identity: executable_file_identity(&executable)?,
        executable,
    })
}

fn executable_file_identity(path: &Path) -> Result<ExecutableFileIdentity, HelperLaunchError> {
    let metadata = fs::metadata(path).map_err(|_| HelperLaunchError::PeerIdentityUnavailable)?;
    let sha256 = sha256_path(path).map_err(|_| HelperLaunchError::PeerIdentityUnavailable)?;
    Ok(ExecutableFileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
        sha256,
    })
}

fn sha256_path(path: &Path) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = std::io::Read::read(&mut file, &mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn executable_for_pid(pid: libc::pid_t) -> Result<PathBuf, HelperLaunchError> {
    let mut buffer = vec![0_u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
    let length =
        unsafe { libc::proc_pidpath(pid, buffer.as_mut_ptr().cast(), buffer.len() as u32) };
    if length <= 0 {
        return Err(HelperLaunchError::PeerIdentityUnavailable);
    }
    buffer.truncate(length as usize);
    if buffer.last() == Some(&0) {
        buffer.pop();
    }
    PathBuf::from(String::from_utf8_lossy(&buffer).into_owned())
        .canonicalize()
        .map_err(|_| HelperLaunchError::PeerIdentityUnavailable)
}

fn terminate_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn close_fd(fd: RawFd) {
    if fd >= 0 {
        unsafe { libc::close(fd) };
    }
}
