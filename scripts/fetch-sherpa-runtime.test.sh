#!/bin/sh
set -eu

readonly SCRIPT_DIR="$(CDPATH= cd -- "$(/usr/bin/dirname "$0")" && pwd)"
readonly FETCH_SCRIPT="${SCRIPT_DIR}/fetch-sherpa-runtime.sh"
readonly ARCHIVE_NAME="sherpa-onnx-v1.13.5-osx-arm64-static-lib.tar.bz2"
readonly ARCHIVE_SIZE="19862746"
readonly ARCHIVE_SHA256="339c8fc19bb4b26e118c80792bbc4546eb263040fac36ef0cc027ec29c756b44"
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

/bin/echo "fetch-sherpa-runtime archive-only tests passed"
