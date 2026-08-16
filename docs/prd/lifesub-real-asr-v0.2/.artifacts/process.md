---
stage: tasks-8-9-approved-target-tasks-10-13
last_updated: 2026-08-16
source: codex-goal
---

# LifeSub 真实本地 ASR V0.2 进度

- 当前阶段：Task 8、9 均已通过最终规格与质量双审；下一连续交付阶段以完成 Tasks 10–13 为目标，之后再进入 Task 14 真实模型 Gate。
- 已确认方向：本地优先；SenseVoiceSmall、Whisper 与 Qwen3-ASR 0.6B 共用 sherpa-onnx 1.13.5；无 Python Sidecar；无云端 ASR。Qwen3-ASR 1.7B 仅在固定可执行资产和 Apple Silicon Gate 通过后启用。
- 已完成研究：sherpa-onnx 1.13.5 已提供 `OfflineQwen3ASRModelConfig` 与 Rust 示例；0.6B INT8 官方 sherpa 包大小 878,702,423 B，SHA-256 为 `393f8a14e2f5fb96746aaab342997a40641001fbd5bf9592a080a8329178ee96`；未发现同等成熟的 1.7B sherpa 发布包。
- Task 8 质量修复：Q1 production qualification 仅暴露 ModelManager-owned `qualify_qwen17_current_device`/`reconcile_qwen17_current_device`，固定当前设备与真实 Qwen Candle/Metal smoke，泛型 fake smoke 仅 `cfg(test)`；Q2 Provider 持有共享 registry 的 RAII execution lease，删除在 Provider 存活时返回 `model_in_use`、drop 后成功，inventory 仅验证一次；Q3 UUID 临时 marker 在所有 publish 错误路径清理，reconcile 扫描并 fsync 清理 stale temp；Q4 显式选择 ignored real gate 而缺环境变量时稳定非零失败；Q5 qualification contract 冻结原文、四个 phrases、2/4 threshold、NFKC+alphanumeric+lowercase normalization、原始/PCM hashes、archive/license/provenance，并以 canonical SHA-256 `b96f1f2f268ae54694e4d2a6a036e3ac8a94759db389e47e1005387239147006` 同时绑定 fixture metadata 与 runtime identity，任一 metadata mutation fail closed。
- Task 8 最终结论：规格与质量双审放行，Critical=0、Important=0。新增错误码精确映射测试、execution lease/delete 原子 reservation、Qwen 1.7B production Metal failure no-fallback seam；验证包括 no-default provider 12/12、asr-runtime provider 13/13、Qwen17 feature provider 14/14、model manager 72/72、lease/delete race 1/1、desktop check、Clippy、fmt 与 diff check。
- Task 9 最终结论：规格与质量双审放行，Critical=0、Important=0、Minor=0。固定 Core boot ID；30 秒 lease/5 秒 renew；RAII 单 Coordinator；raw claim API 收口；`JobControl` 分离；cancel/ownership 分型；recovery 清 stale active；`fail()` 与 cancel 竞态通过事务内 `OwnedMutationResult` 和 fenced acknowledge 原子关闭。
- Task 9 最终验证：目标竞态 1/1、focused 21/21、全量 no-default 283 passed / 5 ignored；fmt、no-default all-targets Clippy `-D warnings`、trusted desktop check、`git diff --check` 与无 `console.log` 均通过。
- ownership 边界：任何 writable Catalog open/migration/reconciliation 前先取得 canonical parent lifetime lock；AppState 的 create/update/import/append 全部通过统一 guarded facade。正常第二 LifeSub 实例 fail closed；Task 11 再改为连接 primary socket。
- 下一阶段目标：先按 ownership 分组提交当前未提交的 Task 8/9 修复与文档，然后连续完成 Tasks 10–13：Task 10 原子 Receipt/Revision/Segment/FTS 发布；Task 11 单一 CoreRuntime、Catalog v4、版本化 Tool/Application API 与安全本地 IPC；Task 12 typed ASR client 和真实设置/模型 UI；Task 13 Job 状态、Receipt 来源与重转写 UI，并彻底移除 `demo-local` revision 路径。
- 阶段退出条件：Tasks 10–13 各自完成 TDD、规格复审和质量复审；Rust 原子发布/迁移/Tool API/IPC/双 Tauri harness、前端 focused tests、生产构建和无 `console.log` 全部通过。Task 10 执行循环必须维持不超过 5 秒 heartbeat，ownership lost 时丢弃结果；Task 11 后第二 Tauri 进程只能连接 primary，不得打开 writable SQLite；Task 13 后导入与重转写只消费真实 Core Job/Operation，不合成 transcript。
- MVP Gate：ASR V0.2 是基础里程碑；其后必须完成 native capture 与 DeepSeek Harness 真实录音到 `lifesub://` Evidence Ref 的完整闭环，才能标记 LifeSub 整体 MVP complete。
- 阻塞项：Task 8/9 修复尚未提交；真实 Qwen 1.7B weights/device acceptance 未执行，但它属于 Task 14，不阻塞 Tasks 10–13。工作树还混有既有 Task 4/后续文档改动，禁止 reset/checkout 或整批提交。
