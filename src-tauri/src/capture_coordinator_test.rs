use std::collections::{HashMap, VecDeque};

use crate::capture::coordinator::{
    NativeCaptureChunkCommitter, NativeCaptureCoordinator, NativeCaptureCoordinatorError,
    NativeCaptureDrainOutcome, NativeCapturePhase, NativeCaptureSourceRequest,
    NativeCaptureStableError, NativeCaptureTransport, NativeCaptureTransportCommand,
    NativeCaptureTransportError,
};
use crate::capture::protocol::{
    CaptureHeader, ChunkSealed, FatalError, MicrophoneDevicesMessage, Source, SourceInterrupted,
    SourceRecovered, SourceRecovering, SourceStopped,
};

#[derive(Default)]
struct FakeTransport {
    log: Vec<NativeCaptureTransportCommand>,
    preflight_failures: HashMap<Source, NativeCaptureTransportError>,
    start_failures: HashMap<Source, NativeCaptureTransportError>,
    pause_failures: HashMap<Source, NativeCaptureTransportError>,
    resume_failures: HashMap<Source, NativeCaptureTransportError>,
    stop_failures: HashMap<Source, NativeCaptureTransportError>,
    events: VecDeque<Result<CaptureHeader, NativeCaptureTransportError>>,
}

#[allow(dead_code)]
impl FakeTransport {
    fn preflight_ok(&mut self, source: Source) {
        self.preflight_failures.remove(&source);
    }

    fn start_ok(&mut self, source: Source) {
        self.start_failures.remove(&source);
    }

    fn pause_ok(&mut self, source: Source) {
        self.pause_failures.remove(&source);
    }

    fn resume_ok(&mut self, source: Source) {
        self.resume_failures.remove(&source);
    }

    fn stop_ok(&mut self, source: Source) {
        self.stop_failures.remove(&source);
    }

    fn fail_preflight(&mut self, source: Source, error: NativeCaptureTransportError) {
        self.preflight_failures.insert(source, error);
    }

    fn fail_start(&mut self, source: Source, error: NativeCaptureTransportError) {
        self.start_failures.insert(source, error);
    }

    fn fail_pause(&mut self, source: Source, error: NativeCaptureTransportError) {
        self.pause_failures.insert(source, error);
    }

    fn fail_resume(&mut self, source: Source, error: NativeCaptureTransportError) {
        self.resume_failures.insert(source, error);
    }

    fn fail_stop(&mut self, source: Source, error: NativeCaptureTransportError) {
        self.stop_failures.insert(source, error);
    }

    fn push_event(&mut self, event: Result<CaptureHeader, NativeCaptureTransportError>) {
        self.events.push_back(event);
    }
}

impl NativeCaptureTransport for FakeTransport {
    fn preflight_source(&mut self, source: Source) -> Result<(), NativeCaptureTransportError> {
        self.log
            .push(NativeCaptureTransportCommand::Preflight(source));
        self.preflight_failures.remove(&source).map_or(Ok(()), Err)
    }

    fn start_sources(
        &mut self,
        sources: &[Source],
        microphone_device_uid: Option<&str>,
    ) -> Result<(), NativeCaptureTransportError> {
        self.log.push(NativeCaptureTransportCommand::Start {
            sources: sources.to_vec(),
            microphone_device_uid: microphone_device_uid.map(ToOwned::to_owned),
        });
        sources
            .iter()
            .find_map(|source| self.start_failures.remove(source))
            .map_or(Ok(()), Err)
    }

    fn pause(&mut self) -> Result<(), NativeCaptureTransportError> {
        self.log.push(NativeCaptureTransportCommand::Pause);
        self.pause_failures
            .drain()
            .next()
            .map(|(_, error)| error)
            .map_or(Ok(()), Err)
    }

    fn resume(&mut self) -> Result<(), NativeCaptureTransportError> {
        self.log.push(NativeCaptureTransportCommand::Resume);
        self.resume_failures
            .drain()
            .next()
            .map(|(_, error)| error)
            .map_or(Ok(()), Err)
    }

    fn stop(&mut self) -> Result<(), NativeCaptureTransportError> {
        self.log.push(NativeCaptureTransportCommand::Stop);
        self.stop_failures
            .drain()
            .next()
            .map(|(_, error)| error)
            .map_or(Ok(()), Err)
    }

    fn shutdown(&mut self) -> Result<(), NativeCaptureTransportError> {
        Ok(())
    }

    fn next_event(&mut self) -> Result<Option<CaptureHeader>, NativeCaptureTransportError> {
        self.events.pop_front().transpose()
    }
}

#[derive(Default)]
struct FakeCommitter {
    sealed: Vec<String>,
    failures: VecDeque<NativeCaptureStableError>,
}

impl FakeCommitter {
    #[allow(dead_code)]
    fn fail_next(&mut self, error: NativeCaptureStableError) {
        self.failures.push_back(error);
    }
}

impl NativeCaptureChunkCommitter for FakeCommitter {
    fn commit(&mut self, sealed: &ChunkSealed) -> Result<(), NativeCaptureStableError> {
        self.sealed.push(sealed.relative_staging_path.clone());
        self.failures.pop_front().map_or(Ok(()), Err)
    }
}

