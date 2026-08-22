---
stage: workspace-consolidated-task7-pending
last_updated: 2026-08-22
source: codex-safe-worktree-merge
---

# LifeSub 真实本地 ASR V0.2 进度

## 2026-08-22 主工作树安全合并

- 唯一开发/发布源改为 `/Users/goldenwave/Documents/MyProject/LifeSub` 的 `main`。
- 合并输入：主工作树保全提交 `dc62d5b` + 最新集成分支 `652abf7`。
- 外部 worktree `/Users/goldenwave/.config/superpowers/worktrees/LifeSub/lifesub-real-asr-v0.2` 降级为回滚/补丁比较来源，不得继续开发或打包。
- 原生采集 Task 1--6 和 `CompatibleNewer` Catalog 启动修复已纳入合并结果。
- 下一产品实现里程碑仍是 Task 7 Catalog v6 + atomic chunk sealing；当前 production capture/native ASR 仍 fail closed。

## 2026-08-21 暂停与 Handoff（历史，已被 2026-08-22 合并取代）

- 应用户要求已暂停开发，当前发布源 HEAD 为 `c68dc0d`。
- Task 1--6 已完成并通过最终复审；Task 7 仅刚进入 migration RED，尚未编写 v6 生产 DDL 或 chunk sealing 实现。
- 当前唯一 Task 7 新增未提交改动为 `src-tauri/src/catalog_migration_test.rs` 中的 `migrates_v5_to_v6_capture_timing_without_rewriting_existing_chunks`；已观察到正确 RED：当前 `user_version` 仍为 5。
- 完整接管文档：`docs/handoffs/2026-08-21-lifesub-v0.2.1-native-capture-handoff.md`。
- 暂停时未打包、未安装、未覆盖 `/Applications/LifeSub.app`；production runtime 仍保持 fail-closed。

## 2026-08-21 V0.2.1 集成任务进度

- Task 1 已完成并通过复审：Sidebar 移除「导入音频」，Timeline 保留导入工作流；提交 `e516845`。
- Task 2 已完成并通过最终质量复审：发布根、分支、版本、dirty count 与 `--locked` 均受门禁约束；production runtime 证明改为 crate 内 sealed native capture/native ASR 类型能力，自报 bool 无法伪造。主要提交 `e538ba7` / `ec7d33b` / `a022fb5` / `371faea` / `f69fe7d`。
- Task 2 验证：release-source shell fixture PASS，`npm run test:release-source` PASS，`cargo fmt --check` PASS，`git diff --check` PASS；当前 production alias 仍为 fail-closed，正式 gate 按预期拒绝，planned audit 明示 `not release-ready`。
- Task 3--6 已完成并通过最终复审：版本化 Rust/Swift 协议、ScreenCaptureKit/AVAudioEngine 双路 helper、arm64 sidecar 构建签名、继承 FD nonce + UID/PID/可执行文件身份认证均已完成。
- 当前接续点仍是 Task 7；2026-08-22 后必须从主路径 `main` 继续，不得返回外部 worktree。

## 2026-08-21 工作区审计与安装源确认

- 当前安装包来源为本 worktree 当前 dirty tree，不是 `main`；bundle 版本已统一为 `0.2.1`。
- 安装二进制 SHA-256：`6b8b7e17f4f509a95434560141b05793f31503530405dc242e8b1b7e620d8f8c`；签名验证通过，Finder 正常启动且无新崩溃。
- 本 worktree 是当前 UI/Catalog/安全收口最完整的可运行基线，但仍有大量未提交改动，需要与 `main` 明确集成后才能形成唯一发布分支。
- `main` 的“15 Task 完成”报告不能作为安装包功能证据；其生产命令仍有 stub 且关键测试模块被注释。
- 实时麦克风/系统音频采集未完成；当前没有 ScreenCaptureKit/AVAudioEngine production adapter，界面“未接通”提示是正确的 fail-closed 状态。
- native ASR production executor 仍未接通；当前桌面 worker 使用 `FailClosedEngine`。
- Task 7/Task 9 的两个干净 worktree 目录已移除，分支引用保留。详细地图与打包护栏见 `docs/workspace-status.md`。

## 2026-08-20 宽窗口修复新包与桌面复核

