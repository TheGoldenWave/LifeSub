# LifeSub Local Tool API 与 `lifesubd` 演进设计

## 1. 决策与范围

LifeSub 采用“契约先独立、进程后独立”的过渡路径：

- 阶段 C：Tauri 进程托管唯一 `CoreRuntime`、Agent IPC 与受控 UI IPC。
- 阶段 A：launchd 管理的 `lifesubd` 托管同一 `CoreRuntime`；Tauri、DeepSeek Harness 与 Gateway 都变为客户端。
- Agent 使用版本化 **Agent Tool Contract V1**；Tauri 使用版本化 **Core Application Contract V1**。两者共享 envelope、错误和 DTO primitives，但方法集合与授权矩阵独立冻结。
- Tauri Command 只能是 Application Contract 的薄映射。secondary Tauri 在阶段 C 也通过 Application Contract 连接 primary，绝不打开 SQLite、运行 migration/reconciliation 或创建 worker。
- 插件不得调用 Tauri Command、打开 SQLite、读取内部音频路径或模型目录。任何未列入两个 V1 方法表的内部函数都不是可调用 API。

阶段 C 退出 Tauri 进程仍会停止录音与 ASR。只有阶段 A 完成后，产品才能宣称“退出管理界面不影响录音与处理”。本地 IPC 只使用 Unix domain socket；不提供 TCP listener。

## 2. MVP 与 capture gate

DeepSeek Harness 的目标闭环是：

```text
start_capture -> get_capture_status -> stop_capture
  -> get_asr_job_status -> search_transcripts
  -> resolve_evidence -> open_evidence -> lifesub:// Evidence Ref
```

真实 native capture 不属于 ASR V0.2。V0.2 的 `start_capture`、`stop_capture` 和 `get_capture_status` 都稳定返回 `unsupported_capability`，`details.capability = native_capture`；不创建 operation、session、chunk、job，不触碰采集设备。完整 DeepSeek capture 闭环在 ScreenCaptureKit + AVAudioEngine 采集里程碑验收，届时才启用 durable capture operation/outbox 与完整 DeepSeek Harness 验收。ASR V0.2 通过文件导入完成真实模型闭环。

ChatGPT developer Gateway 负责 MCP 到 Agent Tool Contract V1 的黄金映射、外部认证、策略与确认。Gateway 不暴露 daemon socket，不访问 SQLite 或内部路径。Task 11 只交付映射/清洗 fixture，不实现 Gateway 服务或要求真实 Gateway 响应。

## 3. 组件与所有权

```text
Tauri primary/secondary -- Core Application Contract V1 --+
DeepSeek Harness ------- Agent Tool Contract V1 -----------+--> CoreRuntime
ChatGPT Gateway -------- Agent Tool golden mapping --------+      |-- Catalog
                                                                  |-- ModelManager
                                                                  |-- ASR Worker
                                                                  +-- future Capture
```

`CoreRuntime` 独占 writable Catalog、migration、capture/device state、模型安装、ASR worker、reconciliation、search 与 Evidence resolution。所有适配器只完成 transport、认证上下文注入、DTO 映射和 host event，不实现业务规则。

## 4. 共享协议原语

### 4.1 Envelope 与可信调用方

请求 envelope 为 `{ contract, contract_version, request_id, method, params }`；mutation 的 `params` 必含 `idempotency_key`。请求中没有 `caller_kind`、`caller_id`、`capabilities` 或任何可提升权限的字段；出现这些字段按未知字段拒绝。

transport/auth adapter 在 dispatch 前生成不可由 payload 覆盖的 `TrustedCallerContext { principal_id, kind, capabilities, auth_source }`：

- 普通 `agent.sock`：通过 current-UID peer check 后固定为 `local_agent` 和 Agent V1 最小能力。
- in-process Tauri adapter：由 host 注入 `tauri_ui` 与 Application V1 能力。
- 受控 `ui.sock`：只供 secondary Tauri/Application client；认证见第 8 节，成功后才生成 `tauri_ui`。
- Gateway adapter：完成 Gateway 自身认证后固定注入 `gateway`；Gateway payload 不能自报本地 caller。

`EvidenceOpener` 只接受 server-side trusted `tauri_ui` claim context 和 Core 内部解析出的 validated Evidence target，永不接受请求内 caller 字段、文件路径、Agent response payload 或任意 URI。

成功响应为 `{ contract, contract_version, request_id, ok: true, result }`；失败响应为 `{ contract, contract_version, request_id, ok: false, error }`。`error` 固定为 `{ code, message_key, retryable, details }`，`details` 仅允许稳定标量、ID 和 capability 名称，禁止数据库、音频、模型、socket、临时目录或 backtrace 路径。

