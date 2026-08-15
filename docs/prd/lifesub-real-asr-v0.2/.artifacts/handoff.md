---
feature: lifesub-real-asr-v0.2
handoff_date: 2026-08-16
branch: codex/lifesub-real-asr-v0.2
worktree: /Users/goldenwave/.config/superpowers/worktrees/LifeSub/lifesub-real-asr-v0.2
head_at_handoff: 3972c28
status: implementation-in-progress-dirty-worktree
---

# LifeSub V0.2 真实 ASR 开发 Handoff

## 1. 新会话目标

继续完成 LifeSub V0.2 真实本地 ASR：SenseVoice、Whisper、Qwen3-ASR 0.6B/1.7B、模型管理、持久设置、任务/Receipt/Revision、设置与重转写 UI、真实模型 Gate、桌面与 DMG 验证。

不要把当前 Goal 标记完成，直到 Tasks 5-15、真实模型、UI、desktop harness 和 DMG Gate 全部通过。

## 2. 必须先读

1. `AGENTS.md`
2. 所有 `docs/prd/**/.artifacts/process.md` 和本文件
3. `docs/context/INDEX.md`
4. `.claude/rules/common/coding-style.md`
5. `.claude/contexts/dev.md`
6. `docs/prd/lifesub-real-asr-v0.2/PRD.md`
7. `docs/superpowers/specs/2026-08-15-lifesub-real-asr-design.md`
8. `docs/superpowers/plans/2026-08-15-lifesub-real-asr-v0.2.md`
9. `docs/superpowers/specs/2026-08-16-lifesub-local-tool-api-design.md`

开发继续使用 TDD、每任务 fresh implementer、规格复审后再代码质量复审。Native Cargo 命令必须通过：

```bash
scripts/with-sherpa-runtime.sh cargo <arguments>
```

## 3. Git 与工作树

- 主仓库：`/Users/goldenwave/Documents/MyProject/LifeSub`
- 实现 worktree：`/Users/goldenwave/.config/superpowers/worktrees/LifeSub/lifesub-real-asr-v0.2`
- 分支：`codex/lifesub-real-asr-v0.2`
- handoff 前 HEAD：`3972c28 docs: adopt contract-first lifesubd architecture`
- 主 worktree 有用户自有 V0.1 改动，禁止清理或 reset。
- 当前实现 worktree 有未提交改动。禁止 `git reset --hard`、`git checkout --` 或批量清理。

最近关键提交：

```text
3972c28 docs: adopt contract-first lifesubd architecture
3bae68d fix: enforce durable audio import ordering
d355f5c feat: harden immutable audio imports
e1788d6 docs: checkpoint ASR domain settings
4d303a7 fix: validate persisted ASR receipts
b84dd36 fix: harden ASR domain serialization
df38606 feat: define validated ASR settings
a09c5cf fix: preserve catalog schema token boundaries
071d6fb fix: harden catalog migration quality
c5e0ba0 fix: reclaim ASR runtime build locks
405235f build: add static local ASR runtime
```

## 4. 已完成任务

### Task 1：可信 sherpa-onnx 运行时

- 固定 sherpa-onnx 1.13.5 与 native archive identity。
- verified fetcher、构建锁、cache quarantine、runtime identity Gate 已完成。
- Task 1 规格与质量批准。

### Task 2：Catalog migration

- `user_version = 2`、真实 V0.1 fixture、严格 fingerprint、busy timeout、并发 opener、回滚测试。
- Provider allowlist 已包含 `qwen3_asr`。
- 最终验证：migration 24/24、Catalog 1/1、全量 31/31、Clippy clean。
- Task 2 规格与质量批准。

### Task 3：ASR domain/settings

- 三 Provider snake_case、tagged options、受控 `AsrLanguage`。
- `ModelLookup` 区分 selectable/installable/executable。
- 完整稳定 `AsrErrorCode`。
- 验证时间范围 custom Deserialize。
- fail-closed `ProviderReceipt`，私有字段、验证 draft、hash/JSON/VAD/time invariants。
- 最终验证：focused 18/18、全量 48/48、Clippy clean。
- Task 3 规格与质量批准。

### Task 4：已提交基线与规格批准

- `3bae68d` 完成 DB-last durable import：目录 fsync、temp sync/hash、atomic rename、parent fsync、session+chunk transaction。
- production initialize reconciliation、typed chunk diagnostics、missing/corrupted/available 修复。
- 验证记录：imported_audio 3/3、service 14/14、Catalog 1/1、全量 59/59、Clippy clean、desktop wrapper check 通过。
- Task 4 规格批准。
- Task 4 代码质量复审发现 symlink escape 和跨进程 reconciliation race；修复已开始但尚未提交/复审，见下一节。

## 5. 当前未提交改动，必须分组处理

运行：

```bash
git status --short
git diff --stat
```

### A. Task 4 安全修复，未提交

文件：

