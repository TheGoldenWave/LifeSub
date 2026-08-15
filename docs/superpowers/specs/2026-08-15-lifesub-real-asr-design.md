# LifeSub 真实本地 ASR V0.2 设计

## 1. 决策摘要

LifeSub V0.2 将演示 ASR 替换为三个可切换的真实本地 Provider：

- 阿里开源模型：SenseVoiceSmall INT8。
- OpenAI 开源模型：Whisper Tiny、Base、Small。
- 阿里开源模型：Qwen3-ASR 0.6B INT8，以及通过独立设备 Gate 后启用的 Qwen3-ASR 1.7B 高质量档。

SenseVoice、Whisper 与 Qwen3-ASR 0.6B 统一使用 sherpa-onnx 1.13.5 Rust API，并静态链接到 Tauri Core。Qwen3-ASR 1.7B 只有在获得可复现的 sherpa-onnx 资产或经单独批准的 Apple Silicon 原生适配器后才启用；V0.2 不为它引入 Python Sidecar。该方案不启动本地 HTTP 服务，不向云端发送音频，也不复制 GPLv3 参考项目代码。

## 2. 方案比较

| 方案 | 优点 | 缺点 | 结论 |
|---|---|---|---|
| sherpa-onnx 统一运行时 | 同一 Rust API 支持 SenseVoice/Whisper/Qwen3-ASR 0.6B；静态链接；错误与模型管理统一 | Qwen3-ASR 1.7B 暂无同等成熟的预转换发布包 | 采用，0.6B 首发，1.7B 受 Gate 控制 |
| sherpa-onnx + whisper.cpp | 可分别使用每个模型的成熟运行时 | 两套 FFI、构建、模型管理和诊断体系 | 延后到性能证据证明必要时 |
| FunASR + faster-whisper Python Sidecar | 模型覆盖广，原型快 | 包体、Python 环境、进程恢复、签名复杂 | 不采用 |

## 3. 参考项目取舍

### OpenWhispr

借鉴：Provider 路由是纯配置解析；路由结果不携带密钥；模型卡同时表达推荐、大小、下载和就绪状态；失败不绕过用户选择；批量转写和重试共享同一 Provider 语义。

不照搬：Electron 主进程结构、Zustand 设置存储、云端 Provider、通用听写和 Agent 功能。

审阅基线为 commit `b3a8368b732ddb57ef68827cd65d2bb3dc0477b5`（MIT）。仅阅读 README、`transcriptionRoute.ts`、`LocalWhisperPicker.tsx`、`settingsStore.ts` 和相关文件路径清单，提取可观察行为与边界。

### TypeWhisper

借鉴：引擎与模型分离；模型可用、已配置和可恢复是不同状态；文件转写具有 pending/loading/transcribing/done/error/cancelled 生命周期；选择切换后重置不兼容模型；本机数据和凭据使用受控 Host Service。

不照搬：Swift 插件实现、工作流、文本插入、插件市场和 GPLv3 代码。

审阅基线为 commit `ea7920263a0d81fcf9713a0f3c2798ef3bc7592b`（GPL-3.0）。仅阅读 README、`FileTranscriptionViewModel.swift`、`ModelManagerService.swift` 与 Plugin README。LifeSub 不复制参考项目源码、测试、资源、UX 文案或文件结构；实现者只能使用本设计记录的行为需求，新增代码必须具有独立来源记录。

sherpa-onnx 运行时基线为 tag `v1.13.5`、commit `3dc7c569f31ca2cd4a20ed6f7db780327e6714c5`（Apache-2.0）。依赖升级需要重新执行真实模型、构建和打包 Gate。

## 4. 组件边界

```text
React UI
├── AsrSettingsView
├── ModelCardList
├── AsrJobStatus
└── RetranscribeCommand

Core Application API
├── get/save_asr_settings
├── list/download/cancel/delete_asr_model
├── enqueue/cancel/retry_asr_job
└── retranscribe_record

Adapters
├── Tauri Commands
└── Versioned Local Tool API / Unix Socket

Rust Core
├── asr/settings.rs          设置与校验
├── asr/model_registry.rs    固定 manifest
├── asr/model_manager.rs     下载、校验、安装、删除
├── asr/audio.rs             解码、单声道转换、重采样
├── asr/vad.rs               语音区间
├── asr/provider.rs          Provider 接口与结果类型
├── asr/sense_voice.rs       SenseVoice 配置
├── asr/whisper.rs           Whisper 配置
├── asr/qwen3_asr.rs         Qwen3-ASR 配置与能力 Gate
├── asr/job.rs               状态机、取消、恢复
└── asr/service.rs           事务编排与 revision 发布
```

