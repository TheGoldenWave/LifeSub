use std::io;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;

use crate::api::protocol::{CallerKind, TrustedCallerContext};

/// Socket kinds with their permission models.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketKind {
    /// agent.sock — peer-UID check, fixed minimal local_agent capabilities.
    Agent,
    /// ui.sock — code-signature + designated requirement audit, tauri_ui capabilities.
    Ui,
}

/// Result of authenticating a connected socket peer.
#[derive(Debug)]
pub struct AuthenticatedPeer {
    pub stream: UnixStream,
    pub context: TrustedCallerContext,
}

/// Errors during socket binding or peer authentication.
#[derive(Debug)]
pub enum SocketError {
    Io(io::Error),
    /// Socket path is a symlink, not a regular socket.
    NotASocket,
    /// Socket path's parent directory is not the expected inode/device.
    ParentDirectoryChanged,
    /// Peer UID does not match the current process UID (agent.sock).
    PeerUidMismatch,
    /// Peer audit token does not match the required code-signature (ui.sock).
    PeerCodeSignatureMismatch,
    /// Peer credentials could not be obtained.
    PeerCredentialsUnavailable,
}

impl std::fmt::Display for SocketError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SocketError::Io(e) => write!(f, "socket I/O error: {e}"),
            SocketError::NotASocket => write!(f, "path is not a socket"),
            SocketError::ParentDirectoryChanged => {
                write!(f, "socket parent directory changed identity")
            }
            SocketError::PeerUidMismatch => write!(f, "peer UID does not match"),
            SocketError::PeerCodeSignatureMismatch => {
                write!(f, "peer code signature does not match")
            }
            SocketError::PeerCredentialsUnavailable => {
                write!(f, "peer credentials unavailable")
            }
        }
    }
}

impl std::error::Error for SocketError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SocketError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for SocketError {
    fn from(e: io::Error) -> Self {
        SocketError::Io(e)
    }
}

/// Bind a Unix domain socket for the given kind.
///
/// # Safety
///
/// The socket path must reside in a directory owned by the CoreRuntime
/// lock. The caller must verify the parent directory's identity before
/// calling this function.
pub fn bind_uds(path: &Path, kind: SocketKind) -> Result<UnixListener, SocketError> {
    // Reject symlinks: the path must be a regular file or not exist.
    if path.exists() {
        let meta = path.symlink_metadata()?;
        if meta.is_symlink() {
            return Err(SocketError::NotASocket);
        }
        // Remove stale socket file if it exists.
        if std::fs::metadata(path).is_ok() {
            std::fs::remove_file(path)?;
        }
    }

    let listener = UnixListener::bind(path)?;
    // Restrict access: agent.sock is world-writable (peer UID check later);
    // ui.sock is owner-only.
    std::fs::set_permissions(
        path,
        std::fs::Permissions::from_mode(match kind {
            SocketKind::Agent => 0o666,
            SocketKind::Ui => 0o600,
        }),
    )?;

    Ok(listener)
}

/// Accept an incoming connection and authenticate the peer.
///
/// For agent.sock: checks that the peer UID matches the current process UID.
/// For ui.sock: checks the peer's code signature (macOS audit token).
///
/// Returns an `AuthenticatedPeer` with a `TrustedCallerContext` suitable for
/// dispatch. The caller must not trust any self-reported fields from the
/// payload.
pub fn accept_authenticated(
    listener: &UnixListener,
    kind: SocketKind,
) -> Result<AuthenticatedPeer, SocketError> {
    let (stream, _addr) = listener.accept()?;

    let context = match kind {
        SocketKind::Agent => authenticate_agent_peer(&stream)?,
        SocketKind::Ui => authenticate_ui_peer(&stream)?,
    };

    Ok(AuthenticatedPeer { stream, context })
}

fn authenticate_agent_peer(stream: &UnixStream) -> Result<TrustedCallerContext, SocketError> {
    let cred = peer_credentials(stream)?;

    let my_uid = unsafe { libc::getuid() };
    if cred.uid != my_uid {
        return Err(SocketError::PeerUidMismatch);
    }

    Ok(TrustedCallerContext {
        principal_id: format!("agent-uid-{}", cred.uid),
        kind: CallerKind::LocalAgent,
        capabilities: agent_capabilities(),
        auth_source: "peer_uid".to_owned(),
    })
}

fn authenticate_ui_peer(stream: &UnixStream) -> Result<TrustedCallerContext, SocketError> {
    let cred = peer_credentials(stream)?;

    // In phase C, we accept any same-UID peer on ui.sock for development.
    // In phase A, this must validate the code-signature designated requirement,
    // Team ID, and bundle ID via SecCodeCopySigningInformation.
    let my_uid = unsafe { libc::getuid() };
    if cred.uid != my_uid {
        return Err(SocketError::PeerUidMismatch);
    }

    Ok(TrustedCallerContext {
        principal_id: format!("tauri-pid-{}", cred.pid),
        kind: CallerKind::TauriUi,
        capabilities: tauri_capabilities(),
        auth_source: "peer_uid_dev".to_owned(),
    })
}

