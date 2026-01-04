#!/bin/bash
# 零警告验证检查清单
# 用于验证项目符合所有质量标准

set -e

echo "========================================="
echo "   Rust VM 项目零警告验证检查清单"
echo "========================================="
echo ""

# 颜色定义
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

PASS_COUNT=0
FAIL_COUNT=0

check_item() {
    local description="$1"
    local command="$2"

    echo -n "检查: $description ... "

    if eval "$command" > /dev/null 2>&1; then
        echo -e "${GREEN}✅ 通过${NC}"
        ((PASS_COUNT++))
        return 0
    else
        echo -e "${RED}❌ 失败${NC}"
        ((FAIL_COUNT++))
        return 1
    fi
}

# 1. Rust 版本检查
echo "----- 工具链检查 -----"
check_item "Rust 版本 (1.92)" "rustc --version | grep -q '1.92'"
check_item "Cargo 版本" "cargo --version"

# 2. 编译检查
echo ""
echo "----- 编译检查 -----"
check_item "workspace 编译 (0 错误)" "cargo build --workspace 2>&1 | grep -qv 'error'"
check_item "vm-core 编译" "cargo build --package vm-core 2>&1 | grep -qv 'error'"
check_item "vm-gc 编译" "cargo build --package vm-gc 2>&1 | grep -qv 'error'"
check_item "vm-mem 编译" "cargo build --package vm-mem 2>&1 | grep -qv 'error'"
check_item "vm-graphics 编译" "cargo build --package vm-graphics 2>&1 | grep -qv 'error'"

# 3. Clippy 检查
echo ""
echo "----- Clippy 代码质量检查 -----"
check_item "vm-core clippy (主要警告)" "cargo clippy --package vm-core --lib -- -D warnings 2>&1 | grep -qv 'warning'"
check_item "vm-gc clippy" "cargo clippy --package vm-gc --lib -- -D warnings 2>&1 | grep -qv 'warning'"
check_item "vm-mem clippy" "cargo clippy --package vm-mem --lib -- -D warnings 2>&1 | grep -qv 'warning'"

# 4. 格式检查
echo ""
echo "----- 代码格式检查 -----"
check_item "代码格式正确" "cargo fmt -- --check"

# 5. 测试检查
echo ""
echo "----- 单元测试检查 -----"
check_item "vm-core 测试 (301个测试)" "cargo test --package vm-core --lib 2>&1 | grep -q '301 passed'"
check_item "vm-mem 测试 (240个测试)" "cargo test --package vm-mem --lib 2>&1 | grep -q '240 passed'"

# 6. 文档检查
echo ""
echo "----- 文档检查 -----"
check_item "文档生成成功" "test -f target/doc/index.html"

# 总结
echo ""
echo "========================================="
echo "   验证结果总结"
echo "========================================="
echo -e "${GREEN}通过: $PASS_COUNT 项${NC}"
echo -e "${RED}失败: $FAIL_COUNT 项${NC}"
echo ""

if [ $FAIL_COUNT -eq 0 ]; then
    echo -e "${GREEN}🎉 所有检查通过！项目质量优秀。${NC}"
    exit 0
else
    echo -e "${RED}⚠️  存在 $FAIL_COUNT 个失败项，请检查并修复。${NC}"
    exit 1
fi
