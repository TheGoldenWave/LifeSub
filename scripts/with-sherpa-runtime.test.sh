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
readonly RESOLVED_TARGET_DIR="$(CDPATH= cd -- "$TARGET_DIR" && pwd -P)"
/usr/bin/touch "${TARGET_DIR}/sherpa-onnx-prebuilt/${ARCHIVE_STEM}/lib/fake-matching-marker.a"
/bin/echo "$ARCHIVE_SHA256" >"${TARGET_DIR}/sherpa-onnx-prebuilt/.lifesub-archive-sha256"

/bin/cat >"${STUB_BIN}/cargo" <<'STUB'
#!/bin/sh
set -eu

: "${CARGO_TARGET_DIR:?}"
: "${SHERPA_ONNX_ARCHIVE_DIR:?}"
: "${TEST_EXPECTED_TARGET_DIR:?}"
: "${TEST_STUB_LOG:?}"
: "${TEST_STUB_SLEEP_SECONDS:?}"
: "${TEST_STUB_FAIL_RUN:?}"

[ "$CARGO_TARGET_DIR" = "$TEST_EXPECTED_TARGET_DIR" ] || exit 92
[ -f "${SHERPA_ONNX_ARCHIVE_DIR}/sherpa-onnx-v1.13.5-osx-arm64-static-lib.tar.bz2" ] || exit 93
owner_pid_file="${CARGO_TARGET_DIR}/.lifesub-sherpa-runtime.lock/owner.pid"
[ -f "$owner_pid_file" ] || exit 95
owner_pid="$(/bin/cat "$owner_pid_file")"
case "$owner_pid" in
    '' | *[!0-9]*) exit 96 ;;
esac
/bin/kill -0 "$owner_pid" 2>/dev/null || exit 97

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
        /bin/sleep "$TEST_STUB_SLEEP_SECONDS"
        ;;
    *)
        /bin/echo "run|${CARGO_TARGET_DIR}|${SHERPA_ONNX_ARCHIVE_DIR}" >>"$TEST_STUB_LOG"
        /bin/mkdir -p "${CARGO_TARGET_DIR}/sherpa-onnx-prebuilt/generated/lib"
        /usr/bin/touch "${CARGO_TARGET_DIR}/sherpa-onnx-prebuilt/generated/lib/generated.a"
        /bin/sleep "$TEST_STUB_SLEEP_SECONDS"
        [ "$TEST_STUB_FAIL_RUN" -eq 0 ] || exit 98
        ;;
esac
STUB
chmod +x "${STUB_BIN}/cargo"

run_wrapper_at() {
    wrapper_target="$1"
    expected_target="$2"
    stub_sleep_seconds="$3"
    stub_fail_run="${4:-0}"
    CARGO_TARGET_DIR="$wrapper_target" \
        LIFESUB_SHERPA_LOCK_POLL_SECONDS=0.05 \
        LIFESUB_SHERPA_LOCK_WAIT_ATTEMPTS=900 \
        LIFESUB_SHERPA_MISSING_PID_GRACE_SECONDS=1 \
        TEST_EXPECTED_TARGET_DIR="$expected_target" \
        TEST_STUB_LOG="$STUB_LOG" \
        TEST_STUB_SLEEP_SECONDS="$stub_sleep_seconds" \
        TEST_STUB_FAIL_RUN="$stub_fail_run" \
        PATH="${STUB_BIN}:${PATH}" \
        "$WRAPPER_SCRIPT" cargo test
}

dead_target="${TEST_ROOT}/dead-owner-target"
dead_lock="${dead_target}/.lifesub-sherpa-runtime.lock"
/bin/mkdir -p "$dead_lock"
/bin/echo "999999" >"${dead_lock}/owner.pid"
readonly RESOLVED_DEAD_TARGET="$(CDPATH= cd -- "$dead_target" && pwd -P)"
run_wrapper_at "$dead_target" "$RESOLVED_DEAD_TARGET" 0.05 >"${TEST_ROOT}/dead.out" 2>&1 &
dead_wrapper_pid=$!
watchdog_attempt=0
while /bin/kill -0 "$dead_wrapper_pid" 2>/dev/null && [ "$watchdog_attempt" -lt 75 ]; do
    watchdog_attempt=$((watchdog_attempt + 1))
    /bin/sleep 0.02
done
if /bin/kill -0 "$dead_wrapper_pid" 2>/dev/null; then
    /bin/kill -TERM "$dead_wrapper_pid" 2>/dev/null || true
    wait "$dead_wrapper_pid" 2>/dev/null || true
    /bin/cat "${TEST_ROOT}/dead.out" >&2
    fail "dead owner lock was not reclaimed within the watchdog window"
fi
set +e
wait "$dead_wrapper_pid"
dead_status=$?
set -e
[ "$dead_status" -eq 0 ] || {
    /bin/cat "${TEST_ROOT}/dead.out" >&2
    fail "dead owner recovery failed with status ${dead_status}"
}
[ ! -d "$dead_lock" ] || fail "recovered dead-owner lock was not released"
dead_stale_count="$(
    /usr/bin/find "$dead_target" -maxdepth 1 -type d -name '.lifesub-sherpa-runtime.lock.stale.*' -print \
        | /usr/bin/wc -l \
        | /usr/bin/tr -d ' '
)"
[ "$dead_stale_count" -eq 0 ] || fail "recovered dead-owner lock quarantine was not cleaned"

