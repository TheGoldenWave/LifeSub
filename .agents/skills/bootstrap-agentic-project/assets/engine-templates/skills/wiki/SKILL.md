---
name: wiki
description: 查询项目知识库、生成 Wiki 页面（实体/概念/比较/综合/总览）、管理 Wiki 层。当用户想要查阅项目积累的知识、生成技术分析对比、或了解知识库状态时触发。
user-invocable: true
argument-hint: "[query <topic>|generate <type> [name]|status|save]"
---

# /wiki — 知识库查询与页面管理

查询项目 Wiki 知识库，按需生成结构化 Wiki 页面，管理知识层状态。

## 三层架构定位

```
.sources/.converted/     ← 原始知识（Markitdown 转换后的 Markdown）
docs/context/            ← Wiki 层（你维护的知识体系）
  ├── INDEX.md           ← 知识索引
  ├── log.md             ← 操作日志
  ├── wiki/
  │   ├── entities/      ← 实体页（API、服务、库、组件）
  │   ├── concepts/      ← 概念页（模式、原则、技术）
  │   ├── comparisons/   ← 比较页（A vs B）
  │   ├── syntheses/     ← 综合页（query 回归存档）
  │   └── overview.md    ← 项目知识总览
  └── project/experience/ ← 团队经验沉淀
```

## 子命令

---

### /wiki query \<topic\>

综合查询知识库，生成带引用的回答。

#### 工作流

1. **索引检索**：读取 `docs/context/INDEX.md`，在 5 个分类表格中搜索与 topic 相关的条目
2. **Wiki 页面扫描**：Glob `docs/context/wiki/**/*.md`，读取文件名和首行标题，找到匹配页面
3. **深度阅读**：读取所有匹配的 Wiki 页面全文
4. **源文档参考**：如需要更详细信息，读取 `.sources/.converted/` 中相关的转换文档
5. **经验文件**：检查 `docs/context/project/experience/` 中的相关文件
6. **综合回答**：基于所有来源，生成结构化回答：
   - 引用具体来源文件路径
   - 标注信息来自哪个 Wiki 页面或原始文档
   - 如有矛盾信息，明确指出并说明各来源观点

7. **回归询问**：回答完成后，用自然语言询问用户：

   ```
   是否将此回答存为 Wiki 综合页？
   选项：
   1. 是，存为 docs/context/wiki/syntheses/{topic-slug}.md
   2. 否，仅作为会话回答
   ```

8. **存档**（若用户选择是）：
   - 写入 `docs/context/wiki/syntheses/{topic-slug}.md`
   - 更新 INDEX.md
   - 追加 log.md：`## [YYYY-MM-DD] query | {topic} → syntheses/{topic-slug}.md`

---

### /wiki generate entity \<name\>

为指定实体生成标准化 Wiki 页面。

#### 工作流

1. **全源扫描**：搜索以下位置中与 `{name}` 相关的内容：
   - `docs/context/wiki/` 已有页面
   - `docs/context/project/experience/`
   - `docs/prd/**/.artifacts/notes.md`
   - `.sources/.converted/`
2. **生成页面**：

```markdown
# {Entity Name}

> 类型：{API|服务|库|组件|工具|框架|...} | 首次记录：{today} | 来源数：{N}

## 概述

{基于所有来源综合的一段话描述}

## 关键属性

| 属性 | 值 |
|------|----|
| ... | ... |

## 使用方式

{代码示例或使用说明，如适用}

## 注意事项

{已知限制、常见陷阱、从 notes.md 中提取的踩坑记录}

## 相关页面

- [概念：{related}](../concepts/{name}.md)
- [实体：{related}](./{name}.md)

## 来源引用

- [{source}]({path}) — {context}
```

3. **写入** `docs/context/wiki/entities/{kebab-name}.md`
4. **更新 INDEX.md + log.md**

---

### /wiki generate concept \<name\>

为指定概念生成标准化 Wiki 页面。

#### 工作流

同 entity，但使用概念页模板：

```markdown
# {Concept Name}

> 首次记录：{today} | 来源数：{N}

## 定义

{精确的一段话定义}

## 核心原理

{底层机制或理论基础}

## 应用场景

{什么时候用，解决什么问题}

## 最佳实践

1. {practice 1}
2. {practice 2}

## 反模式

- ❌ {anti-pattern 1} — {why it's bad}

## 相关概念

- [概念：{related}](../concepts/{name}.md)

## 来源引用

- [{source}]({path}) — {context}
```

写入 `docs/context/wiki/concepts/{kebab-name}.md`。

---

### /wiki generate comparison \<a\> \<b\>

