---
stage: phase-2.1-complete
last_updated: 2026-08-18
source: codex
---

# LifeSub 真实本地 ASR V0.2 进度

- **HEAD: Phase 2.1 完成**。流式 ASR 实时通道已实现：后端 StreamingCapture 通过 Tauri Event 推送 LiveSegment，前端 LiveCapture 监听事件实时追加段落。
- **下一步**: Phase 2.2 — 说话人分离 + CAM++ 声纹，详见 `docs/superpowers/plans/2026-08-18-lifesub-roadmap.md`

## Phase 2.1 完成（流式 ASR 实时通道）

- ✅ `src-tauri/src/capture/streaming.rs` — StreamingCapture 服务，MockStreamingSource 开发模式，StreamingSource trait 供生产 ASR 接入
- ✅ `src-tauri/src/capture/mod.rs` — 模块入口，`#[cfg(feature = "desktop")]` 特性门控
- ✅ `src-tauri/src/commands.rs` — `start_streaming_capture` / `stop_streaming_capture` 命令，AppState 持有 Mutex<StreamingCapture>
- ✅ `src-tauri/src/lib.rs` — 注册 capture 模块 + 2 个新命令
- ✅ `src/services/lifesub.ts` — `startStreamingCapture` / `stopStreamingCapture` invoke 封装
- ✅ `src/components/LiveCapture.tsx` — 监听 `asr-live-segment` 事件，实时追加 LiveSegment，Tauri 模式 fallback demo
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
- `asr_provider_test`: 已知 1 个测试持续失败

## 设计文档
- `docs/superpowers/plans/2026-08-18-lifesub-roadmap.md` — 后续开发总计划
- `docs/superpowers/plans/2026-08-18-lifesub-backend-tasklist.md` — Task 13.5 后端模块设计
- `docs/superpowers/plans/2026-08-18-lifesub-ui-redesign.md` — UI 重构设计
- `docs/superpowers/plans/2026-08-18-lifesub-llm-quick-input.md` — LLM 后处理 + Fn 键设计