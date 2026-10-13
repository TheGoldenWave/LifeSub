use std::collections::VecDeque;
use std::os::unix::fs::PermissionsExt;
use std::sync::{Arc, Mutex};

use tempfile::tempdir;

use crate::capture::coordinator::{
    NativeCaptureTransport, NativeCaptureTransportCommand, NativeCaptureTransportError,
};
use crate::capture::protocol::{CaptureHeader, Source, SourceStarted, SourceStopped};
use crate::capture::session::{
    NativeCaptureMode, NativeCaptureSession, NativeCaptureSessionError, live_capture_error,
};
use crate::domain::CaptureSession;
use crate::service::CoreRuntime;

#[derive(Clone, Default)]
struct FakeTransportFactory {
    state: Arc<Mutex<FakeTransportState>>,
}

#[derive(Default)]
struct FakeTransportState {
    commands: Vec<NativeCaptureTransportCommand>,
    events: VecDeque<CaptureHeader>,
    microphone_device_id: String,
}

struct FakeTransport {
    state: Arc<Mutex<FakeTransportState>>,
}

impl FakeTransportFactory {
    fn commands(&self) -> Vec<NativeCaptureTransportCommand> {
        self.state.lock().unwrap().commands.clone()
    }

    fn set_microphone_device_id(&self, device_id: &str) {
        self.state.lock().unwrap().microphone_device_id = device_id.to_string();
    }

    fn push_event(&self, event: CaptureHeader) {
        self.state.lock().unwrap().events.push_back(event);
    }
}

impl NativeCaptureTransport for FakeTransport {
    fn preflight_source(&mut self, source: Source) -> Result<(), NativeCaptureTransportError> {
        self.state
            .lock()
            .unwrap()
            .commands
            .push(NativeCaptureTransportCommand::Preflight(source));
        Ok(())
    }

    fn start_sources(
        &mut self,
        sources: &[Source],
        microphone_device_uid: Option<&str>,
    ) -> Result<(), NativeCaptureTransportError> {
        let mut state = self.state.lock().unwrap();
        state.commands.push(NativeCaptureTransportCommand::Start {
            sources: sources.to_vec(),
            microphone_device_uid: microphone_device_uid.map(ToOwned::to_owned),
        });
        let configured_microphone_device_id = state.microphone_device_id.clone();
        for source in sources.iter().copied() {
            state
                .events
                .push_back(CaptureHeader::SourceStarted(SourceStarted {
                    source,
                    device_id: match source {
                        Source::Microphone => {
                            if configured_microphone_device_id.is_empty() {
                                "mic-uid".to_string()
                            } else {
                                configured_microphone_device_id.clone()
                            }
                        }
                        Source::SystemAudio => "display-1".to_string(),
                    },
                    device_name: match source {
                        Source::Microphone => "MacBook Pro Microphone".to_string(),
                        Source::SystemAudio => "Built-in Display".to_string(),
                    },
                    sample_rate: 16_000,
                    channel_count: 1,
                    host_time: 1,
                }));
        }
        Ok(())
    }

    fn pause(&mut self) -> Result<(), NativeCaptureTransportError> {
        self.state
            .lock()
            .unwrap()
            .commands
            .push(NativeCaptureTransportCommand::Pause);
        Ok(())
    }

    fn resume(&mut self) -> Result<(), NativeCaptureTransportError> {
        self.state
            .lock()
            .unwrap()
            .commands
            .push(NativeCaptureTransportCommand::Resume);
        Ok(())
    }

    fn stop(&mut self) -> Result<(), NativeCaptureTransportError> {
        let mut state = self.state.lock().unwrap();
        state.commands.push(NativeCaptureTransportCommand::Stop);
        for source in [Source::Microphone, Source::SystemAudio] {
            state
                .events
                .push_back(CaptureHeader::SourceStopped(SourceStopped {
                    source,
                    final_sequence: 7,
                    start_host_time: 10,
                    end_host_time: 20,
                }));
        }
        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), NativeCaptureTransportError> {
        self.state
            .lock()
            .unwrap()
            .events
            .push_back(CaptureHeader::ShutdownAck);
        Ok(())
    }

    fn next_event(&mut self) -> Result<Option<CaptureHeader>, NativeCaptureTransportError> {
        Ok(self.state.lock().unwrap().events.pop_front())
    }
}

