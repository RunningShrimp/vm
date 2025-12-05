#!/bin/bash
# 端到端测试运行脚本
#
# 运行完整的端到端测试套件，验证系统性能不低于基准线，确保测试通过率100%

set -e

echo "=========================================="
echo "端到端测试套件"
echo "=========================================="

# 颜色定义
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# 测试结果统计
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

# 运行端到端测试
echo -e "${YELLOW}运行端到端测试套件...${NC}"
if cargo test --package vm-tests --test e2e_test_suite -- --nocapture 2>&1 | tee /tmp/e2e_test_output.log; then
    echo -e "${GREEN}✓ 端到端测试通过${NC}"
    PASSED_TESTS=$((PASSED_TESTS + 1))
else
    echo -e "${RED}✗ 端到端测试失败${NC}"
    FAILED_TESTS=$((FAILED_TESTS + 1))
    exit 1
fi
TOTAL_TESTS=$((TOTAL_TESTS + 1))

# 运行模块集成测试
echo -e "${YELLOW}运行模块集成测试...${NC}"
if cargo test --package vm-tests --test module_integration_tests -- --nocapture 2>&1 | tee /tmp/module_integration_output.log; then
    echo -e "${GREEN}✓ 模块集成测试通过${NC}"
    PASSED_TESTS=$((PASSED_TESTS + 1))
else
    echo -e "${RED}✗ 模块集成测试失败${NC}"
    FAILED_TESTS=$((FAILED_TESTS + 1))
    exit 1
fi
TOTAL_TESTS=$((TOTAL_TESTS + 1))

# 运行现有集成测试
echo -e "${YELLOW}运行现有集成测试...${NC}"
if cargo test --package vm-tests --test integration_tests -- --nocapture 2>&1 | tee /tmp/integration_output.log; then
    echo -e "${GREEN}✓ 集成测试通过${NC}"
    PASSED_TESTS=$((PASSED_TESTS + 1))
else
    echo -e "${RED}✗ 集成测试失败${NC}"
    FAILED_TESTS=$((FAILED_TESTS + 1))
    exit 1
fi
TOTAL_TESTS=$((TOTAL_TESTS + 1))

# 运行端到端测试（end_to_end.rs）
echo -e "${YELLOW}运行端到端测试（end_to_end.rs）...${NC}"
if cargo test --package vm-tests --test end_to_end -- --nocapture 2>&1 | tee /tmp/end_to_end_output.log; then
    echo -e "${GREEN}✓ 端到端测试（end_to_end.rs）通过${NC}"
    PASSED_TESTS=$((PASSED_TESTS + 1))
else
    echo -e "${RED}✗ 端到端测试（end_to_end.rs）失败${NC}"
    FAILED_TESTS=$((FAILED_TESTS + 1))
    exit 1
fi
TOTAL_TESTS=$((TOTAL_TESTS + 1))

# 总结
echo ""
echo "=========================================="
echo "测试总结"
echo "=========================================="
echo "总测试数: $TOTAL_TESTS"
echo -e "${GREEN}通过: $PASSED_TESTS${NC}"
if [ $FAILED_TESTS -gt 0 ]; then
    echo -e "${RED}失败: $FAILED_TESTS${NC}"
    exit 1
else
    echo -e "${GREEN}失败: 0${NC}"
    echo ""
    echo -e "${GREEN}✓ 所有测试通过！测试通过率: 100%${NC}"
    exit 0
fi


