use chrono::{Duration, Utc};

use crate::asr::model_lookup::{ModelCapabilities, ModelLookup};
use crate::asr::settings::{AsrProviderOptions, AsrSettings, AsrSettingsError, WhisperTask};
use crate::domain::{
    AsrErrorCode, AsrJobState, AsrLanguage, AsrLanguageError, AsrProviderKind, AudioSource,
    ChunkIntegrityState, DataDestination, ProviderOutcome, ProviderReceipt, ProviderReceiptDraft,
    ProviderReceiptError, TranscriptRevision, TranscriptSegment, TranscriptTimeRange,
    TranscriptTimeRangeError,
};

const SENSE_VOICE_MODEL: &str = "sense-voice-small-int8-2024-07-17";
const WHISPER_MODEL: &str = "whisper-base";
const QWEN_MODEL: &str = "qwen3-asr-0.6b-int8-2026-03-25";
const QWEN_UNAVAILABLE_MODEL: &str = "qwen3-asr-1.7b";

struct StubModels;

impl ModelLookup for StubModels {
    fn lookup(&self, model_id: &str) -> Option<ModelCapabilities> {
        match model_id {
            SENSE_VOICE_MODEL => Some(ModelCapabilities::new(
                AsrProviderKind::SenseVoice,
                ["auto", "zh", "en", "ja", "ko", "yue"],
                true,
                true,
                true,
            )),
            WHISPER_MODEL => Some(ModelCapabilities::new(
                AsrProviderKind::Whisper,
                ["auto", "zh", "en"],
                true,
                true,
                true,
            )),
            QWEN_MODEL => Some(ModelCapabilities::new(
                AsrProviderKind::Qwen3Asr,
                ["auto", "zh", "en", "yue"],
                true,
                true,
                true,
            )),
            QWEN_UNAVAILABLE_MODEL => Some(ModelCapabilities::new(
                AsrProviderKind::Qwen3Asr,
                ["auto", "zh", "en", "yue"],
                true,
                false,
                false,
            )),
            _ => None,
        }
    }
}

#[test]
fn provider_options_must_match_the_selected_provider() {
    let invalid = AsrSettings::whisper(WHISPER_MODEL)
        .with_options(AsrProviderOptions::SenseVoice { use_itn: true });

    assert_eq!(
        invalid.validate(&StubModels),
        Err(AsrSettingsError::ProviderOptionsMismatch)
    );
}

#[test]
fn settings_reject_unknown_and_foreign_models() {
    let unknown = AsrSettings::whisper("unknown-model");
    let foreign = AsrSettings::whisper(SENSE_VOICE_MODEL);

    assert_eq!(
        unknown.validate(&StubModels),
        Err(AsrSettingsError::UnknownModel)
    );
    assert_eq!(
        foreign.validate(&StubModels),
        Err(AsrSettingsError::ModelProviderMismatch)
    );
}

#[test]
fn active_settings_require_an_executable_model() {
    let settings = AsrSettings::qwen3_asr(QWEN_UNAVAILABLE_MODEL);
    let capabilities = StubModels.lookup(QWEN_UNAVAILABLE_MODEL).unwrap();

    assert!(capabilities.selectable);
    assert!(!capabilities.installable);
    assert!(!capabilities.executable);
    assert_eq!(
        settings.validate(&StubModels),
        Err(AsrSettingsError::ModelCapabilityUnavailable)
    );
    let public_error = AsrErrorCode::from(AsrSettingsError::ModelCapabilityUnavailable);
    assert_eq!(
        serde_json::to_string(&public_error).unwrap(),
        "\"model_capability_unavailable\""
    );
}

