# LifeSub 真实本地 ASR V0.2 设计记录

- 2026-08-15：用户未明确阿里云或本地模型；依据项目 D-003 本地优先决策，本版选择 SenseVoiceSmall/FunASR 本地路径，DashScope 延后。
- 2026-08-15：sherpa-onnx 1.13.5 的 Rust API 已同时提供 SenseVoice 与 Whisper 离线识别示例，并支持静态链接。
- 2026-08-15：SenseVoiceSmall INT8 官方 sherpa-onnx 模型包约 163 MB；Whisper Tiny/Base/Small 分别约 116/208/639 MB。
- 2026-08-15：SenseVoice 与 Whisper 上游许可证均为 MIT；sherpa-onnx 为 Apache-2.0。
- 2026-08-15：TypeWhisper 为 GPLv3，只进行 clean-room 产品与架构借鉴，不复制其实现。
- 2026-08-15：为保证两个 Provider 具有统一时间范围，设计使用 VAD 语音区间作为 Segment 边界，而不依赖每个模型都提供同等时间戳能力。
- 2026-08-15：模型缺失或失败不得静默回退，否则 Provider Receipt 与用户选择不一致。
- 2026-08-15：规格首审指出旧 Catalog 无版本迁移、Chunk 来源关系不足、Job 恢复状态矛盾、模型安装事务不完整和验收阈值不足；设计已增加 v2 DDL、legacy 策略、Chunk/Receipt/Revision 关系、CAS lease、安装 reconciliation、量化 fixture Gate 和静态链接验证。
- 2026-08-15：规格二审要求继续明确 Chunk integrity 状态、start_ms 兼容语义、boot ID lease 恢复、VAD artifact provenance、model_downloads Schema 和 CER/WER 计算协议；均已纳入第二轮设计。
- 2026-08-15：规格三审发现 claim 与 `preparing` 状态之间存在崩溃窗口，且 boot ID 恢复隐含单实例假设；设计改为持有进程级 `asr-worker.lock`，并在同一 CAS 中完成 claim、attempt 递增和状态切换。
- 2026-08-15：规格四审发现 lease 过期 Worker 可能在被接管后发布结果；设计增加 claim_generation fencing，所有续租、状态转换和 Evidence 成功事务必须校验 claimed_by 与 generation。
- 2026-08-15：实施计划首审发现 `cargo test --exact` 可能零测试通过、Playwright 无法证明 native ASR、native runtime archive 未校验和 evidence commit 自失效等问题；计划已增加完整测试路径、verified archive、production desktop acceptance harness、单一 real-model Gate、固定 digest scope 和先提交代码后生成 evidence 的闭环。
