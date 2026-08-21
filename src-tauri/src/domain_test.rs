use crate::domain::{
    AsrConfig, AsrErrorCode, CaptureSession, CaptureState, DomainError, ImportAsrDisposition,
    ImportModelReadiness,
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
    };

    let error = config.validate_for_persistence().unwrap_err();
    assert_eq!(
        error,
        "model whisper-base does not belong to provider sense_voice"
    );
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
