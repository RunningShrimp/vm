#!/bin/bash
# Feature组合测试脚本
# 用于验证所有feature组合都能正常编译

set -e

echo "======================================"
echo "Feature组合测试"
echo "======================================"
echo ""

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# 测试计数器
TOTAL=0
PASSED=0
FAILED=0

# 测试函数
test_feature() {
    local crate="$1"
    local feature="$2"

    TOTAL=$((TOTAL + 1))
    echo -n "Testing: $crate with feature '$feature' ... "

    if [ -z "$feature" ]; then
        # 测试默认features
        if cargo check --package "$crate" > /dev/null 2>&1; then
            echo -e "${GREEN}✓ PASSED${NC}"
            PASSED=$((PASSED + 1))
        else
            echo -e "${RED}✗ FAILED${NC}"
            FAILED=$((FAILED + 1))
        fi
    else
        # 测试指定feature
        if cargo check --package "$crate" --features "$feature" > /dev/null 2>&1; then
            echo -e "${GREEN}✓ PASSED${NC}"
            PASSED=$((PASSED + 1))
        else
            echo -e "${RED}✗ FAILED${NC}"
            FAILED=$((FAILED + 1))
        fi
    fi
}

echo "======================================"
echo "测试 vm-frontend features"
echo "======================================"

# 测试vm-frontend的默认feature
test_feature "vm-frontend" ""

# 测试单架构features
test_feature "vm-frontend" "x86_64"
test_feature "vm-frontend" "arm64"
test_feature "vm-frontend" "riscv64"

# 测试RISC-V扩展features
test_feature "vm-frontend" "riscv-m"
test_feature "vm-frontend" "riscv-f"
test_feature "vm-frontend" "riscv-d"
test_feature "vm-frontend" "riscv-c"
test_feature "vm-frontend" "riscv-a"

# 测试多架构组合
test_feature "vm-frontend" "all"
test_feature "vm-frontend" "all-extensions"

echo ""
echo "======================================"
echo "测试 vm-mem features"
echo "======================================"

# 测试vm-mem的默认feature
test_feature "vm-mem" ""

# 测试优化features
test_feature "vm-mem" "opt-simd"
test_feature "vm-mem" "opt-tlb"
test_feature "vm-mem" "opt-numa"
test_feature "vm-mem" "opt-prefetch"
test_feature "vm-mem" "opt-concurrent"

# 测试组合优化
test_feature "vm-mem" "optimizations"

# 测试异步支持
test_feature "vm-mem" "async"

# 测试遗留别名
test_feature "vm-mem" "tlb"

echo ""
echo "======================================"
echo "测试其他crates的基本features"
echo "======================================"

# 测试vm-core（无特殊features）
test_feature "vm-core" ""

# 测试vm-engine（无特殊features）
test_feature "vm-engine" ""

# 测试vm-device（无特殊features）
test_feature "vm-device" ""

echo ""
echo "======================================"
echo "测试总结"
echo "======================================"
echo -e "总计: $TOTAL"
echo -e "${GREEN}通过: $PASSED${NC}"
echo -e "${RED}失败: $FAILED${NC}"
echo ""

if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}所有feature测试通过！${NC}"
    exit 0
else
    echo -e "${RED}有$FAILED个feature测试失败${NC}"
    exit 1
fi
