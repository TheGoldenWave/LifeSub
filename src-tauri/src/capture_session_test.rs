use std::collections::HashMap;

use crate::capture::coordinator::NativeCaptureSourceRequest;
use crate::capture::protocol::Source;
use crate::capture::protocol::SourceStarted;
use crate::capture::session::CaptureSourceRegistry;
use crate::capture::session::{CaptureSessionPlan, CaptureSessionPlanError};

#[test]
fn session_plan_requires_ids_for_every_enabled_source() {
    let error = CaptureSessionPlan::new(
        "session-1",
        "/tmp/staging",
        "/tmp/data",
        vec![NativeCaptureSourceRequest {
            source: Source::Microphone,
            enabled: true,
        }],
        HashMap::new(),
    )
    .unwrap_err();
    assert_eq!(
        error,
        CaptureSessionPlanError::MissingSourceId(Source::Microphone)
    );
}

#[test]
fn session_plan_allows_disabled_sources_without_metadata() {
    let plan = CaptureSessionPlan::new(
        "session-1",
        "/tmp/staging",
        "/tmp/data",
        vec![
            NativeCaptureSourceRequest {
                source: Source::Microphone,
                enabled: true,
            },
            NativeCaptureSourceRequest {
                source: Source::SystemAudio,
                enabled: false,
            },
        ],
        HashMap::from([(Source::Microphone, "source-mic".to_string())]),
    )
    .unwrap();
    assert_eq!(plan.session_id, "session-1");
    assert_eq!(plan.source_ids.len(), 1);
}

#[test]
fn source_registry_requires_real_device_metadata_and_preserves_identity() {
    let mut registry = CaptureSourceRegistry::default();
    let event = SourceStarted {
        source: Source::Microphone,
        device_id: "device-123".into(),
        device_name: "Built-in Microphone".into(),
        sample_rate: 48_000,
        channel_count: 1,
        host_time: 10,
    };
    registry.observe_started("session-1", &event).unwrap();
    let record = registry.records().next().unwrap();
    assert_eq!(record.session_id, "session-1");
    assert_eq!(record.device_uid, "device-123");
    assert_eq!(record.sample_rate_hz, 48_000);
}