- 旧 bundle 的 `.app` 时间为 2026-08-19 20:18，早于本轮宽窗口修复（20:42--20:45），因此重新以当前 worktree 打包。
- 前端验证：`npm test` 69/69 通过；`npx tsc -p tsconfig.app.json --noEmit` 与 `git diff --check` 通过。Rust Tier 2 的 fmt、clippy 与 diff 通过；完整 Rust 测试阶段受本次交互运行器的 30 秒输出窗口限制，未将其表述为本次新鲜全量通过证据。
- 直接 Tauri build 首次因未传入 Sherpa runtime attestation 被 `build.rs` fail-closed 拒绝；使用项目受校验 runtime 环境变量后完成新的 macOS `.app` 构建。
- 新 app 已执行 `codesign --force --deep --sign -`，并通过 `codesign --verify --deep --strict`。由该 app 重建并验证的 DMG：`src-tauri/target/release/bundle/dmg/LifeSub_0.1.0_aarch64-wide-ui.dmg`；DMG SHA-256 为 `88b15033ff66849e4ca9ea801ac7a0c69b4ecf7ac4226e841da7bd698acea30b`，内部 app 的可执行文件与构建 app 一致（SHA-256 `02f9bed36878efec06d18f6af6eda670acfb4bbbaa6dce0d30128f79a57caf72`）。
- 新 app 已安装到 `/Applications/LifeSub.app` 并启动。旧 app 保留为 `/Applications/LifeSub.app.pre-wide-ui-20260820`，便于恢复。
- 安装版宽窗口实测：录音页显示“待检测”与四项诊断清单；时间线空查询显示“暂无转写”，有查询无匹配时显示“没有匹配的原话”；词典工具栏/禁用的新建词条/右侧说明符合预期；设置模型页按 Provider 分组、动作列稳定且使用“暂不可安装”；设置对话框未占满窗口。
- 实测限制：当前设备没有接通真实实时采集或已安装模型，点击“开始记录”未进入可录制状态，因此仅验证了 fail-closed 的待检测状态与诊断，不把真实采集成功列为通过。

## 当前状态摘要（以 2026-08-21 Handoff 为准）

- 当前阶段：`v0.2.1-paused-task7-handoff`。
- 已完成：V0.2 UI/Catalog 安全闭环；原生采集计划 Task 1--6（Sidebar 入口、发布源门禁、协议、Swift 双路 helper、sidecar 签名、helper 认证监督）。
- 当前安装 App 是历史 fail-closed 构建，不包含已接通的 production coordinator/native ASR，不是继续开发或发布证据。
- 当前阻塞：Task 7 Catalog v6 + atomic chunk sealing、Task 8 production coordinator、Task 9 NativeAsrEngine、Task 10 persisted UI events、Task 11/12 验收与打包。
- 下一步：仅从 `src-tauri/src/catalog_migration_test.rs` 中已观察 RED 的 `migrates_v5_to_v6_capture_timing_without_rewriting_existing_chunks` 继续 Task 7 TDD。
- 放行原则：在上述真实链路完成前，保持 fail closed，不将 Demo、规则清理或 provider 不可用结果表述为真实录音/ASR 成功。

## 2026-08-19 UI 走查问题修复完成

- 已按 `ui-walkthrough-issues-2026-08-19.md` 完成 Critical/Important/Medium/Minor 的代码与交互收口。
- 生产路径不再输出 Mock 转写或静默 LLM Mock；浏览器数据持续标注为 Demo，桌面采集不可用时显式失败。
- 录音生命周期已加 generation/mutex 语义，覆盖 start/pause/resume/stop/error/restart 的交错；停止不再伪报保存成功。
- 导入使用单一 Core 命令，持久化 stopped session、Chunk 与 durable Job outcome；精确保存 `model_id`，禁止 Provider 下自动挑模型。
- Worker 使用唯一 coordinator，覆盖 recover/claim/renew/cancel/shutdown；不可用 runtime fail closed，不生成伪 revision。
- 时间线从 Catalog 加载，支持 Job/Chunk integrity、manual provenance、只读历史 revision 和按 Segment Chunk 绑定播放。
- 设置 Modal 已 portal 化并支持焦点锁定、多 Modal 栈、背景 inert 与响应式单列 body；词典 CRUD、错误重试和作用范围已补齐。
- 视觉已完成主次层级、中文字体角色、AA 次要文本、标签单行、边框降噪和窄窗口适配。
- 验证：Tier 2 全通过（Rust 484 passed / 0 failed / 6 ignored），前端 69/69，Playwright 12/12，生产构建通过，无 `console.log`，`git diff --check` 通过。
- 截图：`output/playwright/ui-walkthrough-fixed-desktop.png`、`ui-walkthrough-fixed-mobile.png`、`settings-fixed.png`、`settings-fixed-mobile.png`。
- 独立复审：录音、设置、Catalog/Worker 均为 Critical 0 / Important 0。

## 2026-08-19 重新打包与安装替换

