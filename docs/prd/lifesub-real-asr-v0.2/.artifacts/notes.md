# LifeSub 真实本地 ASR V0.2 设计记录

- 2026-10-13：旧 task7/task9 分支不在 `main` 祖先链上；直接强制删分支会让独有提交只靠 reflog 存活。先创建 `refs/archive/branch-cleanup-20261013/` 下的精确 tip 引用并验证，再删可见分支。当前 `main` 虽与 `origin/main` 提交一致，却有 160 条未提交状态项；从远端 HEAD 新建 worktree 不能代表最新产品源码。

- 2026-10-13：VoiceStudio `36862c1` 是 AGPL-3.0-only 的高优先级**参考**，不是可直接复制的依赖；模型资产另有许可。其 Electron/Python/Rust 多进程产品与 LifeSub 的 Rust Core + Swift helper + Evidence 图不同，取机制时须保留精确身份、原始音频与原子发布。源码对照见 `docs/context/wiki/syntheses/voicestudio-source-review-2026-10-13.md`。
- 2026-10-13：LifeSub 诊断已有异步外壳和存储字节投影，但内部目录计数未设时限/文件数上限，也没有“不完整”状态。先把高文件数耗时纳入 S4 测量；确有问题再为 Task 11 设计有界快照，不能把部分结果表示成精确总量。LifeSub 原生 ASR 在进程内线程执行；VoiceStudio 子进程隔离是后续风险研究，不能未经故障证据跨越 0.2.2 边界。

- 2026-09-11：用户明确拒绝“校正转写＋目标对话视图”三层方案，统一为可勾选维度的 AI 校正版。用户层级与底层 Revision 历史不是同一概念；避免按维度落整份文本，仍须保留来源映射和被排除片段依据。纯文本模型不支持音色分离；背景过滤目标不明时保留，不把偏离主题直接当可删除依据。

- 2026-09-11：技术文章中的“最终 ASR 是唯一事实”不能覆盖 LifeSub 原始音频权威；source、匿名 speaker 与人物身份必须区分。Qwen-Audio-3.0-ASR 不自动等于已有 Qwen3-ASR 模型，部署与能力需核实。音频里的命令是被记录的数据，不是用户授予 Agent 的执行权限。
- 2026-09-11：文章的 owner FIFO 是对话/执行解耦而非无限并发；重启失败策略不能取代 LifeSub 已有安全恢复。质量研究不扩张 S1–S4；总路线图已纠正旧采集未完成状态，历史计划版本号保留作背景。

- 2026-09-09 产品评估：README、总 roadmap、PRD 顶部与旧 remaining-capabilities 版本表落后于 9 月发布事实；同一 Task 编号在不同计划中含义不同，不能跨计划汇总完成率。同为 0.2.1 的不同安装包需用 executable hash 与最新过程记录区分；本次安装主程序匹配最终滚动修复包而非 workspace-status 顶部第一次 UI 包。
- 2026-09-09 产品评估：已通过录音 Gate 不等于所有界面文案都表达真实能力。当前录音页恒显“ASR 待接通”，诊断将可选未安装模型计数，声纹空态承诺注册但动作尚计划中。后续应按当前模式/模型所需条件表达 readiness；没有足够现有事实时使用中性汇总，不以 UI 猜测或新增后端语义挤入 0.2.2。完整结论见 `docs/product-current-state-insight.zh-CN.md`。

- 2026-09-03：Tauri CLI 不会可靠地通过 `CARGO` 环境变量改用项目 wrapper；本次 `CARGO=scripts/tauri-verified-cargo-runner.sh npm run tauri -- build` 仍由裸 cargo 进入 `build.rs`，因缺 `LIFESUB_SHERPA_VERIFIED` 正确 fail closed。打包应先由 `fetch-sherpa-runtime.sh` 验证固定 archive/attestation，再显式导出 `SHERPA_ONNX_ARCHIVE_DIR`、`LIFESUB_SHERPA_RUNTIME_ATTESTATION_FILE`、`LIFESUB_SHERPA_ARCHIVE_SHA256`、`LIFESUB_SHERPA_BUILD_ID`、`LIFESUB_SHERPA_VERIFIED=1` 给 Tauri CLI；不得放宽 `build.rs`。
- 2026-09-03：移动主从页面不能只靠 CSS 把多栏改为纵向堆叠。Timeline 需要 list/detail 状态，Dictionary 需要 categories/entries/detail 状态；搜索、scope 和滚动上下文必须由父组件保留。固定录音控制还必须包含稳定宽度的累计时长，并从移动固定区移除非核心“新笔记”动作。
- 2026-09-03：设置草稿通过保留已访问 panel 挂载来避免切 Tab 丢失时，Modal focus trap 必须排除 `[hidden]`、`[inert]` 与 `aria-hidden` 祖先中的控件，否则不可见表单会进入焦点循环边界。

- 2026-09-02：30 分钟场景必须在首块就预检双源 monotonic start drift。第一次运行因锁屏让 system_audio 只持续 `1332.5s`，第二次虽两路满 30 分钟但 start drift `336ms`；两次均作废。第三次先确认约 `210ms` 再继续，最终 `1948.34s / 209ms / 0 drop` 通过。长时验收前使用系统 sleep assertion，不能依赖 UI 保持可见。
- 2026-09-02：真实 sleep/wake 是当前机器触发同 UID engine recovery 的可靠事件。采证必须同时保留签名 helper 的 `source_recovering/source_recovered` 原始帧，以及主应用 Catalog 的连续 pre/post chunk 和 post-recovery ASR；单独一边不足以证明完整闭环。
- 2026-09-02：release capability marker 是证据通过后的编译期 sealed trait，不改变已经由冻结 app/helper 验证的运行行为。为保持 20/20 报告、DMG 与安装版三者精确 hash 一致，本次直接封装冻结候选；不得因一行 marker 重建而使全部硬件报告失配。
- 2026-09-02：`verify-desktop-asr.sh dmg` 的隔离 HOME 需预建 `~/Library/Application Support`，因为 Core 只创建 data directory 最后一层。该脚本的 `packaged-smoke` 仍是未接入 `run()` 的历史 scaffold，不能用超时等待或伪报告替代；发布证据应采用实际 bundle 签名/hash、正常安装、TCC attribution、权限诊断与单实例检查。

- 2026-09-01：`source_started` 是 native source identity 的发布边界。任何 recovery 或 sealed chunk 在全部 requested source identity 发布前都必须由 producer 抑制，不能让 Rust pending committer 接受。
- 2026-09-01：startup 期间观察到的 configuration notification 是旧 generation 噪声；等待 activate 后重放会造成无必要 recovery failure。应丢弃该 generation，让发布后的新通知创建新 generation。
- 2026-09-01：Swift actor 的 async start 会在 `await` 间重入。stop/shutdown 必须 invalidate startup generation，避免停止后继续发布 `source_started` 或 `activate`。
- 2026-09-01：双源 chunk sequence 是 source-local，不能按相同 sequence 跨源配对；额外 startup rotation 会制造约 30 秒假 drift。验收 drift 应从 source monotonic envelope 重算，报告值不可自报。
- 2026-09-01：observation 是不可信输入。无麦克风场景拒绝 microphone facts；permission-denied probe 必须绑定 selected/persisted/start-command identity。
- 2026-09-01：本机 Git 可能配置不可用 external diff driver。机器生成 source identity 必须使用 `git diff --no-ext-diff --binary`，否则外部工具失败可能留下空 digest。