每个模块只有一个责任。UI 不拼装模型文件路径，Tauri Commands 与 Local Tool API 不实现识别逻辑，Provider 不写数据库，Model Manager 不创建 Transcript Revision。`CoreRuntime` 是 Catalog、reconciliation、录音状态与 ASR Worker 的唯一运行时所有者；适配器不得自行打开 SQLite。

Agent/IPC 边界遵循 `docs/superpowers/specs/2026-08-16-lifesub-local-tool-api-design.md`。当前 V0.2 采用 contract-first 的 C 阶段，未来 `lifesubd` 只替换进程宿主，不改变 Core 或工具语义。

## 5. 核心领域类型

```rust
enum AsrProviderKind {
    SenseVoice,
    Whisper,
    Qwen3Asr,
}

struct AsrSettings {
    provider: AsrProviderKind,
    model_id: String,
    language: AsrLanguage,
    num_threads: u16,
    vad_enabled: bool,
    auto_transcribe_imports: bool,
    options: AsrProviderOptions,
}

enum AsrProviderOptions {
    SenseVoice { use_itn: bool },
    Whisper { task: WhisperTask },
    Qwen3Asr,
}

enum AsrJobState {
    Queued,
    BlockedModel,
    Preparing,
    Transcribing,
    Succeeded,
    Failed,
    Cancelled,
}

struct ProviderReceipt {
    job_id: String,
    chunk_id: String,
    provider: AsrProviderKind,
    model_id: String,
    manifest_version: String,
    archive_sha256: String,
    required_file_hashes_json: String,
    model_source_json: String,
    vad_model_id: Option<String>,
    vad_manifest_version: Option<String>,
    vad_archive_sha256: Option<String>,
    vad_required_file_hashes_json: Option<String>,
    runtime_version: String,
    runtime_build_id: String,
    parameters_json: String,
    input_sha256: String,
    started_at: DateTime<Utc>,
    finished_at: DateTime<Utc>,
    data_destination: DataDestination,
    outcome: ProviderOutcome,
}
```

持久化枚举使用稳定 snake_case 字符串。设置、Job 和 Receipt 不能直接保存 Rust Debug 文本。

## 6. 持久化 Schema 与 V0.1 迁移

当前 Catalog 是 `user_version = 0` 的无版本 Schema。V0.2 必须引入显式 `PRAGMA user_version` 迁移器，所有步骤在 `BEGIN IMMEDIATE` 中执行。识别规则：

1. `user_version = 0` 且没有用户表：直接创建 v2。
2. `user_version = 0` 且 schema fingerprint 精确匹配 V0.1 的 `sessions/revisions/segments/chunks/segment_search` 表、列、主外键和 FTS tokenizer：按逻辑 v1 迁移到 v2。
3. `user_version = 0` 但 fingerprint 不匹配：视为未知或损坏库，fail closed，不执行任何 DDL。
4. `user_version = 2`：正常打开并校验 v2 fingerprint。
5. 其他版本：拒绝打开，提示版本不兼容。

迁移完成后最后写入 `PRAGMA user_version = 2`。迁移失败整体回滚，应用保持旧库不变并显示升级错误。

v2 的新增表与索引如下；实际 SQL 名称和约束视为 Contract，计划与测试必须保持一致：

