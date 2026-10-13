---
stage: v0.2.2-in-development
last_updated: 2026-10-13
source: codex-v0.2.2-development
---

# LifeSub 真实本地 ASR V0.2 进度

## 2026-10-13 0.2.2 打包候选与发布门槛

- 把之前已验收但留在工作树的 0.2.1 产品源码与本轮 S1-S3 差量提交到主工作区；候选源码为 `main@5584bab51a170c956f9ded28d0593bd761d2a020`。`origin/main` 仍是旧提交 `db39faa`，未推送；个人 `.agents/skills/ponytail/` 与 `.impeccable/` 未纳入发布源码。
- 最新候选 `LifeSub_0.2.2_aarch64.dmg` 已由受验证的 sherpa runtime runner 构建。App/helper/Gate/DMG SHA-256 依次为 `3f72dde30c27afa1659b669aca4c1424d5f305cd024e7c790a3e6e866b9b5f26` / `5c7b4672fc9da848d900526337d2932efba0b0c8e9196991586acd60949e8e01` / `c49bf64ca22be1857bee8b5e75251b690562776c3fc8dc14985246c05a913e0d` / `34de71df121398949fd5759b56f7e0879f7d4959b066607f32607c6d8756d76e`。
- `npm run verify:release-source` 在代码提交后通过，生产 wiring 已封闭；Tier 2 `583/0/7`、Tier 3 `626/0/9`、Swift `75/75`、前端全量与构建通过。浏览器设置 1180/760px 两视口通过，历史 0.2.1 原生报告仍独立通过 `20/20`，但不转借给 0.2.2 候选。
- 新候选 DMG checksum、只读挂载、包内和外部 .app 三个二进制 byte equality、arm64、版本 0.2.2、nested strict signature 和 audio-input entitlement 均通过。签名为 ad hoc，缺少 notarization 身份，不作为对外分发包。
- 新候选 Gate 在同一 `c49bf64c…` 身份下两次完成 SenseVoice/Whisper 固定 fixture 真实推理，RTF 均小于 1；报告见 `output/asr-v0.2/0.2.2-gate-run1.json` 与 `0.2.2-gate-run2.json`，量化边界见 [性能基线](../../../performance/lifesub-0.2.2-baseline.md)。高文件数诊断完整快照 4000 文件冷/热约 21ms。
- 旧 `verify-desktop-asr.sh dmg` 会误取旧 target 并调用未接线的 `--acceptance-scenario packaged-smoke`，实际启动 GUI 且无法产生有效 smoke。已按 RED/GREEN 改为精确版本 DMG 的只读静态验证；真实新包与 4 种错误/正确夹具均通过。旧 `target` 模式现在显式 fail closed，要求使用 real-model Gate 和 native reports。
- 在隔离 HOME 下新增可复测的 macOS 窗口启动基线，最终 App SHA 对应的 3 次启动至可见窗口为 `488.704 / 314.803 / 303.810ms`，每次均有独立数据目录和可见窗口；这是窗口出现时间，不声称 UI 已可交互。
- **未发布**：0.2.2 仍缺候选身份下 20 个原生采集场景及真实首帧/取消计时，需要权限切换、外接输入和真实睡眠唤醒配合；安装版仍为 0.2.1。不得把旧报告改写为新候选，也不得提前覆盖 `/Applications/LifeSub.app`。

## 2026-10-13 启动 0.2.2 稳定化

- 用户要求继续开发并发布当前版本；按既定路线启动 0.2.2 S1-S4，延后 Task 6、9-16。唯一工作区仍为 main，保留先前 0.2.1 未提交产品源码与全部原有文件。
- S1：设置入口收敛为录音、ASR 与模型、诊断三个目的地，关于信息随诊断呈现；录音与 ASR 准备摘要只投影现有设置、设备和模型事实。
- S2：麦克风首帧测试加入会话内时间标记与重测入口，设备断开或持久化选择变化后清除旧测试结果；模型操作展示四步进度；时间线展示已持久化的 queued/preparing/transcribing，未捏造 publishing；移除固定的“ASR 未接通”结论，并澄清声纹、词典与临时润色的实际边界。桌面转写的说话人来源只读显示，避免把未持久化的临时重命名误当产品能力。当前生产前端没有恢复中/已恢复的可读事件或 DTO，因此没有虚构恢复状态。
- S3：诊断不把可选未安装模型当作需处理问题，权限/设备摘要改为需核对而非断言阻塞；沿用精确 UID 发起首帧重测并刷新快照，增加设备类型与稳定错误代码复制。Finder 打开路径暂不添加，因为诊断字符串不能证明点击时仍指向受锚定目录。
- S4：新增明确忽略、需手动执行的高文件数诊断基准。M4/24 GiB/macOS 15.6.1，4000 个小文件下全量诊断快照耗时冷 21.727ms、热 21.251ms，完整成功。其余模型冷热、启动、取消与新包实机指标仍待采集。
- 已通过 112 项相关前端测试、生产前端构建、2 项 1180/760px 设置视觉验收；高文件数 Rust 基准测试 1 项通过。0.2.2 尚未完成全部门禁、未打包、未安装或对外发布。

## 2026-10-13 分支与 worktree 清理

- 获取并清理远端跟踪引用后确认 `main == origin/main == db39faafeac0e8f4816623456f3ffa8b1ab54564`，双方提交数差为 `0/0`。LifeSub 仅有主目录一个已注册 worktree，`git worktree prune` 无陈旧项。
- 删除 5 条已合并历史分支；另外 2 条未合并任务分支的独有提交先保存到 `refs/archive/branch-cleanup-20261013/` 下的可恢复引用，再删除可见分支。清理后本地分支仅剩 `main`；完整 ref 和恢复点见 `docs/workspace-status.md`。
- 当前主工作区 160 条未提交状态项（98 modified、62 untracked，按 Git 默认目录折叠计）。没有切换、重置、暂存、提交或删除这些工作文件；它们仍是比远端 `main` 提交更新的实际开发内容。
- 后续迭代从 `/Users/goldenwave/Documents/MyProject/LifeSub` 的 `main` 当前工作树继续，先审阅 diff 与已有状态。新建独立 worktree 应等当前工作树经有意识的验证与提交后再从新提交分支，避免只拿到 `db39faa` 的旧快照。0.2.2 仍未启动，当前 stage 不变。

## 2026-10-13 VoiceStudio 高优参考项目源码审阅与迭代计划校对

- 从 `https://github.com/debpalash/VoiceStudio` 拉取到忽略的 `.sources/reference-projects/VoiceStudio`，固定 Git 提交 `36862c1157a25db7f7b885efba50a072ddf70e45`。该目录保留 Git 元数据用于后续核对，不作为 LifeSub 构建或发布源；可选 `omnivoice-gallery` 子模块未初始化。
- 完成与 LifeSub 当前代码的静态对照，覆盖 VoiceStudio Electron 桌面与 UI、Python API/ASR/模型、Rust 原生桥接、诊断/存储、音频信号检查、匿名说话人和许可信息；详细依据见 [源码对照](../../../context/wiki/syntheses/voicestudio-source-review-2026-10-13.md)。未运行 VoiceStudio 模型、下载权重或做真实音频对比。
- 将 VoiceStudio 登记为高优先级**参考**，在 `AGENTS.md` 加入路由，在总路线和 8 月 26 日现行计划中补入具体借鉴点：0.2.2 S4 测诊断大目录扫描及冷/热模型成本；稳定化后按真实失败归因决定 ASR 进程隔离、只读音频质量提示或 Task 11 有界存储快照。新模型发布前分开记录许可声明与逐资产证据。
- VoiceStudio 应用为 AGPL-3.0-only；仅借鉴行为和验证思路，不复制实现。LifeSub 的原始音频、精确模型/输入身份、Job fencing、Receipt/Revision 原子发布继续保持权威。
- 本轮只拉取参考源码并修改文档；未启动 0.2.2，未修改产品代码、运行测试、构建、安装或替换当前应用。当前 stage 仍是 `v0.2.1-ui-refresh-packaged`；0.2.2 S1–S4 待启动，Task 6/9–16 后置。

## 2026-09-16 完成 VibeVoice 本地参考项目归档

- 将 VibeVoice 完整源码快照归档到 `.sources/reference-projects/VibeVoice-main`，登记为后续开发可查阅的只读参考，并在 `AGENTS.md` 增加全局路由；该本地快照通过 `.gitignore` 排除，不进入 LifeSub 仓库历史。
- 参考范围包括长音频 ASR、流式识别、说话人/时间戳/正文联合输出、热词、推理和微调；同步补入现有 ASR/音频参考项目清单与关键源码入口。
- 已用 `rsync --checksum --delete` 校验归档与 Downloads 原目录逐文件一致，双方均为 110 个文件；原 `/Users/goldenwave/Downloads/VibeVoice-main` 已删除。
- 当前快照 `pyproject.toml` 声明版本 `1.0.0`，根目录为 MIT License，但目录没有 `.git`；模型、权重、数据和其他资产仍须逐项核对许可。已完整复制源码、未新增依赖、未修改产品代码或发布范围。

## 2026-09-11 用户确认统一 AI 校正定义

- 确认忠实还原录音的语义/声学内容治理属于 LifeSub，可用 LLM 或受约束 Agent；校正转写与目标对话过滤合并为一个 AI 校正版，维度在设置中勾选。
- 产品定义补充维度建议、设置生效范围、复制/搜索/导出版本语义与最小存储原则；同步洞察、路线与现行计划。维度默认值仍为建议，不冒充用户已确认的细节。
- 保留原始 ASR 和必要追溯，避免按维度存全文/中间产物，不新增第三层产品。未实现功能、未迁移存储；stage 和 S1–S4 边界保持。

## 2026-09-11 Qwen Audio 文章分析与后续方案校对

- 新增 [综合分析](../../../context/wiki/syntheses/qwen-audio-product-direction-review.md)，区分会议结构化 ASR 与语音执行运行时，给出质量评测、职责边界和后续择项门槛。
- 校正总 roadmap 中过期的采集/执行器未完成状态；明确 0.2.2 S1–S4 封顶、后续不预绑定版本号；现行实施计划加入差量盘点和状态真实性要求，产品定义区分来源与说话人身份。
- 本轮只改文档，保留 stage；未启动 0.2.2、构建、安装、模型下载或真实音频实验。文章所述上游机制未作 commit 级源码审计，不作为模型可部署或质量优越的证据。
- 复核唯一 main@db39faa 工作区和生产工厂/worker 接线；历史发布结果继续引用原记录。下一步仍是启动 S1–S4；会议研究用于后续产品择项，不扩大稳定化发布门槛。

## 2026-09-09 产品现状洞察（仅评估与文档）

- 新增 [产品现状洞察](../../../product-current-state-insight.zh-CN.md)，覆盖交付状态、版本口径冲突、Evidence 定位、安装版交互、决策清单和研究建议。
- 只读核对唯一 worktree `main@db39faa`；本轮写入前 dirty 153（94 modified / 59 untracked）。三个版本配置与安装版均为 0.2.1；安装主程序 SHA `4b7538fc...` 与 9 月 3 日时间线滚动修复记录一致。没有构建、安装、提交或开启 0.2.2 开发。
- 实机只读查看录音/设置/ASR/模型/诊断/词典/时间线；发现固定“本地 ASR · 待接通”、诊断将 4 个未安装可选模型计入需处理、声纹注册说明与计划中入口矛盾。代码进一步显示实时说话人重命名与润色为组件状态，复制全部仍取原始文本；均记录为产品评估，未修改业务代码。
- 当日诊断：麦克风已授权，系统音频未授权，首帧未测试，SenseVoice/Whisper Base 为 runtime_qualified。此时机器条件与历史发布验收分开记录，未改变权限或触发录音/下载。
- 保持 stage `v0.2.1-ui-refresh-packaged`；0.2.1 定义内范围已收口，0.2.2 正式计划仍未启动，部分 S1/S3 界面收益已由 9 月 3 日更新提前覆盖，启动时需做差量盘点。
- 本轮验证只覆盖文档格式/链接与事实核对；历史 20/20 原生验收和测试数字明确标日期，未声称本轮重新跑通。下一步由产品负责人决定是否启动 S1–S4，以及 0.3+ 的优先用户成果。

## 2026-09-03 UI 走查问题优化与重新打包

- 完成走查文档中的纯视觉与前端交互项：移动录音固定控制与时长、Timeline/Dictionary 窄屏钻取、Session Tree 展开/选择与键盘行为、搜索命中导航、持续播放控制、设置 dirty/关闭保护、产品内删除/重命名、模型单状态、诊断摘要与完整身份复制。
- 用户安装版截图发现原生录音头部的“收音麦克风”标签覆盖设备下拉框。根因是可收缩 flex/grid track 与 nowrap 文字共同导致文字逃出自身盒；已重构为明确网格区域，标签不可收缩，设备名只在 select 内截断，390px 下标签独立成行。
- 新增 Tauri 运行时视觉夹具和文字 Range/控件 bounding box 碰撞断言，覆盖 1180x760、1280x800、1516x1120、390x844、125%/150% 缩放、长设备名和已断开保存名称。旧实现下 6 个核心场景全部 RED，修复后原生几何专项 `8/8` GREEN。
- 同轮截图复盘收口三项前端问题：移动 Timeline 统计前的大段空白、Dictionary 详情态无关管理工具、模型 `desktop_runtime_required` 缺少原因与下一步；录音稳定 idle 时不再重复展示四行“未检测”诊断占位，启动中或失败时仍保留事实。
- 未修改 Tauri/Rust/Swift、Catalog、服务接口、持久化结构或业务状态机；产品决策项继续保留在走查文档，不通过表现层提前定义。
- 前端验证：Vitest `151/151`、production build、Playwright `45/45`、`git diff --check`、无 `console.log` 通过。
- Tier 2：`583 passed / 0 failed / 7 ignored`，fmt、Clippy warnings denied 与 diff check 通过。
- 合并审查首次为 `Critical 0 / Important 4 / Minor 1`；移动时长、树键盘/片段联动、诊断 runtime identity、删除防重入与隐藏面板焦点过滤修复后，复审为 `Critical 0 / Important 0 / Minor 0`。
- 从唯一工作区 `/Users/goldenwave/Documents/MyProject/LifeSub`、`main@db39faafeac0e8f4816623456f3ffa8b1ab54564`、dirty count `152` 通过 release-source gate，并使用固定 Sherpa attestation 环境重新构建 App/DMG。
- 新安装版 app/helper/gate SHA-256：`55e97c9c1a1dbd799e17b4e3b485aa582bd0e1854f1ce1960d97a6c7d037bc80` / `5c7b4672fc9da848d900526337d2932efba0b0c8e9196991586acd60949e8e01` / `4a2e266d2596548ef2ac3819450361d7e80bf2b30f73e0e642b156595116b843`。
- DMG：`src-tauri/target/release/bundle/dmg/LifeSub_0.2.1_aarch64.dmg`，SHA-256 `6ba5020a5d988e2e59102531c1ce70f4d56bb4558be0222ce43c0f8a76b3d073`；`hdiutil verify`、arm64、版本 `0.2.1`、nested strict signature 与安装前后 executable byte compare 通过。
- `/Applications/LifeSub.app` 已替换并正常启动，活跃实例为 1；前一安装版备份于 `/tmp/LifeSub.pre-overlap-fix-202609031439.app`。真实安装版截图：`output/playwright/lifesub-0.2.1-overlap-fix-installed.png`，确认标签、下拉框、刷新和测试按钮不再重叠。

## 2026-09-03 时间线滚动修复与重新打包

- 用户真实数据截图发现时间线长内容将侧栏“设置”入口推到视口外，左右栏滑动失效。根因是桌面高度链只使用 `min-height: 100vh`，内部 `height: 100%` 没有确定父高度，长 Transcript 将整个 Shell 撑高后又被 `body overflow: hidden` 裁切。
- 桌面端现将 `html/body/#root` 固定为 100% 高度，将 app shell/sidebar/workspace 固定为 `100dvh`，外层保持 hidden；Session Tree 与 Transcript 成为实际独立滚动区。小于 768px 的移动页面继续自然滚动。
- 新成长时间线 Playwright 回归在旧实现下稳定 RED，修复后验证外层不超过 viewport、设置入口可见且可打开、左右栏 `scrollHeight > clientHeight` 且 `scrollTop` 可移动。
- 新鲜验证：Vitest `151/151`、Playwright `46/46`、production build、`git diff --check` 和无 `console.log` 通过；Tier 2 `583 passed / 0 failed / 7 ignored`，fmt、Clippy warnings denied 与 diff check 通过。
- 从唯一发布源 `main@db39faafeac0e8f4816623456f3ffa8b1ab54564`、dirty count `152` 和固定 Sherpa attestation 环境完成重新打包。App/helper/gate SHA-256：`4b7538fc0fbccf2382f8fedc654f067eccbaf548f3a9e386a8611d565b2e7bb5` / `5c7b4672fc9da848d900526337d2932efba0b0c8e9196991586acd60949e8e01` / `cbf37062abc6a93a2868de2702af81e2b420a2dd4c10e3b0ceaebfbce11f12d2`。
- DMG SHA-256：`1ac9bf3a4c7a9f060d46092f67a73af4639b45ed9aa1cf71bcbe46184b56f46d`；`hdiutil verify`、arm64、版本 `0.2.1`、nested strict signature 和安装前后 byte compare 通过。
- `/Applications/LifeSub.app` 已替换，前一版备份于 `/tmp/LifeSub.pre-timeline-scroll-fix-202609031526.app`。真实安装版在 51 个会话、331 个可访问项下显示左右独立滚动条，统计栏与设置入口保持在视口内，设置弹层可正常打开。截图：`output/playwright/lifesub-0.2.1-timeline-scroll-fix-installed.png`。

## 2026-09-02 下版本前端 UI 走查与优化

