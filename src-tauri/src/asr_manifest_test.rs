//! Manifest contract tests for frozen model and VAD manifests.
//!
//! These tests verify that every static manifest entry satisfies the
//! immutable contract: unique IDs, HTTPS URLs, exact byte sizes,
//! 64-character SHA-256 hashes, required files, source provenance,
//! supported languages, license, and allowlisted redirect hosts.

use crate::asr::manifest::{
    self, ModelManifest, ModelSource, RequiredFile, ALLOWLISTED_REDIRECT_HOSTS,
};
use crate::asr::model_lookup::ModelLookup;
use crate::asr::settings::AsrProviderKind;

// ---------------------------------------------------------------------------
// Helper: collect all manifests into a slice
// ---------------------------------------------------------------------------

fn all_manifests() -> Vec<&'static ModelManifest> {
    vec![
        &manifest::SENSE_VOICE_SMALL_INT8,
        &manifest::WHISPER_TINY,
        &manifest::WHISPER_BASE,
        &manifest::WHISPER_SMALL,
        &manifest::SILERO_VAD,
    ]
}

fn asr_manifests() -> Vec<&'static ModelManifest> {
    vec![
        &manifest::SENSE_VOICE_SMALL_INT8,
        &manifest::WHISPER_TINY,
        &manifest::WHISPER_BASE,
        &manifest::WHISPER_SMALL,
    ]
}

// ---------------------------------------------------------------------------
// RED-phase: unique immutable IDs
// ---------------------------------------------------------------------------

#[test]
fn all_model_ids_are_unique() {
    let ids: Vec<&str> = all_manifests().iter().map(|m| m.id).collect();
    let mut deduped = ids.clone();
    deduped.sort_unstable();
    deduped.dedup();
    assert_eq!(
        ids.len(),
        deduped.len(),
        "model IDs must be unique; found duplicates"
    );
}

#[test]
fn model_ids_are_non_empty() {
    for m in all_manifests() {
        assert!(!m.id.is_empty(), "model_id must not be empty");
    }
}

// ---------------------------------------------------------------------------
// RED-phase: HTTPS URLs only
// ---------------------------------------------------------------------------

#[test]
fn all_download_urls_use_https() {
    for m in all_manifests() {
        assert!(
            m.source.download_url.starts_with("https://"),
            "download URL for {} must use HTTPS: {}",
            m.id,
            m.source.download_url
        );
    }
}

#[test]
fn download_urls_are_non_empty() {
    for m in all_manifests() {
        assert!(
            !m.source.download_url.is_empty(),
            "download URL for {} must not be empty",
            m.id
        );
    }
}

// ---------------------------------------------------------------------------
// RED-phase: exact byte sizes
// ---------------------------------------------------------------------------

#[test]
fn sense_voice_small_int8_size() {
    assert_eq!(
        manifest::SENSE_VOICE_SMALL_INT8.archive_size_bytes,
        163_002_883,
        "SenseVoiceSmall INT8 archive size must be exact"
    );
}

#[test]
fn whisper_tiny_size() {
    assert_eq!(
        manifest::WHISPER_TINY.archive_size_bytes,
        116_204_861,
        "Whisper Tiny archive size must be exact"
    );
}

#[test]
fn whisper_base_size() {
    assert_eq!(
        manifest::WHISPER_BASE.archive_size_bytes,
        207_557_382,
        "Whisper Base archive size must be exact"
    );
}

#[test]
fn whisper_small_size() {
    assert_eq!(
        manifest::WHISPER_SMALL.archive_size_bytes,
        639_387_718,
        "Whisper Small archive size must be exact"
    );
}

#[test]
fn silero_vad_size() {
    assert_eq!(
        manifest::SILERO_VAD.archive_size_bytes,
        643_854,
        "Silero VAD archive size must be exact"
    );
}