#[test]
fn settings_errors_map_exhaustively_to_public_error_codes() {
    let cases = [
        (
            AsrSettingsError::UnknownModel,
            AsrErrorCode::ModelNotInstalled,
        ),
        (
            AsrSettingsError::ModelCapabilityUnavailable,
            AsrErrorCode::ModelCapabilityUnavailable,
        ),
        (
            AsrSettingsError::ModelProviderMismatch,
            AsrErrorCode::InvalidProviderParameter,
        ),
        (
            AsrSettingsError::UnsupportedLanguage,
            AsrErrorCode::InvalidProviderParameter,
        ),
        (
            AsrSettingsError::ProviderOptionsMismatch,
            AsrErrorCode::InvalidProviderParameter,
        ),
        (
            AsrSettingsError::InvalidThreadCount,
            AsrErrorCode::InvalidProviderParameter,
        ),
    ];

    for (internal, public) in cases {
        let mapped = AsrErrorCode::from(internal);
        assert_eq!(mapped, public);
        assert!(serde_json::to_string(&mapped).is_ok());
    }
}

#[test]
fn settings_validation_accepts_trait_object_model_lookups() {
    let models: &dyn ModelLookup = &StubModels;

    assert_eq!(AsrSettings::whisper(WHISPER_MODEL).validate(models), Ok(()));
}

#[test]
fn language_support_comes_from_the_selected_model() {
    let language = AsrLanguage::new("yue").unwrap();
    let supported = AsrSettings::sense_voice(SENSE_VOICE_MODEL).with_language(language.clone());
    let unsupported = AsrSettings::whisper(WHISPER_MODEL).with_language(language);

    assert_eq!(supported.validate(&StubModels), Ok(()));
    assert_eq!(
        unsupported.validate(&StubModels),
        Err(AsrSettingsError::UnsupportedLanguage)
    );
}

#[test]
fn languages_are_validated_dynamic_strings_with_transparent_serde() {
    let language = AsrLanguage::new("zh-Hans").unwrap();
    let settings_json = serde_json::to_value(AsrSettings::whisper(WHISPER_MODEL)).unwrap();

    assert_eq!(language.as_str(), "zh-Hans");
    assert_eq!(settings_json["language"], "auto");
    assert_eq!(serde_json::to_string(&language).unwrap(), "\"zh-Hans\"");
    assert_eq!(
        serde_json::from_str::<AsrLanguage>("\"zh-Hans\"").unwrap(),
        language
    );
    assert_eq!(AsrLanguage::new("   "), Err(AsrLanguageError::Empty));
    assert!(serde_json::from_str::<AsrLanguage>("\"\t\"").is_err());
}

#[test]
fn thread_bounds_follow_logical_cpu_count_and_default_to_at_most_four() {
    let logical_cpus = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1);
    let defaults = AsrSettings::sense_voice(SENSE_VOICE_MODEL);

    assert!((1..=4).contains(&defaults.num_threads));
    assert!(usize::from(defaults.num_threads) <= logical_cpus.max(1));
    assert_eq!(
        defaults.clone().with_num_threads(0).validate(&StubModels),
        Err(AsrSettingsError::InvalidThreadCount)
    );

    let out_of_range = u16::try_from(logical_cpus)
        .unwrap_or(u16::MAX)
        .saturating_add(1);
    if usize::from(out_of_range) > logical_cpus {
        assert_eq!(
            defaults
                .with_num_threads(out_of_range)
                .validate(&StubModels),
            Err(AsrSettingsError::InvalidThreadCount)
        );
    }
}

#[test]
fn provider_specific_defaults_and_overrides_remain_isolated() {
    let sense_voice = AsrSettings::sense_voice(SENSE_VOICE_MODEL);
    let whisper = AsrSettings::whisper(WHISPER_MODEL).with_options(AsrProviderOptions::Whisper {
        task: WhisperTask::Translate,
    });
    let qwen = AsrSettings::qwen3_asr(QWEN_MODEL);

    assert_eq!(
        sense_voice.options,
        AsrProviderOptions::SenseVoice { use_itn: true }
    );
    assert_eq!(
        whisper.options,
        AsrProviderOptions::Whisper {
            task: WhisperTask::Translate
        }
    );
    assert_eq!(qwen.options, AsrProviderOptions::Qwen3Asr);
    assert_eq!(sense_voice.validate(&StubModels), Ok(()));
    assert_eq!(whisper.validate(&StubModels), Ok(()));
    assert_eq!(qwen.validate(&StubModels), Ok(()));
}

