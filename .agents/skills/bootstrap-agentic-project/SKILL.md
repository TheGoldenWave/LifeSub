---
name: bootstrap-agentic-project
description: Use when bootstrapping a repository with shared docs-as-code scaffolding, Codex hooks, and Codex project configuration for multi-agent workflows.
---

# Bootstrap Agentic Project Skill

**Description**:
This skill initializes a Next-Gen Agentic Engineering project structure. It creates the necessary directory scaffolding, injects core guardrail files (AGENTS.md, INDEX.md), and sets up the foundational hooks and dynamic contexts inspired by the `everything-Codex` project.

---

## 🛠️ Execution Instructions (For Agent)

When the user invokes this skill, you MUST execute the initialization script provided in this package.

### Step 1: Execute the Bootstrap Script
Find and execute the `init.sh` script associated with this skill. Since this skill might be installed globally or locally for either Codex or Codex, search upward from the current working directory first, then fall back to user-level skill directories:

```bash
SKILL_PATH=""
SEARCH_DIR="$PWD"

while [ "$SEARCH_DIR" != "/" ] && [ -z "$SKILL_PATH" ]; do
  for root in \
    "$SEARCH_DIR/.agents/skills" \
    "$SEARCH_DIR/.Codex/skills" \
    "$SEARCH_DIR/.Codex-plugin"
  do
    [ -d "$root" ] || continue
    SKILL_PATH="$(find "$root" -path "*/bootstrap-agentic-project/scripts/init.sh" -print -quit 2>/dev/null)"
    [ -n "$SKILL_PATH" ] && break
  done

  SEARCH_DIR="$(dirname "$SEARCH_DIR")"
done

if [ -z "$SKILL_PATH" ]; then
  for root in \
    "$HOME/.agents/skills" \
    "$HOME/.Codex/skills" \
    "$HOME/.Codex-plugin"
  do
    [ -d "$root" ] || continue
    SKILL_PATH="$(find "$root" -path "*/bootstrap-agentic-project/scripts/init.sh" -print -quit 2>/dev/null)"
    [ -n "$SKILL_PATH" ] && break
  done
fi

if [ -n "$SKILL_PATH" ]; then
  bash "$SKILL_PATH"
else
  echo "Error: Could not find bootstrap-agentic-project/scripts/init.sh"
fi
```

### Step 2: Verification
Verify that the following core components have been successfully created in the user's current working directory:
- `AGENTS.md`
- `AGENTS.md` symlink or an existing `AGENTS.md` updated with the AGENTS bridge section
- `.Codex/settings.json`
- `.codex/config.toml`
- `.Codex/contexts/dev.md`
- `docs/context/INDEX.md`

### Step 3: Optional Cleanup
Do not auto-delete the skill package. If the project temporarily vendored a local copy of the skill only for bootstrap, let the user decide whether to remove it after initialization.

### Step 4: Final Output
Report to the user:
> "✅ Agentic Engineering 标准底座已初始化完毕！
> 包含 Codex Hooks、动态 Contexts、Codex 多 Agent 配置和底层 Rules 已就绪。
> 对于 Codex，请明确要求使用 `pm` 自定义 subagent 开始需求澄清。"
