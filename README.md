# Rust虚拟机系统

一个高性能、跨平台的Rust虚拟机实现，支持多种架构和执行模式。

## 特性

- **多架构支持**: RISC-V64, ARM64, x86-64
- **多种执行模式**: 解释器、JIT编译、硬件加速、混合模式
- **高性能JIT**: 基于Cranelift的JIT编译器，支持热点检测和优化
- **高级优化**: 图着色寄存器分配、指令调度、多种优化Pass
- **分代GC**: 支持年轻代和老年代，Card Marking优化
- **AOT编译**: 支持AOT编译和缓存，避免重复编译
- **异步执行**: 支持异步执行引擎和异步内存访问
- **跨架构执行**: 支持在不同架构间执行代码

## 快速开始

### 安装

```bash
# 克隆仓库
git clone <repository-url>
cd vm

# 构建项目
cargo build --release

# 运行测试
cargo test
```

### 基本使用

```rust
use vm_core::{GuestArch, VmConfig, ExecMode};
use vm_engine_jit::Jit;
use vm_ir::IRBlock;

// 创建虚拟机配置
let config = VmConfig {
    guest_arch: GuestArch::Riscv64,
    memory_size: 128 * 1024 * 1024, // 128MB
    vcpu_count: 1,
    exec_mode: ExecMode::Jit,
    ..Default::default()
};

// 创建JIT引擎
let mut jit = Jit::new();

// 创建IR块
let block = IRBlock {
    start_pc: 0x1000,
    ops: vec![/* ... */],
    term: Terminator::Ret,
};

// 执行代码块
let result = jit.run(&mut mmu, &block);
```

## 项目结构

```
vm/
├── vm-core/          # 核心抽象和类型定义
├── vm-ir/            # 中间表示（IR）
├── vm-mem/           # 内存管理
├── vm-device/        # 设备模拟
├── vm-frontend-*/    # 架构前端（RISC-V64, ARM64, x86-64）
├── vm-engine-*/      # 执行引擎（JIT, Interpreter）
├── vm-cross-arch/    # 跨架构执行
├── vm-service/       # 服务层
├── vm-runtime/       # 运行时（协程池等）
├── tests/            # 集成测试
├── benches/          # 性能基准测试
└── docs/             # 文档
```

## 主要模块

### vm-core

核心库，提供：
- 基础类型定义（`GuestAddr`, `GuestArch`等）
- Trait抽象（`ExecutionEngine`, `MMU`等）
- 错误处理（`VmError`, `VmResult`）
- 领域事件和聚合根

### vm-engine-jit

JIT编译引擎，提供：
- 基于Cranelift的JIT编译器
- 优化型JIT编译器（寄存器分配、指令调度）
- 图着色寄存器分配器
- 统一GC实现（分代GC、Card Marking）
- AOT编译缓存
- ML引导优化

### vm-runtime

运行时支持，提供：
- 协程池管理
- 任务调度和优先级管理
- 工作窃取算法

## 文档

### 核心文档
- [API参考](docs/API_REFERENCE.md): 完整的API参考文档
- [用户指南](docs/USER_GUIDE.md): 用户指南和快速开始
- [API示例](docs/API_EXAMPLES.md): 详细的API使用示例
- [架构文档](docs/ARCHITECTURE.md): 系统架构说明
- [性能调优指南](docs/PERFORMANCE_TUNING_GUIDE.md): 性能优化建议
- [故障排除指南](docs/TROUBLESHOOTING_GUIDE.md): 常见问题和解决方案

### 实现文档
- [JIT编译器优化指南](docs/JIT_OPTIMIZATION_GUIDE.md): JIT编译器优化技术详解
- [JIT编译器完善报告](docs/JIT_COMPILATION_ENHANCEMENT_REPORT.md): JIT编译器完善工作总结
- [协程池优化](docs/COROUTINE_POOL_OPTIMIZATION.md): 协程池优化实现
- [异步执行引擎](docs/ASYNC_EXECUTION_ENGINE.md): 异步执行引擎实现
- [增量AOT编译](docs/INCREMENTAL_AOT.md): 增量AOT编译实现
- [ML引导优化](docs/ML_GUIDED_OPTIMIZATION.md): ML引导优化实现
- [分代GC实现](docs/GC_ADAPTIVE_ADJUSTMENT.md): 分代GC和自适应调整

## 开发

### 代码风格

项目使用标准的Rust代码风格：

```bash
# 格式化代码
cargo fmt

# 检查代码
cargo clippy --workspace --tests -- -D warnings
```

### 测试

```bash
# 运行所有测试
cargo test

# 运行特定包的测试
cargo test --package vm-core

# 运行基准测试
cargo bench
```

### 文档生成

```bash
# 生成API文档
cargo doc --no-deps --open

# 检查文档警告
cargo doc --no-deps 2>&1 | grep warning
```

## 贡献

欢迎贡献！请查看 [CONTRIBUTING.md](docs/CONTRIBUTING.md) 了解贡献指南。

## 许可证

[添加许可证信息]

## 相关链接

- [问题追踪](https://github.com/your-repo/issues)
- [讨论区](https://github.com/your-repo/discussions)