fn requested(source: Source, enabled: bool) -> NativeCaptureSourceRequest {
    NativeCaptureSourceRequest { source, enabled }
}

fn running_coordinator(requests: Vec<NativeCaptureSourceRequest>) -> NativeCaptureCoordinator {
    NativeCaptureCoordinator::new(requests).expect("coordinator request plan")
}

#[test]
fn preflight_and_atomic_start_rollback_restore_started_sources_on_backpressure() {
    let mut transport = FakeTransport::default();
    transport.fail_start(
        Source::SystemAudio,
        NativeCaptureTransportError::Backpressure,
    );

    let mut coordinator = running_coordinator(vec![
        requested(Source::Microphone, true),
        requested(Source::SystemAudio, true),
    ]);

    coordinator.preflight(&mut transport).unwrap();
    let error = coordinator.start(&mut transport, None).unwrap_err();

    assert!(matches!(
        error,
        NativeCaptureCoordinatorError::Stable(NativeCaptureStableError::Backpressure)
    ));
    assert_eq!(
        coordinator.phase(),
        NativeCapturePhase::Failed(NativeCaptureStableError::Backpressure)
    );
    assert_eq!(
        transport.log,
        vec![
            NativeCaptureTransportCommand::Preflight(Source::Microphone),
            NativeCaptureTransportCommand::Preflight(Source::SystemAudio),
            NativeCaptureTransportCommand::Start {
                sources: vec![Source::Microphone, Source::SystemAudio],
                microphone_device_uid: None,
            },
        ]
    );
}

#[test]
fn pause_resume_and_stop_only_touch_enabled_sources() {
    let mut transport = FakeTransport::default();
    let mut coordinator = running_coordinator(vec![
        requested(Source::Microphone, true),
        requested(Source::SystemAudio, false),
    ]);

    coordinator.preflight(&mut transport).unwrap();
    coordinator.start(&mut transport, None).unwrap();
    coordinator.pause(&mut transport).unwrap();
    assert_eq!(coordinator.phase(), NativeCapturePhase::Paused);

    coordinator.resume(&mut transport).unwrap();
    assert_eq!(coordinator.phase(), NativeCapturePhase::Running);

    coordinator.stop(&mut transport).unwrap();
    assert_eq!(coordinator.phase(), NativeCapturePhase::Stopping);

    assert_eq!(
        transport.log,
        vec![
            NativeCaptureTransportCommand::Preflight(Source::Microphone),
            NativeCaptureTransportCommand::Start {
                sources: vec![Source::Microphone],
                microphone_device_uid: None,
            },
            NativeCaptureTransportCommand::Pause,
            NativeCaptureTransportCommand::Resume,
            NativeCaptureTransportCommand::Stop,
        ]
    );
}

#[test]
fn chunk_sealed_is_counted_only_after_commit_and_success_waits_for_enabled_sources_to_stop() {
    let mut transport = FakeTransport::default();
    let mut committer = FakeCommitter::default();
    let mut coordinator = running_coordinator(vec![requested(Source::Microphone, true)]);

    coordinator.preflight(&mut transport).unwrap();
    coordinator.start(&mut transport, None).unwrap();
    coordinator.stop(&mut transport).unwrap();

    transport.push_event(Ok(CaptureHeader::ChunkSealed(ChunkSealed {
        source: Source::Microphone,
        relative_staging_path: "audio/chunk.partial".to_string(),
        byte_length: 12,
        sha256: "a".repeat(64),
        duration_ms: 30,
        start_sample_position: 0,
        end_sample_position: 480,
        start_host_time: 10,
        end_host_time: 20,
    })));
    transport.push_event(Ok(CaptureHeader::SourceStopped(SourceStopped {
        source: Source::Microphone,
        final_sequence: 7,
        start_host_time: 10,
        end_host_time: 20,
    })));

    assert_eq!(
        coordinator
            .drain_once(&mut transport, &mut committer)
            .unwrap(),
        NativeCaptureDrainOutcome::Pending
    );
    assert_eq!(coordinator.committed_chunk_count(), 1);
    assert!(!coordinator.is_complete());

    assert_eq!(
        coordinator
            .drain_once(&mut transport, &mut committer)
            .unwrap(),
        NativeCaptureDrainOutcome::Pending
    );
    assert_eq!(coordinator.phase(), NativeCapturePhase::Stopping);
    assert!(!coordinator.is_complete());

    assert_eq!(
        coordinator
            .drain_once(&mut transport, &mut committer)
            .unwrap(),
        NativeCaptureDrainOutcome::Complete
    );
    assert_eq!(coordinator.phase(), NativeCapturePhase::Stopped);
    assert!(coordinator.is_complete());
    assert_eq!(committer.sealed, vec!["audio/chunk.partial".to_string()]);
}

