#!/bin/bash

# 修复vm-mem中的AccessViolation字段名问题
# 将所有使用 access 的地方改为 access_type

find . -name "*.rs" -path "*/src/*" | while read file; do
  if [ "$file" != "./fix_vm_mem_errors.sh" ]; then
    echo "Processing $file..."
    # 检查是否有 AccessViolation { 的使用
    if grep -q "AccessViolation {" "$file" 2>/dev/null; then
      # 查找下一行的 access: 并改为 access_type:
      sed -i.bak1 '/AccessViolation {$/,/access:/{ /g; }' "$file"
      echo "Fixed $file"
    fi
  fi
done

echo "Done!"
