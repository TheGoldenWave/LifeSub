---
req_id: lifesub-real-asr-v0.2
source: product-initiated
status: confirmed
created_at: 2026-08-15
---

# LifeSub 真实 ASR 新版本原始设想

## 原始需求

推进 LifeSub 新版本开发，完成真实 ASR 能力接入，支持阿里模型和 OpenAI 开源模型切换，并新增设置功能管理 ASR 配置。方案参考项目既有的 OpenWhispr 与 TypeWhisper 调研结论。

## 已恢复的项目背景

- V0.1 已具备音频导入、不可变 Audio Chunk、Transcript Revision、FTS5 检索、Evidence URI、Markdown 导出和静态设置页。
- 当前“本地演示 ASR”只生成占位文本，没有加载或执行真实模型。
- 项目已有本地优先、Provider 可切换、处理结果可追溯、原始 revision 不覆盖等架构约束。

## 本轮需求解释

- “阿里模型”按本地开源路径解释为 SenseVoiceSmall/FunASR 系列。
- “OpenAI 开源模型”按 Whisper 系列解释。
- 本版本不引入 DashScope 云端 ASR，避免把云端凭据、数据外发授权和地域合规混入本地模型版本。
- 两种模型必须在同一桌面应用中安装、切换、执行真实转写并产生新的不可变 revision。

## 参考项目结论

- OpenWhispr：借鉴模型卡、下载状态、Provider 路由、失败不静默回退和重转写流程。
- TypeWhisper：借鉴引擎/模型分离、模型就绪状态、文件批量任务状态和本机密钥/数据边界。
- TypeWhisper 为 GPLv3，仅做 clean-room 产品与架构借鉴，不复制实现代码。
- sherpa-onnx 为 Apache-2.0，Rust API 同时支持 SenseVoice 与 Whisper，可静态链接进 Tauri。

## 成功定义

用户可以下载至少一个 SenseVoice 模型和一个 Whisper 模型，在设置页切换当前 ASR 配置，导入真实音频后得到非占位转写，并能对同一记录使用另一模型重新转写；每次结果都保留 Provider、模型、参数、时间范围和输入 hash。