```sql
CREATE TABLE asr_settings (
  singleton_id INTEGER PRIMARY KEY CHECK(singleton_id = 1),
  provider TEXT NOT NULL CHECK(provider IN ('sense_voice', 'whisper', 'qwen3_asr')),
  model_id TEXT NOT NULL,
  language TEXT NOT NULL,
  num_threads INTEGER NOT NULL CHECK(num_threads >= 1),
  vad_enabled INTEGER NOT NULL CHECK(vad_enabled IN (0, 1)),
  auto_transcribe_imports INTEGER NOT NULL CHECK(auto_transcribe_imports IN (0, 1)),
  provider_options_json TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE model_installations (
  model_id TEXT PRIMARY KEY,
  provider TEXT NOT NULL CHECK(provider IN ('sense_voice', 'whisper', 'qwen3_asr', 'vad')),
  manifest_version TEXT NOT NULL,
  archive_sha256 TEXT NOT NULL,
  install_dir TEXT NOT NULL UNIQUE,
  state TEXT NOT NULL CHECK(state IN ('ready', 'corrupt', 'deleting')),
  installed_at TEXT NOT NULL,
  last_error_code TEXT
);

CREATE TABLE model_downloads (
  id TEXT PRIMARY KEY,
  model_id TEXT NOT NULL,
  manifest_version TEXT NOT NULL,
  archive_sha256 TEXT NOT NULL,
  state TEXT NOT NULL CHECK(state IN
    ('queued', 'downloading', 'verifying', 'installing', 'succeeded', 'failed', 'cancelled')),
  downloaded_bytes INTEGER NOT NULL DEFAULT 0,
  expected_bytes INTEGER NOT NULL,
  temp_path TEXT,
  error_code TEXT,
  error_summary TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE UNIQUE INDEX model_downloads_one_active_model
ON model_downloads(model_id)
WHERE state IN ('queued', 'downloading', 'verifying', 'installing');

CREATE TABLE asr_jobs (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL REFERENCES sessions(id),
  chunk_id TEXT NOT NULL REFERENCES chunks(id),
  provider TEXT NOT NULL CHECK(provider IN ('sense_voice', 'whisper', 'qwen3_asr')),
  model_id TEXT NOT NULL,
  manifest_version TEXT NOT NULL,
  archive_sha256 TEXT NOT NULL,
  required_file_hashes_json TEXT NOT NULL,
  model_source_json TEXT NOT NULL,
  vad_model_id TEXT,
  vad_manifest_version TEXT,
  vad_archive_sha256 TEXT,
  vad_required_file_hashes_json TEXT,
  parameters_json TEXT NOT NULL,
  input_sha256 TEXT NOT NULL,
  fingerprint TEXT NOT NULL,
  state TEXT NOT NULL CHECK(state IN
    ('queued', 'blocked_model', 'preparing', 'transcribing', 'succeeded', 'failed', 'cancelled')),
  attempt_count INTEGER NOT NULL DEFAULT 0,
  claim_generation INTEGER NOT NULL DEFAULT 0,
  max_attempts INTEGER NOT NULL DEFAULT 3 CHECK(max_attempts BETWEEN 1 AND 10),
  available_at TEXT NOT NULL,
  claimed_by TEXT,
  lease_expires_at TEXT,
  cancel_requested_at TEXT,
  error_code TEXT,
  error_summary TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE UNIQUE INDEX asr_jobs_one_active_fingerprint
ON asr_jobs(fingerprint)
WHERE state IN ('queued', 'blocked_model', 'preparing', 'transcribing');

CREATE INDEX asr_jobs_claimable
ON asr_jobs(state, available_at, lease_expires_at);

CREATE TABLE provider_receipts (
  id TEXT PRIMARY KEY,
  job_id TEXT NOT NULL UNIQUE REFERENCES asr_jobs(id),
  chunk_id TEXT NOT NULL REFERENCES chunks(id),
  provider TEXT NOT NULL,
  model_id TEXT NOT NULL,
  manifest_version TEXT NOT NULL,
  archive_sha256 TEXT NOT NULL,
  required_file_hashes_json TEXT NOT NULL,
  model_source_json TEXT NOT NULL,
  vad_model_id TEXT,
  vad_manifest_version TEXT,
  vad_archive_sha256 TEXT,
  vad_required_file_hashes_json TEXT,
  runtime_version TEXT NOT NULL,
  runtime_build_id TEXT NOT NULL,
  parameters_json TEXT NOT NULL,
  input_sha256 TEXT NOT NULL,
  started_at TEXT NOT NULL,
  finished_at TEXT NOT NULL,
  data_destination TEXT NOT NULL CHECK(data_destination = 'local_device'),
  outcome TEXT NOT NULL CHECK(outcome = 'succeeded')
);

CREATE TABLE revision_receipts (
  revision_id TEXT NOT NULL REFERENCES revisions(id),
  receipt_id TEXT NOT NULL UNIQUE REFERENCES provider_receipts(id),
  PRIMARY KEY(revision_id, receipt_id)
);
```

v2 还需通过 `ALTER TABLE` 增加：

- `chunks.session_offset_ms INTEGER NOT NULL DEFAULT 0`。
- `chunks.duration_ms INTEGER`，旧数据允许为空，首次成功解码后补齐。
- `chunks.integrity_state TEXT NOT NULL DEFAULT 'available'`，取值为 `available | corrupted | missing`。
- `chunks.last_error_code TEXT`、`chunks.last_error_at TEXT`。
- `revisions.provenance_status TEXT NOT NULL DEFAULT 'legacy_unverified'`，取值为 `legacy_unverified | verified_local_asr | manual`。
- `segments.chunk_id TEXT REFERENCES chunks(id)`。
- `segments.chunk_start_ms INTEGER`、`segments.chunk_end_ms INTEGER`。
- `segments.session_start_ms INTEGER`、`segments.session_end_ms INTEGER`。

