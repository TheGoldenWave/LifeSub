# LifeSub Microphone Device Selection Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add exact, persisted macOS microphone selection to LifeSub and fail closed when the chosen input is unavailable or changes unexpectedly.

**Architecture:** The authenticated Swift helper enumerates Core Audio input devices and configures `AVAudioEngine` to the exact selected UID. Rust carries the typed device contract between Tauri and the helper, validates source identity, and React renders one shared persisted selector state on recording and settings surfaces.

**Tech Stack:** Swift 6, CoreAudio, AVFoundation/AVAudioEngine, Rust 2024, Tauri 2, React 19, Vitest.

**Specification:** `docs/superpowers/specs/2026-08-25-lifesub-microphone-device-selection-design.md`

---

## Chunk 1: Native Device Contract

### Task 1: Add Versioned Device DTOs And Enumeration

**Files:**
- Create: `src-tauri/native/capture-helper/Sources/CaptureHelper/AudioInputDevices.swift`
- Modify: `src-tauri/native/capture-helper/Sources/CaptureProtocol/CaptureProtocol.swift`
- Modify: `src-tauri/native/capture-helper/Sources/CaptureHelper/CommandLoop.swift`
- Modify: `src-tauri/native/capture-helper/Tests/CaptureProtocolTests/CaptureProtocolTests.swift`
- Create: `src-tauri/native/capture-helper/Tests/CaptureHelperTests/AudioInputDevicesTests.swift`
- Modify: `src-tauri/native/capture-helper/Tests/CaptureHelperTests/CommandLoopTests.swift`
- Modify: `src-tauri/src/capture/protocol.rs`
- Modify: `src-tauri/src/capture_protocol_test.rs`

- [ ] **Step 1: Write failing Swift and Rust protocol tests**

Add V2 fixtures for `list_microphones`, bounded `microphone_devices`, capability advertisement, transport decoding, exact start UID validation, duplicate UID rejection, unknown fields, and V1/V2 skew rejection.

- [ ] **Step 2: Run focused protocol tests and verify RED**

Run:

```bash
swift test --package-path src-tauri/native/capture-helper --filter CaptureProtocolTests
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features capture_protocol_test
```

Expected: FAIL because the new commands and DTOs do not exist.

- [ ] **Step 3: Implement Core Audio enumeration behind a testable adapter**

Use `AudioObjectGetPropertyData` for device IDs, UIDs, names, alive state, transports, default input, and input-stream capacity. Enforce protocol limits before encoding. Inject permission, engine, staging, and capture side effects behind spies; prove discovery invokes none of them and maps enumeration failure to a sanitized stable error.

- [ ] **Step 4: Run focused tests and verify GREEN**

- [ ] **Step 5: Record protocol decisions in feature notes**

## Chunk 2: Exact Selected Device Capture

### Task 2: Configure AVAudioEngine By Stable UID

**Files:**
- Modify: `src-tauri/native/capture-helper/Sources/CaptureHelper/MicrophoneCapture.swift`
- Modify: `src-tauri/native/capture-helper/Sources/CaptureHelper/CaptureSessionController.swift`
- Modify: `src-tauri/native/capture-helper/Sources/CaptureHelper/CommandLoop.swift`
- Modify: `src-tauri/native/capture-helper/Tests/CaptureHelperTests/CaptureSessionControllerTests.swift`
- Modify: `src-tauri/native/capture-helper/Tests/CaptureHelperTests/CommandLoopTests.swift`

- [ ] **Step 1: Write failing exact-device and no-fallback tests**

Prove the selected UID is applied and read back before tap installation, an unknown UID fails start, benign same-UID configuration change remains recoverable, unplug/default-device change cannot switch the active source, disappearance emits `selected_microphone_disconnected` once, stop-vs-callback is serialized, and every listener is removed.

- [ ] **Step 2: Run focused Swift tests and verify RED**

- [ ] **Step 3: Implement exact Core Audio device binding**

Resolve UID to `AudioDeviceID`, set `kAudioOutputUnitProperty_CurrentDevice` on the input audio unit, verify the active device, register Core Audio and engine configuration listeners, then install the tap and start the engine. On every callback re-resolve the requested UID and verify the active device; remove listeners during every stop/error/deinit path.

- [ ] **Step 4: Run focused Swift tests and verify GREEN**

### Task 3: Carry And Verify The UID Through Rust Capture

**Files:**
- Modify: `src-tauri/src/capture/native_transport.rs`
- Modify: `src-tauri/src/capture/coordinator.rs`
- Modify: `src-tauri/src/capture/session.rs`
- Modify: `src-tauri/src/capture_protocol_test.rs`
- Modify: `src-tauri/src/capture_coordinator_test.rs`
- Modify: `src-tauri/src/capture_session_test.rs`
- Modify: `src-tauri/src/commands.rs`

- [ ] **Step 1: Write failing mode/UID and source identity tests**

Cover required UID for `smart`/`mic-only`, forbidden UID for `system-only`, unavailable-device error mapping, and rejection of `source_started.device_id` mismatch before Catalog registration.

- [ ] **Step 2: Run focused Rust tests and verify RED**

- [ ] **Step 3: Implement typed propagation and fail-closed checks**

- [ ] **Step 4: Run focused Rust tests and verify GREEN**

### Task 4: Implement Authenticated One-Shot Device Discovery

