use std::sync::{Arc, Mutex};

use chrono::{DateTime, Duration, Utc};
use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

use crate::asr::job::{Clock, ExecutionStage, SnapshotError};
use crate::asr::manifest::{
    InstallConstraints, canonical_bundle_payload, model_registry, vad_manifest,
};
use crate::asr::settings::{AsrProviderOptions, WhisperTask};
use crate::domain::{AsrProviderKind, AudioSource};
use crate::service::CoreRuntime;

const NOW: &str = "2026-08-16T08:00:00.000Z";
const INPUT_SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const MODEL_ID: &str = "whisper-small";

#[derive(Clone)]
struct TestClock(Arc<Mutex<DateTime<Utc>>>);

impl TestClock {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(
            DateTime::parse_from_rfc3339(NOW)
                .unwrap()
                .with_timezone(&Utc),
        )))
    }

    fn advance(&self, duration: Duration) {
        *self.0.lock().unwrap() += duration;
    }
}

impl Clock for TestClock {
    fn now(&self) -> DateTime<Utc> {
        *self.0.lock().unwrap()
    }
}

struct Fixture {
    _parent: TempDir,
    data_dir: std::path::PathBuf,
    db_path: std::path::PathBuf,
    runtime: CoreRuntime,
    clock: TestClock,
}

type SnapshotMutation = fn(&Fixture, &mut crate::asr::job::ClaimToken);
type JsonMutation = fn(&mut serde_json::Value);
type JsonColumnCase = (&'static str, &'static str, JsonMutation, &'static str);

impl Fixture {
    fn new() -> Self {
        let parent = tempfile::tempdir().unwrap();
        let data_dir = parent.path().join("data");
        let db_path = data_dir.join("lifesub.sqlite3");
        let runtime = CoreRuntime::initialize_with_boot_id(&data_dir, "boot-a").unwrap();
        let fixture = Self {
            _parent: parent,
            data_dir,
            db_path,
            runtime,
            clock: TestClock::new(),
        };
        fixture.insert_job();
        fixture
    }

    fn insert_job(&self) {
        let connection = Connection::open(&self.db_path).unwrap();
        connection
            .pragma_update(None, "foreign_keys", true)
            .unwrap();
        connection
            .execute(
                "INSERT INTO sessions(id, title, state, started_at)
                 VALUES('session-snapshot', 'snapshot', 'stopped', ?1)",
                [NOW],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO chunks(
                   id, session_id, source, path, sha256, byte_length, session_offset_ms,
                   duration_ms, integrity_state
                 ) VALUES(
                   'chunk-snapshot', 'session-snapshot', 'imported', 'audio/source.wav', ?1,
                   4096, 1250, 8000, 'available'
                 )",
                [INPUT_SHA],
            )
            .unwrap();
        let settings = serde_json::json!({
            "provider": "whisper",
            "model_id": "whisper-small",
            "language": "zh",
            "num_threads": 3,
            "vad_enabled": true,
            "auto_transcribe_imports": false,
            "options": {"provider": "whisper", "task": "translate"}
        });
        let manifest = model_registry().model(MODEL_ID).unwrap();
        let required = required_files_json(manifest.bundle.install_constraints);
        let vad = vad_manifest();
        let vad_required = required_files_json(vad.bundle.install_constraints);
        let source = serde_json::json!({
            "bundle": serde_json::from_str::<serde_json::Value>(
                &canonical_bundle_payload(manifest).unwrap()
            ).unwrap(),
            "repository_url": manifest.source.repository_url,
            "model_card_url": manifest.source.model_card_url,
            "license_spdx": manifest.source.license_spdx,
            "provenance": manifest.source.provenance,
        });
        let source = with_source_contract(source);
        connection
            .execute(
                "INSERT INTO asr_jobs(
                   id, session_id, chunk_id, provider, model_id, manifest_version,
                   archive_sha256, required_file_hashes_json, model_source_json,
                   vad_model_id, vad_manifest_version, vad_archive_sha256,
                   vad_required_file_hashes_json, parameters_json, input_sha256, fingerprint,
                   state, max_attempts, available_at, created_at, updated_at
                 ) VALUES(
                   'job-snapshot', 'session-snapshot', 'chunk-snapshot', 'whisper',
                   'whisper-small', ?1, ?2, ?3, ?4, 'silero-vad-2024-01-17',
                   ?5, ?6, ?7, ?8, ?9, 'snapshot-fingerprint', 'queued', 3, ?10, ?10, ?10
                 )",
                params![
                    manifest.manifest_version,
                    manifest.bundle.identity_sha256,
                    required.to_string(),
                    source.to_string(),
                    vad.manifest_version,
                    vad.bundle.identity_sha256,
                    vad_required.to_string(),
                    settings.to_string(),
                    INPUT_SHA,
                    NOW,
                ],
            )
            .unwrap();
    }

    fn claim(&self) -> crate::asr::job::ClaimToken {
        self.runtime
            .job_repository_with_clock(self.clock.clone())
            .claim("snapshot-worker")
            .unwrap()
            .unwrap()
            .token
    }

    fn mutate(&self, sql: &str) {
        Connection::open(&self.db_path)
            .unwrap()
            .execute_batch(sql)
            .unwrap();
    }

    fn json_column(&self, column: &str) -> serde_json::Value {
        let sql = format!("SELECT {column} FROM asr_jobs WHERE id = 'job-snapshot'");
        let json: String = Connection::open(&self.db_path)
            .unwrap()
            .query_row(&sql, [], |row| row.get(0))
            .unwrap();
        serde_json::from_str(&json).unwrap()
    }

    fn set_json_column(&self, column: &str, value: &serde_json::Value) {
        let sql = format!("UPDATE asr_jobs SET {column} = ?1 WHERE id = 'job-snapshot'");
        Connection::open(&self.db_path)
            .unwrap()
            .execute(&sql, [value.to_string()])
            .unwrap();
    }
}

