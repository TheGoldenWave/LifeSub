# LifeSub Evidence Platform 产品与技术架构设计

## 1. 决策摘要

LifeSub 的产品边界正式收窄为个人音频与 ASR 结果的长期证据系统。

> LifeSub 保存“发生过什么”，Malow 判断“这对当前工作意味着什么”，GoldenWave 治理“什么值得长期相信和复用”。

LifeSub 不负责记忆压缩、跨记录合并、长期事实抽取、Persona、知识治理或正式上下文注入。如果未来建设个人记忆系统，该能力属于 GoldenWave 体系，不属于 LifeSub。

本设计覆盖此前 LifeSub 文档中的以下表述：

- “LifeSub Core 是个人记忆的唯一真相来源”改为“LifeSub 是音频、转写和 Evidence 的唯一真相来源”。
- `Memory`、`Decision`、`ActionItem` 和 `Knowledge Candidate` 不再是 LifeSub 领域对象。
- “GitHub 记忆库”不再是 LifeSub 的主同步方案。
- Agent 接口从记忆检索改为证据检索与引用解析。

## 2. 三项目定位

| 系统 | 核心职责 | 权威数据 | 明确不负责 |
|---|---|---|---|
| LifeSub | 全天或长时录音、音频分片、ASR、文本分片、Markdown 投影、基础检索、证据授权 | Capture Session、Audio Chunk、Transcript Revision、Transcript Segment、Evidence Ref | 记忆压缩、项目理解、长期知识治理 |
| Malow | 在 Project / Matter 中引用证据，结合工作上下文完成 Organizer 与人工 Review | Project、Matter、Conversation、Agent Run、Organizer Result、Knowledge Patch Draft | 保存完整音频、成为 ASR 或长期知识权威 |
| GoldenWave | 接收人工确认后的候选，执行验证、冲突、新鲜度、敏感度、确认、注入和审计 | Profile、Knowledge、Persona、正式 Project Context、治理历史 | 主动扫描全天录音、保存完整会议流水 |

三个项目保持独立仓库、独立数据库和独立发布节奏。它们通过版本化 Contract 协作，不共享 SQLite、不共享内部 Rust 类型，也不要求锁步发布。

## 3. 标准数据流

```mermaid
flowchart LR
    Capture["LifeSub Capture<br/>麦克风 / 系统音频"] --> Chunk["不可变音频分片"]
    Chunk --> ASR["ASR + 时间戳"]
    ASR --> Transcript["Transcript Segment"]
    Transcript --> Markdown["Markdown Record<br/>标题 + 导航摘要 + 正文"]
    Markdown --> Catalog["LifeSub Evidence Catalog"]

    Catalog -->|"Evidence Ref"| Malow["Malow Project / Matter"]
    Malow --> Organizer["Organizer + Human Review"]
    Organizer -->|"user-confirmed Knowledge Patch"| Inbox["GoldenWave Inbox"]
    Inbox --> Governance["GoldenWave Governance"]
    Governance --> Context["Governed Context Pack"]
```

LifeSub 不直接把 ASR 结果写入 GoldenWave 正式层。项目型和非项目型记录均先在 Malow 或等价的人工 Review 入口中形成候选，再进入 GoldenWave Inbox。

## 4. LifeSub 产品职责

### 4.1 采集

- 支持长时间或全天录制，并提供始终可见的录音状态。
- 支持麦克风与系统音频，保留各自来源信息。
- 支持开始、暂停、恢复、停止和隐私暂停。
- 原始数据先持久化，再进入 ASR 和投影流程。
- 录音不能依赖单个超长文件，必须滚动写入有界分片。

### 4.2 ASR 与文本管理

- ASR 默认本地处理，允许用户显式选择云端 Provider。
- 每次处理记录 Provider、模型、参数、数据去向、输入 hash、时间和失败原因。
- ASR 结果必须与音频时间范围稳定对齐。
- 用户编辑转写时创建新 revision，不覆盖原始 ASR 结果。
- 支持对失败分片重新转写，而不重新处理完整录音。

LifeSub 的核心质量目标不是生成更漂亮的文字，而是尽可能忠实地还原“说了什么、谁说的、什么时候说的”。因此，ASR 前处理、术语纠错、说话人处理和受约束的 LLM 校对均属于 LifeSub 的证据质量职责。

### 4.3 音频质量处理

- 麦克风与系统音频默认保留为独立来源，不在采集阶段不可逆混音。
- 原始音频分片保持不可变；降噪、回声消除、增益、重采样和声道处理生成派生版本。
- 检测静音、削波、输入设备失联、系统音频无声、采样率异常和长时间低信号。
- 对每个分片保存音频质量指标和处理链，允许比较原始音频与 ASR 工作副本。
- 首版优先保证录音可恢复、来源可区分和音频未损坏，再优化实时降噪或复杂增强。

### 4.4 受约束的 LLM 校对

LLM 可以用于修正 ASR 中的同音字、专有名词、标点、断句和明显识别错误，但不能把转写改写成文章、会议纪要或长期记忆。

校对必须遵循：

