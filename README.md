# LifeSub / 旁白

> 让生活与工作中的重要声音，成为个人可控、Agent 可用的长期记忆。

LifeSub（中文名：旁白）是一个本地优先的音频 Evidence 工具。它在 macOS 上采集或导入音频，完成本地转写、检索、回放与来源回溯。导航摘要和 Agent 插件仍属后续规划。

LifeSub 由 goldenwave 与 Malow 产品协同打造，可作为 Malow 生态插件，也面向其他 Agent 产品提供通用能力。长期方向是"可穿戴录音硬件 + 桌面与移动伴侣 + 个人记忆核心 + Agent 插件生态"；首版坚持软件先行。

## 当前版本

`0.2.1` 已完成真实双源采集、本地 ASR、Catalog Job/Receipt/Revision 发布，以及签名 DMG 和安装版验收。安装版仍是 `0.2.1`。`0.2.2` 正在进行轻量稳定化，源码版本已经更新；完成验证和重新打包前，不把它称为已发布版本。开发与发布源以 [工作区状态](docs/workspace-status.md) 为准。

| 已交付能力 | 当前边界 |
|---|---|
| 录音与导入 | 麦克风、系统音频、双源采集和音频导入；原始 Chunk 不可变 |
| 本地转写 | SenseVoice 与 Whisper 已通过安装版样本 Gate；精确模型身份、durable Job、Receipt、Revision 和 Segment 证据 |
| 找回与管理 | Catalog 时间线、搜索、按片段回放、Markdown 导出、笔记、词典及设置持久化 |
| 桌面可靠性 | 精确麦克风 UID、首帧测试、同 UID 恢复、诊断导出和模型下载/取消/重试 |
| 浏览器预览 | 仅演示布局；不会录音、保存或运行本地 ASR |

当前不提供真实说话人分离、CAM++ 身份识别、词典自动 ASR 修正或持久的 AI 校正版；临时润色只在页面预览。`0.2.2` 的范围是设置与状态表达、诊断操作和性能基线，不修改 Evidence 图谱。后续方向见 [路线图](docs/roadmap.md)，最新门禁见 [过程记录](docs/prd/lifesub-real-asr-v0.2/.artifacts/process.md)。

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
- [产品开放问题与验证议程](docs/product-open-questions.zh-CN.md)
- [阶段路线图](docs/roadmap.md)
- [决策记录](docs/decisions.md)
- [后端模块设计 (Task 13.5)](docs/superpowers/plans/2026-08-18-lifesub-backend-tasklist.md)
- [UI 重构设计](docs/superpowers/plans/2026-08-18-lifesub-ui-redesign.md)
- [LLM 后处理 + Fn 键快速输入](docs/superpowers/plans/2026-08-18-lifesub-llm-quick-input.md)

## 仓库边界

本仓库是公开的产品与代码仓库，不存放任何真实录音、转写、个人记忆、密钥或用户配置。LifeSub 的 GitHub 记忆同步必须指向用户单独创建的私有仓库。

## License

许可证尚未确定。在许可证明确之前，不授予复制、修改或分发本仓库内容的许可。
