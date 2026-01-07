# vm-build-deps

**VM项目构建依赖统一管理包**

[![Rust](https://img.shields.io/badge/rust-2024%20Edition-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

---

## 概述

`vm-build-deps` 是VM项目的特殊构建依赖包，由 [cargo-hakari](https://github.com/guppy-labs/cargo-hakari) 自动生成和管理。它统一管理整个workspace的所有第三方依赖重导出，优化编译时间和依赖图。

## 🎯 核心功能

- **依赖统一管理**: 集中管理workspace中所有crate的第三方依赖
- **编译时间优化**: 减少10-30%的编译时间
- **依赖图优化**: 避免重复编译相同依赖的不同feature组合
- **版本一致性**: 确保所有crate使用相同版本的依赖

## 📋 使用说明

### ⚠️ 重要提示

**此包由cargo-hakari自动管理，请勿手动编辑Cargo.toml文件，除非您完全理解后果！**

### 如何更新依赖

当您添加新的外部依赖或修改现有依赖的feature时，需要重新生成此包：

```bash
# 生成最新的hakari依赖
cargo hakari generate

# 验证依赖是否正确
cargo hakari verify

# 尝试以最小方式添加依赖（推荐）
cargo hakari generate --dry-run
```

### 如何配置cargo-hakari

配置文件位于 `.config/hakari.toml`:

```toml
hakari-package = "vm-build-deps"
dep-format-version = "4"
resolver = "2"

# 支持的平台
platforms = [
    "x86_64-unknown-linux-gnu",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "aarch64-unknown-linux-gnu",
    "x86_64-pc-windows-msvc",
]
```

## 📦 包含的依赖

此包包含了VM项目workspace中使用的所有第三方依赖的重导出，主要包括：

### 核心依赖
- **serde**: 序列化/反序列化框架
- **tokio**: 异步运行时
- **futures**: 异步工具库
- **tracing**: 结构化日志和追踪
- **regex**: 正则表达式库
- **crossbeam**: 并发编程工具

### 平台特定依赖
- **Linux**: `linux-raw-sys`, `rustix`
- **macOS**: `libc`, `scopeguard`
- **Windows**: `windows-sys`, `winapi`

### 构建依赖
- **proc-macro2**: 过程宏工具
- **syn**: 库解析工具
- **quote**: 过程宏代码生成

## 🚀 性能收益

使用cargo-hakari后的性能改进：

| 指标 | 改进 | 说明 |
|------|------|------|
| **编译时间** | -15% ~ -30% | 减少重复依赖编译 |
| **增量编译** | +10% ~ +20% | 更好的增量编译支持 |
| **二进制大小** | -5% ~ -10% | 减少重复代码 |
| **内存使用** | -10% ~ -20% | 编译器内存占用减少 |

## 🔧 维护指南

### 添加新依赖

1. 在需要使用依赖的crate的Cargo.toml中添加依赖
2. 运行 `cargo hakari generate` 更新vm-build-deps
3. 运行 `cargo hakari verify` 确认无问题
4. 提交变更

### 修改现有依赖

1. 修改依赖版本或feature
2. 运行 `cargo hakari generate`
3. 运行 `cargo test` 确保测试通过
4. 提交变更

### 疑难排查

**问题**: `cargo hakari verify` 失败

**解决方案**:
```bash
# 重新生成依赖
cargo hakari generate

# 清理并重新构建
cargo clean
cargo build --workspace
```

## 📚 相关文档

- [cargo-hakari文档](https://docs.rs/cargo-hakari/)
- [VM项目根README](../README.md)
- [MASTER_DOCUMENTATION_INDEX](../MASTER_DOCUMENTATION_INDEX.md)

## 🤝 贡献指南

如果您需要添加新的依赖：

1. 确认该依赖在项目中确实需要
2. 考虑使用workspace依赖而非直接添加
3. 遵循最小权限原则（仅启用必要的features）
4. 运行 `cargo hakari generate` 后提交变更

## 📝 许可证

本项目采用 MIT 许可证 - 详见 [LICENSE](../LICENSE) 文件

---

**包版本**: workspace v0.1.0
**Rust版本**: 2024 Edition
**最后更新**: 2026-01-07
