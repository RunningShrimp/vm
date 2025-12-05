# 快速开始指南

## 更新时间
2025-12-04

## 概述

本文档提供虚拟机系统的快速开始指南，帮助用户快速上手使用。

## 安装

### 前置要求

- Rust 1.70+ (推荐使用最新稳定版)
- Cargo
- 支持的平台：Linux, macOS, Windows

### 构建项目

```bash
# 克隆仓库
git clone <repository-url>
cd vm

# 构建项目（调试版本）
cargo build

# 构建项目（发布版本）
cargo build --release
```

## 第一个程序

### 创建简单的IR块

```rust
use vm_engine_jit::Jit;
use vm_core::{ExecutionEngine, GuestAddr, MMU};
use vm_ir::{IRBlock, IROp, Terminator};
use vm_mem::SoftMmu;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建MMU
    let mut mmu = SoftMmu::new(1024 * 1024, false); // 1MB内存
    
    // 创建JIT引擎
    let mut jit = Jit::new();
    
    // 创建IR块：计算 10 + 20
    let block = IRBlock {
        start_pc: 0x1000,
        ops: vec![
            IROp::MovImm { dst: 1, imm: 10 },
            IROp::MovImm { dst: 2, imm: 20 },
            IROp::Add { dst: 3, src1: 1, src2: 2 },
        ],
        term: Terminator::Ret,
    };
    
    // 设置PC
    jit.set_pc(0x1000);
    
    // 执行代码块
    let result = jit.run(&mut mmu, &block)?;
    
    // 获取结果（寄存器3应该包含30）
    let sum = jit.get_reg(3);
    println!("Result: {}", sum);
    
    Ok(())
}
```

### 运行

```bash
cargo run --example simple_execution
```

## 常见使用场景

### 场景1: 使用优化型JIT编译器

```rust
use vm_engine_jit::{OptimizingJIT, RegisterAllocationStrategy};

let mut jit = OptimizingJIT::with_allocation_strategy(
    RegisterAllocationStrategy::GraphColoring
);

let code_ptr = jit.compile(&block);
```

### 场景2: 使用分代GC

```rust
use vm_engine_jit::{UnifiedGC, UnifiedGcConfig};

let config = UnifiedGcConfig {
    enable_generational: true,
    promotion_threshold: 3,
    use_card_marking: true,
    // ...
};

let gc = UnifiedGC::new(config);
let roots = vec![0x1000, 0x2000];

// Minor GC
let promoted = gc.minor_gc(&roots);
```

### 场景3: 使用AOT缓存

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

// 查找缓存
if let Some(code) = cache.lookup(&block, 2, "x86_64") {
    // 使用缓存的编译结果
}
```

### 场景4: 使用协程池

```rust
use vm_runtime::{CoroutinePool, CoroutinePoolConfig, TaskPriority};

let config = CoroutinePoolConfig {
    max_coroutines: 100,
    enable_work_stealing: true,
    enable_priority_scheduling: true,
    // ...
};

let pool = CoroutinePool::new(config);

// 提交任务
let task_id = pool.spawn_with_priority(
    async {
        // 任务代码
    },
    TaskPriority::High
).await?;
```

## 下一步

- 阅读 [用户指南](USER_GUIDE.md) 了解更多功能
- 查看 [API参考](API_REFERENCE.md) 了解完整API
- 阅读 [API示例](API_EXAMPLES.md) 查看更多示例
- 查看 [性能调优指南](PERFORMANCE_TUNING_GUIDE.md) 优化性能

## 获取帮助

- 查看 [故障排除指南](TROUBLESHOOTING_GUIDE.md)
- 查看 [架构文档](ARCHITECTURE.md) 了解系统设计
- 提交 [Issue](https://github.com/your-repo/issues) 报告问题

