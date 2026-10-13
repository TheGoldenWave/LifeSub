use std::collections::{HashMap, HashSet};

use crate::capture::protocol::{CaptureHeader, ChunkSealed, FatalError, Source};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct NativeCaptureSourceRequest {
    pub source: Source,
    pub enabled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub(crate) enum NativeCaptureTransportCommand {
    Preflight(Source),
    Start {
        sources: Vec<Source>,
        microphone_device_uid: Option<String>,
    },
    Pause,
    Resume,
    Stop,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum NativeCaptureStableError {
    Fatal { code: String, summary: String },
    HelperExited,
    ProtocolViolation,
    Backpressure,
    CommitFailed,
    DeviceInterrupted { source: Source, reason: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub(crate) enum NativeCaptureTransportError {
    UnsupportedSource(Source),
    HelperExited,
    ProtocolViolation,
    Backpressure,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum NativeCapturePhase {
    Requested,
    Preflighted,
    Running,
    Paused,
    Stopping,
    Stopped,
    Failed(NativeCaptureStableError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeCaptureSourceState {
    Requested,
    Preflighted,
    Running,
    Recovering,
    Paused,
    Stopping,
    Stopped,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeCaptureDrainOutcome {
    Pending,
    Complete,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum NativeCaptureCoordinatorError {
    DuplicateSource(Source),
    NoEnabledSources,
    InvalidState(&'static str),
    PreflightRejected(Source),
    Stable(NativeCaptureStableError),
}

pub(crate) trait NativeCaptureTransport {
    fn preflight_source(&mut self, source: Source) -> Result<(), NativeCaptureTransportError>;
    fn start_sources(
        &mut self,
        sources: &[Source],
        microphone_device_uid: Option<&str>,
    ) -> Result<(), NativeCaptureTransportError>;
    fn pause(&mut self) -> Result<(), NativeCaptureTransportError>;
    fn resume(&mut self) -> Result<(), NativeCaptureTransportError>;
    fn stop(&mut self) -> Result<(), NativeCaptureTransportError>;
    fn shutdown(&mut self) -> Result<(), NativeCaptureTransportError>;
    fn next_event(&mut self) -> Result<Option<CaptureHeader>, NativeCaptureTransportError>;
}

pub(crate) trait NativeCaptureChunkCommitter {
    fn commit(&mut self, sealed: &ChunkSealed) -> Result<(), NativeCaptureStableError>;
}

pub(crate) struct NativeCaptureCoordinator {
    requested_sources: Vec<NativeCaptureSourceRequest>,
    phase: NativeCapturePhase,
    source_states: HashMap<Source, NativeCaptureSourceState>,
    started_sources: HashSet<Source>,
    stopped_enabled_sources: HashSet<Source>,
    committed_chunks: usize,
    selected_microphone_uid: Option<String>,
    last_recovery_generation: HashMap<Source, u64>,
}

impl NativeCaptureCoordinator {
    pub(crate) fn new(
        requested_sources: Vec<NativeCaptureSourceRequest>,
    ) -> Result<Self, NativeCaptureCoordinatorError> {
        if requested_sources.is_empty() {
            return Err(NativeCaptureCoordinatorError::NoEnabledSources);
        }
        let mut seen = HashSet::new();
        let mut enabled = false;
        let mut source_states = HashMap::new();
        for request in &requested_sources {
            if !seen.insert(request.source) {
                return Err(NativeCaptureCoordinatorError::DuplicateSource(
                    request.source,
                ));
            }
            enabled |= request.enabled;
            source_states.insert(request.source, NativeCaptureSourceState::Requested);
        }
        if !enabled {
            return Err(NativeCaptureCoordinatorError::NoEnabledSources);
        }
        Ok(Self {
            requested_sources,
            phase: NativeCapturePhase::Requested,
            source_states,
            started_sources: HashSet::new(),
            stopped_enabled_sources: HashSet::new(),
            committed_chunks: 0,
            selected_microphone_uid: None,
            last_recovery_generation: HashMap::new(),
        })
    }

    #[allow(dead_code)]
    pub(crate) fn phase(&self) -> NativeCapturePhase {
        self.phase.clone()
    }

    #[allow(dead_code)]
    pub(crate) fn source_state(&self, source: Source) -> Option<NativeCaptureSourceState> {
        self.source_states.get(&source).copied()
    }

    #[allow(dead_code)]
    pub(crate) fn committed_chunk_count(&self) -> usize {
        self.committed_chunks
    }

    #[allow(dead_code)]
    pub(crate) fn is_complete(&self) -> bool {
        self.phase == NativeCapturePhase::Stopped
    }

    pub(crate) fn preflight<T: NativeCaptureTransport>(
        &mut self,
        transport: &mut T,
    ) -> Result<(), NativeCaptureCoordinatorError> {
        self.ensure_not_failed()?;
        if !matches!(self.phase, NativeCapturePhase::Requested) {
            return Err(NativeCaptureCoordinatorError::InvalidState("preflight"));
        }
        for source in self.enabled_sources_vec() {
            match transport.preflight_source(source) {
                Ok(()) => {
                    self.source_states
                        .insert(source, NativeCaptureSourceState::Preflighted);
                }
                Err(NativeCaptureTransportError::UnsupportedSource(source)) => {
                    return Err(NativeCaptureCoordinatorError::PreflightRejected(source));
                }
                Err(error) => return self.enter_failed(self.transport_error_to_stable(error)),
            }
        }
        self.phase = NativeCapturePhase::Preflighted;
        Ok(())
    }

    pub(crate) fn start<T: NativeCaptureTransport>(
        &mut self,
        transport: &mut T,
        microphone_device_uid: Option<&str>,
    ) -> Result<(), NativeCaptureCoordinatorError> {
        self.ensure_not_failed()?;
        if !matches!(self.phase, NativeCapturePhase::Preflighted) {
            return Err(NativeCaptureCoordinatorError::InvalidState("start"));
        }
        let enabled = self.enabled_sources_vec();
        if let Err(error) = transport.start_sources(&enabled, microphone_device_uid) {
            return self.enter_failed(self.transport_error_to_stable(error));
        }
        self.selected_microphone_uid = if enabled.contains(&Source::Microphone) {
            microphone_device_uid.map(str::to_owned)
        } else {
            None
        };
        self.last_recovery_generation.clear();
        for source in enabled {
            self.started_sources.insert(source);
            self.source_states
                .insert(source, NativeCaptureSourceState::Running);
        }
        self.phase = NativeCapturePhase::Running;
        Ok(())
    }

    pub(crate) fn pause<T: NativeCaptureTransport>(
        &mut self,
        transport: &mut T,
    ) -> Result<(), NativeCaptureCoordinatorError> {
        self.ensure_not_failed()?;
        if !matches!(self.phase, NativeCapturePhase::Running) {
            return Err(NativeCaptureCoordinatorError::InvalidState("pause"));
        }
        if self
            .source_states
            .values()
            .any(|state| *state == NativeCaptureSourceState::Recovering)
        {
            return Err(NativeCaptureCoordinatorError::InvalidState(
                "pause_recovering",
            ));
        }
        if let Err(error) = transport.pause() {
            return self.enter_failed(self.transport_error_to_stable(error));
        }
        for source in self.enabled_started_sources() {
            self.source_states
                .insert(source, NativeCaptureSourceState::Paused);
        }
        self.phase = NativeCapturePhase::Paused;
        Ok(())
    }

    pub(crate) fn resume<T: NativeCaptureTransport>(
        &mut self,
        transport: &mut T,
    ) -> Result<(), NativeCaptureCoordinatorError> {
        self.ensure_not_failed()?;
        if !matches!(self.phase, NativeCapturePhase::Paused) {
            return Err(NativeCaptureCoordinatorError::InvalidState("resume"));
        }
        if let Err(error) = transport.resume() {
            return self.enter_failed(self.transport_error_to_stable(error));
        }
        for source in self.enabled_started_sources() {
            self.source_states
                .insert(source, NativeCaptureSourceState::Running);
        }
        self.phase = NativeCapturePhase::Running;
        Ok(())
    }

    pub(crate) fn stop<T: NativeCaptureTransport>(
        &mut self,
        transport: &mut T,
    ) -> Result<(), NativeCaptureCoordinatorError> {
        self.ensure_not_failed()?;
        if !matches!(
            self.phase,
            NativeCapturePhase::Running | NativeCapturePhase::Paused
        ) {
            return Err(NativeCaptureCoordinatorError::InvalidState("stop"));
        }
        if let Err(error) = transport.stop() {
            return self.enter_failed(self.transport_error_to_stable(error));
        }
        for source in self.enabled_started_sources() {
            self.source_states
                .insert(source, NativeCaptureSourceState::Stopping);
        }
        self.phase = NativeCapturePhase::Stopping;
        Ok(())
    }

    #[allow(dead_code)]
    pub(crate) fn drain_once<T: NativeCaptureTransport, C: NativeCaptureChunkCommitter>(
        &mut self,
        transport: &mut T,
        committer: &mut C,
    ) -> Result<NativeCaptureDrainOutcome, NativeCaptureCoordinatorError> {
        self.ensure_not_failed()?;
        if !matches!(
            self.phase,
            NativeCapturePhase::Running | NativeCapturePhase::Paused | NativeCapturePhase::Stopping
        ) {
            return Err(NativeCaptureCoordinatorError::InvalidState("drain"));
        }

        let event = match transport.next_event() {
            Err(NativeCaptureTransportError::UnsupportedSource(source)) => {
                return Err(NativeCaptureCoordinatorError::PreflightRejected(source));
            }
            Err(error) => return self.enter_failed(self.transport_error_to_stable(error)),
            Ok(event) => event,
        };
        self.handle_event(event, committer)
    }

    pub(crate) fn handle_event<C: NativeCaptureChunkCommitter>(
        &mut self,
        event: Option<CaptureHeader>,
        committer: &mut C,
    ) -> Result<NativeCaptureDrainOutcome, NativeCaptureCoordinatorError> {
        self.ensure_not_failed()?;
        if !matches!(
            self.phase,
            NativeCapturePhase::Running | NativeCapturePhase::Paused | NativeCapturePhase::Stopping
        ) {
            return Err(NativeCaptureCoordinatorError::InvalidState("drain"));
        }

        match event {
            None => self.finish_if_stopped(),
            Some(CaptureHeader::ChunkSealed(sealed)) => {
                committer
                    .commit(&sealed)
                    .map_err(NativeCaptureCoordinatorError::Stable)?;
                self.committed_chunks += 1;
                self.finish_if_stopped()
            }
            Some(CaptureHeader::SourceStopped(stopped)) => {
                if self
                    .enabled_sources()
                    .any(|source| source == stopped.source)
                {
                    self.stopped_enabled_sources.insert(stopped.source);
                    self.source_states
                        .insert(stopped.source, NativeCaptureSourceState::Stopped);
                }
                Ok(NativeCaptureDrainOutcome::Pending)
            }
            Some(CaptureHeader::SourceInterrupted(interrupted)) => {
                if interrupted.recoverable {
                    if interrupted.source == Source::Microphone {
                        self.enter_failed(NativeCaptureStableError::ProtocolViolation)
                    } else {
                        Ok(NativeCaptureDrainOutcome::Pending)
                    }
                } else {
                    self.enter_failed(NativeCaptureStableError::DeviceInterrupted {
                        source: interrupted.source,
                        reason: interrupted.reason,
                    })
                }
            }
            Some(CaptureHeader::SourceRecovering(recovering)) => {
                let is_running = self.source_states.get(&recovering.source)
                    == Some(&NativeCaptureSourceState::Running);
                let generation_is_new = self
                    .last_recovery_generation
                    .get(&recovering.source)
                    .is_none_or(|generation| recovering.generation > *generation);
                if !is_running || !generation_is_new {
                    return self.enter_failed(NativeCaptureStableError::ProtocolViolation);
                }
                self.last_recovery_generation
                    .insert(recovering.source, recovering.generation);
                self.source_states
                    .insert(recovering.source, NativeCaptureSourceState::Recovering);
                Ok(NativeCaptureDrainOutcome::Pending)
            }
            Some(CaptureHeader::SourceRecovered(recovered)) => {
                let is_recovering = self.source_states.get(&recovered.source)
                    == Some(&NativeCaptureSourceState::Recovering);
                let generation_matches = self.last_recovery_generation.get(&recovered.source)
                    == Some(&recovered.generation);
                let identity_matches = recovered.source != Source::Microphone
                    || self.selected_microphone_uid.as_deref()
                        == Some(recovered.device_id.as_str());
                if !is_recovering || !generation_matches || !identity_matches {
                    return self.enter_failed(NativeCaptureStableError::ProtocolViolation);
                }
                self.source_states
                    .insert(recovered.source, NativeCaptureSourceState::Running);
                Ok(NativeCaptureDrainOutcome::Pending)
            }
            Some(CaptureHeader::FatalError(FatalError { code, summary })) => {
                self.enter_failed(NativeCaptureStableError::Fatal { code, summary })
            }
            Some(CaptureHeader::ShutdownAck) => self.finish_if_stopped(),
            Some(CaptureHeader::SourceStarted(started)) => {
                if self
                    .requested_sources
                    .iter()
                    .any(|request| request.source == started.source)
                {
                    self.started_sources.insert(started.source);
                    self.source_states
                        .insert(started.source, NativeCaptureSourceState::Running);
                    return Ok(NativeCaptureDrainOutcome::Pending);
                }
                self.enter_failed(NativeCaptureStableError::ProtocolViolation)
            }
            Some(CaptureHeader::MicrophoneDevices(_))
            | Some(CaptureHeader::MicrophoneProbeResult(_)) => {
                self.enter_failed(NativeCaptureStableError::ProtocolViolation)
            }
            Some(CaptureHeader::Hello(_))
            | Some(CaptureHeader::PermissionState(_))
            | Some(CaptureHeader::AudioFrame(_))
            | Some(CaptureHeader::Level(_)) => Ok(NativeCaptureDrainOutcome::Pending),
        }
    }

    fn enabled_sources(&self) -> impl Iterator<Item = Source> + '_ {
        self.requested_sources
            .iter()
            .filter(|request| request.enabled)
            .map(|request| request.source)
    }

    fn enabled_sources_vec(&self) -> Vec<Source> {
        self.enabled_sources().collect()
    }

    fn enabled_started_sources(&self) -> Vec<Source> {
        self.enabled_sources()
            .filter(|source| self.started_sources.contains(source))
            .collect()
    }

    fn finish_if_stopped(
        &mut self,
    ) -> Result<NativeCaptureDrainOutcome, NativeCaptureCoordinatorError> {
        if self.phase == NativeCapturePhase::Stopping && self.all_enabled_sources_stopped() {
            self.phase = NativeCapturePhase::Stopped;
            Ok(NativeCaptureDrainOutcome::Complete)
        } else {
            Ok(NativeCaptureDrainOutcome::Pending)
        }
    }

    fn all_enabled_sources_stopped(&self) -> bool {
        self.enabled_sources()
            .all(|source| self.stopped_enabled_sources.contains(&source))
    }

    fn ensure_not_failed(&self) -> Result<(), NativeCaptureCoordinatorError> {
        if let NativeCapturePhase::Failed(error) = &self.phase {
            Err(NativeCaptureCoordinatorError::Stable(error.clone()))
        } else {
            Ok(())
        }
    }

    fn transport_error_to_stable(
        &self,
        error: NativeCaptureTransportError,
    ) -> NativeCaptureStableError {
        match error {
            NativeCaptureTransportError::UnsupportedSource(source) => {
                NativeCaptureStableError::DeviceInterrupted {
                    source,
                    reason: "unsupported source".to_string(),
                }
            }
            NativeCaptureTransportError::HelperExited => NativeCaptureStableError::HelperExited,
            NativeCaptureTransportError::ProtocolViolation => {
                NativeCaptureStableError::ProtocolViolation
            }
            NativeCaptureTransportError::Backpressure => NativeCaptureStableError::Backpressure,
        }
    }

    fn enter_failed<T>(
        &mut self,
        error: NativeCaptureStableError,
    ) -> Result<T, NativeCaptureCoordinatorError> {
        self.phase = NativeCapturePhase::Failed(error.clone());
        Err(NativeCaptureCoordinatorError::Stable(error))
    }
}
