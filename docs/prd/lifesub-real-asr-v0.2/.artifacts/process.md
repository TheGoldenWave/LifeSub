---
stage: implementation-task-1-complete
last_updated: 2026-08-15
source: codex-goal
---

# LifeSub 真实本地 ASR V0.2 进度

- 当前阶段：Task 1 已完成并通过规格、代码质量双审。已固定 sherpa-onnx 1.13.5、native archive URL/size/SHA、无 native feature 边界及可信构建 wrapper。
- 已确认方向：本地优先；SenseVoiceSmall 与 Whisper；统一 sherpa-onnx Rust 运行时；无 Python Sidecar；无云端 ASR。
- 已完成研究：OpenWhispr、TypeWhisper、sherpa-onnx Rust API、官方模型包大小与许可证。
- 验证证据：archive-only shell suite 通过；wrapper locking/quarantine suite 通过；Rust no-default 7/7；native identity 精确测试 1/1；desktop check 通过。
- 下一步：Task 2，引入 `user_version = 2` 的版本化 Catalog 迁移和真实 V0.1 fixture。
- 阻塞项：无技术阻塞；真实模型 checksum、音频 fixture 和 macOS bundle 验证将在实施阶段固化。