V0.1 revision 不伪造 Receipt，保留原 `provider` 字符串并标记 `legacy_unverified`；旧 Segment 的 `start_ms/end_ms` 继续按 session-relative 解释。新 revision 必须有至少一个 `revision_receipts` 关系，新 Segment 必须有 chunk 与两套时间坐标。迁移测试使用真实 v1 fixture，验证所有原记录、搜索结果和 Evidence URI 仍可读取。

`segments.start_ms/end_ms` 在 v2 保留为 session-relative 兼容镜像：所有新写入必须满足 `start_ms = session_start_ms`、`end_ms = session_end_ms`。现有 Markdown、搜索和 Evidence 消费者继续读取兼容字段，新增代码使用显式字段；测试锁定两者一致，后续 Contract 大版本才能移除旧字段。

Chunk reconciliation 将缺失文件标记 `missing`，hash 不一致或无法读取标记 `corrupted`。非 `available` Chunk 不允许创建或 claim 新 Job，已有 active Job 转 `failed/input_unavailable`。音频 Evidence URI 返回 `corrupted` 状态且拒绝音频读取；既有 Transcript Revision 继续可读，但 provenance 显示来源不可重新验证，不自动删除或改写文本。

## 7. Chunk 来源与时间坐标

一个 ASR Job 只处理一个不可变 Audio Chunk，Receipt 与 Job 一一对应。Revision 可聚合一个或多个成功 Receipt，`revision_receipts` 保存完整输入集合。每个新 Segment 同时保存：

- `chunk_id`。
- chunk-relative `chunk_start_ms/chunk_end_ms`。
- session-relative `session_start_ms/session_end_ms`，等于 `chunk.session_offset_ms + chunk-relative time`。

V0.2 的文件导入 Session 只有一个 Chunk，`session_offset_ms = 0`。未来长时录音可由聚合器把多个 Chunk 的成功结果发布为一个 session revision，时间不会在每个 Chunk 重置为零。Receipt 的 `input_sha256` 必须等于关联 Chunk 在执行前重新计算的 hash。

## 8. Model Manifest

Manifest 作为版本控制下的静态数据，至少包含：

- `model_id`、Provider、显示名和模型版本。
- 明确的 `availability`：`Installable(ArtifactSpec)` 或 `ExperimentalUnavailable { reason, unmet_gates }`。
- `Installable` 条目包含下载 URL、归档大小、SHA-256、解压后的必需文件及可选单文件 hash。
- `ExperimentalUnavailable` 条目不伪造资产字段，且不能保存为生效设置、下载、创建 Job 或构造 Provider。
- 支持语言、默认参数和推荐硬件。
- 上游项目、模型卡、许可证和 notice。

首发条目：

| Model ID | Provider | 归档大小 | 定位 |
|---|---|---:|---|
| `sense-voice-small-int8-2024-07-17` | SenseVoice | 163,002,883 B | 默认中文/中英混合模型 |
| `whisper-tiny` | Whisper | 116,204,861 B | 快速验证和低资源 |
| `whisper-base` | Whisper | 207,557,382 B | 默认 Whisper 平衡档 |
| `whisper-small` | Whisper | 639,387,718 B | 更高质量、较高资源 |
| `qwen3-asr-0.6b-int8-2026-03-25` | Qwen3-ASR | 878,702,423 B | 52 种语言/方言覆盖，首发 Qwen 档 |
| `qwen3-asr-1.7b` | Qwen3-ASR | 待固定 | 高质量实验档；通过资产与设备 Gate 后启用 |

Qwen3-ASR 0.6B 使用 sherpa-onnx 发布资产 `sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25.tar.bz2`，SHA-256 为 `393f8a14e2f5fb96746aaab342997a40641001fbd5bf9592a080a8329178ee96`；执行文件至少包含 `conv_frontend.onnx`、`encoder.int8.onnx`、`decoder.int8.onnx` 与完整 `tokenizer/` 目录。Qwen3-ASR 1.7B 的 Hugging Face 原始权重不能直接交给当前 Rust Provider；在固定可执行 ONNX/MLX 资产、转换来源和真实 Apple Silicon 证据前，manifest 只能声明不可安装的实验条目。

```rust
enum ModelAvailability {
    Installable(ArtifactSpec),
    ExperimentalUnavailable {
        reason: &'static str,
        unmet_gates: &'static [ModelGate],
    },
}
```

