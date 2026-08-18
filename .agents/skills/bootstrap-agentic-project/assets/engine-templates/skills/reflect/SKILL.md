---
name: reflect
description: Consolidate project knowledge by scanning feature notes, experience files, and Wiki pages, then updating the knowledge index (docs/context/INDEX.md). Also performs knowledge base health checks (lint). Use after completing a feature, fixing a tricky bug, when session-stop suggests it, or to audit Wiki quality.
user-invocable: true
argument-hint: "[scan|promote|lint|status]"
---

# /reflect — Knowledge Consolidation

Scan project knowledge sources for un-indexed entries and update the team knowledge index (`docs/context/INDEX.md`).

## When to Use

- After completing a feature or significant bugfix
- When `session-stop` hook suggests it (notes.md was recently modified)
- Periodically to keep the knowledge base current
- Before onboarding a new team member to ensure INDEX is up-to-date

## Step 1: Load Current Index

Read `docs/context/INDEX.md`. Parse each category table (架构决策, Bug 模式, 设计模式, 领域知识, 环境与工具) and extract the `详情` (detail path) column values into a set of **already-indexed paths**.

If `docs/context/INDEX.md` doesn't exist, inform the user to run the bootstrap first (`bootstrap-agentic-project` skill or `bash scripts/init.sh` from the skill package).

## Step 2: Scan for Un-indexed Entries

Scan three knowledge source locations:

### 2a. Feature notes
Glob `docs/prd/**/notes.md` and `docs/prd/**/.artifacts/notes.md`. For each file:
- Read content, split by `## ` headers — each section header is a candidate entry
- Check if the file path already appears in INDEX.md — skip if indexed
- Extract a one-line summary from the first non-empty line after the header

### 2b. Experience files
Glob `docs/context/project/experience/**/*.md`. Each file is a candidate:
- Check if its path already appears in INDEX.md — skip if indexed
- Extract title from first `# ` header or filename

### 2b+. Wiki pages
Glob `docs/context/wiki/**/*.md` (excluding `overview.md`). Each file is a candidate:
- Check if its path already appears in INDEX.md — skip if indexed
- Extract title and type from frontmatter or first heading
- Entity pages → classify by entity type; Concept pages → 🧩 设计模式 or 📚 领域知识

### 2c. Blockers from process files (optional)
Glob `docs/prd/**/.artifacts/process.md` and legacy `docs/prd/**/process.txt`. Look for blocker entries:
- In process.md: scan the blocker table for rows with status ≠ "已解决"
- These are potential Bug Pattern entries once resolved

## Step 3: Classify and Present

For each un-indexed candidate, propose a category based on keyword matching:

| Keywords | Category |
|----------|----------|
| 架构, 选型, 模式, pattern, architecture, database, 数据库, API 设计 | 🏗️ 架构决策 |
| bug, 错误, crash, 异常, 边界, race condition, 并发, 失败 | 🐛 Bug 模式 |
| 组件, 封装, 抽象, hook, 工具函数, utility, 复用 | 🧩 设计模式 |
| 业务, 规则, 流程, 用户, 产品, 需求, 域 | 📚 领域知识 |
| CI, 部署, 环境, 版本, 工具, 配置, 脚本 | 🔧 环境与工具 |

Present a numbered list to the user:

```
发现 N 条未索引的知识条目：

1. [🐛 Bug 模式] 并发写入导致乐观锁冲突 — 来源: docs/prd/1.0.0-并发优化-202504/.artifacts/notes.md
2. [🏗️ 架构决策] 选用 Redis 做分布式锁 — 来源: docs/context/project/experience/redis-lock.md
3. ...

请确认要添加到 INDEX.md 的条目编号（如: 1,2,3 或 all），或输入 skip 跳过。
```

Use a normal user-facing confirmation question to get confirmation.

## Step 4: Append to INDEX

For each confirmed entry, append a new table row to the corresponding category in `docs/context/INDEX.md`:

```markdown
| {today's date} | {source feature_id} | {summary ≤ 60 chars} | {relative path to detail file} |
```