### 4.2 共享 DTO primitives

两个公共 contract 共用 `AsrJobSummary`、`AsrErrorCode`、`ModelSummary`、`TranscriptRevisionSummary`、`TranscriptSegmentSummary`、`ProviderReceiptSummary`、`OperationSummary`、`EvidenceRef`、分页 envelope 和稳定 snake_case enum。`PendingOpenEvidenceEvent` 仅属于 internal Host Event V1，不是公共共享 DTO。`TranscriptSegmentSummary` 为 `{ segment_id, revision_id, start_ms, end_ms, text }`，时间非负且 `start_ms < end_ms`。响应仅返回 opaque ID 与 `lifesub://` URI，不返回内部文件定位信息。

`OperationSummary` 字段冻结为 `{ operation_id, kind, state, progress, result_refs, error?, created_at, started_at?, finished_at?, updated_at }`。`kind` 为稳定 snake_case operation kind；`state` 仅为 `queued | running | succeeded | failed | cancelled | recovery_required`；`progress` 为 `{ completed_units, total_units?, unit }`，数值非负且 completed 不大于已知 total；`result_refs` 仅含 allowlisted opaque `{ ref_kind, ref_id }`；`error` 使用共享稳定 error envelope，不含内部路径。异步 mutation 的初始响应必须包含 `operation_id`，客户端用 `get_operation` 观察最终状态。

共同协议错误为 `unsupported_contract_version`、`unknown_method`、`invalid_request`、`unauthorized`、`forbidden`、`request_too_large`、`deadline_exceeded`、`server_busy`、`shutting_down`、`internal_error`。业务方法只能再返回其方法表列出的错误。

## 5. Agent Tool Contract V1

这是完整的公共 Agent V1 方法集合，恰好 8 个：

| Method | Request fields | Response fields | Required capability | Method errors |
|---|---|---|---|---|
| `get_capabilities` | `{}` | `{ tools: [{ name, supported }], capabilities: [{ name, supported, reason? }] }` | `agent_discovery` | none beyond shared |
| `start_capture` | `{ idempotency_key, source: system_audio | microphone | mixed }` | future `{ operation_id, capture_id, state }`; V0.2 error | `capture_control` | `unsupported_capability`, `capture_already_active`, `idempotency_conflict`, `operation_in_progress` |
| `get_capture_status` | `{ capture_id? }` | future `{ capture_id?, state, started_at?, elapsed_ms?, last_error? }`; V0.2 error | `capture_read` | `unsupported_capability`, `capture_not_found` |
| `stop_capture` | `{ idempotency_key, capture_id }` | future `{ operation_id, capture_id, state, chunk_ids, job_ids }`; V0.2 error | `capture_control` | `unsupported_capability`, `capture_not_found`, `invalid_capture_state`, `idempotency_conflict`, `operation_in_progress` |
| `get_asr_job_status` | `{ job_id }` | `{ job: AsrJobSummary }` | `asr_job_read` | `job_not_found` |
| `search_transcripts` | `{ query, limit, cursor?, session_id?, from_ms?, to_ms? }` | `{ items: [{ revision_id, segment_id, session_id, start_ms, end_ms, snippet, score, evidence_ref }], next_cursor? }` | `transcript_search` | `invalid_cursor`, `cursor_expired`, `cursor_scope_mismatch`, `cursor_stale` |
| `resolve_evidence` | `{ evidence_ref }` | `{ revision_id, chunk_id, session_id, start_ms, end_ms, evidence_ref }` | `evidence_resolve` | `invalid_evidence_ref`, `evidence_not_found`, `evidence_revision_unavailable` |
| `open_evidence` | `{ idempotency_key, evidence_ref }` | `{ intent_id, disposition: confirmation_required | ready_for_host, expires_at }` | `evidence_open_intent` | `invalid_evidence_ref`, `evidence_not_found`, `idempotency_conflict`, `operation_in_progress` |

V0.2 capability discovery advertises `native_capture = false`。`get_capture_status` 不伪造 idle 状态，必须与 start/stop 一样返回 `unsupported_capability(native_capture)`。

`open_evidence` 的 Core mutation 创建有 TTL、绑定 immutable `requesting_principal` 与 Evidence 的 pending confirmation intent 并写 ledger；它不打开窗口。Agent/Gateway response 只含 `intent_id`、disposition 和 expiry，绝不含 claim token、内部路径或 host capability。`confirmation_required` 始终是成功 response disposition，不是 error。Core 同时经第 9.3 节 internal Host Event path 通知 authorized Tauri host；host 展示明确用户确认后，用自身 trusted `tauri_ui` identity 调用 Host Control claim，再调用 opener 并 complete/mark uncertain。没有第二个公共 Agent confirm tool 或 Application confirm method，也不得用新的隐藏 Tauri Command。`local_agent` 与 `gateway` 因而可以完整触发用户确认闭环，但不能自己 claim 或执行 opener。

