# 阶段路线图

## 当前进度与近期顺序（2026-10-13 校对）

> 唯一开发/发布源为主工作区 main；旧回滚 worktree 已移除。当前状态以 [过程记录](prd/lifesub-real-asr-v0.2/.artifacts/process.md) 为准，构建前仍须按 [工作区规则](workspace-status.md) 复核。当前实现里程碑是 0.2.2 S1–S4，不是历史 Task 7。

本次依据 [Qwen Audio 产品与方案分析](context/wiki/syntheses/qwen-audio-product-direction-review.md) 校对。LifeSub 负责 Audio/Transcript/Evidence；Malow 负责解释、Review 与行动；GoldenWave 负责正式长期 Memory。会议 ASR 研究不会自动扩大 LifeSub 为语音执行助理。

10 月 13 日新增 [VoiceStudio 固定提交源码对照](context/wiki/syntheses/voicestudio-source-review-2026-10-13.md) 为高优先级参考。它提示在既有 0.2.2 S4 基线中量测诊断扫描和冷/热模型成本；后续把 ASR 原生崩溃隔离、只读音频质量提示、存储快照完整性与模型许可证据分别交给真实失败归因和独立设计 Gate。此项是参考研究和计划校对，未启动 0.2.2，也不将语音克隆、配音、Electron/Python 架构或远程语音 API 纳入 LifeSub 近期范围。

| 里程碑 | 状态 | 说明 |
|---|---|---|
| V0.1 Evidence Core | 已完成 | SQLite Catalog、导入/hash、revision、搜索、Evidence URI、Markdown 导出 |
| V0.2 UI/Catalog 安全闭环 | 已完成 | 桌面真实 Catalog、durable Job、精确 model_id、manual provenance、多 Chunk 回放、设置/词典/笔记持久化 |
| V0.2 UI 走查整改 | 已完成 | 35 项问题收口；Critical/Important 独立复审均清零 |
| V0.2 arm64 重新打包与安装 | 已完成 | DMG 校验、bundle 签名验证、替换 `/Applications/LifeSub.app` 并启动确认 |
| 原生采集协议/Swift helper/签名/认证 | 已完成 | Task 3--6 已实现并通过最终复审 |
| Catalog v6 + atomic chunk sealing | 已完成 | 已进入 0.2.1 生产闭环与恢复验收 |
| production 原生双路采集 | 已完成 | 0.2.1 的 20/20 原生场景验收已收口；不等于逐字字幕 |
| native ASR production executor | 已完成 | 已接通实际模型、Job、Receipt/Revision 发布；质量结论仅覆盖已测样本 |
| Diarization / CAM++ | 未完成 | UI/CRUD 基础已有，模型推理未接通 |
| 0.2.1 发布与 UI 更新 | 已收口 | 9 月 2 日发布闭环，9 月 3 日 UI/滚动修复安装；引用历史验收，本轮未重跑 |
| 0.2.2 轻量稳定化 | 开发与验收中 | S1 信息组织、S2 现有状态、S3 操作便利已完成差量实现；S4 基线及安装包发布门禁进行中 |

近期以 [现行实施计划](superpowers/plans/2026-08-26-lifesub-asr-audio-settings-reference-optimization.md) 的 S1–S4 为上限，不新增 schema、helper 边界或 Evidence 发布语义。该计划 Task 6、9–16 继续后置。

之后先建立会议质量与真实找回基线，并区分听不清/削波、模型错字、双源重复、等待或崩溃、存储压力等失败原因，再选择术语/匿名说话结构、双源重复、找回体验、存储生命周期或一个消费者闭环中的单一主题。若实测出现原生 ASR 崩溃或不可终止的超时，优先评估进程隔离；若有音频完整但不可听的片段，评估只读、可定位的信号告警。结构化会议模型、CAM++、preview、AEC 都须独立验证；不预先把它们承诺为 0.3/0.4 的整包功能。模型研究准备不增加 0.2.2 的发布门槛。

以下 Phase 为长期能力地图，不是已承诺日历排期。旧专题计划中的版本号只代表历史设想，与此处及最新过程记录冲突时以当前口径为准。

## Phase 0：设计与验证准备

- 完成产品、架构、隐私和插件规格。
- 确认 macOS 音频采集可行性与系统权限边界。
- 建立 ASR 与摘要评测样本。
- 定义 Evidence Catalog 数据模型和 Agent 工具契约。
- 明确 Malow 插件接口。

