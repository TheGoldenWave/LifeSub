#!/bin/sh

set -eu

TEST_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
PROJECT_ROOT=$(CDPATH= cd -- "$TEST_DIR/../.." && pwd)
VERIFIER="$PROJECT_ROOT/scripts/verify-release-source.sh"
FIXTURE_BRANCH="codex/lifesub-real-asr-v0.2"
FIXTURE_VERSION="0.2.1"

TMP_ROOT=$(mktemp -d "${TMPDIR:-/tmp}/lifesub-release-source.XXXXXX")
TMP_ROOT=$(CDPATH= cd -- "$TMP_ROOT" && pwd -P)
trap 'rm -rf "$TMP_ROOT"' EXIT HUP INT TERM

fail() {
  printf 'FAIL: %s\n' "$1" >&2
  exit 1
}

[ -f "$VERIFIER" ] || fail "verifier script is missing: $VERIFIER"

assert_contains() {
  haystack=$1
  needle=$2
  description=$3
  case "$haystack" in
    *"$needle"*) ;;
    *) fail "$description (missing: $needle)" ;;
  esac
}

write_valid_fixture() {
  fixture=$1
  mkdir -p \
    "$fixture/docs" \
    "$fixture/src-tauri/src/capture" \
    "$fixture/src-tauri/src/asr"

  printf '%s\n' \
    '{' \
    '  "name": "lifesub-fixture",' \
    '  "version": "0.2.1"' \
    '}' >"$fixture/package.json"
  printf '%s\n' \
    '[package]' \
    'name = "lifesub-fixture"' \
    'version = "0.2.1"' >"$fixture/src-tauri/Cargo.toml"
  printf '%s\n' \
    '{' \
    '  "productName": "LifeSub Fixture",' \
    '  "version": "0.2.1"' \
    '}' >"$fixture/src-tauri/tauri.conf.json"
  printf '%s\n' \
    '# LifeSub Workspace Status' \
    '' \
    '## Integrated release source contract' \
    '' \
    "- Release source worktree: \`$fixture\`" \
    "- Release source branch: \`$FIXTURE_BRANCH\`" \
    "- Release version: \`$FIXTURE_VERSION\`" >"$fixture/docs/workspace-status.md"
  printf '%s\n' \
    'pub struct NativeCaptureCoordinator;' \
    'pub type ProductionCaptureCoordinator = NativeCaptureCoordinator;' \
    >"$fixture/src-tauri/src/capture/mod.rs"
  printf '%s\n' \
    'pub struct NativeAsrEngine;' \
    'pub type ProductionAsrEngine = NativeAsrEngine;' \
    >"$fixture/src-tauri/src/asr/worker.rs"

  git -C "$fixture" init -q
  git -C "$fixture" config user.email "release-source-test@example.invalid"
  git -C "$fixture" config user.name "Release Source Test"
  git -C "$fixture" checkout -q -b "$FIXTURE_BRANCH"
  git -C "$fixture" add .
  git -C "$fixture" commit -qm "fixture"
}

copy_fixture() {
  name=$1
  fixture="$TMP_ROOT/$name"
  git clone -q "$TMP_ROOT/valid" "$fixture"
  git -C "$fixture" checkout -q "$FIXTURE_BRANCH"
  sed "s|$TMP_ROOT/valid|$fixture|" \
    "$TMP_ROOT/valid/docs/workspace-status.md" >"$fixture/docs/workspace-status.md"
  printf '%s\n' "$fixture"
}

run_verifier() {
  fixture=$1
  shift
  env \
    LIFESUB_RELEASE_ROOT="$fixture" \
    LIFESUB_RELEASE_EXPECTED_WORKTREE="$fixture" \
    LIFESUB_RELEASE_EXPECTED_BRANCH="$FIXTURE_BRANCH" \
    LIFESUB_RELEASE_EXPECTED_VERSION="$FIXTURE_VERSION" \
    "$@" \
    sh "$VERIFIER" 2>&1
}

run_with_declared_contract() {
  fixture=$1
  env LIFESUB_RELEASE_ROOT="$fixture" sh "$VERIFIER" 2>&1
}

assert_rejected() {
  fixture=$1
  expected=$2
  shift 2
  if output=$(run_verifier "$fixture" "$@"); then
    fail "expected rejection containing: $expected"
  fi
  assert_contains "$output" "$expected" "rejection reason"
}

write_valid_fixture "$TMP_ROOT/valid"

