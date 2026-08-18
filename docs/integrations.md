# Evidence API 与集成

## 目标

LifeSub 的首个外部能力是**证据检索与引用解析**，不是个人记忆检索。实时上下文注入、录音控制、项目理解和 Knowledge Candidate 生成不进入 V0.1。所有消费者必须通过 LifeSub Core 的 Evidence Contract 访问数据，不能直接解析本地数据库或对象目录。

## 共享能力层

共享客户端 SDK 将宿主调用转换成稳定 Core API，并统一处理：

- 连接发现与健康检查。
- Contract 版本协商。
- 请求与响应数据结构。
- 调用方身份和能力声明。
- 证据授权、撤回和过期。
- 错误码、幂等和审计上下文。

## 工具集合（草案）

### `list_records`

按时间范围、来源、设备和处理状态列出记录。返回 Record Ref、时间范围、来源、当前 Transcript Revision 和 Evidence 状态。

### `search_transcripts`

按文本关键词、时间范围、来源和设备搜索 Transcript。V0.1 使用 FTS5，不提供长期记忆向量检索、主题图谱或跨记录事实聚合。

### `get_transcript_segment`

读取获授权 Transcript Segment，返回 revision、文本、时间戳、来源声道、可用音频范围和 Evidence Ref。

### `resolve_evidence`

解析 `lifesub://` URI，校验 Contract 版本、内容 hash、授权范围和 `available | restricted | revoked | deleted | corrupted` 状态。

### `request_audio_excerpt`

请求获授权的音频时间范围或本地播放句柄。原始音频访问必须单独审计。

### `get_evidence_status`

查询 Evidence 当前状态、revision、撤回或删除信息，帮助下游识别缓存来源是否仍可验证。

具体参数由首个正式 Evidence Contract 确认。LifeSub 不提供 `search_memories`、`get_memory` 或 `create_knowledge_candidate`。

## Codex 与 DeepSeek Harness

插件是宿主适配层，不是第二套领域实现：

- 连接本机 LifeSub Core 或 MCP Adapter。
- 注册 Evidence 工具并保持 Contract 语义。
- 说明何时搜索转写、怎样引用 Evidence、何时申请受限内容。
- 不复制 LifeSub 数据库、ASR、revision、授权或检索逻辑。
- 不把模型生成内容伪装成 Transcript 或原始 Evidence。

公共托管插件需要公网服务、认证和新的数据边界，不属于 V0.1。

## Malow 集成

Malow 是 LifeSub 的重点 Evidence consumer，也是项目型与非项目型记录进入 GoldenWave 前的主要处理和人工 Review 层。

推荐流程：

1. 用户在 Matter 或独立 Review 入口中按时间、来源和文本关键词搜索 LifeSub Record。
2. Malow 通过 Contract 解析所需 Segment 和音频范围。
3. Malow 只保存 `lifesub://` Evidence Ref、内容 hash、授权范围和必要的可撤销快照，不复制原始音频和完整 Transcript 数据库。
4. Organizer 将 LifeSub Evidence、Matter 对话、Agent Run、Project Artifact 和已授权 GoldenWave Context 组合为有界 Context Plan。
5. Malow 生成阶段摘要、主题、决定、行动项和 Knowledge Patch Draft，并保留到 LifeSub Segment 的 source refs。
6. 用户接受、修改、拒绝或拆分候选。
7. 人工确认后的 Patch 通过 GoldenWave Adapter 写入 Inbox。
8. GoldenWave 独立完成验证、冲突、新鲜度、敏感度、render 和 inject。

Malow Review 的 `accepted` 只表示“用户确认提交候选”，不表示 GoldenWave 正式知识已经生效。LifeSub 不直接产生 Knowledge Patch，也不直接写 GoldenWave Inbox 或正式层。

## Contract 所有权

- LifeSub 拥有 Evidence Contract、证据状态和音频访问授权。
- GoldenWave 拥有 Knowledge Patch Contract、validator、兼容策略和正式治理语义。
- Malow 是 Evidence consumer 和 Knowledge Patch producer。
- 三个项目不共享数据库、内部 Rust 类型或锁步发布周期。

## 一致性要求

- 所有消费者看到相同的 Record、Segment、Revision、时间范围和来源。
- 权限判定只在 LifeSub Core 中执行。
- 每次 Transcript、音频、导出和状态解析记录调用方、工具、请求目的和结果。
- 插件不得静默提升权限或绕过 Evidence 状态。
- 未知 Contract 主版本、hash 不一致、权限撤回或来源删除必须 fail closed。
- Agent 输出引用稳定 Evidence Ref，不能把推断伪装成原始记录。