- 完成全前端面 UI 走查，覆盖录音、时间线/Transcript、词典、设置、诊断与模型页面；问题清单与验收矩阵记录于 `docs/design/lifesub-ui-walkthrough-2026-09-02.md`。
- 基于优化后截图与现有交互补充二次体验走查，新增“仍然不好用或不好看的地方”、视觉完成度判断和三阶段建议；重点识别移动端长页面、录音控制可达性、设置未保存保护、列表选择/展开冲突，以及录音空闲态、词典三栏和模型状态页的视觉粗糙问题。
- 本轮严格限定在 React 表现层和 Design Token：未修改 Tauri、Rust、Swift、Catalog、adapter、服务接口或业务状态机，`0.2.1` 已冻结候选身份不因此重建。
- 扩展共享 token，统一控件 default/hover/focus/disabled/loading/error 状态、面板宽度、内容行长与 reduced-motion；保留深色、扁平、锐角、Evidence-first 视觉方向。
- 补齐导航 `aria-current`、图标按钮 tooltip、词典选择态、时间线加载/错误容器和 Transcript 播放状态语义。
- 新增 390x844 响应式验收；首次 RED 发现顶部导航宽度 `427px > 390px`，根因是四个文字标签与 44px 触控目标并排。最窄断点改为图标导航并保留 `aria-label` 后 GREEN。
- 验证：frontend `125/125`、Playwright `35/35`、production build 与 `git diff --check` 通过；多视口截图位于 `output/playwright/ui-refresh-*.png` 与 `wide-window-*.png`。

## 当前进度快照（2026-09-02）

| 版本 / 范围 | 状态 | 完成度 | 当前结论 |
|---|---|---:|---|
| `0.2.1` 本地录音 + ASR 可信发布闭环 | 已发布收口 | 100% | Task 1-5、7-8 完成，20/20 原生场景、DMG、安装版与单实例验收全部通过 |
| `0.2.2` 轻量稳定化 | 待启动 | 0% | 仅限 S1-S4：设置/诊断信息架构、现有状态表达、既有事实上的操作便利性、性能基线包 |
| Task 6、Task 9-16 | 已后置 | 0% | 不进入 `0.2.2`；涉及新设置、新持久化语义、AEC、融合、preview 或性能引擎改造时单独立项 |

- 当前活跃阻塞：无。
- 当前开发动作：无；`0.2.1` 候选二进制身份保持冻结，不因文档整理重建。
- Worktree 状态：仅保留主工作区；已于 2026-09-02 删除干净且无独有提交的 `lifesub-real-asr-v0.2` 回滚 worktree，Git 分支指针保留。
- 下一个建议里程碑：确认 `0.2.2` 启动后，按 S1 -> S2 -> S3 -> S4 推进，以“不新增 schema/helper/Evidence 语义”为停止线。

## 2026-09-02 V0.2.1 正式版收口完成

- 本次范围严格保持为 Task 1-5、Task 7-8；Task 6 与 Task 9-16 未进入发布。
- 最终原生候选身份保持冻结：app `6f2e65235de68c98b14bbed127dbb9762e90773d46685d97c7f162bf3903cf4a`、helper `5c7b4672fc9da848d900526337d2932efba0b0c8e9196991586acd60949e8e01`、gate `c1bafe9a101382cba688f67ec84a9d50c5e76e496afe1162bc3b8c45961d714d`、source `main@db39faafeac0e8f4816623456f3ffa8b1ab54564`、dirty digest `6d5c9bfa897307cb9a74c551c52afd0bb08037eee0eabef319e59b98d9c5c4ef`。
- `dual-source-30m` 正式会话 `rec_2461583fee56467798b51d91887a4f9b`：共同有效时长 `1948.339625s`，microphone/system_audio 各 65 个连续 chunk，130 个 Job 全部 succeeded，drift `209ms`，dropped frames `0`，peak RSS `570.094MiB`；重启后搜索“挺聪明”、回放和 `0600` Markdown 导出通过。
- 真实睡眠/唤醒触发签名 helper 的 `device_configuration_changed` 同 UID recovery：generation `3`，恢复 UID `BuiltInMicrophoneDevice`，新首帧存在且前后 sample 连续。主应用会话 `rec_76870036250c4a47bee96ff5707c0b85` 在同次睡眠后继续录制，18 个 chunk/Job 全部 succeeded；重启后搜索“小学六年级”、回放和私有导出通过。
- 最终目录 `output/asr-v0.2/native-capture-v3-6f2e/reports/` 已通过 `20/20` v3 verifier。Native ASR sealed capability marker 在目录门禁通过后加入，正式 `npm run verify:release-source` 通过。
- 新鲜静态门禁：Swift helper `75/75`，frontend `125/125`，Tier 2 `583 passed / 0 failed / 7 ignored`，Tier 3 `626 passed / 0 failed / 8 ignored`，production frontend build、fmt、Clippy warnings denied 与 `git diff --check` 通过。
- 最终 DMG：`src-tauri/target/release/bundle/dmg/LifeSub_0.2.1_aarch64.dmg`，SHA-256 `37ff9f39b56e03802a48823f42c02822e01cc3ed723a140af27249523532bd14`。`hdiutil verify`、只读挂载、nested strict signature、arm64、app/helper audio-input entitlement、`otool -L` 与 DMG 内三 executable hash 比对通过。
- `/Applications/LifeSub.app` 已正常安装；app/helper/gate hashes 与冻结候选一致。安装版正常启动，TCC attribution 指向 `/Applications/LifeSub.app` 的 `com.goldenwave.lifesub`，诊断页显示麦克风与录屏/系统音频均“已授权”，SenseVoice/Whisper Base 均 `runtime_qualified`。重复启动返回 `Ownership(AlreadyOwned)` 且活跃实例保持 1。
- 最终截图：`output/playwright/lifesub-0.2.1-installed.png`、`output/playwright/lifesub-0.2.1-installed-diagnostics.png`。测试后录音设置已恢复为“智能路由（推荐）”。
- 历史 `scripts/verify-desktop-asr.sh dmg` 的隔离 HOME 父目录缺失已完成 shell RED/GREEN；继续运行后确认旧 `packaged-smoke` scenario 未接入生产 `run()` 且固定为失败 scaffold，因此不作为 release evidence。实际 DMG/安装门禁由只读挂载、签名、精确 hash、正常安装启动、权限归属与单实例证据覆盖。

## 2026-09-02 FreeBuds 设备断开/重连真实采证

- 最终候选 `6f2e6523 / 5c7b4672 / c1bafe9a / 6d5c...` 保持不变；未重建、未复用历史候选报告。
- FreeBuds 7i 真实连接后作为 `78-F0-9B-A1-7E-BC:input`、Bluetooth、16 kHz 单声道被候选正确枚举；首帧 probe 通过。
- 录制中使用 `IOBluetoothDevice.closeConnection()` 断开设备；LifeSub 正确显示“所选麦克风已断开连接”，采集停止且未静默切换到内置麦克风。会话 `rec_c9366841cc1a4095991676a4aae20c3b` 共 9 个连续 microphone chunks、9 个 succeeded Jobs、0 dropped frames。
- 重新连接后 Core Audio 恢复暴露 FreeBuds；LifeSub 仍保留精确 UID，重新 probe 通过，诊断显示“已连接 / 已通过首帧测试”。搜索、回放、`0600` Markdown 导出和诊断导出均已验证。
- 新增并逐份通过正式 v3 verifier：`microphone-active-disconnect.json`、`device-disconnect-reconnect.json`。当前最终候选报告从 `11/20` 推进到 `13/20`。
- 预启动断开 UI 也已真实确认：保持选中 FreeBuds、显示“已断开”、禁用 probe/start；但应用在断开时不自动执行 probe，诊断为 `untested`，因此没有伪造 `microphone-prestart-disconnect` 报告，仍需在正式可观察 `unavailable` probe 合同成立后采集。
- 当前外部阻塞仅剩 LifeSub 的“录屏与系统录音”权限开关为 `off`；用户确认后由用户完成 Touch ID/管理员认证，再继续 `system-only`、`dual-source`、`system-only-no-microphone` 与 30 分钟双源场景。不得在此之前添加 Native ASR marker、构建 DMG 或安装。

## 2026-09-02 预启动断开与 same-UID 证据审计

- 最终候选在 FreeBuds 断开时保留持久化选择 `78-F0-9B-A1-7E-BC:input / HUAWEI FreeBuds 7i`，UI 显示“已断开”，probe/start 控件均禁用；Catalog 未新增 Session/Source/Chunk/Publication。
- 使用同一最终候选内签名 helper、正式 V2 `probe_microphone` 帧协议对断开的精确 UID 执行旁路探测，真实返回 `result=unavailable / first_frame_sample_count=0 / elapsed_ms=77`。helper 在设备不存在分支只返回 UID/unknown；用户可见名称和 Bluetooth transport 由持久化设置与 macOS Bluetooth 事实独立绑定，原始 helper 输出以 `0600` 保存。
- `microphone-prestart-disconnect.json` 已由 collector 生成并通过单文件 verifier；当前最终候选正式进度为 `14/20`，所有 14 份报告逐份验证通过、身份一致。目录 verifier 仍正确停在 `missing report: system-only`。
- same-UID 再审：LifeSub 内置麦克风主会话与签名 helper 旁路会话同时运行，使用 Core Audio 写入 `44.1 -> 48 -> 44.1 kHz`，保持 48 kHz 8 秒；两路均持续正常采集，但 helper 未发出 `source_recovering/source_recovered`。因此该操作只证明健康连续采集，不证明 recovery，仍不生成报告；硬件已恢复 44.1 kHz。
- 第二种 same-UID 触发也作废：保持 LifeSub/旁路 helper 精确绑定 `BuiltInMicrophoneDevice`，将 macOS 默认输入临时切到 `DTAudioDevice` 5 秒再恢复。两路继续产出连续 chunk，仍无 recovery 协议事件。默认输入已恢复内置麦克风，内置设备采样率已恢复 44.1 kHz；该实验不进入正式报告。
- System Settings 已导航到 `Privacy & Security -> Screen & System Audio Recording`，LifeSub 开关仍为 `off`，等待用户在动作时确认。未点击、未代替用户认证。
- 连续第三轮动作时复核：LifeSub 系统录音开关仍为 `off`，用户尚未回复“确认开启”；最终候选仍为 app `6f2e...` / helper `5c7b...`，报告仍为 `14/20`，目录 verifier 首个缺失项仍是 `system-only`。系统音频、双源、30 分钟双源均无法在不改变该安全敏感权限的前提下采集；sleep/wake 与真实 same-UID 设备层 recovery 也需要外部事件。目标按规则标记 blocked，收到用户明确确认后从此状态恢复。

## 2026-09-01 权限与 Bluetooth 真实场景收口

- 用户已明确授权临时切换 LifeSub 麦克风与录屏/系统音频权限。Finder 启动证明当前候选的 TCC responsibility 正确归属 `com.goldenwave.lifesub`；从 Codex 终端直接启动会归属 `com.openai.codex`，不能作为正式权限证据。
- 关闭麦克风权限后，旧候选的显式 probe 没有返回 `permission_denied`，而是 helper `TruncatedFrame`。CrashReporter `lifesub-capture-helper-2026-09-01-195319.ips` 证明未使用的 `AVAudioEngine` 在 `MicrophoneProbe` 异步析构时触发 `AVAudioIOUnit` use-after-free/指针认证崩溃。
- 参考 TypeWhisper 的真实 teardown 处理后，最终修复保持三处最小根因改动：系统 `AVAudioEngine` 首次真实访问才创建；`stop/removeTap` 后延迟保留 300ms，让 CoreAudio 内部 callback 排空；正式 capture 在 exact UID bind/readback 完成后才读取 `inputNode`。OpenWhispr 的 generation fencing/health-check 原则已由现有 LifeSub recovery 合同覆盖，未重复引入 fallback 或热切换。
- RED/GREEN：`liveMicrophoneAudioEngineDoesNotCreateSystemEngineUntilUsed`、`liveMicrophoneAudioEngineDelaysUsedSystemEngineRelease`、`selectedMicrophoneIsBoundBeforeTapAndEngineStart` 均先失败后通过；Swift helper 全量 `75/75`。
- 最终 app-only 候选通过 verified Sherpa runner 重建：app SHA-256 `6f2e65235de68c98b14bbed127dbb9762e90773d46685d97c7f162bf3903cf4a`；helper SHA-256 `5c7b4672fc9da848d900526337d2932efba0b0c8e9196991586acd60949e8e01`；gate SHA-256 `c1bafe9a101382cba688f67ec84a9d50c5e76e496afe1162bc3b8c45961d714d`；source `main@db39faafeac0e8f4816623456f3ffa8b1ab54564`；build-source dirty digest `6d5c9bfa897307cb9a74c551c52afd0bb08037eee0eabef319e59b98d9c5c4ef`。Nested strict signature、arm64 与 app/helper audio-input entitlement 通过。
- FreeBuds 真实 probe 通过 `16 kHz / 1 channel / 1600 first-frame samples / 407ms`。真实会话 `rec_9c5b62261e21418e9af8c98dee5c810d` 产生 5 个连续 chunks、5 个 succeeded Jobs、2 个 Revisions 和 4 个非空 Segments；重启后选择恢复，搜索“快照状态”、19 秒回放和 `0600` Markdown/diagnostics 导出通过。
- 当前最终候选报告完成 `11/20`：`mic-only`、`permission-denied`、`permission-allow-revoke`、`settings-permission-roundtrip`、`helper-crash-recovery`、`restart-recovery`、`search-playback-export`、`microphone-built-in-selection`、`microphone-external-selection`、`microphone-restart-persistence`、`microphone-no-device-ui`，全部单文件 verifier 通过，目录为 `output/asr-v0.2/native-capture-v3-6f2e/reports/`。
- 最终 bundle 内 provider gate 已按新 digest 重跑：SenseVoice CER `0.0769` / RTF `0.0365` / RSS `470.4 MiB`；Whisper WER `0.0` / RTF `0.0702` / RSS `661.4 MiB`，均 passed，报告绑定 `db39faa/6d5c.../c1ba...`。
- 新鲜门禁：collector shell tests、20-scenario mutation verifier、Swift helper `75/75`、Tier 2 `583 passed / 0 failed / 7 ignored`、fmt、Clippy、diff、nested strict signature 均通过。目录 verifier 准确停在 `missing report: system-only`；正式 release-source verifier 仍只因 Native ASR marker 尚未获得而 fail closed。
- 麦克风权限已恢复为 on。FreeBuds 当前物理断开，预启动 UI 正确显示“已断开”并禁用 probe/start，但 diagnostics 为“未测试”，不满足 verifier 的真实 unavailable probe 合同，因此未生成 `microphone-prestart-disconnect` 报告。等待耳机重新上线后继续 active disconnect/reconnect；sleep/wake 仍需真实睡眠唤醒。
- 继续切换最终候选的“录屏与系统录音”权限时，macOS 要求 Touch ID/管理员密码解锁 System Settings。该认证必须由用户本人完成；当前停在解锁弹窗，完成 Touch ID 后可直接继续 LifeSub off/on 与 `system-only` 采证。
- 新候选 helper crash/relaunch 已真实重跑：helper PID `35490 -> 35809`；旧采集 UI `ProtocolViolation` fail closed；恢复会话 `rec_079c1dc9bfd040b4bcf00b91a7660abc` 有 3 chunks / 3 succeeded Jobs / 10 non-empty Segments，并通过重启、搜索“保洁”、回放和 `0600` 导出。
- 内置麦克风最终候选会话 `rec_bf8c12ff929e4c87a3624a3f08c5d194`：probe `44.1 kHz / 1 channel / 4410 samples / 230ms`，2 chunks / 2 succeeded Jobs / 8 Segments，重启选择恢复、搜索“钉群”、回放和私有导出通过。
- Same-UID 配置变化已用 Audio MIDI Setup 在录音中执行 `44.1 -> 48 kHz -> 44.1 kHz`；会话 `rec_f3e88c5feeff4a94ba6dda6732ce9889` 持续产生 3 chunks / 3 succeeded Jobs。但当前 production evidence 未暴露 verifier 要求的 recovery generation 与 post-recovery firstHostTime，无法证明 recovery 事件而非连续正常采集，因此未生成 `microphone-same-uid-recovery` 报告。
- 连续第三轮外部复核：LifeSub 录屏权限切换仍停在 macOS Touch ID/管理员密码弹窗；FreeBuds 仍为 `device_not_connected` 且不在 Core Audio 输入列表。无需这些外部状态的最终候选场景已全部推进到 `11/20`；剩余工作无法在不代替用户认证或伪造硬件事件的前提下继续，目标正式标记 blocked，认证/唤醒设备后从当前候选直接恢复。
- 原 `native-capture-v3-097c/` 与中间 `native-capture-v3-216b/` 报告均为历史证据，不能与最终候选混用。不得添加 release marker、构建 DMG 或覆盖 `/Applications/LifeSub.app`。

## 2026-09-01 v3 采证真实性与双源启动收口

- 最终候选正式 v3 报告已完成 11/20：`mic-only`、`system-only`、`dual-source`、`dual-source-30m`、`helper-crash-recovery`、`restart-recovery`、`search-playback-export`、`microphone-built-in-selection`、`microphone-restart-persistence`、`microphone-no-device-ui`、`system-only-no-microphone`。每份均单文件 verifier 通过，统一绑定 `097c.../1937.../db39faa/dc4f...`。
- 真实 helper crash/relaunch：PID `67947 -> 68099`；旧采集显式 `ProtocolViolation` fail closed，新 helper 拉起后的会话 `rec_c8601752d61e49df8991524781a1834c` 有 3 chunks / 3 succeeded Jobs、0 dropped frames，并通过重启后搜索/回放/导出。
- 第一次 30 分钟运行达到 1914 秒、峰值 RSS 596 MiB、65 连续 chunks/Jobs，但 Catalog 证明只有 microphone source；原因是 UI 切换 smart 后立即保存仍持久化旧 `mic_only`。该运行严格作废。第二次运行前已退出读回 `recording_config.capture_mode=smart`，当前正在重新执行真实 30 分钟双源门禁。
- 第二次 `dual-source-30m` 正式通过：runner `1820s`；Catalog 共同有效时长 `1877.819625s`；microphone/system_audio 各 `63` 个连续 chunks、各 `63` 个 succeeded Jobs；drift `223ms`；dropped frames `0`；peak RSS `668 MiB`。重启后“人教版”搜索、回放和 `0600` Markdown 导出通过。
- 当前目录的 11 份报告已逐份重新验证；目录门禁准确停在首个缺失场景 `permission-denied`。
- 连续三轮外部状态复核后仍阻塞于同一条件：LifeSub 麦克风/录屏权限保持已授权且未获准切换；两副已配对蓝牙耳机均未连接、未暴露 Core Audio 输入；sleep/wake 需要用户在真实睡眠后唤醒。剩余 9 个场景因此不能在不改变安全设置或伪造硬件事实的前提下继续。

