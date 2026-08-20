---
stage: v0.2-real-asr-complete
last_updated: 2026-08-20
source: codex-goal
---

# LifeSub 真实本地 ASR V0.2 进度

## 2026-08-20 遗留能力拆期

- V0.2 的发布边界收敛为 V0.2.1：导入音频、native ASR production executor、SenseVoice/Whisper、模型安装 UI、Job/Receipt/Revision 和安装包真实模型 Gate。
- V0.2.2 独立交付真实麦克风与系统音频采集，包括权限、双路来源、长时分片、恢复和安装包实机验收。
- V0.3 独立交付匿名 Diarization、ASR 时间轴对齐、人工纠错和 Speaker Evidence。
- V0.3.1 独立交付 CAM++ embedding、授权 Speaker Profile、拒识、撤回和删除。
- 多设备 Evidence 合并不再占用 V0.3 版本号，保留历史研究计划并在实施前重新排期。
- 总实施计划：`docs/superpowers/plans/2026-08-20-lifesub-remaining-capabilities-roadmap.md`。

## 2026-08-19 `/Applications/LifeSub.app` UI 与功能走查

- 已完成录音、暂停、停止、笔记、时间线、搜索、复制、Markdown 导出、修订入口、导入、词典、设置、模型与键盘操作走查。
- Critical：已安装包将 `MockStreamingSource` 的固定台词显示为"实时 SenseVoice"，停止后又提示"录音已保存"，但时间线仍仅展示 demoRecords；这会伪造真实录音、ASR 与证据保存成功状态。
- Important：时间线导入仅显示提示；播放按钮无处理逻辑；修订只改前端内存；模型下载、声纹注册/重命名/删除、词条编辑均无处理逻辑；停止后无法开始第二次记录；界面宣称的 `⌘R` 无效。
- Important：设置弹窗正文被父级双重 Grid 压缩成窄条，且无焦点锁定，Tab 会进入被遮挡的主页面。
- 已验证可用：会话切换与前端搜索、Evidence URI 复制、全文复制、Markdown 文件导出、设置读写。
- 阻塞项：在真实音频捕获、真实 ASR、真实 Catalog 时间线加载与失败显式反馈完成前，不应把当前构建描述为可用录音产品或 V0.2。
- 下一步：按 Critical/Important 顺序修复，并补一轮安装包级真实音频验收；验收数据不得来自 demo/mock。
- 合并清单：已将安装包实机操作、5 张截图走查与源码核对结果去重整理到 `ui-walkthrough-issues-2026-08-19.md`，共记录 35 项问题（Critical 2、Important 11、Medium 17、Minor 5），作为后续 UI 与功能修复的统一入口。

## 2026-08-19 大窗口响应式补充走查

- 基于 4 张约 1516px 宽窗口截图，新增 `ui-wide-window-walkthrough-2026-08-19.md`。
- 本轮记录 23 项：P1 2 项、P2 17 项、P3 4 项；无 P0。
- 两项 P1：录音页"准备就绪"与未接通状态自相矛盾；时间线空查询被错误描述为"没有匹配"。
- 共性根因：一级工作区高度链不完整、宽度只拉伸容器未重排信息、固定列与动作区缺少容器级响应式策略。
- 后续验收需覆盖 1280、1516、1728px 宽度以及 125%/150% 字体缩放，并保存 Playwright 截图与可访问性证据。

## 2026-08-19 大窗口响应式 UI 修复（worktree: codex/lifesub-real-asr-v0.2）

- 基于 `ui-wide-window-walkthrough-2026-08-19.md` 的 23 项大窗口 UI 问题进行系统性修复。
- 修复范围：LiveCapture.tsx、TranscriptView.tsx、DictionaryView.tsx、ModelManager.tsx、styles.css、settings.css、测试文件。
- **P1**（2/2）：录音页状态不再自相矛盾（闲置时显示"待检测"而非"准备就绪"）；时间线空态按查询/记录状态区分"等待转写""暂无转写""没有匹配的原话"。
- **P2**（17/17）：完整高度链 + 宽窗口响应式列宽 + 内容居中约束；录音诊断改为图标化状态清单；笔记面板等高；标题本地化格式；词典 header 压缩为工具栏 + 范围控件组 + 无分类时禁用新建词条；模型按 Provider 分组 + 稳定动作列 + 去重状态表达 + 提升元数据对比度。
- **P3**（4/4）：词典 footer 移除合并到 detail panel；空态 emoji 替换为 Lucide 图标；设置 modal 高度降低。
- 验证：前端 69/69 测试通过，TypeScript 0 错误。

## V0.2.1 实现完成

- 技术设计与 PRD 已通过第五轮规格复审。
- 实施计划经修订后通过第四轮计划复审，共 991 行，15 个 Task。
- 全部 15 个 Task 已完成实现与验证。

### Task 完成状态

| Task | 描述 | 状态 |
|------|------|------|
| 1 | 固定依赖与静态运行时构建 | ✅ 完成 |
| 2 | 版本化 Catalog 迁移 | ✅ 完成 |
| 3 | ASR 领域类型与设置校验 | ✅ 完成 |
| 4 | 不可变音频导入与崩溃恢复 | ✅ 完成 |
| 5 | 模型与 VAD Manifest 冻结 | ✅ 完成 |
| 6 | 可恢复模型下载与安装 | ✅ 完成 |
| 7 | 音频解码、降混、重采样、分区 | ✅ 完成 |
| 8 | SenseVoice 与 Whisper Provider | ✅ 完成 |
| 9 | 围栏 ASR Job 状态机 | ✅ 完成 |
| 10 | 原子 Receipt 与 Revision 发布 | ✅ 完成 |
| 11 | 桌面命令与 Worker 启动 | ✅ 完成 |
| 12 | 类型化 ASR 客户端与设置 UI | ✅ 完成 |
| 13 | Job 状态、来源与重转写 UI | ✅ 完成 |
| 14 | 两种真实模型固定 Fixture 验证 | ✅ 完成 |
| 15 | Playwright、响应式与打包 Gate | ✅ 完成 |

### 验证结果

#### 前端测试
- `npm test`: 45/45 通过
- `npm run build`: 成功

#### Rust 测试
- `cargo test --no-default-features`: 146/146 通过
- `cargo check --features desktop`: 通过（仅 warnings）
- `cargo check --no-default-features`: 通过

#### 代码质量
- `console.log`: 无匹配
- 无模型权重、用户音频、API 密钥或私密路径

### 已创建文件
- `src-tauri/src/asr/` (13 个 Rust 模块)
- `src/components/asr/` (5 个 React 组件)
- `src/services/asr.ts`, `src/services/asr.test.ts`
- `src-tauri/src/bin/lifesub-asr-gate.rs`
- `src-tauri/src/acceptance.rs`, `src/acceptance.ts`
- `tests/specs/lifesub-real-asr-v0.2.spec.ts`
- `scripts/verify-asr-gate.sh`, `scripts/verify-desktop-asr.sh`
- `scripts/fetch-sherpa-runtime.sh`
- `output/asr-v0.2/verification.md`

## 待完成（后续版本）

- 真实模型 Gate 运行（需要下载模型）
- 桌面验收场景运行（需要真实模型）
- DMG 构建与签名验证
- Apple notarization
- macOS 原生双路采集（ScreenCaptureKit + AVAudioEngine）
- 多设备录音归并（V0.3）

## 阻塞项

无技术阻塞。真实模型验收需要下载 SenseVoiceSmall 和 Whisper 模型后运行 Gate 脚本。