fn required_files_json(constraints: InstallConstraints) -> serde_json::Value {
    let files = match constraints {
        InstallConstraints::Archive(value) => value.required_files,
        InstallConstraints::Direct(value) => value.required_files,
    };
    serde_json::Value::Array(
        files
            .iter()
            .map(|file| {
                serde_json::json!({
                    "path": file.path,
                    "bytes": file.bytes,
                    "sha256": file.sha256,
                })
            })
            .collect(),
    )
}

fn with_source_contract(mut source: serde_json::Value) -> serde_json::Value {
    let canonical = serde_json_canonicalizer::to_string(&source).unwrap();
    source["source_contract_sha256"] =
        serde_json::json!(hex::encode(Sha256::digest(canonical.as_bytes())));
    source
}

#[test]
fn claimed_worker_loads_complete_immutable_preparing_snapshot() {
    let fixture = Fixture::new();
    let mut coordinator = fixture
        .runtime
        .job_coordinator_with_clock("snapshot-worker", fixture.clock.clone())
        .unwrap();
    let token = coordinator.claim_next().unwrap().unwrap().token;

    let snapshot = coordinator
        .load_execution_snapshot(&token, ExecutionStage::Preparing)
        .unwrap();

    assert_eq!(snapshot.job_id, "job-snapshot");
    assert_eq!(snapshot.session_id, "session-snapshot");
    assert_eq!(snapshot.chunk.id, "chunk-snapshot");
    assert_eq!(snapshot.chunk.source, AudioSource::Imported);
    assert_eq!(snapshot.chunk.relative_path, "audio/source.wav");
    assert_eq!(snapshot.chunk.sha256, INPUT_SHA);
    assert_eq!(snapshot.chunk.byte_length, 4096);
    assert_eq!(snapshot.chunk.session_offset_ms, 1250);
    assert_eq!(snapshot.chunk.duration_ms, Some(8000));
    assert_eq!(snapshot.model.provider, AsrProviderKind::Whisper);
    assert_eq!(snapshot.model.model_id, "whisper-small");
    let manifest = model_registry().model(MODEL_ID).unwrap();
    assert_eq!(snapshot.model.manifest_version, manifest.manifest_version);
    assert_eq!(
        snapshot.model.bundle_identity,
        manifest.bundle.identity_sha256
    );
    assert_eq!(snapshot.model.required_files.len(), 7);
    assert_eq!(snapshot.model.required_files[0].path, "small-decoder.onnx");
    assert_eq!(
        snapshot.model.source["repository_url"],
        manifest.source.repository_url
    );
    assert_eq!(snapshot.parameters.language.as_str(), "zh");
    assert_eq!(snapshot.parameters.num_threads, 3);
    assert!(!snapshot.parameters.settings.auto_transcribe_imports);
    assert!(snapshot.parameters.settings.vad_enabled);
    assert_eq!(
        snapshot.parameters.options,
        AsrProviderOptions::Whisper {
            task: WhisperTask::Translate
        }
    );
    let vad = snapshot.vad.unwrap();
    assert_eq!(vad.model_id, "silero-vad-2024-01-17");
    assert_eq!(vad.manifest_version, vad_manifest().manifest_version);
    assert_eq!(vad.bundle_identity, vad_manifest().bundle.identity_sha256);
    assert_eq!(
        vad.required_files[0].sha256,
        vad_manifest().bundle.artifacts[0].sha256
    );
}

