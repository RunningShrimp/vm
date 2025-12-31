#!/bin/bash

echo "=== 依赖统一分析报告 ==="
echo ""

echo "1. 唯一重复依赖包数量:"
echo "更新前:"
cargo tree --workspace --duplicates 2>&1 | grep -E "^\w+\s+v[0-9]+\.[0-9]+" | awk '{print $2}' | sort -u | wc -l

echo "更新后:"
cargo tree --workspace --duplicates 2>&1 | grep -E "^\w+\s+v[0-9]+\.[0-9]+" | awk '{print $2}' | sort -u | wc -l

echo ""
echo "2. 重复最多的依赖 (Top 10):"
echo "更新前:"
cargo tree --workspace --duplicates 2>&1 | grep -E "^\w+\s+v[0-9]+\.[0-9]+" | awk '{print $1}' | sort | uniq -c | sort -rn | head -10

echo ""
echo "更新后:"
cargo tree --workspace --duplicates 2>&1 | grep -E "^\w+\s+v[0-9]+\.[0-9]+" | awk '{print $1}' | sort | uniq -c | sort -rn | head -10

echo ""
echo "3. 主要重复依赖版本详情:"
echo "hashbrown版本:"
cargo tree --workspace --duplicates 2>&1 | grep -E "^hashbrown\s+v" | awk '{print $2}' | sort -u

echo ""
echo "rand版本:"
cargo tree --workspace --duplicates 2>&1 | grep -E "^rand\s+v" | awk '{print $2}' | sort -u

echo ""
echo "4. Cargo.lock变更统计:"
if [ -f /Users/wangbiao/Desktop/project/vm/Cargo.lock.before_unification ]; then
    echo "Cargo.lock行数变化:"
    echo "更新前: $(wc -l < /Users/wangbiao/Desktop/project/vm/Cargo.lock.before_unification)"
    echo "更新后: $(wc -l < /Users/wangbiao/Desktop/project/vm/Cargo.lock)"
fi
