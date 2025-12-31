#!/bin/bash

echo "=========================================="
echo "RISC-V M扩展实现验证"
echo "=========================================="
echo ""

# 颜色定义
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "1. 检查文件存在性"
echo "------------------------------------------"
if [ -f "vm-frontend/src/riscv64/div.rs" ]; then
    echo -e "${GREEN}✓${NC} div.rs 存在 ($(wc -l < vm-frontend/src/riscv64/div.rs) 行)"
else
    echo -e "${RED}✗${NC} div.rs 不存在"
fi

if [ -f "vm-frontend/src/riscv64/mul.rs" ]; then
    echo -e "${GREEN}✓${NC} mul.rs 存在 ($(wc -l < vm-frontend/src/riscv64/mul.rs) 行)"
else
    echo -e "${RED}✗${NC} mul.rs 不存在"
fi

echo ""
echo "2. 检查模块声明"
echo "------------------------------------------"
if grep -q "pub mod div;" vm-frontend/src/riscv64/mod.rs; then
    echo -e "${GREEN}✓${NC} div 模块已声明"
else
    echo -e "${RED}✗${NC} div 模块未声明"
fi

if grep -q "pub mod mul;" vm-frontend/src/riscv64/mod.rs; then
    echo -e "${GREEN}✓${NC} mul 模块已声明"
else
    echo -e "${RED}✗${NC} mul 模块未声明"
fi

echo ""
echo "3. 检查除法指令实现"
echo "------------------------------------------"
DIV_INSTRUCTIONS=("Div" "Divu" "Rem" "Remu" "Divw" "Divuw" "Remw" "Remuw")
FOUND_COUNT=0

for instr in "${DIV_INSTRUCTIONS[@]}"; do
    if grep -q "$instr," vm-frontend/src/riscv64/div.rs; then
        echo -e "${GREEN}✓${NC} $instr 指令已实现"
        ((FOUND_COUNT++))
    else
        echo -e "${RED}✗${NC} $instr 指令未找到"
    fi
done

echo ""
echo "除法指令: $FOUND_COUNT/8"

echo ""
echo "4. 检查乘法指令实现"
echo "------------------------------------------"
MUL_INSTRUCTIONS=("Mul" "Mulh" "Mulhsu" "Mulhu" "Mulw")
FOUND_COUNT=0

for instr in "${MUL_INSTRUCTIONS[@]}"; do
    if grep -q "$instr," vm-frontend/src/riscv64/mul.rs; then
        echo -e "${GREEN}✓${NC} $instr 指令已实现"
        ((FOUND_COUNT++))
    else
        echo -e "${RED}✗${NC} $instr 指令未找到"
    fi
done

echo ""
echo "乘法指令: $FOUND_COUNT/5"

echo ""
echo "5. 检查编码函数"
echo "------------------------------------------"
DIV_ENCODING_FUNCS=("encode_div" "encode_divu" "encode_rem" "encode_remu" "encode_divw" "encode_divuw" "encode_remw" "encode_remuw")
MUL_ENCODING_FUNCS=("encode_mul" "encode_mulh" "encode_mulhsu" "encode_mulhu" "encode_mulw")

echo "除法编码函数:"
FOUND_COUNT=0
for func in "${DIV_ENCODING_FUNCS[@]}"; do
    if grep -q "pub fn $func" vm-frontend/src/riscv64/div.rs; then
        ((FOUND_COUNT++))
    fi
done
echo "  找到: $FOUND_COUNT/8"

echo "乘法编码函数:"
FOUND_COUNT=0
for func in "${MUL_ENCODING_FUNCS[@]}"; do
    if grep -q "pub fn $func" vm-frontend/src/riscv64/mul.rs; then
        ((FOUND_COUNT++))
    fi
done
echo "  找到: $FOUND_COUNT/5"

echo ""
echo "6. 检查测试实现"
echo "------------------------------------------"
DIV_TESTS=$(grep -c "#\[test\]" vm-frontend/src/riscv64/div.rs 2>/dev/null || echo "0")
MUL_TESTS=$(grep -c "#\[test\]" vm-frontend/src/riscv64/mul.rs 2>/dev/null || echo "0")

echo "div.rs 测试: $DIV_TESTS 个"
echo "mul.rs 测试: $MUL_TESTS 个"

echo ""
echo "7. 基础编译检查"
echo "------------------------------------------"
if cargo check -p vm-frontend --lib 2>&1 | grep -q "Finished"; then
    echo -e "${GREEN}✓${NC} vm-frontend 基础编译通过"
else
    echo -e "${RED}✗${NC} vm-frontend 编译失败"
fi

echo ""
echo "=========================================="
echo "验证完成"
echo "=========================================="
