# LifeSub / 旁白

> 把持续发生的声音，可靠地保存为可定位、可审计、可被上层系统引用的证据。

LifeSub（中文名：旁白）是一个本地优先的长时音频与 ASR 证据管理系统。它负责录音、音频分片、ASR、文本分片、受约束校对、Markdown 投影、基础检索与来源定位，不负责记忆压缩、项目理解或长期知识治理。

LifeSub 的长期方向是“桌面与移动伴侣 + 可穿戴或外部录音设备 + 本地 Evidence Core + 开放 Evidence Contract”。首版坚持软件先行，优先验证长时录音、可靠转写与下游证据引用闭环。

## 生态关系

LifeSub、[malow / 吗喽](https://github.com/TheGoldenWave/malow) 与 [GoldenWave](https://github.com/TheGoldenWave/goldenwave) 组成一条解耦链路：**LifeSub 保存发生过什么，Malow 判断这对当前工作意味着什么，GoldenWave 治理什么值得长期相信和复用。**

| 项目 | 职责 | 权威数据 |
|---|---|---|
| LifeSub | 长时录音、音频与文本分片、ASR revision、Markdown 投影、基础检索与证据授权 | Capture Session、Audio Chunk、Transcript Revision、Transcript Segment、Evidence Ref |
| Malow | 在 Project / Matter 中引用 LifeSub 证据，整理主题、决定、行动项和候选内容，并提供人工 Review | Project、Matter、Conversation、Agent Run、Organizer Result、Knowledge Patch Draft |
| GoldenWave | 接收人工确认后的 Knowledge Patch，执行验证、冲突、新鲜度、敏感度、渲染、注入与 Git 审计 | Profile、Knowledge、Persona、正式 Project Context 与治理历史 |

主链路是：

```text
LifeSub Evidence
  -> Malow Organizer / Review
  -> user-confirmed Knowledge Patch
  -> GoldenWave Inbox
  -> GoldenWave Governance
```

项目型与非项目型记录都必须先经过 Malow 或等价的人工 Review 入口形成候选，才能进入 GoldenWave Inbox。LifeSub 不直接生成或写入 GoldenWave 正式知识。

三个项目保持独立源码仓库、数据库和发布节奏，不共享 SQLite，不复制彼此的权威数据。Malow 只保存稳定的 LifeSub Evidence Ref、hash、授权范围和必要快照；任何下游系统都不得直接读取 LifeSub 数据库。

## 当前状态

V0.1 基础版本已经具备可运行的纵向闭环：

- React + Tauri 桌面应用壳与本地 Evidence Core
- 开始、暂停、恢复和停止记录的明确状态机
- 音频文件导入、内容 hash、本地不可变副本与 SQLite Catalog
- append-only Transcript Revision 与中文关键词检索
- 时间线、记录详情、Evidence URI、人工修订与 Markdown 导出
- 本地 Provider、隐私和数据位置设置页
- 浏览器演示模式与 Tauri 桌面持久化模式

## 本地运行

```bash
npm install
npm run dev
```

浏览器预览使用内置演示数据，可体验录音状态、搜索、revision、导出和设置。运行桌面版：

```bash
npm run tauri -- dev --features desktop
```

验证首版：

```bash
npm test
npm run build
npm run test:e2e
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features
cargo check --manifest-path src-tauri/Cargo.toml --features desktop
```

## V0.1 边界

当前内置的是确定性演示 ASR Provider，用于验证完整 Evidence 流程；真实本地 ASR 模型与 macOS ScreenCaptureKit / AVAudioEngine 双路采集适配器将在下一迭代接入。文件导入、会话状态、持久化 Catalog、revision、检索和导出已经可运行。

已经确认的后续方向：

- macOS 菜单栏手动长时录制，分别采集系统音频与麦克风
- 原始音频滚动写入不可变、有界、可恢复的 Physical Audio Chunk
- 本地 ASR 默认开启，同时保留独立授权的云端 ASR Provider
- Transcript 使用不可变 revision，支持确定性规则、受约束 LLM 与人工校对
- 基于时间戳、静音、长度和录制状态形成 Logical Transcript Segment，不做主题级语义切分
- 将记录投影为可再生 Markdown；可选导航摘要仅用于浏览，不是正式记忆或知识
- 使用 FTS5 按时间、来源、设备和文本关键词进行基础检索
- 通过版本化 Evidence Contract 向 Malow 等消费者提供授权证据
- LifeSub 只审计录音、处理、证据访问、导出、删除与撤回，不承担 GoldenWave 知识治理审计
- GitHub 不作为全天音频与转写的主同步或存储通道

## 明确不做

- 跨记录记忆压缩、人物关系和长期事实推断
- 决定、行动项、项目状态或 Knowledge Candidate 的权威抽取
- Profile、Persona、Knowledge 或正式 Project Context
- Malow Project / Matter / Organizer / Agent Run 状态
- 直接生成或修改 GoldenWave 正式知识

## 文档

- [LifeSub Design System](design.md)
- [Logo 与 macOS 菜单栏呈现决策](docs/design/lifesub-logo-decision.md)
- [Evidence Platform 产品与技术架构设计](docs/superpowers/specs/2026-08-15-lifesub-evidence-platform-design.md)
- [产品定义](docs/product-brief.md)
- [系统架构](docs/architecture.md)
- [Evidence API 与集成](docs/integrations.md)
- [隐私、权限与同步](docs/privacy-and-sync.md)
- [市场与技术参考](docs/research.md)
- [阶段路线图](docs/roadmap.md)
- [决策记录](docs/decisions.md)

## 仓库边界

本仓库是公开的产品与代码仓库，不存放任何真实录音、转写、声纹、Evidence、密钥或用户配置。真实数据必须保存在用户选择的本地数据目录或未来明确设计的加密对象同步空间中，不得提交到本仓库。

## License

许可证尚未确定。在许可证明确之前，不授予复制、修改或分发本仓库内容的许可。