#[test]
fn all_sizes_are_positive() {
    for m in all_manifests() {
        assert!(
            m.archive_size_bytes > 0,
            "archive size for {} must be positive",
            m.id
        );
    }
}

// ---------------------------------------------------------------------------
// RED-phase: 64-character SHA-256 hashes
// ---------------------------------------------------------------------------

const SHA256_HEX_LEN: usize = 64;

#[test]
fn all_archive_sha256_are_64_hex_chars() {
    for m in all_manifests() {
        assert_eq!(
            m.archive_sha256.len(),
            SHA256_HEX_LEN,
            "archive_sha256 for {} must be exactly 64 hex characters",
            m.id
        );
        assert!(
            m.archive_sha256.chars().all(|c| c.is_ascii_hexdigit()),
            "archive_sha256 for {} must contain only hex digits",
            m.id
        );
    }
}

#[test]
fn sense_voice_small_sha256_is_frozen() {
    assert_eq!(
        manifest::SENSE_VOICE_SMALL_INT8.archive_sha256,
        "7d1efa2138a65b0b488df37f8b89e3d91a60676e416f515b952358d83dfd347e"
    );
}

#[test]
fn whisper_tiny_sha256_is_frozen() {
    assert_eq!(
        manifest::WHISPER_TINY.archive_sha256,
        "c46116994e539aa165266d96b325252728429c12535eb9d8b6a2b10f129e66b1"
    );
}

#[test]
fn whisper_base_sha256_is_frozen() {
    assert_eq!(
        manifest::WHISPER_BASE.archive_sha256,
        "911b2083efd7c0dca2ac3b358b75222660dc09fb716d64fbfc417ba6c99ff3de"
    );
}

#[test]
fn silero_vad_sha256_is_frozen() {
    assert_eq!(
        manifest::SILERO_VAD.archive_sha256,
        "9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6"
    );
}

// ---------------------------------------------------------------------------
// RED-phase: required files
// ---------------------------------------------------------------------------

#[test]
fn all_manifests_have_required_files() {
    for m in all_manifests() {
        assert!(
            !m.required_files.is_empty(),
            "{} must have at least one required file",
            m.id
        );
    }
}

#[test]
fn sense_voice_required_files() {
    let files: Vec<&str> = manifest::SENSE_VOICE_SMALL_INT8
        .required_files
        .iter()
        .map(|f| f.path)
        .collect();
    assert!(files.contains(&"model.int8.onnx"), "must contain model.int8.onnx");
    assert!(files.contains(&"tokens.txt"), "must contain tokens.txt");
}

#[test]
fn whisper_tiny_required_files() {
    let files: Vec<&str> = manifest::WHISPER_TINY
        .required_files
        .iter()
        .map(|f| f.path)
        .collect();
    assert!(files.contains(&"tiny-encoder.onnx"), "must contain tiny-encoder.onnx");
    assert!(files.contains(&"tiny-decoder.onnx"), "must contain tiny-decoder.onnx");
    assert!(files.contains(&"tiny-tokens.txt"), "must contain tiny-tokens.txt");
}

#[test]
fn whisper_base_required_files() {
    let files: Vec<&str> = manifest::WHISPER_BASE
        .required_files
        .iter()
        .map(|f| f.path)
        .collect();
    assert!(files.contains(&"base-encoder.onnx"), "must contain base-encoder.onnx");
    assert!(files.contains(&"base-decoder.onnx"), "must contain base-decoder.onnx");
    assert!(files.contains(&"base-tokens.txt"), "must contain base-tokens.txt");
}

#[test]
fn whisper_small_required_files() {
    let files: Vec<&str> = manifest::WHISPER_SMALL
        .required_files
        .iter()
        .map(|f| f.path)
        .collect();
    assert!(files.contains(&"small-encoder.onnx"), "must contain small-encoder.onnx");
    assert!(files.contains(&"small-decoder.onnx"), "must contain small-decoder.onnx");
    assert!(files.contains(&"small-tokens.txt"), "must contain small-tokens.txt");
}

