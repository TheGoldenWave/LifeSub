---
stage: task-4-quality-approved-task-5-design-next
last_updated: 2026-08-16
source: codex-goal
---

# LifeSub 真实本地 ASR V0.2 进度

- 当前阶段：Tasks 1-4 完成。Task 4 的 symlink escape、目录替换竞态、full Core ownership、orphan grace、unknown integrity 和 importer grammar 修复已通过规格复审与代码质量复审；Task 5 仍只有故意 RED 草稿。
- 已确认方向：本地优先；SenseVoiceSmall、Whisper 与 Qwen3-ASR 0.6B 共用 sherpa-onnx 1.13.5；无 Python Sidecar；无云端 ASR。Qwen3-ASR 1.7B 仅在固定可执行资产和 Apple Silicon Gate 通过后启用。
- 已完成研究：sherpa-onnx 1.13.5 已提供 `OfflineQwen3ASRModelConfig` 与 Rust 示例；0.6B INT8 官方 sherpa 包大小 878,702,423 B，SHA-256 为 `393f8a14e2f5fb96746aaab342997a40641001fbd5bf9592a080a8329178ee96`；未发现同等成熟的 1.7B sherpa 发布包。
- 验证证据：fresh trusted wrapper 全量 no-default 89/89；Clippy `-D warnings` clean；desktop feature check 通过；`git diff --check` 通过；无 `console.log`。Task 4 focused service 43/43、Catalog 2/2、desktop guarded mutation 2/2。导入保持 directory fsync -> temp sync/hash -> atomic rename -> parent fsync -> session+chunk transaction，并在成功发布前复验 anchored directory identity。
- ownership 边界：任何 writable Catalog open/migration/reconciliation 前先取得 canonical parent lifetime lock；AppState 的 create/update/import/append 全部通过统一 guarded facade。正常第二 LifeSub 实例 fail closed；Task 11 再改为连接 primary socket。
- 下一步：先完成未提交 Local Tool/lifesubd 文档修订复审与独立提交；再更新 PRD/design/plan，把 Qwen3-ASR 1.7B 定为 Candle/Metal 正式 installable/executable，并重写 Task 5 manifest RED tests。
- MVP Gate：ASR V0.2 是基础里程碑；其后必须完成 native capture 与 DeepSeek Harness 真实录音到 `lifesub://` Evidence Ref 的完整闭环，才能标记 LifeSub 整体 MVP complete。
- 阻塞项：Qwen3-ASR 1.7B runtime/provenance 设计尚未完成规格复审；Task 5 RED 仍基于已推翻的 ExperimentalUnavailable 决策，不能直接实现。
