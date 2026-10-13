use std::collections::VecDeque;
use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write};
use std::os::fd::FromRawFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use sha2::Digest;

use crate::capture::helper::{
    AuthenticatedHelper, DiscoveryHelperSession, HelperLaunchError, HelperTransportError,
    MicrophoneDiscoveryError, PrivateCaptureEndpoint, canonical_executable_sha256_for_test,
    helper_identifier_is_allowed_for_test, launch_test_helper,
    list_microphones_one_shot_with_launcher_for_test,
    list_microphones_with_helper_and_cancel_for_test, list_microphones_with_helper_for_test,
    permission_state_with_helper_for_test, probe_microphone_with_helper_and_cancel_for_test,
    probe_microphone_with_helper_for_test,
};
use crate::capture::helper_auth::{
    ExecutableFileIdentity, HandshakeError, HandshakeVerifier, PeerIdentity,
};
use crate::capture::protocol::{
    CaptureCommand, CaptureHeader, FatalError, Hello, HelloCapability, MicrophoneDevice,
    MicrophoneDevicesMessage, MicrophoneProbeMessage, MicrophoneProbeResult, MicrophoneTransport,
    PROTOCOL_VERSION, PermissionState, PermissionStateMessage, Source,
};

const NONCE: [u8; 32] = [0x5a; 32];

fn hello(pid: u32) -> CaptureHeader {
    CaptureHeader::Hello(Hello {
        protocol_version: PROTOCOL_VERSION,
        helper_pid: pid,
        launch_nonce: hex::encode(NONCE),
        supported_sources: vec![Source::Microphone, Source::SystemAudio],
        capabilities: vec![
            HelloCapability::DeviceDiscovery,
            HelloCapability::ExactMicrophoneSelection,
            HelloCapability::MicrophoneProbe,
            HelloCapability::PermissionDiagnostics,
        ],
    })
}

fn identity(pid: u32, executable: PathBuf) -> PeerIdentity {
    PeerIdentity {
        uid: unsafe { libc::getuid() },
        pid,
        executable,
        file_identity: ExecutableFileIdentity {
            device: 1,
            inode: 2,
            sha256: "11".repeat(32),
        },
    }
}

