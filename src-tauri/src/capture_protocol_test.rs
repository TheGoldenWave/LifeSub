use std::io::Cursor;

use crate::capture::protocol::{
    CaptureCommand, CaptureHeader, CaptureProtocolError, FrameDecoder, Hello, HelloCapability,
    MicrophoneDevice, MicrophoneDevicesMessage, MicrophoneProbeMessage, MicrophoneProbeResult,
    MicrophoneTransport, PcmFormat, ProtocolValidator, Source, SourceRecovered, SourceRecovering,
    encode_frame,
};

fn hello() -> CaptureHeader {
    CaptureHeader::Hello(Hello {
        protocol_version: 2,
        helper_pid: 4242,
        launch_nonce: "0123456789abcdef".to_string(),
        supported_sources: vec![Source::Microphone, Source::SystemAudio],
        capabilities: vec![
            HelloCapability::DeviceDiscovery,
            HelloCapability::ExactMicrophoneSelection,
            HelloCapability::MicrophoneProbe,
            HelloCapability::PermissionDiagnostics,
        ],
    })
}

#[test]
fn capture_protocol_test_round_trips_a_canonical_frame() {
    let bytes = encode_frame(&hello(), &[]).expect("encode hello");
    let fixture = include_str!(
        "../native/capture-helper/Tests/CaptureProtocolTests/Fixtures/canonical-hello-frame.hex"
    );
    let expected = hex::decode(fixture.trim()).expect("shared hello fixture is valid hex");
    assert_eq!(bytes, expected);

    let frame = FrameDecoder::default()
        .decode(&mut Cursor::new(bytes))
        .expect("decode hello");

    assert_eq!(frame.header, hello());
    assert!(frame.payload.is_empty());
}

#[test]
fn capture_protocol_test_rejects_oversized_header_before_allocation() {
    let bytes = (crate::capture::protocol::MAX_HEADER_BYTES as u32 + 1)
        .to_be_bytes()
        .to_vec();
    let error = FrameDecoder::default()
        .decode(&mut Cursor::new(bytes))
        .expect_err("oversized header must fail");

    assert_eq!(error, CaptureProtocolError::HeaderTooLarge);
}

#[test]
fn capture_protocol_test_rejects_oversized_payload_before_allocation() {
    let audio = CaptureHeader::audio_frame(Source::Microphone, 1, 0, 1, PcmFormat::S16Le, 1);
    let header = serde_json_canonicalizer::to_vec(&audio).expect("canonical audio frame");
    let mut bytes = (header.len() as u32).to_be_bytes().to_vec();
    bytes.extend_from_slice(&header);
    bytes
        .extend_from_slice(&(crate::capture::protocol::MAX_PAYLOAD_BYTES as u32 + 1).to_be_bytes());

    let error = FrameDecoder::default()
        .decode(&mut Cursor::new(bytes))
        .expect_err("oversized payload must fail");
    assert_eq!(error, CaptureProtocolError::PayloadTooLarge);
}

#[test]
fn capture_protocol_test_rejects_truncated_declared_lengths() {
    let mut truncated_header = 10_u32.to_be_bytes().to_vec();
    truncated_header.extend_from_slice(b"{}");
    assert_eq!(
        FrameDecoder::default().decode(&mut Cursor::new(truncated_header)),
        Err(CaptureProtocolError::TruncatedFrame)
    );

    let audio = CaptureHeader::audio_frame(Source::Microphone, 1, 0, 1, PcmFormat::S16Le, 1);
    let header = serde_json_canonicalizer::to_vec(&audio).expect("canonical audio frame");
    let mut truncated_payload = (header.len() as u32).to_be_bytes().to_vec();
    truncated_payload.extend_from_slice(&header);
    truncated_payload.extend_from_slice(&4_u32.to_be_bytes());
    truncated_payload.extend_from_slice(&[0, 1]);
    assert_eq!(
        FrameDecoder::default().decode(&mut Cursor::new(truncated_payload)),
        Err(CaptureProtocolError::TruncatedFrame)
    );
}

#[test]
fn capture_protocol_test_rejects_unknown_fields_and_messages() {
    for json in [
        br#"{"capabilities":["device_discovery","exact_microphone_selection"],"helper_pid":4242,"launch_nonce":"n","protocol_version":2,"supported_sources":["microphone"],"type":"hello","unexpected":true}"#.as_slice(),
        br#"{"type":"invented"}"#.as_slice(),
    ] {
        let mut bytes = (json.len() as u32).to_be_bytes().to_vec();
        bytes.extend_from_slice(json);
        bytes.extend_from_slice(&0_u32.to_be_bytes());
        assert!(matches!(
            FrameDecoder::default().decode(&mut Cursor::new(bytes)),
            Err(CaptureProtocolError::InvalidHeader)
        ));
    }
}

