# LifeSub Remaining Capabilities Roadmap Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver the remaining production audio, ASR, model-management, diarization, and speaker-identification capabilities as four independently releasable versions.

**Architecture:** First complete a trustworthy imported-audio-to-Evidence path, then connect macOS native capture to that proven pipeline. Add anonymous diarization only after capture and ASR timestamps are stable, and add CAM++ identity matching only after anonymous speaker revisions are independently useful. Every version preserves fail-closed behavior, immutable audio, append-only revisions, provenance, and explicit failure states.

**Tech Stack:** Tauri 2, Rust, React 19, TypeScript, SQLite/FTS5, sherpa-onnx, ScreenCaptureKit, AVAudioEngine, CAM++ embedding, Vitest, Playwright.

---

## 1. Release Sequence

| Version | User outcome | Included capabilities | Release dependency |
|---|---|---|---|
| V0.2.1 | Imported audio produces real, traceable local transcripts | Native ASR executor, SenseVoice/Whisper, model installation UI, Job/Receipt/Revision publication | Existing Evidence Core; implement and verify approved V0.2 Tasks 1-15 |
| V0.2.2 | Microphone and system audio can be recorded and transcribed | AVAudioEngine, ScreenCaptureKit, permissions, dual-source chunks, long-recording recovery | V0.2.1 production ASR path |
| V0.3 | Users can see which anonymous speaker spoke each segment | Diarization, speaker turns, ASR alignment, manual correction | Stable capture timestamps and ASR segments |
| V0.3.1 | Authorized local speaker profiles can be matched | CAM++ embeddings, enrollment, thresholding, revocation and deletion | V0.3 anonymous speaker revisions |

The versions are sequential release gates. Research and fixture preparation may overlap, but a later capability must not be used to declare an earlier version complete.

## 2. Shared Delivery Guardrails

- Original audio is persisted before downstream processing and is never modified by ASR or speaker processing.
- Provider, model, runtime, or speaker identity must never silently fall back.
- Successful outputs are append-only revisions with input hashes and runtime provenance.
- Failed or cancelled work must not publish partial Evidence.
- Browser fixtures and mocks prove UI mapping only; they cannot satisfy native audio, real-model, permissions, packaging, or biometric-quality gates.
- Behavior, data, security, and contract changes follow RED/GREEN. Native permissions, real models, packaging, long-running capture, and speaker metrics require target-environment evidence under `docs/testing-and-review-policy.md`.

## 3. Release Validation Matrix

Each child PRD must copy and refine its rows before implementation. Thresholds and fixtures marked for freezing cannot be lowered after observing a release failure without an explicit decision record.

| Version / acceptance class | Change type | Risk | Validation level | Pass standard | Required environment / fixture | RED/GREEN | Evidence |
|---|---|---|---|---|---|---|---|
| V0.2.1 Job, Receipt, Revision | Behavior, data, contract | Critical | Rust integration and fault injection | Deterministic states; atomic success; no partial Evidence on failure | Versioned v1 Catalog, corrupted audio, cancellation and restart fixtures | Required | Focused test output and Tier 2 log in feature process artifact |
| V0.2.1 real models | Runtime and quality | Important | Real-model integration and quantitative Gate | Meet PRD CER/WER, phrase recall and timestamp thresholds | macOS 14+ Apple Silicon; pinned Chinese, English and mixed fixtures and model hashes | Required for adapters; quantitative Gate also required | `scripts/verify-asr-gate.sh` output and fixture manifest digest |
| V0.2.1 model installation | Behavior, data, supply chain | Critical | Integration, reconciliation and negative security cases | Damaged or incomplete installs never execute; retry/delete recover deterministically | Isolated model directory; interrupted transfer, low-space and hash-mismatch fixtures | Required | Model-manager tests, installation manifest and diagnostic screenshots |
| V0.2.1 desktop package | Configuration and real environment | Critical | Packaged-app smoke | DMG app signature passes and both providers execute without missing native libraries | Signed release DMG on macOS 14+ Apple Silicon | Build/smoke evidence; no synthetic RED | DMG checksum, signature, `otool` and packaged smoke logs |
| V0.2.2 capture contract | Behavior, data, permissions | Critical | Adapter integration, fault injection and real-device acceptance | No false-ready state; source identity/timestamps persist; sealed chunks survive failure | Real microphone/ScreenCaptureKit source; denied permission, device loss, restart and low-disk cases | Required | Focused tests, permission screenshots and provenance report |
| V0.2.2 durability | Performance and resource | Critical | Soak and recovery | Child PRD freezes duration, loss, corruption and recovery thresholds before implementation | macOS 14+ Apple Silicon, signed app, controlled disk/restart harness | Required for recovery; quantitative soak also required | Soak report and recovered Chunk manifest |
| V0.3 anonymous speakers | Model quality, data and UI behavior | Important | Benchmark, integration and packaged acceptance | Frozen DER/JER thresholds pass; corrections append revisions; ASR survives diarization failure | Frozen multi-speaker, overlap and short-turn fixtures | Required for revisions/alignment; benchmark also required | Benchmark report, alignment tests and packaged screenshots |
| V0.3.1 CAM++ identity | Biometric security, privacy and quality | Critical | Consent/security tests, benchmark and real-app acceptance | No unauthorized profile; unknown speakers reject; deletion removes named projection only | Frozen enrollment, known/unknown corpus and encrypted local store | Required | Consent/revocation tests, FAR/accuracy/rejection report and deletion audit |

