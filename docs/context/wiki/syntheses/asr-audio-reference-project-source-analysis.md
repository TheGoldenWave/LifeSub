# LifeSub ASR 与音频录制参考项目源码分析

> 生成日期：2026-08-26
> 类型：综合技术分析
> 范围：本地 ASR、模型管理、麦克风与系统音频采集、设备恢复、AEC、任务状态和发布边界
> 结论口径：区分已经借鉴、建议继续借鉴、明确不应照搬

> 对应实施计划：[`2026-08-26-lifesub-asr-audio-settings-reference-optimization.md`](../../../superpowers/plans/2026-08-26-lifesub-asr-audio-settings-reference-optimization.md)

## 1. 结论摘要

后续产品校对见 [会议 ASR 与 Qwen-Audio-Agent 分析](qwen-audio-product-direction-review.md)（2026-09-11）。本文保留参考源码的历史观察；已收口能力与后续版本顺序以最新过程记录和总路线图为准。

2026-10-13 新增的 [VoiceStudio 固定提交源码对照](voicestudio-source-review-2026-10-13.md) 是高优先级补充参考，重点是有界诊断、ASR 故障隔离、只读音频信号检查和逐资产许可证据。下列“五层”保留 8 月 26 日当时的分析口径，不表示排除后续参考；VoiceStudio 的研究项不扩大 0.2.2 S1–S4。

LifeSub 当前最重要的参考项目不是一个，而是五层：

1. **OpenWhispr**：产品与跨平台录音工作流参考，尤其是 Provider 路由、模型下载体验、麦克风健康检查、会议双源录音和 WebRTC AEC。
2. **TypeWhisper**：macOS 原生音频工程参考，尤其是 Core Audio 稳定 UID、设备兼容性探测、蓝牙路由稳定、`AVAudioEngine` 配置变化恢复和批量任务状态。
3. **sherpa-onnx 1.13.5**：LifeSub 实际使用的 ASR/VAD 运行时上游，不只是“参考”。SenseVoice、Whisper、Qwen3-ASR 0.6B 和 Silero VAD 的运行 API、线程和模型配置应继续以该版本源码为准。
4. **qwen3-asr-rs 0.2.2**：Qwen3-ASR 1.7B 的实际 Candle/Metal 运行时上游，适合作为高质量档的独立运行时参考，不应反向污染当前 sherpa 主路径。
5. **VibeVoice**：长音频与流式 ASR 研究参考，重点关注说话人、时间戳、正文联合输出、用户热词、流式推理和微调流程；当前仅登记为候选参考，不代表已完成质量、资源占用或生产接入评估。

源码对比后的核心判断：

- LifeSub 在 **Evidence、模型身份、输入 hash、Job fencing、原子 Revision/Receipt 发布、双源身份** 上已经明显强于两个产品参考项目，不应为了“参考成熟项目”而弱化这些边界。
- 当前最值得借鉴的不是另写一套 ASR，而是补齐 **音频引擎真实恢复、设备兼容性探测、蓝牙/外接设备稳定性、模型下载进度 UI、AEC 与双源文本去重**。
- OpenWhispr 的自动回退系统默认麦克风与 LifeSub 已确定的来源真实性原则冲突，**不能照搬**。LifeSub 应借鉴它的健康检查和 generation fencing，但选中设备不可用时必须明确失败。
- TypeWhisper 为 GPL-3.0/商业双许可源码。除非项目明确接受 GPL 传播义务或取得商业授权，只能做 clean-room 行为与架构借鉴，不能复制源码、测试、文件结构或文案。

## 1.1 LifeSub V0.2.1 已完成能力对照复核矩阵

本矩阵以当前 production wiring、`docs/workspace-status.md` 和 `docs/prd/lifesub-real-asr-v0.2/.artifacts/process.md` 的真实候选证据为准。参考项目只提供行为检查点，不替代 LifeSub 验收。

