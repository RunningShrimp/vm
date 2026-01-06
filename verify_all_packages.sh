#!/bin/bash

echo "========================================="
echo "全面验证30个核心包 - 0 Warning 0 Error"
echo "========================================="
echo ""

PASS=0
FAIL=0

check_package() {
    local pkg=$1
    echo -n "[$pkg] "
    if cargo clippy -p $pkg -- -D warnings 2>&1 | grep -q "Finished"; then
        echo "✅ PASS"
        ((PASS++))
    else
        echo "❌ FAIL"
        ((FAIL++))
    fi
}

echo "=== 核心基础设施 (6/6) ==="
check_package "vm-core"
check_package "vm-ir"
check_package "vm-mem"
check_package "vm-cross-arch-support"
check_package "vm-device"
check_package "vm-accel"
echo ""

echo "=== 执行引擎 (3/3) ==="
check_package "vm-engine"
check_package "vm-optimizers"
check_package "vm-gc"
echo ""

echo "=== 服务与平台 (9/9) ==="
check_package "vm-service"
check_package "vm-frontend"
check_package "vm-boot"
check_package "vm-platform"
check_package "vm-smmu"
check_package "vm-passthrough"
check_package "vm-soc"
check_package "vm-graphics"
check_package "vm-plugin"
echo ""

echo "=== 扩展与工具 (8/8) ==="
check_package "vm-osal"
check_package "vm-codegen"
check_package "vm-cli"
check_package "vm-monitor"
check_package "vm-debug"
check_package "vm-desktop"
echo ""

echo "=== 外部兼容性 (2/2) ==="
check_package "security-sandbox"
check_package "syscall-compat"
echo ""

echo "=== 性能基准测试 (4/4) ==="
check_package "perf-bench"
check_package "tiered-compiler"
check_package "parallel-jit"
check_package "vm-build-deps"
echo ""

echo "========================================="
echo "验证结果汇总:"
echo "  通过: $PASS/30"
echo "  失败: $FAIL/30"
echo "========================================="

if [ $FAIL -eq 0 ]; then
    echo "✅ 所有30个核心包全部达到 0 Warning 0 Error！"
    exit 0
else
    echo "❌ 有 $FAIL 个包未通过"
    exit 1
fi
