#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd -P)
SCRIPT="$ROOT/scripts/build-capture-helper.sh"
[ -x "$SCRIPT" ] || { printf 'FAIL: missing executable build script\n' >&2; exit 1; }

TMP=$(mktemp -d "${TMPDIR:-/tmp}/lifesub-helper-build.XXXXXX")
TMP=$(CDPATH= cd -- "$TMP" && pwd -P)
trap 'rm -rf "$TMP"' EXIT HUP INT TERM
mkdir -p "$TMP/project/scripts" "$TMP/project/src-tauri/native/capture-helper" "$TMP/bin"
cp "$SCRIPT" "$TMP/project/scripts/build-capture-helper.sh"
printf '// fixture\n' >"$TMP/project/src-tauri/native/capture-helper/Package.swift"

cat >"$TMP/bin/swift" <<'EOF'
#!/bin/sh
set -eu
project=
triple=
show=0
while [ "$#" -gt 0 ]; do
  case "$1" in
    --package-path) project=$2; shift 2 ;;
    --triple) triple=$2; shift 2 ;;
    --show-bin-path) show=1; shift ;;
    *) shift ;;
  esac
done
mkdir -p "$project/.build/$triple/release"
printf 'fixture helper\n' >"$project/.build/$triple/release/lifesub-capture-helper"
[ "$show" -eq 0 ] || printf '%s\n' "$project/.build/$triple/release"
EOF
cat >"$TMP/bin/lipo" <<'EOF'
#!/bin/sh
if [ "${FIXTURE_ARCH:-arm64}" = arm64 ]; then printf 'arm64\n'; else printf 'x86_64\n'; fi
EOF
cat >"$TMP/bin/codesign" <<'EOF'
#!/bin/sh
printf '%s\n' "$*" >>"$CODESIGN_LOG"
exit 0
EOF
cat >"$TMP/bin/shasum" <<'EOF'
#!/bin/sh
printf 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  %s\n' "$3"
EOF
chmod +x "$TMP/bin/"*

CODESIGN_LOG="$TMP/codesign.log"
export CODESIGN_LOG
output=$(PATH="$TMP/bin:$PATH" LIFESUB_CODESIGN_IDENTITY=- \
  sh "$TMP/project/scripts/build-capture-helper.sh")
OUTPUT="$TMP/project/src-tauri/binaries/lifesub-capture-helper-aarch64-apple-darwin"
[ -x "$OUTPUT" ] || { printf 'FAIL: missing target-suffixed helper\n' >&2; exit 1; }
case "$output" in *'capture helper signing identity: -'*) ;; *) printf 'FAIL: identity not reported\n' >&2; exit 1;; esac
grep -F -- '--sign - --identifier lifesub-capture-helper' "$CODESIGN_LOG" >/dev/null || {
  printf 'FAIL: explicit helper identifier not signed\n' >&2; exit 1;
}
grep -F -- "--verify --strict --verbose=2 $OUTPUT" "$CODESIGN_LOG" >/dev/null || {
  printf 'FAIL: final helper path not verified\n' >&2; exit 1;
}

if PATH="$TMP/bin:$PATH" FIXTURE_ARCH=x86_64 LIFESUB_CODESIGN_IDENTITY=- \
  sh "$TMP/project/scripts/build-capture-helper.sh" >/dev/null 2>&1; then
  printf 'FAIL: non-arm64 helper accepted\n' >&2
  exit 1
fi

node - "$ROOT/src-tauri/tauri.conf.json" "$ROOT/src-tauri/Info.plist" <<'NODE'
const fs = require('fs');
const [configPath, plistPath] = process.argv.slice(2);
const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
if (!config.bundle?.externalBin?.includes('binaries/lifesub-capture-helper')) process.exit(1);
const plist = fs.readFileSync(plistPath, 'utf8');
for (const key of ['NSMicrophoneUsageDescription', 'NSScreenCaptureUsageDescription', 'LSRequiresCarbon', 'NSQuitAlwaysKeepsWindows']) {
  if (!plist.includes(`<key>${key}</key>`)) process.exit(1);
}
NODE

printf 'PASS: capture helper packaging contract\n'
