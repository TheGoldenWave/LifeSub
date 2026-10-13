# LifeSub Workspace Status

Last verified: 2026-10-13

## 2026-10-13 LifeSub 0.2.2 development source

- Release source worktree: `/Users/goldenwave/Documents/MyProject/LifeSub`
- Release source branch: `main`
- Release version: `0.2.2`
- The current 0.2.2 bundle was rebuilt from `main@28a284aea356b1cf2d153e545ed33bcd0dc6f080` after the verified Cargo runner became portable. The old 0.2.1 source plus S1-S3 product changes were committed and pushed; local `main` and `origin/main` both contain the candidate source. Later documentation commits may advance HEAD without changing these binaries. The 2026-10-13 branch cleanup section below describes the pre-development state.
- Current candidate SHA-256: App `3f72dde30c27afa1659b669aca4c1424d5f305cd024e7c790a3e6e866b9b5f26`, helper `5c7b4672fc9da848d900526337d2932efba0b0c8e9196991586acd60949e8e01`, Gate `c49bf64ca22be1857bee8b5e75251b690562776c3fc8dc14985246c05a913e0d`, DMG `52a4748719fecddc4498de56fb9b3d2e548ddf391e562d1bffa8d41452166318` at `src-tauri/target/aarch64-apple-darwin/release/bundle/dmg/LifeSub_0.2.2_aarch64.dmg`. App/helper/Gate hashes stayed identical across the rebuild; the DMG image hash changed.
- Release-source production wiring, Tier 2 `583/0/7`, Tier 3 `626/0/9`, Swift `75/75`, frontend tests and build, browser Settings screenshots, DMG checksum/mount/arm64/nested strict signature and bundle binary equality passed. Packaged SenseVoice/Whisper real-model Gate passed twice against fixed fixtures under this exact Gate identity. Details and limitations: `docs/performance/lifesub-0.2.2-baseline.md`.
- This build uses ad hoc signing and was not notarized; it is a local candidate, not a public macOS distribution artifact. At the 2026-10-13 13:19 read-only audit, `/Applications/LifeSub.app` was unexpectedly already version 0.2.2 with exact App/helper/Gate byte equality and a valid strict nested signature. It had been 0.2.1 earlier in this task. No installation command was issued by this task; the replacement's initiator is unconfirmed. Treat it as an **installed candidate, not a release-closed app**. A candidate-bound isolated-HOME startup-to-visible-window baseline is complete (488.704/314.803/303.810 ms); real first-frame/cancel timing and 20 new candidate-bound native capture reports are missing. The valid 0.2.1 reports cannot be relabeled as this build.
- User confirmed the intended release includes the local closed loop and GitHub Release. GitHub has a **draft prerelease** `v0.2.2-rc.1`, targeted at the exact build commit, with the candidate DMG uploaded; GitHub reports asset digest `sha256:52a4748719fecddc4498de56fb9b3d2e548ddf391e562d1bffa8d41452166318`, equal to the local DMG. It remains unpublished. This Mac has `0` valid code-signing identities, so Developer ID signing/notarization and the new candidate-bound native matrix remain release blockers.

## 2026-10-13 branch and worktree cleanup

- Fetched and pruned `origin`; `main` and `origin/main` both point to `db39faafeac0e8f4816623456f3ffa8b1ab54564` (`git rev-list --left-right --count main...origin/main` = `0 0`). There is no newer remote commit to merge.
- The only registered LifeSub worktree is `/Users/goldenwave/Documents/MyProject/LifeSub` on `main`. `git worktree prune` found no stale registration. No new checkout was created because it would start from the committed tree and omit the current uncommitted product work.
- Removed five local branches already merged into `main`: `codex/integration-premerge-20260822`, `codex/lifesub-real-asr-v0.2`, `codex/main-premerge-20260822`, `codex/merge-lifesub-v0.2.1`, and `feature/lifesub-real-asr-v0.2.1`.
- Two old task branches had commits outside `main`; before removing the visible branches, preserved their exact tips as Git refs: `refs/archive/branch-cleanup-20261013/lifesub-task7-audio-prep` → `99e1af3ced55ad5419324215e01a65902c37c065` and `refs/archive/branch-cleanup-20261013/lifesub-task9-jobs` → `4493de6ac88a3d58ac26e426a374a95e9a6eabba`. Restore only if specifically needed; these are historical task commits, not the next development source.
- The current worktree remains intentionally dirty: at this audit, `git status --porcelain=v1 --untracked-files=normal` reports 160 entries (98 modified, 62 untracked directory-collapsed entries). Branch cleanup did not switch, reset, stash, or remove those files. The working tree contains the latest LifeSub changes beyond `db39faa`; `origin/main` alone does not reproduce them.
- **Continuation point:** use this existing `main` worktree for the next iteration and perform a focused status/diff review before editing. Once the current product tree is intentionally validated and committed, new isolated worktrees can branch from that new commit. Build/install still requires the release-source and production-wiring checks below.

## Authoritative current progress