退出条件：正式设计规格和实施计划获得批准。

## Phase 1：软件 Evidence 闭环

- macOS 菜单栏开始、暂停和停止录制。
- 系统音频与麦克风双路采集。
- 音频文件导入。
- 本地 ASR 默认 Provider、Transcript Revision 和时间戳 Evidence。
- 云端 ASR 与可删除导航摘要为后续候选，分别需要授权边界和找回价值验证，不是本地闭环的必要条件。
- 本地时间线、搜索和 Evidence 详情。

当前状态：本地真实采集、自动转写、Catalog、搜索、回放和导出已完成定义内闭环。导航摘要、云端与完整删除/撤回等更广能力不因此视为已交付。

本地闭环退出条件：用户可以在安装版完成一次真实记录，并在退出重启后检索、播放和导出 Transcript 与 Evidence Ref；既有 0.2.1 验收已覆盖。下一阶段验证真实用户找回价值与复杂会议质量，不把工程验收等同于所有会议都准确。决定与行动项由 Malow 解释和 Review。

## Phase 2：Agent 生态

稳定化后的已确认产品主题包括统一「AI 校正」：通过设置勾选上下文/热词、断句、去重、口头语、说话人和背景过滤等维度；维度默认值为设计建议。用户只面对原始 ASR 与 AI 校正，不建立第三层目标对话副本。声学与文本能力可逐步交付，统一结果和最小追溯数据；详见 [产品定义](product-brief.md)。该确认不表示 0.2.2 已扩范围或功能已启动。

先选一个获授权的 Evidence 消费者验证引用、回听、Review 和撤回状态，再考虑多宿主一致性；不在 LifeSub 内重建语音 Agent、执行调度或长期记忆。

- LifeSub MCP Server 与 Codex 插件。
- DeepSeek Harness 原生插件。
- Malow 原生插件。
- 统一权限、错误与审计行为。

退出条件：三个宿主对同一查询返回一致的 Evidence、Revision、状态与来源。

## Phase 3：隐私同步

- 独立 GitHub 私有 Evidence 投影库初始化。
- 普通 Transcript 与导航派生物的可读同步。
- 敏感 Evidence 投影的客户端加密同步。
- 多设备拉取、冲突和恢复流程。
- 删除与 Git 历史风险提示。

退出条件：在第二台设备恢复可检索 Evidence，且 GitHub 不出现未授权原文、传感器数据或音视频原件。

## Phase 4：随身采集

- 手机伴侣或现有录音设备导入。
- 评估可穿戴硬件原型。
- 验证续航、收音、佩戴、提示和同步。

退出条件：明确自研硬件是否具备相对现成设备的真实优势。

## Phase 5：多模态 Evidence

- 通过 Source Adapter 接入图片、屏幕、低频视频或第一人称视觉来源。
- 建立 OCR、多模态描述等 Derived Evidence，并保留原件、模型、版本和生成轨迹。
- 将多来源记录对齐到统一时间线，但不覆盖或删除原始 Evidence。
- 对视觉采集使用比音频更严格的状态提示、同意、保留和删除策略。

退出条件：至少一种非音频来源能通过统一 Evidence Contract 被解析、授权、撤回和下游引用，且不会被误认为正式 Memory。

## Phase 6：传感器与数字状态 Evidence

- 通过窄 Adapter 接入心率、睡眠、血氧、活动量等智能硬件或健康平台数据。
- 评估日历、通信、浏览和应用事件等数字活动 Evidence。
- 建立时间序列聚合、设备身份、来源真实性、重复合并和撤回语义。
- Malow 消费 Evidence 做 Decision / Action，执行回执可反向形成新的 Evidence。

退出条件：传感器或数字活动来源能保持原始数据权威、派生计算可重建、用户能预测权限与删除结果，并通过稳定 Ref 服务至少一个真实 Malow 工作闭环。

## 明确延后

- 全天候自动录音。
- 公网个人 Evidence 服务。
- 团队与企业版本。
- Agent 实时上下文注入。
- Agent 远程控制录音。

全领域 Evidence 是已确认长期方向，但 Phase 5/6 不进入当前里程碑，不得用于延后真实音频采集和 native ASR 验收。
