---
name: c-startup
description: C 端产品项目初始化 -- 创建兼容 bootstrap-agentic-project 的标准目录、双路径（业务急需/产品自发）文档模板和带状态感知的 AGENTS.md。当用户说"新建C端项目""初始化C端项目""c-startup""创建产品项目"或类似意图时触发。即使用户只是提到要开始一个新的 C 端产品项目，也应该触发此 skill。
---

# C-Startup: C 端产品项目初始化

创建标准化的 C 端产品项目文件夹，包含目录结构、双路径文档模板（业务急需 / 产品自发）、AI 流程指引。目录结构与 `bootstrap-agentic-project` 完全兼容，可随时升级为完整 Agentic 研发底座。

## 执行流程

### 0. 环境检查

在做任何事之前，检查当前工作目录的状态：

1. 执行 `git rev-parse --is-inside-work-tree` 判断是否在 git 仓库内
2. 检查当前目录是否存在 AGENTS.md
3. 列出当前目录的子文件夹

**处理规则：**

- 如果当前目录在 git 仓库内或存在 AGENTS.md，通过 AskUserQuestion 提示用户：
  > 当前目录已是一个项目（检测到 git 仓库 / AGENTS.md）。建议在独立空目录下创建新项目。
  >
  > 请提供新项目的目标路径（如 ~/Projects/新项目名），或输入"继续"在当前目录下创建子目录。
- 如果当前目录是干净的：直接进入 Step 1。

### 1. 收集项目信息

通过 AskUserQuestion 一次性收集以下信息：

- **项目名称**（必填）— 用于文件夹命名，允许中文
- **版本号**（默认 1.0.0）— 用于文档版本追踪
- **需求 ID**（必填）— 如 `FEAT-001`、`趣味单词MVP`，用于 `docs/prd/{feature_id}/` 目录
- **需求方**（路径 A 必填，路径 B 可留空）— 发起需求的业务方，如"初中拉新业务线"
- **产品负责人**（必填）

如果必填字段为空，重新询问该字段。

### 2. 执行初始化脚本

使用收集到的信息，执行 init.sh：

```bash
bash <skill_path>/scripts/init.sh "<项目名称>" "<需求ID>" "<产品负责人>" "<版本号>" "<需求方>"
```

脚本会：
1. 创建兼容 bootstrap-agentic-project 的目录骨架
2. 生成带变量替换的模板文件（AGENTS.md、PRD、确认单、画像等）
3. 初始化 git 仓库并提交
4. 输出目录结构和下一步提示

### 3. 路径引导

脚本完成后，向用户说明两条路径的使用方式：

**路径 A — 业务急需**（业务方提出需求，走确认流程）：
1. 将会议纪要/需求文字稿放入 `docs/context/`
2. 在项目目录中开启新的 AI 会话
3. AI 会自动识别路径 A，引导走完：确认单 → MRD → PRD → 评审

**路径 B — 产品自发**（产品经理基于洞察自主发起）：
1. 直接在项目目录中开启新的 AI 会话
2. AI 引导从用户洞察开始，完成：画像/竞品/假设（≥1） → PRD → 评审

**升级提示**：
> 当项目进入研发阶段，可运行 `bootstrap-agentic-project` 的 `init.sh` 一键升级为完整 Agentic 底座（多 Agent、hooks、知识库、Design Token）。已有文件不会被覆盖。

### 产出目录结构

```
{项目名称}/
├── AGENTS.md                              ← 双路径 AI 行为指南
├── begin.md                               ← 项目概览
├── .gitignore
├── .sources/                              ← 原始文档区（预留给 /ingest 升级）
│   └── .converted/
└── docs/
    ├── context/
    │   ├── README.md                      ← 素材存放指引
    │   ├── INDEX.md                       ← 知识库索引（兼容 /reflect）
    │   └── project/experience/            ← 经验沉淀目录
    ├── design/
    │   └── design-spec.md                 ← 设计规范入口（链接 Design Token）
    └── prd/{feature_id}/
        ├── PRD.md                         ← PRD 模板
        ├── 预览PRD-macOS.command           ← 双击本地预览 PRD 双视窗
        ├── 预览PRD-Windows.bat             ← Windows 本地预览
        ├── 需求确认/                       ← 路径 A
        │   ├── 需求确认单.md
        │   └── 终版需求明细.md
        ├── 用户画像.md                     ← 路径 B（可选）
        ├── 竞品分析.md                     ← 路径 B（可选）
        ├── MVP假设.md                      ← 路径 B（可选）
        └── .artifacts/
            ├── PRD_dual-pane.html         ← PRD 双视窗渲染器
            ├── process.md                 ← 进度追踪（兼容 agent 防失忆）
            └── notes.md                   ← 决策记录
```

### PRD 双视窗预览

初始化后自动生成 PRD 双视窗 HTML 预览工具：

**本地预览**（双击即用）：
- macOS: `docs/prd/{feature_id}/预览PRD-macOS.command`
- Windows: `docs/prd/{feature_id}/预览PRD-Windows.bat`
- 自动启动 HTTP 服务器 → 浏览器打开双视窗（左：PRD Markdown 渲染，右：交互沙盒）
- 支持 Mermaid 流程图 + PlantUML 图表实时渲染

**GitLab Pages 在线浏览**：
- 自动生成 `.gitlab-ci.yml`，push 到 GitLab 默认分支后自动部署
- 访问地址：`https://<namespace>.gitlab.io/<project>/docs/prd/{feature_id}/.artifacts/PRD_dual-pane.html`
- 无需本地环境，团队成员直接在浏览器浏览 PRD
- `fetch('../PRD.md')` 在 GitLab Pages HTTP 环境下可正常工作（同源请求）

### 与 bootstrap-agentic-project 的兼容性

以下目录和文件格式严格对齐 bootstrap-agentic-project 规范：

| 兼容点 | c-startup | bootstrap-agentic-project |
|--------|-----------|---------------------------|
| PRD 目录 | `docs/prd/{feature_id}/` | 同 |
| 知识索引 | `docs/context/INDEX.md`（5 分类表格） | 同格式，/reflect 直接可用 |
| 经验目录 | `docs/context/project/experience/` | 同 |
| 原始文档 | `.sources/` + `.sources/.converted/` | /ingest 直接可用 |
| 进度追踪 | `.artifacts/process.md` YAML frontmatter | agent 防失忆直接衔接 |
| AGENTS.md | 无 bridge note 标记 | init.sh grep guard 可追加 |
