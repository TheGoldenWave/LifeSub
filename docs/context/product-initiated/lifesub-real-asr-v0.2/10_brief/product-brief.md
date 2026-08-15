---
req_id: lifesub-real-asr-v0.2
source: product-initiated
status: confirmed
owner: LifeSub
---

# 产品简报 - LifeSub 真实本地 ASR V0.2

## 发现来源

V0.1 已验证从音频导入到 Evidence Revision、检索与导出的完整链路，但演示 Provider 无法证明 LifeSub 的核心价值。OpenWhispr 与 TypeWhisper 均表明，桌面 ASR 产品需要把引擎选择、模型安装、就绪状态、错误诊断和重新转写作为同一完整体验，而不是只暴露一个模型名称下拉框。

## 核心假设

我们相信，使用统一的本地 Rust 运行时接入 SenseVoice 与 Whisper，可以在不引入 Python 环境和云端隐私风险的前提下完成真实 ASR 闭环。验证方式是让两个模型分别对固定中文/英文语音样本与用户导入音频产生可追溯 Transcript Revision，并完成模型切换和重转写验收。

## 功能概要

- 使用 sherpa-onnx Rust API 执行 SenseVoiceSmall 与 Whisper 离线转写。
- 管理模型目录、下载、校验、安装、删除和就绪状态。
- 在设置页选择 Provider、模型、语言、线程、VAD 和模型专属参数。
- 音频导入后自动创建 ASR Job，成功后追加 Transcript Revision。
- 记录详情支持使用当前设置重新转写，不覆盖历史 revision。
- 保存 Provider Receipt 和可诊断失败状态。

## 预期效果

| 指标 | 当前值 | 目标值 | 衡量方式 |
|---|---:|---:|---|
| 真实本地 ASR Provider | 0 | 2 | SenseVoice 与 Whisper 均通过真实模型验收 |
| Provider 切换可用性 | 不可用 | 100% | 设置保存后新任务使用所选 Provider |
| Revision 可追溯性 | 仅 provider 字符串 | 完整 | 保存模型、参数、输入 hash、耗时和错误 |
| 固定语音样本转写成功率 | 0% | 100% | 中文/英文样本均产生非空 Segment |
| 历史结果保护 | 已支持人工 revision | 继续保持 | 重转写不覆盖旧 revision |

## 风险与依赖

- 模型包体积约 116 MB 至 639 MB，需要可靠下载、空间检查和取消/重试。
- 音频导入当前支持多种容器，真实 ASR 前必须稳定解码并重采样为模型要求的 PCM。
- SenseVoice 原生时间戳能力有限，需要通过 VAD 分段形成一致的时间范围。
- sherpa-onnx 静态链接会增加构建和签名复杂度，必须验证 macOS bundle。
- 模型与运行时许可证需要随应用展示并留存来源信息。

## 关联文档

- 原始设想：`../00_discovery/original-idea-20260815.md`
- PRD：`../../../prd/lifesub-real-asr-v0.2/PRD.md`
- 技术设计：`../../../superpowers/specs/2026-08-15-lifesub-real-asr-design.md`
