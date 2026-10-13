# LifeSub ASR, Audio, And Settings Reference-Led Optimization Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the current packaged local-ASR release with truthful settings, diagnosable and recoverable native audio capture, durable model-management UI, then incrementally add evidence-preserving audio quality and workflow improvements informed by OpenWhispr, TypeWhisper, sherpa-onnx, and qwen3-asr-rs.

**Architecture:** Keep LifeSub's Rust Core, authenticated Swift capture helper, immutable raw audio sources, fenced ASR Jobs, Receipts, and append-only Revisions as the authority. Reference implementations contribute observable behaviors and state-machine patterns, not a replacement architecture: TypeWhisper informs macOS device probing and engine recovery; OpenWhispr informs model progress, microphone health checks, and AEC; sherpa-onnx and qwen3-asr-rs inform runtime-specific optimization. Release-gate work is separated from post-release enhancement work so AEC, live preview, and Qwen 1.7B optimizations cannot delay the first trustworthy packaged closure.

**Tech Stack:** Swift 6, Core Audio, AVFoundation/AVAudioEngine, ScreenCaptureKit, Rust 2024, Tauri 2, SQLite/rusqlite, sherpa-onnx 1.13.5, qwen3-asr 0.2.2 + Candle/Metal, React 19, TypeScript, Vitest, Testing Library, Playwright, shell verification.

**Requirements and analysis inputs:**

**Execution update (2026-10-13):** User started the bounded 0.2.2 S1-S4 bundle. The September 3 UI refresh already covered part of S1-S3; the current implementation records only the remaining delta in the feature process log. Recovery intermediate states are internal to the capture coordinator and have no current frontend-readable fact, so S2 cannot display `recovering/recovered` without violating this plan's no-new-helper-event boundary. Do not fabricate them. S4 and release gates are in progress.

- `docs/context/wiki/syntheses/asr-audio-reference-project-source-analysis.md`
- `docs/context/wiki/syntheses/voicestudio-source-review-2026-10-13.md`
- `docs/context/wiki/syntheses/asr-open-source-comparison-and-scope-review.md`
- `docs/superpowers/specs/2026-08-15-lifesub-real-asr-design.md`
- `docs/superpowers/specs/2026-08-25-lifesub-microphone-device-selection-design.md`
- `docs/superpowers/plans/2026-08-15-lifesub-real-asr-v0.2.md`
- `docs/superpowers/plans/2026-08-25-lifesub-microphone-device-selection.md`
- `docs/testing-and-review-policy.md`

---

## 1. Scope And Release Boundary

### 2026-10-13 VoiceStudio reference addendum

The fixed `VoiceStudio@36862c1157a25db7f7b885efba50a072ddf70e45` source review makes VoiceStudio a **high-priority reference for reliability and operability**, not a new LifeSub dependency or a parity target. The [source review](../../context/wiki/syntheses/voicestudio-source-review-2026-10-13.md) maps its Electron/Python/Rust implementation to LifeSub's current Rust Core, authenticated Swift helper, and Evidence contracts.

- Keep `0.2.2` at S1-S4 and not started. In S4, measure diagnostic refresh on a large Chunk/model fixture and cold/warm model latency and memory alongside the existing startup, first-frame, RTF, RSS, and cancel baselines. This is measurement only; a bounded-scan contract or an ASR sidecar would need its own implementation decision if a measured fault justifies it.
- After stabilization, classify failures in real meeting/retrieval samples before choosing the next product theme. VoiceStudio's read-only, bounded silence/clipping/invalid-sample scan is a candidate only if inaudible-source failures matter; warnings must link to original Chunk time and never alter raw audio or masquerade as model confidence.
- Treat ASR process isolation as a high-priority **risk investigation** after S4 or a real native crash/uncancellable timeout. LifeSub currently executes native ASR on an in-process thread, so a process-level native failure could affect the desktop app. A new process boundary requires a separate design and must prove capture continuity, exact model/input identity, Job recovery, cancellation, and resource limits. Do not import VoiceStudio's automatic fallback behavior.
- For Task 11, make read-only storage totals bounded and explicitly complete/partial before retention work. Before a new model or formal release, document per-asset license/notice evidence separately from the manifest's declared SPDX field. Neither item is a new `0.2.2` runtime requirement.
- VoiceStudio's native Sortformer v1 path rejects audio over 120 seconds; it is not a direct long-meeting diarization solution. Its turn-boundary validation and cancellation are useful acceptance ideas after anonymous-speaker quality has been measured.

### 2026-09-11 product alignment addendum

User-confirmed follow-up: combine corrected transcription and target-conversation filtering into one user-facing **AI Correction** edition, alongside original ASR. Processing dimensions are selectable in settings; terminology correction, deduplication and background filtering must not become separate persisted full-text product layers. Tasks 13 and 16 below are historical decomposition: adapt them within this unified result contract before implementation, preserving source mappings and minimal provenance. Detailed behavior and proposed dimension defaults are in [product definition](../../product-brief.md). This does not start implementation or widen S1–S4.

See [Qwen Audio product-direction review](../../context/wiki/syntheses/qwen-audio-product-direction-review.md) and [current roadmap](../../roadmap.md). This is a documentation alignment, not a development-start signal. Historical release-gate instructions below remain historical where 0.2.1 closure supersedes them.

- Keep 0.2.2 capped at S1–S4; first inventory September 3 UI deliveries so completed layout, copy and navigation work is not repeated.
- S2 should address the September 9 findings: hardcoded ASR-unconnected wording, optional models counted as required issues, and unsupported voiceprint/dictionary/polish promises. Project only existing facts; use unknown/neutral wording when current data cannot establish readiness.
- Treat recording sources, anonymous speakers and identified people as separate concepts. No new diarization, confidence persistence or normalized Revision contract enters stabilization.
- S4 remains a performance baseline. An offline meeting-quality study can inform later choices without adding a 0.2.2 release gate or changing its runtime.
- Tasks 6 and 9–16 remain deferred. Choose later implementation from measured retrieval/quality/storage failures; do not assume AEC, larger models, preview or CAM++ must all ship next.
- Adopt non-blocking interaction and honest completion/cancellation semantics through existing Core/Jobs. Do not copy an Agent Gateway, automatic Memory writer, or voice-execution frontend into LifeSub. Evidence consumers must independently authorize actions; recorded speech is data, not an execution command.

### Current release gate: complete before final DMG/install

1. Re-audit completed capture, ASR, model, Evidence, and UI behavior against the reference projects; keep LifeSub's stronger boundaries and only pull release-critical gaps into the current version.
2. Remove or constrain settings that currently persist without controlling production behavior.
3. Add permission, runtime, model, device, and storage diagnostics.
4. Prove the selected microphone can deliver real PCM before declaring capture ready.
5. Replace `recoverable=true` bookkeeping with a real same-UID engine recovery state machine.
6. Expose durable model download/install/qualification progress, cancellation, and retry.
7. Run packaged real-model, real-device, permission, restart, long-run, signing, DMG, and installed-app gates.

### Post-release optimization: must not block current DMG

1. Storage retention and recovery management.
2. Context-specific ASR profiles, hotkeys, and recording behavior settings.
3. Evidence-preserving AEC derivative audio and cross-source transcript deduplication.
4. sherpa bounded batch decode benchmarking.
5. Ephemeral live preview separated from committed Evidence.
6. Dictionary/normalization as a derived Revision.

### Explicit non-goals

- Do not add cloud ASR, API-key settings, team billing, plugin marketplace, or workspace administration.
- Do not silently fall back to the system-default microphone or another ASR model/runtime.
- Do not copy GPL TypeWhisper code, tests, resource names, file layout, or UX copy.
- Do not overwrite raw microphone/system-audio chunks with AEC or normalized derivatives.
- Do not treat preview text, heartbeats, compilation, or development binaries as release evidence.
- Do not re-implement already reviewed LifeSub architecture merely to resemble a reference project.
- Do not weaken immutable raw audio, exact source/model/runtime identity, fenced Jobs, atomic publication, append-only Revisions, or fail-closed fallback rules when a reference product uses a simpler contract.

### Reference-led change admission rule

A reference-project detail enters the current release only when at least one of these is true:

- the current product can report success while production behavior did not occur;
- settings, device, Provider, model, runtime, or Evidence identity can be misleading;
- capture can stop producing frames while the UI/session remains apparently healthy;
- a packaged permission, model operation, or native failure is not diagnosable or recoverable;
- the detail is required to prove the signed installed application's primary local recording-to-Evidence path.

