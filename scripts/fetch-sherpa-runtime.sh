#!/bin/sh
set -eu

readonly RUNTIME_VERSION="1.13.5"
readonly ARCHIVE_NAME="sherpa-onnx-v${RUNTIME_VERSION}-osx-arm64-static-lib.tar.bz2"
readonly ARCHIVE_URL="https://github.com/k2-fsa/sherpa-onnx/releases/download/v${RUNTIME_VERSION}/${ARCHIVE_NAME}"
readonly ARCHIVE_SIZE="19862746"
readonly ARCHIVE_SHA256="339c8fc19bb4b26e118c80792bbc4546eb263040fac36ef0cc027ec29c756b44"
readonly RUNTIME_GIT_COMMIT="3dc7c569f31ca2cd4a20ed6f7db780327e6714c5"
readonly BUILD_ID="sherpa-onnx-v1.13.5-osx-arm64-static-lib"
readonly CACHE_ROOT="${XDG_CACHE_HOME:-${HOME}/Library/Caches}/lifesub/sherpa-onnx/v${RUNTIME_VERSION}"
readonly ARCHIVE_PATH="${CACHE_ROOT}/${ARCHIVE_NAME}"
readonly ATTESTATION_PATH="${CACHE_ROOT}/.lifesub-sherpa-runtime-attestation-v1"

archive_size() {
    /usr/bin/stat -f '%z' "$1"
}

archive_sha256() {
    /usr/bin/shasum -a 256 "$1" | /usr/bin/awk '{ print $1 }'
}

archive_is_valid() {
    [ ! -L "$1" ] \
        && [ -f "$1" ] \
        && [ "$(archive_size "$1")" = "$ARCHIVE_SIZE" ] \
        && [ "$(archive_sha256 "$1")" = "$ARCHIVE_SHA256" ]
}

attestation_payload() {
    /usr/bin/printf '%s\n' \
        'schema=lifesub.sherpa-runtime-attestation.v1' \
        "version=${RUNTIME_VERSION}" \
        "git_commit=${RUNTIME_GIT_COMMIT}" \
        "archive_name=${ARCHIVE_NAME}" \
        "archive_size=${ARCHIVE_SIZE}" \
        "archive_sha256=${ARCHIVE_SHA256}" \
        "build_id=${BUILD_ID}"
}

attestation_is_valid() {
    [ ! -L "$ATTESTATION_PATH" ] \
        && [ -f "$ATTESTATION_PATH" ] \
        && [ "$(/bin/cat "$ATTESTATION_PATH")" = "$(attestation_payload)" ]
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

if ! attestation_is_valid; then
    temporary_attestation="$(/usr/bin/mktemp "${CACHE_ROOT}/.lifesub-sherpa-runtime-attestation-v1.XXXXXX")"
    trap '/bin/rm -f "$temporary_attestation"' EXIT HUP INT TERM
    attestation_payload >"$temporary_attestation"
    /bin/mv -f "$temporary_attestation" "$ATTESTATION_PATH"
    trap - EXIT HUP INT TERM
fi

/bin/echo "$CACHE_ROOT"