`ModelLookup` 必须分别暴露 `selectable`、`installable` 与 `executable` 能力。1.7B 可在模型列表中 `selectable = true` 以展示说明，但在 Gate 前 `installable = false`、`executable = false`。设置草稿可以暂存该选择用于查看信息，保存为生效 ASR 设置、下载命令、Job 创建和 Provider Factory 必须返回稳定的 `model_capability_unavailable`；防御性 Provider 校验不得成为唯一控制点。

实现前必须下载每个发布模型归档并冻结 SHA-256。Silero VAD 也作为独立 `provider = vad` 的 manifest 条目管理，包含版本、模型 hash 和运行参数。若上游同名资产发生变化，必须发布新的 manifest version 和 model ID；禁止在原 ID 下替换资产。

Receipt 必须快照 manifest version、归档 hash、所有必需模型文件 hash、模型转换来源和运行时 build ID。旧 Evidence 不依赖当前 manifest 解释模型身份。

`model_source_json` 至少固定上游仓库、上游 commit/tag、原始模型 ID、转换工具仓库与 commit、转换参数和下载资产 URL。VAD 开启时，Job 与 Receipt 必须同时快照 VAD model ID、manifest version、archive hash 和必需文件 hash；VAD 关闭时这些字段必须全部为空。

## 9. 模型安装事务

```text
检查 manifest
  -> 检查磁盘空间
  -> 下载到 downloads/<id>.part
  -> 计算并验证 SHA-256
  -> 解压到 models/.staging/<uuid>/
  -> 验证必需文件
  -> fsync 关键文件与目录
  -> 写入 immutable install marker
  -> atomic rename 到 models/asr/<provider>/<id>/<manifest>-<hash>/
  -> 事务更新 model_installations 的 active install_dir
```

安装目录版本化且不可原地替换。下载与安装进度写入 `model_downloads`；`model_installations` 只记录已激活安装。失败下载保留 `failed` 状态和稳定错误码，完整性错误使用 `model_integrity_failed`，网络错误使用 `model_download_failed`。

新目录 rename 成功但 SQLite 更新前崩溃时，启动 reconciliation 根据 install marker 校验并补登记或移入 quarantine；数据库指向缺失目录时标记 corrupt。启动时把旧进程遗留的 active download 转为 `failed/recovery_required`，清理过期 `.part` 与 staging，保留最近一次可用安装直到新版本正式激活。

归档解压必须拒绝绝对路径、`..`、symlink、hardlink 和越界目标；限制文件数量、单文件大小和总展开大小。空间检查同时覆盖归档、staging 与已安装旧版本。下载只跟随 HTTPS，重定向后的 host 必须位于 manifest allowlist。取消、网络失败、hash 错误或解压失败都不能损坏已安装版本。删除模型使用逻辑锁，等待正在使用该模型的任务释放后再删除。

## 10. 不可变音频提交与校验

现有 `fs::write(final_path)` 后再插入 Catalog 的流程不能作为 V0.2 输入保证。导入必须改为：同目录临时文件写入并在写入过程中计算 hash，`sync_all` 文件，原子 rename 到内容寻址或唯一最终路径，`fsync` 父目录，再提交 Chunk metadata。文件系统与 SQLite 无法共享事务，因此启动 reconciliation 清理未被 Catalog 引用的临时/最终孤儿，并将 Catalog 指向缺失文件的 Chunk 标记为 `missing`；文件存在但无法读取或 hash 不一致时标记为 `corrupted`。

ASR 开始前重新读取最终文件并计算 SHA-256，与 Chunk metadata 及 Job snapshot 同时比较；不一致则 Job 失败为 `input_integrity_failed`，不得执行模型。

## 11. 音频与时间范围

现有导入入口允许 WAV、MP3、M4A、AAC、FLAC 和 OGG。ASR 工作副本必须：

1. 从不可变 Audio Chunk 解码。
2. 转换为 `f32` 单声道 PCM。
3. 重采样到 Provider 要求的采样率。
4. 在 VAD 开启时形成有开始/结束采样位置的语音区间。
5. 对每个语音区间独立执行识别。

VAD 是三个 Provider 的共同时间轴。SenseVoice 与 Qwen3-ASR 不伪造模型 token 时间戳；Transcript Segment 的时间范围来自 VAD 区间。关闭 VAD 时，首个 Segment 覆盖完整音频时长。

标准工作格式为 16 kHz `f32` 单声道。多声道按每帧算术平均下混，并在写入 Provider 前 clamp 到 `[-1, 1]`。重采样器必须暴露或补偿 delay；时间换算以原始解码 frame 索引为权威，开始时间向下取整、结束时间向上取整，并校验 `0 <= start < end <= duration`。