#[cfg(unix)]
#[test]
fn invalidated_full_core_capability_rejects_snapshot_before_catalog_access() {
    let fixture = Fixture::new();
    let token = fixture.claim();
    let lock_path = fixture.data_dir.join("asr-worker.lock");
    std::fs::rename(&lock_path, fixture.data_dir.join("old-lock")).unwrap();
    std::fs::write(&lock_path, b"replacement").unwrap();

    assert!(matches!(
        fixture
            .runtime
            .job_repository_with_clock(fixture.clock.clone())
            .load_execution_snapshot(&token, ExecutionStage::Preparing),
        Err(SnapshotError::Ownership(_))
    ));
}

#[test]
fn exact_stage_is_required_and_transcribing_snapshot_uses_same_claim() {
    let fixture = Fixture::new();
    let token = fixture.claim();
    let repository = fixture
        .runtime
        .job_repository_with_clock(fixture.clock.clone());

    assert_eq!(
        repository.load_execution_snapshot(&token, ExecutionStage::Transcribing),
        Err(SnapshotError::StageMismatch)
    );
    repository.mark_transcribing(&token).unwrap();
    assert_eq!(
        repository
            .load_execution_snapshot(&token, ExecutionStage::Transcribing)
            .unwrap()
            .job_id,
        "job-snapshot"
    );
    assert_eq!(
        repository.load_execution_snapshot(&token, ExecutionStage::Preparing),
        Err(SnapshotError::StageMismatch)
    );
}

#[test]
fn stale_foreign_cancelled_expired_or_unavailable_claim_never_returns_a_snapshot() {
    let cases: [(&str, SnapshotMutation, SnapshotError); 6] = [
        (
            "foreign owner",
            |_fixture, token| token.claimed_by.push_str("-foreign"),
            SnapshotError::OwnershipLost,
        ),
        (
            "stale generation",
            |_fixture, token| token.claim_generation += 1,
            SnapshotError::OwnershipLost,
        ),
        (
            "cancelled",
            |fixture, _token| {
                fixture.mutate(
                    "UPDATE asr_jobs SET cancel_requested_at = '2026-08-16T08:00:01.000Z'
                     WHERE id = 'job-snapshot'",
                );
            },
            SnapshotError::CancelRequested,
        ),
        (
            "expired",
            |fixture, _token| fixture.clock.advance(Duration::seconds(31)),
            SnapshotError::LeaseExpired,
        ),
        (
            "unavailable input",
            |fixture, _token| {
                fixture.mutate(
                    "UPDATE chunks SET integrity_state = 'missing' WHERE id = 'chunk-snapshot'",
                );
            },
            SnapshotError::InputUnavailable,
        ),
        (
            "cross-session chunk",
            |fixture, _token| {
                fixture.mutate(
                    "PRAGMA foreign_keys = OFF;
                    INSERT INTO sessions(id, title, state, started_at)
                    VALUES('other-session', 'other', 'stopped', '2026-08-16T08:00:00.000Z');
                    UPDATE chunks SET session_id = 'other-session' WHERE id = 'chunk-snapshot';",
                );
            },
            SnapshotError::InputUnavailable,
        ),
    ];

    for (name, mutate, expected) in cases {
        let fixture = Fixture::new();
        let mut token = fixture.claim();
        mutate(&fixture, &mut token);
        let result = fixture
            .runtime
            .job_repository_with_clock(fixture.clock.clone())
            .load_execution_snapshot(&token, ExecutionStage::Preparing);
        assert_eq!(result, Err(expected), "{name}");
    }
}

