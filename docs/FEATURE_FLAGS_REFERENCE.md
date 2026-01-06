# VM项目Feature Flags完整参考

**生成时间**: 2026-01-06 13:12:11
**最后更新**: 2026-01-06

本文档提供了VM项目中所有crate的feature flags完整参考，包括说明、依赖关系和使用示例。

---

## 📋 目录

- [概述](#概述)
- [分类索引](#分类索引)
- [常用组合](#常用组合)
- [详细参考](#详细参考)
- [使用示例](#使用示例)

---

## 概述

VM项目使用Rust的feature flags系统来控制：
- **平台支持**: x86_64、ARM64、RISC-V64
- **编译后端**: Cranelift、LLVM
- **加速功能**: KVM、HVF、WHPX、SIMD
- **可选组件**: GPU、网络、设备直通
- **调试功能**: 日志、追踪、性能分析

### Feature Flags设计原则

1. **默认启用**: 默认features提供最常用功能
2. **可选特性**: 高级功能通过features启用
3. **向后兼容**: 废弃的features保留别名
4. **清晰命名**: 使用描述性的feature名称

---

## 分类索引

### 🚀 性能优化
- `acceleration`: 硬件加速 (KVM/HVF/WHPX)
- `simd`: SIMD向量操作
- `cpu-detection`: CPU特性检测
- `jit`: JIT编译支持
- `aot`: AOT预编译

### 🖥️ 平台支持
- `x86_64`: AMD64/x86_64架构
- `arm64`: ARM64架构
- `riscv64`: RISC-V64架构

### 🔧 编译后端
- `cranelift-backend`: Cranelift JIT编译器
- `llvm-backend`: LLVM JIT编译器
- `llvm-jit`: LLVM集成 (使用inkwell)

### 🎮 GPU加速
- `cuda`: NVIDIA CUDA支持
- `rocm`: AMD ROCm支持
- `gpu`: 所有GPU支持 (cuda + rocm)

### 🌐 网络功能
- `smoltcp`: TCP/IP网络栈
- `smmu`: IOMMU设备DMA支持

### 🔍 调试与监控
- `debug`: 调试功能
- `tracing`: 性能追踪
- `logging`: 日志记录
- `profiling`: 性能分析

---

## 常用组合

### 最小化构建（无加速）
```toml
[dependencies]
vm-core = { version = "0.1", default-features = false, features = ["std"] }
vm-engine = { version = "0.1", default-features = false }
```

### 完整功能构建（所有加速）
```toml
[dependencies]
vm-core = { version = "0.1", features = ["std", "async", "gpu"] }
vm-engine = { version = "0.1", features = ["jit", "aot"] }
vm-engine-jit = { version = "0.1", features = ["cranelift-backend", "simd", "cpu-detection"] }
vm-accel = { version = "0.1", features = ["acceleration"] }
```

### KVM加速（Linux）
```toml
[dependencies]
vm-accel = { version = "0.1", features = ["acceleration"] }
vm-device = { version = "0.1", features = ["smmu"] }
```

### ARM64平台优化
```toml
[dependencies]
vm-core = { version = "0.1", features = ["std", "arm64"] }
vm-frontend = { version = "0.1", features = ["arm64-frontend"] }
```

### GPU计算（需要CUDA/ROCm）
```toml
[dependencies]
vm-core = { version = "0.1", features = ["gpu"] }
vm-passthrough = { version = "0.1", features = ["cuda"] }  # 或 "rocm"
```

---

## 使用示例

## accel

```toml
default = ["acceleration"]
# Acceleration features (merged: hardware, smmu)
acceleration = ["raw-cpuid", "dep:kvm-ioctls", "dep:kvm-bindings", "dep:vm-smmu"]
# Legacy feature aliases (deprecated, use "acceleration" instead)
hardware = ["acceleration"]
smmu = ["acceleration"]
```

## boot

```toml
default = []
# Feature flags removed: uefi, bios, direct-boot (not used in code)
```

## build-deps

```toml
```

## cli

```toml
```

## codegen

```toml
```

## core

```toml
default = ["std"]
std = []
async = ["tokio", "futures", "async-trait"]
# Architecture features - used by macros
x86_64 = []
arm64 = []
riscv64 = []
# Event sourcing feature
enhanced-event-sourcing = ["chrono", "tokio"]
# Optimization application feature
optimization_application = []
# GPU acceleration features (placeholder - actual implementation in vm-passthrough)
cuda = []
rocm = []
gpu = ["cuda", "rocm"]
```

## cross-arch-support

```toml
default = ["std"]
std = []
```

## debug

```toml
```

## desktop

```toml
default = []
```

## device

```toml
default = ["std"]
std = []
# Network stack support (using smoltcp)
smoltcp = ["dep:smoltcp"]
# SMMU support (IOMMU for device DMA)
smmu = ["dep:vm-smmu", "vm-accel/smmu"]
```

## engine-jit

```toml
jit = []
cranelift-backend = []
async = ["vm-core/async"]
cpu-detection = ["dep:raw-cpuid"]  # CPU特性检测
simd = []  # SIMD向量操作支持（实验性）
default = ["cranelift-backend", "cpu-detection"]
```

## engine

```toml
default = ["std", "interpreter"]
# Standard library support
std = ["serde_json", "vm-core/std"]
# Execution engines
# Note: Both interpreter and JIT are always compiled, but features control optimizations
interpreter = []
jit = []  # JIT is compiled-in, see src/jit/mod.rs for JIT-specific code
# Full JIT engine with vm-engine-jit integration (方案C: Feature统一)
jit-full = ["jit", "vm-engine-jit"]
# Executor (async execution)
executor = ["async"]
# Debugging support
debug = ["std"]
# Async support
async = ["futures", "async-trait", "vm-core/async"]
# Combined features
all-engines = ["interpreter", "jit"]
all-engines-full = ["interpreter", "jit-full"]
# Experimental features
experimental = ["executor"]
```

## frontend

```toml
default = ["riscv64"]
# Single architecture features
x86_64 = []
arm64 = ["vm-accel"]  # ARM64 needs vm-accel for CPU detection
riscv64 = []
# RISC-V extensions
riscv-m = ["riscv64"]
riscv-f = ["riscv64"]
riscv-d = ["riscv64"]
riscv-c = ["riscv64"]
riscv-a = ["riscv64"]
# Multi-architecture combinations
all = ["x86_64", "arm64", "riscv64"]
all-extensions = ["all", "riscv-m", "riscv-f", "riscv-d", "riscv-c", "riscv-a"]
# Parallel processing support
parallel = []
# Dependencies
vm-mem = ["dep:vm-mem"]
vm-accel = ["dep:vm-accel"]
```

## gc

```toml
default = []
# Enable generational GC
generational = []
# Enable incremental GC
incremental = []
# Enable adaptive GC
adaptive = ["generational", "incremental"]
# Enable GC statistics and profiling
stats = []
# Enable benchmarking support
benchmarking = ["stats"]
# Benchmark configuration - disabled until benchmark file is created
# [[bench]]
# name = "gc_benchmark"
# harness = false
# required-features = ["benchmarking"]
```

## graphics

```toml
default = []
# Vulkan support (requires Vulkan SDK)
vulkan = []
# All graphics features
all-graphics = ["vulkan"]
```

## ir

```toml
default = []
llvm = ["inkwell", "llvm-sys"]
```

## mem

```toml
default = ["std", "optimizations"]
# Standard library support
std = []
# Fine-grained optimization features
opt-simd = []
opt-tlb = []
opt-numa = []
opt-prefetch = []
opt-concurrent = []
# Combined optimizations (included in default for backward compatibility)
optimizations = ["opt-simd", "opt-tlb", "opt-numa"]
# Async support
async = ["tokio", "async-trait"]
# Legacy feature aliases (deprecated)
tlb = ["opt-tlb"]
```

## monitor

```toml
```

## optimizers

```toml
async = ["tokio", "num_cpus"]
default = []
```

## osal

```toml
```

## passthrough

```toml
default = []
# CUDA GPU support (requires CUDA SDK)
cuda = ["cudarc"]
# ROCm GPU support (requires ROCm SDK)
rocm = []
# ARM NPU support (experimental)
npu = []
# All GPU/NPU features (for convenience)
gpu = ["cuda", "rocm"]
all-accelerators = ["cuda", "rocm", "npu"]
```

## platform

```toml
```

## plugin

```toml
default = []
# Remote plugin repository support (requires network)
repository = ["reqwest"]
```

## service

```toml
default = ["std", "devices", "performance"]
std = []
# Performance features (merged: jit, async, frontend)
# Note: Uses all architectures by default
performance = ["std", "vm-core/async", "vm-mem/async", "vm-engine/jit", "vm-frontend/all"]
# Device support (CLINT, PLIC, virtio devices)
devices = ["vm-device"]
# Engine support (JIT and interpreter)
vm-engine = ["vm-engine/interpreter", "vm-engine/jit"]
# Frontend decoder support (single architecture)
frontend = ["vm-frontend"]
frontend-x86_64 = ["frontend", "vm-frontend/x86_64"]
frontend-arm64 = ["frontend", "vm-frontend/arm64"]
frontend-riscv64 = ["frontend", "vm-frontend/riscv64"]
# All architectures
all-arch = ["frontend", "vm-frontend/all"]
# RISC-V extensions
riscv-extensions = ["all-arch", "vm-frontend/all-extensions"]
# Legacy feature aliases (deprecated, use "performance" instead)
async = ["performance"]
```

## smmu

```toml
default = ["mmu", "atsu", "tlb", "interrupt"]
# All SMMU components enabled by default (they are part of the SMMUv3 specification)
mmu = []
atsu = []
tlb = []
interrupt = []
```

## soc

```toml
default = []
npu = []
dynamiq = []
huge_pages = []
```


### 在Cargo.toml中使用

```toml
[dependencies.vm-core]
version = "0.1"
default-features = false  # 禁用默认features
features = ["std", "arm64", "async"]  # 选择需要的features
```

### 在命令行中使用

```bash
# 启用特定features构建
cargo build --features "vm-core/gpu,vm-engine-jit/simd"

# 禁用默认features
cargo build --no-default-features

# 启用所有features
cargo build --all-features
```

### 在工作空间中使用

```toml
# Cargo.toml
[workspace.dependencies]
vm-core = { path = "vm-core", features = ["std", "async"] }
vm-engine-jit = { path = "vm-engine-jit", features = ["cranelift-backend", "simd"] }
```

### 条件编译

在Rust代码中使用feature gates：

```rust
#[cfg(feature = "simd")]
mod simd_optimizations {
    // SIMD优化代码
}

#[cfg(feature = "gpu")]
fn use_gpu_acceleration() {
    // GPU加速代码
}

#[cfg(not(feature = "gpu"))]
fn use_gpu_acceleration() {
    // CPU回退代码
}
```

---

## 依赖关系图

```
vm-core
├── std (默认)
├── async → [tokio, futures, async-trait]
├── x86_64/arm64/riscv64 (平台特性)
├── gpu → [cuda, rocm]
└── optimization_application

vm-engine-jit
├── cranelift-backend (默认)
├── simd (实验性)
├── cpu-detection (默认)
└── async → vm-core/async

vm-accel
├── acceleration (默认) → [raw-cpuid, kvm-ioctls, kvm-bindings, vm-smmu]
├── hardware (废弃别名)
└── smmu (废弃别名)

vm-device
├── std (默认)
├── smoltcp → [dep:smoltcp]
└── smmu → [vm-smmu, vm-accel/smmu]
```

---

## 注意事项

### 废弃的Features

以下features已废弃，应使用替代方案：

| 废弃Feature | 替代方案 | 说明 |
|------------|---------|------|
| `vm-accel/hardware` | `acceleration` | 重命名为更清晰的名称 |
| `vm-accel/smmu` | `acceleration` | 合并到主加速feature |
| `vm-boot/uefi` | (已移除) | 未使用的功能 |

### 平台特定Features

某些features仅在特定平台上可用：

- `kvm-ioctls`: 仅Linux
- `hf`: 仅macOS
- `whpx`: 仅Windows
- `cuda`: 需要NVIDIA GPU和CUDA Toolkit
- `rocm`: 需要AMD GPU和ROCm

### 性能考虑

启用所有features可能会：
- 增加编译时间
- 增加二进制大小
- 引入不必要的依赖

**建议**: 仅启用实际需要的features

---

## 最佳实践

1. **明确指定features**: 始终在Cargo.toml中明确列出需要的features
2. **使用feature组合**: 为常见用例创建feature组合
3. **文档化自定义features**: 为添加的features提供清晰的文档
4. **测试feature组合**: 确保不同的feature组合都能正常工作
5. **保持向后兼容**: 当修改features时，保留旧features作为别名

---

## 更新日志

### 2026-01-06
- 创建完整的feature flags参考文档
- 添加分类索引和使用示例
- 记录所有crate的features

---

**维护者**: VM项目团队
**问题反馈**: 请在GitHub Issues中报告问题或提出建议
