# LifeSub Workspace Status

Last verified: 2026-08-21

## Integrated release source contract

- Release source worktree: `/Users/goldenwave/.config/superpowers/worktrees/LifeSub/lifesub-real-asr-v0.2`
- Release source branch: `codex/lifesub-real-asr-v0.2`
- Release version: `0.2.1`
- Verification: run `npm run verify:release-source`. Until native capture and native ASR are production-selected, local planning audits may use `LIFESUB_RELEASE_ALLOW_PLANNED=1 npm run verify:release-source`; that mode is explicitly not release evidence.

## Canonical source for the currently installed app

- Worktree: `/Users/goldenwave/.config/superpowers/worktrees/LifeSub/lifesub-real-asr-v0.2`
- Branch: `codex/lifesub-real-asr-v0.2`
- Committed base: `cce0320`
- State: contains substantial uncommitted UI, Catalog, worker, safety, and verification work. Do not delete, reset, or treat it as disposable.
- Installed bundle: `/Applications/LifeSub.app`
- Bundle version: `0.2.1`
- Executable SHA-256: `6b8b7e17f4f509a95434560141b05793f31503530405dc242e8b1b7e620d8f8c`

This is the most functionally complete runnable workspace currently available, but it is not a clean release branch.

## Other retained workspace

- Worktree: `/Users/goldenwave/Documents/MyProject/LifeSub`
- Branch: `main`
- Committed base: `03f49eb`
- State: divergent V0.2.1 ASR implementation plus local startup-fix changes.
- Do not use it as the release source without integration. Its current desktop wiring still contains production stubs (`job: None`, model management/retry/retranscribe stubs, and disabled test modules).

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
| Real-time microphone/system audio capture | Not implemented; no ScreenCaptureKit/AVAudioEngine production adapter is wired |
| Demo fallback in desktop production | Intentionally blocked |

The statement "V0.2.1 15 tasks are complete on main" describes a task implementation report, not the current integrated product state. It must not be used as release evidence without checking production command wiring and packaged behavior.

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
