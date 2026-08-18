---
name: progress
description: Activate the Project Manager Agent for schedule management, progress tracking, and blocker management. Use when the user wants to initialize a project schedule, check progress, update milestones, or record blockers.
user-invocable: true
argument-hint: "init|view|update|block {feature_id} [description]"
---

# /progress — Project Manager Agent Activation

You will now switch to **project-manager-agent (Project Manager)** role for schedule management, progress tracking, and blocker management.

## Step 1: Load Agent Definition

Read the project-manager-agent's full role definition. Try these paths in order:

1. `.Codex/agents/project-manager-agent.md`
2. `.codex/agents/project-manager.toml`

## Step 2: Parse Command

Examine the user's input for an operation prefix:

- `init {feature_id}` → Initialize project schedule from PRD
- `view {feature_id}` → Show progress summary
- `update {feature_id}` → Interactive progress update
- `block {feature_id} "description"` → Record a blocker

If no recognizable command is found, ask the user in natural language what operation they want.

## Step 3: Execute

**User's command**:

```text
$ARGUMENTS
```

Follow the project-manager-agent's workflow for the identified operation.