- `0.2.1` remains release-closed for backend scope. A frontend-only UI refresh was packaged and installed on 2026-09-03 without changing Tauri/Rust/Swift, Catalog, service contracts or business state machines.
- `0.2.2` stabilization is in development/acceptance within S1-S4 from `docs/superpowers/plans/2026-08-26-lifesub-asr-audio-settings-reference-optimization.md`.
- Task 6 and Task 9-16 remain deferred. They are not current blockers and must not be inferred as unfinished `0.2.1` work.
- The obsolete clean rollback worktree was removed on 2026-09-02 after confirming `652abf7` is an ancestor of `main` with no branch-only commits. Its remaining branch pointer was removed in the 2026-10-13 cleanup above.
- Sections below the release-closure entry are chronological history. Where an older entry says acceptance was blocked or incomplete, the newer release-closure entry supersedes it.

## 2026-09-03 frontend UI refresh package

- Release source worktree: `/Users/goldenwave/Documents/MyProject/LifeSub`
- Release source branch: `main`
- Release version: `0.2.1`
- Source commit: `db39faafeac0e8f4816623456f3ffa8b1ab54564`; packaging-time dirty count `150`, current post-documentation dirty count `151`. The package intentionally includes the reviewed frontend tree and the pre-existing release-closed native baseline.
- Release-source verification passed before packaging; production wiring gate remained sealed and passed.
- Installed app SHA-256: `b1fead3d084edf0075455ae64f8d4c61a8b6ca53b430e3c186ae498cefb928cf`.
- Installed helper SHA-256: `5c7b4672fc9da848d900526337d2932efba0b0c8e9196991586acd60949e8e01`.
- Installed gate SHA-256: `e49f4ba8ff709a940f4caa150699ed4b15002c1d546221010db10406b93df62e`.
- DMG: `src-tauri/target/release/bundle/dmg/LifeSub_0.2.1_aarch64.dmg`, SHA-256 `7d663985c404f4797fd4ad0a65a415edd3999faaca328b58436cccdbb9e04466`.
- App/DMG verification: version `0.2.1`, arm64 app/helper/gate, nested strict signature, build/DMG/installed executable byte equality and `hdiutil verify` all passed.
- `/Applications/LifeSub.app` was replaced and launched normally; previous install retained at `/tmp/LifeSub.pre-ui-20260903121134.app`.
- A second installed executable launch exited `0` with the original process count remaining `1`, preserving the single-instance contract.
- UI evidence: Vitest `150/150`, Playwright `37/37`, responsive/visual set `24/24`, Tier 2 `583/0/7`, combined review `Critical 0 / Important 0 / Minor 0`.
- Installed screenshot: `output/playwright/lifesub-0.2.1-ui-refresh-installed.png`.

## 2026-09-02 LifeSub 0.2.1 release closure

- Release source remains `/Users/goldenwave/Documents/MyProject/LifeSub`, `main@db39faafeac0e8f4816623456f3ffa8b1ab54564`. The former rollback worktree at `codex/lifesub-real-asr-v0.2@652abf720e67765b53de56714a14965fb4cfbcf0` was not used to build or install and has now been removed; its branch pointer remains.
- Scope is closed at Task 1-5 and Task 7-8 only. Task 6 and Task 9-16 remain deferred.
- Frozen acceptance/install identity: app `6f2e65235de68c98b14bbed127dbb9762e90773d46685d97c7f162bf3903cf4a`, helper `5c7b4672fc9da848d900526337d2932efba0b0c8e9196991586acd60949e8e01`, gate `c1bafe9a101382cba688f67ec84a9d50c5e76e496afe1162bc3b8c45961d714d`, source digest `6d5c9bfa897307cb9a74c551c52afd0bb08037eee0eabef319e59b98d9c5c4ef`.
- Final native acceptance directory `output/asr-v0.2/native-capture-v3-6f2e/reports/` passes all `20/20` scenarios. The 30-minute dual-source report is `1948.339625s`, 65 contiguous chunks per source, `209ms` drift, 0 dropped frames and `570.094MiB` peak RSS. Real sleep/wake also proved generation-fenced same-UID recovery and post-recovery ASR.
- Formal release-source verification passes after the evidence-gated Native ASR marker. Fresh gates: Swift `75/75`, frontend `125/125`, Tier 2 `583/0/7`, Tier 3 `626/0/8`, build/fmt/Clippy/diff pass.
- DMG: `src-tauri/target/release/bundle/dmg/LifeSub_0.2.1_aarch64.dmg`, SHA-256 `37ff9f39b56e03802a48823f42c02822e01cc3ed723a140af27249523532bd14`. Image verification, read-only mount, nested signature and exact contained executable hashes pass.
- Installed bundle `/Applications/LifeSub.app` is now the frozen candidate above, version `0.2.1`. It launches normally from `/Applications`; diagnostics shows microphone and screen/system-audio permissions granted, and TCC logs attribute the installed path to `com.goldenwave.lifesub`. A second executable launch exits cleanly with `Ownership(AlreadyOwned)` and leaves one instance.
- Final screenshots: `output/playwright/lifesub-0.2.1-installed.png` and `output/playwright/lifesub-0.2.1-installed-diagnostics.png`. The prior installed bundle is retained temporarily at `/tmp/LifeSub.pre-0.2.1.20260902174505.app`; do not use it as a release source.

