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

assert_not_contains() {
  haystack=$1
  needle=$2
  description=$3
  case "$haystack" in
    *"$needle"*) fail "$description (unexpected: $needle)" ;;
    *) ;;
  esac
}

write_valid_fixture() {
  fixture=$1
  mkdir -p \
    "$fixture/docs" \
    "$fixture/scripts" \
    "$fixture/src-tauri/src" \
    "$fixture/src-tauri/tests"

  printf '%s\n' \
    '{' \
    '  "name": "lifesub-fixture",' \
    '  "version": "0.2.1"' \
    '}' >"$fixture/package.json"
  printf '%s\n' \
    '[package]' \
    'name = "lifesub-fixture"' \
    'version = "0.2.1"' \
    'edition = "2021"' \
    '' \
    '[lib]' \
    'name = "lifesub_lib"' \
    '' \
    '[features]' \
    'desktop = []' >"$fixture/src-tauri/Cargo.toml"
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
  write_runtime_factory "$fixture" native
  cp "$VERIFIER" "$fixture/scripts/verify-release-source.sh"
  cp "$PROJECT_ROOT/src-tauri/tests/release_wiring.rs" \
    "$fixture/src-tauri/tests/release_wiring.rs"
  cargo generate-lockfile --manifest-path "$fixture/src-tauri/Cargo.toml" >/dev/null

  git -C "$fixture" init -q
  git -C "$fixture" config user.email "release-source-test@example.invalid"
  git -C "$fixture" config user.name "Release Source Test"
  git -C "$fixture" checkout -q -b "$FIXTURE_BRANCH"
  git -C "$fixture" add .
  git -C "$fixture" commit -qm "fixture"
}

write_runtime_factory() {
  fixture=$1
  mode=$2
  case "$mode" in
    native) production_factory=NativeDesktopRuntimeFactory ;;
    capture_only) production_factory=CaptureOnlyDesktopRuntimeFactory ;;
    asr_only) production_factory=AsrOnlyDesktopRuntimeFactory ;;
    fail_closed) production_factory=FailClosedDesktopRuntimeFactory ;;
    *) fail "unknown runtime factory fixture mode: $mode" ;;
  esac

  printf '%s\n' \
    '#[cfg(feature = "desktop")]' \
    'pub mod desktop_runtime {' \
    '    pub trait DesktopRuntimeFactory {' \
    '        const USES_NATIVE_CAPTURE: bool;' \
    '        const USES_NATIVE_ASR: bool;' \
    '        fn create_capture() -> &'\''static str;' \
    '        fn spawn_worker() -> &'\''static str;' \
    '    }' \
    '    pub struct NativeDesktopRuntimeFactory;' \
    '    pub struct CaptureOnlyDesktopRuntimeFactory;' \
    '    pub struct AsrOnlyDesktopRuntimeFactory;' \
    '    pub struct FailClosedDesktopRuntimeFactory;' \
    '    macro_rules! factory {' \
    '        ($name:ty, $capture:expr, $asr:expr) => {' \
    '            impl DesktopRuntimeFactory for $name {' \
    '                const USES_NATIVE_CAPTURE: bool = $capture;' \
    '                const USES_NATIVE_ASR: bool = $asr;' \
    '                fn create_capture() -> &'\''static str { "capture" }' \
    '                fn spawn_worker() -> &'\''static str { "worker" }' \
    '            }' \
    '        };' \
    '    }' \
    '    factory!(NativeDesktopRuntimeFactory, true, true);' \
    '    factory!(CaptureOnlyDesktopRuntimeFactory, true, false);' \
    '    factory!(AsrOnlyDesktopRuntimeFactory, false, true);' \
    '    factory!(FailClosedDesktopRuntimeFactory, false, false);' \
    "    pub type ProductionDesktopRuntimeFactory = $production_factory;" \
    '    pub fn initialize_at() -> (&'\''static str, &'\''static str) {' \
    '        (' \
    '            ProductionDesktopRuntimeFactory::create_capture(),' \
    '            ProductionDesktopRuntimeFactory::spawn_worker(),' \
    '        )' \
    '    }' \
    '}' >"$fixture/src-tauri/src/lib.rs"
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
  env "$@" sh "$fixture/scripts/verify-release-source.sh" 2>&1
}

