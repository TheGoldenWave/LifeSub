use std::io::Cursor;

use crate::capture::protocol::{
    CaptureHeader, CaptureProtocolError, FrameDecoder, Hello, PcmFormat, ProtocolValidator, Source,
    encode_frame,
};

fn hello() -> CaptureHeader {
    CaptureHeader::Hello(Hello {
        protocol_version: 1,
        helper_pid: 4242,
        launch_nonce: "0123456789abcdef".to_string(),
        supported_sources: vec![Source::Microphone, Source::SystemAudio],
    })
}

#[test]
fn capture_protocol_test_round_trips_a_canonical_frame() {
    let bytes = encode_frame(&hello(), &[]).expect("encode hello");
    let json = br#"{"helper_pid":4242,"launch_nonce":"0123456789abcdef","protocol_version":1,"supported_sources":["microphone","system_audio"],"type":"hello"}"#;
    let mut expected = (json.len() as u32).to_be_bytes().to_vec();
    expected.extend_from_slice(json);
    expected.extend_from_slice(&0_u32.to_be_bytes());
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
        br#"{"helper_pid":4242,"launch_nonce":"n","protocol_version":1,"supported_sources":["microphone"],"type":"hello","unexpected":true}"#.as_slice(),
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
        br#"{"channel_count":1,"host_time":1,"sample_rate":16000,"source":"mixed","type":"source_started","device_id":"x"}"#.as_slice(),
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
fn capture_protocol_test_rejects_replayed_hello_and_non_monotonic_audio() {
    let mut validator = ProtocolValidator::default();
    validator.observe(&hello()).expect("first hello");
    assert_eq!(
        validator.observe(&hello()),
        Err(CaptureProtocolError::HelloReplay)
    );

    let first = CaptureHeader::audio_frame(Source::Microphone, 7, 0, 10, PcmFormat::S16Le, 1);
    let replay = CaptureHeader::audio_frame(Source::Microphone, 7, 160, 20, PcmFormat::S16Le, 1);
    validator.observe(&first).expect("first sequence");
    assert_eq!(
        validator.observe(&replay),
        Err(CaptureProtocolError::NonMonotonicSequence)
    );
}
