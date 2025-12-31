#!/bin/bash
set -e

# 创建GitHub Release的脚本
# 需要安装gh CLI: https://cli.github.com/

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

# 检查gh CLI是否安装
if ! command -v gh &> /dev/null; then
    log_error "gh CLI未安装"
    log_info "请安装GitHub CLI: https://cli.github.com/"
    log_info "  macOS: brew install gh"
    log_info "  Linux: https://github.com/cli/cli/blob/trunk/docs/install_linux.md"
    log_info "  Windows: winget install --id GitHub.cli"
    exit 1
fi

# 检查是否已登录
if ! gh auth status &> /dev/null; then
    log_error "未登录到GitHub"
    log_info "请运行: gh auth login"
    exit 1
fi

# 显示使用方法
show_usage() {
    cat << EOF
使用方法: $(basename "$0") <VERSION> [OPTIONS]

参数:
  VERSION         版本号 (例如: 0.1.0)

选项:
  --draft         创建为草稿
  --pre-release   标记为预发布版本
  --dry-run       预览，不实际创建
  --help          显示此帮助信息

示例:
  $(basename "$0") 0.1.0
  $(basename "$0") 0.2.0 --draft
  $(basename "$0") 1.0.0-rc.1 --pre-release

EOF
}

# 参数解析
if [ $# -eq 0 ] || [[ "$1" == "--help" ]]; then
    show_usage
    exit 0
fi

VERSION="$1"
DRAFT=false
PRERELEASE=false
DRY_RUN=false

shift
while [[ $# -gt 0 ]]; do
    case $1 in
        --draft)
            DRAFT=true
            shift
            ;;
        --pre-release)
            PRERELEASE=true
            shift
            ;;
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        *)
            log_error "未知选项: $1"
            show_usage
            exit 1
            ;;
    esac
done

# 验证版本号格式
if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$ ]]; then
    log_error "无效的版本号格式: $VERSION"
    log_info "版本号应遵循语义化版本规范: X.Y.Z 或 X.Y.Z-PRERELEASE"
    exit 1
fi

cd "$PROJECT_ROOT"

log_info "准备创建 GitHub Release"
echo
log_info "版本: $VERSION"
log_info "草稿: $DRAFT"
log_info "预发布: $PRERELEASE"
echo

# 检查tag是否存在
TAG="v${VERSION}"
if ! git rev-parse "$TAG" &> /dev/null; then
    log_error "Tag $TAG 不存在"
    log_info "请先创建tag: git tag -a ${TAG} -m 'Release version ${VERSION}'"
    exit 1
fi

log_success "Tag $TAG 已存在"
echo

# 检查tag是否已推送
if git ls-remote --tags origin | grep -q "refs/tags/${TAG}$"; then
    log_success "Tag $TAG 已推送到远程"
else
    log_warning "Tag $TAG 未推送到远程"
    read -p "是否推送tag? (y/N) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        git push origin "$TAG"
        log_success "Tag已推送"
    else
        log_error "无法创建未推送tag的Release"
        exit 1
    fi
fi
echo

# 提取CHANGELOG
CHANGELOG_CONTENT=$(awk "/## \[${VERSION}\]/,/## \[/{print}" "$PROJECT_ROOT/CHANGELOG.md" | head -n -1)

if [ -z "$CHANGELOG_CONTENT" ]; then
    log_error "CHANGELOG.md中未找到版本 $VERSION 的内容"
    exit 1
fi

log_success "已提取CHANGELOG内容"
echo

# 生成Release说明
RELEASE_NOTES="# Version ${VERSION} Release Notes

**Release Date**: $(date +%Y-%m-%d)
**Documentation**: [API Docs](https://docs.rs/vm/${VERSION}/vm)

$CHANGELOG_CONTENT

## Installation

### From crates.io
\`\`\`bash
cargo install vm --version ${VERSION}
\`\`\`

### From Binaries

Download the appropriate archive for your platform from the assets below.

### From Source
\`\`\`bash
git clone https://github.com/example/vm.git
cd vm
git checkout v${VERSION}
cargo build --release
\`\`\`

## Verification

Verify the integrity of downloaded files using the provided SHA256 checksums.

## Full Changelog

View the full changelog: [CHANGELOG.md](https://github.com/example/vm/blob/v${VERSION}/CHANGELOG.md)

---

**Download**: [GitHub Releases](https://github.com/example/vm/releases/tag/v${VERSION})
"

# 预览
log_info "Release说明预览:"
echo "==================================="
echo "$RELEASE_NOTES"
echo "==================================="
echo

if [ "$DRY_RUN" = true ]; then
    log_info "DRY RUN 模式 - 不会实际创建Release"
    exit 0
fi

# 确认创建
read -p "确认创建Release? (y/N) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    log_info "已取消"
    exit 0
fi
echo

# 构建gh命令参数
GH_ARGS=()
if [ "$DRAFT" = true ]; then
    GH_ARGS+=(--draft)
fi
if [ "$PRERELEASE" = true ]; then
    GH_ARGS+=(--prerelease)
fi

# 创建Release
log_info "创建GitHub Release..."

if gh release create "$TAG" \
    --title "Version ${VERSION}" \
    --notes "$RELEASE_NOTES" \
    "${GH_ARGS[@]}"; then

    log_success "Release创建成功!"
    echo

    RELEASE_URL="https://github.com/$(git remote get-url origin | sed 's/.*github.com[:/]\(.*\)\.git/\1/')/releases/tag/${TAG}"
    log_info "Release URL: $RELEASE_URL"
    echo

    log_info "后续步骤:"
    echo "  1. 上传发布包到Release"
    echo "  2. 验证下载链接"
    echo "  3. 发布到crates.io (如果需要)"
    echo "  4. 更新网站和文档"
    echo "  6. 发布公告到社区"
    echo

else
    log_error "创建Release失败"
    exit 1
fi
