#!/bin/bash
# 批量添加vm-build-deps依赖到所有workspace crates

set -e

# 需要添加vm-build-deps的crate列表
crates=(
    "parallel-jit"
    "perf-bench"
    "security-sandbox"
    "syscall-compat"
    "tiered-compiler"
    "vm-accel"
    "vm-boot"
    "vm-cli"
    "vm-codegen"
    "vm-core"
    "vm-cross-arch-support"
    "vm-debug"
    "vm-desktop"
    "vm-device"
    "vm-engine"
    "vm-engine-jit"
    "vm-frontend"
    "vm-gc"
    "vm-graphics"
    "vm-ir"
    "vm-mem"
    "vm-monitor"
    "vm-optimizers"
    "vm-osal"
    "vm-passthrough"
    "vm-platform"
    "vm-plugin"
    "vm-service"
    "vm-smmu"
    "vm-soc"
)

for crate in "${crates[@]}"; do
    cargo_file="$crate/Cargo.toml"

    if [ -f "$cargo_file" ]; then
        # 检查是否已经有vm-build-deps依赖
        if grep -q "vm-build-deps" "$cargo_file"; then
            echo "✓ $crate already has vm-build-deps dependency"
        else
            # 在[dependencies]部分后添加vm-build-deps
            # 使用sed来添加
            sed -i.bak '/^\[dependencies\]/a\
# Hakari-managed dependencies\
vm-build-deps = { path = "../vm-build-deps" }
' "$cargo_file"

            echo "✓ Added vm-build-deps to $crate"

            # 删除备份文件
            rm -f "$cargo_file.bak"
        fi
    else
        echo "⚠️  Warning: $cargo_file not found"
    fi
done

echo ""
echo "✅ All crates updated!"
echo ""
echo "Run 'cargo hakari verify' to verify the configuration."
