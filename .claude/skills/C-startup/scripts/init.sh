#!/usr/bin/env bash
# c-startup init.sh — C 端产品项目轻量初始化脚本
# 用法: bash init.sh <project_name> <feature_id> [owner] [version] [requester]
#
# 产出目录兼容 bootstrap-agentic-project，可随时通过其 init.sh 升级为完整底座。
# 设计原则: safe_copy — 已存在的文件绝不覆盖。

set -euo pipefail

# ─── 参数解析 ───────────────────────────────────────────────
PROJECT_NAME="${1:-}"
FEATURE_ID="${2:-}"
OWNER="${3:-}"
VERSION="${4:-}"
REQUESTER="${5:-}"

if [ -z "$PROJECT_NAME" ] || [ -z "$FEATURE_ID" ]; then
    echo "用法: bash init.sh <项目名称> <需求ID> [负责人] [版本号] [需求方]"
    echo ""
    echo "示例: bash init.sh 趣味单词 FEAT-001 文藏 1.0.0 初中拉新业务线"
    exit 1
fi

OWNER="${OWNER:-TBD}"
VERSION="${VERSION:-1.0.0}"
REQUESTER="${REQUESTER:-}"
DATE_DISPLAY=$(date +%Y-%m-%d)
DATE_DIR=$(date +%Y%m)

# 清理项目名中的文件系统不安全字符
SAFE_PROJECT_NAME=$(echo "$PROJECT_NAME" | sed 's/ /-/g; s/[\/\\:*?"<>|]//g')

# ─── 路径计算 ───────────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ASSETS_DIR="$(cd "$SCRIPT_DIR/../assets" && pwd)"
PROJECT_DIR="${PWD}/${SAFE_PROJECT_NAME}"

# ─── 工具函数 ───────────────────────────────────────────────
safe_copy() {
    local src="$1"
    local dst="$2"
    if [ -e "$dst" ]; then
        echo "  ⏭  跳过 (已存在): ${dst}"
    else
        cp "$src" "$dst"
        echo "  ✅ 创建: ${dst}"
    fi
}

# 模板变量替换
render_template() {
    local src="$1"
    local dst="$2"
    if [ -e "$dst" ]; then
        echo "  ⏭  跳过 (已存在): ${dst}"
        return
    fi
    sed \
        -e "s/{{project_name}}/${PROJECT_NAME}/g" \
        -e "s/{{feature_id}}/${FEATURE_ID}/g" \
        -e "s/{{owner}}/${OWNER}/g" \
        -e "s/{{version}}/${VERSION}/g" \
        -e "s/{{requester}}/${REQUESTER}/g" \
        -e "s/{{date_display}}/${DATE_DISPLAY}/g" \
        -e "s/{{date_dir}}/${DATE_DIR}/g" \
        "$src" > "$dst"
    echo "  ✅ 创建: ${dst}"
}

write_if_missing() {
    local dst="$1"
    local content="$2"
    if [ -e "$dst" ]; then
        echo "  ⏭  跳过 (已存在): ${dst}"
    else
        echo "$content" > "$dst"
        echo "  ✅ 创建: ${dst}"
    fi
}

# ─── Step 1: 创建目录骨架 ────────────────────────────────────
echo ""
echo "📁 创建目录骨架..."
echo ""

dirs=(
    "${PROJECT_DIR}"
    "${PROJECT_DIR}/.claude/templates"
    "${PROJECT_DIR}/.sources"
    "${PROJECT_DIR}/.sources/.converted"
    "${PROJECT_DIR}/docs"
    "${PROJECT_DIR}/docs/context"
    "${PROJECT_DIR}/docs/context/project"
    "${PROJECT_DIR}/docs/context/project/experience"
    "${PROJECT_DIR}/docs/design"
    "${PROJECT_DIR}/docs/prd"
    "${PROJECT_DIR}/docs/prd/${FEATURE_ID}"
    "${PROJECT_DIR}/docs/prd/${FEATURE_ID}/.artifacts"
    "${PROJECT_DIR}/docs/prd/${FEATURE_ID}/需求确认"
)