- 继续执行 `0.2.1` 最小发布范围；未拉回 Task 6、Task 9-16，也未添加 release marker、构建 DMG 或覆盖 `/Applications/LifeSub.app`。
- 报告采集器/验证器修复 release-blocking 真实性问题：无麦克风场景拒绝夹带 microphone probe/binding；permission-denied probe 绑定 selected/persisted/start-command identity；verifier 从 source monotonic envelope 自行重算 drift 并拒绝伪报低值。collector/verifier 自身纳入 dirty digest。
- 诊断快照不再接受 `probe_result=compatible` 但缺完整 probe；完整 probe 必须与 selected UID/name/transport/result 一致。Focused diagnostics `4/4`。
- 真实 smart 双源复现 `native capture protocol violation`。边界日志证明 helper 在 `source_started` 前发出 startup configuration-change 的 `source_recovering + chunk_sealed`。最终修复在所有 source identity 发布后才 `activate` adapter；startup 旧 notification 直接丢弃；startup/stop 增加 generation fence，停止后不得继续发布或激活来源。
- RED/GREEN 覆盖 startup notification、Chunk 提前封存、stop-during-startup publication/activation 和原子回滚。Swift 全量 `73/73`；最终合并复审 `Critical=0 / Important=0 / Minor=0`。
- 修复后的真实双源会话 `rec_4775fa3161d54442bac440e4efebd036`：microphone 5 chunks / 5 succeeded Jobs，system_audio 4 chunks / 4 succeeded Jobs，两路 dropped frames 0，18 个 persisted Segments；共同有效时长 `107.999625s`，source envelope drift `232ms`。
- 最终冻结 candidate：app SHA-256 `097ceb936702e44126a4c7b37fb558526cc46ff6bd9c93df41b5a4201b0fab42`；helper SHA-256 `1937bf38b2e167ce8437cd6732be82b3dbba76fb2f57b255c7339c695df24ef9`；gate SHA-256 `c1bafe9a101382cba688f67ec84a9d50c5e76e496afe1162bc3b8c45961d714d`；source `main@db39faafeac0e8f4816623456f3ffa8b1ab54564`；dirty digest `dc4f8166e26d2652016cc846f6cbda6891dd28aa7426c63acda7e946a00818a4`。Nested strict signature、arm64 与 app/helper audio-input entitlement 通过。
- Packaged provider gate：SenseVoice CER `0.0769` / RTF `0.0381` / RSS `465.1 MiB`；Whisper WER `0.0` / RTF `0.0646` / RSS `767.4 MiB`，均 passed。
- 新鲜 gates：collector shell、20-scenario mutation verifier、diagnostics `4/4`、Swift `73/73`、frontend production build、fmt、Clippy、diff；Tier 2 `583 passed / 0 failed / 7 ignored`；Tier 3 `626 passed / 0 failed / 8 ignored`。
- 正式 `npm run verify:release-source` 只在刻意缺失的 `NativeAsrDesktopRuntimeFactory` marker 上 fail closed；在最终 20 份报告通过前保持该状态。
- `output/asr-v0.2/native-capture-v3-84ee/` 与 `output/asr-v0.2/native-capture-v3-7802/` 是已作废诊断材料，不能进入最终目录验证。最终 `097c.../1937.../dc4f...` 身份尚无可发布 v3 报告，必须从 20 场景重新采集。
- 当前阻塞：FreeBuds 在 `system_profiler` 可见但未进入 LifeSub 可录输入枚举；external/Bluetooth selection 仍需真正可枚举输入。permission revoke/allow、disconnect/reconnect、helper crash、app restart、sleep/wake、same-UID recovery、30 分钟 dual-source 等仍需逐场景操作。完整目录通过前不得加 marker、打 DMG 或安装。

## 2026-08-26 V0.2.1 正式版范围确认

- 当前正式版目标固定为“可信的本地录音 + ASR 正式发布闭环”，执行 `2026-08-26-lifesub-asr-audio-settings-reference-optimization.md` 的 Task 1-5、Task 7-8。
- 新增 Chunk 0，对已经完成的原生采集、ASR、模型、Job/Evidence、UI 状态和打包链路做参考项目对照复核。复核只接纳会导致伪成功、身份误导、静默停帧、不可诊断/不可恢复失败或安装版主链路无法证明的细节。
- LifeSub 已经更强的不可变原始音频、精确来源/模型/运行时身份、认证单 helper、fenced Job、原子 Receipt/Revision 发布、append-only Revision 和 committed-only Evidence 保持不变，不因追平参考项目而降级。
- Task 6 与 Task 9-16 不进入 V0.2.1：模型自动卸载、场景配置/通用设置、存储保留、AEC、跨源去重、batch decode、实时预览和词典派生 Revision 均拆到后续优化版本。
- 当前执行顺序：参考复核分类 -> Task 1 -> Task 3 -> Task 4 -> Task 2 -> Task 5 -> Task 7 -> Task 8。诊断页排在 probe/recovery 合同稳定之后，避免重复修改状态 DTO。
- 本轮范围校准：已完成能力不再按“功能完成”直接视为“体验完成”。0.2.1 只继续吸收会造成伪成功、静默丢帧、身份/权限歧义、不可诊断/不可恢复失败或无法证明 packaged 主链路的优化；设置 IA、状态文案、probe 年龄/重测、模型阶段说明、Job 时间线和性能基线列为 0.2.2 的轻量优化；AEC、跨源去重、实时预览、归一化、保留/备份、Qwen streaming、batch decode、热切换等继续拆到独立后续版本。

## 2026-08-27 最终候选静态门禁更新

- canonical source 仍为 `/Users/goldenwave/Documents/MyProject/LifeSub`，`main@455c733786f76f66de910ec8c66f2062f537377d`；当前 dirty entries 为 121，保留既有集成改动。
- 最新 app-only candidate 已通过 verified Sherpa runner 重建；app executable SHA-256 `1550def5de43244689fd4a3e6edd07f48d21a6331969d43ed4030e3efc23c9ce`，bundled helper SHA-256 `7bb3e0e38b3df1e5babca7d032d22ee5bcba97d2a45697bb3924b2d931c7e7ca`，bundled gate SHA-256 `4176192b6045666f32b1ecde6b44e4e3a38485f780b8aa347c26ed64e291e0a7`。Nested strict signing、arm64 和 app/helper `com.apple.security.device.audio-input` entitlement 均已读回通过。
- 最新 packaged provider gate 已绑定 commit `455c733786f76f66de910ec8c66f2062f537377d`、scoped source digest `3346e509b120483d0dd308a4b9355c4e28954bbd89a7b6be8d7f2d144a895747` 和当前 gate executable；SenseVoice CER `0.0769`、RTF `0.0363`、峰值 RSS `618.4 MiB`，Whisper WER `0.0`、RTF `0.0569`、峰值 RSS `975.5 MiB`，两项均 passed。
- 最新 Tier 2 为 `582 passed / 0 failed / 7 ignored`；最新 Tier 3 为 `622 passed / 0 failed / 8 ignored`。`git diff --check` 通过。正式 `verify-release-source` 仍按预期 fail closed，因为 Native ASR capability marker 尚未获得真实 packaged capture/ASR hardware evidence。
- 真实 native v3 报告目录仍不存在；20 场景不能由测试 fixture 替代。Computer Use 检查确认 Mac 当前锁定，真实权限、设备切换、同 UID configuration recovery 和 dual-source-30m 暂停，未绕过锁屏。
- v3 verifier 的 20 场景 fixture 与 mutation-negative shell tests 新鲜通过，但这只证明 verifier 合同，不是目标机验收证据；缺少 `mic-only.json` 时 verifier 按预期 fail closed。
- 连续复核确认 Mac 仍处于锁屏，Computer Use 无法启动候选应用；真实权限、设备、同 UID recovery 和 dual-source-30m 报告继续缺失。不得用 fixture 或旧候选身份替代这些证据。

## 2026-08-27 解锁后候选应用真实探测

- Mac 已解锁，重新核对 canonical candidate：app executable SHA-256 仍为 `1550def5de43244689fd4a3e6edd07f48d21a6331969d43ed4030e3efc23c9ce`，bundled gate SHA-256 仍为 `4176192b6045666f32b1ecde6b44e4e3a38485f780b8aa347c26ed64e291e0a7`，source 仍为 `main@455c733786f76f66de910ec8c66f2062f537377d`。
- 通过直接运行项目 bundle 启动了精确候选。使用 `open` 首次启动时，macOS 因同 Bundle ID 选择历史 `/Applications/LifeSub.app`，产生 11:07 的旧安装包 startup abort；CrashReporter 的 `procPath`、Mach-O UUID 和当前候选进程均证明该报告不属于项目候选。后续验收必须继续直接运行精确 bundle，不能依赖 LaunchServices 选择。
- 候选诊断页真实读回：app hash 前缀 `1550def5de43...`、capture protocol V2、Sherpa 1.13.5、SenseVoice/Whisper Base `runtime_qualified`；麦克风权限初始为尚未请求，录屏与系统音频为未授权。
- 对当前选中的 `DTAudioPlugin / DTAudioDevice` 执行显式首帧 probe，候选 UI 返回“已通过测试 · 44.1 kHz · 2 声道”。这是真实候选的一次性无 staging probe 证据，但不是 v3 报告矩阵，也不能替代内置麦克风、外接 USB/Bluetooth、权限变换、同 UID recovery 或 30 分钟双源报告。
- 当前下一阻塞已从“Mac 锁屏”转为“需要用户允许 LifeSub 的录屏与系统音频权限，并提供/切换外接 USB 或 Bluetooth 麦克风完成物理场景”。在这些操作前不得添加 Native ASR release marker、构建 DMG 或替换 `/Applications/LifeSub.app`。

## 2026-08-27 真实内置麦克风首块时间修复与新候选

- 精确候选切换为 `mic_only + BuiltInMicrophoneDevice` 后，首帧 probe 真实通过（44.1 kHz、1 声道），随后真实采集进入 Running。首次会话 `rec_ae8827b9b5da48fcaa7685091d9634c5` 暴露：启动首块有 1,480 个 16 kHz 样本和 7,056 bytes，但 `duration_ms=0`，Worker 因 `InvalidSnapshot(chunk.duration_ms)` 按合同持久化 `recovery_required / failed to load execution snapshot`；后续 23.5 秒块仍成功转写。这是 packaged evidence 暴露的 0.2.1 release-blocking false-error。
- 根因位于 `ChunkEncoder`：AVAudioEngine callback 的 host time 是该 buffer 的起始时间，旧实现却在 append 后仍把它保存为 chunk end time。单 callback 短块因此得到相同 start/end host time，尽管已有非零样本。
- RED 新增 `singleCallbackChunkEndsAfterItsCapturedSamples`，修复前明确观察 `endHostTime - startHostTime = 0`，期望 1,480 / 16,000 秒即 92.5 ms。GREEN 后 encoder 按每次写入的 sample offset / sample rate 推进 host time，chunk rotation 内的分段也获得连续时间边界。
- 自动化：Swift helper `71/71`；Tier 2 `582 passed / 0 failed / 7 ignored`；Tier 3 `622 passed / 0 failed / 8 ignored`；`git diff --check` 通过。
- 新 app-only candidate：app executable SHA-256 `83d4e9a7cc8d7f2f11e9c8eac9c21f5e30285c5cd65b66ab096c7d526322cf73`，bundled helper SHA-256 `01f227b271c50462793595dd010998a8a3ab996c5935530fc60a2039949a6243`，bundled gate SHA-256 `4176192b6045666f32b1ecde6b44e4e3a38485f780b8aa347c26ed64e291e0a7`；当前 dirty entries `123`，dirty path digest `e1490b05213f57d36d9228381e5171203d2ca3a0df08ec860af529206b7e87f3`；nested strict signing 与 app/helper audio-input entitlements 通过。
- 修复后真实会话 `rec_d6ef6dd7584f4d3d826080068ecf0e1b`：3 个连续 microphone chunk 分别为 1,480 samples / 92 ms、480,000 samples / 29,995 ms、57,694 samples / 3,607 ms；3 个 SenseVoice Job 全部 succeeded，2 个 Revision、5 个非空 Segment 已持久化，UI 停止后正常显示真实转写且无 snapshot failure。
- packaged provider gate 已用同一新 bundle 内 gate 重跑：SenseVoice CER `0.0769`、RTF `0.0365`、RSS `480.6 MiB`；Whisper WER `0.0`、RTF `0.0785`、RSS `829.9 MiB`，均 passed。由于 app/helper identity 已变化，旧 v3 reports（目前本来也不存在）均不得复用。
- 架构单轮合并审查已完成：`Critical=0 / Important=0 / Minor=0`。审查确认 callback host time 起点语义、sample-offset 时间推进、单 callback RED/GREEN 与真实三块数据库证据一致。

## 2026-08-27 Packaged 搜索、回放与导出主路径加固

- 使用真实会话 `rec_d6ef6dd7584f4d3d826080068ecf0e1b` 验证：重启后 `BuiltInMicrophoneDevice / MacBook Pro麦克风` 选择恢复；搜索“白确认”命中；对应 chunk 回放进入 `播放中`；但旧 Markdown 导出仅触发 WebView download 并立即提示成功，文件系统没有任何输出。诊断导出则因 Tauri capability 缺少 `dialog:allow-save` 直接显示失败。
- 这是 Task 2/Task 7/Task 8 已批准主路径中的伪成功与不可诊断缺口，不是新增产品范围。RED：TranscriptView 导出回调失败时旧实现仍不会调用持久写入；Rust 不存在 exact session/revision Markdown export。GREEN：前端 `21/21`，Rust exact revision/no-follow export focused test 通过。
- Tauri capability 增加 `dialog:allow-save`；诊断与 Markdown 均使用原生 save dialog。Markdown 不再由 WebView 自行下载，而是把 `session_id + revision_number` 交给 Core，从当前 Catalog 精确读取 session/revision，再用统一 `O_NOFOLLOW + 0600 + sync_all` 私有文件写入；取消不提示成功，命令成功后才显示完成。
- 新精确候选 app SHA-256 `0a3b3831b26e50a867698c7a9ed9f4144528b5f417e3df5cda28dd524af7a835`，helper SHA-256 `01f227b271c50462793595dd010998a8a3ab996c5935530fc60a2039949a6243`，gate SHA-256 `476189161b18674c15bffa9438f2e0e919f86ccf102e02bf0ffa44c9a5790c93`；nested strict signature 通过。
- 真实导出证据：`~/Documents/LifeSub-diagnostics-2026-08-27.json` 为 `0600`，包含精确 app hash、权限、模型和存储事实；`~/Documents/实时记录 8-27 11-21.md` 为 `0600`，绑定 `rec_d6ef...`、`transcript_revision: 2`、`sense_voice` 与当前 Revision 文本。
- 最新候选再次完成真实内置麦克风闭环：会话 `rec_502f9cd70a52457cb50229a9ac2af04c`，两个 microphone chunk 为 92 ms 与 28,901 ms，两个 SenseVoice Job 均 succeeded，Revision r1 与两个非空 Segment 已持久化，UI 显示真实转写。
- packaged provider gate 已按新 gate 重跑：SenseVoice CER `0.0769`、RTF `0.0447`、RSS `465.9 MiB`；Whisper WER `0.0`、RTF `0.0749`、RSS `891.3 MiB`，均 passed。Fresh Tier 2 `582/0/7`，Fresh Tier 3 `623/0/8`。

## 2026-08-27 导出原子性复审与最终候选

- 导出合并审查首轮发现 `Critical 1 / Important 1`：直接 `truncate` 覆写现有导出在 write/sync 失败时会损坏旧文件；权限若在写后通过 path 收紧，存在内容暴露与 TOCTOU 间隙。进一步 RED 还证明 `O_NOFOLLOW` 不会拒绝 hardlink，旧实现会改写 hardlink target。
- 最终 `write_private_export` 改为：预检 final target 只允许 single-link regular file；在同目录 `create_new` 唯一临时文件；通过 descriptor 在写前设 `0600`；完整 `write_all + sync_all`；再 atomic rename 覆盖 final 并同步 parent directory。替换前任一失败都会清理 temp 且保留旧文件；symlink/non-regular/hardlink 全部 fail closed。
- RED/GREEN 新增：`write_record_markdown_export_preserves_existing_file_on_failure`、`write_private_export_atomically_replaces_with_private_permissions`，以及 hardlink target unchanged 负向断言。复审结果 `Critical=0 / Important=0 / Minor=0`。
- 最终冻结 app-only candidate：app SHA-256 `bb66bec22833f62d274c441fac56f7d762f69a08b317b75ce8143570bad12436`，helper SHA-256 `01f227b271c50462793595dd010998a8a3ab996c5935530fc60a2039949a6243`，gate SHA-256 `a874d333e8d964d0e633b395517009f007965553ff04c5c37edcd52009e07662`；dirty entries `128`，dirty path digest `bb31d9c208b6a4c86a202e424540f57c7ae2053c9f75a076d0fdc4c4d2caa0e4`。Nested strict signing 通过。
- 最终 packaged provider gate：SenseVoice CER `0.0769`、RTF `0.0390`、RSS `456.7 MiB`；Whisper WER `0.0`、RTF `0.0675`、RSS `761.9 MiB`，均 passed。最终 Tier 2 `582/0/7`，Tier 3 `625/0/8`。
- 最终哈希候选真实内置麦克风会话 `rec_41b0ac9351e140939dc67273e10fa249`：4 个 microphone chunk（92 ms、7,202 ms、19,692 ms、16,393 ms），4 个 SenseVoice Job 全 succeeded，3 个 Revision 与 8 个非空 Segment 已持久化；UI 录制中和停止后均显示真实转写。
- 最终哈希候选原子导出 `~/Documents/LifeSub-final-export-bb66.md` 已真实落盘为 `0600`，绑定 Catalog 会话 `rec_502f9cd70a52457cb50229a9ac2af04c`、revision r1、SenseVoice 和对应两个 Segment。此 smoke 验证当前 Core 导出实现，不替代仍缺失的 v3 20 场景报告。
- macOS“录屏与系统录音”页面已打开，LifeSub 历史条目显示 on；但最终 ad-hoc candidate 的 designated requirement 是新 `cdhash 5585a640271f294735f92fa7f2d0f77ff071c7e2`，旧开关状态不能证明当前候选授权。需要用户手动 off/on 该 LifeSub 开关，让当前 identity 重新授权；同时仍需连接 USB 或 Bluetooth 麦克风。
- 正式 `npm run verify:release-source` 仍准确 fail closed：`NativeCaptureRuntimeFactory` 尚未实现 `NativeAsrDesktopRuntimeFactory`。空目录 v3 verifier 也准确失败于 `missing report: mic-only`。在 20 份同 identity 报告通过前不得添加 marker、构建 DMG 或安装。