Usability parity, optional workflow breadth, performance experiments, new derivative data products, and broad supply-chain automation remain post-release unless they close one of the failures above.

### Current state and recommended version split (2026-09-02)

`0.2.1` is release-closed. Tasks 1-5 and 7-8 are complete, the exact final candidate passed the 20/20 native matrix and packaged provider gates, and the verified DMG is installed at `/Applications/LifeSub.app`. There is no remaining `0.2.1` development or acceptance blocker.

The next version is not yet in development. When started, `0.2.2` should remain the bounded S1-S4 stabilization bundle defined below. Task 6 and Tasks 9-16 stay deferred and must not be pulled forward merely because they remain unchecked in this roadmap.

| Version | Recommended scope | Why this is the right stop line |
|---|---|---|
| 0.2.1 | Complete: Task 1-5 and 7-8; frozen signed app, 20/20 native reports, DMG and installed-app verification | Delivered the first trustworthy local recording -> ASR -> Receipt/Revision/Segment closure without reopening architecture |
| 0.2.2 | Planned, not started: S1-S4 lightweight clarity, operability, and baseline work over already-complete contracts | Improves usability while preserving the proven capture/ASR evidence path |
| 0.3+ | Settings/workflow expansion, retention/recovery products, derivative audio, transcript fusion, throughput experiments, streaming/preview | These change product surface area, evidence graph, or performance envelope and need their own design/acceptance |

### 0.2.1 closure rule

Do not add new product surface to 0.2.1 after the current candidate. The only acceptable late additions are fixes for:

- false success or false-ready states exposed by packaged runs;
- silent frame loss or broken same-UID recovery;
- identity, permission, or provenance ambiguity in the packaged app;
- inability to complete the signed local recording-to-Evidence path.

If a candidate improvement does not fix one of these, move it to 0.2.2 or later.

### Recommended 0.2.2 cap

The immediate next version should stop at lightweight stabilization over already-complete contracts. Treat `0.2.2` as full when it delivers only these four classes of work:

1. settings and diagnostics information architecture/readability improvements;
2. probe, recovery, model, and Job state clarity improvements that reuse existing facts;
3. operator affordances such as re-test/copy/open actions over existing data;
4. fixed-fixture and target-machine performance baselines for startup, first-frame, RTF, RSS, and cancellation.

Move the work out of `0.2.2` if it requires any of the following:

- schema migration or new durable entities;
- new helper/process/service boundaries;
- new capture-source semantics, fallback behavior, or transcript publication rules;
- new release-marker semantics or additional native acceptance matrix categories;
- changes to the source/chunk/Receipt/Revision graph rather than clearer projection of the same graph.

## 2. Reference Source Map

| LifeSub task | Reference source | Borrowed mechanism | Required adaptation |
|---|---|---|---|
| Settings truth and capability projection | OpenWhispr `SettingsPage.tsx`, TypeWhisper `SettingsView.swift` | Hide/disable unavailable modes; separate readiness from configuration | Core returns capabilities; React never infers support from visible fields |
| Permission and diagnostics center | TypeWhisper `AudioDeviceService.swift`, `AdvancedSettingsView.swift` | Device diagnostics report, permission/error visibility, diagnostics export | Use authenticated helper and LifeSub hashes/Evidence IDs |
| Microphone probe | TypeWhisper `AudioDeviceService.swift`, `AudioRecordingService.swift`; OpenWhispr `micTrackHealth.js` | Exact device compatibility test and first-live-frame proof | No default-device fallback; no persistent audio row from a probe |
| Same-UID recovery | TypeWhisper `AudioEngineRecoverySupport.swift`; OpenWhispr `activeMicRecovery.js` | debounce, quiescence, generation fencing, one in-flight recovery, burst limit | Rebind exact UID; seal discontinuity; emit versioned recovery evidence |
| Model progress UI | OpenWhispr `LocalWhisperPicker.tsx`, `useModelDownload` | model card, bytes/progress, cancel, retry, delete | Catalog checkpoint is authoritative; qualification remains distinct from download |
| Provider settings | OpenWhispr `transcriptionRoute.ts`; TypeWhisper `ModelManagerService.swift` | Provider/model capability filtering, ready/restorable distinctions | Preserve exact LifeSub execution snapshot and no-secret UI boundary |
| Recovery and retention | TypeWhisper `DictationRecoveryView.swift`, `AdvancedSettingsView.swift` | recovery recordings, retention days, backup/restore | Evidence-aware deletion; immutable revision and source referential integrity |
| Hotkeys and general behavior | TypeWhisper `HotkeySettingsView.swift`, recording settings; OpenWhispr general settings | conflict-aware hotkeys, audio cues, media pause | Restrict to LifeSub workflows; no text insertion/clipboard scope |
| AEC | OpenWhispr `meetingAecManager.js`, `meeting-aec-helper/src/aec_processor.cc` | WebRTC APM reverse/forward stream, helper supervision | Raw sources remain canonical; AEC is a derivative with provenance |
| ASR throughput | sherpa-onnx 1.13.5 `offline_asr.rs` | `decode_multiple_streams` | Only adopt after RTF/RSS/cancel/identity benchmark passes |
| Qwen 1.7B | qwen3-asr-rs `c5ef096...` | Candle/Metal model loading and streaming/batch APIs | Independent runtime qualification and device gate |
| Long-session diagnostics and storage | VoiceStudio `backend/services/storage_report.py` | Bounded directory scan, explicit partial result, background execution | S4 measures current diagnostics latency; Task 11 designs a truthful completeness contract if needed |
| ASR process fault containment | VoiceStudio `backend/services/subprocess_asr.py`, `asr_backend.py` | Distinguish timeout from a stopped native worker; restart a crashed child | Separate post-S4 risk decision, preserving exact LifeSub Job/Receipt/model/input identity |
| Read-only audio signal checks | VoiceStudio `backend/services/audio_quality.py`, Electron `audio-quality.tsx` | Time-linked silence/clipping/invalid-sample advice | Admit only after real meeting failure study; bound scan and keep raw Chunk authoritative |
| Model rights evidence | VoiceStudio `backend/services/model_licenses.py`, `LICENSE-NOTICE.md` | Separate declared license, inspected evidence, unresolved rights and runtime revision | Release governance for new assets; no claim that SPDX alone clears every use |

### License rules

- OpenWhispr is MIT in the inspected local snapshot. Reuse still requires attribution/notice review and should be adapted to LifeSub boundaries.
- TypeWhisper is GPL-3.0 with a separate commercial license. Use it only to write clean-room behavior specs and tests unless the project obtains commercial permission or intentionally accepts GPL obligations.
- sherpa-onnx is Apache-2.0 and is already an exact runtime dependency.
- qwen3-asr-rs is MIT and is already pinned to exact commit `c5ef09646af6278d2ba8b8ceaf543ffb32d1a5dc`.
- VoiceStudio application code is AGPL-3.0-only at the fixed inspected commit. Treat it as a clean-room behavior and verification reference unless compatible use has been explicitly reviewed; bundled OmniVoice code and downloaded model assets have separate terms. Do not copy VoiceStudio code, tests, UX copy, or structure into LifeSub by default.

### Fixed local reference paths

OpenWhispr snapshot root: `/Users/goldenwave/Downloads/openwhispr-main`

- Provider routing: `/Users/goldenwave/Downloads/openwhispr-main/src/helpers/transcriptionRoute.ts`
- Model UI: `/Users/goldenwave/Downloads/openwhispr-main/src/components/LocalWhisperPicker.tsx`
- Model operation hook: `/Users/goldenwave/Downloads/openwhispr-main/src/hooks/useModelDownload.ts`
- Settings shell and VAD/privacy/system sections: `/Users/goldenwave/Downloads/openwhispr-main/src/components/SettingsPage.tsx`
- Settings state: `/Users/goldenwave/Downloads/openwhispr-main/src/stores/settingsStore.ts`
- Microphone readiness: `/Users/goldenwave/Downloads/openwhispr-main/src/helpers/micTrackHealth.js`
- Active-device recovery: `/Users/goldenwave/Downloads/openwhispr-main/src/helpers/activeMicRecovery.js`
- Meeting transcription settings: `/Users/goldenwave/Downloads/openwhispr-main/src/components/settings/MeetingSettings.tsx`
- Upload transcription settings: `/Users/goldenwave/Downloads/openwhispr-main/src/components/settings/UploadSettings.tsx`
- AEC manager: `/Users/goldenwave/Downloads/openwhispr-main/src/helpers/meetingAecManager.js`
- AEC processor: `/Users/goldenwave/Downloads/openwhispr-main/native/meeting-aec-helper/src/aec_processor.cc`

