#!/bin/bash
# 代码覆盖率生成脚本
# 支持 cargo-llvm-cov 和 cargo-tarpaulin

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_info() {
    echo -e "${BLUE}ℹ${NC} $1"
}

print_success() {
    echo -e "${GREEN}✓${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}⚠${NC} $1"
}

print_error() {
    echo -e "${RED}✗${NC} $1"
}

# 默认使用 llvm-cov
TOOL="${COVERAGE_TOOL:-llvm-cov}"
OUTPUT_DIR="${COVERAGE_OUTPUT_DIR:-test-results/coverage}"
SUMMARY_FILE="${OUTPUT_DIR}/coverage-summary.txt"

# 检查工具是否安装
check_tool() {
    case "$TOOL" in
        llvm-cov)
            if ! command -v cargo-llvm-cov &> /dev/null; then
                print_error "cargo-llvm-cov 未安装"
                print_info "安装命令: cargo install cargo-llvm-cov --locked"
                exit 1
            fi
            ;;
        tarpaulin)
            if ! command -v cargo-tarpaulin &> /dev/null; then
                print_error "cargo-tarpaulin 未安装"
                print_info "安装命令: cargo install cargo-tarpaulin"
                exit 1
            fi
            ;;
        *)
            print_error "未知的覆盖率工具: $TOOL"
            print_info "支持的工具: llvm-cov, tarpaulin"
            exit 1
            ;;
    esac
}

# 生成覆盖率报告
generate_coverage() {
    print_info "使用工具: $TOOL"
    print_info "输出目录: $OUTPUT_DIR"
    
    mkdir -p "$OUTPUT_DIR"
    
    case "$TOOL" in
        llvm-cov)
            print_info "运行 cargo llvm-cov..."
            if cargo llvm-cov \
                --all-features \
                --workspace \
                --html \
                --output-dir "$OUTPUT_DIR" \
                --lcov \
                --output-path "$OUTPUT_DIR/lcov.info" \
                --summary > "$SUMMARY_FILE" 2>&1; then
                print_success "覆盖率报告生成成功"
                print_info "HTML报告: $OUTPUT_DIR/index.html"
                print_info "LCOV报告: $OUTPUT_DIR/lcov.info"
                
                # 显示摘要
                print_info "覆盖率摘要:"
                cat "$SUMMARY_FILE"
                
                # 生成HTML报告（如果Python脚本可用）
                if command -v python3 &> /dev/null && [ -f "scripts/generate-coverage-report.py" ]; then
                    print_info "生成增强HTML报告..."
                    python3 scripts/generate-coverage-report.py \
                        --coverage-summary "$SUMMARY_FILE" \
                        --output "$OUTPUT_DIR/coverage-report.html" || true
                fi
                
                return 0
            else
                print_error "覆盖率报告生成失败"
                return 1
            fi
            ;;
        tarpaulin)
            print_info "运行 cargo tarpaulin..."
            if cargo tarpaulin \
                --all-features \
                --workspace \
                --out Html \
                --out Xml \
                --out Stdout \
                --output-dir "$OUTPUT_DIR" \
                --timeout 300; then
                print_success "覆盖率报告生成成功"
                print_info "HTML报告: $OUTPUT_DIR/tarpaulin-report.html"
                print_info "XML报告: $OUTPUT_DIR/cobertura.xml"
                return 0
            else
                print_error "覆盖率报告生成失败"
                return 1
            fi
            ;;
    esac
}

# 主函数
main() {
    print_info "开始生成代码覆盖率报告..."
    
    check_tool
    generate_coverage
    
    print_success "覆盖率报告生成完成！"
    print_info "查看报告: open $OUTPUT_DIR/index.html"
}

# 运行主函数
main "$@"

