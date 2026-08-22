use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write};
use std::os::fd::FromRawFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use crate::capture::helper::{
    AuthenticatedHelper, HelperLaunchError, PrivateCaptureEndpoint, launch_test_helper,
};
use crate::capture::helper_auth::{
    ExecutableFileIdentity, HandshakeError, HandshakeVerifier, PeerIdentity,
};
use crate::capture::protocol::{CaptureHeader, Hello, Source};

const NONCE: [u8; 32] = [0x5a; 32];

fn hello(pid: u32) -> CaptureHeader {
    CaptureHeader::Hello(Hello {
        protocol_version: 1,
        helper_pid: pid,
        launch_nonce: hex::encode(NONCE),
        supported_sources: vec![Source::Microphone, Source::SystemAudio],
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
        protocol_version: 1,
        helper_pid: reported_pid,
        launch_nonce: hex::encode(nonce),
        supported_sources: vec![Source::Microphone, Source::SystemAudio],
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