## 2026-08-27 版本收口与后续版本分层

- 参考项目复核后的版本建议固定为三层：`0.2.1` 只完成真实 packaged 硬件/权限矩阵、release marker、DMG/安装链路和被这些证据暴露的 release-blocking 修复；`0.2.2` 只做轻量的设置/探测/模型/诊断可读性与性能基线优化；Task 9-16 继续放到后续独立版本。
- 已完成能力不再因“参考项目还有更完整设置或工作流”而重开核心设计。只有伪成功、静默停帧、身份/权限歧义、不可诊断/不可恢复失败、或 signed packaged 主链路无法证明时，才允许继续把参考细节拉回 0.2.1。
- 因此默认的下一版本不是继续做 Task 9-16，而是基于现有合同做轻量收敛：设置 IA/summary、probe age/re-test、recovery 状态文案、model operation timeline、diagnostics next-step hints，以及 startup/first-frame/RTF/RSS/cancel 的量化基线。
- 进一步收口：`0.2.2` 视为完成的上限固定为四类工作：`(1)` 设置与诊断 IA/summary，`(2)` probe/recovery/model/job 状态可读性，`(3)` 基于现有 snapshot 的 copy/open/re-test 操作性增强，`(4)` 固定 fixture 与目标机性能基线报告。凡涉及 schema migration、新 helper/process、source/chunk/Receipt/Revision 图变更、新 release-marker 语义或新增 native matrix 类别的工作，默认移出 `0.2.2`。
- 已将上述判断落成计划文件中的 `0.2.2 done line + task bundle`：S1 设置/诊断 IA，S2 probe/recovery/model/job 状态可读性，S3 diagnostics 操作性增强，S4 baseline report pack。后续若提案不属于这四包之一，默认不进入下一版本。

## 2026-08-26 Task 1 设置真实性完成

- 新增 Core 权威 `SettingsCapabilities`：production capture output 固定 `16_000 Hz`、真实 app data directory、`im_detection_available=false` 和生产支持的 capture modes。React 不再从旧持久字段猜测能力。
- `RecordingConfig` 保持旧 JSON 可读和麦克风 UID/name 不丢失；读取/保存时把 sample rate、storage path、IM 状态收口到生产事实。录音设置改为只读显示 16 kHz 与真实数据目录，不再暴露没有生产消费者的 IM/检测延迟/恢复延迟交互。
- `AsrConfig` 新增向后兼容 `whisper_task`；旧 Whisper 配置默认 `transcribe`，未知 task fail closed。语言选项来自选中模型 `supported_languages`；SenseVoice 只显示 ITN，Whisper 只显示 `transcribe/translate`，Qwen 不显示不适用 Provider 选项。Whisper task 已进入实际 Provider execution snapshot。
- RED 证据：service tests 缺 `getSettingsCapabilities/loadSettingsCapabilities`；Settings tests 复现旧 sample/storage 可编辑、语言静态、Whisper task 缺失。
- GREEN 证据：前端 focused `37/37`；Rust domain `11/11`、settings capability `1/1`、Whisper execution mapping `1/1`、recording projection `1/1`；production frontend build、`cargo fmt --check`、无 `console.log`、`git diff --check` 全通过。
- 独立规格复核 Critical/Important/Minor = 0。代码质量复核未发现需要返工的问题。由于当前 canonical tree 的同一 owned files 已包含大量未提交集成改动，Task 1 独立 commit 延至统一 checkpoint，避免拆分或误带既有修改。

## 2026-08-27 Task 3 精确 UID 首帧兼容性 probe 完成

- Swift/Rust capture protocol 新增 capability `microphone_probe`、精确 UID `probe_microphone` command，以及 `microphone_probe_result`。结果覆盖 compatible、permission denied、unavailable、identity mismatch、invalid format、no audio frames、route conflict、timeout。
- Compatible 结果必须绑定非空 UID/name、有效 input sample rate/channel、同 UID readback 和非零 first-frame sample count；identity mismatch 必须给出不同 readback UID；unknown result、extra fields 和不自洽状态 fail closed。
- `MicrophoneProbe.swift` 使用与 production capture 相同的 Core Audio resolver/binder/readback 和共享 format-settle 函数；显式 probe 才延迟构造 AVAudioEngine，passive listing 不分配 engine、不请求权限。probe 安装临时 tap、等待有效非空 PCM frame，并在成功、timeout、mismatch 和失败路径停止 engine、移除 tap。
- Probe 不持有 `ChunkEncoder`、staging root、CaptureSessionController 或 Catalog。CommandLoop 测试证明 probe 不创建 controller/staging；持久 capture coordinator 收到 probe response 会按 protocol violation 拒绝，禁止结果串入 source/chunk/Job/Evidence。
- Rust authenticated one-shot probe 使用一个总 deadline，发送 exact UID，只接受一次 probe result，再发送 shutdown 并要求 shutdown ack；重复/额外响应、missing ack、timeout 和 cancellation 会 terminate/reap。取消若发生在 launch 后、command 前，不再误发 probe。
- Tauri `probe_microphone` command 与 TypeScript DTO 已接通。共享 microphone controller 使用 generation fencing，选择变化会清空旧 probe 结论，迟到结果不能覆盖新 UID；录音页和设置页提供图标按钮及明确 compatible/error 状态。
- RED：协议类型/分支、one-shot API、Swift live probe、CommandLoop、Tauri service/controller/UI 均先因目标行为缺失失败；取消竞态回归修复前证明 command 被错误发送。
- GREEN：Swift 全量 `62/62`；Rust capture protocol `13/13`；capture helper `22/22`；前端 focused `57/57`；production frontend build、desktop Cargo check、fmt、无 `console.log`、`git diff --check` 全通过。
- 质量复核发现并修复两个 Important：launch 期间取消仍发送 command；passive listing eager 构造 AVAudioEngine。修复后 Critical/Important/Minor = 0。真实 built-in 与 external/Bluetooth probe 报告按计划在 Task 7 收集。

## 2026-08-27 Task 4 同 UID Audio Engine recovery 代码完成

- 根因确认：旧链路的 `device_configuration_changed + recoverable=true` 只让 controller seal、Rust Pending，不执行任何 engine teardown/rebind/restart，也不证明新帧，因而可能假健康。
- 新增严格 `source_recovering` / `source_recovered` 协议事件，绑定 source、非零 generation、reason、exact device UID 和 recovery 首帧 host time。Rust coordinator 仅允许 `Running -> Recovering -> Running`；旧 microphone `recoverable=true`、无 recovering 的 recovered、旧 generation 或 UID mismatch 全部 fail closed 为 ProtocolViolation。
- 新增 `AudioEngineRecovery` 单一调度器：20ms debounce、2 秒窗口最多 3 个实际 attempt、pending burst coalesce、active attempt 吸收重复通知、stop/cancel generation fencing、burst limit 单次失败。参数为 LifeSub 当前配置变化实测与测试边界，不复制 TypeWhisper 常数。
- `MicrophoneCapture` 的 initial start 与 recovery 共用 exact resolver/binder/readback/format-settle/tap/first-frame 路径。恢复有序 await `source_recovering`，停止旧 engine/tap、drain 队列、seal discontinuity，再按同 UID 重建并等待新帧后发送 `source_recovered`。
- Recovery 中的配置通知不会并发启动第二 attempt；startup 未提交首帧时 recovery 等待 commit；stop 或 generation 变化发生后，旧 Task 在 restart 前再次 fencing，禁止复活已停止/新会话。
- Chunk 回归证明恢复前后两个 microphone WAV chunk 的 sample range 连续，第二块 `startSamplePosition ==` 第一块 `endSamplePosition`；encoder/source 身份不变。失败/断连路径只 stop/remove 一次，不重复 teardown。
- RED/GREEN 包括：恢复事件协议、Rust Recovering 状态、同 UID rebuild、新首帧、startup interleaving、stop cancellation、burst limit、UID mismatch、断连和 chunk continuity。
- 新鲜自动化：Swift 全量 `69/69`；Rust capture coordinator `6/6`；capture protocol `14/14`；desktop Cargo check、production frontend build、fmt、无 `console.log`、`git diff --check` 全通过。
- Task 4 尚缺真实 target-Mac configuration-change smoke。该证据与 Task 7 packaged built-in/external/Bluetooth 矩阵一起收集；在报告通过前不得把 Task 4 表述为 release-evidence-complete。

## 2026-08-27 Task 2 diagnostics DTO 与脱敏边界子闭环

- Task 2 已完成。Rust typed snapshot、helper permission query、AppState 聚合、UI、导出/系统设置动作和视觉证据均已接通。
- 新增 `DiagnosticsSnapshot`，固定 permission、selected microphone/probe、app/helper/runtime identity、model identity/state、storage usage 和 recent stable error 字段。
- App/helper/runtime/model 身份使用固定 SHA-256 或精确 runtime commit/build id；模型绝对安装路径和 raw runtime JSON 不进入 DTO。
- `to_redacted_pretty_json` 验证 schema、权限枚举、hash、model identity 和 error 字段。测试证明导出不含 launch nonce、API secret、transcript/audio 内容或 LifeSub data root 外的模型路径。
- RED：`diagnostics` 模块不存在；GREEN：no-default diagnostics focused `2/2`。
- Helper 新增 capability-gated `get_permission_state` one-shot。麦克风权限使用 AVCapture 精确三态；屏幕录制只用 `CGPreflightScreenCaptureAccess()`，truthful 投影为 `granted/not_granted`，不触发 request prompt、controller 或 AVAudioEngine。
- Rust snapshot 聚合当前 app executable SHA、编译期 helper code SHA、capture protocol、固定 sherpa version/commit/archive/build id、manifest bundle identity、qualified runtime identity hash、实际 data root 与 Catalog/audio/model bytes。
- 最近 microphone probe 按 exact UID 缓存；选择变化或进程重启后显示未测试。最近 capture error 同时读取 active worker stable error 与 start/stop/pause/resume 命令缓存。
- Storage 统计使用 `symlink_metadata` 且不跟随 data root 内 symlink。导出重新验证 DTO，要求绝对路径、`O_NOFOLLOW` 与 0600；macOS Settings target 仅 allowlist microphone/screen_recording。
- Diagnostics Settings Tab 使用现有 Token 和全宽 section，覆盖 loading、permission、device/probe、identities、models、storage、recent error、refresh generation fencing、export 和 privacy actions。
- GREEN：diagnostics Rust `5/5`；frontend focused `42/42`；frontend full `116/116`；Swift full `70/70`；production build；最新 Tier 2 `582 passed / 0 failed / 7 ignored`，Clippy `-D warnings`、fmt、diff 全绿。
- 视觉证据：`output/playwright/diagnostics-settings-desktop.png`、`output/playwright/diagnostics-settings-760.png`；浏览器 console errors/warnings = 0。
- Task 2 质量复核发现并修复：screen permission 不可伪分 denied/not-determined、active worker error 未进入 diagnostics、export symlink 风险、error pill 继承 success border、固定 sleep 导致 Swift 并行测试脆弱。当前 Critical/Important/Minor = 0。

## 2026-08-27 Task 5 durable model operations 完成

- Catalog download projection 从单一 state 扩展为 exact download ID、downloaded/expected bytes、artifact index/count、error code/summary；所有字段来自持久 `model_downloads` 与 artifact checkpoints，不扫描临时目录猜测。
- Target ASR 模型卡会先投影其 Silero VAD dependency 的当前 download；VAD ready 后再投影 target download，解决首段依赖下载不可见问题。旧 manifest/bundle download 继续过滤。
- AppState 增加 active operation cancellation registry，VAD 与 target 共用一个 cancellation flag。取消命令只返回 request acknowledgement，不提前把 Catalog 伪写为 cancelled；下载器实际观察 cancellation 后保存 checkpoint/state，UI 持续 polling 到 Catalog ack。
- Retry 命令绑定原 download ID、current manifest/bundle identity、依赖关系和 latest retryable operation；stale bundle、错误 model、非 terminal state 或被新 operation 替代均 fail closed。实际 retry 复用原 artifact checkpoint。
- 新 `ModelManagerController` 分离 refresh/poll 与 operation generation，覆盖 stale result、operation replacement、remount Catalog restoration、cancel ack、exact retry、dispose fencing 和同步 adapter error。组件卸载不取消后台下载；重新打开从 Catalog 恢复。
- UI 显示 accessible progressbar、精确字节、阶段、artifact 位置、稳定错误、取消/正在取消与重试。Promise 返回不再被当作安装成功，最终状态只来自 Catalog refresh。
- 受控 transport 证据：`interrupted_download_resumes_with_range_and_if_range`、`mid_stream_cancellation_persists_actual_bytes_and_validator`、`stalled_response_observes_cancellation_after_bounded_read_timeout` 全通过；ModelManager no-default focused `88/88`。
- 前端 full `124/124`、production build 通过；最新 Rust Tier 2 `582 passed / 0 failed / 7 ignored`，Clippy `-D warnings`、fmt、diff 全绿。
- 视觉证据：`model-download-active.png`、`model-download-failed.png`、`model-download-resumed.png`、`model-download-qualified.png`。Resumed 与 active 使用同一 `mdl_demo` DTO identity，字节由 120MB 增至 180MB；clean fixture console errors/warnings = 0。
- 质量复核发现并修复：VAD dependency progress 缺失、cancel 提前成功风险、retry operation replacement、stale poll 被手动 refresh 永久停止、cancelled 被误报 error、同步 adapter throw、测试 polling 泄漏/OOM。当前 Critical/Important/Minor = 0。

## 2026-08-27 Task 7 acceptance verifier v3 完成、真实矩阵待收集

- Native acceptance schema 从 v2 升级为 `lifesub.native-capture-acceptance.v3`，场景从 19 增至 20，新增 `microphone-same-uid-recovery`。
- 所有实际 microphone capture 场景必须绑定 exact UID/name/transport、compatible result、有效 input format、同 UID readback、非零 first-frame sample count 和 elapsed time。Permission-denied/prestart-unavailable 绑定明确失败 probe 且不得伪造 frames。
- Same-UID recovery 必须绑定非零 generation、reason、recovering/recovered exact UID、source monotonic range 内的新首帧、前后同 capture source 且 sequence/sample 连续的 chunk、无 fallback 和 post-recovery Job/Receipt/Revision/chunk IDs。
- 新增负向变异覆盖 missing/empty probe identity、invalid transport、readback mismatch、zero frame、zero generation、recovered UID mismatch、missing post chunk 和 fallback；既有 mixed app/helper/source identity 负向门禁保留。
- Shell verifier GREEN：20 个正向 fixture + 全部 mutation-negative 通过。
- Task 7 Step 3–5 仍待最终 candidate：内置及 external/Bluetooth、permission denied/allow/revoke/no-device UI、same-UID config recovery 与 30 分钟 dual-source。报告必须绑定同一 app/helper/source hashes。

## 2026-08-27 Task 8 provider gate 接通与模型质量结果

- 旧 `lifesub-asr-gate` 的 `run_scenario` 只是 scaffold，无法作为发布证据。新增 `asr::packaged_gate::transcribe_installed_fixture`，通过当前 LifeSub `CoreRuntime` exact executable installation lease 和 `ProductionProviderFactory` 执行真实模型，不接受裸模型路径或其他应用 artifacts。
- Gate 现在使用模型自身安装目录内的上游固定 fixture：SenseVoice `test_wavs/zh.wav`（sha256 `b77f1794...`，上游参考文本“开放时间早上9点至下午5点。”）和 Whisper Base `test_wavs/0.wav`（sha256 `6bc58a4e...`，固定英文文本）。不再用 Qwen 混合语种样本错误衡量 SenseVoice。
- 真实结果：SenseVoice CER `0.0769`、RTF `0.0325`、峰值 RSS `555.5 MiB`，通过 CER <= 20%；Whisper WER `0.0`、RTF `0.0587`、峰值 RSS `908.2 MiB`，通过 WER <= 20%。两者均 exact manifest/runtime-qualified installation，provider gate 最终退出码 0。
- Gate 执行中发现并修复真实 sherpa 参数缺陷：Whisper `language=auto` 在 sherpa 1.13.5 中必须投影为空字符串，直接传 `auto` 会返回 `Invalid language: auto`。同时无 cross-attention 的模型关闭 token timestamp 开关，避免把不可用能力当作输出。
- Whisper 模型通过 LifeSub `--install-missing` + 官方 GitHub release URL 安装，期间第一次直连超时并持久为 failed；使用机器已配置的官方 SOCKS5 代理重试，同一 ModelManager checkpoint 最终 `runtime_qualified`。没有使用未在 manifest 中声明的镜像。
- Gate report：`output/asr-v0.2/packaged-provider-gate.json`，绑定 tested commit `455c733...`、source digest `73275b38...`、fixture hashes、runtime archive identity、model identities 和实际指标。
- Task 8 仍未完成：当前 candidate 在 gate 之前构建，尚未重新打包最终最新 source；native v3 real hardware reports、DMG、安装版和 release marker 仍待通过。

## 2026-08-26 最终 system-only 复测与 smart 硬件阻塞