```text
src-tauri/Cargo.toml
src-tauri/Cargo.lock
src-tauri/src/catalog/chunks.rs
src-tauri/src/catalog_test.rs
src-tauri/src/commands.rs
src-tauri/src/service.rs
src-tauri/src/service/audio_store.rs
src-tauri/src/service/runtime_lock.rs   (untracked)
src-tauri/src/service_test.rs
```

已实现但未做最终验证/提交：

- `audio/` root、chunk target、reconciliation entries 拒绝 symlink。
- reconciliation 只清理符合 LifeSub importer 命名规则的 regular files。
- production orphan grace 为 10 分钟。
- unknown persisted integrity 不再 fail-open 为 Available。
- `RuntimeOwnershipGuard` 在 writable Catalog open/migration 前获取 full Core lock。
- 第二实例当前 fail closed；Task 11 再改为连接 primary socket。
- 加入 `libc` 以使用 `O_NOFOLLOW`。
- 增加锁竞争、symlink、最近 orphan、未知 integrity 测试。

Task 4 质量 reviewer 原始 Important：

1. symlinked `audio/` 可让 orphan cleanup 逃逸 data_dir。
2. reconciliation 无 full process lock/age grace，可能删除另一个进程的 live import。

新会话第一优先级是审计这组 diff，完成 RED/GREEN、全量/no-default、Clippy、可信 desktop check，然后重新请求 Task 4 代码质量复审。不要与 Task 5 混合提交。

### B. Task 5 Manifest 的故意 RED 草稿，未完成

文件：

```text
src-tauri/src/asr_manifest_test.rs  (untracked)
src-tauri/src/lib.rs                (注册测试模块)
```

当前测试仍假设 Qwen3-ASR 1.7B 为 `ExperimentalUnavailable`，这已经被用户最新决定推翻。不要按当前断言实现。先修改测试与设计，再继续 Task 5。

当前没有 `src-tauri/src/asr/manifest.rs`，因此当前 dirty worktree 全量测试会因为这个 RED 测试缺失实现而失败。这是预期，不代表 Task 4 修复一定失败。

验证 Task 4 时可临时用 `apply_patch` 移除 `lib.rs` 中 `mod asr_manifest_test` 两行，保留 untracked test 文件；验证后再恢复。不要删除测试文件或使用 checkout/reset。

### C. Local Tool/lifesubd 架构复审修订，未提交

文件：

```text
docs/architecture.md
docs/superpowers/plans/2026-08-15-lifesub-real-asr-v0.2.md
docs/superpowers/specs/2026-08-16-lifesub-local-tool-api-design.md
```

`3972c28` 的初版架构审查未批准。当前未提交修订已处理：

- ASR V0.2 的 native `start_capture/stop_capture` capability 明确 unsupported；真实 DeepSeek capture 闭环推迟到 ScreenCaptureKit + AVAudioEngine 里程碑。
- full Core ownership lock 必须在任何 writable Catalog open/migration 和 socket bind 前获取。
- 第二 Tauri 进程必须连接 primary socket，不得打开 SQLite。
- Catalog v3 `tool_requests` 持久 idempotency 合同。
- UDS `0700` runtime dir、`0600` socket、mandatory `getpeereid`、length framing、size/time limits、stale socket 和 shutdown order。
- `open_evidence` 返回 `OpenEvidenceIntent`，Core 不直接打开窗口。
- 完整 V1 method list、caller context、golden JSON、search limit/cursor。

这组文档尚未重新做 architecture review，也未提交。

## 6. Qwen3-ASR 最新决策

用户最新明确：Qwen3-ASR 1.7B 在当前设备性能可用。

当前设备：

```text
Apple M4
24 GB unified memory
macOS 15.6.1
arm64
```

因此，不再把 1.7B 设计为不可安装。候选正式路径：

- 0.6B 低资源档：现有 sherpa-onnx 0.6B INT8 包，878,702,423 B，SHA-256 `393f8a14e2f5fb96746aaab342997a40641001fbd5bf9592a080a8329178ee96`。
- 1.7B 高质量档：`qwen3-asr = 0.2.2` Rust crate，Candle + Metal，MIT。
- crate Git source commit：`c5ef09646af6278d2ba8b8ceaf543ffb32d1a5dc`。
- 上游 M4/16GB benchmark：1.7B avg RTF 0.319，live memory 4.6 GB；仍必须用 LifeSub fixtures 在当前 M4/24GB 重新 Gate。

重要兼容性：

- 用户链接的 `Qwen3-ASR-1.7B-hf` config 是 Transformers 新格式，顶层 `audio_config/text_config`。
- `qwen3-asr 0.2.2` 当前直接读取旧/原始格式的顶层 `thinker_config`，不能未经适配就宣称直接加载 `-hf` 包。
- 建议使用官方原始 `Qwen/Qwen3-ASR-1.7B` safetensors/config，并使用官方 `-hf` 的兼容 `tokenizer.json`，或实现并验证确定性 tokenizer 构建。这个混合官方资产方案必须先写清 provenance 并做真实 Gate。