#[test]
fn native_capture_session_registers_real_sources_before_start_completes() {
    let parent = tempdir().unwrap();
    let data_dir = parent.path().join("data");
    let runtime = Arc::new(CoreRuntime::initialize(&data_dir).unwrap());
    let capture = CaptureSession::new("native session");
    runtime.catalog_ref().insert_session(&capture).unwrap();
    let factory = FakeTransportFactory::default();
    let mut session = NativeCaptureSession::new();

    let status = session
        .start_with_transport_factory_for_test(
            runtime.clone(),
            data_dir.clone(),
            capture.id.clone(),
            NativeCaptureMode::Smart,
            None,
            {
                let factory = factory.clone();
                move |_, _| {
                    Ok(FakeTransport {
                        state: factory.state.clone(),
                    })
                }
            },
        )
        .unwrap_err();

    assert_eq!(
        status,
        NativeCaptureSessionError::Stable(
            crate::capture::coordinator::NativeCaptureStableError::Fatal {
                code: "microphone_selection_required".to_string(),
                summary: "麦克风录制前必须先明确选择一个麦克风。".to_string(),
            }
        )
    );
}

#[test]
fn native_capture_session_registers_real_sources_before_start_completes_with_matching_uid() {
    let parent = tempdir().unwrap();
    let data_dir = parent.path().join("data");
    let runtime = Arc::new(CoreRuntime::initialize(&data_dir).unwrap());
    let capture = CaptureSession::new("native session");
    runtime.catalog_ref().insert_session(&capture).unwrap();
    let factory = FakeTransportFactory::default();
    let mut session = NativeCaptureSession::new();

    let status = session
        .start_with_transport_factory_for_test(
            runtime.clone(),
            data_dir.clone(),
            capture.id.clone(),
            NativeCaptureMode::Smart,
            Some("mic-uid".to_string()),
            {
                let factory = factory.clone();
                move |_, _| {
                    Ok(FakeTransport {
                        state: factory.state.clone(),
                    })
                }
            },
        )
        .unwrap();

    assert_eq!(status.state, "started");
    let microphone = runtime.catalog_ref().capture_source("src_missing").unwrap();
    assert!(microphone.is_none());

    let commands = factory.commands();
    assert_eq!(
        commands,
        vec![
            NativeCaptureTransportCommand::Preflight(Source::Microphone),
            NativeCaptureTransportCommand::Preflight(Source::SystemAudio),
            NativeCaptureTransportCommand::Start {
                sources: vec![Source::Microphone, Source::SystemAudio],
                microphone_device_uid: Some("mic-uid".to_string()),
            },
        ]
    );

    let source_ids = {
        let connection = runtime.catalog_ref().connection();
        ["microphone", "system_audio"].map(|source| {
            connection
                .query_row(
                    "SELECT id FROM capture_sources WHERE session_id = ?1 AND source = ?2",
                    rusqlite::params![capture.id, source],
                    |row| row.get::<_, String>(0),
                )
                .unwrap()
        })
    };
    let stored_sources =
        source_ids.map(|id| runtime.catalog_ref().capture_source(&id).unwrap().unwrap());
    assert_eq!(stored_sources[0].device_uid, "mic-uid");
    assert_eq!(stored_sources[0].device_name, "MacBook Pro Microphone");
    assert_eq!(stored_sources[1].device_uid, "display-1");
    assert_eq!(stored_sources[1].device_name, "Built-in Display");

    session.pause().unwrap();
    session.resume().unwrap();
    session.stop().unwrap();
    assert!(!session.is_running());
}

#[test]
fn native_capture_staging_directories_are_owner_only_before_helper_launch() {
    let parent = tempdir().unwrap();
    let data_dir = parent.path().join("data");
    let runtime = Arc::new(CoreRuntime::initialize(&data_dir).unwrap());
    let capture = CaptureSession::new("private native staging");
    runtime.catalog_ref().insert_session(&capture).unwrap();
    let factory = FakeTransportFactory::default();
    let observed_modes = Arc::new(Mutex::new(None));
    let mut session = NativeCaptureSession::new();

    session
        .start_with_transport_factory_for_test(
            runtime,
            data_dir.clone(),
            capture.id,
            NativeCaptureMode::MicOnly,
            Some("mic-uid".to_string()),
            {
                let factory = factory.clone();
                let observed_modes = observed_modes.clone();
                move |_, staging_root| {
                    let root_mode = staging_root
                        .parent()
                        .unwrap()
                        .metadata()
                        .unwrap()
                        .permissions()
                        .mode()
                        & 0o777;
                    let session_mode =
                        staging_root.metadata().unwrap().permissions().mode() & 0o777;
                    *observed_modes.lock().unwrap() = Some((root_mode, session_mode));
                    Ok(FakeTransport {
                        state: factory.state.clone(),
                    })
                }
            },
        )
        .unwrap();

    assert_eq!(*observed_modes.lock().unwrap(), Some((0o700, 0o700)));
    session.stop().unwrap();
}

