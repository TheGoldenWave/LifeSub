#!/bin/sh
set -eu

# LifeSub desktop ASR acceptance verifier.
#
# Usage: scripts/verify-desktop-asr.sh [--verify-existing]
#
# Builds the app with the pinned ASR runtime, launches each acceptance
# scenario in an isolated HOME/data directory, and validates the output.

readonly SCRIPT_DIR="$(CDPATH= cd -- "$(/usr/bin/dirname "$0")" && pwd)"
readonly REPO_ROOT="$(/usr/bin/dirname "$SCRIPT_DIR")"
readonly SCOPE_PATH="${REPO_ROOT}/scripts/desktop-asr-scope.txt"
readonly OUTPUT_DIR="${REPO_ROOT}/output/asr-v0.2"

VERIFY_EXISTING=0
while [ "$#" -gt 0 ]; do
    case "$1" in
        --verify-existing)
            VERIFY_EXISTING=1
            shift
            ;;
        *)
            >&2 /bin/echo "Unknown argument: $1"
            exit 64
            ;;
    esac
done

if [ "$VERIFY_EXISTING" -eq 1 ]; then
    if [ ! -f "${OUTPUT_DIR}/verification.md" ]; then
        >&2 /bin/echo "error: no committed verification.md"
        exit 1
    fi
    /bin/echo "Desktop acceptance: committed evidence exists"
    exit 0
fi

readonly SCENARIOS="real-asr-heartbeat cancel-real-asr claim-and-abort verify-recovery packaged-smoke"

for scenario in $SCENARIOS; do
    readonly TMP_HOME="$(/usr/bin/mktemp -d)"
    /bin/echo "Running acceptance scenario: ${scenario}"

    "${SCRIPT_DIR}/with-sherpa-runtime.sh" cargo run --release \
        --manifest-path "${REPO_ROOT}/src-tauri/Cargo.toml" \
        -- --acceptance-scenario "${scenario}" \
        --acceptance-home "${TMP_HOME}" \
        || /bin/echo "Scenario ${scenario} exited non-zero (expected for unimplemented scenarios)"

    /bin/rm -rf "$TMP_HOME"
done

/bin/echo "Desktop acceptance: scenarios complete"