## Chunk 1: V0.2.1 Real Local ASR

### Task 1: Use the approved V0.2 plan as the sole V0.2.1 task graph

**Files:**
- Modify: `docs/superpowers/plans/2026-08-15-lifesub-real-asr-v0.2.md`
- Modify: `docs/prd/lifesub-real-asr-v0.2/PRD.md`
- Modify: `docs/prd/lifesub-real-asr-v0.2/.artifacts/process.md`

- [ ] Execute `docs/superpowers/plans/2026-08-15-lifesub-real-asr-v0.2.md` Tasks 1-15 as the only file-level implementation plan for V0.2.1; this roadmap does not duplicate them.
- [ ] Keep native microphone/system capture and all speaker capabilities outside the V0.2.1 Gate.
- [ ] Before resuming, reconcile the reported feature-worktree status against the active worktree and update task evidence; do not infer completion from designs or backend contracts alone.
- [ ] Preserve the existing real-model, atomic publication, recovery and packaged-app gates.

## Chunk 2: V0.2.2 Native macOS Capture

### Task 2: Write and review the native capture child specification

**Files:**
- Create: `docs/prd/lifesub-native-capture-v0.2.2/PRD.md`
- Create: `docs/prd/lifesub-native-capture-v0.2.2/.artifacts/process.md`
- Create: `docs/prd/lifesub-native-capture-v0.2.2/.artifacts/notes.md`
- Create: `docs/superpowers/specs/2026-08-20-lifesub-native-capture-design.md`
- Create: `docs/superpowers/plans/2026-08-20-lifesub-native-capture-v0.2.2.md`

- [ ] Define source identity, monotonic sample time, UTC anchor, discontinuities and immutable chunk sealing.
- [ ] Define permission denial, device loss, source removal, backpressure, disk-full, pause/resume and crash recovery.
- [ ] Define Opus 16 kHz mono, 16 kbps VBR with DTX as the single capture profile.
- [ ] Copy and refine the V0.2.2 validation rows with frozen thresholds, commands, fixtures and artifact paths.
- [ ] Complete specification and plan review before changing production capture code.

### Task 3: Implement and validate real capture

**Expected ownership (final paths must be frozen in the child plan):**
- Rust capture adapters and coordinator under `src-tauri/src/capture/`
- Capture behavior tests near the Rust module
- Recorder state and diagnostics in `src/components/RecorderBar.tsx` and `src/App.tsx`

- [ ] Implement AVAudioEngine microphone and ScreenCaptureKit system-audio adapters.
- [ ] Persist each source as independently traceable Physical Audio Chunks.
- [ ] Feed sealed chunks into the proven V0.2.1 ASR Job path.
- [ ] Remove production mock-source construction while keeping test fixtures isolated.
- [ ] Validate source modes, permission denial, device loss, restart and low-disk scenarios on the target Mac.
- [ ] Pass the frozen soak and packaged-app Gate before setting `v0.2.2-native-capture-complete`.