#[test]
fn native_capture_session_rejects_missing_catalog_session() {
    let parent = tempdir().unwrap();
    let data_dir = parent.path().join("data");
    let runtime = Arc::new(CoreRuntime::initialize(&data_dir).unwrap());
    let mut session = NativeCaptureSession::new();

    let error = session
        .start_with_transport_factory_for_test(
            runtime,
            data_dir,
            "rec_missing".to_string(),
            NativeCaptureMode::Smart,
            None,
            |_, _| Ok(FakeTransport::default()),
        )
        .unwrap_err();

    assert_eq!(error, NativeCaptureSessionError::MissingSession);
}

#[test]
fn native_capture_session_passes_selected_microphone_uid_to_transport_start() {
    let parent = tempdir().unwrap();
    let data_dir = parent.path().join("data");
    let runtime = Arc::new(CoreRuntime::initialize(&data_dir).unwrap());
    let capture = CaptureSession::new("native session");
    runtime.catalog_ref().insert_session(&capture).unwrap();
    let factory = FakeTransportFactory::default();
    factory.set_microphone_device_id("mic-selected-1");
    let mut session = NativeCaptureSession::new();

    session
        .start_with_transport_factory_for_test(
            runtime,
            data_dir,
            capture.id,
            NativeCaptureMode::Smart,
            Some("mic-selected-1".to_string()),
            {
                let factory = factory.clone();
                move |_, _| {
                    Ok(FakeTransport {
                        state: factory.state.clone(),
                    })
                }
            },
        )
        .unwrap();

    assert_eq!(
        factory.commands(),
        vec![
            NativeCaptureTransportCommand::Preflight(Source::Microphone),
            NativeCaptureTransportCommand::Preflight(Source::SystemAudio),
            NativeCaptureTransportCommand::Start {
                sources: vec![Source::Microphone, Source::SystemAudio],
                microphone_device_uid: Some("mic-selected-1".to_string()),
            },
        ]
    );
}