Rules:
- Never delete or modify existing rows — append only
- Keep summaries ≤ 60 characters
- Use the detail file's relative path from project root
- Preserve table alignment (pad with spaces if needed)

## Step 5: Promote (Optional)

If the `promote` argument is given, or if the user confirms during Step 3:

For feature-level notes (`docs/prd/{feature}/notes.md` or `.artifacts/notes.md`) that have team-wide reuse value:
1. Extract the relevant section
2. Create a new file under `docs/context/project/experience/{descriptive-name}.md`
3. The new file should have a `# Title` and the distilled content (not a raw copy)
4. The INDEX.md entry should point to this new promoted file

## Step 6: Lint — 知识库健康检查 (Optional)

如果使用 `lint` 参数，执行以下检查而非 scan 流程：

### 6a. 孤立页面检测
Glob `docs/context/wiki/**/*.md`，检查每个页面路径是否在 INDEX.md 的详情列中出现。未出现的 → 标记为孤立页。

### 6b. 交叉引用完整性
读取所有 Wiki 页面，查找 `相关页面` 部分中的链接目标（`../entities/{name}.md`、`../concepts/{name}.md` 等），检查目标文件是否存在。断链 → 标记。

### 6c. 陈旧检测
对每个 Wiki 页面，找到其 `来源引用` 中列出的 `.sources/.converted/` 文件。比较 mtime：如果来源文件比 Wiki 页面更新（说明来源被重新 ingest 了），标记 Wiki 页面可能需要更新。

### 6d. 覆盖度分析
- 统计 `.sources/` 中的原始文件数（排除 `.converted/`）
- 统计 `.sources/.converted/` 中已转换文件数
- 计算 ingest 覆盖率：`已转换 / 原始文件总数`
- 统计 `wiki/` 各子目录页面数

### 6e. 缺失页面建议
扫描所有 Wiki 页面正文，提取频繁出现但没有独立页面的实体/概念名称。使用简单启发式：
- 被 2+ 个不同页面提到
- 不是 Wiki 中已有页面的标题
- 不是通用词汇

### 6f. 生成 Lint 报告

```
🔍 知识库健康检查报告

📊 覆盖度
  来源文件: {N} | 已转换: {M} | 覆盖率: {M/N}%
  Wiki 页面: entities({n}) concepts({n}) comparisons({n}) syntheses({n})

⚠️ 孤立页面 ({N}):
  - docs/context/wiki/entities/xxx.md (不在 INDEX.md 中)

🔗 断链 ({N}):
  - docs/context/wiki/concepts/yyy.md → ../entities/zzz.md (目标不存在)

📅 可能陈旧 ({N}):
  - docs/context/wiki/entities/aaa.md (来源更新于 2026-04-10，页面更新于 2026-03-15)

💡 建议新建页面:
  - "GraphQL" (被 3 个页面提到，无独立页面)
  - "Redis Cluster" (被 2 个页面提到)
```

用自然语言询问是否自动修复（添加孤立页到 INDEX、删除断链、标记陈旧页面需更新）。

追加 `## [YYYY-MM-DD] lint | {summary}` 到 `docs/context/log.md`。

## Sub-commands

- `/reflect` or `/reflect scan` — Run Steps 1-4 (scan + index update)
- `/reflect promote` — Run Steps 1-5 (scan + index + promote to experience/)
- `/reflect lint` — Run Step 6 (knowledge base health check)
- `/reflect status` — Show stats: total indexed entries per category, last update date, coverage (indexed vs total knowledge files), Wiki page counts

## Execution Rules

1. **Never modify INDEX.md without user confirmation** — always present the list first
2. **Preserve existing table structure** — append rows, don't restructure
3. **Be conservative with classification** — when unsure, ask the user which category fits
4. **Date format**: YYYY-MM-DD
5. **Idempotent**: running `/reflect` twice should produce no duplicates (path-based dedup)
6. **Lint is read-only by default** — only writes fixes after explicit user confirmation
7. **Log all operations** — append to `docs/context/log.md` after scan, promote, or lint
