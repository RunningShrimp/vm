#!/bin/bash
# TODO/FIXME标记扫描脚本
# 扫描项目中所有TODO、FIXME、XXX、HACK、BUG标记并分类

set -e

OUTPUT_FILE="TODO_SCAN_RESULTS.md"
TEMP_FILE=$(mktemp)

echo "# TODO/FIXME标记扫描结果" > "$OUTPUT_FILE"
echo "" >> "$OUTPUT_FILE"
echo "扫描时间: $(date)" >> "$OUTPUT_FILE"
echo "" >> "$OUTPUT_FILE"

# 扫描所有TODO/FIXME标记
echo "## 所有标记列表" >> "$OUTPUT_FILE"
echo "" >> "$OUTPUT_FILE"

# 使用grep查找所有TODO/FIXME/XXX/HACK/BUG标记
grep -rn "TODO\|FIXME\|XXX\|HACK\|BUG" --include="*.rs" --include="*.md" . | grep -v "target/" | grep -v ".git/" | sort > "$TEMP_FILE"

# 统计总数
TOTAL_COUNT=$(wc -l < "$TEMP_FILE")
echo "**总计: $TOTAL_COUNT 个标记**" >> "$OUTPUT_FILE"
echo "" >> "$OUTPUT_FILE"

# 按文件分组
echo "### 按文件分组" >> "$OUTPUT_FILE"
echo "" >> "$OUTPUT_FILE"

current_file=""
while IFS= read -r line; do
    file=$(echo "$line" | cut -d: -f1)
    line_num=$(echo "$line" | cut -d: -f2)
    content=$(echo "$line" | cut -d: -f3-)
    
    if [ "$file" != "$current_file" ]; then
        if [ -n "$current_file" ]; then
            echo "" >> "$OUTPUT_FILE"
        fi
        echo "#### $file" >> "$OUTPUT_FILE"
        current_file="$file"
    fi
    
    echo "- **行 $line_num**: $content" >> "$OUTPUT_FILE"
done < "$TEMP_FILE"

echo "" >> "$OUTPUT_FILE"
echo "## 按类型统计" >> "$OUTPUT_FILE"
echo "" >> "$OUTPUT_FILE"

TODO_COUNT=$(grep -c "TODO" "$TEMP_FILE" || echo "0")
FIXME_COUNT=$(grep -c "FIXME" "$TEMP_FILE" || echo "0")
XXX_COUNT=$(grep -c "XXX" "$TEMP_FILE" || echo "0")
HACK_COUNT=$(grep -c "HACK" "$TEMP_FILE" || echo "0")
BUG_COUNT=$(grep -c "BUG" "$TEMP_FILE" || echo "0")

echo "- TODO: $TODO_COUNT" >> "$OUTPUT_FILE"
echo "- FIXME: $FIXME_COUNT" >> "$OUTPUT_FILE"
echo "- XXX: $XXX_COUNT" >> "$OUTPUT_FILE"
echo "- HACK: $HACK_COUNT" >> "$OUTPUT_FILE"
echo "- BUG: $BUG_COUNT" >> "$OUTPUT_FILE"

rm "$TEMP_FILE"

echo "扫描完成，结果保存在: $OUTPUT_FILE"