#[test]
fn capture_helper_test_private_endpoint_uses_owner_only_permissions() {
    let parent = tempfile::tempdir().unwrap();
    let endpoint = PrivateCaptureEndpoint::bind(parent.path()).unwrap();

    assert_eq!(
        fs::metadata(endpoint.directory())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(endpoint.socket_path())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}

#[test]
fn capture_helper_test_handshake_binds_nonce_uid_pid_and_executable() {
    let executable = std::env::current_exe().unwrap().canonicalize().unwrap();
    let pid = std::process::id();
    let mut verifier = HandshakeVerifier::new(NONCE, pid, unsafe { libc::getuid() }, &executable);

    verifier
        .verify(&identity(pid, executable), &hello(pid))
        .unwrap();
    assert_eq!(
        verifier.verify(
            &identity(pid, std::env::current_exe().unwrap()),
            &hello(pid)
        ),
        Err(HandshakeError::Replay)
    );
}

#[test]
fn capture_helper_test_rejects_identity_and_nonce_mismatches() {
    let executable = std::env::current_exe().unwrap().canonicalize().unwrap();
    let pid = std::process::id();
    for (peer, header, expected) in [
        (
            PeerIdentity {
                uid: unsafe { libc::getuid() } + 1,
                pid,
                executable: executable.clone(),
                file_identity: identity(pid, executable.clone()).file_identity,
            },
            hello(pid),
            HandshakeError::UidMismatch,
        ),
        (
            identity(pid + 1, executable.clone()),
            hello(pid),
            HandshakeError::PidMismatch,
        ),
        (
            identity(pid, executable.with_extension("other")),
            hello(pid),
            HandshakeError::ExecutableMismatch,
        ),
        (
            identity(pid, std::env::current_exe().unwrap()),
            CaptureHeader::Hello(Hello {
                launch_nonce: "00".repeat(32),
                ..match hello(pid) {
                    CaptureHeader::Hello(value) => value,
                    _ => unreachable!(),
                }
            }),
            HandshakeError::NonceMismatch,
        ),
    ] {
        let expected_executable = std::env::current_exe().unwrap().canonicalize().unwrap();
        let mut verifier =
            HandshakeVerifier::new(NONCE, pid, unsafe { libc::getuid() }, expected_executable);
        assert_eq!(verifier.verify(&peer, &header), Err(expected));
    }
}

#[test]
fn capture_helper_test_rejects_same_path_executable_swap() {
    let executable = std::env::current_exe().unwrap().canonicalize().unwrap();
    let pid = std::process::id();
    let expected = identity(pid, executable.clone()).file_identity;
    let mut peer = identity(pid, executable.clone());
    peer.file_identity.inode += 1;
    peer.file_identity.sha256 = "22".repeat(32);
    let mut verifier = HandshakeVerifier::new(NONCE, pid, unsafe { libc::getuid() }, executable)
        .with_file_identity(expected);
    assert_eq!(
        verifier.verify(&peer, &hello(pid)),
        Err(HandshakeError::ExecutableMismatch)
    );
}

#[test]
fn capture_helper_test_canonical_code_hash_survives_tauri_resigning() {
    let temporary = tempfile::tempdir().unwrap();
    let source = std::env::current_exe().unwrap();
    let resigned = temporary.path().join("lifesub-capture-helper");
    fs::copy(source, &resigned).unwrap();

    let sign = |identifier: &str| {
        let status = Command::new("/usr/bin/codesign")
            .args(["--force", "--sign", "-", "--identifier", identifier])
            .arg(&resigned)
            .status()
            .unwrap();
        assert!(status.success());
    };
    sign("lifesub-capture-helper");
    let first_full_hash = fs::read(&resigned).map(sha2::Sha256::digest).unwrap();
    let first_code_hash = canonical_executable_sha256_for_test(&resigned).unwrap();

    sign("lifesub-capture-helper-tauri-sidecar");
    let second_full_hash = fs::read(&resigned).map(sha2::Sha256::digest).unwrap();
    let second_code_hash = canonical_executable_sha256_for_test(&resigned).unwrap();

    assert_ne!(first_full_hash, second_full_hash);
    assert_eq!(first_code_hash, second_code_hash);
}

#[test]
fn capture_helper_test_helper_identifier_rejects_non_lifesub_binaries() {
    assert!(helper_identifier_is_allowed_for_test(
        "lifesub-capture-helper"
    ));
    assert!(helper_identifier_is_allowed_for_test(
        "lifesub-capture-helper-55554944e80760a856e63f23a5f3b0ecd3195cad"
    ));
    assert!(!helper_identifier_is_allowed_for_test("other-helper"));
    assert!(!helper_identifier_is_allowed_for_test(
        "lifesub-capture-helper-"
    ));
    assert!(!helper_identifier_is_allowed_for_test(
        "lifesub-capture-helper-not-hex"
    ));
}

#[test]
fn capture_helper_test_nonce_is_redacted_from_debug_and_errors() {
    let executable = std::env::current_exe().unwrap().canonicalize().unwrap();
    let verifier = HandshakeVerifier::new(NONCE, 42, unsafe { libc::getuid() }, executable);
    let rendered = format!("{verifier:?}");
    assert!(!rendered.contains(&hex::encode(NONCE)));
    assert!(rendered.contains("[REDACTED]"));
}

#[test]
fn capture_helper_test_timeout_and_early_exit_are_distinct() {
    assert_ne!(
        HelperLaunchError::HandshakeTimeout,
        HelperLaunchError::HelperExitedEarly
    );
}

#[test]
fn capture_helper_test_child_entrypoint() {
    let Ok(role) = std::env::var("LIFESUB_CAPTURE_TEST_ROLE") else {
        return;
    };
    let mut nonce = [0_u8; 32];
    unsafe { fs::File::from_raw_fd(3) }
        .read_exact(&mut nonce)
        .unwrap();
    if role == "early_exit" {
        nonce.fill(0);
        return;
    }
    if role == "timeout" {
        nonce.fill(0);
        std::thread::sleep(Duration::from_secs(2));
        return;
    }
    let socket = std::env::var_os("LIFESUB_CAPTURE_SOCKET").unwrap();
    let mut stream = UnixStream::connect(socket).unwrap();
    if role == "connected_timeout" {
        stream.write_all(&[0, 0, 0, 10, b'{']).unwrap();
        nonce.fill(0);
        std::thread::sleep(Duration::from_secs(2));
        return;
    }
    if role == "malformed" {
        stream
            .write_all(&[0, 0, 0, 2, b'{', b'}', 0, 0, 0, 0])
            .unwrap();
        nonce.fill(0);
        let mut byte = [0_u8; 1];
        let _ = stream.read(&mut byte);
        return;
    }
    let reported_pid = if role == "pid_mismatch" {
        std::process::id() + 1
    } else {
        std::process::id()
    };
    if role == "nonce_mismatch" {
        nonce[0] ^= 0xff;
    }
    let header = CaptureHeader::Hello(Hello {
        protocol_version: PROTOCOL_VERSION,
        helper_pid: reported_pid,
        launch_nonce: hex::encode(nonce),
        supported_sources: vec![Source::Microphone, Source::SystemAudio],
        capabilities: vec![
            HelloCapability::DeviceDiscovery,
            HelloCapability::ExactMicrophoneSelection,
            HelloCapability::MicrophoneProbe,
            HelloCapability::PermissionDiagnostics,
        ],
    });
    nonce.fill(0);
    let frame = crate::capture::protocol::encode_frame(&header, &[]).unwrap();
    stream.write_all(&frame).unwrap();
    if role == "replay" {
        stream.write_all(&frame).unwrap();
    }
    let mut byte = [0_u8; 1];
    let _ = stream.read(&mut byte);
}

fn launch_child(role: &str, timeout: Duration) -> Result<AuthenticatedHelper, HelperLaunchError> {
    let parent = tempfile::tempdir().unwrap().keep();
    let executable = std::env::current_exe().unwrap().canonicalize().unwrap();
    launch_test_helper(
        &parent,
        executable,
        vec![
            OsString::from("capture_helper_test::capture_helper_test_child_entrypoint"),
            OsString::from("--exact"),
            OsString::from("--nocapture"),
        ],
        vec![(
            OsString::from("LIFESUB_CAPTURE_TEST_ROLE"),
            OsString::from(role),
        )],
        timeout,
        NONCE,
    )
}

#[derive(Default)]
struct FakeDiscoveryHelper {
    sent: Arc<Mutex<Vec<CaptureCommand>>>,
    received: VecDeque<Result<CaptureHeader, HelperTransportError>>,
    cleanup: Arc<Mutex<Vec<bool>>>,
}

type FakeDiscoveryHarness = (
    FakeDiscoveryHelper,
    Arc<Mutex<Vec<CaptureCommand>>>,
    Arc<Mutex<Vec<bool>>>,
);

impl FakeDiscoveryHelper {
    fn with_events(
        events: impl IntoIterator<Item = Result<CaptureHeader, HelperTransportError>>,
    ) -> FakeDiscoveryHarness {
        let sent = Arc::new(Mutex::new(Vec::new()));
        let cleanup = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                sent: sent.clone(),
                received: events.into_iter().collect(),
                cleanup: cleanup.clone(),
            },
            sent,
            cleanup,
        )
    }
}