#[test]
fn capture_protocol_test_rejects_invalid_source_and_pcm_format() {
    for json in [
        br#"{"channel_count":1,"device_id":"x","device_name":"Test","host_time":1,"sample_rate":16000,"source":"mixed","type":"source_started"}"#.as_slice(),
        br#"{"channel_count":1,"discontinuity_flags":[],"format":"float64","host_time":1,"sample_position":0,"sequence":1,"source":"microphone","type":"audio_frame"}"#.as_slice(),
    ] {
        let mut bytes = (json.len() as u32).to_be_bytes().to_vec();
        bytes.extend_from_slice(json);
        bytes.extend_from_slice(&0_u32.to_be_bytes());
        assert!(matches!(
            FrameDecoder::default().decode(&mut Cursor::new(bytes)),
            Err(CaptureProtocolError::InvalidHeader)
        ));
    }

    assert_eq!(PcmFormat::S16Le.bytes_per_sample(), 2);
}

#[test]
fn capture_protocol_test_rejects_empty_and_sample_misaligned_pcm() {
    let header = CaptureHeader::audio_frame(Source::Microphone, 1, 0, 1, PcmFormat::S16Le, 2);
    for payload in [&[][..], &[0, 1, 2][..]] {
        assert_eq!(
            encode_frame(&header, payload),
            Err(CaptureProtocolError::InvalidAudioPayload)
        );
        let header_bytes = serde_json_canonicalizer::to_vec(&header).expect("audio header");
        let mut bytes = (header_bytes.len() as u32).to_be_bytes().to_vec();
        bytes.extend_from_slice(&header_bytes);
        bytes.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        bytes.extend_from_slice(payload);
        assert!(matches!(
            FrameDecoder::default().decode(&mut Cursor::new(bytes)),
            Err(CaptureProtocolError::InvalidAudioPayload)
        ));
    }
}

#[test]
fn capture_protocol_test_decodes_v2_device_contract_strictly() {
    for (json, expected) in [
        (
            r#"{"type":"start","protocol_version":2,"requested_sources":["microphone"],"staging_root":"/tmp/capture","microphone_device_uid":"mic-1"}"#,
            CaptureCommand::Start {
                protocol_version: 2,
                requested_sources: vec![Source::Microphone],
                staging_root: "/tmp/capture".to_string(),
                microphone_device_uid: Some("mic-1".to_string()),
            },
        ),
        (
            r#"{"type":"start","protocol_version":2,"requested_sources":["system_audio"],"staging_root":"/tmp/capture"}"#,
            CaptureCommand::Start {
                protocol_version: 2,
                requested_sources: vec![Source::SystemAudio],
                staging_root: "/tmp/capture".to_string(),
                microphone_device_uid: None,
            },
        ),
        (
            r#"{"type":"list_microphones","protocol_version":2}"#,
            CaptureCommand::ListMicrophones {
                protocol_version: 2,
            },
        ),
        (
            r#"{"type":"probe_microphone","protocol_version":2,"microphone_device_uid":"mic-1"}"#,
            CaptureCommand::ProbeMicrophone {
                protocol_version: 2,
                microphone_device_uid: "mic-1".to_string(),
            },
        ),
        (
            r#"{"type":"get_permission_state","protocol_version":2}"#,
            CaptureCommand::GetPermissionState {
                protocol_version: 2,
            },
        ),
    ] {
        assert_eq!(
            serde_json::from_str::<CaptureCommand>(json).unwrap(),
            expected
        );
    }

    assert!(serde_json::from_str::<CaptureCommand>(
        r#"{"type":"start","protocol_version":2,"requested_sources":["microphone"],"staging_root":"/tmp/capture"}"#
    )
    .is_err());
    assert!(serde_json::from_str::<CaptureCommand>(
        r#"{"type":"start","protocol_version":2,"requested_sources":["system_audio"],"staging_root":"/tmp/capture","microphone_device_uid":"mic-1"}"#
    )
    .is_err());
}

