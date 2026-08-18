# LifeSub ASR Device Qualification And Cloud Fallback Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add cached per-model device qualification, isolated real-model trials, user-controlled local/cloud routing and auditable cloud fallback without expanding the current ASR V0.2 release scope.

**Architecture:** `CoreRuntime` owns qualification state and routing decisions. Static manifest/device checks produce a cached compatibility result; an isolated short trial upgrades a model to supported. Every ASR Job stores an immutable routing/privacy snapshot, and cloud fallback is allowed only for pre-authorized reasons.

**Tech Stack:** Rust, Tauri, SQLite migrations, existing ModelLookup/ModelManager/RuntimeQualifier/ASR Job services, React, Vitest, Playwright, macOS process/resource APIs, provider SDKs selected during cloud-provider implementation.

---

## Chunk 1: Qualification Domain And Persistence

### Task 1: Freeze Device Qualification Domain

**Files:**
- Create: `src-tauri/src/asr/device_profile.rs`
- Create: `src-tauri/src/asr/qualification.rs`
- Create: `src-tauri/src/asr_device_qualification_test.rs`
- Modify: `src-tauri/src/asr/model_lookup.rs`
- Modify: `src-tauri/src/asr/manifest.rs`
- Modify: `src-tauri/src/domain.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] Write failing serde/state-transition tests for all qualification states, stable reason codes and the cache-key fields.
- [ ] Add manifest contract tests requiring resource tier, memory, backend, RTF and trial protocol metadata for every local model.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features asr_device_qualification` and observe RED.
- [ ] Implement immutable domain types and extend ModelLookup without collapsing selectable/installable/executable/qualification into one boolean.
- [ ] Run focused tests and existing ASR manifest/settings tests.
- [ ] Commit `feat: define ASR device qualification`.

### Task 2: Persist Cached Qualification Results

**Files:**
- Modify: `src-tauri/src/catalog/migrations.rs`
- Modify: `src-tauri/src/catalog/migrations/ddl.rs`
- Modify: `src-tauri/src/catalog/migrations/fingerprint.rs`
- Modify: `src-tauri/src/catalog.rs`
- Create: `src-tauri/src/catalog/qualifications.rs`
- Create: `src-tauri/src/asr_qualification_catalog_test.rs`
- Modify/Create: `src-tauri/src/catalog_migration_test/*`
- Create: `tests/fixtures/catalog/<next-version>.sqlite3`

- [ ] Write failing migration, cache identity, stale transition and unknown-enum tests.
- [ ] Define a new schema version; do not append DDL under V0.2's frozen v4 fingerprint.
- [ ] Store device fingerprint, OS compatibility generation, runtime identity, bundle identity, protocol version, status, metrics, reason and timestamps.
- [ ] Implement compare-and-mark-stale without time-based expiry.
- [ ] Verify fresh and every immutable historical fixture migrate, rollback remains atomic and unknown shapes fail closed.
- [ ] Commit `feat: persist ASR qualification results`.

## Chunk 2: Static Preflight And Isolated Trial

### Task 3: Implement Static Device Profiling

**Files:**
- Modify: `src-tauri/src/asr/device_profile.rs`
- Create: `src-tauri/src/asr/device_profile_macos.rs`
- Create: `src-tauri/src/asr_device_profile_test.rs`

- [ ] Write failing tests for OS, architecture, total memory, Metal availability, disk and runtime identity rules.
- [ ] Add injected probes so tests do not depend on the developer machine.
- [ ] Implement stable device fingerprinting that excludes transient free-memory values.
- [ ] Test that ordinary free-memory changes do not invalidate qualification.
- [ ] Commit `feat: detect local ASR device capabilities`.

### Task 4: Run And Cache Real Model Trials

**Files:**
- Create: `src-tauri/src/asr/trial_runner.rs`
- Create: `src-tauri/src/asr/trial_worker.rs`
- Create: `src-tauri/src/asr_trial_test.rs`
- Modify: `src-tauri/src/asr/runtime_qualifier.rs`
- Modify: `src-tauri/src/asr/model_manager.rs`
- Test: `tests/fixtures/asr/device-trial-*`

- [ ] Write failing tests for success, degraded, invalid output, timeout, OOM, critical memory pressure, Metal failure and worker crash.
- [ ] Require the trial to use a fixed non-user fixture and exact model/runtime identity.
- [ ] Execute the trial outside the UI thread with a hard deadline and process/resource telemetry.
- [ ] Persist results only when the cache key still matches after the trial.
- [ ] Verify environment changes mark stale while unchanged restarts reuse results without rerunning.
- [ ] Commit `feat: qualify ASR models on device`.

### Task 5: Add Instantaneous Resource Preflight

**Files:**
- Create: `src-tauri/src/asr/resource_preflight.rs`
- Create: `src-tauri/src/asr_resource_preflight_test.rs`
- Modify: `src-tauri/src/asr/job.rs`

- [ ] Test temporary low-memory, disk pressure and recovery without invalidating long-term model support.
- [ ] Implement wait/block/fallback recommendations as data; do not perform routing inside the probe.
- [ ] Verify a single failure does not change qualification and two qualifying resource failures request a retest.
- [ ] Commit `feat: guard ASR jobs by current resources`.

