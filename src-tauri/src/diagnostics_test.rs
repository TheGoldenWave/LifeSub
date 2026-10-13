use crate::diagnostics::{
    DiagnosticError, DiagnosticIdentities, DiagnosticMicrophoneProbe, DiagnosticModel,
    DiagnosticPermissions, DiagnosticSelectedMicrophone, DiagnosticStorage, DiagnosticsSnapshot,
    storage_snapshot,
};
use std::os::unix::fs::symlink;
use tempfile::tempdir;

#[test]
fn diagnostics_snapshot_exports_exact_release_facts_without_sensitive_content() {
    let snapshot = DiagnosticsSnapshot::new(
        DiagnosticPermissions {
            microphone: "granted".to_string(),
            screen_recording: "not_granted".to_string(),
        },
        DiagnosticSelectedMicrophone {
            uid: Some("mic-1".to_string()),
            name: Some("Built-in Microphone".to_string()),
            connected: true,
            transport: Some("built_in".to_string()),
            probe_result: Some("compatible".to_string()),
            probe: Some(DiagnosticMicrophoneProbe {
                uid: "mic-1".to_string(),
                name: "Built-in Microphone".to_string(),
                transport: "built_in".to_string(),
                input_sample_rate_hz: Some(44_100),
                input_channel_count: Some(1),
                read_back_uid: Some("mic-1".to_string()),
                first_frame_sample_count: 512,
                elapsed_ms: 12,
                result: "compatible".to_string(),
            }),
        },
        DiagnosticIdentities {
            app_version: "0.2.1".to_string(),
            app_executable_sha256: "a".repeat(64),
            helper_code_sha256: "b".repeat(64),
            capture_protocol_version: 2,
            runtime_version: "1.13.5".to_string(),
            runtime_git_commit: "3dc7c569f31ca2cd4a20ed6f7db780327e6714c5".to_string(),
            runtime_archive_sha256: "c".repeat(64),
            runtime_build_id: "sherpa-build".to_string(),
        },
        vec![DiagnosticModel {
            model_id: "sense-voice-small".to_string(),
            state: "runtime_qualified".to_string(),
            bundle_identity: "d".repeat(64),
            runtime_identity_sha256: Some("e".repeat(64)),
            last_error_code: None,
        }],
        DiagnosticStorage {
            data_directory: "/Users/test/Library/Application Support/LifeSub".to_string(),
            catalog_bytes: 4_096,
            audio_bytes: 32_000,
            model_bytes: 240_000_000,
        },
        Some(DiagnosticError {
            code: "selected_microphone_disconnected".to_string(),
            summary: "所选麦克风已断开".to_string(),
        }),
    );

    let json = snapshot.to_redacted_pretty_json().unwrap();
    assert!(json.contains("lifesub.diagnostics.v1"));
    assert!(json.contains("selected_microphone_disconnected"));
    assert!(json.contains("/Users/test/Library/Application Support/LifeSub"));
    for forbidden in [
        "launch_nonce",
        "api_key",
        "secret-token",
        "transcript text",
        "raw audio bytes",
        "/Users/test/Downloads/model.onnx",
    ] {
        assert!(!json.contains(forbidden), "export leaked {forbidden}");
    }
}

#[test]
fn diagnostics_snapshot_rejects_probe_result_without_complete_probe() {
    let snapshot = DiagnosticsSnapshot::new(
        DiagnosticPermissions {
            microphone: "granted".to_string(),
            screen_recording: "not_granted".to_string(),
        },
        DiagnosticSelectedMicrophone {
            uid: Some("mic-1".to_string()),
            name: Some("Built-in Microphone".to_string()),
            connected: true,
            transport: Some("built_in".to_string()),
            probe_result: Some("compatible".to_string()),
            probe: None,
        },
        DiagnosticIdentities {
            app_version: "0.2.1".to_string(),
            app_executable_sha256: "a".repeat(64),
            helper_code_sha256: "b".repeat(64),
            capture_protocol_version: 2,
            runtime_version: "1.13.5".to_string(),
            runtime_git_commit: "3dc7c569f31ca2cd4a20ed6f7db780327e6714c5".to_string(),
            runtime_archive_sha256: "c".repeat(64),
            runtime_build_id: "sherpa-build".to_string(),
        },
        Vec::new(),
        DiagnosticStorage {
            data_directory: "/tmp/lifesub".to_string(),
            catalog_bytes: 0,
            audio_bytes: 0,
            model_bytes: 0,
        },
        None,
    );

    assert!(snapshot.to_redacted_pretty_json().is_err());
}

#[test]
fn diagnostics_snapshot_rejects_invalid_hashes_and_permission_states() {
    let result = DiagnosticIdentities::validate_hash("not-a-sha256");
    assert!(result.is_err());
    assert!(DiagnosticPermissions::validate_state("invented").is_err());
    assert!(DiagnosticPermissions::validate_state("granted").is_ok());
    assert!(DiagnosticPermissions::validate_state("not_granted").is_ok());
}

#[test]
fn diagnostics_storage_does_not_follow_symlinks_outside_the_data_root() {
    let parent = tempdir().unwrap();
    let data = parent.path().join("data");
    let external = parent.path().join("external-audio");
    std::fs::create_dir_all(data.join("audio")).unwrap();
    std::fs::create_dir_all(data.join("models")).unwrap();
    std::fs::write(data.join("lifesub.sqlite3"), vec![0_u8; 10]).unwrap();
    std::fs::write(data.join("audio/inside.wav"), vec![0_u8; 20]).unwrap();
    std::fs::write(&external, vec![0_u8; 1_000]).unwrap();
    symlink(&external, data.join("audio/outside.wav")).unwrap();
    std::fs::write(data.join("models/model.bin"), vec![0_u8; 30]).unwrap();

    let storage = storage_snapshot(&data).unwrap();

    assert_eq!(storage.catalog_bytes, 10);
    assert_eq!(storage.audio_bytes, 20);
    assert_eq!(storage.model_bytes, 30);
}
