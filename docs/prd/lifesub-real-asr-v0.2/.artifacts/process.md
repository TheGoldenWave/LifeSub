---
stage: implementation-task-3-complete
last_updated: 2026-08-15
source: codex-goal
---

# LifeSub 真实本地 ASR V0.2 进度

- 当前阶段：Task 1 至 Task 3 已完成并通过规格、代码质量双审。Qwen3-ASR 0.6B/1.7B 范围修订也已通过独立规格审查。
- 已确认方向：本地优先；SenseVoiceSmall、Whisper 与 Qwen3-ASR 0.6B 共用 sherpa-onnx 1.13.5；无 Python Sidecar；无云端 ASR。Qwen3-ASR 1.7B 仅在固定可执行资产和 Apple Silicon Gate 通过后启用。
- 已完成研究：sherpa-onnx 1.13.5 已提供 `OfflineQwen3ASRModelConfig` 与 Rust 示例；0.6B INT8 官方 sherpa 包大小 878,702,423 B，SHA-256 为 `393f8a14e2f5fb96746aaab342997a40641001fbd5bf9592a080a8329178ee96`；未发现同等成熟的 1.7B sherpa 发布包。
- 验证证据：Task 1 runtime Gate 通过；Task 2 migration 24/24；Task 3 focused 18/18、全量 no-default 48/48、Clippy `-D warnings` 通过。领域层已固定三 Provider snake_case、受控 `AsrLanguage`、selectable/installable/executable 能力、完整稳定错误码、验证时间范围和 fail-closed Provider Receipt。
- 下一步：执行 Task 4 crash-safe immutable audio import 与启动 reconciliation。
- 阻塞项：Qwen3-ASR 1.7B 缺少已冻结的 sherpa 可执行包；这不阻塞 0.6B 首发，但阻止 1.7B 标记为可安装。Task 10 持久化 Provider Receipt 时需增加受控 getters/into_parts，不得使用 draft 绕过验证。