1. 原始 ASR revision 永久保留，LLM 只能创建新的 `CorrectionRevision`。
2. LLM 输出结构化局部 edit，而不是直接返回一份无法对比的完整重写文本。
3. 每项修改保存原文、修订文本、原因、置信度、模型和词库依据。
4. 禁止新增原始音频与 ASR 中不存在的事实、人物、决定或因果关系。
5. 时间戳、来源声道和 Evidence Ref 不得由 LLM 自行修改。
6. 低置信度修订保持建议状态；用户可以接受、拒绝或恢复原始结果。
7. 模型失败、输出不合法或改动幅度超限时，回退到上一可验证 revision。

自动应用只允许以下操作：

- 插入或修正标点、大小写和句子边界。
- 使用已确认 Vocabulary、第二 ASR 一致结果或确定性规则支持，在已有字符跨度内执行锚定 `replace_span`。

所有实词替换都必须存在已确认 Vocabulary、第二 ASR 一致结果或确定性规则支持，否则只能生成待人工确认的建议。人物、组织、数字、日期、金额、否定词和模态词还必须使用更高风险等级展示并单独确认。自由插入或删除实词、无法锚定到原 Segment 的修改、跨 Segment 语义改写一律拒绝。

“事实新增”不能只依赖另一个 LLM 自动证明。发布 Gate 使用结构化 edit policy 阻止高风险自动应用，并在固定语料上人工标注 `unsupported_lexical_edit_rate`；任何自动生效但缺少音频、ASR 或 Vocabulary 支持的实词变更都阻塞发布。

云端 Correction Provider 与云端 ASR 使用独立授权：

- 默认只发送完成当前 edit 所需的最小文本跨度，不发送完整 Session。
- 不发送音频；如果未来需要音频复核，必须重新取得单独授权。
- Provider Receipt 记录地域、留存、训练使用声明、模型、请求范围和撤回能力。
- 用户选择云端 ASR 不代表同时同意云端 LLM 校对或云端声纹服务。

默认 Markdown 可以显示当前已选择的校对 revision，但必须能切换查看原始 ASR 和修订 diff。

### 4.5 热词与词库体系

LifeSub 维护面向识别质量的 Vocabulary，而不是面向长期知识的 Knowledge：

- 支持人名、组织名、产品名、缩写、专业术语和多语言写法。
- 支持全局词库、语言词库、用户选择的词包和单次 Session 临时词表。
- Malow 可以通过受限接口为某次记录提供 Project / Matter 术语提示，但这些提示不反向创建 LifeSub Project 状态。
- 词条记录标准写法、读音或别名、适用语言、来源、优先级、有效期和版本。
- Provider 支持 prompt/hotword 时在 ASR 前注入；不支持时进入确定性后处理或 LLM 校对上下文。
- 用户对转写的重复修正可以生成词条建议，但必须经用户确认后进入长期词库。
- 词库修改只影响新的处理任务或显式重跑，不静默改写历史 Evidence。

### 4.6 说话人分离与声纹识别

说话人能力分为两层：

1. **Speaker Diarization**：判断不同时间段由不同匿名说话人发言，输出 `speaker_1`、`speaker_2` 等临时标签。
2. **Speaker Identification**：将匿名说话人与经用户授权保存的 `SpeakerProfile` 匹配。

声纹属于高敏感生物特征数据，必须满足：

- 默认关闭实名声纹识别，用户明确选择后才创建 Speaker Profile。
- 创建第三方 Speaker Profile 前必须展示生物特征用途、保留期限和删除影响，并由用户确认已获得适用场景要求的知情同意或合法依据。
- 优先保存加密的声纹 embedding、模型版本和质量信息，不保存额外声纹原始音频副本。
- 匹配结果包含置信度；低于阈值时保持匿名，不猜测身份。
- 用户可以锁定、纠正、合并、撤回或删除 Speaker Profile。
- 删除 Profile 后，所有默认投影、检索和 Contract 响应解除实名关联并恢复匿名标签；受限审计只保留不可反查的 profile tombstone，不保留可显示姓名或可继续匹配的 embedding。
- 云端声纹服务必须单独授权，不与普通云端 ASR 授权合并。
- Speaker Profile 只表示声音身份匹配，不扩展为人物关系、Persona 或长期事实档案。

### 4.7 分片

LifeSub 使用两层分片：

1. **Physical Audio Chunk**：按有界时长或大小封存，用于崩溃恢复、重试、归档与增量同步。
2. **Logical Transcript Segment**：基于时间戳、静音、长度和录制状态形成可读段落，可重新计算。

首版不根据主题、决定或人物关系做语义切分。此类解释属于 Malow。

### 4.8 Markdown 投影

LifeSub 将转写渲染为可读、可导出、可重新生成的 Markdown。Markdown 不是独立事实源，其内容必须能从 Evidence Catalog 重建。

允许生成一个可选的 `navigation_summary`，但它必须：

- 仅帮助用户浏览当前记录。
- 与转写正文和来源字段明确分离。
- 标记生成 Provider、模型和时间。
- 可被重新生成或删除。
- 不被描述为正式记忆、决定或知识。

示例：

```markdown
---
record_id: rec_01...
started_at: 2026-08-15T09:00:00+08:00
ended_at: 2026-08-15T10:12:00+08:00
audio_chunks:
  - chk_01...
transcript_revision: tr_01...
asr_provider: local
asr_model: pending-benchmark
content_hash: sha256:...
---

# 2026-08-15 上午记录

> 导航摘要：本段记录围绕 LifeSub 首版边界展开。该摘要仅用于浏览。

## 09:00:12

[麦克风] 我们今天先讨论首个版本的范围。

## 09:00:18

[系统音频] 首版重点应该是录音和证据链。
```

