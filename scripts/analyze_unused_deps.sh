#!/bin/bash
# 分析未使用的依赖
# 这是一个简单的静态分析工具

echo "=== VM项目依赖使用分析 ==="
echo ""
echo "分析时间: $(date)"
echo ""

# 检查各个crate的依赖
for crate_dir in vm-*/; do
    if [ -d "$crate_dir" ]; then
        crate_name=$(basename "$crate_dir")
        cargo_file="${crate_dir}Cargo.toml"

        if [ -f "$cargo_file" ]; then
            echo "=== $crate_name ==="

            # 检查dependencies部分
            if grep -q "\[dependencies\]" "$cargo_file"; then
                # 统计依赖数量
                dep_count=$(awk '/^\[dependencies\]/,/\[/{if (!/^\[dependencies\]/ && !/^\[/ && /=/) print}' "$cargo_file" | wc -l | tr -d ' ')
                echo "  直接依赖: $dep_count"
            fi

            # 检查dev-dependencies
            if grep -q "\[dev-dependencies\]" "$cargo_file"; then
                dev_dep_count=$(awk '/^\[dev-dependencies\]/,/\[/{if (!/^\[dev-dependencies\]/ && !/^\[/ && /=/) print}' "$cargo_file" | wc -l | tr -d ' ')
                echo "  开发依赖: $dev_dep_count"
            fi

            # 检查build-dependencies
            if grep -q "\[build-dependencies\]" "$cargo_file"; then
                build_dep_count=$(awk '/^\[build-dependencies\]/,/\[/{if (!/^\[build-dependencies\]/ && !/^\[/ && /=/) print}' "$cargo_file" | wc -l | tr -d ' ')
                echo "  构建依赖: $build_dep_count"
            fi

            echo ""
        fi
    fi
done

echo "=== 总体统计 ==="
total_crates=$(find . -name "Cargo.toml" -not -path "./target/*" | wc -l | tr -d ' ')
echo "总crate数: $total_crates"
echo ""

# 检查workspace依赖统一性
echo "=== Workspace依赖使用检查 ==="
echo "检查各个crate是否使用workspace依赖..."

non_workspace_deps=$(grep -h "dependencies.*version.*=" vm-*/Cargo.toml | grep -v "workspace = true" | grep -v "path =" | grep -v "vm-" | wc -l | tr -d ' ')

echo "非workspace依赖实例: $non_workspace_deps"
echo ""

if [ "$non_workspace_deps" -gt 0 ]; then
    echo "⚠️  发现可能未使用workspace的依赖:"
    grep -h "dependencies.*version.*=" vm-*/Cargo.toml | grep -v "workspace = true" | grep -v "path =" | grep -v "vm-" | head -10
    echo ""
fi

echo "=== 分析完成 ==="
echo ""
echo "建议:"
echo "1. 使用 'cargo machete' 进行更深入的未使用依赖分析"
echo "2. 使用 'cargo tree' 检查依赖树"
echo "3. 使用 'cargo +nightly udeps' 查找未使用的依赖"
