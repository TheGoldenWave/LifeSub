# LifeSub Real Local ASR V0.2 Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the demo transcript path with real, local SenseVoice, Whisper, and Qwen3-ASR transcription, model switching, persistent ASR settings, recoverable jobs, immutable revisions, and a complete settings experience.

**Architecture:** The Rust Core owns schema migration, immutable audio, model installation, ASR jobs, sherpa-onnx providers, receipts, and revision publication. React only reads and changes settings, displays model/job state, and requests retranscription. SenseVoiceSmall, Whisper, and Qwen3-ASR 0.6B share sherpa-onnx 1.13.5 with a single fenced background worker; Qwen3-ASR 1.7B remains disabled until its independent asset/device Gate passes.

**Tech Stack:** Rust 2024, rusqlite, sherpa-onnx 1.13.5 static runtime, Symphonia 0.6, Rubato 5, reqwest blocking + rustls, tar/bzip2, fs2, React 19, TypeScript, Vitest, Testing Library, Playwright, Tauri 2.

**Required references:**

- `docs/prd/lifesub-real-asr-v0.2/PRD.md`
- `docs/superpowers/specs/2026-08-15-lifesub-real-asr-design.md`
- `.claude/rules/common/coding-style.md`
- `.claude/contexts/dev.md`
- `docs/design/tokens/base.json`

---

## File Map

### Rust Core

- Modify `src-tauri/Cargo.toml`: pin ASR, audio, download, archive, locking, cancellation, and test dependencies.
- Modify `src-tauri/src/lib.rs`: register ASR modules, tests, commands, and worker lifecycle.
- Modify `src-tauri/src/domain.rs`: add chunk integrity and compatible ASR provenance fields.
- Modify `src-tauri/src/catalog.rs`: delegate versioned migration and expose transactional ASR persistence.
- Create `src-tauri/src/catalog/migrations.rs`: v0 fingerprint detection, fresh v2 creation, v1-to-v2 migration.
- Create `src-tauri/src/asr/mod.rs`: public ASR module boundary.
- Create `src-tauri/src/asr/settings.rs`: tagged provider settings and validation.
- Create `src-tauri/src/asr/manifest.rs`: pinned model/VAD manifests and artifact identity.
- Create `src-tauri/src/asr/model_manager.rs`: downloads, safe extraction, versioned install activation, reconciliation.
- Create `src-tauri/src/asr/audio.rs`: decoding, downmix, resampling, duration and time conversion.
- Create `src-tauri/src/asr/vad.rs`: VAD ranges, 25-second partitioning, timing invariants.
- Create `src-tauri/src/asr/provider.rs`: provider trait, request/result/error types, fake provider.
- Create `src-tauri/src/asr/sense_voice.rs`: sherpa-onnx SenseVoice adapter.
- Create `src-tauri/src/asr/whisper.rs`: sherpa-onnx Whisper adapter.
- Create `src-tauri/src/asr/qwen3_asr.rs`: sherpa-onnx Qwen3-ASR adapter and 1.7B capability Gate.
- Create `src-tauri/src/asr/job.rs`: job state machine, singleton lock, claims, leases, fencing, cancellation.
- Create `src-tauri/src/asr/service.rs`: job execution and atomic Receipt/Revision publication.
- Create `src-tauri/src/asr/model_lookup.rs`: minimal model lookup interface used by settings and the static manifest.
- Create `src-tauri/src/acceptance.rs`: hidden desktop acceptance scenarios using the production event loop.
- Create `src-tauri/src/bin/lifesub-asr-gate.rs`: single real-model quality Gate runner.
- Modify `src-tauri/src/service.rs`: crash-safe immutable import and chunk reconciliation.
- Modify `src-tauri/src/commands.rs`: settings, model, job, retranscription, status commands.

### Rust Tests And Fixtures

- Create `src-tauri/src/catalog_migration_test.rs`: real v1 fixture migration and rollback tests.
- Create `src-tauri/src/asr_settings_test.rs`: provider-specific settings validation.
- Create `src-tauri/src/asr_model_manager_test.rs`: interrupted download, integrity, extraction, reconciliation.
- Create `src-tauri/src/asr_audio_test.rs`: declared formats, resampling, VAD partition and timestamps.
- Create `src-tauri/src/asr_job_test.rs`: claim/lease/fencing/cancel/recovery/atomic publish tests.
- Create `src-tauri/src/asr_runtime_test.rs`: opt-in real SenseVoice/Whisper/Qwen3-ASR fixture tests.
- Create `tests/fixtures/asr/fixture-manifest.json`: hashes, transcripts, intervals, phrases, licenses.
- Create `tests/fixtures/asr/zh.wav`, `en.wav`, `zh-en.wav`: redistributable fixed speech samples.
- Create `tests/fixtures/catalog/lifesub-v0.1.sqlite3`: pre-v2 migration fixture.

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

