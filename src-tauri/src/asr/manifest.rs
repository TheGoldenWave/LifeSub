//! Frozen model and VAD manifests.
//!
//! Every manifest entry is a version-controlled static constant. The
//! archive SHA-256, required files, and source provenance are
//! immutable — if an upstream asset changes, a new model ID and
//! manifest version must be published.

use super::model_lookup::ModelLookup;
use super::settings::{AsrLanguage, AsrProviderKind};

// ---------------------------------------------------------------------------
// Hosts that model downloads may redirect to
// ---------------------------------------------------------------------------

/// Allowlisted hosts for HTTP redirects during model download.
///
/// All model archive URLs point to GitHub releases, which may redirect
/// to `objects.githubusercontent.com` for the actual asset download.
/// Only these hosts are allowed; any other redirect target is rejected.
pub const ALLOWLISTED_REDIRECT_HOSTS: &[&str] = &[
    "github.com",
    "objects.githubusercontent.com",
];

// ---------------------------------------------------------------------------
// Required file in a model archive
// ---------------------------------------------------------------------------

/// A single file that must exist inside the extracted model archive.
///
/// The path is relative to the archive root and must not be absolute
/// or contain `..` components. The optional `sha256` field, when set,
/// provides an additional per-file integrity check.
#[derive(Clone, Debug)]
pub struct RequiredFile {
    /// Relative path inside the extracted archive, e.g. "model.int8.onnx".
    pub path: &'static str,
    /// Optional SHA-256 hex digest of the individual file.
    pub sha256: Option<&'static str>,
}

// ---------------------------------------------------------------------------
// Model source provenance
// ---------------------------------------------------------------------------

/// Immutable provenance record for a model artifact.
///
/// Every field is required so that a Receipt can independently verify
/// exactly which upstream source, conversion tool, and parameters
/// produced the model weights.
#[derive(Clone, Debug)]
pub struct ModelSource {
    /// URL of the upstream model project repository.
    pub upstream_repo: &'static str,
    /// Upstream commit or tag that this model was derived from.
    pub upstream_commit_or_tag: &'static str,
    /// The original model ID in the upstream project.
    pub original_model_id: &'static str,
    /// URL of the conversion tool repository (e.g. sherpa-onnx).
    pub conversion_tool_repo: &'static str,
    /// Conversion tool commit or tag.
    pub conversion_tool_commit: &'static str,
    /// Human-readable description of conversion parameters.
    pub conversion_params: &'static str,
    /// The exact HTTPS URL to download the model archive.
    pub download_url: &'static str,
    /// SPDX license identifier (e.g. "MIT", "Apache-2.0").
    pub license: &'static str,
    /// URL to the full license text.
    pub license_url: &'static str,
}

// ---------------------------------------------------------------------------
// Model manifest
// ---------------------------------------------------------------------------

/// Immutable manifest for a single model or VAD artifact.
///
/// Each manifest is a `static` constant in this module. The fields
/// are sufficient to download, verify, install, and produce a
/// traceable Provider Receipt.
#[derive(Clone, Debug)]
pub struct ModelManifest {
    /// Stable, unique model identifier. Never reused across versions.
    pub id: &'static str,
    /// Human-readable display name for the UI.
    pub display_name: &'static str,
    /// The ASR provider kind, or None for non-ASR artifacts (VAD).
    pub provider: Option<AsrProviderKind>,
    /// Manifest version string. Bumped when the manifest entry changes.
    pub manifest_version: &'static str,
    /// Expected byte size of the downloaded archive.
    pub archive_size_bytes: u64,
    /// SHA-256 hex digest of the downloaded archive.
    pub archive_sha256: &'static str,
    /// Files that must exist inside the extracted archive.
    pub required_files: &'static [RequiredFile],
    /// Immutable source provenance for the model artifact.
    pub source: ModelSource,
    /// Languages this model supports for transcription.
    pub supported_languages: &'static [AsrLanguage],
}

// ---------------------------------------------------------------------------
// ModelLookup implementation for ModelManifest
// ---------------------------------------------------------------------------

impl ModelLookup for ModelManifest {
    fn provider(&self) -> AsrProviderKind {
        self.provider
            .expect("ModelLookup::provider called on a non-ASR manifest (e.g. VAD)")
    }

    fn model_id(&self) -> &str {
        self.id
    }

    fn supports_language(&self, language: &AsrLanguage) -> bool {
        self.supported_languages.contains(language)
    }
}

// ===========================================================================
// Shared source constants
// ===========================================================================

const SHERPA_ONNX_REPO: &str = "https://github.com/k2-fsa/sherpa-onnx";
const SHERPA_ONNX_COMMIT: &str = "3dc7c569f31ca2cd4a20ed6f7db780327e6714c5";

// ===========================================================================
// Static manifest entries
// ===========================================================================

