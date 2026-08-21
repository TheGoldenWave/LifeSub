use std::fmt;
use std::path::{Path, PathBuf};

use super::protocol::{CaptureHeader, PROTOCOL_VERSION};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeerIdentity {
    pub uid: u32,
    pub pid: u32,
    pub executable: PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandshakeError {
    Replay,
    UidMismatch,
    PidMismatch,
    ExecutableMismatch,
    ExpectedHello,
    VersionMismatch,
    NonceMismatch,
    InvalidNonce,
}

pub struct HandshakeVerifier {
    expected_nonce: [u8; 32],
    expected_pid: u32,
    expected_uid: u32,
    expected_executable: PathBuf,
    consumed: bool,
}

impl HandshakeVerifier {
    pub fn new(
        nonce: [u8; 32],
        expected_pid: u32,
        expected_uid: u32,
        expected_executable: impl AsRef<Path>,
    ) -> Self {
        Self {
            expected_nonce: nonce,
            expected_pid,
            expected_uid,
            expected_executable: canonical_or_original(expected_executable.as_ref()),
            consumed: false,
        }
    }

    pub fn verify(
        &mut self,
        peer: &PeerIdentity,
        header: &CaptureHeader,
    ) -> Result<(), HandshakeError> {
        if self.consumed {
            return Err(HandshakeError::Replay);
        }
        if peer.uid != self.expected_uid {
            return Err(HandshakeError::UidMismatch);
        }
        if peer.pid != self.expected_pid {
            return Err(HandshakeError::PidMismatch);
        }
        if canonical_or_original(&peer.executable) != self.expected_executable {
            return Err(HandshakeError::ExecutableMismatch);
        }
        let CaptureHeader::Hello(hello) = header else {
            return Err(HandshakeError::ExpectedHello);
        };
        if hello.protocol_version != PROTOCOL_VERSION {
            return Err(HandshakeError::VersionMismatch);
        }
        if hello.helper_pid != self.expected_pid {
            return Err(HandshakeError::PidMismatch);
        }
        let mut observed = [0_u8; 32];
        if hello.launch_nonce.len() != 64
            || hex::decode_to_slice(&hello.launch_nonce, &mut observed).is_err()
        {
            observed.fill(0);
            return Err(HandshakeError::InvalidNonce);
        }
        let matches = constant_time_eq(&self.expected_nonce, &observed);
        observed.fill(0);
        if !matches {
            return Err(HandshakeError::NonceMismatch);
        }
        self.consumed = true;
        Ok(())
    }
}

impl Drop for HandshakeVerifier {
    fn drop(&mut self) {
        self.expected_nonce.fill(0);
    }
}

impl fmt::Debug for HandshakeVerifier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HandshakeVerifier")
            .field("expected_nonce", &"[REDACTED]")
            .field("expected_pid", &self.expected_pid)
            .field("expected_uid", &self.expected_uid)
            .field("expected_executable", &self.expected_executable)
            .field("consumed", &self.consumed)
            .finish()
    }
}

fn canonical_or_original(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn constant_time_eq(expected: &[u8; 32], observed: &[u8; 32]) -> bool {
    expected
        .iter()
        .zip(observed)
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}