- 最终 app-only 候选 executable SHA-256 `40a4eda80f134ff2915cc4081218c912ce03b5019c8ea81e28e6c79c7dd0a8dd`、Mach-O UUID `18ECE4D4-2631-34BF-A8D2-E09CCDEC21AD`，bundled helper SHA-256 `2b198f5ae316b1703b254c52ec770bb829d5b01b700c8c9f4aaa7d4f0e068eb9`。用户对该固定 identity 完成 ScreenCapture 关/开授权后，system-only 直接进入单一“正在记录”状态。
- 最终真实 system-only 会话 `rec_adb953eb4e914bb2bf3b06ee8f0f6d72` 产生 4 个 `system_audio` WAV chunk；4 个 Job 全部 `succeeded`、4 个 Provider Receipt、0 error。两个含语音块各发布真实 Segment；静音完整块与停止尾块成功但不生成空 Revision/Segment，UI 不再出现 `InputUnavailable`。
- 静音根因分两层完成 RED/GREEN：Native engine 允许 0 utterance 生成空成功结果；`partition_detector_cores` 允许空 detector core 列表作为正常静音，同时继续拒绝倒序、重叠、零长度和越界 core。Catalog 以同一 fenced transaction 保存成功 Receipt 和 Job 状态，但不创建空 Revision。Tier 2 最新 `569 passed / 0 failed / 7 ignored`。
- smart 双源真实尝试：MacBook Pro 内置麦克风曾建立 `microphone + system_audio` 两个 source，随后在当前系统默认输入为 HUAWEI FreeBuds 7i 时收到设备配置变化并被判定为 selected-device disconnect；HUAWEI FreeBuds 7i 与 DTAudioPlugin 均可枚举为 connected，但 2 秒内未交付首帧，稳定返回 `selected_microphone_unavailable`。应用均 fail closed、无伪录音、无新崩溃。当前设置已恢复为经过验证的 `system_only`；完成 smart 证据需要将 macOS 默认输入切到 MacBook Pro 麦克风或接入可稳定产帧的 USB 麦克风后再测。
- 用户看到的 CrashReporter 文件是历史 `lifesub-2026-08-26-194823.ips`：报告时间 19:48、旧 binary UUID `713059E1-5FBA-3CA1-A2EF-4E79E34C7877`，faulting main thread 位于 Tauri `did_finish_launching` 的 `panic_cannot_unwind`，属于旧候选重复实例/ownership 启动异常。当前最终候选 UUID 不同，19:48 后无任何新 LifeSub crash report；本轮麦克风失败是受控错误，不是进程崩溃。

## 2026-08-26 system-only 真实验收与静音语义修复

- 用户明确同意后已执行 `tccutil reset ScreenCapture com.goldenwave.lifesub`，旧 ad-hoc ScreenCapture requirement 被清理；同一冻结候选重启并重新授权后，system-only 成功进入单一“正在记录”状态，不再报权限拒绝。
- 真实 system-only 会话 `rec_f7c42e91a8224846a9125b5216df22e9` 产生 6 个连续 `system_audio` WAV chunk；5 个完整 30 秒块均为 480,000 samples、0 discontinuity、0 dropped frames，sample/host duration 偏差在约 0 ms，尾块 1.7 秒。4 个含 `say -v Ting-Ting` 中文语音的完整块全部生成 succeeded Job、Provider Receipt、`verified_local_asr` Revision 和非空 Segment，UI 可见真实中文转写。
- 首个录制前纯静音块与停止后的 1.7 秒静音尾块被 VAD 判定为 0 utterance，但旧 `transcribe_all_windows` 将空 utterance 集误报为 `InputUnavailable`，从而污染整场 UI。已完成 RED/GREEN：VAD 静音现在返回空文本成功；Catalog 在同一 fenced transaction 中保存 Provider Receipt 并把 Job 标记 succeeded，但不创建会覆盖 Timeline 的空 Revision，Worker 也不发送空 publication 回调。
- 新增回归 `vad_silence_produces_a_successful_result_without_segments` 与 `publishes_silent_success_without_an_empty_revision`；focused native engine `7/7`、ASR service `24/24`，Tier 2 `568 passed / 0 failed / 7 ignored`，desktop check、fmt、Clippy `-D warnings`、diff 与 production frontend build 全绿。
- 已用 verified sherpa runner 重建 app-only 候选。当前 executable SHA-256 `5a59b3a37c2683adf14fb5c1cedf084c48a2d602cf43ba39f37f1d039f0cf425`，bundled helper SHA-256 `2b198f5ae316b1703b254c52ec770bb829d5b01b700c8c9f4aaa7d4f0e068eb9`，app CDHash `5d39140d235ccab314bf126eb8658182e1e6b4c9`，nested strict codesign 通过。当前唯一人工阻塞：用户需在 macOS“录屏与系统录音”中对新 ad-hoc identity 再开启一次 LifeSub；完成后重启并复测静音 system-only，再继续 smart 双源。

## 2026-08-26 启动恢复、system-only 配置与权限语义

- 异常退出恢复已完成真实 RED/GREEN：旧候选退出时留下 `rec_8cc4aa3402c944eda0b20d640d085d30` 为 `recording` 且含 `.wav.partial`。启动初始化现在在独占 Core ownership 后清理严格识别的 current/legacy `capture-staging`，并把 `recording/paused` 会话按最后持久化 chunk timing（无 chunk 则 `started_at`）转为 `stopped`。真实重启后遗留会话已停止，staging 为 0 文件/0 会话目录，最新真实 Revision 在 Timeline 可见。
- staging 新建路径收紧为 root/session `0700`；相关 Service `91/91`、desktop Session `10/10`，最终恢复实现复审 Critical/Important/Minor = 0。
- 录音设置保存后立即启动曾继续使用旧 mode。根因是 LiveCapture 依赖延迟更新的 React ref。启动现在先锁 lifecycle、等待麦克风选择保存队列、重新读取持久化 config，并 action-time 解析 mode/UID/availability；失败选择回退最后持久化配置，mic-required discovery 未就绪继续阻止，system-only 不等待麦克风。前端全量 `102/102`，复审 Critical/Important/Minor = 0。
- system-only UI 已隐藏麦克风下拉，显示“系统音频 / 系统录音权限”；Swift 权限错误现在携带 source，稳定区分 `microphone_permission_denied` 与 `screen_recording_permission_denied`。Swift 全量 `57/57`，真实 system-only 已正确显示“录屏与系统录音权限未授权”，不再误报麦克风。
- 最新门禁：Tier 2 `566 passed / 0 failed / 7 ignored`；Tier 3 `597 passed / 0 failed / 8 ignored`；fmt、Clippy `-D warnings`、diff、frontend production build 全绿。
- 当前 app-only 候选 executable SHA-256 `7093178c34780810754e965d2f45cea869ebb6a6abd725cfedcf5b84a71f4e39`，bundled helper SHA-256 `2b198f5ae316b1703b254c52ec770bb829d5b01b700c8c9f4aaa7d4f0e068eb9`，dirty path digest `7d6379dc50fd94e58ebc3e76feb0056b25fe07aaff1bd3fefa88770797b20466`（99 paths）。当前唯一人工阻塞：用户需在 macOS 屏幕与系统音频录制设置中为 LifeSub 开启权限；开启后继续 system-only/dual-source 实录。

## 2026-08-26 packaged WAV -> ASR 闭环与重采样漂移修复

- 真实 packaged `mic_only` 会话 `rec_099bb3fde39d450ab8cec0daeaaf9fa8` 已产生 5 个连续 `.wav` chunk、5 个成功 ASR Job、5 个本地 Provider Receipt、5 个 `verified_local_asr` Revision 和 6 个非空 Segment；`input_unavailable` 已消失，UI 跨过多个 30 秒 rotation 后持续显示真实中文转写。
- Catalog 同时暴露稳定时间漂移：每个 480,000 个 16 kHz 输出样本对应约 32.4 秒 host/session 时间。独立 AVFoundation 探针证明 `MonoPcm16Resampler` 每个短硬件回调都重建 `AVAudioConverter`，导致重复丢弃 priming frames；44.1 kHz 一秒输入按 256-frame buffers 只累计 14,982 个输出样本，而不是约 16,000。
- 两轮 RED/GREEN 已固定连续流与采样率切换：连续短 buffer 修复前 14,982、修复后约 15,994；`44.1k -> 16k direct -> 44.1k` 修复前恢复首 buffer 为 93 samples、fresh 为 87，现 direct path 会清空 converter 状态。Swift helper 全量 `57/57`，最终两文件复审 Critical/Important/Minor = 0。
- 当前冻结 app-only 候选主程序 SHA-256 `587535fd30f9fb4688b67b0089c07e8d5e8909cc191b79c3379998b853d93831`，bundled helper SHA-256 `d9656790c480efcb815d253414f00cb6380dcecdde101d93986589da152c0d2a`；verified sherpa runner 构建、nested strict codesign、arm64 和 app/helper audio-input entitlement 均通过。
- 当前新 ad-hoc identity 等待 LifeSub 麦克风系统弹窗的“允许”。允许后不再重建，直接复测 30 秒 chunk 的 sample-duration/host-duration 偏差、WAV -> Job -> Receipt/Revision/Segment 和重启 Timeline。

## 2026-08-26 packaged native chunk -> ASR 闭环推进

- 真实 packaged `mic_only` 首个 30 秒 rotation 的 `ProtocolViolation` 已完成根因 RED/GREEN：旧 source metadata 使用 `mach_continuous_time()`，而 `AVAudioTime.hostTime` 属于 `mach_absolute_time()` 域；机器休眠后两者永久偏移。麦克风启动现在等待首个成功转换并提交的非空 PCM frame，metadata 冻结该帧 host time；无首帧时 2 秒有界失败并清理 tap/engine。Swift 新增等待、首帧锚点和超时清理回归，全量 `55/55`。
- 系统音频预启动锚点也从 continuous time 改为 `mach_absolute_time()` 同域。回归在修复前测得约 `23,820` 秒累计偏差，修复后进入 100 ms 阈值；避免 sleep/wake 后双源 session anchor 下溢。
- rotation 随后推进到 `CommitFailed`，根因是 Swift staging 文件名 `chk_<id>.microphone.caf.partial` 与 Rust sealer 的 `chk_<id>.<ext>.partial` 安全 grammar 漂移。helper 已统一生成不含 source 中间段的 canonical 名称；Rust sealer 保持严格 grammar，不放宽路径攻击面。
- 原生 chunk 原先只原子落库、不创建 ASR Job。现已复用 imported-audio canonical enqueue，sealer 成功后才回调 enqueue；新 Rust 回归证明回调发生时 final 文件、chunk 行和 timing 行均已持久化。真实 packaged 复测产生 6 个 ASR Job，证明 native capture -> Job 已接通。
- 上述 6 个 Job 首轮均 `input_unavailable`。真实 `.caf` 经 `afinfo` 验证为有效 16 kHz/mono/Int16，但当前 Symphonia 只启用 WAV/FLAC/MP3/OGG 等格式，不支持 CAF demux。helper 改为先写合法 `.wav`，关闭并 fsync 后原子重命名为 `.wav.partial`，文件头回归严格要求 `RIFF/WAVE`，权限固定 `0600`；Swift 全量仍 `55/55`。
- 当前冻结 app-only 候选路径 `src-tauri/target/release/bundle/macos/LifeSub.app`，主程序 SHA-256 `32cc828e47a949e4ffbef2dd4b90750e6746f8682e6334a2c4104964b99520d6`，bundled helper SHA-256 `1608ed319a8f2f228f6753aac27989157472bcde5ec9f9a9fa8c33feb937bfe8`，nested strict codesign 通过。当前该新 ad-hoc identity 的 LifeSub 麦克风弹窗仍等待用户点击“允许”；允许后不再重建，直接做 35 秒 WAV -> Job -> Receipt/Revision/Segment 验收。

## 2026-08-26 麦克风权限参考实现复核

- TypeWhisper 采用三态分支：已授权直接返回，未决定才请求，拒绝后只引导系统设置；OpenWhispr 将权限查询与申请拆成独立 IPC，并让已授权 UI 不再展示授权按钮。LifeSub helper 的权限分支已与该原则一致，录制前重复执行 preflight 不会在稳定身份的 `.authorized` 状态触发系统申请。
- 当前重复弹窗根因仍是签名身份：`tauri.conf.json` 使用 ad-hoc `signingIdentity = "-"`，当前 app/helper designated requirement 都是构建相关 `cdhash`，本机没有可用 codesigning identity。参考项目正式包使用稳定 Team/证书签名，因此版本更新后 TCC 可以匹配同一应用身份。
- 后续权限体验可按参考项目拆为“只查询状态 / 用户显式申请 / denied 打开系统设置 / 录制启动只消费结果”；但在稳定签名证书可用前，这只能改善交互，不能消除每次重建后的重新授权。

## 2026-08-26 packaged 仅麦克风权限与 format settle 复测

- 用户截图中的权限页是“录屏与系统录音”，不是麦克风。Catalog 记录证明 `recording_config.capture_mode` 在 08:41 才保存为 `mic_only`；08:39 的旧启动仍是 `smart`，因此当时请求系统音频权限符合旧模式。当前 `mic_only` 真实复测只产生 `kTCCServiceMicrophone`，不再请求 ScreenCapture。
- 系统设置当前显示 LifeSub 麦克风已开启；但候选使用 ad-hoc hardened-runtime 签名，每次重建改变 code requirement。TCC 日志对新候选明确记录 `Failed to match existing code requirement`，随后显示新的 LifeSub 麦克风系统弹窗。这是开发候选身份变化，不是稳定签名产品每次启动都会重复请求。
- 真实 packaged 启动再次复现 AVFAudio crash：绑定内置麦克风后 hardware 为 44.1 kHz，`AVAudioEngine` 首次读取仍返回 48 kHz，安装 tap 触发 `Input HW format and tap format not matching`；Rust 因 helper 异常退出只显示 `ProtocolViolation`。
- 根因修复不再只依赖调用顺序：Swift 从所选 Core Audio device 读取标称采样率与输入声道，在 500 ms 总 deadline 内每 10 ms 读取 input-node format，只有与所选硬件格式一致后才安装 tap。超时映射到 `selected_microphone_unavailable`，禁止 Objective-C exception 崩溃或泛化设备错误。
- RED/GREEN：新测试先观测 tap 使用 48 kHz 且仅读取一次；修复后等待 44.1 kHz 再安装。第二个 RED/GREEN 先观测 settle timeout 为 `deviceUnavailable`，修复后稳定映射为 `selectedMicrophoneUnavailable`。
- 新鲜验证：Swift helper `52/52`；LiveCapture `28/28`；前端 production build 通过；`git diff --check` 通过；最终窄范围合并复审 Critical=0 / Important=0 / Minor=0。
- verified sherpa runner 重建的当前 app-only 候选通过 nested `codesign --deep --strict`，app/helper 均有 `com.apple.security.device.audio-input`。app executable SHA `4e7152b0765ea45a84748c604778d36302ba30e99373224904210d3fe06f4ac2`，bundled helper SHA `18cffa7ab732c4c2f4e4694ca9f37f84e94f0bf05aed5bef8735b54904194986`，pre-bundle signed helper SHA `b602d5ba2a2e149841c805787a13e08da4208ade8812d6f03b332e7c7f8c7a10`。
- 当前真实阻塞点：该新候选的 LifeSub 麦克风系统弹窗仍在屏幕中央，需用户手动点击“允许”。允许后继续 `mic_only -> recording -> speech -> stop -> sealed chunk -> real ASR -> Receipt/Revision/non-empty Segment` packaged 闭环。
- 录制页 mode-specific 启动文案已完成 RED/GREEN：`mic-only` 不再显示“连接系统音频”；此 UI 改动尚未重新打入当前等待授权的候选，避免再次改变 TCC code requirement。完成真实 helper 复测后再做最终重建。

## 2026-08-26 麦克风选择 Task 收口与 packaged 候选验证

- 麦克风设备选择 Task 已完成并通过最终合并复审：Critical=0 / Important=0 / Minor=0。首版支持内置、USB、外接、蓝牙等 Core Audio 输入的显式选择；录制中锁定，禁止静默回退；热切换仍保留为后续 Todo。
- Swift helper 现按稳定 Core Audio UID 精确解析设备，设置并读回 `kAudioOutputUnitProperty_CurrentDevice`，再读取绑定后 hardware format、安装 tap 并启动 engine。设备列表/alive/current-device/engine configuration listeners 会重查目标 UID；断连发出一次性 `selected_microphone_disconnected`，stop/error/deinit 清理监听。
- 真实 packaged 启动曾复现 `Input HW format and tap format not matching`：旧实现先读取默认设备 48 kHz format，再绑定内置麦克风 44.1 kHz。操作顺序 RED/GREEN 已固定为 `resolve -> bind -> readback -> outputFormat -> observe -> tap -> start`，不再使用旧设备 format。
- Rust authenticated one-shot discovery 现在使用单一总 deadline，支持 async cancellation，并在 success/timeout/cancel/protocol error 全路径 orderly wait 或 terminate/reap。`smart`/`mic-only` 缺 UID、`system-only` 带 UID、helper `source_started` UID mismatch 和选中设备断连都保留稳定 fail-closed 错误。
- Tauri bundle 会二次签名 sidecar，导致完整文件 SHA 和 helper identifier 变化。发布认证已改为：strict code-sign verification + LifeSub helper identifier allowlist + `codesign --remove-signature` 后的 canonical code SHA 精确 attestation；父子握手仍比较完整文件 device/inode/SHA，防止 launch-time 替换。真实 packaged RED 为 `SignatureInvalid`，修复后设备 discovery GREEN。
- macOS hardened runtime 首次 packaged 启动因缺少 `com.apple.security.device.audio-input` entitlement 被 TCC 拒绝且不弹提示。已新增 `src-tauri/Entitlements.plist` 并接入 Tauri；主 app 与 bundled helper 实际签名均读回该 entitlement。helper 现在将权限/设备错误映射为稳定 code，不再泛化为 `invalid_control_frame`。
- 前端共享 controller 覆盖 loading/ready/unselected/disconnected/empty/error/locked，保存 UID + last-known name；录制页和设置页共享选择与 lifecycle lock。初始 load、refresh、快速选择的 stale success/failure 均按 generation/queue 收敛。
- Task 11 verifier 已升级为 `lifesub.native-capture-acceptance.v2`，从 12 扩展为 19 个场景，绑定 enumerated/UI/persisted/start/helper/readback/source_started/Catalog UID + name，并含 fallback/mismatch/zero-publication 负向变异。
- 新鲜门禁：Swift `50/50`；前端 `97/97` + production build；native report verifier `19/19`；Tier 2 `564 passed / 0 failed / 7 ignored`，fmt、Clippy `-D warnings`、diff 全绿；上一轮 Tier 3 `592 passed / 0 failed / 8 ignored`。
- 当前验收候选：`main@455c733786f76f66de910ec8c66f2062f537377d`，dirty count 98，dirty digest `c60959efaba2c85cf1c163813daf4eea015316e1b17c7fe96f5751c686c587a9`。候选 app executable SHA `6a897afebb1d7343ab2dc855d4f00ff8d5e99a39291e78abda561ee86f3511f9`，bundled helper SHA `be7b66c93a2ff4c4cd9428ddf5d7775ad810ada404c460edc7d41e27147e5318`，source signed helper SHA `c6b4dceb635ba0d88c3c8d3aec03114abbc41c14b15f4ce31e8e15b1712d1348`。
- 当前候选是 ad-hoc 签名且本机没有有效 codesigning identity。每次重建会改变 TCC code requirement，因此重建后需要重新授权；正式稳定签名/固定最终包授权一次后不应反复弹窗。
- 历史 `/Applications/LifeSub.app` 仍为 SHA `6b8b7e17...`，缺新 entitlement。macOS 权限变更曾按相同 Bundle ID 错误重启历史包并产生 08:28 两份 crash；报告 `procPath` 明确指向 `/Applications/LifeSub.app`。历史路径已从 LaunchServices 暂时注销，文件未移动、未覆盖、未删除。
- 当前默认模式已保存为 `mic_only`，用于先完成 packaged 内置麦克风 -> chunk -> real ASR -> persisted Revision/Segment UI 闭环。当前候选重建后的麦克风 TCC 授权仍需用户在系统设置中开启；在该权限完成前，不得宣称 packaged hardware/ASR 通过。

