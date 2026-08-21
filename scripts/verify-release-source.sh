#!/bin/sh

set -eu

fail() {
  printf 'release-source verification failed: %s\n' "$1" >&2
  exit 1
}

report_gap() {
  message=$1
  if [ "$ALLOW_PLANNED" = "1" ]; then
    printf 'planned gap: %s\n' "$message"
    PLANNED_GAPS=$((PLANNED_GAPS + 1))
    return
  fi
  fail "$message"
}

read_json_version() {
  file=$1
  node -e '
    const fs = require("fs");
    const value = JSON.parse(fs.readFileSync(process.argv[1], "utf8")).version;
    if (typeof value !== "string" || value.length === 0) process.exit(1);
    process.stdout.write(value);
  ' "$file" 2>/dev/null || fail "cannot read version from $file"
}

read_cargo_version() {
  file=$1
  awk '
    /^\[package\][[:space:]]*$/ { in_package = 1; next }
    in_package && /^\[/ { exit }
    in_package && /^[[:space:]]*version[[:space:]]*=/ {
      line = $0
      sub(/^[^=]*=[[:space:]]*"/, "", line)
      sub(/".*/, "", line)
      print line
      exit
    }
  ' "$file"
}

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
DEFAULT_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd -P)
ROOT_INPUT=${LIFESUB_RELEASE_ROOT:-$DEFAULT_ROOT}
[ -d "$ROOT_INPUT" ] || fail "release root does not exist: $ROOT_INPUT"
ROOT=$(CDPATH= cd -- "$ROOT_INPUT" && pwd -P)
ALLOW_PLANNED=${LIFESUB_RELEASE_ALLOW_PLANNED:-0}
PLANNED_GAPS=0

case "$ALLOW_PLANNED" in
  0|1) ;;
  *) fail "LIFESUB_RELEASE_ALLOW_PLANNED must be 0 or 1" ;;
esac

WORKSPACE_STATUS="$ROOT/docs/workspace-status.md"
[ -f "$WORKSPACE_STATUS" ] || fail "missing workspace status: $WORKSPACE_STATUS"

declared_value() {
  label=$1
  sed -n "s/^- $label: \`\([^\`]*\)\`.*/\1/p" "$WORKSPACE_STATUS" | head -n 1
}

EXPECTED_WORKTREE=$(declared_value "Release source worktree")
EXPECTED_BRANCH=$(declared_value "Release source branch")
EXPECTED_VERSION=$(declared_value "Release version")

[ -n "$EXPECTED_WORKTREE" ] || fail "workspace status does not declare Release source worktree"
[ -n "$EXPECTED_BRANCH" ] || fail "workspace status does not declare Release source branch"
[ -n "$EXPECTED_VERSION" ] || fail "workspace status does not declare Release version"

if [ -d "$EXPECTED_WORKTREE" ]; then
  EXPECTED_WORKTREE=$(CDPATH= cd -- "$EXPECTED_WORKTREE" && pwd -P)
fi
[ "$ROOT" = "$EXPECTED_WORKTREE" ] || \
  fail "unexpected release worktree: expected $EXPECTED_WORKTREE, got $ROOT"

GIT_ROOT=$(git -C "$ROOT" rev-parse --show-toplevel 2>/dev/null) || \
  fail "release worktree is not a Git repository: $ROOT"
GIT_ROOT=$(CDPATH= cd -- "$GIT_ROOT" && pwd -P)
[ "$GIT_ROOT" = "$ROOT" ] || \
  fail "release root is not the Git worktree root: expected $ROOT, got $GIT_ROOT"

BRANCH=$(git -C "$ROOT" branch --show-current)
[ "$BRANCH" = "$EXPECTED_BRANCH" ] || \
  fail "unexpected release branch: expected $EXPECTED_BRANCH, got ${BRANCH:-detached HEAD}"
HEAD=$(git -C "$ROOT" rev-parse HEAD)
DIRTY_COUNT=$(git -C "$ROOT" status --porcelain | wc -l | tr -d '[:space:]')

[ -f "$ROOT/package.json" ] || fail "missing package.json"
[ -f "$ROOT/src-tauri/Cargo.toml" ] || fail "missing src-tauri/Cargo.toml"
[ -f "$ROOT/src-tauri/tauri.conf.json" ] || fail "missing src-tauri/tauri.conf.json"

PACKAGE_VERSION=$(read_json_version "$ROOT/package.json")
CARGO_VERSION=$(read_cargo_version "$ROOT/src-tauri/Cargo.toml")
TAURI_VERSION=$(read_json_version "$ROOT/src-tauri/tauri.conf.json")

[ "$PACKAGE_VERSION" = "$EXPECTED_VERSION" ] || \
  fail "package.json version mismatch: expected $EXPECTED_VERSION, got ${PACKAGE_VERSION:-missing}"
[ "$CARGO_VERSION" = "$EXPECTED_VERSION" ] || \
  fail "Cargo.toml version mismatch: expected $EXPECTED_VERSION, got ${CARGO_VERSION:-missing}"
[ "$TAURI_VERSION" = "$EXPECTED_VERSION" ] || \
  fail "tauri.conf.json version mismatch: expected $EXPECTED_VERSION, got ${TAURI_VERSION:-missing}"

SOURCE_ROOT="$ROOT/src-tauri/src"
[ -d "$SOURCE_ROOT" ] || fail "missing Rust source tree: $SOURCE_ROOT"

if ! grep -R -q -F 'NativeCaptureCoordinator' "$SOURCE_ROOT/capture" 2>/dev/null; then
  report_gap "production capture marker missing: NativeCaptureCoordinator"
fi
if ! grep -R -q -F 'NativeAsrEngine' "$SOURCE_ROOT" 2>/dev/null; then
  report_gap "production native ASR marker missing: NativeAsrEngine"
fi
if grep -R -q -F 'run_unavailable_loop(app, stop_clone, pause_clone);' "$SOURCE_ROOT/capture" 2>/dev/null; then
  report_gap "unavailable capture remains production-selected"
fi
if grep -R -q -F 'crate::asr::worker::spawn_fail_closed_worker' "$SOURCE_ROOT" 2>/dev/null; then
  report_gap "fail-closed ASR engine remains production-selected"
fi

printf 'worktree: %s\n' "$ROOT"
printf 'branch: %s\n' "$BRANCH"
printf 'HEAD: %s\n' "$HEAD"
printf 'dirty count: %s\n' "$DIRTY_COUNT"
printf 'version: %s\n' "$EXPECTED_VERSION"

if [ "$ALLOW_PLANNED" = "1" ]; then
  printf 'mode: PLANNED AUDIT ONLY (%s production gap(s)); not release-ready\n' "$PLANNED_GAPS"
else
  printf 'capture marker: NativeCaptureCoordinator\n'
  printf 'ASR marker: NativeAsrEngine\n'
  printf 'release source: verified\n'
fi
