# LifeSub Local Tool API 与 `lifesubd` 演进设计

## 1. 决策

LifeSub 采用“契约先独立、进程后独立”的过渡路径：

- 当前阶段 C：从 Tauri 中抽出不依赖 Tauri 的 `CoreRuntime` 与版本化 Local Tool API；Tauri 进程暂时托管唯一 Core 实例和本机 IPC。
- 目标阶段 A：由 launchd 管理的独立常驻服务 `lifesubd` 托管同一 `CoreRuntime`；Tauri、DeepSeek Harness 与 Gateway 都变为客户端。
- 不采用 B 作为长期架构。任何插件不得直接调用 Tauri Command、打开 SQLite、读取内部音频路径或模型目录。

C 阶段退出 Tauri 进程仍会停止录音与 ASR。只有完成 A 后，产品才能宣称“退出管理界面不影响录音与处理”。

## 2. MVP 闭环

DeepSeek Harness MVP 必须验证：

```text
Agent start_capture
  -> get_capture_status
  -> stop_capture
  -> get_asr_job_status
  -> search_transcripts
  -> resolve_evidence
  -> open_evidence (explicit local side effect)
  -> answer with lifesub:// Evidence Ref
```

ChatGPT App MVP 复用同一工具语义。由于 ChatGPT 不能直接连接 Mac Unix socket，开发者模式 Gateway 负责 MCP 到 Local Tool API 的映射、认证和策略执行；Gateway 不暴露 daemon socket，也不能访问 SQLite 或内部文件路径。

## 3. 组件边界

```text
                  versioned Local Tool Contract
                              |
         +--------------------+--------------------+
         |                    |                    |
  Tauri Adapter       Unix Socket Adapter    Future Process Host
         |                    |                    |
         +--------------------+--------------------+
                              |
                         CoreRuntime
                  capture / ASR / search / evidence
                              |
                  single Catalog + single Worker

DeepSeek Harness ------ Unix Socket Adapter
ChatGPT Gateway ------- MCP mapping -> Unix Socket Adapter
```

`CoreRuntime` owns:

- the only writable Catalog connection policy;
- capture state and capture device ownership;
- the singleton ASR worker lock, recovery and reconciliation;
- model installation and active model leases;
- search, Evidence resolution and stable errors.

Adapters translate transport and host events only. They do not implement business rules or open the database independently.

## 4. Tool Contract V1

The initial contract contains task semantics, not CRUD:

| Tool | Side effect | Result identity |
|---|---|---|
| `start_capture` | yes | `capture_id`, optional queued `job_id` |
| `get_capture_status` | no | capture state and progress |
| `stop_capture` | yes | sealed capture/chunk IDs and queued jobs |
| `get_asr_job_status` | no | stable job state, progress and error code |
| `search_transcripts` | no | ranked snippets with revision and Evidence Ref |
| `resolve_evidence` | no | revision, chunk, exact time range and `lifesub://` URI |
| `open_evidence` | yes, local UI | accepted/confirmation-required/unsupported |

Every response includes `contract_version = 1`, a request correlation ID, stable snake_case states/errors, and no raw SQLite path, audio path, model path or sensitive diagnostic path.

Mutating requests require an idempotency key. The Core stores the operation fingerprint and result so retries cannot create two captures, two stop operations or duplicate retranscription jobs. Reusing a key with different parameters fails closed.

`resolve_evidence` is read-only. `open_evidence` is a separate local side effect and requires an explicit caller capability or user confirmation. A remote Gateway may return confirmation-required, but search must never open a window automatically.

## 5. Transport And Security

The contract is transport-independent Rust DTOs and application services. C/A local transport uses a Unix domain socket under the app data runtime directory:

- socket and parent directory are current-user only (`0600` socket, restrictive directory permissions);
- server verifies peer UID where macOS APIs permit;
- protocol envelope carries contract version and request ID;
- bounded request and response sizes, timeouts and cancellation are mandatory;
- no TCP listener is enabled by default.

The ChatGPT developer Gateway is a separate authenticated component. It maps MCP tools to the same V1 contract, applies authorization and confirmation policy, and connects locally to the socket. The daemon is never exposed directly to LAN or public networks.

## 6. Lifecycle

### Phase C

- Tauri startup creates one `CoreRuntime`, acquires `asr-worker.lock`, then performs model/audio/job reconciliation.
- Tauri Commands and local IPC share references to that runtime.
- Closing a window may keep the process in the menu bar; quitting the process ends capture and work.

### Phase A

- launchd starts `lifesubd` and owns restart policy.
- `lifesubd` owns `CoreRuntime`, Catalog and worker locks.
- Tauri contains no writable Catalog and connects through the V1 client.
- Migration from C to A changes the process host, not tool semantics or persistence ownership.

## 7. Acceptance Gates

- Tauri adapter, direct Core tests and local IPC return identical states and `AsrErrorCode` values.
- A repeated `start_capture` idempotency key returns the original `capture_id` and never starts a second capture.
- Concurrent Tauri and Harness reads do not create another Catalog writer or ASR worker.
- Search results from every adapter carry the same revision, chunk, time range and `lifesub://` Evidence Ref.
- Gateway responses never include internal database, audio or model paths.
- `open_evidence` is never triggered by `search_transcripts` or `resolve_evidence`.
- C-stage docs and UI do not claim process-independent recording.

## 8. Deferred

- Agent editing transcripts or Evidence.
- Evidence deletion and bulk retention tools.
- Public daemon networking.
- Complex multi-user authorization.
- Automatic rollout of the launchd `lifesubd` host before the V1 contract and ASR Gate are stable.