impl DiscoveryHelperSession for FakeDiscoveryHelper {
    fn send_command(&mut self, command: &CaptureCommand) -> Result<(), HelperTransportError> {
        self.sent.lock().unwrap().push(command.clone());
        Ok(())
    }

    fn receive_header(&mut self) -> Result<CaptureHeader, HelperTransportError> {
        self.received
            .pop_front()
            .unwrap_or(Err(HelperTransportError::Timeout))
    }

    fn cleanup(&mut self, orderly: bool) {
        self.cleanup.lock().unwrap().push(orderly);
    }
}

fn microphone_device() -> MicrophoneDevice {
    MicrophoneDevice {
        uid: "mic-uid-1".to_string(),
        name: "Built-in Microphone".to_string(),
        is_default: true,
        is_connected: true,
        transport: MicrophoneTransport::BuiltIn,
    }
}

#[test]
fn capture_helper_test_real_child_authenticates_without_nonce_leaks() {
    let helper = launch_child("success", Duration::from_secs(10)).unwrap();
    assert_ne!(helper.child_id(), 0);
    let nonce = hex::encode(NONCE);
    let process = Command::new("/bin/ps")
        .args([
            "eww",
            "-p",
            &helper.child_id().to_string(),
            "-o",
            "command=",
        ])
        .output()
        .unwrap();
    assert!(process.status.success());
    assert!(!String::from_utf8_lossy(&process.stdout).contains(&nonce));
    for entry in fs::read_dir(helper.endpoint_directory()).unwrap() {
        let entry = entry.unwrap();
        assert!(!entry.file_name().to_string_lossy().contains(&nonce));
        if entry.file_type().unwrap().is_file() {
            let bytes = fs::read(entry.path()).unwrap();
            assert!(
                !bytes
                    .windows(nonce.len())
                    .any(|window| window == nonce.as_bytes())
            );
        }
    }
}

