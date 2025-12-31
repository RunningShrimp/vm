#!/bin/bash

# 不依赖 LLVM 的项目编译脚本
# 用于临时解决 LLVM 依赖问题

set -e

echo "🚀 不依赖 LLVM 的项目编译脚本"
echo "=============================="

# 定义不依赖 LLVM 的模块列表
CORE_MODULES=(
    "vm-core"
    "vm-ir" 
    "vm-mem"
    "vm-frontend-x86_64"
    "vm-frontend-arm64"
    "vm-frontend-riscv64"
    "vm-engine-interpreter"
    "vm-accel"
    "vm-device"
    "vm-simd"
    "vm-osal"
    "vm-passthrough"
    "vm-boot"
    "vm-cli"
    "vm-tests"
    "vm-service"
    "vm-monitor"
    "vm-adaptive"
    "vm-gpu"
    "vm-debug"
    "vm-runtime"
    "vm-plugin"
    "vm-codegen"
)

# 依赖 LLVM 的模块（将被排除）
LLVM_MODULES=(
    "vm-ir-lift"
    "vm-engine-jit"
    "aot-builder"
    "vm-cross-arch"
)

echo "📦 将编译的核心模块:"
for module in "${CORE_MODULES[@]}"; do
    echo "  ✓ $module"
done

echo ""
echo "🚫 将排除的 LLVM 依赖模块:"
for module in "${LLVM_MODULES[@]}"; do
    echo "  ✗ $module"
done

echo ""
echo "🔧 开始编译..."

# 构建编译命令
BUILD_CMD="cargo build --workspace"
for module in "${LLVM_MODULES[@]}"; do
    BUILD_CMD="$BUILD_CMD --exclude $module"
done

echo "执行命令: $BUILD_CMD"
echo ""

# 执行编译
if $BUILD_CMD; then
    echo ""
    echo "✅ 编译成功！"
    echo ""
    echo "📝 注意事项:"
    echo "  - JIT 编译功能已禁用"
    echo "  - AOT 编译功能已禁用"
    echo "  - 跨架构优化已禁用"
    echo "  - 指令提升功能已禁用"
    echo ""
    echo "💡 要启用完整功能，请安装 LLVM:"
    echo "  1. 运行: ./install_llvm.sh"
    echo "  2. 或者查看: LLVM_INSTALLATION_GUIDE.md"
    echo ""
    echo "🚀 运行项目:"
    echo "  cargo run --bin vm-cli"
else
    echo ""
    echo "❌ 编译失败！"
    echo ""
    echo "🔍 可能的原因:"
    echo "  1. vm-core 模块有编译错误（与 LLVM 无关）"
    echo "  2. 其他依赖问题"
    echo ""
    echo "📋 调试建议:"
    echo "  1. 检查 vm-core 模块的编译错误"
    echo "  2. 运行: cargo check -p vm-core"
    echo "  3. 修复 vm-core 中的错误后重试"
    exit 1
fi