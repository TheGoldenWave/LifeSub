//! ASR quality metrics for the release Gate.
//!
//! Implements the approved metric protocol from the design spec:
//!
//! 1. NFKC + lowercase Latin normalization
//! 2. CER: remove punctuation/whitespace, tokenize by grapheme clusters
//! 3. WER: punctuation → spaces, collapse whitespace, split by space
//! 4. Key phrases: normalized contiguous token subsequence
//! 5. Text metrics use all segments sorted by session_start_ms
//! 6. Boundary errors: segment count must match, pair by time order

use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Expected threshold for SenseVoice Chinese CER.
pub const SENSEVOICE_CER_MAX: f64 = 0.20;

/// Expected threshold for Whisper English WER.
pub const WHISPER_WER_MAX: f64 = 0.20;

/// Expected median boundary error in milliseconds.
pub const MEDIAN_BOUNDARY_ERROR_MAX_MS: u64 = 500;

/// Expected maximum boundary error in milliseconds.
pub const MAX_BOUNDARY_ERROR_MAX_MS: u64 = 1500;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A predicted segment with text and timing.
#[derive(Clone, Debug)]
pub struct PredictedSegment {
    /// The transcribed text for this segment.
    pub text: String,
    /// Segment start time in milliseconds (session-relative).
    pub start_ms: u64,
    /// Segment end time in milliseconds (session-relative).
    pub end_ms: u64,
}

/// A ground-truth segment from the fixture manifest.
#[derive(Clone, Debug)]
pub struct GroundTruthSegment {
    /// The expected text for this segment.
    pub text: String,
    /// Expected start time in milliseconds.
    pub start_ms: u64,
    /// Expected end time in milliseconds.
    pub end_ms: u64,
}

/// Computed metrics for a single fixture/provider pair.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct FixtureMetrics {
    /// The fixture ID from the manifest.
    pub fixture_id: String,
    /// The provider kind that produced the result.
    pub provider: String,
    /// The model ID used.
    pub model_id: String,
    /// Whether the segment count matched the ground truth.
    pub segment_count_match: bool,
    /// Predicted segment count.
    pub predicted_segments: usize,
    /// Expected segment count.
    pub expected_segments: usize,
    /// Character Error Rate (0.0 to 1.0, or higher for poor matches).
    pub cer: f64,
    /// Word Error Rate (0.0 to 1.0, or higher for poor matches).
    pub wer: f64,
    /// Whether all key phrases were found (true if no key phrases defined).
    pub all_key_phrases_present: bool,
    /// Key phrases that were found.
    pub key_phrases_found: Vec<String>,
    /// Key phrases that were missing.
    pub key_phrases_missing: Vec<String>,
    /// Median boundary error in milliseconds (None if segment count mismatch).
    pub median_boundary_error_ms: Option<u64>,
    /// Maximum boundary error in milliseconds (None if segment count mismatch).
    pub max_boundary_error_ms: Option<u64>,
    /// Whether the CER threshold was met.
    pub cer_pass: bool,
    /// Whether the WER threshold was met.
    pub wer_pass: bool,
    /// Whether the key phrase threshold was met.
    pub key_phrase_pass: bool,
    /// Whether the boundary error thresholds were met.
    pub boundary_pass: bool,
    /// Whether all metrics passed.
    pub all_pass: bool,
}

/// A single fixture entry for the real ASR Gate.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct GateFixture {
    /// Unique fixture identifier.
    pub id: String,
    /// Path to the audio file relative to the project root.
    pub path: String,
    /// SHA-256 hex digest of the audio file (null until file is frozen).
    pub sha256: Option<String>,
    /// The language of the speech content.
    pub language: String,
    /// The expected transcript text (full text, all segments joined).
    pub expected_transcript: String,
    /// Expected speech segments with timing.
    pub expected_segments: Vec<GateFixtureSegment>,
    /// Key phrases that must appear in the transcript.
    pub key_phrases: Vec<String>,
    /// Which provider(s) to test against.
    pub test_providers: Vec<String>,
    /// Expected CER threshold (for SenseVoice).
    pub cer_max: Option<f64>,
    /// Expected WER threshold (for Whisper).
    pub wer_max: Option<f64>,
}

