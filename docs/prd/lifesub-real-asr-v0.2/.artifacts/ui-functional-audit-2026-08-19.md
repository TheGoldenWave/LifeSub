# LifeSub 安装包 UI 与功能走查

日期：2026-08-19  
对象：`/Applications/LifeSub.app`（Info.plist 版本 0.1.0，界面“关于”显示 0.2.0）  
方法：真实桌面操作 + 对照生成该安装包的 `codex/lifesub-real-asr-v0.2` 工作树源码

## Anti-Patterns Verdict

视觉风格本身通过：深色、扁平、锐利边界和等宽操作层与 LifeSub 的“安静、可信、证据化”方向基本一致，没有明显的渐变、玻璃拟态或卡片堆叠式 AI 模板感。

产品可信度不通过：大量演示数据、Mock 结果和无处理逻辑的按钮以正式能力呈现，尤其“实时 SenseVoice”“录音已保存”“证据可用”“已安装”等文案会让用户把演示状态误认为真实事实。

## Executive Summary

- Critical：2
- Important：8
- Minor：2
- 综合质量：38/100
- 结论：可作为交互原型，不可作为真实录音、ASR 或证据归档产品交付。

## Critical

### C1. 固定 Mock 台词被呈现为真实 SenseVoice 实时转写

- 位置：`src-tauri/src/capture/streaming.rs:126`、`src/components/LiveCapture.tsx:181`
- 违反项：真实录音状态、ASR 来源和证据来源必须明确且不可伪造。
- 证据：点击“开始记录”后，无论现场声音如何，均按固定顺序出现“张伟 / 我 / 可能是李娜”等预置台词；后端启动路径固定构造 `MockStreamingSource::new()`。
- 影响：用户会误判麦克风、系统音频、声纹识别与 SenseVoice 已工作，形成错误证据。
- 最小修复：生产构建只允许真实 Capture Adapter + Provider；Mock 必须由显式 demo feature 控制，并在全局持续显示“演示数据，不会录音”。启动失败必须停留在失败态，不得静默填充 Demo。
- 验证：对安装包播放唯一测试音频，断言转写包含该音频独有口令，且断网/移除模型时显示明确失败而非固定台词。

### C2. “录音已保存”没有产生真实时间线记录

- 位置：`src/components/LiveCapture.tsx:92`、`src/App.tsx:14`、`src/App.tsx:43`
- 违反项：保存成功提示必须对应可恢复、可检索的持久化记录。
- 证据：停止后出现“录音已保存，可在时间线页面查看”，但时间线始终只显示 `demoRecords`；`LiveCapture` 没有向 App 返回新记录，也没有创建/封存 Catalog session。
- 影响：用户可能关闭应用后才发现录音从未保存，属于核心数据丢失风险。
- 最小修复：先创建真实 capture session，持续写入音频与 segment，停止事务成功后再提示保存；失败时保留恢复入口并显示具体错误。
- 验证：录制一段独有口令，退出并重启应用后，时间线仍可检索、播放并导出该记录。

## Important

### I1. 停止后无法开始第二次记录，且 `⌘R` 无效

- 位置：`src/components/LiveCapture.tsx:188`、`src/components/LiveCapture.tsx:253`
- 现象：按钮只在 `idle` 显示，`stopped` 没有“新记录”动作；界面宣称 `⌘R`，但没有键盘监听。
- 修复：允许 `stopped -> idle/recording` 新会话；实现并测试菜单级快捷键，或删除无效提示。

### I2. 两个“导入音频”入口都不导入文件

- 位置：`src/App.tsx:24`、`src/components/TimelineView.tsx:45`
- 现象：侧栏入口只提示“将在时间线可用”；时间线入口仍只显示“选择本地音频文件”，没有文件选择和导入调用。
- 修复：复用现有 Tauri dialog 与 `import_audio_file`，完成导入、hash、任务创建、错误态和进度态。

### I3. 时间线记录、搜索和统计均基于 Demo 数据

- 位置：`src/App.tsx:14`、`src/components/TimelineView.tsx:67`
- 现象：记录固定从 `demoRecords` 初始化；统计强制传入 `demoStats`，真实 `loadStats()` 永远不执行。
- 修复：桌面运行时从 Catalog 加载记录/统计；Demo 仅用于浏览器预览或显式 demo 模式。