ModelScope 已取得的 1.7B 原始资产 identity（revision `d69410f1c275f2b0fa60cbb9960edfcdb0ae0aec`）：

```text
config.json                         6,194 B       sha256 2e74a751548b8ad7d7526d29365ad8144c345d8b412b1152d25dc6698452712f
model-00001-of-00002.safetensors    4,220,320,824 B sha256 a4cd1f1a04d90b757dc7f7dd26254e69a013b19e80efe590a83c6a3bde8608d6
model-00002-of-00002.safetensors    478,200,688 B sha256 6e0b9d9e09e2e0238e7ef3cc8a484ab387e91b90f1900bedf88bc92d7929ccfc
model.safetensors.index.json        64,821 B      sha256 f994739fe38e5210b9e3e8ce6c6307315e2ceac3cb630e7b7414d69dce520f60
```

官方 `Qwen3-ASR-1.7B-hf` tokenizer：

```text
tokenizer.json 11,429,653 B sha256 fe1fad59be22a41ee293363fcf95fdedbc7c93f3b49270b1d2e18bd1399a7a05
```

下一会话必须先修改 PRD/design/plan：1.7B 为正式 installable/executable，Qwen Provider 支持 sherpa 0.6B 与 Candle/Metal 1.7B 两种 runtime identity。Task 14、packaged smoke 和 DMG smoke 必须包含 1.7B。

## 7. Local Tool / `lifesubd` 决策

用户批准：C 作为实施路径，A 作为最终架构。

- C：contract-first，Tauri 暂托管唯一 `CoreRuntime` 和 Unix socket。
- A：launchd 常驻 `lifesubd` 托管同一 Core；Tauri、DeepSeek Harness、Gateway 是客户端。
- SQLite、capture、model manager、reconciliation、ASR worker 只有一个 Core owner。
- 插件/Gateway 不得调用 Tauri Command、打开 SQLite 或读取内部路径。
- ChatGPT 通过认证 Gateway 映射 MCP，daemon 不直接暴露网络。
- V0.2 不实现真实 native capture；`get_capabilities` 必须诚实返回 unsupported。

## 8. 推荐恢复顺序

1. 先隔离 Task 5 RED module，审计并完成 Task 4 未提交安全修复。
2. 重新运行 Task 4 spec/quality review，单独提交 Task 4 fix。
3. 审查未提交 Local Tool 文档修订，补完后重新 architecture review，单独提交。
4. 更新 Qwen 1.7B PRD/design/plan，做独立规格审查。
5. 重写 Task 5 failing tests，使 1.7B 为 installable Candle/Metal 模型；确定 multi-file artifact/bundle identity、tokenizer provenance和下载事务。
6. 完成 Task 5 manifests 与第三方 notices，双审后继续 Tasks 6-15。

后续任务顺序：

```text
Task 5  Model/VAD manifests + Qwen 1.7B runtime identity
Task 6  recoverable multi-file/archive downloads and installs
Task 7  decode/resample/VAD/timestamps
Task 8  SenseVoice/Whisper + Qwen 0.6 sherpa + Qwen 1.7 Candle providers
Task 9  fenced jobs
Task 10 atomic Receipt/Revision
Task 11 CoreRuntime + Tool Contract V1 + Tauri/UDS adapters
Task 12 settings/model UI
Task 13 job/provenance/retranscription UI
Task 14 real-model Gate including both Qwen sizes
Task 15 desktop/DMG/release verification
```

## 9. 已知质量要求与坑点

- 不得静默 fallback Provider 或模型。
- 1.7B 不得回退到 0.6B。
- 只有 manifest hash/required files/runtime identity 全部匹配才可执行。
- Model files 不提交仓库。
- `ProviderReceipt` draft 不得用于绕过验证；Task 10 增加 controlled getters/into_parts。
- Task 11 结构化返回稳定 `AsrErrorCode`，不要 `format!("{error:?}")` 暴露第二套错误。
- Task 11 full Core lock 必须早于 Catalog migration/reconciliation。
- Task 1 Minor：`with-sherpa-runtime.sh` fetcher 路径应加引号以兼容含空格路径。
- TypeScript/JavaScript 禁止 `console.log`。
- UI 样式只用 `docs/design/tokens/base.json`。

## 10. 新会话建议首条指令

```text
请读取 docs/prd/lifesub-real-asr-v0.2/.artifacts/handoff.md 和所有 process.md，进入开发模式。先不要清理工作树：按 handoff 分组审计未提交 Task 4、Task 5 RED 和 Local Tool 文档改动。优先完成 Task 4 symlink/full Core lock 修复的验证、双审和独立提交；随后修订并审查 Qwen3-ASR 1.7B Candle/Metal 方案，再继续 Task 5。
```