#[test]
fn silero_vad_required_files() {
    let files: Vec<&str> = manifest::SILERO_VAD
        .required_files
        .iter()
        .map(|f| f.path)
        .collect();
    assert!(files.contains(&"silero_vad.onnx"), "must contain silero_vad.onnx");
}

#[test]
fn required_file_paths_are_non_empty() {
    for m in all_manifests() {
        for f in m.required_files {
            assert!(!f.path.is_empty(), "required file path must not be empty");
        }
    }
}

#[test]
fn required_file_paths_relative_no_absolute() {
    for m in all_manifests() {
        for f in m.required_files {
            assert!(
                !f.path.starts_with('/'),
                "required file path for {} must be relative: {}",
                m.id,
                f.path
            );
            assert!(
                !f.path.contains(".."),
                "required file path for {} must not contain '..': {}",
                m.id,
                f.path
            );
        }
    }
}

// ---------------------------------------------------------------------------
// RED-phase: manifest version is non-empty
// ---------------------------------------------------------------------------

#[test]
fn all_manifest_versions_are_non_empty() {
    for m in all_manifests() {
        assert!(
            !m.manifest_version.is_empty(),
            "manifest_version for {} must not be empty",
            m.id
        );
    }
}

// ---------------------------------------------------------------------------
// RED-phase: source provenance
// ---------------------------------------------------------------------------

#[test]
fn all_sources_have_upstream_repo() {
    for m in all_manifests() {
        assert!(
            !m.source.upstream_repo.is_empty(),
            "upstream_repo for {} must not be empty",
            m.id
        );
    }
}

#[test]
fn all_sources_have_license() {
    for m in all_manifests() {
        assert!(
            !m.source.license.is_empty(),
            "license for {} must not be empty",
            m.id
        );
    }
}

#[test]
fn all_sources_have_license_url() {
    for m in all_manifests() {
        assert!(
            !m.source.license_url.is_empty(),
            "license_url for {} must not be empty",
            m.id
        );
    }
}

#[test]
fn all_sources_have_original_model_id() {
    for m in all_manifests() {
        assert!(
            !m.source.original_model_id.is_empty(),
            "original_model_id for {} must not be empty",
            m.id
        );
    }
}

// ---------------------------------------------------------------------------
// RED-phase: supported languages
// ---------------------------------------------------------------------------

#[test]
fn all_asr_manifests_have_supported_languages() {
    for m in asr_manifests() {
        assert!(
            !m.supported_languages.is_empty(),
            "{} must support at least one language",
            m.id
        );
    }
}

// ---------------------------------------------------------------------------
// RED-phase: allowlisted redirect hosts
// ---------------------------------------------------------------------------

#[test]
fn allowlist_is_non_empty() {
    assert!(
        !ALLOWLISTED_REDIRECT_HOSTS.is_empty(),
        "redirect host allowlist must not be empty"
    );
}

#[test]
fn download_hosts_are_in_allowlist() {
    for m in all_manifests() {
        let host = extract_host(&m.source.download_url);
        assert!(
            ALLOWLISTED_REDIRECT_HOSTS.contains(&host),
            "download host '{}' for {} must be in the allowlist",
            host,
            m.id
        );
    }
}

fn extract_host(url: &str) -> &str {
    // Extract host from URL: "https://host/path" -> "host"
    let without_protocol = url.strip_prefix("https://").unwrap_or(url);
    without_protocol
        .split('/')
        .next()
        .unwrap_or(without_protocol)
}

// ---------------------------------------------------------------------------
// RED-phase: display name is non-empty
// ---------------------------------------------------------------------------

#[test]
fn all_display_names_are_non_empty() {
    for m in all_manifests() {
        assert!(
            !m.display_name.is_empty(),
            "display_name for {} must not be empty",
            m.id
        );
    }
}

