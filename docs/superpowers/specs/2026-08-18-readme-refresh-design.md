# LifeSub README 更新设计

> 历史设计记录：本文记录 2026-08-18 README 更新时的版本口径。2026-08-20 起，现行路线以 `docs/roadmap.md` 和 `docs/superpowers/plans/2026-08-20-lifesub-remaining-capabilities-roadmap.md` 为准。

## 目标

将根目录 `README.md` 更新为同时面向产品访客与开发者的项目入口，并以最近的 PRD、进度存档、架构决策和实际代码状态为事实来源。

## 读者与信息顺序

README 采用产品优先、开发信息紧随其后的结构：

1. 产品名称、一句话定位和当前阶段标签。
2. 已有桌面界面截图，让访客快速建立产品认知。
3. LifeSub 解决的问题与 Evidence-first 边界。
4. V0.1 已交付能力矩阵，明确真实能力和演示能力。
5. 运行前提、支持范围、本地运行、桌面运行与验证命令。
6. 分开说明 V0.1 已实现架构与目标架构、核心数据对象和目录说明。
7. V0.2、V0.3 与更长期路线图。
8. LifeSub、Malow、GoldenWave 的生态职责。
9. 文档导航、数据安全边界和许可证状态。

## 状态口径

- `V0.1 已交付`：Tauri + React 桌面壳、本地 SQLite Evidence Core、录音状态机、音频导入与 hash、append-only Transcript Revision、中文检索、Evidence URI、Markdown 导出、设置页和浏览器演示模式。
- `V0.1 当前边界`：录音控制与 ASR 使用确定性演示路径；真实系统音频/麦克风采集和真实模型尚未接入。
- `V0.2 已设计，待实现`：仅本地 SenseVoiceSmall 与 Whisper，统一 sherpa-onnx Rust 运行时，包含模型管理、任务恢复、Provider Receipt 和重转写；本版本不包含云端 ASR。
- `V0.3 规划中`：多设备 Evidence 对齐、去重、冲突治理和不可变合并 Revision。
- 后续方向：在本地 ASR 稳定后接入独立授权、明确展示数据去向并保留 Provider Receipt 的可选云端 ASR；同时推进原生双路采集、Evidence Contract 与获授权的下游集成。不将路线图能力描述为当前可用。
- 架构分层：V0.1 已实现部分只描述 React/Tauri、Rust Core、SQLite、本地对象副本和 Markdown 投影；macOS 原生 Capture Adapter 标注为待实现，sherpa-onnx ASR 标注为 V0.2 计划，多设备 Reconciler 标注为 V0.3 计划。

## 内容修正

- 将 README 中未标版本的云端 ASR 表达改为分阶段口径：V0.2 只做本地 ASR，云端 Provider 在后续版本独立授权接入。
- 不沿用路线图中已过时的“个人记忆”“结构化摘要”措辞；README 始终使用 Audio、Transcript 和 Evidence 的产品边界。
- 安装包只说明 2026-08-18 快照中已完成本地构建、checksum 和 app bundle 签名验证的 arm64 DMG，不提供不存在的公开 Release 下载链接，也不宣称已公证。
- 测试数字标注为 2026-08-18 已验证快照：前端 6 项、Rust 6 项、Playwright 1 项；命令部分提供可重复验证方式。
- 运行前提的具体版本从仓库 manifest 与 lockfile 核对；支持范围只陈述有文档依据的事实：V0.1 聚焦 macOS、已验证 DMG 为 arm64，V0.2 Gate 为 macOS 14+ Apple Silicon。

## 表达与视觉

- 中文为主，保留必要的英文领域名词和命令。
- 使用简短段落、状态表格和有限列表，减少长篇愿景叙述。
- 使用仓库内现有桌面截图，不生成新的品牌素材。
- 不使用大量徽章或动态服务，避免 README 依赖尚未建立的 CI/Release 基础设施。

## 验证

- 检查所有相对链接和图片路径存在。
- 对照 V0.1/V0.2 进度存档与 PRD，确认没有将规划能力写成已交付。
- 检查 Markdown 格式、命令和 Git diff。
- 更新 `docs/prd/lifesub-v0.1/.artifacts/process.md`，记录 README 已按最新进度刷新。
