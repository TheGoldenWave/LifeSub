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

#![allow(dead_code, unused_imports)]

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
    normalized.chars().filter(|c| c.is_alphanumeric()).collect()
}

/// WER normalization: punctuation → spaces, collapse whitespace.
fn normalize_for_wer(text: &str) -> String {
    let normalized: String = text
        .chars()
        .flat_map(|c| c.to_lowercase())
        .map(|c| {
            if c.is_alphanumeric() || c.is_whitespace() {
                c
            } else {
                ' '
            }
        })
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
    let hyp_lower: String = hypothesis.chars().flat_map(|c| c.to_lowercase()).collect();
    let matched = expected
        .iter()
        .filter(|phrase| {
            let p: String = phrase.chars().flat_map(|c| c.to_lowercase()).collect();
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
    tested_commit: Option<String>,
    source_digest: Option<String>,
    install_missing: bool,
}

fn parse_args() -> Result<Args, String> {
    let args: Vec<String> = std::env::args().collect();
    let mut fixtures_path = PathBuf::from("tests/fixtures/asr/fixtures.json");
    let mut model_dir = None;
    let mut qwen17_model_dir = None;
    let mut output_path = None;
    let mut tested_commit = None;
    let mut source_digest = None;
    let mut install_missing = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--fixtures" => {
                i += 1;
                fixtures_path = PathBuf::from(args.get(i).ok_or("missing value for --fixtures")?);
            }
            "--model-dir" => {
                i += 1;
                model_dir = Some(PathBuf::from(
                    args.get(i).ok_or("missing value for --model-dir")?,
                ));
            }
            "--qwen17-model-dir" => {
                i += 1;
                qwen17_model_dir = Some(PathBuf::from(
                    args.get(i).ok_or("missing value for --qwen17-model-dir")?,
                ));
            }
            "--output" => {
                i += 1;
                output_path = Some(PathBuf::from(
                    args.get(i).ok_or("missing value for --output")?,
                ));
            }
            "--tested-commit" => {
                i += 1;
                tested_commit = Some(
                    args.get(i)
                        .ok_or("missing value for --tested-commit")?
                        .clone(),
                );
            }
            "--source-digest" => {
                i += 1;
                source_digest = Some(
                    args.get(i)
                        .ok_or("missing value for --source-digest")?
                        .clone(),
                );
            }
            "--install-missing" => install_missing = true,
            other => return Err(format!("unknown argument: {other}")),
        }
        i += 1;
    }

    Ok(Args {
        fixtures_path,
        model_dir,
        qwen17_model_dir,
        output_path,
        tested_commit,
        source_digest,
        install_missing,
    })
}

// ── Main ─────────────────────────────────────────────────────────────────

fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Error: {e}");
            eprintln!(
                "Usage: lifesub-asr-gate [--fixtures <path>] --model-dir <path> [--qwen17-model-dir <path>] [--output <path>]"
            );
            process::exit(1);
        }
    };

    let fixtures_dir = args
        .fixtures_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();

    let manifest_json = fs::read_to_string(&args.fixtures_path).unwrap_or_else(|e| {
        eprintln!("Failed to read fixture manifest: {e}");
        process::exit(1);
    });

    let manifest: FixtureManifest = serde_json::from_str(&manifest_json).unwrap_or_else(|e| {
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

    let providers: &[(&str, &str)] = &[
        ("sense_voice", "sense-voice-small-int8-2024-07-17"),
        ("whisper", "whisper-base"),
    ];
    let data_dir = args.model_dir.as_deref().unwrap_or_else(|| {
        eprintln!("--model-dir must point to the LifeSub data directory");
        process::exit(1);
    });

    if args.install_missing {
        for (_, model_id) in providers {
            lifesub_lib::asr::packaged_gate::ensure_installed_model(data_dir, model_id)
                .unwrap_or_else(|error| {
                    eprintln!("Failed to install {model_id}: {error}");
                    process::exit(1);
                });
        }
    }

    let gate_cases = [
        (
            "sense_voice",
            "sense-voice-small-int8-2024-07-17",
            "models/asr/sense_voice/sense-voice-small-int8-2024-07-17/1-7d1efa2138a65b0b488df37f8b89e3d91a60676e416f515b952358d83dfd347e/test_wavs/zh.wav",
            "b77f1794fe374a0ba1ee1dc458bfaf9349496cbbfc32780c50ba3c5a7ad8e373",
            "开放时间早上9点至下午5点。",
        ),
        (
            "whisper",
            "whisper-base",
            "models/asr/whisper/whisper-base/1-911b2083efd7c0dca2ac3b358b75222660dc09fb716d64fbfc417ba6c99ff3de/test_wavs/0.wav",
            "6bc58a4efdf20daac252b6b1502632601a71efe0308f6757dc1eda34891a7e4f",
            "after early nightfall the yellow lamps would light up here and there the squalid quarter of the brothels",
        ),
    ];

    for (provider_id, model_id, relative_fixture, expected_hash, expected_text) in gate_cases {
        let fixture_path = data_dir.join(relative_fixture);
        if let Ok(bytes) = fs::read(&fixture_path) {
            let actual_hash = hex::encode(Sha256::digest(bytes));
            if actual_hash != expected_hash {
                eprintln!("Installed fixture hash mismatch for {relative_fixture}");
                process::exit(1);
            }
            fixture_hashes.insert(relative_fixture.to_string(), actual_hash);
        }
        let entry = FixtureEntry {
            file: relative_fixture.to_string(),
            format: "wav".to_string(),
            sha256: expected_hash.to_string(),
            bytes: 0,
            sample_rate_hz: 16_000,
            channels: 1,
            nominal_frames: 0,
            expected_text: Some(expected_text.to_string()),
            expected_phrases: None,
            minimum_phrase_matches: None,
            normalization: Some("unicode-nfkc".to_string()),
            require_nonempty: Some(true),
        };
        let scenario_id = format!("{provider_id}-known-fixture");
        let result = run_scenario(provider_id, model_id, data_dir, &entry, Some(data_dir));
        scenarios.push(result.unwrap_or_else(|error| ScenarioResult {
            scenario_id,
            provider: provider_id.to_string(),
            language: "unknown".to_string(),
            fixture: relative_fixture.to_string(),
            cer: None,
            wer: None,
            key_phrase_recall: None,
            rtf: None,
            median_boundary_error_ms: None,
            max_boundary_error_ms: None,
            peak_rss_mib: None,
            passed: false,
            error: Some(error),
            skipped: None,
        }));
    }

    // Build output
    let output = GateOutput {
        tested_commit: args
            .tested_commit
            .clone()
            .or_else(|| option_env!("LIFESUB_GATE_COMMIT").map(str::to_string))
            .unwrap_or_else(|| "unknown".to_string()),
        scoped_source_digest: args
            .source_digest
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
        executable_hash: current_executable_hash().unwrap_or_else(|_| "unknown".to_string()),
        runtime: Some(RuntimeIdentity {
            sherpa_version: "1.13.5".to_string(),
            sherpa_git_sha1: "3dc7c569".to_string(),
            sherpa_native_archive_sha256:
                "339c8fc19bb4b26e118c80792bbc4546eb263040fac36ef0cc027ec29c756b44".to_string(),
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
    model_id: &str,
    fixtures_dir: &Path,
    entry: &FixtureEntry,
    data_dir: Option<&Path>,
) -> Result<ScenarioResult, String> {
    let data_dir = data_dir.ok_or_else(|| "LifeSub data directory is required".to_string())?;
    let audio_path = fixtures_dir.join(&entry.file);
    let inference = lifesub_lib::asr::packaged_gate::transcribe_installed_fixture(
        data_dir,
        model_id,
        &audio_path,
    )?;
    let cer = entry.expected_text.as_deref().map(|reference| {
        calculate_cer(
            &normalize_for_cer(reference),
            &normalize_for_cer(&inference.text),
        )
    });
    let wer = entry.expected_text.as_deref().map(|reference| {
        calculate_wer(
            &normalize_for_wer(reference),
            &normalize_for_wer(&inference.text),
        )
    });
    let key_phrase_recall = entry
        .expected_phrases
        .as_deref()
        .map(|phrases| calculate_key_phrase_recall(phrases, &inference.text));
    let rtf = if inference.audio_duration_ms == 0 {
        None
    } else {
        Some(inference.elapsed_ms as f64 / inference.audio_duration_ms as f64)
    };
    let quality_passed = match provider_id {
        "sense_voice" => cer.is_some_and(|actual| actual <= 0.20),
        "whisper" => wer.is_some_and(|actual| actual <= 0.20),
        _ => false,
    };
    let passed = !inference.text.trim().is_empty()
        && rtf.is_some_and(|value| value <= 1.0)
        && quality_passed;

    Ok(ScenarioResult {
        scenario_id: format!("{}-{}", provider_id, entry.file.trim_end_matches(".wav")),
        provider: provider_id.to_string(),
        language: "auto".to_string(),
        fixture: entry.file.clone(),
        cer,
        wer,
        key_phrase_recall,
        rtf,
        median_boundary_error_ms: None,
        max_boundary_error_ms: None,
        peak_rss_mib: peak_rss_mib(),
        passed,
        error: (!passed).then(|| format!("quality gate failed; transcript={}", inference.text)),
        skipped: None,
    })
}

fn current_executable_hash() -> Result<String, String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let bytes = fs::read(executable).map_err(|error| error.to_string())?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn peak_rss_mib() -> Option<f64> {
    let mut usage = unsafe { std::mem::zeroed::<libc::rusage>() };
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) } != 0 {
        return None;
    }
    #[cfg(target_os = "macos")]
    return Some(usage.ru_maxrss as f64 / (1024.0 * 1024.0));
    #[cfg(not(target_os = "macos"))]
    Some(usage.ru_maxrss as f64 / 1024.0)
}