#[test]
fn malformed_frozen_identity_is_rejected_instead_of_rebuilt_from_current_state() {
    let cases = [
        (
            "UPDATE asr_jobs SET required_file_hashes_json = '[]' WHERE id = 'job-snapshot'",
            "required_file_hashes_json",
        ),
        (
            "UPDATE asr_jobs SET model_source_json = '[]' WHERE id = 'job-snapshot'",
            "model_source_json",
        ),
        (
            "UPDATE asr_jobs SET parameters_json = '{\"provider\":\"sense_voice\",\"model_id\":\"whisper-small\",\"language\":\"zh\",\"num_threads\":3,\"vad_enabled\":true,\"auto_transcribe_imports\":false,\"options\":{\"provider\":\"sense_voice\",\"use_itn\":true}}' WHERE id = 'job-snapshot'",
            "parameters_json",
        ),
        (
            "UPDATE chunks SET path = '../escape.wav' WHERE id = 'chunk-snapshot'",
            "chunk.path",
        ),
    ];

    for (mutation, field) in cases {
        let fixture = Fixture::new();
        let token = fixture.claim();
        fixture.mutate(mutation);
        assert_eq!(
            fixture
                .runtime
                .job_repository_with_clock(fixture.clock.clone())
                .load_execution_snapshot(&token, ExecutionStage::Preparing),
            Err(SnapshotError::InvalidSnapshot(field)),
        );
    }
}

