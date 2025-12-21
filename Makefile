# Makefile for VM project with timeout protection

# 默认超时时间（秒）
TEST_TIMEOUT ?= 300
BUILD_TIMEOUT ?= 600
CLIPPY_TIMEOUT ?= 600
BENCH_TIMEOUT ?= 1800

# 脚本目录
SCRIPTS_DIR := scripts
WITH_TIMEOUT := $(SCRIPTS_DIR)/with_timeout.sh

.PHONY: help test build clippy fmt bench clean all

help:
	@echo "VM Project Makefile"
	@echo ""
	@echo "目标:"
	@echo "  make test          - 运行测试（超时: $(TEST_TIMEOUT)秒）"
	@echo "  make build         - 构建项目（超时: $(BUILD_TIMEOUT)秒）"
	@echo "  make clippy        - 运行 Clippy（超时: $(CLIPPY_TIMEOUT)秒）"
	@echo "  make fmt           - 格式化代码"
	@echo "  make bench         - 运行基准测试（超时: $(BENCH_TIMEOUT)秒）"
	@echo "  make clean         - 清理构建产物"
	@echo "  make all           - 运行所有检查"
	@echo ""
	@echo "环境变量:"
	@echo "  TEST_TIMEOUT       - 测试超时时间（默认: $(TEST_TIMEOUT)秒）"
	@echo "  BUILD_TIMEOUT      - 构建超时时间（默认: $(BUILD_TIMEOUT)秒）"
	@echo "  CLIPPY_TIMEOUT     - Clippy超时时间（默认: $(CLIPPY_TIMEOUT)秒）"
	@echo "  BENCH_TIMEOUT      - 基准测试超时时间（默认: $(BENCH_TIMEOUT)秒）"

# 确保脚本可执行
$(WITH_TIMEOUT):
	chmod +x $(WITH_TIMEOUT)

test: $(WITH_TIMEOUT)
	@echo "运行测试（超时: $(TEST_TIMEOUT)秒）..."
	@$(WITH_TIMEOUT) $(TEST_TIMEOUT) cargo test --workspace --lib

test-all: $(WITH_TIMEOUT)
	@echo "运行所有测试（超时: $(TEST_TIMEOUT)秒）..."
	@$(WITH_TIMEOUT) $(TEST_TIMEOUT) cargo test --workspace --all-targets

test-unit: $(WITH_TIMEOUT)
	@echo "运行单元测试（超时: 60秒）..."
	@$(WITH_TIMEOUT) 60 cargo test --workspace --lib --test '*'

test-integration: $(WITH_TIMEOUT)
	@echo "运行集成测试（超时: 180秒）..."
	@$(WITH_TIMEOUT) 180 cargo test --workspace --test '*'

test-performance: $(WITH_TIMEOUT)
	@echo "运行性能测试（超时: 300秒）..."
	@$(WITH_TIMEOUT) 300 cargo test --workspace --test '*performance*'

test-concurrency: $(WITH_TIMEOUT)
	@echo "运行并发测试（超时: 600秒）..."
	@$(WITH_TIMEOUT) 600 cargo test --workspace --test '*concurrent*'

build: $(WITH_TIMEOUT)
	@echo "构建项目（超时: $(BUILD_TIMEOUT)秒）..."
	@$(WITH_TIMEOUT) $(BUILD_TIMEOUT) cargo build --workspace --all-targets

build-release: $(WITH_TIMEOUT)
	@echo "构建发布版本（超时: 1800秒）..."
	@$(WITH_TIMEOUT) 1800 cargo build --workspace --release --all-targets

clippy: $(WITH_TIMEOUT)
	@echo "运行 Clippy（超时: $(CLIPPY_TIMEOUT)秒）..."
	@$(WITH_TIMEOUT) $(CLIPPY_TIMEOUT) cargo clippy --workspace --all-targets -- -D warnings

fmt:
	@echo "格式化代码..."
	@cargo fmt --all --check || cargo fmt --all

fmt-fix:
	@echo "修复代码格式..."
	@cargo fmt --all

bench: $(WITH_TIMEOUT)
	@echo "运行基准测试（超时: $(BENCH_TIMEOUT)秒）..."
	@$(WITH_TIMEOUT) $(BENCH_TIMEOUT) cargo bench --workspace

clean:
	@echo "清理构建产物..."
	@cargo clean

all: fmt build clippy test
	@echo "所有检查完成！"

# CI/CD 目标
ci: fmt build clippy test-all
	@echo "CI 检查完成！"

# 快速检查（不运行测试）
quick-check: fmt build clippy
	@echo "快速检查完成！"

