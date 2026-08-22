# LifeSub ASR 设备资格检测与云端回退设计

## 1. 决策

LifeSub 为每个本地 ASR 模型建立“静态设备预检 + 一次真实短试跑”的设备资格状态。检测采用事件驱动缓存，不按天或按月重复执行。用户可在全局设置中选择默认 ASR 策略，并在单次导入或重转写时覆盖；云端自动回退只在用户预先授权的 Provider 和条件内发生。

本设计是 LifeSub 真实本地 ASR V0.2 之后的独立里程碑。V0.2 继续保持本地-only 发布边界；本设计复用其 Model Manifest、ModelLookup、RuntimeQualifier、ASR Job、Provider Receipt 与不可变 Audio Chunk。

## 2. 用户策略

全局默认与单次任务覆盖使用同一枚举：

| 策略 | 行为 |
|---|---|
| `local_only` | 只使用明确指定且当前设备已支持的本地模型；不可用时任务阻塞，绝不上传 |
| `automatic` | 优先使用用户选择的本地模型；满足预授权回退条件时切换到指定云端 Provider |
| `cloud_only` | 直接使用用户选择的云端 Provider |
| `exact_provider` | 严格使用任务指定 Provider/模型；除非任务快照显式允许 fallback，否则不替换 |

隐私尺度由用户控制。设置保存：允许的云端 Provider、凭据引用、允许上传的任务类型、自动回退条件和默认策略。任务创建时保存完整策略快照，后续全局设置变化不改变运行中任务。

## 3. 模型说明合同

每个当前及未来本地 ASR manifest 必须包含：

- `resource_tier = lightweight | balanced | high_quality`
- 下载字节数与安装后预计占用
- runtime/backend：CPU、Metal 或其他加速器
- 最低与推荐内存
- 预计峰值内存和 RTF 档位
- 支持语言、优势场景、速度/质量倾向
- 静态设备要求和 qualification protocol version

UI 主列表只显示资源档位、一句话说明和当前设备状态；高级详情展示具体内存、RTF、runtime identity、检测时间和失败原因。数值来自 manifest，不硬编码在前端。

首发说明基线：

| 模型 | 资源档位 | 精简说明 |
|---|---|---|
| SenseVoice Small INT8 | lightweight | 中文与中英混合优先，资源消耗低，适合日常快速转写 |
| Whisper Tiny/Base | lightweight | 多语言、低内存，适合旧设备与快速草稿 |
| Whisper Small | balanced | 多语言准确率更好，耗时和内存较高 |
| Qwen3-ASR 0.6B INT8 | balanced | 中文及混合语言表现较强，兼顾速度和质量 |
| Qwen3-ASR 1.7B | high_quality | Candle/Metal 高质量档，约 4.71 GB，当前正式基线为 M4/24GB |

## 4. 设备资格状态

每个模型按当前设备独立保存状态：

```text
unknown
checking
compatible_unverified
trial_running
supported
supported_degraded
unsupported_device
unsupported_runtime
trial_failed
stale
```

`supported_degraded` 表示能够完成试跑，但 RTF、峰值内存或内存压力接近 manifest 阈值；模型仍可手动使用，`automatic` 优先选择更稳定的已支持模型。任何模型失败只影响该模型，不污染其他模型，也不触发本地模型间静默替换。

资格缓存键：

```text
device_fingerprint
+ os_compatibility_generation
+ app_asr_runtime_identity
+ model_bundle_identity
+ qualification_protocol_version
```

键完全一致时复用检测结果，不设置时间型过期。

## 5. 检测触发与频率

- 安装或首次启用模型：运行静态预检；通过后执行一次 5-15 秒真实试跑。
- 日常启动、打开设置和普通任务：读取缓存，不重新试跑。
- App 普通升级：只有 ASR runtime identity 变化才让相关结果 `stale`。
- 模型 manifest、bundle、runtime、macOS compatibility generation、CPU 架构或 Metal device 变化：只让受影响模型 `stale`。
- 可用内存的日常波动不让资格失效；任务开始前另做即时资源余量检查。
- 单次运行失败不撤销支持状态；连续两次 OOM、Metal 初始化失败、严重超时或模型加载失败时暂停自动选择并提示重新试跑。
- 用户可随时手动“重新检测”或“重新试跑”。

