# 产品定义

## 一句话定位

LifeSub（旁白）当前是一个本地优先的长时音频与 ASR 证据管理系统：可靠记录和分片个人声音资料，将 ASR 结果保存为可修订、可定位、可导出的 Markdown 与 Evidence，并交由 Malow 和 GoldenWave 完成后续理解与治理。长期上，LifeSub 演进为跨设备、跨模态的全领域个人 Evidence 基础设施。

## 核心问题

LifeSub 当前音频楔子只回答：

> **谁在什么时候说了什么，原始音频和对应文本在哪里？**

“这些内容对当前项目意味着什么”由 Malow 回答；“哪些内容值得成为长期事实、知识或人格上下文”由 GoldenWave 治理。

长期的全领域 Evidence Fabric 进一步回答：人经历了什么、设备感知到了什么、外部系统记录了什么，以及对应原件、来源和派生版本在哪里。

## 当前楔子与长期方向

- **当前实现楔子**：macOS 长时音频、声纹、ASR、Transcript Revision、Catalog 和 Evidence Ref。
- **长期产品方向**：统一接入声音、图片、视频、屏幕、智能硬件、健康传感器、环境传感器和数字活动等 Evidence。
- **不变边界**：LifeSub 记录 Observation 与可追溯 Derived Evidence；Malow 负责 Decision / Action；GoldenWave 负责正式长期 Memory。

长期架构和演进 Gate 见 `docs/full-spectrum-personal-evidence-strategy.zh-CN.md`。该方向不改变 V0.1/V0.2 的音频与 ASR 交付范围。

## 背景

长时或全天录音产品通常只覆盖部分链路：录音设备擅长采集，会议产品擅长单次转写，知识或 Agent 产品擅长理解，但个人仍缺少一个本地优先、可长期运行的 Evidence 基础层，统一处理：

1. 长时或全天录音与明确的录音状态。
2. 系统音频和麦克风等多来源采集。
3. 崩溃可恢复的有界音频分片。
4. 本地优先、可切换 Provider 的 ASR。
5. 音频、转写、revision 和时间范围的稳定对应。
6. 可再生 Markdown、基础检索和跨系统 Evidence Ref。
7. 对云端处理、声纹、读取、导出、删除与撤回的清晰控制。

## 产品愿景

LifeSub 最终由四部分组成：

- 采集端：桌面常驻采集、移动 companion、外部录音设备或可穿戴硬件。
- LifeSub Core：录音、分片、ASR、revision、Evidence Catalog、保留与访问控制。
- 管理界面：时间线、录音状态、转写修订、导出、存储、隐私和 Provider 设置。
- Evidence Contract：向 Malow、Codex 和其他获得授权的消费者提供稳定引用与证据解析。

LifeSub 不建设平行的个人记忆、Project 或知识治理体系。当前它是上层系统可以信赖的声音资料与证据底座；长期由 Source Adapter 将多模态与传感器来源接入同一个 Evidence Core。

## V0.1 目标

V0.1 聚焦 macOS，完成以下主链路：

1. 用户从菜单栏开始、暂停、恢复和停止一次长时记录。
2. LifeSub 分别采集系统音频和麦克风，并始终显示录音状态。
3. 原始音频先持久化，再滚动封存为不可变 Physical Audio Chunk。
4. 本地 ASR 默认生成带时间戳转写，也允许用户独立授权云端 ASR。
5. ASR 原始结果永久保留；规则、受约束 LLM 和人工修改创建新的 Transcript / Correction Revision。
6. LifeSub 按时间戳、静音、长度和录制状态形成 Logical Transcript Segment，不进行主题或决定级语义切分。
7. LifeSub 将记录投影为可重建 Markdown，可选导航摘要只帮助浏览。
8. 用户可以按时间、来源、设备和文本关键词查找记录，并回到对应音频范围。
9. Malow 可以仅凭 Evidence Contract 读取获授权片段，不需要访问 LifeSub 数据库。

音频文件导入作为补录与调试入口保留，但不是 V0.1 主体验。

## V0.1 非目标

- 自动开始的全天候录音；V0.1 先验证用户显式启动的长时录音。
- 自研可穿戴硬件。
- iOS、Android、Windows 和 Linux 正式客户端。
- 会议纪要、主题、决定、行动项或项目状态的权威抽取。
- 跨天、跨会议的记忆压缩和长期事实推断。
- Profile、Persona、Knowledge、Experience 或正式 Project Context。
- Malow Project / Matter / Organizer / Agent Run 状态。
- GoldenWave 候选治理、冲突检测、渲染和注入。
- 公网托管的个人 Evidence 服务。
- 多人团队知识库和企业权限体系。

## 核心用户价值

- 可靠：长时录音中断不会损坏已经封存的音频分片。
- 真实：保留原始 ASR、每次校对 revision、Provider receipt 和处理链。
- 可定位：每段文本都能回到准确的音频时间范围和来源声道。
- 可读：转写可以投影为结构清晰、可导出、可重建的 Markdown。
- 可控：云端 ASR、校对、声纹和证据访问分别授权，删除与撤回可追踪。
- 可组合：Malow 等消费者通过开放 Contract 引用证据，而不是复制数据库或制造第二事实源。

## 生态定位

```text
LifeSub Evidence
  -> Malow Organizer / Human Review
  -> user-confirmed Knowledge Patch
  -> GoldenWave Inbox / Governance
```

- LifeSub 是 Audio、Transcript 和 Evidence 的权威。
- Malow 是 Project / Matter 解释、候选整理和人工 Review 的权威。
- GoldenWave 是正式 Profile、Knowledge、Persona 和长期上下文治理的权威。

项目型和非项目型记录都需先经过 Malow 或等价人工 Review 入口形成候选；LifeSub 不直接生成或写入 GoldenWave 正式层。

## 成功标准（草案）

首轮验证至少应回答：

- 长时录音能否稳定形成多个独立、可校验、可恢复的音频分片。
- 任一处理阶段失败时，原始音频和已有转写是否仍可使用和单片重试。
- 中文及中英混合 ASR 是否足以忠实还原“谁在什么时候说了什么”。
- 原始 ASR、规则校对、LLM 校对和人工修订是否可 diff、可回滚、可追溯。
- Markdown 是否可以完全从 Evidence Catalog 重建。
- Malow 是否可以通过 Evidence Contract 准确解析到对应文本和音频范围。
- 删除或撤回 Evidence 后，下游是否能检测来源不可用。
- 用户是否能理解每次云端处理、声纹处理和证据读取的数据去向。