## 6. Core Application Contract V1

这是阶段 C/A Tauri 管理功能所需的完整 Application-only V1 方法集合；secondary Tauri 必须经它工作，不能回退到数据库或隐藏 Command。每个 request/response 都是具名 serde DTO，未知字段拒绝：

| Method | Named request DTO and exact fields/constraints | Named response DTO and exact fields | Capability | Method errors |
|---|---|---|---|---|
| `import_audio` | `ImportAudioRequest { idempotency_key, source_bookmark, display_name }`; key 1..128 bytes，bookmark 非空，display_name 1..255 chars | `ImportAudioResponse { operation: OperationSummary }` | `audio_import` | `invalid_audio`, `source_unavailable`, `integrity_failed`, `idempotency_conflict`, `operation_in_progress` |
| `get_asr_settings` | `GetAsrSettingsRequest {}` | `GetAsrSettingsResponse { settings, etag }` | `asr_settings_read` | none beyond shared |
| `save_asr_settings` | `SaveAsrSettingsRequest { idempotency_key, settings, expected_etag }`; key 1..128 bytes，etag 必填 | `SaveAsrSettingsResponse { settings, etag }` | `asr_settings_write` | `settings_conflict`, `invalid_settings`, `model_capability_unavailable`, `idempotency_conflict`, `operation_in_progress` |
| `list_models` | `ListModelsRequest { provider?, limit, cursor? }`; provider 为已知 enum，limit 1..50 | `ListModelsResponse { models: [ModelSummary], next_cursor? }` | `model_read` | `invalid_provider`, `invalid_cursor`, `cursor_expired`, `cursor_scope_mismatch`, `cursor_stale` |
| `download_model` | `DownloadModelRequest { idempotency_key, model_id }`; key 1..128 bytes，opaque model ID 非空 | `DownloadModelResponse { operation: OperationSummary }` | `model_manage` | `model_not_found`, `model_capability_unavailable`, `model_already_installed`, `idempotency_conflict`, `operation_in_progress` |
| `cancel_model_download` | `CancelModelDownloadRequest { idempotency_key, download_operation_id }`; IDs 非空 | `CancelModelDownloadResponse { operation: OperationSummary }` | `model_manage` | `operation_not_found`, `invalid_operation_state`, `idempotency_conflict`, `operation_in_progress` |
| `delete_model` | `DeleteModelRequest { idempotency_key, model_id }`; IDs 非空 | `DeleteModelResponse { operation: OperationSummary }` | `model_manage` | `model_not_found`, `model_in_use`, `active_model_required`, `idempotency_conflict`, `operation_in_progress` |
| `enqueue_asr_job` | `EnqueueAsrJobRequest { idempotency_key, chunk_id }`; IDs 非空 | `EnqueueAsrJobResponse { operation: OperationSummary, job: AsrJobSummary }` | `asr_job_manage` | `chunk_not_found`, `chunk_integrity_failed`, `model_capability_unavailable`, `job_already_active`, `idempotency_conflict`, `operation_in_progress` |
| `retry_asr_job` | `RetryAsrJobRequest { idempotency_key, job_id }`; IDs 非空 | `RetryAsrJobResponse { operation: OperationSummary, job: AsrJobSummary }` | `asr_job_manage` | `job_not_found`, `invalid_job_state`, `model_capability_unavailable`, `idempotency_conflict`, `operation_in_progress` |
| `cancel_asr_job` | `CancelAsrJobRequest { idempotency_key, job_id }`; IDs 非空 | `CancelAsrJobResponse { operation: OperationSummary, job: AsrJobSummary }` | `asr_job_manage` | `job_not_found`, `invalid_job_state`, `already_committed`, `idempotency_conflict`, `operation_in_progress` |
| `retranscribe_chunk` | `RetranscribeChunkRequest { idempotency_key, chunk_id, settings_snapshot? }`; IDs 非空，snapshot 必须完整合法 | `RetranscribeChunkResponse { operation: OperationSummary, job: AsrJobSummary }` | `asr_job_manage` | `chunk_not_found`, `chunk_integrity_failed`, `invalid_settings`, `model_capability_unavailable`, `idempotency_conflict`, `operation_in_progress` |
| `get_operation` | `GetOperationRequest { operation_id }`; ID 非空 | `GetOperationResponse { operation: OperationSummary }` | `operation_read` | `operation_not_found` |
| `list_operations` | `ListOperationsRequest { kinds?, states?, limit, cursor? }`; kinds/states 为已知 enum，limit 1..50 | `ListOperationsResponse { operations: [OperationSummary], next_cursor? }` | `operation_read` | `invalid_operation_filter`, `invalid_cursor`, `cursor_expired`, `cursor_scope_mismatch`, `cursor_stale` |
| `list_transcript_revisions` | `ListTranscriptRevisionsRequest { session_id, limit, cursor? }`; ID 非空，limit 1..50 | `ListTranscriptRevisionsResponse { revisions: [TranscriptRevisionSummary], next_cursor? }` | `transcript_read` | `session_not_found`, `invalid_cursor`, `cursor_expired`, `cursor_scope_mismatch`, `cursor_stale` |
| `get_transcript_revision` | `GetTranscriptRevisionRequest { revision_id }`; ID 非空 | `GetTranscriptRevisionResponse { revision: TranscriptRevisionSummary, segments: [TranscriptSegmentSummary] }` | `transcript_read` | `revision_not_found` |
| `list_provider_receipts` | `ListProviderReceiptsRequest { revision_id, limit, cursor? }`; ID 非空，limit 1..50 | `ListProviderReceiptsResponse { receipts: [ProviderReceiptSummary], next_cursor? }` | `receipt_read` | `revision_not_found`, `invalid_cursor`, `cursor_expired`, `cursor_scope_mismatch`, `cursor_stale` |

