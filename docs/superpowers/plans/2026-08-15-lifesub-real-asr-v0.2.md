# LifeSub Real Local ASR V0.2 Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the demo transcript path with real, local SenseVoice, Whisper, and Qwen3-ASR transcription, model switching, persistent ASR settings, recoverable jobs, immutable revisions, and a complete settings experience.

**Architecture:** The Rust Core owns schema migration, immutable audio, multi-artifact model installation, ASR jobs, exact runtime dispatch, receipts, and revision publication. React only reads and changes settings, displays model/job state, and requests retranscription. SenseVoiceSmall, Whisper, and Qwen3-ASR 0.6B share sherpa-onnx 1.13.5; Qwen3-ASR 1.7B is a formal installable/executable model using pinned `qwen3-asr` 0.2.2 + Candle/Metal. The single fenced worker never falls back between their runtime identities.

**Tech Stack:** Rust 2024, rusqlite, sherpa-onnx 1.13.5 static runtime, `qwen3-asr` 0.2.2 pinned to Git commit `c5ef09646af6278d2ba8b8ceaf543ffb32d1a5dc`, Candle + Metal, Symphonia 0.6, Rubato 5, reqwest blocking + rustls, tar/bzip2, fs2, React 19, TypeScript, Vitest, Testing Library, Playwright, Tauri 2.

**Required references:**

- `docs/prd/lifesub-real-asr-v0.2/PRD.md`
- `docs/superpowers/specs/2026-08-15-lifesub-real-asr-design.md`
- `docs/superpowers/specs/2026-08-16-lifesub-local-tool-api-design.md`
- `.claude/rules/common/coding-style.md`
- `.claude/contexts/dev.md`
- `docs/design/tokens/base.json`

---

## File Map

### Rust Core

- Modify `src-tauri/Cargo.toml`: pin ASR, audio, download, archive, locking, cancellation, and test dependencies, including exact `qwen3-asr` Git revision and Candle Metal features.
- Modify `src-tauri/src/lib.rs`: register ASR modules, tests, commands, and worker lifecycle.
- Modify `src-tauri/src/domain.rs`: add chunk integrity and compatible ASR provenance fields.
- Modify `src-tauri/src/catalog.rs`: delegate versioned migration and expose transactional ASR persistence.
- Create `src-tauri/src/catalog/migrations.rs`: schema fingerprinting and ordered v1/v2/v3/v4 migrations; each version has immutable DDL/fingerprint ownership.
- Create `src-tauri/src/asr/mod.rs`: public ASR module boundary.
- Create `src-tauri/src/asr/settings.rs`: tagged provider settings and validation.
- Create `src-tauri/src/asr/manifest.rs`: pinned model/VAD manifests, multi-artifact canonical bundle identity, provenance, runtime and device requirements.
- Create `src-tauri/src/asr/model_manager.rs`: downloads, safe extraction, versioned install activation, reconciliation.
- Create `src-tauri/src/asr/audio.rs`: decoding, downmix, resampling, duration and time conversion.
- Create `src-tauri/src/asr/vad.rs`: VAD ranges, 25-second partitioning, timing invariants.
- Create `src-tauri/src/asr/provider.rs`: provider trait, request/result/error types, fake provider.
- Create `src-tauri/src/asr/sense_voice.rs`: sherpa-onnx SenseVoice adapter.
- Create `src-tauri/src/asr/whisper.rs`: sherpa-onnx Whisper adapter.
- Create `src-tauri/src/asr/qwen3_asr.rs`: Qwen 0.6B sherpa and Qwen 1.7B Candle/Metal adapters with exact dispatch and compatibility Gate.
- Create `src-tauri/src/asr/job.rs`: job state machine consuming the Task 4 ownership guard, claims, leases, fencing, cancellation.
- Create `src-tauri/src/asr/service.rs`: job execution and atomic Receipt/Revision publication.
- Create `src-tauri/src/core_runtime.rs`: primary host extracted around Task 4's existing guarded owner for Catalog, capture state, reconciliation, model manager, sockets, and ASR worker.
- Create `src-tauri/src/tool_api.rs`: versioned transport-independent Local Tool API contract and handlers.
- Create `src-tauri/src/host_control.rs`: non-public Host Event subscription and open-intent claim/finish service.
- Create `src-tauri/src/local_ipc.rs`: current-user Unix socket adapter over `tool_api`.
- Modify `src-tauri/src/service/runtime_lock.rs`: preserve the Task 4 anchored full-Core ownership guard and extend its primary-host/socket lifecycle integration.
- Create `src-tauri/src/asr/model_lookup.rs`: minimal model lookup interface used by settings and the static manifest.
- Create `src-tauri/src/acceptance.rs`: hidden desktop acceptance scenarios using the production event loop.
- Create `src-tauri/src/bin/lifesub-asr-gate.rs`: single real-model quality Gate runner.
- Modify `src-tauri/src/service.rs`: crash-safe immutable import and chunk reconciliation.
- Modify `src-tauri/src/commands.rs`: settings, model, job, retranscription, status commands.

### Rust Tests And Fixtures

- Create `src-tauri/src/catalog_migration_test.rs`: sequential fresh/v1->v2, v2->v3 and v3->v4 fingerprint, rollback and concurrency tests owned by Tasks 2, 6 and 11 respectively.
- Create `src-tauri/src/asr_settings_test.rs`: provider-specific settings validation.
- Create `src-tauri/src/asr_model_manager_test.rs`: interrupted download, integrity, extraction, reconciliation.
- Create `src-tauri/src/asr_audio_test.rs`: declared formats, resampling, VAD partition and timestamps.
- Create `src-tauri/src/asr_job_test.rs`: claim/lease/fencing/cancel/recovery/atomic publish tests.
- Create `src-tauri/src/asr_runtime_test.rs`: opt-in real SenseVoice/Whisper/Qwen3-ASR fixture tests.
- Create `src-tauri/src/host_control_test.rs`: requester/claimer separation, event replay, consent CAS and crash recovery tests.
- Create `tests/fixtures/asr/fixture-manifest.json`: hashes, transcripts, intervals, phrases, licenses.
- Create `tests/fixtures/asr/zh.wav`, `en.wav`, `zh-en.wav`: redistributable fixed speech samples.
- Create `tests/fixtures/models/qwen17-bundle-v2.json`: shipping manifest version 2 RFC 8785/JCS golden payload with identity `8a5c16d08be3c49e638689b6438a9a3be9d5d732e49f904d2c0666d5229c995a`.
- Create `tests/fixtures/catalog/lifesub-v0.1.sqlite3`: pre-v2 migration fixture.
- Create `tests/fixtures/catalog/lifesub-v0.2.sqlite3`: immutable pre-v3 migration fixture.
- Create `tests/fixtures/catalog/lifesub-v0.3.sqlite3`: immutable pre-v4 migration fixture.
- Create `tests/fixtures/catalog/lifesub-v0.4.sqlite3`: final v4 fingerprint fixture.
- Create `tests/fixtures/tool-api/agent-v1/*.json`: exact Agent V1 request/response/error golden fixtures.
- Create `tests/fixtures/tool-api/application-v1/*.json`: exact Application V1 request/response/error golden fixtures.
- Create `tests/fixtures/tool-api/gateway/*.json`: MCP mapping and sanitizer contract fixtures only.
- Create `tests/fixtures/tool-api/host-control-v1/*.json`: internal Host Event and claim/complete/uncertain golden frames.
- Create `tests/fixtures/code-signing/*`: test-only authorized and forged ad-hoc signed peer fixtures.

### Frontend

- Modify `src/domain.ts`: ASR settings, models, downloads, jobs, receipts, revision provenance.
- Create `src/services/asr.ts`: typed Tauri command client.
- Modify `src/services/lifesub.ts`: import returns chunk/job information instead of demo revision.
- Create `src/components/asr/ProviderSelector.tsx`: SenseVoice/Whisper/Qwen3-ASR segmented control.
- Create `src/components/asr/ModelCardList.tsx`: stable model cards and download controls.
- Create `src/components/asr/AsrSettingsForm.tsx`: language, threads, VAD, ITN/task, auto-transcribe.
- Create `src/components/asr/AsrJobStatus.tsx`: queued, progress, cancel, retry, diagnostic state.
- Modify `src/components/SettingsView.tsx`: replace static demo rows with functional ASR settings.
- Modify `src/components/TranscriptView.tsx`: retranscribe action, provenance, revision selection.
- Modify `src/App.tsx`: load settings/models/jobs, remove demo ASR import path, refresh real revisions.
- Modify `src/styles.css`: token-based ASR controls with fixed dimensions and responsive layouts.
- Modify `src/App.test.tsx`, `src/services/lifesub.test.ts`, `tests/specs/lifesub-v0.1.spec.ts`.
- Create `src/services/asr.test.ts`, `src/components/asr/AsrSettingsForm.test.tsx`.
- Create `tests/specs/lifesub-real-asr-v0.2.spec.ts`.
- Create `src/acceptance.ts`: desktop-only heartbeat and scenario coordinator.

### Documentation And Release Evidence

- Modify `README.md`, `docs/architecture.md`, `docs/research.md`, `docs/decisions.md`.
- Create `THIRD_PARTY_NOTICES.md`.
- Create `output/asr-v0.2/verification.md` and screenshots after final verification.
- Create `scripts/fetch-sherpa-runtime.sh`: verified native runtime archive fetch.
- Create `scripts/with-sherpa-runtime.sh`: locked, cache-isolated wrapper for native Cargo commands.
- Create `scripts/fetch-sherpa-runtime.test.sh` and `scripts/with-sherpa-runtime.test.sh`: archive, cache, lock, and concurrency regressions.
- Create `scripts/verify-asr-gate.sh`: real-model Gate wrapper that rejects missing tests/results.
- Create `scripts/asr-gate-scope.txt`: explicit version-controlled source paths included in the Gate digest.
- Create `scripts/verify-desktop-asr.sh`: production app launch, cancellation, crash recovery, and packaged smoke harness.
- Create `scripts/verify-local-ipc.sh`: real host/client process orchestration including Agent-to-host confirmation flow.
- Create `scripts/verify-packaged-peer-auth.sh`: release `.app` primary/secondary peer-authorization Gate.
- Create `scripts/desktop-asr-scope.txt`: explicit version-controlled production/acceptance paths included in the desktop digest.
- Update `docs/prd/lifesub-real-asr-v0.2/.artifacts/process.md` and `notes.md` after each chunk.

---

## Chunk 1: Persistence, Provenance, And Immutable Input

### Task 1: Pin Dependencies And Prove The Static Runtime Builds

**Files:**

- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/asr/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/asr_runtime_test.rs`
- Create: `scripts/fetch-sherpa-runtime.sh`
- Create: `scripts/with-sherpa-runtime.sh`
- Create: `scripts/fetch-sherpa-runtime.test.sh`
- Create: `scripts/with-sherpa-runtime.test.sh`

- [ ] **Step 1: Write a failing runtime version test**

```rust
#[cfg(feature = "asr-runtime")]
#[test]
fn sherpa_runtime_reports_the_pinned_build() {
    assert_eq!(crate::asr::runtime_version(), "1.13.5");
    assert_eq!(crate::asr::runtime_git_sha1(), "3dc7c569f31ca2cd4a20ed6f7db780327e6714c5");
}
```

- [ ] **Step 2: Run the test and verify the ASR module is missing**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --features asr-runtime asr_runtime_test::sherpa_runtime_reports_the_pinned_build -- --exact`

Expected: FAIL because `crate::asr` and the feature do not exist.

- [ ] **Step 3: Add pinned dependencies and feature boundaries**

Add these dependency families, using exact resolved versions in `Cargo.lock`:

```toml
sherpa-onnx = { version = "=1.13.5", default-features = false, features = ["static"], optional = true }
symphonia = { version = "0.6.1", features = ["aac", "flac", "isomp4", "mp3", "ogg", "vorbis", "wav"] }
rubato = "5.0.0"
reqwest = { version = "0.12", default-features = false, features = ["blocking", "rustls-tls"] }
tar = "0.4"
bzip2 = "0.5"
fs2 = "0.4.3"
unicode-normalization = "0.1"
unicode-segmentation = "1"

[features]
asr-runtime = ["dep:sherpa-onnx"]
desktop = ["dep:tauri", "dep:tauri-plugin-dialog", "asr-runtime"]
```

Expose `runtime_version()` behind `asr-runtime`; keep fake-provider tests available without the native runtime.

Create `scripts/fetch-sherpa-runtime.sh` to download and verify exactly:

```text
URL: https://github.com/k2-fsa/sherpa-onnx/releases/download/v1.13.5/sherpa-onnx-v1.13.5-osx-arm64-static-lib.tar.bz2
Size: 19,862,746 bytes
SHA-256: 339c8fc19bb4b26e118c80792bbc4546eb263040fac36ef0cc027ec29c756b44
```

The fetch script writes outside the repository, verifies size/hash, and prints the archive directory only. Native commands must run through `scripts/with-sherpa-runtime.sh`, which holds an inter-process lock, honors `CARGO_TARGET_DIR`, quarantines stale Cargo prebuilts, forces `sherpa-onnx-sys` to rebuild from the verified archive, and cleans its own quarantine. Direct native Cargo commands are not a valid Gate because the upstream build script may reuse an older extracted cache.

- [ ] **Step 4: Run focused and baseline builds**

Run:

```bash
scripts/with-sherpa-runtime.sh cargo test --manifest-path src-tauri/Cargo.toml --features asr-runtime asr_runtime_test::sherpa_runtime_reports_the_pinned_build -- --exact
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features
scripts/with-sherpa-runtime.sh cargo check --manifest-path src-tauri/Cargo.toml --features desktop
```

Expected: all PASS. Record native build issues in `.artifacts/notes.md` before changing versions.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/asr/mod.rs src-tauri/src/asr_runtime_test.rs src-tauri/src/lib.rs scripts/fetch-sherpa-runtime.sh scripts/with-sherpa-runtime.sh scripts/fetch-sherpa-runtime.test.sh scripts/with-sherpa-runtime.test.sh
git commit -m "build: add static local ASR runtime"
```

### Task 2: Introduce Versioned Catalog v2 Migration

**Files:**

- Create: `src-tauri/src/catalog/migrations.rs`
- Modify: `src-tauri/src/catalog.rs`
- Create: `src-tauri/src/catalog_migration_test.rs`
- Create: `tests/fixtures/catalog/lifesub-v0.1.sqlite3`
- Create: `tests/fixtures/catalog/lifesub-v0.2.sqlite3`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing schema classification tests**

Cover:

```rust
assert_eq!(classify_schema(&empty_db)?, SchemaKind::Fresh);
assert_eq!(classify_schema(&v1_fixture)?, SchemaKind::LegacyV1);
assert_eq!(classify_schema(&unknown_v0)?, SchemaKind::Unknown);
```

The checked-in v1 fixture is immutable: tests copy it before opening and verify its SHA-256 remains unchanged. Also assert rollback when one v2 statement is forced to fail, wrong `user_version`, unknown tables/columns/indexes/FTS tokenizer fail closed, two processes racing migration yield one ownership winner, and reopening v2 is idempotent.

- [ ] **Step 2: Verify the tests fail against the current unversioned migration**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features catalog_migration`

Expected: FAIL because v2 schema fingerprinting and `user_version = 2` do not exist.

- [ ] **Step 3: Implement exact v2 ASR DDL from the approved design**

Use `BEGIN IMMEDIATE`, fingerprint the v1 tables/columns/FTS tokenizer, create:

- `asr_settings`
- `model_installations`
- `model_downloads`
- `asr_jobs` with `claim_generation`
- `provider_receipts`
- `revision_receipts`
- partial unique indexes
- chunk integrity and dual timestamp columns

Support fresh -> v2 and immutable v1 -> v2 in one `BEGIN IMMEDIATE` migration that commits `user_version = 2`. A forced failure leaves original bytes and `user_version` unchanged. Do not infer that every `user_version = 0` database is V0.1. Artifact checkpoints belong to Task 6/v3; Local Tool tables belong to Task 11/v4 and must not be added under v2.

- [ ] **Step 4: Add legacy compatibility assertions**

Assert existing revisions remain readable, keep their provider string, and receive `legacy_unverified`; existing `start_ms/end_ms` values remain unchanged.

- [ ] **Step 5: Run migration and existing Catalog tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features catalog_migration
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features catalog_test
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/catalog.rs src-tauri/src/catalog src-tauri/src/catalog_migration_test.rs src-tauri/src/lib.rs tests/fixtures/catalog
git commit -m "feat: add versioned ASR catalog v2 schema"
```

### Task 3: Add ASR Domain Types And Settings Validation

**Files:**

- Modify: `src-tauri/src/domain.rs`
- Create: `src-tauri/src/asr/settings.rs`
- Create: `src-tauri/src/asr/model_lookup.rs`
- Create: `src-tauri/src/asr_settings_test.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing tagged-settings tests**

```rust
let invalid = AsrSettings::whisper("whisper-base")
    .with_options(AsrProviderOptions::SenseVoice { use_itn: true });
assert_eq!(invalid.validate(&stub_models), Err(AsrSettingsError::ProviderOptionsMismatch));
```

Test thread bounds, language support, model/provider ownership, Whisper translate, SenseVoice ITN, Qwen3-ASR option isolation, and the complete 1.7B capability matrix: unsupported device remains selectable for display but cannot install/execute; supported uninstalled can install but cannot execute; installed-unqualified cannot execute; runtime-qualified can execute.

- [ ] **Step 2: Run the tests and confirm missing types**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features asr_settings`

Expected: FAIL.

- [ ] **Step 3: Implement immutable serialized types**

Add `AsrProviderKind`, `AsrProviderOptions`, `AsrSettings`, `AsrJobState`, `ChunkIntegrityState`, `ProviderReceipt`, `AsrErrorCode`, and validated transcript time ranges. Define a minimal `ModelLookup` trait containing provider ownership, language capability, and separate selectable/installable/executable capability. Active settings validation requires executable capability and returns `model_capability_unavailable` for any missing, corrupt, runtime-incompatible, or device-incompatible installation; Task 3 tests use a stub, and Task 5's static manifest implements it. Persist enums as snake_case strings, never `Debug` output.

- [ ] **Step 4: Keep old API compatibility explicit**

Legacy `TranscriptRevision.provider` remains readable. New revision DTOs add provenance without changing old browser demo fixtures until Task 11.

- [ ] **Step 5: Run tests and commit**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features asr_settings`

```bash
git add src-tauri/src/domain.rs src-tauri/src/asr/settings.rs src-tauri/src/asr/model_lookup.rs src-tauri/src/asr_settings_test.rs src-tauri/src/lib.rs
git commit -m "feat: define validated ASR settings"
```

### Task 4: Make Imported Audio Crash-Safe And Reconciled

**Files:**

- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`
- Modify: `src-tauri/src/service.rs`
- Create/Modify: `src-tauri/src/service/audio_store.rs`
- Create: `src-tauri/src/service/error.rs`
- Create: `src-tauri/src/service/evidence_uri.rs`
- Create: `src-tauri/src/service/runtime_lock.rs`
- Modify: `src-tauri/src/catalog.rs`
- Create/Modify: `src-tauri/src/catalog/chunks.rs`
- Modify: `src-tauri/src/catalog_test.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/service_test.rs`

**Completed baseline:** commits `d355f5c`, `3bae68d` and `2a587c9` completed crash-safe import plus full-Core ownership. `service/runtime_lock.rs` now acquires the canonical-parent lifetime guard before any writable Catalog open/migration/reconciliation; guarded service/command entry points own import and mutation access, second instances fail closed, and reconciliation cannot run through an unguarded AppState path. Tasks 9-15 must extend this baseline rather than recreate ownership.

- [x] **Step 1: Add failing crash-window, integrity and ownership tests**

Test temporary write, final rename, orphan cleanup, missing final file, changed bytes, and re-hash before ASR.

```rust
assert_eq!(catalog.chunk_integrity(&chunk.id)?, ChunkIntegrityState::Missing);
assert_eq!(service.verify_chunk(&chunk.id), Err(ServiceError::InputUnavailable));
```

- [x] **Step 2: Run focused tests and observe direct-final-write/unguarded-owner failures**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features imported_audio`

- [x] **Step 3: Implement durable import and full-Core ownership**

Write and hash a same-directory temp file, `sync_all`, atomic rename, fsync the parent, then insert metadata. Add startup reconciliation and `available/corrupted/missing` behavior. Re-hash immediately before ASR. Acquire and retain the canonical-parent full-Core ownership guard before writable Catalog open/migration/reconciliation; route commands and service mutations through the guarded facade.

- [x] **Step 4: Verify old source preservation, ownership and new failure behavior**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features service_test
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features catalog_test
```

Expected completed baseline: source preservation, crash windows, symlink/directory replacement, orphan grace, unknown integrity, guarded writable open/reconciliation and second-instance fail-closed tests PASS.

- [x] **Step 5: Commit**

```bash
git show --stat d355f5c
git show --stat 3bae68d
git show --stat 2a587c9
```

The completed Task 4 baseline spans the files listed above. Do not create another ownership implementation in Task 9 or Task 11.

### Chunk 1 Checkpoint

- [ ] Update `docs/prd/lifesub-real-asr-v0.2/.artifacts/process.md` with completed Tasks 1-4, exact test results, and the next task.
- [ ] Record migration, native archive, or filesystem issues in `.artifacts/notes.md`.
- [ ] Stage only `process.md` and `notes.md`, then commit them with message `docs: checkpoint ASR persistence work`.

## Chunk 2: Models, Audio Preparation, And Provider Runtime

### Task 5: Freeze Model And VAD Manifests

**Files:**

- Create: `src-tauri/src/asr/manifest.rs`
- Create: `src-tauri/src/asr_manifest_test.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`
- Create: `THIRD_PARTY_NOTICES.md`
- Create: `tests/fixtures/models/qwen17-bundle-v2.json`

- [ ] **Step 1: Write failing manifest contract tests**

Require unique immutable IDs, supported languages, SPDX/license provenance, runtime identity, `qualification_policy` and an explicit installable artifact bundle. Every `ArtifactFile` requires `artifact_id`, source repository/model, provider API endpoint or immutable normalized HTTPS URL, immutable revision, exact byte size, 64-character SHA-256, normalized non-overlapping required path, required flag, direct/extract mode, SPDX, provenance and exact redirect allowlist. Assert RFC 8785 JCS canonical payload schema v1 and SHA-256 identity excluding the identity field itself, with artifacts sorted by bytewise UTF-8 ID. Qwen 1.7B manifest version must be `2`; compare serialization byte-for-byte with `tests/fixtures/models/qwen17-bundle-v2.json` and assert SHA `8a5c16d08be3c49e638689b6438a9a3be9d5d732e49f904d2c0666d5229c995a`. Assert config/index allow only `[www.modelscope.cn]`, each large shard allows `[cdn-lfs-cn-1.modelscope.cn,www.modelscope.cn]`, every redirect hop checks the current artifact-specific list, and a bundle-wide union is rejected. Assert HF canonical endpoint has no query, `26fea…` post-discovery version 1 and `8279…` pre-discovery drafts are both rejected. Assert every shipping model/VAD has an explicit policy: sherpa-backed entries use `structural_with_pinned_runtime`, only Qwen 1.7B uses `runtime_smoke_required`; no null/TODO/zero-hash placeholder or unhandled policy enters the registry. For VAD, assert every `VadManifest` field below, the exact sherpa-onnx version/commit/source-header provenance, and the eight canonical source defaults. Add a separate mutation test for `threshold`, `min_silence_duration_seconds`, `min_speech_duration_seconds`, `max_speech_duration_seconds`, `window_size_samples`, `sample_rate_hz`, `num_threads`, and `provider`; separately mutate the sherpa version, commit, and each source-header path. Also reject NaN/infinity, invalid ranges, empty provenance, a malformed artifact hash, and a required-files list that is not exactly the pinned `silero_vad.onnx` identity.

The RED tests must also prove the exact 1.7B runtime contract: crate version 0.2.2, Git commit `c5ef09646af6278d2ba8b8ceaf543ffb32d1a5dc`, discovered Candle Metal feature wiring, original config requiring top-level `thinker_config`, and rejection of a config containing only top-level `audio_config/text_config`. They must prove the four-row ModelLookup matrix: unsupported device `true/false/false`, compatible uninstalled `true/true/false`, `installed_unqualified` `true/true/false`, and `runtime_qualified` `true/true/true`, each with stable reason codes.

- [ ] **Step 2: Download the exact upstream assets outside the repository and compute hashes**

Use these immutable manifest inputs and do not commit archives or model weights:

```text
SenseVoiceSmall INT8
https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17.tar.bz2
size 163,002,883; sha256 7d1efa2138a65b0b488df37f8b89e3d91a60676e416f515b952358d83dfd347e