- 使用受校验的 sherpa runtime attestation 执行 `tauri build --features desktop`，生成 arm64 `.app` 与 DMG。
- 完整 bundle 已重新执行 deep ad-hoc 签名并通过 `codesign --verify --deep --strict`。
- DMG 已由签名后的 `.app` 重新生成并通过 `hdiutil verify`。
- 已替换 `/Applications/LifeSub.app`，安装版可执行文件与构建产物 SHA-256 一致：`8ba5699ada608475b626b03dae6db1a0fe6aa2ff8aea45b253d00da1744a5b1a`。
- 安装版已启动并保持运行，进程路径为 `/Applications/LifeSub.app/Contents/MacOS/lifesub`。
- DMG：`src-tauri/target/release/bundle/dmg/LifeSub_0.1.0_aarch64.dmg`，SHA-256 `9bd72bda3da4245805e8bbae1e7f1a8cfe2992eaa430f7b0ba65b5037e1b0783`。

- 历史阶段记录保留如下；如与“当前状态摘要”冲突，以摘要为准。

## Phase 3 完成（LLM 后处理 + Fn 键快速输入 + 场景感知）

- ✅ `src-tauri/src/llm/polish.rs` — LLM 润色服务与 Ollama CLI 调用；历史 Mock 降级已废止，当前失败显式返回
- ✅ `src-tauri/src/quick_input.rs` — 全局快捷键（CommandOrControl+Shift+Space）+ `CGEventPost` 光标写入 + 场景感知
- ✅ `src-tauri/Cargo.toml` — 新增 `tauri-plugin-global-shortcut` 依赖
- ✅ `src-tauri/src/commands.rs` — 5 个新命令：`llm_polish` / `register_quick_input_hotkey` / `get_frontmost_app` / `paste_text_at_cursor`
- ✅ `src-tauri/src/lib.rs` — 注册 llm + quick_input 模块 + 5 个新命令 + global-shortcut 插件
- ✅ `src/services/lifesub.ts` — 5 个新 invoke wrapper
- ✅ `src/components/LiveCapture.tsx` — AI 润色按钮 + 原始/润色切换 + 快速输入状态指示器
- ✅ `src/styles.css` — 新增 `.quick-input-indicator` 样式
- ✅ 验证: TSC 0 错误，10/10 测试，npm build 成功，Rust 453/453 测试通过（0 失败）

## Phase 2.1 完成（流式 ASR 实时通道）

- ✅ `src-tauri/src/capture/streaming.rs` — StreamingCapture 生命周期与事件合同；生产 Mock Source 已禁用，采集未接通时显式失败
- ✅ `src-tauri/src/capture/mod.rs` — 模块入口，`#[cfg(feature = "desktop")]` 特性门控
- ✅ `src-tauri/src/commands.rs` — `start_streaming_capture` / `stop_streaming_capture` 命令，AppState 持有 Mutex<StreamingCapture>
- ✅ `src-tauri/src/lib.rs` — 注册 capture 模块 + 2 个新命令
- ✅ `src/services/lifesub.ts` — `startStreamingCapture` / `stopStreamingCapture` invoke 封装
- ✅ `src/components/LiveCapture.tsx` — 监听 `asr-live-segment` / error 事件；历史 Tauri fallback Demo 已废止，Demo 仅限浏览器预览
- ✅ `src/test/setup.ts` — mock `@tauri-apps/api/event` 使 Vitest 测试通过
- ✅ 验证: TSC 0 错误，10/10 测试，npm build 成功，Rust 449/450（1 个已知失败 unchanged）

## Phase 1.1 完成（前端 API 接入）

- ✅ `src/data/adapter.ts` — 统一数据适配层，Tauri 运行时走真实 invoke，dev/test fallback demo
- ✅ DictionaryView — 分类/词条 CRUD 通过 adapter 加载和持久化
- ✅ StatsBar — 24h 统计通过 adapter 加载
- ✅ AsrSettings — Provider/语言/VAD/ITN 配置 + 声纹库通过 adapter 加载
- ✅ RecordingSettings — 捕获模式/IM 检测/音频格式/存储通过 adapter 加载
- ✅ LiveCapture — 笔记 CRUD 通过 adapter 持久化
- ✅ 验证: TSC 0 错误，10/10 测试通过，npm build 成功

## 已完成

### Task 1-13: 真实本地 ASR 引擎
- ✅ 真实本地 ASR 引擎（SenseVoice / Whisper / Qwen3-ASR）
- ✅ 模型管理（下载/安装/卸载/版本化/原子恢复）
- ✅ Job 调度（claim/lease/fencing/cancel/recovery）
- ✅ 原子 Receipt/Revision/Segment/FTS 发布
- ✅ Catalog V4 迁移 + Tool API V1 + 安全本地 IPC
- ✅ 8 个 mutation 方法 + idempotency 集成
- ✅ Task 13: 449/455 测试通过

