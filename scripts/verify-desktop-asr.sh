#!/usr/bin/env bash
# LifeSub desktop ASR verification harness.
#
# Usage:
#   scripts/verify-desktop-asr.sh [target|dmg|--verify-existing]
#
# Modes:
#   target       Build the app and run acceptance scenarios against the
#                development binary with an isolated HOME.
#   dmg          Locate the produced DMG, mount it, verify the contained
#                .app signature, run packaged-smoke, then detach.
#   --verify-existing  Read-only: validate committed report JSON without
#                      executing models or rewriting results.
#
# Environment:
#   LIFESUB_ASR_MODEL_DIR   Path to installed model cache (required for
#                           scenarios that run real inference).
#   SHERPA_ONNX_ARCHIVE_DIR Path to the verified native runtime archive.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
SCOPE_FILE="$SCRIPT_DIR/desktop-asr-scope.txt"
REPORT_DIR="$PROJECT_DIR/output/asr-v0.2"

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

red()    { printf '\033[31m%s\033[0m\n' "$*" >&2; }
green()  { printf '\033[32m%s\033[0m\n' "$*" >&2; }
yellow() { printf '\033[33m%s\033[0m\n' "$*" >&2; }

die() { red "FATAL: $*"; exit 1; }

require_passing_report() {
  local report="$1"
  local scenario="$2"
  if [ ! -f "$report" ]; then
    die "$scenario did not create its acceptance report"
  fi
  if ! python3 -c "import json; r=json.load(open('$report')); raise SystemExit(0 if r.get('passed') else 1)"; then
    die "$scenario report did not pass"
  fi
}

