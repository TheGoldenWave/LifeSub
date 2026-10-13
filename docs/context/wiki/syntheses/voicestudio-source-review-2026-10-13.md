# VoiceStudio 源码对照：LifeSub 可借鉴的机制与迭代边界

> 审阅日期：2026-10-13。参考仓库：[debpalash/VoiceStudio](https://github.com/debpalash/VoiceStudio)，本地只读参考目录 `.sources/reference-projects/VoiceStudio`，固定 `main@36862c1157a25db7f7b885efba50a072ddf70e45`。这是源码与测试的静态审阅，未安装 VoiceStudio、下载权重或运行其模型；未初始化与本次音频路径无关的 `omnivoice-gallery` 子模块。LifeSub 的生产现状仍以 [过程记录](../../../prd/lifesub-real-asr-v0.2/.artifacts/process.md) 和 [工作区状态](../../../workspace-status.md) 为准。

## 结论

VoiceStudio 应列为 **高优先级参考项目**，重点参考长时间使用时的可诊断性、模型运行风险隔离、音频信号检查、存储可见性和许可证据表达。它的产品中心是语音生成、配音和听写；当前桌面产品使用 Electron + React、Python/FastAPI 后端及 Rust 原生桥接，早期 Tauri 桌面壳已退役。LifeSub 不应因它的功能广度改用 Electron/Python，或把 TTS、克隆、配音和语音执行放进近期版本。

VoiceStudio 应用源码声明 **AGPL-3.0-only**；随仓库的 OmniVoice 包和下载的模型/声音资产另有许可。以下仅以行为与验证思路为参考，不复制其 AGPL 源码、测试、界面文案或目录结构。任何直接代码复用须先完成兼容性与义务审查。参见固定提交的 [LICENSE-NOTICE](https://github.com/debpalash/VoiceStudio/blob/36862c1157a25db7f7b885efba50a072ddf70e45/LICENSE-NOTICE.md)。

## 整体代码地图与产品差异

| VoiceStudio 层 | 源码入口与实际职责 | 对 LifeSub 的判断 |
|---|---|---|
| 桌面与 UI | `electron/src/main/` 管理桌面采集、后端监督、IPC；`electron/src/renderer/src/features/` 承载多工作区和设置 | 参考状态反馈与故障路径；不迁移桌面框架 |
| 语音后端 | `backend/api/routers/` 暴露本地 API；`backend/services/` 管理 ASR、模型、存储、音频处理及配音 | 参考小范围机制；LifeSub 的 Rust Core/Catalog 仍是事实源 |
| 原生桥接 | `native/desktop-bridge/` 处理快捷键和听写输出 | 属于听写产品路径，不是 LifeSub 的录音/引用主路径 |
| 模型与资产 | `backend/services/model_licenses.py`、`model_lifecycle.py`、`subprocess_asr.py` 等 | 用于补充许可与运行风险的审阅清单；LifeSub 已有更强的精确 manifest、hash 与 Receipt |
| 质量与验收 | `tests/test_storage_report.py`、`tests/test_audio_quality.py`、`backend/tests/test_asr_transcribe_timeout.py`、Electron 组件/主进程测试 | 可参考负例和重复验证方式，不能将其测试结果视为 LifeSub 验收 |

VoiceStudio 的本地 HTTP/WebSocket/MCP 是“语音服务和听写输出”接口；LifeSub 的目标是能回到原始音频的 Evidence Ref、授权读取和撤回状态。前者的 `partial/final` 文本不能直接成为 LifeSub 已提交的 Revision。参见 VoiceStudio [接口说明](https://github.com/debpalash/VoiceStudio/blob/36862c1157a25db7f7b885efba50a072ddf70e45/docs/speech-platform.md)。

## 借鉴矩阵

| 优先级 | VoiceStudio 源码证据 | LifeSub 当前对照 | 计划处置与采用门槛 |
|---|---|---|---|
| **近期测量** | [`storage_report.py`](https://github.com/debpalash/VoiceStudio/blob/36862c1157a25db7f7b885efba50a072ddf70e45/backend/services/storage_report.py) 对目录扫描设置时限、缓存、`complete=false`；API 将扫描放在工作线程 | `src-tauri/src/diagnostics.rs::directory_bytes` 同步递归计数，`get_diagnostics` 已在阻塞线程，但扫描本身没有时间/文件数上限，也没有结果完整性标记 | 0.2.2 S4 增加“大量 Chunk/模型文件时诊断刷新耗时”基线与固定样本。若出现实际卡顿或超时，单独评估有界扫描及“部分结果”合同；不在 S1/S2 凭空新增容量承诺。后续 Task 11 的只读存储投影优先处理此问题。 |
| **后续可靠性重点** | [`subprocess_asr.py`](https://github.com/debpalash/VoiceStudio/blob/36862c1157a25db7f7b885efba50a072ddf70e45/backend/services/subprocess_asr.py) 把可能原生崩溃的 ASR 引擎隔离为可重启子进程，并区分超时、EOF、子进程失败；[`asr_backend.py`](https://github.com/debpalash/VoiceStudio/blob/36862c1157a25db7f7b885efba50a072ddf70e45/backend/services/asr_backend.py) 明言超时取消 future 不等于杀死运行中的原生线程 | `src-tauri/src/asr/native_engine.rs` 在 LifeSub 进程内的线程调用 Provider；Job fencing、取消和原子发布已有，但进程级 native 崩溃仍会影响桌面进程 | S4 后先做故障注入/测量决策，不直接移植 sidecar。只有观察到原生崩溃、不可终止的超时或长录音期间的进程风险，才另立 ASR 隔离方案；验收须证明录音不中断、失败 Job 可恢复、模型/输入身份不漂移、取消和 RSS/RTF 可接受。新进程边界不进入 0.2.2。 |
| **后续质量候选** | [`audio_quality.py`](https://github.com/debpalash/VoiceStudio/blob/36862c1157a25db7f7b885efba50a072ddf70e45/backend/services/audio_quality.py) 有界、只读检测空音频、静音、削波、非法样本及相对音量；[`audio-quality.tsx`](https://github.com/debpalash/VoiceStudio/blob/36862c1157a25db7f7b885efba50a072ddf70e45/electron/src/renderer/src/features/clone/audio-quality.tsx) 将告警时间连接到回放 | LifeSub 已有 Chunk hash/时长/来源完整性、首帧探测和时间锚点，但没有面向用户的静音/削波片段提示 | 在真实会议样本中确认“音频有文件但关键原话不可听”的比例后，设计只读、按需、限时检查；告警链接到精确 Chunk/时间，不修改原音频、不生成伪置信度或自动删除。与会议质量研究并行，不作为 0.2.2 发布门槛。 |
| **发布治理重点** | [`model_licenses.py`](https://github.com/debpalash/VoiceStudio/blob/36862c1157a25db7f7b885efba50a072ddf70e45/backend/services/model_licenses.py) 区分声明的许可类别、精确 revision/文档/资产证据和 `not_verified`，明确一般下载路径不受“已审阅”标记自动保证 | LifeSub manifest 已固定来源、revision、bytes、SHA-256、SPDX；模型页展示 SPDX，但 SPDX 字段本身不能证明权重、转换资产、输出或再分发权利均已核对 | 在引入新模型或下一次正式发布前，补一份逐资产许可/notice 审阅记录，区分“上游声明”“已验证事实”“未知”。先治理文档和发布检查；若要在 UI 展示更细状态，另行设计状态来源，不能凭许可证字符串推断许可已通过。 |
| **条件研究** | [`diarization_native.py`](https://github.com/debpalash/VoiceStudio/blob/36862c1157a25db7f7b885efba50a072ddf70e45/backend/services/diarization_native.py) 对 Sortformer turn 校验采样范围、截断最多一帧尾部 padding，并在取消/超时后终止子进程；其 v1 路径只接受最多 120 秒音频 | LifeSub 要覆盖 30 分钟以上会议，当前匿名 speaker 推理未交付 | 可参考 turn 边界校验与失败隔离的验收项；该 120 秒实现不能直接作为长会议能力。先以真实会议样本评测 speaker 错归、重叠和跨 Chunk 对齐，再选候选模型/窗口策略；匿名 speaker 不等于真实身份。 |
| **已覆盖，保持** | VoiceStudio 模型阶段、诊断复制和后端监督提供工作流参考 | LifeSub 已有 durable 下载字节/阶段/取消/重试、`runtime_qualified`、私有诊断导出、精确输入/模型/VAD Receipt、Job lease/fencing、append-only Revision 和双源 Chunk | 0.2.2 只改善既有状态的可读性。不要重复建一套模型管理器或因“对齐参考”弱化原始音频、身份与原子发布。 |

## 对当前迭代计划的具体调整

1. **0.2.2 不变更范围和启动状态。** S1 先盘点 9 月已完成 UI；S2 继续解决状态真实性；S3 复用现有动作；S4 除既定启动/首帧/RTF/RSS/取消指标外，增加诊断扫描延迟与完整性检查，并记录冷/热模型的内存与响应差异。所有结果只进入报告；问题成立后按原停止线决定是否另立修复。
2. **稳定化后先做离线失败归因。** 在授权的长会议样本上把“找不到”“听不清/静音/削波”“模型错字”“双源重复”“等待或崩溃”“存储压力”分开计数。这决定下一项功能，而不是 VoiceStudio 的功能清单。
3. **优先准备两个技术决策门。** 若故障归因指向进程级 ASR 风险，先评估隔离；若指向不可听的输入，先评估只读音频质量提示。二者都要保持原音频与现有 Evidence 语义，单独设计、测试和真实机验收。
4. **Task 11 存储首步强化。** 先给可重复且明确完整性的用量快照；不能把超时/不可读目录算成精确总量。自动清理仍需删除图与撤回设计。
5. **模型/声纹/外部接口保持独立 Gate。** 新模型先核对许可与资产链；匿名说话人先过长音频质量；MCP/远程语音服务先过 LifeSub 授权和 Evidence Contract。不引入 VoiceStudio 的自动 Provider fallback、语音克隆、配音或远程开启录音。

这份审阅仅改变参考优先级和计划中的测量/决策顺序，**没有启动 0.2.2、修改产品代码、替换模型、构建或安装 LifeSub**。
