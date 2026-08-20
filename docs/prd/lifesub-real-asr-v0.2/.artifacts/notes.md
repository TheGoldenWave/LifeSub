# LifeSub 真实本地 ASR V0.2 设计记录

- 2026-08-20：遗留生产能力正式拆为 V0.2.1 真实本地 ASR、V0.2.2 原生 macOS 采集、V0.3 匿名 Diarization、V0.3.1 CAM++ 声纹身份。排序采用“先导入音频真实 ASR，再接真实采集”，用于隔离 ASR 与采集故障域；模型安装 UI 是 V0.2.1 可用闭环的一部分，不单独作为版本。多设备合并从 V0.3 释放并待重新排期。

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
- 2026-08-20：补充存储策略。全天录制存储增长风险确认，架构设计新增 §11 音频编码优化（Opus 16kbps + DTX 可降至 ~7 MB/h）和 §12 存储保留策略（配额上限、分级保留 L1-L4、冷归档、存储仪表盘）。版本路线已更新。重要标记两个时间点：录制开始时影响编码质量（HQ 档），录制后只影响保留策略。
- 2026-08-20：ASR 兼容性确认。SenseVoice 与 Whisper 均以 16kHz 单声道为原生输入（sherpa-onnx `sampling_rate=16000`、Whisper 特征提取器重采样到 16kHz），故 16kHz 不是降级而是最优。Opus 压缩影响经 Amazon Science Interspeech 2021 论文验证：32kbps 相对 WER 退化 <1%，16kbps 退化 <3-5%（近讲不可感知），远优于同码率 MP3/AAC。§11.2 已补充 ASR 兼容性分析表。
- 2026-08-20：Qwen3-ASR 评估。模型同样以 16kHz 单声道为原生输入（Qwen3ASRFeatureExtractor `sampling_rate=16000`），架构为 Whisper 风格 Encoder + Qwen3 LLM Decoder，0.6B/1.7B 两个尺寸，Apache 2.0 许可。精度优于 Whisper Large v3，但模型体积大（0.6B ONNX ~1.2GB），不适合默认 Provider。已列入 V0.5 评估候选。与现有 16kHz Opus 编码策略完全兼容。
- 2026-08-20：简化为单一编码配置。三模型均确认 16kHz 为原生最优输入，16kbps Opus 对 ASR 无实质影响，去掉 Voice HQ 24kHz 档和"录制开始时标记重要→HQ 编码"逻辑。统一为 Opus 16kbps VBR 16kHz Mono + DTX，每小时 ~7MB。重要标记只影响保留策略（永久），不影响编码。
- 2026-08-19：安装包实机走查确认 `src-tauri/src/capture/streaming.rs` 的生产启动路径固定实例化 `MockStreamingSource`，前端却显示“实时 SenseVoice”，属于能力与来源误报，必须在真实 Capture Adapter/Provider 接通前显式标记演示模式并禁止生成“已保存”证据提示。
- 2026-08-19：当前 UI 存在多处可点击但无处理逻辑的控件：音频播放、模型下载、声纹注册/重命名/删除、词条编辑；时间线导入仅弹提示，统计与记录固定使用 demo 数据。
- 2026-08-19：设置弹窗 `.modal-body` 与 `.settings-layout` 同时定义双列 Grid，但后者作为前者唯一子元素被放进 180px 首列，导致设置正文压缩；Modal 也未管理初始焦点、焦点循环和关闭后的焦点恢复。
- 2026-08-19：已将实机功能审计与 5 张截图的 UI 观察合并为 `ui-walkthrough-issues-2026-08-19.md`。后续修复应以该文件为统一问题列表，优先解决 Mock 冒充真实能力、虚假保存成功、真实数据加载、音频播放、修订持久化与设置弹窗阻断问题。