### I4. 播放按钮完全没有处理逻辑

- 位置：`src/components/TranscriptView.tsx:55`
- 现象：点击“播放 00:12”没有状态变化；源码按钮无 `onClick`。
- 修复：绑定真实音频资源与时间范围，提供播放/暂停、当前进度、缺失文件错误态。

### I5. 修订仅修改前端内存，不会持久化

- 位置：`src/components/TimelineView.tsx:24`
- 现象：创建修订表单可打开，但保存只替换 React state；重启后丢失，也没有 append-only Catalog 写入。
- 修复：调用 `append_transcript_revision`，成功后刷新记录；失败时不得先显示成功状态。

### I6. 设置弹窗正文被压成窄条，且焦点泄漏到背景页面

- 位置：`src/styles.css:197`、`src/styles.css:395`、`src/components/Modal.tsx:14`
- 现象：默认窗口下设置正文只剩约 30px 宽；按一次 Tab 后，焦点落到弹窗后的“开始记录”。
- 原因：`.modal-body` 已是双列 Grid，其唯一子元素 `.settings-layout` 又是双列 Grid；子元素被放进父 Grid 的 180px 首列。Modal 没有初始焦点、焦点循环和焦点恢复。
- 修复：设置 Modal 的 body 使用单列布局；补完整 dialog focus management，并使背景 `inert`。

### I7. 模型、声纹和词典存在多组无逻辑按钮

- 位置：`src/components/ModelManager.tsx:21`、`src/components/AsrSettings.tsx:135`、`src/components/DictionaryView.tsx:192`
- 无逻辑控件：Whisper/Qwen 下载；声纹注册、重命名、删除；词条编辑；时间线音频播放。
- 现象补充：词典“新建分类”依赖 `window.prompt`，实机点击未出现可操作流程；“新建词条”在无分类时仍呈现为可点击但无反馈。
- 修复：未实现能力应隐藏或明确禁用并标注状态；实现后补进度、错误、取消与回滚。

### I8. AI 润色静默降级为字符串替换，却提示“润色完成”

- 位置：`src-tauri/src/llm/polish.rs:62`、`src/components/LiveCapture.tsx:107`
- 现象：Ollama 不可用时静默调用 `mock_polish` 删除少量填充词，前端仍显示“润色完成”。
- 修复：返回实际 provider、model、fallback 和 error；正式模式禁止静默 Mock，界面明确区分“规则清理”和“本地 LLM 润色”。

## Minor

### M1. 版本与安装状态写死

- 位置：`src/components/AboutTab.tsx:9`、`src/components/ModelManager.tsx:7`
- 现象：安装包 Info.plist 为 0.1.0，关于页显示 0.2.0；SenseVoice “已安装”和下载日期均为硬编码。
- 修复：版本取运行时包信息；模型状态从 manifest、checksum 和本地目录实时读取。

### M2. 笔记时间戳和失败反馈不可靠

- 位置：`src/components/NoteEditor.tsx:21`、`src/components/LiveCapture.tsx:146`
- 现象：时间戳使用 `Date.now() % 3600000`，不是录音相对时间；写入固定 session `current`，异步失败未等待也未提示；空笔记点击保存无反馈。
- 修复：绑定真实 session 和 monotonic capture offset；等待持久化结果；空值时禁用保存或展示校验消息。

## Positive Findings

- 侧栏导航、会话切换和前端搜索可用。
- 暂停/继续能暂停 Mock 事件流，状态反馈清晰。
- Evidence URI 和“复制全部”确实写入系统剪贴板。
- Markdown 确实导出到下载目录，内容结构和 frontmatter 正常，但当前数据仍来自 Demo。
- ASR/录音设置具备真实 Catalog 读写接口；实机修改后在重新打开设置时仍可读取。
- 视觉语言基本符合设计上下文，录音、暂停、成功颜色区分明确。

## Priority Plan

1. 立即：移除生产路径 Mock 和所有虚假成功提示，打通真实 capture -> session -> audio -> segment -> timeline 闭环。
2. 本轮：修复设置布局/焦点、重复录音、导入、播放和修订持久化。
3. 下一轮：接通模型/声纹/词典完整动作；运行时读取版本与模型状态。
4. 发布门禁：安装包级真实音频验收必须使用独有口令和重启恢复验证，禁止 Demo 数据参与通过条件。
