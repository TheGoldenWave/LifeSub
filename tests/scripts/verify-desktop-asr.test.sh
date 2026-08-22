#!/usr/bin/env bash

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
HARNESS="$PROJECT_ROOT/scripts/verify-desktop-asr.sh"

run_dmg_case() {
  local codesign_exit="$1"
  local smoke_exit="$2"
  local expected_exit="$3"
  local temp_root
  temp_root="$(mktemp -d)"
  trap 'rm -rf "$temp_root"' RETURN

  mkdir -p \
    "$temp_root/project/scripts" \
    "$temp_root/project/src-tauri/target/release/bundle/dmg" \
    "$temp_root/bin"
  cp "$HARNESS" "$temp_root/project/scripts/verify-desktop-asr.sh"
  touch "$temp_root/project/src-tauri/target/release/bundle/dmg/LifeSub_0.2.1_aarch64.dmg"

  cat > "$temp_root/bin/hdiutil" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
if [[ "$1" == "attach" ]]; then
  while [[ $# -gt 0 ]]; do
    if [[ "$1" == "-mountpoint" ]]; then
      shift
      mount_point="$1"
      mkdir -p "$mount_point/LifeSub.app/Contents/MacOS"
      cat > "$mount_point/LifeSub.app/Contents/MacOS/lifesub" <<'APP'
#!/usr/bin/env bash
if [[ "${SMOKE_EXIT:?}" -eq 0 ]]; then
  mkdir -p "${LIFESUB_ACCEPTANCE_DIR:?}"
  printf '{"scenario":"packaged-smoke","passed":true}\n' \
    > "$LIFESUB_ACCEPTANCE_DIR/acceptance-packaged-smoke.json"
fi
exit "${SMOKE_EXIT:?}"
APP
      chmod +x "$mount_point/LifeSub.app/Contents/MacOS/lifesub"
      exit 0
    fi
    shift
  done
fi
exit 0
EOF
  chmod +x "$temp_root/bin/hdiutil"

  cat > "$temp_root/bin/codesign" <<'EOF'
#!/usr/bin/env bash
exit "${CODESIGN_EXIT:?}"
EOF
  chmod +x "$temp_root/bin/codesign"

  set +e
  PATH="$temp_root/bin:$PATH" \
    CODESIGN_EXIT="$codesign_exit" \
    SMOKE_EXIT="$smoke_exit" \
    "$temp_root/project/scripts/verify-desktop-asr.sh" dmg >/dev/null 2>&1
  local actual_exit=$?
  set -e

  if [[ "$actual_exit" -ne "$expected_exit" ]]; then
    printf 'expected exit %s, got %s (codesign=%s smoke=%s)\n' \
      "$expected_exit" "$actual_exit" "$codesign_exit" "$smoke_exit" >&2
    return 1
  fi
}

run_dmg_case 1 0 1
run_dmg_case 0 1 1
run_dmg_case 0 0 0

printf 'verify-desktop-asr shell tests passed\n'