## Chunk 3: Routing Policy And Cloud Provider Boundary

### Task 6: Freeze Routing And Privacy Policy

**Files:**
- Create: `src-tauri/src/asr/routing.rs`
- Create: `src-tauri/src/asr_routing_test.rs`
- Modify: `src-tauri/src/domain.rs`
- Modify: `src-tauri/src/asr/settings.rs`

- [ ] Write failing truth-table tests for local_only, automatic, cloud_only and exact_provider.
- [ ] Test global defaults plus per-task overrides and immutable policy snapshots.
- [ ] Test every authorized fallback reason and assert unapproved reasons fail closed.
- [ ] Implement pure routing decisions with no network or database access.
- [ ] Commit `feat: define user-controlled ASR routing`.

### Task 7: Add Cloud Provider Contract And Credentials Boundary

**Files:**
- Create: `src-tauri/src/asr/cloud_provider.rs`
- Create: `src-tauri/src/asr/cloud_provider_test.rs`
- Modify: `src-tauri/src/asr/provider.rs`
- Modify: `src-tauri/src/asr/receipt.rs`
- Modify: `src-tauri/src/asr/service.rs`

- [ ] Select the first cloud Provider in a separate provider-specific spec before adding its SDK.
- [ ] Write contract tests for credential references, upload disclosure, cancellation, retry, redacted errors and no raw secret persistence.
- [ ] Extend Receipt with requested/actual provider/model, fallback reason, `audio_left_device`, policy snapshot and qualification reference.
- [ ] Prove local_only and unauthorized paths make zero cloud client/upload calls.
- [ ] Commit `feat: add auditable cloud ASR boundary`.

### Task 8: Orchestrate Local Failure And Cloud Fallback

**Files:**
- Modify: `src-tauri/src/asr/service.rs`
- Modify: `src-tauri/src/asr/job.rs`
- Modify: `src-tauri/src/catalog.rs`
- Create: `src-tauri/src/asr_fallback_test.rs`

- [ ] Write failing tests for local unsupported, not installed, resource blocked, execution failure and predicted-duration fallback.
- [ ] Preserve every attempt as a fenced Job generation; never overwrite the failed local attempt.
- [ ] Publish the final Revision only from the successful actual Provider Receipt.
- [ ] Return `blocked_provider` when cloud is unavailable or authorization expires.
- [ ] Verify restart recovery cannot duplicate uploads or revisions.
- [ ] Commit `feat: route ASR jobs with cloud fallback`.

## Chunk 4: Settings, Task Overrides And Verification

### Task 9: Add Qualification And Routing APIs

**Files:**
- Modify: `src-tauri/src/tool_api.rs`
- Modify: `src-tauri/src/desktop_api.rs`
- Modify: `src-tauri/src/local_ipc.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/tool_api_test.rs`

- [ ] Extend the versioned Application Contract rather than adding hidden Tauri commands.
- [ ] Add read qualification, request precheck/trial/retrial, save routing defaults and task override DTOs with stable errors.
- [ ] Freeze golden JSON and verify direct Core, Tauri and IPC mappings are identical.
- [ ] Commit `feat: expose ASR qualification controls`.

### Task 10: Build The Settings And Per-Task Experience

**Files:**
- Modify: `src/services/asr.ts`
- Modify: `src/domain.ts`
- Modify: `src/components/asr/ModelCardList.tsx`
- Modify: `src/components/asr/AsrSettingsForm.tsx`
- Create: `src/components/asr/AsrRoutingPolicy.tsx`
- Modify: `src/components/TranscriptView.tsx`
- Modify: `src/styles.css`
- Modify/Create: corresponding Vitest files

- [ ] Test concise resource descriptions, support states, advanced metrics, test/retest actions and stable card dimensions.
- [ ] Test global default plus per-import/retranscription override for local_only/automatic/cloud_only/exact_provider.
- [ ] Show cloud upload state and actual Provider before execution; do not use a generic readiness boolean.
- [ ] Use existing design tokens and responsive controls.
- [ ] Run focused Vitest and production build.
- [ ] Commit `feat: add ASR device and privacy controls`.

### Task 11: Prove Caching, Privacy And Fallback End To End

**Files:**
- Create: `scripts/verify-asr-device-routing.sh`
- Create: `tests/specs/asr-device-routing.spec.ts`
- Create: `output/asr-device-routing/verification.md`
- Modify: `docs/prd/lifesub-real-asr-v0.2/.artifacts/process.md`
- Modify: `docs/prd/lifesub-real-asr-v0.2/.artifacts/notes.md`

- [ ] Prove unchanged restart performs zero trial runs.
- [ ] Prove model/runtime changes invalidate only the affected model.
- [ ] Inject OOM, timeout, Metal failure and trial-worker crash without affecting other models.
- [ ] Prove local_only and unauthorized policies perform zero network/upload calls.
- [ ] Prove automatic fallback records requested/actual Provider, reason and `audio_left_device`.
- [ ] Prove all-local-unavailable plus configured cloud succeeds, while missing cloud configuration becomes blocked_provider.
- [ ] Run full Rust, frontend, desktop and Playwright verification and record exact evidence.
- [ ] Commit `docs: verify ASR device routing`.

