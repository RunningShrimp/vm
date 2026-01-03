#!/bin/bash
set -e

echo "======================================"
echo "  VM Project 依赖更新工具"
echo "======================================"
echo ""

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# 日志函数
log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# 检查工具是否存在
check_tool() {
    if command -v $1 &> /dev/null; then
        log_info "$1 已安装"
        return 0
    else
        log_warn "$1 未安装"
        return 1
    fi
}

# 1. 检查过时依赖
echo "======================================"
echo "1. 检查过时依赖"
echo "======================================"

OUTDATED_FILE="/tmp/outdated_deps.txt"
AUDIT_FILE="/tmp/audit_report.txt"

if check_tool "cargo-outdated"; then
    log_info "正在检查过时依赖..."
    cargo outdated --workspace 2>&1 | tee "$OUTDATED_FILE" || true

    if [ -s "$OUTDATED_FILE" ] && grep -q "Name" "$OUTDATED_FILE"; then
        log_warn "发现过时依赖!"
        echo ""
        echo "过时依赖列表:"
        cat "$OUTDATED_FILE"
    else
        log_info "✓ 所有依赖都是最新的"
    fi
else
    log_warn "cargo-outdated 未安装，跳过过时依赖检查"
    echo "  安装: cargo install cargo-outdated"
fi

echo ""
echo "======================================"
echo "2. 安全审计"
echo "======================================"

if check_tool "cargo-audit"; then
    log_info "正在运行安全审计..."
    cargo audit --workspace 2>&1 | tee "$AUDIT_FILE" || true

    if grep -q "warning: unused" "$AUDIT_FILE" || ! grep -q "Vulnerability" "$AUDIT_FILE"; then
        log_info "✓ 未发现已知安全漏洞"
    else
        log_warn "发现安全漏洞，请查看审计报告"
    fi
else
    log_warn "cargo-audit 未安装，跳过安全审计"
    echo "  安装: cargo install cargo-audit"
fi

echo ""
echo "======================================"
echo "3. 更新依赖"
echo "======================================"

read -p "是否要更新所有依赖到最新兼容版本? (y/N): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    log_info "正在更新依赖..."
    cargo update

    echo ""
    log_info "✓ 依赖已更新"

    # 显示更新内容
    echo ""
    echo "更新摘要:"
    git diff Cargo.lock | head -50 || true
else
    log_info "跳过依赖更新"
fi

echo ""
echo "======================================"
echo "4. 验证编译"
echo "======================================"

log_info "正在验证编译..."
if cargo check --workspace 2>&1 | tee /tmp/check_output.txt; then
    log_info "✓ 编译检查通过"
else
    log_error "✗ 编译检查失败"
    echo ""
    echo "错误信息:"
    cat /tmp/check_output.txt | grep "error" | head -20

    read -p "是否要回滚依赖更新? (y/N): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        log_info "正在回滚依赖更新..."
        git checkout Cargo.lock
        log_info "✓ 已回滚"
    fi
    exit 1
fi

echo ""
echo "======================================"
echo "5. 运行测试"
echo "======================================"

log_info "正在运行测试..."
if cargo test --workspace 2>&1 | tee /tmp/test_output.txt; then
    log_info "✓ 所有测试通过"
else
    log_error "✗ 测试失败"
    echo ""
    echo "失败信息:"
    cat /tmp/test_output.txt | grep "test result" | tail -5

    read -p "是否要回滚依赖更新? (y/N): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        log_info "正在回滚依赖更新..."
        git checkout Cargo.lock
        log_info "✓ 已回滚"
    fi
    exit 1
fi

echo ""
echo "======================================"
echo "6. 性能基准测试"
echo "======================================"

read -p "是否要运行性能基准测试? (可能需要几分钟) (y/N): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    log_info "正在运行性能基准测试..."
    cargo bench --workspace 2>&1 | tee /tmp/bench_output.txt

    echo ""
    log_info "✓ 基准测试完成"
    log_info "结果已保存到: /tmp/bench_output.txt"
fi

echo ""
echo "======================================"
echo "7. 生成依赖报告"
echo "======================================"

REPORT_FILE="dependency_report_$(date +%Y%m%d_%H%M%S).md"

cat > "$REPORT_FILE" <<EOF
# 依赖更新报告

**生成时间**: $(date)
**Rust版本**: $(rustc --version)
**Cargo版本**: $(cargo --version)

---

## 1. 过时依赖

EOF

if [ -s "$OUTDATED_FILE" ] && grep -q "Name" "$OUTDATED_FILE"; then
    echo '```' >> "$REPORT_FILE"
    cat "$OUTDATED_FILE" >> "$REPORT_FILE"
    echo '```' >> "$REPORT_FILE"
else
    echo "无" >> "$REPORT_FILE"
fi

cat >> "$REPORT_FILE" <<EOF

---

## 2. 安全审计

EOF

if [ -s "$AUDIT_FILE" ]; then
    echo '```' >> "$REPORT_FILE"
    cat "$AUDIT_FILE" >> "$REPORT_FILE"
    echo '```' >> "$REPORT_FILE"
else
    echo "审计工具未安装" >> "$REPORT_FILE"
fi

cat >> "$REPORT_FILE" <<EOF

---

## 3. 依赖树

\`\`\`
EOF

cargo tree --depth 1 >> "$REPORT_FILE"

echo '```' >> "$REPORT_FILE"

cat >> "$REPORT_FILE" <<EOF

---

## 4. 建议

EOF

# 根据检查结果给出建议
if ! check_tool "cargo-outdated"; then
    echo "- [ ] 安装 cargo-outdated 以检测过时依赖" >> "$REPORT_FILE"
fi

if ! check_tool "cargo-audit"; then
    echo "- [ ] 安装 cargo-audit 以检测安全漏洞" >> "$REPORT_FILE"
fi

echo "" >> "$REPORT_FILE"
echo "建议定期运行此脚本以确保依赖安全和性能。" >> "$REPORT_FILE"

log_info "✓ 依赖报告已生成: $REPORT_FILE"

echo ""
echo "======================================"
echo "           更新完成!"
echo "======================================"
echo ""
echo "下一步操作:"
echo "  1. 查看依赖报告: cat $REPORT_FILE"
echo "  2. 提交更新: git add Cargo.lock Cargo.toml"
echo "  3. 运行完整测试: cargo test --workspace"
echo "  4. 推送更改: git push"
echo ""
