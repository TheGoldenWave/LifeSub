# Agent 插件与工具

## 目标

LifeSub 的首个 Agent 能力是“个人记忆检索”。实时上下文注入和录音控制不进入首版。所有插件必须通过 LifeSub Core 访问数据，不能各自直接解析本地数据库或 GitHub 记忆库。

## 共享能力层

建议提供一个共享客户端 SDK，将宿主插件调用转换成稳定的 Core API。该层统一处理：

- 连接发现与健康检查。
- 请求与响应数据结构。
- 调用方身份和能力声明。
- 敏感内容授权。
- 错误码和审计上下文。

## 工具集合（草案）

### `search_memories`

按自然语言、时间范围、来源、人物、主题和敏感级别搜索记忆。返回摘要、相关度、时间、来源与可用证据范围。

### `get_memory`

读取单条记忆的结构化详情。普通内容可以包含原文；敏感内容根据权限返回受限信息或授权要求。

### `get_transcript_excerpt`

按记忆 ID 或时间范围获取带时间戳的原文证据，用于验证摘要与回答。是否允许读取由隐私策略决定。

具体命名与参数将在正式规格阶段确认。

## Codex 插件

Codex 插件是可安装 bundle，而不只是一个独立 MCP 配置。建议结构：

```text
plugins/codex/
├── .codex-plugin/
│   └── plugin.json
├── .mcp.json
├── skills/
│   └── recall-life/
│       └── SKILL.md
└── assets/
```

- `.mcp.json` 启动或连接 LifeSub MCP Server。
- `skills/` 规定何时搜索记忆、怎样引用证据、何时申请敏感内容。
- `plugin.json` 提供身份、版本、能力和展示元数据。
- 首版不打包 Hooks，减少安装后的自动执行面和信任负担。
- 首版通过本地或仓库插件市场分发，连接本机 LifeSub Core。

公共 ChatGPT/Codex 插件需要公网 MCP Server、认证与新的数据边界，不属于首版。

参考：[OpenAI - Package your plugin](https://developers.openai.com/plugins/build/plugins)

## DeepSeek Harness 插件

DeepSeek Harness 使用 Cordis 原生插件机制：

- TypeScript 模块导出 `apply(ctx)`。
- 通过 `inject = ['tools']` 等待工具服务。
- 使用 `ctx.tools.register(defineTool(...))` 注册 LifeSub 工具。
- 插件卸载时由 Cordis 清理注册项与副作用。
- 以声明 `dsh.bundle` 的 npm 组合包分发，并通过 `dsh plugin add` 安装。

Harness 插件只负责把原生 Tool 调用转换成共享 SDK 请求，不重复实现记忆、权限或检索逻辑。

参考：[DeepSeek Harness - 第一个插件](https://deepseek-harness.github.io/deepseek-harness/develop/basic/) 与 [开发一个 Tool](https://deepseek-harness.github.io/deepseek-harness/develop/basic/tool)

## Malow 插件

Malow 是 LifeSub 的重点生态宿主，也是**项目型记录结果的主要处理与人工审核层**。插件使用 Malow 的原生 Project / Matter / Organizer / Review 体验，同时只通过 LifeSub Core 的稳定接口访问证据。

推荐流程：

1. 用户在 Matter 中关联一个 LifeSub Session，或按时间、人物、关键词搜索相关记录。
2. Malow 保存 `lifesub://` Evidence Ref，不复制原始音频和完整转写数据库。
3. Organizer 将 LifeSub 的 ASR 结果、当前 Matter 对话、Agent Run 结果和已授权的 GoldenWave Context 组合为有界输入。
4. Malow 生成阶段摘要、主题、决定、行动项及 Knowledge Patch Draft，并保留到 Transcript Segment 的来源映射。
5. 用户执行接受、修改、拒绝或拆分；只有人工确认后的 Patch 才能通过 GoldenWave Adapter 写入 Inbox。
6. GoldenWave 独立完成 route、score、冲突检查、敏感度、新鲜度、render 与 inject；Malow Review 成功不等于正式知识已经生效。

LifeSub 拥有 Evidence Contract 与音频访问授权；GoldenWave 拥有 Knowledge Patch Contract；Malow 分别作为 Evidence consumer 与 Knowledge Patch producer。三个项目不得共享数据库，也不得用内部目录或实现类型代替版本化 Contract。

非项目型的个人记录不强制经过 Malow。LifeSub 可以在独立人工确认后直接产生符合 GoldenWave Contract 的候选，但仍只能进入 Inbox，不能直接修改正式知识层。

## 一致性要求

- 三个宿主看到相同的记忆 ID、时间和来源。
- 权限判定只在 Core 中执行。
- 每次原文访问都记录宿主、工具、请求目的和结果。
- 插件不得静默提升权限或绕过敏感级别。
- Agent 输出应能引用 LifeSub 记忆，而不是把模型生成内容伪装成原始记录。

