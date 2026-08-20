#!/usr/bin/env bash
# LifeSub ASR Quality Gate — wrapper script that:
# 1. Fetches/verifies the native sherpa-onnx archive
# 2. Verifies expected real tests appear in cargo test -- --list
# 3. Runs the lifesub-asr-gate binary
# 4. Parses results and fails unless every scenario passes
# 5. Supports --verify-existing mode to validate committed JSON
#
# Usage:
#   LIFESUB_ASR_MODEL_DIR=/path/to/models scripts/verify-asr-gate.sh
#   scripts/verify-asr-gate.sh --verify-existing

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
MANIFEST_DIR="$PROJECT_ROOT/src-tauri"
OUTPUT_DIR="$PROJECT_ROOT/output/asr-v0.2"
RESULTS_FILE="$OUTPUT_DIR/fixture-results.json"
SCOPE_FILE="$PROJECT_ROOT/scripts/asr-gate-scope.txt"

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info()  { echo -e "${GREEN}[INFO]${NC}  $*"; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*"; }

# ---------------------------------------------------------------------------
# Parse arguments
# ---------------------------------------------------------------------------

VERIFY_EXISTING=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --verify-existing)
            VERIFY_EXISTING=true
            shift
            ;;
        *)
            log_error "Unknown argument: $1"
            echo "Usage: $0 [--verify-existing]"
            exit 1
            ;;
    esac
done

# ---------------------------------------------------------------------------
# Verify scoped files are clean relative to HEAD
# ---------------------------------------------------------------------------

log_info "Checking scoped files are clean relative to HEAD..."

if ! git -C "$PROJECT_ROOT" diff --quiet HEAD --; then
    log_error "Working tree has uncommitted changes. Commit or stash before running the Gate."
    git -C "$PROJECT_ROOT" diff --stat HEAD --
    exit 1
fi

# Check that only evidence/docs paths are allowed to differ from tested_commit
# (enforced in --verify-existing mode; for fresh runs, everything must be clean)

# ---------------------------------------------------------------------------
# Verify scope file exists and is non-empty
# ---------------------------------------------------------------------------

if [[ ! -f "$SCOPE_FILE" ]]; then
    log_error "Scope file not found: $SCOPE_FILE"
    exit 1
fi

SCOPE_COUNT=$(grep -c -v '^#' "$SCOPE_FILE" | grep -c -v '^$' || true)
if [[ "$SCOPE_COUNT" -eq 0 ]]; then
    log_error "Scope file is empty: $SCOPE_FILE"
    exit 1
fi
log_info "Scope file contains $SCOPE_COUNT paths"

# ---------------------------------------------------------------------------
# --verify-existing mode: validate committed JSON without running models
# ---------------------------------------------------------------------------

if [[ "$VERIFY_EXISTING" == true ]]; then
    log_info "=== Verify Existing Mode ==="

    if [[ ! -f "$RESULTS_FILE" ]]; then
        log_error "Results file not found: $RESULTS_FILE"
        exit 1
    fi

    # Check that results are valid JSON
    if ! python3 -c "import json; json.load(open('$RESULTS_FILE'))" 2>/dev/null; then
        log_error "Results file is not valid JSON: $RESULTS_FILE"
        exit 1
    fi

    # Check that all_pass is true
    if ! python3 -c "
import json
data = json.load(open('$RESULTS_FILE'))
if not data.get('all_pass'):
    print('FAIL: all_pass is false')
    failures = data.get('failures', [])
    for f in failures:
        print(f'  - {f}')
    exit(1)
