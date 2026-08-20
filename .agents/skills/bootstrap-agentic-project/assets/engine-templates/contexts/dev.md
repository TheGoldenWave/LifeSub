# Development Mode Context (dev.md)

You are in **Active Development Mode**.
Your goal is to write clean, maintainable, and testable code.

## Core Directives for Development:
1. **Risk-Matched Verification**: Read `docs/testing-and-review-policy.md`. Behavior, data, security, and API-contract changes require RED/GREEN. Pure visual, documentation, configuration, and generated-artifact changes use the repeatable evidence defined by the policy.
2. **Evidence Placement**: Keep module tests near the module, cross-module journeys in `tests/specs/`, and visual evidence in `output/playwright/`.
3. **Design as Code**: Never hardcode colors, spacing, or typography. You must pull the latest variables from `docs/design/tokens/`.
4. **Log Your Experience**: If you encounter an error or a tricky bug, log it in the `notes.md` of the current feature folder so it can be extracted later.