## 2026-08-25 麦克风设备选择设计

- 用户确认首版支持录制前选择内置、外接、USB 或蓝牙麦克风，录制期间锁定选择器；热切换延期。
- 选中设备按稳定 Core Audio UID 与最后已知名称持久化。保存设备断连时保留名称并显示“已断开”，不得静默回退系统默认输入。
- 麦克风必需模式在无有效显式选择时禁止启动；`system-only` 不受麦克风缺失影响。录制中选中设备断连必须显式失败，不自动换源。
- 已新增独立规格 `docs/superpowers/specs/2026-08-25-lifesub-microphone-device-selection-design.md` 与实施计划 `docs/superpowers/plans/2026-08-25-lifesub-microphone-device-selection.md`。下一步先完成规格/计划复审，再按 RED/GREEN 实现协议、Core Audio 精确选源、Rust 身份校验、持久化与 UI 状态。
- 首轮独立复审 Critical=0，发现并补齐 7 项 Important：真实 Core Audio/engine 监听与清理、认证一次性枚举 lifecycle、协议升级 V2/版本错配、枚举零权限副作用、跨页面共享状态与竞态、Task 11 端到端 UID 证据链、稳定预启动错误码。等待复审确认后进入实现。

## 2026-08-25 麦克风设备选择实现第一段闭环

- 已完成设备选择最短契约闭环的 RED/GREEN：`RecordingConfig` 新增可选 `microphone_device_uid` / `microphone_device_name`，旧 JSON 默认回落为 `(null, null)`，partial/empty pair fail closed。
- `start_streaming_capture` 已扩展为显式接收 `microphone_device_uid`；`NativeCaptureSession`、`NativeCaptureCoordinator` 和 `AuthenticatedCaptureTransport` 现在会把该 UID 透传到 V2 `start` command，而不是硬编码 `None`。
- 已新增 Tauri command `list_microphone_devices`，通过已认证 helper 发送 `list_microphones`、接收单次 `microphone_devices`、随后发送 `shutdown` 并要求 `shutdown_ack`；TypeScript wrapper 与 camelCase payload 已接通。
- 新鲜 focused 证据（均在 2026-08-25 代码基线上）：
  - `vitest src/services/lifesub.test.ts`: `10/10`
  - `cargo test domain_test --features desktop`: `7/7`
  - `cargo test session_test --features desktop`: `3/3`
  - `cargo test capture_coordinator_test --features desktop`: `5/5`
  - `npm run build`: passed
  - `cargo fmt --check`: passed
  - `git diff --check`: passed
- 当前仍未完成：React 共享麦克风 selector controller、录制页/设置页状态 UI、真实 start 前 `selected_microphone_*` 错误映射，以及 helper 一次性 discovery lifecycle 的更细负向回归。下一轮从这些剩余项继续，不要重复改协议和 Tauri 契约层。

## 2026-08-25 可恢复麦克风配置变化误判修复

- macOS TCC 日志确认开发态麦克风授权已完成：`com.openai.codex` 的 `kTCCServiceMicrophone` 结果为 `authValue=2`。由于 helper 由 Codex 启动，开发态权限显示为 ChatGPT/Codex；正式打包后才会显示 LifeSub。
- 用户看到的 `DeviceInterrupted { source: Microphone, reason: "device_configuration_changed" }` 不是权限拒绝。helper 协议明确携带 `recoverable: true`，但 Rust `NativeCaptureCoordinator` 原先忽略该字段并无条件进入 Failed，导致一次可恢复配置变化终止整次录制。
- 新增 `recoverable_device_interruption_keeps_capture_running` 回归，修复前准确失败；coordinator 现仅对 `recoverable: false` 进入稳定失败，可恢复事件保持 Running/Pending。
- 新鲜验证：recoverable focused `1/1`、不可恢复错误 focused `1/1`、coordinator module `5/5`、desktop Cargo check、`cargo fmt --check` 与 `git diff --check` 通过。
- 修复后的 verified-runtime 开发版正在运行（PID `61842`，Vite PID `61780`，terminal session `11177`），debug executable SHA-256 为 `f9bbfecc46fa8c8c1a5cc4b7eee10ffdf16add2271f3f67260c426e5e403796e`。当前等待用户在该开发窗口再次点击“开始记录”，验证 UI 进入“正在记录”并产生真实 chunk/ASR evidence；不得启动历史 `/Applications/LifeSub.app`。

## 2026-08-25 模型下载进度 UI 待办确认

- 当前 ModelManager 后端已支持真实下载、4 MiB checkpoint、断点续传、校验、安装和 runtime qualification；模型页也能发起安装并显示笼统的“安装中…”。
- 实时下载进度 UI 尚未完成：当前前端 DTO 只暴露 `download.state`，没有 download ID、`downloaded_bytes`、`expected_bytes`、稳定错误信息或可观察的 operation progress；页面也没有进度条、百分比、字节数、验证/安装阶段投影及取消/重试控制。
- 已把该缺口明确补入实施计划 Task 12 和 PRD 验收矩阵。发布前必须完成 persisted Catalog progress 投影、刷新/重启恢复、RED/GREEN 组件与 Tauri 集成测试，以及真实慢速/中断下载截图证据；不得以安装按钮显示“安装中…”或安装 Promise 最终返回替代进度验收。

## 2026-08-25 Task 11 开始按钮与开发态卡顿修复

- 用户实机点击“开始记录”后出现两个同名按钮；根因是 idle 按钮与 `idle && lifecycleBusy` 按钮同时渲染。新增 pending-start 回归先稳定失败（2 个按钮），合并为单按钮后通过，LiveCapture focused `24/24` 通过。
- 卡顿的开发态根因是 Vite 在缺少 macOS `fsevents` optional dependency 时轮询整个仓库，而 `src-tauri/target` 约 20 GB / 103k 文件，导致 Vite 约 415% CPU。`vite.config.ts` 已忽略 Rust target 与 Swift `.build`；重启后 Vite 稳定约 5%--9%，LifeSub 约 0%--0.1%。
- 用户再次实机点击仍卡顿后，process sample 证明 `start_streaming_capture` 在 AppKit 主线程同步等待 `startup_rx.recv()`。start/pause/resume/stop Tauri commands 已转为 async + `spawn_blocking`；启动 Promise 未完成时 UI 立即显示“正在启动”，只有真实 `source_started` 后才显示“正在记录”。
- 继续跟踪发现 Rust 已发送并且 helper 已收取 234-byte Start 帧，但 Swift `FileHandle.read(upToCount: 4096)` 等待填满读取，不会立即解析单条命令。新增“只发 Start 即必须响应”回归先超时 RED，helper 改为按 `header length/header/payload length/payload` 精确读取后 GREEN，单测约 12--14 ms。
- 新 helper 已构建/签名到 Tauri sidecar（SHA-256 `c68e13f7d63b8b2ce63baa4a0c49f692cf5f871fdfecadf8bbe101839d054ce7`）。真实点击后 UI 保持可响应并显示“正在启动”，macOS 成功弹出麦克风授权；开发态责任进程被标识为 ChatGPT，只用于开发路径验证，不是 packaged LifeSub 权限证据。
- 最新回归：前端 `76/76`，Swift helper `23/23`，Vite production build、desktop Cargo check、Rust fmt 与 `git diff --check` 通过。当前 verified-runtime 开发实例保持运行（terminal session `81199`），等待用户选择麦克风授权，随后观察是否进入真实“正在记录”。
- 生产构建 `npm run build` 通过，`git diff --check` 通过；实机截图为 `output/debug/lifesub-after-start-button-fix.png`（SHA-256 `75d2a68e54a53e69e1340ef2d38d5f43e70aa998d2bbe0dc69bad746ab46bf33`），确认只有一个“开始记录”按钮。
- 当前开发实例已用 verified Sherpa runtime 重启（terminal session `33877`），等待用户再次点击“开始记录”并处理 macOS 麦克风/屏幕录制权限。不得并行打开历史 `/Applications/LifeSub.app`。

## 2026-08-25 Task 10 收口与 Task 11 接续

- Task 10 已完成：Worker 只在 Catalog commit 返回 persisted Revision 后发 committed event；瞬时 `asr-live-segment` 不再进入桌面 transcript。
- committed DTO 由 Catalog 构造，包含 revision ID/number、non-optional chunk ID、native capture source ID、source enum 与 text；LiveCapture 用 revision cursor 忽略重复/旧事件。
- WebView gap 通过 `get_asr_session_state` 恢复：先注册 success/failure listeners，再查询 Catalog latest revision/job；session ID 存在 sessionStorage，查询与事件并发由 cursor 收敛。
- durable fail 只在 `coordinator.fail` 成功后发 `asr-job-failed`，包含 session/job/chunk/error identity；UI 显示持久错误且不生成 demo transcript。
- Task 10 复审 Critical=0 / Important=0。Worker 8/8、LiveCapture 23/23、前端全量 75/75、native DTO identity 1/1、Vite build通过；最新 Tier 2 为 548 passed / 0 failed / 7 ignored。
- Task 11 strict verifier 已建立：要求 12 类场景、app/helper/source identity、权限/设备/source/chunk/ASR IDs 与 drift/drop/RSS/30min 阈值；shell 正负向测试与 desktop harness integration 均通过。
- Task 11 尚未完成：真实报告必须由 packaged `.app` 在目标 Mac 产生。下一步需要用户确认麦克风与屏幕录制权限，并协助 permission deny/allow/revoke、设备断开/恢复、sleep/wake 和 30 分钟双源操作；在此之前不得生成或伪造通过报告。
- Task 11 verifier 最终复审 Critical=0 / Important=0。严格检查 scenario transition、同一 candidate identity、macOS 14+ arm64、device/source/chunk referential identity、sample/host ranges、16k sample-duration 一致性、连续 rotation、PID/reconnect identity 和 fail-closed denial。当前 `main@455c733` dirty count 68，回滚 worktree dirty count 0。

## 2026-08-25 Task 9 真实模型开发态证据

- 唯一开发/发布源为 `/Users/goldenwave/Documents/MyProject/LifeSub` 的 `main@455c733`；当前 dirty count 61，包含既有 Task 7--9 集成、真实 Gate 与文档更新。回滚 worktree 保持 clean，不用于开发或发布。
- Task 9 合并审查发现并修复 VAD Receipt provenance Critical：只有 exact VAD lease 校验、真实 Sherpa detector 执行和 utterance partition 成功后，才把 VAD snapshot 写入 Receipt；未执行 VAD 不再伪造来源。
- 修复长音频多窗口结果与 utterance 数量不匹配、Worker cancel 与 Provider token 分离、重型 provider/VAD prepare 超过 30 秒 lease 且无法续租的问题。重型初始化已移入可轮询 run，Worker 能持续续租并观察取消/所有权。
- 增加 canonical 模型安装命令和模型页动作，固定先确保 Silero VAD，再确保目标 ASR 模型；失败优先复用同一 download ID 的 checkpoint，`model_download_failed` 最多执行 3 次有界续传。
- 真实 LifeSub Catalog 已通过 ModelManager 安装并记录 `runtime_qualified`：SenseVoiceSmall bundle `7d1efa2138a6...`、Silero VAD bundle `9e2449e10874...`。官方下载经历网络抖动，最终仅在单次 Gate 进程注入本机已有 HTTP proxy 后完成；未持久化代理设置。
- 真实 `qwen06-codeswitch.wav` 已两次走通 canonical import -> anchored decode -> VAD -> SenseVoice -> fenced publication；两个 job 均 `succeeded`，Receipt/Revision/Segment 均持久化。Segment 范围 `486..6208 ms`，实际文本为 `I'm alone all by myself. If too sell Sono too as toy soda.`。
- 技术接入 Gate 已通过，但该混合英/法/意/西 fixture 的识别质量偏弱，不能据此宣称质量 Gate 通过。后续需要中文、英文和真实用户语音样本做质量对比。
- 第二实例启动 abort 已修：修复前 `Ownership(AlreadyOwned)` 经 Tauri setup 传播导致 exit 134 / CrashReporter；修复后第二实例 clean exit 0，stderr 保留稳定原因且无新增 crash report。历史 `/Applications/LifeSub.app` 尚不含修复。
- 新鲜 Tier 2：`fmt`、`clippy -D warnings`、`git diff --check` 通过；Rust `545 passed / 0 failed / 7 ignored`。Native engine focused `6/6`、前端模型/服务 focused `26/26`、真实模型安装 `1/1`、真实 E2E `1/1`。
- Native ASR release capability 仍刻意未标记：当前真实推理来自开发测试二进制，不是 packaged `.app`；Task 10 persisted UI events、Task 11 packaged capture/hardware acceptance 和 Task 12 签名安装证据仍未完成。

## 2026-08-25 当前会话 Handoff

- 当前唯一开发/发布源为 `/Users/goldenwave/Documents/MyProject/LifeSub` 的 `main@0c0eb3c`；文档更新前 dirty count 为 54，最终 handoff 检查为 60（包含本次 handoff 与其他现存文档改动）。回滚 worktree `codex/lifesub-real-asr-v0.2@652abf7` 保持 clean，仅用于比较。
- Task 7 已完成并复审；Task 8 已完成 production native capture 接线，桌面 production factory 具备 Native Capture capability，但真实权限、设备、packaged helper、恢复与 30 分钟硬件验收尚未执行。
- Task 9 代码路径已实质接通：桌面 Worker 选择 `ProductionNativeAsrEngine`，并覆盖 exact installation/snapshot identity、anchored audio open、decode/resample、bounded inference、cancel/run state、receipt/segment 与 fenced publication contract。
- Native ASR release capability 刻意保持未标记。2026-08-25 查询真实 LifeSub Catalog，`model_installations` 为 0；没有 installed/runtime-qualified 模型、exact execution lease 或 packaged real-audio inference 证据。
- 本轮新鲜核验：前端全量 `69/69`；Rust no-default lib check 通过（0 errors / 40 warnings）；`git diff --check` 通过；正式 release-source verifier 按预期只因缺 `NativeAsrDesktopRuntimeFactory` capability 失败。
- 当前 `/Applications/LifeSub.app` executable SHA-256 仍为历史 `6b8b7e17f4f509a95434560141b05793f31503530405dc242e8b1b7e620d8f8c`；当前 target tree 无 DMG，本轮未打包、未安装、未覆盖应用。
- 下一顺序：合并审查 Task 9 → 通过 canonical ModelManager 安装并 qualification 真实模型 → packaged real-audio inference 与持久化端到端验收 → Task 10 persisted UI events → Task 11 硬件矩阵 → 正常 macOS Tier 2/Tier 3 → capability marker、签名、DMG、安装与截图。
- 完整接管文档：`docs/handoffs/2026-08-25-lifesub-v0.2.1-native-capture-asr-handoff.md`。

## 2026-08-22 主工作树安全合并

- 唯一开发/发布源改为 `/Users/goldenwave/Documents/MyProject/LifeSub` 的 `main`。
- 合并输入：主工作树保全提交 `dc62d5b` + 最新集成分支 `652abf7`。
- 外部 worktree `/Users/goldenwave/.config/superpowers/worktrees/LifeSub/lifesub-real-asr-v0.2` 降级为回滚/补丁比较来源，不得继续开发或打包。
- 原生采集 Task 1--6 和 `CompatibleNewer` Catalog 启动修复已纳入合并结果。
- Task 7 Catalog v6 + atomic chunk sealing 已完成 focused 验证并通过最终复审（Critical/Important=0）；当前进入 Task 8 NativeCaptureCoordinator，production capture/native ASR 仍 fail closed。

## 2026-08-24 Task 7 收口与 Task 8 接续