Application-only V1 共 16 methods。所有异步 import/model/job mutation 都返回可由 `get_operation` 轮询的 `OperationSummary`，UI 不能只依赖易丢失 event；`operation_in_progress` error 的 details 必须含同一个 `operation_id`，随后也用 `get_operation` 观察终态。

完整 Tauri UI surface 是 **Application-only V1 + Agent V1 的 trusted UI projection**：Tauri 可通过 trusted in-process adapter 或已授权 `ui.sock` 调用 `get_capabilities`、`get_capture_status`、`get_asr_job_status`、`search_transcripts`、`resolve_evidence`、`open_evidence`；capture mutation 仍遵守 V0.2 unsupported gate。请求/响应、errors 与 Agent V1 完全复用，不能另建 Tauri-only read/open Commands。阶段 C commands 只能一一映射这两个已列方法集合；阶段 A 只替换 host。

Application list 排序冻结：`list_models` 为 `provider ASC, display_name ASC, model_id ASC`；`list_operations` 为 `created_at DESC, operation_id ASC`；`list_transcript_revisions` 为 `created_at DESC, revision_id ASC`；`list_provider_receipts` 为 `created_at DESC, receipt_id ASC`。最终 ID 是 tie-breaker；四个 list 均绑定 limit/filter/caller 的 MAC cursor，并只返回 `invalid_cursor | cursor_expired | cursor_scope_mismatch | cursor_stale` 四种 cursor error。

## 7. Cursor 语义

所有 list/search cursor 都是版本化、opaque、带 server MAC 的 payload，包含 `cursor_version`、contract/method、trusted `principal_id`、规范化 query/filter hash、固定 `limit`、keyset last tuple、`snapshot_high_watermark`、`issued_at`、`expires_at` 和 Catalog identity/epoch。客户端不得解码或修改。

- `search_transcripts` 排序固定为 `score DESC, session_start_ms DESC, revision_id ASC, segment_id ASC`；其他 list 方法也声明稳定主排序并以 opaque ID 作最终 tie-breaker。
- cursor 与 caller、method、query、filters、limit 绑定；任一改变返回 `cursor_scope_mismatch`。
- MAC/格式错误为 `invalid_cursor`，超过 15 分钟为 `cursor_expired`，不支持的 cursor version 也按 `invalid_cursor` 且 details 指明 version。
- 新增数据高于 high-watermark，不进入本次遍历；删除可令一页少于 limit，但不得重复或跳回。Catalog replacement、migration epoch 改变或 keyset anchor 无法安全继续时返回 `cursor_stale`，客户端从第一页重启。

## 8. Transport、认证与 socket lifecycle

### 8.1 安全创建与认证

runtime root 从已锚定的 app-support parent directory fd 开始，以 `openat(O_DIRECTORY|O_NOFOLLOW)` 逐段创建/打开；每段都用 `fstat` 验证 current UID、目录类型、无 group/other 权限。禁止先 path-check 再普通 `bind`。socket parent 与 lock file 都相对 anchored fd 操作，bind 前后分别用 `lstat/fstatat(AT_SYMLINK_NOFOLLOW)` 验证 node 类型、owner 与 mode；runtime dir 为 `0700`，socket 为 `0600`。