fixture=$(copy_fixture unexpected-worktree)
if output=$(env \
  LIFESUB_RELEASE_ROOT="$fixture" \
  LIFESUB_RELEASE_EXPECTED_WORKTREE="$TMP_ROOT/not-the-release-source" \
  LIFESUB_RELEASE_EXPECTED_BRANCH="$FIXTURE_BRANCH" \
  LIFESUB_RELEASE_EXPECTED_VERSION="$FIXTURE_VERSION" \
  sh "$VERIFIER" 2>&1); then
  fail "unexpected worktree identity was accepted"
fi
assert_contains "$output" "unexpected release worktree" "worktree identity rejection"

fixture=$(copy_fixture unexpected-branch)
assert_rejected "$fixture" "unexpected release branch" \
  LIFESUB_RELEASE_EXPECTED_BRANCH="codex/not-the-release-branch"

fixture=$(copy_fixture missing-workspace-status)
rm "$fixture/docs/workspace-status.md"
assert_rejected "$fixture" "missing workspace status"

fixture=$(copy_fixture package-version-mismatch)
sed -i.bak 's/"version": "0.2.1"/"version": "0.2.0"/' "$fixture/package.json"
rm "$fixture/package.json.bak"
assert_rejected "$fixture" "package.json version mismatch"

fixture=$(copy_fixture cargo-version-mismatch)
sed -i.bak 's/version = "0.2.1"/version = "0.2.0"/' "$fixture/src-tauri/Cargo.toml"
rm "$fixture/src-tauri/Cargo.toml.bak"
assert_rejected "$fixture" "Cargo.toml version mismatch"

fixture=$(copy_fixture tauri-version-mismatch)
sed -i.bak 's/"version": "0.2.1"/"version": "0.2.0"/' "$fixture/src-tauri/tauri.conf.json"
rm "$fixture/src-tauri/tauri.conf.json.bak"
assert_rejected "$fixture" "tauri.conf.json version mismatch"

fixture=$(copy_fixture missing-capture-marker)
printf '%s\n' \
  'pub struct UnavailableStreamingSource;' \
  'pub type ProductionCaptureCoordinator = UnavailableStreamingSource;' \
  >"$fixture/src-tauri/src/capture/mod.rs"
assert_rejected "$fixture" "production capture marker missing"

fixture=$(copy_fixture missing-native-engine-marker)
printf '%s\n' \
  'pub struct FailClosedEngine;' \
  'pub type ProductionAsrEngine = FailClosedEngine;' \
  >"$fixture/src-tauri/src/asr/worker.rs"
assert_rejected "$fixture" "production native ASR marker missing"

fixture=$(copy_fixture unavailable-capture-selected)
printf '%s\n' \
  'pub struct NativeCaptureCoordinator;' \
  'fn start(app: AppHandle, stop_clone: Stop, pause_clone: Pause) {' \
  '    run_unavailable_loop(app, stop_clone, pause_clone);' \
  '}' >"$fixture/src-tauri/src/capture/mod.rs"
assert_rejected "$fixture" "unavailable capture remains production-selected"

fixture=$(copy_fixture fail-closed-engine-selected)
printf '%s\n' \
  'pub struct NativeAsrEngine;' \
  'fn initialize(runtime: Runtime) {' \
  '    crate::asr::worker::spawn_fail_closed_worker(runtime);' \
  '}' >"$fixture/src-tauri/src/asr/worker.rs"
assert_rejected "$fixture" "fail-closed ASR engine remains production-selected"

fixture=$(copy_fixture planned-audit)
printf '%s\n' 'pub struct UnavailableStreamingSource;' \
  >"$fixture/src-tauri/src/capture/mod.rs"
printf '%s\n' 'pub struct FailClosedEngine;' \
  >"$fixture/src-tauri/src/asr/worker.rs"
output=$(run_verifier "$fixture" LIFESUB_RELEASE_ALLOW_PLANNED=1) || \
  fail "planned development audit was rejected: $output"
assert_contains "$output" "PLANNED AUDIT ONLY" "planned audit warning"
assert_contains "$output" "production capture marker missing" "planned capture gap"
assert_contains "$output" "production native ASR marker missing" "planned ASR gap"

fixture=$(copy_fixture valid-release-source)
printf '%s\n' "dirty" >"$fixture/dirty-file"
output=$(run_with_declared_contract "$fixture") || fail "valid fixture was rejected: $output"
assert_contains "$output" "worktree: $fixture" "absolute worktree report"
assert_contains "$output" "branch: $FIXTURE_BRANCH" "branch report"
assert_contains "$output" "HEAD:" "HEAD report"
assert_contains "$output" "dirty count: 2" "dirty count report"
assert_contains "$output" "version: $FIXTURE_VERSION" "version report"
assert_contains "$output" "capture marker: NativeCaptureCoordinator" "capture marker report"
assert_contains "$output" "ASR marker: NativeAsrEngine" "ASR marker report"

printf 'PASS: release-source verifier contract\n'
