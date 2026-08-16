#!/bin/sh
set -eu

readonly SCRIPT_DIR="$(CDPATH= cd -- "$(/usr/bin/dirname "$0")" && pwd)"
readonly FETCH_SCRIPT="${SCRIPT_DIR}/fetch-sherpa-runtime.sh"
readonly ARCHIVE_NAME="sherpa-onnx-v1.13.5-osx-arm64-static-lib.tar.bz2"
readonly ARCHIVE_SIZE="19862746"
readonly ARCHIVE_SHA256="339c8fc19bb4b26e118c80792bbc4546eb263040fac36ef0cc027ec29c756b44"
readonly ATTESTATION_NAME=".lifesub-sherpa-runtime-attestation-v1"
readonly TEST_ROOT="$(/usr/bin/mktemp -d "${TMPDIR:-/tmp}/lifesub-sherpa-fetch-test.XXXXXX")"
readonly TARGET_DIR="${TEST_ROOT}/target"
readonly FAKE_LIB="${TARGET_DIR}/sherpa-onnx-prebuilt/fake/lib/fake.a"

cleanup() {
    /usr/bin/find "$TEST_ROOT" -depth -delete
}
trap cleanup EXIT HUP INT TERM

fail() {
    >&2 /bin/echo "$1"
    exit 1
}

expected_attestation() {
    /usr/bin/printf '%s\n' \
        'schema=lifesub.sherpa-runtime-attestation.v1' \
        'version=1.13.5' \
        'git_commit=3dc7c569f31ca2cd4a20ed6f7db780327e6714c5' \
        "archive_name=${ARCHIVE_NAME}" \
        "archive_size=${ARCHIVE_SIZE}" \
        "archive_sha256=${ARCHIVE_SHA256}" \
        'build_id=sherpa-onnx-v1.13.5-osx-arm64-static-lib'
}

/bin/mkdir -p "$(/usr/bin/dirname "$FAKE_LIB")"
/usr/bin/touch "$FAKE_LIB"

archive_dir="$(
    CARGO_TARGET_DIR="$TARGET_DIR" \
        "$FETCH_SCRIPT"
)"

[ -f "$FAKE_LIB" ] || fail "archive fetch unexpectedly modified the Cargo target cache"
[ -f "${archive_dir}/${ARCHIVE_NAME}" ] || fail "verified archive path was not returned"
[ "$(/usr/bin/stat -f '%z' "${archive_dir}/${ARCHIVE_NAME}")" = "$ARCHIVE_SIZE" ] \
    || fail "verified archive size changed"
[ "$(/usr/bin/shasum -a 256 "${archive_dir}/${ARCHIVE_NAME}" | /usr/bin/awk '{ print $1 }')" = "$ARCHIVE_SHA256" ] \
    || fail "verified archive SHA-256 changed"
[ -f "${archive_dir}/${ATTESTATION_NAME}" ] || fail "runtime attestation was not created"
[ "$(/bin/cat "${archive_dir}/${ATTESTATION_NAME}")" = "$(expected_attestation)" ] \
    || fail "runtime attestation content changed"

/bin/echo 'forged-attestation' >"${archive_dir}/${ATTESTATION_NAME}"
repaired_archive_dir="$(CARGO_TARGET_DIR="$TARGET_DIR" "$FETCH_SCRIPT")"
[ "$repaired_archive_dir" = "$archive_dir" ] || fail "attestation repair changed cache root"
[ "$(/bin/cat "${archive_dir}/${ATTESTATION_NAME}")" = "$(expected_attestation)" ] \
    || fail "runtime attestation was not repaired"

/bin/echo "fetch-sherpa-runtime archive-only tests passed"
