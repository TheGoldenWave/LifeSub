# LifeSub / 旁白

> 让生活与工作中的重要声音，成为个人可控、Agent 可用的长期记忆。

LifeSub（中文名：旁白）是一个本地优先的个人记忆系统。它从 macOS 会议音频和麦克风开始，完成录音、转写、摘要、检索与证据回溯，并通过插件把个人记忆安全地提供给 Codex、DeepSeek Harness、Malow 等 Agent。

LifeSub 由 goldenwave 与 Malow 产品协同打造，可作为 Malow 生态插件，也面向其他 Agent 产品提供通用能力。长期方向是"可穿戴录音硬件 + 桌面与移动伴侣 + 个人记忆核心 + Agent 插件生态"；首版坚持软件先行。

## 当前版本：V0.2

> 开发源护栏（2026-08-22）：唯一开发/发布工作树为 `/Users/goldenwave/Documents/MyProject/LifeSub`，分支 `main`。原外部 worktree 仅作回滚和补丁比较；历史 DMG 和 `/Applications/LifeSub.app` 不是源码真值。

当前已完成 V0.2 UI/Catalog 安全闭环，以及 V0.2.1 原生采集计划 Task 1--6：Rust/Swift 采集协议、ScreenCaptureKit 系统音频、AVAudioEngine 麦克风、arm64 sidecar 构建签名、继承 FD nonce 和 UID/PID/可执行文件身份认证。这些能力尚未经 Task 7/8 接入 Catalog 原子 chunk sealing 和 production coordinator，因此桌面生产路径仍 fail closed，不回退 Mock 或伪报保存成功。

### 已完成

| 模块 | 状态 |
|------|------|
| 前端 4 页面架构（录音 / 时间线 / 词典 / 设置弹窗） | ✅ |
| 浏览器 Demo 预览 | ✅ 显式标识，不录音、不保存、不调用本地 ASR |
| 桌面 Catalog 时间线、搜索、统计与错误重试 | ✅ 真实数据 |
| 音频导入、hash、不可变 Chunk 与 durable Job outcome | ✅ |
| 时间戳笔记（待办/备忘/问题/决定） | ✅ Catalog 持久化 |
| 词典管理（分类/词条/别名/启用停用） | ✅ Catalog 持久化 |
| ASR Provider + 精确 model_id 设置 | ✅ Catalog 持久化，禁止静默换模型 |
| revision 历史、人工修订与按 Segment/Chunk 回放 | ✅ |
| 设置弹窗焦点锁定、多 Modal 栈与响应式布局 | ✅ |
| 声纹库（FunASR CAM++ embedding） | UI/CRUD 就绪，embedding 提取未接通 |
| 后端 Catalog V5 迁移（notes / dictionary / voiceprints / settings） | ✅ |
| 后端 22 个 Tauri 命令（CRUD + 统计 + 配置） | ✅ |
| 后端 idempotency + mutation flow | ✅ |
| ASR Provider/模型/Receipt/Job 基础设施（SenseVoice / Whisper / Qwen3-ASR） | ✅；production executor 待接通 |
| 模型管理（下载/安装/卸载） | ✅ |
| 本地 LLM 润色与快速输入基础设施 | ✅ 失败显式；禁止静默 Mock |
| V0.2 arm64 安装包与 `/Applications/LifeSub.app` | ✅ 已重新打包、签名验证并替换 |

### 进行中

| 模块 | 状态 |
|------|------|
| ScreenCaptureKit + AVAudioEngine helper | 已实现、测试和复审；待 Task 7/8 接入 Catalog/production coordinator |
| Catalog v6 capture timing + atomic chunk sealing | Task 7 RED，暂停中 |
| production NativeCaptureCoordinator | 未实现；当前桌面路径 fail closed |
| native ASR production executor | 待接通；Job worker 生命周期已完成，执行器当前 fail closed |
| 模型安装/下载交互 | 后端基础设施已完成，UI 暂禁用并标注计划中 |
| 说话人分离（Diarization）与 CAM++ embedding | 待开发 |

### 规划中

| 模块 | 设计文档 |
|------|---------|
| Agent 插件生态（MCP Server） | Phase 2 |
| 隐私同步（GitHub 加密记忆库） | Phase 3 |

### 当前质量门禁