TypeWhisper snapshot root: `/Users/goldenwave/Downloads/typewhisper-mac-main`

- Settings information architecture: `/Users/goldenwave/Downloads/typewhisper-mac-main/TypeWhisper/Views/SettingsView.swift`
- General settings: `/Users/goldenwave/Downloads/typewhisper-mac-main/TypeWhisper/Views/GeneralSettingsView.swift`
- Advanced/retention/diagnostics settings: `/Users/goldenwave/Downloads/typewhisper-mac-main/TypeWhisper/Views/AdvancedSettingsView.swift`
- Hotkeys: `/Users/goldenwave/Downloads/typewhisper-mac-main/TypeWhisper/Views/HotkeySettingsView.swift`
- Recovery: `/Users/goldenwave/Downloads/typewhisper-mac-main/TypeWhisper/Views/DictationRecoveryView.swift`
- Device identity/compatibility/diagnostics: `/Users/goldenwave/Downloads/typewhisper-mac-main/TypeWhisper/Services/AudioDeviceService.swift`
- Recording lifecycle/Bluetooth handling: `/Users/goldenwave/Downloads/typewhisper-mac-main/TypeWhisper/Services/AudioRecordingService.swift`
- Engine recovery policy: `/Users/goldenwave/Downloads/typewhisper-mac-main/TypeWhisper/Services/AudioEngineRecoverySupport.swift`
- Model readiness: `/Users/goldenwave/Downloads/typewhisper-mac-main/TypeWhisper/Services/ModelManagerService.swift`
- File/batch jobs: `/Users/goldenwave/Downloads/typewhisper-mac-main/TypeWhisper/ViewModels/FileTranscriptionViewModel.swift`
- License: `/Users/goldenwave/Downloads/typewhisper-mac-main/LICENSE`

Runtime upstreams:

- sherpa Rust API: `/Users/goldenwave/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sherpa-onnx-1.13.5/src`
- Qwen 1.7B runtime: `/Users/goldenwave/.cargo/git/checkouts/qwen3-asr-rs-4ea82386f332ccaa/c5ef096`

High-priority product reference: `.sources/reference-projects/VoiceStudio` (`main@36862c1157a25db7f7b885efba50a072ddf70e45`, Git metadata retained, ignored by LifeSub). See the linked source review for exact files and scope. Its optional `omnivoice-gallery` submodule was not initialized for this audit.

The OpenWhispr and TypeWhisper download directories do not contain `.git` metadata. Treat them as inspected source snapshots, not commit-verifiable release inputs. Re-fetch an exact Git commit before copying any MIT implementation or performing a formal license/security audit.

## 3. File Ownership Map

### Settings truth and diagnostics

- Modify `src-tauri/src/domain.rs`: canonical recording/ASR configuration validation and backward compatibility.
- Modify `src-tauri/src/commands.rs`: settings capability projection, actual runtime paths, diagnostics commands, model control commands.
- Create `src-tauri/src/diagnostics.rs`: typed app/runtime/model/capture/storage diagnostic snapshot without UI concerns.
- Create `src-tauri/src/diagnostics_test.rs`: permission/device/model/storage identity and redaction tests.
- Modify `src-tauri/src/lib.rs`: register commands and tests.
- Modify `src/services/lifesub.ts`: typed diagnostics, model progress, cancel/retry clients.
- Modify `src/services/lifesub.test.ts`: exact invoke payload and DTO tests.
- Modify `src/components/SettingsModal.tsx`: new Settings tabs.
- Modify `src/components/RecordingSettings.tsx`: truthful capture controls only.
- Modify `src/components/AsrSettings.tsx`: provider-specific capability controls.
- Modify `src/components/ModelManager.tsx`: durable progress and lifecycle controls.
- Create `src/components/DiagnosticsSettings.tsx`: permission/device/runtime/storage diagnostics.
- Create `src/components/DiagnosticsSettings.test.tsx`.
- Modify `src/components/SettingsModal.test.tsx`.
- Modify `src/settings.css` and `src/styles.css` using existing tokens only.

### Native microphone probe and recovery

- Modify `src-tauri/native/capture-helper/Sources/CaptureProtocol/CaptureProtocol.swift`: versioned probe/recovery DTOs.
- Modify `src-tauri/src/capture/protocol.rs`: matching Rust DTOs and strict codecs.
- Create `src-tauri/native/capture-helper/Sources/CaptureHelper/MicrophoneProbe.swift`: transient exact-UID probe with first-frame proof.
- Create `src-tauri/native/capture-helper/Sources/CaptureHelper/AudioEngineRecovery.swift`: recovery policy and state machine.
- Modify `src-tauri/native/capture-helper/Sources/CaptureHelper/MicrophoneCapture.swift`: delegate recovery, first-frame tracking, exact-UID rebind.
- Modify `src-tauri/native/capture-helper/Sources/CaptureHelper/CaptureSessionController.swift`: recovery lifecycle and event ordering.
- Modify `src-tauri/native/capture-helper/Sources/CaptureHelper/CommandLoop.swift`: authenticated probe command and recovery events.
- Create `src-tauri/native/capture-helper/Tests/CaptureHelperTests/MicrophoneProbeTests.swift`.
- Create `src-tauri/native/capture-helper/Tests/CaptureHelperTests/AudioEngineRecoveryTests.swift`.
- Modify `src-tauri/native/capture-helper/Tests/CaptureHelperTests/MicrophoneCaptureTests.swift`.
- Modify `src-tauri/src/capture/helper.rs`: one-shot authenticated probe lifecycle and reap.
- Modify `src-tauri/src/capture/coordinator.rs`: recovering/recovered source state.
- Modify `src-tauri/src/capture/session.rs`: startup/probe/recovery stable errors and Catalog boundaries.
- Modify `src-tauri/src/capture_helper_test.rs`, `capture_protocol_test.rs`, `capture_coordinator_test.rs`, `session_test.rs`.

### Model progress and release

- Modify `src-tauri/src/asr/model_manager.rs` and its focused modules only where command-safe control/query APIs are missing.
- Modify `src-tauri/src/catalog/jobs.rs` or model download query owner, preserving Catalog as fact source.
- Create `src/modelManager.ts`: shared model operation controller and generation fencing.
- Create `src/modelManager.test.ts`.
- Modify `scripts/verify-native-capture.sh`, `tests/scripts/verify-native-capture.test.sh`.
- Modify `scripts/verify-desktop-asr.sh`, `tests/scripts/verify-desktop-asr.test.sh`.
- Modify `scripts/verify-release-source.sh` only when the final capability marker is legitimately earned.
- Update `docs/prd/lifesub-real-asr-v0.2/.artifacts/process.md`, `notes.md`, and `docs/workspace-status.md` after each completed chunk.

### Post-release derivative audio and settings

- Create a dedicated design/spec before implementation because AEC changes the evidence graph.
- Likely create `src-tauri/src/audio_derivative.rs`, `catalog/audio_derivatives.rs`, migration DDL/fingerprint, and focused tests.
- Add a C/C++ WebRTC APM target linked into the existing authenticated capture helper or a separately authenticated helper only after a packaging spike proves the closure.
- Do not add a second unauthenticated local service.

---

## Chunk 0: Re-Audit Completed Capabilities Against References

This is a bounded release-hardening audit. It does not reopen approved architecture or require churn where current evidence already proves a stronger LifeSub contract. Each finding is classified as `keep`, `current-release-hardening`, `post-release`, or `reject` and names its observable risk and verification evidence.

### Task R1: Freeze The Completed-Capability Baseline And Gap Matrix

**What LifeSub will build:** A traceable matrix mapping completed production capabilities to LifeSub evidence, relevant reference behavior, and the disposition of any gap.

**Files:**

- Modify: `docs/context/wiki/syntheses/asr-audio-reference-project-source-analysis.md`
- Modify: `docs/prd/lifesub-real-asr-v0.2/.artifacts/process.md`
- Modify: `docs/prd/lifesub-real-asr-v0.2/.artifacts/notes.md`

- [x] **Step 1: Inventory completed production capabilities and fresh evidence**

Cover authenticated helper lifecycle, exact-UID selection, start cancellation, first committed PCM anchoring, chunk rotation/sealing, restart reconciliation, native Job enqueue, provider execution, Receipt/Revision/Segment publication, model qualification, committed-only UI, and packaged signing/permission behavior.