#[test]
fn capture_helper_test_one_shot_microphone_discovery_sends_shutdown_and_cleans_up_orderly() {
    let (helper, sent, cleanup) = FakeDiscoveryHelper::with_events([
        Ok(CaptureHeader::MicrophoneDevices(MicrophoneDevicesMessage {
            devices: vec![microphone_device()],
        })),
        Ok(CaptureHeader::ShutdownAck),
    ]);

    let devices = list_microphones_with_helper_for_test(helper, Duration::from_millis(50)).unwrap();

    assert_eq!(devices, vec![microphone_device()]);
    assert_eq!(
        *sent.lock().unwrap(),
        vec![
            CaptureCommand::ListMicrophones {
                protocol_version: PROTOCOL_VERSION,
            },
            CaptureCommand::Shutdown {
                protocol_version: PROTOCOL_VERSION,
            },
        ]
    );
    assert_eq!(*cleanup.lock().unwrap(), vec![true]);
}

#[test]
fn capture_helper_test_one_shot_microphone_probe_sends_exact_uid_and_cleans_up_orderly() {
    let result = MicrophoneProbeMessage {
        uid: "mic-uid-1".to_string(),
        name: "Built-in Microphone".to_string(),
        transport: MicrophoneTransport::BuiltIn,
        input_sample_rate_hz: Some(44_100),
        input_channel_count: Some(1),
        read_back_uid: Some("mic-uid-1".to_string()),
        first_frame_sample_count: 371,
        elapsed_ms: 42,
        result: MicrophoneProbeResult::Compatible,
    };
    let (helper, sent, cleanup) = FakeDiscoveryHelper::with_events([
        Ok(CaptureHeader::MicrophoneProbeResult(result.clone())),
        Ok(CaptureHeader::ShutdownAck),
    ]);

    let observed =
        probe_microphone_with_helper_for_test(helper, "mic-uid-1", Duration::from_millis(50))
            .unwrap();

    assert_eq!(observed, result);
    assert_eq!(
        *sent.lock().unwrap(),
        vec![
            CaptureCommand::ProbeMicrophone {
                protocol_version: PROTOCOL_VERSION,
                microphone_device_uid: "mic-uid-1".to_string(),
            },
            CaptureCommand::Shutdown {
                protocol_version: PROTOCOL_VERSION,
            },
        ]
    );
    assert_eq!(*cleanup.lock().unwrap(), vec![true]);
}

#[test]
fn capture_helper_test_one_shot_microphone_probe_rejects_extra_response_and_reaps() {
    let result = MicrophoneProbeMessage {
        uid: "mic-uid-1".to_string(),
        name: "Built-in Microphone".to_string(),
        transport: MicrophoneTransport::BuiltIn,
        input_sample_rate_hz: Some(44_100),
        input_channel_count: Some(1),
        read_back_uid: Some("mic-uid-1".to_string()),
        first_frame_sample_count: 371,
        elapsed_ms: 42,
        result: MicrophoneProbeResult::Compatible,
    };
    let (helper, _sent, cleanup) = FakeDiscoveryHelper::with_events([
        Ok(CaptureHeader::MicrophoneProbeResult(result.clone())),
        Ok(CaptureHeader::MicrophoneProbeResult(result)),
    ]);

    let error =
        probe_microphone_with_helper_for_test(helper, "mic-uid-1", Duration::from_millis(50))
            .unwrap_err();

    assert_eq!(error, MicrophoneDiscoveryError::UnexpectedResponse);
    assert_eq!(*cleanup.lock().unwrap(), vec![false]);
}

#[test]
fn capture_helper_test_one_shot_microphone_probe_cancels_before_sending_and_reaps() {
    let (helper, sent, cleanup) = FakeDiscoveryHelper::with_events([]);
    let cancelled = AtomicBool::new(true);

    let error = probe_microphone_with_helper_and_cancel_for_test(
        helper,
        "mic-uid-1",
        Duration::from_millis(50),
        &cancelled,
    )
    .unwrap_err();

    assert_eq!(error, MicrophoneDiscoveryError::Cancelled);
    assert!(sent.lock().unwrap().is_empty());
    assert_eq!(*cleanup.lock().unwrap(), vec![false]);
}

