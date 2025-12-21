# 命令超时保护指南

## 概述

为了防止长时间运行的命令（如测试、构建）卡死，所有关键命令都应该使用超时保护。

## 使用方法

### 1. 使用 `with_timeout.sh` 脚本

这是通用的超时包装脚本，可以包装任何命令：

```bash
./scripts/with_timeout.sh <超时秒数> <命令>
```

示例：
```bash
# 运行测试，超时5分钟
./scripts/with_timeout.sh 300 cargo test --workspace

# 运行构建，超时10分钟
./scripts/with_timeout.sh 600 cargo build --workspace

# 运行 Clippy，超时10分钟
./scripts/with_timeout.sh 600 cargo clippy --workspace
```

### 2. 使用专用脚本

#### `safe_cargo_test.sh` - 安全的测试脚本

```bash
# 运行单元测试（默认60秒超时）
./scripts/safe_cargo_test.sh --unit

# 运行集成测试（默认180秒超时）
./scripts/safe_cargo_test.sh --integration

# 运行性能测试（默认300秒超时）
./scripts/safe_cargo_test.sh --performance

# 运行并发测试（默认600秒超时）
./scripts/safe_cargo_test.sh --concurrency

# 运行所有测试（默认1800秒超时）
./scripts/safe_cargo_test.sh --full

# 自定义超时时间
./scripts/safe_cargo_test.sh --timeout 1200 cargo test --workspace
```

#### `safe_cargo_build.sh` - 安全的构建脚本

```bash
# 普通构建（默认600秒超时）
./scripts/safe_cargo_build.sh

# 发布构建（默认1800秒超时）
./scripts/safe_cargo_build.sh --release

# 自定义超时时间
./scripts/safe_cargo_build.sh --timeout 1200 cargo build --workspace
```

### 3. 使用 Makefile

```bash
# 运行测试（默认300秒超时）
make test

# 运行单元测试（60秒超时）
make test-unit

# 运行集成测试（180秒超时）
make test-integration

# 运行性能测试（300秒超时）
make test-performance

# 运行并发测试（600秒超时）
make test-concurrency

# 构建项目（默认600秒超时）
make build

# 构建发布版本（1800秒超时）
make build-release

# 运行 Clippy（默认600秒超时）
make clippy

# 运行基准测试（默认1800秒超时）
make bench

# 自定义超时时间
TEST_TIMEOUT=600 make test
BUILD_TIMEOUT=1200 make build
```

## 默认超时时间

| 操作类型 | 默认超时 | 说明 |
|---------|---------|------|
| 单元测试 | 60秒 | 快速测试 |
| 集成测试 | 180秒 | 中等复杂度 |
| 性能测试 | 300秒 | 5分钟 |
| 并发测试 | 600秒 | 10分钟 |
| 完整测试套件 | 1800秒 | 30分钟 |
| 普通构建 | 600秒 | 10分钟 |
| 发布构建 | 1800秒 | 30分钟 |
| Clippy检查 | 600秒 | 10分钟 |
| 基准测试 | 1800秒 | 30分钟 |

## 超时错误处理

当命令超时时，脚本会：

1. 终止命令执行
2. 返回退出码 124（timeout 命令的标准退出码）
3. 显示清晰的错误信息，包括：
   - 超时时间
   - 可能的原因（死锁、无限循环等）
   - 建议的解决方案

## 系统兼容性

脚本会自动检测系统并选择合适的超时实现：

- **Linux**: 使用 `timeout` 命令
- **macOS (Homebrew)**: 使用 `gtimeout` 命令（GNU coreutils）
- **macOS (默认)**: 使用 Perl 实现

## CI/CD 集成

在 CI/CD 配置中使用超时保护：

```yaml
# GitHub Actions 示例
- name: Run tests with timeout
  run: ./scripts/safe_cargo_test.sh --full
  timeout-minutes: 30

# GitLab CI 示例
test:
  script:
    - ./scripts/safe_cargo_test.sh --full
  timeout: 30m
```

## 故障排查

### 问题：命令总是超时

**可能原因：**
1. 超时时间设置太短
2. 系统资源不足
3. 网络问题（依赖下载）

**解决方案：**
- 增加超时时间
- 检查系统资源
- 检查网络连接

### 问题：超时脚本不工作

**可能原因：**
1. 脚本没有执行权限
2. 系统没有 timeout 命令

**解决方案：**
```bash
# 添加执行权限
chmod +x scripts/with_timeout.sh

# 检查系统是否有 timeout
which timeout || which gtimeout || echo "需要安装 timeout 工具"
```

## 相关文件

- `scripts/with_timeout.sh`: 通用超时包装脚本
- `scripts/safe_cargo_test.sh`: 安全的测试脚本
- `scripts/safe_cargo_build.sh`: 安全的构建脚本
- `Makefile`: Make 目标（带超时保护）
- `tests/test_timeout_utils.rs`: Rust 测试超时工具
- `tests/test_timeout_config.rs`: 测试超时配置
- `tests/TEST_TIMEOUT_GUIDE.md`: 测试超时指南

