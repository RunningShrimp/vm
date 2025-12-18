#!/bin/bash

# 多后端安装脚本
# 支持 LLVM、Cranelift 等多种后端的安装和配置

set -e

echo "🔧 多后端安装脚本"
echo "=================="

# 默认参数
BACKEND=""
LLVM_VERSION="18"
INSTALL_ALL=false
SKIP_DEPS=false

# 解析命令行参数
while [[ $# -gt 0 ]]; do
    case $1 in
        --backend)
            BACKEND="$2"
            shift 2
            ;;
        --llvm-version)
            LLVM_VERSION="$2"
            shift 2
            ;;
        --all)
            INSTALL_ALL=true
            shift
            ;;
        --skip-deps)
            SKIP_DEPS=true
            shift
            ;;
        --help|-h)
            echo "用法: $0 [选项]"
            echo ""
            echo "选项:"
            echo "  --backend BACKEND    指定要安装的后端 (llvm|cranelift|all)"
            echo "  --llvm-version VER   指定LLVM版本 (默认: 18)"
            echo "  --all               安装所有支持的后端"
            echo "  --skip-deps         跳过系统依赖安装"
            echo "  --help, -h          显示此帮助信息"
            echo ""
            echo "示例:"
            echo "  $0 --backend llvm"
            echo "  $0 --backend cranelift"
            echo "  $0 --all"
            exit 0
            ;;
        *)
            echo "❌ 未知参数: $1"
            echo "使用 --help 查看帮助信息"
            exit 1
            ;;
    esac
done

# 如果没有指定后端，询问用户
if [[ -z "$BACKEND" && "$INSTALL_ALL" == false ]]; then
    echo "请选择要安装的后端:"
    echo "1) LLVM"
    echo "2) Cranelift"
    echo "3) 所有后端"
    read -p "请输入选择 (1-3): " choice
    
    case $choice in
        1)
            BACKEND="llvm"
            ;;
        2)
            BACKEND="cranelift"
            ;;
        3)
            INSTALL_ALL=true
            ;;
        *)
            echo "❌ 无效选择"
            exit 1
            ;;
    esac
fi

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

# 安装系统依赖
if [[ "$SKIP_DEPS" == false ]]; then
    echo "🔧 安装系统依赖..."
    case $INSTALL_METHOD in
        "homebrew")
            if ! command -v brew &> /dev/null; then
                echo "❌ Homebrew 未安装，请先安装 Homebrew"
                echo "   访问 https://brew.sh/ 了解安装方法"
                exit 1
            fi
            brew update
            ;;
        "apt")
            sudo apt update
            sudo apt install -y build-essential cmake git
            ;;
        "yum"|"dnf")
            sudo $INSTALL_METHOD install -y gcc gcc-c++ cmake git
            ;;
    esac
fi

# 安装LLVM后端
install_llvm() {
    echo "🔧 安装 LLVM 后端..."
    
    case $INSTALL_METHOD in
        "homebrew")
            echo "🍺 使用 Homebrew 安装 LLVM $LLVM_VERSION..."
            brew install llvm@$LLVM_VERSION
            
            # 设置环境变量
            LLVM_PREFIX=$(brew --prefix llvm@$LLVM_VERSION)
            ;;
        "apt")
            echo "📦 使用 apt 安装 LLVM $LLVM_VERSION..."
            sudo apt install -y llvm-$LLVM_VERSION llvm-$LLVM_VERSION-dev clang-$LLVM_VERSION
            
            # 设置环境变量
            LLVM_PREFIX="/usr/lib/llvm-$LLVM_VERSION"
            ;;
        "yum")
            echo "📦 使用 yum 安装 LLVM $LLVM_VERSION..."
            sudo yum install -y llvm$LLVM_VERSION llvm$LLVM_VERSION-devel clang$LLVM_VERSION
            
            # 设置环境变量
            LLVM_PREFIX="/usr/lib64/llvm$LLVM_VERSION"
            ;;
        "dnf")
            echo "📦 使用 dnf 安装 LLVM $LLVM_VERSION..."
            sudo dnf install -y llvm$LLVM_VERSION llvm$LLVM_VERSION-devel clang$LLVM_VERSION
            
            # 设置环境变量
            LLVM_PREFIX="/usr/lib64/llvm$LLVM_VERSION"
            ;;
    esac
    
    # 设置环境变量
    setup_llvm_env "$LLVM_PREFIX"
    
    echo "✅ LLVM 后端安装完成！"
}

# 安装Cranelift后端
install_cranelift() {
    echo "🔧 安装 Cranelift 后端..."
    
    # Cranelift 主要是 Rust crate，通过 Cargo 安装
    if ! command -v cargo &> /dev/null; then
        echo "❌ Cargo 未找到，请先安装 Rust"
        echo "   访问 https://rustup.rs/ 了解安装方法"
        exit 1
    fi
    
    echo "📦 Cranelift 通过 Cargo crate 自动安装"
    echo "   无需额外的系统级安装"
    
    # 验证 Cranelift 可用性
    echo "🧪 验证 Cranelift 支持..."
    if cargo search cranelift --limit 1 &> /dev/null; then
        echo "✅ Cranelift crate 可用"
    else
        echo "⚠️  无法验证 Cranelift crate 可用性"
    fi
    
    echo "✅ Cranelift 后端配置完成！"
}