### 4.9 基础检索与证据读取

LifeSub 提供按时间、来源、设备和文本关键词的基础检索。首版使用 SQLite FTS5，不建设面向长期记忆的向量检索、主题图谱或跨记录聚合。

检索结果返回 Evidence Ref、时间范围、转写片段和可用音频范围，不返回“已确认的长期事实”。

### 4.10 质量评测与 Provider Gate

LifeSub 必须建立固定、可重放的质量语料，而不是根据单次体验选择模型。语料至少覆盖：

- 普通话、中文方言、英文和中英混合。
- 单人近讲、多人会议、远场、重叠说话和背景噪声。
- 人名、品牌、项目名、缩写、数字、日期和专业术语。
- 麦克风、系统音频、蓝牙设备和导入文件。
- 短片段、长时间连续录音、静音和设备中断。

核心指标包括：

| 能力 | 指标 |
|---|---|
| ASR | CER / WER、漏字率、数字与专有名词准确率 |
| 时间对齐 | Segment 起止偏差、音频跳转命中率 |
| 热词词库 | 目标术语召回率、非目标误替换率 |
| LLM 校对 | 正确修订率、错误修订率、事实新增率、平均改动幅度 |
| 说话人分离 | DER / JER、重叠语音表现 |
| 声纹识别 | 已知说话人准确率、未知说话人误认率、拒识率 |
| 长时可靠性 | 音频缺口、损坏分片、恢复成功率、任务积压时间 |

任何默认 Provider、模型升级、校对 Prompt、词库规则或声纹模型变更，都必须在同一语料上回放并保存版本化结果。质量退化时保持当前稳定版本，不以“模型更新”本身作为升级理由。

V0.1 发布前冻结首批 benchmark manifest。初始 Gate 为：

- 至少 12 小时人工校对语料，覆盖不少于 60 个独立录制样本；保留从未进入 Prompt、Vocabulary 建议或模型调优的隔离 holdout。
- 加权中文 CER 不高于 15%；近讲普通话不高于 10%；中英混合与噪声场景不高于 25%。
- 已确认热词召回率不低于 90%，非目标误替换率不高于 1%。
- Segment 时间对齐误差中位数不高于 500ms，P95 不高于 1.5s。
- 自动应用校对的正确率不低于 95%，`unsupported_lexical_edit_rate` 必须为 0。
- 同一 benchmark 升级后的加权 CER、术语准确率或校对正确率不得相对退化超过 3%；超过时必须保留旧版本或提交单独决策记录。

最终语料清单、硬件基线和阈值可在 Phase 0 实测后通过 ADR 调整，但必须在选定默认 Provider 和开始 V0.1 实现前冻结，不能在发布失败后临时降低。

## 5. 明确排除的 LifeSub 能力

- 跨天、跨会议的记忆压缩。
- 决定、行动项、项目状态或人物关系的权威抽取。
- Profile、Persona、Knowledge、Experience 或正式 Project Context。
- Knowledge Candidate 的治理、冲突检测、评分和注入。
- 将转写大规模压缩、润色或改写为新的叙事文本。
- 跨 Session 推断人物关系、偏好、承诺、项目状态或长期事实。
- GoldenWave Context Pack 的构建。
- Malow Project、Matter、Organizer 或 Agent Run 状态。
- 通用 Agent 聊天、全局语音听写、自动粘贴、词典和文本片段。
- GitHub 作为全天音频和转写的主数据库。

## 6. 领域模型

| 对象 | 说明 | 可变性 |
|---|---|---|
| `CaptureSession` | 一次连续录制周期 | 状态机可变，完成后冻结 |
| `AudioChunk` | 有界物理音频分片 | 不可变 |
| `TranscriptRevision` | 一次 ASR 或人工修订结果 | 不可变，使用 revision 链 |
| `CorrectionRevision` | 确定性规则、LLM 或人工产生的可审计局部修订 | 不可变，使用 revision 链 |
| `TranscriptSegment` | 带开始和结束时间的文本片段 | 属于特定 revision |
| `VocabularyEntry` | 用于 ASR 和校对的人名、术语、别名与读音提示 | 版本化 |
| `SpeakerProfile` | 经授权保存的声纹 embedding 和显示身份 | 可撤回、可删除 |
| `SpeakerAssignment` | Segment 与匿名或实名说话人的匹配结果 | 属于特定 revision |
| `MarkdownRecord` | 面向用户和导出的可再生投影 | 可再生 |
| `EvidenceRef` | 跨系统稳定引用 | 版本化 |
| `ProviderReceipt` | Provider、模型、参数和外发信息 | 不可变 |
| `AccessGrant` | 调用方可读取的资源、操作和有效期 | 可撤回 |
| `AuditEvent` | 读取、导出、删除和撤回事件 | 追加写入 |
| `Tombstone` | 删除、撤回或来源不可用状态 | 追加写入 |

稳定 ID 推荐使用 UUIDv7 或 ULID。所有持久时间统一保存为 UTC，界面和 Markdown 投影按用户时区显示。

## 7. Evidence Contract

LifeSub 拥有并发布 Evidence Contract。稳定 URI 示例：