- 2026-08-27：已完成能力的后续优化也必须按证据风险分层，不能按参考项目功能清单继续扩版本。当前建议把下一版本 `0.2.2` 固定为四类轻量工作：设置/诊断 IA 与 summary、probe/recovery/model/job 状态可读性、基于现有 snapshot 的 copy/open/re-test 操作性增强，以及 startup/first-frame/RTF/RSS/cancel 基线报告。凡需要 schema、helper/process、Evidence 图或新增原生验收矩阵的改动，默认移到更后版本。

- 2026-08-27：私有导出不能对 final path 直接 `truncate + write`。正确的本地 Evidence 导出需要同目录私有 temp、descriptor 先收紧权限、file sync、atomic rename、parent dir sync；失败必须保留旧文件。`O_NOFOLLOW` 仅拒绝 symlink，不拒绝 hardlink，因此 final target 还要拒绝 `nlink != 1`，且不能在验证前 truncate。

- 2026-08-27：Tauri WebView 中调用临时 `<a download>` 不能作为桌面安装版导出成功证据；浏览器 API 返回不代表文件落盘。Evidence 导出必须使用原生 save dialog，并把 session/revision identity 交回 Core 从 Catalog 重建内容，只有 `O_NOFOLLOW` 私有文件完成 write/sync/chmod 后才能提示成功。Tauri capability 还必须显式允许 `dialog:allow-save`，否则 UI 只能得到保存失败。

- 2026-08-27：音频 callback 的 host time 表示 buffer 起点，不能直接同时作为 chunk 终点。尤其启动/停止/恢复形成的单 callback 短块会有非零 PCM 与相同 start/end host time，Catalog 得到 `duration_ms=0`，ASR snapshot 正确 fail closed。Encoder 应按已写 sample offset / sample rate 计算每段 start/end host time；这也覆盖单个回调跨 chunk rotation 的边界。

- 2026-08-27：同 Bundle ID 的历史安装包仍会被 LaunchServices 选中，即使调用 `open` 的目标是项目 bundle。候选验收应直接运行 `Contents/MacOS/lifesub`，并用 `procPath`、app hash 和 Mach-O UUID区分延迟出现的历史 CrashReporter。当前候选的 DTAudioPlugin 首帧 probe 已真实通过（44.1 kHz、2 声道），但一次 probe 不得外推为 v3 矩阵通过。

- 2026-08-27：最终 app-only candidate 的静态门禁和 packaged provider gate 已重新绑定到当前构建；Tier 2 `582/0/7`、Tier 3 `622/0/8`。v3 verifier fixture 通过不等于真实硬件通过，缺少目标机报告时必须保持 fail closed；Mac 锁屏期间不得伪造权限、设备、恢复或 30 分钟双源证据。
- 2026-08-27：版本分层要按“数据/证据风险”而不是“参考项目功能数”决定。当前建议固定为：`0.2.1` 只收口 packaged 主链路和其暴露的 release-blocking 修复；`0.2.2` 只做轻量的设置/探测/模型/诊断可读性与性能基线；Task 9-16 这类扩产品面或改 evidence graph 的工作继续拆到后续版本。
- 2026-08-27：对“已完成能力”的后续开发计划也必须写成有完成线的 bounded task bundle，而不是只列优化方向。当前 `0.2.2` 已固定为 S1 设置/诊断 IA、S2 probe/recovery/model/job 状态可读性、S3 diagnostics copy/open/re-test 操作性、S4 baseline report pack；凡超出这四类并触及 schema、helper/process、Evidence 图或新验收矩阵的提案，默认升级到更后版本。

- 2026-08-26：CrashReporter 弹窗可能延迟显示历史报告，不能按用户看到弹窗的时刻推断当前操作崩溃。必须比较 report `captureTime`、`slice_uuid`、当前 Mach-O UUID 和是否存在更新报告；本次截图对应 19:48 的旧 startup ownership abort，当前 20:xx 双源失败没有生成新报告。
- 2026-08-26：CoreAudio 设备“枚举为 alive/has input”不等于能在当前路由中稳定交付首帧。系统默认输入为蓝牙耳机时，精确绑定内置麦克风可能随后收到配置变化并被系统路由抢回；蓝牙或虚拟设备也可能具有合法 nominal format 但不产帧。smart 验收必须同时记录默认输入、selected UID、首帧 readiness 和配置变化，不能只凭设备列表标记“已连接”。
- 2026-08-26：VAD 返回 0 utterance 表示输入可用但没有检测到语音，不能复用 `InputUnavailable`。静音 chunk 应完成 fenced Job 并保留可追溯成功回执，但不得创建空 Revision，否则最新空 Revision 会遮蔽 Timeline 中此前已有的真实转写；Worker 也不应发送空 publication 事件。真实 system-only 证据同时证明录制前静音块和停止尾块都会稳定触发这一边界。
- 2026-08-26：Tauri 管理状态的 `Drop` 不能当作录制会话退出恢复机制。正常/异常进程退出都可能留下 Catalog `recording` 行和没有足够 timing 元数据、不可安全发布的 staging 尾块；下一次独占 Core 启动必须先严格清理 staging，再把未结束会话关闭到最后一个已持久化证据边界。不可把未知 partial 猜测为有效 chunk。
- 2026-08-26：录制启动不得只消费 React render/ref 中的 mode/UID。设置保存、设备 discovery 和 optimistic microphone selection 都可能与点击同帧竞态；启动动作应等待选择保存队列、重读持久化配置，并用同一 action-time snapshot 做 mode/UID/availability preflight。system-only 必须在麦克风为空或未授权时仍能独立启动。
- 2026-08-26：权限拒绝错误必须携带 capture source。把 ScreenCaptureKit 的拒绝统一映射成“麦克风权限未授权”会误导用户和验收报告；helper 仍可复用既有 fatal_error schema，但 stable code/summary 应分别为 microphone 与 screen/system-audio。

- 2026-08-26：短 buffer 重采样不能为每个回调新建 `AVAudioConverter`。macOS 实际回调常小于请求的 tap buffer size；converter 每次初始化都会重新支付 priming 延迟。真实 packaged 证据表现为 480,000 个 16 kHz 样本需要约 32.4 秒墙钟时间，独立 44.1 kHz/256-frame 探针则是一秒只输出 14,982 samples。应按实际输入采样率在连续流内复用 converter，并在进入 16 kHz direct path 时清空缓存状态；测试必须覆盖短 buffer 累计和 `44.1k -> 16k -> 44.1k`，不能只测试单个大 buffer。

