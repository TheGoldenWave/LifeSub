#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

pass()  { echo -e "${GREEN}[PASS]${NC} $*"; }
fail()  { echo -e "${RED}[FAIL]${NC} $*"; exit 1; }
info()  { echo -e "${YELLOW}[INFO]${NC} $*"; }

usage() {
  echo "Usage: $0 <tier1 <test_name> | tier2 | tier3 | fmt | clippy | diff>"
  echo ""
  echo "  tier1 <name>  Run a single focused test module (fast dev loop)"
  echo "  tier2         Pre-commit check: focused+related tests + fmt + clippy + diff"
  echo "  tier3         Full suite: cargo test --all-features (Task completion only)"
  echo "  fmt           cargo fmt --check only"
  echo "  clippy        cargo clippy -- -D warnings only"
  echo "  diff          git diff --check only"
  exit 1
}

# ---- cargo wrapper for native builds ----
cargo_cmd() {
  if [ -f scripts/with-sherpa-runtime.sh ]; then
    scripts/with-sherpa-runtime.sh cargo "$@"
  else
    cargo "$@"
  fi
}

# ---- individual checks ----
check_fmt() {
  info "Running cargo fmt --check..."
  cargo fmt --check || fail "cargo fmt --check"
  pass "fmt"
}

check_clippy() {
  info "Running cargo clippy -- -D warnings..."
  cargo_cmd clippy --all-targets --all-features -- -D warnings || fail "clippy"
  pass "clippy"
}

check_diff() {
  info "Running git diff --check..."
  git diff --check || fail "git diff --check"
  pass "diff"
}

# ---- tiered test runs ----
run_tier1() {
  local test_name="$1"
  info "Tier 1: running focused test '${test_name}'..."
  cargo_cmd test "${test_name}" --features "asr-runtime,asr-qwen17-runtime" -- --nocapture || fail "tier1 test '${test_name}'"
  pass "tier1: ${test_name}"
}

run_tier2() {
  info "Tier 2: pre-commit checks..."
  check_fmt
  check_clippy
  check_diff
  info "Tier 2: running focused + related tests..."
  cargo_cmd test --features "asr-runtime,asr-qwen17-runtime" -- --nocapture || fail "tier2 tests"
  pass "tier2: all checks passed"
}

run_tier3() {
  info "Tier 3: full suite (Task completion)..."
  cargo_cmd test --all-features -- --nocapture || fail "tier3 full suite"
  pass "tier3: full suite passed"
}

# ---- main ----
case "${1:-}" in
  tier1)
    [ $# -ge 2 ] || { echo "tier1 requires a test name"; usage; }
    run_tier1 "$2"
    ;;
  tier2)
    run_tier2
    ;;
  tier3)
    run_tier3
    ;;
  fmt)
    check_fmt
    ;;
  clippy)
    check_clippy
    ;;
  diff)
    check_diff
    ;;
  *)
    usage
    ;;
esac