for d in "${dirs[@]}"; do
    mkdir -p "$d"
done
echo "  ✅ 目录骨架就绪"

# ─── Step 2: 写入模板文件 ────────────────────────────────────
echo ""
echo "📝 写入模板文件..."
echo ""

# 核心文件
render_template "${ASSETS_DIR}/CLAUDE-template.md"                "${PROJECT_DIR}/CLAUDE.md"
render_template "${ASSETS_DIR}/begin-template.md"                 "${PROJECT_DIR}/begin.md"
render_template "${ASSETS_DIR}/INDEX-template.md"                 "${PROJECT_DIR}/docs/context/INDEX.md"
render_template "${ASSETS_DIR}/process-template.md"               "${PROJECT_DIR}/docs/prd/${FEATURE_ID}/.artifacts/process.md"
render_template "${ASSETS_DIR}/notes-template.md"                 "${PROJECT_DIR}/docs/prd/${FEATURE_ID}/.artifacts/notes.md"
render_template "${ASSETS_DIR}/PRD-template.md"                   "${PROJECT_DIR}/docs/prd/${FEATURE_ID}/PRD.md"

# 设计规范入口
render_template "${ASSETS_DIR}/design-spec-template.md"              "${PROJECT_DIR}/docs/design/design-spec.md"

# 路径 A (业务急需) 模板
render_template "${ASSETS_DIR}/require-confirm-template.md"       "${PROJECT_DIR}/docs/prd/${FEATURE_ID}/需求确认/需求确认单.md"
render_template "${ASSETS_DIR}/MRD-template.md"                   "${PROJECT_DIR}/docs/prd/${FEATURE_ID}/需求确认/终版需求明细.md"

# 参考模板（AI 撰写时引用，不做变量替换）
safe_copy "${ASSETS_DIR}/PRD-writing-guide.md"                    "${PROJECT_DIR}/.claude/templates/PRD-writing-guide.md"

# 路径 B (产品自发) 模板
render_template "${ASSETS_DIR}/user-persona-template.md"          "${PROJECT_DIR}/docs/prd/${FEATURE_ID}/用户画像.md"
render_template "${ASSETS_DIR}/competitive-analysis-template.md"  "${PROJECT_DIR}/docs/prd/${FEATURE_ID}/竞品分析.md"
render_template "${ASSETS_DIR}/mvp-hypothesis-template.md"        "${PROJECT_DIR}/docs/prd/${FEATURE_ID}/MVP假设.md"

# PRD 双视窗预览 (HTML + 本地预览脚本)
render_template "${ASSETS_DIR}/PRD_dual-pane.html-template"       "${PROJECT_DIR}/docs/prd/${FEATURE_ID}/.artifacts/PRD_dual-pane.html"
safe_copy "${ASSETS_DIR}/preview-macOS.command-template"           "${PROJECT_DIR}/docs/prd/${FEATURE_ID}/预览PRD-macOS.command"
chmod +x "${PROJECT_DIR}/docs/prd/${FEATURE_ID}/预览PRD-macOS.command" 2>/dev/null || true
safe_copy "${ASSETS_DIR}/preview-Windows.bat-template"             "${PROJECT_DIR}/docs/prd/${FEATURE_ID}/预览PRD-Windows.bat"

# GitLab Pages CI 配置（可选，放在项目根目录）
safe_copy "${ASSETS_DIR}/gitlab-ci-pages-template.yml"             "${PROJECT_DIR}/.gitlab-ci.yml"

# 占位文件
write_if_missing "${PROJECT_DIR}/docs/context/README.md" "# 原始素材

将用户访谈记录、会议纪要、竞品截图等原始材料放在这里。