- 2026-08-26：原生音频工作格式必须与 ASR 解码器的实际 feature 集一致。macOS `AVAudioFile` 默认写出的 CAF 即使系统可正确读取，也不能假设 Symphonia 支持；当前 Rust 仅启用 WAV/FLAC/MP3/OGG/Vorbis/AAC/MP4。原生 helper 因此写 PCM WAV，并在封块前验证实际文件容器行为；仅改变扩展名会产生“名为 WAV 的 CAF”并继续导致 `InputUnavailable`。
- 2026-08-26：staging 文件名是跨语言安全契约。Swift 生成器不能自行插入 source 中间段，Rust sealer 也不能为兼容漂移而放宽 grammar。录制中的 active WAV 使用真实 `.wav` 扩展让 AVFoundation选择正确容器；关闭、同步、收紧 `0600` 后才原子改名为 canonical `.wav.partial` 并发布给 Rust。
- 2026-08-26：native chunk 持久化与 ASR enqueue 的顺序必须是 `seal final file -> insert chunk+timing transaction -> enqueue job`。反向顺序会让 Worker 观察不存在或未同步的输入；只 seal 不 enqueue 则形成可播放但永远“等待转写”的静默缺口。回调测试必须在 enqueue 边界重新读取 final 文件、chunk 和 timing。
- 2026-08-26：麦克风权限参考实现复核：TypeWhisper 先读 `AVAudioApplication.shared.recordPermission`，仅 `.undetermined` 调用 `requestRecordPermission`，`.denied` 只打开系统设置；OpenWhispr 将 `getMediaAccessStatus` 与 `askForMediaAccess` 分离，并在 UI 已授权时隐藏申请按钮。LifeSub 的 `AVCaptureDevice.authorizationStatus` 分支已具备同样的“authorized 直接返回、notDetermined 才申请”语义，因此重复系统弹窗不是重复调用申请 API 的逻辑缺陷。当前 Tauri 配置显式 `signingIdentity = "-"`，实际 app/helper designated requirement 均退化为随构建变化的 `cdhash`，且本机 `security find-identity -p codesigning` 为 0；TCC 因身份变化要求重新授权。可借鉴的产品模式是把无副作用状态查询、显式授权动作和录制启动分离，但真正消除重建后反复授权必须使用稳定 Apple Development/Developer ID 签名。
- 2026-08-25：跨进程协议中的 `recoverable` 字段必须在消费端参与状态转换，不能只作为日志元数据。Swift helper 将 `device_configuration_changed` 标记为可恢复时，Rust coordinator 若仍无条件进入 Failed，会把正常音频设备重配置误报为整次录制失败；回归必须同时锁定 recoverable 保持 Running 与 non-recoverable 继续 fail closed。
- 2026-08-25：异步启动期不应用两个相互重叠的 JSX 条件表达“可点击/禁用”。`captureState === idle` 与 `captureState === idle && lifecycleBusy` 会同时为真，导致重复按钮；正确形式是单个按钮通过 `disabled={lifecycleBusy}` 投影状态，并用 pending Promise 回归锁定。
- 2026-08-25：Tauri/Vite 开发服务器的 watch root 是仓库根目录时，必须显式忽略 `src-tauri/target` 和 Swift `.build`。当 optional `fsevents` 不存在时，轮询 20 GB / 10 万个构建文件会把开发 UI 卡顿伪装成原生录音阻塞；应通过分进程 CPU 与 stack sample 区分前端 watcher、LifeSub 主进程和 capture helper。
- 2026-08-25：Tauri 同步 command 中即使业务内部已经创建 worker thread，只要 command 本身在 AppKit invoke handler 中阻塞等待 channel，WebView 仍会整体冻结。任何权限、设备、helper lifecycle 或 drain/join 等待都必须移入 `spawn_blocking`，UI 用独立 starting 投影呈现真实过渡态。
- 2026-08-25：Unix stream 上的 `FileHandle.read(upToCount: 4096)` 在当前 Foundation 实现中可继续等待 requested count，不能当作“读当前可用字节”的 framing primitive。已知长度前缀协议应先精确读 4-byte length，再精确读 body；测试 socket reader 也必须使用同样语义，否则批量发送命令会掩盖单帧生产死锁。

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
- 2026-08-15：仅设置 `SHERPA_ONNX_ARCHIVE_DIR` 不能阻止 `sherpa-onnx-sys` 复用 `target/sherpa-onnx-prebuilt`。Task 1 增加持锁 wrapper，每次原子隔离旧 prebuilt、强制 scoped rebuild，并通过 PID/mtime 回收 stale lock。
- 2026-08-15：sherpa native API 报告 8 位 Git SHA，不能伪装成完整 commit。实现分别保存 observed SHA 与 pinned full SHA，并使用非 panic 的 typed validation。
- 2026-08-15：代码质量审查留下 Minor：`with-sherpa-runtime.sh` 调用 fetcher 时应为脚本路径加引号，以兼容仓库路径包含空格；当前工作区路径不触发，后续维护时处理。
- 2026-08-15：用户要求同时考虑 Qwen3-ASR 0.6B 与 1.7B。sherpa-onnx 1.13.5 已提供 Qwen3-ASR Rust 配置；其官方模型发布中存在 0.6B INT8 包（878,702,423 B，SHA-256 `393f8a14e2f5fb96746aaab342997a40641001fbd5bf9592a080a8329178ee96`），因此 0.6B 可复用当前静态 Rust 运行时。
- 2026-08-15：未发现 Qwen3-ASR 1.7B 的 sherpa-onnx 预转换发布包。Hugging Face `-hf` 原始权重需要 Transformers/Python 或另一原生适配器，不能直接交给当前 Rust Provider。V0.2 将其作为高质量实验选项，但在固定 size/hash/required-files/conversion provenance 和 Apple Silicon memory/RTF Gate 前禁止安装，且不得静默回退到 0.6B。
- 2026-08-15：Task 2 最终质量复审发现 SQL normalizer 会丢失 token 边界，使 `TEXT NOT NULL` 与 `TEXTNOT NULL` 发生碰撞。修复必须先加入负向测试，再保留 token 边界或改用结构化 PRAGMA 校验。
- 2026-08-16：Qwen 范围规格审查要求 Manifest 区分 `Installable` 与 `ExperimentalUnavailable`，并把 selectable/installable/executable 能力贯穿设置、下载、Core enqueue 与 Provider Factory。1.7B Gate 固定 Apple Silicon/16 GB/macOS 14+、CER/WER <= 20%、混合关键短语 100%、RTF <= 1.0、RSS <= 6 GiB，并使用确定性 300 秒 fixture。
- 2026-08-16：Task 3 质量审查发现内部设置错误会形成第二套序列化错误码，且公开 Provider Receipt 字段可构造不可信 provenance。实现改为内部错误 exhaustive 映射到 `AsrErrorCode`，Receipt 仅能通过验证 draft/custom Deserialize 构造，并校验 hash、JSON、VAD identity 与时间顺序。
- 2026-08-16：Agent MVP 架构采用 C -> A：先抽出 CoreRuntime 与版本化 Local Tool API，Tauri/Unix socket 仅作适配器；最终由 launchd `lifesubd` 托管。插件/Gateway 不得调用 Tauri Command、打开 SQLite 或读取内部路径。C 阶段退出 Tauri 进程仍会停止录音，不能提前宣称 daemon 生命周期。
- 2026-08-16：Task 4 安全复审证明 `symlink_metadata` 后再按路径打开存在 check/use 竞态。音频导入、校验和 reconciliation 改为以 no-follow 目录 fd 为锚点，使用 descriptor-relative 操作，并在文件副作用与 DB commit 前复验目录 `(dev, ino)`。
- 2026-08-16：文件锁必须绑定完整 Core 生命周期，且所有 Catalog 写入口必须结构化经过 ownership guard。当前单用户 macOS 产品以 canonical parent directory inode 锁防止协作式第二 LifeSub 实例进入 writable Catalog/migration/reconciliation；同 UID 恶意篡改文件系统 namespace 不属于该并发锁的安全边界，因为该主体本已能直接修改本地 SQLite 与音频。
- 2026-08-16：reconciliation 的可删除文件 grammar 必须与 importer 生成规则完全一致。最终仅接受 lowercase SHA/chunk ID 和规范化的 1–16 字节 ASCII 字母数字扩展名；空、非 UTF-8、过长或含标点扩展名统一落为 `audio`，未知或伪装 entry fail closed 且不删除。
- 2026-08-16：未知持久化 `integrity_state` 不能在启动修复或 ASR 前校验中被重新哈希后覆盖为 `available`；服务在任何文件副作用与状态更新前先解析当前 DB 状态，未知值直接返回 Catalog 错误。
- 2026-08-16：Task 6 migration 必须保留历史 v2 DDL 不变，再以独立 v3 transaction 重建 `model_installations` 并创建 `model_download_artifacts`。旧 `ready` 缺少结构与 runtime attestation，唯一安全迁移是 `installed_unqualified`。
- 2026-08-16：可恢复下载的 source identity 不能只比较 repository/model/revision/URL；还必须包含 expected SHA-256 与 required path。长度已完整但 hash 错误的 `.part` 必须从 0 重启，不能发送 `Range: bytes=<total>-`。
- 2026-08-16：测试 loopback HTTP 通过注入 transport 实现；production `ReqwestTransport` 始终 HTTPS-only，并在每个 redirect hop 和最终 URL 使用当前 artifact 的精确 host allowlist。
- 2026-08-16：sherpa structural qualification 必须消费 Task 5 canonical full identity（version/full Git/native archive SHA/build ID），四字段任一不匹配都不发布 installation，并 quarantine staging/install directory。Qwen 1.7B Task 6 只做五文件 structural Gate，状态保持 `installed_unqualified`，禁止初始化 Candle。
- 2026-08-16：streaming checkpoint 必须在 response copy loop 内按 4 MiB bound 持久化；checkpoint 之前先 flush/sync_data。network EOF、cancel 与短 body 都保存实际 durable bytes 和 validators，retry 才能从真实 checkpoint 发 Range/If-Range。
- 2026-08-16：archive install contract 是 Task5 manifest 的 canonical derived metadata：scan 全 entry，但去掉唯一顶层 root 后只写 exact whitelist；entry count、单文件 bytes、总写入 bytes、required size/hash 全部精确匹配。README/scripts/optional INT8 不安装，Qwen06 tokenizer 三件套必须安装。
- 2026-08-16：磁盘预检比较 available free bytes 时不能重复加入已占用的 verified parts 或旧安装；同卷 staging->final rename 不复制 payload。精确 additional requirement 是 `remaining parts + one required inventory total + 512 MiB`，并使用 checked arithmetic 与 volume identity 检查。
- 2026-08-16：删除使用 Catalog deletion lease 保存完整 prior qualification，安装目录先写 durable delete marker、rename 到 trash 并 fsync，再删除 Catalog row；DB 失败 rename 回并恢复 exact prior。startup global reconcile 处理 marker 前崩溃、trash 残留与 DB/FS 不一致。
- 2026-08-16：Task 6 review 收尾发现，仅从 manifest 复制 `install_constraints` 不足以证明消费端 canonical。凡 `model_id + manifest_version + bundle_identity` 命中静态 registry 的下载计划，manager 必须在任何 DB/network 副作用前对完整 `ModelInstallPlan` 做等值绑定，防止扩大 archive bounds、替换 required hash/path 或漂移 source/runtime/device contract。
- 2026-08-16：delete marker 是独立的两阶段持久化协议。`.lifesub-delete.json.tmp` 的 write、sync、rename 或最终 directory fsync 任一失败，都必须移除 tmp 与 final marker 并再次 fsync 安装目录；startup 看到 `deleting` 但没有 durable final marker 时也执行同一清理后恢复精确 prior state，避免残留 tmp 阻塞后续删除。
- 2026-08-16：global model reconciliation 不能丢弃 `reconcile_installation` 错误。实现以 `ReconcileOutcome::RejectedDurably` 区分“已完成 quarantine/Catalog 降级的语义拒绝”和真正的操作失败；quarantine rename/fsync、Catalog recovery/publish 等错误必须向上传播，使 `CoreRuntime::initialize` 返回 Err，禁止旧 `runtime_qualified` 状态继续形成可用 runtime。
- 2026-08-16：download row 进入 `installing` 后必须由统一 completion 状态机收口。assembly/structural/rename 等 pre-commit 失败写入 `failed + stable error code` 释放 active unique index；final rename 后的 Catalog publication 歧义立即 reconcile，无法确认时写入 `failed + recovery_required`。verified parts 保留，使同进程 `retry_install` 不需重启或重新下载。
- 2026-08-16：下载前磁盘预检不能替代 assembly 前预检。安装状态切换后、创建 staging payload/marker 前必须再次使用当前 checkpoint 与 available space 计算 exact formula；流复制在每次 write 前以 checked arithmetic 校验 `expected_body` hard bound，生产 transport 同时设置 connect 与 bounded response timeout，timeout 后重新观察 cancellation。
- 2026-08-16：所有进入路径 join 的 identifier 必须通过 bounded ASCII single-component grammar；production API 只接受 model ID 并从静态 registry resolve canonical plan，合成 plan API 仅保留 crate/test 可见。reconciliation 目录发现统一使用 `symlink_metadata`，只遍历真实目录、只 hash regular file，FIFO/socket/device 和 provider/model/final symlink 均 fail closed。
- 2026-08-16：deletion marker 必须逐字段等于当前 `deleting` Catalog lease，包含 model/path/prior state/runtime/qualified timestamp/error；trash recovery 也先查询当前 lease，Catalog DELETE 必须 CAS 命中恰好一行。stale/mismatch marker 不删除 payload 或 row。
- 2026-08-16：Task 6 model manager 最终按 download/install/delete/reconcile/archive/fs/types 拆分；一次机械 split 因 apply_patch 同路径 delete+add 限制删除了未跟踪单文件，随后依据 69 个行为测试、manifest 与 Catalog contract 从模块化实现完整恢复，并重新通过全部质量门。后续大文件拆分必须先落目标模块或保留可验证副本，再删除源文件。
- 2026-08-16：安装已经 publish 后，最终 `model_downloads.state = succeeded` 写入失败同样属于 post-commit ambiguity；必须再次 reconcile 安装并重试状态转换，若仍失败则尽力写入 `failed + recovery_required`，不能留下 active `installing`。对应 one-shot state transition fault 已加入回归测试。
- 2026-08-16：文件尺寸剩余 Minor：`model_manager/download.rs` 为 531 行，略高于约 500 行目标但已从原 2570 行单文件隔离；按新 review 规则不再为零 Minor 做第三轮结构调整，后续下载协议变更时再拆 transport/checkpoint 两层。
- 2026-08-16：stalled-response 测试的 server 不能永久阻塞在 `accept()`。并行测试中 cancellation 可能在 request 前触发，导致 client 不连接而测试线程永远 join accept。fixture 改为 nonblocking accept + shutdown channel + 2 秒 server deadline，并只在 accept 确认后启动 cancellation；production response timeout 同步收紧为 2 秒。exact 与 focused 均在外部 10 秒 alarm 内退出。
- 2026-08-16：Task 8 qualification smoke 不能把可注入泛型 adapter 暴露为 production API，否则测试 fake 可成为生产资格来源。production 只允许 ModelManager 绑定当前设备、重验 bundle 并构造真实 Qwen Candle/Metal smoke；generic RuntimeQualifier 仅供 crate 内测试与该固定入口使用。
- 2026-08-16：Job 的零行 fenced UPDATE 不能统一解释为 ownership lost。renew、阶段转换与 fail 都必须在同一事务内区分 `CancelRequested` / `OwnershipLost`；Coordinator 只有在 fenced acknowledge 将数据库写成 `cancelled` 后才能清 active 并领取下一任务。
- 2026-08-16：模型删除不能只依赖 Catalog job lease。Provider 必须持有共享 registry 的 RAII execution lease，Manager clone 共享同一计数；Provider 构造消费已重验 installation，不能重复执行 inventory hash pass。
- 2026-08-16：资格音频 hash 本身不足以冻结 smoke 语义。phrases、匹配阈值、normalization、expected text、source/PCM hashes、archive path/hash、license 与 provenance 必须形成 canonical digest，并写入 runtime identity，防止 fixture metadata 漂移后仍资格通过。
- 2026-08-16：UUID qualification temp marker 的错误清理必须覆盖 create/write/sync/rename/publish 冲突，启动 reconcile 还需枚举并删除合法 UUID grammar 的 stale temp 后 fsync 目录；不可用固定 `.tmp` 假设。
- 2026-08-16：ignored real-model gate 被显式选择时不能因缺环境静默 return。缺 `LIFESUB_RUN_QWEN17_SMOKE=1` 或 `LIFESUB_QWEN17_MODEL_DIR` 必须测试失败，使 CI/人工 gate 不会产生零工作假绿。
- 2026-08-16：目录 fd 经 `F_GETPATH`/`readlink` 转回普通 pathname 不是 SQLite 锚定，路径解析后仍可二次 swap。macOS 首发采用进程级 tokenized wrapper VFS：主库与 sidecar 用 `openat(O_NOFOLLOW)`，NULL temp fail closed + `temp_store=MEMORY`，`unix-excl` 避免物理 SHM，`HAS_MOVED` 比较 open fd 与 anchored entry 的 dev/ino，并验证 regular/current uid/0600/nlink=1。
- 2026-08-16：文件 cleanup 即使先比较 dev/ino，identity check 与裸 `unlink` 之间仍有同名替换窗口。Evidence 与 Model startup reconciliation 的破坏性清理统一使用 no-replace 随机 tombstone 原子 rename，移动后再次核验 identity；失配时恢复原名并 fail closed。
- 2026-08-16：Task 10 publication 成功事务必须同时 fence claimed_by、claim_generation、`state = transcribing`、cancel marker 与同一 `now` 下未过期 lease；只检查 generation 仍允许过期 worker 在 recovery 前发布。
- 2026-08-16：Core startup fd-anchored 不等于 ModelManager 全生命周期安全。Task 10 M3 必须把 download/install/delete/qualification/provider lease 统一收口到 AnchoredFs capability，并让 provider 从已验证 fd 加载，禁止 production 模块继续直接 `root.join`。
- 2026-08-17：流程优化——双审合并为单轮合并审查、测试改为三层分级、文件行数放宽至 600 行、新增 `scripts/check.sh` 一键验证。优化原因：双审串行导致每 Task 至少 2 轮沟通，全量 283+ tests 每次必跑耗时巨大，300 行限制在 Rust 模块中不切实际。新流程：`make check`（Tier2）作为日常预提交指令，`make check-full`（Tier3）仅在 Task 收尾时执行一次。
- 2026-08-19：设置弹窗缺陷的根因不是单点 bug，而是“演示态 UI”残留：`Modal` 只有视觉层没有真实 dialog 焦点管理；设置页同时继承了全局 `.modal-body` grid 和局部 `.settings-layout` grid，窄窗/放大字时双重压缩；模型/关于/声纹动作依赖硬编码占位数据或无反馈按钮。修复时先补组件级失败测试，再把版本与模型状态改为只读运行时投影，未落地动作统一降级为禁用的“计划中”。
- 2026-08-19：二轮 review 指出第一版设置修复仍有生命周期/错误路径缺口：多实例 modal 会双重处理 Escape/Tab；Tauri 设置加载错误被 adapter 静默吞掉；模型安装/下载状态只按 `model_id` 聚合；声纹重命名/删除没有 pending/error。修复方式：Modal 改为全局 stack + inert 引用计数；Tauri adapter 加载接口改为直接抛错，由组件显式显示错误并提供 retry；模型投影精确按 `(model_id, manifest_version, bundle_identity)` 匹配并选择当前 bundle 最新下载记录；声纹动作增加 pending/禁用/error UI。
- 2026-08-19：UI 走查修复的核心安全原则是“不可用必须 fail closed”。生产采集、LLM、Catalog、模型或音频绑定失败时，不得回退 Demo、替换文本或播放另一 Chunk；浏览器 Demo 必须持续标记且永不宣称保存。
- 2026-08-19：录音状态不能只锁 UI。后端 error、pause/resume/stop 的迟到 Promise 也必须以 lifecycle generation + session ID 在远端 mutation 前校验，否则 Catalog 仍可能被旧操作复活。
- 2026-08-19：设置弹窗双重 Grid 不能只靠低特异性覆盖。实际 CSS 顺序使 `.modal-body` 覆盖 `.settings-modal__body`；最终使用 `.modal-body.settings-modal__body` 并以浏览器计算样式确认 body=block、内容宽度 738px。
- 2026-08-19：`/dev/fd/N` 在并行测试中可能立即被其他线程复用，不能用 `path.exists() == false` 证明 fd 已释放；应比较关闭前后的 `(dev, ino)` 文件身份。
- 2026-08-21：Task 2 release gate 修复提交 `371faea` 因 `commands.rs` / `lib.rs` 已是 dirty integration baseline，提交同时捕获了既有 import job、Timeline projection、model/runtime API、worker lifecycle 与 command registration 改动。现有 focused 覆盖包括 `commands::tests::{import_persists_failed_job_when_configured_model_is_invalid, invalidated_ownership_blocks_every_write_class_without_db_changes, list_timeline_records_returns_resolved_audio_and_revision_history, list_asr_models_ignores_stale_installations_and_downloads, list_asr_models_uses_latest_current_bundle_download}`，以及 `asr::worker::tests` 的 recovery、lease renewal、cancel、failure、shutdown 和 single-coordinator cases。本轮尝试新鲜运行 `commands::tests::` 时被另一协作者 PID 9693 持有的 `.lifesub-sherpa-runtime.lock` 阻塞，未将该次执行表述为通过；行为代码未在本修复中重写。
- 2026-08-19：Job snapshot 的 required-file inventory 是 JSON 数组，Receipt 层若强制 JSON object 会让真实 publication 永远失败。Receipt 只校验合法 JSON，snapshot loader 校验数组结构/路径/hash，publication 再做字符串身份等值。
- 2026-08-19：Tauri 项目若 Cargo 默认 feature 为空，直接 `tauri build` 会把 `#[cfg(not(feature = "desktop"))] run()` 打成空实现，应用启动后立即正常退出。桌面打包必须显式使用 `tauri build --features desktop`，并以“启动后进程持续存在”作为安装前门禁，不能只看 bundle 生成成功。
- 2026-08-21：Task 2 release wiring 证明不能依赖 factory 自报的布尔常量，否则 fail-closed 实现可伪装成 native。门禁改为 crate 内 sealed capability marker，外部测试仅接受同时具备 native capture 与 native ASR capability 的 production factory；新增 lying-flags 回归 fixture。当前 production alias 仍指向 `FailClosedDesktopRuntimeFactory`，因此正式门禁必须失败，planned audit 也必须明确 `not release-ready`。
- 2026-08-21：Task 3 协议复审发现 PCM payload 对齐、Swift socket 合并帧消费和跨语言 fixture 三个 Important 缺口；已改为编解码均校验非空 frame stride、`decodePrefix` 返回消费字节数，并让 Rust/Swift 共读单一 hex fixture。剩余 Minor：Foundation `JSONSerialization` 对重复 key 的行为尚未与 Rust 做显式一致化；已严格拒绝未知字段/未知消息/尾随数据，duplicate-key 加固在 helper 认证与 fuzz 覆盖时处理。
- 2026-08-24：Task 7 原子 sealing 的 crash window 不能只靠 final 文件 hash/长度恢复，否则新请求可伪造 source/timing provenance。最终协议在 rename 前写入并 fsync canonical durable intent，绑定 chunk/path/hash/length/session/source/完整 timing；恢复必须逐字段全等。Intent temp 使用随机 UUID grammar，所有写/sync/rename 失败都 identity-safe 清理并同步目录，startup 对严格 stale temp 做 no-follow/owner/mode 校验后回收，未知 entry fail closed。
- 2026-08-24：`scripts/check.sh tier2` 在当前受限执行环境中全量测试为 439 passed / 94 failed；失败集中在 Unix socket sandbox EPERM、只读 DB、子进程 helper 等环境敏感路径。Task 7 focused（capture 17、Catalog 16、migration 39、production startup 3）与 fmt/diff 通过，最终审查 Critical/Important=0。正式发布前必须在正常 macOS 环境重跑字面 Tier 2/Tier 3，当前证据不能替代发布门禁。
- 2026-08-24：Task 8 接续检查发现 SwiftPM 当前执行器无法编译 helper：CommandLineTools 与 SDK 的 Swift effective 版本不匹配（6.0.3.1.10 vs SDK 6.0.3.1.5），且默认 clang ModuleCache 路径不可写。该环境错误不能作为 helper/硬件验收证据；Rust coordinator core 可继续做注入式 RED/GREEN，但在匹配 Xcode toolchain 的正常 macOS 环境前不得声称 helper 控制循环或生产 capture 已接通。
- 2026-08-24：使用 `--disable-sandbox` 与 `/tmp` ModuleCache 后 Swift helper 可验证。Task 8 helper 控制循环已从 hello-only 改为严格 start/pause/resume/stop/shutdown，接入真实双源 adapter factory 和独立 staging encoders；release Swift build 通过，CaptureSessionController 11/11，Rust coordinator 4/4。CommandLoop socket integration test 仍存在测试 harness 阻塞，不能替代真实 packaged helper acceptance；Rust AuthenticatedHelper transport/runtime wiring 尚未完成。
- 2026-08-24：Task 8 合并审查为 Critical 0 / Important 7 / REJECT。主要缺口：Rust/Swift lifecycle frame version 漂移、coordinator per-source 命令与 helper session命令冲突、source_started DTO/真实 metadata 缺失、helper staging/sealer路径不一致、sample/host time provenance错误、production仍不可达。已修 control protocol_version、共享顶层 staging、累计 sample positions、no-default cfg/重复module/移动错误；剩余双源单次命令、真实 source start/stop metadata、统一连续时钟和 production wiring。
- 2026-08-24：desktop verified-runtime 下任意 `CoreRuntime::initialize`（不仅 session test）在 ModelManager reconciliation 阶段返回 SQLite `ReadOnly`；相同核心初始化路径 no-default 正常。独立 target与单线程复现，排除 session worker和target quarantine本身。该问题阻断 desktop `NativeCaptureSession` 行为验证与 production factory切换，需在 runtime/model reconciliation 层定位，不能绕过。
- 2026-08-24：SQLite `ReadOnly` 根因定位并修复：anchored VFS 将已由 `openat(O_RDWR)` 验证的 Catalog fd通过 `/dev/fd` 委托给 native `unix-excl` 时，native `xOpen` output flags降为 `READONLY`；wrapper需在成功 native open后按原始 requested `SQLITE_OPEN_READWRITE` 恢复 output flag。修复后 CoreRuntime write regression通过，desktop NativeCaptureSession tests `2/2`通过。
- 2026-08-24：Task 8 最终复审剩余 Critical 伪设备 fallback已修为真实 microphone device缺失即 fail-closed；source metadata改为immutable 16k/mono工作域；committer使用单一 session continuous-ns anchor；UI desktop stop失败不再转 stopped/显示成功。前端 stop regression 19/19通过。Task 8仍有source-mode typed status与Native ASR后续工作。
- 2026-08-25：Native ASR 的生产 Worker 接线与 release capability 是两个不同门槛。Worker 可以选择 `ProductionNativeAsrEngine` 并在无模型时稳定 fail closed，但只有 canonical ModelManager 产生 exact `runtime_qualified` installation、packaged executable 完成真实音频 inference、正式 receipt/segment 被 fenced publication 持久化后，才允许实现 `NativeAsrDesktopRuntimeFactory` marker。编译通过、Sherpa archive 存在或其他应用的 ONNX 文件都不能替代该证据。
- 2026-08-25：handoff 审计确认真实 LifeSub Catalog 的 `model_installations` 为 0，正式 release verifier 仅剩 Native ASR capability gap；因此当前正确动作是保留 fail-closed marker、继续真实模型 gate 与端到端验收，而不是提前打包或覆盖历史安装包。
- 2026-08-25：启动真实模型验收时，开发版持有 `asr-worker.lock` 后又启动历史 `/Applications/LifeSub.app`，第二实例的 `CoreRuntime::initialize` 正确返回 `Ownership(AlreadyOwned)`，但 Tauri setup 用 `?` 传播错误，触发 `tauri::app` panic 并跨 macOS `did_finish_launching` FFI abort。修复为 setup 将初始化失败转换为明确退出状态，不进入 event loop。回归复现修复前退出码 134；修复后第二实例退出码 0、stderr 记录稳定原因，DiagnosticReports 数量不增长。历史安装包仍不含修复，发布前不得与开发版并行启动。
- 2026-08-25：ModelManager production HTTP timeout 原设为 connect 15 秒、blocking I/O 2 秒；本机直连 GitHub asset 的实测首包约 23 秒，导致 VAD 和 SenseVoice 下载稳定失败。调整为 30 秒，并保留下载循环 cancellation 与 4 MiB durable checkpoint。SenseVoice 在 134,234,112 bytes 后响应中断，暴露 UI/Core 没有编排现有 `retry_model_download`/`retry_model_install`；新增 `ensure_model_installed` 复用 exact current-bundle download ID，并对纯 `model_download_failed` 做最多 3 次有界续传。
- 2026-08-25：Task 9 production `ExecutionEngine::prepare` 同步初始化 Provider/VAD，debug 实测超过 job 30 秒 lease；Worker 只有进入 run 后才 renew，导致 job 永久停在 expired `preparing` 并让 worker 线程退出。修复为 prepare 只冻结 snapshot，重型 native preparation 移入 background `ExecutionRun`，使 Worker 能按 200 ms poll 续租/取消/所有权。真实 recovered job attempt 2 与 queued job attempt 1 随后均 succeeded 并 fenced publish。
- 2026-08-25：SenseVoice 对 Qwen codeswitch fixture 输出 `I'm alone all by myself. If too sell Sono too as toy soda.`，只可靠识别英语开头，对法语/意大利语/西语片段失真。该结果证明真实接入和 provenance，不证明多语质量；后续质量 Gate 需按模型适配 fixture，不应拿 Qwen qualification 文本直接作为 SenseVoice 的通过阈值。
- 2026-08-25：Task 10 事件不能作为事实源。最终把 event 定义为 invalidation signal，LiveCapture 先注册 listener 再查询 Catalog，使用 persisted revision number cursor 收敛重复/乱序；成功 DTO 从 Catalog 补齐 native capture_source_id，失败 event 只在 fail transaction 后发出。
- 2026-08-25：Task 11 verifier 必须拒绝“heartbeat 通过即原生验收通过”。新增结构化报告 schema 和 12 场景矩阵，权限拒绝允许无 chunks/ASR，其余场景必须有 exact binary/source/device/capture/ASR identity；30 分钟双源使用 duration、drift、drop、RSS 硬阈值。真实报告采集仍需 packaged app 和人工硬件/权限操作。
# 2026-08-25 麦克风选择边界

