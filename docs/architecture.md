# 系统架构草案

## 实现快照（2026-08-19）

- 当前代码仍由 Tauri 主进程托管 `CoreRuntime`，尚未迁移到 `lifesubd`。
- Core 已独占 Catalog、模型状态和 ASR Job coordinator；worker 具备 recover、claim、lease renew、cancel、fencing 和 shutdown 生命周期。
- 音频导入已使用 fd-anchored 存储、hash 和 Chunk integrity；时间线只读取 Catalog，桌面失败不得回退 Demo。
- ASR 设置持久化精确 `provider + model_id`，Provider 切换后必须显式选择模型，不允许按 registry 顺序自动替换。
- 人工 revision 从最新 revision 派生，保留 Chunk/time binding 与 `manual` provenance；混合或不可用 Chunk binding fail closed。
- 当前桌面 worker 的 production executor 仍为 fail-closed：不可用时写稳定 failed outcome，不生成 Mock transcript 或伪 revision。真实 provider 解码/VAD/推理执行是下一阶段。
- 实时采集源尚未接通；浏览器 Demo 只用于界面预览，桌面采集失败进入显式错误态。
- 2026-08-19 安装包仍标记 `0.1.0`，但包含 V0.2 UI/Catalog 整改代码；正式 V0.2 版本号等待真实采集与 native executor Gate。

## 已选方案

采用“契约先独立、进程后独立”的本地核心服务 + 薄客户端架构。LifeSub Core 是唯一的业务与数据真相来源；macOS App 和各 Agent 插件只负责交互、宿主适配与调用。

```text
macOS Menu Bar App
  |  system audio + microphone
  v
CoreRuntime（C 阶段由 Tauri 托管，A 阶段由 lifesubd 托管）
  |-- Capture and import
  |-- ASR provider router
  |-- Summary provider router
  |-- Memory and evidence store
  |-- Search index
  |-- Privacy policy and audit log
  |-- GitHub sync
  |
  |-- Shared protocol primitives
        |-- Core Application Contract V1 -> Tauri management mutations/lists
        |-- Agent Tool Contract V1 -> DeepSeek + Tauri trusted read/open projection
        |-- Authenticated MCP Gateway -> ChatGPT developer mode
```

## 建议的代码边界

```text
LifeSub/
├── apps/
│   └── macos/                  菜单栏采集与本地管理界面
├── services/
│   └── core/                   本地记忆服务
├── packages/
│   ├── schema/                 跨组件数据契约
│   ├── client/                 Core 客户端 SDK
│   ├── asr-providers/          本地与云端 ASR 适配器
│   └── summary-providers/      摘要模型适配器
├── plugins/
│   ├── codex/
│   ├── deepseek-harness/
│   └── malow/
└── docs/
```

此目录是设计意图，不是最终技术选型；正式规格批准前不创建实现脚手架。

## 核心组件

### macOS App

- 菜单栏入口、录音状态和显式提示。
- 系统音频与麦克风双路采集。
- 录制历史、时间线、搜索、详情和设置管理页。
- ASR、摘要、同步和权限状态展示。
- 音频文件导入。

### LifeSub CoreRuntime

- 管理录制任务和处理队列。
- 统一调度本地或云端 ASR Provider。
- 统一调度摘要 Provider。
- 生成记忆、证据片段与索引。
- 执行敏感级别、调用方权限与审计策略。
- 管理 GitHub 导出、加密、拉取、合并和冲突。
- 对 Tauri 提供完整枚举的 Core Application Contract 管理面，并复用 Agent Tool Contract 的 status/search/resolve/open 读取面；对 Agent 提供独立的 8-method Agent Tool Contract。两者共享 envelope、errors、DTO primitives 与 `OperationSummary`，不存在隐藏 Tauri Command。
- 独占 SQLite 写入、录音设备状态、模型安装与 ASR Worker；客户端不得绕过 Core。

### 进程演进

- C 阶段：primary Tauri 进程托管 CoreRuntime、普通 Agent socket 和受控 UI socket；secondary Tauri 经 Core Application Contract 连接 primary，绝不打开数据库。关闭窗口可驻留菜单栏，但退出 primary 会停止录音与处理。
- A 阶段：launchd 管理 `lifesubd`，Tauri 仍只通过同一 Core Application Contract 访问 Core；进程宿主变化不改变两个 V1 契约。
- ChatGPT Gateway 是单独的认证适配器，不把本机 daemon 端口暴露到网络。
- 普通 UDS 连接固定获得最小 `local_agent` 权限；请求不得自报 caller/capability。Tauri UI 权限只由 in-process host，或受控 UI endpoint 对 `LOCAL_PEERTOKEN` audit token 和 macOS code-signature designated requirement/Team ID/bundle ID 的校验产生；同 UID、unsigned/debug client 或验证失败都不能提升权限。
- Evidence 确认使用非公共 Host Event + Host Control Protocol V1：Core 将不含 token/路径的 pending intent event 推送给 authorized Tauri host，host 以自身 trusted identity 调用 CoreRuntime 串行 claim/complete/mark-uncertain ledger。requesting Agent/Gateway 与 claiming Tauri 分别审计；内部协议不计入两个公共契约，Agent/Gateway 不可访问，客户端不得直写 Catalog。
- full Core ownership lock 必须早于 writable Catalog open/migration、socket bind、reconciliation、模型/导入 mutation 和 worker；本地 IPC 不启用 TCP。

### Provider 层

ASR 与摘要使用独立接口。每次处理任务记录：Provider、模型、是否外发数据、开始和结束时间、输入来源、失败原因与重试结果。

默认策略：

- ASR：本地优先；当前版本仅实现本地 Provider 合同与模型基础设施，云端通路尚未交付。
- 摘要：可配置本地或云端模型。
- Provider 故障不应损坏原始音频或已有转写。

### 数据层

本地数据库保存结构化元数据、转写、记忆、证据关系、任务状态、权限和审计日志。原始音频使用本地文件存储。搜索索引是可重建的派生数据，不作为唯一事实来源。

## 数据流

```text
开始录制
  -> 创建 session
  -> 分别写入系统音频与麦克风轨道
  -> 停止并封存音频
  -> ASR 生成带时间戳转写
  -> 摘要模型提取主题、决定、行动项和记忆
  -> 用户或规则设置敏感级别
  -> 建立全文与语义索引
  -> Agent 检索摘要或获准原文
  -> 可选导出并同步到 GitHub 私有记忆库
```

## 可靠性原则

- 原始录音先持久化，再进入异步处理。
- 每个处理阶段可单独重试，并保持幂等。
- Provider 失败时保留任务和中间产物。
- 搜索结果必须携带来源 ID、时间范围和敏感级别。
- 删除操作需要覆盖本地文件、数据库、索引和同步记录，并明确提示 Git 历史限制。

## 待定技术选型

- 独立 `lifesubd` 是否继续复用当前 Tauri/Rust Core crate 的打包结构。
- launchd 安装/升级、应用签名分发与 Gateway 外部认证的具体实现（socket envelope、local caller trust 和两个 V1 contract 已冻结）。
- ScreenCaptureKit + AVAudioEngine Swift helper、权限预检、有界重连、独立 `.partial` 封存、sidecar 签名和认证已实现并复审；待定部分是 Task 7 Catalog v6/atomic sealing 和 Task 8 production coordinator 的 Core 接线。
- native ASR worker 如何将 fd-anchored Chunk、VAD 与 Provider 执行器接入现有 coordinator，同时持续续租和响应取消。
- 本地搜索使用 SQLite FTS、向量扩展或组合方案。
- 音频格式、分段策略与长期压缩方案。