```text
lifesub://record/{record_id}
lifesub://segment/{segment_id}?revision={revision}
lifesub://audio/{chunk_id}#t=120,165
```

Contract 至少定义：

- `contract_version`、稳定 ID 和内容 hash。
- 记录、分片与时间范围。
- 音频来源、设备和 capture adapter。
- Transcript revision、ASR Provider 与模型。
- 音频质量指标、处理链和派生音频 hash。
- Correction revision、局部 edit、依据和模型 receipt。
- 匿名说话人标签、可选 Speaker Profile Ref 和匹配置信度。
- 调用方能力、授权范围、有效期和撤回语义。
- `available | restricted | revoked | deleted | corrupted` 等证据状态。
- 幂等键、分页、错误码和兼容策略。

Capture Adapter Contract 还必须定义统一时间语义：

- 每个音频帧使用单调时钟位置，Session 同时保存单调时钟与 UTC wall-clock anchor。
- 麦克风和系统音频记录各自采样位置、采样率与时钟来源。
- Rust Core 负责检测漂移；重采样和对齐只生成派生版本，不修改原始分轨。
- 睡眠、设备切换、采集重启和丢帧产生显式 `Discontinuity`，包含原因、持续时间和前后 frame position。
- 对齐后的双路音频每小时漂移目标不超过 50ms；超过时标记质量降级，不伪造连续时间线。

Malow 只保存 Evidence Ref、hash、授权范围和必要快照。GoldenWave Knowledge Patch 的 provenance 同时保留 Malow Project / Matter / Run Ref 与 LifeSub Evidence Ref。

## 8. Agent 与应用接口

LifeSub 不再提供 `search_memories`、`get_memory` 或 `create_knowledge_candidate`。

首版接口候选：

- `list_records`
- `search_transcripts`
- `get_transcript_segment`
- `resolve_evidence`
- `request_audio_excerpt`
- `get_evidence_status`

接口只返回证据与访问状态。Malow 负责解释、组织和候选生成；GoldenWave 负责正式治理。

## 9. 技术架构

```text
Tauri Desktop App
├── React / TypeScript UI
│   ├── 菜单栏与录音状态
│   ├── 时间线与记录详情
│   ├── 转写修订与导出
│   └── 存储、隐私和 Provider 设置
│
├── Platform Capture Adapter
│   ├── macOS: Swift + ScreenCaptureKit + AVAudioEngine
│   ├── Windows: WASAPI
│   └── Linux: PipeWire
│
└── lifesubd / Rust Core
    ├── Capture Session Manager
    ├── Physical Chunk Writer
    ├── ASR Job Pipeline
    ├── Audio Quality Pipeline
    ├── Transcript Segmenter
    ├── Vocabulary / Hotword Resolver
    ├── Constrained Correction Pipeline
    ├── Speaker Diarization / Identification
    ├── Markdown Renderer
    ├── Evidence Catalog
    ├── Retention / Archive
    ├── Access Grant / Audit
    └── Evidence API / MCP Adapter
```

### 9.1 选择 Tauri + Rust

- 与 Malow 的 Tauri + Rust 技术方向一致，降低共同维护工具链和 Contract 测试的成本。
- 相比 Electron 更适合长时间常驻、低资源占用和本地文件处理。
- Rust Core 可在 macOS、Windows 和 Linux 复用。
- 音频、权限和后台能力仍通过平台原生 Adapter 实现，不假设 Tauri 可以屏蔽系统差异。

LifeSub 与 Malow 可以共享通用 Contract 工具、ID、错误格式和测试方法，但不能共享领域数据库或业务状态机。

### 9.2 数据存储

- SQLite：Session、Chunk metadata、Transcript revision、Evidence、任务、授权和审计。
- 对象目录：音频分片和必要的中间产物。
- Markdown：可再生的人类可读投影和主动导出格式。
- FTS5：首版文本检索。
- 搜索索引和 Markdown 均为派生数据，不是唯一事实源。
- 密钥由 Keychain、Windows Credential Manager 或 Linux Secret Service 保存。

SwiftData、CoreData、UserDefaults 或前端状态不能成为跨平台主数据源。

## 10. 长时录音可靠性

- 每个物理分片独立封存、校验和进入 ASR 队列。
- 当前分片失败不能损坏已封存分片。
- 录音停止和音频封装是两个不同状态。
- Provider 失败保留音频与任务，可单片重试。
- 校对、分离和身份识别失败不得使原始 ASR 变为不可用。
- 存储空间不足、设备切换、睡眠、权限撤回和系统音频静音必须产生明确状态。
- 原始或归档音频的删除必须更新 Evidence 状态并产生 Tombstone。
- 下游系统收到撤回事件后标记来源不可用，不能继续宣称证据仍可验证。

V0.1 已包含以下可靠性能力：