生成 A vs B 对比分析页。

#### 工作流

1. 在 Wiki 和 .converted/ 中搜索 A 和 B 的信息
2. 生成比较页：

```markdown
# {A} vs {B}

> 生成日期：{today}

## 对比概览

| 维度 | {A} | {B} |
|------|-----|-----|
| 类型 | ... | ... |
| 适用场景 | ... | ... |
| 优势 | ... | ... |
| 劣势 | ... | ... |
| 性能 | ... | ... |
| 社区/生态 | ... | ... |

## 详细对比

### {维度 1}
{深入分析...}

### {维度 2}
{深入分析...}

## 推荐场景

- **选 {A} 当**：{conditions}
- **选 {B} 当**：{conditions}

## 项目中的使用

{如果项目中已有使用记录，说明当前状态}

## 来源引用

- [{source}]({path}) — {context}
```

写入 `docs/context/wiki/comparisons/{a}-vs-{b}.md`。

---

### /wiki generate overview

重新生成项目知识总览。

#### 工作流

1. Glob `docs/context/wiki/**/*.md` 获取所有 Wiki 页面
2. 读取每个页面的标题和概述段
3. 读取 INDEX.md 获取各分类条目数
4. 生成 `docs/context/wiki/overview.md`：

```markdown
# 项目知识总览

> 上次更新：{today}
> 实体页：{N} | 概念页：{N} | 比较页：{N} | 综合页：{N}
> INDEX 条目：{N} | 来源文档：{N}

## 知识全景

{2-3 段话概括项目积累的核心知识领域}

## 实体索引

| 实体 | 类型 | 概述 |
|------|------|------|
| [{name}](entities/{name}.md) | {type} | {one-line} |

## 概念索引

| 概念 | 概述 |
|------|------|
| [{name}](concepts/{name}.md) | {one-line} |

## 比较索引

| 对比 | 结论 |
|------|------|
| [{a} vs {b}](comparisons/{a}-vs-{b}.md) | {one-line} |

## 综合研究

| 主题 | 日期 | 概述 |
|------|------|------|
| [{topic}](syntheses/{topic}.md) | {date} | {one-line} |

## 知识空白

{基于当前覆盖度，建议需要补充的领域}
```

---

### /wiki status

显示知识库统计信息。

#### 工作流

1. 统计各目录文件数：
   - `.sources/` 中的原始文件数（按类型）
   - `.sources/.converted/` 中的转换文件数
   - `docs/context/wiki/entities/` 页面数
   - `docs/context/wiki/concepts/` 页面数
   - `docs/context/wiki/comparisons/` 页面数
   - `docs/context/wiki/syntheses/` 页面数
   - `docs/context/project/experience/` 文件数
2. 解析 INDEX.md 各分类条目数
3. 读取 log.md 最近 5 条操作
4. 输出格式化报告：

```
📊 Wiki 知识库状态

来源层 (.sources/)
  📄 原始文件: {N} (articles: {n}, docs: {n}, data: {n})
  📝 已转换:   {N}
  📊 覆盖率:   {N/M}% ({N} 已 ingest / {M} 总文件)

Wiki 层 (docs/context/wiki/)
  🏢 实体页:   {N}
  💡 概念页:   {N}
  ⚖️  比较页:   {N}
  🔬 综合页:   {N}
  📖 总览:     {last updated or '尚未生成'}

索引层 (docs/context/)
  📋 INDEX 条目: {N} (架构:{n} Bug:{n} 设计:{n} 领域:{n} 工具:{n})
  📓 经验文件:  {N}

最近操作:
  {last 5 log entries}
```

---

### /wiki save

将上一次 `/wiki query` 的回答手动存为综合页。

作为自动询问机制的补充：如果用户在 query 后选择了"否"，但后来改变主意，可以用此命令。

#### 工作流

1. 提示用户提供主题名称（如不在 query 上下文中）
2. 用自然语言向用户确认文件名
3. 写入 `docs/context/wiki/syntheses/{topic-slug}.md`
4. 更新 INDEX.md + log.md

## 执行规则

1. **只读 .sources/**：Wiki 技能只读取 `.sources/.converted/`，不修改任何原始文件
2. **幂等生成**：如目标页面已存在，先读取再决定是更新还是跳过
3. **交叉引用**：每次创建新页面时，扫描已有页面添加双向 `相关页面` 链接
4. **日期格式**：YYYY-MM-DD
5. **文件命名**：kebab-case（`redis-sentinel.md`，`event-driven-architecture.md`）
6. **log.md 格式**：`## [YYYY-MM-DD] {operation} | {description}`