## 2026-09-02 final candidate FreeBuds disconnect/reconnect evidence

- Canonical source remains `/Users/goldenwave/Documents/MyProject/LifeSub`, `main@db39faafeac0e8f4816623456f3ffa8b1ab54564`; final candidate identity remains app `6f2e65235de68c98b14bbed127dbb9762e90773d46685d97c7f162bf3903cf4a`, helper `5c7b4672fc9da848d900526337d2932efba0b0c8e9196991586acd60949e8e01`, gate `c1bafe9a101382cba688f67ec84a9d50c5e76e496afe1162bc3b8c45961d714d`, dirty digest `6d5c9bfa897307cb9a74c551c52afd0bb08037eee0eabef319e59b98d9c5c4ef`.
- FreeBuds 7i is now genuinely connected and enumerated as `78-F0-9B-A1-7E-BC:input`, Bluetooth, 16 kHz mono. The final candidate passed explicit probe, captured session `rec_c9366841cc1a4095991676a4aae20c3b`, and produced 9 contiguous chunks / 9 succeeded Jobs / 0 dropped frames.
- Real active disconnect showed the selected microphone disconnect error, stopped capture, and did not fall back to the built-in microphone. Reconnect restored the exact UID and a passing probe. Search, playback, private Markdown export and diagnostics export were verified.
- New v3 reports `microphone-active-disconnect`, `device-disconnect-reconnect`, and `microphone-prestart-disconnect` each pass single-file verification; final-candidate report progress is now `14/20`.
- Prestart-disconnect binds disabled UI and zero Catalog publication to a real signed-helper V2 probe returning `unavailable / 0 frames`; its raw helper output is retained separately. All 14 reports pass individually under the same final identity.
- Remaining external gates are LifeSub Screen & System Audio permission, real `system-only` / `dual-source` / `system-only-no-microphone` / 30-minute capture, sleep/wake, and observable same-UID recovery fields. No marker, DMG, or installation is allowed before the full directory gate.
- After three consecutive action-time audits, LifeSub Screen & System Audio remains `off` and the user has not confirmed changing this security-sensitive setting. Acceptance is formally blocked at `14/20`; resume by confirming the toggle, then complete system-audio evidence without rebuilding the final candidate.

## 2026-09-01 final permission/Bluetooth hardening candidate

- Canonical source remains `/Users/goldenwave/Documents/MyProject/LifeSub`, `main@db39faafeac0e8f4816623456f3ffa8b1ab54564`; build-source dirty digest is `6d5c9bfa897307cb9a74c551c52afd0bb08037eee0eabef319e59b98d9c5c4ef`.
- Reference-led root-cause fixes: lazy system engine creation, TypeWhisper-style 300ms delayed `AVAudioEngine` release, and exact UID bind/readback before accessing `inputNode`. Swift helper passes `75/75`.
- Final candidate: app `6f2e65235de68c98b14bbed127dbb9762e90773d46685d97c7f162bf3903cf4a`; helper `5c7b4672fc9da848d900526337d2932efba0b0c8e9196991586acd60949e8e01`; gate `c1bafe9a101382cba688f67ec84a9d50c5e76e496afe1162bc3b8c45961d714d`. Nested strict signing, arm64 and app/helper audio-input entitlements pass.
- Final-candidate native v3 progress is `11/20` under `output/asr-v0.2/native-capture-v3-6f2e/reports/`; every present report passes single-file verification. Added final-identity built-in selection, no-device injected UI, and real helper crash/relaunch reports.
- Final packaged provider gate passes under the same source digest: SenseVoice CER `0.0769`, RTF `0.0365`, RSS `470.4 MiB`; Whisper WER `0.0`, RTF `0.0702`, RSS `661.4 MiB`. Fresh Tier 2 is `583 passed / 0 failed / 7 ignored`; Swift helper is `75/75`.
- Microphone permission is restored to on. FreeBuds is currently physically disconnected and must be woken before active-disconnect/reconnect evidence can continue. Sleep/wake still requires a real sleep cycle. No release marker, DMG, or installation is allowed yet.
- Screen/system-audio permission rebinding is paused at the macOS Touch ID/admin unlock dialog. After the user unlocks System Settings, continue the LifeSub toggle off/on and `system-only` report without rebuilding the candidate.
- Same-UID `44.1 -> 48 -> 44.1 kHz` target-device run stayed healthy and published 3 chunks/jobs, but no report was emitted because generation/firstHostTime recovery evidence is not observable in the current production report surface.
- After three consecutive audits, external state is unchanged: System Settings is waiting for Touch ID/admin authentication and FreeBuds remains disconnected. Acceptance is formally user-auth blocked at `11/20`; the exact `6f2e.../5c7b.../6d5c...` candidate remains unchanged.
- Reports under `native-capture-v3-097c` and `native-capture-v3-216b` are historical only and cannot be mixed with the final candidate.

