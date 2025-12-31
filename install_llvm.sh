#!/bin/bash

# LLVM 安装脚本
# 支持 macOS 和 Linux

set -e

echo "🔧 LLVM 安装脚本"
echo "=================="

# 检测操作系统
OS=$(uname -s)
if [[ "$OS" == "Darwin" ]]; then
    echo "🍎 检测到 macOS"
    INSTALL_METHOD="homebrew"
elif [[ "$OS" == "Linux" ]]; then
    echo "🐧 检测到 Linux"
    if command -v apt &> /dev/null; then
        INSTALL_METHOD="apt"
    elif command -v yum &> /dev/null; then
        INSTALL_METHOD="yum"
    elif command -v dnf &> /dev/null; then
        INSTALL_METHOD="dnf"
    else
        echo "❌ 不支持的 Linux 发行版"
        exit 1
    fi
else
    echo "❌ 不支持的操作系统: $OS"
    exit 1
fi

echo "📦 使用安装方法: $INSTALL_METHOD"

# 安装 LLVM
case $INSTALL_METHOD in
    "homebrew")
        echo "🍺 使用 Homebrew 安装 LLVM..."
        if ! command -v brew &> /dev/null; then
            echo "❌ Homebrew 未安装，请先安装 Homebrew"
            echo "   访问 https://brew.sh/ 了解安装方法"
            exit 1
        fi
        
        # 安装 LLVM 18
        brew install llvm@18
        
        # 设置环境变量
        LLVM_PREFIX=$(brew --prefix llvm@18)
        
        echo "🔧 设置环境变量..."
        SHELL_RC=""
        if [[ "$SHELL" == */zsh ]]; then
            SHELL_RC="$HOME/.zshrc"
        elif [[ "$SHELL" == */bash ]]; then
            SHELL_RC="$HOME/.bash_profile"
        else
            echo "⚠️  未知的 shell: $SHELL，请手动设置环境变量"
            SHELL_RC="$HOME/.profile"
        fi
        
        # 备份现有配置
        if [[ -f "$SHELL_RC" ]]; then
            cp "$SHELL_RC" "$SHELL_RC.backup.$(date +%s)"
        fi
        
        # 添加环境变量到 shell 配置
        {
            echo ""
            echo "# LLVM 配置 (由 install_llvm.sh 添加)"
            echo "export LLVM_SYS_211_PREFIX=\"$LLVM_PREFIX\""
            echo "export PATH=\"\$LLVM_SYS_211_PREFIX/bin:\$PATH\""
            echo "export DYLD_LIBRARY_PATH=\"\$LLVM_SYS_211_PREFIX/lib:\$DYLD_LIBRARY_PATH\""
            echo "export LD_LIBRARY_PATH=\"\$LLVM_SYS_211_PREFIX/lib:\$LD_LIBRARY_PATH\""
        } >> "$SHELL_RC"
        
        # 立即设置环境变量
        export LLVM_SYS_211_PREFIX="$LLVM_PREFIX"
        export PATH="$LLVM_SYS_211_PREFIX/bin:$PATH"
        export DYLD_LIBRARY_PATH="$LLVM_SYS_211_PREFIX/lib:$DYLD_LIBRARY_PATH"
        export LD_LIBRARY_PATH="$LLVM_SYS_211_PREFIX/lib:$LD_LIBRARY_PATH"
        ;;
        
    "apt")
        echo "📦 使用 apt 安装 LLVM..."
        sudo apt update
        sudo apt install -y llvm-18 llvm-18-dev clang-18
        
        # 设置环境变量
        LLVM_PREFIX="/usr/lib/llvm-18"
        export LLVM_SYS_211_PREFIX="$LLVM_PREFIX"
        export PATH="$LLVM_PREFIX/bin:$PATH"
        export LD_LIBRARY_PATH="$LLVM_PREFIX/lib:$LD_LIBRARY_PATH"
        ;;
        
    "yum")
        echo "📦 使用 yum 安装 LLVM..."
        sudo yum install -y llvm18 llvm18-devel clang18
        
        # 设置环境变量
        LLVM_PREFIX="/usr/lib64/llvm18"
        export LLVM_SYS_211_PREFIX="$LLVM_PREFIX"
        export PATH="$LLVM_PREFIX/bin:$PATH"
        export LD_LIBRARY_PATH="$LLVM_PREFIX/lib64:$LD_LIBRARY_PATH"
        ;;
        
    "dnf")
        echo "📦 使用 dnf 安装 LLVM..."
        sudo dnf install -y llvm18 llvm18-devel clang18
        
        # 设置环境变量
        LLVM_PREFIX="/usr/lib64/llvm18"
        export LLVM_SYS_211_PREFIX="$LLVM_PREFIX"
        export PATH="$LLVM_PREFIX/bin:$PATH"
        export LD_LIBRARY_PATH="$LLVM_PREFIX/lib64:$LD_LIBRARY_PATH"
        ;;
esac

echo ""
echo "✅ LLVM 安装完成！"
echo ""
echo "📍 安装位置: $LLVM_PREFIX"
echo "🔧 环境变量已设置:"
echo "   LLVM_SYS_211_PREFIX=$LLVM_SYS_211_PREFIX"
echo "   PATH=\$LLVM_SYS_211_PREFIX/bin:\$PATH"

if [[ "$OS" == "Linux" ]]; then
    echo "   LD_LIBRARY_PATH=\$LLVM_SYS_211_PREFIX/lib:\$LD_LIBRARY_PATH"
else
    echo "   DYLD_LIBRARY_PATH=\$LLVM_SYS_211_PREFIX/lib:\$DYLD_LIBRARY_PATH"
fi

echo ""
echo "🔄 请重新加载你的 shell 配置文件:"
if [[ "$INSTALL_METHOD" == "homebrew" ]]; then
    echo "   source $SHELL_RC"
    echo "   或者重新打开终端"
else
    echo "   重新登录或运行: source ~/.bashrc"
fi

echo ""
echo "🧪 验证安装..."

# 验证安装
if command -v llvm-config &> /dev/null; then
    LLVM_VERSION=$(llvm-config --version 2>/dev/null || echo "未知")
    echo "✅ LLVM 版本: $LLVM_VERSION"
else
    echo "⚠️  llvm-config 未找到，请检查 PATH 环境变量"
fi

if command -v clang &> /dev/null; then
    CLANG_VERSION=$(clang --version | head -n1)
    echo "✅ Clang 版本: $CLANG_VERSION"
else
    echo "⚠️  clang 未找到，请检查 PATH 环境变量"
fi

echo ""
echo "🚀 现在可以尝试编译项目了:"
echo "   cargo build"
echo ""
echo "💡 如果要启用所有 LLVM 功能，使用:"
echo "   cargo build --features llvm"
echo ""
echo "💡 如果要禁用 LLVM 功能，使用:"
echo "   cargo build --no-default-features"

echo ""
echo "📚 更多信息请查看 LLVM_INSTALLATION_GUIDE.md"