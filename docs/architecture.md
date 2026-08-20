# 系统架构

## 已选方案

采用“本地 Evidence Core + 薄客户端 + 平台采集适配器”架构。LifeSub Core 是 Audio、Transcript 和 Evidence 的唯一业务与数据权威；桌面界面、平台 Capture Adapter 和下游插件只负责交互、平台能力或 Contract 适配。

```text
Tauri Desktop App
├── React / TypeScript UI
│   ├── 菜单栏与录音状态
│   ├── 时间线与记录详情
│   ├── 转写 revision 与导出
│   └── 存储、隐私和 Provider 设置
│
├── macOS Capture Adapter
│   ├── ScreenCaptureKit
│   └── AVAudioEngine
│
└── lifesubd / Rust Core
    ├── Capture Session Manager
    ├── Physical Chunk Writer
    ├── ASR Job Pipeline
    ├── Multi-Device Evidence Reconciler
    ├── Audio Quality Pipeline
    ├── Transcript Segmenter
    ├── Vocabulary / Hotword Resolver
    ├── Constrained Correction Pipeline
    ├── Markdown Renderer
    ├── Evidence Catalog
    ├── Retention / Archive
    ├── Access Grant / Audit
    └── Evidence API / MCP Adapter
```

## 建议的代码边界

```text
LifeSub/
├── apps/
│   └── desktop/                 Tauri + React 管理界面
├── services/
│   └── core/                    Rust Evidence Core
├── adapters/
│   └── macos-capture/           Swift 平台采集适配器
├── packages/
│   ├── evidence-contract/       语言无关 Schema 与测试向量
│   ├── client/                  Core 客户端 SDK
│   └── asr-providers/           本地与云端 ASR 适配器
├── plugins/
│   ├── codex/
│   ├── deepseek-harness/
│   └── malow/
└── docs/
```

此目录是设计意图，不是最终实现脚手架。

## 核心组件

### Desktop App

- 菜单栏入口、录音状态和显式提示。
- 开始、暂停、恢复、停止和隐私暂停。
- 录制历史、时间线、关键词搜索、详情和设置。
- ASR、revision、存储、导出、授权和处理状态展示。
- 音频文件导入。

### Capture Adapter

- 调用平台原生 API 采集系统音频和麦克风。
- 报告设备、采样率、声道、权限、静音和 discontinuity。
- 不包含 ASR、检索、Project 或知识治理逻辑。
- 原始来源默认分开保存，不在采集阶段不可逆混音。

### LifeSub Core

- 管理 Capture Session、处理队列和启动恢复。
- 滚动写入、封存和校验不可变 Physical Audio Chunk。
- 调度本地或云端 ASR Provider，并记录 Provider Receipt。
- 保存原始 ASR、确定性校对、受约束 LLM 校对和人工修订的 revision 链。
- 生成 Logical Transcript Segment、Markdown 投影和 FTS5 派生索引。
- 管理 Vocabulary、可选 Speaker Evidence、保留、删除、Tombstone 与诊断。
- 执行证据访问授权和读取、导出、删除、撤回审计。
- 通过 Evidence Contract 提供稳定 API。

### Multi-Device Evidence Reconciler（V0.3 规划）

- 将 Mac、手机、手表等设备的录音登记为同一事件的候选来源。
- 使用设备时间、录音时间、音频指纹、采样率、连续性和质量信息进行确定性对齐。
- 识别重复、重叠、缺口和设备时钟偏移；保存校准参数与不确定范围。
- 对不同设备的 ASR 结果做时间对齐和冲突标注，不覆盖任何来源 Revision。
- 生成新的合并 Audio/Transcript Revision 和可再生 Markdown 投影。
- 为每个合并片段保留设备、Chunk、原始 Revision、时间范围、规则版本和冲突来源。

该组件只治理音频、转写和 Evidence。它不负责会议纪要、主题、决定、行动项、Project/Matter 状态或 GoldenWave 正式知识。

LifeSub Core 不生成 Memory、Decision、ActionItem、Knowledge Candidate、Profile、Persona 或 Project/Matter 状态。

## Provider 层

ASR 和受约束校对使用独立 Provider 接口。每次处理必须记录：Provider、模型、参数、数据去向、输入 hash、开始和结束时间、失败原因和重试结果。

