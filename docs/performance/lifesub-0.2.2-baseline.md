# LifeSub 0.2.2 性能基线

日期：2026-10-13。目标机：Apple M4、24 GiB 统一内存、macOS 15.6.1、arm64。最终候选产品源码 `main@5584bab51a170c956f9ded28d0593bd761d2a020`。本文件只记录实测及其边界；未测值不按通过处理。

## 固定输入与复测命令

| 测量 | 固定输入与命令 | 判定 |
|---|---|---|
| 诊断大目录 | `scripts/with-sherpa-runtime.sh cargo test --locked --manifest-path src-tauri/Cargo.toml --features desktop diagnostics_snapshot_high_file_count_baseline -- --ignored --nocapture` | 4000 个各 16 B 的音频/模型文件；校验分类字节数并记录完整/耗时 |
| 模型进程首轮与次轮 | 对同一 0.2.2 `.app/Contents/MacOS/lifesub-asr-gate`，相同 `--model-dir`、固定 fixture、相同 Gate 参数分别执行两次，报告写入独立 JSON | 每次退出码、SenseVoice/Whisper RTF、peak RSS、模型和 runtime identity；首轮/次轮只代表进程与 OS 文件缓存状态，不冒充重启后真正冷机 |
| 麦克风首帧 | 安装版诊断中对同一精确 UID 执行首帧测试，导出 `selected_microphone.probe.elapsed_ms` 与 frame count | `result=compatible`、UID/readback 一致、frames>0 |
| 桌面启动与取消 | 对同一安装版在目标机执行有时间戳的启动至首个可交互窗口、开始后立即取消至停止确认流程 | 保存计时方式、两端事件和结果；没有可观测两端时明确记为未测 |

## 2026-10-13 已完成量测

- 诊断整快照：冷 `21.727ms`、热 `21.251ms`；4000 个文件，音频与模型分类均为 `32,000 B`，断言通过，`complete=true`。这是 `AppState::diagnostics_snapshot` 的固定权限/设备夹具，包含模型投影与存储遍历；不包含真实 helper 权限查询或 WebView 渲染。未发现需要在 0.2.2 中引入有界扫描合同的量化理由。
- 最终候选：App `3f72dde30c27afa1659b669aca4c1424d5f305cd024e7c790a3e6e866b9b5f26`、helper `5c7b4672fc9da848d900526337d2932efba0b0c8e9196991586acd60949e8e01`、Gate `c49bf64ca22be1857bee8b5e75251b690562776c3fc8dc14985246c05a913e0d`、DMG `34de71df121398949fd5759b56f7e0879f7d4959b066607f32607c6d8756d76e`。源码归档 SHA-256 `069061bcc7dd8b74afebbeac013bfebfcbe4f3c705312aeb6c61fd1e6971d7d7`。DMG checksum、nested strict signature、arm64、版本 0.2.2 与包内可执行文件 byte equality 均通过。
- 模型 Gate 对同一 `c49bf64c…` 可执行文件连续运行两次，均对固定 fixture 完整通过；见 `output/asr-v0.2/0.2.2-gate-run1.json` 和 `0.2.2-gate-run2.json`，两份都绑定最终候选代码提交、归档 digest、模型/runtime/fixture 身份。首轮墙钟 `17.95s`，SenseVoice RTF `0.05866` / RSS `476.3 MiB`，Whisper RTF `0.10898` / RSS `631.7 MiB`；次轮墙钟 `17.50s`，SenseVoice RTF `0.05687` / RSS `529.9 MiB`，Whisper RTF `0.09087` / RSS `705.7 MiB`。两轮 RTF 均低于 1，结果无 fallback。这里的“首轮/次轮”不能解释成机器重启后的真正冷/热模型加载差值。
- 0.2.1 历史参考，**不能替代 0.2.2 候选验收**：最终 20/20 报告中的内置麦克风 probe 首帧 `230ms`、蓝牙 FreeBuds probe `407ms`；同一候选打包 Provider Gate 中 SenseVoice RTF `0.0365`/RSS `470.4 MiB`，Whisper RTF `0.0702`/RSS `661.4 MiB`。固定来源见 `output/asr-v0.2/native-capture-v3-6f2e/reports/` 与 `output/asr-v0.2/packaged-provider-gate.json`。

## 待填候选结果

- 0.2.2 候选真实目标机首帧、启动和取消确认延迟，以及候选身份下原生采集场景报告。
- 若候选的目标机操作无法取得双端时间戳，记录缺口与原因，不补造数字。
