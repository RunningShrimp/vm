#!/bin/bash
set -e

# 发布前检查脚本
# 执行所有必要的检查以确保准备好发布

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

PASS_COUNT=0
FAIL_COUNT=0
WARN_COUNT=0

# 检查函数
check_pass() {
    echo -e "${GREEN}✓${NC} $1"
    ((PASS_COUNT++))
}

check_fail() {
    echo -e "${RED}✗${NC} $1"
    ((FAIL_COUNT++))
}

check_warn() {
    echo -e "${YELLOW}⚠${NC} $1"
    ((WARN_COUNT++))
}

check_info() {
    echo -e "${BLUE}ℹ${NC} $1"
}

echo "═══════════════════════════════════════════════════════════════"
echo "  发布前检查 (Pre-Release Check)"
echo "═══════════════════════════════════════════════════════════════"
echo

# 1. Git状态检查
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "1. Git状态检查"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

cd "$PROJECT_ROOT"

if [ -n "$(git status --porcelain)" ]; then
    check_fail "工作目录不干净"
    git status --short
else
    check_pass "工作目录干净"
fi

BRANCH=$(git branch --show-current)
if [ "$BRANCH" = "master" ] || [ "$BRANCH" = "main" ]; then
    check_pass "在正确的分支 ($BRANCH)"
else
    check_warn "不在master/main分支，当前在: $BRANCH"
fi

# 检查是否有未推送的提交
if [ -n "$(git log origin/$BRANCH..HEAD 2>/dev/null)" ]; then
    check_warn "有未推送的提交"
else
    check_pass "所有提交已推送"
fi

echo

# 2. 版本号检查
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "2. 版本号检查"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

VERSION=$(grep '^\[workspace.package\]' -A 10 "$PROJECT_ROOT/Cargo.toml" | grep 'version =' | sed 's/.*version = "\([^"]*\)".*/\1/')
check_info "当前版本: $VERSION"