## 2026-09-01 final candidate after dual-source startup fencing

- Native v3 collection progress: 11/20 final-candidate reports pass single-file verification under `output/asr-v0.2/native-capture-v3-097c/reports/`. Directory verification currently stops at `permission-denied`.
- Final `dual-source-30m` evidence passes: `1877.819625s`, 63 contiguous chunks and succeeded Jobs per source, 223 ms drift, 0 dropped frames, 668 MiB peak RSS, plus restart/search/playback/private export.
- Acceptance is user-assisted blocked at 11/20 reports. Required unblock actions: authorize temporary microphone/screen-recording permission toggles; wake/connect a paired Bluetooth headset so it appears as a Core Audio input; remain available to wake the Mac after the sleep/wake scenario. Candidate, marker, DMG and installed app remain unchanged.
- A 1914-second run was rejected because Catalog contained only microphone source despite the UI having briefly displayed smart mode; persisted config was still `mic_only`. The next soak is gated by an exit-time Catalog readback proving `capture_mode=smart` before start.

- Canonical source: `/Users/goldenwave/Documents/MyProject/LifeSub`, `main@db39faafeac0e8f4816623456f3ffa8b1ab54564`; dirty entries `132`; build-source dirty digest `dc4f8166e26d2652016cc846f6cbda6891dd28aa7426c63acda7e946a00818a4`.
- Frozen app-only candidate SHA-256: app `097ceb936702e44126a4c7b37fb558526cc46ff6bd9c93df41b5a4201b0fab42`; helper `1937bf38b2e167ce8437cd6732be82b3dbba76fb2f57b255c7339c695df24ef9`; gate `c1bafe9a101382cba688f67ec84a9d50c5e76e496afe1162bc3b8c45961d714d`. Nested strict signature, arm64 and app/helper audio-input entitlements pass.
- Real dual-source startup protocol violation was fixed at the source publication boundary: startup recovery cannot emit or seal before all source identities are published; stale startup notifications are dropped; stop invalidates in-flight startup generation. Swift `73/73`; combined review `0/0/0`.
- Repaired dual-source session `rec_4775fa3161d54442bac440e4efebd036` produced 5 microphone chunks/jobs and 4 system-audio chunks/jobs, all succeeded, with 0 dropped frames and 232 ms envelope drift over 107.999625 seconds.
- Packaged provider gate passes for SenseVoice CER `0.0769` and Whisper WER `0.0`. Fresh Tier 2: `583 passed / 0 failed / 7 ignored`; fresh Tier 3: `626 passed / 0 failed / 8 ignored`.
- Formal release-source verification fails closed only because the Native ASR marker remains intentionally absent pending the final 20-report directory gate.
- Previous `84ee...` and `7802...` report attempts are invalidated by candidate identity changes. The final candidate still has 0/20 publishable native reports. Do not add Native ASR marker, build a DMG, or replace `/Applications/LifeSub.app` until all 20 reports bind the final app/helper/source identity and pass directory verification.
- External/Bluetooth evidence remains blocked: macOS sees `HUAWEI FreeBuds 7i`, but LifeSub/Core Audio input enumeration exposes only the built-in microphone and `DTAudioPlugin`. A genuinely enumerable external input is required; do not relabel a virtual device as external hardware.

## 2026-08-27 final candidate static gates