### Task 2: Introduce Versioned Catalog Migration

**Files:**

- Create: `src-tauri/src/catalog/migrations.rs`
- Modify: `src-tauri/src/catalog.rs`
- Create: `src-tauri/src/catalog_migration_test.rs`
- Create: `tests/fixtures/catalog/lifesub-v0.1.sqlite3`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing schema classification tests**

Cover:

```rust
assert_eq!(classify_schema(&empty_db)?, SchemaKind::Fresh);
assert_eq!(classify_schema(&v1_fixture)?, SchemaKind::LegacyV1);
assert_eq!(classify_schema(&unknown_v0)?, SchemaKind::Unknown);
```

Also assert migration rollback when one v2 statement is forced to fail.

- [ ] **Step 2: Verify the tests fail against the current unversioned migration**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features catalog_migration`

Expected: FAIL because schema fingerprinting and `user_version = 2` do not exist.

- [ ] **Step 3: Implement exact v2 DDL from the approved design**

Use `BEGIN IMMEDIATE`, fingerprint the v1 tables/columns/FTS tokenizer, create:

- `asr_settings`
- `model_installations`
- `model_downloads`
- `asr_jobs` with `claim_generation`
- `provider_receipts`
- `revision_receipts`
- partial unique indexes
- chunk integrity and dual timestamp columns

Do not infer that every `user_version = 0` database is V0.1.

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
git commit -m "feat: add versioned ASR catalog schema"
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

Test thread bounds, language support, model/provider ownership, Whisper translate, SenseVoice ITN, Qwen3-ASR option isolation, and unavailable 1.7B rejection.

- [ ] **Step 2: Run the tests and confirm missing types**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features asr_settings`

Expected: FAIL.

- [ ] **Step 3: Implement immutable serialized types**

Add `AsrProviderKind`, `AsrProviderOptions`, `AsrSettings`, `AsrJobState`, `ChunkIntegrityState`, `ProviderReceipt`, `AsrErrorCode`, and validated transcript time ranges. Define a minimal `ModelLookup` trait containing provider ownership and language capability; Task 3 tests use a stub, and Task 5's static manifest implements it. Persist enums as snake_case strings, never `Debug` output.

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

- Modify: `src-tauri/src/service.rs`
- Modify: `src-tauri/src/catalog.rs`
- Modify: `src-tauri/src/service_test.rs`

- [ ] **Step 1: Add failing crash-window and integrity tests**

Test temporary write, final rename, orphan cleanup, missing final file, changed bytes, and re-hash before ASR.

```rust
assert_eq!(catalog.chunk_integrity(&chunk.id)?, ChunkIntegrityState::Missing);
assert_eq!(service.verify_chunk(&chunk.id), Err(ServiceError::InputUnavailable));
```

- [ ] **Step 2: Run focused tests and observe direct-final-write failures**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features imported_audio`

- [ ] **Step 3: Implement durable import**

Write and hash a same-directory temp file, `sync_all`, atomic rename, fsync the parent, then insert metadata. Add startup reconciliation and `available/corrupted/missing` behavior. Re-hash immediately before ASR.

- [ ] **Step 4: Verify old source preservation and new failure behavior**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features service_test
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features catalog_test
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/service.rs src-tauri/src/service_test.rs src-tauri/src/catalog.rs
git commit -m "feat: harden immutable audio imports"
```

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
- Create: `THIRD_PARTY_NOTICES.md`

- [ ] **Step 1: Write failing manifest contract tests**

Require unique immutable IDs, HTTPS URLs, exact byte sizes, 64-character hashes, required files, source/conversion provenance, supported languages, license, and allowlisted redirect hosts.

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
Declare an experimental, non-installable registry entry until an immutable sherpa-onnx or approved native Apple Silicon package has exact size, SHA-256, required files, conversion provenance, and passing Gate evidence. Raw Hugging Face Transformers weights are not executable by the Rust provider.

