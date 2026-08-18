# LifeSub Design System

## 1. 设计定位

LifeSub 是本地优先的声音 Evidence 管理工具。界面不是消费级 AI 助手，也不是传统会议纪要产品；它应像一个可信的证据终端：安静、克制、精确，始终让用户看清录音状态、来源、时间、revision 和数据去向。

本设计系统参考 `/Users/goldenwave/Downloads/DESIGN.md` 的深色工程美学，但不直接复制 xAI 品牌。LifeSub 保留自己的产品语义：等宽字体用于 Evidence、状态与操作，比例字体用于中文长文本；单色系统为主，仅在录音、暂停、成功和错误等不可替代的状态上使用功能色。

## 2. 核心原则

1. **Evidence first**：稳定 ID、时间范围、来源和 revision 始终比装饰更重要。
2. **Dark by default**：统一使用带蓝灰底色的近黑背景，降低长时间运行时的视觉干扰。
3. **Sharp and flat**：不使用阴影、渐变和大圆角；层级只通过边框、明度与留白表达。
4. **Mono for operations**：按钮、状态、标签、时间和 Evidence URI 使用等宽字体；正文使用易读的中文无衬线字体。
5. **Color is state**：颜色只传达录音、暂停、成功、警告、错误和键盘焦点，不承担装饰。
6. **Local-first clarity**：任何云端、导出、修订和失败状态都必须明确说明数据发生了什么。

## 3. 视觉基线

### 3.1 色彩

| Token | 值 | 用途 |
|---|---|---|
| `canvas` | `#1f2228` | 全局背景 |
| `surface` | `#242830` | 主工作区与一级表面 |
| `surfaceSubtle` | `#292e37` | Hover、输入与选中背景 |
| `textPrimary` | `#f4f6f8` | 标题与主要文本 |
| `textSecondary` | `#b8bec8` | 正文说明 |
| `textMuted` | `#7d8591` | 时间、元数据、占位符 |
| `border` | `#383e48` | 默认边界 |
| `borderStrong` | `#59616e` | Active 与强调边界 |
| `focus` | `#6ea8fe` | 键盘焦点 |
| `recording` | `#ff6b63` | 正在录音、危险停止 |
| `paused` | `#e5b454` | 暂停与警告 |
| `available` | `#71c48d` | Evidence 可用与成功 |

禁止使用纯黑、纯白、渐变、装饰性色块或阴影。功能色在同一屏幕上应保持稀少。

### 3.2 字体

- Display / controls / metadata：`SFMono-Regular`, `Menlo`, `Monaco`, `Consolas`, monospace。
- Body / transcript：`Avenir Next`, `PingFang SC`, `Helvetica Neue`, sans-serif。
- 标题使用 300–500 字重；禁止厚重的 700 字重展示标题。
- 按钮使用 12–13px 等宽大写或短中文标签，字间距 `0.08em`。
- 转写正文为 16–18px，行高 1.7，单行宽度控制在约 45–75 个中文字符。

### 3.3 间距与形状

- 以 8px 为基础网格，允许 4px 半格。
- 主要间距：4、8、16、24、32、48、64px。
- 默认圆角为 0；输入、次级小控件最多 4px。
- 触控目标最小 44×44px。

### 3.4 深度

| 层级 | 处理 |
|---|---|
| Canvas | 单一近黑背景，无边框 |
| Surface | 轻微明度差，无阴影 |
| Bordered | 1px 默认边框 |
| Active | 更强边框 + 轻微表面明度变化 |
| Focus | 2px focus ring，不改变布局 |

## 4. 组件规范

### 4.1 导航

- 固定窄侧栏，与 Canvas 同色。
- 品牌标识使用同源 Evidence Trace；英文名使用等宽字体。菜单栏采用 `A1 · Balanced Evidence Trace`，Dock / App Icon 采用已定稿的 `B2 · Evidence First Narration Bubble`，完整决策见 [`docs/design/lifesub-logo-decision.md`](docs/design/lifesub-logo-decision.md)。
- 当前页面通过高对比文本和左侧 2px 标记识别，不使用大面积填充胶囊。

### 4.2 录音控制