- Latest app-only candidate was rebuilt from canonical `main@455c733786f76f66de910ec8c66f2062f537377d` with verified Sherpa attestation after short-chunk timing and atomic packaged export fixes. App SHA-256: `bb66bec22833f62d274c441fac56f7d762f69a08b317b75ce8143570bad12436`; bundled helper SHA-256: `01f227b271c50462793595dd010998a8a3ab996c5935530fc60a2039949a6243`; bundled gate SHA-256: `a874d333e8d964d0e633b395517009f007965553ff04c5c37edcd52009e07662`. Current dirty entries: `128`; dirty path digest: `bb31d9c208b6a4c86a202e424540f57c7ae2053c9f75a076d0fdc4c4d2caa0e4`.
- Nested strict signing, arm64 and app/helper `com.apple.security.device.audio-input` entitlements pass. Packaged provider gate passes for SenseVoice CER `0.0769` and Whisper WER `0.0`, bound to source digest `3346e509b120483d0dd308a4b9355c4e28954bbd89a7b6be8d7f2d144a895747`.
- Fresh Tier 2: `582 passed / 0 failed / 7 ignored`; fresh Tier 3: `625 passed / 0 failed / 8 ignored`. Native v3 reports are still missing; real permission/device/recovery/30-minute scenarios remain pending. Do not add the Native ASR release marker, build a DMG, or replace `/Applications/LifeSub.app` yet.
- Unlock follow-up: the exact project candidate launches when its executable is run directly. Its diagnostics page reports app hash `1550def5de43...`, Sherpa 1.13.5 and runtime-qualified SenseVoice/Whisper Base. A real explicit probe of the selected `DTAudioPlugin / DTAudioDevice` passed at 44.1 kHz / 2 channels. Screen/system-audio permission remains not granted, and external USB/Bluetooth plus permission/recovery/30-minute v3 scenarios still require user-assisted hardware/privacy actions.
- `open <project LifeSub.app>` is not identity-safe while historical `/Applications/LifeSub.app` shares the Bundle ID: macOS selected the old installed app and produced an 11:07 startup abort. Continue candidate acceptance by launching the exact project executable and verify `procPath`/hash before attributing crashes.
- Real built-in-microphone evidence exposed and fixed a callback timestamp defect: a 1,480-sample startup chunk was previously stored as 0 ms and failed the ASR snapshot. The rebuilt candidate produced session `rec_d6ef6dd7584f4d3d826080068ecf0e1b` with three valid continuous chunks (92 ms, 29,995 ms, 3,607 ms), three succeeded SenseVoice Jobs and non-empty persisted Revisions/Segments. Fresh Swift is `71/71`, Tier 2 `582/0/7`, Tier 3 `622/0/8`.
- Packaged provider gate was rerun from the final bundle: SenseVoice CER `0.0769`, RTF `0.0390`, RSS `456.7 MiB`; Whisper WER `0.0`, RTF `0.0675`, RSS `761.9 MiB`. Both pass. Fresh Tier 2 is `582/0/7`; fresh Tier 3 is `625/0/8`.
- Exact final-candidate built-in microphone smoke: `rec_41b0ac9351e140939dc67273e10fa249` produced four valid chunks, four succeeded SenseVoice Jobs, three persisted Revisions and eight non-empty Segments. Final atomic Markdown export `~/Documents/LifeSub-final-export-bb66.md` is mode `0600` and was inspected. Export review is Critical/Important/Minor `0/0/0`. Native v3 reports still need to be collected against the final app/helper hashes.
- Formal release-source verification still fails closed only at the intentionally absent Native ASR capability marker; an empty native-report directory fails at `missing report: mic-only`. The System Settings LifeSub toggle currently reflects an older ad-hoc identity. The user must toggle it off/on for final candidate cdhash `5585a640271f294735f92fa7f2d0f77ff071c7e2` and connect an external USB/Bluetooth microphone before the v3 matrix can continue.

## 2026-08-27 settings truth and microphone probe hardening

- Task 1 settings truth is code-complete: Core projects fixed 16 kHz output, actual data directory and unavailable IM detection; ASR language/options are model/Provider-driven and Whisper task reaches the production execution snapshot.
- Task 3 exact-UID compatibility probe is code-complete. The authenticated helper now supports a capability-gated, no-staging probe that reuses the production resolver/binder and format-settle path, requires same-UID readback plus a non-empty PCM frame, and always tears down its temporary engine/tap.
- Passive microphone listing remains permission- and engine-free; the live probe is lazily constructed only for an explicit user action. Probe results cannot enter the persistent capture coordinator or Catalog.
- Fresh Task 3 gates: Swift `62/62`, Rust capture protocol `13/13`, Rust helper `22/22`, frontend focused `57/57`, production frontend build and desktop Cargo check passed. Real built-in/external/Bluetooth probe reports remain Task 7 release evidence.
- The next code task is Task 2 diagnostics, while Task 4 remains hardware-evidence-pending. Do not treat the historical `recoverable=true` interruption as successful recovery.
- Task 4 code is now complete: explicit generation-fenced recovering/recovered events, same-UID engine/tap rebuild, post-recovery first-frame proof and sequence-contiguous chunk rotation are implemented. Fresh gates are Swift `69/69`, Rust coordinator `6/6`, Rust protocol `14/14`, desktop check and frontend build.
- Task 4 release evidence is not complete until Task 7 collects a packaged target-Mac configuration-change report proving post-recovery chunk and ASR publication. The previous `recoverable=true` report is not acceptable evidence.
- Task 2 diagnostics is code- and Tier-2-complete. The Settings diagnostics tab shows passive permission facts, exact selected device/latest probe, app/helper/runtime/model identities, storage totals and active/recent capture errors; export is redacted and no-follow.
- Fresh combined gates after diagnostics: frontend `116/116`, Swift `70/70`, Rust Tier 2 `582 passed / 0 failed / 7 ignored`, Clippy warnings denied, fmt/diff/build passed. Visual evidence is under `output/playwright/diagnostics-settings-*.png`.
- Task 5 durable model operations is complete. Catalog-backed bytes/stages/artifacts/errors, VAD dependency projection, exact cancel acknowledgement and same-download retry are exposed through a generation-fenced controller and accessible progress UI.
- Fresh frontend after Task 5: `124/124` and production build passed. Controlled transport interruption/resume/cancellation tests and ModelManager `88/88` pass. UI state evidence is under `output/playwright/model-download-*.png`.
- The next work is Task 7 packaged acceptance extension. Task 4 remains code-complete but needs its real configuration-change report in that matrix.
- Task 7 verifier v3 is ready: 20 scenarios now require exact microphone probe and same-UID recovery evidence, with mutation-negative coverage. Real reports remain uncollected and must use the final frozen candidate.
- Task 8 provider gate is now real and passed for installed SenseVoice + Whisper fixtures. SenseVoice CER `0.0769` / RTF `0.0325` / RSS `555.5 MiB`; Whisper WER `0.0` / RTF `0.0587` / RSS `908.2 MiB`. Report: `output/asr-v0.2/packaged-provider-gate.json`.
- Gate exposed and fixed Whisper `auto` language mapping (`""` for sherpa), and disabled unavailable token timestamps for models without cross-attention. Final app candidate must be rebuilt after this fix.