- 首版不支持录制中热切换。后续热切换必须在同一 Record 内新建 capture-source/Evidence 边界并先封存旧源 chunk，不能改写已有来源。
- 枚举与精确选源由已认证 Swift helper 负责；前端不直接访问系统设备 API，Rust 必须校验 `source_started.device_id` 与请求 UID 完全一致。
- 已保存设备缺失时不得自动选择 Mac 内置麦克风。系统默认仅可作为未选择状态下的推荐项，仍需用户显式确认。
- 2026-08-26：协议复审保留两项 Minor 加固：增加含斜杠与浮点值的跨语言 canonicalization 共享 fixture；在 Swift/Rust 两端于 JSON 解析前显式拒绝重复 key，避免依赖各自解析器的覆盖语义。
- 2026-08-25：设备选择第一段实现先打通协议/持久化/Tauri 最短闭环，再碰 UI。否则 React 侧很容易先引入一套本地状态与 DTO 命名，后面再回头改成 helper 真实 contract 会出现第二次迁移。当前 TS payload 统一使用 camelCase，Rust 内部 DTO 仍保持协议侧 snake_case / enum 语义。
# 2026-08-26 packaged 麦克风选择与签名踩坑

- Tauri 会在 bundle 阶段重签 external sidecar，完整文件 SHA 与 identifier 都会变化。不能用 pre-bundle 完整 SHA 直接认证 packaged sidecar，也不能只允许 source helper 的裸 identifier。安全做法是同时要求 strict signature、限定 Tauri sidecar identifier 格式、canonical unsigned-code SHA 精确匹配，并继续用完整 file identity 绑定实际启动进程。
- Hardened runtime 下仅有 `NSMicrophoneUsageDescription` 不够。主 app/responsible process 和 helper 的签名都必须包含 `com.apple.security.device.audio-input`; 否则 TCC 会记录 `requires entitlement ... but it is missing`，直接拒绝 prompt，业务层只看到 start failure。
- AVAudioEngine 精确换设备时，必须在 `CurrentDevice` set/readback 之后重新读取 input-node format。绑定前缓存 format 会在 48 kHz 默认 aggregate -> 44.1 kHz built-in 等真实设备组合下触发不可捕获的 Objective-C `Input HW format and tap format not matching` 异常。
- Swift Testing 顶层 CommandLoop socket tests 不可并行：async 测试内部运行同步 `FileHandle` read 会占满 cooperative executor，造成 peer writer 饥饿和 SIGPIPE。将 socket integration tests 放入 `.serialized` suite，而不是无限放大 timeout。
- ad-hoc 签名每次重建都会改变 macOS TCC code requirement。开发候选重复授权是构建身份变化导致，不是最终产品预期；有稳定 Apple codesigning identity 后，同一 Bundle ID/requirement 的更新应复用授权。
- 同 Bundle ID 的历史 `/Applications/LifeSub.app` 与项目候选并存时，权限修改触发的系统 relaunch 可能选择历史包。必须核对 CrashReporter `procPath`，并在候选验收时避免并行注册/运行旧包。
- `CurrentDevice` set/readback 后立即读取 `AVAudioEngine.inputNode.outputFormat` 仍可能得到旧 aggregate 的 client format；真实内置麦克风在约 4 ms 内从 48 kHz 切到 44.1 kHz。必须读取所选 Core Audio hardware 的预期格式并做有界 settle，不能把“调用顺序正确”等同于 format 已稳定。
- Swift/AVFAudio 的 `installTap` format mismatch 会抛 Objective-C exception 并直接终止 helper，普通 Swift `do/catch` 无法兜住。首要防线是 tap 前格式契约；后续设备 compatibility probe 仍应考虑 exception bridge，避免异常硬件让 helper crash。
- TCC 设置页的开关为 on 不足以证明当前 ad-hoc 候选已授权。必须结合当前 app code requirement 与 TCC `AUTHREQ_RESULT`；重建后旧记录可继续显示 on，但日志会 `Failed to match existing code requirement` 并重新 prompt。
- 2026-09-01：权限拒绝前也不能急切构造 `AVAudioEngine`。真实 denied probe 证明未使用 engine 在异步析构时可能与 `AVAudioIOUnit` property listener 竞态并使 helper SIGSEGV；系统 engine 必须延迟到权限通过后的首次真实访问。
- 2026-09-01：正式 TCC 采证必须由 Finder/正常应用启动链拉起精确 bundle。Codex 终端子进程的 `responsible` 会是 `com.openai.codex`，即使 binary path 是候选 LifeSub，也不能证明 LifeSub 自身权限所有权。
# 2026-08-26 参考项目对照复核边界