Silero VAD
https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/silero_vad.onnx
size 643,854; sha256 9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6
```

Expected ASR archive contents are the model directory, token file, ONNX model file(s), and upstream test WAVs named by the sherpa release; inspect and record exact required relative paths before writing the registry. The VAD asset is the single file `silero_vad.onnx`. The manifest test re-download helper must verify URL, size, SHA-256, and required paths against the registry.

Required executable model files are:

```text
SenseVoice: model.int8.onnx, tokens.txt
Whisper Tiny: tiny-encoder.onnx, tiny-decoder.onnx, tiny-tokens.txt
Whisper Base: base-encoder.onnx, base-decoder.onnx, base-tokens.txt
Whisper Small: small-encoder.onnx, small-decoder.onnx, small-tokens.txt
Qwen3-ASR 0.6B: conv-frontend.int8.onnx, encoder.int8.onnx, decoder.int8.onnx, tokenizer.json (verify exact archive paths before freezing)
VAD: silero_vad.onnx
```

- [ ] **Step 3: Implement the static registry**

```rust
pub struct ModelManifest {
    pub id: &'static str,
    pub manifest_version: &'static str,
    pub archive_sha256: &'static str,
    pub required_files: &'static [RequiredFile],
    pub source: ModelSource,
}
```

Implement `ModelLookup` for the registry. Use a new model ID if any asset hash changes.

- [ ] **Step 4: Add notices and verify tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features asr_manifest`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/asr/manifest.rs src-tauri/src/asr_manifest_test.rs src-tauri/src/lib.rs THIRD_PARTY_NOTICES.md
git commit -m "feat: pin local ASR model manifests"
```

### Task 6: Implement Recoverable Model Downloads And Installs

**Files:**

- Create: `src-tauri/src/asr/model_manager.rs`
- Create: `src-tauri/src/asr_model_manager_test.rs`
- Modify: `src-tauri/src/catalog.rs`

- [ ] **Step 1: Write failing HTTP fixture tests**

Cover interrupted downloads, incorrect content length, redirect to a disallowed host, wrong SHA-256, path traversal, symlink/hardlink entries, expanded-size limit, rename-before-DB crash, DB-before-directory mismatch, cancellation, and deletion while leased.

- [ ] **Step 2: Verify failure**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features asr_model_manager`

- [ ] **Step 3: Implement persistent download state**

Use `model_downloads` for `queued/downloading/verifying/installing/succeeded/failed/cancelled`. Persist byte progress at bounded intervals; keep user-facing errors separate from diagnostic summaries.

- [ ] **Step 4: Implement safe versioned activation**

Install to:

```text
models/asr/<provider>/<model-id>/<manifest-version>-<archive-hash>/
```

Reject unsafe archive entries, write an immutable marker, fsync, rename, then activate through a SQLite transaction. Reconcile `.part`, staging, unrecorded installs, missing active directories, and corrupt files at startup.

- [ ] **Step 5: Run all model manager tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features asr_model_manager`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/asr/model_manager.rs src-tauri/src/asr_model_manager_test.rs src-tauri/src/catalog.rs
git commit -m "feat: add recoverable ASR model installs"
```

### Task 7: Decode, Downmix, Resample, And Partition Audio

**Files:**

- Create: `src-tauri/src/asr/audio.rs`
- Create: `src-tauri/src/asr/vad.rs`
- Create: `src-tauri/src/asr_audio_test.rs`
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

Test 16 kHz output, arithmetic-mean downmix, clamp, resampler delay compensation, 200 ms VAD padding, 25-second maximum windows, and hard-split fallback.

- [ ] **Step 3: Implement decoding and timing**

Decode with Symphonia, downmix to `f32`, resample with Rubato, retain original frame coordinates, and use floor-start/ceil-end conversion.

- [ ] **Step 4: Implement VAD boundary orchestration**

Wrap the pinned sherpa VAD when `asr-runtime` is enabled. Provide a deterministic fake detector for fast tests. Evidence ranges use non-overlapping core intervals, not padded inference context.

