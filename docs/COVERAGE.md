# 代码覆盖率指南

本文档介绍如何使用代码覆盖率工具来评估测试质量。

## 支持的工具

项目支持两种代码覆盖率工具：

1. **cargo-llvm-cov** (推荐) - 基于 LLVM 的覆盖率工具，速度快，支持多种输出格式
2. **cargo-tarpaulin** (备用) - Rust 原生覆盖率工具，兼容性好

## 安装工具

### 安装 cargo-llvm-cov

```bash
cargo install cargo-llvm-cov --locked
```

### 安装 cargo-tarpaulin

```bash
cargo install cargo-tarpaulin
```

或者使用项目提供的安装脚本：

```bash
./scripts/setup-dev.sh
```

## 使用方法

### 快速开始

使用项目提供的脚本（推荐）：

```bash
# 使用默认工具 (llvm-cov)
./scripts/coverage.sh

# 指定工具
COVERAGE_TOOL=tarpaulin ./scripts/coverage.sh

# 指定输出目录
COVERAGE_OUTPUT_DIR=my-coverage ./scripts/coverage.sh
```

### 手动运行

#### 使用 cargo-llvm-cov

```bash
# 生成HTML报告
cargo llvm-cov --all-features --workspace --html --output-dir coverage

# 生成LCOV格式（用于CI/CD）
cargo llvm-cov --all-features --workspace --lcov --output-path coverage/lcov.info

# 生成摘要
cargo llvm-cov --all-features --workspace --summary
```

#### 使用 cargo-tarpaulin

```bash
# 生成HTML报告
cargo tarpaulin --all-features --workspace --out Html --output-dir coverage

# 生成XML格式（用于CI/CD）
cargo tarpaulin --all-features --workspace --out Xml --output-dir coverage
```

## 查看报告

### HTML报告

生成后，在浏览器中打开：

```bash
# macOS
open test-results/coverage/index.html

# Linux
xdg-open test-results/coverage/index.html

# Windows
start test-results/coverage/index.html
```

### 增强HTML报告

项目还提供了增强的HTML报告生成器：

```bash
python3 scripts/generate-coverage-report.py \
    --coverage-summary test-results/coverage/coverage-summary.txt \
    --output test-results/coverage/coverage-report.html
```

## 覆盖率目标

项目设定了以下覆盖率目标：

- **代码行覆盖率**: ≥ 80%
- **函数覆盖率**: ≥ 80%
- **分支覆盖率**: ≥ 70%

## CI/CD集成

### GitHub Actions

项目已配置 GitHub Actions 工作流，在以下情况自动生成覆盖率报告：

- Push 到 main 或 develop 分支
- Pull Request
- 每周一自动运行
- 手动触发

覆盖率报告会自动上传到：
- Codecov（如果配置了token）
- GitHub Actions Artifacts

### 本地CI模拟

```bash
# 运行完整的测试和覆盖率检查
./scripts/test.sh --coverage
```

## 排除文件

某些文件默认被排除在覆盖率统计之外：

- 测试文件 (`*/tests/*`)
- 基准测试文件 (`*/benches/*`)
- 示例文件 (`*/examples/*`)
- 构建脚本 (`build.rs`)

可以在 `coverage.toml` 中自定义排除规则。

## 故障排除

### 问题：覆盖率报告为空

**解决方案**：
1. 确保已安装 `llvm-tools-preview` 组件：
   ```bash
   rustup component add llvm-tools-preview
   ```
2. 确保代码编译时包含调试信息（已配置在 `.cargo/config.toml`）

### 问题：某些文件未包含在报告中

**解决方案**：
1. 检查 `coverage.toml` 中的排除规则
2. 确保文件路径匹配包含模式

### 问题：覆盖率工具运行缓慢

**解决方案**：
1. 使用 `cargo-llvm-cov`（通常比 tarpaulin 快）
2. 减少测试范围，只测试特定包：
   ```bash
   cargo llvm-cov --package vm-core --html
   ```

## 最佳实践

1. **定期检查覆盖率**：在每次重大功能添加后检查覆盖率
2. **关注关键路径**：优先提高核心功能的覆盖率
3. **设置CI阈值**：在CI中设置最低覆盖率要求
4. **审查未覆盖代码**：识别并测试未覆盖的关键代码路径

## 相关资源

- [cargo-llvm-cov 文档](https://github.com/taiki-e/cargo-llvm-cov)
- [cargo-tarpaulin 文档](https://github.com/xd009642/tarpaulin)
- [Codecov 文档](https://docs.codecov.com/)

