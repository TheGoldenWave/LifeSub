# LifeSub 阶段路线图

> 2026-08-20 更新：遗留生产能力按“真实 ASR → 原生采集 → 匿名说话人 → 授权声纹”拆成独立版本。完整工作与验收要求见[遗留能力总实施计划](superpowers/plans/2026-08-20-lifesub-remaining-capabilities-roadmap.md)。

## 当前基础：V0.1 Evidence Core

已完成 Tauri/React 桌面壳、本地 SQLite Evidence Core、音频导入与 hash、append-only Transcript Revision、中文搜索、Evidence URI、Markdown 导出和显式 fail-closed 桌面路径。

## V0.2.1：真实本地 ASR

用户价值：导入音频后，可通过真实本地模型获得可追溯转写。

核心工作：

- 接通 native ASR production executor 和已完成的 Job worker 生命周期。
- 通过 sherpa-onnx 支持 SenseVoiceSmall 与 Whisper，不引入 Python Sidecar。
- 完成解码、16 kHz 单声道重采样、VAD、Segment、Receipt 和原子 Revision 发布。
- 将模型下载、取消、校验、安装、切换和删除后端能力接入 UI。
- 保留 fail closed、无静默回退、不可变音频和 append-only Revision。

退出条件：中文与英文/混合真实 fixture、重转写历史保护、失败无半条 Evidence，以及签名安装包内的双模型 smoke 全部通过。

## V0.2.2：真实桌面采集

用户价值：可直接录制麦克风或系统音频，并自动进入 V0.2.1 转写链路。

核心工作：

- AVAudioEngine 麦克风采集与 ScreenCaptureKit 系统音频采集。
- 权限、设备断开、来源移除、背压、磁盘保护和崩溃恢复。
- 双路来源分别保存为可追溯 Physical Audio Chunk。
- 统一 Opus 16 kHz Mono、16 kbps VBR + DTX 编码。
- 移除生产路径的 mock source，界面只展示真实可诊断状态。

退出条件：麦克风、系统音频、双路、权限拒绝、设备丢失、重启恢复、8 小时 soak 和签名安装包实机验收通过。

## V0.3：匿名说话人分离

用户价值：在转写时间线中看到匿名的“谁在什么时候说了什么”。

核心工作：

- Diarization runtime、匿名 speaker turn、重叠和置信度。
- 说话人时间段与 ASR Segment 的确定性对齐。
- 原始结果和人工合并、拆分、改派均保存为 append-only revision。
- Speaker 信息进入搜索、Evidence 解析和 Markdown 投影。

退出条件：固定多人语音集达到冻结的 DER/JER 阈值，人工修订不覆盖原始结果，Diarization 失败不影响既有 ASR Evidence。

## V0.3.1：CAM++ 声纹身份

用户价值：用户明确授权后，可将匿名说话人与本地 Speaker Profile 匹配。

核心工作：

- CAM++ embedding、多样本注册、质量检查和版本化。
- 本地加密 Speaker Profile、匹配阈值、拒识和未知说话人状态。
- 注册、重命名、撤回和删除交互。
- 保存模型版本、阈值、分数和来源 Diarization Revision。

退出条件：已知识别、未知说话人误认和拒识率达到冻结阈值；未授权不能创建 Profile；删除 Profile 后解除实名关联但保留匿名历史 Evidence。

## 后续重新排期

- 多设备时间校准、重复消除、ASR 冲突标注和合并 Revision。历史 V0.3 规划保留为研究输入，但实施前必须重新编号。
- 日历提醒、会议检测、确认式自动开始和全天候录音。
- Windows/Linux Capture Adapter、稳定公开 Contract、CLI 与更多 Evidence consumer。
- Qwen3-ASR、云端 ASR Provider 和第二套 ASR runtime。
- 移动 companion、外部录音设备和加密对象同步。

GitHub 不作为全天音频与转写的主存储或同步通道。