#[test]
fn fatal_helper_exit_protocol_violation_and_device_interruption_become_stable_errors() {
    let cases = [
        (
            "helper exit",
            Err(NativeCaptureTransportError::HelperExited),
            NativeCaptureStableError::HelperExited,
        ),
        (
            "protocol violation",
            Err(NativeCaptureTransportError::ProtocolViolation),
            NativeCaptureStableError::ProtocolViolation,
        ),
        (
            "discovery response during capture",
            Ok(CaptureHeader::MicrophoneDevices(MicrophoneDevicesMessage {
                devices: Vec::new(),
            })),
            NativeCaptureStableError::ProtocolViolation,
        ),
        (
            "device interruption",
            Ok(CaptureHeader::SourceInterrupted(SourceInterrupted {
                source: Source::Microphone,
                reason: "device lost".to_string(),
                recoverable: false,
            })),
            NativeCaptureStableError::DeviceInterrupted {
                source: Source::Microphone,
                reason: "device lost".to_string(),
            },
        ),
        (
            "fatal error",
            Ok(CaptureHeader::FatalError(FatalError {
                code: "fatal".to_string(),
                summary: "boom".to_string(),
            })),
            NativeCaptureStableError::Fatal {
                code: "fatal".to_string(),
                summary: "boom".to_string(),
            },
        ),
    ];

    for (label, event, expected_error) in cases {
        let mut transport = FakeTransport::default();
        let mut committer = FakeCommitter::default();
        let mut coordinator = running_coordinator(vec![requested(Source::Microphone, true)]);
        coordinator.preflight(&mut transport).unwrap();
        coordinator.start(&mut transport, None).unwrap();
        transport.push_event(event);

        let error = coordinator
            .drain_once(&mut transport, &mut committer)
            .unwrap_err();

        assert!(
            matches!(error, NativeCaptureCoordinatorError::Stable(actual) if actual == expected_error),
            "{label}"
        );
        assert_eq!(
            coordinator.phase(),
            NativeCapturePhase::Failed(expected_error)
        );
        assert!(coordinator.pause(&mut transport).is_err());
    }
}

#[test]
fn same_uid_recovery_requires_matching_generation_and_device_before_running() {
    let mut transport = FakeTransport::default();
    let mut committer = FakeCommitter::default();
    let mut coordinator = running_coordinator(vec![requested(Source::Microphone, true)]);
    coordinator.preflight(&mut transport).unwrap();
    coordinator.start(&mut transport, Some("mic-1")).unwrap();
    transport.push_event(Ok(CaptureHeader::SourceRecovering(SourceRecovering {
        source: Source::Microphone,
        generation: 1,
        reason: "device_configuration_changed".to_string(),
    })));

    assert_eq!(
        coordinator
            .drain_once(&mut transport, &mut committer)
            .unwrap(),
        NativeCaptureDrainOutcome::Pending
    );
    assert_eq!(coordinator.phase(), NativeCapturePhase::Running);
    assert_eq!(
        coordinator.source_state(Source::Microphone),
        Some(crate::capture::coordinator::NativeCaptureSourceState::Recovering)
    );

    transport.push_event(Ok(CaptureHeader::SourceRecovered(SourceRecovered {
        source: Source::Microphone,
        generation: 1,
        device_id: "mic-1".to_string(),
        first_host_time: 42,
    })));
    coordinator
        .drain_once(&mut transport, &mut committer)
        .unwrap();
    assert_eq!(
        coordinator.source_state(Source::Microphone),
        Some(crate::capture::coordinator::NativeCaptureSourceState::Running)
    );
}

#[test]
fn microphone_recovery_fails_closed_for_legacy_bookkeeping_or_identity_mismatch() {
    for (event, begin_recovery) in [
        (
            CaptureHeader::SourceInterrupted(SourceInterrupted {
                source: Source::Microphone,
                reason: "device_configuration_changed".to_string(),
                recoverable: true,
            }),
            false,
        ),
        (
            CaptureHeader::SourceRecovered(SourceRecovered {
                source: Source::Microphone,
                generation: 1,
                device_id: "mic-1".to_string(),
                first_host_time: 42,
            }),
            false,
        ),
        (
            CaptureHeader::SourceRecovered(SourceRecovered {
                source: Source::Microphone,
                generation: 1,
                device_id: "mic-2".to_string(),
                first_host_time: 42,
            }),
            true,
        ),
    ] {
        let mut transport = FakeTransport::default();
        let mut committer = FakeCommitter::default();
        let mut coordinator = running_coordinator(vec![requested(Source::Microphone, true)]);
        coordinator.preflight(&mut transport).unwrap();
        coordinator.start(&mut transport, Some("mic-1")).unwrap();
        if begin_recovery {
            transport.push_event(Ok(CaptureHeader::SourceRecovering(SourceRecovering {
                source: Source::Microphone,
                generation: 1,
                reason: "device_configuration_changed".to_string(),
            })));
            coordinator
                .drain_once(&mut transport, &mut committer)
                .unwrap();
        }
        transport.push_event(Ok(event));
        assert!(
            coordinator
                .drain_once(&mut transport, &mut committer)
                .is_err()
        );
        assert!(matches!(coordinator.phase(), NativeCapturePhase::Failed(_)));
    }
}