- 参考项目的行为细节按 `keep / current-release-hardening / post-release / reject` 四类处置，不以功能数量或 UI 相似度作为纳入依据。
- V0.2.1 只吸收会导致伪成功、身份误导、静默停帧、不可诊断/不可恢复失败或安装版主链路无法证明的细节。
- 已完成并保持的 LifeSub 优势：不可变 raw audio、精确 UID 且无默认 fallback、单一认证 helper、anchored 文件访问、exact model/runtime、fenced Job、原子 Receipt/Revision 发布、append-only Revision、committed-only Evidence。
- 当前版必要加固归属：设置真实性 Task 1；无副作用兼容性 probe Task 3；同 UID engine recovery Task 4；诊断 Task 2；模型进度/取消/重试 Task 5；报告与硬件矩阵 Task 7；最终 packaged/installed 证据 Task 8。
- 过重项继续后置：模型缓存卸载、上下文 profile、通用行为设置、存储保留、AEC、跨源去重、batch decode、preview、normalization、Qwen 1.7B streaming、麦克风热切换。
- 原 Task 1 示例命令 `cargo test ... domain_test commands::tests` 同时传两个过滤器，Cargo 会报 `unexpected argument`；计划已拆成两条 focused 命令，不能把命令语法失败当作行为 RED。
- Task 3 原示例同样同时传 `capture_protocol_test capture_helper_test` 两个过滤器，已拆成两条 Cargo 命令。
- Probe response 是一次性 helper 控制结果，不是 capture telemetry。持久 coordinator 必须拒绝它；只有专用 one-shot reader 可以消费，避免 probe 被误注册为 source/chunk/Evidence。
- Passive `list_microphones` 必须 lazy 保持：不要在 `HelperCommandLoop` init 的默认参数中构造 `AVAudioEngine` 或 live probe。只有显式 `probe_microphone` command 才调用 prober factory。
- One-shot cancellation 必须在发送 command 前再次检查；只在 launch 前检查会导致 future 已取消后仍触发麦克风 probe/权限路径。
- Recovery 事件必须通过 microphone 专用 async signal handler 顺序 await；把同步 closure 自动提升为 async、但内部再创建独立 Task，会让 recovered 可能先于 recovering 到达 Rust 并触发 ProtocolViolation。
- Recovery teardown 后的失败路径不能再次无条件 stop/remove tap。必须记录 engine path active 状态，确保 stop/error/deinit 每个资源只清理一次。
- 配置变化发生在 startup 等待首帧期间时，recovery 需等待 initial start commit；直接并发 teardown 会让 startup 与 recovery 争用同一个 tap/engine。
- macOS `CGPreflightScreenCaptureAccess()` 只能可靠区分 granted 与 not-granted，不能无提示区分首次未决定和已拒绝。诊断合同应使用 truthful `not_granted`，不得伪造 denied/not_determined；麦克风 AVCapture 权限仍可精确区分三态。
- Diagnostics export 只允许 LifeSub data root 本身；不得输出模型 install absolute path、launch nonce、runtime raw JSON、转写正文或音频内容。Runtime/model identity 只投影 hash 与固定版本字段。
- ASR model install 首先安装 VAD；模型 UI 必须在 target download 尚未创建时投影 active/failed VAD dependency，否则用户会看到“安装中”但无任何 durable operation。
- Cancel command 只能请求 cancellation，不能同步写 cancelled 或在 Promise 返回时隐藏进度；Catalog cancelled 才是确认。
- Model controller 的 poll wait 不能使用立即 resolved Promise 运行 pending operation，否则会形成 microtask busy loop、阻塞 timer 并触发 Node OOM。测试与生产都需要有界实际等待，并在 dispose 后退出。

