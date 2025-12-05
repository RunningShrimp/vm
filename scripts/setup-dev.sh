#!/bin/bash

# FVP虚拟机系统开发环境设置脚本
# 一键设置完整的开发环境和工具链

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_header() {
    echo -e "${BLUE}=====================================${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}=====================================${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️ $1${NC}"
}

print_info() {
    echo -e "${BLUE}ℹ️ $1${NC}"
}

# 检查命令是否存在
check_command() {
    if ! command -v $1 &> /dev/null; then
        print_error "$1 未安装"
        return 1
    else
        print_success "$1 已安装"
        return 0
    fi
}

# 安装系统依赖
install_system_deps() {
    print_header "安装系统依赖"

    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        # Linux (Ubuntu/Debian)
        print_info "检测到Linux系统，安装依赖..."

        sudo apt-get update

        sudo apt-get install -y \
            build-essential \
            pkg-config \
            libssl-dev \
            qemu-kvm \
            libvirt-daemon-system \
            libvirt-clients \
            bridge-utils \
            curl \
            wget \
            git \
            jq \
            bc \
            lcov \
            python3 \
            python3-pip \
            nodejs \
            npm

        print_success "系统依赖安装完成"

    elif [[ "$OSTYPE" == "darwin"* ]]; then
        # macOS
        print_info "检测到macOS系统，检查Homebrew..."

        if ! command -v brew &> /dev/null; then
            print_info "安装Homebrew..."
            /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
        fi

        print_info "通过Homebrew安装依赖..."
        brew install \
            rust \
            llvm \
            pkg-config \
            openssl \
            qemu \
            libvirt \
            jq \
            bc \
            node

        print_success "系统依赖安装完成"

    else
        print_warning "不支持的操作系统，请手动安装依赖"
    fi
}

# 安装Rust工具链
install_rust() {
    print_header "安装Rust工具链"

    if ! check_command "rustc"; then
        print_info "安装Rust..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source "$HOME/.cargo/env"
    fi

    print_info "更新Rust工具链..."
    rustup update stable
    rustup component add rustfmt clippy rust-src llvm-tools-preview

    print_success "Rust工具链安装完成"
}

# 安装Cargo工具
install_cargo_tools() {
    print_header "安装Cargo开发工具"

    local tools=(
        "cargo-llvm-cov"     # 代码覆盖率
        "cargo-criterion"    # 性能基准测试
        "cargo-audit"        # 安全审计
        "cargo-deny"         # 依赖检查
        "cargo-watch"        # 文件监控重新编译
        "cargo-edit"         # 依赖管理
        "cargo-expand"       # 宏展开
    )

    for tool in "${tools[@]}"; do
        if ! command -v "$tool" &> /dev/null; then
            print_info "安装 $tool..."
            cargo install "$tool"
        else
            print_success "$tool 已安装"
        fi
    done

    print_success "Cargo工具安装完成"
}

# 安装开发工具
install_dev_tools() {
    print_header "安装开发工具"

    # 安装Python依赖
    print_info "安装Python依赖..."
    pip3 install --user \
        jinja2 \
        markdown \
        pygments

    # 安装Node.js依赖（用于文档生成）
    if command -v npm &> /dev/null; then
        print_info "安装Node.js依赖..."
        npm install -g \
            markdown-it \
            markdown-it-cli
    fi

    print_success "开发工具安装完成"
}

# 设置Git配置
setup_git() {
    print_header "设置Git配置"

    # 设置Git钩子
    print_info "设置Git钩子..."
    git config core.hooksPath .githooks

    # 检查Git用户配置
    if ! git config user.name &> /dev/null; then
        print_warning "未设置Git用户名"
        read -p "请输入Git用户名: " git_name
        git config user.name "$git_name"
    fi

    if ! git config user.email &> /dev/null; then
        print_warning "未设置Git邮箱"
        read -p "请输入Git邮箱: " git_email
        git config user.email "$git_email"
    fi

    print_success "Git配置完成"
}

