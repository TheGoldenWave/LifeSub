---
name: ingest
description: 将外部知识源（PDF、网页、文档、数据文件）转换为 Markdown 并纳入项目 Wiki 知识体系。当用户想要导入研究文章、技术文档、竞品分析等外部资料时触发。依赖 Markitdown（MCP 或 CLI）进行格式转换。
user-invocable: true
argument-hint: "<path-or-url> [--batch <dir>] [--type article|doc|data]"
---

# /ingest — 外部知识源导入

将外部文档（PDF、DOCX、PPTX、HTML、XLSX、URL 等）转换为 Markdown，提取关键知识，生成或更新 Wiki 页面，并更新索引和操作日志。

## 三层架构定位

```
.sources/              ← Layer 1: Raw immutable（你只读不写）
  ├── .converted/      ← Markitdown 转换缓存（你写入转换结果）
docs/context/          ← Layer 2: Wiki layer（你生成和更新页面）
  ├── INDEX.md         ← 知识索引（你追加条目）
  ├── log.md           ← 操作日志（你追加记录）
  └── wiki/            ← Wiki 页面（你创建和维护）
```

**核心原则**：原始文件不可变 → 转换为 Markdown → 提取知识生成 Wiki 页面 → 知识复利累积。

## 前置检查

1. 确认 `.sources/` 目录存在（不存在则提示运行 bootstrap）
2. 确认 Markitdown 可用（按优先级检测）：
   - **Codex**：检查 `.Codex/mcp-servers.json` 中是否配置了 `markitdown` MCP server
   - **Codex CLI**：检查 `.codex/config.toml` 中是否有 `[mcp_servers.markitdown]` 配置
   - **CLI 备选**：检查 `markitdown` CLI 是否可用（`which markitdown` 或 `pip show markitdown`）
   - 都不可用 → 提示用户安装：`pip install markitdown` 或 `uvx markitdown-mcp`

## Step 1: 检测源类型

解析用户传入的参数：

| 参数形式 | 处理方式 |
|----------|----------|
| 本地文件路径（`.pdf`, `.docx`, `.pptx`, `.html`, `.xlsx` 等） | 单文件转换 |
| URL（`http://`, `https://`） | 通过 MCP 的 `convert_to_markdown(uri)` 获取 |
| `--batch <dir>` | 批量转换整个目录 |

如果文件不在 `.sources/` 中，先复制到对应子目录：
- `--type article` 或推断为文章 → `.sources/articles/`
- `--type doc` 或推断为文档 → `.sources/docs/`
- `--type data` 或推断为数据 → `.sources/data/`
- 默认 → `.sources/docs/`

## Step 2: Markitdown 转换

### 单文件转换

**MCP 模式**（优先）：
```
调用 MCP markitdown 的 convert_to_markdown 工具：
  uri: "file:///absolute/path/to/source.pdf"
```

**CLI 模式**（备选）：
```bash
markitdown ".sources/docs/source.pdf" > ".sources/.converted/source.md"
```

### 批量目录转换
```bash
python .Codex/scripts/markitdown/batch_convert.py \
  ".sources/articles/" \
  ".sources/.converted/" \
  --extensions .pdf .docx .pptx .html .xlsx \
  -r --workers 4
```

### 转换结果处理

将转换结果存入 `.sources/.converted/{original-name}.md`，并在文件顶部添加 YAML frontmatter：

```yaml
---
source: .sources/docs/original-filename.pdf
original_format: pdf
converted_date: 2026-04-13
size_chars: 15234
title: {从内容中提取的标题}
---
```

**缓存检查**：转换前检查 `.sources/.converted/{name}.md` 是否已存在。若存在且 mtime 晚于源文件，跳过转换并告知用户 "已有缓存，直接使用"。

## Step 3: LLM 分段阅读

读取 `.sources/.converted/` 中的转换结果。