Whisper Tiny
https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-tiny.tar.bz2
size 116,204,861; compute and freeze sha256

Whisper Base
https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-base.tar.bz2
size 207,557,382; compute and freeze sha256

Whisper Small
https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-small.tar.bz2
size 639,387,718; compute and freeze sha256

Qwen3-ASR 0.6B INT8
https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25.tar.bz2
size 878,702,423; sha256 393f8a14e2f5fb96746aaab342997a40641001fbd5bf9592a080a8329178ee96

Qwen3-ASR 1.7B
Formal five-file bundle; do not commit any file or model weight:

Official original `Qwen/Qwen3-ASR-1.7B`, ModelScope revision `d69410f1c275f2b0fa60cbb9960edfcdb0ae0aec`:
config.json size 6,194; sha256 2e74a751548b8ad7d7526d29365ad8144c345d8b412b1152d25dc6698452712f
model-00001-of-00002.safetensors size 4,220,320,824; sha256 a4cd1f1a04d90b757dc7f7dd26254e69a013b19e80efe590a83c6a3bde8608d6
model-00002-of-00002.safetensors size 478,200,688; sha256 6e0b9d9e09e2e0238e7ef3cc8a484ab387e91b90f1900bedf88bc92d7929ccfc
model.safetensors.index.json size 64,821; sha256 f994739fe38e5210b9e3e8ce6c6307315e2ceac3cb630e7b7414d69dce520f60

Official Hugging Face `Qwen/Qwen3-ASR-1.7B-hf` tokenizer.json:
size 11,429,653; sha256 fe1fad59be22a41ee293363fcf95fdedbc7c93f3b49270b1d2e18bd1399a7a05
Resolve its exact immutable commit through the provider revision API and verify the existing size/hash. Query parameters such as `?blobs=true` are discovery-only and excluded from canonical source; persist `/resolve/{commit}/tokenizer.json` without query. Freeze config/index allowlist as `[www.modelscope.cn]`, each large shard as `[cdn-lfs-cn-1.modelscope.cn,www.modelscope.cn]`, manifest version 2, golden fixture v2 and expected identity `8a5c16d08be3c49e638689b6438a9a3be9d5d732e49f904d2c0666d5229c995a`. Test every redirect hop against the current artifact's list and reject a bundle-wide union. Reject `26fea…` because it is the same verified endpoints/allowlist under manifest version 1, and reject `8279…` because its pre-discovery allowlist was incomplete. A branch name, floating `main`, missing discovery field, v1 shipping fixture or placeholder identity keeps the registry RED.

Silero VAD
https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/silero_vad.onnx
size 643,854; sha256 9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6
```

Expected ASR archive contents are the model directory, token file, ONNX model file(s), and upstream test WAVs named by the sherpa release; inspect and record exact required relative paths before writing the registry. The VAD asset is the single file `silero_vad.onnx`. The manifest test re-download helper must verify URL, size, SHA-256, and required paths against the registry.

Freeze the VAD config from sherpa-onnx `1.13.5`, commit `3dc7c569f31ca2cd4a20ed6f7db780327e6714c5`. Canonical provenance is `sherpa-onnx/csrc/silero-vad-model-config.h` for `threshold = 0.5`, `min_silence_duration = 0.5`, `min_speech_duration = 0.25`, `max_speech_duration = 20`, and `window_size = 512`; and `sherpa-onnx/csrc/vad-model-config.h` for `sample_rate = 16000`, `num_threads = 1`, and `provider = "cpu"`.

Required executable model files are:

```text
SenseVoice: model.int8.onnx, tokens.txt
Whisper Tiny: tiny-encoder.onnx, tiny-decoder.onnx, tiny-tokens.txt
Whisper Base: base-encoder.onnx, base-decoder.onnx, base-tokens.txt
Whisper Small: small-encoder.onnx, small-decoder.onnx, small-tokens.txt
Qwen3-ASR 0.6B: conv_frontend.onnx, encoder.int8.onnx, decoder.int8.onnx, tokenizer/ directory (verify the complete tokenizer file set before freezing)
Qwen3-ASR 1.7B: config.json, model-00001-of-00002.safetensors, model-00002-of-00002.safetensors, model.safetensors.index.json, tokenizer.json
VAD: silero_vad.onnx
```

- [ ] **Step 3: Inspect and freeze the exact Cargo dependency contract**

Before editing `Cargo.toml`, inspect the pinned commit's `Cargo.toml` and resolved crate metadata. Record the exact Git URL, package name/version, available features, `default-features` behavior, Candle/Metal feature path, target cfg and whether the dependency can be optional. Do not invent feature names. Freeze this graph: `asr-runtime` contains sherpa only; `asr-qwen17-runtime` contains the pinned qwen/Candle/Metal path only; production `desktop` directly includes both or depends on `desktop-full = ["desktop-base", "asr-runtime", "asr-qwen17-runtime"]`. Keep no-default fast tests free of both native runtimes where supported. Run `cargo metadata --locked`, `cargo tree -e features`, `cargo tree -i qwen3-asr`, and inspect every `Cargo.lock` source/checksum; tests fail if source/revision/features drift or desktop omits qwen/Metal.

- [ ] **Step 4: Implement the static registry and golden payload**

```rust
pub struct ModelManifest {
    pub id: &'static str,
    pub manifest_version: &'static str,
    pub bundle: ArtifactBundle,
    pub runtime: RuntimeRequirement,
    pub device: DeviceRequirement,
    pub qualification_policy: QualificationPolicy,
    pub source: ModelSource,
}

pub struct VadManifest {
    pub id: &'static str,
    pub manifest_version: &'static str,
    pub download_url: &'static str,
    pub archive_size_bytes: u64,
    pub archive_sha256: &'static str,
    pub required_files: &'static [RequiredFile],
    pub sherpa_onnx_version: &'static str,
    pub sherpa_onnx_commit: &'static str,
    pub silero_config_source_header: &'static str,
    pub vad_config_source_header: &'static str,
    pub threshold: f32,
    pub min_silence_duration_seconds: f32,
    pub min_speech_duration_seconds: f32,
    pub max_speech_duration_seconds: f32,
    pub window_size_samples: i32,
    pub sample_rate_hz: i32,
    pub num_threads: i32,
    pub provider: &'static str,
}
```

Implement `ModelLookup` for the registry. For the existing persistence field named `archive_sha256`, store a single archive hash for legacy bundles and the canonical manifest SHA-256 for multi-file bundles. `VadManifest::validate()` must require the exact asset URL, `643854` byte size, 64-character pinned SHA-256, and exactly one required file named `silero_vad.onnx`; require non-empty exact version/commit/header provenance; reject non-finite numeric values; require threshold in `(0, 1]`, positive durations, `max_speech_duration_seconds >= min_speech_duration_seconds`, and positive window/sample/thread values; then compare every frozen scalar with named canonical constants, using `f32::to_bits()` for exact float equality. Use a new model ID or manifest version if any artifact, source revision, runtime identity, compatibility rule, provenance, or frozen VAD parameter changes. `200 ms` LifeSub padding and `25 s` orchestration hard split are deliberately absent from `VadManifest`.

- [ ] **Step 5: Pin the runtime and add notices**

Pin the exact inspected Git URL at commit `c5ef09646af6278d2ba8b8ceaf543ffb32d1a5dc` with observed `default-features`/feature wiring in `Cargo.toml`/`Cargo.lock`. Add MIT crate notice, full direct/transitive runtime license closure, and per-artifact source/revision/SPDX/provenance. Add a contract test that reconciles manifest licenses with `THIRD_PARTY_NOTICES.md` and reconciles the locked qwen/Candle dependency closure with notices; undocumented locked runtime dependencies or notices without a locked/manifest source fail.

- [ ] **Step 6: Verify tests and locked dependency evidence**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features asr_manifest
cargo metadata --manifest-path src-tauri/Cargo.toml --locked --format-version 1
cargo tree --manifest-path src-tauri/Cargo.toml --locked -e features
cargo tree --manifest-path src-tauri/Cargo.toml --locked -i qwen3-asr
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/asr/manifest.rs src-tauri/src/asr_manifest_test.rs src-tauri/src/lib.rs src-tauri/Cargo.toml src-tauri/Cargo.lock THIRD_PARTY_NOTICES.md tests/fixtures/models/qwen17-bundle-v2.json
git commit -m "feat: pin local ASR model manifests"
```

### Task 6: Implement Recoverable Model Downloads And Installs

**Files:**

- Create: `src-tauri/src/asr/model_manager.rs`
- Create: `src-tauri/src/asr_model_manager_test.rs`
- Modify: `src-tauri/src/catalog.rs`
- Modify: `src-tauri/src/catalog/migrations.rs`
- Modify: `src-tauri/src/catalog_migration_test.rs`
- Test: `tests/fixtures/catalog/lifesub-v0.2.sqlite3`
- Create: `tests/fixtures/catalog/lifesub-v0.3.sqlite3`

- [ ] **Step 1: Write failing HTTP fixture tests**

Cover interrupted single-file and five-file bundle downloads, restart/resume from persisted byte checkpoints, Range accepted/ignored, ETag or Last-Modified changes, incorrect content length, redirect to a disallowed host, one corrupt shard among otherwise valid artifacts, duplicate/escaping required paths, path traversal, symlink/hardlink archive entries, expanded-size limit, cancellation and explicit retry. Assert completed verified shards are reused only when their manifest source identity still matches.

Cover disk preflight for the worst simultaneous footprint: remaining `.part` bytes + completed temporary artifacts + extracted/copied staging + complete final directory + safety margin, while retaining the current installed version. Cover DB-before-directory mismatch, incomplete bundle marker, structural compatibility failure, cancellation, and deletion while leased. Assert non-M4/<24GB/device-incompatible Qwen 1.7B rejects before a `model_downloads` row/network request; supported M4/24GB but uninstalled Qwen 1.7B is accepted for download.

Add policy matrix tests: every `structural_with_pinned_runtime` model/VAD with matching sherpa identity ends `runtime_qualified` in the structural publication transaction; mismatched runtime identity publishes no installation and returns `model_runtime_identity_mismatch`; Qwen 1.7B `runtime_smoke_required` ends only `installed_unqualified`; unknown policy fails closed.

Add rename-success-before-DB crash tests for both policies. Reconciliation must re-hash all files and validate the structural marker. For `structural_with_pinned_runtime`, exact pinned sherpa identity atomically restores `runtime_qualified`; a wrong tag/commit/native archive/build identity creates no installation, records `model_runtime_identity_mismatch`, and quarantines the directory. For `runtime_smoke_required`, reconciliation restores only `installed_unqualified` and leaves Task 8 to qualify it. Assert neither path silently chooses the other policy.