两个 endpoint：

- `agent.sock`：current UID + mandatory `getpeereid`，固定最小 `local_agent` context。
- `ui.sock`：accept 后先以 `getpeereid` 验证同 UID，再通过 `LOCAL_PEERTOKEN` 取得不可由客户端 payload 伪造的 `audit_token_t`；server 以 audit token 调用 `SecCodeCopyGuestWithAttributes` 获取 peer code object，并用 `SecCodeCheckValidity` 校验 pinned designated requirement、Team ID 与 bundle ID。pinned requirement 来自 primary 启动时对自身签名 identity 求得并与版本内置的主应用 requirement policy 交集验证，不能由 secondary 或请求提供。验证成功才生成 `tauri_ui`；同 UID 本身绝不足以提升权限。

生产签名缺失、audit token 不可用、Security.framework 查询失败、designated requirement/Team ID/bundle ID 任一不匹配都 fail closed，不授予 Application 或 opener 权限。unsigned/debug build 默认也 fail closed；只有编译为测试 harness 的专用 server 才可使用版本控制内的 ad-hoc signed test identity requirement，且该开关不能出现在 production binary。验证失败的 connection 可关闭，或重新连接 `agent.sock` 降级为固定 `local_agent`；禁止在同一 connection 上由 payload 请求升级。

请求自报 caller/capability 永远不能补偿认证失败。Gateway 使用固定 gateway context；host opener 只接受 trusted context。

### 8.2 framing、背压与取消

- data frame：4-byte big-endian length + JSON；request 最大 1 MiB，response 最大 4 MiB，超限在分配大 buffer 前拒绝。
- control frame 是独立 envelope `{ control: cancel, request_id }`，不是 Agent/Application business method。它只取消仍在执行且尚未越过方法 commit point 的 request；响应最终仍使用原 request ID。
- Host Control frame 使用独立 internal envelope `{ host_control_version: 1, request_id, method, params }`，只在 authorized `ui.sock` route 或 in-process service boundary 可达；它与 transport cancel frame、Agent envelope、Application envelope 均不混用。认证在解析/dispatch host method 前完成，普通 `agent.sock` 与 Gateway 不注册该 route。
- 每 connection 最多 8 个 in-flight request，server 全局最多 32 个；超限返回 `server_busy`。每个 connection 有 bounded input/output queue，禁止无限缓冲。
- frame read deadline 10 秒、request execution deadline 按方法定义、response write deadline 10 秒；half frame、slowloris、client 不读 response 都必须终止连接且释放 slot。

### 8.3 ownership、启动与关闭

full Core ownership lock 必须早于任何 writable Catalog open/migration、socket bind、reconciliation、import、model mutation 或 worker。secondary 失败后以 25/50/100/200/400 ms 加随机抖动重试连接 primary，总上限 2 秒；仍失败则明确报 primary unavailable，不自行打开数据库。

只有持有 ownership lock 的进程可处理 stale socket。probe 只有明确 `ENOENT`（node 已不存在）或 `ECONNREFUSED`（无人监听）才是 stale candidate；timeout、`EMFILE`、`ENFILE`、`EACCES`、`EPERM`、`ENOBUFS`、协议错误或其他不确定结果全部 fail closed，禁止 unlink。对于 `ECONNREFUSED`，lock holder 还必须用 anchored `lstat/fstatat` 复验同一个 node identity（device/inode）、current UID、socket type 和 mode 后才可 unlink；成功连接或任何 live response 一律视为 live。恶意 replacement、非 socket node、owner/mode/identity 改变均 fail closed。并发启动只有 lock winner 可 bind。

关闭顺序：停止 accept，拒绝新请求；最多 5 秒 drain；对未过 commit point 的请求发送 cancel；完成 durable recovery marker；unlink 两个 socket；关闭 Core/Catalog；最后释放 ownership lock。

## 9. Mutation、commit point 与恢复

所有 mutation 的 idempotency scope 为 `(contract, version, trusted principal_id, method, idempotency_key)`，fingerprint 是去除 key 后 canonical request 的 SHA-256。Catalog v3 的 `tool_requests` 保存 fingerprint、`in_progress | succeeded | failed`、operation ID、exact response/error、commit marker、timestamps 与 30-day expiry。不同 fingerprint 重用 key 返回 `idempotency_conflict`；相同请求并发遇到 `in_progress` 返回同一 `operation_id` 和 `operation_in_progress`，不启动第二 executor；完成后 exact replay 原 response/error。

