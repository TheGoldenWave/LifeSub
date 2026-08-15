# 系统架构草案

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
  |-- Versioned Local Tool API
        |-- Tauri Adapter
        |-- Unix Socket Adapter -> DeepSeek Harness
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
- 对插件提供稳定的本地 API。
- 独占 SQLite 写入、录音设备状态、模型安装与 ASR Worker；客户端不得绕过 Core。

### 进程演进

- C 阶段：Tauri 进程托管 CoreRuntime 和 Unix socket；关闭窗口可驻留菜单栏，但退出进程会停止录音与处理。
- A 阶段：launchd 管理 `lifesubd`，Tauri 只通过版本化客户端访问 Core；进程宿主变化不改变工具契约。
- ChatGPT Gateway 是单独的认证适配器，不把本机 daemon 端口暴露到网络。

### Provider 层

ASR 与摘要使用独立接口。每次处理任务记录：Provider、模型、是否外发数据、开始和结束时间、输入来源、失败原因与重试结果。

默认策略：

- ASR：本地优先，允许用户显式选择云端。
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

- macOS App 使用 SwiftUI/AppKit，还是采用跨平台桌面框架。
- Unix socket envelope、launchd 安装与 Gateway 认证的具体实现。
- 本地 ASR 的首选模型与硬件要求。
- 本地搜索使用 SQLite FTS、向量扩展或组合方案。
- 音频格式、分段策略与长期压缩方案。
