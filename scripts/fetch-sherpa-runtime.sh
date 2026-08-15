#!/bin/sh
set -eu

readonly RUNTIME_VERSION="1.13.5"
readonly ARCHIVE_NAME="sherpa-onnx-v${RUNTIME_VERSION}-osx-arm64-static-lib.tar.bz2"
readonly ARCHIVE_URL="https://github.com/k2-fsa/sherpa-onnx/releases/download/v${RUNTIME_VERSION}/${ARCHIVE_NAME}"
readonly ARCHIVE_SIZE="19862746"
readonly ARCHIVE_SHA256="339c8fc19bb4b26e118c80792bbc4546eb263040fac36ef0cc027ec29c756b44"
readonly SCRIPT_DIR="$(CDPATH= cd -- "$(/usr/bin/dirname "$0")" && pwd)"
readonly REPO_ROOT="$(/usr/bin/dirname "$SCRIPT_DIR")"
readonly CACHE_ROOT="${XDG_CACHE_HOME:-${HOME}/Library/Caches}/lifesub/sherpa-onnx/v${RUNTIME_VERSION}"
readonly ARCHIVE_PATH="${CACHE_ROOT}/${ARCHIVE_NAME}"
readonly CARGO_TARGET_DIR="${LIFESUB_CARGO_TARGET_DIR:-${REPO_ROOT}/src-tauri/target}"
readonly PREBUILT_ROOT="${CARGO_TARGET_DIR}/sherpa-onnx-prebuilt"
readonly EXTRACTED_LIB_DIR="${PREBUILT_ROOT}/sherpa-onnx-v${RUNTIME_VERSION}-osx-arm64-static-lib/lib"
readonly PREBUILT_MARKER="${PREBUILT_ROOT}/.lifesub-archive-sha256"

archive_size() {
    /usr/bin/stat -f '%z' "$1"
}

archive_sha256() {
    /usr/bin/shasum -a 256 "$1" | /usr/bin/awk '{ print $1 }'
}

archive_is_valid() {
    [ -f "$1" ] \
        && [ "$(archive_size "$1")" = "$ARCHIVE_SIZE" ] \
        && [ "$(archive_sha256 "$1")" = "$ARCHIVE_SHA256" ]
}

prebuilt_marker_is_valid() {
    [ -f "$PREBUILT_MARKER" ] \
        && [ "$(/bin/cat "$PREBUILT_MARKER")" = "$ARCHIVE_SHA256" ]
}

quarantine_unverified_prebuilt() {
    prebuilt_was_quarantined=false
    if [ -e "$PREBUILT_ROOT" ] && ! prebuilt_marker_is_valid; then
        quarantine_path="${PREBUILT_ROOT}.quarantine.$(/bin/date '+%Y%m%d%H%M%S').$$"
        >&2 /bin/echo "Quarantining unverified sherpa-onnx prebuilt cache at ${quarantine_path}"
        /bin/mv "$PREBUILT_ROOT" "$quarantine_path"
        prebuilt_was_quarantined=true
    fi

    if ! prebuilt_marker_is_valid; then
        /bin/mkdir -p "$PREBUILT_ROOT"
        temporary_marker="$(/usr/bin/mktemp "${PREBUILT_ROOT}/.lifesub-archive-sha256.XXXXXX")"
        trap '/bin/rm -f "$temporary_marker"' EXIT HUP INT TERM
        /bin/echo "$ARCHIVE_SHA256" >"$temporary_marker"
        /bin/mv -f "$temporary_marker" "$PREBUILT_MARKER"
        trap - EXIT HUP INT TERM
    fi

    [ "$prebuilt_was_quarantined" = false ] || [ ! -e "$EXTRACTED_LIB_DIR" ] || {
        >&2 /bin/echo "Unverified sherpa-onnx extracted library path remains after quarantine"
        exit 1
    }
}

/bin/mkdir -p "$CACHE_ROOT"

if ! archive_is_valid "$ARCHIVE_PATH"; then
    temporary_archive="$(/usr/bin/mktemp "${CACHE_ROOT}/.${ARCHIVE_NAME}.XXXXXX")"
    trap '/bin/rm -f "$temporary_archive"' EXIT HUP INT TERM

    >&2 /bin/echo "Downloading sherpa-onnx ${RUNTIME_VERSION} static runtime"
    /usr/bin/curl --fail --location --silent --show-error \
        --output "$temporary_archive" "$ARCHIVE_URL"

    if ! archive_is_valid "$temporary_archive"; then
        >&2 /bin/echo "Downloaded sherpa-onnx archive failed size or SHA-256 verification"
        exit 1
    fi

    /bin/mv -f "$temporary_archive" "$ARCHIVE_PATH"
    trap - EXIT HUP INT TERM
fi

quarantine_unverified_prebuilt

/bin/echo "$CACHE_ROOT"