# 2026-09-02 UI 响应式走查

- 移动端导航不能同时固定 44px 触控目标、保留四个文字标签并假设 390px 一定容纳；应以真实 `scrollWidth/clientWidth` 验收。最窄断点使用熟悉图标时，必须同步提供 `aria-label` 和 tooltip，不能用缩小触控目标换取不溢出。
- 不要在窄屏规则中笼统把所有 `.icon-button` 设为 `width: 100%`。关闭、刷新、复制等图标工具需要稳定方形尺寸；全宽规则应只作用于明确的文字命令或具体动作容器。
# 2026-09-02 UI 二次体验走查

- 第一轮样式优化解决了控件一致性、响应式溢出和基础状态表达，但“通过验收”不等于“使用高效或视觉成熟”。后续走查必须把合规缺陷与体验债务分开记录。
- 当前最值得优先处理的前端交互债务：移动端录音控制会随内容滚出视野；时间线和词典在窄屏使用整页堆叠而非主从钻取；Session Tree 把选择与展开绑定；设置缺少 dirty 状态和关闭保护。
- 当前最明显的视觉债务：录音空闲态内容真空、词典三栏等权且边框过多、模型页状态/动作/进度混排、全局重复英文 eyebrow 和小号等宽字体过度使用。
- 下一轮优化应继续区分 V（纯视觉）、I（前端交互）和 P（需产品确认）三类，避免把视觉优化误扩展为后端或 Evidence 语义变更。