/// A ground-truth segment inside a Gate fixture.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct GateFixtureSegment {
    /// Expected transcript text for this segment.
    pub text: String,
    /// Expected start time in milliseconds.
    pub start_ms: u64,
    /// Expected end time in milliseconds.
    pub end_ms: u64,
}

/// The top-level fixture manifest.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct GateManifest {
    /// Human-readable description.
    pub description: String,
    /// SPDX license identifier.
    pub license: String,
    /// Source provenance.
    pub source: String,
    /// Real speech fixtures for ASR quality evaluation.
    #[serde(default)]
    pub gate_fixtures: Vec<GateFixture>,
}

// ---------------------------------------------------------------------------
// Text normalization
// ---------------------------------------------------------------------------

/// Apply NFKC normalization and lowercase all ASCII Latin letters.
///
/// This is the first step of the approved metric protocol.
/// Non-ASCII characters are preserved; only A-Z letters are lowercased.
pub fn normalize_text(text: &str) -> String {
    let nfkc: String = text.nfkc().collect();
    // Lowercase only ASCII Latin letters; preserve other scripts
    nfkc
        .chars()
        .map(|c| {
            if c.is_ascii_uppercase() {
                c.to_ascii_lowercase()
            } else {
                c
            }
        })
        .collect()
}

/// Remove Unicode punctuation and all whitespace, then tokenize into
/// grapheme clusters. Used for CER computation.
pub fn cer_tokens(text: &str) -> Vec<String> {
    let normalized = normalize_text(text);
    let no_punct: String = normalized
        .chars()
        .filter(|c| !c.is_ascii_punctuation() && !c.is_whitespace())
        .collect();
    no_punct
        .graphemes(true)
        .map(|g| g.to_string())
        .collect()
}

/// Replace Unicode punctuation with spaces, collapse consecutive whitespace,
/// and split into word tokens. Used for WER computation.
pub fn wer_tokens(text: &str) -> Vec<String> {
    let normalized = normalize_text(text);
    let spaced: String = normalized
        .chars()
        .map(|c| {
            if c.is_ascii_punctuation() {
                ' '
            } else {
                c
            }
        })
        .collect();
    spaced
        .split_whitespace()
        .map(|s| s.to_string())
        .collect()
}

/// Normalize text for key phrase matching: NFKC, lowercase, punctuation→spaces,
/// whitespace collapse. Then tokenize into grapheme clusters for language-agnostic
/// subsequence matching (works for both space-separated languages and CJK).
pub fn key_phrase_tokens(text: &str) -> Vec<String> {
    let normalized = normalize_text(text);
    // Replace punctuation with spaces, then collapse whitespace
    let spaced: String = normalized
        .chars()
        .map(|c| {
            if c.is_ascii_punctuation() {
                ' '
            } else {
                c
            }
        })
        .collect();
    // Collapse whitespace: filter out spaces and keep only non-whitespace
    // grapheme clusters. This preserves word boundaries for English while
    // allowing CJK character-level matching.
    let no_spaces: String = spaced.chars().filter(|c| !c.is_whitespace()).collect();
    no_spaces
        .graphemes(true)
        .map(|g| g.to_string())
        .collect()
}

// ---------------------------------------------------------------------------
// Levenshtein distance (edit distance)
// ---------------------------------------------------------------------------

