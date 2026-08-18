# LifeSub V0.2 后续开发计划

> **分支**: `codex/lifesub-real-asr-v0.2`
> **当前阶段**: Task 13.5 完成，前端 UI 重构完成，后端 22 个新命令就绪
> **下次会话恢复**: 读取 `docs/prd/lifesub-real-asr-v0.2/.artifacts/process.md`

---

## 现状总览

```
已完成 ✅
├── Tasks 1-13: 真实本地 ASR 引擎 + 模型管理 + Job 调度 + 原子发布 + Tool API
├── Task 13.5: 笔记/词典/声纹/统计/设置后端模块（22 个 Tauri 命令）
├── UI 重构: 4 页面架构（录音/时间线/词典/设置弹窗）+ 设计 Token 体系
└── 构建: npm build ✅ / cargo build ✅ / tauri build ✅（.app + .dmg）

待完成 ⬜
├── Task 14: 真实 Provider Gate 验证（运行中）
├── Task 15: Playwright E2E + 响应式 + 打包 Gate
├── 前端 API 接入: 替换 Demo 数据为真实后端调用
├── 流式 ASR: 实时转写通道
├── 说话人分离: Diarization + CAM++ 声纹
├── LLM 后处理: 本地 LLM 润色管道
└── Fn 键快速输入: 全局快捷键 + 光标写入
```

---

## Phase 1: V0.2 收尾 — 完成原有计划

### Phase 1.1: 前端 API 接入（当前阻塞）

> 优先级: **P0** — 前端所有页面仍用 demo 数据，无法验证后端

| 页面 | 接入的 API |
|------|-----------|
| 录音页 | `createNote` / `listNotes` / `deleteNote` |
| 时间线 | `getStatsSnapshot`（替换 demo 统计） |
| 词典页 | `listCategories` / `listEntries` / `createEntry` / `updateEntry` / `toggleEntry` / `deleteEntry` |
| 设置弹窗 | `getAsrConfig` / `setAsrConfig` / `getRecordingConfig` / `setRecordingConfig` / `listVoiceprints` |

**预估**: 1-2 个 session，不涉及新 Rust 代码

### Phase 1.2: Task 14 — 真实 Provider Gate 验证

> 优先级: **P0** — 原计划 QA 里程碑

- 运行 `lifesub-asr-gate` 对 SenseVoice / Whisper / Qwen3-ASR 0.6B / 1.7B 做 CER/WER/RTF 基准测试
- 生成 `output/asr-v0.2/fixture-results.json` 证据
- M4/24GB 设备上验证 1.7B Metal 推理

**预估**: 1 个 session（需要物理设备 + 模型下载）

### Phase 1.3: Task 15 — Playwright E2E + 打包 Gate

> 优先级: **P0** — 原计划 QA 里程碑

- 编写 Playwright 浏览器场景（Provider 切换、模型状态、Job 状态映射）
- 实现桌面验收 harness（`--acceptance-scenario`）
- 完成 DMG 签名验证 + peer-auth Gate
- 生成视觉证据（截图）和验证文档

**预估**: 1-2 个 session

---

## Phase 2: 核心能力补全

### Phase 2.1: 流式 ASR 实时通道

> 优先级: **P0** — 录音页目前只能展示 Demo 模拟数据

- 后端: ASR 引擎实时输出 `LiveSegment` 流，通过 Tauri Event 推送到前端
- 前端: `LiveCapture` 组件监听事件，实时追加段落，支持说话人标注
- 同时支持完整转录持久化（Job 模式）

**预估**: 1-2 个 session

### Phase 2.2: 说话人分离 + CAM++ 声纹

> 优先级: **P1** — 录音页的声纹标注功能

- 后端: 集成 FunASR CAM++ embedding 提取 + 声纹库比对
- 前端: 未知说话人重命名 → 自动保存声纹
- 支持 Qwen3-ASR 自带 Diarization 的变体路径

**预估**: 1-2 个 session（主要工作在 Rust 端集成 CAM++）

---

## Phase 3: LLM 后处理 + Fn 键快速输入（新功能）

### Phase 3.1: 本地 LLM 后处理管道

> 优先级: **P1** — 提升 ASR 输出质量

| 步骤 | 内容 |
|------|------|
| 选型 | 本地 LLM（Qwen2.5-0.5B GGUF / llama.cpp），Apple Silicon 优化 |
| 集成 | ASR 段落完成后 → 发送到 LLM → 润色（去口头禅/纠错/格式化） |
| 前端 | 录音页展示"原始转写"和"润色后"两个版本 |

**预估**: 1-2 个 session

### Phase 3.2: Fn 键快速输入

> 优先级: **P1** — 即按即说即上屏

| 步骤 | 内容 |
|------|------|
| 全局快捷键 | `tauri-plugin-global-shortcut` 注册 Fn 键 |
| ASR 切片 | Fn 按下/松开记录时间戳，从 ASR 流中提取区间文本 |
| LLM 润色 | 拼接文本 → 本地 LLM 润色 |
| 光标写入 | macOS Accessibility API 模拟键盘输入 |
| 状态提示 | 菜单栏图标短暂变色 |

**预估**: 1-2 个 session

### Phase 3.3: 场景感知（可选）

> 优先级: **P2** — 锦上添花

- 获取前台 App bundle identifier
- 根据 App 类型（Slack/邮件/VS Code）调整 LLM prompt
- 用户可配置每个 App 的语气偏好

**预估**: 1 个 session

---

## 推荐执行顺序

```
Phase 1.1 (前端 API 接入) ──────┐
                                 ├──→ Phase 1.2 (Task 14 Gate)
                                 ├──→ Phase 1.3 (Task 15 E2E)
                                 │
Phase 2.1 (流式 ASR) ───────────┤
                                 ├──→ Phase 2.2 (说话人分离)
                                 │
Phase 3.1 (LLM 润色) ───────────┤
                                 └──→ Phase 3.2 (Fn 键)
                                      └──→ Phase 3.3 (场景感知)
```

- Phase 1.1 可以立即开始（不依赖新 Rust 代码）
- Phase 1.2 和 1.3 需要物理设备，可以并行准备
- Phase 2 依赖 Phase 1.1 的前端 API 接入
- Phase 3 依赖 Phase 2.1 的流式 ASR 通道

**关键路径**: 前端 API 接入 → 流式 ASR → LLM 润色 → Fn 键，共约 5-6 个 session

---

## 里程碑

| 里程碑 | 退出条件 |
|--------|---------|
| M1: V0.2 前端闭环 | 所有页面使用真实 API，不再依赖 demo 数据 |
| M2: V0.2 QA 完成 | Task 14 + 15 全部通过，证据文档生成 |
| M3: 实时 ASR 可用 | 录音页展示真实流式转写，支持说话人标注 |
| M4: LLM 润色可用 | ASR 输出自动经过本地 LLM 后处理 |
| M5: 快速输入可用 | 按住 Fn 说话 → 松开上屏，零跳转 |

---

## 技术债务

| 项目 | 说明 |
|------|------|
| 前端 demo 数据替换 | `LiveCapture` / `DictionaryView` / `StatsBar` / `SettingsModal` 仍用 demo 数据 |
| 流式 ASR 事件通道 | 后端有 ASR 引擎但无实时推送 |
| CAM++ 集成 | 声纹表已建但无 embedding 提取 |
| V4→V5 迁移 fixture | `lifesub-v0.4.sqlite3` 需要升级到 V5 fixture |
| `asr_provider_test` 已知失败 | `verified_sherpa_provider_consumes_held_fds_and_releases_them_on_drop` 持续失败 |