- [x] **Step 2: Classify every reference detail**

```text
keep                     current LifeSub behavior is equal or stronger
current-release-hardening observable release risk; absorb into Task 1-5 or 7-8
post-release             useful improvement without current release risk
reject                   conflicts with LifeSub Evidence, identity, license, or security boundaries
```

- [x] **Step 3: Require evidence for every current-release finding**

Each finding names the observable failure, owned task, affected contract, RED/GREEN or real-environment proof, and release gate. Feature parity alone is not a finding.

- [x] **Step 4: Record preserved strengths**

Preserve immutable raw audio, exact UID without default fallback, authenticated single helper, anchored file access, immutable model identity, fenced Jobs, atomic Receipt/Revision publication, append-only Revisions, and committed-only Evidence.

### Task R2: Re-Audit Native Capture And Lifecycle Details

**Reference focus:** TypeWhisper device probing, route readiness, configuration recovery, cancellation, and teardown; OpenWhispr health checks and generation fencing. TypeWhisper remains clean-room behavior reference only.

- [x] **Step 1: Prove the existing completed capture path before changing it**

Confirm exact UID bind/readback, selected-device mismatch rejection, start cancellation, first committed non-empty PCM frame, sample/host clock consistency, canonical WAV staging, atomic sealing, helper shutdown/reap, listener/tap cleanup, and restart reconciliation.

- [x] **Step 2: Pull only uncovered release risks into current tasks**

Map passive listing versus active first-frame probe to Task 3; stale callback fencing, one in-flight same-UID recovery, teardown safety, post-recovery frame proof, and burst failure to Task 4; report bindings and real Bluetooth/external-device evidence to Task 7.

- [x] **Step 3: Defer broader audio behavior**

Keep active-device hot switching, Bluetooth system-default mutation, AEC, codec/storage changes, and multi-device aggregation outside the current release.

### Task R3: Re-Audit ASR, Model, Job, And Evidence Details

**Reference focus:** OpenWhispr provider routing and model operations; TypeWhisper configured/ready/restorable and job lifecycle distinctions; exact sherpa-onnx and qwen3-asr-rs runtime APIs.

- [x] **Step 1: Prove existing LifeSub authority boundaries**

Confirm one canonical Provider validation path, exact execution snapshot and installation lease, anchored audio verification, cancellation/lease renewal, silent-audio success semantics, durable failure replay, Receipt provenance, chunk-bound Segments, atomic publication, and no overwrite of prior Revision.

- [x] **Step 2: Improve projections rather than duplicate engines**

Map capability-driven Provider/model/language controls to Task 1, durable model stage/bytes/cancel/retry/restoration to Task 5, and packaged known-fixture plus persisted Evidence identity to Task 8.

- [x] **Step 3: Defer performance and new data products**

Keep warm caches/model unload, Qwen 1.7B streaming, sherpa batch decode, preview text, normalization, AEC, and cross-source fusion in Tasks 6 and 9-16 or later dedicated plans.

### Task R4: Re-Audit Product State, Permissions, And Error Recovery

- [x] **Step 1: Check every completed user-visible state against production facts**

Cover capture mode, selected microphone, permission owner/status, source readiness, lifecycle busy states, durable capture/ASR errors, model readiness, storage location, and restart replay.

- [x] **Step 2: Absorb only actionable release gaps**

Map persisted-only or Provider-inapplicable controls to Task 1, diagnostic and redacted export gaps to Task 2, microphone compatibility status to Task 3, model operation visibility to Task 5, and signed installed-app permission ownership to Task 8.

- [x] **Step 3: Reject misleading convenience behavior**

Do not add automatic source/model fallback, success inferred from command return, preview text presented as Evidence, or controls whose values are not consumed by production.

### Chunk 0 Completion Gate

- [x] Every completed capability has a disposition and evidence link.
- [x] Every `current-release-hardening` item is owned by Task 1-5 or 7-8 with a concrete verifier.
- [x] Tasks 6 and 9-16 remain non-blocking for LifeSub 0.2.1.
- [x] No accepted reference detail weakens LifeSub's Evidence or identity contracts.

## Chunk 0.5: Optimize Completed Capabilities Without Reopening The Release

This chunk records the optimization boundary for capabilities that are already implemented. “Complete” means the contract exists and is covered by focused tests; it does not mean every presentation, recovery hint, or performance path is finished. Only the first group below may still enter 0.2.1, and only when final packaged evidence exposes a concrete failure.

### Must remain in 0.2.1 release closure

| Completed area | Reference-led refinement | Why it is release-critical | Acceptance evidence |
|---|---|---|---|
| Settings truth | Keep capability-driven controls; verify displayed values come from Core/runtime facts and unsupported options are hidden or read-only | Prevents false claims about what production can do | Settings tests, packaged screenshot, no persisted-only release control |
| Microphone selection/probe | Keep exact UID, explicit selection, first-frame proof, and stable failure codes; no fallback or implicit default-device mutation | A selectable but silent device is a false-ready state | v3 probe matrix, permission/no-device fixtures, UID/readback evidence |
| Same-UID recovery | Keep one in-flight recovery, generation fencing, discontinuity sealing, and post-recovery first-frame proof | `recoverable=true` without rebind can silently lose audio | Packaged configuration-change smoke, post-recovery chunk and ASR IDs |
| Model operations | Keep durable bytes/stages, dependency projection, cancel acknowledgement, exact retry, and restart restoration | UI-inferred readiness can orphan work or misreport install state | Catalog-backed tests, interrupted/resumed evidence, provider gate |
| Diagnostics | Keep permission owner/status, identity hashes, model/runtime state, storage and stable errors; export remains redacted | Packaged failures must be explainable without leaking private data | DTO/redaction tests, screenshots, export inspection |
| Packaged primary path | Finish capture -> WAV -> Job -> Receipt/Revision/Segment with signed app/helper identity | Development success cannot prove the installed product | v3 reports, provider gate, Tier 2/3, DMG/install evidence |

### Worth optimizing immediately after 0.2.1, but not a release blocker

| Area | Lightweight improvement | Explicitly exclude for now |
|---|---|---|
| Settings UX | Reorder into Capture, ASR/Models, Diagnostics; show concise readiness summaries and next actions | Profiles, broad general settings, hotkeys, media control |
| Capture UX | Show probe age, transport/readiness explanation, a re-test action, and clearer recovery status | Active hot switching, automatic fallback, aggregate-device orchestration |
| Model UX | Add a compact operation timeline and clearer downloaded/verified/qualified/usable language | Auto-unload, cache eviction policy, model marketplace |
| Job/ASR UX | Show queued/preparing/transcribing/publishing and durable recovery in the session timeline | Live preview, streaming Qwen, provider fusion |
| Diagnostics UX | Add copyable stable error codes, next-action guidance, and bounded recent-attempt history | Full telemetry, cloud reporting, raw log export |
| Performance | Measure startup, first-frame, RTF, RSS, and cancellation latency on representative fixtures | Batch decode adoption, warm model cache, codec/storage migration |

### Defer to later optimization versions

Keep Tasks 9-16 as separate follow-on work. AEC, cross-source deduplication, preview, normalization, retention/deletion, Qwen streaming, diarization, Opus/DTX, and active-device hot switching change the data graph, lifecycle, or performance envelope and require their own design and acceptance gates.

For the immediate next version, prefer the lightweight improvements in this chunk over starting Tasks 9-16. Those larger tasks are roadmap candidates, not the default 0.2.2 scope.

### Optimization admission rule

An optimization may enter 0.2.1 only if packaged evidence shows false success, silent frame loss, identity/permission ambiguity, an unrecoverable user-visible failure, or inability to complete the primary local recording-to-Evidence path. Visual polish, convenience parity, lower latency, and broader workflow coverage remain post-release unless they resolve one of those conditions.

### Recommended immediate follow-on release: 0.2.2 stabilization

This is the default next version after `0.2.1`, and it is intentionally smaller than Tasks 9-16. It improves already-shipped capabilities without changing the Evidence model, Provider contracts, helper topology, or release gate type.

#### Stabilization S1: Settings And Diagnostics IA Cleanup

**What LifeSub will build:** Reorganize the current Settings surface around the facts users actually need to complete recording and ASR setup, while preserving the same production contracts.

