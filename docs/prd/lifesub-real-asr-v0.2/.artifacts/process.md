---
stage: spec-approved-user-review
last_updated: 2026-08-15
source: codex-goal
---

# LifeSub 真实本地 ASR V0.2 进度

- 当前阶段：技术设计与 PRD 已通过第五轮独立规格复审，等待用户确认后生成 TDD 实施计划。
- 已确认方向：本地优先；SenseVoiceSmall 与 Whisper；统一 sherpa-onnx Rust 运行时；无 Python Sidecar；无云端 ASR。
- 已完成研究：OpenWhispr、TypeWhisper、sherpa-onnx Rust API、官方模型包大小与许可证。
- 下一步：用户审阅并确认规格；随后生成逐文件、逐测试的 TDD 实施计划。
- 阻塞项：无技术阻塞；真实模型 checksum、音频 fixture 和 macOS bundle 验证将在实施阶段固化。