# 设置LLVM环境变量
setup_llvm_env() {
    local llvm_prefix="$1"
    
    echo "🔧 设置 LLVM 环境变量..."
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
    
    # 移除旧的 LLVM 配置
    sed -i.tmp '/# LLVM 配置 (由 install_llvm.sh 添加)/,/export LD_LIBRARY_PATH/d' "$SHELL_RC" 2>/dev/null || true
    rm -f "$SHELL_RC.tmp"
    
    # 添加环境变量到 shell 配置
    {
        echo ""
        echo "# LLVM 配置 (由 install_backend.sh 添加)"
        echo "export LLVM_SYS_211_PREFIX=\"$llvm_prefix\""
        echo "export PATH=\"\$LLVM_SYS_211_PREFIX/bin:\$PATH\""
    } >> "$SHELL_RC"
    
    if [[ "$OS" == "Linux" ]]; then
        echo "export LD_LIBRARY_PATH=\"\$LLVM_SYS_211_PREFIX/lib:\$LD_LIBRARY_PATH\"" >> "$SHELL_RC"
    else
        echo "export DYLD_LIBRARY_PATH=\"\$LLVM_SYS_211_PREFIX/lib:\$DYLD_LIBRARY_PATH\"" >> "$SHELL_RC"
    fi
    
    # 立即设置环境变量
    export LLVM_SYS_211_PREFIX="$llvm_prefix"
    export PATH="$LLVM_SYS_211_PREFIX/bin:$PATH"
    if [[ "$OS" == "Linux" ]]; then
        export LD_LIBRARY_PATH="$LLVM_SYS_211_PREFIX/lib:$LD_LIBRARY_PATH"
    else
        export DYLD_LIBRARY_PATH="$LLVM_SYS_211_PREFIX/lib:$DYLD_LIBRARY_PATH"
    fi
}

# 验证安装
verify_installation() {
    echo ""
    echo "🧪 验证安装..."
    
    if [[ "$BACKEND" == "llvm" || "$INSTALL_ALL" == true ]]; then
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
    fi
    
    if [[ "$BACKEND" == "cranelift" || "$INSTALL_ALL" == true ]]; then
        if command -v cargo &> /dev/null; then
            echo "✅ Cargo 可用，Cranelift 可通过 crate 安装"
        else
            echo "⚠️  Cargo 不可用，Cranelift 安装可能失败"
        fi
    fi
}

# 生成后端配置文件
generate_backend_config() {
    echo ""
    echo "📝 生成后端配置文件..."
    
    config_file="scripts/backend_config.json"
    
    # 创建配置目录
    mkdir -p "$(dirname "$config_file")"
    
    # 基础配置
    cat > "$config_file" << EOF
{
  "backends": {
EOF
    
    first=true
    if [[ "$BACKEND" == "llvm" || "$INSTALL_ALL" == true ]]; then
        if [[ "$first" == false ]]; then echo "," >> "$config_file"; fi
        cat >> "$config_file" << EOF
    "llvm": {
      "enabled": true,
      "version": "$LLVM_VERSION",
      "prefix": "${LLVM_PREFIX:-}",
      "features": ["llvm-backend"]
    }
EOF
        first=false
    fi
    
    if [[ "$BACKEND" == "cranelift" || "$INSTALL_ALL" == true ]]; then
        if [[ "$first" == false ]]; then echo "," >> "$config_file"; fi
        cat >> "$config_file" << EOF
    "cranelift": {
      "enabled": true,
      "version": "latest",
      "features": ["cranelift-backend"]
    }
EOF
        first=false
    fi
    
    cat >> "$config_file" << EOF
  },
  "default_backend": "$([ "$BACKEND" != "" ] && echo "$BACKEND" || echo "cranelift")",
  "install_date": "$(date -Iseconds)",
  "os": "$OS",
  "install_method": "$INSTALL_METHOD"
}
EOF
    
    echo "✅ 后端配置文件已生成: $config_file"
}

# 主安装逻辑
if [[ "$INSTALL_ALL" == true ]]; then
    echo "🚀 安装所有支持的后端..."
    install_llvm
    install_cranelift
elif [[ "$BACKEND" == "llvm" ]]; then
    install_llvm
elif [[ "$BACKEND" == "cranelift" ]]; then
    install_cranelift
else
    echo "❌ 未知后端: $BACKEND"
    exit 1
fi

# 验证安装
verify_installation

# 生成配置文件
generate_backend_config

echo ""
echo "🎉 后端安装完成！"
echo ""
echo "🔄 请重新加载你的 shell 配置文件:"
if [[ "$INSTALL_METHOD" == "homebrew" ]]; then
    echo "   source $SHELL_RC"
    echo "   或者重新打开终端"
else
    echo "   重新登录或运行: source ~/.bashrc"
fi

echo ""
echo "🚀 现在可以尝试编译项目了:"
echo "   cargo build"
echo ""
echo "💡 使用特定后端构建:"
if [[ "$BACKEND" == "llvm" || "$INSTALL_ALL" == true ]]; then
    echo "   cargo build --features llvm"
fi
if [[ "$BACKEND" == "cranelift" || "$INSTALL_ALL" == true ]]; then
    echo "   cargo build --features cranelift-backend"
fi
echo ""
echo "💡 使用所有后端:"
echo "   cargo build --features full-backends"
echo ""
echo "📚 配置文件位置: scripts/backend_config.json"