- Reorder the surface into `Capture`, `ASR & Models`, and `Diagnostics`.
- Add compact readiness summaries and next-action hints sourced from the existing diagnostics and capability DTOs.
- Keep unsupported controls hidden/read-only; do not add profiles, general behavior settings, or retention flows.
- Verify with focused UI tests plus desktop and 760px screenshots.

#### Stabilization S2: Probe, Recovery, Model, And Job State Clarity

**What LifeSub will build:** Make the already-implemented lifecycle states easier to read and act on without changing their underlying contracts.

- Show probe age, transport/readiness explanation, and explicit re-test affordance for the selected microphone.
- Show clearer same-UID recovery status wording using the existing `recovering/recovered/failed` facts.
- Add a compact model operation timeline and clearer language for `downloaded`, `verified`, `installed`, and `runtime-qualified`.
- Show `queued/preparing/transcribing/publishing` in the session timeline only if the states are already durably present.
- Do not add live preview, hot switching, new events, or provider fusion.

#### Stabilization S3: Diagnostics Operator Affordances

**What LifeSub will build:** Improve field usability for support and self-debugging using data that already exists in the current diagnostic snapshot.

- Make stable error codes, candidate hashes, and selected-device facts easy to copy.
- Add open-in-Finder/open-data-location style actions only for existing anchored paths.
- Add bounded recent-attempt visibility only if it can be derived from current in-memory or persisted stable errors without new storage.
- Do not export raw logs, transcript bodies, model install absolute paths, or telemetry streams.

#### Stabilization S4: Performance Baseline Pack

**What LifeSub will build:** A repeatable report for the primary local path, so later optimizations have a fixed baseline instead of anecdotal comparisons.

- Measure startup latency, first-frame latency, packaged RTF, peak RSS, and cancel acknowledgement/settle latency on fixed fixtures and target hardware.
- Include a high-file-count diagnostics refresh fixture and cold/warm model runs; report latency, memory, and whether the current storage totals finished. This records the current contract and does not silently turn a slow or failed scan into a complete total.
- Save the report and thresholds in docs/output artifacts, not in new runtime state.
- Only implement code changes here when the measurements expose a regression in an already-approved contract.
- Do not start batch decode, warm cache, codec migration, or Qwen streaming work from this task.

#### 0.2.2 done line and task bundle

Treat `0.2.2` as complete when the four bounded tasks below land together and none of them crosses into schema/helper/Evidence-graph changes. This is the intended stop line for the immediate next version.

### Task S1: Settings And Diagnostics IA Cleanup

**Why this is necessary:** Task 1 and Task 2 already made the settings truthful, but the current surface still exposes those facts in a fragmented way. The next version should improve comprehension, not add new knobs.

**Files:**

- Modify: `src/components/SettingsModal.tsx`
- Modify: `src/components/RecordingSettings.tsx`
- Modify: `src/components/AsrSettings.tsx`
- Modify: `src/components/DiagnosticsSettings.tsx`
- Modify: `src/components/SettingsModal.test.tsx`
- Modify: `src/components/DiagnosticsSettings.test.tsx`
- Modify: `src/settings.css`
- Modify: `src/styles.css`

- [x] Reorder the settings surface into `Capture`, `ASR & Models`, and `Diagnostics`.
- [x] Add compact readiness summaries sourced only from existing capability/diagnostic DTOs.
- [x] Keep unsupported controls hidden or read-only; do not add new persisted settings.
- [x] Verify with focused UI tests and 1180px desktop plus 760px browser screenshots.

### Task S2: Existing State Clarity For Probe, Recovery, Model, And Job

**Why this is necessary:** the contracts are implemented, but some important states still read like internal engineering terms. The next version should make already-durable facts easier to interpret without changing capture or publication behavior.

**Files:**

- Modify: `src/components/MicrophoneDeviceSelector.tsx`
- Modify: `src/components/LiveCapture.tsx`
- Modify: `src/components/ModelManager.tsx`
- Modify: `src/components/TimelineView.tsx`
- Modify: `src/microphoneDevices.ts`
- Modify: `src/modelManager.ts`
- Modify: `src/components/MicrophoneDeviceSelector.test.tsx`
- Modify: `src/components/ModelManager.test.tsx`
- Modify: `src/components/live_capture.test.tsx`

- [x] Show probe age, transport/readiness explanation, and an explicit re-test affordance using the existing probe snapshot.
- [ ] Reword same-UID recovery states from raw lifecycle language into user-actionable status text without inventing new states. These states are internal to the capture coordinator and have no frontend-readable fact; this item needs a separate contract decision outside the no-new-helper-event cap.
- [x] Show model operation progression with clearer `downloaded` / `verified` / `installed` / `runtime-qualified` wording.
- [x] Surface `queued` / `preparing` / `transcribing` / `publishing` only where those states are already durable. `publishing` is not durable and is not shown.
- [x] Do not add live preview, hot switching, provider fusion, or new helper events.

### Task S3: Diagnostics Operator Affordances Over Existing Facts

**Why this is necessary:** current diagnostics are truthful but still a little expensive to use during self-support and bug triage. The next version should improve actionability, not widen the snapshot boundary.

**Files:**

- Modify: `src/components/DiagnosticsSettings.tsx`
- Modify: `src/components/DiagnosticsSettings.test.tsx`
- Modify: `src/services/lifesub.ts`
- Modify: `src/services/lifesub.test.ts`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/diagnostics.rs`
- Modify: `src-tauri/src/diagnostics_test.rs`

- [x] Make stable error codes, candidate hashes, selected-device facts, and anchored data paths easy to copy.
- [x] Add bounded open/copy/re-test actions only when they operate on already-anchored paths or existing commands. Copy and exact-UID re-test are shipped; Finder open is omitted because a snapshot path is not a live anchor.
- [x] If recent-attempt history is shown, derive it from current stable errors or in-memory facts only; do not add new storage. No history is shown.
- [x] Keep exports redacted; do not add raw logs, transcript bodies, model install absolute paths, or telemetry streaming.

### Task S4: Baseline Report Pack For The Proven Local Path

**Why this is necessary:** after `0.2.1`, optimization decisions should come from repeatable measurement rather than from reference-project intuition.

**Files:**

- Modify: `scripts/verify-desktop-asr.sh`
- Modify: `scripts/collect-native-capture-report.py`
- Modify: `docs/prd/lifesub-real-asr-v0.2/.artifacts/process.md`
- Modify: `docs/prd/lifesub-real-asr-v0.2/.artifacts/notes.md`
- Create or update: `output/asr-v0.2/` baseline reports

- [ ] Freeze fixture and target-machine measurement commands for startup, first-frame, packaged RTF, peak RSS, and cancel settle latency.
- [ ] Measure diagnostics refresh with many Chunk/model files and compare cold versus warm model load on the same frozen machine/build. Record failures, incomplete measurements, and elapsed time explicitly; open a separate bounded-scan or process-isolation design only if the evidence warrants it.
- [x] Save the available baseline as reports and notes, not as new runtime state or new release-marker semantics; missing target-machine measurements remain explicitly open.
- [x] Only open implementation work when the measured result reveals regression in an already-approved contract.
- [x] Do not start batch decode, warm cache, codec migration, Qwen streaming, or new acceptance-matrix categories from this task.

#### 0.2.2 exclusion gate

Move the work out of `0.2.2` immediately if any proposed change needs one of these:

- schema migration or new durable entities;
- new helper, process, or service boundaries;
- changes to source/chunk/Receipt/Revision semantics;
- new release-marker meaning or new native acceptance-matrix categories;
- new settings whose values do not already control proven production behavior.

---

## Chunk 1: Settings Truth Before More Settings

### Task 1: Replace Persisted-Only Controls With Runtime Truth

**What LifeSub will build:** Make every visible setting either control production behavior or display as read-only capability information. Keep old JSON readable, but reject or migrate unsupported values instead of pretending they are active.

**Reference modules:**

- OpenWhispr: `src/components/SettingsPage.tsx` mode availability and disabled-state patterns.
- TypeWhisper: `TypeWhisper/Services/ModelManagerService.swift` configured/ready/restorable distinction.
- Do not copy TypeWhisper implementation; translate states into LifeSub contracts.

**Files:**

- Modify: `src-tauri/src/domain.rs`
- Modify: `src-tauri/src/domain_test.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src/components/RecordingSettings.tsx`
- Modify: `src/components/AsrSettings.tsx`
- Modify: `src/components/SettingsModal.test.tsx`
- Modify: `src/services/lifesub.ts`

- [x] **Step 1: Write failing settings-truth tests**

Prove:

- native capture exposes fixed output sample rate `16_000` instead of accepting 8/44.1/48 kHz as active choices;
- the displayed storage location is the actual anchored LifeSub data directory;
- IM detection, detection delay, and recovery delay are not interactive while no production detector consumes them;
- ITN appears only for SenseVoice;
- Whisper exposes `transcribe/translate` from canonical provider options;
- language choices come from the selected model capabilities, including zh/en/ja/ko/yue when declared;
- old persisted recording config remains readable and is normalized without data loss.

- [x] **Step 2: Run focused tests and verify RED**

```bash
npx vitest run src/components/SettingsModal.test.tsx
scripts/with-sherpa-runtime.sh cargo test --manifest-path src-tauri/Cargo.toml --features desktop domain_test -- --nocapture
scripts/with-sherpa-runtime.sh cargo test --manifest-path src-tauri/Cargo.toml --features desktop settings_capabilities -- --nocapture
```

Expected: FAIL because controls are currently static/editable and runtime capabilities are not projected.

- [x] **Step 3: Add a typed settings capability projection**

Add a DTO similar to:

```rust
pub struct SettingsCapabilities {
    pub capture_output_sample_rate_hz: u32,
    pub data_directory: String,
    pub im_detection_available: bool,
    pub supported_capture_modes: Vec<String>,
}
```

React consumes this DTO; it does not infer support from stored config fields.

- [x] **Step 4: Make Provider controls capability-driven**

Map canonical `AsrProviderOptions` to UI fields. Never show SenseVoice ITN for Whisper/Qwen or Whisper task for other Providers.

- [x] **Step 5: Run focused tests and production frontend build GREEN**

```bash
npx vitest run src/components/SettingsModal.test.tsx src/services/lifesub.test.ts
npm run build
git diff --check
```

- [x] **Step 6: Preserve the canonical integration tree without a partial commit**

Task 1 behavior and verification are complete. The release source is intentionally identified by commit plus dirty digest; creating an isolated partial commit would mix or split pre-existing production changes in the same owned files, so no partial commit was made.

```bash
git add src-tauri/src/domain.rs src-tauri/src/domain_test.rs src-tauri/src/commands.rs src/components/RecordingSettings.tsx src/components/AsrSettings.tsx src/components/SettingsModal.test.tsx src/services/lifesub.ts src/services/lifesub.test.ts
git commit -m "fix: make recording and ASR settings truthful"
```

Do not include unrelated dirty integration files in the commit.

### Task 2: Add Permissions And Diagnostics Settings

**What LifeSub will build:** A Settings tab that shows microphone/screen-recording permission, selected device identity and compatibility, helper/app/runtime/model hashes, actual data location, recent stable capture error, and actions to refresh/export/open macOS Settings.

**Reference modules:**

- TypeWhisper `AudioDeviceService.swift`: structured device diagnostics report.
- TypeWhisper `AdvancedSettingsView.swift`: diagnostics export and user-facing status.
- OpenWhispr `SettingsPage.tsx` system/privacy sections: visible permission and storage state.

**Files:**

- Create: `src-tauri/src/diagnostics.rs`
- Create: `src-tauri/src/diagnostics_test.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Create: `src/components/DiagnosticsSettings.tsx`
- Create: `src/components/DiagnosticsSettings.test.tsx`
- Modify: `src/components/SettingsModal.tsx`
- Modify: `src/services/lifesub.ts`
- Modify: `src/services/lifesub.test.ts`
- Modify: `src/settings.css`

