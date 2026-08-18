# Universal Coding Style (Always-Apply)

1. **Immutability First**: Prefer immutable data structures. Avoid mutating state where possible.
2. **File Organization**: Keep files reasonably sized (under 600 lines for Rust, under 400 lines for TypeScript). Extract complex logic into helper modules when approaching the limit. Exceeding the limit is a Minor note, not a blocker.
3. **No Magic Numbers**: Avoid hardcoding numbers or strings in the logic; define them as constants at the top of the file.
4. **Agentic Comments**: Write comments that explain *WHY* something was done, not *WHAT* was done (the code explains the 'what').