默认策略：

- ASR：本地优先；云端 ASR 必须独立显式授权。
- 校对：优先确定性规则；LLM 只能生成结构化局部 edit，不能改写为文章、会议纪要或长期记忆。
- Provider 失败不应损坏原始音频、原始 ASR 或已有 revision。
- 声纹云服务与普通云端 ASR 使用不同授权，不合并许可。

## 分片模型

LifeSub 使用两种不同对象：

1. `PhysicalAudioChunk`：按有界时间或大小封存，用于崩溃恢复、增量处理和归档；不可变。
2. `LogicalTranscriptSegment`：按时间戳、静音、长度和录制状态形成可读段落；属于特定 revision，可重新计算。

LifeSub 不根据主题、决定、行动项、人物关系或 Project 做语义切分；此类解释属于 Malow。

## 数据层

- SQLite：Session、Chunk metadata、Transcript revision、Segment、任务、Vocabulary、Access Grant 和审计。
- 对象目录：原始及派生音频分片和必要中间产物。
- Markdown：可再生的人类可读投影与主动导出格式。
- FTS5：按时间、来源、设备和文本关键词的基础检索。
- Keychain：密钥和 Provider credential reference。

原始音频分片和不可变 revision 是证据链核心；Markdown 与索引均为可重建派生数据，不是独立事实源。

## 数据流

```text
开始长时录制
  -> 创建 CaptureSession
  -> 系统音频与麦克风分别滚动写入临时分片
  -> 原子封存并校验 PhysicalAudioChunk
  -> Chunk 独立进入 ASR 队列
  -> ASR 生成带时间戳的原始 TranscriptRevision
  -> 规则 / 受约束 LLM / 人工修订创建新 Revision
  -> 形成 LogicalTranscriptSegment
  -> 渲染可再生 Markdown 与可选 navigation_summary
-> 更新 FTS5 和 Evidence Catalog
  -> 多设备来源归并与时间校准（V0.3）
  -> 生成新的合并 Audio/Transcript Revision（V0.3）
  -> Malow 通过 Evidence Ref 读取获授权片段
  -> Malow / Human Review 形成候选
  -> GoldenWave 独立完成正式治理
```

## Evidence Contract

LifeSub 拥有并发布版本化 Evidence Contract。稳定引用示例：

```text
lifesub://record/{record_id}
lifesub://segment/{segment_id}?revision={revision}
lifesub://audio/{chunk_id}#t=120,165
```

Contract 至少包括：稳定 ID、版本、内容 hash、时间范围、来源、revision、Provider、授权、状态、撤回、删除、错误和兼容语义。Malow 等消费者只保存 Evidence Ref、hash、授权范围和必要快照，不直接读取 LifeSub 数据库。

## 可靠性原则

- 原始录音先持久化，再进入异步处理。
- 当前分片失败不能损坏已经封存的分片。
- 录音停止与音频封装是两个不同状态。
- 每个处理阶段可单片重试并保持幂等。
- 原始 ASR 永久可访问，校对只创建新 revision。
- 存储不足、设备切换、睡眠、权限撤回和静音必须产生明确状态。
- 删除必须覆盖目标资源、更新 Evidence 状态并产生 Tombstone。
- 下游收到撤回或删除状态后，不能继续宣称证据可验证。

## 同步边界

GitHub 不作为全天音频和转写的主存储或同步通道。未来同步使用加密对象、manifest、内容 hash 和增量游标；Markdown 仅作为主动导出或可再生投影。GoldenWave 继续使用 Git 管理正式知识与治理历史。

## 待定技术选型

- ~~首选本地 ASR 模型及最低硬件要求。~~ → V0.2 已确定：SenseVoiceSmall INT8 + Whisper Tiny/Base/Small，通过 sherpa-onnx 1.13.5 静态链接。
- 音频编码、Chunk 时长和长期归档策略。
- ~~V0.1 是否包含匿名 Speaker Diarization，还是延后到 V0.3。~~ → 延后至 V0.3+。
- 多设备来源归并、时钟校准和 ASR 冲突治理（V0.3）。
- Evidence Contract 的首个正式 Schema、错误码和兼容范围。
- 加密对象同步的后端与密钥恢复方案。