#[test]
fn every_model_source_identity_layer_rejects_single_field_drift() {
    let mutations: &[(&str, JsonMutation)] = &[
        ("unknown root", |value| {
            value["unexpected"] = serde_json::json!(true)
        }),
        ("repository", |value| {
            value["repository_url"] = serde_json::json!("http://invalid")
        }),
        ("model card", |value| {
            value["model_card_url"] = serde_json::json!("https://EXAMPLE.com/model")
        }),
        ("license", |value| {
            value["license_spdx"] = serde_json::json!("")
        }),
        ("provenance", |value| {
            value["provenance"] = serde_json::json!("TODO")
        }),
        ("schema", |value| {
            value["bundle"]["schema"] = serde_json::json!("future")
        }),
        ("model id", |value| {
            value["bundle"]["model_id"] = serde_json::json!("whisper-base")
        }),
        ("manifest", |value| {
            value["bundle"]["manifest_version"] = serde_json::json!("2")
        }),
        ("provider", |value| {
            value["bundle"]["provider"] = serde_json::json!("sense_voice")
        }),
        ("compatibility", |value| {
            value["bundle"]["compatibility_contract"]["archive_contract"] =
                serde_json::json!("other")
        }),
        ("artifact unknown", |value| {
            value["bundle"]["artifacts"][0]["unexpected"] = serde_json::json!(true)
        }),
        ("artifact id", |value| {
            value["bundle"]["artifacts"][0]["artifact_id"] = serde_json::json!("")
        }),
        ("source repository", |value| {
            value["bundle"]["artifacts"][0]["source_repository"] =
                serde_json::json!("http://invalid")
        }),
        ("source model", |value| {
            value["bundle"]["artifacts"][0]["source_model"] = serde_json::json!("")
        }),
        ("source endpoint", |value| {
            value["bundle"]["artifacts"][0]["source_endpoint"] = serde_json::json!("http://invalid")
        }),
        ("resolved url", |value| {
            value["bundle"]["artifacts"][0]["resolved_url"] = serde_json::json!("http://invalid")
        }),
        ("revision", |value| {
            value["bundle"]["artifacts"][0]["revision"] = serde_json::json!("")
        }),
        ("artifact bytes", |value| {
            value["bundle"]["artifacts"][0]["bytes"] = serde_json::json!(0)
        }),
        ("artifact sha", |value| {
            value["bundle"]["artifacts"][0]["sha256"] = serde_json::json!("bad")
        }),
        ("artifact path", |value| {
            value["bundle"]["artifacts"][0]["required_path"] = serde_json::json!("../escape")
        }),
        ("artifact required", |value| {
            value["bundle"]["artifacts"][0]["required"] = serde_json::json!(false)
        }),
        ("install mode", |value| {
            value["bundle"]["artifacts"][0]["install_mode"] = serde_json::json!("unknown")
        }),
        ("artifact license", |value| {
            value["bundle"]["artifacts"][0]["license_spdx"] = serde_json::json!("")
        }),
        ("artifact provenance", |value| {
            value["bundle"]["artifacts"][0]["provenance"] = serde_json::json!("TODO")
        }),
        ("redirect host", |value| {
            value["bundle"]["artifacts"][0]["redirect_hosts"] = serde_json::json!(["EXAMPLE.com"])
        }),
        ("required paths", |value| {
            value["bundle"]["required_paths"] = serde_json::json!(["other"])
        }),
        ("runtime unknown", |value| {
            value["bundle"]["runtime_requirement"]["unexpected"] = serde_json::json!(true)
        }),
        ("device bound", |value| {
            value["bundle"]["device_requirement"]["kind"] = serde_json::json!("unknown")
        }),
        ("qualification", |value| {
            value["bundle"]["qualification_policy"] = serde_json::json!("unknown")
        }),
    ];

    for (name, mutate) in mutations {
        let fixture = Fixture::new();
        let token = fixture.claim();
        let mut source = fixture.json_column("model_source_json");
        mutate(&mut source);
        fixture.set_json_column("model_source_json", &source);
        assert_eq!(
            fixture
                .runtime
                .job_repository_with_clock(fixture.clock.clone())
                .load_execution_snapshot(&token, ExecutionStage::Preparing),
            Err(SnapshotError::InvalidSnapshot("model_source_json")),
            "{name}"
        );
    }
}

#[test]
fn source_contract_hash_rejects_legal_but_wrong_runtime_and_provenance_drift() {
    let mutations: &[(&str, JsonMutation)] = &[
        ("contract hash", |value| {
            value["source_contract_sha256"] = serde_json::json!(INPUT_SHA)
        }),
        ("runtime version", |value| {
            value["bundle"]["runtime_requirement"]["version"] = serde_json::json!("1.13.4")
        }),
        ("runtime build", |value| {
            value["bundle"]["runtime_requirement"]["build_id"] = serde_json::json!("other-build")
        }),
        ("runtime git", |value| {
            value["bundle"]["runtime_requirement"]["git_commit"] =
                serde_json::json!("1111111111111111111111111111111111111111")
        }),
        ("runtime archive", |value| {
            value["bundle"]["runtime_requirement"]["native_archive_sha256"] =
                serde_json::json!(INPUT_SHA)
        }),
        ("qualification legal", |value| {
            value["bundle"]["qualification_policy"] = serde_json::json!("runtime_smoke_required")
        }),
        ("repository legal", |value| {
            value["repository_url"] = serde_json::json!("https://github.com/other/model")
        }),
        ("endpoint legal", |value| {
            value["bundle"]["artifacts"][0]["source_endpoint"] = serde_json::json!(
                "https://api.github.com/repos/other/model/releases/tags/asr-models"
            )
        }),
        ("license legal", |value| {
            value["license_spdx"] = serde_json::json!("Apache-2.0")
        }),
        ("provenance legal", |value| {
            value["provenance"] = serde_json::json!("Different but valid provenance")
        }),
    ];

    for (name, mutate) in mutations {
        let fixture = Fixture::new();
        let token = fixture.claim();
        let mut source = fixture.json_column("model_source_json");
        mutate(&mut source);
        fixture.set_json_column("model_source_json", &source);
        assert_eq!(
            fixture
                .runtime
                .job_repository_with_clock(fixture.clock.clone())
                .load_execution_snapshot(&token, ExecutionStage::Preparing),
            Err(SnapshotError::InvalidSnapshot("model_source_json")),
            "{name}"
        );
    }
}