#[test]
fn native_capture_session_rejects_selected_microphone_identity_mismatch_before_catalog_write() {
    let parent = tempdir().unwrap();
    let data_dir = parent.path().join("data");
    let runtime = Arc::new(CoreRuntime::initialize(&data_dir).unwrap());
    let capture = CaptureSession::new("native mismatch");
    runtime.catalog_ref().insert_session(&capture).unwrap();
    let factory = FakeTransportFactory::default();
    factory.set_microphone_device_id("mic-actual");
    let mut session = NativeCaptureSession::new();

    let error = session
        .start_with_transport_factory_for_test(
            runtime.clone(),
            data_dir,
            capture.id.clone(),
            NativeCaptureMode::Smart,
            Some("mic-requested".to_string()),
            {
                let factory = factory.clone();
                move |_, _| {
                    Ok(FakeTransport {
                        state: factory.state.clone(),
                    })
                }
            },
        )
        .unwrap_err();

    assert_eq!(
        error,
        NativeCaptureSessionError::Stable(
            crate::capture::coordinator::NativeCaptureStableError::Fatal {
                code: "selected_microphone_identity_mismatch".to_string(),
                summary: "所选麦克风与实际启动设备不一致。".to_string(),
            }
        )
    );
    let capture_source_count: i64 = runtime
        .catalog_ref()
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM capture_sources WHERE session_id = ?1",
            rusqlite::params![capture.id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(capture_source_count, 0);
    assert!(runtime.catalog_ref().list_chunks().unwrap().is_empty());
}

#[test]
fn native_capture_session_preserves_selected_microphone_unavailable_before_source_start() {
    let parent = tempdir().unwrap();
    let data_dir = parent.path().join("data");
    let runtime = Arc::new(CoreRuntime::initialize(&data_dir).unwrap());
    let capture = CaptureSession::new("native unavailable");
    runtime.catalog_ref().insert_session(&capture).unwrap();
    let factory = FakeTransportFactory::default();
    factory.push_event(CaptureHeader::FatalError(
        crate::capture::protocol::FatalError {
            code: "selected_microphone_unavailable".to_string(),
            summary: "所选麦克风当前不可用。".to_string(),
        },
    ));
    let mut session = NativeCaptureSession::new();

    let error = session
        .start_with_transport_factory_for_test(
            runtime.clone(),
            data_dir,
            capture.id.clone(),
            NativeCaptureMode::Smart,
            Some("mic-requested".to_string()),
            {
                let factory = factory.clone();
                move |_, _| {
                    Ok(FakeTransport {
                        state: factory.state.clone(),
                    })
                }
            },
        )
        .unwrap_err();

    assert_eq!(
        error,
        NativeCaptureSessionError::Stable(
            crate::capture::coordinator::NativeCaptureStableError::Fatal {
                code: "selected_microphone_unavailable".to_string(),
                summary: "所选麦克风当前不可用。".to_string(),
            }
        )
    );
    let ui_error = live_capture_error(&error);
    assert_eq!(ui_error.code, "selected_microphone_unavailable");
    assert_eq!(ui_error.message, "所选麦克风当前不可用。");
    let capture_source_count: i64 = runtime
        .catalog_ref()
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM capture_sources WHERE session_id = ?1",
            rusqlite::params![capture.id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(capture_source_count, 0);
    assert!(runtime.catalog_ref().list_chunks().unwrap().is_empty());
}

#[test]
fn native_capture_session_rejects_microphone_uid_for_system_only_mode() {
    let parent = tempdir().unwrap();
    let data_dir = parent.path().join("data");
    let runtime = Arc::new(CoreRuntime::initialize(&data_dir).unwrap());
    let capture = CaptureSession::new("native system only");
    runtime.catalog_ref().insert_session(&capture).unwrap();
    let mut session = NativeCaptureSession::new();

    let error = session
        .start_with_transport_factory_for_test(
            runtime,
            data_dir,
            capture.id,
            NativeCaptureMode::SystemOnly,
            Some("mic-selected-1".to_string()),
            |_, _| Ok(FakeTransport::default()),
        )
        .unwrap_err();

    assert_eq!(
        error,
        NativeCaptureSessionError::Stable(
            crate::capture::coordinator::NativeCaptureStableError::Fatal {
                code: "invalid_capture_mode_microphone_selection".to_string(),
                summary: "仅系统音频模式不接受麦克风选择。".to_string(),
            }
        )
    );
}

#[test]
fn live_capture_error_preserves_stable_microphone_codes() {
    let unavailable = live_capture_error(&NativeCaptureSessionError::Stable(
        crate::capture::coordinator::NativeCaptureStableError::Fatal {
            code: "selected_microphone_unavailable".to_string(),
            summary: "所选麦克风当前不可用。".to_string(),
        },
    ));
    assert_eq!(unavailable.code, "selected_microphone_unavailable");
    assert_eq!(unavailable.message, "所选麦克风当前不可用。");

    let disconnected = live_capture_error(&NativeCaptureSessionError::Stable(
        crate::capture::coordinator::NativeCaptureStableError::DeviceInterrupted {
            source: Source::Microphone,
            reason: "selected_microphone_disconnected".to_string(),
        },
    ));
    assert_eq!(disconnected.code, "selected_microphone_disconnected");
    assert_eq!(disconnected.message, "所选麦克风已断开连接。");
}

#[test]
fn live_capture_error_keeps_helper_fatal_codes_out_of_generic_native_capture_failed() {
    let error = live_capture_error(&NativeCaptureSessionError::Stable(
        crate::capture::coordinator::NativeCaptureStableError::Fatal {
            code: "selected_microphone_identity_mismatch".to_string(),
            summary: "所选麦克风与实际启动设备不一致。".to_string(),
        },
    ));
    assert_eq!(error.code, "selected_microphone_identity_mismatch");
    assert_ne!(error.code, "native_capture_failed");
}

impl Default for FakeTransport {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new(FakeTransportState::default())),
        }
    }
}
