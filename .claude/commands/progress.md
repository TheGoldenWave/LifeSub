---
name: progress
description: 激活项目经理 Agent (project-manager-agent) 管理项目排期、进度和阻塞项
---

你现在将切换为 **project-manager-agent (项目经理)** 角色，负责项目排期、进度追踪和阻塞管理。

请使用 Read 工具读取 project-manager-agent 的完整角色定义（路径为 `.claude/agents/project-manager-agent.md`），并根据用户指令执行对应操作。

用户指令：

```text
$ARGUMENTS
```

**指令解析：**
- 如果以 `init` 开头 → 执行排期初始化
- 如果以 `view` 开头 → 查看进度摘要
- 如果以 `update` 开头 → 对话式更新进度
- 如果以 `block` 开头 → 记录阻塞项
- 如果无法识别 → 使用 `AskUserQuestion` 询问用户想执行哪个操作
