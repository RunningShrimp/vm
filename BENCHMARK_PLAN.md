# 性能基准测试计划

**日期**: 2025-12-30
**状态**: P2任务 - 待实施

## 目标

建立全面的性能基准测试框架，用于：
- 监控JIT编译性能
- 测量内存分配性能
- 评估TLB查找效率
- 跟踪性能回归

## 现有基准测试

### vm-engine JIT性能基准
- 文件: `vm-engine/benches/jit_performance.rs`
- 状态: ✅ 已实现（60个基准测试）

### vm-mem TLB性能基准
- 文件: `vm-mem/benches/lockfree_tlb.rs`
- 文件: `vm-mem/benches/tlb_flush_advanced.rs`
- 文件: `vm-mem/benches/tlb_enhanced_stats_bench.rs`
- 状态: ✅ 已实现

## 需要添加的基准测试

### 1. 内存分配性能基准

**文件**: `vm-mem/benches/memory_allocation_bench.rs`

**测试内容**:
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};

use vm_mem::memory::MemoryPool;
use vm_mem::optimization::unified::{MemoryPool as OptimizationPool};

fn bench_memory_pool_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_pool_allocation");

    for size in [1024, 4096, 65536].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let mut pool = OptimizationPool::new();
            b.iter(|| {
                let addr = pool.allocate(size).unwrap();
                pool.deallocate(addr).ok();
                black_box(addr)
            });
        });
    }

    group.finish();
}

fn bench_memory_pool_reallocation(c: &mut Criterion) {
    c.bench_function("memory_pool_reuse", |b| {
        let mut pool = OptimizationPool::new();
        let addr = pool.allocate(4096).unwrap();
        pool.deallocate(addr).unwrap();

        b.iter(|| {
            let new_addr = pool.allocate(4096).unwrap();
            pool.deallocate(new_addr).ok();
            black_box(new_addr)
        });
    });
}

criterion_group!(memory_benches, bench_memory_pool_allocation, bench_memory_pool_reallocation);
criterion_main!(memory_benches);
```

### 2. JIT编译性能基准

**文件**: `vm-engine/benches/jit_compilation_bench.rs`

**测试内容**:
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use vm_engine::jit::JITCompiler;
use vm_ir::IRBlock;
use vm_core::GuestAddr;

fn bench_jit_small_block(c: &mut Criterion) {
    c.bench_function("jit_compile_small_block", |b| {
        let mut compiler = JITCompiler::new();
        let block = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops: vec![
                // 10条简单指令
            ],
            term: vm_ir::Terminator::Ret,
        };

        b.iter(|| {
            black_box(compiler.compile(black_box(&block)))
        });
    });
}

fn bench_jit_large_block(c: &mut Criterion) {
    c.bench_function("jit_compile_large_block", |b| {
        let mut compiler = JITCompiler::new();
        let block = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops: vec![
                // 100条指令
            ],
            term: vm_ir::Terminator::Ret,
        };

        b.iter(|| {
            black_box(compiler.compile(black_box(&block)))
        });
    });
}

criterion_group!(jit_benches, bench_jit_small_block, bench_jit_large_block);
criterion_main!(jit_benches);
```

### 3. 跨架构翻译性能基准

**文件**: `benches/cross_arch_translation_bench.rs`

**测试内容**:
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_x86_to_arm(c: &mut Criterion) {
    c.bench_function("x86_to_arm_translation", |b| {
        b.iter(|| {
            // 跨架构翻译基准测试
        });
    });
}

fn bench_x86_to_riscv(c: &mut Criterion) {
    c.bench_function("x86_to_riscv_translation", |b| {
        b.iter(|| {
            // 跨架构翻译基准测试
        });
    });
}

criterion_group!(cross_arch_benches, bench_x86_to_arm, bench_x86_to_riscv);
criterion_main!(cross_arch_benches);
```

## 实施步骤

### 阶段1: 设置基础设施
1. ✅ 确认criterion.rs已添加到依赖
2. 创建基准测试目录结构
3. 配置Cargo.toml

### 阶段2: 实现核心基准
1. 创建内存分配基准测试
2. 创建JIT编译基准测试
3. 创建跨架构翻译基准测试

### 阶段3: CI/CD集成
1. 自动运行基准测试
2. 生成性能报告
3. 设置性能回归检测

### 阶段4: 持续监控
1. 建立性能基线
2. 设置性能回归告警
3. 生成性能趋势图

## 运行基准测试

```bash
# 运行所有基准测试
cargo bench

# 运行特定基准
cargo bench --bench memory_allocation_bench
cargo bench --bench jit_compilation_bench

# 保存基准结果
cargo bench -- --save-baseline main

# 与基线比较
cargo bench -- --baseline main
```

## 性能目标

### JIT编译性能
- 小代码块（<100条指令）: < 1ms
- 中等代码块（100-1000条指令）: < 10ms
- 大代码块（>1000条指令）: < 100ms

### 内存分配性能
- 单次分配: < 1μs
- 批量分配（100次）: < 100μs
- 分配+释放周期: < 2μs

### TLB查找性能
- 单次查找: < 100ns
- 批量查找（1000次）: < 100μs
- TLB未命中处理: < 500ns

## 当前状态

- ✅ 基础基准测试已实现（JIT, TLB）
- ⏳ 需要添加：内存分配、完整性能监控
- ⏳ 需要CI/CD集成

## 下一步行动

1. 实现上述基准测试
2. 运行并建立性能基线
3. 集成到CI/CD流水线
4. 设置性能回归监控

---

**预计完成时间**: 4-8小时
**优先级**: P2
**状态**: ⏳ 计划中