- Rust Tier 2：`504 passed / 0 failed / 7 ignored`，`cargo fmt`、Clippy `-D warnings`、`git diff --check` 通过。
- 前端：Vitest `69/69`、Playwright `32/32`、TypeScript/Vite 生产构建通过。
- 源码版本：`package.json`、Cargo 与 Tauri bundle 统一为 `0.2.1`。
- 当前 `/Applications/LifeSub.app` 和历史 `0.1.0` DMG 早于本轮主工作树合并，只作为历史运行记录，不是当前源码的发布证据；Tasks 7--12 与硬件验收通过前不生成或安装新的发布候选。

## 生态关系

LifeSub、[malow / 吗喽](https://github.com/TheGoldenWave/malow) 与 [GoldenWave](https://github.com/TheGoldenWave/goldenwave) 组成一条解耦链路：**LifeSub 记录现实，Malow 处理工作，GoldenWave 治理长期上下文。**

| 项目 | 职责 | 权威数据 |
|---|---|---|
| LifeSub | 采集声音，完成 ASR、说话人、时间戳、检索与证据回溯 | 原始音频、Transcript、Session、Evidence Segment |
| Malow | 在 Project / Matter 中引用 LifeSub 证据，整理主题、决定、行动项和候选内容，并提供人工 Review | Project、Matter、Conversation、Agent Run、Review、Knowledge Patch Draft |
| GoldenWave | 接收人工确认后的 Knowledge Patch，继续执行路由、冲突、新鲜度、敏感度与 Git 审计治理 | Profile、Knowledge、Persona、正式 Project Context 与治理历史 |

主链路是：

```text
LifeSub Evidence
  -> Malow Organizer / Review
  -> user-confirmed Knowledge Patch
  -> GoldenWave Inbox
  -> GoldenWave route / score / render / inject
```

Malow 可以作为 LifeSub ASR 结果的主要处理层，但不是必经层：与明确 Project / Matter 相关的记录优先经 Malow 整理和确认；健康、生活、偏好等非项目内容可以由 LifeSub 通过同一 GoldenWave Contract 直接提交候选。Malow 的"接受"只表示候选已获用户确认并提交，不能替代 GoldenWave 的正式治理。

三个项目保持独立源码仓库、数据库和发布节奏，不共享 SQLite，不复制彼此的权威数据。Malow 只保存稳定的 LifeSub Evidence Ref；LifeSub 不维护 Project 状态；任何 producer 都不得绕过 GoldenWave Inbox 直接修改正式 Profile / Knowledge / Persona。

## 技术栈

| 层 | 技术 |
|----|------|
| 桌面框架 | Tauri 2 + Rust |
| 前端 | React 19 + TypeScript + Vite |
| 样式 | CSS Custom Properties（设计 Token 体系） |
| 数据库 | SQLite（Catalog V5） |
| ASR 引擎 | sherpa-onnx（SenseVoice / Whisper / Qwen3-ASR） |
| 声纹识别 | FunASR CAM++（规划中） |
| 本地 LLM | Ollama CLI / Qwen2.5（基础管道已接入，严格失败反馈） |
| 测试 | Vitest + Testing Library（前端）/ Rust 内置测试（后端） |

## 文档

- [产品定义](docs/product-brief.md)
- [系统架构草案](docs/architecture.md)
- [Agent 插件与工具](docs/integrations.md)
- [隐私、权限与 GitHub 同步](docs/privacy-and-sync.md)
- [市场与技术参考](docs/research.md)
- [阶段路线图](docs/roadmap.md)
- [决策记录](docs/decisions.md)
- [后端模块设计 (Task 13.5)](docs/superpowers/plans/2026-08-18-lifesub-backend-tasklist.md)
- [UI 重构设计](docs/superpowers/plans/2026-08-18-lifesub-ui-redesign.md)
- [LLM 后处理 + Fn 键快速输入](docs/superpowers/plans/2026-08-18-lifesub-llm-quick-input.md)

## 仓库边界

本仓库是公开的产品与代码仓库，不存放任何真实录音、转写、个人记忆、密钥或用户配置。LifeSub 的 GitHub 记忆同步必须指向用户单独创建的私有仓库。

## License

许可证尚未确定。在许可证明确之前，不授予复制、修改或分发本仓库内容的许可。