### 9.1 SQLite-only mutations

`save_asr_settings` 与 intent issuance 在一个 `BEGIN IMMEDIATE` 内同时写业务行和 succeeded/failed replay row。job mutations `enqueue_asr_job`、`retry_asr_job`、`cancel_asr_job`、`retranscribe_chunk` 在一个 transaction 内写 job state、对应 `OperationSummary` row 和 exact replay response；accept response commit 后，operation 从 `queued/running` 跟随 job 执行，最终映射为 `succeeded/failed/cancelled/recovery_required`，因此 UI 可只凭 `operation_id` 观察完整终态。commit point 是该 transaction commit。commit 前 transport cancel 可 rollback；commit 后返回/replay durable result，cancel control 不得改写结果。crash-before-commit 无业务效果，重试重新执行；crash-after-commit-before-response exact replay。

Job cancellation 的业务 commit point 是 durable `cancel_requested_at`/final state transaction：queued/blocked 可在同 transaction 进入 `cancelled`；running 先提交 `cancelling`，worker 在 publish transaction 前再次检查 generation 与 cancel marker。若 Receipt/Revision publish transaction 已 commit，返回 `already_committed`，不能删除已发布 revision。

### 9.2 File/device/executor mutations

`import_audio`、`download_model`、`cancel_model_download`、`delete_model`，以及未来 capture start/stop，先在一个 transaction 写 `tool_requests` 和 durable `operations/outbox`，commit 后才由 executor 执行。这个 accept transaction 是“请求已提交”的 commit point，不代表外部副作用已完成；response 明确返回 operation state。executor 以 `operation_id` 幂等，每一步有 durable state/checkpoint，重启扫描 accepted/executing operations 并从可证明的 checkpoint 恢复或补偿：

- import 用 operation-owned temp、hash、atomic publish 与 DB publication checkpoint，重复执行不会创建第二 chunk/job；
- model download/delete 用 content identity、lease check、temp/rename checkpoint，重复执行不会重复安装或删除在用模型；
- future capture/device 用 outbox state `accepted -> device_starting -> active -> stopping -> sealed | failed | cancelled`，device command 带 operation ID；只有确认设备状态后推进。V0.2 unsupported 路径不创建 outbox，也不产生 device side effect。

cancel 在 accept commit 前 rollback；commit 后只追加 durable cancel intent。executor 尚未开始时转 `cancelled`；已经执行时按 state machine 停在最近安全点；已越过 irreversible/publish checkpoint 返回当前 committed state，不谎报取消。任何 stale `in_progress` 必须通过 business row/operation checkpoint 判定并 replay、恢复或标记 `recovery_required`，不能盲目重做副作用。

### 9.3 `open_evidence` host side effect

Core intent issuance 是 SQLite-only mutation，exact replay 同一 `intent_id`/expiry。host execution 使用独立、非公共的 **Host Event + Host Control Protocol V1**；它不计入 Agent 8 methods 或 Application 16 methods，也不对 `agent.sock`、Gateway 或任何未授权 principal 暴露。authorized `ui.sock` 可建立 Host Event subscription 并发送内部 control frame；primary in-process Tauri host 使用注入的 event sink 并直接调用同一个 `HostControlService`。secondary/adapter 永不直接写 Catalog。

pending intent commit 后，Core 向所有 authorized active host subscription 推送 `PendingOpenEvidenceEvent { event_id, intent_id, requesting_principal_id, requesting_principal_kind, evidence_ref, display_metadata, expires_at }`。`display_metadata` 仅含 allowlisted title/time/session label，不含原始 transcript、文件路径或 claim secret；event 绝不发往 `agent.sock` 或 Gateway。delivery capability 保存在 Core subscription state并绑定 trusted `tauri_ui` principal，不序列化给 requester；Host Control claim 由 Core 结合 subscription/in-process sink identity 验证该 capability。

event delivery 是 at-least-once、event ID 幂等。host 不在线或 event 丢失时 intent 保持 pending；authorized host 建立/恢复 subscription 后，Core 在同一 internal channel replay 尚未过期的 pending events，再切换到 live stream，无公共 list/replay method。订阅以 resume cursor/ack 去重；即使 cursor 丢失，重新发送同一 event 也不会创建新 intent。到期 recovery 将 pending 标为 `expired`，不再 replay/claim；host UI 删除对应 prompt。

Host Control Protocol V1 固定三个内部方法：