- Task 7 完成：v6 DDL/fingerprint、真实 v5 fixture 与阶段 rollback、capture source/timing provenance、durable seal intent、native orphan/stale-temp reconciliation、原子 no-replace sealing。
- 新鲜证据：capture seal 17/17，Catalog 16/16，migration 39/39，production startup 3/3，`cargo fmt --check` 与 `git diff --check` 通过。
- Tier 2 全量测试仍有 94 个环境敏感失败（Unix socket sandbox EPERM、只读 DB、子进程 helper 等），Task 7 focused 与审查通过；不得将该全量失败表述为产品发布通过。
- 下一步：实现 Task 8 NativeCaptureCoordinator，接通 helper lifecycle、source preflight、seal drain 和 durable Catalog enqueue；在 Task 8/9 完成前不得运行正式 release gate 或覆盖 `/Applications/LifeSub.app`。
- Task 8 环境注意：当前 SwiftPM 因 CommandLineTools/SDK toolchain mismatch 与不可写 ModuleCache 无法编译，且 helper 当前源码仍需完成控制帧消费循环；Rust coordinator core 可先做注入式验证，但生产接线与 helper build 必须在匹配 Xcode 环境复验。
- 当前进展：Swift helper 已接入 `HelperCommandLoop`、真实 `MicrophoneCapture`/`SystemAudioCapture` adapter factory、独立 chunk encoders 和 socket event sink；chunk staging 名称现与 Task 7 sealer 的 `chk_<id>.<source>.caf.partial` provenance grammar 对齐，`ChunkSealed` 事件包含 SHA-256，release helper build 通过，CaptureSessionController 测试 11/11，控制协议测试通过。Rust `NativeCaptureCoordinator` core 4/4，`AuthenticatedHelper` 已支持严格 control-frame send/receive + validator，并新增 `AuthenticatedCaptureTransport` 与 Catalog sealer committer；Rust `cargo check --no-default-features` 通过（仅既有 dead-code warnings）。当前尚未切 production runtime：committer 仍需真实 session/source registration 与长期 event loop；commands/runtime wiring 和 packaged helper integration test 未完成，尚未进入 Task 9。
- 前端 session contract 已收口：`start_streaming_capture` 现在接收并校验真实 Catalog session ID，LiveCapture 传入当前 session；前端 focused 测试 19/19 通过。桌面 feature check 在当前环境被既有 Sherpa runtime attestation 门禁拒绝，未绕过该 fail-closed 约束。
- Catalog 增加 `insert_capture_sources_atomic` 事务 API，并用冲突回归证明 source registration 不会留下部分写入；真实 device UID/name 仍必须来自 helper `source_started` 事件，尚未使用默认值伪造。
- 新增不可变 `CaptureSessionPlan`，集中校验真实 session ID、启用 source IDs、staging/final roots；focused plan tests 2/2 通过。它尚未替代旧 `StreamingCapture`，因为长期 helper event loop 和 runtime ownership wiring 仍未完成。
- 最新回归：前端全量 `69/69`，capture sealing `17/17`，coordinator `4/4`，Swift helper release build 通过，`git diff --check` 通过。生产工厂仍为 `FailClosedDesktopRuntimeFactory`，所以这些结果不是发布证据。
- 当前阻塞精确为 Task 8 runtime lifecycle：工作树没有 `NativeCaptureSession`/后台 drain worker；只有可测试的 plan、registry、transport、coordinator 和 committer。未将这些模块伪装成 production wiring，也未改变 release gate。
- 更新：未完成代理实际写入了 desktop-only `NativeCaptureSession`/后台 worker（525 行）；已修复重复 module、cfg 污染和两个 Rust move errors，no-default lib check 恢复通过。desktop focused session tests 可编译，但 2/2 在 CoreRuntime 初始化时因当前环境 SQLite `ReadOnly` 失败，尚未证明 lifecycle 行为；production factory保持 fail-closed。
- Task 8 Important #2 已修：transport/coordinator 改为单次 source-set atomic start，以及每次 lifecycle 单次 pause/resume/stop；fake coordinator回归 4/4 通过，不再按 source 重复发送 session级命令。
- Task 8 Important #3 已修：Rust/Swift `source_started` 严格 DTO 统一包含 `device_id` + `device_name` + sample/channel/host fields；Swift release build与Rust no-default check通过。真实 helper source lifecycle emission仍未完成。
- Task 8 lifecycle emission 已接入真实 controller：preflight/start成功后发 permission_state + 每个 source_started（adapter metadata），stop drain后发 source_stopped；Swift controller 11/11。当前 source_stopped sequence/host range仍为占位 0，尚未满足最终 timing provenance。
- source_stopped 占位已移除：adapter stop返回 encoder累计 frame count与实际首/末 host time，controller按源上报；Swift controller 11/11。剩余 timing blocker为mic mach ticks与system ns单位尚未统一。
- helper新增统一 `CaptureContinuousClock`：microphone mach ticks转换为continuous ns，system/metadata也使用同单位；Swift controller 12/12、release build通过。Rust committer按每源首chunk anchor计算session-relative ms并拒绝倒退，no-default check通过。
- Task 8 source-mode/status 已接入：`start_streaming_capture` 接收 `smart`/`mic-only`/`system-only`，session按请求构建 source set，typed start status返回 enabled sources、permission、device readiness和metadata；desktop session tests 2/2、前端 LiveCapture 19/19、TypeScript check与desktop cargo check通过。
- Task 9 已开始接口审计：现有 Provider/ProductionProviderFactory 支持真实 backend transcribe，Audio preparation 与 Job publication/fencing 已存在；尚缺 `ExecutionSnapshot -> anchored ExecutableInstallationLease -> exact ProviderRequest` bridge、bounded inference run/cancellation和真实 Receipt/Segment mapping。Worker仍保留 FailClosedEngine，未切生产ASR。
- Task 9 增加 ownership-safe `CoreRuntime::executable_installation(model_id)` 入口，先校验 runtime ownership 再取 exact executable lease；no-default lib check通过。尚未切 Worker，仍需 snapshot/provider/VAD/receipt bridge。
- Task 9 新增 `NativeAsrEngine::validate_snapshot_identity`：exact 校验 model_id/manifest_version/bundle_identity 与 anchored lease，并拒绝非法 input hash；no-default lib check通过。仍未执行 Provider/inference或切 Worker。
- Task 9 增加 `ProviderSelection::from_snapshot` 与 `NativeAsrEngine::selection_from_snapshot`，参数从 ExecutionSnapshot 精确复制；no-default lib check通过。仍需 ProductionProviderFactory lease构造、audio/VAD/inference/run fencing和publish bridge。
- Task 9 新增 `NativeAsrEngine::prepare`：通过 ownership-safe CoreRuntime lease入口、exact snapshot identity校验后构造 `ProductionProvider`，模型未安装/未认证/identity mismatch均 fail-closed；no-default lib check通过。尚未执行 inference或切 Worker。
- audio decode bridge审计确认 `DataDirectoryCapability` 只允许锚定目录 fd，不提供可安全拼接路径；不能用裸 pathname绕过 anchored VFS。NativeAsrEngine prepare暂只构造 exact Provider，working-audio读取需新增 descriptor-relative audio capability后再接入，避免路径伪造。
- 新增 anchored audio read bridge：`AudioStore::open_verified` 通过 directory fd + no-follow 打开并重新校验 hash/byte length，`CoreRuntime::open_verified_audio_chunk` 做 ownership check后暴露给 ASR；no-default lib check通过。NativeAsrEngine尚未切 decode/inference。
- NativeAsrEngine prepare现接入 decode-only：Catalog chunk identity -> anchored verified file -> Symphonia working audio decode/resample；错误统一 fail-closed为InputUnavailable/InputIdentityMismatch。no-default lib check通过，尚未执行 Provider inference/VAD或切 Worker。
- NativeAsrEngine prepare进一步生成 working audio 的受限 utterance window（现阶段使用现有 `partition_without_vad`，边界校验严格）；no-default lib check通过。真实 VAD detector、inference run/cancel/fencing和publication仍未完成。
- PreparedNativeAsr新增严格 `transcribe_window`：边界检查、16k AudioSlice、CancellationToken和Provider错误码映射；无模型/runtime时fail-closed，不生成伪文本。no-default lib check通过，Worker仍未切换。
- PreparedNativeAsr新增纯函数 publication mapping：utterance sample range按16k转换chunk-relative ms，segment ID绑定chunk/range/source，空文本/倒置范围fail-closed；no-default lib check通过。尚未接 JobCoordinator fenced publish。
- PreparedNativeAsr新增正式 `ProviderReceiptDraft` builder：字段来自 ExecutionSnapshot + Provider identity/runtime identity，最终通过 `TryFrom` receipt validation；no-default lib check通过。尚未接 Worker token renewal/inference/publish。
- PreparedNativeAsr新增 `publish_text` fenced shell：使用 JobCoordinator token、正式 receipt和chunk-bound segment调用现有 publication transaction；no-default lib check通过。尚未把真实 inference loop切入 Worker。
- NativeAsrEngine新增 `inference_windows`：每窗口最多400k samples、覆盖完整 working audio、空输入fail-closed；focused tests 2/2通过。仍需在Worker中逐窗口transcribe、renew/cancel和最终publish。
- PreparedNativeAsr新增 `transcribe_all_windows`：逐窗口前检查 CancellationToken、调用注入式 renewal callback，再执行 Provider；任一步失败立即停止，不生成publication。no-default lib check通过，尚未接 Worker ExecutionEngine::Run。
- 新增 `PreparedRun` ExecutionRun状态层：支持一次性 Result、pending和CancellationToken取消映射到现有 Worker trait；no-default lib check通过。仍需真实 provider输出构造ExecutionSuccess后再切 Worker。
- PreparedNativeAsr新增 `build_success`：要求每个 utterance一个非空文本，使用正式 receipt/segment builders构造 ExecutionSuccess；no-default lib check通过。仍未实现Arc runtime异步Run或切 Worker。
- `ProductionNativeAsrEngine` 现实现现有 `ExecutionEngine`：持有 `Arc<CoreRuntime>`，prepare exact provider/decode，start后台线程执行 bounded windows并产出 pending/cancel/ExecutionSuccess；no-default lib check通过。尚未替换 FailClosed worker，需 verified model/runtime inference gate后再切。
- 最新核心审计：Rust no-default lib check通过、前端全量 69/69、git diff check通过；planned release gate仍唯一失败于 `NativeAsrDesktopRuntimeFactory` trait。dirty count 54，Worker仍选 FailClosedEngine，未生成发布包。
- 当前最终回归（2026-08-25）：Rust no-default lib check通过，前端全量 69/69，git diff check通过，dirty count 53。NativeAsrEngine prepare/decode/windows/transcribe/run-state/receipt/segment/fenced publish contracts均编译，但没有真实 verified model inference证据，Worker仍FailClosedEngine，未打包。
- Task 8 复审收口：anchored VFS read/write flag regression已修复；desktop `NativeCaptureSession` tests 2/2通过；production factory已切到 native capture marker，release planned gate只剩 NativeAsr trait gap。Task 9 尚未完成：底层 `ProductionProviderFactory` 存在，但 Worker仍使用 `FailClosedEngine`，需要真实 installation lease、anchored chunk decode、Provider/VAD/Receipt/Segment构造与fenced publish，禁止以stub标记release-ready。
- 当前审计：主 worktree `main@0c0eb3c`，保留回滚 worktree `codex/lifesub-real-asr-v0.2@652abf7`；dirty count 38。新鲜证据：Rust coordinator 4/4、前端 LiveCapture 19/19、Swift release helper build 通过；planned release verifier 仍报告 1 个 production wiring gap，未打包安装。
- 2026-08-25 最终当前审计：dirty count 54；前端全量 69/69，Rust no-default lib check与git diff check通过。`verify-release-source` planned audit仍仅报告NativeAsr production gap；代码搜索确认Worker仍选 `FailClosedEngine`，没有NativeAsrDesktopRuntimeFactory marker。当前版本未达到发布条件，未覆盖安装包。
- 2026-08-25 继续回归：dirty count 53；前端全量 `69/69`、Rust no-default lib check、diff check通过；安装包 executable SHA仍为历史 `6b8b7e17f4f509a95434560141b05793f31503530405dc242e8b1b7e620d8f8c`。SwiftPM release命令本轮因已有 `.build` 并发进程等待，前一轮同命令已成功；未把等待结果当新鲜发布证据。Worker仍FailClosed，未打包。
- 2026-08-25：桌面 production worker 已切换到 `ProductionNativeAsrEngine`（native capture marker保留，NativeAsr marker刻意未加）。verified desktop check与session tests 2/2、前端69/69通过；无 installed/qualified model时 engine prepare稳定返回 ModelLeaseUnavailable，release wiring仍拒绝，未宣称Native ASR ready或打包。
- 最新发布源审计：唯一开发源 `/Users/goldenwave/Documents/MyProject/LifeSub` `main@0c0eb3c`，回滚源 `codex/lifesub-real-asr-v0.2@652abf7`；dirty count 55。未发现模型 artifacts或新DMG；`/Applications/LifeSub.app` executable SHA仍 `6b8b7e17f4f509a95434560141b05793f31503530405dc242e8b1b7e620d8f8c`；planned verifier仍为1 gap/not release-ready。
- 2026-08-25 verified runtime复核：Sherpa archive `v1.13.5` 与 attestation 均存在，desktop `cargo check --features desktop` 在独立 target通过；正式 `LIFESUB_RELEASE_ALLOW_PLANNED=0 npm run verify:release-source` 仍因 `NativeCaptureRuntimeFactory: NativeAsrDesktopRuntimeFactory` trait缺失拒绝。模型 artifacts未发现，不能加NativeAsr marker或打包。
- 外部状态补核：Sherpa archive/attestation存在，但当前 LifeSub app data Catalog 的 `model_installations` 查询为空，未安装/qualified模型；其他应用目录下的 ONNX 文件不属于 LifeSub registry，不能冒用为 release evidence。NativeAsr marker与安装包保持不变。
- 2026-08-25 最终构建证据：`npm run build` 成功（Vite production bundle生成）；Rust no-default lib check与diff check通过。Swift release build本轮因已有同一 `.build` SwiftPM进程占用而等待，前一轮相同命令已成功；安装包SHA仍历史 `6b8b7e17...`，未覆盖。
- 最终发布审计：仓库未发现 installed/qualified ASR model artifacts（仅测试 Catalog fixtures）；`verify-release-source` 仍报告唯一 NativeAsr production gap，dirty count 55。没有足够证据通过真实模型 gate，因此不覆盖历史安装包、不生成 DMG。
- 2026-08-25 继续推进：Task 9 合并审查发现 VAD Receipt provenance Critical、长音频窗口归并和取消 token Important。已完成 RED/GREEN：未执行 VAD 不再写 VAD Receipt，production 在成功 detector 后才保存 exact VAD snapshot；多 provider window 按 utterance 合并；Worker cancel 与 Provider 共用 token。no-default 与 desktop focused 均为 5/5。新增 canonical 模型安装命令/UI，固定先确保 Silero VAD 再安装目标模型，前端 focused 26/26、desktop 安装计划测试通过。
- 同轮修复第二实例启动 abort：开发版持锁时历史安装包触发 `Ownership(AlreadyOwned)`，旧 setup 错误传播导致 `panic_cannot_unwind`。当前源码将初始化失败转换为 clean exit；真实双进程复现从 exit 134 + CrashReporter 改为 exit 0，且无新增 crash report。真实模型安装与推理 Gate 继续进行，未覆盖 `/Applications/LifeSub.app`。

## 2026-08-21 暂停与 Handoff（历史，已被 2026-08-22 合并取代）

- 应用户要求已暂停开发，当前发布源 HEAD 为 `c68dc0d`。
- Task 1--6 已完成并通过最终复审；Task 7 仅刚进入 migration RED，尚未编写 v6 生产 DDL 或 chunk sealing 实现。
- 当前唯一 Task 7 新增未提交改动为 `src-tauri/src/catalog_migration_test.rs` 中的 `migrates_v5_to_v6_capture_timing_without_rewriting_existing_chunks`；已观察到正确 RED：当前 `user_version` 仍为 5。
- 完整接管文档：`docs/handoffs/2026-08-21-lifesub-v0.2.1-native-capture-handoff.md`。
- 暂停时未打包、未安装、未覆盖 `/Applications/LifeSub.app`；production runtime 仍保持 fail-closed。

## 2026-08-21 V0.2.1 集成任务进度

- Task 1 已完成并通过复审：Sidebar 移除「导入音频」，Timeline 保留导入工作流；提交 `e516845`。
- Task 2 已完成并通过最终质量复审：发布根、分支、版本、dirty count 与 `--locked` 均受门禁约束；production runtime 证明改为 crate 内 sealed native capture/native ASR 类型能力，自报 bool 无法伪造。主要提交 `e538ba7` / `ec7d33b` / `a022fb5` / `371faea` / `f69fe7d`。
- Task 2 验证：release-source shell fixture PASS，`npm run test:release-source` PASS，`cargo fmt --check` PASS，`git diff --check` PASS；当前 production alias 仍为 fail-closed，正式 gate 按预期拒绝，planned audit 明示 `not release-ready`。
- Task 3--6 已完成并通过最终复审：版本化 Rust/Swift 协议、ScreenCaptureKit/AVAudioEngine 双路 helper、arm64 sidecar 构建签名、继承 FD nonce + UID/PID/可执行文件身份认证均已完成。
- 当前接续点仍是 Task 7；2026-08-22 后必须从主路径 `main` 继续，不得返回外部 worktree。

## 2026-08-21 工作区审计与安装源确认

- 当前安装包来源为本 worktree 当前 dirty tree，不是 `main`；bundle 版本已统一为 `0.2.1`。
- 安装二进制 SHA-256：`6b8b7e17f4f509a95434560141b05793f31503530405dc242e8b1b7e620d8f8c`；签名验证通过，Finder 正常启动且无新崩溃。
- 这是 2026-08-21 审计时的历史状态；外部 worktree 的集成已在 2026-08-22 完成，现在唯一发布源为主路径 `main`。
- `main` 的“15 Task 完成”报告不能作为安装包功能证据；其生产命令仍有 stub 且关键测试模块被注释。
- 实时麦克风/系统音频采集未完成；当前没有 ScreenCaptureKit/AVAudioEngine production adapter，界面“未接通”提示是正确的 fail-closed 状态。
- native ASR production executor 仍未接通；当前桌面 worker 使用 `FailClosedEngine`。
- Task 7/Task 9 的两个干净 worktree 目录已移除，分支引用保留。详细地图与打包护栏见 `docs/workspace-status.md`。

## 2026-08-20 宽窗口修复新包与桌面复核

