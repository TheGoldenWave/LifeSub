# LifeSub ASR 开源项目对比与范围审查

> 生成日期：2026-08-16
> 状态：综合分析；区分已实现能力、已批准设计与建议延期项

## 结论

LifeSub 与 OpenWhispr、TypeWhisper 的根本区别，是它不把 ASR 结果只视为文本，而是视为可定位、可追溯、可重新验证的音频 Evidence。当前设计在证据链、模型身份、失败恢复和 Agent 引用方面明显更强，但 V0.2 同时承担 ASR 产品、模型供应链、双运行时、daemon/IPC、安全协议、数据库演进、打包和 native capture，整体范围过重。

建议保留不可变音频、Revision/Receipt、无静默 fallback、模型 hash、任务状态机和真实模型 Gate；把 Qwen 1.7B 高级供应链、完整 lifesubd/IPC 安全体系和 production peer-auth 拆成后续里程碑。

## 开源项目对比

| 维度 | OpenWhispr | TypeWhisper | LifeSub |
|---|---|---|---|
| 产品重心 | 通用听写、Provider 路由 | macOS 文件转写、插件工作流 | 音频 Evidence、检索、重转写与 Agent 引用 |
| 模型体系 | 本地 Whisper 与云端 Provider | 引擎与模型分离 | SenseVoice、Whisper、Qwen3-ASR 0.6B/1.7B |
| 运行时 | Electron/服务式集成 | Swift/macOS 原生 | Rust Core；sherpa-onnx + Candle/Metal |
| 结果模型 | 以转写文本为主 | 以文件任务为主 | 不可变 Revision、Segment、Receipt、输入 hash |
| 模型切换 | 路由和选择体验成熟 | 模型状态管理完整 | 每次重转写追加 Revision，不覆盖历史 |
| 可靠性 | 偏产品工作流 | 有任务生命周期 | 原子音频导入、恢复、fencing、单 Core owner |
| 可追溯性 | 一般 | 一般 | 固定模型、文件 hash、运行时、参数、VAD 和 provenance |
| Agent 集成 | 通用 Agent 功能 | 插件体系 | 版本化 Tool API 与 `lifesub://` Evidence Ref |
| 许可策略 | MIT，可参考实现 | GPL-3.0，只做 clean-room 借鉴 | 自有实现，依赖与模型 notice 固定 |

## LifeSub 的差异化优势

### 1. Evidence-first，而不是 transcript-first

每次成功结果关联不可变 Audio Chunk、Provider、模型、参数、运行时、时间范围和输入 hash。重新转写产生新 Revision，旧结果继续可读。这比常见桌面听写工具更适合个人知识、审计和 Agent 回答溯源。

### 2. 多模型不是简单下拉框

SenseVoice、Whisper 和 Qwen 覆盖不同语言、资源和质量档。Provider Factory 按 manifest 和 runtime identity 精确分派，1.7B 不得静默回退到 0.6B、Whisper、CPU 或其他运行时。

### 3. 本地 Rust Core 的部署边界更干净

主路径不依赖 Python Sidecar、不启动本地 HTTP 服务、不把音频发送到云端。SenseVoice、Whisper、Qwen 0.6B 使用静态 sherpa-onnx；Qwen 1.7B 使用 Candle/Metal。

### 4. 可靠性与供应链证据更强

已完成基础包括 crash-safe 音频导入、symlink/目录替换防护、reconciliation、unknown integrity fail-closed 和单 Core ownership。设计还要求模型 immutable revision、逐文件大小/hash、许可证、来源与 runtime qualification。

### 5. 跨模型时间轴和 Agent 引用统一

VAD 统一不同 Provider 的 Segment 时间语义。搜索结果携带 revision、chunk、精确时间范围与 `lifesub://` 引用，使 Agent 能说明结论来自哪段录音。

## 当前设计过重的部分

### P0：一个版本承载了过多产品目标

V0.2 同时包含真实 ASR、六个模型、模型下载器、Qwen 双运行时、任务系统、Revision/Receipt、前端设置、Local Tool API、双契约、Host Control、UDS、code-sign peer auth、Catalog v2-v4、真实模型 Gate、桌面/DMG 和 native capture。任何一项都可以独立成为里程碑，组合后会显著拉长反馈周期。

### P1：Qwen 1.7B 的首发成本偏高

1.7B 引入约 4.71 GB 五文件 bundle、第二套 Candle/Metal runtime、多阶段安装状态、RuntimeQualifier、M4/24GB Gate 和额外打包验证。它能形成高质量档优势，但不应阻塞 SenseVoice、Whisper 和 Qwen 0.6B 的首个可用版本。