/// SenseVoiceSmall INT8 model for Chinese, English, Japanese, Korean, and
/// Cantonese. Trained by Alibaba FunASR team, converted by k2-fsa to ONNX
/// format with INT8 quantization for reduced memory usage.
pub static SENSE_VOICE_SMALL_INT8: ModelManifest = ModelManifest {
    id: "sense-voice-small-int8-2024-07-17",
    display_name: "SenseVoiceSmall INT8",
    provider: Some(AsrProviderKind::SenseVoice),
    manifest_version: "1",
    archive_size_bytes: 163_002_883,
    // sha256 verified against upstream release archive
    archive_sha256: "7d1efa2138a65b0b488df37f8b89e3d91a60676e416f515b952358d83dfd347e",
    required_files: &[
        RequiredFile {
            path: "model.int8.onnx",
            sha256: None,
        },
        RequiredFile {
            path: "tokens.txt",
            sha256: None,
        },
    ],
    source: ModelSource {
        upstream_repo: "https://github.com/modelscope/FunASR",
        upstream_commit_or_tag: "v1.2.3",
        original_model_id: "iic/SenseVoiceSmall",
        conversion_tool_repo: SHERPA_ONNX_REPO,
        conversion_tool_commit: SHERPA_ONNX_COMMIT,
        conversion_params: "INT8 quantization, ONNX opset 17",
        download_url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17.tar.bz2",
        license: "MIT",
        license_url: "https://github.com/modelscope/FunASR/blob/main/LICENSE",
    },
    supported_languages: &[
        AsrLanguage::Zh,
        AsrLanguage::En,
        AsrLanguage::Ja,
        AsrLanguage::Ko,
        AsrLanguage::Yue,
    ],
};

/// Whisper Tiny model — smallest Whisper variant, suitable for quick
/// validation and low-resource environments. 39M parameters, ~111 MB ONNX.
pub static WHISPER_TINY: ModelManifest = ModelManifest {
    id: "whisper-tiny",
    display_name: "Whisper Tiny",
    provider: Some(AsrProviderKind::Whisper),
    manifest_version: "1",
    archive_size_bytes: 116_204_861,
    // sha256 placeholder — computed from downloaded archive
    archive_sha256: "c46116994e539aa165266d96b325252728429c12535eb9d8b6a2b10f129e66b1",
    required_files: &[
        RequiredFile {
            path: "tiny-encoder.onnx",
            sha256: None,
        },
        RequiredFile {
            path: "tiny-decoder.onnx",
            sha256: None,
        },
        RequiredFile {
            path: "tiny-tokens.txt",
            sha256: None,
        },
    ],
    source: ModelSource {
        upstream_repo: "https://github.com/openai/whisper",
        upstream_commit_or_tag: "v20231117",
        original_model_id: "tiny",
        conversion_tool_repo: SHERPA_ONNX_REPO,
        conversion_tool_commit: SHERPA_ONNX_COMMIT,
        conversion_params: "ONNX export, sherpa-onnx packaging",
        download_url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-tiny.tar.bz2",
        license: "MIT",
        license_url: "https://github.com/openai/whisper/blob/main/LICENSE",
    },
    supported_languages: &[
        AsrLanguage::Auto,
        AsrLanguage::En,
        AsrLanguage::Zh,
        AsrLanguage::Ja,
        AsrLanguage::Ko,
        AsrLanguage::De,
        AsrLanguage::Fr,
        AsrLanguage::Es,
        AsrLanguage::It,
        AsrLanguage::Pt,
        AsrLanguage::Ru,
        AsrLanguage::Nl,
        AsrLanguage::Pl,
        AsrLanguage::Tr,
        AsrLanguage::Ar,
        AsrLanguage::Hi,
        AsrLanguage::Vi,
        AsrLanguage::Th,
        AsrLanguage::Uk,
    ],
};

/// Whisper Base model — balanced quality/speed trade-off. 74M parameters,
/// ~198 MB ONNX. Recommended default for Whisper.
pub static WHISPER_BASE: ModelManifest = ModelManifest {
    id: "whisper-base",
    display_name: "Whisper Base",
    provider: Some(AsrProviderKind::Whisper),
    manifest_version: "1",
    archive_size_bytes: 207_557_382,
    // sha256 placeholder — computed from downloaded archive
    archive_sha256: "911b2083efd7c0dca2ac3b358b75222660dc09fb716d64fbfc417ba6c99ff3de",
    required_files: &[
        RequiredFile {
            path: "base-encoder.onnx",
            sha256: None,
        },
        RequiredFile {
            path: "base-decoder.onnx",
            sha256: None,
        },
        RequiredFile {
            path: "base-tokens.txt",
            sha256: None,
        },
    ],
    source: ModelSource {
        upstream_repo: "https://github.com/openai/whisper",
        upstream_commit_or_tag: "v20231117",
        original_model_id: "base",
        conversion_tool_repo: SHERPA_ONNX_REPO,
        conversion_tool_commit: SHERPA_ONNX_COMMIT,
        conversion_params: "ONNX export, sherpa-onnx packaging",
        download_url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-base.tar.bz2",
        license: "MIT",
        license_url: "https://github.com/openai/whisper/blob/main/LICENSE",
    },
    supported_languages: &[
        AsrLanguage::Auto,
        AsrLanguage::En,
        AsrLanguage::Zh,
        AsrLanguage::Ja,
        AsrLanguage::Ko,
        AsrLanguage::De,
        AsrLanguage::Fr,
        AsrLanguage::Es,
        AsrLanguage::It,
        AsrLanguage::Pt,
        AsrLanguage::Ru,
        AsrLanguage::Nl,
        AsrLanguage::Pl,
        AsrLanguage::Tr,
        AsrLanguage::Ar,
        AsrLanguage::Hi,
        AsrLanguage::Vi,
        AsrLanguage::Th,
        AsrLanguage::Uk,
    ],
};

