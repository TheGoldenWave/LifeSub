# Reference-Led Release Scope Splitting

## Background

On 2026-08-27, the LifeSub 0.2.1 local-ASR release had already completed most code-facing release-hardening work: settings truth, diagnostics, exact-UID probe, same-UID recovery, durable model operations, verifier v3, and packaged provider gates. The remaining pressure came from reference-project comparison and from the temptation to keep pulling in more "nice to have" settings, workflow, and audio features.

The project already had a stronger evidence and identity model than the reference products. Reopening architecture for parity would have increased release risk instead of reducing it.

## Decision

Split reference-led follow-up into three layers:

1. `0.2.1` only closes the signed packaged primary path:
   - real hardware/privacy report matrix;
   - release marker and release-source verification;
   - DMG/install proof;
   - only the release-blocking fixes exposed by packaged evidence.
2. `0.2.2` is a lightweight stabilization release:
   - settings/capture/model/job/diagnostics clarity;
   - probe age and re-test affordance;
   - clearer recovery and operation wording;
   - startup/first-frame/RTF/RSS/cancel performance baselines.
3. `0.3+` carries product-surface and evidence-graph expansion:
   - context-specific profiles;
   - hotkeys/general behavior settings;
   - storage recovery/backup/retention;
   - AEC, transcript deduplication, preview, normalization;
   - throughput experiments and streaming paths.

## Admission Rule

A reference-project detail may still enter the current release only if it fixes one of these:

- false success or false-ready state;
- identity, permission, or provenance ambiguity;
- silent frame loss or fake recovery;
- packaged failure that is not diagnosable or recoverable;
- inability to prove the signed local recording-to-Evidence path.

If a proposed refinement does not fix one of those, it moves out of `0.2.1`.

## Consequences

- Completed contracts are not reopened merely because a reference product offers broader settings or workflow coverage.
- The immediate next version defaults to light UX/operability improvements over proven contracts, not to Tasks 9-16.
- Heavy items that change data shape, evidence semantics, or runtime behavior require their own design and acceptance gates.
- `0.2.2` should be considered "done enough" when it stays inside four bounded work classes: IA/readability, state clarity, operator affordances over existing facts, and baseline measurement. If a proposal needs schema changes, new helper/process boundaries, new publication semantics, or extra acceptance-matrix categories, it no longer belongs to `0.2.2`.