# 2026-09-03 原生 UI 几何验收

- 浏览器 Demo 与 Tauri 原生分支不是等价视觉样本。凡是由 `isTauriRuntime()` 控制的设备、权限或诊断控件，必须通过 IPC 视觉夹具或真实安装版单独覆盖。
- `scrollWidth <= clientWidth` 不能证明相邻元素没有覆盖；文字使用 `white-space: nowrap` 时，元素盒可被压缩而文字继续越界绘制。需要比较 `Range.getBoundingClientRect()` 与相邻控件矩形。
- 修复控件碰撞应调整容器分区和 track 收缩规则，不能用局部 margin 补偿。设备名可以在 select 内截断，标签、状态和图标按钮必须保持独立可见区域。

# 2026-09-03 时间线滚动边界

- 桌面工具型界面中，给内部面板设置 `overflow: auto` 之前，必须先建立 `html/body/#root -> app shell -> workspace -> page -> pane` 的确定高度链，并在每个 grid/flex 中间层保留 `min-height: 0`。
- `min-height: 100vh` 只保证最小高度，长内容仍会把应用和侧栏一起撑出视口；若 `body` 又是 `overflow: hidden`，底部导航会被无声裁切。
- 短 Demo 数据无法证明滚动容器正确。桌面验收必须注入足够长的左右栏内容，断言外层高度、底部导航可见性、`scrollHeight > clientHeight` 和实际 `scrollTop` 位移。
## 2026-10-13 0.2.2 稳定化发现