| 已完成能力 | 当前证据与判断 | 参考项目细节 | 处置 | 当前版归属 |
|---|---|---|---|---|
| 单一认证 Swift helper | inherited-FD nonce、UID/PID/path/file identity、严格签名与 launch-time 文件身份、全路径 shutdown/reap 已有自动化与 packaged 证据 | OpenWhispr helper supervision | `keep`：LifeSub 身份边界更强 | Task 8 只复验最终 bundle identity |
| 精确麦克风 UID 选择 | 枚举、持久化、bind/readback、`source_started`/Catalog identity、无默认设备 fallback 已完成；断连明确失败 | TypeWhisper stable UID；OpenWhispr active-device tracking | `keep`：保留 fail-closed；拒绝自动回退 | Task 7 复验内置与外接/蓝牙绑定 |
| 麦克风真实启动首帧锚点 | production start 已等待首个成功转换并提交的非空 PCM frame，超时清理 engine/tap；解决 host clock 域错误 | OpenWhispr mic health；TypeWhisper route readiness | `keep`，但它属于持久录制启动，不等同无副作用兼容性测试 | Task 3 增加独立 probe，不重复 binder |
| AVAudioEngine format settle 与连续重采样 | 已修复 48k/44.1k tap mismatch、converter priming 丢帧和采样率切换状态；真实 30 秒 chunk 漂移约 0 ms | TypeWhisper 蓝牙、多声道与 format readiness | `keep`；外接/蓝牙真实矩阵仍缺 | Task 3、7 |
| 配置变化处理 | 同 UID 仍 alive/bound 时仅发送 `recoverable=true`；Rust 保持 Pending；设备消失明确失败 | TypeWhisper recovery state machine；OpenWhispr generation fencing | `current-release-hardening`：当前未证明重建 engine 后继续产帧 | Task 4 |
| WAV staging、rotation 与原子 sealing | helper 生成 canonical WAV partial；Rust 严格 grammar、seal intent、fsync/rename、Catalog transaction 与重启 reconciliation 已完成 | TypeWhisper recovery recordings | `keep`：LifeSub Evidence/文件边界更强 | Task 7、8 复验长时与重启 |
| 原生 chunk 自动进入 ASR Job | seal 成功后 canonical enqueue；失败不提前创建伪 publication | TypeWhisper file/batch lifecycle | `keep` | Task 8 packaged end-to-end |
| 精确模型/runtime 执行 | exact manifest/bundle/runtime qualification、anchored audio read、Provider snapshot、cancel/lease renewal 已完成 | OpenWhispr provider routing；TypeWhisper engine/model readiness | `keep`：LifeSub identity/provenance 更强 | Task 8 known fixtures |
| 静音音频语义 | VAD 0 utterance 按成功处理，持久化 Receipt/Job success，不创建空 Revision 覆盖 Timeline | 参考项目通常只投影空结果 | `keep`：LifeSub committed Evidence 语义更严格 | Task 8 回归 |
| Receipt/Revision/Segment 发布 | fenced transaction、chunk-bound Segment、Provider/VAD provenance、append-only Revision、durable replay 已完成 | 参考项目主要保存文本/任务结果 | `keep`：禁止简化或覆盖原文 | Task 8 身份复验 |
| committed-only 录音 UI | 只显示 Catalog 已提交结果；durable failure/restart replay 已完成 | OpenWhispr/TypeWhisper 提供较早 preview | `keep`：当前版不引入 preview | Task 15 后续独立实现 |
| 录音设置 | `sample_rate`、IM 检测/延迟和 `storage_path` 仍可编辑，但没有生产消费者；实际输出固定 16 kHz，数据目录由 Core 锚定 | OpenWhispr/TypeWhisper 隐藏不可用能力 | `current-release-hardening`：用户可被误导 | Task 1 |
| ASR 设置投影 | Provider/model校验存在；UI 语言静态，ITN 对所有 Provider 可见，Whisper task 未持久投影 | OpenWhispr capability routing；TypeWhisper configured/ready distinction | `current-release-hardening` | Task 1 |
| 权限/运行时/设备诊断 | 稳定错误和底层事实存在，但缺统一、可刷新、可脱敏导出的用户入口 | TypeWhisper diagnostics；OpenWhispr privacy/system sections | `current-release-hardening` | Task 2 |
| 模型安装体验 | Catalog checkpoint 与 qualification 事实存在；UI 仅展示“安装中”与最终错误，缺阶段、字节、取消、重试、重启恢复 | OpenWhispr model progress；TypeWhisper restorable states | `current-release-hardening` | Task 5 |
| AEC、跨源去重、preview、normalization、batch decode | 当前版本没有这些派生产品或实验优化 | 参考项目/运行时提供可借鉴机制 | `post-release`：不阻塞 V0.2.1 | Task 12-16 |
| 自动切默认麦克风、修改系统默认输入、云 Provider | 与显式 Evidence 来源、本地发布目标或副作用边界冲突 | OpenWhispr/TypeWhisper 的部分便利行为 | `reject` | 不进入计划 |

