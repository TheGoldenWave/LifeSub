#!/bin/bash
# 预览PRD-macOS.command — 双击即可在浏览器预览 PRD 双视窗
# 服务器根目录设为 PRD 目录（本脚本所在目录），确保 fetch('../PRD.md') 可以正常访问

cd "$(dirname "$0")" || {
    echo "❌ 找不到 PRD 目录，请确认文件结构完整。"
    exit 1
}

PORT=8080
HTML_FILE=".artifacts/PRD_dual-pane.html"
PID_FILE=".artifacts/.server.pid"

# --- 清理残留进程 ---
cleanup_port() {
    # Step 1: 通过 PID 文件精确杀旧进程
    if [ -f "$PID_FILE" ]; then
        OLD_PID=$(cat "$PID_FILE" 2>/dev/null)
        if [ -n "$OLD_PID" ] && kill -0 "$OLD_PID" 2>/dev/null; then
            echo "🧹 正在停止上次残留的服务器 (PID: $OLD_PID)..."
            kill "$OLD_PID" 2>/dev/null
            sleep 1
            # 如果还没死，强制杀
            kill -0 "$OLD_PID" 2>/dev/null && kill -9 "$OLD_PID" 2>/dev/null
        fi
        rm -f "$PID_FILE"
    fi

    # Step 2: lsof 兜底 — 端口仍被占用时清理
    if command -v lsof &>/dev/null; then
        BLOCKING_PID=$(lsof -ti:"$PORT" 2>/dev/null)
        if [ -n "$BLOCKING_PID" ]; then
            echo "🧹 端口 $PORT 仍被占用 (PID: $BLOCKING_PID)，正在清理..."
            echo "$BLOCKING_PID" | xargs kill 2>/dev/null
            sleep 1
            # 兜底强杀
            BLOCKING_PID=$(lsof -ti:"$PORT" 2>/dev/null)
            [ -n "$BLOCKING_PID" ] && echo "$BLOCKING_PID" | xargs kill -9 2>/dev/null
        fi
    fi
}

# --- 退出时清理 ---
on_exit() {
    if [ -f "$PID_FILE" ]; then
        SERVER_PID=$(cat "$PID_FILE" 2>/dev/null)
        [ -n "$SERVER_PID" ] && kill "$SERVER_PID" 2>/dev/null
        rm -f "$PID_FILE"
    fi
    exit
}
trap on_exit INT TERM EXIT

# --- 主流程 ---
cleanup_port

echo "🚀 正在启动 PRD 双视窗预览服务..."

if command -v npx &>/dev/null; then
    echo "📦 使用 npx serve 启动..."
    npx serve -l $PORT >/dev/null 2>&1 &
elif command -v python3 &>/dev/null; then
    echo "🐍 使用 Python3 启动..."
    python3 -m http.server $PORT >/dev/null 2>&1 &
elif command -v python &>/dev/null; then
    echo "🐍 使用 Python 启动..."
    python -m SimpleHTTPServer $PORT >/dev/null 2>&1 &
else
    echo "❌ 未找到 Node.js 或 Python，请先安装其中之一。"
    exit 1
fi

# 记录服务器 PID
echo $! > "$PID_FILE"

sleep 2

echo "✅ 服务已启动，正在打开浏览器..."
open "http://localhost:${PORT}/${HTML_FILE}"

echo ""
echo "💡 按 [Ctrl+C] 停止服务器"
wait
