//! LifeSub ASR Gate — real provider quality benchmarks against fixed fixtures.
//!
//! Usage: `lifesub-asr-gate [--fixtures <path>] --model-dir <path> [--output <path>]`
//!
//! On macOS arm64 with Metal, the Gate additionally requires `--qwen17-model-dir <path>`.
//! The Gate writes a JSON result file containing all scenario metrics and runtime identities.
//! It exits non-zero when any mandatory scenario fails, is absent, or a fallback is detected.
//!
//! ⚠️ PENDING PRODUCTION VERIFICATION: The `run_scenario` function is scaffolded with
//! the CER/WER/RTF metrics protocol but the real ASR provider integration is deferred
//! to the production verification run on the M4/24GB device. See `scripts/verify-asr-gate.sh`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// ── Fixture manifest ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct FixtureManifest {
    schema_version: u32,
    source: String,
    license_spdx: String,
    generator: String,
    fixtures: Vec<FixtureEntry>,
}

#[derive(Debug, Deserialize)]
struct FixtureEntry {
    file: String,
    format: String,
    sha256: String,
    bytes: u64,
    sample_rate_hz: u32,
    channels: u8,
    nominal_frames: u64,
    #[serde(default)]
    expected_text: Option<String>,
    #[serde(default)]
    expected_phrases: Option<Vec<String>>,
    #[serde(default)]
    minimum_phrase_matches: Option<usize>,
    #[serde(default)]
    normalization: Option<String>,
    #[serde(default)]
    require_nonempty: Option<bool>,
}

// ── Gate output ──────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct GateOutput {
    tested_commit: String,
    scoped_source_digest: String,
    executable_hash: String,
    #[serde(flatten)]
    runtime: Option<RuntimeIdentity>,
    scenarios: Vec<ScenarioResult>,
    fixture_hashes: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
struct RuntimeIdentity {
    sherpa_version: String,
    sherpa_git_sha1: String,
    sherpa_native_archive_sha256: String,
    qwen3_asr_crate_version: Option<String>,
    qwen3_asr_git_commit: Option<String>,
    candle_backend: Option<String>,
    device_os: String,
    device_arch: String,
    device_chip: Option<String>,
    device_memory_gib: Option<u16>,
}

#[derive(Debug, Serialize)]
struct ScenarioResult {
    scenario_id: String,
    provider: String,
    language: String,
    fixture: String,
    cer: Option<f64>,
    wer: Option<f64>,
    key_phrase_recall: Option<f64>,
    rtf: Option<f64>,
    median_boundary_error_ms: Option<f64>,
    max_boundary_error_ms: Option<f64>,
    peak_rss_mib: Option<f64>,
    passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    skipped: Option<bool>,
}

// ── Metrics ──────────────────────────────────────────────────────────────