**命名建议**: 按日期 + 主题命名，如 \`0408-用户访谈.txt\`、\`0415-竞品分析.md\`

**支持格式**: TXT（听记导出）、MD（手动整理）、图片

放入文件后，AI 会根据 CLAUDE.md 中的流程指引协助整理。"

# .gitignore
write_if_missing "${PROJECT_DIR}/.gitignore" ".DS_Store
Thumbs.db
*.pen~
.sources/.converted/"

# ─── Step 3: Git 初始化 ──────────────────────────────────────
echo ""
echo "🔧 初始化 Git..."
echo ""

cd "${PROJECT_DIR}"
if [ ! -d ".git" ]; then
    git init -q
    git add -A
    git commit -q -m "初始化 C 端产品项目: ${PROJECT_NAME} (${FEATURE_ID})"
    echo "  ✅ Git 仓库已初始化并提交"
else
    echo "  ⏭  Git 仓库已存在，跳过初始化"
fi

# ─── Step 4: 完成提示 ────────────────────────────────────────
echo ""
echo "═══════════════════════════════════════════════════════════"
echo "  ✅  项目 '${PROJECT_NAME}' 初始化完成！"
echo "═══════════════════════════════════════════════════════════"
echo ""
echo "  📂 项目目录: ${PROJECT_DIR}"
echo ""
echo "  目录结构:"
echo "  ${SAFE_PROJECT_NAME}/"
echo "  ├── CLAUDE.md                         ← AI 行为指南（双路径状态机）"
echo "  ├── begin.md                          ← 项目概览"
echo "  ├── .claude/templates/                ← 参考模板（AI 撰写时引用）"
echo "  │   └── PRD-writing-guide.md          ← PRD 编写规范"
echo "  ├── .sources/                         ← 原始文档（/ingest 升级后可用）"
echo "  └── docs/"
echo "      ├── context/"
echo "      │   └── INDEX.md                  ← 知识库索引（兼容 /reflect）"
echo "      ├── design/"
echo "      │   └── design-spec.md            ← 设计规范入口"
echo "      └── prd/${FEATURE_ID}/"
echo "          ├── PRD.md                    ← PRD 模板"
echo "          ├── 预览PRD-macOS.command      ← 双击本地预览 PRD 双视窗"
echo "          ├── 预览PRD-Windows.bat        ← Windows 本地预览"
echo "          ├── 需求确认/                  ← 路径 A 模板"
echo "          ├── 用户画像.md                ← 路径 B 模板（可选）"
echo "          ├── 竞品分析.md                ← 路径 B 模板（可选）"
echo "          ├── MVP假设.md                 ← 路径 B 模板（可选）"
echo "          └── .artifacts/"
echo "              ├── PRD_dual-pane.html     ← PRD 双视窗渲染器"
echo "              ├── process.md             ← 进度追踪"
echo "              └── notes.md               ← 决策记录"
echo ""
echo "  下一步:"
echo "  1. cd ${SAFE_PROJECT_NAME}"
echo "  2. 路径 A（业务急需）: 将会议纪要放入 docs/context/，开启 AI 会话"
echo "  3. 路径 B（产品自发）: 直接开启 AI 会话，从用户洞察开始"
echo ""
echo "  📄 PRD 双视窗预览:"
echo "     macOS: 双击 docs/prd/${FEATURE_ID}/预览PRD-macOS.command"
echo "     Windows: 双击 docs/prd/${FEATURE_ID}/预览PRD-Windows.bat"
echo ""
echo "  🌐 GitLab Pages 在线浏览:"
echo "     Push 到 GitLab 后自动部署，访问:"
echo "     https://<namespace>.gitlab.io/<project>/docs/prd/${FEATURE_ID}/.artifacts/PRD_dual-pane.html"
echo ""
echo "  🔄 升级到完整 Agentic 底座:"
echo "     将 bootstrap-agentic-project 的 init.sh 在此目录运行即可"
echo "     已有文件不会被覆盖，多 Agent/hooks/rules/知识库 会自动注入"
echo ""