| Internal method | Internal request -> response | Authorization/errors |
|---|---|---|
| `claim_open_intent` | `{ intent_id } -> { intent_id, state: executing, expires_at }` | 仅 trusted `tauri_ui` + Core-held subscription/in-process delivery capability; `intent_not_found`, `intent_expired`, `intent_already_claimed`, `intent_consumed`, `intent_event_not_delivered`, `forbidden` |
| `complete_open_intent` | `{ intent_id } -> { intent_id, state: consumed, consumed_at }` | 必须是同一 trusted host principal 的 executing claim；`intent_not_found`, `invalid_intent_state`, `intent_claim_owner_mismatch`, `forbidden` |
| `mark_open_intent_uncertain` | `{ intent_id, diagnostic_id } -> { intent_id, state: uncertain, updated_at }` | diagnostic ID 是无路径 opaque audit ID；同 owner/executing 条件；`intent_not_found`, `invalid_intent_state`, `intent_claim_owner_mismatch`, `forbidden` |

`HostControlService` 由 CoreRuntime 串行写 Catalog v3 ledger。ledger 明确区分 immutable `requesting_principal_id/kind`（原 local_agent/gateway/tauri caller）与 `claim_principal_id`（实际 authorized `tauri_ui` host）；二者通常不同，claim 不要求相等。`claim_open_intent` 在 `BEGIN IMMEDIATE` 中以 pending state、`expires_at > now`、Evidence/requester immutable binding 未变化、calling trusted host 拥有该 intent 的 Core-held delivery capability 为 CAS 条件，原子更新 `executing`、claim principal、claim request ID、consent timestamp/metadata 与 execution lease；并发 claim 只有一个成功。相同 claim principal 对同一 executing intent 的同 request ID 重试可幂等 replay；不同 request ID/host 或已 consumed/uncertain/expired 均不得重新执行。`complete` 和 `mark_uncertain` 也分别以 executing + claim principal 为 CAS 条件，重复相同完成结果 exact replay，冲突 outcome 返回 `invalid_intent_state`。所有 transition 记录 requesting principal、claim principal、用户确认时间和 opaque audit IDs，形成可审计 consent record。

host 从 internal event 收到 pending confirmation 后提示用户；用户确认后调用 `claim_open_intent(intent_id)`，只有 CAS 成功才调用系统 opener，然后调用 `complete_open_intent`。若 opener 返回错误或 host 无法证明是否已打开，则调用 `mark_open_intent_uncertain`。若 crash 发生在 claim 后、finish 前，启动 recovery 将超过短执行 lease 的 `executing` ledger 转为 `uncertain`，绝不自动调用 opener；用户必须重新调用公共 `open_evidence` 获得新 intent 并再次确认。若 crash 在 claim commit 前，无 host side effect；若 claim commit 后但 opener 前，恢复仍保守标记 uncertain。该边界明确是 at-most-one successful claim、不是跨 OS opener exactly-once。

### 9.4 逐 mutation 冻结矩阵

| Mutation | Commit point | Replay/recovery rule |
|---|---|---|
| `start_capture` / `stop_capture` | V0.2 无 commit；future 为 operation/outbox accept transaction | V0.2 永远 unsupported 且零 device side effect；future executor 按 capture state machine 恢复 |
| `open_evidence` | pending intent + replay row transaction commit | exact replay intent ID/expiry；internal host event at-least-once replay；host crash window要求重新确认，绝不自动重开 UI |
| `import_audio` | operation/outbox accept transaction commit；chunk/job publication 是后续 checkpoint | replay 同一 operation；executor 以 temp/hash/publish checkpoint 去重 |
| `save_asr_settings` | settings + replay row transaction commit | exact replay settings/etag；commit 前 rollback，commit 后 cancel 无效 |
| `download_model` | operation/outbox accept transaction commit | replay 同一 download/operation；从 byte/hash/extract/rename/DB checkpoint 恢复 |
| `cancel_model_download` | cancel intent + replay row transaction commit | executor 未开始则 cancelled；开始后停在安全 checkpoint；已发布则返回 committed state |
| `delete_model` | operation/outbox accept transaction commit | replay 同一 operation；lease/content identity 防重复或误删 |
| `enqueue_asr_job` | job + operation + replay row transaction commit | exact replay同一 job/operation；operation 跟随 job 至终态；并发 in-progress 不创建第二 job |
| `retry_asr_job` | new generation/state + operation + replay row transaction commit | exact replay同一 generation/operation；旧 worker 由 fencing 拒绝，operation 跟随新 generation |
| `cancel_asr_job` | cancel marker/final state + operation + replay row transaction commit | queued/blocked 原子 cancelled；running cancelling；operation 追踪取消终态；publish 已 commit 则 `already_committed` |
| `retranscribe_chunk` | new job + settings snapshot + operation + replay row transaction commit | exact replay同一 job/operation；operation 跟随 job 至终态；不覆盖旧 revision |

