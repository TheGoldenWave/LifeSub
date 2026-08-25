# 🤖 Agentic Engineering 项目协同规范与全局路由

> **⚠️ TO ALL AI AGENTS（Claude Code & Codex CLI）：**
> 这是本项目的**唯一全局入口指南与寻址路由**，适用于所有 AI 工具。
> 本项目严格遵循"下一代 Agentic 项目标准目录规范"。
> 你的任务是根据此地图**精准寻址（Progressive Disclosure）**，只获取你当前任务最需要的上下文。
>
> **🧠 会话恢复指令（每次会话必读）：**
> 在执行任何任务前，你**必须**首先在 `docs/prd/` 目录下搜索所有 `.artifacts/process.md` 文件，并兼容读取遗留的 `process.txt`，以恢复上次会话的上下文状态。
> （Claude Code 用户：此操作已由 SessionStart Hook 自动完成。Codex 用户：请手动执行。）

> **构建与安装附加检查：** 在判断“最新版本”或构建/覆盖 `/Applications/LifeSub.app` 前，必须读取 `docs/workspace-status.md`，核对所有 worktree 的分支、commit、dirty 状态和生产接线。禁止仅凭 `main`、版本号或现有 bundle 判断发布源。

---

## 📍 全局寻址地图 (Context Routing)

请根据你当前的任务目标，前往以下目录读取详细上下文：

- **🧠 记忆与经验库** → `docs/context/`
  - 遇到未知错误或架构决策前，**必须**先查阅 `docs/context/INDEX.md` 和 `docs/context/project/experience/`。
  - 研究长期采集、跨设备、视觉、硬件或 Evidence 上层流转前，先读 `docs/product-open-questions.zh-CN.md`，不得把开放问题提前当作当前范围。
- **🧑‍💼 业务与需求 (PM 视窗)** → `docs/prd/{feature_id}/`
  - 开发新需求前，必须阅读对应的 `PRD.md`（可通过双击 `预览PRD-macOS.command` / `预览PRD-Windows.bat` 启动双视窗预览）。
  - **文档规范：** PRD 必须采用 Markdown 格式。业务流程请使用 ` ```mermaid `，时序图/架构图请优先使用 ` ```plantuml `。
  - **存档规则：** 每次会话结束前或执行关键步骤后，**必须**将进度更新到 `.artifacts/process.md`（含 YAML frontmatter stage 字段），将踩坑记录写到 `.artifacts/notes.md`，以防跨会话失忆。
  - **目录约定：** `PRD.md` + 启动脚本在根目录供人查阅；`PRD_dual-pane.html`、`process.md`、`notes.md` 统一放在 `.artifacts/` 子目录，由 AI Agent 维护。
- **📊 项目进度 (PM 视窗)** → `docs/prd/{feature_id}/.artifacts/process.md`
  - 查看排期：`/progress view {feature_id}`
  - 更新进度：`/progress update {feature_id}`
  - 记录阻塞：`/progress block {feature_id} "描述"`
- **📂 需求上下文** → `docs/context/`
  - 业务提需：`docs/context/business/{unit}/{req_id}/` — 含原始设想、需求确认单、MRD
  - 产品自发：`docs/context/product-initiated/{req_id}/` — 含洞察、产品简报
  - 技术实现：`docs/context/technical/{feature_id}/` — 技术笔记、决策记录
- **🎨 设计与样式 (UI 视窗)** → `docs/design/`
  - 严禁在代码中硬编码颜色、间距等样式值，必须引用 `docs/design/tokens/base.json` 中的 Token。
  - 项目设计规范存储在 `docs/design/tokens/impeccable.md`，与 `base.json` 同目录，由 ui-agent 通过 `teach-impeccable` Skill 建立，不要移动该文件。
- **🕵️ 测试与验收 (QA 视窗)** → `tests/`
  - 测试与审查的单一政策源：`docs/testing-and-review-policy.md`。
  - 行为、数据、安全和接口契约变更必须 RED/GREEN；纯视觉、文档、配置和生成物使用与风险匹配的可重复验证，不强制制造单元测试。
  - 跨模块用户路径放在 `tests/specs/`；模块测试靠近模块；视觉证据存入 `output/playwright/`。
- **🔌 外部系统扩展 (MCP)**
  - Claude Code → `.claude/mcp-servers.json`
  - Codex CLI → `.codex/config.toml`（`[mcp_servers.*]` 部分）
  - 需要连接 TAPD、知识库或数据库时，请检查并使用对应配置。

---

## 👥 跨角色协作 (Agent Roster)

遇到非你专长领域的问题，请请求对应专职 Agent 协助：

