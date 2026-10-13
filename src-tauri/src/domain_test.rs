use crate::domain::{
    AsrConfig, AsrErrorCode, CaptureSession, CaptureState, DomainError, ImportAsrDisposition,
    ImportModelReadiness, RecordingConfig,
};

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

#[test]
fn legacy_asr_config_json_migrates_default_model_id_by_provider() {
    let cases = [
        ("sense_voice", "sense-voice-small-int8-2024-07-17", true),
        ("whisper", "whisper-base", false),
        ("qwen3_asr", "qwen3-asr-0.6b-int8-2026-03-25", false),
    ];

    for (provider, expected_model_id, expected_itn) in cases {
        let json = format!(
            r#"{{
                "provider": "{provider}",
                "language": "auto",
                "auto_transcribe": true,
                "threads": 4,
                "vad_enabled": true,
                "vad_min_speech_ms": 300,
                "vad_silence_ms": 800,
                "itn_enabled": {expected_itn}
            }}"#
        );

        let config: AsrConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.provider, provider);
        assert_eq!(config.model_id, expected_model_id);
    }
}

#[test]
fn asr_config_validation_rejects_provider_model_mismatch() {
    let config = AsrConfig {
        provider: "sense_voice".into(),
        model_id: "whisper-base".into(),
        language: "auto".into(),
        auto_transcribe: true,
        threads: 4,
        vad_enabled: true,
        vad_min_speech_ms: 300,
        vad_silence_ms: 800,
        itn_enabled: true,
        whisper_task: None,
    };

    let error = config.validate_for_persistence().unwrap_err();
    assert_eq!(
        error,
        "model whisper-base does not belong to provider sense_voice"
    );
}

#[test]
fn legacy_whisper_config_defaults_to_the_transcribe_task() {
    let config: AsrConfig = serde_json::from_str(
        r#"{
            "provider": "whisper",
            "model_id": "whisper-base",
            "language": "auto",
            "auto_transcribe": true,
            "threads": 4,
            "vad_enabled": true,
            "vad_min_speech_ms": 300,
            "vad_silence_ms": 800,
            "itn_enabled": false
        }"#,
    )
    .unwrap();

    let json = serde_json::to_value(config).unwrap();

    assert_eq!(json["whisper_task"], "transcribe");
}

#[test]
fn asr_config_validation_rejects_unsupported_languages_for_the_selected_model() {
    let config = AsrConfig {
        provider: "whisper".into(),
        model_id: "whisper-base".into(),
        language: "fr".into(),
        auto_transcribe: true,
        threads: 4,
        vad_enabled: true,
        vad_min_speech_ms: 300,
        vad_silence_ms: 800,
        itn_enabled: false,
        whisper_task: Some("transcribe".into()),
    };

    assert_eq!(
        config.validate_for_persistence().unwrap_err(),
        "language fr is not supported by model whisper-base"
    );
}

#[test]
fn asr_config_rejects_unknown_whisper_tasks_in_json() {
    let result = serde_json::from_str::<AsrConfig>(
        r#"{
            "provider": "whisper",
            "model_id": "whisper-base",
            "language": "auto",
            "auto_transcribe": true,
            "threads": 4,
            "vad_enabled": true,
            "vad_min_speech_ms": 300,
            "vad_silence_ms": 800,
            "itn_enabled": false,
            "whisper_task": "summarize"
        }"#,
    );

    assert!(result.is_err());
}

#[test]
fn import_asr_disposition_only_blocks_for_model_readiness_errors() {
    assert_eq!(
        ImportAsrDisposition::classify(
            true,
            ImportModelReadiness::Ready,
            Err(AsrErrorCode::ModelNotInstalled),
        ),
        ImportAsrDisposition::BlockedModel(AsrErrorCode::ModelNotInstalled),
    );
    assert_eq!(
        ImportAsrDisposition::classify(
            true,
            ImportModelReadiness::Ready,
            Err(AsrErrorCode::InvalidProviderParameter),
        ),
        ImportAsrDisposition::Failed(AsrErrorCode::InvalidProviderParameter),
    );
    assert_eq!(
        ImportAsrDisposition::classify(
            false,
            ImportModelReadiness::Blocked(AsrErrorCode::ModelCapabilityUnavailable),
            Ok(()),
        ),
        ImportAsrDisposition::NoJob,
    );
}

#[test]
fn legacy_recording_config_json_loads_with_null_microphone_selection() {
    let config: RecordingConfig = serde_json::from_str(
        r#"{
            "capture_mode": "smart",
            "im_detection_enabled": true,
            "im_apps": ["wechat"],
            "detection_delay_secs": 3,
            "recovery_delay_secs": 5,
            "sample_rate": 16000,
            "storage_path": "~/.lifesub/recordings/"
        }"#,
    )
    .unwrap();

    assert_eq!(config.microphone_device_uid, None);
    assert_eq!(config.microphone_device_name, None);
}

#[test]
fn recording_config_round_trips_a_valid_microphone_selection_pair() {
    let original = RecordingConfig {
        capture_mode: "smart".to_string(),
        im_detection_enabled: true,
        im_apps: vec!["wechat".to_string()],
        detection_delay_secs: 3,
        recovery_delay_secs: 5,
        sample_rate: 16_000,
        storage_path: "~/.lifesub/recordings/".to_string(),
        microphone_device_uid: Some("mic-uid-1".to_string()),
        microphone_device_name: Some("Built-in Microphone".to_string()),
    };

    let json = serde_json::to_string(&original).unwrap();
    let decoded: RecordingConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(decoded, original);
}

#[test]
fn recording_config_rejects_partial_or_empty_microphone_selection() {
    for json in [
        r#"{
            "capture_mode": "smart",
            "im_detection_enabled": true,
            "im_apps": [],
            "detection_delay_secs": 3,
            "recovery_delay_secs": 5,
            "sample_rate": 16000,
            "storage_path": "~/.lifesub/recordings/",
            "microphone_device_uid": "mic-1"
        }"#,
        r#"{
            "capture_mode": "smart",
            "im_detection_enabled": true,
            "im_apps": [],
            "detection_delay_secs": 3,
            "recovery_delay_secs": 5,
            "sample_rate": 16000,
            "storage_path": "~/.lifesub/recordings/",
            "microphone_device_name": "Built-in Microphone"
        }"#,
        r#"{
            "capture_mode": "smart",
            "im_detection_enabled": true,
            "im_apps": [],
            "detection_delay_secs": 3,
            "recovery_delay_secs": 5,
            "sample_rate": 16000,
            "storage_path": "~/.lifesub/recordings/",
            "microphone_device_uid": "",
            "microphone_device_name": "Built-in Microphone"
        }"#,
        r#"{
            "capture_mode": "smart",
            "im_detection_enabled": true,
            "im_apps": [],
            "detection_delay_secs": 3,
            "recovery_delay_secs": 5,
            "sample_rate": 16000,
            "storage_path": "~/.lifesub/recordings/",
            "microphone_device_uid": "mic-1",
            "microphone_device_name": ""
        }"#,
    ] {
        assert!(serde_json::from_str::<RecordingConfig>(json).is_err());
    }
}
