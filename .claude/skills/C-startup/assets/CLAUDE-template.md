# CLAUDE.md

## 项目概述

| 字段 | 内容 |
|------|------|
| 项目名称 | {{project_name}} |
| 版本号 | {{version}} |
| 需求 ID | {{feature_id}} |
| 需求方 | {{requester}} |
| 产品负责人 | {{owner}} |
| 创建日期 | {{date_display}} |

## C 端产品交付流程

本项目支持两条需求路径，AI 通过关键词和文件状态自动识别并推进。

**行为准则**：AI 默默感知进度，不主动提示流程建议。具体而言：
- 每次会话开始时，静默读取 `process.md` 和相关文件判断当前阶段，**不输出"检测到XX阶段"之类的提示**
- 仅在以下情况提示下一步：
  1. 用户显式询问进度或下一步
  2. 当前步骤的产出物刚刚在本次会话中完成（自然衔接）
- 如果用户的请求与某个步骤明确相关，直接执行，不需要先声明"我们现在处于 Step X"

---

### 路径识别规则

- 用户提到"业务方""运营""客户反馈""老板说""会议纪要""确认单" → **路径 A（业务急需）**
- 用户提到"我想做""用户调研""竞品""假设验证""MVP" → **路径 B（产品自发）**
- 用户显式说"业务提需" → 路径 A；"产品自发" → 路径 B
- 不确定时，直接询问用户

---

### 路径 A：业务急需

适用场景：业务方提出需求，需走确认 → MRD → PRD 正式流程。

#### A-Step 1: 需求沟通 → 会议文字稿

- **产出物**: `docs/context/` 下的会议记录文件（TXT/MD）
- **完成标志**: `docs/context/` 下存在非 `README.md` 的文件
- **AI 职责**: 用户提供会议文字稿后，协助整理要点，为 Step 2 做准备

#### A-Step 2: 会议文字稿 → 需求确认单

- **输入**: `docs/context/` 下的会议记录 + `begin.md`
- **产出物**: `docs/prd/{{feature_id}}/需求确认/需求确认单.md`（填充完整）
- **完成标志**: 需求确认单项目信息表已填写，各章节（需求背景、范围、策略）内容非空
- **AI 职责**: 阅读全部会议材料和 begin.md，输出结构化的需求确认单，区分「已确认」和「待确认」

#### A-Step 3: 需求确认 → 终版需求明细 (MRD)

- **输入**: 填好的需求确认单 + 确认沟通记录（如有）
- **产出物**: `docs/prd/{{feature_id}}/需求确认/终版需求明细.md`
- **完成标志**: YAML frontmatter 中 `status` 为 `confirmed`，功能范围表格已填写
- **AI 职责**: 将确认单中所有待确认项更新为已确认，填充终版需求明细各章节，更新 status

#### A-Step 4: 终版需求明细 → PRD + 可交互 Demo

- **输入**: 终版需求明细 + `docs/design/design-spec.md`（如已加载设计规范）
- **产出物**: `docs/prd/{{feature_id}}/PRD.md` + 原型文件（如适用）
- **完成标志**: PRD.md 中功能清单和验收标准已填写，`<!-- optional -->` 标记的模块已决定保留或删除
- **AI 职责**: 基于需求明细撰写 PRD（参照 `.claude/templates/PRD-writing-guide.md`），按需保留/删除 `<!-- optional -->` 标记的模块（流程图、时序图、功能交互）。如 `docs/design/design-spec.md` 已配置设计规范，撰写 PRD 时引用相关 Token

#### A-Step 5: 评审确认

- **输入**: PRD + 评审反馈
- **产出物**: 更新后的 PRD
- **完成标志**: `docs/context/` 下存在文件名含"评审"或"review"的文件
- **AI 职责**: 根据评审反馈更新 PRD，将重要决策记录到 `.artifacts/notes.md`

#### A-Step 6: 推送 Git 远端

- **完成标志**: `git remote -v` 有输出且存在推送记录
- **AI 职责**: 检查目录完整性，协助配置 remote 并推送

---

### 路径 B：产品自发

适用场景：产品经理基于用户洞察自主发起，快速验证假设。

#### B-Step 1: 用户洞察

- **产出物**: `docs/context/` 下的用户反馈/调研/数据文件
- **完成标志**: `docs/context/` 下存在非 `README.md` 的文件
- **AI 职责**: 引导用户整理已有素材（访谈记录、数据、反馈），提炼关键发现

#### B-Step 2: 用户画像 + 竞品 + MVP 假设（至少完成 1 项）

- **产出物**（可选组合，至少完成 1 项）:
  - `docs/prd/{{feature_id}}/用户画像.md`
  - `docs/prd/{{feature_id}}/竞品分析.md`
  - `docs/prd/{{feature_id}}/MVP假设.md`
- **完成标志**: 上述三个文件中，至少有 1 个内容非模板初始状态
- **AI 职责**: 根据用户洞察素材，辅助生成画像/竞品/假设。用户可选择跳过某些文档，最少提供 1 项即可推进

#### B-Step 3: PRD

- **输入**: Step 2 产出的文档 + 用户洞察素材 + `docs/design/design-spec.md`（如已加载）
- **产出物**: `docs/prd/{{feature_id}}/PRD.md`
- **完成标志**: PRD.md 中功能清单和验收标准已填写，`<!-- optional -->` 标记的模块已决定保留或删除
- **AI 职责**: 基于已有素材撰写 PRD（参照 `.claude/templates/PRD-writing-guide.md`），按需保留/删除 `<!-- optional -->` 标记的模块。如设计规范已加载则引用

#### B-Step 4: 评审 + 归档

- **完成标志**: `.artifacts/process.md` 的 stage 更新为 `review`
- **AI 职责**: 生成评审摘要，更新 process.md，将关键决策写入 notes.md

#### B-Step 5: 推送 Git 远端

- 同路径 A 的 Step 6

---

### 进度追踪

每完成一个 Step，AI 应更新 `docs/prd/{{feature_id}}/.artifacts/process.md` 的 YAML frontmatter:
- `stage` 字段更新为当前阶段
- `last_updated` 更新为当前日期

跨会话恢复时，AI 读取 process.md 的 stage 字段，自动接续对应步骤。

## 文档规范

- **PRD 编写必须遵循 `.claude/templates/PRD-writing-guide.md` 规范**（功能点 6 要素、异常覆盖、数据量化）
- 文件使用中文命名（与 bootstrap-agentic-project 的 PRD 目录规范兼容）
- Markdown 格式
- 会议记录/用户素材放 `docs/context/`，建议按日期命名
- 不使用 emoji（文件内容中）
- 业务流程图用 Mermaid，系统交互用 PlantUML

## 升级路径

本项目目录与 `bootstrap-agentic-project` 完全兼容。当项目进入研发阶段，可运行其 `init.sh` 一键升级：
- 注入多 Agent 配置（PM/Dev/QA/Architect/UI）
- 注入 hooks、rules、contexts
- 注入 Design Token 体系
- 注入知识库 Wiki 层（/ingest、/wiki、/reflect）
- 已有的 `docs/`、`.sources/`、`CLAUDE.md` 均保持不变
