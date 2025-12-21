#!/bin/bash
# 安全的 Cargo 构建脚本（带超时保护）

set -e

# 默认超时时间（秒）
DEFAULT_TIMEOUT=600  # 10分钟
RELEASE_BUILD_TIMEOUT=1800  # 30分钟

# 获取脚本目录
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WITH_TIMEOUT="${SCRIPT_DIR}/with_timeout.sh"

# 确保 with_timeout.sh 可执行
chmod +x "${WITH_TIMEOUT}"

# 解析参数
TIMEOUT=$DEFAULT_TIMEOUT
CARGO_ARGS=()

while [[ $# -gt 0 ]]; do
    case $1 in
        --release)
            TIMEOUT=$RELEASE_BUILD_TIMEOUT
            CARGO_ARGS+=("$1")
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
echo "运行 Cargo 构建（超时: ${TIMEOUT}秒）"
echo "=========================================="

# 执行构建
"${WITH_TIMEOUT}" $TIMEOUT cargo build "${CARGO_ARGS[@]}"

EXIT_CODE=$?

if [ $EXIT_CODE -eq 124 ]; then
    echo ""
    echo "=========================================="
    echo "构建超时！"
    echo "=========================================="
    echo "构建运行时间超过了 ${TIMEOUT} 秒"
    echo "可能的原因："
    echo "  1. 依赖下载缓慢"
    echo "  2. 编译时间过长"
    echo "  3. 系统资源不足"
    echo ""
    echo "建议："
    echo "  - 使用更长的超时时间: --timeout <秒数>"
    echo "  - 检查网络连接"
    echo "  - 检查系统资源使用情况"
    exit 124
fi

exit $EXIT_CODE

