---
stage: task-6-spec-review-fixes-implemented-rereview-next
last_updated: 2026-08-16
source: codex-goal
---

# LifeSub 真实本地 ASR V0.2 进度

- 当前阶段：Tasks 1-5 完成；Task 6 首轮规格复审的六项缺口均已按 RED→GREEN 修复，等待独立规格复审确认。Task 6 尚未批准完成。
- 已确认方向：本地优先；SenseVoiceSmall、Whisper 与 Qwen3-ASR 0.6B 共用 sherpa-onnx 1.13.5；无 Python Sidecar；无云端 ASR。Qwen3-ASR 1.7B 仅在固定可执行资产和 Apple Silicon Gate 通过后启用。
- 已完成研究：sherpa-onnx 1.13.5 已提供 `OfflineQwen3ASRModelConfig` 与 Rust 示例；0.6B INT8 官方 sherpa 包大小 878,702,423 B，SHA-256 为 `393f8a14e2f5fb96746aaab342997a40641001fbd5bf9592a080a8329178ee96`；未发现同等成熟的 1.7B sherpa 发布包。
- 验证证据：Task 6 focused model manager 70/70 no-default、trusted sherpa wrapper 71/71、Catalog migration 31/31、manifest 45/45（5 ignored discovery tests）；full no-default 212 passed / 5 ignored；Clippy all-targets `-D warnings` clean；trusted desktop feature check 与 `cargo fmt --check` 通过。新增证据覆盖 install 二次 preflight、assembly/structural/rename/Catalog publication/final state transition 的 failed/recovery_required 与同进程 retry、overlong/stalled response hard bound、canonical model-ID facade 与 path grammar、delete marker exact lease CAS、provider/model/final symlink 和 FIFO fail-closed，以及此前全部 checkpoint/extractor/reconcile/crash 窗口。Model manager 已拆为 facade + 8 个职责模块，测试拆为 11 个主题文件；除 531 行 download 模块外实现文件均不超过 419 行。v2/v3 fixture SHA-256 分别固定为 `e2956f8a5c0531e8b444519c8e11e2de5952f6b4b4ec391c3321e9f60e6e4639` 与 `79f8ec380b1555691e9bc4fd79bd743213b275270d35a61e791c0f278d970de2`，且无 WAL/SHM。
- ownership 边界：任何 writable Catalog open/migration/reconciliation 前先取得 canonical parent lifetime lock；AppState 的 create/update/import/append 全部通过统一 guarded facade。正常第二 LifeSub 实例 fail closed；Task 11 再改为连接 primary socket。
- 下一步：完成 Task 6 规格复审修复并重新执行独立规格/质量复审；批准后才进入 Task 7。Task 8 才允许初始化 Qwen3-ASR 1.7B Candle/Metal runtime qualification。
- MVP Gate：ASR V0.2 是基础里程碑；其后必须完成 native capture 与 DeepSeek Harness 真实录音到 `lifesub://` Evidence Ref 的完整闭环，才能标记 LifeSub 整体 MVP complete。
- 阻塞项：无实现阻塞；等待 Task 6 独立规格复审。Task5 `ArtifactBundle.install_constraints` 已成为 canonical derived install contract，且明确排除于 JCS identity。未提交工作区包含其他 agent 改动，本次继续保留且不提交。