// ---------------------------------------------------------------------------
// RED-phase: ModelLookup implementation
// ---------------------------------------------------------------------------

#[test]
fn sense_voice_lookup_provider() {
    assert_eq!(
        manifest::SENSE_VOICE_SMALL_INT8.provider(),
        AsrProviderKind::SenseVoice
    );
}

#[test]
fn whisper_tiny_lookup_provider() {
    assert_eq!(
        manifest::WHISPER_TINY.provider(),
        AsrProviderKind::Whisper
    );
}

#[test]
fn whisper_base_lookup_provider() {
    assert_eq!(
        manifest::WHISPER_BASE.provider(),
        AsrProviderKind::Whisper
    );
}

#[test]
fn whisper_small_lookup_provider() {
    assert_eq!(
        manifest::WHISPER_SMALL.provider(),
        AsrProviderKind::Whisper
    );
}

#[test]
fn sense_voice_lookup_model_id() {
    assert_eq!(
        manifest::SENSE_VOICE_SMALL_INT8.model_id(),
        "sense-voice-small-int8-2024-07-17"
    );
}

#[test]
fn whisper_tiny_lookup_model_id() {
    assert_eq!(manifest::WHISPER_TINY.model_id(), "whisper-tiny");
}

#[test]
fn whisper_base_lookup_model_id() {
    assert_eq!(manifest::WHISPER_BASE.model_id(), "whisper-base");
}

#[test]
fn whisper_small_lookup_model_id() {
    assert_eq!(manifest::WHISPER_SMALL.model_id(), "whisper-small");
}

#[test]
fn sense_voice_supports_zh() {
    assert!(manifest::SENSE_VOICE_SMALL_INT8.supports_language(
        &crate::asr::settings::AsrLanguage::Zh
    ));
}

#[test]
fn sense_voice_supports_en() {
    assert!(manifest::SENSE_VOICE_SMALL_INT8.supports_language(
        &crate::asr::settings::AsrLanguage::En
    ));
}

#[test]
fn whisper_base_supports_en() {
    assert!(manifest::WHISPER_BASE.supports_language(
        &crate::asr::settings::AsrLanguage::En
    ));
}

#[test]
fn whisper_base_supports_auto() {
    assert!(manifest::WHISPER_BASE.supports_language(
        &crate::asr::settings::AsrLanguage::Auto
    ));
}

#[test]
fn sense_voice_does_not_support_unknown_language() {
    // SenseVoice supports zh, en, ja, ko, yue — not de
    assert!(!manifest::SENSE_VOICE_SMALL_INT8.supports_language(
        &crate::asr::settings::AsrLanguage::De
    ));
}

#[test]
fn vad_manifest_has_no_provider() {
    // VAD is not an ASR model — it should not implement ModelLookup
    // This test just confirms the VAD manifest exists and is well-formed
    assert_eq!(manifest::SILERO_VAD.id, "silero-vad");
}

// ---------------------------------------------------------------------------
// RED-phase: static registry exposes all entries
// ---------------------------------------------------------------------------

#[test]
fn static_registry_contains_all_five_entries() {
    let entries = manifest::all_manifests();
    assert_eq!(entries.len(), 5, "static registry must contain exactly 5 entries");
    let ids: Vec<&str> = entries.iter().map(|m| m.id).collect();
    assert!(ids.contains(&"sense-voice-small-int8-2024-07-17"));
    assert!(ids.contains(&"whisper-tiny"));
    assert!(ids.contains(&"whisper-base"));
    assert!(ids.contains(&"whisper-small"));
    assert!(ids.contains(&"silero-vad"));
}

#[test]
fn find_by_id_returns_correct_manifest() {
    let found = manifest::find_by_id("whisper-base");
    assert!(found.is_some());
    assert_eq!(found.unwrap().id, "whisper-base");
}

#[test]
fn find_by_id_returns_none_for_unknown() {
    assert!(manifest::find_by_id("nonexistent-model").is_none());
}