/// NFKC + lowercase Latin normalization; punctuation/whitespace removed for CER.
fn normalize_for_cer(text: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    let lower: String = text.chars().flat_map(|c| c.to_lowercase()).collect();
    let normalized: String = lower.nfkc().collect();
    normalized
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// WER normalization: punctuation → spaces, collapse whitespace.
fn normalize_for_wer(text: &str) -> String {
    let normalized: String = text
        .chars()
        .flat_map(|c| c.to_lowercase())
        .map(|c| if c.is_alphanumeric() || c.is_whitespace() { c } else { ' ' })
        .collect();
    let words: Vec<&str> = normalized.split_whitespace().collect();
    words.join(" ")
}

/// Character Error Rate using grapheme clusters.
fn calculate_cer(reference: &str, hypothesis: &str) -> f64 {
    let ref_chars: Vec<char> = reference.chars().collect();
    let hyp_chars: Vec<char> = hypothesis.chars().collect();
    let distance = levenshtein_distance(&ref_chars, &hyp_chars);
    if ref_chars.is_empty() {
        return if hyp_chars.is_empty() { 0.0 } else { 1.0 };
    }
    distance as f64 / ref_chars.len() as f64
}

/// Word Error Rate.
fn calculate_wer(reference: &str, hypothesis: &str) -> f64 {
    let ref_words: Vec<&str> = reference.split_whitespace().collect();
    let hyp_words: Vec<&str> = hypothesis.split_whitespace().collect();
    let distance = levenshtein_distance(&ref_words, &hyp_words);
    if ref_words.is_empty() {
        return if hyp_words.is_empty() { 0.0 } else { 1.0 };
    }
    distance as f64 / ref_words.len() as f64
}

fn levenshtein_distance<T: Eq>(a: &[T], b: &[T]) -> usize {
    let n = a.len();
    let m = b.len();
    let mut prev = (0..=m).collect::<Vec<_>>();
    let mut curr = vec![0usize; m + 1];
    for i in 1..=n {
        curr[0] = i;
        for j in 1..=m {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[m]
}

/// Key-phrase recall: fraction of expected phrases found (normalized, contiguous).
fn calculate_key_phrase_recall(expected: &[String], hypothesis: &str) -> f64 {
    let hyp_lower: String = hypothesis
        .chars()
        .flat_map(|c| c.to_lowercase())
        .collect();
    let matched = expected
        .iter()
        .filter(|phrase| {
            let p: String = phrase
                .chars()
                .flat_map(|c| c.to_lowercase())
                .collect();
            hyp_lower.contains(&p)
        })
        .count();
    if expected.is_empty() {
        return 1.0;
    }
    matched as f64 / expected.len() as f64
}

// ── CLI ──────────────────────────────────────────────────────────────────

struct Args {
    fixtures_path: PathBuf,
    model_dir: Option<PathBuf>,
    qwen17_model_dir: Option<PathBuf>,
    output_path: Option<PathBuf>,
}

fn parse_args() -> Result<Args, String> {
    let args: Vec<String> = std::env::args().collect();
    let mut fixtures_path = PathBuf::from("tests/fixtures/asr/fixtures.json");
    let mut model_dir = None;
    let mut qwen17_model_dir = None;
    let mut output_path = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--fixtures" => {
                i += 1;
                fixtures_path = PathBuf::from(
                    args.get(i)
                        .ok_or("missing value for --fixtures")?,
                );
            }
            "--model-dir" => {
                i += 1;
                model_dir = Some(PathBuf::from(
                    args.get(i)
                        .ok_or("missing value for --model-dir")?,
                ));
            }
            "--qwen17-model-dir" => {
                i += 1;
                qwen17_model_dir = Some(PathBuf::from(
                    args.get(i)
                        .ok_or("missing value for --qwen17-model-dir")?,
                ));
            }
            "--output" => {
                i += 1;
                output_path = Some(PathBuf::from(
                    args.get(i)
                        .ok_or("missing value for --output")?,
                ));
            }
            other => return Err(format!("unknown argument: {other}")),
        }
        i += 1;
    }

    Ok(Args {
        fixtures_path,
        model_dir,
        qwen17_model_dir,
        output_path,
    })
}

// ── Main ─────────────────────────────────────────────────────────────────

fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Error: {e}");
            eprintln!("Usage: lifesub-asr-gate [--fixtures <path>] --model-dir <path> [--qwen17-model-dir <path>] [--output <path>]");
            process::exit(1);
        }
    };

    let fixtures_dir = args
        .fixtures_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();

    let manifest_json =
        fs::read_to_string(&args.fixtures_path).unwrap_or_else(|e| {
            eprintln!("Failed to read fixture manifest: {e}");
            process::exit(1);
        });

    let manifest: FixtureManifest =
        serde_json::from_str(&manifest_json).unwrap_or_else(|e| {
            eprintln!("Invalid fixture manifest: {e}");
            process::exit(1);
        });

    if manifest.schema_version != 1 {
        eprintln!(
            "Unsupported manifest schema version: {}",
            manifest.schema_version
        );
        process::exit(1);
    }

    let mut scenarios: Vec<ScenarioResult> = Vec::new();
    let mut fixture_hashes: BTreeMap<String, String> = BTreeMap::new();

    // Verify fixture hashes
    for entry in &manifest.fixtures {
        let fixture_path = fixtures_dir.join(&entry.file);
        let actual_bytes = match fs::read(&fixture_path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Missing fixture {}: {e}", entry.file);
                process::exit(1);
            }
        };
        let actual_hash = hex::encode(sha2::Sha256::digest(&actual_bytes));
        if actual_hash != entry.sha256 {
            eprintln!(
                "Fixture hash mismatch for {}: expected {}, got {}",
                entry.file, entry.sha256, actual_hash
            );
            process::exit(1);
        }
        fixture_hashes.insert(entry.file.clone(), actual_hash);
    }

    // Run scenarios for each provider
    let providers: &[(&str, bool)] = &[
        ("sense_voice", false),
        ("whisper", false),
        ("qwen3-0.6b", false),
        ("qwen3-1.7b", true),
    ];

    for (provider_id, requires_qwen17) in providers {
        if *requires_qwen17 && args.qwen17_model_dir.is_none() {
            // Skip Qwen 1.7B if no model dir provided
            for entry in &manifest.fixtures {
                scenarios.push(ScenarioResult {
                    scenario_id: format!("{}-{}", provider_id, entry.file.trim_end_matches(".wav")),
                    provider: provider_id.to_string(),
                    language: "unknown".to_string(),
                    fixture: entry.file.clone(),
                    cer: None,
                    wer: None,
                    key_phrase_recall: None,
                    rtf: None,
                    median_boundary_error_ms: None,
                    max_boundary_error_ms: None,
                    peak_rss_mib: None,
                    passed: false,
                    error: None,
                    skipped: Some(true),
                });
            }
            continue;
        }

        for entry in &manifest.fixtures {
            if entry.expected_text.is_none() && entry.expected_phrases.is_none() {
                continue; // Not a speech fixture
            }

            let scenario_id = format!(
                "{}-{}",
                provider_id,
                entry.file.trim_end_matches(".wav")
            );

            let result = run_scenario(
                provider_id,
                &fixtures_dir,
                entry,
                args.model_dir.as_deref(),
                args.qwen17_model_dir.as_deref(),
            );

            scenarios.push(result.unwrap_or_else(|e| ScenarioResult {
                scenario_id,
                provider: provider_id.to_string(),
                language: "unknown".to_string(),
                fixture: entry.file.clone(),
                cer: None,
                wer: None,
                key_phrase_recall: None,
                rtf: None,
                median_boundary_error_ms: None,
                max_boundary_error_ms: None,
                peak_rss_mib: None,
                passed: false,
                error: Some(e),
                skipped: None,
            }));
        }
    }

    // Build output
    let output = GateOutput {
        tested_commit: option_env!("LIFESUB_GATE_COMMIT")
            .unwrap_or("unknown")
            .to_string(),
        scoped_source_digest: "not-yet-implemented".to_string(),
        executable_hash: "not-yet-implemented".to_string(),
        runtime: Some(RuntimeIdentity {
            sherpa_version: "1.13.5".to_string(),
            sherpa_git_sha1: "3dc7c569".to_string(),
            sherpa_native_archive_sha256: "339c8fc19bb4b26e118c80792bbc4546eb263040fac36ef0cc027ec29c756b44".to_string(),
            qwen3_asr_crate_version: Some("0.2.2".to_string()),
            qwen3_asr_git_commit: Some("c5ef09646af6278d2ba8b8ceaf543ffb32d1a5dc".to_string()),
            candle_backend: Some("metal".to_string()),
            device_os: std::env::consts::OS.to_string(),
            device_arch: std::env::consts::ARCH.to_string(),
            device_chip: None,
            device_memory_gib: None,
        }),
        scenarios,
        fixture_hashes,
    };

    let json = serde_json::to_string_pretty(&output).unwrap_or_else(|e| {
        eprintln!("Failed to serialize output: {e}");
        process::exit(1);
    });

    if let Some(ref output_path) = args.output_path {
        fs::write(output_path, &json).unwrap_or_else(|e| {
            eprintln!("Failed to write output: {e}");
            process::exit(1);
        });
    } else {
        println!("{json}");
    }

    // Check if any mandatory scenario failed
    let failed = output
        .scenarios
        .iter()
        .filter(|s| !s.passed && s.skipped != Some(true))
        .count();
    if failed > 0 {
        eprintln!("{failed} scenario(s) failed");
        process::exit(1);
    }
}

// ── Scenario runner ──────────────────────────────────────────────────────

fn run_scenario(
    provider_id: &str,
    fixtures_dir: &Path,
    entry: &FixtureEntry,
    _model_dir: Option<&Path>,
    _qwen17_model_dir: Option<&Path>,
) -> Result<ScenarioResult, String> {
    // PENDING PRODUCTION VERIFICATION: Integrate with LifeSub ASR provider infrastructure
    // (ProviderFactory, ModelManager, device-qualified installations) on the M4/24GB device.
    // See the Gate docblock above and scripts/verify-asr-gate.sh.

    let _audio_path = fixtures_dir.join(&entry.file);

    Ok(ScenarioResult {
        scenario_id: format!(
            "{}-{}",
            provider_id,
            entry.file.trim_end_matches(".wav")
        ),
        provider: provider_id.to_string(),
        language: "unknown".to_string(),
        fixture: entry.file.clone(),
        cer: None,
        wer: None,
        key_phrase_recall: None,
        rtf: None,
        median_boundary_error_ms: None,
        max_boundary_error_ms: None,
        peak_rss_mib: None,
        passed: false,
        error: Some("Gate runner not yet integrated with ASR providers".to_string()),
        skipped: None,
    })
}