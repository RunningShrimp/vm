# 用户指南

## 更新时间
2025-12-04

## 概述

本文档提供虚拟机系统的用户指南，包括快速开始、基本使用、高级功能和常见问题。

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

## 核心概念

### 执行引擎

虚拟机支持多种执行引擎：

1. **解释器** (`ExecMode::Interpreter`): 纯解释执行，性能最低但实现最简单
2. **JIT编译器** (`ExecMode::Jit`): 即时编译执行，需要编译开销但执行快速
3. **硬件加速** (`ExecMode::Accelerated`): 使用硬件虚拟化（KVM/HVF/WHPX），性能最好
4. **混合模式** (`ExecMode::Hybrid`): 热点代码JIT编译，冷代码解释执行

### 内存管理

虚拟机使用MMU（内存管理单元）管理内存：

- **Guest虚拟地址** (`GuestAddr`): 虚拟机内部程序看到的虚拟地址
- **Guest物理地址** (`GuestPhysAddr`): 虚拟机内部的物理地址
- **Host虚拟地址** (`HostAddr`): 宿主机进程的虚拟地址

### 垃圾收集

虚拟机支持多种GC策略：

- **统一GC** (`UnifiedGC`): 整合所有GC实现的最佳特性
- **分代GC**: 支持年轻代和老年代，自动对象晋升
- **Card Marking**: 优化写屏障，减少开销

## 配置指南

### JIT配置

```rust
use vm_core::JitConfig;

let jit_config = JitConfig {
    enable_jit: true,
    jit_threshold: 100, // 热点阈值
    jit_cache_size: 32 * 1024 * 1024, // 32MB
    // ...
};
```

### GC配置

```rust
use vm_engine_jit::{UnifiedGC, UnifiedGcConfig};

let gc_config = UnifiedGcConfig {
    enable_generational: true,
    promotion_threshold: 3,
    use_card_marking: true,
    young_gen_ratio: 0.3,
    // ...
};

let gc = UnifiedGC::new(gc_config);
```

### AOT配置

```rust
use vm_core::AotConfig;

let aot_config = AotConfig {
    enable_aot: true,
    aot_image_path: Some("aot_image.bin".into()),
    aot_hotspot_threshold: 50,
    // ...
};
```

## 高级功能

### 异步执行

```rust
use vm_core::{AsyncExecutionEngine, AsyncMMU};
use vm_engine_jit::Jit;

#[tokio::main]
async fn main() {
    let mut jit = Jit::new();
    let mut mmu = /* ... */;
    let block = /* ... */;
    
    let result = jit.run_async(&mut mmu, &block).await;
}
```

### 图着色寄存器分配

```rust
use vm_engine_jit::{OptimizingJIT, RegisterAllocationStrategy, GraphColoringConfig};

let config = GraphColoringConfig {
    available_registers: 31,
    enable_coalescing: true,
    enable_spill_optimization: true,
    enable_priority_coloring: true,
};

let mut jit = OptimizingJIT::with_allocation_strategy(
    RegisterAllocationStrategy::GraphColoring
);
```

### AOT编译缓存

```rust
use vm_engine_jit::{AotCache, AotCacheConfig};
use std::path::PathBuf;

let cache_config = AotCacheConfig {
    max_entries: 10000,
    enable_persistence: true,
    persistence_path: Some(PathBuf::from("aot_cache.bin")),
    // ...
};

let cache = AotCache::new(cache_config);
```

### 协程池

```rust
use vm_runtime::{CoroutinePool, CoroutinePoolConfig, TaskPriority};

let pool_config = CoroutinePoolConfig {
    max_coroutines: 100,
    enable_work_stealing: true,
    enable_priority_scheduling: true,
    // ...
};

let pool = CoroutinePool::new(pool_config);
```

## 性能调优

### JIT优化

1. **调整热点阈值**: 降低阈值可以更早触发JIT编译，但会增加编译开销
2. **使用分层编译**: 根据代码执行频率选择不同的优化级别
3. **启用ML引导**: 使用机器学习模型指导编译决策

### GC优化

1. **调整GC配额**: 增加配额可以减少GC频率，但会增加暂停时间
2. **启用自适应调整**: 让GC根据实际情况自动调整参数
3. **使用分代GC**: 对于短生命周期对象较多的场景，分代GC效果更好

### 内存优化

1. **调整堆大小**: 根据实际需求设置合适的堆大小
2. **启用NUMA感知**: 在多NUMA节点系统上启用NUMA感知分配
3. **优化TLB**: 调整TLB大小和替换策略

## 常见问题

### Q: 如何选择合适的执行模式？

A: 
- **解释器**: 适合调试、小规模代码、快速启动
- **JIT**: 适合中等规模代码、需要平衡启动时间和执行性能
- **硬件加速**: 适合大规模代码、追求最佳性能
- **混合模式**: 适合不确定的场景，自动选择最佳策略

### Q: GC暂停时间过长怎么办？

A:
1. 启用增量GC，减少单次暂停时间
2. 调整GC配额，增加每次GC的处理量
3. 启用自适应调整，让GC自动优化参数
4. 使用分代GC，减少老年代GC频率

### Q: 如何调试虚拟机问题？

A:
1. 启用详细日志：`RUST_LOG=debug cargo run`
2. 使用GDB调试：虚拟机支持GDB远程调试协议
3. 检查错误信息：所有错误都使用统一的`VmError`类型
4. 查看统计信息：使用`get_stats()`方法获取详细统计

### Q: 如何提升性能？

A:
1. 使用JIT或硬件加速执行模式
2. 启用AOT编译缓存
3. 优化GC配置
4. 使用图着色寄存器分配
5. 启用ML引导优化

## 相关文档

- `docs/API_REFERENCE.md`: API参考文档
- `docs/API_EXAMPLES.md`: API使用示例
- `docs/PERFORMANCE_TUNING_GUIDE.md`: 性能调优指南
- `docs/TROUBLESHOOTING_GUIDE.md`: 故障排除指南
- `docs/ARCHITECTURE.md`: 架构文档