- 诊断快照列出全部模型，但其中多数是可选候选；不能把 `state != runtime_qualified` 直接累计为必修故障。权限/设备也依赖当前录制模式，摘要使用“需核对”并保留原始事实。
- 麦克风 probe 是会话内缓存的首帧事实，设备断开或选择变化后应失效；时间标记不进入持久化合同。现有按钮改为清晰的重新测试动作。
- Catalog Job 仅持久化 `queued/preparing/transcribing` 等阶段；`publishing` 和原生 helper 的恢复中/已恢复状态目前未作为可读前端事实暴露。0.2.2 不应为状态文案新增 helper 事件或假造进度。
- 桌面已提交转写 Segment 的说话人名称只由来源生成；旧按钮可以在组件内临时改名，但不会形成 Revision，退出即消失。0.2.2 将桌面来源标签设为只读，演示模式的交互保持与真实证据明确分离。
- 诊断页提供的数据目录是展示字符串；没有点击时重新验证路径锚定的 Finder 动作会引入目录替换风险，因此当前仅支持复制路径。后续若要打开目录，需要由 Core 持有的路径能力执行。
- 前端 9 月 UI 已有设置 dirty 保护、诊断导出/哈希复制、模型取消/重试、麦克风测试按钮。本轮只补差量，避免重复实现。
