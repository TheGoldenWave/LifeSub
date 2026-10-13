# LifeSub Microphone Device Selection Design

**Status:** Approved for implementation

**Date:** 2026-08-25

**Parent specification:** `docs/superpowers/specs/2026-08-21-lifesub-v0.2.1-integrated-native-capture-design.md`

## 1. Goal

Let a user explicitly choose the macOS microphone used by LifeSub before recording. The choice must survive restart, remain truthful when a device disconnects, and flow unchanged from the UI through the Tauri command and authenticated helper protocol into the actual Core Audio device used by `AVAudioEngine`.

This work does not add hot switching while a recording is active.

## 2. User Contract

- The recording surface shows a microphone selector near the capture controls whenever the selected capture mode includes microphone input.
- The selector lists usable built-in, external, USB, and Bluetooth input devices by display name.
- A refresh icon re-enumerates devices and has an accessible name and tooltip.
- The selector is disabled while capture is starting, recording, paused, stopping, or otherwise lifecycle-busy.
- The selected device is persisted by stable Core Audio UID together with its last known display name.
- If the saved UID is absent, LifeSub retains the saved name and renders it as disconnected. It does not select the system default or another device.
- Microphone-required modes cannot start without an explicitly selected, currently available device.
- `system-only` remains usable when no microphone exists.
- If the selected device disappears during recording, microphone capture fails with a visible, stable device-disconnected error. It never switches to another input.
- If no usable microphone is detected, the selector area shows an explicit empty/error state and a refresh action.

## 3. Device Identity And Enumeration

The Swift helper owns macOS audio-device discovery because it already owns the capture APIs and signed permission boundary. It enumerates Core Audio devices with input channels and returns:

```text
uid: stable Core Audio device UID
name: user-visible device name
is_default: whether macOS currently marks it as default input
is_connected: true for an enumerated usable device
transport: built_in | usb | bluetooth | aggregate | virtual | unknown
```

Enumeration does not request microphone permission, allocate staging, create an engine/tap, or start capture. The helper rejects devices that are not alive or have no usable input streams. Aggregate and virtual inputs remain selectable because they are valid Core Audio sources; per-device property failures skip that untrusted entry, while a failure to enumerate device identity fails the whole request with a sanitized stable error. Duplicate UIDs make the response invalid. Results are sorted with the current default first, then by localized display name, while selection remains UID-based.

The Tauri query command launches the same authenticated packaged helper used for capture, sends a versioned `list_microphones` command, receives exactly one bounded `microphone_devices` response before a deadline, sends `shutdown`, requires `shutdown_ack`, and reaps the child on every success, error, timeout, cancellation, malformed/duplicate/extra response, or early-exit path. The existing nonce, peer UID/PID, executable identity, and signature checks remain mandatory. No second, unauthenticated discovery executable or direct frontend native bridge is introduced.

This change advances the capture protocol from V1 to V2. V2 `hello` advertises device discovery and exact microphone selection. Old-helper/new-app and new-helper/old-app pairs fail closed with a stable protocol-version error; there is no schema downgrade.

## 4. Persistence

`RecordingConfig` gains backward-compatible optional fields:

```text
microphone_device_uid: string | null
microphone_device_name: string | null
```

Existing stored JSON without these fields continues to load. The only valid persisted states are `(null, null)` or two non-empty strings. UID-only, name-only, and empty-string pairs are rejected. Saving a selection stores both fields atomically in the existing recording configuration setting. Refresh or device loss never rewrites the pair. Only an explicit user clear action may store `(null, null)`.

On first use with no saved UID, the UI may present the current system default as a recommended option, but recording remains blocked until the user explicitly selects and saves a microphone. This preserves the fail-closed source-identity contract.

## 5. Capture Protocol And Runtime Flow

Capture protocol V2 adds:

- command `list_microphones`;
- response `microphone_devices` with a bounded array of device descriptors;
- optional `microphone_device_uid` on `start`, required whenever `requested_sources` contains `microphone` and forbidden when it does not;
- explicit V2 capability advertisement in `hello` and deterministic V1/V2 skew rejection.

Start flow:

1. React loads the recording configuration and current device list.
2. React blocks microphone-required start when the saved UID is missing or unavailable.
3. `start_streaming_capture` receives the selected UID.
4. Rust validates the mode/UID relationship and includes the UID in the authenticated helper command.
5. Swift resolves that exact Core Audio UID, sets it on the audio input unit before installing the tap, and starts `AVAudioEngine`.
6. `source_started.device_id` must equal the requested UID. Rust fails closed on mismatch before registering capture provenance.