## Current consolidation state

- The divergent worktrees have been consolidated through a reviewed merge workflow. The final canonical branch is `main` in the primary project path.
- The integrated source imported through the merge is `652abf720e67765b53de56714a14965fb4cfbcf0` from `codex/lifesub-real-asr-v0.2`.
- Tasks 1--7 of the approved native-capture plan are implemented and reviewed.
- Task 8 native capture is production-wired; real macOS hardware and packaged-helper acceptance remains outstanding.
- Task 9 has fresh-directory development-binary real-model evidence. Task 10 committed-only UI events, Catalog replay and durable failure visibility are complete and reviewed. Packaged-app and hardware evidence are still missing, so the Native ASR release capability remains intentionally absent.
- Do not package, install or replace `/Applications/LifeSub.app` until Tasks 9--12, real-model inference and hardware acceptance pass.

## Integrated release source contract

- Release source worktree: `/Users/goldenwave/Documents/MyProject/LifeSub`
- Release source branch: `main`
- Release version: `0.2.1`
- Verification: run `npm run verify:release-source`. Until Native ASR is real-model-qualified and release-capable, local planning audits may use `LIFESUB_RELEASE_ALLOW_PLANNED=1 npm run verify:release-source`; that mode is explicitly not release evidence.

## Canonical integrated development and release source

- Worktree: `/Users/goldenwave/Documents/MyProject/LifeSub`
- Branch: `main`
- Merge inputs: preserved main workspace `dc62d5b` plus integrated source `652abf7`.
- Current milestone: Task 10 complete; Task 11 strict report verifier is ready and target-machine packaged hardware collection is pending.
- Current canonical HEAD: `455c733786f76f66de910ec8c66f2062f537377d`.
- Current dirty entries: 61 before this status update. Preserve all existing changes.

## Currently installed app

- Installed bundle: `/Applications/LifeSub.app`
- Bundle version: `0.2.1`
- Executable SHA-256: `6b8b7e17f4f509a95434560141b05793f31503530405dc242e8b1b7e620d8f8c`

The installed app predates the current Task 7--9 integration state. It is not evidence that the present native capture or Native ASR path works in the installed product. Do not use the installed bundle as the source for further development.

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
| Imported-audio production ASR executor | Worker selects `ProductionNativeAsrEngine`; development binary completed real VAD + SenseVoice inference and fenced publication, but packaged-app evidence remains pending |
| Versioned Rust/Swift capture protocol | Implemented and reviewed; shared fixture, bounds, replay/sequence and PCM validation pass |
| ScreenCaptureKit system-audio helper | Implemented and connected through the production native coordinator; hardware acceptance pending |
| AVAudioEngine microphone helper | Implemented and connected through the production native coordinator; hardware acceptance pending |
| Helper build/bundle/signing | Implemented and reviewed; arm64 sidecar uses fixed identifier and compile-time SHA attestation |
| Helper authentication/supervision | Implemented and reviewed; inherited-FD nonce, UID/PID/path/file identity, replay and timeout gates pass |
| Catalog v6 capture timing and atomic chunk sealing | Implemented and reviewed, including durable seal intent, anchored atomic sealing and recovery |
| Production NativeCaptureCoordinator | Implemented and selected by the desktop production factory; target-machine hardware acceptance remains pending |
| Native ASR release capability | Intentionally absent; formal verifier correctly rejects release until real-model evidence exists |
| Demo fallback in desktop production | Intentionally blocked |

The real LifeSub Catalog now contains exact `runtime_qualified` installations for SenseVoiceSmall and Silero VAD, installed through LifeSub ModelManager. Other applications' ONNX files remain invalid evidence.

After the 2026-08-22 consolidation, `main` at the primary project path is the only development/release source. Semantic version strings, old DMGs, `/Applications/LifeSub.app`, or the retained rollback worktree must not override this contract.

## Current verification snapshot

