# Bootstrap Agentic Project Skill 🚀

[![GitHub](https://img.shields.io/github/license/TheGoldenWave/bootstrap-agentic-project)](https://github.com/TheGoldenWave/bootstrap-agentic-project)

**GitHub**: https://github.com/TheGoldenWave/bootstrap-agentic-project

这是一个为 **Claude Code** 和 **Codex CLI** 深度定制的"Agentic Engineering（智能体研发引擎）"项目初始化技能。它可以帮助研发团队**一键**将当前的代码仓库转化为拥有 **多角色子 Agent 团队**、**项目级隔离环境** 和 **完美兼容 ECC (Everything-Claude-Code)** 的标准化 AI 协同工作区。

## ✨ 核心特性

- 🤖 **开箱即用的 Agent 团队**：内置六大专职角色（PM、Project Manager、Dev、Architect、UI、QA），覆盖完整的产品研发链路。
- 📋 **PM 双路径工作流**：支持业务提需（确认单 → MRD → PRD）和产品自发（产品简报 → PRD）两条需求路径，智能识别 + 会话内自动流转。
- 🔄 **双引擎兼容**：同一个项目同时兼容 **Claude Code**（`.claude/`）和 **Codex CLI**（`.codex/`），无缝切换。
- 🛡️ **可工作的 Hook 护栏（Claude Code）**：`console.log` 自动拦截 + 硬编码样式值拦截（PreToolUse）+ 进度存档提醒（Stop）+ 会话记忆恢复（SessionStart）。
- 📜 **Codex 默认指令护栏**：模板默认不启用 Codex hooks，先通过指令约束实现基础安全规范；如需更强自动化，可再补 `.codex/hooks.json`。
- 🔌 **项目专属 MCP 容器**：Claude Code 使用 `mcp-servers.json`，Codex CLI 使用 `.codex/config.toml`，`.gitignore` 自动保护 API Key。
- 🧠 **防失忆机制（跨会话记忆）**：`.artifacts/process.md` 状态存档 + Claude Code SessionStart Hook 自动恢复（兼容读取遗留 `process.txt`）+ Codex `developer_instructions` 持续提醒。
- 📚 **结构化知识库 + Wiki 层**：`docs/context/INDEX.md` 五分类表格索引 + `docs/context/wiki/` LLM 生成的 Wiki 页面体系（实体/概念/比较/综合/总览）+ `/reflect` 技能自动扫描与健康检查，团队知识持续积累不丢失。
- 📥 **外部知识源导入**：`.sources/` 目录存放外部文件（PDF/DOCX/PPTX/HTML 等），通过 `/ingest` 技能 + Markitdown 自动转换并生成 Wiki 页面，知识复利累积。
- 🎨 **Design as Code 基础设施**：内置 `docs/design/tokens/base.json` 基准 Token 文件，所有颜色/间距/字号禁止硬编码。
- 🎭 **"所见即所得"的双视窗 PRD 架构**：内置 `PRD_dual-pane.html` 模板，左侧实时渲染 Markdown 需求（支持 **Mermaid** + **PlantUML**），右侧内嵌可交互 UI 组件沙盒。
- 🔄 **完全幂等**：重复运行不会覆盖任何已有文件，安全追加而非覆盖。

---

## 📂 Skill 源码目录结构

```text
bootstrap-agentic-project/
├── SKILL.md                          # 核心技能定义，供 Claude Code 读取并触发
├── README.md                         # 本文档
├── scripts/
│   └── init.sh                       # 核心的自动化初始化脚本（幂等）
├── .agents/                          # Codex 技能自发现目录
│   └── skills/
│       └── bootstrap-agentic-project/
│           ├── SKILL.md              # Codex 技能入口
│           ├── scripts/init.sh       # 自包含初始化脚本
│           ├── assets/               # 初始化所需模板资源
│           ├── references/           # 技能参考文档
│           └── agents/openai.yaml    # Codex 界面元数据
├── assets/
│   ├── root-templates/
│   │   └── AGENTS.md                 # 全局路由（Claude Code + Codex 通用）
│   ├── docs-templates/
│   │   ├── INDEX.md                  # 知识库索引
│   │   ├── log.md                    # 知识库操作日志模板
│   │   ├── wiki/overview.md          # Wiki 知识总览模板
│   │   └── prd-demo/                 # 双视窗 PRD 示例（含 preview.sh）
│   ├── engine-templates/             # Claude Code 引擎模板
│   │   ├── agents/                   # 六大子 Agent 角色定义（.md）
│   │   │   ├── pm-agent.md           # 产品经理（双路径需求工作流）
│   │   │   ├── project-manager-agent.md  # 项目经理（排期/进度/阻塞）
│   │   │   ├── dev-agent.md
│   │   │   ├── architect-agent.md
│   │   │   ├── ui-agent.md
│   │   │   └── qa-agent.md
│   │   ├── commands/                 # 快捷指令
│   │   │   ├── prd.md                # /prd 需求工作流
│   │   │   └── progress.md           # /progress 项目管理
│   │   ├── skills/                   # 技能定义（Skill 系统路由）
│   │   │   ├── prd/SKILL.md          # /prd 技能化
│   │   │   ├── progress/SKILL.md     # /progress 技能化
│   │   │   ├── reflect/SKILL.md      # /reflect 知识整理 + lint 健康检查
│   │   │   ├── ingest/SKILL.md       # /ingest 外部知识源导入
│   │   │   ├── wiki/SKILL.md         # /wiki 知识库查询与页面生成
│   │   │   └── C-startup/SKILL.md    # /C-startup 项目初始化
│   │   ├── templates/                # PM 工作流模板
│   │   │   ├── confirmation_sheet.md # 需求确认单
│   │   │   ├── MRD_template.md       # MRD 模板
│   │   │   ├── product_brief_template.md  # 产品简报模板
│   │   │   └── process_template.md   # 进度看板模板（含甘特图）
│   │   ├── contexts/                 # dev.md, review.md 动态 Prompt
│   │   ├── rules/common/coding-style.md
│   │   ├── scripts/
│   │   │   ├── hooks/                # check-console-log.js, check-hardcoded-styles.js, session-start.js, session-stop.js
│   │   │   └── markitdown/           # batch_convert.py, convert_literature.py（批量转换脚本）
│   │   ├── design/tokens/base.json   # Design Token 基准
│   │   ├── settings.json             # Hook 配置模板（含 {{ENGINE_DIR}} 占位符）
│   │   └── mcp-servers.json          # 项目级 MCP 注册表（含 markitdown MCP）
│   └── codex-templates/              # Codex CLI 引擎模板
│       ├── config.toml               # 运行时配置（沙箱/MCP/多 Agent/AGENTS fallback）
│       ├── AGENTS.md                 # Codex 专属补充说明
│       └── agents/                   # 六大子 Agent 角色定义（.toml）
│           ├── pm.toml
│           ├── project-manager.toml
│           ├── dev.toml
│           ├── architect.toml
│           ├── ui.toml
│           └── qa.toml
└── references/
    └── agentic-engineering-guide.md
```

---

## 🏗️ 注入目标项目的结构

```text
your-target-project/
├── AGENTS.md                    # 全局路由、角色分工（唯一事实来源）
├── CLAUDE.md -> AGENTS.md       # 软链接，与 AGENTS.md 始终同步
├── .gitignore                   # 自动保护 mcp-servers.json 和 .codex/config.toml
│
├── .sources/                    # 📥 外部知识源（LLM 只读）
│   ├── articles/                # 研究文章、博客
│   ├── docs/                    # 技术文档、PDF
│   ├── data/                    # 表格、数据文件
│   └── .converted/              # Markitdown 转换缓存（自动生成，已 gitignore）
│
├── .claude/                     # Claude Code 引擎底座
│   ├── settings.json            # Hook 配置（SessionStart + PreToolUse + Stop）
│   ├── mcp-servers.json         # 项目专属 MCP（含 markitdown，已加入 .gitignore）
│   ├── agents/                  # pm, project-manager, dev, architect, ui, qa（.md）
│   ├── commands/                # /prd, /progress 快捷指令
│   ├── skills/                  # /prd, /progress, /reflect, /ingest, /wiki, /C-startup
│   ├── templates/               # 需求确认单、MRD、产品简报、进度看板模板
│   ├── contexts/                # dev.md, review.md 动态 Prompt
│   ├── rules/common/            # coding-style.md 全局规范
│   └── scripts/
│       ├── hooks/               # check-console-log.js, check-hardcoded-styles.js, session-start.js, session-stop.js
│       └── markitdown/          # batch_convert.py, convert_literature.py
│
├── .codex/                      # Codex CLI 引擎底座
│   ├── config.toml              # 运行时配置（已加入 .gitignore）
│   ├── AGENTS.md                # Codex 专属补充说明
│   └── agents/                  # pm.toml, dev.toml ... 五大角色（.toml 格式）
│
├── .agents/                     # Codex 技能自动发现目录
│   └── skills/                  # 已安装的技能包（SKILL.md + openai.yaml）
│
├── docs/
│   ├── context/
│   │   ├── INDEX.md             # 🧠 知识库索引（5 分类表格）
│   │   ├── log.md               # 📝 知识库操作日志（ingest/query/lint 记录）
│   │   ├── wiki/                # 📖 LLM 生成的 Wiki 页面
│   │   │   ├── entities/        # 实体页（API、服务、库、组件）
│   │   │   ├── concepts/        # 概念页（模式、原则、技术）
│   │   │   ├── comparisons/     # 比较页（A vs B）
│   │   │   ├── syntheses/       # 综合页（query 回归存档）
│   │   │   └── overview.md      # 项目知识总览
│   │   ├── business/            # 📂 业务提需上下文（确认单、MRD）
│   │   ├── product-initiated/   # 🎯 产品自发上下文（产品简报）
│   │   └── project/experience/  # 🔧 团队经验沉淀
│   ├── prd/{feature_id}/        # 🧑‍💼 PRD.md, process.md, notes.md
│   └── design/tokens/base.json  # 🎨 Design Token 基准
│
├── tests/specs/                 # 🕵️ 验收测试
└── src/                         # 👨‍💻 业务代码
```

---

## 🛠️ 如何使用本 Skill

### 1. 触发初始化

**Claude Code：**
```
使用 Bootstrap Agentic Project 技能初始化当前项目
```

**Codex CLI：**
```text
在提示中显式提及 `bootstrap-agentic-project`，或通过 `/skills` / `$bootstrap-agentic-project` 调用此技能
```

脚本会自动检测已有的目录结构（幂等，安全重复运行）。

### 2. 填写 MCP 配置（重要）

**Claude Code：**
```bash
vim .claude/mcp-servers.json   # 填入 TAPD/GitHub API Key
```

**Codex CLI：**
```bash
vim .codex/config.toml         # 取消注释并填入 MCP API Key
```

> 两个文件已自动加入 `.gitignore`，不会意外提交 API Key。

### 3. 体验双视窗 PRD 演示

- macOS：在 Finder 中双击 `docs/prd/.demo-feature/预览PRD-macOS.command`
- Windows：双击 `docs/prd/.demo-feature/预览PRD-Windows.bat`

### 4. 开始你的第一个需求

**Claude Code：**
```
/prd 我想做一个用户登录页面
```

**业务提需路径（含确认单 → MRD → PRD）：**
```
/prd business 运营团队提了一个新的会员积分系统需求
```

**Codex CLI：**
```
请使用 `pm` 自定义 subagent，先帮我澄清需求并产出用户登录页面的 PRD
```

pm-agent 会自动识别需求路径，通过对话式追问细化需求，按路径流转生成对应文档。

### 5. 管理项目排期

**Claude Code：**
```
/progress init 1.0.0-用户登录-202604
/progress view 1.0.0-用户登录-202604
/progress update 1.0.0-用户登录-202604
/progress block 1.0.0-用户登录-202604 "法务确认优惠券叠加规则"
```

**Codex CLI：**
```
请使用 `project-manager` 自定义 subagent，为 `1.0.0-用户登录-202604` 初始化项目排期
```

### 6. 知识管理（三层知识库）

**导入外部知识源：**
```
/ingest path/to/file.pdf           # 单文件导入（PDF/DOCX/PPTX/HTML/XLSX 等）
/ingest https://example.com/doc    # URL 导入
/ingest .sources/docs/ --batch     # 批量导入目录
```

**查询与生成 Wiki 页面：**
```
/wiki query React Server Components  # 综合查询，带引用的回答
/wiki generate entity Next.js        # 生成实体 Wiki 页面
/wiki generate concept TDD           # 生成概念 Wiki 页面
/wiki generate comparison A B        # 生成 A vs B 比较页
/wiki generate overview              # 重新生成知识总览
/wiki status                         # 知识库统计信息
```

**知识整理与健康检查：**
```
/reflect              # 扫描未索引的经验记录，追加到 INDEX.md
/reflect promote      # 扫描 + 提升功能级笔记到团队经验库
/reflect lint         # 知识库健康检查（孤立页面、断链、陈旧检测、覆盖度）
/reflect status       # 查看知识库覆盖率统计
```

> session-stop hook 会在检测到 notes.md 有更新时自动建议运行 `/reflect`。
> 需要安装 Markitdown：`pip install markitdown` 或使用 MCP server `uvx markitdown-mcp`。

---

## 👥 五大 Agent 角色

| Agent | 核心职责 | Claude Code 调用 | Codex CLI 调用 |
|-------|---------|----------------|--------------|
| `pm-agent` | 需求澄清、双路径工作流、PRD 撰写 | `/prd [需求]` | 明确要求使用 `pm` 自定义 subagent |
| `project-manager-agent` | 项目排期、进度追踪、阻塞管理 | `/progress [指令]` | 明确要求使用 `project-manager` 自定义 subagent |
| `dev-agent` | 业务代码实现、TDD 开发循环 | 直接 @提及 | 明确要求使用 `dev` 自定义 subagent |
| `architect-agent` | Code Review（不写 CRUD）、架构设计 | 直接 @提及 | 明确要求使用 `architect` 自定义 subagent |
| `ui-agent` | Design Token 解析、界面还原度把控 | 直接 @提及 | 明确要求使用 `ui` 自定义 subagent |
| `qa-agent` | BDD 测试用例、质量评估 | 直接 @提及 | 明确要求使用 `qa` 自定义 subagent |

---

## ⚙️ Hook 说明（Claude Code 专属）

本 Skill 注册了三条 **有效的** Claude Code Hooks（格式符合官方 `settings.json` 规范）：

### `session-start.js`（SessionStart）
- 会话开始时扫描 `docs/prd/` 下所有 `.artifacts/process.md` 文件，并兼容读取遗留 `process.txt`
- 对 `process.md` 文件，提取 YAML frontmatter 中的 `stage` 字段生成结构化状态摘要
- 通过 stdout JSON `{ hookSpecificOutput: { hookEventName, additionalContext } }` 注入上下文
- **V3 上下文预算管理**：`MAX_CONTEXT_CHARS = 8000`（≈2000 tokens），按文件修改时间优先级注入；超出预算的文件仅注入摘要行，并提示 Claude 需要时使用 Read 工具查看完整内容

### `check-console-log.js`（PreToolUse / Edit）
- 在每次 Edit 工具写入 `.ts/.tsx/.js/.jsx` 时触发
- 从 stdin 读取 JSON 工具输入，提取 `tool_input.new_string` 检测 `console.log`
- 发现后 exit 2，**阻止** 本次编辑并向 Claude 注入错误说明

### `check-hardcoded-styles.js`（PreToolUse / Edit）— V3 新增
- 在每次 Edit 工具写入 `.css/.scss/.less/.vue/.svelte/.tsx/.jsx` 时触发
- 检测硬编码颜色值：hex 颜色（`#xxx`/`#xxxxxx`）、`rgb()`/`rgba()`/`hsl()`/`hsla()` 函数
- 智能跳过：Token 定义文件（`design/tokens/`）、测试文件、注释行、`var(--xxx)` 引用、含 `token` 关键词的行
- 逃生出口：在该行添加 `// token-ok` 或 `/* token-ok */` 注释可跳过检查
- 发现违规后 exit 2，**阻止** 编辑并给出修复建议（查阅 base.json、使用 CSS 变量）

### `session-stop.js`（Stop）
- 在 Claude 即将结束回复时触发
- 首次触发（`stop_hook_active: false`）：exit 2，注入进度存档提醒（同时检测 `.artifacts/process.md` 与遗留 `process.txt`），迫使 Claude 先保存再结束
- **V3 /reflect 建议**：如检测到 `notes.md` 在近 2 小时内被修改，额外提示运行 `/reflect` 将新知识同步到团队知识库索引
- 二次触发（`stop_hook_active: true`）：exit 0，避免无限循环

---

## 🔷 Codex CLI 多 Agent 支持

`.codex/config.toml` 中预配置了 6 个角色，通过 Codex 内置的多 Agent 框架运行：

```toml
[features]
multi_agent = true

[agents.pm]
description = "产品经理：需求澄清、PRD 撰写"
config_file = "agents/pm.toml"
# ... 其余 4 个角色
```

启动子 Agent 时，优先使用自然语言明确说明角色和目标：
```
请使用 `pm` 自定义 subagent，帮我写一个用户登录功能的 PRD
```

---

## 🔌 ECC (Everything-Claude-Code) 兼容性

当项目中存在 `.claude-plugin/plugin.json`（ECC 安装标志）时，`init.sh` 会自动检测并：
1. 保持使用 `.claude/` 作为引擎目录（与 ECC 共存）
2. 发出警告，提示检查 Hook 重复（ECC 的 `hooks/hooks.json` 和 bootstrap 的 `settings.json` 均会激活）
3. 建议合并重复的 Hook 逻辑

---

## 💡 最佳实践

- **Git 协同**：将 `.claude/`（不含 `mcp-servers.json`）和 `.codex/`（不含 `config.toml`）提交到 Git，团队成员拉取后即获得一致的 Agent 团队配置。
- **保护密钥**：`mcp-servers.json` 和 `.codex/config.toml` 含 API Key，**不要提交**。init.sh 已自动将其加入 `.gitignore`。
- **幂等性**：重复运行 init.sh 是安全的。所有文件操作均为"不存在则创建，存在则跳过"。
- **AGENTS.md / CLAUDE.md 同步**：`CLAUDE.md` 是 `AGENTS.md` 的软链接，编辑任意一方（手动、AI 工具、任何编辑器）都天然同步，无需额外维护。
- **contexts/ 激活方式**：在任务开始时告知 Agent "请进入开发模式" 或 "请进入 Review 模式"，Agent 会主动读取对应的 Context 文件。

Enjoy your Vibe Coding! 🎉
