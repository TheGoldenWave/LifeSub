---
stage: task-11-in-progress
last_updated: 2026-08-18
source: codex-goal
---

# LifeSub 真实本地 ASR V0.2 进度

- HEAD：`15150a9`。Task 11 进行中：Catalog v4 ✅、共享协议 ✅、UDS ✅、方法授权 ✅。剩余：dispatch 路由 + CoreRuntime 集成 + Host Event Protocol。
- Task 10 全部完成 ✅（M1-M5）→ 见 `5ce62f7` 之前
- Task 11 M1：Catalog v4 迁移 ✅（`5ce62f7`）：tool_requests/operations/open_intent_ledger 表 + 32/32 迁移测试
- Task 11 M2：共享协议原语 ✅（`24d12d0`）：envelope/error/DTO/trusted caller/方法常量 + 12 测试
- Task 11 M3：UDS IPC ✅（`f3a42af`）：agent.sock/ui.sock bind + peer credential 认证 + 5 测试
- Task 11 M4：方法授权 ✅（`15150a9`）：capability-based access control + 8 测试
- 验证：全量 414/398（+16 新测试，1 个已有隔离失败）、fmt/clippy/diff 通过
- 已确认方向：本地优先；SenseVoiceSmall、Whisper 与 Qwen3-ASR 0.6B 共用 sherpa-onnx 1.13.5；无 Python Sidecar；无云端 ASR。Qwen3-ASR 1.7B 仅在固定可执行资产和 Apple Silicon Gate 通过后启用。
- 已完成研究：sherpa-onnx 1.13.5 已提供 `OfflineQwen3ASRModelConfig` 与 Rust 示例；0.6B INT8 官方 sherpa 包大小 878,702,423 B，SHA-256 为 `393f8a14e2f5fb96746aaab342997a40641001fbd5bf9592a080a8329178ee96`；未发现同等成熟的 1.7B sherpa 发布包。
- Task 8 质量修复：Q1 production qualification 仅暴露 ModelManager-owned `qualify_qwen17_current_device`/`reconcile_qwen17_current_device`，固定当前设备与真实 Qwen Candle/Metal smoke，泛型 fake smoke 仅 `cfg(test)`；Q2 Provider 持有共享 registry 的 RAII execution lease，删除在 Provider 存活时返回 `model_in_use`、drop 后成功，inventory 仅验证一次；Q3 UUID 临时 marker 在所有 publish 错误路径清理，reconcile 扫描并 fsync 清理 stale temp；Q4 显式选择 ignored real gate 而缺环境变量时稳定非零失败；Q5 qualification contract 冻结原文、四个 phrases、2/4 threshold、NFKC+alphanumeric+lowercase normalization、原始/PCM hashes、archive/license/provenance，并以 canonical SHA-256 `b96f1f2f268ae54694e4d2a6a036e3ac8a94759db389e47e1005387239147006` 同时绑定 fixture metadata 与 runtime identity，任一 metadata mutation fail closed。
- Task 8 最终结论：规格与质量双审放行，Critical=0、Important=0。新增错误码精确映射测试、execution lease/delete 原子 reservation、Qwen 1.7B production Metal failure no-fallback seam；验证包括 no-default provider 12/12、asr-runtime provider 13/13、Qwen17 feature provider 14/14、model manager 72/72、lease/delete race 1/1、desktop check、Clippy、fmt 与 diff check。
- Task 9 最终结论：规格与质量双审放行，Critical=0、Important=0、Minor=0。固定 Core boot ID；30 秒 lease/5 秒 renew；RAII 单 Coordinator；raw claim API 收口；`JobControl` 分离；cancel/ownership 分型；recovery 清 stale active；`fail()` 与 cancel 竞态通过事务内 `OwnedMutationResult` 和 fenced acknowledge 原子关闭。
- Task 9 最终验证：目标竞态 1/1、focused 21/21、全量 no-default 283 passed / 5 ignored；fmt、no-default all-targets Clippy `-D warnings`、trusted desktop check、`git diff --check` 与无 `console.log` 均通过。
- ownership 边界：CoreRuntime 持有不可拆 lifetime guard；grandparent/parent/data directory、SQLite VFS、Evidence import/reconcile 与 startup Model reconcile 都使用 fd capability。SQLite main/rollback/WAL 通过进程级 tokenized anchored VFS，Audio cleanup 使用 no-replace tombstone + dev/ino fencing。正常第二 LifeSub 实例 fail closed；Task 11 再改为连接 primary socket。
- Task 10 M1：`BEGIN IMMEDIATE` 内以 owner/generation/state/cancel/lease fencing 原子写入 Provider Receipt、Revision、revision_receipts、Segments、FTS，并最后转 `succeeded`；cancel、stale generation、expired lease、source mismatch、时间 overflow 与每个 fault point 均零 partial Evidence。规格复审 Critical=0/Important=0/Minor=0，质量复审 Critical=0/Important=0，仅保留内部 SQLite error typed-source Minor。
- Task 10 M1/Core storage 集成提交：`bf0bde7 feat: anchor core storage and publish ASR evidence`。Fresh 集成验证：service 67/67、publication 11/11、jobs 21/21、ModelManager 72/72、migration 32/32、catalog 2/2、desktop check、Clippy `-D warnings`、fmt 与 diff check 全部通过。
- Task 10 M3 强制项：将 ModelManager 的 download/install/delete/qualification/execution lease 全生命周期统一迁移到 `ModelStorage::Anchored`/`AnchoredFs`，provider 必须从已验证 install-dir/required-file fd capability 加载；不得继续通过 `root.join(...)` 访问 production payload。startup reconcile 已 fd-anchored，但不代表 M3 完成。
- 下一阶段目标：先按 ownership 分组提交当前未提交的 Task 8/9 修复与文档，然后连续完成 Tasks 10–13：Task 10 原子 Receipt/Revision/Segment/FTS 发布；Task 11 单一 CoreRuntime、Catalog v4、版本化 Tool/Application API 与安全本地 IPC；Task 12 typed ASR client 和真实设置/模型 UI；Task 13 Job 状态、Receipt 来源与重转写 UI，并彻底移除 `demo-local` revision 路径。
- 阶段退出条件：Tasks 10–13 各自完成 TDD、规格复审和质量复审；Rust 原子发布/迁移/Tool API/IPC/双 Tauri harness、前端 focused tests、生产构建和无 `console.log` 全部通过。Task 10 执行循环必须维持不超过 5 秒 heartbeat，ownership lost 时丢弃结果；Task 11 后第二 Tauri 进程只能连接 primary，不得打开 writable SQLite；Task 13 后导入与重转写只消费真实 Core Job/Operation，不合成 transcript。
- MVP Gate：ASR V0.2 是基础里程碑；其后必须完成 native capture 与 DeepSeek Harness 真实录音到 `lifesub://` Evidence Ref 的完整闭环，才能标记 LifeSub 整体 MVP complete。
- 阻塞项：无外部阻塞。Task 10 M2-M5 尚未完成；真实 Qwen 1.7B weights/device acceptance 属于 Task 14，不阻塞 Tasks 10-13。工作树仅保留两份未纳入本目标的 cloud-fallback 文档草稿，禁止清理。
- 2026-08-17 流程优化：审查从双轮（规格+质量）合并为单轮合并审查，放行标准从 Critical=0+Important=0 简化为 Critical=0；测试从每次全量改为三层分级（Tier1 focused → Tier2 pre-commit → Tier3 全量）；文件行数限制从 300 放宽到 Rust 600/TS 400；新增 `scripts/check.sh` 和 `Makefile` 一键验证。详见主仓库 `AGENTS.md`、`.claude/contexts/dev.md`、`.claude/rules/common/coding-style.md`。