**Files:**
- Modify: `src-tauri/src/capture/helper.rs`
- Modify: `src-tauri/src/capture/native_transport.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/capture_protocol_test.rs`
- Modify: `src-tauri/src/capture_session_test.rs`

- [ ] **Step 1: Write failing lifecycle and authentication tests**

Cover valid one-response shutdown, nonce/peer/helper identity failure, timeout, cancellation, malformed/duplicate/extra response, helper exit, missing `shutdown_ack`, and guaranteed child reap.

- [ ] **Step 2: Run focused Rust tests and verify RED**

- [ ] **Step 3: Reuse the authenticated launcher for bounded discovery**

Send `list_microphones`, accept exactly one V2 response before the deadline, send `shutdown`, require ACK, and reap on all paths. Expose one typed Tauri command; do not create a parallel unauthenticated helper path.

- [ ] **Step 4: Run focused Rust tests and verify GREEN**

## Chunk 3: Persistence And UI

### Task 5: Persist UID And Last Known Name

**Files:**
- Modify: `src-tauri/src/domain.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/service_test.rs`
- Modify: `src/services/lifesub.ts`
- Modify: `src/services/lifesub.test.ts`
- Modify: `src/data/adapter.ts`

- [ ] **Step 1: Write failing backward-compatibility and persistence tests**

Prove old recording-config JSON loads with null fields; `(null,null)` and two non-empty strings round-trip; malformed partial/empty pairs fail; and refresh/device loss never changes the saved pair without an explicit user action.

- [ ] **Step 2: Run focused tests and verify RED**

- [ ] **Step 3: Add optional typed fields and microphone-list Tauri service**

- [ ] **Step 4: Run focused tests and verify GREEN**

### Task 6: Add Recording-Surface And Settings Selectors

**Files:**
- Modify: `src/components/LiveCapture.tsx`
- Modify: `src/components/RecordingSettings.tsx`
- Modify: `src/components/live_capture.test.tsx`
- Modify: `src/components/SettingsModal.test.tsx`
- Create: `src/hooks/useMicrophoneDevices.ts`
- Create: `src/hooks/useMicrophoneDevices.test.tsx`
- Modify: `src/styles.css`
- Modify: `src/settings.css`

- [ ] **Step 1: Write failing UI state tests**

Cover loading, ready, explicit unselected, disconnected saved device, empty, refresh error, lifecycle lock, microphone-mode start blocking, `system-only` bypass, Settings-to-Live and Live-to-Settings synchronization, stale refresh rejection, and start using the displayed persisted UID rather than a stale closure.

- [ ] **Step 2: Run focused Vitest tests and verify RED**

- [ ] **Step 3: Implement the shared selector behavior**

Implement one shared controller for enumeration, selection, persistence, refresh generation, and lifecycle lock. Use the native select control, a Lucide refresh icon with tooltip/accessible name, existing tokens, stable control dimensions, and the named stable error copy. Save explicit selection through the existing recording config API.

- [ ] **Step 4: Run focused tests and verify GREEN**

- [ ] **Step 5: Run production frontend build and capture desktop screenshot**

## Chunk 4: Integration And Release Continuation

### Task 7: Integrate With Task 11 And Resume ASR Release Closure

**Files:**
- Modify: `scripts/verify-native-capture.sh`
- Modify: `tests/scripts/verify-native-capture.test.sh`
- Modify: `docs/prd/lifesub-real-asr-v0.2/PRD.md`
- Modify: `docs/prd/lifesub-real-asr-v0.2/.artifacts/process.md`
- Modify: `docs/prd/lifesub-real-asr-v0.2/.artifacts/notes.md`
- Modify: `docs/workspace-status.md`

- [ ] **Step 1: Write failing acceptance-contract tests**

Require enumerated UID/name, displayed selection, persisted pair before/after restart, authenticated start UID, helper-resolved/read-back UID, `source_started` UID, Catalog device identity, stable errors, and zero microphone publication on failure. Cover built-in, external/Bluetooth, pre-start disconnect, active disconnect, restart persistence, injected no-device UI, and `system-only` with no microphone. Add negative mutations for every identity binding and reject all fallback evidence.

- [ ] **Step 2: Extend the verifier and run shell tests GREEN**

- [ ] **Step 3: Run Swift, frontend, Rust focused suites and Tier 2**

Run:

```bash
swift test --package-path src-tauri/native/capture-helper
npm test -- --run
npm run build
scripts/check.sh tier2
git diff --check
```

- [ ] **Step 4: Run the combined current-task review**

Gate: Critical = 0. Important issues are fixed or explicitly evidenced per project policy.

- [ ] **Step 5: Require the augmented Task 11 matrix before Task 12**

The new device scenarios augment the existing 12 reports and are mandatory before packaging/install.

- [ ] **Step 6: Continue the existing V0.2.1 Task 11/12 sequence**

Build the verified-runtime release app, run real model and packaged capture acceptance, verify signatures/dependencies, produce the DMG, install only after every release gate passes, and update final evidence.

### Deferred Todo: Active-Recording Microphone Hot Switch

- [ ] Seal the current source chunk and create a new capture-source/Evidence boundary before changing devices.
- [ ] Preserve source identity and continuous timing across the switch.
- [ ] Add dedicated protocol, persistence, UI, and hardware acceptance coverage in a later milestone.
