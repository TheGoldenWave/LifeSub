#!/bin/sh
set -eu

readonly SCRIPT_DIR="$(CDPATH= cd -- "$(/usr/bin/dirname "$0")" && pwd)"
readonly WRAPPER_SCRIPT="${SCRIPT_DIR}/with-sherpa-runtime.sh"
readonly ARCHIVE_STEM="sherpa-onnx-v1.13.5-osx-arm64-static-lib"
readonly ARCHIVE_SHA256="339c8fc19bb4b26e118c80792bbc4546eb263040fac36ef0cc027ec29c756b44"
readonly TEST_ROOT="$(/usr/bin/mktemp -d "${TMPDIR:-/tmp}/lifesub-sherpa-wrapper-test.XXXXXX")"
readonly TARGET_DIR="${TEST_ROOT}/custom-target"
readonly STUB_BIN="${TEST_ROOT}/bin"
readonly STUB_LOG="${TEST_ROOT}/cargo.log"

cleanup() {
    /usr/bin/find "$TEST_ROOT" -depth -delete
}
trap cleanup EXIT HUP INT TERM

fail() {
    >&2 /bin/echo "$1"
    exit 1
}

/bin/mkdir -p "${TARGET_DIR}/sherpa-onnx-prebuilt/${ARCHIVE_STEM}/lib" "$STUB_BIN"
/usr/bin/touch "${TARGET_DIR}/sherpa-onnx-prebuilt/${ARCHIVE_STEM}/lib/fake-matching-marker.a"
/bin/echo "$ARCHIVE_SHA256" >"${TARGET_DIR}/sherpa-onnx-prebuilt/.lifesub-archive-sha256"

/bin/cat >"${STUB_BIN}/cargo" <<'STUB'
#!/bin/sh
set -eu

: "${CARGO_TARGET_DIR:?}"
: "${SHERPA_ONNX_ARCHIVE_DIR:?}"
: "${TEST_EXPECTED_TARGET_DIR:?}"
: "${TEST_STUB_LOG:?}"

[ "$CARGO_TARGET_DIR" = "$TEST_EXPECTED_TARGET_DIR" ] || exit 92
[ -f "${SHERPA_ONNX_ARCHIVE_DIR}/sherpa-onnx-v1.13.5-osx-arm64-static-lib.tar.bz2" ] || exit 93

guard_dir="${CARGO_TARGET_DIR}/.stub-cargo-active"
if ! /bin/mkdir "$guard_dir" 2>/dev/null; then
    /bin/echo "overlap" >>"$TEST_STUB_LOG"
    exit 94
fi
cleanup_guard() {
    /bin/rmdir "$guard_dir"
}
trap cleanup_guard EXIT HUP INT TERM

case "${1:-}" in
    clean)
        /bin/echo "clean|${CARGO_TARGET_DIR}|${SHERPA_ONNX_ARCHIVE_DIR}" >>"$TEST_STUB_LOG"
        /bin/sleep 1
        ;;
    *)
        /bin/echo "run|${CARGO_TARGET_DIR}|${SHERPA_ONNX_ARCHIVE_DIR}" >>"$TEST_STUB_LOG"
        /bin/mkdir -p "${CARGO_TARGET_DIR}/sherpa-onnx-prebuilt/generated/lib"
        /usr/bin/touch "${CARGO_TARGET_DIR}/sherpa-onnx-prebuilt/generated/lib/generated.a"
        /bin/sleep 1
        ;;
esac
STUB
chmod +x "${STUB_BIN}/cargo"

run_wrapper() {
    CARGO_TARGET_DIR="$TARGET_DIR" \
        TEST_EXPECTED_TARGET_DIR="$TARGET_DIR" \
        TEST_STUB_LOG="$STUB_LOG" \
        PATH="${STUB_BIN}:${PATH}" \
        "$WRAPPER_SCRIPT" cargo test
}

set +e
run_wrapper >"${TEST_ROOT}/first.out" 2>&1 &
first_pid=$!
run_wrapper >"${TEST_ROOT}/second.out" 2>&1 &
second_pid=$!
wait "$first_pid"
first_status=$?
wait "$second_pid"
second_status=$?
set -e

if [ "$first_status" -ne 0 ]; then
    /bin/cat "${TEST_ROOT}/first.out" >&2
    fail "first concurrent wrapper failed with status ${first_status}"
fi
if [ "$second_status" -ne 0 ]; then
    /bin/cat "${TEST_ROOT}/second.out" >&2
    fail "second concurrent wrapper failed with status ${second_status}"
fi
[ "$(/usr/bin/grep -c '^clean|' "$STUB_LOG")" -eq 2 ] || fail "wrapper did not clean before both commands"
[ "$(/usr/bin/grep -c '^run|' "$STUB_LOG")" -eq 2 ] || fail "wrapper did not execute both commands"
if /usr/bin/grep -q '^overlap$' "$STUB_LOG"; then
    fail "concurrent wrapper commands were not serialized"
fi

quarantined_fake="$(/usr/bin/find "$TARGET_DIR" -path '*/sherpa-onnx-prebuilt.quarantine.*/*/lib/fake-matching-marker.a' -print -quit)"
[ -n "$quarantined_fake" ] || fail "matching-marker fake cache was trusted instead of quarantined"
[ ! -d "${TARGET_DIR}/.lifesub-sherpa-runtime.lock" ] || fail "wrapper lock was not released"

/bin/echo "with-sherpa-runtime locking and quarantine tests passed"