当前版准入结论：只接纳设置真实性、独立首帧 probe、真正的同 UID recovery、统一诊断、持久模型操作和最终 packaged 验收。其他参考细节继续留在后续优化版本。

## 2. 参考项目清单与可信度

| 项目/上游 | 本地源码证据 | 身份与许可 | 对 LifeSub 的角色 | 可借鉴强度 |
|---|---|---|---|---|
| OpenWhispr | `/Users/goldenwave/Downloads/openwhispr-main` | 当前下载快照声明 `1.8.3`、MIT；目录无 `.git`。LifeSub 历史设计审阅基线为 `b3a8368...` | 产品工作流、模型 UI、跨平台录音、会议 AEC | 高，可读实现并在保留 notice 后复用 MIT 思路；仍建议独立实现 |
| TypeWhisper | `/Users/goldenwave/Downloads/typewhisper-mac-main` | 当前工程 `MARKETING_VERSION=1.6.0`、GPL-3.0/商业许可；目录无 `.git`。历史设计基线为 `ea79202...` | macOS 原生录音、设备与恢复、任务生命周期 | 高，但必须 clean-room |
| VibeVoice | `.sources/reference-projects/VibeVoice-main` | 当前本地归档快照 `pyproject.toml` 声明 `1.0.0`、根目录 MIT；目录无 `.git`。模型、权重、数据和其他资产仍需分别核对许可 | 60 分钟长音频 ASR、流式识别、说话人/时间戳/正文联合输出、热词、推理与微调流程 | 中，先做资源、质量、模型许可和本地部署评估；不得直接视为当前生产 Provider |
| sherpa-onnx | Cargo registry `sherpa-onnx-1.13.5` | Apache-2.0；项目固定 commit `3dc7c569...` 与原生归档 hash | 当前生产 ASR/VAD 运行时 | 最高，属于直接依赖上游 |
| qwen3-asr-rs | Cargo Git checkout `c5ef096...` | crate `0.2.2`、MIT、Candle 0.9.2 + Metal | Qwen3-ASR 1.7B 独立运行时 | 中高，仅用于对应模型档 |
| SenseVoice/FunASR | 模型 manifest 和 sherpa 转换资产 | 模型/转换资产按 manifest 固定来源与许可 | 中文/中英混合模型上游 | 中，参考模型语义，不引入 Python 服务架构 |
| OpenAI Whisper | sherpa Whisper 模型资产 | 模型与转换资产按 manifest 固定 | 英文与通用多语种模型上游 | 中，参考模型参数与质量基准 |
| Silero VAD | sherpa VAD 模型与源码默认值 | 由 LifeSub manifest 固定 | 当前切分与时间轴基础 | 高，继续作为统一 VAD 基线 |
| Apple ScreenCaptureKit / AVAudioEngine / Core Audio | macOS SDK 和 LifeSub/TypeWhisper 源码 | Apple 平台 API | 原生采集平台基线 | 最高，但不是可复制的第三方项目 |

### 源码快照限制

OpenWhispr、TypeWhisper 和 VibeVoice 的本地快照目录没有 `.git`，不能从本机源码证明其精确 commit。本文对其当前实现的判断来自文件内容、包版本和工程版本；历史 commit 只引用 LifeSub 已有设计文档中的审阅记录。若未来要把某段实现作为许可证或安全审计依据，应重新以带 Git 元数据的固定 commit 获取源码。

## 3. LifeSub 当前实现基线

### 3.1 已经做得更强的部分

LifeSub 的 Provider 请求绑定 Provider、模型 ID、manifest version、bundle identity、运行时、设备和参数；sherpa 与 Qwen Candle/Metal 是不可互换的运行时身份。见 `src-tauri/src/asr/provider.rs`。