- [ ] **Step 2: Verify failure**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features asr_model_manager`

- [ ] **Step 3: Write and verify the Catalog v3 migration**

Task 6 exclusively migrates v2 -> v3 and owns `model_download_artifacts` plus the rebuilt `model_installations` two-stage states. Implement the exact DDL/FKs/PK/unique/state/bytes/hash/source/checkpoint fields from the design, update schema fingerprinting, and support fresh/v1/v2 -> complete v3 without changing historical v2. The immutable v2 fixture is copied before migration; create an immutable v3 fixture and golden fingerprint. Cover forced rollback, unknown v2/v3 shapes, concurrent migration, `user_version = 3`, and idempotent v3 reopen. Task 11 must consume v3 and migrate to v4; it may not append DDL under version 3.

Run: `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features catalog_migration`

- [ ] **Step 4: Implement persistent download state**

Use `model_downloads` for bundle-level `queued/downloading/verifying/installing/succeeded/failed/cancelled`, plus persistent per-artifact checkpoint records keyed by download ID and required path. Persist byte progress, validators and verified state at bounded intervals; compute UI progress from the sum of exact artifact bytes; keep user-facing errors separate from diagnostic summaries.

- [ ] **Step 5: Implement safe structural installation**

Install to:

```text
models/asr/<provider>/<model-id>/<manifest-version>-<bundle-identity>/
```

Reject unsafe archive entries and unsafe/overlapping direct-install paths; assemble all artifacts in one staging directory; verify every file/hash and policy-specific structural contract; write an immutable structural marker with per-file provenance/runtime requirements; fsync and rename once. For `structural_with_pinned_runtime`, verify the exact sherpa 1.13.5 tag/commit/native archive/current build identity and publish `runtime_qualified` in the same SQLite transaction as the installation. For `runtime_smoke_required`, verify top-level `thinker_config`, shard index coverage and tokenizer static contract, then publish `installed_unqualified`; runtime initialization is forbidden in Task 6. Tests enumerate SenseVoice, all Whisper variants, Qwen 0.6B and Silero VAD to prove none remains unqualified. Reconcile per-file `.part` checkpoints, staging, unrecorded structural installs, incomplete markers, missing directories, and corrupt files without publishing a partial bundle.

- [ ] **Step 6: Run model manager and migration tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features asr_model_manager
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features catalog_migration
scripts/with-sherpa-runtime.sh cargo test --manifest-path src-tauri/Cargo.toml --features asr-runtime asr_model_manager
```