Stable errors cross Swift, Rust, and UI without being replaced by generic text:

- `microphone_selection_required`: microphone mode has no explicit saved UID/name pair;
- `selected_microphone_unavailable`: the saved UID is absent or not usable before start;
- `selected_microphone_identity_mismatch`: the active/source-started device differs from the requested UID;
- `selected_microphone_disconnected`: the selected UID disappears or stops being active during recording.

The helper must not use `AVCaptureDevice.default(for: .audio)` as the final capture choice. It may use AVFoundation only for authorization state; Core Audio provides enumeration and exact device selection.

## 6. Interruption And Disconnect Semantics

- The microphone adapter registers Core Audio device-list/alive/current-device property listeners plus `AVAudioEngineConfigurationChange` before start, serializes callbacks with lifecycle stop, and removes every listener during stop/deinit.
- Every configuration callback re-resolves the requested UID and reads back the input audio unit's current device. A generic, recoverable `device_configuration_changed` event may keep capture running only when the UID remains alive and the audio unit still uses it.
- If the requested UID is no longer present or active, the adapter seals/stops once and emits a non-recoverable interruption with stable reason `selected_microphone_disconnected`. Stop/callback races must not emit duplicate failure or access a released engine.
- Rust transitions the capture session to its existing stable device failure path and the UI renders a clear message naming the disconnected microphone when available.
- Reconnecting the device does not resume the failed recording automatically. The user starts a new recording after refreshing device state.

For an explicitly selected microphone, this rule supersedes the parent specification's bounded automatic device recovery. Benign same-UID reconfiguration remains recoverable; selected-device loss is not.

## 7. UI States

The recording surface supports these states without layout shifts:

- `loading`: selector disabled, device discovery in progress;
- `ready`: selected connected device shown;
- `unselected`: devices exist, explicit selection required;
- `disconnected`: saved name retained with a disconnected label;
- `empty`: no usable microphones detected;
- `error`: enumeration failed, with retry;
- `locked`: current selection displayed but disabled during capture lifecycle.

Settings exposes the same persisted choice as a secondary configuration surface. A shared microphone-device controller owns enumeration, selection, persistence, refresh generation, and lifecycle lock state. Both surfaces subscribe to that controller; neither maintains an independent preference. Late enumeration responses are discarded by generation so they cannot overwrite a newer refresh or selection. Start reads the controller's displayed persisted UID, not a stale closure.

## 8. Deferred Hot Switching Todo

Hot switching during an active recording is explicitly deferred. A future implementation must:

- seal the current microphone chunk before switching;
- create a new capture-source/Evidence boundary for the new device within the same record;
- preserve both device identities and timing continuity;
- never rewrite earlier source provenance.

This Todo belongs in the V0.2.1 feature notes/PRD but is not part of the release gate for the first selector version.

## 9. Verification

RED/GREEN automated coverage must prove:

- Swift device enumeration filters output-only devices and preserves stable UID/name/transport metadata;
- protocol codecs reject missing/mismatched start UID fields, duplicate UIDs, oversized device lists, and unknown fields;
- exact selected UID is configured before the microphone tap starts;
- unavailable or mismatched selected UIDs fail closed without default fallback;
- recording configuration remains backward-compatible and persists UID/name;
- frontend renders loading, empty, disconnected, error, ready, and locked states;
- microphone modes block start without an available explicit selection while `system-only` remains enabled;
- selected-device disconnect becomes a visible non-recoverable failure;
- `source_started.device_id` mismatch is rejected before Catalog source registration.

Target-Mac acceptance must cover built-in, one external or Bluetooth microphone, selection across restart, disconnect before start, disconnect during recording, and no-device UI. These checks supplement rather than replace the existing Task 11 packaged capture matrix.

Each acceptance report records and binds:

- enumerated UID/name/availability and the explicit UI selection;
- persisted UID/name before and after restart;
- UID sent in the authenticated V2 start command;
- helper-resolved UID/name and read-back active AudioUnit device;
- `source_started` UID/name and Catalog capture-source device identity;
- stable failure code and zero microphone source/chunk/publication rows on unavailable or mismatch paths.

The verifier mutates each binding negatively and must reject fallback, missing identity, or inconsistent names/UIDs. The matrix adds built-in selection, external/Bluetooth selection, pre-start disconnect, active disconnect, restart persistence, and `system-only` with no microphone to the existing 12 Task 11 reports. It must pass before Task 12 packaging/install. The no-device visual state uses an injected enumeration fixture because normal Macs retain a built-in input; real unplug evidence remains a separate hardware scenario.