真实执行路径在准备阶段校验模型 lease、输入文件与 VAD snapshot，执行后构造带输入 hash、运行时 build ID、模型来源和参数的 Receipt，再由 Worker 使用 claim generation/fencing 原子发布 Revision 与 Segment。见：

- `src-tauri/src/asr/native_engine.rs`
- `src-tauri/src/asr/worker.rs`
- `src-tauri/src/asr/job.rs`

模型安装不是简单的“文件存在即就绪”：manifest 固定来源、revision、大小、SHA-256、required path、许可证和 qualification policy；下载 checkpoint 记录 `downloaded_bytes/expected_bytes`，安装和恢复路径均 fail closed。见：

- `src-tauri/src/asr/model_manager/types.rs`
- `src-tauri/src/asr/model_manager/download.rs`
- `src-tauri/src/asr/model_manager/install.rs`

原生采集把麦克风与系统音频作为独立 capture source，维护 sample range、continuous host time、chunk sequence/hash，并在 `source_started` UID 与用户请求 UID 不一致时先拒绝、后写 Catalog。见：

- `src-tauri/native/capture-helper/Sources/CaptureHelper/MicrophoneCapture.swift`
- `src-tauri/native/capture-helper/Sources/CaptureHelper/SystemAudioCapture.swift`
- `src-tauri/src/capture/session.rs`
- `src-tauri/src/capture/committer.rs`

这些 Evidence 边界不应被参考项目中更宽松的“拿到文本即可成功”逻辑替换。

### 3.2 当前主要缺口

1. **可恢复事件没有完成真实恢复。** `MicrophoneCapture` 在同 UID 配置变化时只发出 `device_configuration_changed/recoverable=true`；`CaptureSessionController` 只封存当前 chunk 并继续保持 running；Rust coordinator 也只是忽略该事件。现有代码没有证明引擎配置变化后重新安装 tap、重新绑定设备、重新启动并继续产帧。
2. **设备“可枚举”不等于“可录”。** 当前可精确 UID 绑定和读回，但缺少 TypeWhisper 那种实际启动探测、输入格式/路由兼容性缓存和用户可见诊断。
3. **模型下载后端进度丰富，前端投影过弱。** Catalog 已有字节级 checkpoint、状态、重试和取消能力，当前 `ModelManager` DTO/UI 仍主要显示笼统状态。
4. **双源会产生声学重复。** 系统音频可能再次被扬声器和麦克风收录。LifeSub 保存了两条可信原始 source，但尚未有 AEC derivative 或跨 source transcript 去重/融合策略。
5. **实时页面实际是 committed-only。** 这保证了真实性，但首个 chunk/Job 完成前没有低延迟预览；后续若增加预览，必须与 committed Evidence 明确分层。

## 4. OpenWhispr 源码中值得借鉴的部分

### 4.1 纯路由结果，不携带密钥

`transcriptionRoute.ts` 把选择解析为 `local/proxied/http-batch/error`，并明确要求路由只携带 `keyRef`，执行器再解析密钥。它还对不安全 URL、错误 Provider/模型组合和策略绕过 fail closed。

LifeSub 已经通过 execution snapshot 和 Provider Factory 实现了更严格的本地版本。可以继续借鉴的是：

- 把 UI 的 Provider/模型/语言组合错误在启动 Job 前解释成稳定、可操作的原因；
- 让导入、重试、重转写和实时采集共享同一配置解析结果；
- UI 不接触模型路径、运行时路径或任何密钥。

不建议引入 OpenWhispr 的云端 Provider 和 HTTP route 到当前本地 ASR 发布闭环。

### 4.2 模型卡、下载进度和删除反馈

`LocalWhisperPicker.tsx` 把模型大小、推荐标记、是否下载、下载中、进度、取消和删除确认统一到一张模型卡中。LifeSub 后端已经具备更强的 durable checkpoint，因此这里是最容易直接转化为产品价值的一项参考。

建议 LifeSub 增加：

- `download_id`、artifact 数、已下载/总字节、百分比；
- `downloading/verifying/installing/qualifying` 分阶段状态；
- 刷新或应用重启后从 Catalog 恢复进度；
- 取消、同一 download ID 断点重试和稳定错误详情；
- 安装完成后显示 runtime qualification，而不是仅显示“文件已下载”。