#[test]
fn capture_protocol_test_decodes_probe_contract_strictly() {
    let compatible = CaptureHeader::MicrophoneProbeResult(MicrophoneProbeMessage {
        uid: "mic-1".to_string(),
        name: "Built-in Microphone".to_string(),
        transport: MicrophoneTransport::BuiltIn,
        input_sample_rate_hz: Some(44_100),
        input_channel_count: Some(1),
        read_back_uid: Some("mic-1".to_string()),
        first_frame_sample_count: 371,
        elapsed_ms: 42,
        result: MicrophoneProbeResult::Compatible,
    });
    let bytes = encode_frame(&compatible, &[]).expect("encode compatible probe result");
    assert_eq!(
        FrameDecoder::default()
            .decode(&mut Cursor::new(bytes))
            .expect("decode compatible probe result")
            .header,
        compatible
    );

    for json in [
        br#"{"type":"probe_microphone","protocol_version":2,"microphone_device_uid":""}"#.as_slice(),
        br#"{"type":"microphone_probe_result","uid":"mic-1","name":"Built-in Microphone","transport":"built_in","input_sample_rate_hz":44100,"input_channel_count":1,"read_back_uid":"mic-1","first_frame_sample_count":0,"elapsed_ms":42,"result":"compatible"}"#.as_slice(),
        br#"{"type":"microphone_probe_result","uid":"mic-1","name":"Built-in Microphone","transport":"built_in","input_sample_rate_hz":44100,"input_channel_count":1,"read_back_uid":"mic-1","first_frame_sample_count":0,"elapsed_ms":42,"result":"identity_mismatch"}"#.as_slice(),
        br#"{"type":"microphone_probe_result","uid":"mic-1","name":"Built-in Microphone","transport":"built_in","input_sample_rate_hz":44100,"input_channel_count":1,"read_back_uid":"mic-1","first_frame_sample_count":371,"elapsed_ms":42,"result":"invented"}"#.as_slice(),
        br#"{"type":"microphone_probe_result","uid":"mic-1","name":"Built-in Microphone","transport":"built_in","input_sample_rate_hz":44100,"input_channel_count":1,"read_back_uid":"mic-1","first_frame_sample_count":371,"elapsed_ms":42,"result":"compatible","extra":true}"#.as_slice(),
    ] {
        let mut frame = (json.len() as u32).to_be_bytes().to_vec();
        frame.extend_from_slice(json);
        frame.extend_from_slice(&0_u32.to_be_bytes());
        let result = FrameDecoder::default().decode(&mut Cursor::new(frame));
        assert!(
            matches!(result, Err(CaptureProtocolError::InvalidHeader)),
            "unexpected probe decode result for {}: {result:?}",
            String::from_utf8_lossy(json)
        );
    }
}

#[test]
fn capture_protocol_test_decodes_recovery_events_strictly() {
    for header in [
        CaptureHeader::SourceRecovering(SourceRecovering {
            source: Source::Microphone,
            generation: 7,
            reason: "device_configuration_changed".to_string(),
        }),
        CaptureHeader::SourceRecovered(SourceRecovered {
            source: Source::Microphone,
            generation: 7,
            device_id: "mic-1".to_string(),
            first_host_time: 42,
        }),
    ] {
        let bytes = encode_frame(&header, &[]).expect("encode recovery event");
        assert_eq!(
            FrameDecoder::default()
                .decode(&mut Cursor::new(bytes))
                .expect("decode recovery event")
                .header,
            header
        );
    }

    for json in [
        br#"{"type":"source_recovering","source":"microphone","generation":0,"reason":"device_configuration_changed"}"#.as_slice(),
        br#"{"type":"source_recovered","source":"microphone","generation":0,"device_id":"mic-1","first_host_time":42}"#.as_slice(),
        br#"{"type":"source_recovered","source":"microphone","generation":7,"device_id":"","first_host_time":42}"#.as_slice(),
        br#"{"type":"source_recovered","source":"microphone","generation":7,"device_id":"mic-1","first_host_time":0}"#.as_slice(),
    ] {
        let mut frame = (json.len() as u32).to_be_bytes().to_vec();
        frame.extend_from_slice(json);
        frame.extend_from_slice(&0_u32.to_be_bytes());
        assert!(matches!(
            FrameDecoder::default().decode(&mut Cursor::new(frame)),
            Err(CaptureProtocolError::InvalidHeader)
        ));
    }
}