- [ ] **Step 5: Run tests and update the import allowlist**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features asr_audio
cargo test --manifest-path src-tauri/Cargo.toml --features asr-runtime asr_audio
npm test -- --run src/App.test.tsx
```

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/asr/audio.rs src-tauri/src/asr/vad.rs src-tauri/src/asr_audio_test.rs src-tauri/src/lib.rs tests/fixtures/asr src/App.tsx src/App.test.tsx docs/prd/lifesub-real-asr-v0.2/PRD.md
git commit -m "feat: prepare timestamped ASR audio"
```

### Task 8: Implement SenseVoice, Whisper, And Qwen3-ASR Providers

**Files:**

- Create: `src-tauri/src/asr/provider.rs`
- Create: `src-tauri/src/asr/sense_voice.rs`
- Create: `src-tauri/src/asr/whisper.rs`
- Create: `src-tauri/src/asr/qwen3_asr.rs`
- Create: `src-tauri/src/asr_provider_test.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write fake-provider contract tests**

Test provider/model identity, language mapping, SenseVoice ITN, Whisper task, Qwen3-ASR parameters and 1.7B capability rejection, empty-output rejection, cancellation between windows, and error mapping without loading native models.

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

Providers receive validated PCM and do not read settings, select fallback, write SQLite, or assign revision numbers. The token is checked before and after the synchronous native call; window orchestration remains responsible for the documented maximum cancellation latency.

- [ ] **Step 4: Implement sherpa adapters behind `asr-runtime`**

Map manifest files into `OfflineSenseVoiceModelConfig`, `OfflineWhisperModelConfig`, and `OfflineQwen3ASRModelConfig`. Enable the approved language/task/ITN values. Capture runtime version and build identity. The Qwen adapter must refuse a manifest whose executable capability Gate is not satisfied; it must never substitute 0.6B for 1.7B.

- [ ] **Step 5: Run provider tests and compile both feature sets**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features asr_provider
cargo test --manifest-path src-tauri/Cargo.toml --features asr-runtime asr_provider
cargo check --manifest-path src-tauri/Cargo.toml --features desktop
```

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/asr/provider.rs src-tauri/src/asr/sense_voice.rs src-tauri/src/asr/whisper.rs src-tauri/src/asr/qwen3_asr.rs src-tauri/src/asr_provider_test.rs src-tauri/src/lib.rs
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
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing claim and recovery tests**

Cover:

- exclusive `asr-worker.lock`
- claim CAS changes `queued -> preparing`
- `attempt_count` and `claim_generation` increment together
- lease 30 seconds and renewal every 5 seconds/stage
- stale boot ID recovery within 5 seconds
- 5-second and 30-second retry backoff
- maximum 3 total claims
- queued/blocked cancellation
- model-ready transition excludes cancelled jobs

- [ ] **Step 2: Add the stale-worker fencing test**

```rust
let first = jobs.claim("boot-a", "worker-1")?.unwrap();
jobs.expire_and_requeue(&first.id)?;
let second = jobs.claim("boot-a", "worker-2")?.unwrap();
assert!(jobs.complete(&first.claim).is_err());
assert!(jobs.complete(&second.claim).is_ok());
```

- [ ] **Step 3: Run tests and verify current Catalog cannot express them**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features asr_job`

- [ ] **Step 4: Implement ownership-fenced transitions**

Every renewal and state transition uses `id + claimed_by + claim_generation`. A zero-row update means ownership is lost and in-memory results must be discarded.

- [ ] **Step 5: Run tests and commit**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features asr_job`