### 4.3 麦克风健康检查与 generation fencing

`activeMicRecovery.js` 同时观察 `devicechange`、track `ended/mute/unmute`，用 debounce、generation 和单一 in-flight recovery 防止 stop/start 竞态。`micTrackHealth.js` 还验证 getUserMedia 返回的 track 是否真正解除 mute 并开始工作。

LifeSub 应借鉴：

- 不能只验证 Audio Unit 当前设备 UID，还要在启动 deadline 内验证真实 PCM 帧到达；
- stop/start、设备变化和延迟回调均使用 generation fencing；
- 同一恢复只允许一个 in-flight attempt；
- 监听器、timer 和旧 stream 在所有退出路径清理。

但 OpenWhispr 会在设备静音/失效时尝试系统默认麦克风。LifeSub 已明确要求不静默换源，因此只应借鉴健康检查与并发结构，失败结果必须是 `selected_microphone_unavailable/disconnected`。

### 4.4 WebRTC AEC helper

OpenWhispr 的 `meeting-aec-helper` 接收系统参考流与麦克风流，用 WebRTC Audio Processing Module 的 `ProcessReverseStream` 和 `ProcessStream` 执行 echo cancellation，同时启用 high-pass 与 noise suppression。Node 管理层用长度前缀帧、启动/停止 timeout 和子进程错误回调监督 helper。

LifeSub 可以借鉴，但建议采用不同的数据产品定义：

- 原始 microphone/system_audio chunk 继续分别持久化，绝不被 AEC 结果覆盖；
- AEC 输出作为带 parent source/chunk、算法版本、参数和 hash 的 derivative；
- ASR 可选择 raw mic 或 AEC mic，Receipt 必须记录选择；
- AEC 失败不能伪装成成功，也不能让系统音频证据消失；
- 在引入 WebRTC APM 前先用双源 fixture 测量重复率和实际收益。

## 5. TypeWhisper 源码中值得借鉴的部分

### 5.1 引擎、模型、认证和可恢复状态分离

`ModelManagerService.swift` 不把 `isConfigured` 简化为单一布尔值，而是区分 engine 是否存在、认证是否可用、模型是否已配置、是否能从持久状态恢复、是否有可下载 catalog。`FileTranscriptionViewModel.swift` 则把单文件与 batch 状态分别建模，包含 loading、transcribing、done、error 和 cancelled。

LifeSub 的 durable Job/Catalog 比这更强，但 UI 投影仍可以借鉴：

- 模型不可用时区分“未安装、安装未完成、已安装未 qualification、设备不支持、运行时不匹配、文件损坏”；
- Job 显示 queued/preparing/transcribing/publishing，而不是只显示最终成功/失败；
- cancel 是已确认的状态转换，不是前端隐藏进度；
- 每次重试继续绑定原 Job generation、模型和输入身份。

### 5.2 设备兼容性探测和诊断报告

`AudioDeviceService.swift` 为设备保存 UID、名称、Core Audio device ID、transport、默认状态、输入/输出 channel、sample rate、aggregate/virtual 标记和兼容性结果；选择器能显示 unavailable、cannotSetDevice、invalidInputFormat、engineStartFailed 和 routingConflict。

这比 LifeSub 当前“枚举 + UID bind/readback”更接近真实用户问题。建议增加一个不持久化音频的短时 probe：

1. 精确 UID 解析与 alive/input channel 检查；
2. 绑定 input Audio Unit；
3. 安装 tap 并在有界 deadline 内等待有效、非空 PCM；
4. 停止并清理；
5. 缓存兼容性到当前设备快照，不把临时错误永久写死；
6. UI 展示“不兼容/蓝牙路由冲突/未收到音频”的具体原因。

### 5.3 `AVAudioEngine` 恢复状态机

`AudioEngineRecoverySupport.swift` 对配置变化做 150ms debounce、1s quiescence、5s burst window、最多 4 次配置变化和有界 backoff；恢复运行在专用串行队列，主线程只允许极短等待。它还用 generation 阻止过期恢复，超过 burst limit 后明确失败。