- [x] **Step 1: Write failing diagnostic DTO and redaction tests**

Require exact fields and prove the export excludes launch nonce, API secrets, full transcript/audio content, and user-home paths outside the LifeSub data root.

- [x] **Step 2: Write failing UI tests**

Cover loading, permission granted/denied/not-determined, device disconnected/incompatible, helper/model mismatch, refresh race, export failure, and macOS Settings action.

- [x] **Step 3: Run focused tests RED**

```bash
scripts/with-sherpa-runtime.sh cargo test --manifest-path src-tauri/Cargo.toml --features desktop diagnostics_test -- --nocapture
npx vitest run src/components/DiagnosticsSettings.test.tsx src/services/lifesub.test.ts
```

- [x] **Step 4: Implement the Rust diagnostic snapshot**

Read state from the current CoreRuntime, Catalog, helper discovery, build attestation, and filesystem metadata. Do not open a second writable Catalog or bypass the authenticated helper.

- [x] **Step 5: Implement the compact Settings UI**

Use full-width sections rather than nested cards. Use Lucide icons for refresh/export/open actions, tooltips, existing tokens, `role=status`, and stable row heights.

- [x] **Step 6: Verify GREEN and capture visual evidence**

```bash
npm test
npm run build
scripts/check.sh tier2
```

Capture desktop and 760px-minimum-width Settings screenshots under `output/playwright/`.

---

## Chunk 2: Prove The Selected Device Actually Records

### Task 3: Add Exact-UID Microphone Compatibility Probe

**What LifeSub will build:** A transient authenticated probe that binds the selected UID, reads it back, installs the capture path, waits for at least one valid non-empty PCM frame before a total deadline, reports compatibility, and cleans up without creating Catalog source/chunk/ASR rows.

**Reference modules:**

- TypeWhisper `AudioDeviceService.swift`: cannotSetDevice/invalidInputFormat/engineStartFailed compatibility states.
- TypeWhisper `AudioRecordingService.swift`: Bluetooth readiness deadline and multi-channel tap-format lessons.
- OpenWhispr `micTrackHealth.js`: track readiness is separate from API acquisition success.

**Files:**

- Create: `src-tauri/native/capture-helper/Sources/CaptureHelper/MicrophoneProbe.swift`
- Create: `src-tauri/native/capture-helper/Tests/CaptureHelperTests/MicrophoneProbeTests.swift`
- Modify: `CaptureProtocol.swift`, Rust `capture/protocol.rs`, and their tests.
- Modify: `CommandLoop.swift`, Rust `capture/helper.rs`, `commands.rs`.
- Modify: `MicrophoneDeviceSelector.tsx` and tests to show compatibility/test action.

- [x] **Step 1: Define versioned probe contract tests RED**

The response includes UID/name, input sample rate/channels, transport, read-back UID, first-frame sample count, elapsed milliseconds, and one stable result:

```text
compatible
permission_denied
unavailable
identity_mismatch
invalid_format
no_audio_frames
route_conflict
timeout
```

Unknown fields/results, duplicate responses, extra frames, timeout, cancellation, missing shutdown ACK, and child leak fail closed.

- [x] **Step 2: Prove zero side effects**

Tests assert no staging file, capture source, chunk, ASR Job, permission prompt during passive listing, or fallback device.

- [x] **Step 3: Implement exact-UID first-frame probe**

Use the same device resolver/binder as production capture. Share code; do not build a second device-selection implementation.

- [x] **Step 4: Run Swift/Rust/UI focused tests GREEN**

```bash
swift test --package-path src-tauri/native/capture-helper
scripts/with-sherpa-runtime.sh cargo test --manifest-path src-tauri/Cargo.toml --features desktop capture_protocol_test -- --nocapture
scripts/with-sherpa-runtime.sh cargo test --manifest-path src-tauri/Cargo.toml --features desktop capture_helper_test -- --nocapture
npx vitest run src/components/MicrophoneDeviceSelector.test.tsx src/components/SettingsModal.test.tsx
```

### Task 4: Implement Real Same-UID Audio Engine Recovery

**What LifeSub will build:** Replace the current “seal and keep running” behavior with a bounded recovery that rebuilds the engine path for the same stable UID and proves new frames arrive.

**Reference modules:**

- TypeWhisper `AudioEngineRecoverySupport.swift`: debounce, quiescence, backoff, generation, burst limit.
- OpenWhispr `activeMicRecovery.js`: one in-flight recovery and stale-attempt fencing.
- Clean-room requirement applies to all TypeWhisper behavior.

**Files:**

- Create: `AudioEngineRecovery.swift` and tests.
- Modify: `MicrophoneCapture.swift`, `CaptureSessionController.swift`, `CommandLoop.swift`.
- Modify: Swift/Rust protocol, coordinator/session, focused tests.

- [x] **Step 1: Write recovery state-machine tests RED**

Cover:

- config-change burst coalesces to one recovery;
- recovery during start waits until startup commits;
- stop cancels delayed recovery;
- old generation cannot restart a stopped/new session;
- exact same UID rebind/readback succeeds;
- first-frame proof is mandatory;
- disappearance, UID mismatch, timeout, and burst limit fail once;
- recovery never changes to system default;
- all listeners/taps/engines are cleaned exactly once.