- 旧 bundle 的 `.app` 时间为 2026-08-19 20:18，早于本轮宽窗口修复（20:42--20:45），因此重新以当前 worktree 打包。
- 前端验证：`npm test` 69/69 通过；`npx tsc -p tsconfig.app.json --noEmit` 与 `git diff --check` 通过。Rust Tier 2 的 fmt、clippy 与 diff 通过；完整 Rust 测试阶段受本次交互运行器的 30 秒输出窗口限制，未将其表述为本次新鲜全量通过证据。
- 直接 Tauri build 首次因未传入 Sherpa runtime attestation 被 `build.rs` fail-closed 拒绝；使用项目受校验 runtime 环境变量后完成新的 macOS `.app` 构建。
- 新 app 已执行 `codesign --force --deep --sign -`，并通过 `codesign --verify --deep --strict`。由该 app 重建并验证的 DMG：`src-tauri/target/release/bundle/dmg/LifeSub_0.1.0_aarch64-wide-ui.dmg`；DMG SHA-256 为 `88b15033ff66849e4ca9ea801ac7a0c69b4ecf7ac4226e841da7bd698acea30b`，内部 app 的可执行文件与构建 app 一致（SHA-256 `02f9bed36878efec06d18f6af6eda670acfb4bbbaa6dce0d30128f79a57caf72`）。
- 新 app 已安装到 `/Applications/LifeSub.app` 并启动。旧 app 保留为 `/Applications/LifeSub.app.pre-wide-ui-20260820`，便于恢复。
- 安装版宽窗口实测：录音页显示“待检测”与四项诊断清单；时间线空查询显示“暂无转写”，有查询无匹配时显示“没有匹配的原话”；词典工具栏/禁用的新建词条/右侧说明符合预期；设置模型页按 Provider 分组、动作列稳定且使用“暂不可安装”；设置对话框未占满窗口。
- 实测限制：当前设备没有接通真实实时采集或已安装模型，点击“开始记录”未进入可录制状态，因此仅验证了 fail-closed 的待检测状态与诊断，不把真实采集成功列为通过。

## 当前状态摘要（以 2026-08-21 Handoff 为准）

- 当前阶段：`v0.2.1-paused-task7-handoff`。
- 已完成：V0.2 UI/Catalog 安全闭环；原生采集计划 Task 1--6（Sidebar 入口、发布源门禁、协议、Swift 双路 helper、sidecar 签名、helper 认证监督）。
- 当前安装 App 是历史 fail-closed 构建，不包含已接通的 production coordinator/native ASR，不是继续开发或发布证据。
- 当前阻塞：Task 7 Catalog v6 + atomic chunk sealing、Task 8 production coordinator、Task 9 NativeAsrEngine、Task 10 persisted UI events、Task 11/12 验收与打包。
- 下一步：仅从 `src-tauri/src/catalog_migration_test.rs` 中已观察 RED 的 `migrates_v5_to_v6_capture_timing_without_rewriting_existing_chunks` 继续 Task 7 TDD。
- 放行原则：在上述真实链路完成前，保持 fail closed，不将 Demo、规则清理或 provider 不可用结果表述为真实录音/ASR 成功。

## 2026-08-19 UI 走查问题修复完成

- 已按 `ui-walkthrough-issues-2026-08-19.md` 完成 Critical/Important/Medium/Minor 的代码与交互收口。
- 生产路径不再输出 Mock 转写或静默 LLM Mock；浏览器数据持续标注为 Demo，桌面采集不可用时显式失败。
- 录音生命周期已加 generation/mutex 语义，覆盖 start/pause/resume/stop/error/restart 的交错；停止不再伪报保存成功。
- 导入使用单一 Core 命令，持久化 stopped session、Chunk 与 durable Job outcome；精确保存 `model_id`，禁止 Provider 下自动挑模型。
- Worker 使用唯一 coordinator，覆盖 recover/claim/renew/cancel/shutdown；不可用 runtime fail closed，不生成伪 revision。
- 时间线从 Catalog 加载，支持 Job/Chunk integrity、manual provenance、只读历史 revision 和按 Segment Chunk 绑定播放。
- 设置 Modal 已 portal 化并支持焦点锁定、多 Modal 栈、背景 inert 与响应式单列 body；词典 CRUD、错误重试和作用范围已补齐。
- 视觉已完成主次层级、中文字体角色、AA 次要文本、标签单行、边框降噪和窄窗口适配。
- 验证：Tier 2 全通过（Rust 484 passed / 0 failed / 6 ignored），前端 69/69，Playwright 12/12，生产构建通过，无 `console.log`，`git diff --check` 通过。
- 截图：`output/playwright/ui-walkthrough-fixed-desktop.png`、`ui-walkthrough-fixed-mobile.png`、`settings-fixed.png`、`settings-fixed-mobile.png`。
- 独立复审：录音、设置、Catalog/Worker 均为 Critical 0 / Important 0。

## 2026-08-19 重新打包与安装替换

- 使用受校验的 sherpa runtime attestation 执行 `tauri build --features desktop`，生成 arm64 `.app` 与 DMG。
- 完整 bundle 已重新执行 deep ad-hoc 签名并通过 `codesign --verify --deep --strict`。
- DMG 已由签名后的 `.app` 重新生成并通过 `hdiutil verify`。
- 已替换 `/Applications/LifeSub.app`，安装版可执行文件与构建产物 SHA-256 一致：`8ba5699ada608475b626b03dae6db1a0fe6aa2ff8aea45b253d00da1744a5b1a`。
- 安装版已启动并保持运行，进程路径为 `/Applications/LifeSub.app/Contents/MacOS/lifesub`。
- DMG：`src-tauri/target/release/bundle/dmg/LifeSub_0.1.0_aarch64.dmg`，SHA-256 `9bd72bda3da4245805e8bbae1e7f1a8cfe2992eaa430f7b0ba65b5037e1b0783`。

- 历史阶段记录保留如下；如与“当前状态摘要”冲突，以摘要为准。

## Phase 3 完成（LLM 后处理 + Fn 键快速输入 + 场景感知）

- ✅ `src-tauri/src/llm/polish.rs` — LLM 润色服务与 Ollama CLI 调用；历史 Mock 降级已废止，当前失败显式返回
- ✅ `src-tauri/src/quick_input.rs` — 全局快捷键（CommandOrControl+Shift+Space）+ `CGEventPost` 光标写入 + 场景感知
- ✅ `src-tauri/Cargo.toml` — 新增 `tauri-plugin-global-shortcut` 依赖
- ✅ `src-tauri/src/commands.rs` — 5 个新命令：`llm_polish` / `register_quick_input_hotkey` / `get_frontmost_app` / `paste_text_at_cursor`
- ✅ `src-tauri/src/lib.rs` — 注册 llm + quick_input 模块 + 5 个新命令 + global-shortcut 插件
- ✅ `src/services/lifesub.ts` — 5 个新 invoke wrapper
- ✅ `src/components/LiveCapture.tsx` — AI 润色按钮 + 原始/润色切换 + 快速输入状态指示器
- ✅ `src/styles.css` — 新增 `.quick-input-indicator` 样式
- ✅ 验证: TSC 0 错误，10/10 测试，npm build 成功，Rust 453/453 测试通过（0 失败）

## Phase 2.1 完成（流式 ASR 实时通道）

- ✅ `src-tauri/src/capture/streaming.rs` — StreamingCapture 生命周期与事件合同；生产 Mock Source 已禁用，采集未接通时显式失败
- ✅ `src-tauri/src/capture/mod.rs` — 模块入口，`#[cfg(feature = "desktop")]` 特性门控
- ✅ `src-tauri/src/commands.rs` — `start_streaming_capture` / `stop_streaming_capture` 命令，AppState 持有 Mutex<StreamingCapture>
- ✅ `src-tauri/src/lib.rs` — 注册 capture 模块 + 2 个新命令
- ✅ `src/services/lifesub.ts` — `startStreamingCapture` / `stopStreamingCapture` invoke 封装
- ✅ `src/components/LiveCapture.tsx` — 监听 `asr-live-segment` / error 事件；历史 Tauri fallback Demo 已废止，Demo 仅限浏览器预览
- ✅ `src/test/setup.ts` — mock `@tauri-apps/api/event` 使 Vitest 测试通过
- ✅ 验证: TSC 0 错误，10/10 测试，npm build 成功，Rust 449/450（1 个已知失败 unchanged）

## Phase 1.1 完成（前端 API 接入）

- ✅ `src/data/adapter.ts` — 统一数据适配层，Tauri 运行时走真实 invoke，dev/test fallback demo
- ✅ DictionaryView — 分类/词条 CRUD 通过 adapter 加载和持久化
- ✅ StatsBar — 24h 统计通过 adapter 加载
- ✅ AsrSettings — Provider/语言/VAD/ITN 配置 + 声纹库通过 adapter 加载
- ✅ RecordingSettings — 捕获模式/IM 检测/音频格式/存储通过 adapter 加载
- ✅ LiveCapture — 笔记 CRUD 通过 adapter 持久化
- ✅ 验证: TSC 0 错误，10/10 测试通过，npm build 成功

## 已完成

### Task 1-13: 真实本地 ASR 引擎
- ✅ 真实本地 ASR 引擎（SenseVoice / Whisper / Qwen3-ASR）
- ✅ 模型管理（下载/安装/卸载/版本化/原子恢复）
- ✅ Job 调度（claim/lease/fencing/cancel/recovery）
- ✅ 原子 Receipt/Revision/Segment/FTS 发布
- ✅ Catalog V4 迁移 + Tool API V1 + 安全本地 IPC
- ✅ 8 个 mutation 方法 + idempotency 集成
- ✅ Task 13: 449/455 测试通过

### Task 13.5: 后端模块补全（2026-08-18）
- ✅ Catalog V5 迁移（5 张新表: notes / dictionary_categories / dictionary_entries / voiceprints / settings）
- ✅ 22 个新 Tauri 命令（笔记 4 + 词典 8 + 声纹 5 + 统计+设置 5）
- ✅ 前端 invoke wrapper 全部就绪
- ✅ Rust 450/450 测试通过，TSC 0 错误，前端 10/10 测试通过
- ✅ `npm run build` + `cargo build` + `tauri build` 全部成功

### UI 重构（2026-08-18）
- ✅ 4 页面架构（录音 / 时间线 / 词典 / 设置弹窗）
- ✅ 流式 ASR 展示（说话人声纹标注、Demo 数据）
- ✅ 时间戳笔记（待办/备忘/问题/决定、Demo 数据）
- ✅ 会话树形目录 + 24h 录音统计条
- ✅ 词典管理（分类/词条/别名/启用停用）
- ✅ 声纹库 UI（FunASR CAM++ 规划中）
- ✅ 设置弹窗（录音设置 / ASR 设置 / 模型 / 关于）

## 待完成

| 阶段 | 内容 | 预估 |
|------|------|------|
| Phase 2.2 | 说话人分离 + CAM++ 声纹 | 1-2 session |
| Phase 3.1 | LLM 后处理管道 | 1-2 session |
| Phase 3.2 | Fn 键快速输入 | 1-2 session |
| Phase 3.3 | 场景感知（可选） | 1 session |

## 技术债务
- CAM++ 集成: 声纹表已建但无 embedding 提取
- V4→V5 迁移 fixture: 需要生成 `lifesub-v0.5.sqlite3`
- Gate 二进制: 指标协议已实现，待生产设备上集成真实 ASR provider
- 真实采集与 native provider executor 尚未接到生产 worker；当前 worker 显式失败，不发布伪 revision。
- bundle 版本仍为 `0.1.0`，待真实音频 Gate 通过后升级 V0.2。
- 模型安装/下载已有后端能力，但设置 UI 当前保持禁用“计划中”。

## 设计文档
- `docs/superpowers/plans/2026-08-18-lifesub-roadmap.md` — 后续开发总计划
- `docs/superpowers/plans/2026-08-18-lifesub-backend-tasklist.md` — Task 13.5 后端模块设计
- `docs/superpowers/plans/2026-08-18-lifesub-ui-redesign.md` — UI 重构设计
- `docs/superpowers/plans/2026-08-18-lifesub-llm-quick-input.md` — LLM 后处理 + Fn 键设计

## 2026-08-19 设置弹窗修复

- ✅ `Modal` 改为语义 `dialog` 面板：初始焦点落到关闭按钮，`Tab`/`Shift+Tab` 焦点循环，`Escape` 关闭，背景 sibling 自动 `inert`/`aria-hidden`，关闭后恢复触发按钮焦点。
- ✅ `SettingsModal` 增加 `tablist`/`tab`/`tabpanel` 语义，并拆到 `src/settings.css` 覆盖局部布局，避免双重 grid 在窄窗口和放大字体下压缩内容。
- ✅ `AsrSettings` / `RecordingSettings` 保存错误改为页内 `role="status"` 反馈；声纹“重命名/删除”接上真实 adapter 持久化；无样本来源的“注册新声纹”改为禁用的“计划中”。
- ✅ `ModelManager` / `AboutTab` 不再硬编码模型卡和版本号，改为读取运行时只读投影；模型未实现的安装/管理动作统一禁用，不再可点击无反馈。
- ✅ 证据：`npm test -- src/components/SettingsModal.test.tsx src/services/lifesub.test.ts` 通过（10/10）。
- ⚠️ 仓库现状：`npm run build` 仍被时间线页既有类型漂移阻断；`cargo check --manifest-path src-tauri/Cargo.toml --features desktop` 仍缺 sherpa 运行时构建环境变量，二者都不是本次设置修复引入。

## 2026-08-19 reviewer 第二轮收口

- ✅ `Modal` 支持多实例 stack：仅顶层实例处理 `Escape`/`Tab`，背景 inert 改为引用计数，`onClose` 通过 ref 保持最新回调且不重置焦点副作用。
- ✅ `SettingsModal` tabs 补齐方向键 roving，`Modal body` 在 `.modal-body > .settings-layout` 结构下保持单列，避免 reviewer 指出的双 grid 再现。
- ✅ `loadVoiceprints` / `loadAsrConfig` / `loadRecordingConfig` 在 Tauri 失败时不再回退 demo/default，错误由 `AsrSettings` / `RecordingSettings` 明示并提供 retry，保存按钮在加载失败时禁用。
- ✅ `AsrSettings` 声纹重命名/删除补齐 pending/禁用/error/status；`ModelManager` / `AboutTab` 增加加载失败提示与重试。
- ✅ `list_asr_models` 改为按 `(model_id, manifest_version, bundle_identity)` 精确匹配安装状态，并只选择当前 bundle 的最新下载记录；补了对应 Rust 单测。
- ✅ focused 证据：`npm test -- src/components/SettingsModal.test.tsx src/data/adapter.test.ts src/services/lifesub.test.ts` 通过（25/25）。
- ⚠️ 额外验证：`npx tsc -p tsconfig.app.json --noEmit` 仍被仓库现有 `src/components/StatsBar.tsx` 语法错误阻断；Rust focused test 继续被 Tauri allowlist 校验（`protocol-asset` 特性）阻断，不是本轮设置修复引入。

## 2026-08-19 大窗口响应式 UI 修复

- 基于 `ui-wide-window-walkthrough-2026-08-19.md` 的 23 项大窗口 UI 问题进行系统性修复。
- **P1 修复**（2/2）：
  - ✅ W-R01 录音页"准备就绪"状态矛盾：`statusTitle` 闲置时改为"待检测"（有 errorMessage 时显示"采集未就绪"），不再同时显示"准备就绪"与未接通诊断。
  - ✅ W-T01 时间线空搜索错误提示"没有匹配"：空查询无 segment 时按记录状态分"等待转写""暂无转写""转写不可用"；仅在有查询且无结果时显示"没有匹配的原话"。
- **P2 修复**（17/17）：
  - ✅ W-C01 主工作区高度链：所有页面使用 `min-height: 0` + `grid-template-rows: auto minmax(0, 1fr)` 完整高度链。
  - ✅ W-C02 宽度分配：宽窗口（≥1280px）增加 session tree 列宽至 280–360px、词典三栏弹性宽度、内容区 max-width 约束；≥1600px 进一步放宽。
  - ✅ W-C03 密度平衡：note panel 移除 `align-content: start` 使其填满等高；stats bar 内容居中约束；dictionary header 压缩为单行工具栏。
  - ✅ W-R02 诊断信息：改为带图标的状态清单，每项显示"名称 + 状态 + 提示动作"，阻塞项高亮。
  - ✅ W-R03 笔记面板：移除 `align-content: start`，笔记面板与录音工作区等高独立滚动。
  - ✅ W-R04 空态图标：💡 emoji 替换为 Lucide `Mic` 图标，与产品图标库一致。
  - ✅ W-T02 原始 ISO 标题：`createCapture` 标题改为本地化格式 `"实时记录 8/19 20:19"`。
  - ✅ W-T03 记录列过窄：session tree 在宽窗口下弹性宽度 280–360px。
  - ✅ W-T04 Evidence 信息条：宽窗口下内容居中约束，与 body 对齐。
  - ✅ W-T05 统计条：内容居中约束，不再无限拉伸。
  - ✅ W-D01 范围说明与选择器：合并为同一控件组，放在标题旁。
  - ✅ W-D02 顶部说明区过高：header 压缩为单行工具栏，标题与范围选择器同行。
  - ✅ W-D03 创建依赖关系：无分类时"新建词条"按钮 disabled，`title` 提示"请先创建分类"。
  - ✅ W-D04 说明栏过窄：宽窗口下 detail 栏弹性宽度 200–300px。
  - ✅ W-S01 模型动作列失稳：model card 改为 grid 布局，meta 区 `flex-shrink: 0` + `white-space: nowrap`。
  - ✅ W-S02 重复表达：非 runtime_qualified 模型显示"暂不可安装"替代"安装计划中"按钮。
  - ✅ W-S03 模型一次性展开：按 Provider 分组，SenseVoice 标记"（推荐）"。
  - ✅ W-S04 模态框满屏：settings layout `min-height` 从 40rem 降至 24rem。
  - ✅ W-S05 模型元数据对比度：从 `textMuted` 提升至 `textSecondary`。
- **P3 修复**（4/4）：
  - ✅ W-D05 底部说明条：footer 移除，内容合并到 detail panel 的"工作原理"字段。
- 验证：前端 69/69 测试通过，TypeScript 0 错误。
