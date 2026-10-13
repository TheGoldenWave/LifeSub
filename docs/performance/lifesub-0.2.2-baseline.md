# LifeSub 0.2.2 性能基线

日期：2026-10-13。目标机：Apple M4、24 GiB 统一内存、macOS 15.6.1、arm64。当前候选构建源码 `main@28a284aea356b1cf2d153e545ed33bcd0dc6f080`。本文件只记录实测及其边界；未测值不按通过处理。

## 固定输入与复测命令

| 测量 | 固定输入与命令 | 判定 |
|---|---|---|
| 诊断大目录 | `scripts/with-sherpa-runtime.sh cargo test --locked --manifest-path src-tauri/Cargo.toml --features desktop diagnostics_snapshot_high_file_count_baseline -- --ignored --nocapture` | 4000 个各 16 B 的音频/模型文件；校验分类字节数并记录完整/耗时 |
| 桌面启动到可见窗口 | `python3 scripts/measure-macos-startup.py --output output/asr-v0.2/0.2.2-startup-window.json` | 同一包、每次隔离 HOME，重复 3 次；以进程启动至首个屏幕上 layer-0 窗口计时，不代表 UI 可交互 |
| 模型进程首轮与次轮 | 对同一 0.2.2 `.app/Contents/MacOS/lifesub-asr-gate`，相同 `--model-dir`、固定 fixture、相同 Gate 参数分别执行两次，报告写入独立 JSON | 每次退出码、SenseVoice/Whisper RTF、peak RSS、模型和 runtime identity；首轮/次轮只代表进程与 OS 文件缓存状态，不冒充重启后真正冷机 |
| 麦克风首帧 | 安装版诊断中对同一精确 UID 执行首帧测试，导出 `selected_microphone.probe.elapsed_ms` 与 frame count | `result=compatible`、UID/readback 一致、frames>0 |
| 桌面启动与取消 | 对同一安装版在目标机执行有时间戳的启动至首个可交互窗口、开始后立即取消至停止确认流程 | 保存计时方式、两端事件和结果；没有可观测两端时明确记为未测 |

## 2026-10-13 已完成量测

- 诊断整快照：冷 `21.727ms`、热 `21.251ms`；4000 个文件，音频与模型分类均为 `32,000 B`，断言通过，`complete=true`。这是 `AppState::diagnostics_snapshot` 的固定权限/设备夹具，包含模型投影与存储遍历；不包含真实 helper 权限查询或 WebView 渲染。未发现需要在 0.2.2 中引入有界扫描合同的量化理由。
- 最终候选在隔离 HOME 下启动到首个屏幕上窗口的 3 次结果为 `488.704ms`、`314.803ms`、`303.810ms`，中位数 `314.803ms`；每次均创建独立数据目录并显示窗口，报告 `output/asr-v0.2/0.2.2-startup-window.json` 绑定 App SHA-256 `3f72dde3…5f26`，`complete=true`。计时包含窗口查询与最多 50ms 轮询误差，不能视为窗口内容完全可交互或设备就绪。
- 当前候选：App `3f72dde30c27afa1659b669aca4c1424d5f305cd024e7c790a3e6e866b9b5f26`、helper `5c7b4672fc9da848d900526337d2932efba0b0c8e9196991586acd60949e8e01`、Gate `c49bf64ca22be1857bee8b5e75251b690562776c3fc8dc14985246c05a913e0d`、DMG `52a4748719fecddc4498de56fb9b3d2e548ddf391e562d1bffa8d41452166318`。源码归档 SHA-256 `ed0246eebca23eae7b565322219c4b0a67858451283634915217d4e3786c3fea`。DMG checksum、nested strict signature、arm64、版本 0.2.2 与包内可执行文件 byte equality 均通过。可执行文件与此前候选逐字节相同，重新生成的 DMG 镜像哈希不同。
- 模型 Gate 对同一 `c49bf64c…` 可执行文件连续运行两次，均对固定 fixture 完整通过；见 `output/asr-v0.2/0.2.2-gate-run1.json` 和 `0.2.2-gate-run2.json`，两份都绑定当前构建源码提交、归档 digest、模型/runtime/fixture 身份。首轮墙钟 `18.14s`，SenseVoice RTF `0.05740` / RSS `486.0 MiB`，Whisper RTF `0.09389` / RSS `598.5 MiB`；次轮墙钟 `17.39s`，SenseVoice RTF `0.05848` / RSS `480.0 MiB`，Whisper RTF `0.08574` / RSS `743.2 MiB`。两轮 RTF 均低于 1，结果无 fallback。这里的“首轮/次轮”不能解释成机器重启后的真正冷/热模型加载差值。
- 0.2.1 历史参考，**不能替代 0.2.2 候选验收**：最终 20/20 报告中的内置麦克风 probe 首帧 `230ms`、蓝牙 FreeBuds probe `407ms`；同一候选打包 Provider Gate 中 SenseVoice RTF `0.0365`/RSS `470.4 MiB`，Whisper RTF `0.0702`/RSS `661.4 MiB`。固定来源见 `output/asr-v0.2/native-capture-v3-6f2e/reports/` 与 `output/asr-v0.2/packaged-provider-gate.json`。

## 待填候选结果

- 0.2.2 候选真实目标机首帧与取消确认延迟，以及候选身份下原生采集场景报告。启动至可见窗口已有基线，交互就绪尚未量测。Gate 的墙钟包含模型装载、音频解码和推理，RTF 只计推理；当前没有逐模型装载时间字段，因此不声称已测得独立的冷/热加载延迟。
- 若候选的目标机操作无法取得双端时间戳，记录缺口与原因，不补造数字。
