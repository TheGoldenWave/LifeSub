# LLM 后处理管道 + Fn 键快速输入

> **状态**: 设计阶段 | **依赖**: Task 13.5 后端, 流式 ASR | **分支**: `codex/lifesub-real-asr-v0.2`

---

## 概述

在 LifeSub 已有的全局录音 + ASR 流式转写基础上，新增两个能力：

1. **LLM 后处理管道**：ASR 原始转写 → 本地 LLM 润色（去口头禅、改口纠错、自动格式化）
2. **Fn 键快速输入**：按住 Fn 说话 → 松开即把润色后的文本写入当前光标位置，零跳转，不打断全局录音

核心设计原则：**全局录音是持续的水龙头，Fn 键只是在上面接一杯水**。

---

## 架构

```
全局录音（持续运行，不中断）
    │
    ├── ASR 流式转写 → 持久化到会话（不变）
    │
    └── Fn 按下 ────────────────── Fn 松开
            │                            │
            │    标记时间戳区间            │
            │    [t_start, t_end]         │
            │            │                │
            │    从 ASR 流中提取该区间      │
            │    的转写文本                │
            │            │                │
            │    本地 LLM 润色             │
            │    （去口头禅+纠错+格式化）     │
            │            │                │
            │    通过 Accessibility API    │
            │    写入当前光标位置           │
            │                            │
            ▼                            ▼
      全局录音继续            上屏完成，零跳转
```

## 与 Typeless 的对比

| 维度 | Typeless | LifeSub |
|------|---------|---------|
| 音频来源 | 每次 Fn 按下才启动录音 | 全局录音一直在跑，Fn 只是切片 |
| 响应延迟 | 按住→说话→ASR→LLM→上屏 | 按住→说话，松开时 ASR 已有结果，只需 LLM |
| 录音保留 | 不保存 | 自动归入当前会话，可回溯 |
| 架构耦合 | 强耦合（输入法级） | 松耦合（全局快捷键 + ASR 切片） |
| 离线能力 | 依赖云端 LLM | 本地 LLM，完全离线 |

---

## 功能清单

### 1. LLM 后处理管道

| 能力 | 说明 | Prompt 示例 |
|------|------|------------|
| 去口头禅 | 过滤"呃""啊""那个""就是说"等 | "删除所有口头禅和填充词，保留原意" |
| 改口纠错 | 识别修正意图，丢弃被推翻的表述 | "如果说话人中途改口，只保留最终版本" |
| 自动格式化 | 口述"第一…第二…"→ 自动编号列表 | "将口语化列表转为 Markdown 有序列表" |
| 场景适配 | 根据当前 App 选择语气 | Slack→简洁 / 邮件→正式 / VS Code→原样 |

### 2. Fn 键快速输入

| 行为 | 实现 |
|------|------|
| 触发 | Fn 键按下（或自定义组合键），通过 Tauri `global-shortcut` 插件注册 |
| 切片 | 记录 `t_start` / `t_end`，从 ASR 流中筛选 `startMs` 在区间内的 `LiveSegment` |
| 润色 | 拼接段落文本 → 发送到本地 LLM（带 prompt）→ 返回润色文本 |
| 上屏 | 通过 macOS Accessibility API（`CGEventPost`）模拟键盘输入，写入当前焦点位置 |
| 状态提示 | 菜单栏图标短暂变色/闪烁，提示"正在润色→已上屏" |

### 3. 场景感知（可选阶段）

| App | 检测方式 | LLM Prompt 调整 |
|-----|---------|----------------|
| Slack / 微信 / 钉钉 | `NSWorkspace.frontmostApplication.bundleIdentifier` | "输出简洁对话风格" |
| Mail / Outlook | 同上 | "输出正式商务邮件风格" |
| VS Code / Terminal | 同上 | "保持原样，不格式化" |
| 浏览器 | 同上 | 默认格式 |

---

## 技术实现

### 新增依赖

| 层 | 依赖 |
|----|------|
| Rust 后端 | `tauri-plugin-global-shortcut`（全局快捷键） |
| Rust 后端 | `tauri-plugin-shell` 或 `std::process::Command`（调用 LLM CLI） |
| Rust 后端 | macOS Accessibility API（`CGEventPost` 模拟键盘输入） |
| 本地 LLM | llama.cpp / ollama / Qwen2.5-0.5B（通过 CLI 调用） |

### 新增 Tauri 命令

| 命令 | 签名 |
|------|------|
| `register_quick_input_hotkey` | `() → ()` — 注册 Fn 键全局快捷键 |
| `unregister_quick_input_hotkey` | `() → ()` — 注销快捷键 |
| `get_frontmost_app` | `() → String` — 获取当前焦点 App 的 bundle identifier |
| `paste_text_at_cursor` | `(text: String) → ()` — 通过 Accessibility API 写入光标位置 |
| `llm_polish` | `(text: String, context: AppContext) → String` — 调用本地 LLM 润色 |

### 前端事件

| 事件 | 方向 | 说明 |
|------|------|------|
| `quick-input-started` | 后端→前端 | Fn 按下，记录 t_start |
| `quick-input-stopped` | 后端→前端 | Fn 松开，触发切片+润色+上屏 |
| `quick-input-polished` | 后端→前端 | 润色完成，通知前端更新状态 |

---

## 与现有功能的协同

- **全局录音**：Fn 快速输入期间，全局录音不中断，两者独立
- **录音会话**：Fn 输入的音频片段仍属于当前录音会话，事后可回溯原始转写
- **词典**：LLM 润色前，词典中的术语/人名/缩写优先匹配，防止 LLM 误改
- **声纹库**：Fn 输入的说话人标注沿用全局录音的声纹识别结果

---

## 分阶段实施

### Phase A: 本地 LLM 后处理管道（无 Fn 键）

- 选型本地 LLM（Qwen2.5-0.5B GGUF / llama.cpp）
- 集成到 ASR pipeline，每次 ASR 段落完成后自动润色
- 在录音页展示"原始转写"和"润色后"两个版本

### Phase B: Fn 键快速输入

- 注册全局快捷键
- 实现 ASR 时间戳切片
- 实现 macOS Accessibility API 光标写入
- 菜单栏状态提示

### Phase C: 场景感知

- 获取前台 App bundle identifier
- 根据 App 类型调整 LLM prompt
- 用户可配置每个 App 的语气偏好

---

## 风险与注意事项

1. **macOS 辅助功能权限**：`CGEventPost` 模拟键盘输入需要用户在"系统设置 → 隐私与安全性 → 辅助功能"中授权 LifeSub
2. **本地 LLM 延迟**：小模型（0.5B）在 Apple Silicon 上约 200-500ms，可接受
3. **Fn 键冲突**：macOS 原生 Fn 键用于切换功能键，可能需要用 `Ctrl+Shift+Space` 等替代
4. **ASR 切片精度**：ASR 段落有时间戳，但 Fn 按下/松开的时刻可能落在段落中间，需要处理边界情况