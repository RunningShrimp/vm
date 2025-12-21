#!/bin/bash

# FVP虚拟机系统自动化测试脚本（带超时保护）
# 用于本地开发和CI/CD流水线

set -e

# 获取脚本目录
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WITH_TIMEOUT="${SCRIPT_DIR}/with_timeout.sh"

# 确保 with_timeout.sh 可执行
chmod +x "${WITH_TIMEOUT}" 2>/dev/null || true

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 打印函数
print_header() {
    echo -e "${BLUE}=====================================${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}=====================================${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️ $1${NC}"
}

print_info() {
    echo -e "${BLUE}ℹ️ $1${NC}"
}

# 检查命令是否存在
check_command() {
    if ! command -v $1 &> /dev/null; then
        print_error "$1 命令未找到，请先安装"
        exit 1
    fi
}

# 清理函数
cleanup() {
    print_info "清理临时文件..."
    rm -f /tmp/fvp-test-*
}

# 设置信号处理
trap cleanup EXIT

# 解析命令行参数
VERBOSE=false
COVERAGE=false
BENCH=false
INTEGRATION=false
PERFORMANCE=false
ALL=false

show_help() {
    echo "用法: $0 [选项]"
    echo ""
    echo "选项:"
    echo "  -v, --verbose      详细输出"
    echo "  -c, --coverage     生成代码覆盖率报告"
    echo "  -b, --bench        运行性能基准测试"
    echo "  -i, --integration  运行集成测试"
    echo "  -p, --performance  运行性能测试"
    echo "  -a, --all          运行所有测试"
    echo "  -h, --help         显示此帮助信息"
    echo ""
    echo "示例:"
    echo "  $0 -v                    # 详细模式运行基础测试"
    echo "  $0 -a                    # 运行所有测试"
    echo "  $0 -c -i                 # 运行基础测试和集成测试，并生成覆盖率"
}

while [[ $# -gt 0 ]]; do
    case $1 in
        -v|--verbose)
            VERBOSE=true
            shift
            ;;
        -c|--coverage)
            COVERAGE=true
            shift
            ;;
        -b|--bench)
            BENCH=true
            shift
            ;;
        -i|--integration)
            INTEGRATION=true
            shift
            ;;
        -p|--performance)
            PERFORMANCE=true
            shift
            ;;
        -a|--all)
            ALL=true
            shift
            ;;
        -h|--help)
            show_help
            exit 0
            ;;
        *)
            print_error "未知选项: $1"
            show_help
            exit 1
            ;;
    esac
done

# 如果指定了--all，启用所有测试
if [ "$ALL" = true ]; then
    COVERAGE=true
    BENCH=true
    INTEGRATION=true
    PERFORMANCE=true
fi

# 检查必要的命令
check_command "cargo"
check_command "rustc"

# 设置环境变量
export RUST_BACKTRACE=1
if [ "$VERBOSE" = true ]; then
    export RUST_LOG=debug
fi

print_header "FVP虚拟机系统自动化测试"
print_info "测试开始时间: $(date)"

# 创建测试结果目录
TEST_RESULTS_DIR="test-results"
mkdir -p "$TEST_RESULTS_DIR"

# 函数：运行单元测试
run_unit_tests() {
    print_header "运行单元测试"

    local test_args=""
    if [ "$VERBOSE" = true ]; then
        test_args="-- --nocapture"
    fi

    if "${WITH_TIMEOUT}" 300 cargo test $test_args --all-features --lib; then
        print_success "单元测试通过"
        return 0
    else
        print_error "单元测试失败"
        return 1
    fi
}

# 函数：运行文档测试
run_doc_tests() {
    print_header "运行文档测试"

    if "${WITH_TIMEOUT}" 180 cargo test --all-features --doc; then
        print_success "文档测试通过"
        return 0
    else
        print_error "文档测试失败"
        return 1
    fi
}

# 函数：生成覆盖率报告
generate_coverage() {
    if [ "$COVERAGE" != true ]; then
        return 0
    fi

    print_header "生成代码覆盖率报告"

    # 检查是否安装了cargo-llvm-cov
    if ! command -v cargo-llvm-cov &> /dev/null; then
        print_warning "cargo-llvm-cov 未安装，跳过覆盖率生成"
        print_info "安装命令: cargo install cargo-llvm-cov"
        return 0
    fi

    local coverage_dir="$TEST_RESULTS_DIR/coverage"
    mkdir -p "$coverage_dir"

    if "${WITH_TIMEOUT}" 1800 cargo llvm-cov --all-features --workspace --html --output-dir "$coverage_dir"; then
        print_success "覆盖率报告生成成功: $coverage_dir/index.html"

        # 生成文本摘要（超时5分钟）
        "${WITH_TIMEOUT}" 300 cargo llvm-cov --all-features --workspace --summary > "$coverage_dir/coverage-summary.txt"
        print_info "覆盖率摘要:"
        cat "$coverage_dir/coverage-summary.txt"
        return 0
    else
        print_error "覆盖率报告生成失败"
        return 1
    fi
}