#[test]
fn capture_helper_test_one_shot_permission_query_is_orderly_and_unique() {
    let permissions = PermissionStateMessage {
        microphone: PermissionState::Denied,
        screen_recording: PermissionState::NotGranted,
    };
    let (helper, sent, cleanup) = FakeDiscoveryHelper::with_events([
        Ok(CaptureHeader::PermissionState(permissions.clone())),
        Ok(CaptureHeader::ShutdownAck),
    ]);

    let observed =
        permission_state_with_helper_for_test(helper, Duration::from_millis(50)).unwrap();

    assert_eq!(observed, permissions);
    assert_eq!(
        *sent.lock().unwrap(),
        vec![
            CaptureCommand::GetPermissionState {
                protocol_version: PROTOCOL_VERSION,
            },
            CaptureCommand::Shutdown {
                protocol_version: PROTOCOL_VERSION,
            },
        ]
    );
    assert_eq!(*cleanup.lock().unwrap(), vec![true]);
}

#[test]
fn capture_helper_test_one_shot_microphone_discovery_times_out_and_cleans_up() {
    let (helper, _sent, cleanup) =
        FakeDiscoveryHelper::with_events([Err(HelperTransportError::Timeout)]);

    let error =
        list_microphones_with_helper_for_test(helper, Duration::from_millis(1)).unwrap_err();

    assert_eq!(error, MicrophoneDiscoveryError::Timeout);
    assert_eq!(*cleanup.lock().unwrap(), vec![false]);
}

#[test]
fn capture_helper_test_one_shot_microphone_discovery_uses_one_total_deadline() {
    let (helper, sent, cleanup) = FakeDiscoveryHelper::with_events([
        Ok(CaptureHeader::MicrophoneDevices(MicrophoneDevicesMessage {
            devices: vec![microphone_device()],
        })),
        Ok(CaptureHeader::ShutdownAck),
    ]);

    let error =
        list_microphones_one_shot_with_launcher_for_test(Duration::from_millis(1), move |_| {
            std::thread::sleep(Duration::from_millis(5));
            Ok(helper)
        })
        .unwrap_err();

    assert_eq!(error, MicrophoneDiscoveryError::Timeout);
    assert!(sent.lock().unwrap().is_empty());
    assert_eq!(*cleanup.lock().unwrap(), vec![false]);
}

#[test]
fn capture_helper_test_one_shot_microphone_discovery_cancels_and_reaps() {
    let (helper, sent, cleanup) = FakeDiscoveryHelper::with_events([]);
    let cancelled = Arc::new(AtomicBool::new(false));
    let cancellation_trigger = cancelled.clone();
    let trigger = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(2));
        cancellation_trigger.store(true, Ordering::Release);
    });

    let error = list_microphones_with_helper_and_cancel_for_test(
        helper,
        Duration::from_millis(100),
        &cancelled,
    )
    .unwrap_err();
    trigger.join().unwrap();

    assert_eq!(error, MicrophoneDiscoveryError::Cancelled);
    assert_eq!(sent.lock().unwrap().len(), 1);
    assert_eq!(*cleanup.lock().unwrap(), vec![false]);
}

#[test]
fn capture_helper_test_one_shot_microphone_discovery_rejects_fatal_unexpected_and_missing_ack() {
    let (fatal_helper, _fatal_sent, fatal_cleanup) =
        FakeDiscoveryHelper::with_events([Ok(CaptureHeader::FatalError(FatalError {
            code: "selected_microphone_unavailable".to_string(),
            summary: "所选麦克风当前不可用。".to_string(),
        }))]);
    assert_eq!(
        list_microphones_with_helper_for_test(fatal_helper, Duration::from_millis(50)).unwrap_err(),
        MicrophoneDiscoveryError::Fatal {
            code: "selected_microphone_unavailable".to_string(),
            summary: "所选麦克风当前不可用。".to_string(),
        }
    );
    assert_eq!(*fatal_cleanup.lock().unwrap(), vec![false]);

    let (unexpected_helper, _unexpected_sent, unexpected_cleanup) =
        FakeDiscoveryHelper::with_events([Ok(CaptureHeader::Hello(match hello(1) {
            CaptureHeader::Hello(value) => value,
            _ => unreachable!(),
        }))]);
    assert_eq!(
        list_microphones_with_helper_for_test(unexpected_helper, Duration::from_millis(50))
            .unwrap_err(),
        MicrophoneDiscoveryError::UnexpectedResponse
    );
    assert_eq!(*unexpected_cleanup.lock().unwrap(), vec![false]);

    let (missing_ack_helper, _missing_ack_sent, missing_ack_cleanup) =
        FakeDiscoveryHelper::with_events([
            Ok(CaptureHeader::MicrophoneDevices(MicrophoneDevicesMessage {
                devices: vec![microphone_device()],
            })),
            Err(HelperTransportError::Timeout),
        ]);
    assert_eq!(
        list_microphones_with_helper_for_test(missing_ack_helper, Duration::from_millis(1))
            .unwrap_err(),
        MicrophoneDiscoveryError::MissingShutdownAck
    );
    assert_eq!(*missing_ack_cleanup.lock().unwrap(), vec![false]);
}

