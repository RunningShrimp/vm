#!/bin/bash

# VM Manager 启动脚本

echo "🚀 VM Manager 启动脚本"
echo "===================="
echo ""

# 检查 Python 是否安装
if command -v python3 &> /dev/null; then
    echo "✓ Python 3 已安装"
    echo "正在启动开发服务器..."
    echo ""
    echo "打开浏览器访问: http://localhost:8000"
    echo "按 Ctrl+C 停止服务器"
    echo ""
    python3 -m http.server 8000
elif command -v python &> /dev/null; then
    echo "✓ Python 已安装"
    echo "正在启动开发服务器..."
    echo ""
    echo "打开浏览器访问: http://localhost:8000"
    echo "按 Ctrl+C 停止服务器"
    echo ""
    python -m SimpleHTTPServer 8000
elif command -v php &> /dev/null; then
    echo "✓ PHP 已安装"
    echo "正在启动开发服务器..."
    echo ""
    echo "打开浏览器访问: http://localhost:8000"
    echo "按 Ctrl+C 停止服务器"
    echo ""
    php -S localhost:8000
else
    echo "❌ 错误: 未找到 Python 或 PHP"
    echo ""
    echo "请安装以下任一工具:"
    echo "  - Python 3: https://www.python.org/downloads/"
    echo "  - PHP: https://www.php.net/downloads"
    echo ""
    echo "或使用 Node.js:"
    echo "  npm install -g serve"
    echo "  serve ."
    exit 1
fi