if [[ $VERSION =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    check_pass "版本号格式正确 (Semver)"
else
    check_fail "版本号格式不正确: $VERSION"
fi

# 检查是否有未发布的变更
if grep -q "## \[Unreleased\]" "$PROJECT_ROOT/CHANGELOG.md"; then
    UNRELEASED_CONTENT=$(awk '/## \[Unreleased\]/,/## \[/{print}' "$PROJECT_ROOT/CHANGELOG.md" | grep -E "^\*\*.*\*\*" | grep -v "Unreleased" || true)
    if [ -n "$UNRELEASED_CONTENT" ]; then
        check_warn "CHANGELOG中有未发布的变更"
        echo "$UNRELEASED_CONTENT"
    else
        check_pass "CHANGELOG中没有未发布的变更"
    fi
else
    check_fail "CHANGELOG缺少[Unreleased]部分"
fi

echo

# 3. 构建检查
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "3. 构建检查"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

check_info "构建项目 (这可能需要几分钟)..."
if cargo build --workspace 2>&1 | tee /tmp/build.log | tail -1 > /dev/null; then
    check_pass "构建成功"
else
    check_fail "构建失败"
    tail -20 /tmp/build.log
fi

# 检查构建警告
WARNINGS=$(grep -c "warning:" /tmp/build.log || true)
if [ "$WARNINGS" -eq 0 ]; then
    check_pass "无编译警告"
else
    check_warn "发现 $WARNINGS 个编译警告"
fi

echo

# 4. 测试检查
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "4. 测试检查"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

check_info "运行测试套件..."
if cargo test --workspace 2>&1 | tee /tmp/test.log | tail -1 > /dev/null; then
    check_pass "所有测试通过"
else
    check_fail "测试失败"
    tail -20 /tmp/test.log
fi

# 解析测试结果
TEST_RESULT=$(grep "test result" /tmp/test.log | tail -1)
check_info "$TEST_RESULT"

echo

# 5. 代码质量检查
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "5. 代码质量检查"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# Clippy
check_info "运行 Clippy..."
if cargo clippy --workspace -- -D warnings 2>&1 | tee /tmp/clippy.log | tail -1 > /dev/null; then
    check_pass "Clippy检查通过"
else
    check_fail "Clippy发现警告或错误"
    tail -20 /tmp/clippy.log
fi

# 格式检查
check_info "检查代码格式..."
if cargo fmt -- --check 2>&1 | tee /tmp/fmt.log; then
    check_pass "代码格式正确"
else
    check_fail "代码格式不正确"
    cat /tmp/fmt.log
fi

echo

# 6. 文档检查
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "6. 文档检查"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

check_info "构建文档..."
if cargo doc --workspace --no-deps 2>&1 | tee /tmp/doc.log | tail -1 > /dev/null; then
    check_pass "文档构建成功"

    # 检查文档警告
    DOC_WARNINGS=$(grep -c "warning:" /tmp/doc.log || true)
    if [ "$DOC_WARNINGS" -eq 0 ]; then
        check_pass "无文档警告"
    else
        check_warn "发现 $DOC_WARNINGS 个文档警告"
    fi
else
    check_fail "文档构建失败"
    tail -20 /tmp/doc.log
fi

echo

# 7. 安全审计
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "7. 安全审计"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

if command -v cargo-audit &> /dev/null; then
    check_info "运行 cargo audit..."
    if cargo audit 2>&1 | tee /tmp/audit.log | grep -q "No vulnerabilities found"; then
        check_pass "无已知安全漏洞"
    else
        check_warn "发现潜在的安全漏洞"
        cat /tmp/audit.log
    fi
else
    check_warn "cargo-audit未安装，跳过安全检查"
    check_info "安装: cargo install cargo-audit"
fi

echo

# 8. CHANGELOG检查
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "8. CHANGELOG检查"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

if [ -f "$PROJECT_ROOT/CHANGELOG.md" ]; then
    check_pass "CHANGELOG.md存在"

    # 检查格式
    if grep -q "Keep a Changelog" "$PROJECT_ROOT/CHANGELOG.md"; then
        check_pass "遵循Keep a Changelog格式"
    else
        check_warn "未明确遵循Keep a Changelog格式"
    fi

    # 检查当前版本条目
    if grep -q "## \[$VERSION\]" "$PROJECT_ROOT/CHANGELOG.md"; then
        check_pass "版本 $VERSION 的CHANGELOG条目存在"
    else
        check_fail "缺少版本 $VERSION 的CHANGELOG条目"
    fi
else
    check_fail "CHANGELOG.md不存在"
fi

echo

# 9. 性能基准
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "9. 性能基准检查（可选）"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

if [ -d "$PROJECT_ROOT/benches" ] || [ -d "$PROJECT_ROOT/benchmarks" ]; then
    check_info "检测到基准测试，运行中..."
    if cargo bench --workspace 2>&1 | tee /tmp/bench.log | tail -1 > /dev/null; then
        check_pass "基准测试运行成功"
    else
        check_warn "基准测试运行失败（非阻塞）"
    fi
else
    check_info "未找到基准测试，跳过"
fi

echo

# 10. README检查
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "10. README检查"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

if [ -f "$PROJECT_ROOT/README.md" ]; then
    check_pass "README.md存在"

    # 检查版本徽章
    if grep -q "version" "$PROJECT_ROOT/README.md"; then
        check_info "README包含版本信息（请确认是否为当前版本）"
    fi
else
    check_warn "README.md不存在"
fi

echo

# 总结
echo "═══════════════════════════════════════════════════════════════"
echo "  检查总结"
echo "═══════════════════════════════════════════════════════════════"
echo

echo -e "${GREEN}✓ 通过${NC}: $PASS_COUNT"
echo -e "${YELLOW}⚠ 警告${NC}: $WARN_COUNT"
echo -e "${RED}✗ 失败${NC}: $FAIL_COUNT"
echo

if [ $FAIL_COUNT -eq 0 ]; then
    if [ $WARN_COUNT -eq 0 ]; then
        echo -e "${GREEN}🎉 所有检查通过！可以准备发布。${NC}"
        exit 0
    else
        echo -e "${YELLOW}⚠ 所有必需检查通过，但有 $WARN_COUNT 个警告。${NC}"
        echo -e "${YELLOW}请评估这些警告是否影响发布。${NC}"
        exit 0
    fi
else
    echo -e "${RED}❌ 发现 $FAIL_COUNT 个失败项，请修复后再发布。${NC}"
    exit 1
fi