- 始终位于工作区首位，状态文字与操作在同一水平线上。
- 录音状态使用小型实心点；只有 `recording` 使用红色。
- 主操作为浅色实底按钮，次操作为透明描边按钮，停止为红色文字/边框。
- 状态持久化失败时必须回滚视觉状态并显示可恢复提示。

### 4.3 时间线与记录列表

- 列表使用平面行，不使用卡片。
- Active 行通过强边框与轻微表面变化表示。
- 标题、时间、状态分三层展示；长标题单行截断，完整标题可通过辅助说明获取。

### 4.4 Transcript

- Header 显示记录标题和基本元数据，不重复解释页面用途。
- Evidence URI 放在独立 provenance strip 中，并保持等宽字体。
- Segment 使用“时间轨 + 正文”布局；时间和来源为等宽元数据，正文为比例字体。
- 人工修订通过页面内编辑区完成，不使用模态框；原始 revision 永远可回看。

### 4.5 设置

- 设置使用分区列表，不使用卡片网格。
- Provider、云端状态和本地存储位置必须用完整句子解释。
- 状态标签使用小型锐角描边标签，禁止胶囊化装饰。

### 4.6 输入与搜索

- 透明或微亮表面，1px 边框，0–4px 圆角。
- Focus 使用 `focus` Token 的 2px outline。
- Placeholder 使用 muted 文本；清除按钮必须可键盘访问。

## 5. 交互与动效

- Hover 采用轻微变暗或边框增强，不使用浮起与阴影。
- Active 只允许 `transform: translateY(1px)`；不得动画布局尺寸。
- 过渡时长 120–180ms，使用自然减速曲线。
- 遵循 `prefers-reduced-motion`，减少运动时取消所有非必要过渡。
- 所有按钮和输入必须有可见 `:focus-visible` 状态。

## 6. 响应式规则

- ≥1024px：侧栏 + 记录列表 + Transcript 三栏结构。
- 768–1023px：侧栏收窄为图标导航，记录列表缩窄。
- <768px：导航改为顶部横排，记录列表与正文垂直堆叠。
- 移动端不隐藏录音、搜索、导出、revision 或设置等关键功能。
- 小屏幕元数据允许换行，但 Evidence URI 必须可横向滚动或自动换行，不能截断不可恢复信息。

## 7. 无障碍与内容规则

- 正文与背景对比度达到 WCAG AA。
- 状态不能只依赖颜色，必须同时有文本或图标。
- 图标按钮必须有可访问名称。
- 中文按钮不强制转大写；英文命令使用 uppercase 等宽样式。
- 错误文案说明“发生了什么”和“数据是否仍安全”，避免只显示技术异常。

## 8. 禁止事项

- 不使用阴影、渐变、玻璃拟态、霓虹发光。
- 不使用 8px 以上圆角或胶囊按钮，状态点除外。
- 不使用彩色装饰图标；功能色仅用于状态。
- 不使用衬线字体作为产品标题或 Transcript 正文。
- 不通过隐藏 provenance、revision 或 Provider 信息来换取视觉简洁。
- 不在组件中硬编码色彩、间距、字体、圆角和阴影；所有样式必须来自 `docs/design/tokens/base.json`。

## 9. 治理流程

1. Design Token 由 `docs/design/tokens/base.json` 维护。
2. `scripts/generate-design-tokens.mjs` 在开发、测试和构建前生成 `src/design-tokens.css`。
3. 功能开发不得新增未登记 Token；确需新增时先更新本文件与 `base.json`。
4. 每轮发布前检查桌面、平板和移动视口，验证默认、hover、focus、active、disabled、loading、error 和 success 状态。
5. 视觉验收截图保存到 `output/playwright/`，并在对应 PRD 的 process.md 中记录。

## 10. 设计验收清单

- [ ] 页面为统一深色单色基线。
- [ ] 无阴影、渐变和大圆角。
- [ ] 操作、状态、时间和 URI 使用等宽字体。
- [ ] Transcript 长文本使用比例字体并保持舒适行长。
- [ ] 所有样式值来自 Token。
- [ ] 键盘焦点清晰可见。
- [ ] 录音状态同时使用文字与功能色。
- [ ] 桌面、平板与移动端无横向溢出。
- [ ] 浏览器控制台无错误或警告。
- [ ] Tauri 安装包构建通过。
