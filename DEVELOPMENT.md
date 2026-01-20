# 开发指南

本指南帮助您快速上手项目开发。

## 📋 目录

- [环境准备](#环境准备)
- [项目结构](#项目结构)
- [构建和测试](#构建和测试)
- [开发工作流](#开发工作流)
- [代码规范](#代码规范)
- [贡献指南](#贡献指南)
- [常见问题](#常见问题)

---

## 🔧 环境准备

### 必需工具

- **Rust** 1.92 或更高版本
  ```bash
  rustup update stable
  rustup default stable
  ```

- **Cargo** - Rust 包管理器（随 Rust 一起安装）

### 平台特定依赖

#### Linux (x86_64/ARM64)
```bash
# KVM 支持
sudo apt-get install kvm qemu-kvm libvirt-daemon-system libvirt-clients

# 其他依赖
sudo apt-get install build-essential libssl-dev pkg-config
```

#### macOS (ARM64)
```bash
# HVF 已包含在 macOS 中
# 需要 Xcode Command Line Tools
xcode-select --install
```

#### Windows (x86_64)
```bash
# WHP 需要 Windows 10/11 Pro/Enterprise
# 启用 Hyper-V
dism.exe /online /enable-feature /featurename:Microsoft-Hyper-V /all /norestart
```

---

## 🏗️ 项目结构

```
vm/
├── crates/              # 核心库（8个分类）
│   ├── core/            # 领域模型、IR、启动
│   ├── execution/       # 执行引擎、JIT
│   ├── memory/          # 内存管理、GC
│   ├── platform/        # 平台抽象、加速
│   ├── devices/         # 设备模拟
│   ├── runtime/         # 服务、插件、监控
│   ├── compatibility/    # 沙箱、系统调用
│   └── architecture/    # 跨架构、代码生成
├── tools/              # 用户工具
│   ├── cli/            # 命令行
│   ├── desktop/        # GUI 应用
│   ├── debug/          # 调试工具
│   └── passthrough/    # 设备直通
├── research/           # 研究项目
│   ├── perf-bench/      # 性能基准
│   ├── tiered-compiler/ # 分层编译器
│   ├── parallel-jit/    # 并行 JIT
│   └── benches/         # 基准测试
├── docs/               # 文档
├── tests/              # 测试
├── scripts/            # 脚本
└── plans/              # 规划文档
```

详细导航：参见 [NAVIGATION.md](./NAVIGATION.md)

---

## 🚀 构建和测试

### 快速开始

```bash
# 克隆仓库
git clone <repository-url>
cd vm

# 构建所有项目
cargo build --workspace

# 运行测试
cargo test --workspace

# 运行特定测试
cargo test -p vm-core
cargo test --lib vm_device
```

### 构建选项

```bash
# Debug 构建（默认）
cargo build

# Release 构建（优化）
cargo build --release

# 仅构建特定包
cargo build -p vm-cli
cargo build -p vm-desktop

# 构建所有工具
cargo build --release -p vm-cli -p vm-debug -p vm-passthrough
```

### 测试

```bash
# 运行所有测试
cargo test --workspace

# 运行单元测试
cargo test --lib

# 运行集成测试
cargo test --test '*'

# 运行特定测试
cargo test test_name

# 显示测试输出
cargo test -- --nocapture

# 运行文档测试
cargo test --doc
```

### 基准测试

```bash
# 运行所有基准测试
cargo bench --workspace

# 运行特定基准测试
cargo bench -p perf-bench
```

---

## 🔄 开发工作流

### 1. 创建功能分支

```bash
git checkout -b feature/your-feature-name
# 或
git checkout -b fix/bug-description
```

### 2. 开发和测试

```bash
# 开发代码
vim crates/memory/vm-mem/src/lib.rs

# 运行相关测试
cargo test -p vm-mem

# 运行完整测试
cargo test --workspace
```

### 3. 代码质量检查

```bash
# 运行 clippy
cargo clippy --workspace -- -D warnings

# 运行 fmt 检查
cargo fmt --check

# 自动格式化
cargo fmt
```

### 4. 提交代码

```bash
# 查看变更
git status

# 添加文件
git add .

# 提交（使用清晰的提交信息）
git commit -m "feat(vm-mem): add NUMA-aware memory allocation"
```

### 5. 推送和 PR

```bash
# 推送分支
git push origin feature/your-feature-name

# 在 GitHub 上创建 Pull Request
```

---

## 📐 代码规范

### Rust 代码规范

项目遵循 Rust 官方风格指南：

```bash
# 自动格式化代码
cargo fmt

# 检查格式
cargo fmt --check
```

### Lint 规则

项目使用严格的 Clippy 规则：

```bash
# 运行所有 lints
cargo clippy --workspace -- -D warnings
```

### 代码风格

- 使用清晰的变量和函数名
- 避免不必要的注释（代码应该自解释）
- 使用模块化设计
- 编写文档注释（pub items）

示例：
```rust
/// Allocates NUMA-aware memory pages.
///
/// # Arguments
///
/// * `size` - Size of allocation in bytes
/// * `node` - NUMA node ID
///
/// # Returns
///
/// Pointer to allocated memory
pub fn allocate_numa_memory(size: usize, node: u32) -> *mut u8 {
    // 实现
}
```

---

## 🤝 贡献指南

### 贡献类型

- **feat**: 新功能
- **fix**: Bug 修复
- **docs**: 文档更新
- **style**: 代码格式调整（不影响功能）
- **refactor**: 代码重构
- **perf**: 性能优化
- **test**: 测试相关
- **chore**: 构建过程或工具链

### 提交信息格式

遵循 Conventional Commits：

```
<type>(<scope>): <subject>

<body>

<footer>
```

示例：
```
feat(vm-engine): add tiered JIT compilation

Implements three-tier compilation:
1. Interpreter (fast startup)
2. Simple JIT (frequently executed)
3. Optimized JIT (hot paths)

Closes #123
```

### Pull Request 流程

1. Fork 仓库
2. 创建功能分支
3. 开发和测试
4. 确保 CI 通过
5. 创建 PR 并描述变更

---

## 📚 添加新功能

### 添加新设备

1. 在 `crates/devices/vm-device/` 中创建新设备
2. 实现设备 trait
3. 在 `vm-service` 中注册设备
4. 添加测试
5. 更新文档

### 添加新架构

1. 在 `crates/execution/vm-frontend/` 中添加解码器
2. 在 `vm-codegen` 中添加代码生成
3. 在 `vm-cross-arch-support` 中添加支持
4. 添加测试和基准
5. 更新文档

### 添加新工具

1. 在 `tools/` 中创建新目录
2. 添加 `Cargo.toml`
3. 实现工具逻辑
4. 更新 `tools/README.md`
5. 在 workspace 中注册

---

## 🧪 测试策略

### 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_allocation() {
        let mem = allocate_memory(1024);
        assert!(!mem.is_null());
    }
}
```

### 集成测试

在 `tests/` 目录中创建测试文件：

```rust
// tests/integration_test.rs
use vm_core::VmEngine;

#[test]
fn test_vm_lifecycle() {
    let vm = VmEngine::new().unwrap();
    vm.start().unwrap();
    vm.shutdown().unwrap();
}
```

### 基准测试

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_allocation(c: &mut Criterion) {
    c.bench_function("allocate_memory", |b| {
        b.iter(|| allocate_memory(black_box(1024)))
    });
}

criterion_group!(benches, benchmark_allocation);
criterion_main!(benches);
```

---

## 🐛 调试技巧

### 使用日志

```rust
use log::{info, debug, error};

info!("Starting VM");
debug!("Allocating memory: {} bytes", size);
error!("Failed to allocate memory: {}", e);
```

启用日志：
```bash
RUST_LOG=debug cargo run -p vm-cli start my-vm
```

### 使用断言

```rust
debug_assert!(ptr != null_ptr(), "Pointer should not be null");
```

### 使用调试工具

- `gdb` / `lldb` - 调试 Rust 程序
- `valgrind` - 内存泄漏检测（Linux）
- `perf` - 性能分析（Linux）
- `Instruments` - 性能分析（macOS）

---

## ❓ 常见问题

### Q: 构建失败，提示找不到依赖

A: 更新依赖：
```bash
cargo update
cargo build
```

### Q: 测试失败

A: 运行单个测试查看详细输出：
```bash
cargo test test_name -- --nocapture -- --test-threads=1
```

### Q: 性能下降

A: 使用基准测试定位问题：
```bash
cargo bench -p perf-bench
```

### Q: 如何添加新的 workspace member？

A: 在 `Cargo.toml` 的 `[workspace.members]` 中添加路径。

---

## 📖 更多资源

- [Rust Book](https://doc.rust-lang.org/book/)
- [Cargo Guide](https://doc.rust-lang.org/cargo/)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [项目架构文档](./docs/architecture/ARCHITECTURE.md)
- [用户指南](./docs/user-guides/USER_GUIDE.md)
- [快速导航](./NAVIGATION.md)

---

**祝开发顺利！如有问题，请提交 Issue 或 Pull Request。**