试跑只使用随应用发布的无隐私固定短音频，不读取用户录音。检测模型加载、有效输出、启动耗时、RTF、峰值 RSS 与系统内存压力；试跑执行在隔离 worker 中，超时或崩溃不能拖垮 UI/Core。

## 6. 判定规则

静态预检至少检查：OS、架构、总内存、加速后端、磁盘、模型完整性、runtime identity 和 manifest 条件。

真实试跑通过条件：

- 模型完整加载并正常退出；
- 输出非空且固定短语校验通过；
- 未发生 OOM 或 critical memory pressure；
- RTF、峰值 RSS 和启动时间不超过 manifest 硬阈值；
- 实际 runtime/device identity 与 manifest 和安装资格 marker 一致。

接近软阈值标记 `supported_degraded`。失败必须保存稳定 reason code、无敏感路径诊断 ID 和可操作建议。

## 7. 自动选择与云端回退

自动选择严格按任务策略快照执行：

1. 校验请求的本地模型是否 `supported | supported_degraded`，安装与 runtime identity 是否仍有效。
2. 做即时资源余量检查；资源暂时不足时可等待、提示切换，或按授权条件进入云端回退。
3. 执行本地 Job。单次普通失败保留支持状态；满足用户授权的回退原因时创建云端 Job generation。
4. 云端 Provider 未配置、凭据失效或不在授权范围时进入 `blocked_provider`，保留音频和任务供后续重试。

可授权回退原因：本地设备不支持、模型未安装、当前资源不足、本地执行失败、本地预计耗时超过用户阈值。`local_only` 和未授权任务永远不得上传。

云端音频上传前，任务状态明确显示实际 Provider。所有本地/云端切换写入 Receipt：

```text
requested_provider
requested_model
actual_provider
actual_model
fallback_reason
audio_left_device
policy_snapshot
device_qualification_ref
```

## 8. 组件边界

- `DeviceProfiler`：采集稳定设备指纹与静态资源能力，不决定业务 fallback。
- `ModelQualificationService`：管理资格缓存、静态预检和隔离试跑。
- `ResourcePreflight`：任务开始前检查瞬时内存/磁盘余量，不改长期资格状态。
- `AsrRoutingPolicy`：根据任务策略快照、资格状态和授权选择 Provider。
- `CloudAsrProvider`：实现云端请求、凭据引用、取消和稳定 Receipt；不得读取全局设置绕过任务快照。
- `CoreRuntime`：仍是 Job、Catalog、资格状态和 Receipt 的唯一写入者。

## 9. 错误与恢复

- 资格 worker 崩溃或超时：该次试跑失败，Core 保持可用并允许重试。
- 两次连续资源类失败：模型暂停参与 automatic，状态提示重新试跑。
- 环境变化：标记 `stale`，不静默沿用旧结论。
- 云端网络或鉴权失败：Job 保留输入与 Provider 诊断，可按原策略重试；不得改写本地资格结果。
- fallback 发生后不得覆盖原失败 generation；最终 Revision 关联实际成功 Provider Receipt 和完整尝试链。

## 10. 验收门禁

- 环境未变化时，重启和日常使用不重复试跑。
- 模型/runtime 更新只使相关缓存失效。
- 每个模型独立判定；OOM、超时、Metal 初始化失败和 worker 崩溃均有确定状态。
- `local_only`、未授权和 exact-provider 任务绝不上传。
- `automatic` 在授权条件下正确回退，并在 UI/Receipt 中披露。
- 本地全部不可用且云端可用时任务成功；云端未配置时进入 `blocked_provider`。
- Receipt 能证明请求/实际模型、回退原因、资格结果和音频是否离开设备。

