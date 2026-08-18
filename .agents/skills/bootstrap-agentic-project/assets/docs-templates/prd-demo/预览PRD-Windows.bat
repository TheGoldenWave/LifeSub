@echo off
chcp 65001 >nul
REM 预览PRD-Windows.bat — 双击即可在浏览器预览 PRD 双视窗
REM 服务器根目录设为 PRD 目录（本脚本所在目录），确保 fetch('../PRD.md') 可以正常访问

cd /d "%~dp0"
IF NOT EXIST ".artifacts\PRD_dual-pane.html" (
    echo ❌ 找不到 .artifacts\PRD_dual-pane.html，请确认文件结构完整。
    pause
    exit /b 1
)

SET PORT=8080
SET HTML_FILE=.artifacts/PRD_dual-pane.html
SET PID_FILE=.artifacts\.server.pid

REM --- 清理残留进程 ---
IF EXIST "%PID_FILE%" (
    SET /P OLD_PID=<"%PID_FILE%"
    IF DEFINED OLD_PID (
        echo 🧹 正在停止上次残留的服务器 (PID: %OLD_PID%)...
        taskkill /PID %OLD_PID% /F >nul 2>&1
    )
    del /f "%PID_FILE%" >nul 2>&1
)

REM 兜底：检查端口是否仍被占用并清理
for /f "tokens=5" %%a in ('netstat -aon ^| findstr ":%PORT% " ^| findstr "LISTENING" 2^>nul') do (
    if not "%%a"=="0" (
        echo 🧹 端口 %PORT% 仍被占用 (PID: %%a)，正在清理...
        taskkill /PID %%a /F >nul 2>&1
    )
)

REM --- 启动服务器 ---
echo 🚀 正在启动 PRD 双视窗预览服务...

where python >nul 2>&1
IF %ERRORLEVEL%==0 (
    echo 🐍 使用 Python 启动...
    start "" /B python -m http.server %PORT% >nul 2>&1
    REM 获取 python 服务器 PID
    for /f "tokens=5" %%a in ('netstat -aon ^| findstr ":%PORT% " ^| findstr "LISTENING" 2^>nul') do (
        echo %%a> "%PID_FILE%"
        goto :open
    )
    REM 等待服务器启动后重试获取 PID
    timeout /t 2 /nobreak >nul
    for /f "tokens=5" %%a in ('netstat -aon ^| findstr ":%PORT% " ^| findstr "LISTENING" 2^>nul') do (
        echo %%a> "%PID_FILE%"
        goto :open
    )
    goto :open
)

where npx >nul 2>&1
IF %ERRORLEVEL%==0 (
    echo 📦 使用 npx serve 启动...
    start "" /B npx serve -l %PORT% >nul 2>&1
    timeout /t 3 /nobreak >nul
    for /f "tokens=5" %%a in ('netstat -aon ^| findstr ":%PORT% " ^| findstr "LISTENING" 2^>nul') do (
        echo %%a> "%PID_FILE%"
        goto :open
    )
    goto :open
)

echo ❌ 未找到 Python 或 Node.js，请先安装其中之一。
pause
exit /b 1

:open
timeout /t 2 /nobreak >nul
start "" http://localhost:%PORT%/%HTML_FILE%
echo ✅ 预览已在浏览器打开。关闭此窗口将停止服务器。
echo.
echo 💡 按任意键停止服务器并退出
pause >nul

REM --- 退出时清理 ---
IF EXIST "%PID_FILE%" (
    SET /P SERVER_PID=<"%PID_FILE%"
    IF DEFINED SERVER_PID (
        taskkill /PID %SERVER_PID% /F >nul 2>&1
    )
    del /f "%PID_FILE%" >nul 2>&1
)