/// Compute the Levenshtein distance between two sequences of tokens.
///
/// Uses the standard dynamic programming algorithm with O(n*m) time and
/// O(min(n,m)) space. Returns the minimum number of insertions, deletions,
/// or substitutions to transform `a` into `b`.
pub fn levenshtein_distance<T: Eq>(a: &[T], b: &[T]) -> usize {
    let n = a.len();
    let m = b.len();

    if n == 0 {
        return m;
    }
    if m == 0 {
        return n;
    }

    // Ensure b is the shorter sequence for space optimization
    let (a, b) = if n < m { (b, a) } else { (a, b) };
    let n = a.len();
    let m = b.len();

    let mut prev: Vec<usize> = (0..=m).collect();
    let mut curr = vec![0usize; m + 1];

    for i in 1..=n {
        curr[0] = i;
        for j in 1..=m {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1) // deletion
                .min(curr[j - 1] + 1) // insertion
                .min(prev[j - 1] + cost); // substitution
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[m]
}

// ---------------------------------------------------------------------------
// CER: Character Error Rate
// ---------------------------------------------------------------------------

/// Compute the Character Error Rate using grapheme-cluster tokens.
///
/// CER = edit_distance / reference_length.
/// Returns a value in [0.0, ∞); 0.0 is perfect, higher is worse.
pub fn compute_cer(reference: &str, hypothesis: &str) -> f64 {
    let ref_tokens = cer_tokens(reference);
    let hyp_tokens = cer_tokens(hypothesis);

    if ref_tokens.is_empty() {
        return if hyp_tokens.is_empty() { 0.0 } else { f64::INFINITY };
    }

    let distance = levenshtein_distance(&ref_tokens, &hyp_tokens);
    distance as f64 / ref_tokens.len() as f64
}

// ---------------------------------------------------------------------------
// WER: Word Error Rate
// ---------------------------------------------------------------------------

/// Compute the Word Error Rate using space-delimited tokens.
///
/// WER = edit_distance / reference_length.
/// Returns a value in [0.0, ∞); 0.0 is perfect, higher is worse.
pub fn compute_wer(reference: &str, hypothesis: &str) -> f64 {
    let ref_tokens = wer_tokens(reference);
    let hyp_tokens = wer_tokens(hypothesis);

    if ref_tokens.is_empty() {
        return if hyp_tokens.is_empty() { 0.0 } else { f64::INFINITY };
    }

    let distance = levenshtein_distance(&ref_tokens, &hyp_tokens);
    distance as f64 / ref_tokens.len() as f64
}

// ---------------------------------------------------------------------------
// Key phrase detection
// ---------------------------------------------------------------------------

/// Check whether all key phrases appear as contiguous token subsequences
/// in the hypothesis text.
///
/// Returns (found_phrases, missing_phrases).
pub fn check_key_phrases(
    hypothesis: &str,
    key_phrases: &[String],
) -> (Vec<String>, Vec<String>) {
    let hyp_tokens = key_phrase_tokens(hypothesis);
    let mut found = Vec::new();
    let mut missing = Vec::new();

    for phrase in key_phrases {
        let phrase_tokens = key_phrase_tokens(phrase);
        if phrase_tokens.is_empty() {
            found.push(phrase.clone());
            continue;
        }
        if is_contiguous_subsequence(&hyp_tokens, &phrase_tokens) {
            found.push(phrase.clone());
        } else {
            missing.push(phrase.clone());
        }
    }

    (found, missing)
}

/// Check whether `needle` appears as a contiguous subsequence in `haystack`.
fn is_contiguous_subsequence<T: Eq>(haystack: &[T], needle: &[T]) -> bool {
    if needle.is_empty() {
        return true;
    }
    if needle.len() > haystack.len() {
        return false;
    }
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

// ---------------------------------------------------------------------------
// Boundary errors
// ---------------------------------------------------------------------------

/// Compute median and maximum absolute boundary errors between predicted
/// and ground-truth segments, paired by time order.
///
/// Returns None if the segment counts don't match.
pub fn compute_boundary_errors(
    predicted: &[PredictedSegment],
    expected: &[GroundTruthSegment],
) -> Option<(u64, u64)> {
    if predicted.len() != expected.len() {
        return None;
    }

    let mut errors: Vec<u64> = Vec::with_capacity(predicted.len() * 2);

    for (pred, exp) in predicted.iter().zip(expected.iter()) {
        let start_err = if pred.start_ms >= exp.start_ms {
            pred.start_ms - exp.start_ms
        } else {
            exp.start_ms - pred.start_ms
        };
        let end_err = if pred.end_ms >= exp.end_ms {
            pred.end_ms - exp.end_ms
        } else {
            exp.end_ms - pred.end_ms
        };
        errors.push(start_err);
        errors.push(end_err);
    }

    errors.sort_unstable();

    let median = if errors.len() % 2 == 0 {
        let mid = errors.len() / 2;
        (errors[mid - 1] + errors[mid]) / 2
    } else {
        errors[errors.len() / 2]
    };

    let max = *errors.last().unwrap_or(&0);

    Some((median, max))
}

// ---------------------------------------------------------------------------
// Full metric computation
// ---------------------------------------------------------------------------

/// Compute all metrics for a set of predicted segments against a fixture.
pub fn compute_metrics(
    fixture: &GateFixture,
    predicted_segments: &[PredictedSegment],
    provider: &str,
    model_id: &str,
) -> FixtureMetrics {
    // Sort predicted segments by start_ms for text concatenation
    let mut sorted_pred = predicted_segments.to_vec();
    sorted_pred.sort_by_key(|s| s.start_ms);

    let predicted_text: String = sorted_pred
        .iter()
        .map(|s| s.text.as_str())
        .collect::<Vec<&str>>()
        .join(" ");

    let expected_text = &fixture.expected_transcript;

    let cer = compute_cer(expected_text, &predicted_text);
    let wer = compute_wer(expected_text, &predicted_text);

    let (key_phrases_found, key_phrases_missing) =
        check_key_phrases(&predicted_text, &fixture.key_phrases);
    let all_key_phrases_present = key_phrases_missing.is_empty();

    let segment_count_match = predicted_segments.len() == fixture.expected_segments.len();

    let expected_segments: Vec<GroundTruthSegment> = fixture
        .expected_segments
        .iter()
        .map(|s| GroundTruthSegment {
            text: s.text.clone(),
            start_ms: s.start_ms,
            end_ms: s.end_ms,
        })
        .collect();

    let (median_boundary_error_ms, max_boundary_error_ms) = if segment_count_match {
        let sorted_expected = {
            let mut segs = expected_segments.clone();
            segs.sort_by_key(|s| s.start_ms);
            segs
        };
        compute_boundary_errors(&sorted_pred, &sorted_expected)
            .map(|(m, x)| (Some(m), Some(x)))
            .unwrap_or((None, None))
    } else {
        (None, None)
    };

    let cer_pass = cer <= fixture.cer_max.unwrap_or(SENSEVOICE_CER_MAX);
    let wer_pass = wer <= fixture.wer_max.unwrap_or(WHISPER_WER_MAX);
    let key_phrase_pass = all_key_phrases_present;
    let boundary_pass = segment_count_match
        && median_boundary_error_ms.map_or(false, |m| m <= MEDIAN_BOUNDARY_ERROR_MAX_MS)
        && max_boundary_error_ms.map_or(false, |x| x <= MAX_BOUNDARY_ERROR_MAX_MS);

    let all_pass = cer_pass && wer_pass && key_phrase_pass && boundary_pass;

    FixtureMetrics {
        fixture_id: fixture.id.clone(),
        provider: provider.to_string(),
        model_id: model_id.to_string(),
        segment_count_match,
        predicted_segments: predicted_segments.len(),
        expected_segments: fixture.expected_segments.len(),
        cer,
        wer,
        all_key_phrases_present,
        key_phrases_found,
        key_phrases_missing,
        median_boundary_error_ms,
        max_boundary_error_ms,
        cer_pass,
        wer_pass,
        key_phrase_pass,
        boundary_pass,
        all_pass,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Text normalization
    // -----------------------------------------------------------------------

    #[test]
    fn normalize_lowercases_ascii() {
        assert_eq!(normalize_text("HELLO World"), "hello world");
    }

    #[test]
    fn normalize_preserves_non_ascii() {
        assert_eq!(normalize_text("你好世界"), "你好世界");
    }

    #[test]
    fn normalize_applies_nfkc() {
        // Fullwidth 'A' (U+FF21) should become normal 'a' after NFKC + lowercase
        let fullwidth_a = "\u{FF21}";
        let result = normalize_text(fullwidth_a);
        assert_eq!(result, "a");
    }

    // -----------------------------------------------------------------------
    // CER tokens
    // -----------------------------------------------------------------------

    #[test]
    fn cer_tokens_remove_punctuation() {
        let tokens = cer_tokens("Hello, world!");
        // After NFKC + lowercase: "hello, world!"
        // After removing punctuation/whitespace: "helloworld"
        assert_eq!(tokens.join(""), "helloworld");
    }

    #[test]
    fn cer_tokens_grapheme_clusters() {
        // e with acute accent is a single grapheme cluster
        let tokens = cer_tokens("café");
        assert_eq!(tokens.len(), 4); // c, a, f, é
    }

    #[test]
    fn cer_tokens_chinese_characters() {
        let tokens = cer_tokens("你好，世界！");
        // After removing punctuation: "你好世界"
        assert_eq!(tokens.len(), 4); // 你, 好, 世, 界
    }

    // -----------------------------------------------------------------------
    // WER tokens
    // -----------------------------------------------------------------------

    #[test]
    fn wer_tokens_replace_punctuation_with_spaces() {
        let tokens = wer_tokens("Hello, world!");
        assert_eq!(tokens, vec!["hello", "world"]);
    }

    #[test]
    fn wer_tokens_collapse_whitespace() {
        let tokens = wer_tokens("Hello,  world!!!  ");
        assert_eq!(tokens, vec!["hello", "world"]);
    }

    #[test]
    fn wer_tokens_chinese_mixed() {
        let tokens = wer_tokens("你好，world！");
        // NFKC + lowercase: "你好，world！"
        // Punctuation → spaces: "你好  world  "
        // Split + collapse: ["你好", "world"]
        assert_eq!(tokens, vec!["你好", "world"]);
    }

    // -----------------------------------------------------------------------
    // Levenshtein distance
    // -----------------------------------------------------------------------

    #[test]
    fn levenshtein_identical() {
        assert_eq!(levenshtein_distance(&[1, 2, 3], &[1, 2, 3]), 0);
    }

    #[test]
    fn levenshtein_substitution() {
        assert_eq!(levenshtein_distance(&[1, 2, 3], &[1, 4, 3]), 1);
    }

    #[test]
    fn levenshtein_insertion() {
        assert_eq!(levenshtein_distance(&[1, 2], &[1, 2, 3]), 1);
    }

    #[test]
    fn levenshtein_deletion() {
        assert_eq!(levenshtein_distance(&[1, 2, 3], &[1, 2]), 1);
    }

    #[test]
    fn levenshtein_empty() {
        assert_eq!(levenshtein_distance::<i32>(&[], &[]), 0);
        assert_eq!(levenshtein_distance(&[1, 2], &[]), 2);
        assert_eq!(levenshtein_distance::<i32>(&[], &[1, 2]), 2);
    }

    // -----------------------------------------------------------------------
    // CER
    // -----------------------------------------------------------------------

    #[test]
    fn cer_perfect_match() {
        assert_eq!(compute_cer("hello", "hello"), 0.0);
    }

    #[test]
    fn cer_single_substitution() {
        let cer = compute_cer("hello", "hallo");
        assert!((cer - 0.2).abs() < 0.01); // 1 substitution / 5 chars
    }

    #[test]
    fn cer_empty_reference() {
        assert!(compute_cer("", "hello").is_infinite());
    }

    #[test]
    fn cer_both_empty() {
        assert_eq!(compute_cer("", ""), 0.0);
    }

    // -----------------------------------------------------------------------
    // WER
    // -----------------------------------------------------------------------

    #[test]
    fn wer_perfect_match() {
        assert_eq!(compute_wer("hello world", "hello world"), 0.0);
    }

    #[test]
    fn wer_single_substitution() {
        let wer = compute_wer("hello world", "hello earth");
        assert!((wer - 0.5).abs() < 0.01); // 1 substitution / 2 words
    }

    #[test]
    fn wer_empty_reference() {
        assert!(compute_wer("", "hello world").is_infinite());
    }

    // -----------------------------------------------------------------------
    // Key phrases
    // -----------------------------------------------------------------------

    #[test]
    fn key_phrase_found() {
        let (found, missing) = check_key_phrases(
            "hello world today is sunny",
            &["hello world".to_string(), "is sunny".to_string()],
        );
        assert_eq!(found.len(), 2);
        assert!(missing.is_empty());
    }

    #[test]
    fn key_phrase_not_contiguous() {
        let (found, missing) = check_key_phrases(
            "hello world today",
            &["hello today".to_string()], // not contiguous
        );
        assert!(found.is_empty());
        assert_eq!(missing.len(), 1);
    }

    #[test]
    fn key_phrase_case_insensitive() {
        let (found, missing) = check_key_phrases(
            "HELLO World",
            &["hello world".to_string()],
        );
        assert_eq!(found.len(), 1);
        assert!(missing.is_empty());
    }

    #[test]
    fn key_phrase_punctuation_insensitive() {
        let (found, missing) = check_key_phrases(
            "Hello, world!",
            &["hello world".to_string()],
        );
        assert_eq!(found.len(), 1);
        assert!(missing.is_empty());
    }

    // -----------------------------------------------------------------------
    // Boundary errors
    // -----------------------------------------------------------------------

    #[test]
    fn boundary_errors_perfect_match() {
        let predicted = vec![PredictedSegment {
            text: "hello".to_string(),
            start_ms: 1000,
            end_ms: 2000,
        }];
        let expected = vec![GroundTruthSegment {
            text: "hello".to_string(),
            start_ms: 1000,
            end_ms: 2000,
        }];
        let (median, max) = compute_boundary_errors(&predicted, &expected).unwrap();
        assert_eq!(median, 0);
        assert_eq!(max, 0);
    }

    #[test]
    fn boundary_errors_with_offset() {
        let predicted = vec![
            PredictedSegment {
                text: "a".to_string(),
                start_ms: 1050,
                end_ms: 2050,
            },
            PredictedSegment {
                text: "b".to_string(),
                start_ms: 2950,
                end_ms: 4050,
            },
        ];
        let expected = vec![
            GroundTruthSegment {
                text: "a".to_string(),
                start_ms: 1000,
                end_ms: 2000,
            },
            GroundTruthSegment {
                text: "b".to_string(),
                start_ms: 3000,
                end_ms: 4000,
            },
        ];
        let (median, max) = compute_boundary_errors(&predicted, &expected).unwrap();
        // Errors: 50, 50, 50, 50 → median = 50, max = 50
        assert_eq!(median, 50);
        assert_eq!(max, 50);
    }

    #[test]
    fn boundary_errors_mismatched_count() {
        let predicted = vec![PredictedSegment {
            text: "a".to_string(),
            start_ms: 1000,
            end_ms: 2000,
        }];
        let expected = vec![
            GroundTruthSegment {
                text: "a".to_string(),
                start_ms: 1000,
                end_ms: 2000,
            },
            GroundTruthSegment {
                text: "b".to_string(),
                start_ms: 3000,
                end_ms: 4000,
            },
        ];
        assert!(compute_boundary_errors(&predicted, &expected).is_none());
    }

    // -----------------------------------------------------------------------
    // Full metrics
    // -----------------------------------------------------------------------

    #[test]
    fn compute_metrics_all_pass() {
        let fixture = GateFixture {
            id: "test-fixture".to_string(),
            path: "test.wav".to_string(),
            sha256: None,
            language: "zh".to_string(),
            expected_transcript: "你好世界".to_string(),
            expected_segments: vec![GateFixtureSegment {
                text: "你好世界".to_string(),
                start_ms: 0,
                end_ms: 2000,
            }],
            key_phrases: vec!["你好".to_string()],
            test_providers: vec!["sense_voice".to_string()],
            cer_max: Some(0.20),
            wer_max: Some(0.20),
        };

        let predicted = vec![PredictedSegment {
            text: "你好世界".to_string(),
            start_ms: 100,
            end_ms: 1800,
        }];

        let metrics = compute_metrics(&fixture, &predicted, "sense_voice", "test-model");
        assert!(metrics.all_pass);
        assert!(metrics.cer_pass);
        assert!(metrics.wer_pass);
        assert!(metrics.key_phrase_pass);
        assert!(metrics.boundary_pass);
    }
}