print('PASS: all gate scenarios passed')
" 2>/dev/null; then
        log_error "Existing results do not pass the Gate"
        exit 1
    fi

    # Check that tested_commit is an ancestor of HEAD
    TESTED_COMMIT=$(python3 -c "import json; print(json.load(open('$RESULTS_FILE'))['tested_commit'])" 2>/dev/null)
    if ! git -C "$PROJECT_ROOT" merge-base --is-ancestor "$TESTED_COMMIT" HEAD; then
        log_error "tested_commit ($TESTED_COMMIT) is not an ancestor of HEAD"
        exit 1
    fi

    # Verify scoped source digest matches current files
    # (recompute from scope file and compare with recorded digest)
    CURRENT_DIGEST=$(cd "$PROJECT_ROOT" && python3 -c "
import hashlib, os

scope_file = 'scripts/asr-gate-scope.txt'
paths = []
with open(scope_file) as f:
    for line in f:
        line = line.strip()
        if line and not line.startswith('#'):
            paths.append(line)

paths.sort()
combined = hashlib.sha256()
for p in paths:
    try:
        with open(p, 'rb') as f:
            h = hashlib.sha256(f.read()).hexdigest()
    except Exception as e:
        h = f'ERROR:{p}:{e}'
    combined.update(f'{p}:{h}\n'.encode())

print(combined.hexdigest())
" 2>/dev/null)

    RECORDED_DIGEST=$(python3 -c "import json; print(json.load(open('$RESULTS_FILE'))['scoped_source_digest'])" 2>/dev/null)

    if [[ "$CURRENT_DIGEST" != "$RECORDED_DIGEST" ]]; then
        log_error "Scoped source digest mismatch: current=$CURRENT_DIGEST, recorded=$RECORDED_DIGEST"
        exit 1
    fi

    log_info "Existing results verified: all_pass=true, digest matches, commit is ancestor"
    exit 0
fi

# ---------------------------------------------------------------------------
# Fresh run mode
# ---------------------------------------------------------------------------

# Verify expected real tests exist
log_info "Verifying expected real tests..."
REAL_TESTS=$(cd "$MANIFEST_DIR" && cargo test --features asr-runtime -- --list 2>&1 || true)

# Check that the real model tests are listed
REQUIRED_TESTS=(
    "sense_voice_zh_cer_gate"
    "whisper_en_wer_gate"
    "whisper_zh_en_key_phrase_gate"
)

MISSING_TESTS=()
for test_name in "${REQUIRED_TESTS[@]}"; do
    if ! echo "$REAL_TESTS" | grep -q "$test_name"; then
        MISSING_TESTS+=("$test_name")
    fi
done

if [[ ${#MISSING_TESTS[@]} -gt 0 ]]; then
    log_error "Missing required tests in cargo test -- --list:"
    for t in "${MISSING_TESTS[@]}"; do
        echo "  - $t"
    done
    exit 1
fi

# Count total real tests
REAL_TEST_COUNT=$(echo "$REAL_TESTS" | grep -c "real_model_tests::" || true)
if [[ "$REAL_TEST_COUNT" -eq 0 ]]; then
    log_error "Zero real model tests found in cargo test -- --list"
    exit 1
fi
log_info "Found $REAL_TEST_COUNT real model tests"

# Verify LIFESUB_ASR_MODEL_DIR is set
if [[ -z "${LIFESUB_ASR_MODEL_DIR:-}" ]]; then
    log_error "LIFESUB_ASR_MODEL_DIR is not set"
    echo "Set it to the directory containing installed ASR models, e.g.:"
    echo "  export LIFESUB_ASR_MODEL_DIR=\"\$HOME/Library/Application Support/com.goldenwave.lifesub/models\""
    exit 1
fi

if [[ ! -d "$LIFESUB_ASR_MODEL_DIR" ]]; then
    log_error "LIFESUB_ASR_MODEL_DIR does not exist: $LIFESUB_ASR_MODEL_DIR"
    exit 1
fi

log_info "Model directory: $LIFESUB_ASR_MODEL_DIR"

# Fetch and verify native archive if SHERPA_ONNX_ARCHIVE_DIR not set
if [[ -z "${SHERPA_ONNX_ARCHIVE_DIR:-}" ]]; then
    FETCH_SCRIPT="$PROJECT_ROOT/scripts/fetch-sherpa-runtime.sh"
    if [[ -f "$FETCH_SCRIPT" ]]; then
        log_info "Fetching native sherpa-onnx archive..."
        SHERPA_ONNX_ARCHIVE_DIR=$("$FETCH_SCRIPT")
        export SHERPA_ONNX_ARCHIVE_DIR
        log_info "Native archive directory: $SHERPA_ONNX_ARCHIVE_DIR"
    else
        log_warn "fetch-sherpa-runtime.sh not found; skipping native archive fetch"
    fi
fi

# Build the gate binary
log_info "Building lifesub-asr-gate..."
cd "$MANIFEST_DIR"
cargo build --features asr-runtime --bin lifesub-asr-gate 2>&1 | tail -5

# Run the gate binary
log_info "Running lifesub-asr-gate..."
GATE_EXIT=0
LIFESUB_ASR_MODEL_DIR="$LIFESUB_ASR_MODEL_DIR" \
    cargo run --features asr-runtime --bin lifesub-asr-gate -- \
    --output "$RESULTS_FILE" \
    --model-dir "$LIFESUB_ASR_MODEL_DIR" 2>&1 || GATE_EXIT=$?

# Parse results
if [[ $GATE_EXIT -ne 0 ]]; then
    log_error "Gate binary exited with code $GATE_EXIT"
fi

if [[ ! -f "$RESULTS_FILE" ]]; then
    log_error "Results file was not created: $RESULTS_FILE"
    exit 1
fi

# Verify results JSON structure
if ! python3 -c "
import json, sys

with open('$RESULTS_FILE') as f:
    data = json.load(f)

required = ['tested_commit', 'scoped_source_digest', 'executable_hash',
            'runtime_version', 'runtime_git_sha1', 'fixture_hashes',
            'metrics', 'all_pass', 'failures']
for key in required:
    if key not in data:
        print(f'FAIL: missing required field: {key}')
        sys.exit(1)

if not data['metrics']:
    print('FAIL: no metrics in results')
    sys.exit(1)

if not data['all_pass']:
    print('FAIL: all_pass is false')
    for f in data.get('failures', []):
        print(f'  - {f}')
    sys.exit(1)

# Verify every expected scenario exists
fixture_ids = set(m['fixture_id'] for m in data['metrics'])
expected = {'zh-mandarin', 'en-english', 'zh-en-mixed'}
missing = expected - fixture_ids
if missing:
    print(f'FAIL: missing fixture scenarios: {missing}')
    sys.exit(1)

print('PASS: all gate scenarios passed')
for m in data['metrics']:
    print(f'  {m[\"fixture_id\"]}/{m[\"provider\"]}: CER={m[\"cer\"]:.4f} WER={m[\"wer\"]:.4f} all_pass={m[\"all_pass\"]}')
" 2>/dev/null; then
    log_error "Results verification failed"
    exit 1
fi

log_info "=== Gate Passed ==="
log_info "Results written to $RESULTS_FILE"
exit 0