#[test]
fn persisted_enums_and_tagged_options_use_stable_snake_case_strings() {
    assert_eq!(
        serde_json::to_string(&AsrProviderKind::SenseVoice).unwrap(),
        "\"sense_voice\""
    );
    assert_eq!(
        serde_json::to_string(&AsrProviderKind::Qwen3Asr).unwrap(),
        "\"qwen3_asr\""
    );
    assert_eq!(
        serde_json::to_value(AsrProviderOptions::Whisper {
            task: WhisperTask::Translate,
        })
        .unwrap(),
        serde_json::json!({"provider": "whisper", "task": "translate"})
    );
    assert_eq!(
        serde_json::to_string(&AsrJobState::BlockedModel).unwrap(),
        "\"blocked_model\""
    );
    assert_eq!(
        serde_json::to_string(&ChunkIntegrityState::Available).unwrap(),
        "\"available\""
    );
}

#[test]
fn all_asr_error_codes_have_stable_serde_strings() {
    let cases = [
        (AsrErrorCode::ModelNotInstalled, "model_not_installed"),
        (
            AsrErrorCode::ModelCapabilityUnavailable,
            "model_capability_unavailable",
        ),
        (AsrErrorCode::ModelDownloadFailed, "model_download_failed"),
        (AsrErrorCode::ModelIntegrityFailed, "model_integrity_failed"),
        (
            AsrErrorCode::InsufficientDiskSpace,
            "insufficient_disk_space",
        ),
        (
            AsrErrorCode::UnsupportedOrCorruptAudio,
            "unsupported_or_corrupt_audio",
        ),
        (AsrErrorCode::InputIntegrityFailed, "input_integrity_failed"),
        (AsrErrorCode::InputUnavailable, "input_unavailable"),
        (
            AsrErrorCode::InvalidProviderParameter,
            "invalid_provider_parameter",
        ),
        (
            AsrErrorCode::ProviderInitializationFailed,
            "provider_initialization_failed",
        ),
        (AsrErrorCode::TranscriptionFailed, "transcription_failed"),
        (AsrErrorCode::Cancelled, "cancelled"),
        (AsrErrorCode::RecoveryRequired, "recovery_required"),
        (AsrErrorCode::ReceiptInvalid, "receipt_invalid"),
    ];

    for (code, stable) in cases {
        let json = format!("\"{stable}\"");
        assert_eq!(serde_json::to_string(&code).unwrap(), json);
        assert_eq!(serde_json::from_str::<AsrErrorCode>(&json).unwrap(), code);
    }
    assert!(serde_json::from_str::<AsrErrorCode>("\"provider_failed\"").is_err());
    assert!(serde_json::from_str::<AsrErrorCode>("\"chunk_unavailable\"").is_err());
}

#[test]
fn transcript_ranges_must_be_positive_ordered_and_within_audio_bounds() {
    let range = TranscriptTimeRange::new(0, 500, 1_000).unwrap();
    let wire = serde_json::json!({
        "start_ms": 0,
        "end_ms": 500,
        "audio_duration_ms": 1_000
    });

    assert_eq!(range.start_ms(), 0);
    assert_eq!(range.end_ms(), 500);
    assert_eq!(range.audio_duration_ms(), 1_000);
    assert_eq!(serde_json::to_value(range).unwrap(), wire);
    assert_eq!(
        serde_json::from_value::<TranscriptTimeRange>(wire).unwrap(),
        range
    );
    assert!(
        serde_json::from_value::<TranscriptTimeRange>(serde_json::json!({
            "start_ms": 500,
            "end_ms": 1_001,
            "audio_duration_ms": 1_000
        }))
        .is_err()
    );
    assert_eq!(
        TranscriptTimeRange::new(-1, 500, 1_000),
        Err(TranscriptTimeRangeError::NegativeStart)
    );
    assert_eq!(
        TranscriptTimeRange::new(500, 500, 1_000),
        Err(TranscriptTimeRangeError::EmptyOrReversed)
    );
    assert_eq!(
        TranscriptTimeRange::new(500, 1_001, 1_000),
        Err(TranscriptTimeRangeError::ExceedsAudioDuration)
    );
}

