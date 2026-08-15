#!/bin/sh
set -eu

readonly RUNTIME_VERSION="1.13.5"
readonly ARCHIVE_NAME="sherpa-onnx-v${RUNTIME_VERSION}-osx-arm64-static-lib.tar.bz2"
readonly ARCHIVE_URL="https://github.com/k2-fsa/sherpa-onnx/releases/download/v${RUNTIME_VERSION}/${ARCHIVE_NAME}"
readonly ARCHIVE_SIZE="19862746"
readonly ARCHIVE_SHA256="339c8fc19bb4b26e118c80792bbc4546eb263040fac36ef0cc027ec29c756b44"
readonly CACHE_ROOT="${XDG_CACHE_HOME:-${HOME}/Library/Caches}/lifesub/sherpa-onnx/v${RUNTIME_VERSION}"
readonly ARCHIVE_PATH="${CACHE_ROOT}/${ARCHIVE_NAME}"

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

/bin/echo "$CACHE_ROOT"