- [x] **Step 2: Add versioned events**

Prefer explicit events:

```text
source_recovering { source, generation, reason }
source_recovered { source, generation, device_id, first_host_time }
```

Rust transitions source state `Running -> Recovering -> Running` only after `source_recovered`. A recoverable notification alone is not success.

- [x] **Step 3: Implement clean-room recovery policy**

Use project constants for debounce/quiescence/burst/backoff selected from LifeSub hardware measurements, not copied TypeWhisper numbers by default.

- [x] **Step 4: Prove chunk and Evidence continuity**

Seal the pre-change chunk. The recovered path starts a new sequence-contiguous chunk on the same capture source only if the device UID is unchanged. A changed UID requires a new source boundary and remains deferred to hot switch.

- [x] **Step 5: Run all native focused tests and a real config-change smoke**

Automated native gates are GREEN. A real target-Mac sleep/wake produced generation-fenced `device_configuration_changed` recovery for the same built-in UID, a new first frame, sequence-contiguous pre/post chunks, and post-recovery ASR; the Task 7 report passes.

Automated GREEN is necessary but not sufficient. Record target-Mac evidence showing frame/chunk production after recovery.

---

## Chunk 3: Durable Model Management Experience

### Task 5: Expose Download, Install, Qualification, Cancel, And Retry

**What LifeSub will build:** A shared model-operation controller driven by persisted Catalog state. Users see exact bytes and lifecycle stages, can cancel, and can retry the same durable download after restart.

**Reference modules:**

- OpenWhispr `LocalWhisperPicker.tsx` and `useModelDownload`: progress, cancellation, deletion confirmation.
- TypeWhisper `ModelManagerService.swift`: ready/configured/restorable distinction and optional auto-unload.
- LifeSub remains stricter: downloaded is not installed; installed is not runtime-qualified.

**Files:**

- Modify: `src-tauri/src/commands.rs`
- Modify: model manager query/control owner and focused tests.
- Modify: `src/services/lifesub.ts`, `lifesub.test.ts`.
- Create: `src/modelManager.ts`, `src/modelManager.test.ts`.
- Modify: `src/components/ModelManager.tsx`.
- Create or modify: `src/components/ModelManager.test.tsx`.
- Modify: `src/settings.css`.

- [x] **Step 1: Write DTO/control tests RED**

Require:

```ts
interface CoreModelDownload {
  id: string
  state: 'queued' | 'downloading' | 'verifying' | 'installing' | 'qualifying' | 'succeeded' | 'failed' | 'cancelled'
  downloadedBytes: number
  expectedBytes: number
  artifactIndex: number
  artifactCount: number
  errorCode: string | null
  errorSummary: string | null
}
```

Exact field names may follow current DTO conventions, but the information cannot be omitted.

- [x] **Step 2: Write controller race tests RED**

Cover stale polling results, operation replacement, unmount/remount, restart restoration, cancellation acknowledgement, retry same ID, and model identity changes.

- [x] **Step 3: Add typed cancel/retry commands**

Commands call existing ModelManager cancellation/retry APIs and preserve exact bundle/download identity. They cannot synthesize success from a Promise return.

- [x] **Step 4: Implement progress UI**

Show percentage, bytes, current stage, artifact position, stable error, cancel/retry. Use a progress element or accessible progressbar and existing tokens.

- [x] **Step 5: Verify real interrupted download**

Use a controlled network interruption or test transport. Capture before-interrupt, restored checkpoint, resumed bytes, verification, and runtime-qualified screenshots.

### Task 6: Add Model Resource Controls After Current Release

**What LifeSub will build:** Optional immediate/2/5/10/30-minute model unload for memory pressure, only after instrumentation shows persistent model residency.

**Reference:** TypeWhisper `AdvancedSettingsView.swift` auto-unload model setting.

- [ ] **Step 1: Measure current residency before adding a setting**
- [ ] **Step 2: If no persistent cache exists, close this task as YAGNI**
- [ ] **Step 3: If a cache is added, write lease-aware eviction tests first**
- [ ] **Step 4: Never unload an active Job lease or misreport model readiness**

---

## Chunk 4: Packaged Local ASR Release Closure

### Task 7: Extend Native Acceptance For Probe And Recovery

**What LifeSub will build:** Upgrade the structured acceptance report to prove compatibility probe identity, first-frame readiness, same-UID recovery, and continued chunk/ASR publication.

**Files:**

- Modify: `scripts/verify-native-capture.sh`
- Modify: `tests/scripts/verify-native-capture.test.sh`
- Modify: desktop acceptance harness/report producer.

- [x] **Step 1: Write mutation-negative report tests RED**

Reject missing/mismatched probe UID, zero first-frame samples, recovery without generation, recovered UID mismatch, no post-recovery chunk, fallback device, or mixed app/helper identity.

- [x] **Step 2: Extend verifier GREEN**
- [x] **Step 3: Collect built-in and external/Bluetooth reports**
- [x] **Step 4: Collect permission denied/allow/revoke and no-device injected UI reports**
- [x] **Step 5: Collect 30-minute dual-source report with existing thresholds**

### Task 8: Run Packaged Real-Model And Installed-App Gates

**What LifeSub will build:** No new product behavior. This task proves the full requested objective using the exact release candidate.

**Reference projects:** None can substitute for this evidence. Use only LifeSub gates.

- [x] **Step 1: Re-read `docs/workspace-status.md` and audit every worktree**
- [x] **Step 2: State exact worktree, branch, commit, dirty digest, app/helper hashes**
- [x] **Step 3: Build helper and app through verified Sherpa attestation**
- [x] **Step 4: Run packaged SenseVoice and Whisper known-fixture quality gates**

The provider gate is now implemented and has passed against the final post-fix packaged candidate; the report is bound to the candidate commit, scoped source digest, and bundled gate executable identity.
- [x] **Step 5: Run packaged native capture report matrix**
- [x] **Step 6: Run literal Tier 2 and Tier 3**
- [x] **Step 7: Add Native ASR capability marker only after evidence passes**
- [x] **Step 8: Verify nested signature, entitlements, `otool -L`, helper identity, model path resolution**
- [x] **Step 9: Build DMG, verify, mount, compare contained app hashes**
- [x] **Step 10: Install normally, launch one instance, verify permission owner is LifeSub**
- [x] **Step 11: Capture final screenshots and update process/workspace status**

Run at minimum:

```bash
swift test --package-path src-tauri/native/capture-helper
npm test
npm run build
scripts/check.sh tier2
scripts/check.sh tier3
npm run verify:release-source
scripts/verify-desktop-asr.sh ...
scripts/verify-native-capture.sh --verify-report-dir ...
git diff --check
```

Do not install or replace `/Applications/LifeSub.app` until every required gate passes.

---

## Chunk 5: Later Product-Surface Settings And Workflow Improvements

This chunk is intentionally later than the recommended `0.2.2` stabilization release. Start it only after the stabilization work above is complete, or when the team explicitly decides to expand product surface area rather than continue tightening the current recording-to-Evidence path.

### Task 9: Add Context-Specific ASR Profiles

**What LifeSub will build:** Separate persisted defaults for live recording, imported audio, and explicit retranscription while preserving a single Provider validation engine.

**Reference modules:**

- OpenWhispr `MeetingSettings.tsx`, `UploadSettings.tsx`, and `transcriptionRoute.ts`.
- TypeWhisper separates Dictation, Recorder, Recovery, and File Transcription.

- [ ] **Step 1: Write profile migration and fallback tests RED**
- [ ] **Step 2: Add `live/import/retranscribe` profile identifiers**
- [ ] **Step 3: Reuse one canonical validator; do not duplicate Provider logic**
- [ ] **Step 4: Add Settings sub-tabs and explicit “inherit global” behavior**
- [ ] **Step 5: Prove each Job freezes the resolved profile snapshot**

### Task 10: Add LifeSub-Relevant General Settings

**What LifeSub will build:** Conflict-aware recording hotkeys, launch-at-login, audio cues, optional media pause/audio ducking, and update notifications. Exclude clipboard/text insertion features.

**Reference modules:**

- TypeWhisper `HotkeySettingsView.swift`, recording Sound/Audio Ducking/Media Pause sections.
- OpenWhispr General and Hotkeys settings.

