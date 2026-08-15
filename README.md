# LifeSub / 旁白

> 让生活与工作中的重要声音，成为个人可控、Agent 可用的长期记忆。

LifeSub（中文名：旁白）是一个本地优先的个人记忆系统。它从 macOS 会议音频和麦克风开始，完成录音、转写、摘要、检索与证据回溯，并通过插件把个人记忆安全地提供给 Codex、DeepSeek Harness、Malow 等 Agent。

LifeSub 由 goldenwave 与 Malow 产品协同打造，可作为 Malow 生态插件，也面向其他 Agent 产品提供通用能力。长期方向是“可穿戴录音硬件 + 桌面与移动伴侣 + 个人记忆核心 + Agent 插件生态”；首版坚持软件先行。

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

Malow 可以作为 LifeSub ASR 结果的主要处理层，但不是必经层：与明确 Project / Matter 相关的记录优先经 Malow 整理和确认；健康、生活、偏好等非项目内容可以由 LifeSub 通过同一 GoldenWave Contract 直接提交候选。Malow 的“接受”只表示候选已获用户确认并提交，不能替代 GoldenWave 的正式治理。

三个项目保持独立源码仓库、数据库和发布节奏，不共享 SQLite，不复制彼此的权威数据。Malow 只保存稳定的 LifeSub Evidence Ref；LifeSub 不维护 Project 状态；任何 producer 都不得绕过 GoldenWave Inbox 直接修改正式 Profile / Knowledge / Persona。

## 当前状态

项目处于产品与架构设计阶段。已经确认的首版方向：

- macOS 菜单栏采集系统音频与麦克风
- 本地 ASR 默认开启，同时保留云端 ASR Provider
- 摘要模型可配置，支持接入 Codex、DeepSeek 等服务
- 本地核心服务统一负责记忆、权限、索引、同步和审计
- Codex、DeepSeek Harness、Malow 使用各自原生插件形态接入
- Agent 按敏感级别访问内容：普通记忆可返回原文，敏感记忆需要更严格授权
- GitHub 用于分级同步：普通摘要可读、敏感记录加密、原始音频默认仅留本地

## 文档

- [产品定义](docs/product-brief.md)
- [系统架构草案](docs/architecture.md)
- [Agent 插件与工具](docs/integrations.md)
- [隐私、权限与 GitHub 同步](docs/privacy-and-sync.md)
- [市场与技术参考](docs/research.md)
- [阶段路线图](docs/roadmap.md)
- [决策记录](docs/decisions.md)

## 仓库边界

本仓库是公开的产品与代码仓库，不存放任何真实录音、转写、个人记忆、密钥或用户配置。LifeSub 的 GitHub 记忆同步必须指向用户单独创建的私有仓库。

## License

许可证尚未确定。在许可证明确之前，不授予复制、修改或分发本仓库内容的许可。
