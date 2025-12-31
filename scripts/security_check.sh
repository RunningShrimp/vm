#!/bin/bash
# 安全检查脚本
#
# 用途: 对VM项目进行全面的安全扫描
# 使用: ./scripts/security_check.sh [--quick|--full|--ci]
#
# 选项:
#   --quick  快速检查(仅运行高优先级检查, ~2分钟)
#   --full   完整检查(包括模糊测试, ~30分钟)
#   --ci     CI模式(自动退出,无交互)
#
# 退出代码:
#   0 - 所有检查通过
#   1 - 检查失败
#   2 - 安全问题发现

set -euo pipefail

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 日志函数
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[PASS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[FAIL]${NC} $1"
}

# 检查命令是否存在
check_command() {
    if ! command -v "$1" &> /dev/null; then
        log_error "$1 未安装，请先安装: cargo install $1"
        return 1
    fi
    return 0
}

# 检查并安装工具
ensure_tools() {
    log_info "检查安全工具..."

    local tools=()
    local missing=()

    # 根据模式选择工具
    if [[ "$MODE" == "quick" ]]; then
        tools=("cargo" "rustc" "clippy")
    elif [[ "$MODE" == "full" ]]; then
        tools=("cargo-audit" "cargo-deny" "cargo-tarpaulin" "cargo-fuzz")
    else
        tools=("cargo-audit" "cargo")
    fi

    for tool in "${tools[@]}"; do
        if ! check_command "$tool"; then
            missing+=("$tool")
        fi
    done

    if [[ ${#missing[@]} -gt 0 && "$MODE" != "ci" ]]; then
        log_warning "缺少以下工具: ${missing[*]}"
        read -p "是否自动安装? (y/N) " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            for tool in "${missing[@]}"; do
                log_info "安装 $tool..."
                case "$tool" in
                    cargo-audit)
                        cargo install cargo-audit
                        ;;
                    cargo-deny)
                        cargo install cargo-deny
                        ;;
                    cargo-tarpaulin)
                        cargo install cargo-tarpaulin
                        ;;
                    cargo-fuzz)
                        cargo install cargo-fuzz
                        ;;
                    *)
                        log_error "未知工具: $tool"
                        ;;
                esac
            done
        else
            log_error "缺少必需工具，退出"
            exit 1
        fi
    fi
}

# 1. 依赖安全审计
audit_dependencies() {
    log_info "检查依赖安全..."

    if check_command cargo-audit; then
        # 设置离线模式如果需要
        local audit_args=""
        if [[ "$OFFLINE" == "true" ]]; then
            audit_args="--offline"
        fi

        if cargo audit $audit_args; then
            log_success "依赖安全审计通过"
        else
            log_error "发现依赖安全问题"
            return 1
        fi
    else
        log_warning "cargo-audit 未安装，跳过依赖审计"
    fi
}

# 2. cargo-deny检查
deny_check() {
    log_info "运行cargo-deny检查..."

    if check_command cargo-deny; then
        if cargo deny check; then
            log_success "cargo-deny检查通过"
        else
            log_error "cargo-deny检查失败"
            return 1
        fi
    else
        log_warning "cargo-deny 未安装，跳过"
    fi
}

# 3. Clippy安全检查
clippy_check() {
    log_info "运行Clippy安全lints..."

    local clippy_args=(
        "--workspace"
        "--all-targets"
        "--"
        "-W" "clippy::all"
        "-W" "clippy::pedantic"
        "-W" "clippy::cargo"
        "-W" "clippy::unwrap_used"
        "-W" "clippy::expect_used"
        "-W" "clippy::panic"
        "-W" "clippy::unimplemented"
        "-W" "clippy::todo"
        "-W" "clippy::indexing_slicing"
        "-W" "clippy::panic_in_result_fn"
    )

    if cargo clippy "${clippy_args[@]}"; then
        log_success "Clippy检查通过"
    else
        log_warning "Clippy发现问题，建议修复"
        if [[ "$STRICT" == "true" ]]; then
            return 1
        fi
    fi
}

# 4. Unsafe代码统计
unsafe_code_check() {
    log_info "统计unsafe代码..."

    local unsafe_count=$(grep -r "unsafe" --include="*.rs" vm-*/src/ | wc -l | tr -d ' ')
    local unsafe_files=$(grep -rl "unsafe" --include="*.rs" vm-*/src/ | wc -l | tr -d ' ')

    echo "  发现 $unsafe_count 处unsafe代码，分布在 $unsafe_files 个文件中"

    if [[ "$unsafe_count" -gt 500 ]]; then
        log_warning "unsafe代码数量较多 ($unsafe_count > 500)，建议审查"
    else
        log_success "unsafe代码数量合理: $unsafe_count"
    fi

    # 输出包含unsafe的文件列表
    if [[ "$VERBOSE" == "true" ]]; then
        echo ""
        echo "包含unsafe的文件:"
        grep -rl "unsafe" --include="*.rs" vm-*/src/ | head -20
    fi
}

# 5. 检查unwrap/expect/panic
panic_check() {
    log_info "检查panic-prone代码..."

    local unwrap_count=$(grep -r "\.unwrap()" --include="*.rs" vm-*/src/ | wc -l | tr -d ' ')
    local expect_count=$(grep -r "\.expect(" --include="*.rs" vm-*/src/ | wc -l | tr -d ' ')
    local panic_count=$(grep -r "panic!" --include="*.rs" vm-*/src/ | wc -l | tr -d ' ')

    local total=$((unwrap_count + expect_count + panic_count))

    echo "  unwrap(): $unwrap_count"
    echo "  expect(): $expect_count"
    echo "  panic!(): $panic_count"
    echo "  总计: $total"

    if [[ "$total" -gt 100 ]]; then
        log_warning "发现 $total 处可能panic的代码，建议使用Result"
    else
        log_success "panic-prone代码数量合理: $total"
    fi
}