#[test]
fn transcript_range_deserialization_rejects_every_invalid_wire_boundary() {
    let invalid = [
        serde_json::json!({"start_ms": -1, "end_ms": 1, "audio_duration_ms": 1}),
        serde_json::json!({"start_ms": 1, "end_ms": 1, "audio_duration_ms": 1}),
        serde_json::json!({"start_ms": 2, "end_ms": 1, "audio_duration_ms": 2}),
        serde_json::json!({"start_ms": 0, "end_ms": 1, "audio_duration_ms": -1}),
        serde_json::json!({"start_ms": i64::MIN, "end_ms": i64::MAX, "audio_duration_ms": i64::MAX}),
        serde_json::json!({"start_ms": 0, "end_ms": 1}),
    ];

    for wire in invalid {
        assert!(serde_json::from_value::<TranscriptTimeRange>(wire).is_err());
    }

    let edge = serde_json::json!({
        "start_ms": i64::MAX - 1,
        "end_ms": i64::MAX,
        "audio_duration_ms": i64::MAX
    });
    assert!(serde_json::from_value::<TranscriptTimeRange>(edge).is_ok());
}

#[test]
fn provider_receipt_round_trips_without_debug_persistence() {
    let receipt = ProviderReceipt::try_from(valid_receipt_draft()).unwrap();

    let json = serde_json::to_string(&receipt).unwrap();
    let restored: ProviderReceipt = serde_json::from_str(&json).unwrap();

    assert_eq!(restored, receipt);
    assert_eq!(receipt.job_id(), "job-1");
    assert_eq!(receipt.outcome(), ProviderOutcome::Succeeded);
    assert!(json.contains("\"provider\":\"whisper\""));
    assert!(json.contains("\"data_destination\":\"local_device\""));
}

#[test]
fn provider_receipt_rejects_invalid_fields_and_partial_vad_identity() {
    let empty_fields = [
        ("job_id", {
            let mut draft = valid_receipt_draft();
            draft.job_id.clear();
            draft
        }),
        ("chunk_id", {
            let mut draft = valid_receipt_draft();
            draft.chunk_id.clear();
            draft
        }),
        ("model_id", {
            let mut draft = valid_receipt_draft();
            draft.model_id.clear();
            draft
        }),
        ("manifest_version", {
            let mut draft = valid_receipt_draft();
            draft.manifest_version.clear();
            draft
        }),
        ("runtime_version", {
            let mut draft = valid_receipt_draft();
            draft.runtime_version.clear();
            draft
        }),
        ("runtime_build_id", {
            let mut draft = valid_receipt_draft();
            draft.runtime_build_id.clear();
            draft
        }),
    ];
    for (field, draft) in empty_fields {
        assert_eq!(
            ProviderReceipt::try_from(draft),
            Err(ProviderReceiptError::EmptyField(field))
        );
    }

    let mut invalid_hash = valid_receipt_draft();
    invalid_hash.archive_sha256 = "z".repeat(64);
    assert_eq!(
        ProviderReceipt::try_from(invalid_hash),
        Err(ProviderReceiptError::InvalidSha256("archive_sha256"))
    );

    let mut invalid_input_hash = valid_receipt_draft();
    invalid_input_hash.input_sha256 = "a".repeat(63);
    assert_eq!(
        ProviderReceipt::try_from(invalid_input_hash),
        Err(ProviderReceiptError::InvalidSha256("input_sha256"))
    );

    let mut malformed_json = valid_receipt_draft();
    malformed_json.model_source_json = "{".to_owned();
    assert_eq!(
        ProviderReceipt::try_from(malformed_json),
        Err(ProviderReceiptError::InvalidJson("model_source_json"))
    );

    let mut non_object = valid_receipt_draft();
    non_object.required_file_hashes_json = "[]".to_owned();
    assert_eq!(
        ProviderReceipt::try_from(non_object),
        Err(ProviderReceiptError::ExpectedJsonObject(
            "required_file_hashes_json"
        ))
    );

    let mut partial_vad = valid_receipt_draft();
    partial_vad.vad_archive_sha256 = None;
    assert_eq!(
        ProviderReceipt::try_from(partial_vad),
        Err(ProviderReceiptError::PartialVadIdentity)
    );

    let mut invalid_vad_hash = valid_receipt_draft();
    invalid_vad_hash.vad_archive_sha256 = Some("x".repeat(64));
    assert_eq!(
        ProviderReceipt::try_from(invalid_vad_hash),
        Err(ProviderReceiptError::InvalidSha256("vad_archive_sha256"))
    );

    let mut invalid_vad_json = valid_receipt_draft();
    invalid_vad_json.vad_required_file_hashes_json = Some("[]".to_owned());
    assert_eq!(
        ProviderReceipt::try_from(invalid_vad_json),
        Err(ProviderReceiptError::ExpectedJsonObject(
            "vad_required_file_hashes_json"
        ))
    );

    let mut no_vad = valid_receipt_draft();
    no_vad.vad_model_id = None;
    no_vad.vad_manifest_version = None;
    no_vad.vad_archive_sha256 = None;
    no_vad.vad_required_file_hashes_json = None;
    assert!(ProviderReceipt::try_from(no_vad).is_ok());

    let mut reversed = valid_receipt_draft();
    reversed.finished_at = reversed.started_at - Duration::milliseconds(1);
    assert_eq!(
        ProviderReceipt::try_from(reversed),
        Err(ProviderReceiptError::FinishedBeforeStarted)
    );
}

