#!/bin/bash
# 设置构建时间baseline
# 用于追踪构建性能回归

set -e

echo "========================================"
echo "设置构建时间Baseline"
echo "========================================"
echo ""

# 清理
echo "1. 清理构建产物..."
cargo clean

# 构建并计时
echo "2. 执行完整构建..."
START_TIME=$(date +%s)
cargo build --workspace
END_TIME=$(date +%s)

BUILD_TIME=$((END_TIME - START_TIME))

echo ""
echo "✅ 构建完成"
echo "   构建时间: ${BUILD_TIME}s"
echo ""

# 保存baseline
mkdir -p .github/baselines
echo "${BUILD_TIME}" > .github/baselines/build-time.txt

echo "3. 保存baseline..."
echo "   Baseline已保存到: .github/baselines/build-time.txt"
echo ""

# 生成报告
echo "========================================"
echo "构建时间Baseline报告"
echo "========================================"
echo ""
echo "构建时间: ${BUILD_TIME}s"
echo ""
echo "说明:"
echo "  - 此baseline将用于检测性能回归"
echo "  - 如果构建时间增加超过5%，CI会警告"
echo "  - 建议在稳定版本上更新baseline"
echo ""
echo "更新方法:"
echo "  bash scripts/set_build_baseline.sh"
echo ""
echo "========================================"

# 可选：提交baseline
echo ""
read -p "是否提交baseline到Git? (y/N) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    git add .github/baselines/build-time.txt
    git commit -m "chore: 更新构建时间baseline为${BUILD_TIME}s"
    echo "✅ Baseline已提交"
else
    echo "⚠️  Baseline未提交，请手动提交"
fi

echo ""
echo "完成！"
