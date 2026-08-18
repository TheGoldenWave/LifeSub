# LifeSub Logo 与 macOS 菜单栏呈现决策

- 状态：已确认
- 日期：2026-08-15
- 适用范围：菜单栏模板图标、录音状态表达、Dock 图标、官网与品牌主视觉

## 1. 最终决策

LifeSub 采用同源但分层的视觉标识体系：

- **菜单栏常驻图标**：采用 `A1 · Balanced Evidence Trace`。
- **录音状态变体**：继续使用 A1 的轮廓，通过间断播放的音波脉冲表达“正在录音”，不在基础 Logo 上永久附加独立录音圆点。
- **Dock / App Icon**：采用已定稿的 `B2 · Evidence First Narration Bubble`。
- **官网和品牌主视觉**：以 B2 为核心母版，可按场景扩展文字组合与低频动效。
- **统一关系**：A1 是 B 内部“声音成为可定位证据”轨迹的菜单栏精简版本；两者不是互不相关的两套 Logo。

核心语义：

> LifeSub 把持续发生的声音，可靠地保存为可定位、可引用的证据。

## 2. 菜单栏图标

### 2.1 基础形态

A1 使用单轨、单主峰、左右延伸稳定的 Evidence Trace：

- 逻辑画布：`18 × 18 pt`。
- 透明背景、单色 alpha template。
- 圆角线端与连接。
- 不烘焙白色、黑色、品牌色、阴影、渐变或底板。
- 由 macOS 根据菜单栏外观和选中状态自动着色。
- 资源母版：[`evidence-trace-variants/a1-balanced.svg`](lifesub-logo-concepts/evidence-trace-variants/a1-balanced.svg)。

A1 作为常驻标识时保持静止，不持续运动，避免干扰用户或制造“始终在录音”的错觉。

### 2.2 录音状态

录音中仍以 A1 为母形，不采用右上角常驻圆点方案。状态通过**间断播放的音波脉冲**表达：

1. 动画按短时 pulse 播放，之后停顿；不做永不停止的连续抖动。
2. 波形以轻微振幅变化、局部线段推进或亮度脉冲为主，不改变 Logo 基本轮廓。
3. 单次 pulse 建议约 `500–800 ms`，组间停顿建议约 `1.5–3 s`；最终时序需在原生菜单栏中验证。
4. 动画必须克制，不影响旁边的系统状态和文字扫描。
5. `Reduce Motion` 开启时禁用动画，使用静态状态替代，并在菜单或状态文字中明确显示“正在录音”。
6. 状态不能只依赖动画或颜色；下拉菜单、辅助功能名称和管理界面必须同步提供文字状态。
7. 暂停、异常、权限丢失与存储不足不复用“录音中”脉冲，应使用静态状态变体并配合明确文字。

录音状态动画是产品状态，不是品牌 Logo 本体。导出 Logo、文档和官网静态使用时只使用基础 A1。

## 3. 刘海屏位置探索

在带刘海屏的 MacBook 上，可以探索将 A1 放在刘海左侧或右侧的专属邻近位置，使 LifeSub 与普通第三方菜单栏图标形成更明确的产品识别，减少混排感。

该方向是**优先探索，不是已保证可实现的系统能力**：

- 先验证 macOS 公共 API、不同菜单栏布局和不同屏幕型号下是否能稳定呈现。
- 不使用依赖固定屏幕坐标的脆弱实现。
- 不遮挡系统图标、App 菜单、控制中心或辅助功能区域。
- 需要兼容无刘海外接显示器、较窄窗口菜单、多个显示器和菜单栏自动隐藏。
- 用户必须能够选择普通菜单栏位置；专属刘海邻近位置不可成为唯一入口。
- 如果公共 API 无法保证稳定位置，则退回标准 `NSStatusItem` 菜单栏排列，不使用私有 API强行定位。

建议偏好设置：

```text
菜单栏位置
- 自动（推荐）
- 标准菜单栏
- 刘海左侧（支持时）
- 刘海右侧（支持时）
```

“自动”应根据可用空间、系统能力和用户既有菜单栏布局选择，且随外接屏切换安全回退。