```bash
git add src-tauri/src/asr/job.rs src-tauri/src/asr_job_test.rs src-tauri/src/catalog.rs src-tauri/src/lib.rs
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

- [ ] **Step 2: Write cancellation and stale-generation race tests**

Cancel before the transaction: no revision. Cancel after commit: revision remains and Job is succeeded. A stale generation cannot insert a receipt.

- [ ] **Step 3: Implement the service orchestration**

The service verifies chunk hash, resolves exact ASR/VAD artifacts, decodes audio, transcribes windows, rejects empty results, and publishes in `BEGIN IMMEDIATE` with fencing and `cancel_requested_at IS NULL`.

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

### Task 11: Expose Desktop Commands And Start The Worker

**Files:**

- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Create: `src-tauri/src/commands_test.rs`
- Create: `src-tauri/src/desktop_api.rs`
- Modify: `src-tauri/src/service.rs`

- [ ] **Step 1: Write command contract tests**

Cover `get_asr_settings`, `save_asr_settings`, `list_asr_models`, `download_asr_model`, `cancel_model_download`, `delete_asr_model`, `list_asr_jobs`, `cancel_asr_job`, `retry_asr_job`, and `retranscribe_record`. Put framework-independent handlers and DTO mapping in `desktop_api.rs`; Tauri commands remain thin wrappers.

- [ ] **Step 2: Change import behavior in a failing test**

`import_audio_file` must return the immutable Chunk and optional queued Job; it must never append `demo-local` text.

- [ ] **Step 3: Implement AppState services and worker startup**

Acquire `asr-worker.lock` first. Only the lock holder may then run ModelManager, Chunk, and Job reconciliation, initialize the boot ID/cancellation registry/event emitter, and accept work. Run native inference on a blocking thread, never the UI thread.

- [ ] **Step 4: Register commands and model/job events**

Use stable event payloads for model progress and Job state. Frontend polling remains a fallback if an event is missed.

- [ ] **Step 5: Run Rust verification and commit**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features
scripts/with-sherpa-runtime.sh cargo test --manifest-path src-tauri/Cargo.toml --features desktop commands_test
scripts/with-sherpa-runtime.sh cargo check --manifest-path src-tauri/Cargo.toml --features desktop
```

```bash
git add src-tauri/src/commands.rs src-tauri/src/commands_test.rs src-tauri/src/desktop_api.rs src-tauri/src/lib.rs src-tauri/src/service.rs
git commit -m "feat: expose desktop ASR commands"
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

Assert exact Tauri command names and payloads for settings and model operations.

- [ ] **Step 2: Write failing settings interaction tests**

Test SenseVoice/Whisper segmented control, compatible model cards, download/cancel/delete buttons, language menu, thread stepper, VAD and auto-transcribe toggles, SenseVoice ITN, Whisper task, save errors, and fixed loading layout.

- [ ] **Step 3: Implement typed DTOs and client**

No `any`; keep snake_case Core DTOs at the service boundary and map them to camelCase view models in one place.

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
- Create: `src-tauri/src/bin/lifesub-asr-gate.rs`
- Create: `scripts/verify-asr-gate.sh`
- Create: `scripts/asr-gate-scope.txt`
- Create: `output/asr-v0.2/fixture-results.json`

- [ ] **Step 1: Implement the approved metric protocol**

NFKC + lowercase Latin; CER removes punctuation/whitespace and uses grapheme clusters; WER converts punctuation to spaces and splits collapsed whitespace; key phrases are normalized contiguous token subsequences; Segment counts must match and pair by time order.

- [ ] **Step 2: Implement a single real-model Gate runner**

`lifesub-asr-gate` loads the fixture manifest, verifies every fixture/model/runtime input hash, runs SenseVoice, Whisper, and Qwen3-ASR 0.6B fixtures, calculates all approved metrics, and writes the result JSON atomically. Qwen3-ASR 1.7B is enabled only when the same Gate includes its immutable executable asset and records peak memory plus RTF on the supported Apple Silicon baseline. The JSON includes `tested_commit`, a deterministic digest of the exact paths listed in version-controlled `scripts/asr-gate-scope.txt`, executable hash, runtime version/git SHA, native archive hash, model/VAD artifact hashes, and fixture hashes. Dynamic globs are prohibited. Unrelated dirty files outside the declared source scope do not invalidate the Gate; any scoped modification does.

`scripts/verify-asr-gate.sh` must:

1. Fetch/verify the native archive.
2. Verify all expected real tests appear in `cargo test -- --list` with nonzero count.
3. Run the Gate binary.
4. Parse the result and fail unless every expected scenario exists and passes.
5. Support `--verify-existing`, which validates committed JSON without running models or rewriting files.

- [ ] **Step 3: Commit the Gate implementation before generating evidence**

```bash
git add src-tauri/src/asr_runtime_test.rs src-tauri/src/bin/lifesub-asr-gate.rs tests/fixtures/asr/fixture-manifest.json tests/fixtures/asr/zh.wav tests/fixtures/asr/en.wav tests/fixtures/asr/zh-en.wav scripts/verify-asr-gate.sh scripts/asr-gate-scope.txt
git commit -m "test: add real local ASR gate"
```

The Gate script must now assert that every scoped source/fixture path is clean relative to `HEAD` before running.

- [ ] **Step 4: Run SenseVoice and Whisper through the Gate from the committed source snapshot**

Run with the installed model cache path:

```bash
LIFESUB_ASR_MODEL_DIR="$HOME/Library/Application Support/com.goldenwave.lifesub/models" \
scripts/verify-asr-gate.sh
```

Expected: SenseVoice CER <= 20%; Whisper WER <= 20%; all mixed-language phrases present; median boundary error <= 500 ms; max <= 1.5 s. The script fails if zero tests/scenarios ran.

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
- `cancel-real-asr`: `cancelling` acknowledged <= 500 ms and final cancelled <= 30 seconds.
- `claim-and-abort`: claim a Job, persist the Job ID/generation, then terminate without cleanup.
- `verify-recovery`: new boot ID recovers the stale claim <= 5 seconds.
- `packaged-smoke`: run both Provider fixtures from the packaged executable and verify Receipt identity.

- [ ] **Step 3: Implement the desktop harness**

`scripts/verify-desktop-asr.sh` internally calls `fetch-sherpa-runtime.sh`, exports the verified `SHERPA_ONNX_ARCHIVE_DIR`, hashes only the exact paths in version-controlled `scripts/desktop-asr-scope.txt`, builds the app, launches each scenario with an isolated temporary HOME/data directory, terminates the claim scenario process, relaunches recovery, and rejects reports containing `mock`, zero scenarios, mismatched executable/Git/runtime/model/fixture hashes, or failed thresholds. It also supports read-only `--verify-existing` mode.

- [ ] **Step 4: Commit acceptance code before building or running it**

```bash
git add tests/specs/lifesub-v0.1.spec.ts tests/specs/lifesub-real-asr-v0.2.spec.ts playwright.config.ts src-tauri/src/acceptance.rs src-tauri/src/main.rs src-tauri/src/lib.rs src/acceptance.ts src/App.tsx scripts/verify-desktop-asr.sh scripts/desktop-asr-scope.txt
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
scripts/with-sherpa-runtime.sh cargo test --manifest-path src-tauri/Cargo.toml --features desktop commands_test
scripts/with-sherpa-runtime.sh cargo check --manifest-path src-tauri/Cargo.toml --features desktop
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

