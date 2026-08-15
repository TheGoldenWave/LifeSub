#!/bin/sh
set -eu

readonly SCRIPT_DIR="$(CDPATH= cd -- "$(/usr/bin/dirname "$0")" && pwd)"
readonly REPO_ROOT="$(/usr/bin/dirname "$SCRIPT_DIR")"
readonly MANIFEST_PATH="${REPO_ROOT}/src-tauri/Cargo.toml"

if [ "$#" -eq 0 ] || [ "$(/usr/bin/basename "$1")" != "cargo" ]; then
    >&2 /bin/echo "Usage: $0 cargo <arguments>"
    exit 64
fi

readonly CARGO_COMMAND="$1"
shift
readonly REQUESTED_TARGET_DIR="${CARGO_TARGET_DIR:-${REPO_ROOT}/src-tauri/target}"
readonly ARCHIVE_DIR="$(${SCRIPT_DIR}/fetch-sherpa-runtime.sh)"

/bin/mkdir -p "$REQUESTED_TARGET_DIR"
readonly TARGET_DIR="$(CDPATH= cd -- "$REQUESTED_TARGET_DIR" && pwd -P)"
readonly PREBUILT_ROOT="${TARGET_DIR}/sherpa-onnx-prebuilt"
readonly LOCK_DIR="${TARGET_DIR}/.lifesub-sherpa-runtime.lock"
readonly LOCK_OWNER_PID_FILE="${LOCK_DIR}/owner.pid"
readonly LOCK_WAIT_ATTEMPTS="${LIFESUB_SHERPA_LOCK_WAIT_ATTEMPTS:-900}"
readonly LOCK_WAIT_SECONDS="${LIFESUB_SHERPA_LOCK_POLL_SECONDS:-1}"
readonly MISSING_PID_GRACE_SECONDS="${LIFESUB_SHERPA_MISSING_PID_GRACE_SECONDS:-5}"

release_lock() {
    if [ -f "$LOCK_OWNER_PID_FILE" ] && [ "$(/bin/cat "$LOCK_OWNER_PID_FILE")" = "$$" ]; then
        /bin/rm -f "$LOCK_OWNER_PID_FILE"
        /bin/rmdir "$LOCK_DIR"
    fi
}

remove_scoped_stale_lock() {
    stale_lock_path="$1"
    case "$stale_lock_path" in
        "$TARGET_DIR"/.lifesub-sherpa-runtime.lock.stale.*) ;;
        *)
            >&2 /bin/echo "Refusing to remove unscoped stale lock path: ${stale_lock_path}"
            return 1
            ;;
    esac

    if [ -e "$stale_lock_path" ] || [ -L "$stale_lock_path" ]; then
        /usr/bin/find "$stale_lock_path" -depth -delete
    fi
}

lock_owner_is_alive() {
    [ -f "$LOCK_OWNER_PID_FILE" ] || return 1
    lock_owner_pid="$(/bin/cat "$LOCK_OWNER_PID_FILE")"
    case "$lock_owner_pid" in
        '' | *[!0-9]*) return 1 ;;
    esac
    /bin/kill -0 "$lock_owner_pid" 2>/dev/null
}

lock_without_live_owner_is_reclaimable() {
    if [ -f "$LOCK_OWNER_PID_FILE" ]; then
        lock_owner_pid="$(/bin/cat "$LOCK_OWNER_PID_FILE")"
        case "$lock_owner_pid" in
            '' | *[!0-9]*) ;;
            *)
                if /bin/kill -0 "$lock_owner_pid" 2>/dev/null; then
                    return 1
                fi
                return 0
                ;;
        esac
    fi

    lock_modified_at="$(/usr/bin/stat -f '%m' "$LOCK_DIR" 2>/dev/null)" || return 0
    lock_age="$(( $(/bin/date '+%s') - lock_modified_at ))"
    [ "$lock_age" -ge "$MISSING_PID_GRACE_SECONDS" ]
}

reclaim_stale_lock() {
    stale_lock_path="${LOCK_DIR}.stale.$(/bin/date '+%Y%m%d%H%M%S').$$.${lock_attempt}"
    if /bin/mv "$LOCK_DIR" "$stale_lock_path" 2>/dev/null; then
        >&2 /bin/echo "Reclaimed stale sherpa-onnx target lock: ${stale_lock_path}"
        remove_scoped_stale_lock "$stale_lock_path"
        return 0
    fi
    return 1
}

acquire_lock() {
    lock_attempt=0
    while ! /bin/mkdir "$LOCK_DIR" 2>/dev/null; do
        if ! lock_owner_is_alive && lock_without_live_owner_is_reclaimable; then
            reclaim_stale_lock || true
            continue
        fi

        lock_attempt=$((lock_attempt + 1))
        if [ "$lock_attempt" -ge "$LOCK_WAIT_ATTEMPTS" ]; then
            >&2 /bin/echo "Timed out waiting for sherpa-onnx target lock: ${LOCK_DIR}"
            exit 75
        fi
        /bin/sleep "$LOCK_WAIT_SECONDS"
    done

    /bin/echo "$$" >"$LOCK_OWNER_PID_FILE"
}

remove_scoped_quarantine() {
    [ -n "$quarantine_path" ] || return 0
    case "$quarantine_path" in
        "$TARGET_DIR"/sherpa-onnx-prebuilt.quarantine.*) ;;
        *)
            >&2 /bin/echo "Refusing to remove unscoped quarantine path: ${quarantine_path}"
            return 1
            ;;
    esac

    if [ -e "$quarantine_path" ] || [ -L "$quarantine_path" ]; then
        /usr/bin/find "$quarantine_path" -depth -delete
    fi
}

cleanup_on_exit() {
    command_status=$?
    trap - EXIT HUP INT TERM
    cleanup_status=0
    remove_scoped_quarantine || cleanup_status=$?
    release_lock || cleanup_status=$?
    if [ "$command_status" -ne 0 ]; then
        exit "$command_status"
    fi
    exit "$cleanup_status"
}

quarantine_path=""
acquire_lock
trap cleanup_on_exit EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

if [ -e "$PREBUILT_ROOT" ] || [ -L "$PREBUILT_ROOT" ]; then
    quarantine_path="${PREBUILT_ROOT}.quarantine.$(/bin/date '+%Y%m%d%H%M%S').$$"
    >&2 /bin/echo "Quarantining sherpa-onnx prebuilt cache at ${quarantine_path}"
    /bin/mv "$PREBUILT_ROOT" "$quarantine_path"
fi

export CARGO_TARGET_DIR="$TARGET_DIR"
export SHERPA_ONNX_ARCHIVE_DIR="$ARCHIVE_DIR"

"$CARGO_COMMAND" clean --manifest-path "$MANIFEST_PATH" -p sherpa-onnx-sys
"$CARGO_COMMAND" "$@"