- 使用 append-safe 临时格式写入当前分片，按不超过 5 秒的 durability interval 刷新可恢复数据。
- 分片完成采用 `fsync -> atomic rename -> metadata commit`；对象成功封存后才进入 ASR 队列。
- SQLite 使用 WAL；启动时对账数据库、对象目录、`.part` 文件和 processing job，恢复或隔离不完整状态。
- 捕获队列与 ASR 队列解耦。处理积压时继续优先持久化音频，并降低或暂停非关键派生任务。
- 开始录制前预留至少满足当前编码配置两小时的安全空间；达到保护阈值时明确警告并安全停止，不写坏当前分片。
- 记录实时磁盘增长率、队列积压和预计剩余录制时间。
- V0.1 通过 8 小时与 24 小时 soak，以及至少 20 次随机崩溃注入；已封存分片损失和损坏均为 0，未封存音频最大允许损失不超过 5 秒，启动对账恢复率为 100%。
- Apple Silicon M1 / 16GB 基线下，capture-only 平均 CPU 不高于 10%，常驻内存不高于 350MB，24 小时运行无持续内存增长；ASR 资源另行按模型记录且不得阻塞采集。

## 11. 音频编码优化

全天录制场景下，编码选择对存储成本有数量级的影响。LifeSub 的音频编码策略必须同时满足 ASR 精度、长期存储效率和磁盘保护三个目标。

### 11.1 编码器选择：Opus

Opus（RFC 6716）是当前语音/音频编码的最优选择，理由如下：

| 维度 | Opus 优势 | 对比 |
|---|---|---|
| 语音质量 | 12 kbps 即可达到窄带语音透明质量，24 kbps 达到全频带语音透明 | AAC-LC 在 32 kbps 以下语音质量显著下降 |
| 比特率范围 | 6–510 kbps，支持 CBR/VBR/CVBR | MP3 最低 32 kbps，低于此值时质量崩溃 |
| 延迟 | 算法延迟 2.5–60 ms 可调 | 远低于 HE-AAC 的典型 40–60 ms |
| 许可 | BSD 三条款，无专利负担 | AAC 涉及专利池 |
| 静音处理 | 原生 DTX（不连续传输），静音段比特率可降至 ~2 kbps | 多数编码器需要外部 VAD + 静音帧丢弃 |
| 抗丢包 | 内置 FEC（前向纠错），适合流式写入 | 多数编码器无内置抗丢包 |

### 11.2 编码配置与 ASR 兼容性分析

#### 11.2.1 模型原生输入要求

LifeSub 当前及候选的 ASR 模型均以 **16kHz 单声道** 为原生输入：