### 小文件（≤ 8000 字符）
直接完整读取，提取：
- 关键实体（API、服务、库、组件、工具、人物、组织）
- 核心概念（模式、原则、方法论、技术）
- 重要关系（依赖、替代、比较、冲突）

### 大文件（> 8000 字符）
分段读取：
1. 先读取前 4000 字符 + 后 2000 字符，获取文档结构概览
2. 按章节/段落边界分段（每段 ~4000 字符，相邻段有 500 字符 overlap）
3. 逐段阅读，提取实体和概念
4. 最后合并所有提取结果，去重

## Step 4: Wiki 页面生成/更新

基于提取的知识，生成或更新 Wiki 页面。

### 页面类型和模板

**Entity 页**（`docs/context/wiki/entities/{kebab-name}.md`）：
```markdown
# {Entity Name}

> 类型：{API|服务|库|组件|工具|...} | 首次记录：{date} | 来源数：{N}

## 概述

{一段话描述该实体是什么、做什么}

## 关键属性

{结构化属性列表，因实体类型而异}

## 相关页面

- [概念：{related-concept}](../concepts/{name}.md)
- [实体：{related-entity}](./{name}.md)

## 来源引用

- [{source title}](../../../.sources/.converted/{filename}.md) — {one-line context}
```

**Concept 页**（`docs/context/wiki/concepts/{kebab-name}.md`）：
```markdown
# {Concept Name}

> 首次记录：{date} | 来源数：{N}

## 定义

{清晰的一段话定义}

## 应用场景

{什么时候用，解决什么问题}

## 最佳实践

{从来源中提炼的实践建议}

## 反模式

{常见的错误用法}

## 相关概念

- [概念：{related}](../concepts/{name}.md)
```

### 更新已有页面

如果 `wiki/entities/` 或 `wiki/concepts/` 中已有同名页面：
1. 读取现有内容
2. 在 "来源引用" 部分追加新来源
3. 如新来源包含现有页面没有的信息，补充到对应章节
4. 更新 frontmatter 中的 `来源数`

### 用户确认

展示变更列表，用自然语言向用户确认：

```
📥 Ingest 完成：{source name}

发现以下知识条目，将创建/更新 Wiki 页面：

🆕 新建页面：
  1. [实体] Redis Sentinel → docs/context/wiki/entities/redis-sentinel.md
  2. [概念] 主从复制 → docs/context/wiki/concepts/master-slave-replication.md

📝 更新页面：
  3. [实体] Redis (追加 Sentinel 相关内容)

请确认要执行的操作（如: 1,2,3 或 all），或输入 skip 跳过。
```

**只有用户确认后才写入文件。**

## Step 5: 索引更新

### INDEX.md
将新建的 Wiki 页面添加到 `docs/context/INDEX.md` 对应分类表格：
- Entity 页 → 根据实体类型选择分类（API/架构类 → 🏗️ 架构决策，工具类 → 🔧 环境与工具）
- Concept 页 → 根据概念类型选择分类（设计模式类 → 🧩 设计模式，业务概念 → 📚 领域知识）

### log.md
追加操作记录到 `docs/context/log.md`：

```markdown
## [2026-04-13] ingest | Redis Sentinel 技术文档

- 来源：`.sources/docs/redis-sentinel-guide.pdf`
- 转换：`.sources/.converted/redis-sentinel-guide.md`（15234 字符）
- 新建页面：entities/redis-sentinel.md, concepts/master-slave-replication.md
- 更新页面：entities/redis.md
- INDEX 新增条目：2
```

## 执行规则

1. **永远不修改 `.sources/` 中的原始文件**（`.converted/` 除外）
2. **缓存优先**：转换前检查 `.converted/` 是否已有有效缓存
3. **用户确认**：Wiki 页面创建/更新前必须请求确认
4. **幂等性**：重复 ingest 同一文件不会产生重复页面（通过来源路径去重）
5. **交叉引用**：新建页面时扫描已有页面，添加双向链接
6. **增量更新**：更新已有页面时只追加，不删除已有内容