# Compute a deterministic hash of the scoped source paths.
# Only the exact paths listed in desktop-asr-scope.txt are included.
hash_scoped_paths() {
  if [ ! -f "$SCOPE_FILE" ]; then
    die "scope file not found: $SCOPE_FILE"
  fi

  local tmp
  tmp="$(mktemp)"
  while IFS= read -r path; do
    # Skip empty lines and comments
    [[ -z "$path" || "$path" == \#* ]] && continue
    local full_path="$PROJECT_DIR/$path"
    if [ -f "$full_path" ]; then
      sha256sum "$full_path" >> "$tmp"
    fi
  done < "$SCOPE_FILE"
  sort "$tmp" | sha256sum | cut -d' ' -f1
  rm -f "$tmp"
}

# Verify scoped files are clean relative to HEAD.
verify_clean_scope() {
  local dirty=0
  while IFS= read -r path; do
    [[ -z "$path" || "$path" == \#* ]] && continue
    if git diff --quiet HEAD -- "$path" 2>/dev/null; then
      continue
    else
      yellow "  dirty: $path"
      dirty=1
    fi
  done < "$SCOPE_FILE"

  if [ "$dirty" -ne 0 ]; then
    die "scoped files are dirty — commit or stash before running the harness"
  fi
  green "  all scoped files are clean"
}

# ---------------------------------------------------------------------------
# Verify existing report (read-only)
# ---------------------------------------------------------------------------

verify_existing() {
  green "=== verify-desktop-asr.sh --verify-existing ==="

  local report="$REPORT_DIR/acceptance-real-asr-heartbeat.json"
  if [ ! -f "$report" ]; then
    die "no existing acceptance report at $report; run the harness first"
  fi

  green "  found: $report"

  # Check that the report is valid JSON
  if ! python3 -c "import json; json.load(open('$report'))" 2>/dev/null; then
    die "report is not valid JSON"
  fi

  # Check that the report is not a mock
  if grep -qi 'mock' "$report"; then
    die "report contains 'mock' — rejected"
  fi

  # Check that scenarios were actually run
  local scenario_count
  scenario_count="$(python3 -c "
import json
r = json.load(open('$report'))
print(1 if r.get('passed') else 0)
" 2>/dev/null || echo "0")"

  if [ "$scenario_count" -eq 0 ]; then
    die "report exists but scenario did not pass"
  else
    green "  report verified: scenario passed"
  fi

  green "=== verify-existing complete ==="
}

# ---------------------------------------------------------------------------
# Build and run acceptance scenarios
# ---------------------------------------------------------------------------

run_target_scenarios() {
  green "=== verify-desktop-asr.sh target ==="

  # 1. Hash scoped paths
  green "1. hashing scoped production/acceptance paths..."
  local source_digest
  source_digest="$(hash_scoped_paths)"
  green "   source digest: $source_digest"

  # 2. Verify scoped files are clean
  green "2. verifying scoped files are clean..."
  verify_clean_scope

  # 3. Fetch the native runtime
  green "3. fetching sherpa-onnx runtime..."
  if [ -f "$SCRIPT_DIR/fetch-sherpa-runtime.sh" ]; then
    SHERPA_ONNX_ARCHIVE_DIR="$("$SCRIPT_DIR/fetch-sherpa-runtime.sh")"
    export SHERPA_ONNX_ARCHIVE_DIR
    green "   SHERPA_ONNX_ARCHIVE_DIR=$SHERPA_ONNX_ARCHIVE_DIR"
  else
    yellow "   fetch-sherpa-runtime.sh not found — skipping runtime fetch"
  fi

  # 4. Build the app
  green "4. building LifeSub desktop app..."
  cd "$PROJECT_DIR"
  npm run tauri -- build --features desktop --bundles app 2>&1 | tail -5
  green "   build complete"

  # 5. Create isolated HOME for acceptance scenarios
  local isolated_home
  isolated_home="$(mktemp -d)"
  trap "rm -rf '$isolated_home'" EXIT
  green "5. isolated HOME: $isolated_home"

  local app_binary="$PROJECT_DIR/src-tauri/target/release/bundle/macos/LifeSub.app/Contents/MacOS/lifesub"
  if [ ! -f "$app_binary" ]; then
    # Try debug build
    app_binary="$PROJECT_DIR/src-tauri/target/debug/lifesub"
  fi
  if [ ! -f "$app_binary" ]; then
    die "LifeSub binary not found at $app_binary"
  fi
  green "   binary: $app_binary"

  # 6. Run acceptance scenarios
  mkdir -p "$REPORT_DIR"

  green "6. running acceptance scenarios..."

  local acceptance_dir="$isolated_home/acceptance"
  mkdir -p "$acceptance_dir"

  # 6a. real-asr-heartbeat
  green "   6a. real-asr-heartbeat..."
  HOME="$isolated_home" \
  LIFESUB_ACCEPTANCE_DIR="$acceptance_dir" \
  LIFESUB_ACCEPTANCE_DATA_DIR="$isolated_home/Library/Application Support/com.goldenwave.lifesub" \
  "$app_binary" --acceptance-scenario real-asr-heartbeat 2>&1 || \
    die "real-asr-heartbeat failed"
  local heartbeat_report="$acceptance_dir/acceptance-real-asr-heartbeat.json"
  require_passing_report "$heartbeat_report" "real-asr-heartbeat"
  cp "$heartbeat_report" "$REPORT_DIR/"
  green "   report saved to $REPORT_DIR/acceptance-real-asr-heartbeat.json"

  # 6b. claim-and-abort
  green "   6b. claim-and-abort..."
  HOME="$isolated_home" \
  LIFESUB_ACCEPTANCE_DIR="$acceptance_dir" \
  LIFESUB_ACCEPTANCE_DATA_DIR="$isolated_home/Library/Application Support/com.goldenwave.lifesub" \
  "$app_binary" --acceptance-scenario claim-and-abort 2>&1 || \
    die "claim-and-abort failed"
  local claim_report="$acceptance_dir/acceptance-claim-and-abort.json"
  require_passing_report "$claim_report" "claim-and-abort"
  cp "$claim_report" "$REPORT_DIR/"
  green "   report saved"

  # 6c. verify-recovery
  green "   6c. verify-recovery..."
  HOME="$isolated_home" \
  LIFESUB_ACCEPTANCE_DIR="$acceptance_dir" \
  LIFESUB_ACCEPTANCE_DATA_DIR="$isolated_home/Library/Application Support/com.goldenwave.lifesub" \
  "$app_binary" --acceptance-scenario verify-recovery 2>&1 || \
    die "verify-recovery failed"
  local recovery_report="$acceptance_dir/acceptance-verify-recovery.json"
  require_passing_report "$recovery_report" "verify-recovery"
  cp "$recovery_report" "$REPORT_DIR/"
  green "   report saved"

  # 6d. packaged-smoke
  green "   6d. packaged-smoke..."
  HOME="$isolated_home" \
  LIFESUB_ACCEPTANCE_DIR="$acceptance_dir" \
  LIFESUB_ACCEPTANCE_DATA_DIR="$isolated_home/Library/Application Support/com.goldenwave.lifesub" \
  "$app_binary" --acceptance-scenario packaged-smoke 2>&1 || \
    die "packaged-smoke failed"
  local packaged_report="$acceptance_dir/acceptance-packaged-smoke.json"
  require_passing_report "$packaged_report" "packaged-smoke"
  cp "$packaged_report" "$REPORT_DIR/"
  green "   report saved"

  green "=== target scenarios complete ==="
  green "   reports in: $REPORT_DIR"
}

# ---------------------------------------------------------------------------
# DMG verification
# ---------------------------------------------------------------------------

run_dmg_verification() {
  green "=== verify-desktop-asr.sh dmg ==="

  local dmg_path
  dmg_path="$(find "$PROJECT_DIR/src-tauri/target/release/bundle/dmg" -name '*.dmg' 2>/dev/null | sort | tail -1)"

  if [ -z "$dmg_path" ]; then
    die "no DMG found in target/release/bundle/dmg/"
  fi

  green "1. found DMG: $dmg_path"

  # Mount the DMG
  green "2. mounting DMG..."
  local mount_point
  mount_point="$(mktemp -d)"
  hdiutil attach -readonly -mountpoint "$mount_point" "$dmg_path" 2>&1 || {
    rm -rf "$mount_point"
    die "failed to mount DMG"
  }
  trap "hdiutil detach '$mount_point' 2>/dev/null; rm -rf '$mount_point'" EXIT
  green "   mounted at $mount_point"

  # Find the .app bundle
  local app_bundle
  app_bundle="$(find "$mount_point" -name '*.app' -maxdepth 2 -type d | head -1)"
  if [ -z "$app_bundle" ]; then
    die "no .app bundle found in DMG"
  fi
  green "3. app bundle: $app_bundle"

  # Verify the .app signature
  green "4. verifying code signature..."
  codesign --verify --deep --strict --verbose=2 "$app_bundle" 2>&1 || \
    die "app bundle signature is invalid"
  green "   signature: VALID"

  # Run packaged-smoke from the mounted DMG
  green "5. running packaged-smoke from DMG..."
  local isolated_home
  isolated_home="$(mktemp -d)"
  trap "hdiutil detach '$mount_point' 2>/dev/null; rm -rf '$isolated_home' '$mount_point'" EXIT

  local acceptance_dir="$isolated_home/acceptance"
  mkdir -p "$acceptance_dir" "$REPORT_DIR"

  local app_executable="$app_bundle/Contents/MacOS/lifesub"
  if [ ! -f "$app_executable" ]; then
    die "executable not found at $app_executable"
  fi

  HOME="$isolated_home" \
  LIFESUB_ACCEPTANCE_DIR="$acceptance_dir" \
  LIFESUB_ACCEPTANCE_DATA_DIR="$isolated_home/Library/Application Support/com.goldenwave.lifesub" \
  "$app_executable" --acceptance-scenario packaged-smoke 2>&1 || \
    die "packaged-smoke from DMG failed"

  local smoke_report="$acceptance_dir/acceptance-packaged-smoke.json"
  if [ ! -f "$smoke_report" ]; then
    die "packaged-smoke did not create its acceptance report"
  fi
  require_passing_report "$smoke_report" "packaged-smoke"
  cp "$smoke_report" "$REPORT_DIR/acceptance-dmg-packaged-smoke.json"
  green "   report saved"

  # Detach DMG
  green "6. detaching DMG..."
  hdiutil detach "$mount_point" 2>/dev/null || true
  green "   detached"

  green "=== dmg verification complete ==="
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

case "${1:-target}" in
  target)
    run_target_scenarios
    ;;
  dmg)
    run_dmg_verification
    ;;
  --verify-existing)
    verify_existing
    ;;
  *)
    echo "Usage: $0 [target|dmg|--verify-existing]" >&2
    exit 1
    ;;
esac