这是 LifeSub 当前最应优先借鉴的音频工程部分。建议 LifeSub 的 Swift helper 增加明确状态：

```text
running -> recovering -> rebind same UID -> reinstall tap -> first-frame proof -> running
                         \-> unavailable/mismatch/burst-limit -> failed
```

恢复前封存当前 chunk，恢复成功后创建连续 sequence 的新 chunk；如果实际设备 UID 变化，必须失败，不得当作恢复成功。真实恢复完成后再发送 `source_recovered` 或带 generation 的新事件，不能仅发送 `recoverable=true`。

### 5.4 蓝牙、外接多声道和生命周期细节

TypeWhisper 源码还体现了几类真实硬件经验：

- 蓝牙输入需要 route stabilization/readiness deadline；
- 多声道 USB 设备直接交给 `AVAudioConverter` 下混可能得到零数据，因此要验证 tap format 和真实样本；
- 异步 start 支持 cancellation，取消发生在 commit 前不得留下运行中的 engine；
- teardown 后短暂保留旧 `AVAudioEngine`，避免 Core Audio 内部回调访问已释放对象；
- 配置变化通知与恢复逻辑放在同一串行队列。

这些行为值得做成 LifeSub 的注入式 Swift tests 和真实硬件 matrix，但不能复制 GPL 实现代码。

## 6. sherpa-onnx 与 qwen3-asr-rs 的直接借鉴

### 6.1 sherpa-onnx

LifeSub 当前每个 inference window 创建 `OfflineStream`、`accept_waveform`、单独 `decode`。sherpa 1.13.5 同时提供 `decode_multiple_streams`，因此存在批量推理优化空间。

建议先 benchmark，再决定是否改：

- 同一 Job 内把若干 VAD window 组成有界 batch；
- batch 仍受最大样本数、内存、取消和 lease renewal 限制；
- 结果顺序必须与 Evidence window 一一对应；
- 任何一条结果缺失时整批 fail closed；
- 不要为了吞吐牺牲低延迟或把不同模型/Receipt 混到同一批。

Silero VAD 应继续使用固定版本和 manifest 参数。不要把 OpenWhispr 的 VAD 配置直接复制过来，因为 LifeSub 已用 VAD snapshot 参与 Receipt provenance。

### 6.2 qwen3-asr-rs

该 crate 提供 batch/streaming 方向和 Metal backend，但 LifeSub 已正确把它限定为 Qwen3-ASR 1.7B 的独立 runtime identity。建议：

- 继续独立 qualification、设备 Gate 和 Receipt；
- 不让 1.7B 阻塞 SenseVoice/Whisper 的当前发布闭环；
- 只有真实质量和 RTF 证明收益后，才把 streaming API 接到 UI；
- 不回退到 0.6B、sherpa 或 CPU，同时不把 Metal 失败解释为模型失败。

## 7. 分优先级建议

### P0：当前本地 ASR 与打包闭环前

#### P0-1 真正实现同 UID 音频引擎恢复

当前 `recoverable=true` 只表示“不立刻失败”，不证明采集恢复。实现同 UID rebind/restart/first-frame proof、generation fencing、burst limit 和稳定失败码，并加入配置变化后继续产生 chunk 的自动化与真实硬件证据。

#### P0-2 增加设备兼容性 probe 与首帧 deadline

开始录制前验证所选设备能产生有效 PCM。对蓝牙、USB 多声道、aggregate/virtual 分别记录兼容性和错误，不自动回退默认设备。

#### P0-3 完成模型下载进度 UI

直接投影现有 Catalog checkpoint：字节、百分比、阶段、取消、重试、重启恢复。该项无需重写 ModelManager，是低风险高收益工作。

#### P0-4 以 packaged app 验证真实模型和采集

参考项目的开发态成功不能替代 LifeSub 的签名 bundle 验收。必须验证 app/helper hash、模型/runtime identity、权限归属、真实 UID、chunk、ASR Job/Receipt/Revision 和安装后的启动路径。

### P1：质量与稳定性提升

#### P1-1 AEC derivative + 双源 transcript 去重

先测量扬声器回灌造成的重复文本，再引入 WebRTC APM derivative。保留 raw source，AEC 和融合结果都要有 provenance。文本层可按时间重叠、source 优先级和相似度消除重复，但不得删除原始 Revision。

