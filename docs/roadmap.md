# 阶段路线图

## 当前进度（2026-08-21）

> 唯一开发/发布源：`/Users/goldenwave/Documents/MyProject/LifeSub` / `main`。外部 worktree 已降级为回滚来源。下一个产品实现里程碑是 Task 7。

| 里程碑 | 状态 | 说明 |
|---|---|---|
| V0.1 Evidence Core | 已完成 | SQLite Catalog、导入/hash、revision、搜索、Evidence URI、Markdown 导出 |
| V0.2 UI/Catalog 安全闭环 | 已完成 | 桌面真实 Catalog、durable Job、精确 model_id、manual provenance、多 Chunk 回放、设置/词典/笔记持久化 |
| V0.2 UI 走查整改 | 已完成 | 35 项问题收口；Critical/Important 独立复审均清零 |
| V0.2 arm64 重新打包与安装 | 已完成 | DMG 校验、bundle 签名验证、替换 `/Applications/LifeSub.app` 并启动确认 |
| 原生采集协议/Swift helper/签名/认证 | 已完成 | Task 3--6 已实现并通过最终复审 |
| Catalog v6 + atomic chunk sealing | 进行中（暂停） | Task 7 只有已观察的 migration RED，尚无生产实现 |
| production 真实实时双路采集 | 未完成 | Task 8 coordinator 未实现，当前生产采集路径继续 fail closed |
| native ASR production executor | 未完成 | coordinator/lease/recovery/cancel/shutdown 已完成；执行器当前 fail closed |
| Diarization / CAM++ | 未完成 | UI/CRUD 基础已有，模型推理未接通 |

当前下一里程碑：从 Task 7 RED 完成 Catalog v6 capture timing 和 atomic chunk sealing，再依次完成 Task 8 production coordinator、Task 9 NativeAsrEngine、Task 10 persisted UI events 与 Task 11/12 真机/安装包验收。

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
- 本地 ASR 默认 Provider，以及一个云端 ASR 通路。
- 可删除的导航摘要、Transcript Revision 和时间戳 Evidence。
- 本地时间线、搜索和 Evidence 详情。

当前状态：Catalog 与导入闭环已完成；真实双路采集、native 自动转写和导航摘要仍未完成，因此尚未达到 Phase 1 退出条件。

退出条件：用户可以在安装版完成一次真实会议记录，并在退出重启后准确检索、播放、导出 Transcript、导航派生物和 Evidence Ref；决定与行动项由 Malow 解释和 Review。验收数据不得来自 Demo/Mock。

## Phase 2：Agent 生态

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