#[test]
fn provider_receipt_deserialization_rejects_non_success_outcomes_and_invalid_drafts() {
    let receipt = ProviderReceipt::try_from(valid_receipt_draft()).unwrap();
    let mut value = serde_json::to_value(&receipt).unwrap();

    for outcome in ["failed", "cancelled"] {
        value["outcome"] = serde_json::json!(outcome);
        assert!(serde_json::from_value::<ProviderReceipt>(value.clone()).is_err());
    }

    let mut remote = serde_json::to_value(&receipt).unwrap();
    remote["data_destination"] = serde_json::json!("remote");
    assert!(serde_json::from_value::<ProviderReceipt>(remote).is_err());

    let mut invalid = serde_json::to_value(&receipt).unwrap();
    invalid["parameters_json"] = serde_json::json!("{");
    assert!(serde_json::from_value::<ProviderReceipt>(invalid).is_err());
}

fn valid_receipt_draft() -> ProviderReceiptDraft {
    let now = Utc::now();
    ProviderReceiptDraft {
        job_id: "job-1".to_owned(),
        chunk_id: "chunk-1".to_owned(),
        provider: AsrProviderKind::Whisper,
        model_id: WHISPER_MODEL.to_owned(),
        manifest_version: "1".to_owned(),
        archive_sha256: "a".repeat(64),
        required_file_hashes_json: "{}".to_owned(),
        model_source_json: "{}".to_owned(),
        vad_model_id: Some("silero-vad".to_owned()),
        vad_manifest_version: Some("1".to_owned()),
        vad_archive_sha256: Some("c".repeat(64)),
        vad_required_file_hashes_json: Some("{}".to_owned()),
        runtime_version: "1.13.5".to_owned(),
        runtime_build_id: "build-1".to_owned(),
        parameters_json: "{\"task\":\"transcribe\"}".to_owned(),
        input_sha256: "b".repeat(64),
        started_at: now,
        finished_at: now,
        data_destination: DataDestination::LocalDevice,
        outcome: ProviderOutcome::Succeeded,
    }
}

#[test]
fn legacy_transcript_revision_keeps_its_string_provider_api() {
    let revision = TranscriptRevision {
        id: "revision-1".to_owned(),
        session_id: "session-1".to_owned(),
        number: 1,
        provider: "demo-local".to_owned(),
        created_at: Utc::now(),
        segments: vec![TranscriptSegment::new(
            0,
            500,
            AudioSource::Imported,
            "legacy transcript",
        )],
    };
    let json = serde_json::to_string(&revision).unwrap();
    let restored: TranscriptRevision = serde_json::from_str(&json).unwrap();

    assert_eq!(restored.provider, "demo-local");
}
