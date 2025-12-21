#!/bin/bash
# 安全的 Cargo 测试脚本（带超时保护）
#
# 为不同类型的测试设置不同的超时时间，防止测试卡死

set -e

# 默认超时时间（秒）
DEFAULT_TIMEOUT=300  # 5分钟
UNIT_TEST_TIMEOUT=60  # 1分钟
INTEGRATION_TEST_TIMEOUT=180  # 3分钟
PERFORMANCE_TEST_TIMEOUT=300  # 5分钟
CONCURRENCY_TEST_TIMEOUT=600  # 10分钟
FULL_TEST_TIMEOUT=1800  # 30分钟

# 获取脚本目录
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WITH_TIMEOUT="${SCRIPT_DIR}/with_timeout.sh"

# 确保 with_timeout.sh 可执行
chmod +x "${WITH_TIMEOUT}"

# 解析参数
TEST_TYPE="default"
TIMEOUT=$DEFAULT_TIMEOUT
CARGO_ARGS=()

while [[ $# -gt 0 ]]; do
    case $1 in
        --unit)
            TEST_TYPE="unit"
            TIMEOUT=$UNIT_TEST_TIMEOUT
            shift
            ;;
        --integration)
            TEST_TYPE="integration"
            TIMEOUT=$INTEGRATION_TEST_TIMEOUT
            shift
            ;;
        --performance)
            TEST_TYPE="performance"
            TIMEOUT=$PERFORMANCE_TEST_TIMEOUT
            shift
            ;;
        --concurrency)
            TEST_TYPE="concurrency"
            TIMEOUT=$CONCURRENCY_TEST_TIMEOUT
            shift
            ;;
        --full)
            TEST_TYPE="full"
            TIMEOUT=$FULL_TEST_TIMEOUT
            shift
            ;;
        --timeout)
            TIMEOUT="$2"
            shift 2
            ;;
        *)
            CARGO_ARGS+=("$1")
            shift
            ;;
    esac
done

echo "=========================================="
echo "运行 Cargo 测试（类型: $TEST_TYPE, 超时: ${TIMEOUT}秒）"
echo "=========================================="

# 执行测试
"${WITH_TIMEOUT}" $TIMEOUT cargo test "${CARGO_ARGS[@]}"

EXIT_CODE=$?

if [ $EXIT_CODE -eq 124 ]; then
    echo ""
    echo "=========================================="
    echo "测试超时！"
    echo "=========================================="
    echo "测试运行时间超过了 ${TIMEOUT} 秒"
    echo "可能的原因："
    echo "  1. 死锁或资源竞争"
    echo "  2. 无限循环"
    echo "  3. 测试超时配置不合理"
    echo ""
    echo "建议："
    echo "  - 检查测试日志"
    echo "  - 使用更长的超时时间: --timeout <秒数>"
    echo "  - 检查是否有并发问题"
    exit 124
fi

exit $EXIT_CODE