## 4. Dock 与品牌主视觉

`B2 · Evidence First Narration Bubble` 用于更大尺寸和更完整的品牌表达：

- 对话气泡体现中文名“旁白”和声音记录。
- 气泡内部使用与 A1 同源的 Evidence Trace，而不是另一套波形。
- Dock / App Icon 使用 `brand.canvas` 深色圆角底板、`brand.surfaceSubtle` 气泡内层、`brand.borderStrong` 加宽边框和 `brand.textPrimary` Evidence Trace；菜单栏模板仍保持纯单色。
- 气泡作为第二层语义，Evidence Trace 保持第一视觉焦点；不加入永久录音红点，避免与录音状态混淆。
- 正式几何母版为 `1024 × 1024` SVG，气泡边框宽度为 `42/1024`，由 Tauri CLI 生成平台图标集。
- 官网、启动页、文档封面与社交媒体可使用 B 的完整形态。
- B2 已通过 16/32/64/128/256/512/1024 尺寸导出检查；小尺寸保留气泡轮廓和单主峰轨迹。

正式母版：[`lifesub-app-icon.svg`](lifesub-app-icon.svg)。历史概念稿 [`concept-b-narration-bubble.svg`](lifesub-logo-concepts/concept-b-narration-bubble.svg) 仅保留设计演进记录，不再用于导出。

## 5. 状态与资产矩阵

| 场景 | 标识 | 动态 | 色彩 |
|---|---|---|---|
| 菜单栏空闲 | A1 Evidence Trace | 静止 | macOS template 自动着色 |
| 菜单栏录音中 | A1 Evidence Trace | 间断音波 pulse | template；必要状态色由原生实现评估 |
| 菜单栏暂停/异常 | A1 的静态状态变体 + 文字 | 不使用录音 pulse | 系统状态色与文字共同表达 |
| Dock / App Icon | B2 Evidence First Narration Bubble | 静态 | LifeSub 品牌色体系 |
| 官网与品牌主视觉 | B2 Narration Bubble + A1 Evidence Trace | 可做低频品牌动效 | 品牌色体系 |
| Evidence 引用与产品内功能 | A1 / Evidence Point 等派生符号 | 按交互需要 | 遵循 Design Tokens |

## 6. 禁止事项

- 不把 A6 的独立圆点永久并入常驻主 Logo。
- 不让菜单栏图标持续高频抖动、闪烁或循环播放。
- 不用颜色作为唯一录音状态。
- 不在 template image 内固定白色或黑色。
- 不为刘海位置使用私有 API、固定坐标或无法安全回退的方案。
- 不让 Dock Logo 与菜单栏 Logo 发展成没有共同 Evidence Trace 的两套视觉语言。

## 7. 现有设计资产

```text
docs/design/lifesub-logo-concepts/
├── concept-a-evidence-trace.svg
├── concept-b-narration-bubble.svg
├── preview.html
├── preview.html.png
└── evidence-trace-variants/
    ├── a1-balanced.svg            # 菜单栏最终方向
    ├── a2-calm.svg
    ├── a3-evidence-point.svg
    ├── a4-split-sources.svg
    ├── a5-chunk-marks.svg
    ├── a6-record-trace.svg        # 保留为探索稿，不作为录音最终方案
    ├── preview.html
    └── preview.html.png

docs/design/lifesub-app-icon.svg       # B2 正式 App Icon 母版
src-tauri/icons/                       # Tauri CLI 生成的平台图标集
public/favicon.svg                     # 同源简化网页图标
```

## 8. 后续交付

- 基于 A1 制作 macOS template image 资产与原生 `NSStatusItem` 验证。
- 制作 A1 的录音 pulse 动画原型，并验证 CPU、功耗、Reduce Motion 和视觉干扰。
- Spike 刘海左/右邻近位置的公共 API 可行性与多屏回退。
- 基于 B2 扩展官网横版、文档封面和社交媒体组合规范。
- 在真实浅色、深色、彩色壁纸菜单栏及 1x/2x 缩放下做截图验收。
