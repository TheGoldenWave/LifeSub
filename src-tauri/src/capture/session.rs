use std::collections::{HashMap, HashSet};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use chrono::Utc;
use tauri::{AppHandle, Emitter};

use super::committer::CatalogCaptureCommitter;
use super::coordinator::{
    NativeCaptureCoordinator, NativeCaptureCoordinatorError, NativeCaptureDrainOutcome,
    NativeCaptureSourceRequest, NativeCaptureStableError, NativeCaptureTransport,
};
use super::helper::{HelperLaunchError, launch_packaged_helper};
use super::native_transport::AuthenticatedCaptureTransport;
use super::protocol::{CaptureHeader, ChunkSealed, Source, SourceStarted};
use crate::capture::streaming::{LIVE_ERROR_EVENT, LiveCaptureError};
use crate::domain::{AudioSource, CaptureSourceRecord};
use crate::service::CoreRuntime;

const HELPER_LAUNCH_TIMEOUT: Duration = Duration::from_secs(5);
const EVENT_POLL_INTERVAL: Duration = Duration::from_millis(20);
const STAGING_DIRECTORY: &str = "capture-staging";
const STARTED_STATE: &str = "started";
const MICROPHONE_SELECTION_REQUIRED_CODE: &str = "microphone_selection_required";
const INVALID_SYSTEM_ONLY_SELECTION_CODE: &str = "invalid_capture_mode_microphone_selection";
const SELECTED_MICROPHONE_IDENTITY_MISMATCH_CODE: &str = "selected_microphone_identity_mismatch";
const SELECTED_MICROPHONE_DISCONNECTED_REASON: &str = "selected_microphone_disconnected";

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeCaptureStartStatus {
    pub state: String,
    pub session_id: String,
    pub enabled_sources: Vec<Source>,
    pub sources: Vec<NativeCaptureSourceStatus>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeCaptureSourceStatus {
    pub source: Source,
    pub permission: String,
    pub device_id: String,
    pub device_name: String,
    pub ready: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NativeCaptureMode {
    Smart,
    MicOnly,
    SystemOnly,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeCaptureSessionError {
    AlreadyRunning,
    NotRunning,
    MissingSession,
    LaunchFailed(String),
    InvalidState(&'static str),
    ProtocolViolation,
    Catalog(String),
    Stable(NativeCaptureStableError),
    WorkerExited,
}

impl std::fmt::Display for NativeCaptureSessionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyRunning => formatter.write_str("streaming capture already running"),
            Self::NotRunning => formatter.write_str("streaming capture not running"),
            Self::MissingSession => formatter.write_str("capture session does not exist"),
            Self::LaunchFailed(summary) => {
                write!(formatter, "capture helper launch failed: {summary}")
            }
            Self::InvalidState(state) => write!(formatter, "native capture invalid state: {state}"),
            Self::ProtocolViolation => formatter.write_str("native capture protocol violation"),
            Self::Catalog(summary) => write!(formatter, "native capture catalog error: {summary}"),
            Self::Stable(error) => write!(formatter, "{error:?}"),
            Self::WorkerExited => formatter.write_str("native capture worker exited"),
        }
    }
}

impl std::error::Error for NativeCaptureSessionError {}

impl From<NativeCaptureCoordinatorError> for NativeCaptureSessionError {
    fn from(value: NativeCaptureCoordinatorError) -> Self {
        match value {
            NativeCaptureCoordinatorError::InvalidState(state) => Self::InvalidState(state),
            NativeCaptureCoordinatorError::Stable(error) => Self::Stable(error),
            NativeCaptureCoordinatorError::DuplicateSource(_)
            | NativeCaptureCoordinatorError::NoEnabledSources
            | NativeCaptureCoordinatorError::PreflightRejected(_) => Self::ProtocolViolation,
        }
    }
}

impl From<HelperLaunchError> for NativeCaptureSessionError {
    fn from(value: HelperLaunchError) -> Self {
        Self::LaunchFailed(format!("{value:?}"))
    }
}

type ControlResult = Result<(), NativeCaptureSessionError>;

enum ControlMessage {
    Pause(SyncSender<ControlResult>),
    Resume(SyncSender<ControlResult>),
    Stop(SyncSender<ControlResult>),
}

struct ActiveNativeCaptureSession {
    control: SyncSender<ControlMessage>,
    handle: JoinHandle<()>,
    last_error: Arc<Mutex<Option<NativeCaptureSessionError>>>,
}

#[derive(Default)]
pub struct NativeCaptureSession {
    active: Option<ActiveNativeCaptureSession>,
}

struct NativeCaptureStartRequest {
    session_id: String,
    mode: NativeCaptureMode,
    microphone_device_uid: Option<String>,
}

impl Drop for NativeCaptureSession {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

impl NativeCaptureSession {
    pub fn new() -> Self {
        Self { active: None }
    }

    pub fn is_running(&self) -> bool {
        self.active
            .as_ref()
            .is_some_and(|active| !active.handle.is_finished())
    }

    pub fn start(
        &mut self,
        app: AppHandle,
        runtime: Arc<CoreRuntime>,
        data_dir: PathBuf,
        session_id: String,
        mode: NativeCaptureMode,
        microphone_device_uid: Option<String>,
    ) -> Result<NativeCaptureStartStatus, NativeCaptureSessionError> {
        self.start_with_transport_factory(
            Some(app),
            runtime,
            data_dir,
            NativeCaptureStartRequest {
                session_id,
                mode,
                microphone_device_uid,
            },
            |runtime_parent, staging_root| {
                let helper = launch_packaged_helper(runtime_parent, HELPER_LAUNCH_TIMEOUT)?;
                Ok(AuthenticatedCaptureTransport::new(
                    helper,
                    staging_root,
                    None,
                ))
            },
        )
    }

    #[cfg(test)]
    pub(crate) fn start_with_transport_factory_for_test<T, F>(
        &mut self,
        runtime: Arc<CoreRuntime>,
        data_dir: PathBuf,
        session_id: String,
        mode: NativeCaptureMode,
        microphone_device_uid: Option<String>,
        factory: F,
    ) -> Result<NativeCaptureStartStatus, NativeCaptureSessionError>
    where
        T: NativeCaptureTransport + Send + 'static,
        F: FnOnce(&Path, &Path) -> Result<T, NativeCaptureSessionError> + Send + 'static,
    {
        self.start_with_transport_factory(
            None,
            runtime,
            data_dir,
            NativeCaptureStartRequest {
                session_id,
                mode,
                microphone_device_uid,
            },
            factory,
        )
    }

    fn start_with_transport_factory<T, F>(
        &mut self,
        app: Option<AppHandle>,
        runtime: Arc<CoreRuntime>,
        data_dir: PathBuf,
        request: NativeCaptureStartRequest,
        factory: F,
    ) -> Result<NativeCaptureStartStatus, NativeCaptureSessionError>
    where
        T: NativeCaptureTransport + Send + 'static,
        F: FnOnce(&Path, &Path) -> Result<T, NativeCaptureSessionError> + Send + 'static,
    {
        let NativeCaptureStartRequest {
            session_id,
            mode,
            microphone_device_uid,
        } = request;
        if self.is_running() {
            return Err(NativeCaptureSessionError::AlreadyRunning);
        }
        if !runtime
            .catalog_ref()
            .session_exists(&session_id)
            .map_err(|error| NativeCaptureSessionError::Catalog(error.to_string()))?
        {
            return Err(NativeCaptureSessionError::MissingSession);
        }
        let microphone_device_uid = validate_microphone_selection(mode, microphone_device_uid)?;

        let staging_root = staging_root_for(&data_dir, &session_id)?;
        let requests = requests_for_mode(mode);
        let enabled_sources = requests
            .iter()
            .filter(|request| request.enabled)
            .map(|request| request.source)
            .collect::<Vec<_>>();
        let (startup_tx, startup_rx) = mpsc::sync_channel(1);
        let (control_tx, control_rx) = mpsc::sync_channel(8);
        let last_error = Arc::new(Mutex::new(None));
        let worker_error = last_error.clone();
        let worker_app = app.clone();
        let worker_runtime = runtime.clone();
        let worker_session_id = session_id.clone();
        let handle = thread::spawn(move || {
            let error_app = worker_app.clone();
            let result = run_worker(
                worker_app,
                worker_runtime,
                data_dir,
                worker_session_id.clone(),
                staging_root,
                requests,
                microphone_device_uid,
                factory,
                startup_tx,
                control_rx,
            );
            if let Err(error) = result {
                *worker_error.lock().unwrap() = Some(error.clone());
                if let Some(app) = error_app.as_ref() {
                    emit_stream_error(app, &error);
                }
            }
        });
        match startup_rx.recv() {
            Ok(Ok(started_sources)) => {
                self.active = Some(ActiveNativeCaptureSession {
                    control: control_tx,
                    handle,
                    last_error,
                });
                Ok(NativeCaptureStartStatus {
                    state: STARTED_STATE.to_string(),
                    session_id,
                    enabled_sources,
                    sources: started_sources
                        .into_iter()
                        .map(|source| NativeCaptureSourceStatus {
                            source: source.source,
                            permission: "granted".to_string(),
                            device_id: source.device_id,
                            device_name: source.device_name,
                            ready: true,
                        })
                        .collect(),
                })
            }
            Ok(Err(error)) => {
                let _ = handle.join();
                Err(error)
            }
            Err(_) => {
                let _ = handle.join();
                Err(NativeCaptureSessionError::WorkerExited)
            }
        }
    }

    pub fn pause(&self) -> Result<(), NativeCaptureSessionError> {
        self.send_control(ControlMessage::Pause)
    }

    pub fn recent_error(&self) -> Option<LiveCaptureError> {
        self.active
            .as_ref()
            .and_then(|active| active.last_error.lock().unwrap().clone())
            .map(|error| live_capture_error(&error))
    }

    pub fn resume(&self) -> Result<(), NativeCaptureSessionError> {
        self.send_control(ControlMessage::Resume)
    }

    pub fn stop(&mut self) -> Result<(), NativeCaptureSessionError> {
        let Some(active) = self.active.take() else {
            return Err(NativeCaptureSessionError::NotRunning);
        };
        send_control_message(&active.control, ControlMessage::Stop)?;
        let _ = active.handle.join();
        if let Some(error) = active.last_error.lock().unwrap().clone()
            && matches!(error, NativeCaptureSessionError::Stable(_))
        {
            return Err(error);
        }
        Ok(())
    }

    fn send_control(
        &self,
        builder: impl FnOnce(SyncSender<ControlResult>) -> ControlMessage,
    ) -> Result<(), NativeCaptureSessionError> {
        let Some(active) = self.active.as_ref() else {
            return Err(NativeCaptureSessionError::NotRunning);
        };
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        active.control.send(builder(reply_tx)).map_err(|_| {
            active
                .last_error
                .lock()
                .unwrap()
                .clone()
                .unwrap_or(NativeCaptureSessionError::WorkerExited)
        })?;
        reply_rx.recv().map_err(|_| {
            active
                .last_error
                .lock()
                .unwrap()
                .clone()
                .unwrap_or(NativeCaptureSessionError::WorkerExited)
        })?
    }
}

#[allow(clippy::too_many_arguments)]
fn run_worker<T, F>(
    _app: Option<AppHandle>,
    runtime: Arc<CoreRuntime>,
    data_dir: PathBuf,
    session_id: String,
    staging_root: PathBuf,
    requests: Vec<NativeCaptureSourceRequest>,
    microphone_device_uid: Option<String>,
    factory: F,
    startup_tx: SyncSender<Result<Vec<SourceStarted>, NativeCaptureSessionError>>,
    control_rx: Receiver<ControlMessage>,
) -> Result<(), NativeCaptureSessionError>
where
    T: NativeCaptureTransport + Send + 'static,
    F: FnOnce(&Path, &Path) -> Result<T, NativeCaptureSessionError>,
{
    let runtime_parent = data_dir.as_path();
    let mut transport = factory(runtime_parent, &staging_root)?;
    let mut coordinator = NativeCaptureCoordinator::new(requests.clone())?;
    coordinator.preflight(&mut transport)?;
    coordinator.start(&mut transport, microphone_device_uid.as_deref())?;

    let enabled_sources: HashSet<Source> = requests
        .iter()
        .filter(|request| request.enabled)
        .map(|request| request.source)
        .collect();
    let mut started_sources = HashMap::<Source, SourceStarted>::new();
    let mut committer = None;
    let mut pending_committer = PendingCommitter;
    let mut startup_sent = false;
    let mut stop_requested = false;
    let mut shutdown_sent = false;
    let mut shutdown_ack = false;

    loop {
        while let Ok(control) = control_rx.try_recv() {
            match control {
                ControlMessage::Pause(reply) => {
                    let result = coordinator.pause(&mut transport).map_err(Into::into);
                    let _ = reply.send(result);
                }
                ControlMessage::Resume(reply) => {
                    let result = coordinator.resume(&mut transport).map_err(Into::into);
                    let _ = reply.send(result);
                }
                ControlMessage::Stop(reply) => {
                    let result = coordinator.stop(&mut transport).map_err(Into::into);
                    if result.is_ok() {
                        stop_requested = true;
                    }
                    let _ = reply.send(result);
                }
            }
        }

        let event = match transport.next_event() {
            Ok(event) => event,
            Err(error) => {
                if !startup_sent {
                    let _ = startup_tx.send(Err(NativeCaptureSessionError::Stable(
                        map_transport_error(error.clone()),
                    )));
                }
                return Err(NativeCaptureSessionError::Stable(map_transport_error(
                    error,
                )));
            }
        };
        if let Some(CaptureHeader::SourceStarted(started)) = &event {
            if !enabled_sources.contains(&started.source) {
                return Err(NativeCaptureSessionError::ProtocolViolation);
            }
            if started.source == Source::Microphone
                && let Some(requested_uid) = microphone_device_uid.as_deref()
                && started.device_id != requested_uid
            {
                let error = stable_fatal(
                    SELECTED_MICROPHONE_IDENTITY_MISMATCH_CODE,
                    "所选麦克风与实际启动设备不一致。",
                );
                if !startup_sent {
                    let _ = startup_tx.send(Err(error.clone()));
                }
                return Err(error);
            }
            started_sources.insert(started.source, started.clone());
            if committer.is_none() && started_sources.len() == enabled_sources.len() {
                let source_ids =
                    register_capture_sources(runtime.catalog_ref(), &session_id, &started_sources)?;
                let session_anchor_ns = started_sources
                    .values()
                    .map(|source| source.host_time)
                    .min()
                    .ok_or(NativeCaptureSessionError::ProtocolViolation)?;
                committer = Some(
                    CatalogCaptureCommitter::new(
                        runtime.catalog_ref(),
                        session_id.clone(),
                        &staging_root,
                        &data_dir,
                        source_ids,
                        session_anchor_ns,
                        |chunk| {
                            crate::commands::AppState::enqueue_auto_transcription(
                                &runtime,
                                &session_id,
                                chunk,
                            )
                            .map(|_| ())
                        },
                    )
                    .map_err(|error| NativeCaptureSessionError::Catalog(error.to_string()))?,
                );
                if !startup_sent {
                    let _ = startup_tx.send(Ok(started_sources.values().cloned().collect()));
                    startup_sent = true;
                }
            }
        }

        if matches!(event, Some(CaptureHeader::ChunkSealed(_))) && committer.is_none() {
            if !startup_sent {
                let _ = startup_tx.send(Err(NativeCaptureSessionError::ProtocolViolation));
            }
            return Err(NativeCaptureSessionError::ProtocolViolation);
        }

        let saw_shutdown_ack = matches!(event, Some(CaptureHeader::ShutdownAck));
        let timed_out = event.is_none();
        if saw_shutdown_ack {
            shutdown_ack = true;
        }

        let outcome = match committer.as_mut() {
            Some(committer) => coordinator.handle_event(event, committer),
            None => coordinator.handle_event(event, &mut pending_committer),
        };
        let outcome = match outcome {
            Ok(outcome) => outcome,
            Err(error) => {
                let error = NativeCaptureSessionError::from(error);
                if !startup_sent {
                    let _ = startup_tx.send(Err(error.clone()));
                }
                return Err(error);
            }
        };

        if !startup_sent && committer.is_some() {
            let _ = startup_tx.send(Ok(started_sources.values().cloned().collect()));
            startup_sent = true;
        }

        if outcome == NativeCaptureDrainOutcome::Complete && stop_requested && !shutdown_sent {
            transport
                .shutdown()
                .map_err(|error| NativeCaptureSessionError::Stable(map_transport_error(error)))?;
            shutdown_sent = true;
        }

        if shutdown_sent && shutdown_ack {
            return Ok(());
        }

        if timed_out {
            thread::sleep(EVENT_POLL_INTERVAL);
        }
    }
}

fn requests_for_mode(mode: NativeCaptureMode) -> Vec<NativeCaptureSourceRequest> {
    match mode {
        NativeCaptureMode::Smart => vec![
            NativeCaptureSourceRequest {
                source: Source::Microphone,
                enabled: true,
            },
            NativeCaptureSourceRequest {
                source: Source::SystemAudio,
                enabled: true,
            },
        ],
        NativeCaptureMode::MicOnly => vec![NativeCaptureSourceRequest {
            source: Source::Microphone,
            enabled: true,
        }],
        NativeCaptureMode::SystemOnly => vec![NativeCaptureSourceRequest {
            source: Source::SystemAudio,
            enabled: true,
        }],
    }
}

fn register_capture_sources(
    catalog: &crate::catalog::Catalog,
    session_id: &str,
    started_sources: &HashMap<Source, SourceStarted>,
) -> Result<HashMap<Source, String>, NativeCaptureSessionError> {
    let created_at = Utc::now();
    let mut records = Vec::with_capacity(started_sources.len());
    let mut source_ids = HashMap::with_capacity(started_sources.len());
    for (source, started) in started_sources {
        let source_id = format!("src_{}", uuid::Uuid::new_v4().simple());
        source_ids.insert(*source, source_id.clone());
        records.push(CaptureSourceRecord {
            id: source_id,
            session_id: session_id.to_string(),
            source: match source {
                Source::Microphone => AudioSource::Microphone,
                Source::SystemAudio => AudioSource::SystemAudio,
            },
            device_uid: started.device_id.clone(),
            device_name: started.device_name.clone(),
            channel_count: started.channel_count,
            sample_rate_hz: started.sample_rate,
            created_at,
        });
    }
    catalog
        .insert_capture_sources_atomic(&records)
        .map_err(|error| NativeCaptureSessionError::Catalog(error.to_string()))?;
    Ok(source_ids)
}

fn staging_root_for(
    data_dir: &Path,
    session_id: &str,
) -> Result<PathBuf, NativeCaptureSessionError> {
    let staging = data_dir.join(STAGING_DIRECTORY);
    fs::create_dir_all(&staging)
        .map_err(|error| NativeCaptureSessionError::Catalog(error.to_string()))?;
    fs::set_permissions(&staging, fs::Permissions::from_mode(0o700))
        .map_err(|error| NativeCaptureSessionError::Catalog(error.to_string()))?;
    let root = staging.join(session_id);
    fs::create_dir_all(&root)
        .map_err(|error| NativeCaptureSessionError::Catalog(error.to_string()))?;
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
        .map_err(|error| NativeCaptureSessionError::Catalog(error.to_string()))?;
    Ok(root)
}

fn emit_stream_error(app: &AppHandle, error: &NativeCaptureSessionError) {
    let _ = app.emit(LIVE_ERROR_EVENT, &live_capture_error(error));
}

pub(crate) fn live_capture_error(error: &NativeCaptureSessionError) -> LiveCaptureError {
    match error {
        NativeCaptureSessionError::Stable(NativeCaptureStableError::Fatal { code, summary }) => {
            LiveCaptureError {
                code: code.clone(),
                message: summary.clone(),
            }
        }
        NativeCaptureSessionError::Stable(NativeCaptureStableError::DeviceInterrupted {
            reason,
            ..
        }) => LiveCaptureError {
            code: reason.clone(),
            message: match reason.as_str() {
                SELECTED_MICROPHONE_DISCONNECTED_REASON => "所选麦克风已断开连接。".to_string(),
                _ => reason.clone(),
            },
        },
        _ => LiveCaptureError {
            code: "native_capture_failed".to_string(),
            message: error.to_string(),
        },
    }
}

fn map_transport_error(
    error: super::coordinator::NativeCaptureTransportError,
) -> NativeCaptureStableError {
    match error {
        super::coordinator::NativeCaptureTransportError::UnsupportedSource(source) => {
            NativeCaptureStableError::DeviceInterrupted {
                source,
                reason: "unsupported source".to_string(),
            }
        }
        super::coordinator::NativeCaptureTransportError::HelperExited => {
            NativeCaptureStableError::HelperExited
        }
        super::coordinator::NativeCaptureTransportError::ProtocolViolation => {
            NativeCaptureStableError::ProtocolViolation
        }
        super::coordinator::NativeCaptureTransportError::Backpressure => {
            NativeCaptureStableError::Backpressure
        }
    }
}

fn validate_microphone_selection(
    mode: NativeCaptureMode,
    microphone_device_uid: Option<String>,
) -> Result<Option<String>, NativeCaptureSessionError> {
    let normalized = microphone_device_uid.map(|uid| uid.trim().to_string());
    match mode {
        NativeCaptureMode::Smart | NativeCaptureMode::MicOnly => match normalized {
            Some(uid) if !uid.is_empty() => Ok(Some(uid)),
            _ => Err(stable_fatal(
                MICROPHONE_SELECTION_REQUIRED_CODE,
                "麦克风录制前必须先明确选择一个麦克风。",
            )),
        },
        NativeCaptureMode::SystemOnly => match normalized {
            Some(uid) if !uid.is_empty() => Err(stable_fatal(
                INVALID_SYSTEM_ONLY_SELECTION_CODE,
                "仅系统音频模式不接受麦克风选择。",
            )),
            _ => Ok(None),
        },
    }
}

fn stable_fatal(code: &str, summary: &str) -> NativeCaptureSessionError {
    NativeCaptureSessionError::Stable(NativeCaptureStableError::Fatal {
        code: code.to_string(),
        summary: summary.to_string(),
    })
}

struct PendingCommitter;

impl super::coordinator::NativeCaptureChunkCommitter for PendingCommitter {
    fn commit(&mut self, _sealed: &ChunkSealed) -> Result<(), NativeCaptureStableError> {
        Err(NativeCaptureStableError::ProtocolViolation)
    }
}

fn send_control_message(
    control: &SyncSender<ControlMessage>,
    command: impl FnOnce(SyncSender<ControlResult>) -> ControlMessage,
) -> Result<(), NativeCaptureSessionError> {
    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    control
        .send(command(reply_tx))
        .map_err(|_| NativeCaptureSessionError::WorkerExited)?;
    reply_rx
        .recv()
        .map_err(|_| NativeCaptureSessionError::WorkerExited)?
}