### P1：Agent/daemon 安全体系早于核心 ASR 价值验证

Agent Contract、Application Contract、Host Event/Control、UDS framing、peer code-sign verification、outbox/idempotency 和 lifesubd 演进都合理，但并非完成“导入音频并得到可信转写”的必要条件。过早实现会让 ASR 主链路被基础设施工作淹没。

### P1：供应链规范部分达到发布平台级别

逐文件 provenance、redirect allowlist、RFC 8785 JCS bundle identity、传递依赖 notice closure 很严谨。首版必须保留 immutable revision、size/hash、required files 和 license；JCS、完整 redirect 策略与自动 notice reconciliation 可以在模型安装链路跑通后补强。

### P2：本地文件系统威胁模型曾过度扩大

防止协作式第二实例并发迁移、清理和写入是必要的；试图防御同 UID 恶意进程替换任意祖先目录则没有有限锚点，而且同 UID 主体本已能直接删除 SQLite 和音频。应明确威胁边界，避免继续增加路径锁复杂度。

### P2：验收矩阵过宽

每个模型、每个 runtime、真实 fixture、性能、桌面、DMG、双进程和崩溃恢复全部同时作为单版本 Gate，会使任何局部改动触发昂贵全量验证。应建立分层 Gate：快速合同测试、单模型真实 Gate、发布候选全量 Gate。

## 建议的分期方案

### V0.2 Core ASR

- SenseVoice + Whisper，最多附带 Qwen 0.6B。
- 基础 immutable manifest：revision、size、SHA-256、required files、license。
- 音频导入、设置持久化、任务状态、Receipt、Revision、重转写。
- 一个中文和一个英文/混合真实 fixture Gate。
- 桌面开发包 smoke；DMG 作为发布候选 Gate。

### V0.2.1 High-quality Qwen

- Qwen 1.7B 五文件 bundle。
- Candle/Metal runtime、RuntimeQualifier、24GB 设备 Gate。
- 多文件断点下载和完整供应链 provenance。

### V0.3 Core Service And Agent API

- Core Application/Agent contracts。
- UDS、secondary Tauri、idempotency/outbox、Host Control。
- contract-first `lifesubd` 演进。

### V0.4 Native Capture MVP

- ScreenCaptureKit + AVAudioEngine。
- DeepSeek Harness 从真实录音到 `lifesub://` Evidence Ref 的完整闭环。
- production peer-auth、release `.app` 与 DMG 全量 Gate。

如果业务目标要求 native capture 尽快验证，V0.3 与 V0.4 的顺序可以调整：先用最小内部 API 完成 capture，再硬化公开 Agent/daemon 合同。

## 不应裁掉的底线

- 原始音频先可靠持久化，ASR 失败不得损坏源文件。
- 不静默切换 Provider、模型或 runtime。
- Revision append-only，历史结果不可覆盖。
- Receipt 至少保存模型、参数、输入 hash 和时间范围。
- 模型必须固定 immutable source、size 和 SHA-256。
- Job 成功发布必须是原子事务，不能出现半条 Evidence。
- 至少一个真实模型 fixture 和最终发布包 smoke。

## 当前状态说明

截至 2026-08-16，Tasks 1-4 的 runtime、Catalog v2、ASR domain/settings 和 crash-safe audio 已完成并通过审查；Qwen 1.7B、Local Tool 架构已完成设计复审，Task 5 manifest 正在实现。本文中的后续模型管理、UI、IPC、DMG 与 native capture 多数仍是已批准设计，而不是已交付功能。

## 来源引用

- [真实 ASR 设计](/Users/goldenwave/.config/superpowers/worktrees/LifeSub/lifesub-real-asr-v0.2/docs/superpowers/specs/2026-08-15-lifesub-real-asr-design.md) — Provider、参考项目、模型、可靠性与 Gate。
- [实施计划](/Users/goldenwave/.config/superpowers/worktrees/LifeSub/lifesub-real-asr-v0.2/docs/superpowers/plans/2026-08-15-lifesub-real-asr-v0.2.md) — Tasks 1-15、文件和验收范围。
- [当前进度](/Users/goldenwave/.config/superpowers/worktrees/LifeSub/lifesub-real-asr-v0.2/docs/prd/lifesub-real-asr-v0.2/.artifacts/process.md) — 已实现和待实现状态。
- [产品发现记录](../../product-initiated/lifesub-real-asr-v0.2/00_discovery/original-idea-20260815.md) — OpenWhispr、TypeWhisper 参考边界。