#[test]
fn capture_helper_test_one_shot_microphone_discovery_rejects_duplicate_devices_before_shutdown_ack()
{
    let (extra_helper, extra_sent, extra_cleanup) = FakeDiscoveryHelper::with_events([
        Ok(CaptureHeader::MicrophoneDevices(MicrophoneDevicesMessage {
            devices: vec![microphone_device()],
        })),
        Ok(CaptureHeader::MicrophoneDevices(MicrophoneDevicesMessage {
            devices: vec![microphone_device()],
        })),
    ]);
    assert_eq!(
        list_microphones_with_helper_for_test(extra_helper, Duration::from_millis(50)).unwrap_err(),
        MicrophoneDiscoveryError::UnexpectedResponse
    );
    assert_eq!(
        *extra_sent.lock().unwrap(),
        vec![
            CaptureCommand::ListMicrophones {
                protocol_version: PROTOCOL_VERSION,
            },
            CaptureCommand::Shutdown {
                protocol_version: PROTOCOL_VERSION,
            },
        ]
    );
    assert_eq!(*extra_cleanup.lock().unwrap(), vec![false]);
}

#[test]
fn capture_helper_test_one_shot_microphone_discovery_rejects_io_transport() {
    let (io_helper, _io_sent, io_cleanup) = FakeDiscoveryHelper::with_events([Err(
        HelperTransportError::Io(std::io::Error::other("broken pipe")),
    )]);
    let error =
        list_microphones_with_helper_for_test(io_helper, Duration::from_millis(50)).unwrap_err();
    assert!(
        matches!(error, MicrophoneDiscoveryError::Transport(summary) if summary.contains("broken pipe"))
    );
    assert_eq!(*io_cleanup.lock().unwrap(), vec![false]);
}

#[test]
fn capture_helper_test_real_child_rejects_bad_handshakes_and_lifecycle_failures() {
    for (role, timeout, expected) in [
        (
            "pid_mismatch",
            Duration::from_secs(10),
            HelperLaunchError::Handshake(HandshakeError::PidMismatch),
        ),
        (
            "nonce_mismatch",
            Duration::from_secs(10),
            HelperLaunchError::Handshake(HandshakeError::NonceMismatch),
        ),
        (
            "malformed",
            Duration::from_secs(10),
            HelperLaunchError::MalformedHello,
        ),
        (
            "early_exit",
            Duration::from_secs(10),
            HelperLaunchError::HelperExitedEarly,
        ),
        (
            "timeout",
            Duration::from_millis(150),
            HelperLaunchError::HandshakeTimeout,
        ),
        (
            "connected_timeout",
            Duration::from_millis(150),
            HelperLaunchError::HandshakeTimeout,
        ),
    ] {
        assert_eq!(
            launch_child(role, timeout).unwrap_err(),
            expected,
            "role={role}"
        );
    }
}

#[test]
fn capture_helper_test_real_child_replay_is_rejected_after_handshake() {
    let mut helper = launch_child("replay", Duration::from_secs(10)).unwrap();
    let stream = helper.stream().try_clone().unwrap();
    let frame = crate::capture::protocol::FrameDecoder::default()
        .decode(&mut &stream)
        .unwrap();
    assert_eq!(
        helper.validator_mut().observe(&frame.header),
        Err(crate::capture::protocol::CaptureProtocolError::HelloReplay)
    );
}