- 2026-08-26 16:20 native capture recovery/system-only follow-up: startup now cleans strict current/legacy capture staging under exclusive ownership and reconciles stale `recording/paused` sessions to the last durable boundary; real `rec_8cc4aa3402c944eda0b20d640d085d30` recovery left 0 staging files/directories and Timeline reload showed persisted real ASR. Recording start now action-time reloads persisted mode/UID after the selection queue; system-only UI no longer shows a microphone selector. Swift permission errors distinguish microphone from screen/system audio. Fresh frontend `102/102`, Swift `57/57`, Tier 2 `566/0/7`, Tier 3 `597/0/8`, fmt/Clippy/diff/build pass; focused reviews Critical/Important/Minor = 0. Current app-only candidate executable SHA is `7093178c34780810754e965d2f45cea869ebb6a6abd725cfedcf5b84a71f4e39`, bundled helper SHA is `2b198f5ae316b1703b254c52ec770bb829d5b01b700c8c9f4aaa7d4f0e068eb9`, dirty path digest is `7d6379dc50fd94e58ebc3e76feb0056b25fe07aaff1bd3fefa88770797b20466` across 99 paths. Real system-only now fails with the truthful `screen_recording_permission_denied`; user must enable LifeSub under macOS Screen & System Audio Recording before system-only/dual-source hardware evidence can continue. No DMG or install has occurred.

- 2026-08-26 14:40 packaged native ASR follow-up: real session `rec_099bb3fde39d450ab8cec0daeaaf9fa8` proved 5 WAV chunks -> 5 succeeded SenseVoice Jobs -> 5 local Receipts -> 5 verified Revisions -> 6 non-empty Segments. The same evidence exposed per-buffer `AVAudioConverter` priming loss: 480,000 output samples took about 32.4 seconds host time. RED observed 14,982 samples from one second of 44.1 kHz input split into 256-frame callbacks; cached same-rate converter GREEN restores about 15,994, and a second RED/GREEN clears cached state across `44.1k -> 16k direct -> 44.1k`. Swift is `57/57`; final focused review Critical/Important/Minor = 0. Frozen app-only candidate executable SHA is `587535fd30f9fb4688b67b0089c07e8d5e8909cc191b79c3379998b853d93831`, bundled helper SHA is `d9656790c480efcb815d253414f00cb6380dcecdde101d93986589da152c0d2a`; nested strict signing, arm64 and audio-input entitlements pass. This new ad-hoc identity is waiting for the LifeSub microphone Allow prompt before timing revalidation. No DMG or install has occurred.

- 2026-08-26 09:22 packaged mic follow-up: current app-only candidate was rebuilt through the verified sherpa Cargo runner after a real 48 kHz client / 44.1 kHz hardware AVFAudio crash. Swift now waits up to 500 ms for the selected Core Audio hardware format before tap installation and maps timeout to `selected_microphone_unavailable`. Fresh Swift `52/52`, LiveCapture `28/28`, frontend production build and diff check pass; final narrow review Critical/Important/Minor = 0. Candidate app SHA is `4e7152b0765ea45a84748c604778d36302ba30e99373224904210d3fe06f4ac2`, bundled helper SHA `18cffa7ab732c4c2f4e4694ca9f37f84e94f0bf05aed5bef8735b54904194986`; nested signature and microphone entitlements pass. The new ad-hoc identity has a visible LifeSub microphone prompt awaiting the user's manual Allow click. No DMG or install has occurred.

- 2026-08-26 microphone selection Task is code-complete and combined-review clean (Critical/Important/Minor = 0). Swift exact UID capture, Rust authenticated discovery/identity fencing, shared frontend selector states, lifecycle locking, persistence and Task 11 v2 evidence bindings are implemented.
- Fresh gates: Swift 50/50, frontend 97/97 plus production build, native verifier 19 scenarios, Tier 2 564 passed / 0 failed / 7 ignored, fmt + Clippy warnings denied + diff passed. Latest Tier 3 remains 592 passed / 0 failed / 8 ignored from before the packaging-only follow-up changes.
- Current acceptance candidate app exists at `src-tauri/target/release/bundle/macos/LifeSub.app`; it is ad-hoc signed, nested signature verification passes, and both app/helper signatures contain `com.apple.security.device.audio-input`. Executable SHA-256 is `6a897afebb1d7343ab2dc855d4f00ff8d5e99a39291e78abda561ee86f3511f9`; bundled helper SHA-256 is `be7b66c93a2ff4c4cd9428ddf5d7775ad810ada404c460edc7d41e27147e5318`.
- The candidate fixed two packaged-only failures with real RED/GREEN evidence: Tauri sidecar re-signing broke full-file attestation (`SignatureInvalid`), and reading input format before exact UID binding crashed AVFAudio on 48 kHz -> 44.1 kHz device transition. Device discovery now succeeds; exact capture awaits current-build microphone TCC authorization.
- `/Applications/LifeSub.app` remains the historical executable SHA `6b8b7e17...` and has not been replaced. Permission-triggered relaunch selected that stale app twice and created 08:28 crash reports whose `procPath` is `/Applications/LifeSub.app`; its LaunchServices registration was temporarily removed to keep candidate acceptance isolated.
- Current worktree: `main@455c733786f76f66de910ec8c66f2062f537377d`, dirty count 98 and dirty digest `c60959efaba2c85cf1c163813daf4eea015316e1b17c7fe96f5751c686c587a9`. Preserve all changes. No DMG has been built and no final install has occurred.

