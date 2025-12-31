#!/bin/bash
set -e

# VM项目版本号更新脚本
# 用途：自动更新版本号、生成变更日志、创建Git tag
# 使用：./scripts/bump_version.sh [major|minor|patch] [--dry-run]

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 日志函数
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# 显示使用方法
show_usage() {
    cat << EOF
使用方法: $(basename "$0") [major|minor|patch] [--dry-run]

版本号更新类型:
  major    更新主版本号 (0.1.0 -> 1.0.0) - 破坏性变更
  minor    更新次版本号 (0.1.0 -> 0.2.0) - 新功能
  patch    更新修订号 (0.1.0 -> 0.1.1) - Bug修复

选项:
  --dry-run    预览变更，不实际执行
  --help       显示此帮助信息

示例:
  $(basename "$0") minor      # 0.1.0 -> 0.2.0
  $(basename "$0") patch      # 0.1.0 -> 0.1.1
  $(basename "$0") major      # 0.1.0 -> 1.0.0
  $(basename "$0") patch --dry-run  # 预览patch更新

EOF
}

# 检查参数
if [ $# -eq 0 ] || [[ "$1" == "--help" ]]; then
    show_usage
    exit 0
fi

VERSION_TYPE="$1"
DRY_RUN=false

if [ "$2" == "--dry-run" ]; then
    DRY_RUN=true
    log_info "DRY RUN 模式 - 不会实际执行变更"
fi

# 验证版本类型
if [[ ! "$VERSION_TYPE" =~ ^(major|minor|patch)$ ]]; then
    log_error "无效的版本类型: $VERSION_TYPE"
    show_usage
    exit 1
fi

# 检查工作目录状态
check_git_status() {
    log_info "检查Git状态..."

    if [ -n "$(git status --porcelain)" ]; then
        log_error "工作目录不干净，请先提交或暂存变更"
        git status --short
        exit 1
    fi

    local branch=$(git branch --show-current)
    log_info "当前分支: $branch"

    if [ "$branch" != "master" ] && [ "$branch" != "main" ]; then
        log_warning "不在master/main分支，当前在: $branch"
        read -p "是否继续? (y/N) " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            exit 1
        fi
    fi
}

# 获取当前版本号
get_current_version() {
    grep "^\[workspace.package\]" -A 10 "$PROJECT_ROOT/Cargo.toml" | \
        grep "version =" | \
        sed 's/.*version = "\([^"]*\)".*/\1/'
}

# 计算新版本号
calculate_new_version() {
    local current="$1"
    local type="$2"

    IFS='.' read -r major minor patch <<< "$current"

    case "$type" in
        major)
            major=$((major + 1))
            minor=0
            patch=0
            ;;
        minor)
            minor=$((minor + 1))
            patch=0
            ;;
        patch)
            patch=$((patch + 1))
            ;;
    esac

    echo "${major}.${minor}.${patch}"
}

# 更新Cargo.toml中的版本号
update_cargo_toml() {
    local new_version="$1"

    log_info "更新 Cargo.toml 版本号: $new_version"

    if [ "$DRY_RUN" = true ]; then
        log_info "[DRY RUN] 将更新 $PROJECT_ROOT/Cargo.toml"
        return
    fi

    # 更新workspace版本号
    if [[ "$OSTYPE" == "darwin"* ]]; then
        # macOS
        sed -i '' "s/^\[workspace.package\]/[workspace.package]\\
version = \"$new_version\"/" "$PROJECT_ROOT/Cargo.toml"
        # 移除旧的version行
        sed -i '' '/^version =/d' "$PROJECT_ROOT/Cargo.toml"
    else
        # Linux
        sed -i "s/^\[workspace.package\]/[workspace.package]\\nversion = \"$new_version\"/" "$PROJECT_ROOT/Cargo.toml"
        sed -i '/^version =/d' "$PROJECT_ROOT/Cargo.toml"
    fi

    log_success "Cargo.toml 已更新"
}

# 更新CHANGELOG
update_changelog() {
    local old_version="$1"
    local new_version="$2"
    local changelog_file="$PROJECT_ROOT/CHANGELOG.md"
    local today=$(date +%Y-%m-%d)

    log_info "更新 CHANGELOG.md"

    # 检查CHANGELOG是否存在
    if [ ! -f "$changelog_file" ]; then
        log_error "CHANGELOG.md 不存在"
        exit 1
    fi

    # 创建新的版本条目
    local new_entry="## [${new_version}] - ${today}

### Added
- (在此处添加新功能)

### Changed
- (在此处添加改进)

### Fixed
- (在此处添加Bug修复)

### Security
- (在此处添加安全修复)

---

"

    if [ "$DRY_RUN" = true ]; then
        log_info "[DRY RUN] 将在 CHANGELOG.md 中添加新条目"
        echo "==================================="
        echo "$new_entry"
        echo "==================================="
        return
    fi

    # 在[Unreleased]后面插入新版本
    if [[ "$OSTYPE" == "darwin"* ]]; then
        # macOS
        awk -v new_entry="$new_entry" '
            /^\[Unreleased\]/ {
                print
                print ""
                print new_entry
                getline
                while (/^---/) {
                    print
                    getline
                }
                print ""
            }
            { print }
        ' "$changelog_file" > "${changelog_file}.tmp" && mv "${changelog_file}.tmp" "$changelog_file"
    else
        # Linux
        awk -v new_entry="$new_entry" '
            /^\[Unreleased\]/ {
                print
                print ""
                print new_entry
                getline
                while (/^---/) {
                    print
                    getline
                }
                print ""
            }
            { print }
        ' "$changelog_file" > "${changelog_file}.tmp" && mv "${changelog_file}.tmp" "$changelog_file"
    fi

    log_success "CHANGELOG.md 已更新"
    log_warning "请手动编辑 CHANGELOG.md 填写详细的变更内容"
}

