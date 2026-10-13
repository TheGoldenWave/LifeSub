# LifeSub 0.2.2 性能基线

日期：2026-10-13。目标机：Apple M4、24 GiB 统一内存、macOS 15.6.1、arm64。产品源码 `main@bbdb632a7defdf43f9961d45d9b522849583731e`。本文件只记录实测及其边界；未测值不按通过处理。

## 固定输入与复测命令

| 测量 | 固定输入与命令 | 判定 |
|---|---|---|
| 诊断大目录 | `scripts/with-sherpa-runtime.sh cargo test --locked --manifest-path src-tauri/Cargo.toml --features desktop diagnostics_snapshot_high_file_count_baseline -- --ignored --nocapture` | 4000 个各 16 B 的音频/模型文件；校验分类字节数并记录完整/耗时 |
| 模型进程首轮与次轮 | 对同一 0.2.2 `.app/Contents/MacOS/lifesub-asr-gate`，相同 `--model-dir`、固定 fixture、相同 Gate 参数分别执行两次，报告写入独立 JSON | 每次退出码、SenseVoice/Whisper RTF、peak RSS、模型和 runtime identity；首轮/次轮只代表进程与 OS 文件缓存状态，不冒充重启后真正冷机 |
| 麦克风首帧 | 安装版诊断中对同一精确 UID 执行首帧测试，导出 `selected_microphone.probe.elapsed_ms` 与 frame count | `result=compatible`、UID/readback 一致、frames>0 |
| 桌面启动与取消 | 对同一安装版在目标机执行有时间戳的启动至首个可交互窗口、开始后立即取消至停止确认流程 | 保存计时方式、两端事件和结果；没有可观测两端时明确记为未测 |

## 2026-10-13 已完成量测

- 诊断整快照：冷 `21.727ms`、热 `21.251ms`；4000 个文件，音频与模型分类均为 `32,000 B`，断言通过，`complete=true`。这是 `AppState::diagnostics_snapshot` 的固定权限/设备夹具，包含模型投影与存储遍历；不包含真实 helper 权限查询或 WebView 渲染。未发现需要在 0.2.2 中引入有界扫描合同的量化理由。
- 0.2.1 历史参考，**不能替代 0.2.2 候选验收**：最终 20/20 报告中的内置麦克风 probe 首帧 `230ms`、蓝牙 FreeBuds probe `407ms`；同一候选打包 Provider Gate 中 SenseVoice RTF `0.0365`/RSS `470.4 MiB`，Whisper RTF `0.0702`/RSS `661.4 MiB`。固定来源见 `output/asr-v0.2/native-capture-v3-6f2e/reports/` 与 `output/asr-v0.2/packaged-provider-gate.json`。

## 待填候选结果

- 0.2.2 候选 app/helper/Gate/DMG 哈希、签名与固定 fixture 对应身份。
- 0.2.2 模型首轮/次轮 RTF、RSS、耗时；真实目标机首帧、启动和取消确认延迟。
- 若候选的目标机操作无法取得双端时间戳，记录缺口与原因，不补造数字。