# 函数：运行集成测试
run_integration_tests() {
    if [ "$INTEGRATION" != true ]; then
        return 0
    fi

    print_header "运行集成测试"

    local test_args=""
    if [ "$VERBOSE" = true ]; then
        test_args="-- --nocapture"
    else
        test_args=""
    fi
    # 移除 --test-threads=1 限制，允许并行执行测试以提升速度
    # 如果某些测试需要串行执行，应在测试代码中使用适当的同步机制

    # 构建项目（超时10分钟）
    print_info "构建项目（超时10分钟）..."
    if ! "${WITH_TIMEOUT}" 600 cargo build --release --all-features; then
        print_error "项目构建失败"
        return 1
    fi

    # 运行集成测试（超时10分钟）
    if "${WITH_TIMEOUT}" 600 cargo test --release --package vm-tests --test integration $test_args; then
        print_success "集成测试通过"
        return 0
    else
        print_error "集成测试失败"
        return 1
    fi
}

# 函数：运行性能测试
run_performance_tests() {
    if [ "$PERFORMANCE" != true ]; then
        return 0
    fi

    print_header "运行性能测试"

    # 构建优化版本（超时10分钟）
    print_info "构建性能优化版本（超时10分钟）..."
    if ! "${WITH_TIMEOUT}" 600 cargo build --release --all-features; then
        print_error "性能版本构建失败"
        return 1
    fi

    # 运行JIT性能测试（超时5分钟）
    print_info "运行JIT性能测试（超时5分钟）..."
    if "${WITH_TIMEOUT}" 300 cargo test --release --package vm-tests --test jit_performance_tests -- --nocapture; then
        print_success "JIT性能测试通过"
    else
        print_error "JIT性能测试失败"
        return 1
    fi

    # 运行TLB性能测试（超时5分钟）
    print_info "运行TLB性能测试（超时5分钟）..."
    if "${WITH_TIMEOUT}" 300 cargo test --release --package vm-tests --test tlb_performance_tests -- --nocapture; then
        print_success "TLB性能测试通过"
    else
        print_error "TLB性能测试失败"
        return 1
    fi

    # 运行系统性能测试（超时5分钟）
    print_info "运行系统性能测试（超时5分钟）..."
    if "${WITH_TIMEOUT}" 300 cargo test --release --package vm-tests --test system_performance_tests -- --nocapture; then
        print_success "系统性能测试通过"
    else
        print_error "系统性能测试失败"
        return 1
    fi

    return 0
}

# 函数：运行基准测试
run_benchmarks() {
    if [ "$BENCH" != true ]; then
        return 0
    fi

    print_header "运行性能基准测试"

    # 检查是否安装了cargo-criterion
    if ! command -v cargo-criterion &> /dev/null; then
        print_warning "cargo-criterion 未安装，跳过基准测试"
        print_info "安装命令: cargo install cargo-criterion"
        return 0
    fi

    local bench_dir="$TEST_RESULTS_DIR/benchmarks"
    mkdir -p "$bench_dir"

    # 运行基准测试（超时30分钟）
    if "${WITH_TIMEOUT}" 1800 cargo bench --all-features -- --output-format html; then
        print_success "基准测试完成"
        print_info "基准测试报告: target/criterion/report/index.html"
        return 0
    else
        print_error "基准测试失败"
        return 1
    fi
}

# 函数：运行代码质量检查
run_quality_checks() {
    print_header "运行代码质量检查"

    # 代码格式检查（超时2分钟）
    print_info "检查代码格式（超时2分钟）..."
    if "${WITH_TIMEOUT}" 120 cargo fmt --all -- --check; then
        print_success "代码格式检查通过"
    else
        print_error "代码格式检查失败，请运行 'cargo fmt' 修复"
        return 1
    fi

    # Clippy检查（超时10分钟）
    print_info "运行Clippy检查（超时10分钟）..."
    if "${WITH_TIMEOUT}" 600 cargo clippy --all-targets --all-features -- -D warnings; then
        print_success "Clippy检查通过"
    else
        print_error "Clippy检查失败"
        return 1
    fi

    return 0
}

# 函数：生成测试报告
generate_test_report() {
    local report_file="$TEST_RESULTS_DIR/test-report.md"

    cat > "$report_file" << EOF
# FVP虚拟机系统测试报告

## 测试概览

- **测试时间**: $(date)
- **测试环境**: $(rustc --version)
- **操作系统**: $(uname -s)

## 测试结果

EOF

    # 添加测试结果到报告
    echo "## 测试执行完成" >> "$report_file"
    echo "- 测试结果保存在: $TEST_RESULTS_DIR/" >> "$report_file"
    echo "- 详细日志请查看测试输出" >> "$report_file"

    print_success "测试报告生成: $report_file"
}

# 主测试流程
main() {
    local failed_tests=0

    # 代码质量检查
    if ! run_quality_checks; then
        ((failed_tests++))
    fi

    # 单元测试
    if ! run_unit_tests; then
        ((failed_tests++))
    fi

    # 文档测试
    if ! run_doc_tests; then
        ((failed_tests++))
    fi

    # 集成测试
    if ! run_integration_tests; then
        ((failed_tests++))
    fi

    # 性能测试
    if ! run_performance_tests; then
        ((failed_tests++))
    fi

    # 基准测试
    if ! run_benchmarks; then
        ((failed_tests++))
    fi

    # 覆盖率报告
    generate_coverage

    # 生成测试报告
    generate_test_report

    # 最终结果
    print_header "测试完成"
    if [ $failed_tests -eq 0 ]; then
        print_success "所有测试通过！ 🎉"
        exit 0
    else
        print_error "有 $failed_tests 个测试失败"
        exit 1
    fi
}

# 执行主函数
main "$@"