VAD speech padding 为 200 ms。连续语音区间最大 25 秒；超过时优先在最小能量点切分，否则硬切，所有核心区间单调且不重叠。上下文 padding 不能扩大对外 Evidence 时间范围或产生重复 Segment。Provider 每完成一个最多 25 秒窗口后检查取消；同步 native inference 不宣称支持窗口内抢占。

音频解码优先使用纯 Rust、可打包方案。实现计划必须先用所有声明格式的 fixture 验证解码库覆盖率；不支持的格式从 UI allowlist 移除，不能继续宣称支持后在 ASR 阶段失败。

## 12. Provider 接口

```rust
trait AsrProvider {
    fn kind(&self) -> AsrProviderKind;
    fn model_id(&self) -> &str;
    fn transcribe(
        &self,
        audio: AudioSlice<'_>,
        request: &AsrRequest,
        cancellation: &CancellationToken,
    ) -> Result<AsrText, AsrError>;
}
```

Provider 只接收已经解码的音频切片和验证后的请求。它不读取全局设置、不选择 fallback、不写 Catalog、不更新 UI，也不决定 revision 编号。

Provider Factory 根据 Job 的设置快照和已安装 manifest 构建实例。未知 Provider、模型不属于 Provider 或模型损坏时 fail closed。

## 13. Job、租约与事务

```text
Audio Chunk committed
  -> ASR Job queued with settings snapshot
  -> validate model
  -> decode and resample
  -> VAD
  -> transcribe slices
  -> assemble non-empty segments
  -> transaction:
       insert provider_receipt
       insert transcript_revision
       insert transcript_segments
       update FTS5
       mark job succeeded
```

ASR Worker 在恢复或 claim 之前，必须取得应用数据目录中的进程级 OS advisory file lock `asr-worker.lock`，并持有到进程退出。未取得锁的第二个应用实例可以读取状态，但不得执行 recovery、claim、模型安装或 reconciliation。只有持锁实例可以把其他 boot ID 视为 stale，从而避免并行实例互相接管。

Worker 使用 compare-and-swap claim：仅当 Job 为 `queued`、`cancel_requested_at IS NULL`、`available_at <= now` 且 lease 为空或已过期时，单条 `UPDATE ... WHERE ...` 同时设置 `state = 'preparing'`、`claimed_by`、`lease_expires_at`，并增加 `attempt_count` 与 `claim_generation`；受影响行数必须为 1。不存在“已 claim 但仍为 queued”的中间状态。Worker 保存本次返回的 `claim_generation` 作为 fencing token。`claimed_by` 包含每次进程启动生成的 `boot_id` 与 worker ID，lease 时长 30 秒，worker 每 5 秒或每个阶段边界续租。

所有续租、`preparing -> transcribing`、失败、取消和恢复转换都必须使用 `WHERE id = ? AND claimed_by = ? AND claim_generation = ?`；影响行数不是 1 时，当前 Worker 已失去所有权，必须停止并丢弃内存结果。续租还要求当前 lease 未过期，禁止旧 Worker 在被接管后复活。

启动时，任何 `claimed_by.boot_id` 不等于当前 boot ID 的 `preparing/transcribing` Job 立即视为 stale，不等待旧 lease 到期；同一进程内只在 lease 过期后接管。若收到取消请求则转 `cancelled`；否则当 `attempt_count < max_attempts` 时回到 `queued`。`max_attempts = 3` 表示总共最多 3 次 claim：第 1 次失败后退避 5 秒，第 2 次失败后退避 30 秒，第 3 次失败后直接转 `failed/recovery_retry_exhausted`。

取消 sweeper 将尚未 claim 的 `queued/blocked_model` Job 直接转 `cancelled`。模型安装完成时，只有 `cancel_requested_at IS NULL` 的 `blocked_model` Job 才转 `queued`。不存在 `failed_recoverable` 状态。

取消请求先写 `cancel_requested_at` 并立即反馈 UI。成功发布使用 `BEGIN IMMEDIATE`：先以 `id + claimed_by + claim_generation + state = transcribing + cancel_requested_at IS NULL` 条件确认 fencing token，再插入 Receipt、Revision、Segment 和 FTS，最后以相同 token 将 Job 更新为 `succeeded`；任一条件更新影响行数不是 1 时整体回滚并丢弃结果。事务提交前已存在取消请求则转 `cancelled` 且不发布 revision；成功事务提交后到达的取消请求不回滚 Evidence，Job 保持 `succeeded`。任何事务前错误只更新 Job 失败状态。事务内错误整体回滚，不发布部分 revision。

