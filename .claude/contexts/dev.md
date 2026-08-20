# Development Mode Context (dev.md)

You are in **Active Development Mode**.
Your goal is to write clean, maintainable, and testable code.

## Core Directives for Development

1. **Risk-Matched Verification**: Read `docs/testing-and-review-policy.md` first. Behavior, data, security, and API-contract changes require a valid RED/GREEN loop. Pure visual, documentation, configuration, and generated-artifact changes use the repeatable evidence defined by the policy instead of forced unit tests.

2. **Layered Testing Strategy** (replaces mandatory full-suite every change):
   - **Tier 1 — Development loop**: Run only the focused test module for the current task. Use `scripts/check.sh tier1 <test_name>` or `cargo test <test_name> --features <relevant>`.
   - **Tier 2 — Pre-commit**: Run focused + related module tests + fmt + clippy + diff check. Use `scripts/check.sh tier2`.
   - **Tier 3 — Task completion**: Run the full suite (`cargo test --all-features`), only once at the end of a Task before review.

3. **Single Combined Review** (replaces separate spec + quality review):
   - One reviewer performs both spec-conformance and code-quality checks in a single pass.
   - Gate: **Critical = 0** to pass. Critical means data corruption, permission bypass, identity forgery, unsafe publish, or unrecoverable security failure.
   - Important findings are addressed in the same PR but do not block the review gate; they may be deferred to the next Task with evidence recorded in `notes.md`.
   - Minor findings are recorded in `notes.md` without blocking.
   - Each finding records: severity, file/line, violation, and minimal fix. Verification commands are optional for Important/Minor.

4. **Pre-commit Automation**: Before committing, run `scripts/check.sh tier2` (or `make check`). This aggregates fmt, clippy, diff check, and focused tests into one command.

5. **Evidence Placement**: Keep module tests near the module, cross-module user journeys in `tests/specs/`, and visual evidence in `output/playwright/`. Mixed changes split behavior and visual verification.

6. **Design as Code**: Never hardcode colors, spacing, or typography. Pull the latest variables from `docs/design/tokens/`.

7. **Log Your Experience**: If you encounter a novel error or a tricky bug, log it in the `notes.md` of the current feature folder. Routine findings from the review process do not need to be logged.