### Task 13.5: 后端模块补全（2026-08-18）
- ✅ Catalog V5 迁移（5 张新表: notes / dictionary_categories / dictionary_entries / voiceprints / settings）
- ✅ 22 个新 Tauri 命令（笔记 4 + 词典 8 + 声纹 5 + 统计+设置 5）
- ✅ 前端 invoke wrapper 全部就绪
- ✅ Rust 450/450 测试通过，TSC 0 错误，前端 10/10 测试通过
- ✅ `npm run build` + `cargo build` + `tauri build` 全部成功

### UI 重构（2026-08-18）
- ✅ 4 页面架构（录音 / 时间线 / 词典 / 设置弹窗）
- ✅ 流式 ASR 展示（说话人声纹标注、Demo 数据）
- ✅ 时间戳笔记（待办/备忘/问题/决定、Demo 数据）
- ✅ 会话树形目录 + 24h 录音统计条
- ✅ 词典管理（分类/词条/别名/启用停用）
- ✅ 声纹库 UI（FunASR CAM++ 规划中）
- ✅ 设置弹窗（录音设置 / ASR 设置 / 模型 / 关于）

## 待完成

| 阶段 | 内容 | 预估 |
|------|------|------|
| Phase 2.2 | 说话人分离 + CAM++ 声纹 | 1-2 session |
| Phase 3.1 | LLM 后处理管道 | 1-2 session |
| Phase 3.2 | Fn 键快速输入 | 1-2 session |
| Phase 3.3 | 场景感知（可选） | 1 session |

## 技术债务
- CAM++ 集成: 声纹表已建但无 embedding 提取
- V4→V5 迁移 fixture: 需要生成 `lifesub-v0.5.sqlite3`
- Gate 二进制: 指标协议已实现，待生产设备上集成真实 ASR provider
- 真实采集与 native provider executor 尚未接到生产 worker；当前 worker 显式失败，不发布伪 revision。
- bundle 版本仍为 `0.1.0`，待真实音频 Gate 通过后升级 V0.2。
- 模型安装/下载已有后端能力，但设置 UI 当前保持禁用“计划中”。

## 设计文档
- `docs/superpowers/plans/2026-08-18-lifesub-roadmap.md` — 后续开发总计划
- `docs/superpowers/plans/2026-08-18-lifesub-backend-tasklist.md` — Task 13.5 后端模块设计
- `docs/superpowers/plans/2026-08-18-lifesub-ui-redesign.md` — UI 重构设计
- `docs/superpowers/plans/2026-08-18-lifesub-llm-quick-input.md` — LLM 后处理 + Fn 键设计

## 2026-08-19 设置弹窗修复

- ✅ `Modal` 改为语义 `dialog` 面板：初始焦点落到关闭按钮，`Tab`/`Shift+Tab` 焦点循环，`Escape` 关闭，背景 sibling 自动 `inert`/`aria-hidden`，关闭后恢复触发按钮焦点。
- ✅ `SettingsModal` 增加 `tablist`/`tab`/`tabpanel` 语义，并拆到 `src/settings.css` 覆盖局部布局，避免双重 grid 在窄窗口和放大字体下压缩内容。
- ✅ `AsrSettings` / `RecordingSettings` 保存错误改为页内 `role="status"` 反馈；声纹“重命名/删除”接上真实 adapter 持久化；无样本来源的“注册新声纹”改为禁用的“计划中”。
- ✅ `ModelManager` / `AboutTab` 不再硬编码模型卡和版本号，改为读取运行时只读投影；模型未实现的安装/管理动作统一禁用，不再可点击无反馈。
- ✅ 证据：`npm test -- src/components/SettingsModal.test.tsx src/services/lifesub.test.ts` 通过（10/10）。
- ⚠️ 仓库现状：`npm run build` 仍被时间线页既有类型漂移阻断；`cargo check --manifest-path src-tauri/Cargo.toml --features desktop` 仍缺 sherpa 运行时构建环境变量，二者都不是本次设置修复引入。

## 2026-08-19 reviewer 第二轮收口

