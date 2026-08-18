# 🧠 团队知识库索引 (Knowledge Index)

> **用途**：开始任何任务前，先按分类检索本索引，避免重踩已知坑点。
> **维护**：运行 `/reflect` 自动扫描未索引的经验记录并追加到对应分类表格中。
> **格式**：每条记录为一行表格行，摘要 ≤ 60 字，详情列指向完整文档路径。

---

## 🏗️ 架构决策 (Architecture Decisions)

| 日期 | 来源 | 摘要 | 详情 |
|------|------|------|------|
| <!-- 示例: 2025-04-10 --> | <!-- 1.0.0-用户登录-202504 --> | <!-- 选用 PostgreSQL 而非 MongoDB，因为需要事务一致性 --> | <!-- docs/context/project/experience/db-choice.md --> |

## 🐛 Bug 模式 (Bug Patterns)

| 日期 | 来源 | 摘要 | 详情 |
|------|------|------|------|
| <!-- 示例: 2025-04-12 --> | <!-- 1.0.0-并发优化-202504 --> | <!-- 并发写入导致乐观锁冲突，需 retry 机制 --> | <!-- docs/context/project/experience/optimistic-lock-retry.md --> |

## 🧩 设计模式 (Design Patterns)

| 日期 | 来源 | 摘要 | 详情 |
|------|------|------|------|
| <!-- 示例: 2025-04-11 --> | <!-- 1.0.0-用户登录-202504 --> | <!-- 统一使用 Repository 模式封装数据访问层 --> | <!-- docs/context/project/experience/repository-pattern.md --> |

## 📚 领域知识 (Domain Knowledge)

| 日期 | 来源 | 摘要 | 详情 |
|------|------|------|------|
| <!-- 示例: 2025-04-13 --> | <!-- 1.1.0-优惠券系统-202504 --> | <!-- 优惠券叠加规则：同类不叠加，异类最多叠 2 张 --> | <!-- docs/context/project/experience/coupon-rules.md --> |

## 🔧 环境与工具 (Environment & Tooling)

| 日期 | 来源 | 摘要 | 详情 |
|------|------|------|------|
| <!-- 示例: 2025-04-09 --> | <!-- infra --> | <!-- CI 环境 Node 版本必须 ≥ 18，否则 esbuild 构建失败 --> | <!-- docs/context/project/experience/ci-node-version.md --> |

---

> **给 Agent 的指示**：当你在需求开发中（如 `docs/prd/*/notes.md`）记录了坑点后，请将其提炼到 `docs/context/project/experience/` 目录下，并运行 `/reflect` 或手动更新此索引。保持摘要简洁（≤ 60 字），详情列填写完整相对路径。