| Agent | 职责范围 | Claude Code | Codex CLI |
|-------|---------|------------|-----------|
| `pm-agent` | 需求澄清、双路径工作流（业务提需/产品自发）、PRD 维护 | `/prd [需求描述]` | 明确要求使用 `pm` 自定义 subagent |
| `project-manager-agent` | 项目排期、进度追踪、阻塞管理、状态报告 | `/progress [指令]` | 明确要求使用 `project-manager` 自定义 subagent |
| `dev-agent` | 业务代码实现、按风险选择验证策略 | 直接 @提及 | 明确要求使用 `dev` 自定义 subagent |
| `ui-agent` | 设计规范建设（teach-impeccable）、Design Token 维护、PRD 双视窗创建 | 直接 @提及 | 明确要求使用 `ui` 自定义 subagent |
| `architect-agent` | 系统设计、架构决策、Code Review | 直接 @提及 | 明确要求使用 `architect` 自定义 subagent |
| `qa-agent` | 验收策略、自动化与视觉/真实环境质量评估 | 直接 @提及 | 明确要求使用 `qa` 自定义 subagent |

---

## 🛡️ 强制护栏与动态上下文 (Guardrails & Contexts)

### 基础规则（始终生效）

你在编写任何代码前，必须确保遵守引擎目录下 `rules/common/coding-style.md` 中定义的全局规范：
- Claude Code 路径：`.claude/rules/common/coding-style.md`
- Codex 路径：优先读取 `.claude/rules/common/coding-style.md`

测试、验收与审查必须遵守 `docs/testing-and-review-policy.md`。任何角色文件中的简写若与该政策冲突，以该政策为准。

### 动态上下文（按需激活）

进入特定工作模式时，可主动读取对应的 Context 文件以获取补充指导：

- **开发模式**：读取 `.claude/contexts/dev.md`
  → 告知 Agent "请进入开发模式"，Agent 会主动读取该文件。
- **审查模式**：读取 `.claude/contexts/review.md`
  → 执行 Code Review 任务时，主动读取该文件获取审查清单。

### 复审节奏与放行标准

- 每个 Task 默认**单轮合并审查**：同一 reviewer 在一次 pass 中同时覆盖规格符合性和代码质量与安全。
- 放行标准：**Critical = 0**。Critical 定义：数据损坏、权限绕过、身份/来源伪造、不安全发布或不可恢复安全失败。
- Important（核心验收失败或重大行为缺陷）在审查中提出，修复后可同 PR 合入，或记录证据后延至下一 Task 处理，不阻塞当前审查放行。
- Minor 记录到 feature 的 `.artifacts/notes.md`，不阻塞交付。
- 每个 finding 包含：严重级别、文件/行号、违反项、最小修复。验证命令仅对 Critical 为必填。
- 复审只覆盖当前 Task 的 ownership 与验收条件，不把未来 Task 要求前置。
- 审查前须通过 Tier 2 检查（`scripts/check.sh tier2` 或 `make check`）：focused+related tests、`cargo fmt --check`、`cargo clippy -- -D warnings`、`git diff --check`。
- Reviewer 必须判断验证手段是否匹配变更类型；不得仅因纯视觉/文档/配置改动没有新增单元测试而拒绝放行。

### 强制拦截（Claude Code 自动触发 / Codex 指令驱动）

**Claude Code（Hook 自动执行）：**
- **`check-console-log.js`**：任何 Edit 工具写入 `.ts/.tsx/.js/.jsx` 文件时，自动检测并阻止提交遗留的 `console.log`。
- **`session-stop.js`**：会话结束前自动提醒你保存进度到 `.artifacts/process.md`（兼容遗留 `process.txt`）。
- **`session-start.js`**：会话开始时自动扫描并注入所有 `.artifacts/process.md` 与遗留 `process.txt`（含 YAML stage 字段提取）到上下文，恢复记忆。

**Codex CLI（当前模板默认使用指令约束）：**
- 提交前手动检查是否含 `console.log`，如有则删除。
- 完成关键步骤后手动更新 `docs/prd/{feature_id}/.artifacts/process.md`。
- 会话开始时优先读取 `.artifacts/process.md`，再兼容读取遗留的 `process.txt`。

---

## 🔧 工具兼容性速查

| 功能 | Claude Code | Codex CLI |
|------|------------|-----------|
| Agent 角色定义 | `.claude/agents/*.md` | `.codex/agents/*.toml` |
| MCP 配置 | `.claude/mcp-servers.json` | `.codex/config.toml` |
| 自动化 Hooks | ✅ `.claude/scripts/hooks/` | 可选 `.codex/hooks.json`（本模板默认未配置，且主要用于 Bash 级护栏） |
| 技能加载 | `.claude/skills/` 或插件 | `.agents/skills/`（自动发现） |
| 斜线命令 | `.claude/commands/` | 自然语言 + `/skills`；自定义 subagent 通过明确委派触发 |
| 进度存档 | Stop Hook 自动提醒 | 手动执行 |
