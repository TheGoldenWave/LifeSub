use chrono::Utc;

use crate::asr::model_lookup::{ModelCapabilities, ModelLookup};
use crate::asr::settings::{AsrProviderOptions, AsrSettings, AsrSettingsError, WhisperTask};
use crate::domain::{
    AsrErrorCode, AsrJobState, AsrProviderKind, AudioSource, ChunkIntegrityState, DataDestination,
    ProviderOutcome, ProviderReceipt, TranscriptRevision, TranscriptSegment, TranscriptTimeRange,
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
    assert_eq!(
        serde_json::to_string(&AsrSettingsError::ModelCapabilityUnavailable).unwrap(),
        "\"model_capability_unavailable\""
    );
}

#[test]
fn language_support_comes_from_the_selected_model() {
    let supported = AsrSettings::sense_voice(SENSE_VOICE_MODEL).with_language("yue");
    let unsupported = AsrSettings::whisper(WHISPER_MODEL).with_language("yue");

    assert_eq!(supported.validate(&StubModels), Ok(()));
    assert_eq!(
        unsupported.validate(&StubModels),
        Err(AsrSettingsError::UnsupportedLanguage)
    );
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
    assert_eq!(
        serde_json::to_string(&AsrErrorCode::UnsupportedOrCorruptAudio).unwrap(),
        "\"unsupported_or_corrupt_audio\""
    );
}

#[test]
fn transcript_ranges_must_be_positive_ordered_and_within_audio_bounds() {
    let range = TranscriptTimeRange::new(0, 500, 1_000).unwrap();

    assert_eq!(range.start_ms(), 0);
    assert_eq!(range.end_ms(), 500);
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
fn provider_receipt_round_trips_without_debug_persistence() {
    let now = Utc::now();
    let receipt = ProviderReceipt {
        job_id: "job-1".to_owned(),
        chunk_id: "chunk-1".to_owned(),
        provider: AsrProviderKind::Whisper,
        model_id: WHISPER_MODEL.to_owned(),
        manifest_version: "1".to_owned(),
        archive_sha256: "a".repeat(64),
        required_file_hashes_json: "{}".to_owned(),
        model_source_json: "{}".to_owned(),
        vad_model_id: None,
        vad_manifest_version: None,
        vad_archive_sha256: None,
        vad_required_file_hashes_json: None,
        runtime_version: "1.13.5".to_owned(),
        runtime_build_id: "build-1".to_owned(),
        parameters_json: "{}".to_owned(),
        input_sha256: "b".repeat(64),
        started_at: now,
        finished_at: now,
        data_destination: DataDestination::LocalDevice,
        outcome: ProviderOutcome::Succeeded,
    };

    let json = serde_json::to_string(&receipt).unwrap();
    let restored: ProviderReceipt = serde_json::from_str(&json).unwrap();

    assert_eq!(restored, receipt);
    assert!(json.contains("\"provider\":\"whisper\""));
    assert!(json.contains("\"data_destination\":\"local_device\""));
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