- [ ] **Step 1: Write hotkey registration rollback/conflict tests RED**
- [ ] **Step 2: Add one typed hotkey registry for start/stop, pause/resume, and quick note**
- [ ] **Step 3: Add audio cue/media behavior behind explicit toggles**
- [ ] **Step 4: Verify accessibility, reduced motion, and no unexpected system-volume persistence**

### Task 11: Add Storage Visibility, Recovery, Backup, And Retention

**What LifeSub will build:** Storage usage by raw audio/derivatives/models/database, failed-session recovery, settings/metadata backup, and later an Evidence-aware retention policy.

**Reference modules:** TypeWhisper `DictationRecoveryView.swift`, `AdvancedSettingsView.swift`, `SettingsBackupExporter.swift`; OpenWhispr privacy/data retention UI.

**VoiceStudio cross-check:** `backend/services/storage_report.py` returns bounded, cached category totals with an explicit incomplete result on timeout/unreadable paths. LifeSub already displays raw-audio/model/Catalog bytes, but its recursive diagnostic walk is unbounded; measure first in S4, then make Task 11's read-only projection truthful before adding retention.

- [ ] **Step 1: Add a bounded read-only storage projection with complete/partial state and volume identity; never label an incomplete scan an exact total**
- [ ] **Step 2: Add recovery actions for known staging/seal/job states**
- [ ] **Step 3: Add versioned settings/metadata export-import**
- [ ] **Step 4: Design retention deletion graph before exposing auto-delete**
- [ ] **Step 5: Prove deletion never leaves orphan Revision/Receipt/source references**

---

## Chunk 6: Evidence-Preserving Audio And ASR Quality

This chunk is explicitly later than 0.2.2. Each task here changes the evidence model, the ASR publication model, or the runtime/performance envelope; none should be bundled into the first post-release stabilization pass.

### Task 12: Design And Implement AEC As Derivative Evidence

**What LifeSub will build:** Optional WebRTC APM echo-reduced microphone audio for ASR while raw mic and system audio remain canonical and replayable.

**Reference modules:**

- OpenWhispr `meetingAecManager.js` process supervision and framing.
- OpenWhispr `meeting-aec-helper/src/aec_processor.cc` reverse-stream/system-reference and forward-stream/mic processing.

- [ ] **Step 1: Write a dedicated AEC/Evidence design spec before code**
- [ ] **Step 2: Pin WebRTC APM source/license/build identity**
- [ ] **Step 3: Add schema migration for derivative audio parentage and algorithm receipt**
- [ ] **Step 4: Write aligned dual-stream, drift, failure, and raw-preservation tests RED**
- [ ] **Step 5: Implement a bounded native AEC boundary**
- [ ] **Step 6: Prove AEC failure leaves raw evidence and ASR retry available**
- [ ] **Step 7: Benchmark duplicate reduction and recognition quality before default enablement**

### Task 13: Add Cross-Source Transcript Deduplication

**What LifeSub will build:** A derived fusion Revision that suppresses duplicated far-end speech across microphone and system sources without deleting either original transcription.

- [ ] **Step 1: Create aligned fixture with known duplicated phrases**
- [ ] **Step 2: Write time-overlap/source/similarity decision tests RED**
- [ ] **Step 3: Persist fusion algorithm version and parent Revision IDs**
- [ ] **Step 4: Keep original source Revisions selectable and exportable**
- [ ] **Step 5: Measure false merge/split rates before automatic activation**

### Task 14: Benchmark sherpa Bounded Batch Decode

**What LifeSub will build:** A benchmark-backed decision, not an assumed optimization.

**Reference:** sherpa-onnx 1.13.5 `OfflineRecognizer::decode_multiple_streams`.

- [ ] **Step 1: Freeze sequential output/RTF/RSS/cancel baseline**
- [ ] **Step 2: Add an experimental bounded batch adapter behind a feature flag**
- [ ] **Step 3: Prove result order, text, Receipt, cancellation, and lease renewal equivalence**
- [ ] **Step 4: Adopt only if RTF improves without violating RSS/cancel thresholds**

### Task 15: Separate Ephemeral Preview From Committed Evidence

**What LifeSub will build:** Optional low-latency preview that is visibly provisional and cannot enter search/export/Agent Evidence until a persisted Revision exists.

**Reference modules:** TypeWhisper live indicator preview and OpenWhispr streaming transcription settings.

- [ ] **Step 1: Write UI/Catalog boundary tests RED**
- [ ] **Step 2: Add preview-only event namespace and session generation**
- [ ] **Step 3: Clear preview on failure/restart and reconcile with committed Revision**
- [ ] **Step 4: Prove preview never writes Segment/FTS/Receipt rows**

### Task 16: Add Dictionary Normalization As Derived Revision

**What LifeSub will build:** Provider-original Revision plus optional normalized Revision with dictionary/rule snapshot and diff provenance.

**Reference:** TypeWhisper normalization/dictionary pipeline behavior, clean-room only.

- [ ] **Step 1: Define normalization receipt and parent Revision contract**
- [ ] **Step 2: Write Chinese numbers, names, aliases, punctuation, and no-op tests RED**
- [ ] **Step 3: Never mutate provider-original text**
- [ ] **Step 4: Let users switch/export original or normalized Revision**

---

## 4. Verification And Review Gates

### Focused development

- Swift behavior: `swift test --package-path src-tauri/native/capture-helper --filter <test>`
- Rust desktop behavior: always through `scripts/with-sherpa-runtime.sh cargo ...`
- Frontend behavior: `npx vitest run <test files>`
- Shell contracts: run the exact `tests/scripts/*.test.sh`

### Per-chunk Tier 2

```bash
npm test
npm run build
swift test --package-path src-tauri/native/capture-helper
scripts/check.sh tier2
git diff --check
```

### Task completion Tier 3

```bash
scripts/check.sh tier3
```

Add Playwright screenshots for Settings changes, target-Mac hardware reports for native audio, real-model quality reports for ASR, and signed bundle/DMG evidence for release.

### Combined review

After each chunk, one reviewer covers spec compliance, code quality, security, and verification fitness. Release gate is Critical = 0. Important findings are fixed in the same chunk or explicitly deferred with evidence according to `docs/testing-and-review-policy.md`.

## 5. Recommended Execution Order

```text
Current release:
Task R1-R4 audit/classification
  -> freeze the completed-capability optimization boundary
  -> Task 1 settings truth
  -> Task 3 exact-UID first-frame probe
  -> Task 4 same-UID engine recovery
  -> Task 2 diagnostics projection over the final contracts
  -> Task 5 durable model operations
  -> Task 7 acceptance extension and hardware evidence
  -> Task 8 packaged/installed release closure
  -> apply only release-blocking fixes exposed by packaged evidence

Immediate follow-on (0.2.2, lightweight):
Stabilization S1 -> S2 -> S3 -> S4
  -> reuse completed contracts; do not reopen Provider/capture architecture
  -> stop when the surface is clearer and the baseline report exists

Post-release settings and workflow:
Task 9 -> Task 10 -> Task 11, each re-scoped through a dedicated design and not assumed to be the immediate next version

Post-release quality:
Task 12 -> Task 13
Task 14 independently after profiling fixtures exist
Task 15 and Task 16 after committed-only release behavior is stable
```

VoiceStudio risk gate after S4: classify actual long-meeting failures; investigate ASR process containment if native crashes/timeouts appear, read-only signal checks if source-audio defects dominate, and bounded storage reporting via Task 11 if scan/space pressure appears. These are conditional studies, not precommitted 0.3 features.

Task 6 is conditional and may be closed as YAGNI. Tasks 12-16 must never be pulled into the current release gate merely because reference projects already implement similar features.

## 6. Final Completion Criteria

The current release portion is complete only when:

- completed capture, ASR, model, Evidence, and UI paths have been reference-audited, with every accepted gap owned and verified and every deferred item explicitly non-blocking;
- every visible setting reflects production behavior;
- the selected microphone is exact, compatible, and proven to deliver frames;
- same-UID configuration changes recover or fail explicitly without fallback;
- model operations expose durable progress/cancel/retry/qualification;
- packaged SenseVoice and Whisper produce persisted Receipt/Revision/Segment evidence;
- native capture report matrix, Tier 2, Tier 3, signature, DMG, installation, and screenshots all pass;
- `/Applications/LifeSub.app` contains the exact verified candidate.
- no post-release optimization is silently included in the 0.2.1 release claim.

Post-release tasks have their own completion gates and do not redefine the current release as incomplete after Task 8 passes.