#[test]
fn capture_protocol_test_rejects_invalid_microphone_device_lists() {
    let valid = CaptureHeader::MicrophoneDevices(MicrophoneDevicesMessage {
        devices: vec![
            MicrophoneDevice {
                uid: "builtin-1".to_string(),
                name: "Built-in Microphone".to_string(),
                is_default: true,
                is_connected: true,
                transport: MicrophoneTransport::BuiltIn,
            },
            MicrophoneDevice {
                uid: "usb-1".to_string(),
                name: "USB Mic".to_string(),
                is_default: false,
                is_connected: true,
                transport: MicrophoneTransport::Usb,
            },
        ],
    });
    let encoded = encode_frame(&valid, &[]).expect("encode device list");
    let frame = FrameDecoder::default()
        .decode(&mut Cursor::new(encoded))
        .expect("decode valid device list");
    assert_eq!(frame.header, valid);

    for json in [
        br#"{"type":"microphone_devices","devices":[{"uid":"dup","name":"Built-in Microphone","is_default":true,"is_connected":true,"transport":"built_in"},{"uid":"dup","name":"USB Mic","is_default":false,"is_connected":true,"transport":"usb"}]}"#.as_slice(),
        br#"{"type":"microphone_devices","devices":[{"uid":"","name":"Built-in Microphone","is_default":true,"is_connected":true,"transport":"built_in"}]}"#.as_slice(),
        br#"{"type":"microphone_devices","devices":[{"uid":"builtin-1","name":"","is_default":true,"is_connected":true,"transport":"built_in"}]}"#.as_slice(),
        br#"{"type":"microphone_devices","devices":[{"uid":"builtin-1","name":"Built-in Microphone","is_default":true,"is_connected":true,"transport":"built_in","extra":true}]}"#.as_slice(),
        br#"{"type":"microphone_devices","devices":[{"uid":"builtin-1","name":"Built-in Microphone","is_default":true,"is_connected":false,"transport":"built_in"}]}"#.as_slice(),
        br#"{"type":"microphone_devices","devices":[{"uid":"builtin-1","name":"Built-in Microphone","is_default":true,"is_connected":true,"transport":"airplay"}]}"#.as_slice(),
        br#"{"type":"microphone_devices","devices":[{"uid":"builtin-1","name":"Built-in Microphone","isDefault":true,"is_connected":true,"transport":"built_in"}]}"#.as_slice(),
        br#"{"type":"microphone_devices","devices":[{"uid":"builtin-1","name":"Built-in Microphone","is__default":true,"is_connected":true,"transport":"built_in"}]}"#.as_slice(),
    ] {
        let mut bytes = (json.len() as u32).to_be_bytes().to_vec();
        bytes.extend_from_slice(json);
        bytes.extend_from_slice(&0_u32.to_be_bytes());
        assert!(matches!(
            FrameDecoder::default().decode(&mut Cursor::new(bytes)),
            Err(CaptureProtocolError::InvalidHeader)
        ));
    }

    for (wire_value, expected) in [
        ("built_in", MicrophoneTransport::BuiltIn),
        ("usb", MicrophoneTransport::Usb),
        ("bluetooth", MicrophoneTransport::Bluetooth),
        ("aggregate", MicrophoneTransport::Aggregate),
        ("virtual", MicrophoneTransport::Virtual),
        ("unknown", MicrophoneTransport::Unknown),
    ] {
        let json = format!(
            r#"{{"type":"microphone_devices","devices":[{{"uid":"mic-1","name":"Test Microphone","is_default":false,"is_connected":true,"transport":"{wire_value}"}}]}}"#
        );
        let mut bytes = (json.len() as u32).to_be_bytes().to_vec();
        bytes.extend_from_slice(json.as_bytes());
        bytes.extend_from_slice(&0_u32.to_be_bytes());
        let frame = FrameDecoder::default()
            .decode(&mut Cursor::new(bytes))
            .expect("supported microphone transport");
        assert_eq!(
            frame.header,
            CaptureHeader::MicrophoneDevices(MicrophoneDevicesMessage {
                devices: vec![MicrophoneDevice {
                    uid: "mic-1".to_string(),
                    name: "Test Microphone".to_string(),
                    is_default: false,
                    is_connected: true,
                    transport: expected,
                }],
            })
        );
    }

    let oversized = CaptureHeader::MicrophoneDevices(MicrophoneDevicesMessage {
        devices: (0..=crate::capture::protocol::MAX_MICROPHONE_DEVICES)
            .map(|index| MicrophoneDevice {
                uid: format!("mic-{index}"),
                name: format!("Microphone {index}"),
                is_default: index == 0,
                is_connected: true,
                transport: MicrophoneTransport::Unknown,
            })
            .collect(),
    });
    assert_eq!(
        encode_frame(&oversized, &[]),
        Err(CaptureProtocolError::InvalidHeader)
    );
}

