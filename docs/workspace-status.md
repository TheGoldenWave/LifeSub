# LifeSub Workspace Status

Last verified: 2026-08-22

## Current consolidation state

- The divergent worktrees have been consolidated through a reviewed merge workflow. The final canonical branch is `main` in the primary project path.
- The integrated source imported through the merge is `652abf720e67765b53de56714a14965fb4cfbcf0` from `codex/lifesub-real-asr-v0.2`.
- Tasks 1--6 of the approved native-capture plan are implemented, reviewed and committed.
- Task 7 production Catalog v6/atomic sealing remains unfinished. Continue from the integrated process records, not from the old external-worktree path.
- Do not package, install or replace `/Applications/LifeSub.app` until Tasks 7--12 and hardware acceptance pass.

## Integrated release source contract

- Release source worktree: `/Users/goldenwave/Documents/MyProject/LifeSub`
- Release source branch: `main`
- Release version: `0.2.1`
- Verification: run `npm run verify:release-source`. Until native capture and native ASR are production-selected, local planning audits may use `LIFESUB_RELEASE_ALLOW_PLANNED=1 npm run verify:release-source`; that mode is explicitly not release evidence.

## Canonical integrated development and release source

- Worktree: `/Users/goldenwave/Documents/MyProject/LifeSub`
- Branch: `main`
- Merge inputs: preserved main workspace `dc62d5b` plus integrated source `652abf7`.
- Current milestone: workspace consolidation complete; native-capture Tasks 1--6 complete; Task 7 remains the next implementation milestone.

## Currently installed app

- Installed bundle: `/Applications/LifeSub.app`
- Bundle version: `0.2.1`
- Executable SHA-256: `6b8b7e17f4f509a95434560141b05793f31503530405dc242e8b1b7e620d8f8c`

The installed app predates the Task 1--6 native-capture integration commits. It is not evidence that ScreenCaptureKit/AVAudioEngine capture or authenticated helper supervision works in the installed product. Do not use the installed bundle as the source for further development.

## Retained rollback workspace

- Worktree: `/Users/goldenwave/.config/superpowers/worktrees/LifeSub/lifesub-real-asr-v0.2`
- Branch: `codex/lifesub-real-asr-v0.2`
- Frozen integrated source: `652abf7`.
- Purpose: rollback and patch comparison only. Do not continue product development or build releases there after consolidation.

## Removed worktree directories

The clean Task 7 and Task 9 worktree directories were removed on 2026-08-21. Their branch refs remain:

- `codex/lifesub-task7-audio-prep` at `99e1af3`
- `codex/lifesub-task9-jobs` at `4493de6`

They can be recreated if patch-level comparison is needed. Do not delete the branch refs until integration is complete.

## Capability truth

| Capability | Current status |
|---|---|
| Four-page desktop UI, Timeline, Dictionary, Settings, Notes | Implemented in the canonical functional worktree |
| Catalog v5 and durable UI CRUD | Implemented |
| ASR schemas, manifests, model manager, job fencing, receipts, providers, quality Gate infrastructure | Implemented at module/test level |
| Imported-audio production ASR executor | Not release-verified; canonical worker currently uses `FailClosedEngine` |
| Versioned Rust/Swift capture protocol | Implemented and reviewed; shared fixture, bounds, replay/sequence and PCM validation pass |
| ScreenCaptureKit system-audio helper | Implemented and reviewed; not yet connected to production coordinator |
| AVAudioEngine microphone helper | Implemented and reviewed; not yet connected to production coordinator |
| Helper build/bundle/signing | Implemented and reviewed; arm64 sidecar uses fixed identifier and compile-time SHA attestation |
| Helper authentication/supervision | Implemented and reviewed; inherited-FD nonce, UID/PID/path/file identity, replay and timeout gates pass |
| Catalog v6 capture timing and atomic chunk sealing | Not implemented; Task 7 is paused at a failing migration test |
| Production NativeCaptureCoordinator | Not implemented; current production capture path remains fail closed |
| Demo fallback in desktop production | Intentionally blocked |

After the 2026-08-22 consolidation, `main` at the primary project path is the only development/release source. Semantic version strings, old DMGs, `/Applications/LifeSub.app`, or the retained rollback worktree must not override this contract.

## Required pre-build check

Before building or installing LifeSub:

1. Read this file and every feature `process.md`.
2. Run `git worktree list` and `git status --short` in every retained worktree.
3. State the exact source worktree, branch, commit, and dirty-file count.
4. Verify production wiring, not only module presence or test totals.
5. Build with the verified Sherpa runtime attestation.
6. Compare source and installed executable SHA-256 values.
7. Verify the installed bundle version and signature.
8. Launch the installed app normally and capture a screenshot proving the expected UI.

Do not infer "latest" from `main`, a semantic version string, commit time, or an existing bundle directory alone.
