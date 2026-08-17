.PHONY: check check-full fmt clippy diff help

# Default: run tier 2 pre-commit checks
check:
	@scripts/check.sh tier2

# Full suite (Task completion)
check-full:
	@scripts/check.sh tier3

# Quick focused test: make test name=asr_model_manager_test
test:
	@scripts/check.sh tier1 $(name)

fmt:
	@scripts/check.sh fmt

clippy:
	@scripts/check.sh clippy

diff:
	@scripts/check.sh diff

help:
	@echo "make check        Tier 2: pre-commit (fmt+clippy+diff+focused tests)"
	@echo "make check-full   Tier 3: full suite (Task completion)"
	@echo "make test name=X  Tier 1: single focused test module"
	@echo "make fmt          cargo fmt --check only"
	@echo "make clippy       cargo clippy -- -D warnings only"
	@echo "make diff         git diff --check only"