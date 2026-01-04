@echo off
chcp 65001 >nul
title VM Manager 启动脚本

echo.
echo 🚀 VM Manager 启动脚本
echo ====================
echo.

REM 检查 Python
python --version >nul 2>&1
if %errorlevel% == 0 (
    echo ✓ Python 已安装
    echo 正在启动开发服务器...
    echo.
    echo 打开浏览器访问: http://localhost:8000
    echo 按 Ctrl+C 停止服务器
    echo.
    python -m http.server 8000
    goto :end
)

REM 检查 Python 3
python3 --version >nul 2>&1
if %errorlevel% == 0 (
    echo ✓ Python 3 已安装
    echo 正在启动开发服务器...
    echo.
    echo 打开浏览器访问: http://localhost:8000
    echo 按 Ctrl+C 停止服务器
    echo.
    python3 -m http.server 8000
    goto :end
)

REM 检查 PHP
php --version >nul 2>&1
if %errorlevel% == 0 (
    echo ✓ PHP 已安装
    echo 正在启动开发服务器...
    echo.
    echo 打开浏览器访问: http://localhost:8000
    echo 按 Ctrl+C 停止服务器
    echo.
    php -S localhost:8000
    goto :end
)

echo ❌ 错误: 未找到 Python 或 PHP
echo.
echo 请安装以下任一工具:
echo   - Python 3: https://www.python.org/downloads/
echo   - PHP: https://windows.php.net/download/
echo.
echo 或使用 Node.js:
echo   npm install -g serve
echo   serve .
pause
goto :end

:end
