#!/bin/sh
set -eu

readonly SCRIPT_DIR="$(CDPATH= cd -- "$(/usr/bin/dirname "$0")" && pwd)"
readonly REPO_ROOT="$(/usr/bin/dirname "$SCRIPT_DIR")"
readonly MANIFEST_PATH="${REPO_ROOT}/src-tauri/Cargo.toml"
readonly TARGET_DIR="${CARGO_TARGET_DIR:-${REPO_ROOT}/src-tauri/target}"
readonly PREBUILT_ROOT="${TARGET_DIR}/sherpa-onnx-prebuilt"
readonly LOCK_DIR="${TARGET_DIR}/.lifesub-sherpa-runtime.lock"
readonly LOCK_WAIT_ATTEMPTS=300
readonly LOCK_WAIT_SECONDS=0.1

if [ "$#" -eq 0 ] || [ "$(/usr/bin/basename "$1")" != "cargo" ]; then
    >&2 /bin/echo "Usage: $0 cargo <arguments>"
    exit 64
fi

readonly CARGO_COMMAND="$1"
shift
readonly ARCHIVE_DIR="$(${SCRIPT_DIR}/fetch-sherpa-runtime.sh)"

/bin/mkdir -p "$TARGET_DIR"

lock_attempt=0
while ! /bin/mkdir "$LOCK_DIR" 2>/dev/null; do
    lock_attempt=$((lock_attempt + 1))
    if [ "$lock_attempt" -ge "$LOCK_WAIT_ATTEMPTS" ]; then
        >&2 /bin/echo "Timed out waiting for sherpa-onnx target lock: ${LOCK_DIR}"
        exit 75
    fi
    /bin/sleep "$LOCK_WAIT_SECONDS"
done

release_lock() {
    if [ -d "$LOCK_DIR" ]; then
        /bin/rmdir "$LOCK_DIR"
    fi
}

handle_signal() {
    trap - EXIT HUP INT TERM
    release_lock
    exit 130
}

trap release_lock EXIT
trap handle_signal HUP INT TERM

if [ -e "$PREBUILT_ROOT" ] || [ -L "$PREBUILT_ROOT" ]; then
    quarantine_path="${PREBUILT_ROOT}.quarantine.$(/bin/date '+%Y%m%d%H%M%S').$$"
    >&2 /bin/echo "Quarantining sherpa-onnx prebuilt cache at ${quarantine_path}"
    /bin/mv "$PREBUILT_ROOT" "$quarantine_path"
fi

export CARGO_TARGET_DIR="$TARGET_DIR"
export SHERPA_ONNX_ARCHIVE_DIR="$ARCHIVE_DIR"

"$CARGO_COMMAND" clean --manifest-path "$MANIFEST_PATH" -p sherpa-onnx-sys
"$CARGO_COMMAND" "$@"
