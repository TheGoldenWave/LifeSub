# Agentic Engineering 核心理念指南

> 本文档按官方推荐格式放置于 `references/` 目录下，作为 Skill 按需加载的参考文档。

## 1. 核心定义
Agentic Engineering 是一种将 AI 视为一等公民的工程架构范式。与传统的代码自动补全不同，它强调由多个专职智能体（Sub-agents）协同工作，通过定义良好的护栏（Guardrails）和动态上下文（Dynamic Contexts）来约束 AI 的行为。

## 2. 关键设计原则

- **Docs as Code (文档即代码)**：所有的 PRD、设计稿和验收标准都必须以文本格式（如 Markdown, JSON）存储在代码库中，随代码一同进行版本控制。
- **Progressive Disclosure (渐进式呈现)**：不要将所有上下文塞给 Agent。通过 `AGENTS.md` 作为路由地图，让 Agent 自行根据任务需要去读取对应的知识。
- **Context Isolation (上下文隔离)**：每个项目的能力（Skills）和扩展（MCP）应该在项目级别被隔离，避免全局工具带来的权限污染和上下文混乱。

## 3. 角色演进
- **产品经理 (PM)**：从写文档转变为“上下文工程师”，主要维护 `docs/prd/` 目录。
- **UI 设计师**：从交付 Figma 切图转变为交付 Design Tokens。
- **开发工程师**：从 CRUD 编写者转变为系统架构师与 Agent 编排者，负责维护 `rules/` 和 `hooks/` 拦截器。
- **测试工程师 (QA)**：转变为评估工程师，负责编写自动化 Eval 脚本。