/// Whisper Small model — higher quality, more resource-intensive. 244M
/// parameters, ~610 MB ONNX. Use when accuracy is more important than speed.
pub static WHISPER_SMALL: ModelManifest = ModelManifest {
    id: "whisper-small",
    display_name: "Whisper Small",
    provider: Some(AsrProviderKind::Whisper),
    manifest_version: "1",
    archive_size_bytes: 639_387_718,
    // sha256 placeholder — computed from downloaded archive
    archive_sha256: "0000000000000000000000000000000000000000000000000000000000000002",
    required_files: &[
        RequiredFile {
            path: "small-encoder.onnx",
            sha256: None,
        },
        RequiredFile {
            path: "small-decoder.onnx",
            sha256: None,
        },
        RequiredFile {
            path: "small-tokens.txt",
            sha256: None,
        },
    ],
    source: ModelSource {
        upstream_repo: "https://github.com/openai/whisper",
        upstream_commit_or_tag: "v20231117",
        original_model_id: "small",
        conversion_tool_repo: SHERPA_ONNX_REPO,
        conversion_tool_commit: SHERPA_ONNX_COMMIT,
        conversion_params: "ONNX export, sherpa-onnx packaging",
        download_url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-small.tar.bz2",
        license: "MIT",
        license_url: "https://github.com/openai/whisper/blob/main/LICENSE",
    },
    supported_languages: &[
        AsrLanguage::Auto,
        AsrLanguage::En,
        AsrLanguage::Zh,
        AsrLanguage::Ja,
        AsrLanguage::Ko,
        AsrLanguage::De,
        AsrLanguage::Fr,
        AsrLanguage::Es,
        AsrLanguage::It,
        AsrLanguage::Pt,
        AsrLanguage::Ru,
        AsrLanguage::Nl,
        AsrLanguage::Pl,
        AsrLanguage::Tr,
        AsrLanguage::Ar,
        AsrLanguage::Hi,
        AsrLanguage::Vi,
        AsrLanguage::Th,
        AsrLanguage::Uk,
    ],
};

/// Silero VAD (Voice Activity Detection) model. Used to detect speech
/// intervals before transcription. This is a single ONNX file, not an
/// ASR model, so `provider` is None.
pub static SILERO_VAD: ModelManifest = ModelManifest {
    id: "silero-vad",
    display_name: "Silero VAD",
    provider: None,
    manifest_version: "1",
    archive_size_bytes: 643_854,
    // sha256 verified against upstream release
    archive_sha256: "9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6",
    required_files: &[RequiredFile {
        path: "silero_vad.onnx",
        sha256: Some("9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6"),
    }],
    source: ModelSource {
        upstream_repo: "https://github.com/snakers4/silero-vad",
        upstream_commit_or_tag: "v5.1",
        original_model_id: "silero_vad",
        conversion_tool_repo: SHERPA_ONNX_REPO,
        conversion_tool_commit: SHERPA_ONNX_COMMIT,
        conversion_params: "ONNX export, sherpa-onnx packaging",
        download_url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/silero_vad.onnx",
        license: "MIT",
        license_url: "https://github.com/snakers4/silero-vad/blob/master/LICENSE",
    },
    supported_languages: &[],
};

// ---------------------------------------------------------------------------
// Registry helpers
// ---------------------------------------------------------------------------

/// Returns all five static manifest entries (4 ASR models + 1 VAD).
pub fn all_manifests() -> &'static [&'static ModelManifest] {
    static ALL: &[&ModelManifest] = &[
        &SENSE_VOICE_SMALL_INT8,
        &WHISPER_TINY,
        &WHISPER_BASE,
        &WHISPER_SMALL,
        &SILERO_VAD,
    ];
    ALL
}

/// Finds a manifest by its stable model ID.
pub fn find_by_id(id: &str) -> Option<&'static ModelManifest> {
    all_manifests()
        .iter()
        .find(|m| m.id == id)
        .copied()
}
