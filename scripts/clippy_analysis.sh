#!/bin/bash

# 创建临时文件存储所有警告
TEMP_FILE=$(mktemp)

echo "正在运行Clippy检查..."

# 运行clippy并收集所有警告
cargo clippy --all-targets -- -W clippy::all 2>&1 | \
    grep -E "warning:" | \
    grep -v "^warning: build failed" | \
    sed 's/^warning: //' | \
    sort > "$TEMP_FILE"

# 统计总数
TOTAL_WARNINGS=$(wc -l < "$TEMP_FILE")
echo "总共发现 $TOTAL_WARNINGS 个Clippy警告"
echo ""

# 显示所有警告
echo "=== 所有Clippy警告 ==="
cat "$TEMP_FILE"

# 分类统计
echo ""
echo "=== 按类型分类统计 ==="

# 性能相关警告
PERFORMANCE_WARNINGS=$(grep -E -i "(performance|slow|alloc|clone|copy)" "$TEMP_FILE" | wc -l)
echo "性能相关警告: $PERFORMANCE_WARNINGS"

# 安全相关警告
SECURITY_WARNINGS=$(grep -E -i "(unsafe|ptr|memory|buffer|overflow)" "$TEMP_FILE" | wc -l)
echo "安全相关警告: $SECURITY_WARNINGS"

# 代码风格警告
STYLE_WARNINGS=$(grep -E -i "(style|naming|convention|unused|dead)" "$TEMP_FILE" | wc -l)
echo "代码风格警告: $STYLE_WARNINGS"

# 复杂度相关警告
COMPLEXITY_WARNINGS=$(grep -E -i "(complex|long|deep|nested)" "$TEMP_FILE" | wc -l)
echo "复杂度相关警告: $COMPLEXITY_WARNINGS"

# 清理临时文件
rm -f "$TEMP_FILE"