# 创建开发目录结构
create_dev_dirs() {
    print_header "创建开发目录结构"

    local dirs=(
        "logs"
        "tmp"
        "bench-results"
        "test-results"
        "coverage"
        "docs/generated"
        "scripts/output"
    )

    for dir in "${dirs[@]}"; do
        if [ ! -d "$dir" ]; then
            mkdir -p "$dir"
            print_info "创建目录: $dir"
        fi
    done

    # 创建.gitignore条目
    cat >> .gitignore << EOF

# 开发环境
logs/
tmp/
bench-results/
test-results/
coverage/

# IDE
.vscode/
.idea/
*.swp
*.swo

# 性能测试
criterion/

# 覆盖率报告
*.profraw
*.profdata

# 临时文件
*.tmp
*.bak

# 操作系统
.DS_Store
Thumbs.db
EOF

    print_success "开发目录结构创建完成"
}

# 验证安装
verify_installation() {
    print_header "验证安装"

    local required_commands=(
        "rustc"
        "cargo"
        "rustfmt"
        "clippy"
        "git"
    )

    local optional_commands=(
        "cargo-llvm-cov"
        "cargo-criterion"
        "cargo-audit"
        "cargo-deny"
    )

    print_info "检查必需命令..."
    for cmd in "${required_commands[@]}"; do
        check_command "$cmd"
    done

    print_info "检查可选命令..."
    for cmd in "${optional_commands[@]}"; do
        if ! check_command "$cmd"; then
            print_warning "可选工具 $cmd 未安装，某些功能可能不可用"
        fi
    done

    # 验证项目编译
    print_info "验证项目编译..."
    if cargo check --all-features; then
        print_success "项目编译验证通过"
    else
        print_error "项目编译验证失败"
        return 1
    fi

    # 运行快速测试
    print_info "运行快速测试..."
    if cargo test --lib --quiet; then
        print_success "快速测试通过"
    else
        print_warning "快速测试失败，可能需要进一步配置"
    fi

    print_success "安装验证完成"
}

# 显示开发指南
show_dev_guide() {
    print_header "开发指南"

    cat << 'EOF'
🚀 FVP虚拟机系统开发环境设置完成！

常用命令：
  编译项目:      cargo build --all-features
  运行测试:      cargo test --all-features
  代码格式:      cargo fmt
  代码检查:      cargo clippy --all-features
  覆盖率:        cargo llvm-cov --all-features
  基准测试:      cargo bench --all-features
  安全审计:      cargo audit

运行所有测试:
  ./scripts/test.sh --all

生成文档:
  cargo doc --all-features --open

监控仪表板:
  cargo run --release --package vm-monitor --features dashboard

开发工作流：
  1. 创建功能分支
  2. 开发功能
  3. 运行测试验证
  4. 提交代码（自动运行预提交检查）
  5. 创建Pull Request

遇到问题？
  - 查看日志: tail -f logs/dev.log
  - 清理缓存: cargo clean
  - 更新依赖: cargo update
  - 检查工具: ./scripts/check-tools.sh

Happy hacking! 🎉
EOF
}

# 主函数
main() {
    print_header "FVP虚拟机系统开发环境设置"
    print_info "开始时间: $(date)"

    # 检查是否在项目根目录
    if [ ! -f "Cargo.toml" ]; then
        print_error "请在项目根目录运行此脚本"
        exit 1
    fi

    # 询问是否安装系统依赖
    read -p "是否安装系统依赖？(y/N): " install_deps
    if [[ $install_deps =~ ^[Yy]$ ]]; then
        install_system_deps
    fi

    # 安装开发环境
    install_rust
    install_cargo_tools
    install_dev_tools
    setup_git
    create_dev_dirs
    verify_installation
    show_dev_guide

    print_header "开发环境设置完成！"
    print_success "现在可以开始开发了 🚀"
}

# 运行主函数
main "$@"