#[test]
fn archive_required_file_inventory_is_an_exact_bijection() {
    let mutations: &[(&str, JsonMutation)] = &[
        ("missing", |value| {
            value.as_array_mut().unwrap().pop();
        }),
        ("extra", |value| {
            value.as_array_mut().unwrap().push(serde_json::json!({
                "path": "extra.bin", "bytes": 1, "sha256": INPUT_SHA
            }));
        }),
        ("extra child", |value| {
            value.as_array_mut().unwrap().push(serde_json::json!({
                "path": "small-decoder.onnx/child", "bytes": 1, "sha256": INPUT_SHA
            }));
        }),
    ];

    for (name, mutate) in mutations {
        let fixture = Fixture::new();
        let token = fixture.claim();
        let mut files = fixture.json_column("required_file_hashes_json");
        mutate(&mut files);
        fixture.set_json_column("required_file_hashes_json", &files);
        assert_eq!(
            fixture
                .runtime
                .job_repository_with_clock(fixture.clock.clone())
                .load_execution_snapshot(&token, ExecutionStage::Preparing),
            Err(SnapshotError::InvalidSnapshot("model_source_json")),
            "{name}"
        );
    }
}

#[test]
fn sha256_fields_reject_uppercase_and_all_zero_values() {
    for sha in [
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        "0000000000000000000000000000000000000000000000000000000000000000",
    ] {
        let fixture = Fixture::new();
        let token = fixture.claim();
        let mut files = fixture.json_column("required_file_hashes_json");
        files[0]["sha256"] = serde_json::json!(sha);
        fixture.set_json_column("required_file_hashes_json", &files);
        assert_eq!(
            fixture
                .runtime
                .job_repository_with_clock(fixture.clock.clone())
                .load_execution_snapshot(&token, ExecutionStage::Preparing),
            Err(SnapshotError::InvalidSnapshot("required_file_hashes_json")),
            "{sha}"
        );
    }
}

#[test]
fn parameters_required_files_and_vad_reject_unknown_fields_and_contract_drift() {
    let cases: &[JsonColumnCase] = &[
        (
            "unknown parameter",
            "parameters_json",
            |value| value["unexpected"] = serde_json::json!(true),
            "parameters_json",
        ),
        (
            "language",
            "parameters_json",
            |value| value["language"] = serde_json::json!("xx-unknown"),
            "parameters_json",
        ),
        (
            "thread zero",
            "parameters_json",
            |value| value["num_threads"] = serde_json::json!(0),
            "parameters_json",
        ),
        (
            "thread excessive",
            "parameters_json",
            |value| value["num_threads"] = serde_json::json!(u16::MAX),
            "parameters_json",
        ),
        (
            "option unknown",
            "parameters_json",
            |value| value["options"]["unexpected"] = serde_json::json!(true),
            "parameters_json",
        ),
        (
            "required unknown",
            "required_file_hashes_json",
            |value| value[0]["unexpected"] = serde_json::json!(true),
            "required_file_hashes_json",
        ),
        (
            "vad unknown",
            "vad_required_file_hashes_json",
            |value| value[0]["unexpected"] = serde_json::json!(true),
            "vad_required_file_hashes_json",
        ),
        (
            "vad mismatch",
            "vad_required_file_hashes_json",
            |value| value[0]["sha256"] = serde_json::json!(INPUT_SHA),
            "vad_required_file_hashes_json",
        ),
    ];

    for (name, column, mutate, field) in cases {
        let fixture = Fixture::new();
        let token = fixture.claim();
        let mut value = fixture.json_column(column);
        mutate(&mut value);
        fixture.set_json_column(column, &value);
        assert_eq!(
            fixture
                .runtime
                .job_repository_with_clock(fixture.clock.clone())
                .load_execution_snapshot(&token, ExecutionStage::Preparing),
            Err(SnapshotError::InvalidSnapshot(field)),
            "{name}"
        );
    }
}