所有 read/list/resolve/get 方法无业务 commit point且不写 `tool_requests`；transport cancel 仅停止其尚未完成的计算/IO。

## 10. Catalog v3

Catalog v3 包含 ASR V0.2 schema 与 `tool_requests`、`operations/outbox`、open-intent ledger、schema/cursor epoch。open-intent ledger 至少保存 `intent_id`、immutable requesting principal ID/kind 与 Evidence binding、`pending | executing | consumed | uncertain | expired` state、expiry、host event ID/delivery metadata、claim principal/request ID、execution lease、auditable consent timestamp、consumed/uncertain timestamps、opaque diagnostic ID 和 exact replay metadata。claim capability 保持在 Core authorized subscription/in-process state，不作为 bearer token存入 Agent response或 Host Event。migration 支持 fresh -> v3、immutable v1 fixture -> v3、immutable v2 fixture -> v3；全部在 `BEGIN IMMEDIATE` 内设置 `user_version = 3`。任何失败完整 rollback，旧 DB 保持原 bytes/`user_version`。启动 fingerprint 在并发连接、错误 `user_version`、未知表/列/index/FTS tokenizer 时 fail closed。

测试必须覆盖 fresh/v1/v2 -> v3、v2 fixture 不被测试准备过程修改、rollback、`user_version = 3`、fingerprint、两个进程并发 migration（只有 ownership winner 写）、以及已是 v3 的幂等 reopen。

## 11. Acceptance gates

- direct Core、in-process Tauri、受控 UI IPC 和普通 Agent IPC 的共享 envelope/error/DTO fixture 一致；两个公共方法集合分别完整且无隐藏 API。Host Control V1 有独立 internal golden frames，普通 Agent/Gateway 必须收到 `forbidden` 或无法路由。
- V0.2 三个 capture 方法稳定 unsupported，且 device/outbox/session/chunk/job side-effect counters 都为 0。
- Task 11 仅用 test-harness ad-hoc signed identity fixture 验证 audit-token/Security.framework requirement plumbing、authorized success 与 forged/mismatched/unsigned rejection，不宣称 production Tauri 签名验收。
- release Gate 必须用实际签名的 packaged `.app` 启动 primary 与 secondary，证明 audit token、designated requirement、Team ID、bundle ID 全匹配时才获得 `tauri_ui`，并证明 unsigned、不同 Team/bundle/requirement 的 client 被拒或只能另连 `agent.sock` 降级。
- Host Event/Control V1 测试覆盖 Agent/Gateway requester 与 Tauri claimer 分离、event 无 token/path 泄露、offline/lost-event pending replay、expiry、concurrent claim 单赢家、requester/evidence immutable binding、Core-held delivery capability、complete/uncertain 幂等与冲突、crash-after-claim recovery 为 uncertain，以及 unauthorized Agent 不能 claim、secondary/adapter 零直接 Catalog 写。
- SQLite-only mutation 覆盖 concurrent in-progress、changed fingerprint、crash-before/after commit、restart exact replay；executor mutation覆盖所有 checkpoint、cancel 前后和 recovery。
- 两个真实 Tauri 进程在隔离 HOME 启动：primary 独占 DB/socket/worker，secondary 经受控 Application V1 完成 read/save/list/import/job 操作，且 DB open/migration/reconciliation/worker/model/import side-effect counters 证明 secondary 全为 0。
- socket 测试覆盖 symlink/path replacement、malicious live replacement、`ENOENT`/`ECONNREFUSED` stale、timeout/`EMFILE`/`EACCES` fail-closed、并发启动、half frame、oversize、slow read、slow write、connection/global limits、startup retry 与 ordered shutdown。
- cursor 测试覆盖 tie、caller/query/limit binding、tamper、expiry、data insert/delete、migration/reset stale。
- Gateway acceptance 仅检查 Agent V1 golden MCP mapping 与 sanitizer fixture；Task 11 不启动 Gateway。
- search/resolve 永不触发 opener；公共 `open_evidence` 只返回 intent ID/disposition/expiry，validated target 只在 Core/authorized host execution boundary 内解析。
- C 阶段文档和 UI 不宣称 process-independent recording；全程无 TCP listener。

## 12. Deferred

- Agent 编辑或删除 transcript/Evidence。
- 公网 daemon networking和复杂多用户授权。
- launchd 安装、升级、签名与 Gateway 外部认证的具体实现；contract、local IPC 与 trusted caller 规则已经冻结。
- native capture 与完整 DeepSeek capture milestone；届时必须使用本规格的 durable operation/outbox，不得新增旁路。
