---
stage: handoff-task-4-quality-fix-and-task-5-design
last_updated: 2026-08-15
source: codex-goal
---

# LifeSub 真实本地 ASR V0.2 进度

- 当前阶段：会话 handoff。Tasks 1-3 完成；Task 4 已规格批准，质量修复未提交；Task 5 只有故意 RED 草稿。完整现场见 `.artifacts/handoff.md`。
- 已确认方向：本地优先；SenseVoiceSmall、Whisper 与 Qwen3-ASR 0.6B 共用 sherpa-onnx 1.13.5；无 Python Sidecar；无云端 ASR。Qwen3-ASR 1.7B 仅在固定可执行资产和 Apple Silicon Gate 通过后启用。
- 已完成研究：sherpa-onnx 1.13.5 已提供 `OfflineQwen3ASRModelConfig` 与 Rust 示例；0.6B INT8 官方 sherpa 包大小 878,702,423 B，SHA-256 为 `393f8a14e2f5fb96746aaab342997a40641001fbd5bf9592a080a8329178ee96`；未发现同等成熟的 1.7B sherpa 发布包。
- 验证证据：Task 4 imported_audio 3/3、service 14/14、Catalog 1/1、全量 no-default 59/59、Clippy clean、可信 native wrapper desktop check 通过。导入按 audio directory fsync -> temp sync/hash -> rename -> parent fsync -> session+chunk transaction 顺序提交；生产初始化自动 reconciliation。
- 下一步：按 handoff 先完成 Task 4 symlink/full Core ownership 修复验证与质量复审；再把 Qwen3-ASR 1.7B 改为 Candle/Metal 正式可安装模型并重启 Task 5。
- MVP Gate：ASR V0.2 是基础里程碑；其后必须完成 native capture 与 DeepSeek Harness 真实录音到 `lifesub://` Evidence Ref 的完整闭环，才能标记 LifeSub 整体 MVP complete。
- 阻塞项：当前 dirty worktree 混有 Task 4 未提交修复、Task 5 RED 和 Local Tool 文档复审修订，必须分组处理；Qwen3-ASR 1.7B runtime/provenance 设计尚未完成规格复审。
