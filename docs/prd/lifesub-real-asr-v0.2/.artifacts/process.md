---
stage: implementation-task-2-complete
last_updated: 2026-08-15
source: codex-goal
---

# LifeSub 真实本地 ASR V0.2 进度

- 当前阶段：Task 1 与 Task 2 已完成并通过规格、代码质量双审。用户新增 Qwen3-ASR 0.6B 与 1.7B 可选项，PRD、设计和计划已修订。
- 已确认方向：本地优先；SenseVoiceSmall、Whisper 与 Qwen3-ASR 0.6B 共用 sherpa-onnx 1.13.5；无 Python Sidecar；无云端 ASR。Qwen3-ASR 1.7B 仅在固定可执行资产和 Apple Silicon Gate 通过后启用。
- 已完成研究：sherpa-onnx 1.13.5 已提供 `OfflineQwen3ASRModelConfig` 与 Rust 示例；0.6B INT8 官方 sherpa 包大小 878,702,423 B，SHA-256 为 `393f8a14e2f5fb96746aaab342997a40641001fbd5bf9592a080a8329178ee96`；未发现同等成熟的 1.7B sherpa 发布包。
- 验证证据：Task 1 archive/wrapper/Rust/native/desktop Gate 通过；Task 2 migration 24/24、Catalog 1/1、全量 no-default 31/31、Clippy `-D warnings` 通过。独立质量复审确认无 Critical/Important；SQL token 边界与 `qwen3_asr` Schema allowlist 均有回归测试。
- 下一步：执行 Task 3 ASR domain/settings，Provider 枚举包含 `sense_voice | whisper | qwen3_asr`，并明确拒绝未通过 capability Gate 的 1.7B。
- 阻塞项：Qwen3-ASR 1.7B 缺少已冻结的 sherpa 可执行包；这不阻塞 0.6B 首发，但阻止 1.7B 标记为可安装。