#### P1-2 有界 batch decode

用 sherpa `decode_multiple_streams` 对短 VAD window 做受控 batch benchmark，比较 RTF、峰值 RSS、取消延迟和文本一致性；只有全指标更好才替换顺序 decode。

#### P1-3 区分预览与证据

若产品需要更实时的反馈，可增加明确标记的 ephemeral preview；最终 UI 仍只能用 committed Revision 覆盖或确认。预览文本不能进入搜索、导出或 Agent Evidence。

#### P1-4 词典进入可追溯后处理

借鉴 TypeWhisper 的 normalization/dictionary pipeline，但不要直接修改 Provider 原始文本。建议发布 `provider_original` Revision，再生成带规则版本、词典 snapshot 和 diff 的 `normalized` Revision。

### P2：后续版本

- Qwen3-ASR 1.7B streaming 与 warm model cache；先完成设备/RTF Gate。
- CAM++/speaker embedding 与 diarization；必须绑定 source、segment 和模型 identity。
- Opus/DTX 长时存储；保留可验证 hash、解码参数和迁移策略。
- 活跃录制麦克风热切换；必须创建新的 capture-source/Evidence 边界，不能改写旧 source 身份。

## 8. 明确不建议照搬的实现

| 参考实现 | 不建议照搬的原因 | LifeSub 应采用的方式 |
|---|---|---|
| OpenWhispr 自动回退系统默认麦克风 | 会改变证据来源，违反显式选择 | 同 UID 恢复；不可用则明确失败 |
| OpenWhispr Electron/Node 多 sidecar 主架构 | 增加进程、签名、监督和攻击面 | 保持 Rust Core + 单一认证 Swift helper |
| OpenWhispr 云端 Provider 路由 | 当前目标是本地 ASR 闭环 | 保留本地 Provider snapshot；云端另立范围 |
| TypeWhisper GPL 源码/测试/文件结构 | 可能触发 GPL 传播义务 | clean-room 行为规格 + 独立实现和测试 |
| TypeWhisper 改系统默认输入以激活路由的策略 | 可能影响用户其他应用，来源和副作用复杂 | 优先 Audio Unit 精确绑定；蓝牙例外需独立设计和显式提示 |
| 直接覆盖原 ASR 文本的 normalization | 丢失原始模型输出与可审计性 | 追加派生 Revision，记录规则和词典 snapshot |
| 为“实时”把未提交文本当 Evidence | WebView/进程崩溃会造成伪持久化 | ephemeral preview 与 committed Revision 分层 |

## 9. 推荐实施顺序

```text
1. 同 UID engine recovery + first-frame proof
2. 设备 compatibility probe + 蓝牙/USB 硬件矩阵
3. 模型下载进度/取消/重启恢复 UI
4. packaged SenseVoice/Whisper 真实闭环与 DMG
5. 双源重复率基线
6. AEC derivative 与 transcript dedup
7. sherpa batch decode benchmark
8. preview/normalization/diarization 等上层能力
```

这个顺序优先解决“录得到、来源真实、模型可用、包能运行”，再追求低延迟和高级质量能力。它也避免 Qwen 1.7B、AEC、speaker 和 streaming 同时扩大当前发布面的风险。

## 10. 来源引用

### LifeSub

- [`src-tauri/src/asr/provider.rs`](../../../../src-tauri/src/asr/provider.rs) — Provider/runtime/model identity 与 sherpa 执行边界。
- [`src-tauri/src/asr/native_engine.rs`](../../../../src-tauri/src/asr/native_engine.rs) — 输入、VAD、window、Receipt 和 Segment 构造。
- [`src-tauri/src/asr/model_manager/`](../../../../src-tauri/src/asr/model_manager/) — durable 下载、安装、恢复和 qualification。
- [`MicrophoneCapture.swift`](../../../../src-tauri/native/capture-helper/Sources/CaptureHelper/MicrophoneCapture.swift) — 精确 UID 绑定、监听和当前 recoverable 行为。
- [`CaptureSessionController.swift`](../../../../src-tauri/native/capture-helper/Sources/CaptureHelper/CaptureSessionController.swift) — 原生 source 状态机和 interruption 处理。
- [`SystemAudioCapture.swift`](../../../../src-tauri/native/capture-helper/Sources/CaptureHelper/SystemAudioCapture.swift) — ScreenCaptureKit 系统音频采集。
- [真实 ASR 设计](../../../superpowers/specs/2026-08-15-lifesub-real-asr-design.md) — 历史参考项目基线与许可边界。
- [当前进度](../../../prd/lifesub-real-asr-v0.2/.artifacts/process.md) — 已实现能力与尚缺发布证据。

