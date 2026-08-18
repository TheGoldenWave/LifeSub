# Codex CLI 参考说明 — Agentic Engineering 项目

> 本文件位于 `.codex/AGENTS.md`，用于和 `.codex/config.toml` 放在一起方便维护。
> Codex 自动加载的项目指令文件仍然是项目根 `AGENTS.md`；本文件默认不会被额外自动注入。

---

## Codex 与 Claude Code 差异对照

| 特性 | Claude Code | Codex CLI |
|------|------------|-----------|
| 上下文文件 | `CLAUDE.md` + `AGENTS.md` | 根 `AGENTS.md`（自动加载） + `.codex/AGENTS.md`（人工参考） |
| Hooks（自动拦截） | 支持 8+ 事件类型 | 支持，但本模板默认未启用；启用时以 `.codex/hooks.json` 为入口，主要作用于 Bash 级事件 |
| 技能加载 | `.claude/skills/` 或插件 | `.agents/skills/`（自动发现） |
| 技能调用 | 自然语言或插件约定 | 显式在提示中提及技能，或用 `/skills` / `$skill-name` |
| 子 Agent | `Task` 工具 | 通过自然语言明确要求使用某个自定义 subagent；`/agent` 主要用于查看和切换已有 agent 会话 |
| MCP 集成 | `mcp-servers.json` | `.codex/config.toml [mcp_servers.*]` |

---

## 可用 Agent 角色（多 Agent 模式）

在 Codex 中，优先通过自然语言明确说明角色来启动子 Agent：

| 角色名 | 职责 | 调用方式 |
|--------|------|---------|
| `pm` | 需求澄清、PRD 维护 | “请使用 `pm` 自定义 subagent 先澄清需求并产出 PRD” |
| `dev` | 业务代码实现、TDD | “请使用 `dev` 自定义 subagent 实现当前功能” |
| `architect` | Code Review、架构设计 | “请使用 `architect` 自定义 subagent 做审查或架构建议” |
| `ui` | 界面实现、Design Token | “请使用 `ui` 自定义 subagent 处理视觉和设计规范” |
| `qa` | 测试用例、质量评估 | “请使用 `qa` 自定义 subagent 编写或执行验收测试” |

详细角色配置：`.codex/agents/*.toml`

---

## 技能说明

技能位于 `.agents/skills/`，Codex 会自动发现并加载。

默认 bootstrap 只保证安装 `bootstrap-agentic-project` 这个初始化技能；其他项目级技能是否存在，取决于仓库是否额外提供。

---

## 安全护栏（默认配置）

当前模板默认不启用 Codex hooks，安全护栏主要通过**指令约束**实现：

1. **禁止遗留调试代码**：提交前检查 `.ts/.tsx/.js/.jsx` 文件中是否含 `console.log`，如有则删除
2. **进度强制存档**：每次完成关键步骤或结束会话前，将进度更新到 `docs/prd/{feature_id}/.artifacts/process.md`
3. **禁止硬编码样式**：颜色、间距、字体大小等必须引用 `docs/design/tokens/base.json`
4. **不暴露密钥**：MCP API Key 存放于 `.codex/config.toml`，必须已加入 `.gitignore`

---

## 会话恢复

当前模板没有为 Codex 打开自动 SessionStart 恢复，因此请在每次会话开始时手动执行：

```
请搜索 docs/prd/ 目录下所有 `.artifacts/process.md` 文件并读取；若发现遗留 `process.txt`，一并兼容读取，用于恢复上次会话上下文。
```

`.codex/config.toml` 已通过 `developer_instructions` 追加同样的提醒，但这不替代手动核对上下文文件。

---

## 模型建议

- 优先使用当前 Codex 环境中的默认编码模型。
- 需要更快或更低成本时，切换到当前可用的 `mini` 变体。
- 子 Agent 的具体模型选择以当前官方可用模型和团队成本约束为准。
