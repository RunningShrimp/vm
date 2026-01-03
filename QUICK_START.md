# VM项目快速开始指南

欢迎使用VM虚拟机项目！这是一个用Rust编写的高性能虚拟机实现。

## 📋 前置要求

- Rust 1.92+ (推荐使用stable版本)
- Git
- (可选) LLVM 16+ (用于某些优化功能)

## 🚀 快速开始

### 1. 克隆项目

```bash
git clone https://github.com/example/vm.git
cd vm
```

### 2. 验证Rust版本

```bash
rustc --version
# 应该显示: rustc 1.92.0 或更高版本
```

如果版本低于1.92，请更新Rust：

```bash
rustup update stable
rustup default stable
```

### 3. 构建项目

```bash
# 构建整个workspace
cargo build --release

# 或者只构建特定crate
cargo build --package vm-core
cargo build --package vm-engine
```

### 4. 运行测试

```bash
# 运行所有测试
cargo test --workspace

# 运行特定crate的测试
cargo test --package vm-core

# 带输出的测试
cargo test --package vm-core -- --nocapture
```

### 5. 运行示例

```bash
# 查看所有示例
ls examples/

# 运行示例（需要实现）
cargo run --example quick_start
```

## 📦 项目结构

```
vm/
├── vm-core/              # 核心库（类型定义、Trait、基础设施）
├── vm-frontend/          # 前端指令解码（RISC-V、ARM64、x86_64）
├── vm-ir/                # 中间表示
├── vm-engine/            # 执行引擎（解释器、JIT）
├── vm-engine-jit/        # 高级JIT优化
├── vm-mem/               # 内存管理（MMU、TLB、NUMA）
├── vm-device/            # 设备模拟（VirtIO、块设备、网络）
├── vm-accel/             # 硬件加速（KVM、HVF、WHPF）
├── vm-boot/              # 启动和快照管理
├── vm-service/           # VM服务接口
├── vm-platform/          # 平台抽象层
├── vm-plugin/            # 插件系统
├── vm-cli/               # 命令行工具
├── vm-desktop/           # 桌面应用
├── vm-monitor/           # 监控和分析
├── vm-debug/             # 调试工具
├── vm-optimizers/        # 优化器
└── vm-gc/                # 垃圾回收
```

## 🎯 基本使用

### 创建虚拟机

```rust
use vm_core::{GuestArch, VmConfig, ExecMode};
use vm_engine::JITCompiler;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. 创建VM配置
    let config = VmConfig {
        guest_arch: GuestArch::Riscv64,
        memory_size: 128 * 1024 * 1024, // 128MB
        vcpu_count: 1,
        exec_mode: ExecMode::JIT,
        kernel_path: Some("kernel.bin".to_string()),
        ..Default::default()
    };

    // 2. 创建JIT编译器
    let mut jit = JITCompiler::new(Default::default());

    // 3. 执行代码（示例）
    // ... 具体实现取决于你的需求

    Ok(())
}
```

### 跨架构执行

```rust
use vm_frontend::{X86Decoder, Arm64Encoder};
use vm_cross_arch_support::CrossArchTranslator;

// 在ARM64主机上运行x86_64 Guest
fn execute_x86_on_arm64() -> Result<(), Box<dyn std::error::Error>> {
    // 1. 解码x86_64指令
    let mut decoder = X86Decoder::new();
    let x86_insn = decoder.decode(&memory, pc)?;

    // 2. 翻译为ARM64
    let translator = CrossArchTranslator::new(
        Arch::X86_64,
        Arch::ARM64
    );
    let arm64_insn = translator.translate(&x86_insn)?;

    // 3. 编译执行
    // ...

    Ok(())
}
```

## 🔧 开发指南

### 代码风格

项目使用严格的代码质量标准：

```bash
# 格式化代码
cargo fmt

# 运行Clippy检查
cargo clippy --workspace -- -D warnings

# 所有检查必须通过
cargo check --workspace
```

### Feature系统

项目支持细粒度的feature选择：

```bash
# RISC-V架构 + M扩展
cargo build --package vm-frontend --features "riscv64,riscv-m"

# 所有架构
cargo build --package vm-frontend --features all

# 异步内存 + SIMD优化
cargo build --package vm-mem --features "async,opt-simd"
```

### 运行基准测试

```bash
# MMU翻译性能
cargo bench --bench mmu_translate

# TLB优化性能
cargo bench --bench tlb_optimized

# 所有基准测试
cargo bench
```

## 📚 文档

- [架构文档](docs/ARCHITECTURE.md) - 整体架构说明
- [API文档](https://docs.rs/vm) - Rust API文档
- [性能基准](docs/BENCHMARKING.md) - 性能测试指南
- [贡献指南](CONTRIBUTING.md) - 如何贡献代码

## 🐛 故障排除

### 编译错误

如果遇到编译错误：

```bash
# 清理构建产物
cargo clean

# 重新构建
cargo build --workspace
```

### 依赖问题

如果遇到依赖版本冲突：

```bash
# 更新依赖
cargo update

# 检查依赖树
cargo tree
```

### 性能问题

如果遇到性能问题：

```bash
# 使用release模式
cargo build --release

# 运行性能分析
cargo bench
```

## 🤝 贡献

欢迎贡献！请查看[贡献指南](CONTRIBUTING.md)了解详情。

## 📄 许可证

MIT OR Apache-2.0

---

**需要帮助？**
- 查看文档：`docs/`
- 提交issue：GitHub Issues
- 联系维护者：wangbiao

🤖 Generated with [Claude Code](https://claude.com/claude-code)