### OpenWhispr 当前本地快照

- `/Users/goldenwave/Downloads/openwhispr-main/package.json` — 版本、依赖和 native helper 打包入口。
- `/Users/goldenwave/Downloads/openwhispr-main/src/helpers/transcriptionRoute.ts` — 纯路由、fail-closed 和 keyRef 边界。
- `/Users/goldenwave/Downloads/openwhispr-main/src/components/LocalWhisperPicker.tsx` — 模型卡、下载、取消和删除 UI。
- `/Users/goldenwave/Downloads/openwhispr-main/src/helpers/activeMicRecovery.js` — devicechange/track health/generation recovery。
- `/Users/goldenwave/Downloads/openwhispr-main/src/helpers/micTrackHealth.js` — 首帧健康与重新获取。
- `/Users/goldenwave/Downloads/openwhispr-main/src/helpers/meetingAecManager.js` — AEC sidecar supervision 与帧协议。
- `/Users/goldenwave/Downloads/openwhispr-main/native/meeting-aec-helper/src/aec_processor.cc` — WebRTC APM reverse/forward stream 处理。

### TypeWhisper 当前本地快照

- `/Users/goldenwave/Downloads/typewhisper-mac-main/TypeWhisper/Services/AudioDeviceService.swift` — UID、兼容性、设备诊断和预览恢复。
- `/Users/goldenwave/Downloads/typewhisper-mac-main/TypeWhisper/Services/AudioRecordingService.swift` — 异步启动、蓝牙稳定、录音与恢复。
- `/Users/goldenwave/Downloads/typewhisper-mac-main/TypeWhisper/Services/AudioEngineRecoverySupport.swift` — debounce/quiescence/burst/backoff/generation 状态机。
- `/Users/goldenwave/Downloads/typewhisper-mac-main/TypeWhisper/Services/ModelManagerService.swift` — engine/model/auth/readiness 分离。
- `/Users/goldenwave/Downloads/typewhisper-mac-main/TypeWhisper/ViewModels/FileTranscriptionViewModel.swift` — 单文件和 batch task 状态、取消与进度。
- `/Users/goldenwave/Downloads/typewhisper-mac-main/LICENSE` — GPL-3.0 许可边界。

### VibeVoice 当前本地快照

- `.sources/reference-projects/VibeVoice-main/README.md` — 模型家族、长音频 ASR、流式识别与能力边界。
- `.sources/reference-projects/VibeVoice-main/docs/vibevoice-asr.md` — 长音频 Who/When/What 输出与热词用法。
- `.sources/reference-projects/VibeVoice-main/docs/vibevoice-asr-streaming.md` — 流式 ASR 协议与推理方式。
- `.sources/reference-projects/VibeVoice-main/vibevoice/modular/modeling_vibevoice_asr.py` — ASR 模型实现参考。
- `.sources/reference-projects/VibeVoice-main/demo/vibevoice_asr_streaming_inference_from_file.py` — 文件流式推理入口。
- `.sources/reference-projects/VibeVoice-main/finetuning-asr/` — ASR 微调示例与数据格式。
- `.sources/reference-projects/VibeVoice-main/LICENSE` — 根目录 MIT 许可；模型、权重、数据和其他资产仍需逐项核对。

### 运行时上游

- Cargo registry `sherpa-onnx-1.13.5/src/offline_asr.rs` — `OfflineRecognizer`、单流和多流 decode。
- Cargo registry `sherpa-onnx-1.13.5/src/vad.rs` — Silero VAD Rust API。
- Cargo Git checkout `qwen3-asr-rs.../c5ef096/Cargo.toml` 与 `src/` — Qwen3-ASR 1.7B Candle/Metal 运行时。