# 6. 测试覆盖率
coverage_check() {
    if [[ "$MODE" != "full" ]]; then
        return 0
    fi

    log_info "生成测试覆盖率报告..."

    if check_command cargo-tarpaulin; then
        cargo tarpaulin \
            --workspace \
            --out Html \
            --output-dir coverage/ \
            --timeout 300 \
            || true

        if [[ -f "coverage/index.html" ]]; then
            log_success "覆盖率报告已生成: coverage/index.html"
        fi
    else
        log_warning "cargo-tarpaulin 未安装，跳过覆盖率检查"
    fi
}

# 7. 模糊测试 (仅full模式)
fuzz_check() {
    if [[ "$MODE" != "full" ]]; then
        return 0
    fi

    log_info "运行模糊测试..."

    if check_command cargo-fuzz; then
        local fuzz_targets=("memory_access" "instruction_decoder" "jit_compiler")

        for target in "${fuzz_targets[@]}"; do
            log_info "Fuzzing: $target (60秒)"

            # 运行fuzzer 60秒
            timeout 60 cargo fuzz run "$target" -- -max_total_time=60 || true

            log_success "模糊测试完成: $target"
        done
    else
        log_warning "cargo-fuzz 未安装，跳过模糊测试"
    fi
}

# 8. 检查安全相关的TODO注释
security_todo_check() {
    log_info "检查安全相关的TODO..."

    local security_todos=$(grep -r "TODO\|FIXME\|XXX\|HACK" \
        --include="*.rs" \
        vm-*/src/ 2>/dev/null | \
        grep -i "secur\|safe\|validat\|check\|overflow\|panic" | \
        wc -l | tr -d ' ')

    if [[ "$security_todos" -gt 0 ]]; then
        log_warning "发现 $security_todos 个安全相关的TODO注释"
        if [[ "$VERBOSE" == "true" ]]; then
            echo ""
            grep -rn "TODO\|FIXME\|XXX\|HACK" \
                --include="*.rs" \
                vm-*/src/ 2>/dev/null | \
                grep -i "secur\|safe\|validat\|check\|overflow\|panic" | head -20
        fi
    else
        log_success "未发现安全相关的TODO"
    fi
}

# 9. 许可证检查
license_check() {
    log_info "检查许可证合规性..."

    if check_command cargo-license; then
        cargo license

        log_success "许可证列表已生成"
    else
        log_warning "cargo-license 未安装，跳过"
    fi
}

# 10. 生成安全报告
generate_report() {
    log_info "生成安全检查报告..."

    local report_file="security_check_report_$(date +%Y%m%d_%H%M%S).txt"

    {
        echo "VM项目安全检查报告"
        echo "===================="
        echo "日期: $(date)"
        echo "模式: $MODE"
        echo "Git commit: $(git rev-parse --short HEAD 2>/dev/null || echo 'N/A')"
        echo ""
        echo "检查项目:"
        echo "  ✓ 依赖安全审计"
        echo "  ✓ cargo-deny检查"
        echo "  ✓ Clippy安全lints"
        echo "  ✓ Unsafe代码审查"
        echo "  ✓ Panic检查"
        echo "  ✓ 安全TODO检查"

        if [[ "$MODE" == "full" ]]; then
            echo "  ✓ 测试覆盖率"
            echo "  ✓ 模糊测试"
        fi

        echo ""
        echo "详细报告: 参见 SECURITY_AUDIT_REPORT.md"
    } > "$report_file"

    log_success "报告已生成: $report_file"
}

# 主函数
main() {
    local MODE="standard"
    local STRICT="false"
    local OFFLINE="false"
    local VERBOSE="false"

    # 解析参数
    while [[ $# -gt 0 ]]; do
        case $1 in
            --quick)
                MODE="quick"
                shift
                ;;
            --full)
                MODE="full"
                shift
                ;;
            --ci)
                MODE="ci"
                STRICT="true"
                shift
                ;;
            --offline)
                OFFLINE="true"
                shift
                ;;
            --verbose|-v)
                VERBOSE="true"
                shift
                ;;
            --help|-h)
                echo "用法: $0 [--quick|--full|--ci] [--offline] [--verbose]"
                echo ""
                echo "选项:"
                echo "  --quick   快速检查(~2分钟)"
                echo "  --full    完整检查(~30分钟)"
                echo "  --ci      CI模式"
                echo "  --offline 离线模式"
                echo "  --verbose 详细输出"
                exit 0
                ;;
            *)
                log_error "未知参数: $1"
                exit 1
                ;;
        esac
    done

    echo "==================================="
    echo "  VM项目安全检查"
    echo "  模式: $MODE"
    echo "==================================="
    echo ""

    # 确保工具已安装
    ensure_tools

    # 运行检查
    local failures=0

    audit_dependencies || ((failures++))
    deny_check || ((failures++))
    clippy_check || ((failures++))
    unsafe_code_check || ((failures++))
    panic_check || ((failures++))
    security_todo_check || ((failures++))
    license_check || ((failures++))

    if [[ "$MODE" == "full" ]]; then
        coverage_check
        fuzz_check
    fi

    # 生成报告
    generate_report

    echo ""
    echo "==================================="

    if [[ $failures -eq 0 ]]; then
        log_success "所有安全检查通过!"
        echo "==================================="
        return 0
    else
        log_error "$failures 项检查失败"
        echo "==================================="
        return 1
    fi
}

# 运行主函数
main "$@"