run_with_declared_contract() {
  fixture=$1
  sh "$fixture/scripts/verify-release-source.sh" 2>&1
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
git -C "$TMP_ROOT/valid" ls-files --error-unmatch src-tauri/Cargo.lock >/dev/null || \
  fail "fixture Cargo.lock was not committed before verification"

evil_fixture=$(copy_fixture root-override-evil)
trusted_fixture=$(copy_fixture root-override-trusted)
mkdir -p "$trusted_fixture/scripts"
cp "$VERIFIER" "$trusted_fixture/scripts/verify-release-source.sh"
sed -i.bak "s|Release source worktree: \`$trusted_fixture\`|Release source worktree: \`$TMP_ROOT/declared-trusted-release\`|" \
  "$trusted_fixture/docs/workspace-status.md"
rm "$trusted_fixture/docs/workspace-status.md.bak"
if output=$(env LIFESUB_RELEASE_ROOT="$evil_fixture" \
  sh "$trusted_fixture/scripts/verify-release-source.sh" 2>&1); then
  fail "release root override attack was accepted: $output"
fi
assert_contains "$output" "unexpected release worktree" "release root override attack rejection"

fixture=$(copy_fixture env-override-attack)
git -C "$fixture" checkout -q -b "evil/release"
sed -i.bak "s|Release source worktree: \`$fixture\`|Release source worktree: \`$TMP_ROOT/trusted-release\`|" \
  "$fixture/docs/workspace-status.md"
rm "$fixture/docs/workspace-status.md.bak"
sed -i.bak 's/0.2.1/9.9.9/' "$fixture/package.json"
rm "$fixture/package.json.bak"
sed -i.bak 's/0.2.1/9.9.9/' "$fixture/src-tauri/Cargo.toml"
rm "$fixture/src-tauri/Cargo.toml.bak"
sed -i.bak 's/0.2.1/9.9.9/' "$fixture/src-tauri/tauri.conf.json"
rm "$fixture/src-tauri/tauri.conf.json.bak"
if output=$(env \
  LIFESUB_RELEASE_EXPECTED_WORKTREE="$fixture" \
  LIFESUB_RELEASE_EXPECTED_BRANCH="evil/release" \
  LIFESUB_RELEASE_EXPECTED_VERSION="9.9.9" \
  sh "$fixture/scripts/verify-release-source.sh" 2>&1); then
  fail "release identity override attack was accepted: $output"
fi
assert_contains "$output" "unexpected release worktree" "identity override attack rejection"

fixture=$(copy_fixture unexpected-worktree)
sed -i.bak "s|Release source worktree: \`$fixture\`|Release source worktree: \`$TMP_ROOT/not-the-release-source\`|" \
  "$fixture/docs/workspace-status.md"
rm "$fixture/docs/workspace-status.md.bak"
assert_rejected "$fixture" "unexpected release worktree"

fixture=$(copy_fixture unexpected-branch)
git -C "$fixture" checkout -q -b "codex/not-the-release-branch"
assert_rejected "$fixture" "unexpected release branch"

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
write_runtime_factory "$fixture" asr_only
assert_rejected "$fixture" "production desktop runtime must select native capture"

fixture=$(copy_fixture missing-native-engine-marker)
write_runtime_factory "$fixture" capture_only
assert_rejected "$fixture" "production desktop runtime must select native ASR"

fixture=$(copy_fixture fail-closed-engine-selected)
write_runtime_factory "$fixture" fail_closed
assert_rejected "$fixture" "production desktop runtime must select native capture"

fixture=$(copy_fixture stale-lockfile)
lock_before=$(cksum "$fixture/src-tauri/Cargo.lock")
mkdir -p "$fixture/src-tauri/fixture-helper/src"
printf '%s\n' \
  '[package]' \
  'name = "fixture-helper"' \
  'version = "0.1.0"' \
  'edition = "2021"' >"$fixture/src-tauri/fixture-helper/Cargo.toml"
printf '%s\n' >"$fixture/src-tauri/fixture-helper/src/lib.rs"
printf '%s\n' \
  '' \
  '[dependencies]' \
  'fixture-helper = { path = "fixture-helper" }' >>"$fixture/src-tauri/Cargo.toml"
assert_rejected "$fixture" "--locked was passed"
lock_after=$(cksum "$fixture/src-tauri/Cargo.lock")
[ "$lock_before" = "$lock_after" ] || fail "--locked verification modified Cargo.lock"

fixture=$(copy_fixture planned-audit)
write_runtime_factory "$fixture" fail_closed
output=$(run_verifier "$fixture" LIFESUB_RELEASE_ALLOW_PLANNED=1) || \
  fail "planned development audit was rejected: $output"
assert_contains "$output" "PLANNED AUDIT ONLY" "planned audit warning"
assert_contains "$output" "production release wiring gate failed" "planned wiring gap"
assert_not_contains "$output" "release source: verified" "planned audit verification claim"

fixture=$(copy_fixture planned-audit-with-markers)
output=$(run_verifier "$fixture" LIFESUB_RELEASE_ALLOW_PLANNED=1) || \
  fail "planned marker-complete audit was rejected: $output"
assert_contains "$output" "PLANNED AUDIT ONLY" "planned marker-complete warning"
assert_contains "$output" "not release-ready" "planned marker-complete release warning"
assert_not_contains "$output" "release source: verified" "planned marker-complete verification claim"

fixture=$(copy_fixture valid-release-source)
mkdir -p "$fixture/untracked/nested"
printf '%s\n' "one" >"$fixture/untracked/one"
printf '%s\n' "two" >"$fixture/untracked/two"
printf '%s\n' "three" >"$fixture/untracked/nested/three"
output=$(run_with_declared_contract "$fixture") || fail "valid fixture was rejected: $output"
assert_contains "$output" "worktree: $fixture" "absolute worktree report"
assert_contains "$output" "branch: $FIXTURE_BRANCH" "branch report"
assert_contains "$output" "HEAD:" "HEAD report"
assert_contains "$output" "dirty count: 4" "dirty count report"
assert_contains "$output" "version: $FIXTURE_VERSION" "version report"
assert_contains "$output" "release wiring gate: passed" "release wiring gate report"

printf 'PASS: release-source verifier contract\n'