- 2026-08-25 follow-up hardware start: macOS TCC logs prove the development responsibility identity `com.openai.codex` has microphone authorization (`authValue=2`), so absence of a second LifeSub-named prompt is expected in Codex-launched development. The helper emitted `device_configuration_changed` with `recoverable: true`, while Rust previously treated every interruption as fatal. A RED/GREEN coordinator regression now keeps recoverable interruptions Running/Pending and preserves fail-closed behavior for non-recoverable interruptions. Focused coordinator tests `5/5`, desktop Cargo check, fmt and diff checks pass. The repaired development binary is running for a fresh user click; packaged LifeSub evidence remains pending.
- 2026-08-25 Task 11 hardware-start incident: duplicate idle/startup buttons were reduced to one stateful button with a RED/GREEN regression; LiveCapture `24/24`, production frontend build and diff check pass. Vite now ignores the 20 GB Rust target and Swift `.build`; observed idle Vite CPU fell from about 415% to 5%--9%, with LifeSub at about 0%--0.1%. Visual evidence: `output/debug/lifesub-after-start-button-fix.png`.
- Follow-up start freeze was traced with a live process sample to synchronous `startup_rx.recv()` on the AppKit invoke thread. Capture lifecycle commands now use Tauri blocking workers, and the UI exposes a truthful `正在启动` state until real source readiness. A second root cause in the Swift helper used a 4096-byte `FileHandle` read for length-prefixed commands; a single-Start regression timed out before the fix and now responds in about 12--14 ms with exact frame reads. Fresh verification: frontend `76/76`, Swift helper `23/23`, production frontend build, desktop Cargo check, fmt and diff check pass.
- The rebuilt sidecar SHA-256 is `c68e13f7d63b8b2ce63baa4a0c49f692cf5f871fdfecadf8bbe101839d054ce7`. A live development click now reaches the macOS microphone permission prompt without freezing. Because the development app was launched through Codex, macOS labels the responsible process as ChatGPT; this is development evidence only and does not replace packaged LifeSub permission acceptance.
- The verified-runtime development app is running for the next permission attempt. Do not launch the historical installed app concurrently; the supplied 14:06 CrashReporter file belongs to `/Applications/LifeSub.app` and matches the already-fixed second-instance startup abort, not this current UI incident.

- Task 10 combined review: Critical 0 / Important 0; committed-only UI, Catalog replay, revision cursor and durable failure events are implemented.
- Task 11 verifier review: Critical 0 / Important 0; 12-scenario report contract and mutation-negative shell tests pass. Real packaged hardware reports are pending user-assisted permission/device scenarios.
- Latest canonical worktree audit: `main@455c733786f76f66de910ec8c66f2062f537377d`, 68 dirty entries before this status update; rollback worktree remains clean.

- Tier 2 passed on 2026-08-25: fmt, Clippy with warnings denied, diff check, Rust 545 passed / 0 failed / 7 ignored.
- Native engine focused: 6/6; frontend model/service focused: 26/26.
- Canonical real model install Gate: 1/1, SenseVoiceSmall + Silero VAD both `runtime_qualified`.
- Canonical real import-to-publication Gate: 1/1; two durable jobs succeeded with exact provider/VAD Receipt, Revision and non-empty Segment.
- Observed mixed-language text: `I'm alone all by myself. If too sell Sono too as toy soda.` Technical wiring passed; multilingual quality remains below the intended quality Gate.
- Second-instance startup regression: before fix exit 134 with CrashReporter; after fix clean exit 0 and no new crash report.
- Task 10: Worker 8/8, LiveCapture 23/23, frontend full 75/75, native committed DTO identity 1/1; combined review Critical=0 / Important=0.
- Latest Tier 2 after Task 10: 548 passed / 0 failed / 7 ignored, with fmt, Clippy warnings denied and diff checks passing.
- Task 11 verifier shell tests: 12-scenario positive set plus missing source identity and drift-threshold negative cases passed; desktop harness integration test passed. Real hardware reports have not yet been collected.

- Frontend full suite: 69/69 passed on 2026-08-25.
- Rust no-default library check: passed with 0 errors and 40 warnings on 2026-08-25.
- `git diff --check`: passed on 2026-08-25.
- Formal release-source verifier: failed closed as expected because `NativeCaptureRuntimeFactory` does not implement `NativeAsrDesktopRuntimeFactory`.
- Current target tree contains no DMG.
- Latest recorded focused evidence: capture sealing 17/17, native coordinator 4/4, desktop native capture session 2/2, Swift controller 12/12, frontend production build passed, verified Sherpa desktop Cargo check passed.

Complete continuation details are recorded in `docs/handoffs/2026-08-25-lifesub-v0.2.1-native-capture-asr-handoff.md`.

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
