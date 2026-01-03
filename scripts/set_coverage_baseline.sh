#!/bin/bash
# 设置覆盖率baseline
# 用于追踪覆盖率趋势和防止回归

set -e

echo "========================================"
echo "设置覆盖率Baseline"
echo "========================================"
echo ""

# 检查cargo-llvm-cov是否安装
if ! command -v cargo-llvm-cov &> /dev/null; then
    echo "❌ cargo-llvm-cov未安装"
    echo "   正在安装..."
    cargo install cargo-llvm-cov
fi

# 生成覆盖率
echo "1. 生成覆盖率报告..."
cargo llvm-cov --workspace --all-features --summary --output-file /tmp/coverage-summary.txt

# 提取覆盖率百分比
COVERAGE=$(grep -oP '\d+\.\d+%' /tmp/coverage-summary.txt | head -1 | tr -d '%')

echo ""
echo "✅ 覆盖率报告生成完成"
echo "   当前覆盖率: ${COVERAGE}%"
echo ""

# 保存baseline
mkdir -p .github/baselines
echo "${COVERAGE}" > .github/baselines/coverage.txt

echo "2. 保存baseline..."
echo "   Baseline已保存到: .github/baselines/coverage.txt"
echo ""

# 生成报告
echo "========================================"
echo "覆盖率Baseline报告"
echo "========================================"
echo ""
echo "覆盖率: ${COVERAGE}%"
echo ""

# 评估覆盖率水平
if [ $(echo "$COVERAGE > 80" | bc -l) -eq 1 ]; then
    echo "✅ 覆盖率优秀 (>80%)"
    STATUS="excellent"
elif [ $(echo "$COVERAGE > 60" | bc -l) -eq 1 ]; then
    echo "📊 覆盖率良好 (>60%)"
    STATUS="good"
elif [ $(echo "$COVERAGE > 40" | bc -l) -eq 1 ]; then
    echo "⚠️ 覆盖率一般 (>40%)"
    STATUS="fair"
else
    echo "❌ 覆盖率较低 (<40%)"
    STATUS="poor"
fi

echo ""
echo "说明:"
echo "  - 此baseline将用于检测覆盖率回归"
echo "  - 如果覆盖率下降超过5%，CI会警告"
echo "  - 建议在达到80%覆盖率后更新baseline"
echo ""
echo "更新方法:"
echo "  bash scripts/set_coverage_baseline.sh"
echo ""
echo "========================================"

# 生成详细报告
echo ""
echo "是否查看详细覆盖率报告? (y/N) "
read -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    cargo llvm-cov --workspace --all-features --open
fi

# 可选：提交baseline
echo ""
read -p "是否提交baseline到Git? (y/N) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    git add .github/baselines/coverage.txt
    git commit -m "chore: 更新覆盖率baseline为${COVERAGE}% (status: $STATUS)"
    echo "✅ Baseline已提交"
else
    echo "⚠️  Baseline未提交，请手动提交"
fi

echo ""
echo "完成！"
