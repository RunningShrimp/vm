# VM 项目架构设计文档

## 概述

本项目是一个高性能、跨架构的虚拟机实现，支持 AMD64、ARM64、RISC-V64 和 PowerPC64 架构。

## 核心组件

### 1. JIT 编译引擎

**位置**: `vm-engine-jit`

**功能**:
- 三层编译策略（L1 冷代码、L2 温代码、L3 热代码）
- 自适应优化阈值
- 共享代码池支持

**性能目标**:
- L1 编译: < 1ms
- L2 编译: < 10ms
- L3 编译: < 100ms

**关键文件**:
- `vm-engine-jit/src/jit.rs`: 主 JIT 实现
- `vm-engine-jit/src/aot.rs`: AOT 编译器
- `vm-engine-jit/src/adaptive.rs`: 自适应优化

### 2. 内存管理单元 (MMU)

**位置**: `vm-mem`

**功能**:
- 软件 MMU 实现
- 统一 TLB（支持所有架构）
- NUMA 感知内存分配器

**性能目标**:
- TLB 命中: < 5 ns
- TLB 未命中: < 50 ns
- TLB 命中率: > 95%

**关键文件**:
- `vm-mem/src/mmu.rs`: MMU 实现
- `vm-mem/src/tlb/unified_tlb.rs`: 统一 TLB
- `vm-mem/src/numa.rs`: NUMA 分配器

### 3. 跨架构支持

**位置**: `vm-cross-arch`

**功能**:
- 指令解码器（x86_64, ARM64, RISC-V64）
- 统一 IR 表示
- 架构翻译器
- 寄存器映射优化

**关键文件**:
- `vm-cross-arch/src/translator.rs`: 架构翻译
- `vm-cross-arch/src/register_mapping.rs`: 寄存器映射
- `vm-frontend/`: 指令解码器

### 4. 虚拟机服务

**位置**: `vm-service`

**功能**:
- VM 生命周期管理
- 快照管理
- 内核加载
- 异步 I/O 支持

**关键文件**:
- `vm-service/src/vm_service/service.rs`: VirtualMachineService
- `vm-service/src/vm_service/lifecycle.rs`: 生命周期管理
- `vm-service/src/vm_service/snapshot_manager.rs`: 快照管理

## 数据流图

```
用户代码 (x86_64/ARM64/RISC-V64)
    ↓
前端解码器 (vm-frontend)
    ↓
统一 IR (vm-ir)
    ↓
优化器 (vm-optimizers)
    ↓
JIT/AOT 编译 (vm-engine-jit)
    ↓
执行 (vm-engine-interpreter)
    ↓
内存访问 (vm-mem)
```

## 模块依赖关系

```
vm-core (核心类型和 Trait)
    ↓
├── vm-mem (内存管理)
├── vm-frontend (指令解码)
├── vm-ir (中间表示)
├── vm-engine-jit (JIT 编译)
├── vm-engine-interpreter (解释器)
├── vm-cross-arch (跨架构)
└── vm-service (服务层)
```

## Feature 标志

### 主要 Features

- **performance**: 启用 JIT、async、前端支持（推荐）
- **std**: 标准库支持
- **devices**: 设备模拟（CLINT, PLIC, virtio）
- **accel**: 硬件加速（KVM, HVF, WHPX）
- **smmu**: SMMUv3 支持
- **all-arch**: 所有架构支持

### Feature 依赖关系

```toml
[features]
default = ["std", "devices", "performance"]
performance = ["std", "vm-core/async", "vm-mem/async", "vm-engine-jit", "vm-frontend"]
async = ["performance"]  # 别名
jit = ["performance"]     # 别名
```

## 性能优化策略

### 1. 分层编译

- **L1 (Cold)**: 快速编译，无优化
- **L2 (Warm)**: 中等优化
- **L3 (Hot)**: 最大优化

### 2. 热点检测

- 基于执行计数
- 动态阈值调整
- 自动重新编译

### 3. 内存优化

- TLB 缓存
- 大页支持
- NUMA 感知分配

## 配置管理

### JIT 配置

```rust
use vm_engine_jit::AdaptiveThresholdConfig;

let config = AdaptiveThresholdConfig {
    cold_threshold: 100,
    hot_threshold: 1000,
    enable_adaptive: true,
    ..Default::default()
};
```

### MMU 配置

```rust
use vm_mem::{SoftMMU, PagingMode};

let mmu = SoftMMU::new(
    1024 * 1024 * 1024, // 1GB
    true,                // enable_tlb
    PagingMode::Sv39,    // RISC-V Sv39
);
```

## 扩展性

### 添加新架构支持

1. 在 `vm-frontend` 中实现解码器
2. 实现 Decoder trait
3. 在 `vm-cross-arch` 中添加翻译规则
4. 更新 GuestArch 枚举

### 添加新设备

1. 实现 `vm_device::BusDevice` trait
2. 在 `vm-service` 中注册设备
3. 处理 MMIO 访问

## 测试

### 运行测试

```bash
# 单元测试
cargo test --workspace

# 集成测试
cargo test --workspace --test integration_test

# 基准测试
cargo bench --bench vm_benchmark

# 覆盖率测试
cargo tarpaulin --workspace --out Html
```

### 性能回归检测

基准测试会自动与基线对比，检测性能回归。如果性能下降超过 10%，CI 将失败。

## 安全考虑

1. **内存隔离**: MMU 提供地址翻译和访问控制
2. **I/O 虚拟化**: 设备模拟层隔离真实硬件
3. **边界检查**: 所有内存访问都经过验证

## 故障排查

### JIT 编译失败

```rust
use log::debug;

// 启用 JIT 调试日志
RUST_LOG=vm_engine_jit=debug cargo run
```

### TLB 性能问题

```rust
// 检查 TLB 统计
let stats = mmu.get_tlb_stats();
println!("Hit rate: {:.2}%", stats.hit_rate * 100.0);
```

### 内存泄漏

```rust
// 使用 Valgrind 检测
valgrind --leak-check=full --show-leak-kinds=all ./vm
```

## 参考资源

- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [The Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [RISC-V Spec](https://riscv.org/technical/specifications/)
- [ARM64 Reference Manual](https://developer.arm.com/documentation/)

## 版本历史

- **v0.1.0**: 初始版本
  - 支持 x86_64, ARM64, RISC-V64
  - JIT 编译器
  - 基础设备模拟
  - Edition 2024

## 贡献指南

参见 [CONTRIBUTING.md](../CONTRIBUTING.md) 了解如何贡献代码。