Expected: no `console.log`; no model weights, user audio, API keys, or private paths staged.

- [ ] **Step 8: Build, mount, and execute the DMG application**

Run:

```bash
scripts/with-sherpa-runtime.sh npm run tauri -- build --features desktop
otool -L src-tauri/target/release/bundle/macos/LifeSub.app/Contents/MacOS/lifesub
codesign --verify --deep --strict --verbose=2 src-tauri/target/release/bundle/macos/LifeSub.app
scripts/verify-desktop-asr.sh dmg
```

The `dmg` scenario must deterministically locate the produced DMG, mount it read-only with `hdiutil`, verify the image-contained `.app` signature, run its `Contents/MacOS/lifesub --acceptance-scenario packaged-smoke` under an isolated HOME, verify real SenseVoice and Whisper results, then detach the image even on failure. Expected: no unresolved sherpa-onnx/onnxruntime dylib, signature passes, and image-contained real ASR smoke passes. Re-sign the full bundle and rebuild the DMG using the established V0.1 procedure before this Gate if needed.

- [ ] **Step 9: Capture visual and verification evidence**

Use Playwright screenshots at desktop and mobile widths. Write exact commands, model hashes, metrics, test counts, `otool`, signature, and DMG results to `output/asr-v0.2/verification.md`.

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

- [ ] SenseVoice and Whisper both execute real local inference.
- [ ] Settings selection changes the next Job's persisted provider/model snapshot.
- [ ] Model downloads are hashed, safely extracted, versioned, recoverable, and removable.
- [ ] Audio is immutable, re-hashed before ASR, decoded only for declared formats, and timestamped correctly.
- [ ] Jobs use singleton worker locking, leases, boot IDs, claim-generation fencing, cancellation, and bounded retries.
- [ ] Successful results atomically publish Receipt, Revision, Segments, FTS rows, and succeeded state.
- [ ] Retranscription creates a new revision and preserves the previous revision.
- [ ] Real fixture CER/WER, phrase, and timing thresholds pass with saved evidence.
- [ ] UI responsiveness, cancellation, restart recovery, desktop/mobile layout, and error states pass.
- [ ] Static runtime, `otool`, signatures, DMG, licenses, docs, and no-secret/no-model-weight checks pass.