| 模型 | 输入要求 | 处理管线 | 来源 |
|---|---|---|---|
| **SenseVoice** | 16kHz 单声道 PCM | FBank 特征提取 → Transformer | sherpa-onnx FeatureExtractorConfig `sampling_rate=16000` |
| **Whisper** | 16kHz 单声道 PCM | 80 维 log-Mel 频谱 → Encoder-Decoder | OpenAI Whisper 特征提取器，输入自动重采样到 16kHz |
| **Qwen3-ASR** | 16kHz 单声道 PCM | 80 维 log-Mel 频谱 → Whisper 风格 Encoder → Qwen3 LLM Decoder | HuggingFace `Qwen3ASRFeatureExtractor` 默认 `sampling_rate=16000`，[技术报告 arxiv 2601.21337](https://arxiv.org/abs/2601.21337) |

关于 Qwen3-ASR：

- **架构**：采用 Whisper 风格音频 Encoder + Qwen3 LLM Decoder，是 End-to-End 模型而非纯 CTC/Attention。提供 0.6B 和 1.7B 两个参数量级。
- **精度**：在 52 语言 Fleurs 基准上，1.7B 版本的整体 WER 低于 Whisper Large v3 和 GPT-4o-Transcribe，中文表现尤其突出。
- **许可**：Apache 2.0，无商用限制。
- **运行时**：已有 Rust 绑定（[qwen3_asr_rs](https://github.com/second-state/qwen3_asr_rs)）和 ONNX 导出（[andrewleech/qwen3-asr-0.6b-onnx](https://huggingface.co/andrewleech/qwen3-asr-0.6b-onnx)），但尚未进入 sherpa-onnx 主分支。LifeSub 可将其列为 V0.4+ 的第三 Provider 候选。
- **模型体积**：0.6B ONNX 约 1.2GB，1.7B 约 3.4GB——远大于 SenseVoiceSmall（163MB）和 Whisper Tiny（116MB），不适合作为默认 Provider，但可作为高精度选项供用户选择。
- **与 LifeSub 编码策略的兼容性**：16kHz 输入要求与 SenseVoice、Whisper 完全一致，16kHz Opus 编码无需做任何调整即可兼容。

- 16kHz 不是"降级"，而是模型的**原生采样率**。奈奎斯特频率 8kHz，覆盖了人类语音的全部关键频段（基频 85–300Hz，辅音特征 4–8kHz）。
- 使用 48kHz 录制对 ASR 精度**无额外收益**，因为模型在下采样后只使用 0–8kHz 频段信息，高频部分被丢弃。
- 使用 8kHz 录制则**会丢失 4–8kHz 的辅音区分信息**（如 /s/ vs /f/、/θ/ vs /t/），影响清晰度。

#### 11.2.2 Opus 压缩对 ASR 精度的影响

有损压缩对 ASR 的影响已被学术研究验证。Amazon Science 在 Interspeech 2021 发表的论文 "[Multi-channel Opus compression for far-field automatic speech recognition with a fixed bitrate budget](https://www.amazon.science/publications/multi-channel-opus-compression-for-far-field-automatic-speech-recognition-with-a-fixed-bitrate-budget)" 测试了 Opus 多档比特率下的远场 ASR 表现。其核心结论：

| 比特率 | 相对 WER 退化（vs 无压缩 PCM） | 适用性 |
|---|---|---|
| **32 kbps 全频带** | **< 1% 相对退化** | 对 ASR 几乎无影响，可视为透明 |
| **16 kbps 全频带** | **< 3–5% 相对退化** | 轻微退化，近距离语音几乎不可感知 |
| 8 kbps | 退化显著 | 不推荐用于 ASR 证据 |

关键发现：
- Opus 在 16–32 kbps 区间对 ASR 的退化**远小于**同码率下的 MP3 或 AAC-LC，因为 Opus 的 SILK 层专为语音优化。
- 退化主要出现在远场、重叠语音和强噪声场景；近讲单人语音（LifeSub 的主要场景）在 16 kbps 时退化可忽略。
- VAD 分段 + 逐段转写进一步降低了压缩噪声对 ASR 的影响，因为静音段不参与转写。

#### 11.2.3 LifeSub 编码配置

基于三个 ASR 模型（SenseVoice、Whisper、Qwen3-ASR）均以 16kHz 单声道为原生输入，且 Opus 16kbps 对 ASR 精度影响可忽略，LifeSub 采用**单一编码配置**，不做多档切换：

| 配置 | 比特率 | 采样率 | 声道 | 一小时体积 | ASR 影响 |
|---|---|---|---|---|---|
| **Opus Voice** | 16 kbps VBR | 16 kHz | Mono | **~7 MB** | 相对 WER 退化 < 3–5%，近讲不可感知 |

- 统一配置消除了用户在不同场景下做编码决策的认知负担，也避免了"录制时选 HQ、事后发现不需要"的浪费。
- 研究数据和模型输入要求均表明 16kHz 16kbps 已是最优平衡点，更高的采样率或比特率对 ASR 精度无实质增益。
- 用户标记"重要"只影响**保留策略**（§12.2），不改变编码——因为编码质量对所有记录已经足够好。

对比此前预估的 32 kbps Opus（~14 MB/h），默认配置降至 16 kbps 可再节省 **50%** 存储：

| 场景 | 32 kbps Opus | 16 kbps Opus | 节省 |
|---|---|---|---|
| 8h 工作日双路 | ~230 MB/天 | ~115 MB/天 | 50% |
| 30 天（22 工作日） | ~5 GB | ~2.5 GB | 50% |
| 全年 24h 全天 | ~240 GB | ~120 GB | 50% |

### 11.3 DTX 与静音优化

LifeSub 必须启用 Opus DTX（不连续传输），结合 VAD（语音活动检测）实现：

- 静音段：编解码器自动降低至 ~2 kbps 级比特率，而非固定 16 kbps。
- 对典型的全天录音（有效语音占比约 30–50%），DTX 可额外节省 **20–40%** 存储。
- DTX 帧与正常帧平铺在同一 Opus 流中，解码器透明处理，不影响 ASR 流程。
- Audio Chunk 元数据记录实际编码配置（含 DTX 启用标记），使存储统计可审计。

### 11.4 采样率与声道约束

- **采样率**：语音 ASR 模型（SenseVoice、Whisper、Qwen3-ASR）的标准输入均为 16 kHz 单声道。使用 16 kHz 采集避免不必要的重采样，也避免 48 kHz 录制带来的 3× 存储浪费。
- **声道**：麦克风和系统音频各自独立保存为单声道 Chunk，不在采集阶段不可逆混音。双路合计 2× 单声道，非 2× 立体声。
- **ASR 工作副本**：原始 Opus 音频不解码转码为中间 PCM 文件；ASR 流程在内存中解码为 PCM 16kHz 送入模型，不在磁盘上产生派生音频文件。

### 11.5 静音裁剪（可选增强）

在 V0.1 已有的 VAD 分段基础上，未来可考虑：

- 在 Audio Chunk 封存时，对连续超过 N 秒的静音段（< 阈值 dBFS）生成 `Discontinuity` 并停止写入音频帧，而非保留静音 Opus 帧。
- 保留前后各 500ms 的上下文（padding），避免截断句首句尾。
- 此策略不适合作为默认策略，因为 DTX 已大幅降低静音成本，且保留完整音频流在证据链中更有价值。用户可通过设置显式开启。

### 11.6 编码升级路径

- 未来 Opus 1.5+ 引入的 ML-based 语音增强编码（如 Deep Redundancy）可在不改变容器格式的前提下提升低比特率质量。
- 原始音频 Chunk 不可变，编码升级只影响新采集的 Chunk。
- 用户可随时在设置中切换编码配置；已封存 Chunk 不重新编码。

---

## 12. 存储保留策略

LifeSub 全天录制的存储增长呈线性累积，必须建立明确的保留策略，让用户在磁盘空间和证据保存之间做出可控权衡。

### 12.1 存储配额上限

- 用户在设置中配置总存储上限（默认 50 GB，可选 20/50/100/200 GB 或自定义）。
- 当存储使用量达到配额的 **90%** 时，系统发出一次非阻塞提醒。
- 当存储使用量达到配额 **100%** 时，系统自动触发清理：按时间从最旧到最新删除原始音频 Chunk，直到使用量降至配额的 **80%**。
- 清理只删除原始音频文件；对应的 Transcript、Segment、Provider Receipt 和 Evidence Ref **永久保留**。
- 被清理的 Chunk 的 Evidence 状态更新为 `audio_archived`，产生 Tombstone。
- 用户可以查看已清理 Chunk 的转写文本，但不能重新验证、重转写或导出音频。

### 12.2 分级保留

| 层级 | 数据 | 默认保留期 | 行为 |
|---|---|---|---|
| **L1 原始音频** | Audio Chunk（Opus 文件） | **30 天** | 到期后自动清理（除非标记为重要或配额未满） |
| **L2 重要记录** | 用户标记为"重要"的记录的原始音频 | **永久** | 不计入配额清理范围，只能手动删除 |
| **L3 转写文本** | Transcript Revision、Segment、Provider Receipt | **永久** | 体积极小（~10 KB/小时），不参与配额清理 |
| **L4 Evidence** | Evidence Ref、Markdown 投影、FTS5 索引 | **永久** | 纯派生数据，可随时从 L3 重建 |

- 用户可在设置中调整 L1 保留期（7/14/30/60/90 天）。
- 用户可在记录详情页一键标记/取消"重要"。此操作只影响保留策略，不改变已封存的音频编码。
- 重要记录的原始音频永不自动清理；用户手动删除时需二次确认。

### 12.3 冷归档

用户可将超过指定天数的原始音频自动迁移到外部存储：

- **支持的目标**：外置硬盘、NAS（SMB/NFS 挂载路径）、网盘（本地同步目录）。
- **归档时机**：当音频 Chunk 的创建时间超过保留期，或用户手动触发。
- **归档操作**：将原始 Opus 文件复制到目标路径，校验 hash 一致后，原位置替换为占位符（symlink 或 stub 文件），Catalog 中记录归档路径和状态。
- **隐私声明**：用户首次配置网盘目标时，系统必须展示隐私风险声明——"网盘同步可能将音频上传至第三方服务器，请确认已了解隐私风险"——并需要用户主动确认。
- **回取**：用户访问已归档记录的音频时，系统检测归档路径是否可访问。若可访问，按需从归档位置读取；若不可访问，提示"音频已归档至 <路径>，当前无法访问"。
- **归档状态**：`audio_cold_stored` — 原始音频不在本地热存储中，但可回取。区别于 `audio_archived`（已永久删除）。

### 12.4 清理与归档的排除规则

以下 Chunk 不会被自动清理或归档，除非用户手动操作：

- 存在未完成的 ASR Job（状态为 `queued | preparing | transcribing | blocked_model`）。
- 关联的记录被用户标记为"重要"。
- Chunk 的创建时间在保留期（默认 30 天）内。

### 12.5 存储仪表盘

设置页提供存储概览：

```
┌─────────────────────────────────────────┐
│  存储使用                                │
│  ████████████░░░░░░░░  62.3 GB / 100 GB │
│                                          │
│  原始音频    58.2 GB  (L1, 保留 30 天)    │
│  重要记录     3.8 GB  (L2, 永久保留)      │
│  转写文本     0.02 GB (L3, 永久保留)      │
│  其他        0.28 GB                      │
│                                          │
│  预计下次清理: 15 天后 (12.5 GB)           │
│  归档状态: 未配置                          │
└─────────────────────────────────────────┘
```

---

## 13. 同步与多端

全天音频不使用 GitHub 作为主同步通道。

- 音频同步使用加密对象、manifest、内容 hash 和增量游标。
- Transcript 与 metadata 可以同步为加密结构化对象。
- Markdown 仅作为用户主动导出或可再生投影。
- GoldenWave 继续使用 Git 管理正式知识和治理审计。
- Malow 继续使用 Project 文件、SQLite 和 Git-ready Artifact。

桌面端采用共享 Rust Core 与不同 Capture Adapter。移动端采用 React Native / Expo companion，并通过服务 API或 UniFFI 复用 Core 能力。

iOS 无法承诺任意条件下全天后台录音；后续移动方案必须区分：

- 显式前台录制。
- Android 前台服务。
- 桌面常驻采集。
- 外部录音设备或 LifeSub 可穿戴硬件。

## 14. 跨系统一致性与故障边界

三系统不实现跨数据库分布式事务。跨项目写入使用 outbox / inbox、幂等键、回执和可重试状态。

- LifeSub 审计证据读取与撤回。
- Malow 审计证据引用、Organizer 和人工 Review。
- GoldenWave 审计 Candidate、确认、注入、撤回与 Context Pack。
- 任一系统暂时离线时，上游保留 pending 状态，不伪造下游成功。
- 未知 Contract 主版本 fail closed。

## 15. 版本路线

| 版本 | 目标 | 核心能力 |
|---|---|---|
| V0.1 | 可靠证据闭环 | macOS 手动长时录音、双路采集、原子分片、崩溃恢复、磁盘保护、队列背压、本地 ASR、原始/校对 revision、基础热词、文本分片、Markdown、FTS5、Evidence API、Malow 最小消费 |
| V0.2 | 质量与管理增强 | 高级音频质量检测、Opus 16kHz 16kbps 编码 + DTX 静音压缩、存储配额上限与自动清理、分级保留（L1-L4）、冷归档、诊断导出、音频导入、字幕导出、完整词库管理、多 Provider 对比重跑 |
| V0.3 | 说话人证据 | 匿名说话人分离、人工纠错、可选本地声纹 Profile、身份撤回和匹配质量评测 |
| V0.4 | 记录自动化 | 日历提醒、会议检测、确认式自动开始、结束提醒、后台状态、LLM 自动识别重要分片 |
| V0.5 | 多端与生态 | Windows/Linux Adapter、稳定 Contract、CLI、本地 API、更多 Evidence consumer、Qwen3-ASR Provider 候选评估 |
| V0.6 | 加密同步 | 多设备 manifest、加密对象同步、冲突和恢复 |
| V1.0 | 日常长期记录 | 质量、功耗、存储、隐私和长期运行达到可持续使用标准 |
| V2+ | 随身采集 | 移动 companion、现有录音设备、可穿戴硬件验证 |

Malow 的 Organizer、Knowledge Patch Review 和 GoldenWave Governance 不作为 LifeSub 版本内功能，只作为集成 Gate。

## 16. 参考项目取舍

### OpenWhispr

借鉴：双路会议音频、录音恢复、Provider 抽象、会议检测、本地 API/MCP、说话人能力和派生索引可重建。

不纳入 LifeSub 核心：通用 AI Agent、团队空间、云笔记、企业策略和全局语音听写。

### TypeWhisper

借鉴：macOS 原生采集、录音状态机、封装阶段、单录音重新转写、Provider/模型选择、隐私安全的本地 API和错误诊断。

不纳入 LifeSub 核心：文本插入、工作流、Snippets、Widgets、通用插件市场、面向目标应用的自动改写和日常听写统计。其热词、术语提示和用户纠错经验可用于设计 LifeSub 的证据质量词库。

TypeWhisper 使用 GPLv3；LifeSub 只做 clean-room 产品与架构借鉴，不复制其实现代码，除非未来明确接受 GPLv3 或取得商业许可。

## 17. V0.1 架构护栏

1. 所有持久领域对象由 Rust Core 管理。
2. Capture Adapter 不包含 ASR、检索或知识治理逻辑。
3. Contract 使用版本化、语言无关 Schema，不暴露平台类型。
4. 物理分片不可变，Transcript 使用 revision，不原地覆盖来源。
5. LLM 校对只产生结构化局部 edit，原始 ASR 永久可访问。
6. Speaker Profile 默认关闭并按生物特征数据加密、授权和撤回。
7. 文件引用使用资源 ID 和相对布局，不保存不可迁移绝对路径。
8. Core 从 V0.1 起在 macOS、Windows 和 Linux CI 运行测试。
9. Markdown、FTS 和未来向量索引均可从主数据重建。
10. LifeSub 不生成或写入 GoldenWave 正式知识。
11. Malow 与 GoldenWave 只通过 Contract 获取 Evidence，不读取 LifeSub 数据库。
12. 任何“记忆系统”需求先路由到 GoldenWave，不在 LifeSub 内扩展平行能力。
13. Provider、Prompt、词库规则和声纹模型变更必须通过固定语料回放 Gate。

## 18. 验收标准

### 18.1 V0.1 Gate

- 可以连续录制并形成多个可校验音频分片。
- 8 小时、24 小时和随机崩溃测试不会损坏已封存分片，未封存音频损失不超过 5 秒。
- 启动时可以对账并恢复数据库、对象目录、临时分片和处理任务。
- ASR 或校对积压不会阻塞采集，磁盘不足时能够预警并安全停止。
- 每段转写能够回到对应音频时间范围。
- 8 小时和 24 小时双路录音中，对齐后每小时漂移与累计折算漂移均不超过 50ms/小时；超过时必须标记质量降级并阻塞默认发布。
- 睡眠、输入设备切换、Capture Adapter 重启和模拟丢帧均生成准确的 `Discontinuity`，其前后 frame position、原因和持续时间可验证。
- 原始 ASR、LLM 校对和人工修订之间存在完整、可回滚的 revision 与 diff。
- 热词可以影响新 ASR 或显式重跑，但不会静默改变历史记录。
- 默认 ASR 与校对模型均有固定语料、指标结果和可回滚版本。
- 自动应用校对不允许出现缺少证据支持的实词变更；高风险实体和数字变更只能等待人工确认。
- 云端 ASR、云端校对和云端声纹分别授权，Provider Receipt 可以说明实际外发范围。
- Markdown 能从 Catalog 重建，且导航摘要不会被标记为正式知识。
- Malow 可以仅凭 Evidence Contract 读取获授权片段，无需访问 LifeSub 数据库。
- GoldenWave Candidate 可以保留 Malow 与 LifeSub 的完整 provenance 链。
- 删除或撤回 Evidence 后，下游可以检测其不可用状态。
- LifeSub 代码和数据模型中不存在 GoldenWave Profile、Persona、Knowledge 或 Malow Project/Matter 权威状态；V0.3 的 `SpeakerProfile` 仅是受限声纹身份对象。

### 18.2 V0.3 Speaker Gate

- 匿名说话人标签和实名声纹匹配有明确置信度，低置信度不会被强行命名。
- Speaker benchmark 报告 DER / JER、已知识别准确率、未知说话人误认率和拒识率。
- 未获得适用同意或合法依据时不能创建第三方 Speaker Profile。
- 删除 Speaker Profile 后，新任务不再使用该声纹，默认投影与 Contract 响应解除实名关联。
- 本地 embedding 加密保存；云端声纹处理需要独立授权和独立 Provider Receipt。