- ✅ `Modal` 支持多实例 stack：仅顶层实例处理 `Escape`/`Tab`，背景 inert 改为引用计数，`onClose` 通过 ref 保持最新回调且不重置焦点副作用。
- ✅ `SettingsModal` tabs 补齐方向键 roving，`Modal body` 在 `.modal-body > .settings-layout` 结构下保持单列，避免 reviewer 指出的双 grid 再现。
- ✅ `loadVoiceprints` / `loadAsrConfig` / `loadRecordingConfig` 在 Tauri 失败时不再回退 demo/default，错误由 `AsrSettings` / `RecordingSettings` 明示并提供 retry，保存按钮在加载失败时禁用。
- ✅ `AsrSettings` 声纹重命名/删除补齐 pending/禁用/error/status；`ModelManager` / `AboutTab` 增加加载失败提示与重试。
- ✅ `list_asr_models` 改为按 `(model_id, manifest_version, bundle_identity)` 精确匹配安装状态，并只选择当前 bundle 的最新下载记录；补了对应 Rust 单测。
- ✅ focused 证据：`npm test -- src/components/SettingsModal.test.tsx src/data/adapter.test.ts src/services/lifesub.test.ts` 通过（25/25）。
- ⚠️ 额外验证：`npx tsc -p tsconfig.app.json --noEmit` 仍被仓库现有 `src/components/StatsBar.tsx` 语法错误阻断；Rust focused test 继续被 Tauri allowlist 校验（`protocol-asset` 特性）阻断，不是本轮设置修复引入。

## 2026-08-19 大窗口响应式 UI 修复

- 基于 `ui-wide-window-walkthrough-2026-08-19.md` 的 23 项大窗口 UI 问题进行系统性修复。
- **P1 修复**（2/2）：
  - ✅ W-R01 录音页"准备就绪"状态矛盾：`statusTitle` 闲置时改为"待检测"（有 errorMessage 时显示"采集未就绪"），不再同时显示"准备就绪"与未接通诊断。
  - ✅ W-T01 时间线空搜索错误提示"没有匹配"：空查询无 segment 时按记录状态分"等待转写""暂无转写""转写不可用"；仅在有查询且无结果时显示"没有匹配的原话"。
- **P2 修复**（17/17）：
  - ✅ W-C01 主工作区高度链：所有页面使用 `min-height: 0` + `grid-template-rows: auto minmax(0, 1fr)` 完整高度链。
  - ✅ W-C02 宽度分配：宽窗口（≥1280px）增加 session tree 列宽至 280–360px、词典三栏弹性宽度、内容区 max-width 约束；≥1600px 进一步放宽。
  - ✅ W-C03 密度平衡：note panel 移除 `align-content: start` 使其填满等高；stats bar 内容居中约束；dictionary header 压缩为单行工具栏。
  - ✅ W-R02 诊断信息：改为带图标的状态清单，每项显示"名称 + 状态 + 提示动作"，阻塞项高亮。
  - ✅ W-R03 笔记面板：移除 `align-content: start`，笔记面板与录音工作区等高独立滚动。
  - ✅ W-R04 空态图标：💡 emoji 替换为 Lucide `Mic` 图标，与产品图标库一致。
  - ✅ W-T02 原始 ISO 标题：`createCapture` 标题改为本地化格式 `"实时记录 8/19 20:19"`。
  - ✅ W-T03 记录列过窄：session tree 在宽窗口下弹性宽度 280–360px。
  - ✅ W-T04 Evidence 信息条：宽窗口下内容居中约束，与 body 对齐。
  - ✅ W-T05 统计条：内容居中约束，不再无限拉伸。
  - ✅ W-D01 范围说明与选择器：合并为同一控件组，放在标题旁。
  - ✅ W-D02 顶部说明区过高：header 压缩为单行工具栏，标题与范围选择器同行。
  - ✅ W-D03 创建依赖关系：无分类时"新建词条"按钮 disabled，`title` 提示"请先创建分类"。
  - ✅ W-D04 说明栏过窄：宽窗口下 detail 栏弹性宽度 200–300px。
  - ✅ W-S01 模型动作列失稳：model card 改为 grid 布局，meta 区 `flex-shrink: 0` + `white-space: nowrap`。
  - ✅ W-S02 重复表达：非 runtime_qualified 模型显示"暂不可安装"替代"安装计划中"按钮。
  - ✅ W-S03 模型一次性展开：按 Provider 分组，SenseVoice 标记"（推荐）"。
  - ✅ W-S04 模态框满屏：settings layout `min-height` 从 40rem 降至 24rem。
  - ✅ W-S05 模型元数据对比度：从 `textMuted` 提升至 `textSecondary`。
- **P3 修复**（4/4）：
  - ✅ W-D05 底部说明条：footer 移除，内容合并到 detail panel 的"工作原理"字段。
- 验证：前端 69/69 测试通过，TypeScript 0 错误。