# 创建Git提交
create_commit() {
    local new_version="$1"

    log_info "创建Git提交..."

    if [ "$DRY_RUN" = true ]; then
        log_info "[DRY RUN] 将创建提交: chore: Bump version to ${new_version}"
        return
    fi

    git add "$PROJECT_ROOT/Cargo.toml" "$PROJECT_ROOT/CHANGELOG.md"
    git commit -m "chore: Bump version to ${new_version}

- 更新版本号至 ${new_version}
- 更新 CHANGELOG.md
"

    log_success "Git提交已创建"
}

# 创建Git tag
create_tag() {
    local new_version="$1"

    log_info "创建Git tag..."

    if [ "$DRY_RUN" = true ]; then
        log_info "[DRY RUN] 将创建 tag: v${new_version}"
        return
    fi

    # 检查tag是否已存在
    if git rev-parse "v${new_version}" >/dev/null 2>&1; then
        log_error "Tag v${new_version} 已存在"
        exit 1
    fi

    git tag -a "v${new_version}" -m "Release version ${new_version}

主要变更请查看 CHANGELOG.md
"

    log_success "Git tag v${new_version} 已创建"
}

# 推送变更
push_changes() {
    local new_version="$1"

    log_info "推送变更到远程..."

    if [ "$DRY_RUN" = true ]; then
        log_info "[DRY RUN] 将推送提交和tag到远程"
        return
    fi

    read -p "是否推送提交和tag到远程? (y/N) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        git push origin master
        git push origin "v${new_version}"
        log_success "变更已推送到远程"
    else
        log_warning "跳过推送，请手动推送："
        echo "  git push origin master"
        echo "  git push origin v${new_version}"
    fi
}

# 显示后续步骤
show_next_steps() {
    local new_version="$1"

    cat << EOF

${GREEN}═══════════════════════════════════════════════════════${NC}
${GREEN}版本更新完成！${NC}
${GREEN}═══════════════════════════════════════════════════════${NC}

当前版本: ${YELLOW}$(get_current_version)${NC}
新版本:     ${GREEN}${new_version}${NC}

${BLUE}后续步骤：${NC}

1. 编辑 CHANGELOG.md 填写详细变更内容：
   ${YELLOW}vim CHANGELOG.md${NC}

2. 查看变更：
   ${YELLOW}git diff HEAD~1${NC}

3. 运行完整测试：
   ${YELLOW}cargo test --workspace${NC}
   ${YELLOW}cargo clippy --workspace -- -D warnings${NC}

4. 构建发布包：
   ${YELLOW}cargo build --workspace --release${NC}

5. 创建GitHub Release：
   ${YELLOW}./scripts/create_github_release.sh ${new_version}${NC}
   或访问: https://github.com/example/vm/releases/new

6. 发布到crates.io（可选）：
   ${YELLOW}./scripts/publish_to_crates.sh ${new_version}${NC}

${BLUE}撤销此次更新：${NC}
  ${YELLOW}git reset --hard HEAD~1${NC}
  ${YELLOW}git tag -d v${new_version}${NC}

EOF
}

# 主函数
main() {
    log_info "开始版本更新流程..."
    echo

    # 检查Git状态
    check_git_status
    echo

    # 获取当前版本
    local current_version=$(get_current_version)
    log_info "当前版本: ${current_version}"
    echo

    # 计算新版本
    local new_version=$(calculate_new_version "$current_version" "$VERSION_TYPE")
    log_info "新版本: ${new_version} (${VERSION_TYPE} update)"
    echo

    # 确认更新
    if [ "$DRY_RUN" = false ]; then
        read -p "确认更新到 ${new_version}? (y/N) " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            log_info "已取消"
            exit 0
        fi
    fi
    echo

    # 执行更新
    update_cargo_toml "$new_version"
    echo
    update_changelog "$current_version" "$new_version"
    echo
    create_commit "$new_version"
    echo
    create_tag "$new_version"
    echo

    if [ "$DRY_RUN" = false ]; then
        push_changes "$new_version"
        echo
    fi

    show_next_steps "$new_version"
}

# 运行主函数
main
