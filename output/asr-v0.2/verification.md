# LifeSub V0.2 Real Local ASR — Verification Evidence

> Generated: 2026-08-20
> Feature: `lifesub-real-asr-v0.2`
> Plan: `docs/superpowers/plans/2026-08-15-lifesub-real-asr-v0.2.md`

## 1. Test Suite Results

### Rust Core Tests

```bash
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features
# Expected: all PASS

SHERPA_ONNX_ARCHIVE_DIR="$(scripts/fetch-sherpa-runtime.sh)" \
  cargo test --manifest-path src-tauri/Cargo.toml --features asr-runtime
# Expected: all PASS

SHERPA_ONNX_ARCHIVE_DIR="$(scripts/fetch-sherpa-runtime.sh)" \
  cargo test --manifest-path src-tauri/Cargo.toml --features desktop commands_test
# Expected: all PASS

SHERPA_ONNX_ARCHIVE_DIR="$(scripts/fetch-sherpa-runtime.sh)" \
  cargo check --manifest-path src-tauri/Cargo.toml --features desktop
# Expected: PASS
```

### Frontend Tests

```bash
npm test
# Expected: all PASS

npm run build
# Expected: PASS
```

### Playwright E2E

```bash
npm run test:e2e
# Expected: all PASS
```

## 2. Real Model Gate

```bash
LIFESUB_ASR_MODEL_DIR="$HOME/Library/Application Support/com.goldenwave.lifesub/models" \
  scripts/verify-asr-gate.sh
```

### Metrics

| Metric | Threshold | Actual |
|--------|-----------|--------|
| SenseVoice CER | ≤ 20% | (requires real model) |
| Whisper WER | ≤ 20% | (requires real model) |
| Mixed-language phrase recall | 100% | (requires real model) |
| Median boundary error | ≤ 500 ms | (requires real model) |
| Max boundary error | ≤ 1.5 s | (requires real model) |

## 3. Desktop Acceptance Harness

```bash
LIFESUB_ASR_MODEL_DIR="$HOME/Library/Application Support/com.goldenwave.lifesub/models" \
  scripts/verify-desktop-asr.sh target
```

### Scenarios

| Scenario | Threshold | Result |
|----------|-----------|--------|
| real-asr-heartbeat | P95 ≤ 250 ms | (requires real ASR job) |
| cancel-real-asr | ack ≤ 500 ms, done ≤ 30 s | (requires real ASR job) |
| claim-and-abort | persist claim, terminate | (runs without models) |
| verify-recovery | recover ≤ 5 s | (runs without models) |
| packaged-smoke | both providers from binary | (requires real model) |

## 4. DMG Build Verification

```bash
SHERPA_ONNX_ARCHIVE_DIR="$(scripts/fetch-sherpa-runtime.sh)" \
  npm run tauri -- build --features desktop

otool -L src-tauri/target/release/bundle/macos/LifeSub.app/Contents/MacOS/lifesub
# Expected: no unresolved sherpa-onnx/onnxruntime dylibs

codesign --verify --deep --strict --verbose=2 \
  src-tauri/target/release/bundle/macos/LifeSub.app
# Expected: valid signature

scripts/verify-desktop-asr.sh dmg
```

## 5. Console Log Verification

```bash
grep -RIn "console\.log" src tests --include='*.ts' --include='*.tsx'
# Expected: no matches
```

## 6. Responsive Layout

### Desktop (1440×900)

Settings page renders with provider selector, model cards, language menu,
thread stepper, VAD toggle, auto-transcribe toggle, and ITN/task controls
without text overflow or element overlap.

### Mobile (375×812)

Settings page switches to single-column layout. Buttons remain touchable
(≥ 44 px height). No horizontal scroll.

### Tablet (768×1024)

Settings page uses compact sidebar with settings content scrollable.

## 7. Model Artifact Verification

| Model | Size | SHA-256 |
|-------|------|---------|
| SenseVoiceSmall INT8 | 163,002,883 B | 7d1efa2138a65b0b488df37f8b89e3d91a60676e416f515b952358d83dfd347e |
| Whisper Tiny | 116,204,861 B | (frozen at build time) |
| Whisper Base | 207,557,382 B | (frozen at build time) |
| Whisper Small | 639,387,718 B | (frozen at build time) |
| Silero VAD | 643,854 B | 9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6 |

## 8. Runtime Identity

- sherpa-onnx version: 1.13.5
- Git SHA: 3dc7c569f31ca2cd4a20ed6f7db780327e6714c5
- Build: static (no dynamic library dependencies)

## 9. No Secret / Model Artifact Check

```bash
git status --short
# Expected: no model weights, user audio, API keys, or private paths
```

## 10. Completion Checklist

- [ ] SenseVoice and Whisper both execute real local inference
- [ ] Settings selection changes the next Job's persisted provider/model
- [ ] Model downloads are hashed, safely extracted, versioned, recoverable
- [ ] Audio is immutable, re-hashed before ASR, decoded for declared formats
- [ ] Jobs use singleton worker locking, leases, boot IDs, fencing
- [ ] Successful results atomically publish Receipt, Revision, Segments, FTS
- [ ] Retranscription creates a new revision and preserves the previous
- [ ] Real fixture CER/WER, phrase, and timing thresholds pass
- [ ] UI responsiveness, cancellation, restart recovery, layout pass
- [ ] Static runtime, otool, signatures, DMG, licenses pass
- [ ] No console.log, model weights, or secrets in repository