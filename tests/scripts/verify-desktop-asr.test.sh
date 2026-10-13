#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
HARNESS="$PROJECT_ROOT/scripts/verify-desktop-asr.sh"

run_dmg_case() {
  local signature_exit="$1"
  local image_exit="$2"
  local mismatch="$3"
  local expected_exit="$4"
  local temp_root
  temp_root="$(mktemp -d)"
  trap 'rm -rf "$temp_root"' RETURN

  local project="$temp_root/project"
  local bundle="$project/src-tauri/target/aarch64-apple-darwin/release/bundle"
  mkdir -p "$project/scripts" "$bundle/dmg" "$bundle/macos/LifeSub.app/Contents/MacOS" "$temp_root/bin"
  cp "$HARNESS" "$project/scripts/verify-desktop-asr.sh"
  printf '{"version":"0.2.2"}\n' > "$project/package.json"
  touch "$bundle/dmg/LifeSub_0.2.2_aarch64.dmg"
  mkdir -p "$project/src-tauri/target/release/bundle/dmg"
  touch "$project/src-tauri/target/release/bundle/dmg/LifeSub_0.2.1_aarch64.dmg"
  for binary in lifesub lifesub-capture-helper lifesub-asr-gate; do
    printf '%s\n' "$binary" > "$bundle/macos/LifeSub.app/Contents/MacOS/$binary"
  done

  cat > "$temp_root/bin/hdiutil" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
case "$1" in
  verify) exit "$IMAGE_EXIT" ;;
  attach)
    args=("$@")
    mount_point=''
    for ((index=0; index<${#args[@]}; index++)); do
      if [[ "${args[index]}" == '-mountpoint' ]]; then mount_point="${args[index+1]}"; fi
    done
    [[ -n "$mount_point" ]]
    last_arg_index=$((${#args[@]} - 1))
    printf '%s\n' "${args[last_arg_index]}" > "$TEST_ROOT/selected-dmg"
    mkdir -p "$mount_point/LifeSub.app/Contents/MacOS"
    cp "$SOURCE_BUNDLE/Contents/MacOS/"* "$mount_point/LifeSub.app/Contents/MacOS/"
    if [[ "$BUNDLE_MISMATCH" == 1 ]]; then printf 'changed\n' > "$mount_point/LifeSub.app/Contents/MacOS/lifesub-capture-helper"; fi
    ;;
  detach) rm -rf "$2/LifeSub.app" ;;
esac
EOF
  chmod +x "$temp_root/bin/hdiutil"

  cat > "$temp_root/bin/codesign" <<'EOF'
#!/usr/bin/env bash
exit "$SIGNATURE_EXIT"
EOF
  cat > "$temp_root/bin/plutil" <<'EOF'
#!/usr/bin/env bash
printf '0.2.2\n'
EOF
  cat > "$temp_root/bin/lipo" <<'EOF'
#!/usr/bin/env bash
printf 'arm64\n'
EOF
  chmod +x "$temp_root/bin/codesign" "$temp_root/bin/plutil" "$temp_root/bin/lipo"

  local actual_exit
  set +e
  PATH="$temp_root/bin:$PATH" TEST_ROOT="$temp_root" SOURCE_BUNDLE="$bundle/macos/LifeSub.app" \
    SIGNATURE_EXIT="$signature_exit" IMAGE_EXIT="$image_exit" BUNDLE_MISMATCH="$mismatch" \
    "$project/scripts/verify-desktop-asr.sh" dmg > "$temp_root/output" 2>&1
  actual_exit=$?
  set -e

  if [[ "$actual_exit" -ne "$expected_exit" ]]; then
    cat "$temp_root/output" >&2
    printf 'expected exit %s, got %s\n' "$expected_exit" "$actual_exit" >&2
    return 1
  fi
  if [[ "$expected_exit" == 0 ]]; then
    [[ "$(cat "$temp_root/selected-dmg")" == "$bundle/dmg/LifeSub_0.2.2_aarch64.dmg" ]]
  fi
}

run_dmg_case 1 0 0 1
run_dmg_case 0 1 0 1
run_dmg_case 0 0 1 1
run_dmg_case 0 0 0 0

legacy_root="$(mktemp -d)"
trap 'rm -rf "$legacy_root"' EXIT
mkdir -p "$legacy_root/scripts"
cp "$HARNESS" "$legacy_root/scripts/verify-desktop-asr.sh"
legacy_output="$($legacy_root/scripts/verify-desktop-asr.sh target 2>&1 || true)"
if [[ "$legacy_output" != *'not wired into the desktop executable'* ]]; then
  printf 'legacy target mode did not fail closed with a wiring explanation\n' >&2
  exit 1
fi

printf 'verify-desktop-asr DMG contract tests passed\n'