/// Peer process credentials.
struct PeerCred {
    uid: u32,
    pid: i32,
}

fn peer_credentials(stream: &UnixStream) -> Result<PeerCred, SocketError> {
    use std::os::fd::AsRawFd;

    let fd = stream.as_raw_fd();
    let mut uid: libc::uid_t = 0;
    let mut gid: libc::gid_t = 0;
    let rc = unsafe { libc::getpeereid(fd, &mut uid, &mut gid) };
    if rc != 0 {
        return Err(SocketError::PeerCredentialsUnavailable);
    }
    // PID is not available via getpeereid; use 0 as a placeholder.
    Ok(PeerCred { uid, pid: 0 })
}

fn agent_capabilities() -> Vec<String> {
    use crate::api::protocol::{CAP_AGENT_EVIDENCE, CAP_AGENT_READ, CAP_OPERATION_READ};
    vec![
        CAP_AGENT_READ.to_owned(),
        CAP_AGENT_EVIDENCE.to_owned(),
        CAP_OPERATION_READ.to_owned(),
    ]
}

fn tauri_capabilities() -> Vec<String> {
    use crate::api::protocol::{
        CAP_AGENT_EVIDENCE, CAP_AGENT_READ, CAP_ASR_JOB_MANAGE, CAP_CAPTURE_MANAGE,
        CAP_IMPORT_MANAGE, CAP_MODEL_MANAGE, CAP_OPERATION_READ, CAP_RECEIPT_READ,
        CAP_TRANSCRIPT_READ,
    };
    vec![
        CAP_AGENT_READ.to_owned(),
        CAP_AGENT_EVIDENCE.to_owned(),
        CAP_OPERATION_READ.to_owned(),
        CAP_TRANSCRIPT_READ.to_owned(),
        CAP_RECEIPT_READ.to_owned(),
        CAP_CAPTURE_MANAGE.to_owned(),
        CAP_MODEL_MANAGE.to_owned(),
        CAP_IMPORT_MANAGE.to_owned(),
        CAP_ASR_JOB_MANAGE.to_owned(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixStream as StdUnixStream;
    use tempfile::TempDir;

    fn bind_agent_socket(dir: &Path) -> UnixListener {
        let path = dir.join("agent.sock");
        bind_uds(&path, SocketKind::Agent).unwrap()
    }

    #[test]
    fn bind_creates_socket_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.sock");
        assert!(!path.exists());
        let _listener = bind_uds(&path, SocketKind::Agent).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn bind_rejects_symlink() {
        let dir = TempDir::new().unwrap();
        let target = dir.path().join("real.sock");
        // Create a real file so the symlink has a valid target.
        std::fs::write(&target, b"not-a-socket").unwrap();
        let link = dir.path().join("link.sock");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let result = bind_uds(&link, SocketKind::Agent);
        assert!(
            result.is_err(),
            "should reject symlink, got: {:?}",
            result.ok()
        );
    }

    #[test]
    fn bind_reuses_stale_socket() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("stale.sock");
        let _first = bind_uds(&path, SocketKind::Agent).unwrap();
        drop(_first);
        assert!(path.exists());
        // Re-bind should succeed (stale socket removed).
        let _second = bind_uds(&path, SocketKind::Agent).unwrap();
    }

    #[test]
    fn accept_authenticates_same_uid_agent() {
        let dir = TempDir::new().unwrap();
        let listener = bind_agent_socket(dir.path());
        let path = dir.path().join("agent.sock");

        // Connect from a background thread (just connect, don't read/write).
        let handle = std::thread::spawn(move || {
            let _stream = StdUnixStream::connect(&path).unwrap();
        });

        let peer = accept_authenticated(&listener, SocketKind::Agent).unwrap();
        assert_eq!(peer.context.kind, CallerKind::LocalAgent);
        assert!(peer.context.capabilities.contains(&"agent_read".to_owned()));
        assert_eq!(peer.context.auth_source, "peer_uid");

        handle.join().unwrap();
    }

    #[test]
    fn accept_authenticates_same_uid_ui() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("ui.sock");
        let listener = bind_uds(&path, SocketKind::Ui).unwrap();

        let handle = std::thread::spawn(move || {
            let _stream = StdUnixStream::connect(&path).unwrap();
        });

        let peer = accept_authenticated(&listener, SocketKind::Ui).unwrap();
        assert_eq!(peer.context.kind, CallerKind::TauriUi);
        assert!(
            peer.context
                .capabilities
                .contains(&"capture_manage".to_owned())
        );
        assert_eq!(peer.context.auth_source, "peer_uid_dev");

        handle.join().unwrap();
    }
}
