# LifeSub 文档结构整理设计

## 目标

将当前平铺在 `docs/` 根目录的产品与技术资料，整理为项目协作规范要求的三层结构：产品自发需求上下文、正式 PRD、技术实现上下文。整理过程中保留有效判断，消除重复维护点，并移除与 LifeSub 无关的初始化演示需求。

## 当前问题

- 产品定义、路线图、架构、隐私与插件资料全部平铺在 `docs/` 根目录，无法通过项目路由快速定位。
- 项目没有 LifeSub 正式 PRD，`docs/prd/` 下只有用户登录演示需求。
- `README.md` 直接链接平铺文档，迁移后需要统一更新。
- 产品定义与路线图在目标、范围和阶段描述上存在重复，继续分别维护容易漂移。
- 正式需求没有对应的 `process.md` 与 `notes.md`，无法跨会话恢复进度。

## 目标结构

```text
docs/
├── context/
│   ├── product-initiated/lifesub-mvp-202608/
│   │   ├── product-brief.md
│   │   └── research.md
│   ├── technical/lifesub-mvp-202608/
│   │   ├── architecture.md
│   │   ├── decisions.md
│   │   ├── integrations.md
│   │   └── privacy-and-sync.md
│   └── INDEX.md
├── prd/0.1.0-lifesub-mvp-202608/
│   ├── PRD.md
│   ├── 预览PRD-macOS.command
│   ├── 预览PRD-Windows.bat
│   └── .artifacts/
│       ├── PRD_dual-pane.html
│       ├── process.md
│       └── notes.md
└── design/
    └── tokens/
```

## 内容迁移映射

| 现有文档 | 目标文档 | 处理方式 |
|---|---|---|
| `docs/product-brief.md` | `docs/context/product-initiated/lifesub-mvp-202608/product-brief.md` | 保留产品愿景、用户价值、首版方向和生态定位；减少与 PRD 重复的功能清单 |
| `docs/research.md` | `docs/context/product-initiated/lifesub-mvp-202608/research.md` | 原样迁移并补充与产品简报的关联链接 |
| `docs/roadmap.md` | `PRD.md` 与 `.artifacts/process.md` | 产品阶段边界进入 PRD；当前阶段与后续工作进入进度文件；删除独立路线图 |
| `docs/architecture.md` | `docs/context/technical/lifesub-mvp-202608/architecture.md` | 保留架构意图，明确仍待技术评审的选型 |
| `docs/decisions.md` | `docs/context/technical/lifesub-mvp-202608/decisions.md` | 保留已确认决策与待决事项，补充关联文档 |
| `docs/integrations.md` | `docs/context/technical/lifesub-mvp-202608/integrations.md` | 保留插件边界和工具草案 |
| `docs/privacy-and-sync.md` | `docs/context/technical/lifesub-mvp-202608/privacy-and-sync.md` | 保留隐私、授权、同步及合规边界 |
| `docs/prd/.demo-feature/` | 删除 | 该目录是初始化演示，不属于 LifeSub 产品需求 |

## 正式 PRD 设计

`PRD.md` 以首个 macOS 软件记忆闭环为 V0.1 范围，内容来源于现有产品、路线图、架构、插件和隐私文档，不凭空补齐尚未确认的技术选择或业务指标。

PRD 包含：

1. 业务目标与核心用户价值。
2. 从录制到 Agent 引用证据的主流程。
3. macOS 录制、处理、记忆检索、权限审计与插件访问等功能模块。
4. 明确的首版范围与非目标。
5. 性能、隐私、可靠性和数据可追溯约束。
6. 可验证的验收标准。
7. 产品、研究和技术上下文的关联链接。

尚未确认的量化指标、技术栈、模型选择、授权生命周期和 Malow 接口统一标记为待确认，不把草案升级为既定承诺。

## 进度与笔记

- `.artifacts/process.md` 使用 YAML frontmatter，`stage` 设置为 `prd`，记录当前已完成的产品与架构方向、待确认事项、下一步里程碑和本次文档整理日志。
- `.artifacts/notes.md` 只记录当前需求的决策背景、风险与踩坑，不复制技术上下文正文。
- 双视窗 HTML 和两个启动脚本以正式 PRD 路径为准，移除演示需求名称和失效链接。

## README 与索引

- `README.md` 的文档入口改为正式 PRD、产品上下文、技术上下文和项目进度。
- 全部相对链接在迁移后检查目标是否存在。
- `docs/context/INDEX.md` 遵循 `/reflect` 规则：先列出未索引候选，获得用户确认后再追加，不在迁移时静默修改。

## 保留与删除规则

- 使用移动加编辑的方式保留 Git 历史可读性，不保留旧路径的重复副本。
- 不修改 `docs/design/`、知识库模板或工程配置。
- 不把真实录音、个人记忆、密钥或用户配置加入仓库。
- 不扩展当前产品范围，不创建业务代码脚手架。

## 验收

1. `docs/` 根目录不再存在七份平铺的产品与技术文档。
2. LifeSub 正式 PRD、进度和笔记均存在，并符合项目目录约定。
3. 初始化演示 PRD 已移除。
4. README 与文档内所有本地 Markdown 链接均可解析。
5. PRD 对首版范围、非目标、待确认项和验收标准有明确区分。
6. Git 差异只包含本次文档整理相关文件，不覆盖用户的其他改动。
