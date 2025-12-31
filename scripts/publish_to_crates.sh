#!/bin/bash
set -e

# 发布到crates.io的脚本
# 按照正确的依赖顺序发布workspace成员

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

# 显示使用方法
show_usage() {
    cat << EOF
使用方法: $(basename "$0") <VERSION>

参数:
  VERSION         版本号 (例如: 0.1.0)

示例:
  $(basename "$0") 0.1.0

注意:
  - 请确保已登录crates.io: cargo login
  - 请确保token有效: ~/.cargo/credentials
  - 按照依赖顺序发布

EOF
}

# 参数解析
if [ $# -eq 0 ] || [[ "$1" == "--help" ]]; then
    show_usage
    exit 0
fi

VERSION="$1"

# 验证版本号
if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    log_error "无效的版本号: $VERSION"
    exit 1
fi

cd "$PROJECT_ROOT"

# 检查是否已登录
if ! grep -q "registry" ~/.cargo/credentials 2>/dev/null; then
    log_error "未登录crates.io"
    log_info "请运行: cargo login"
    exit 1
fi

log_info "准备发布版本 ${VERSION} 到crates.io"
echo

# 定义发布顺序（按照依赖关系）
# 被依赖的包先发布
declare -a CRATES=(
    "vm-core"
    "vm-mem"
    "vm-frontend"
    "vm-device"
    "vm-engine"
    "vm-runtime"
    "vm-cli"
)

# 验证版本号一致
log_info "验证版本号..."
for crate in "${CRATES[@]}"; do
    if [ -d "$PROJECT_ROOT/$crate" ]; then
        crate_version=$(grep '^version = ' "$PROJECT_ROOT/$crate/Cargo.toml" | sed 's/.*version = "\([^"]*\)".*/\1/')
        if [ "$crate_version" != "$VERSION" ]; then
            log_error "$crate 版本不匹配: $crate_version != $VERSION"
            exit 1
        fi
        log_success "$crate 版本正确: $VERSION"
    fi
done
echo

# 发布前检查
log_info "发布前检查..."
echo

# 1. 检查是否在正确的tag上
TAG="v${VERSION}"
CURRENT_TAG=$(git describe --exact-match --tags 2>/dev/null || echo "")

if [ "$CURRENT_TAG" != "$TAG" ]; then
    log_warning "当前不在tag $TAG 上"
    log_info "当前tag: $CURRENT_TAG"
    read -p "是否继续? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 0
    fi
fi

# 2. 检查是否已发布
log_info "检查crates.io上是否已存在..."
for crate in "${CRATES[@]}"; do
    if [ -d "$PROJECT_ROOT/$crate" ]; then
        if curl -s "https://crates.io/api/v1/crates/${crate}" | grep -q "\"num\":\"${VERSION}\""; then
            log_warning "$crate ${VERSION} 已在crates.io上"
            read -p "是否跳过已发布的crate? (y/N) " -n 1 -r
            echo
            if [[ ! $REPLY =~ ^[Yy]$ ]]; then
                log_error "取消发布"
                exit 1
            fi
        fi
    fi
done
echo

# 开始发布
log_info "开始发布到crates.io..."
echo

PUBLISHED=()
FAILED=()
SKIPPED=()

for crate in "${CRATES[@]}"; do
    if [ ! -d "$PROJECT_ROOT/$crate" ]; then
        log_warning "跳过不存在的crate: $crate"
        SKIPPED+=("$crate")
        continue
    fi

    echo
    log_info "========================================="
    log_info "发布 $crate ($VERSION)"
    log_info "========================================="
    echo

    cd "$PROJECT_ROOT/$crate"

    # 清理之前的构建
    log_info "清理构建..."
    cargo clean

    # 发布
    log_info "发布中..."
    if cargo publish --no-verbose 2>&1 | tee /tmp/publish_${crate}.log; then
        log_success "$crate 发布成功"
        PUBLISHED+=("$crate")

        # 等待crates.io索引更新
        log_info "等待crates.io索引更新 (30秒)..."
        sleep 30
    else
        # 检查是否已存在
        if grep -q "is already uploaded" /tmp/publish_${crate}.log; then
            log_warning "$crate 已存在，跳过"
            SKIPPED+=("$crate")
        else
            log_error "$crate 发布失败"
            FAILED+=("$crate")
            log_info "查看日志: cat /tmp/publish_${crate}.log"

            # 询问是否继续
            read -p "是否继续发布其他crate? (y/N) " -n 1 -r
            echo
            if [[ ! $REPLY =~ ^[Yy]$ ]]; then
                log_info "停止发布"
                break
            fi
        fi
    fi

    cd "$PROJECT_ROOT"
done

# 总结
echo
echo "═══════════════════════════════════════════════════════════"
log_info "发布总结"
echo "═══════════════════════════════════════════════════════════"
echo

if [ ${#PUBLISHED[@]} -gt 0 ]; then
    log_success "成功发布 (${#PUBLISHED[@]}):"
    for crate in "${PUBLISHED[@]}"; do
        echo "  ✅ $crate"
    done
    echo
fi

if [ ${#SKIPPED[@]} -gt 0 ]; then
    log_warning "已跳过 (${#SKIPPED[@]}):"
    for crate in "${SKIPPED[@]}"; do
        echo "  ⏭️  $crate"
    done
    echo
fi

if [ ${#FAILED[@]} -gt 0 ]; then
    log_error "发布失败 (${#FAILED[@]}):"
    for crate in "${FAILED[@]}"; do
        echo "  ❌ $crate"
    done
    echo
fi

# 验证发布
echo
log_info "验证发布..."
echo

for crate in "${PUBLISHED[@]}"; do
    if curl -s "https://crates.io/api/v1/crates/${crate}" | grep -q "\"num\":\"${VERSION}\""; then
        log_success "$crate ${VERSION} 已在crates.io上"
    else
        log_warning "$crate ${VERSION} 未在crates.io上找到（可能还在索引）"
    fi
done

echo
if [ ${#FAILED[@]} -eq 0 ]; then
    log_success "发布完成!"
    echo
    log_info "后续步骤:"
    echo "  1. 验证所有crate在crates.io上可用"
    echo "  2. 更新文档链接"
    echo "  3. 测试安装: cargo install vm --version ${VERSION}"
    echo "  4. 更新README和文档"
else
    log_error "部分crate发布失败，请检查并重试"
    exit 1
fi
