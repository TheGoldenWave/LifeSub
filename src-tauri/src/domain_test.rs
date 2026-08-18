use crate::domain::{CaptureSession, CaptureState, DomainError};

#[test]
fn capture_session_accepts_the_valid_lifecycle() {
    let session = CaptureSession::new("工作讨论");
    let session = session.transition(CaptureState::Recording).unwrap();
    let session = session.transition(CaptureState::Paused).unwrap();
    let session = session.transition(CaptureState::Recording).unwrap();
    let session = session.transition(CaptureState::Stopped).unwrap();

    assert_eq!(session.state, CaptureState::Stopped);
}

#[test]
fn stopped_capture_session_cannot_resume() {
    let session = CaptureSession::new("工作讨论")
        .transition(CaptureState::Recording)
        .unwrap()
        .transition(CaptureState::Stopped)
        .unwrap();

    assert_eq!(
        session.transition(CaptureState::Recording),
        Err(DomainError::InvalidCaptureTransition {
            from: CaptureState::Stopped,
            to: CaptureState::Recording,
        })
    );
}