默认只有一个 ASR Worker，避免同时驻留多个大模型。后续性能数据证明有收益后再增加并发。

Qwen3-ASR 1.7B 的启用 Gate 固定在 macOS 14+、Apple Silicon、16 GB unified memory 基线。使用同一中文、英文和中英混合 fixture 时，中文 CER 与英文 WER 均不得超过 20%，混合关键短语召回率必须为 100%，且三项质量不得劣于 0.6B 对应结果；5 分钟 fixture 的 RTF 必须 `<= 1.0`，进程峰值 RSS 必须 `<= 6 GiB`，UI heartbeat 与取消阈值沿用发布 Gate。证据必须记录芯片型号、内存、macOS 版本、运行时/模型/fixture hash、峰值 RSS 与 RTF；任一条件缺失时保持不可安装。

## 14. 设置体验

设置页使用适合选项切换的控件，而不是静态说明行：

- Provider：SenseVoice / Whisper / Qwen3-ASR 分段控件。
- Model Cards：名称、说明、大小、许可、推荐标识、安装/下载/错误状态和操作图标。
- Language：Provider 支持语言菜单。
- Threads：数值步进器。
- VAD、自动转写、SenseVoice ITN：开关。
- Whisper task：transcribe / translate 分段控件。
- Qwen3-ASR：展示模型档位、语言覆盖、预计内存与实验性 Gate 状态；不可执行的 1.7B 不显示下载动作。
- Advanced：模型目录、运行时版本、最近错误。

下载过程中卡片尺寸固定，进度、错误和长模型名不能推动布局跳动。移动端宽度下设置项改为单列，按钮保持可触达尺寸。所有颜色、间距和字体使用现有 Design Token。

## 15. 重转写

记录详情提供“重新转写”命令。确认界面必须显示 Provider、模型、语言和预计模型状态。成功后追加 revision，失败不改变当前 revision。

Job 使用提交时的设置快照。用户在任务运行期间修改设置，不会改变当前任务；下一任务使用新设置。

## 16. 错误模型

稳定错误码至少包括：

- `model_not_installed`
- `model_integrity_failed`
- `model_download_failed`
- `insufficient_disk_space`
- `unsupported_or_corrupt_audio`
- `input_integrity_failed`
- `input_unavailable`
- `invalid_provider_parameter`
- `provider_initialization_failed`
- `transcription_failed`
- `cancelled`
- `recovery_required`

用户文案与错误码分离。日志允许保存诊断上下文，但不保存音频内容，不在 UI 展示完整用户路径。

## 17. 测试策略

### 单元测试

- Settings 验证、Provider/模型归属和参数兼容性。
- Model Manifest 结构与 checksum 格式。
- Job 状态机、重启恢复、取消和幂等约束。
- v1 到 v2 Schema 迁移、legacy provenance 和失败回滚。
- CAS claim、lease 过期、退避、重试上限和取消/提交竞态。
- claim_generation fencing：过期 Worker 不能续租、改变状态或发布 Receipt/Revision。
- VAD 区间到 Transcript Segment 时间换算。
- Receipt、Revision 和 Job 成功事务原子性。

### 集成测试

- 本地 HTTP fixture 模拟下载中断、错误 hash 和重试。
- 各声明音频格式的解码和重采样。
- 长连续语音的 25 秒切窗、时间单调性和边界校验。
- 使用真实 SenseVoice 模型转写固定中文 WAV。
- 使用真实 Whisper 模型转写固定英文或中英混合 WAV。
- 同一 Audio Chunk 使用两个 Provider 生成两个 revision。

真实模型测试允许在普通快速测试中通过环境变量定位已缓存模型，但发布 Gate 必须执行，不能只以 mock Provider 代替。

### UI 与端到端测试

- Provider 切换后模型和专属参数更新。
- 模型下载、取消、失败、就绪和删除状态。
- 导入音频后 Job 状态到成功 revision。
- 重新转写后 revision 切换与旧结果保留。
- 桌面、平板和移动视口无文本溢出或控件重叠。

### 量化验收 fixture

发布仓库保存小型、可再分发的固定 WAV fixture 及 manifest，manifest 固定文件 SHA-256、人工 transcript、语言、标注语音区间、关键短语和许可。至少包含 20 至 40 秒普通话、英语和中英混合各一条。性能 Gate 另包含确定性生成并提交 hash 的 5 分钟 WAV：按 `zh.wav -> en.wav -> zh-en.wav -> 500 ms 16 kHz mono silence` 顺序循环拼接，达到 300 秒后在 frame 边界截断；生成器版本、源 fixture hash 和结果 hash 全部写入 manifest。