## Chunk 3: V0.3 Anonymous Speaker Diarization

### Task 4: Write and review the diarization child specification

**Files:**
- Create: `docs/prd/lifesub-diarization-v0.3/PRD.md`
- Create: `docs/prd/lifesub-diarization-v0.3/.artifacts/process.md`
- Create: `docs/prd/lifesub-diarization-v0.3/.artifacts/notes.md`
- Create: `docs/superpowers/specs/2026-08-20-lifesub-diarization-design.md`
- Create: `docs/superpowers/plans/2026-08-20-lifesub-diarization-v0.3.md`

- [ ] Freeze the diarization model/runtime and license evidence.
- [ ] Define anonymous IDs, turns, overlap, confidence, provenance and append-only corrections.
- [ ] Define deterministic alignment between speaker turns and ASR Segments.
- [ ] Freeze benchmark fixtures and DER/JER thresholds.
- [ ] Copy and refine the V0.3 validation row with exact benchmark commands and artifacts.

### Task 5: Implement and validate diarization

**Expected ownership (final paths must be frozen in the child plan):**
- Speaker runtime, alignment and service under `src-tauri/src/speaker/`
- Speaker model and alignment tests near the Rust module
- Speaker timeline/editor components under `src/components/speaker/`

- [ ] Run diarization as a separate recoverable job over immutable audio.
- [ ] Publish assignments as a separate append-only revision with model/input provenance.
- [ ] Add manual corrections without modifying raw model results.
- [ ] Include anonymous speakers in search, Evidence resolution and Markdown projection.
- [ ] Pass DER/JER and packaged-app Gates before setting `v0.3-diarization-complete`.

## Chunk 4: V0.3.1 CAM++ Speaker Identification

### Task 6: Write and review the CAM++ child specification

**Files:**
- Create: `docs/prd/lifesub-campp-speaker-v0.3.1/PRD.md`
- Create: `docs/prd/lifesub-campp-speaker-v0.3.1/.artifacts/process.md`
- Create: `docs/prd/lifesub-campp-speaker-v0.3.1/.artifacts/notes.md`
- Create: `docs/superpowers/specs/2026-08-20-lifesub-campp-speaker-design.md`
- Create: `docs/superpowers/plans/2026-08-20-lifesub-campp-speaker-v0.3.1.md`

- [ ] Define consent, enrollment quality, encrypted storage, retention, deletion and revocation.
- [ ] Define CAM++ model identity, embedding versions, thresholds, rejection and unknown-speaker behavior.
- [ ] Freeze known-speaker accuracy, unknown false-accept and rejection thresholds.
- [ ] Keep Speaker Profile authority limited to voice matching.
- [ ] Copy and refine the V0.3.1 validation row with exact security, benchmark and deletion evidence.

### Task 7: Implement and validate CAM++ matching

**Expected ownership (final paths must be frozen in the child plan):**
- Embedding, profile and matching modules under `src-tauri/src/speaker/`
- Profile/security tests near the Rust module
- Enrollment and profile management under `src/components/speaker/`

- [ ] Implement multi-sample enrollment with input-quality rejection.
- [ ] Match anonymous clusters to authorized profiles with explicit unknown results below threshold.
- [ ] Store model version, threshold, score and source diarization revision.
- [ ] Ensure deletion stops future/named projection without deleting anonymous historical Evidence.
- [ ] Pass quality, consent, revocation and packaged-app Gates before setting `v0.3.1-campp-complete`.

## 4. Work Explicitly Deferred

- Qwen3-ASR 0.6B/1.7B and a second ASR runtime.
- Cloud ASR providers.
- Public daemon/IPC and complete Agent API hardening.
- Automatic meeting detection and always-on recording.
- Multi-device calibration and merged revisions; the historical V0.3 plan must be re-versioned before implementation.
- Semantic summaries, long-term memory and knowledge-governance features outside LifeSub's Evidence authority.

## 5. Release Completion Rule

A version is complete only when its PRD acceptance items have authoritative evidence, Tier 2 passes before review, Critical findings equal zero, and the target-environment Gate passes. Backend infrastructure, disabled UI, or browser fixtures alone do not advance the release stage.
