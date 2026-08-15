#!/bin/sh
set -eu

readonly SCRIPT_DIR="$(CDPATH= cd -- "$(/usr/bin/dirname "$0")" && pwd)"
readonly FETCH_SCRIPT="${SCRIPT_DIR}/fetch-sherpa-runtime.sh"
readonly ARCHIVE_STEM="sherpa-onnx-v1.13.5-osx-arm64-static-lib"
readonly ARCHIVE_SHA256="339c8fc19bb4b26e118c80792bbc4546eb263040fac36ef0cc027ec29c756b44"
readonly MARKER_NAME=".lifesub-archive-sha256"
readonly TEST_ROOT="$(/usr/bin/mktemp -d "${TMPDIR:-/tmp}/lifesub-sherpa-fetch-test.XXXXXX")"

cleanup() {
    /usr/bin/find "$TEST_ROOT" -depth -delete
}
trap cleanup EXIT HUP INT TERM

fail() {
    >&2 /bin/echo "$1"
    exit 1
}

seed_fake_prebuilt() {
    target_dir="$1"
    lib_dir="${target_dir}/sherpa-onnx-prebuilt/${ARCHIVE_STEM}/lib"
    /bin/mkdir -p "$lib_dir"
    /usr/bin/touch "${lib_dir}/libonnxruntime.a"
}

unmarked_target="${TEST_ROOT}/unmarked-target"
seed_fake_prebuilt "$unmarked_target"
LIFESUB_CARGO_TARGET_DIR="$unmarked_target" "$FETCH_SCRIPT" >/dev/null

unmarked_lib_dir="${unmarked_target}/sherpa-onnx-prebuilt/${ARCHIVE_STEM}/lib"
[ ! -e "$unmarked_lib_dir" ] || fail "unmarked prebuilt library cache was not quarantined"
quarantined_lib="$(/usr/bin/find "$unmarked_target" -path '*/sherpa-onnx-prebuilt.quarantine.*/*/lib/libonnxruntime.a' -print -quit)"
[ -n "$quarantined_lib" ] || fail "quarantine did not preserve the unmarked prebuilt cache"

mismatched_target="${TEST_ROOT}/mismatched-target"
seed_fake_prebuilt "$mismatched_target"
mismatched_prebuilt="${mismatched_target}/sherpa-onnx-prebuilt"
/bin/echo "deadbeef" >"${mismatched_prebuilt}/${MARKER_NAME}"
LIFESUB_CARGO_TARGET_DIR="$mismatched_target" "$FETCH_SCRIPT" >/dev/null

mismatched_lib_dir="${mismatched_prebuilt}/${ARCHIVE_STEM}/lib"
[ ! -e "$mismatched_lib_dir" ] || fail "mismatched prebuilt library cache was not quarantined"
quarantined_mismatched_lib="$(/usr/bin/find "$mismatched_target" -path '*/sherpa-onnx-prebuilt.quarantine.*/*/lib/libonnxruntime.a' -print -quit)"
[ -n "$quarantined_mismatched_lib" ] \
    || fail "quarantine did not preserve the mismatched prebuilt cache"

marked_target="${TEST_ROOT}/marked-target"
seed_fake_prebuilt "$marked_target"
marked_prebuilt="${marked_target}/sherpa-onnx-prebuilt"
/bin/echo "$ARCHIVE_SHA256" >"${marked_prebuilt}/${MARKER_NAME}"
LIFESUB_CARGO_TARGET_DIR="$marked_target" "$FETCH_SCRIPT" >/dev/null

marked_lib_dir="${marked_prebuilt}/${ARCHIVE_STEM}/lib"
[ -f "${marked_lib_dir}/libonnxruntime.a" ] || fail "matching marked prebuilt cache was not preserved"
[ "$(/bin/cat "${marked_prebuilt}/${MARKER_NAME}")" = "$ARCHIVE_SHA256" ] \
    || fail "matching cache marker changed unexpectedly"

/bin/echo "fetch-sherpa-runtime cache marker tests passed"