Expected: PASS, including real pinned sherpa identity publication and wrong-identity rejection/quarantine.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/asr/model_manager.rs src-tauri/src/asr_model_manager_test.rs src-tauri/src/catalog.rs src-tauri/src/catalog/migrations.rs src-tauri/src/catalog_migration_test.rs tests/fixtures/catalog/lifesub-v0.2.sqlite3 tests/fixtures/catalog/lifesub-v0.3.sqlite3
git commit -m "feat: add recoverable ASR model installs"
```

### Task 7: Decode, Downmix, Resample, And Partition Audio

**Files:**

- Create: `src-tauri/src/asr/audio.rs`
- Create: `src-tauri/src/asr/vad.rs`
- Create: `src-tauri/src/asr_audio_test.rs`
- Create: `src-tauri/src/asr_vad_test.rs`
- Create/Modify: `tests/fixtures/asr/*`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add redistributable fixture manifest and failing decode tests**

Provide source/license/hash metadata. Include fixtures for every format the UI claims. If Symphonia cannot reliably decode one current format, remove that format from the UI and docs in the same task.

- [ ] **Step 2: Add failing time invariant tests**

```rust
assert!(segments.windows(2).all(|pair| pair[0].end_ms <= pair[1].start_ms));
assert!(segments.iter().all(|s| 0 <= s.start_ms && s.start_ms < s.end_ms));
assert!(segments.iter().all(|s| s.end_ms <= decoded.duration_ms));
```

Keep decode/timing cases in `asr_audio_test.rs`. In `asr_vad_test.rs`, test the exact validated manifest-to-sherpa field mapping under `asr-runtime`, plus 200 ms LifeSub padding, 25-second maximum orchestration windows, and hard-split fallback. Assert that padding and hard-split constants are not fields in `VadManifest`, do not replace `max_speech_duration_seconds = 20`, and do not alter the config passed to sherpa-onnx.

- [ ] **Step 3: Implement decoding and timing**

Decode with Symphonia, downmix to `f32`, resample with Rubato, retain original frame coordinates, and use floor-start/ceil-end conversion.

- [ ] **Step 4: Implement VAD boundary orchestration**

Wrap the pinned sherpa VAD when `asr-runtime` is enabled. Construct `SileroVadModelConfig` and `VadModelConfig` by explicitly assigning every validated `VadManifest` field: model path, threshold, all three durations, window size, sample rate, thread count, and provider. Do not use `Default::default()`, `..Default::default()`, or zero/empty placeholders because the Rust wrapper's derived defaults are not the canonical C++ source defaults. Provide a deterministic fake detector for fast tests. Apply LifeSub's 200 ms padding and 25-second energy-aware/hard-split policy only after VAD detection as Task 7 orchestration; Evidence ranges use non-overlapping core intervals, not padded inference context.

- [ ] **Step 5: Run tests and update the import allowlist**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features asr_audio
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features asr_vad
cargo test --manifest-path src-tauri/Cargo.toml --features asr-runtime asr_vad
npm test -- --run src/App.test.tsx
```

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/asr/audio.rs src-tauri/src/asr/vad.rs src-tauri/src/asr_audio_test.rs src-tauri/src/asr_vad_test.rs src-tauri/src/lib.rs tests/fixtures/asr src/App.tsx src/App.test.tsx docs/prd/lifesub-real-asr-v0.2/PRD.md
git commit -m "feat: prepare timestamped ASR audio"
```

### Task 8: Implement SenseVoice, Whisper, And Qwen3-ASR Providers

**Files:**

- Create: `src-tauri/src/asr/provider.rs`
- Create: `src-tauri/src/asr/sense_voice.rs`
- Create: `src-tauri/src/asr/whisper.rs`
- Create: `src-tauri/src/asr/qwen3_asr.rs`
- Create: `src-tauri/src/asr/runtime_qualifier.rs`
- Create: `src-tauri/src/asr_provider_test.rs`
- Create: `src-tauri/src/asr_runtime_qualifier_test.rs`
- Modify: `src-tauri/src/asr/manifest.rs`
- Modify: `src-tauri/src/asr/model_manager.rs`
- Modify: `src-tauri/src/catalog.rs`
- Modify: `src-tauri/src/asr_manifest_test.rs`
- Modify: `src-tauri/src/asr_settings_test.rs`
- Modify: `src-tauri/src/asr_model_manager_test.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`

- [ ] **Step 1: Write fake-provider contract tests**

Test provider/model/runtime identity, language mapping, SenseVoice ITN, Whisper task, Qwen3-ASR parameters, exact 0.6B sherpa dispatch, exact 1.7B Candle/Metal dispatch, empty-output rejection, cancellation between windows, and error mapping without loading native models.

Freeze the language matrix against the actual runtime APIs:

- Whisper accepts `auto` or a concrete manifest-supported runtime language code. Remove the `multilingual` pseudo-value from `LANG_WHISPER`; it describes model capability but is not passed to `OfflineWhisperModelConfig.language`, Job parameters or Receipt metadata.
- sherpa 1.13.5 `OfflineQwen3ASRModelConfig` has no language field. Change the 0.6B manifest capability to `auto` only; explicit language must fail in Settings and provider construction with `invalid_provider_parameter`, and tests must prove it is not copied into `hotwords` or inert runtime metadata.
- Qwen 1.7B maps `auto` to `TranscribeOptions.language = None` and each manifest code to the frozen crate prompt names from the design; unknown or unsupported codes fail before native inference.

Assert 1.7B never constructs the sherpa adapter and 0.6B never constructs the Candle adapter; no failure path substitutes either model, runtime family, backend or device. Add a regression test that rejects `qwen3_asr::best_device()` semantics: simulated Metal construction failure must return an error and must not construct/load with `Device::Cpu`.

Add RuntimeQualifier orchestration tests for 1.7B: start from `installed_unqualified`; ModelManager invokes a pure adapter smoke, fsyncs a temp marker, atomically publishes the marker, then uses Catalog CAS `installed_unqualified -> runtime_qualified`. Assert Provider never receives Catalog/DB handles and cannot write state. Cover smoke failure, marker-write/fsync/rename failure, crash before marker, crash after durable marker before CAS, DB qualified but marker missing/mismatched, concurrent qualifiers, idempotent same-identity retry and conflicting identity. Failure/recovery keeps or restores `installed_unqualified`, records stable qualification errors and never marks valid files corrupt.

- [ ] **Step 2: Run tests and verify missing providers**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features asr_provider`

- [ ] **Step 3: Implement the provider boundary**

```rust
pub trait AsrProvider: Send {
    fn identity(&self) -> &ProviderIdentity;
    fn transcribe(
        &self,
        audio: AudioSlice<'_>,
        request: &AsrRequest,
        cancellation: &CancellationToken,
    )
        -> Result<AsrText, AsrError>;
}
```

Providers receive validated PCM and do not read settings, select fallback, write SQLite, or assign revision numbers. The token is checked before and after each synchronous native call and between Task 7 windows; there is no native-call preemption. Because every orchestration window is at most 25 seconds, the maximum cancellation contract is the current single window plus boundary overhead, not a separate 30-second promise.

- [ ] **Step 4: Implement exact adapters behind their separate runtime features**

Map SenseVoice, Whisper and Qwen 0.6B manifest files into `OfflineSenseVoiceModelConfig`, `OfflineWhisperModelConfig`, and `OfflineQwen3ASRModelConfig`. Enable only the runtime-supported language/task/ITN values and capture sherpa runtime version/build identity. Do not invent a Qwen 0.6B language field or treat language as hotwords/metadata.

For Qwen 1.7B, add a direct target-specific optional dependency `candle-core = "=0.9.2"` with `default-features = false` and `features = ["metal"]`; `asr-qwen17-runtime` must own both `dep:qwen3-asr` and `dep:candle-core`. The adapter loads only the structurally installed five-file bundle through the Task 5-inspected `qwen3-asr` crate 0.2.2 at Git commit `c5ef09646af6278d2ba8b8ceaf543ffb32d1a5dc`, calls `candle_core::Device::new_metal(0)` directly, verifies `device.is_metal()` plus the actual backend/device identity, binds to the qualified M4/24GB Metal device, runs a fixed short smoke and returns crate/git/Candle/backend/target/device identity. It must not call `qwen3_asr::best_device()`, whose upstream contract falls back to CPU. `RuntimeQualifier`, not the adapter, owns marker durability, Catalog CAS and reconciliation through ModelManager. Provider Factory only consumes a matching `runtime_qualified` installation/marker and never performs qualification. Neither adapter may fallback to the other.

- [ ] **Step 5: Run provider tests and compile both feature sets**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features asr_provider
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features asr_manifest
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features asr_settings
scripts/with-sherpa-runtime.sh cargo test --manifest-path src-tauri/Cargo.toml --features asr-runtime asr_provider
scripts/with-sherpa-runtime.sh cargo test --manifest-path src-tauri/Cargo.toml --features asr-qwen17-runtime asr_runtime_qualifier
scripts/with-sherpa-runtime.sh cargo check --manifest-path src-tauri/Cargo.toml --features 'asr-runtime,asr-qwen17-runtime'
scripts/with-sherpa-runtime.sh cargo check --manifest-path src-tauri/Cargo.toml --features desktop
cargo metadata --manifest-path src-tauri/Cargo.toml --locked --format-version 1
cargo tree --manifest-path src-tauri/Cargo.toml --locked -e features
cargo tree --manifest-path src-tauri/Cargo.toml --locked -e features --features asr-qwen17-runtime -i candle-core@0.9.2
```

The feature graph contract is: `asr-runtime` enables sherpa only; `asr-qwen17-runtime` enables the pinned qwen crate plus LifeSub's direct optional `candle-core 0.9.2/metal` dependency; production `desktop` must include both (or depend on a named `desktop-full` that includes both). The metadata/tree assertion must prove the selected production feature contains exactly the intended qwen/Candle Metal path, no CUDA/hub feature, and a LifeSub-owned direct candle-core edge; a desktop build containing only sherpa or only an indirect Candle dependency fails Task 8 and Task 15.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/asr/provider.rs src-tauri/src/asr/sense_voice.rs src-tauri/src/asr/whisper.rs src-tauri/src/asr/qwen3_asr.rs src-tauri/src/asr/runtime_qualifier.rs src-tauri/src/asr/manifest.rs src-tauri/src/asr_provider_test.rs src-tauri/src/asr_runtime_qualifier_test.rs src-tauri/src/asr_manifest_test.rs src-tauri/src/asr_settings_test.rs src-tauri/src/asr/model_manager.rs src-tauri/src/asr_model_manager_test.rs src-tauri/src/catalog.rs src-tauri/src/lib.rs src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "feat: add local ASR providers"
```

### Chunk 2 Checkpoint

- [ ] Update `docs/prd/lifesub-real-asr-v0.2/.artifacts/process.md` with completed Tasks 5-8 and model/audio/provider evidence.
- [ ] Record asset, license, decoder, VAD, or runtime issues in `.artifacts/notes.md`.
- [ ] Commit only those two files with message `docs: checkpoint ASR provider work`.

## Chunk 3: Fenced Jobs, Atomic Revisions, And Desktop Commands

### Task 9: Implement The Fenced ASR Job State Machine

**Files:**

- Create: `src-tauri/src/asr/job.rs`
- Create: `src-tauri/src/asr_job_test.rs`
- Modify: `src-tauri/src/catalog.rs`
- Modify: `src-tauri/src/catalog_migration_test.rs`
- Modify: `src-tauri/src/catalog_migration_test/*`
- Modify: `src-tauri/src/domain.rs`
- Modify: `src-tauri/src/asr_settings_test.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing claim and recovery tests**

Cover:

- exclusive worker authority comes from the existing Task 4 full-Core lifetime guard; no separate `asr-worker.lock`
- claim CAS changes `queued -> preparing`
- `attempt_count` and `claim_generation` increment together
- lease 30 seconds and renewal every 5 seconds/stage
- stale boot ID recovery within 5 seconds
- 5-second and 30-second retry backoff
- maximum 3 total claims
- queued/blocked cancellation
- claim excludes `missing/corrupted` chunks even if a queued row already exists
- all Job timestamps use canonical UTC RFC 3339 milliseconds
- model-ready leaves `blocked_model` unchanged and only projects ready-to-retry
- explicit Application retry opens a new manual execution generation on the same Job ID
- retry generation resets `attempt_count` to zero, increments `claim_generation`, preserves the immutable settings/fingerprint and cannot violate active uniqueness
- retry accepts only ready `blocked_model` or `failed`; `cancelled` requires enqueue/retranscribe
- third-claim recovery exhaustion writes stable `AsrErrorCode::RecoveryRetryExhausted`
- Task 9 has no path that writes `succeeded`

Add schema-ownership assertions in `catalog_migration_test.rs` and its contract/concurrency submodules: Job fields already exist in v2; Task 9 leaves DDL, `user_version` and migration fingerprints unchanged, Task 6 remains the sole v3 owner, and Task 11 remains the sole v4 owner. These are no-drift/ownership assertions only; Task 9 must not edit migration DDL or fixtures.

- [ ] **Step 2: Add the stale-worker fencing test**

```rust
let first = jobs.claim("boot-a", "worker-1")?.unwrap();
jobs.expire_and_requeue(&first.id)?;
let second = jobs.claim("boot-a", "worker-2")?.unwrap();
assert!(jobs.mark_transcribing(&first.claim).is_err());
assert!(jobs.mark_transcribing(&second.claim).is_ok());
```

Also prove that claim/recovery consumes the existing Task 4 full-Core lifetime guard from `service/runtime_lock.rs`; repository construction without that guard is impossible or returns an ownership error. Task 9 must not create a new runtime lock, owner type or lock file. A stale generation cannot renew, mark transcribing, fail, cancel or participate in Task 10 publication.

- [ ] **Step 3: Run tests and verify the Job API is missing**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features asr_job`

Expected: FAIL because the Job repository/state-transition API does not yet consume the existing Task 4 ownership guard. The guard and v2 Job schema already exist; do not create a new ownership implementation, Task 9 migration or v3/v4 schema change.

- [ ] **Step 4: Implement ownership-fenced transitions**

Every renewal and running-state transition uses `id + claimed_by + claim_generation`; claim additionally joins/checks `chunks.integrity_state = 'available'`. A zero-row update means ownership is lost and in-memory results must be discarded. Normalize every stored timestamp to UTC RFC 3339 milliseconds and inject a deterministic clock in tests.

Model-ready is read-side capability information only. `retry_asr_job` is an explicit user/Application mutation: for a ready `blocked_model` or `failed` row, one CAS reuses the same Job ID, increments `claim_generation` immediately to fence old workers, resets the per-generation `attempt_count`, clears owner/lease/cancel and active error fields, and queues the immutable snapshot. Task 11 later stores the exact old state/new generation in its operation/replay row. Do not retry `cancelled`, auto-queue model-ready jobs or insert a duplicate active fingerprint.

Task 9 exposes claim, renew, mark-transcribing, fail, cancel, recovery and manual-retry-generation operations only. Add `RecoveryRetryExhausted` to the stable `AsrErrorCode` serde set. Do not expose `complete()` or update `succeeded`; Task 10 exclusively owns the fenced atomic Receipt/Revision/Segment/FTS/succeeded transaction.

- [ ] **Step 5: Run tests and commit**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features asr_job
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features asr_settings
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features catalog_migration_test
```

Expected: Job and error-code contracts PASS, and migration tests prove Task 9 did not change v2/v3/v4 ownership or schema fingerprints.

```bash
git add src-tauri/src/asr/job.rs src-tauri/src/asr_job_test.rs src-tauri/src/catalog.rs src-tauri/src/catalog_migration_test.rs src-tauri/src/catalog_migration_test src-tauri/src/domain.rs src-tauri/src/asr_settings_test.rs src-tauri/src/lib.rs
git commit -m "feat: add fenced ASR jobs"
```

### Task 10: Publish Receipts And Revisions Atomically

**Files:**

- Create: `src-tauri/src/asr/service.rs`
- Create: `src-tauri/src/asr_service_test.rs`
- Modify: `src-tauri/src/catalog.rs`
- Modify: `src-tauri/src/domain.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing atomic publication tests**

Assert one transaction inserts Receipt, Revision, revision_receipts, Segments, FTS rows, compatible `start_ms/end_ms`, and `succeeded`. Force each insert to fail and assert no partial Evidence is visible.

Add a direct Core enqueue test that passes a `ModelLookup` entry with `executable = false` and asserts `model_capability_unavailable`, zero `asr_jobs` inserts, and no Provider construction. This test must call the service/job API directly, not a desktop command.

- [ ] **Step 2: Write cancellation and stale-generation race tests**

Cancel before the transaction: no revision. Cancel after commit: revision remains and Job is succeeded. A stale generation cannot insert a receipt.

- [ ] **Step 3: Implement the service orchestration**

The service verifies executable model capability before inserting or claiming a Job, then verifies chunk hash, resolves exact ASR/VAD artifacts, decodes audio, transcribes windows, rejects empty results, and publishes in `BEGIN IMMEDIATE` with fencing and `cancel_requested_at IS NULL`.

- [ ] **Step 4: Preserve complete time provenance**

New Segment writes must satisfy:

```text
start_ms == session_start_ms
end_ms == session_end_ms
session_start_ms == chunk.session_offset_ms + chunk_start_ms
session_end_ms == chunk.session_offset_ms + chunk_end_ms
```

- [ ] **Step 5: Run tests and commit**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features asr_service
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features catalog_test
```

```bash
git add src-tauri/src/asr/service.rs src-tauri/src/asr_service_test.rs src-tauri/src/catalog.rs src-tauri/src/domain.rs src-tauri/src/lib.rs
git commit -m "feat: publish traceable ASR revisions"
```

### Task 11: Build The CoreRuntime And Versioned Local Tool API

**Files:**

- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Create: `src-tauri/src/commands_test.rs`
- Create: `src-tauri/src/desktop_api.rs`
- Create: `src-tauri/src/core_runtime.rs`: extract/migrate the Task 4 guarded owner into the primary runtime host; do not create a second ownership mechanism.
- Create: `src-tauri/src/tool_api.rs`
- Create: `src-tauri/src/host_control.rs`
- Create: `src-tauri/src/host_control_test.rs`
- Create: `src-tauri/src/local_ipc.rs`
- Create: `src-tauri/src/local_ipc_test.rs`
- Modify: `src-tauri/src/service/runtime_lock.rs` to integrate the existing full-Core guard with the final CoreRuntime/socket host; do not introduce `src-tauri/src/runtime_lock.rs` as a second owner.
- Create: `src-tauri/src/bin/lifesub-ipc-test-client.rs`
- Create: `src-tauri/src/bin/lifesub-ipc-test-host.rs`
- Create: `src-tauri/src/bin/lifesub-two-tauri-harness.rs`
- Create: `scripts/verify-local-ipc.sh`
- Create: `src-tauri/src/tool_api_test.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/catalog/migrations.rs`
- Modify: `src-tauri/src/catalog/migrations/ddl.rs`
- Modify: `src-tauri/src/catalog/migrations/fingerprint.rs`
- Modify/Create: `src-tauri/src/catalog_migration_test/*`
- Modify: `src-tauri/src/service.rs`
- Test: `tests/fixtures/catalog/lifesub-v0.3.sqlite3`
- Create: `tests/fixtures/catalog/lifesub-v0.4.sqlite3`
- Modify: `docs/superpowers/specs/2026-08-16-lifesub-local-tool-api-design.md`
- Create: `tests/fixtures/tool-api/agent-v1/*.json`
- Create: `tests/fixtures/tool-api/application-v1/*.json`
- Create: `tests/fixtures/tool-api/gateway/*.json`
- Create: `tests/fixtures/tool-api/host-control-v1/*.json`: internal claim/complete/uncertain frames and errors; not public contract fixtures.
- Create: `tests/fixtures/code-signing/*`: ad-hoc signed authorized and forged peer identity fixtures for test-only Security.framework authorization.

- [ ] **Step 1: Freeze and test both V1 contracts**

Freeze the public Agent Tool Contract V1 as exactly `get_capabilities`, `start_capture`, `get_capture_status`, `stop_capture`, `get_asr_job_status`, `search_transcripts`, `resolve_evidence`, and `open_evidence`. For every method assert the exact request fields, response fields, capability and allowed error set from the Local Tool design. V0.2 advertises native capture unsupported; all three capture methods return `unsupported_capability(native_capture)` and create zero device/outbox/session/chunk/job side effects.

Separately freeze the 16-method Application-only V1 management surface: `import_audio`, settings read/save, model list/download/cancel/delete, job enqueue/retry/cancel/retranscribe, operation get/list, revision list/get and receipt list. Every method has the exact named Request/Response DTO, field constraints, capability and error set from the Local Tool design. All async import/model/job mutations return `OperationSummary`; freeze its state/progress/result/error/timestamp fields, require `operation_in_progress` to expose the existing operation ID, and prove `get_operation` observes succeeded/failed/cancelled/recovery_required terminal states while `list_operations` exposes recoverable history with stable ordering/cursor semantics. Both contracts share envelope/error/DTO primitives and exact golden JSON under `tests/fixtures/tool-api/{agent-v1,application-v1}`.

The complete Tauri UI surface is Application-only V1 plus the trusted UI projection of Agent V1 `get_capabilities`, `get_capture_status`, `get_asr_job_status`, `search_transcripts`, `resolve_evidence` and `open_evidence`. Tauri commands, in-process adapter and authorized secondary-Tauri IPC map one-to-one to those existing methods; tests reject hidden settings/job-read/search/open Commands or a third contract. Freeze Application list ordering, final ID tie-breakers, limit 1..50 and all four cursor errors.

Requests contain no caller/capability authority. Tests prove `caller_kind = tauri_ui` or injected capabilities in ordinary UDS JSON are rejected. Dispatch receives only server-created `TrustedCallerContext`: ordinary `agent.sock` is fixed `local_agent`; in-process host injects `tauri_ui`; `ui.sock` grants `tauri_ui` only after `getpeereid` current UID plus `LOCAL_PEERTOKEN` audit token and `SecCodeCopyGuestWithAttributes`/`SecCodeCheckValidity` validation against the primary's pinned designated requirement, Team ID and bundle ID; Gateway mapping fixes `gateway`. Same UID alone is insufficient. Missing/forged audit token, signature mismatch, unsigned/debug build or Security.framework failure must fail closed or reconnect as `local_agent`, never elevate. Only a test-harness build may accept the checked-in ad-hoc signed test identity; test both authorized and forged fixtures. `EvidenceOpener` accepts only trusted host claim context and Core-internal validated Evidence target, never Agent response data.

Freeze MAC'd cursor behavior: search ordering is `score DESC, session_start_ms DESC, revision_id ASC, segment_id ASC`; cursor binds contract/method/principal/query/filters/limit/keyset/high-watermark/expiry/Catalog epoch. Test ties, tamper, caller/query/limit mismatch, expiry, inserts, deletes and stale epoch.

- [ ] **Step 2: Test Catalog v4 and mutation recovery**

Extend migration tests for fresh/v1/v2/v3 -> v4 using immutable checked-in fixtures, rollback with unchanged old bytes and `user_version`, final `user_version = 4`, fingerprint fail-closed, concurrent two-process migration and v4 reopen. Task 6's v3 fixture and `model_download_artifacts`/two-stage installation DDL are immutable inputs: Task 11 must preserve them byte-for-byte at the schema-contract level and may not append or alter tables under `user_version = 3`. Catalog v4 exclusively adds `tool_requests`, durable operations/outbox, open-intent ledger and schema/cursor epoch.

Before implementation, update every Catalog v3 reference in the Local Tool design to Catalog v4, including `tool_requests`, Host Control ledger, migration paths, acceptance text and `user_version`. This documentation change is part of Task 11; no code may implement Local Tool DDL while its referenced design still calls it v3.

For SQLite-only mutations, test one transaction commits business state plus exact replay result; cover identical concurrent `in_progress`, changed fingerprint, cancel-before-commit rollback, crash-before-commit retry, crash-after-commit-before-response replay, and restart recovery. For `import_audio` and model operations, test accept transaction -> operation/outbox -> idempotent executor checkpoints, cancel before/after accept and publish checkpoints, and restart recovery without duplicate file/device effects. `import_audio` returns operation/session/chunk/optional Job and never appends `demo-local` text.

Test `open_evidence` as intent issuance only: `confirmation_required` is a successful disposition, not an error, and the requester response contains only intent ID/disposition/expiry. Add non-public Host Event + Host Control Protocol V1 in `host_control.rs`, excluded from Agent 8/Application 16. Core pushes `PendingOpenEvidenceEvent { event_id, intent_id, requesting_principal_id/kind, evidence_ref, display_metadata, expires_at }` only to authorized in-process event sinks or `ui.sock` subscriptions; it contains no claim token, raw transcript or path. Authorized host-only controls are `claim_open_intent(intent_id)`, `complete_open_intent(intent_id)` and `mark_open_intent_uncertain(intent_id, diagnostic_id)`. Primary in-process Tauri calls the same service; secondary uses internal frames over authorized `ui.sock`; ordinary Agent/Gateway cannot subscribe or route controls. CoreRuntime alone serializes Catalog v4 ledger writes, so adapters never write DB.

Freeze/test `pending -> executing -> consumed | uncertain` plus pending -> expired CAS transactions. Ledger separately stores immutable requesting principal/evidence binding and authorized Tauri claim principal; they need not match. Claim verifies the event/requester/evidence were not altered and the caller owns Core-held subscription/in-process delivery capability, not a bearer token. Cover auditable consent timestamp, exact idempotent retries, conflicting outcome errors, concurrent claim single winner, host offline/event loss followed by internal pending replay on subscription resume, expiry removal, and crash after executing claim before finish -> uncertain without opening UI. A new public `open_evidence` intent plus fresh confirmation is then required. There is no second public Agent confirm tool or Application confirm method.

- [ ] **Step 3: Extend existing ownership into secure IPC**

Extract/migrate Task 4's existing guarded owner into one primary `CoreRuntime`; do not reacquire, duplicate or redefine ownership. Preserve the `service/runtime_lock.rs` contract that the lifetime guard precedes any writable Catalog open/migration/reconciliation, then extend the same owner to socket bind, model/capture mutation and workers. Primary hosts the guard and sockets; secondary never opens writable Catalog or creates an owner and must route through authorized `ui.sock`/`agent.sock`. Run native inference on a blocking thread, never the UI thread.

Create the runtime directory relative to an anchored parent fd with `openat(O_DIRECTORY|O_NOFOLLOW)`, `fstat`/`fstatat`/`lstat` owner/type/mode checks, `0700` directory and `0600` sockets. Ordinary `agent.sock` uses mandatory `getpeereid` and minimal authority. Controlled `ui.sock` obtains `audit_token_t` with `LOCAL_PEERTOKEN`, resolves peer code via Security.framework and checks the primary-pinned designated requirement, Team ID and bundle ID. Document the Rust FFI/framework wrapper boundary in `local_ipc.rs`; production has no unsigned/debug bypass. Authentication failure refuses Application/opener authority; the client may separately reconnect to `agent.sock` for minimal Agent reads.

Use 4-byte framing with 1 MiB request/4 MiB response limits, max 8 in-flight per connection and 32 globally, bounded queues, 10-second read/write deadlines and method execution deadlines. Define `{ control: cancel, request_id }` as a transport control frame, not a business method; it cannot reverse a passed commit point. Secondary startup retries primary connection with 25/50/100/200/400 ms jittered backoff, capped at 2 seconds, then fails without opening DB.

Only the ownership lock holder may remove a socket. Treat only `ENOENT` or `ECONNREFUSED` as stale candidates; timeout, `EMFILE`, `ENFILE`, `EACCES`, `EPERM`, resource exhaustion, protocol errors and all other uncertain results fail closed. Before unlink after `ECONNREFUSED`, anchored revalidation must prove unchanged device/inode, current UID, socket type and mode. Add malicious replacement/live socket/concurrent start/half frame/oversize/slow read/slow write/limit/startup retry/stale-probe decision/ordered shutdown tests. Ordered shutdown stops accept, drains 5 seconds, cancels pre-commit work, persists recovery markers, unlinks endpoints, closes Core/Catalog, then releases the lock. No TCP listener.

- [ ] **Step 4: Prove real secondary-Tauri behavior and contract fixtures**

Register Tauri commands only as Application V1 or trusted Agent V1 projection mappings and use stable event payloads for model/job/operation state; `get_operation` polling remains the authoritative fallback and must observe every terminal async state. Task 11's `lifesub-two-tauri-harness` launches ad-hoc signed **test-harness** primary/secondary processes against one isolated HOME to prove audit token and code-requirement plumbing only; it is not production Tauri signature acceptance. The primary owns lock/DB/sockets/worker; the authorized test secondary performs settings read/save, model/revision/receipt lists, audio import, job control, operation polling, job status, search/resolve/open and internal host-control frames. Forged/mismatched/unsigned test clients are rejected. Instrument test-only side-effect counters and assert secondary has zero writable DB opens, migrations, reconciliations, workers, direct ledger writes, direct model mutations, direct imports and device access.

Add `lifesub-ipc-test-host`/`lifesub-ipc-test-client` for crash windows, connection limits and peer authorization. `scripts/verify-local-ipc.sh` must build the two binaries, create an isolated runtime, spawn the primary host and authorized `ui.sock` subscriber, then run a separate ordinary Agent client that sends `open_evidence` over `agent.sock`. Assert the Agent receives only intent ID/disposition/expiry; the authorized host receives the sanitized pending event, simulates the user prompt, claims via Host Control using trusted `tauri_ui`, invokes a fake opener, completes consumed, and Core records different requesting/claim principals plus consent audit. Also prove event loss/offline replay, unauthorized Agent cannot subscribe/claim and no token/path appears in any Agent/Gateway/event fixture. Run authorized/forged/ordinary/slow/oversize clients as separate OS processes, terminate/restart host for recovery cases, and fail if zero child scenarios ran. A unit-test invocation of the bin is not acceptance evidence. Direct Core, in-process Tauri, ordinary Agent IPC and controlled Application IPC must match golden fixtures. Gateway acceptance is limited to `tests/fixtures/tool-api/gateway` MCP mapping and path/error sanitizer fixtures; do not implement or launch Gateway in Task 11.

- [ ] **Step 5: Run Rust verification and commit**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features catalog_migration_test
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features tool_api_test
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features local_ipc_test
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features host_control_test
scripts/verify-local-ipc.sh
scripts/with-sherpa-runtime.sh cargo test --manifest-path src-tauri/Cargo.toml --features desktop commands_test
scripts/with-sherpa-runtime.sh cargo run --manifest-path src-tauri/Cargo.toml --features desktop --bin lifesub-two-tauri-harness
scripts/with-sherpa-runtime.sh cargo check --manifest-path src-tauri/Cargo.toml --features desktop
```

```bash
git add src-tauri/Cargo.toml src-tauri/src/commands.rs src-tauri/src/commands_test.rs src-tauri/src/desktop_api.rs src-tauri/src/core_runtime.rs src-tauri/src/tool_api.rs src-tauri/src/tool_api_test.rs src-tauri/src/host_control.rs src-tauri/src/host_control_test.rs src-tauri/src/local_ipc.rs src-tauri/src/local_ipc_test.rs src-tauri/src/service/runtime_lock.rs src-tauri/src/bin/lifesub-ipc-test-host.rs src-tauri/src/bin/lifesub-ipc-test-client.rs src-tauri/src/bin/lifesub-two-tauri-harness.rs src-tauri/src/catalog.rs src-tauri/src/catalog src-tauri/src/catalog_migration_test.rs src-tauri/src/catalog_migration_test src-tauri/src/lib.rs src-tauri/src/service.rs scripts/verify-local-ipc.sh tests/fixtures/catalog/lifesub-v0.3.sqlite3 tests/fixtures/catalog/lifesub-v0.4.sqlite3 tests/fixtures/tool-api tests/fixtures/code-signing docs/superpowers/specs/2026-08-16-lifesub-local-tool-api-design.md
git commit -m "feat: expose versioned local tool API"
```

### Chunk 3 Checkpoint

- [ ] Update `docs/prd/lifesub-real-asr-v0.2/.artifacts/process.md` with completed Tasks 9-11 and fencing/transaction evidence.
- [ ] Record claim, lease, cancellation, recovery, or Tauri issues in `.artifacts/notes.md`.
- [ ] Commit only those two files with message `docs: checkpoint ASR job work`.

## Chunk 4: Settings UI, Retranscription, Real Model Gates, And Packaging

### Task 12: Add The Typed ASR Client And Functional Settings UI

**Files:**

- Modify: `src/domain.ts`
- Create: `src/services/asr.ts`
- Create: `src/services/asr.test.ts`
- Create: `src/components/asr/ProviderSelector.tsx`
- Create: `src/components/asr/ModelCardList.tsx`
- Create: `src/components/asr/AsrSettingsForm.tsx`
- Create: `src/components/asr/AsrSettingsForm.test.tsx`
- Modify: `src/components/SettingsView.tsx`
- Modify: `src/App.tsx`
- Modify: `src/styles.css`
- Modify: `src/App.test.tsx`

- [ ] **Step 1: Write failing client mapping tests**

Assert exact adapter mappings for Application V1 settings/model/import/job/revision/receipt/operation methods and the trusted UI projection of Agent V1 job status/search/resolve/open methods. Every async action stores `operation_id`, polls `get_operation` through terminal `succeeded | failed | cancelled | recovery_required`, and treats events only as refresh hints. Frontend code may call Tauri invoke in phase C, but every command must map one-to-one to one of the two frozen contracts and reuse shared envelope/error/DTO primitives; it must not define a third contract or hidden read/open Command.

- [ ] **Step 2: Write failing settings interaction tests**

Test SenseVoice/Whisper/Qwen3-ASR segmented control, compatible model cards, download/pause-resume-or-retry/cancel/delete buttons, operation progress and every terminal operation state, language menu, thread stepper, VAD and auto-transcribe toggles, SenseVoice ITN, Whisper task, and both Qwen sizes. On the supported M4/24GB macOS 14+ arm64 Metal device, 1.7B must show its approximately 4.71 GB bundle size, Candle/Metal runtime, download action, aggregate multi-file progress, `installed_unqualified` runtime-validation state and final `runtime_qualified` ready state. On 16GB or another incompatible device it stays selectable/visible with exact unmet conditions and no download action. Test save errors, structural failure, runtime qualification retry, fixed loading layout, and the exact four-row ModelLookup capability presentation.

- [ ] **Step 3: Implement typed DTOs and client**

No `any`; keep snake_case Core DTOs at the service boundary and map them to camelCase view models in one place. Preserve separate `selectable`, `installable` and `executable` booleans plus reason codes, runtime identity, artifact count/total bytes and downloaded bytes; UI must not infer readiness from a single boolean or download completion alone.

- [ ] **Step 4: Replace static SettingsView content**

Use lucide icons and existing tokens. Do not add nested cards, hardcoded colors, viewport-scaled fonts, or text that explains keyboard shortcuts. Model cards remain stable while progress/error labels change.

- [ ] **Step 5: Implement desktop and demo states**

Tauri mode uses real commands. Browser mode uses a clearly labeled non-executable model catalog; it must not claim a model is installed or produce fake ASR evidence.

- [ ] **Step 6: Run frontend tests and commit**

Run:

```bash
npm test -- --run src/services/asr.test.ts src/components/asr/AsrSettingsForm.test.tsx src/App.test.tsx
npm run build
```

```bash
git add src/domain.ts src/services/asr.ts src/services/asr.test.ts src/components/asr src/components/SettingsView.tsx src/App.tsx src/styles.css src/App.test.tsx
git commit -m "feat: add local ASR settings experience"
```

### Task 13: Add Job Status, Provenance, And Retranscription UI

**Files:**

- Create: `src/components/asr/AsrJobStatus.tsx`
- Modify: `src/components/TranscriptView.tsx`
- Modify: `src/App.tsx`
- Modify: `src/services/lifesub.ts`
- Modify: `src/services/lifesub.test.ts`
- Modify: `src/App.test.tsx`
- Modify: `src/styles.css`

- [ ] **Step 1: Write failing user-flow tests**

Test import -> queued Job, processing status, cancel/retry, successful revision refresh, Provider Receipt display, corrupted source warning, retranscription confirmation, and two preserved revisions.

- [ ] **Step 2: Remove the demo revision path**

Delete the `appendTranscriptRevision(..., 'demo-local', ...)` import behavior from `App.tsx`. Import must wait for Core Job events/polling and never synthesize transcript text.

- [ ] **Step 3: Implement Job and retranscription components**

Show concise state and actionable recovery. Confirmation displays Provider, model, language, and readiness. Successful retranscription adds a revision; failure leaves the current revision selected.

- [ ] **Step 4: Run focused tests and commit**

Run:

```bash
npm test -- --run src/services/lifesub.test.ts src/App.test.tsx
npm run build
```

```bash
git add src/components/asr/AsrJobStatus.tsx src/components/TranscriptView.tsx src/App.tsx src/services/lifesub.ts src/services/lifesub.test.ts src/App.test.tsx src/styles.css
git commit -m "feat: add ASR jobs and retranscription"
```

### Task 14: Prove Real Provider Families Against Fixed Fixtures

**Files:**

- Create/Modify: `src-tauri/src/asr_runtime_test.rs`
- Create/Modify: `tests/fixtures/asr/fixture-manifest.json`
- Create/Modify: `tests/fixtures/asr/qwen-perf-5m.wav`
- Create: `src-tauri/src/bin/lifesub-asr-gate.rs`
- Create: `scripts/verify-asr-gate.sh`
- Create: `scripts/asr-gate-scope.txt`
- Create: `output/asr-v0.2/fixture-results.json`

- [ ] **Step 1: Implement the approved metric protocol**

NFKC + lowercase Latin; CER removes punctuation/whitespace and uses grapheme clusters; WER converts punctuation to spaces and splits collapsed whitespace; key phrases are normalized contiguous token subsequences; Segment counts must match and pair by time order.

- [ ] **Step 2: Implement a single real-model Gate runner**

`lifesub-asr-gate` loads the fixture manifest, verifies every fixture/model/runtime input hash, runs SenseVoice, Whisper, Qwen3-ASR 0.6B and Qwen3-ASR 1.7B fixtures, calculates all approved metrics, and writes the result JSON atomically. The 1.7B run is mandatory on the current M4/24GB macOS arm64 release device and requires Metal; unsupported hosts must report SKIP for developer convenience but the release verifier treats any 1.7B SKIP as failure. The JSON includes `tested_commit`, a deterministic digest of the exact paths listed in version-controlled `scripts/asr-gate-scope.txt`, executable hash, sherpa runtime version/git/native archive hash, `qwen3-asr` crate version/Git commit/Candle Metal backend/target/actual device, canonical model bundle identity, every model/VAD artifact source revision/hash, fixture hashes, and each Receipt runtime identity. Dynamic globs are prohibited. Unrelated dirty files outside the declared source scope do not invalidate the Gate; any scoped modification does.

Required Qwen3-ASR 0.6B scenario IDs are `qwen3-0.6b-zh`, `qwen3-0.6b-en`, `qwen3-0.6b-zh-en`, and `qwen3-0.6b-perf-300s`. Thresholds are: Mandarin CER `<= 20%`, English WER `<= 20%`, mixed-language key-phrase recall `= 100%`, median boundary error `<= 500 ms`, maximum boundary error `<= 1.5 s`, 300-second RTF `<= 1.0`, and peak RSS `<= 4 GiB` on the same M4/24GB host. Its Receipt must identify sherpa 1.13.5, exact runtime/native archive and the 0.6B bundle; any Candle/1.7B identity fails. The verifier fails if any scenario ID, metric or receipt identity is absent.

Required Qwen3-ASR 1.7B scenario IDs are `qwen3-1.7b-zh`, `qwen3-1.7b-en`, `qwen3-1.7b-zh-en`, and `qwen3-1.7b-perf-300s`. Gate evidence records the formally supported M4, 24 GB unified memory, macOS/arm64, Metal device, exact crate/git/Candle runtime, canonical bundle and fixture hashes, Mandarin CER `<= 20%`, English WER `<= 20%`, mixed-language key-phrase recall `= 100%`, no quality metric worse than 0.6B on the same fixtures, 300-second RTF `<= 1.0`, and peak RSS `<= 6 GiB`. The Receipt must identify the 1.7B Candle/Metal runtime and model; any sherpa/0.6B/CPU identity fails the no-fallback assertion. Upstream M4/16GB avg RTF 0.319 and live memory 4.6 GB are reference values only; 16 GB is unsupported until a future LifeSub Gate passes.

The five-minute performance input is `tests/fixtures/asr/qwen-perf-5m.wav`, generated deterministically by cycling `zh.wav`, `en.wav`, `zh-en.wav`, then 500 ms of 16 kHz mono silence until exactly 300 seconds and truncating only on a PCM frame boundary. The fixture manifest records generator version, ordered source hashes, final SHA-256, license inheritance, sample rate, channels, and exact duration. It is included in `scripts/asr-gate-scope.txt`; the Gate rejects a locally regenerated file whose hash differs.

`scripts/verify-asr-gate.sh` must:

1. Fetch/verify the native archive.
2. Verify the five-file 1.7B bundle, canonical identity, structural marker and runtime qualification marker without copying weights into the repository.
3. Verify all expected real tests appear in `cargo test -- --list` with nonzero count.
4. Run the Gate binary on the required M4/24 device.
5. Parse the result and fail unless every expected scenario exists and passes with the exact runtime Receipt identities and no fallback.
6. Support `--verify-existing`, which validates committed JSON without running models or rewriting files.

- [ ] **Step 3: Commit the Gate implementation before generating evidence**

```bash
git add src-tauri/src/asr_runtime_test.rs src-tauri/src/bin/lifesub-asr-gate.rs tests/fixtures/asr/fixture-manifest.json tests/fixtures/asr/zh.wav tests/fixtures/asr/en.wav tests/fixtures/asr/zh-en.wav tests/fixtures/asr/qwen-perf-5m.wav scripts/verify-asr-gate.sh scripts/asr-gate-scope.txt
git commit -m "test: add real local ASR gate"
```

The Gate script must now assert that every scoped source/fixture path is clean relative to `HEAD` before running.

- [ ] **Step 4: Run all providers and both Qwen sizes through the Gate from the committed source snapshot**

Run with the installed model cache path:

```bash
LIFESUB_ASR_MODEL_DIR="$HOME/Library/Application Support/com.goldenwave.lifesub/models" \
scripts/verify-asr-gate.sh
```

Expected: SenseVoice CER <= 20%; Whisper WER <= 20%; both Qwen sizes have Mandarin CER <= 20%, English WER <= 20%, and all required mixed-language phrases; both have RTF <= 1.0 on the identical 300-second fixture, with 0.6B RSS <= 4 GiB and 1.7B RSS <= 6 GiB; 1.7B is not worse than 0.6B on paired quality metrics; median boundary error <= 500 ms; max <= 1.5 s. Each Receipt must match its exact runtime/model identity. The script fails if any required provider/scenario/runtime identity is absent, if zero scenarios ran, or if fallback is detected.

- [ ] **Step 5: Verify evidence is bound to the tested source digest**

Run the Gate result verifier again after any code, model manifest, fixture, or runtime archive change. A result is valid only if the current scoped source digest exactly matches the recorded digest and `tested_commit` is an ancestor whose later changes are limited to evidence/docs paths. A different runtime build, executable hash, model hash, or fixture hash is invalid.

- [ ] **Step 6: Commit only non-sensitive result evidence**

```bash
git add output/asr-v0.2/fixture-results.json
git commit -m "test: record real local ASR evidence"
```

### Task 15: Complete Playwright, Responsiveness, And Packaging Gates

**Files:**

- Create: `tests/specs/lifesub-real-asr-v0.2.spec.ts`
- Modify: `tests/specs/lifesub-v0.1.spec.ts`
- Modify: `playwright.config.ts`
- Create: `src-tauri/src/acceptance.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/src/lib.rs`
- Create: `src/acceptance.ts`
- Modify: `src/App.tsx`
- Create: `scripts/verify-desktop-asr.sh`
- Create: `scripts/verify-packaged-peer-auth.sh`
- Create: `scripts/desktop-asr-scope.txt`
- Modify: `README.md`
- Modify: `docs/architecture.md`
- Modify: `docs/research.md`
- Modify: `docs/decisions.md`
- Create: `output/asr-v0.2/verification.md`
- Create: `output/asr-v0.2/settings-desktop.png`
- Create: `output/asr-v0.2/settings-mobile.png`
- Modify: `docs/prd/lifesub-real-asr-v0.2/.artifacts/process.md`
- Modify: `docs/prd/lifesub-real-asr-v0.2/.artifacts/notes.md`

- [ ] **Step 1: Write failing browser Playwright scenarios**

Cover provider switching, model states, parameter persistence, import Job state mapping, retranscription UI, revision preservation, long/error labels, and desktop/mobile no-overlap screenshots. Browser mode may use deterministic state fixtures, but these tests are UI mapping evidence only and cannot satisfy native ASR, cancellation, responsiveness, or recovery Gates.

- [ ] **Step 2: Implement the production desktop acceptance mode**

Add a hidden command-line option `--acceptance-scenario <name>` handled by the production binary. It uses the real Tauri WebView and Core, never a mock provider. `src/acceptance.ts` records a 100 ms UI heartbeat while the real native Job runs and sends the measurements to `acceptance.rs`, which writes an atomic JSON report and exits.

Required scenarios:

- `real-asr-heartbeat`: real fixture inference, P95 drift <= 250 ms.
- `cancel-real-asr`: `cancelling` acknowledged <= 500 ms; final cancellation occurs after at most the current Task 7 25-second native window plus boundary overhead. The harness must record the active window duration and must not claim native-call preemption or use an independent 30-second contract.
- `claim-and-abort`: claim a Job, persist the Job ID/generation, then terminate without cleanup.
- `verify-recovery`: new boot ID recovers the stale claim <= 5 seconds.
- `packaged-smoke`: run SenseVoice, Whisper, Qwen3-ASR 0.6B and Qwen3-ASR 1.7B fixtures from the packaged executable; verify every Receipt identity, canonical bundle identity, 1.7B structural plus runtime qualification markers, Candle/Metal device and no fallback.
- `packaged-peer-auth-primary` / `packaged-peer-auth-secondary`: launch two instances of the actual release-signed `.app`; secondary must obtain `tauri_ui` only when audit token, designated requirement, Team ID and bundle ID match, exercise Application/Agent projection plus Host Control claim/uncertain frames, and keep all secondary direct-DB side-effect counters at zero.

- [ ] **Step 3: Implement the desktop harness**

`scripts/verify-desktop-asr.sh` internally calls `fetch-sherpa-runtime.sh`, exports the verified `SHERPA_ONNX_ARCHIVE_DIR`, verifies the external five-file Qwen 1.7B bundle, hashes only the exact paths in version-controlled `scripts/desktop-asr-scope.txt`, builds the app with the pinned `qwen3-asr`/Candle Metal path, launches each scenario with an isolated temporary HOME/data directory, terminates the claim scenario process, relaunches recovery, and rejects reports containing `mock`, zero scenarios, mismatched executable/Git/runtime/model/fixture hashes, missing Metal/Candle identity, or failed thresholds. It also supports read-only `--verify-existing` mode. The harness stages model assets only under the isolated user model directory; it fails if weights appear in Git, source archives or `.app` resources.

`scripts/verify-packaged-peer-auth.sh` is the first production-signing authorization Gate; Task 11's ad-hoc test identities do not satisfy it. It inspects the release `.app` with `codesign -dr -` and signing metadata, records the expected designated requirement/Team ID/bundle ID, launches primary and secondary instances from that exact `.app` under one isolated HOME, and verifies successful `ui.sock` authorization plus Host Control access. It then runs copied/re-signed or dedicated negative fixture clients proving unsigned, mismatched Team ID, bundle ID and designated requirement cannot obtain `tauri_ui` or Host Control and can only reconnect to `agent.sock` as `local_agent`. The script must fail if any positive/negative scenario is skipped or if the identities were not derived from the packaged app.

- [ ] **Step 4: Commit acceptance code before building or running it**

```bash
git add tests/specs/lifesub-v0.1.spec.ts tests/specs/lifesub-real-asr-v0.2.spec.ts playwright.config.ts src-tauri/src/acceptance.rs src-tauri/src/main.rs src-tauri/src/lib.rs src/acceptance.ts src/App.tsx scripts/verify-desktop-asr.sh scripts/verify-packaged-peer-auth.sh scripts/desktop-asr-scope.txt
git commit -m "test: add desktop ASR acceptance harness"
```

The harness must assert its scoped production/acceptance files are clean relative to `HEAD`, then record `tested_commit` and the deterministic scoped source digest.

- [ ] **Step 5: Run the desktop target harness from committed code**

Run: `LIFESUB_ASR_MODEL_DIR="$HOME/Library/Application Support/com.goldenwave.lifesub/models" scripts/verify-desktop-asr.sh target`

- [ ] **Step 6: Run the complete suite**

Run:

```bash
npm test
npm run build
npm run test:e2e
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features
scripts/with-sherpa-runtime.sh cargo test --manifest-path src-tauri/Cargo.toml --features asr-runtime
scripts/with-sherpa-runtime.sh cargo test --manifest-path src-tauri/Cargo.toml --features asr-qwen17-runtime asr_runtime_qualifier
scripts/with-sherpa-runtime.sh cargo check --manifest-path src-tauri/Cargo.toml --features 'asr-runtime,asr-qwen17-runtime'
scripts/with-sherpa-runtime.sh cargo test --manifest-path src-tauri/Cargo.toml --features desktop commands_test
scripts/with-sherpa-runtime.sh cargo check --manifest-path src-tauri/Cargo.toml --features desktop
cargo metadata --manifest-path src-tauri/Cargo.toml --locked --format-version 1
cargo tree --manifest-path src-tauri/Cargo.toml --locked -e features
scripts/verify-asr-gate.sh
scripts/verify-desktop-asr.sh target
```

Expected: all PASS.

The `scripts/verify-asr-gate.sh` invocation in this step must regenerate `output/asr-v0.2/fixture-results.json` after the Task 15 acceptance commit. Assert that its `tested_commit` equals that acceptance code commit and that the new scoped digest/executable hash are current. The latest Gate run supersedes Task 14's earlier evidence.

- [ ] **Step 7: Verify no console logs and no secret/model artifacts**

Run:

```bash
/usr/bin/grep -RIn "console\.log" src tests --include='*.ts' --include='*.tsx' --include='*.js' --include='*.jsx'
git status --short
```

Expected: no `console.log`; no model weights, user audio, API keys, or private paths staged, tracked, present in source archives, or embedded in the `.app` resources.

- [ ] **Step 8: Build, mount, and execute the DMG application**

Run:

```bash
scripts/with-sherpa-runtime.sh npm run tauri -- build --features desktop
cargo tree --manifest-path src-tauri/Cargo.toml --locked -e features --features desktop
otool -L src-tauri/target/release/bundle/macos/LifeSub.app/Contents/MacOS/lifesub
nm -m src-tauri/target/release/bundle/macos/LifeSub.app/Contents/MacOS/lifesub
codesign --verify --deep --strict --verbose=2 src-tauri/target/release/bundle/macos/LifeSub.app
scripts/verify-packaged-peer-auth.sh src-tauri/target/release/bundle/macos/LifeSub.app
scripts/verify-desktop-asr.sh dmg
```

The packaged peer-auth Gate must use the actual release-signed `.app`, launch primary/secondary app processes, validate audit token against the extracted designated requirement/Team ID/bundle ID, prove unsigned/mismatched identities are rejected, and record Host Control authorization results. The `dmg` scenario must then deterministically locate the produced DMG, mount it read-only with `hdiutil`, verify the image-contained `.app` signature, rerun packaged peer authorization against the image-contained app, inspect the executable for expected Candle/Metal linkage or symbols and absence of development-machine paths, then run its `Contents/MacOS/lifesub --acceptance-scenario packaged-smoke` under an isolated HOME. The smoke resolves externally installed fixed assets and verifies real SenseVoice, Whisper, Qwen3-ASR 0.6B and Qwen3-ASR 1.7B results, exact Receipt runtime identities, 1.7B structural/qualification markers and no fallback, then detaches the image even on failure. Expected: no unresolved sherpa-onnx/onnxruntime dylib, Candle/Metal runtime present, no model weights inside the `.app`/DMG, signature and peer authorization pass, and image-contained real ASR smoke passes. Re-sign the full bundle and rebuild the DMG using the established V0.1 procedure before this Gate if needed.

- [ ] **Step 9: Capture visual and verification evidence**

Use Playwright screenshots at desktop and mobile widths. Write exact commands, per-file model hashes/revisions and bundle identities, sherpa and Qwen 1.7B crate/git/Candle/Metal/device identities, metrics, test counts, `otool`/symbol evidence, release designated requirement/Team ID/bundle ID, positive/negative packaged peer-auth results, no-weight checks, signature, packaged smoke and DMG results to `output/asr-v0.2/verification.md`.

- [ ] **Step 10: Update docs and progress**

Remove the README statement that real ASR is deferred. Document model sizes, storage, local-only processing, supported formats, download behavior, retranscription, and verification commands. Set process stage to `v0.2-real-asr-complete` only after every PRD checkbox has authoritative evidence.

- [ ] **Step 11: Commit only docs and evidence**

```bash
git add README.md docs/architecture.md docs/research.md docs/decisions.md docs/prd/lifesub-real-asr-v0.2/.artifacts/process.md docs/prd/lifesub-real-asr-v0.2/.artifacts/notes.md output/asr-v0.2/fixture-results.json output/asr-v0.2/verification.md output/asr-v0.2/settings-desktop.png output/asr-v0.2/settings-mobile.png
git commit -m "docs: verify LifeSub real ASR v0.2"
```

After this evidence/docs commit, run `scripts/verify-asr-gate.sh --verify-existing` and `scripts/verify-desktop-asr.sh --verify-existing`. They must not execute models or rewrite results; they must accept the unchanged scoped source digests and confirm that changes after the latest `tested_commit` are limited to the declared evidence/docs paths.

### Chunk 4 Checkpoint

- [ ] Confirm `process.md` contains exact real-model, desktop harness, DMG, signature, and UI results before marking the stage complete.
- [ ] Confirm `notes.md` contains every non-obvious runtime, model, decoder, signing, or recovery issue encountered.

---

## Final Completion Audit

- [ ] Agent Tool Contract V1 remains exactly 8 methods, Application-only V1 remains exactly 16 methods, and non-public Host Event/Control V1 is unreachable from Agent/Gateway and has no hidden Tauri Command or direct adapter Catalog write.
- [ ] A real two-process `agent.sock` request -> authorized `ui.sock` Host Event -> user-prompt simulation -> Host Control claim -> fake opener -> consumed flow passes; requester/claimer are separately audited, pending events replay after loss/offline, and no claim token/path leaks.
- [ ] Full Core ownership prevents secondary writable DB open/migration/reconciliation/worker/device/model/import/ledger side effects; only primary CoreRuntime commits business, operation and consent ledgers.
- [ ] Packaged peer-auth Gate passes for release-signed primary/secondary `.app` and DMG-contained app, while unsigned or mismatched designated requirement/Team ID/bundle ID clients cannot obtain `tauri_ui` or Host Control.
- [ ] SenseVoice, Whisper, Qwen3-ASR 0.6B and Qwen3-ASR 1.7B all execute real local inference; both Qwen scenario sets pass, 1.7B runs on M4/24GB Candle/Metal with RTF/RSS thresholds, and Receipt evidence proves no fallback.
- [ ] Settings selection changes the next Job's persisted provider/model snapshot.
- [ ] Single-archive and multi-file model downloads are individually hashed, checkpointed/resumable, safely assembled, versioned, atomically recoverable, compatibility-validated and removable.
- [ ] Audio is immutable, re-hashed before ASR, decoded only for declared formats, and timestamped correctly.
- [ ] Jobs use singleton worker locking, leases, boot IDs, claim-generation fencing, cancellation, and bounded retries.
- [ ] Successful results atomically publish Receipt, Revision, Segments, FTS rows, and succeeded state.
- [ ] Retranscription creates a new revision and preserves the previous revision.
- [ ] Real fixture CER/WER, phrase, and timing thresholds pass with saved evidence.
- [ ] UI responsiveness, cancellation, restart recovery, desktop/mobile layout, and error states pass.
- [ ] Static sherpa runtime, pinned Candle/Metal runtime, `otool`/symbol and asset-resolution checks, signatures, packaged/DMG smoke for both Qwen sizes, licenses, docs, and no-secret/no-model-weight checks pass.

---

## Post-V0.2 Milestone

After every V0.2 completion audit item passes, execute `docs/superpowers/plans/2026-08-16-lifesub-asr-device-qualification-cloud-fallback.md`. It adds cached static/device trials and user-controlled cloud fallback without weakening this plan's local-only V0.2 Gate.
