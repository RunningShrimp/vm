#!/bin/bash
# Pre-commit hook脚本
# 在提交前检查代码格式和Clippy警告

set -e

echo "Running pre-commit checks..."

# 检查代码格式
echo "Checking code formatting..."
if ! cargo fmt --all -- --check; then
    echo "Error: Code is not formatted. Run 'cargo fmt --all' to fix."
    exit 1
fi

# 运行Clippy检查
echo "Running Clippy..."
if ! cargo clippy --all-features --all-targets -- -D warnings; then
    echo "Error: Clippy found warnings or errors."
    exit 1
fi

echo "Pre-commit checks passed!"

