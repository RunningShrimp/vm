#!/bin/bash

# 跨架构集成测试运行脚本（带超时保护）
# 用于运行所有跨架构集成测试并生成报告

set -e

# 脚本目录
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
WITH_TIMEOUT="${SCRIPT_DIR}/with_timeout.sh"

# 确保 with_timeout.sh 可执行
chmod +x "${WITH_TIMEOUT}" 2>/dev/null || true

# 默认参数
ENABLE_PERFORMANCE_TESTS=true
ENABLE_STRESS_TESTS=false
TIMEOUT=30
VERBOSE=false
OUTPUT=""
HELP=false

# 解析命令行参数
while [[ $# -gt 0 ]]; do
    case $1 in
        --enable-performance-tests)
            ENABLE_PERFORMANCE_TESTS=true
            shift
            ;;
        --disable-performance-tests)
            ENABLE_PERFORMANCE_TESTS=false
            shift
            ;;
        --enable-stress-tests)
            ENABLE_STRESS_TESTS=true
            shift
            ;;
        --disable-stress-tests)
            ENABLE_STRESS_TESTS=false
            shift
            ;;
        --timeout)
            TIMEOUT="$2"
            shift 2
            ;;
        --verbose)
            VERBOSE=true
            shift
            ;;
        --output)
            OUTPUT="$2"
            shift 2
            ;;
        --help)
            HELP=true
            shift
            ;;
        *)
            echo "未知参数: $1"
            HELP=true
            shift
            ;;
    esac
done

# 显示帮助信息
if [ "$HELP" = true ]; then
    echo "跨架构集成测试运行脚本"
    echo ""
    echo "用法: $0 [选项]"
    echo ""
    echo "选项:"
    echo "  --enable-performance-tests   启用性能测试 (默认: 启用)"
    echo "  --disable-performance-tests  禁用性能测试"
    echo "  --enable-stress-tests        启用压力测试 (默认: 禁用)"
    echo "  --disable-stress-tests       禁用压力测试"
    echo "  --timeout <秒>               设置测试超时时间 (默认: 30秒)"
    echo "  --verbose                    启用详细日志"
    echo "  --output <路径>              设置报告输出路径"
    echo "  --help                       显示此帮助信息"
    echo ""
    echo "示例:"
    echo "  $0"
    echo "  $0 --enable-stress-tests --verbose"
    echo "  $0 --timeout 60 --output report.md"
    exit 0
fi

# 构建测试参数
TEST_ARGS=""
if [ "$ENABLE_PERFORMANCE_TESTS" = true ]; then
    TEST_ARGS="$TEST_ARGS --enable-performance-tests"
else
    TEST_ARGS="$TEST_ARGS --disable-performance-tests"
fi

if [ "$ENABLE_STRESS_TESTS" = true ]; then
    TEST_ARGS="$TEST_ARGS --enable-stress-tests"
else
    TEST_ARGS="$TEST_ARGS --disable-stress-tests"
fi

TEST_ARGS="$TEST_ARGS --timeout $TIMEOUT"

if [ "$VERBOSE" = true ]; then
    TEST_ARGS="$TEST_ARGS --verbose"
fi

if [ -n "$OUTPUT" ]; then
    TEST_ARGS="$TEST_ARGS --output $OUTPUT"
fi

# 检查依赖
echo "检查依赖..."

# 检查Rust工具链
if ! command -v cargo &> /dev/null; then
    echo "错误: 未找到cargo，请安装Rust工具链"
    exit 1
fi

# 检查必要的crate
echo "检查必要的crate..."
cd "$PROJECT_ROOT"

# 构建测试（超时10分钟）
echo "构建跨架构集成测试（超时10分钟）..."
"${WITH_TIMEOUT}" 600 cargo build --release -p vm-cross-arch-integration-tests --bin cross_arch_integration_test_runner

if [ $? -ne 0 ]; then
    echo "错误: 构建失败"
    exit 1
fi

# 运行测试（使用配置的超时时间，但至少30分钟）
echo "运行跨架构集成测试..."
echo "参数: $TEST_ARGS"
echo ""

# 创建输出目录
OUTPUT_DIR="$PROJECT_ROOT/test_results"
mkdir -p "$OUTPUT_DIR"

# 运行测试并捕获结果（超时保护）
TEST_START_TIME=$(date +%s)
# 使用更大的超时时间（测试超时时间 + 10分钟缓冲）
TEST_TIMEOUT_WITH_BUFFER=$((TIMEOUT + 600))
"${WITH_TIMEOUT}" $TEST_TIMEOUT_WITH_BUFFER "$PROJECT_ROOT/target/release/cross_arch_integration_test_runner" $TEST_ARGS
TEST_EXIT_CODE=$?
TEST_END_TIME=$(date +%s)
TEST_DURATION=$((TEST_END_TIME - TEST_START_TIME))

# 检查测试结果
if [ $TEST_EXIT_CODE -eq 0 ]; then
    echo ""
    echo "✅ 所有跨架构集成测试通过！"
    echo "⏱️  总耗时: ${TEST_DURATION}秒"
else
    echo ""
    echo "❌ 跨架构集成测试失败！"
    echo "⏱️  总耗时: ${TEST_DURATION}秒"
    exit $TEST_EXIT_CODE
fi

# 如果指定了输出文件，显示文件位置
if [ -n "$OUTPUT" ]; then
    echo ""
    echo "📄 测试报告已保存到: $OUTPUT"
fi

echo ""
echo "🎉 跨架构集成测试完成！"