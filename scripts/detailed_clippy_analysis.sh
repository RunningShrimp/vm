#!/bin/bash

echo "=== 详细Clippy警告分析 ==="
echo "时间: $(date)"
echo "工作目录: $(pwd)"
echo ""

# 创建临时文件
TEMP_FILE=$(mktemp)
DETAILS_FILE=$(mktemp)

# 运行clippy并获取详细信息
echo "正在运行Clippy检查..."
cargo clippy --all-targets -- -W clippy::all 2>&1 | tee clippy_full_output.log

# 提取所有警告
echo ""
echo "=== 收集所有警告 ==="
grep -E "warning:" clippy_full_output.log | \
    grep -v "^warning: build failed" | \
    grep -v "^warning: could not compile" | \
    sort > "$TEMP_FILE"

TOTAL=$(wc -l < "$TEMP_FILE")
echo "总计: $TOTAL 个警告"
echo ""

# 显示所有警告
echo "=== 警告详情 ==="
cat -n "$TEMP_FILE"

# 按优先级分类
echo ""
echo "=== 按优先级分类 ==="

# P0 - 高优先级（安全、性能、复杂度）
P0_COUNT=$(grep -E -i "(unsafe|ptr|memory|buffer|overflow|performance|slow|alloc|clone|copy|complex|long|deep|nested)" "$TEMP_FILE" | wc -l)
echo "P0 - 高优先级（安全、性能、复杂度）: $P0_COUNT"
echo ""

# P1 - 中优先级（风格、可读性）
P1_COUNT=$(grep -E -i "(style|naming|convention|unused|dead|read|write|mut)" "$TEMP_FILE" | wc -l)
echo "P1 - 中优先级（风格、可读性）: $P1_COUNT"
echo ""

# P2 - 低优先级（建议、pedantic）
P2_COUNT=$(grep -E -i "(acronym|postfix|collapse|else.*if)" "$TEMP_FILE" | wc -l)
echo "P2 - 低优先级（建议、pedantic）: $P2_COUNT"
echo ""

# 显示前20个高优先级警告
echo "=== 前20个警告详情 ==="
head -20 "$TEMP_FILE" | nl

# 清理
rm -f "$TEMP_FILE" "$DETAILS_FILE"
echo ""
echo "详细输出已保存到: clippy_full_output.log"