#[test]
fn capture_protocol_test_rejects_payload_for_non_audio_messages() {
    assert_eq!(
        encode_frame(&hello(), &[0]),
        Err(CaptureProtocolError::UnexpectedPayload)
    );

    let header = serde_json_canonicalizer::to_vec(&hello()).expect("canonical hello");
    let mut bytes = (header.len() as u32).to_be_bytes().to_vec();
    bytes.extend_from_slice(&header);
    bytes.extend_from_slice(&1_u32.to_be_bytes());
    bytes.push(0);
    assert_eq!(
        FrameDecoder::default().decode(&mut Cursor::new(bytes)),
        Err(CaptureProtocolError::UnexpectedPayload)
    );
}

#[test]
fn capture_protocol_test_rejects_replayed_hello_and_non_monotonic_audio() {
    let mut validator = ProtocolValidator::default();
    validator.observe(&hello()).expect("first hello");
    assert_eq!(
        validator.observe(&hello()),
        Err(CaptureProtocolError::HelloReplay)
    );

    assert_eq!(
        ProtocolValidator::default().observe(&CaptureHeader::Hello(Hello {
            protocol_version: 1,
            helper_pid: 4242,
            launch_nonce: "0123456789abcdef".to_string(),
            supported_sources: vec![Source::Microphone, Source::SystemAudio],
            capabilities: vec![HelloCapability::DeviceDiscovery],
        })),
        Err(CaptureProtocolError::UnsupportedVersion)
    );
    assert_eq!(
        ProtocolValidator::default().observe(&CaptureHeader::Hello(Hello {
            protocol_version: 2,
            helper_pid: 4242,
            launch_nonce: "0123456789abcdef".to_string(),
            supported_sources: vec![Source::Microphone, Source::SystemAudio],
            capabilities: vec![HelloCapability::DeviceDiscovery],
        })),
        Err(CaptureProtocolError::InvalidHeader)
    );

    let first = CaptureHeader::audio_frame(Source::Microphone, 7, 0, 10, PcmFormat::S16Le, 1);
    let replay = CaptureHeader::audio_frame(Source::Microphone, 7, 160, 20, PcmFormat::S16Le, 1);
    validator.observe(&first).expect("first sequence");
    assert_eq!(
        validator.observe(&replay),
        Err(CaptureProtocolError::NonMonotonicSequence)
    );
}

#[test]
fn capture_protocol_test_decodes_control_commands_strictly() {
    for (json, expected) in [
        (
            r#"{"type":"start","protocol_version":2,"requested_sources":["microphone"],"staging_root":"/tmp/capture","microphone_device_uid":"mic-1"}"#,
            CaptureCommand::Start {
                protocol_version: 2,
                requested_sources: vec![Source::Microphone],
                staging_root: "/tmp/capture".to_string(),
                microphone_device_uid: Some("mic-1".to_string()),
            },
        ),
        (
            r#"{"type":"pause","protocol_version":2}"#,
            CaptureCommand::Pause {
                protocol_version: 2,
            },
        ),
        (
            r#"{"type":"resume","protocol_version":2}"#,
            CaptureCommand::Resume {
                protocol_version: 2,
            },
        ),
        (
            r#"{"type":"stop","protocol_version":2}"#,
            CaptureCommand::Stop {
                protocol_version: 2,
            },
        ),
        (
            r#"{"type":"shutdown","protocol_version":2}"#,
            CaptureCommand::Shutdown {
                protocol_version: 2,
            },
        ),
    ] {
        assert_eq!(
            serde_json::from_str::<CaptureCommand>(json).unwrap(),
            expected
        );
    }
    assert!(
        serde_json::from_str::<CaptureCommand>(
        r#"{"type":"start","protocol_version":2,"requested_sources":["microphone"],"staging_root":"/tmp/capture","microphone_device_uid":"mic-1","extra":true}"#
        )
        .is_err()
    );
    for json in [
        r#"{"type":"start","protocol_version":1,"requested_sources":["microphone"],"staging_root":"/tmp/capture","microphone_device_uid":"mic-1"}"#,
        r#"{"type":"list_microphones","protocol_version":1}"#,
        r#"{"type":"pause","protocol_version":1}"#,
        r#"{"type":"resume","protocol_version":1}"#,
        r#"{"type":"stop","protocol_version":1}"#,
        r#"{"type":"shutdown","protocol_version":1}"#,
    ] {
        assert!(serde_json::from_str::<CaptureCommand>(json).is_err());
    }
}