missing_pid_target="${TEST_ROOT}/missing-pid-target"
missing_pid_lock="${missing_pid_target}/.lifesub-sherpa-runtime.lock"
/bin/mkdir -p "$missing_pid_lock"
readonly RESOLVED_MISSING_PID_TARGET="$(CDPATH= cd -- "$missing_pid_target" && pwd -P)"
missing_pid_started_at="$(/bin/date '+%s')"
run_wrapper_at "$missing_pid_target" "$RESOLVED_MISSING_PID_TARGET" 0.05 \
    >"${TEST_ROOT}/missing-pid.out" 2>&1
missing_pid_elapsed="$(( $(/bin/date '+%s') - missing_pid_started_at ))"
[ "$missing_pid_elapsed" -ge 1 ] || fail "missing-PID lock was reclaimed before its mtime grace"
[ ! -d "$missing_pid_lock" ] || fail "missing-PID lock was not reclaimed and released"

concurrent_started_at="$(/bin/date '+%s')"
set +e
run_wrapper_at "$TARGET_DIR" "$RESOLVED_TARGET_DIR" 1 >"${TEST_ROOT}/first.out" 2>&1 &
first_pid=$!
run_wrapper_at "$TARGET_DIR" "$RESOLVED_TARGET_DIR" 1 >"${TEST_ROOT}/second.out" 2>&1 &
second_pid=$!
wait "$first_pid"
first_status=$?
wait "$second_pid"
second_status=$?
set -e
concurrent_elapsed="$(( $(/bin/date '+%s') - concurrent_started_at ))"

if [ "$first_status" -ne 0 ]; then
    /bin/cat "${TEST_ROOT}/first.out" >&2
    fail "first concurrent wrapper failed with status ${first_status}"
fi
if [ "$second_status" -ne 0 ]; then
    /bin/cat "${TEST_ROOT}/second.out" >&2
    fail "second concurrent wrapper failed with status ${second_status}"
fi
[ "$(/usr/bin/grep -c '^clean|' "$STUB_LOG")" -eq 4 ] || fail "wrapper did not clean before every command"
[ "$(/usr/bin/grep -c '^run|' "$STUB_LOG")" -eq 4 ] || fail "wrapper did not execute every command"
if /usr/bin/grep -q '^overlap$' "$STUB_LOG"; then
    fail "concurrent wrapper commands were not serialized"
fi
# A 0.05-second test poll makes this cover at least 60 production-equivalent wait intervals.
[ "$concurrent_elapsed" -ge 3 ] \
    || fail "healthy owner was stolen instead of waiting for the long-running command"

[ ! -e "${TARGET_DIR}/sherpa-onnx-prebuilt/${ARCHIVE_STEM}/lib/fake-matching-marker.a" ] \
    || fail "matching-marker fake cache was trusted"
quarantine_count="$(
    /usr/bin/find "$TARGET_DIR" -maxdepth 1 -type d -name 'sherpa-onnx-prebuilt.quarantine.*' -print \
        | /usr/bin/wc -l \
        | /usr/bin/tr -d ' '
)"
[ "$quarantine_count" -eq 0 ] || fail "successful wrappers leaked ${quarantine_count} quarantine directories"
[ ! -d "${TARGET_DIR}/.lifesub-sherpa-runtime.lock" ] || fail "wrapper lock was not released"

failure_target="${TEST_ROOT}/failure-target"
/bin/mkdir -p "${failure_target}/sherpa-onnx-prebuilt/fake/lib"
/usr/bin/touch "${failure_target}/sherpa-onnx-prebuilt/fake/lib/fake.a"
readonly RESOLVED_FAILURE_TARGET="$(CDPATH= cd -- "$failure_target" && pwd -P)"
set +e
run_wrapper_at "$failure_target" "$RESOLVED_FAILURE_TARGET" 0.05 1 \
    >"${TEST_ROOT}/failure.out" 2>&1
failure_status=$?
set -e
[ "$failure_status" -eq 98 ] || {
    /bin/cat "${TEST_ROOT}/failure.out" >&2
    fail "failing wrapped command status changed to ${failure_status}"
}
failure_quarantine_count="$(
    /usr/bin/find "$failure_target" -maxdepth 1 -type d -name 'sherpa-onnx-prebuilt.quarantine.*' -print \
        | /usr/bin/wc -l \
        | /usr/bin/tr -d ' '
)"
[ "$failure_quarantine_count" -eq 0 ] \
    || fail "failing wrapper leaked ${failure_quarantine_count} quarantine directories"
[ ! -d "${failure_target}/.lifesub-sherpa-runtime.lock" ] || fail "failing wrapper lock was not released"

/bin/echo "with-sherpa-runtime locking and quarantine tests passed"
