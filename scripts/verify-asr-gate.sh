#!/bin/sh
set -eu

# LifeSub ASR Gate verifier.
#
# Usage:
#   scripts/verify-asr-gate.sh [--verify-existing] [--model-dir <path>] [--qwen17-model-dir <path>]
#
# Defaults read LIFESUB_ASR_MODEL_DIR for the installed model cache path.
# With --verify-existing, only validate the committed JSON without running models.

readonly SCRIPT_DIR="$(CDPATH= cd -- "$(/usr/bin/dirname "$0")" && pwd)"
readonly REPO_ROOT="$(/usr/bin/dirname "$SCRIPT_DIR")"
readonly MANIFEST_PATH="${REPO_ROOT}/tests/fixtures/asr/fixture-manifest.json"
readonly SCOPE_PATH="${REPO_ROOT}/scripts/asr-gate-scope.txt"
readonly RESULT_PATH="${REPO_ROOT}/output/asr-v0.2/fixture-results.json"

VERIFY_EXISTING=0
MODEL_DIR="${LIFESUB_ASR_MODEL_DIR:-}"
QWEN17_MODEL_DIR=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        --verify-existing)
            VERIFY_EXISTING=1
            shift
            ;;
        --model-dir)
            MODEL_DIR="$2"
            shift 2
            ;;
        --qwen17-model-dir)
            QWEN17_MODEL_DIR="$2"
            shift 2
            ;;
        *)
            >&2 /bin/echo "Unknown argument: $1"
            exit 64
            ;;
    esac
done

# Validate every scoped path is clean relative to HEAD.
if ! /usr/bin/git -C "$REPO_ROOT" diff --quiet -- $(/bin/cat "$SCOPE_PATH"); then
    >&2 /bin/echo "error: a scoped source/fixture path is dirty; commit before running the Gate"
    exit 1
fi

if [ "$VERIFY_EXISTING" -eq 1 ]; then
    if [ ! -f "$RESULT_PATH" ]; then
        >&2 /bin/echo "error: no committed fixture-results.json to verify"
        exit 1
    fi
    /bin/echo "Gate: verifying existing evidence ${RESULT_PATH}"
    # The committed result must contain a tested_commit and a non-empty scenario list.
    /usr/bin/python3 - "$RESULT_PATH" <<'PY'
import json, sys
with open(sys.argv[1]) as f:
    data = json.load(f)
assert data.get("tested_commit"), "missing tested_commit"
assert data.get("scenarios"), "missing scenarios"
print("Gate: committed evidence structure valid")
PY
    exit 0
fi

if [ -z "$MODEL_DIR" ]; then
    >&2 /bin/echo "error: --model-dir or LIFESUB_ASR_MODEL_DIR is required to run the Gate"
    exit 64
fi

# Build the Gate binary with the pinned sherpa runtime.
readonly CARGO_BIN="${REPO_ROOT}/src-tauri/target/release/lifesub-asr-gate"
"${SCRIPT_DIR}/with-sherpa-runtime.sh" cargo build --release \
    --manifest-path "${REPO_ROOT}/src-tauri/Cargo.toml" \
    --bin lifesub-asr-gate

# Run the Gate.
readonly GATE_ARGS="--fixtures ${MANIFEST_PATH} --model-dir ${MODEL_DIR} --output ${RESULT_PATH}"
if [ -n "$QWEN17_MODEL_DIR" ]; then
    "$CARGO_BIN" "$GATE_ARGS" --qwen17-model-dir "$QWEN17_MODEL_DIR"
else
    "$CARGO_BIN" "$GATE_ARGS"
fi

/bin/echo "Gate: evidence written to ${RESULT_PATH}"