#[test]
fn malformed_or_boundary_lease_timestamp_fails_closed() {
    for lease in ["not-a-time", "2026-08-16T08:00:30Z", NOW] {
        let fixture = Fixture::new();
        let token = fixture.claim();
        fixture.mutate(&format!(
            "UPDATE asr_jobs SET lease_expires_at = '{lease}' WHERE id = 'job-snapshot'"
        ));
        assert_eq!(
            fixture
                .runtime
                .job_repository_with_clock(fixture.clock.clone())
                .load_execution_snapshot(&token, ExecutionStage::Preparing),
            Err(SnapshotError::LeaseExpired),
            "{lease}"
        );
    }
}

#[test]
fn qwen17_direct_bundle_freezes_conversion_and_exact_artifact_files() {
    let fixture = Fixture::new();
    let token = fixture.claim();
    let manifest = model_registry().model("qwen3-asr-1.7b").unwrap();
    let source = serde_json::json!({
        "bundle": serde_json::from_str::<serde_json::Value>(
            &canonical_bundle_payload(manifest).unwrap()
        ).unwrap(),
        "repository_url": manifest.source.repository_url,
        "model_card_url": manifest.source.model_card_url,
        "license_spdx": manifest.source.license_spdx,
        "provenance": manifest.source.provenance,
    });
    let source = with_source_contract(source);
    let required = required_files_json(manifest.bundle.install_constraints);
    let parameters = serde_json::json!({
        "provider": "qwen3_asr",
        "model_id": manifest.id,
        "language": "ro",
        "num_threads": 1,
        "vad_enabled": true,
        "auto_transcribe_imports": false,
        "options": {"provider": "qwen3_asr"}
    });
    let connection = Connection::open(&fixture.db_path).unwrap();
    connection
        .execute(
            "UPDATE asr_jobs SET provider = 'qwen3_asr', model_id = ?1,
             manifest_version = ?2, archive_sha256 = ?3, required_file_hashes_json = ?4,
             model_source_json = ?5, parameters_json = ?6 WHERE id = 'job-snapshot'",
            params![
                manifest.id,
                manifest.manifest_version,
                manifest.bundle.identity_sha256,
                required.to_string(),
                source.to_string(),
                parameters.to_string(),
            ],
        )
        .unwrap();

    let snapshot = fixture
        .runtime
        .job_repository_with_clock(fixture.clock.clone())
        .load_execution_snapshot(&token, ExecutionStage::Preparing)
        .unwrap();

    assert_eq!(snapshot.model.required_files.len(), 5);
    assert_eq!(
        snapshot.model.source["bundle"]["compatibility_contract"]["conversion"],
        "none"
    );
    assert_eq!(snapshot.parameters.language.as_str(), "ro");

    let mut drift = fixture.json_column("required_file_hashes_json");
    drift[0]["sha256"] = serde_json::json!(INPUT_SHA);
    fixture.set_json_column("required_file_hashes_json", &drift);
    assert_eq!(
        fixture
            .runtime
            .job_repository_with_clock(fixture.clock.clone())
            .load_execution_snapshot(&token, ExecutionStage::Preparing),
        Err(SnapshotError::InvalidSnapshot("model_source_json"))
    );
}

#[test]
fn snapshot_loader_is_one_job_chunk_query_and_does_not_read_current_manifests_or_settings() {
    let source = include_str!("catalog/job_snapshot.rs");

    assert_eq!(source.matches("query_row(").count(), 1);
    assert!(!source.contains("model_registry"));
    assert!(!source.contains("vad_manifest()"));
    assert!(!source.contains("AsrSettingsStore"));
}