指标计算协议固定如下：

1. 文本先做 Unicode NFKC，并将拉丁字母转小写。
2. CER：移除 Unicode 标点与全部空白，以 Unicode grapheme cluster 为 token；数字不做单词/阿拉伯数字互转。
3. WER：Unicode 标点替换为空格，连续空白折叠，以空格切 token；数字不做语义归一化。
4. 关键短语：对 transcript 与 phrase 执行 NFKC、小写、标点转空格和空白折叠后，要求 phrase token 序列为连续精确子序列；每条必需 phrase 均命中才为 100%。
5. 文本指标使用所有预测 Segment 按 `session_start_ms` 排序后的拼接文本。
6. 时间指标要求预测 Segment 数量与 fixture 标注区间数量一致；数量不一致直接失败。数量一致时按时间顺序一一配对，对每对计算 start/end 绝对误差，汇总全部边界的中位数与最大值。

- SenseVoice 普通话 fixture 的归一化 CER 不高于 20%。
- Whisper 英语 fixture 的归一化 WER 不高于 20%；中英混合关键短语召回率为 100%。
- Qwen3-ASR 0.6B 必须运行 `qwen3-0.6b-zh`、`qwen3-0.6b-en`、`qwen3-0.6b-zh-en`，普通话 CER 与英语 WER 均不高于 20%，混合关键短语召回率为 100%。
- Segment 起止相对人工标注的中位误差不高于 500 ms，最大误差不高于 1.5 秒。
- enqueue 命令在 Audio Chunk 提交后 500 ms 内返回 Job；ASR 在独立 blocking worker 执行。
- Playwright 运行 ASR 时 100 ms UI heartbeat 的 P95 漂移不高于 250 ms。
- 取消请求 500 ms 内显示 `cancelling`，基线模型任务在 30 秒内进入 `cancelled`。
- 进程终止后重新启动，基于新 boot ID 在 5 秒内完成 stale claim 的确定性恢复。

### 构建与打包

- `cargo test --no-default-features`
- `cargo check --features desktop`
- 前端单测、生产构建和 Playwright。
- 采用 sherpa-onnx crate 的 `static` feature；`otool -L` 不得出现未打包的 sherpa-onnx/onnxruntime 动态库。
- 启动时调用运行时版本 API，版本与 Receipt 中的 `runtime_version/runtime_build_id` 一致。
- bundle 签名、DMG 重打包和镜像内运行验证。

## 18. 发布 Gate

V0.2 只有在以下证据全部存在时才完成：

1. SenseVoice、Whisper 与 Qwen3-ASR 0.6B 均通过固定 fixture 的 CER/WER、关键短语和时间误差阈值，Qwen 的三个固定 scenario ID 不得缺失，不能仅以非空文本通过。
2. 三种 Provider 结果都包含稳定时间范围、来源和 Provider Receipt。
3. 设置切换实际改变后续 Job 的 Provider 和模型。
4. 重转写创建新 revision，旧 revision 未被覆盖。
5. 下载、hash 错误、模型缺失、音频损坏、取消和重启恢复均通过测试。
6. 所有声明支持的音频格式均有真实解码 fixture；否则同步收窄 UI allowlist 和文档。
7. macOS Apple Silicon bundle 可在无开发环境的情况下运行静态 ASR runtime；`otool -L`、运行时版本和签名证据齐全。
8. 第三方许可证、模型来源和 hash 在应用与发布材料中可查。
9. v1 Catalog 迁移 fixture 通过，旧 revision 保持可读且明确标记 `legacy_unverified`。
10. Chunk、Job、Receipt、Revision 和 Segment 的来源关系可以从任一新 Segment 完整追溯到输入音频 hash。

## 19. 明确延后

- DashScope/OpenAI API 等云端 ASR及其密钥管理。
- 自动 fallback、Provider 竞速与自动质量选择。
- 实时流式字幕和原生双路采集。
- Speaker Diarization、声纹、LLM 校对和高级词库。
- Windows/Linux 发布包。

## 20. 关联文档

- PRD：`docs/prd/lifesub-real-asr-v0.2/PRD.md`
- 产品简报：`docs/context/product-initiated/lifesub-real-asr-v0.2/10_brief/product-brief.md`
- 上位架构：`docs/superpowers/specs/2026-08-15-lifesub-evidence-platform-design.md`
- 参考研究：`docs/research.md`
