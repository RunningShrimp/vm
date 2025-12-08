//! JIT编译器和GC性能基准测试
//!
//! 使用criterion进行性能基准测试，包括：
//! - JIT编译性能基准
//! - GC性能基准
//! - 缓存性能基准
//! - 热点检测性能基准
//! - 集成性能基准

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use vm_core::GuestAddr;
use vm_engine_jit::Jit;
use vm_engine_jit::optimizing_compiler::{
    RegisterAllocator, InstructionScheduler, OptimizationPassManager
};
use vm_engine_jit::unified_gc::{
    UnifiedGc, UnifiedGcConfig, UnifiedGcStats
};
use vm_engine_jit::unified_cache::{
    UnifiedCodeCache, CacheConfig, EvictionPolicy
};
use vm_engine_jit::ewma_hotspot::{EwmaHotspotDetector, EwmaHotspotConfig};
use vm_ir::{IRBlock, IRBuilder, IROp, Terminator};

/// 创建基准测试用的IR块
fn create_benchmark_ir_block(addr: GuestAddr, size: usize) -> IRBlock {
    let mut builder = IRBuilder::new(addr);
    
    for i in 1..=size {
        builder.push(IROp::MovImm { dst: i as u32, imm: i as u64 });
        if i > 1 {
            builder.push(IROp::Add {
                dst: i as u32,
                src1: (i - 1) as u32,
                src2: 1,
            });
            builder.push(IROp::Mul {
                dst: (i + size) as u32,
                src1: i as u32,
                src2: (i - 1) as u32,
            });
        }
    }
    
    builder.set_term(Terminator::Ret);
    builder.build()
}

/// 创建基准测试用的配置
fn create_benchmark_gc_config() -> UnifiedGcConfig {
    UnifiedGcConfig {
        young_gen: vm_engine_jit::unified_gc::YoungGenConfig {
            initial_size: 4 * 1024 * 1024,      // 4MB
            max_size: 64 * 1024 * 1024,         // 64MB
            promotion_threshold: 50,
            ..Default::default()
        },
        old_gen: vm_engine_jit::unified_gc::OldGenConfig {
            initial_size: 16 * 1024 * 1024,  // 16MB
            max_size: 256 * 1024 * 1024,     // 256MB
            ..Default::default()
        },
        concurrent: vm_engine_jit::unified_gc::ConcurrentConfig {
            enabled: true,
            marking_threads: 2,
            sweeping_threads: 1,
            ..Default::default()
        },
        adaptive: vm_engine_jit::unified_gc::AdaptiveConfig {
            enabled: true,
            target_pause_time_ms: 10,
            ..Default::default()
        },
        ..Default::default()
    }
}

fn create_benchmark_cache_config() -> CacheConfig {
    CacheConfig {
        max_entries: 10000,
        max_memory_bytes: 100 * 1024 * 1024, // 100MB
        eviction_policy: EvictionPolicy::LRU_LFU,
        cleanup_interval_secs: 60,
        hotness_decay_factor: 0.99,
        warmup_size: 1000,
    }
}

fn create_benchmark_hotspot_config() -> EwmaHotspotConfig {
    EwmaHotspotConfig {
        frequency_alpha: 0.3,
        execution_time_alpha: 0.2,
        hotspot_threshold: 1.0,
        min_execution_count: 10,
        min_execution_time_us: 5,
        frequency_weight: 0.4,
        execution_time_weight: 0.4,
        complexity_weight: 0.2,
        base_threshold: 100,
        time_window_ms: 1000,
        decay_factor: 0.95,
    }
}

// ============================================================================
// JIT编译性能基准
// ============================================================================

fn bench_jit_compilation_small(c: &mut Criterion) {
    let mut jit = Jit::new();
    let block = create_benchmark_ir_block(0x1000, 10);
    
    c.bench_function("jit_compilation_small", |b| {
        b.iter(|| {
            let compiled = jit.compile_block(black_box(&block));
            black_box(compiled)
        })
    });
}

fn bench_jit_compilation_medium(c: &mut Criterion) {
    let mut jit = Jit::new();
    let block = create_benchmark_ir_block(0x1000, 50);
    
    c.bench_function("jit_compilation_medium", |b| {
        b.iter(|| {
            let compiled = jit.compile_block(black_box(&block));
            black_box(compiled)
        })
    });
}

fn bench_jit_compilation_large(c: &mut Criterion) {
    let mut jit = Jit::new();
    let block = create_benchmark_ir_block(0x1000, 200);
    
    c.bench_function("jit_compilation_large", |b| {
        b.iter(|| {
            let compiled = jit.compile_block(black_box(&block));
            black_box(compiled)
        })
    });
}

fn bench_jit_compilation_throughput(c: &mut Criterion) {
    let mut jit = Jit::new();
    let blocks: Vec<_> = (0..100)
        .map(|i| create_benchmark_ir_block(0x1000 + i * 0x100, 25))
        .collect();
    
    let mut group = c.benchmark_group("jit_compilation_throughput");
    group.throughput(Throughput::Elements(blocks.len() as u64));
    
    group.bench_function("jit_compilation_throughput", |b| {
        b.iter(|| {
            for block in &blocks {
                let compiled = jit.compile_block(black_box(block));
                black_box(compiled);
            }
        })
    });
    
    group.finish();
}

// ============================================================================
// 寄存器分配性能基准
// ============================================================================

fn bench_register_allocation_linear_scan(c: &mut Criterion) {
    let mut allocator = RegisterAllocator::new();
    let block = create_benchmark_ir_block(0x1000, 50);
    
    c.bench_function("register_allocation_linear_scan", |b| {
        b.iter(|| {
            allocator.analyze_lifetimes(black_box(&block.ops));
            let allocations = allocator.allocate_registers(black_box(&block.ops));
            black_box(allocations)
        })
    });
}

fn bench_register_allocation_graph_coloring(c: &mut Criterion) {
    let mut allocator = RegisterAllocator::new();
    let block = create_benchmark_ir_block(0x1000, 50);
    
    c.bench_function("register_allocation_graph_coloring", |b| {
        b.iter(|| {
            allocator.analyze_lifetimes(black_box(&block.ops));
            let allocations = allocator.allocate_registers(black_box(&block.ops));
            black_box(allocations)
        })
    });
}

fn bench_register_allocation_spill_heavy(c: &mut Criterion) {
    let mut allocator = RegisterAllocator::new();
    // 创建需要大量寄存器的块（会导致溢出）
    let block = create_benchmark_ir_block(0x1000, 100);
    
    c.bench_function("register_allocation_spill_heavy", |b| {
        b.iter(|| {
            allocator.analyze_lifetimes(black_box(&block.ops));
            let allocations = allocator.allocate_registers(black_box(&block.ops));
            black_box(allocations)
        })
    });
}

// ============================================================================
// 指令调度性能基准
// ============================================================================

fn bench_instruction_scheduling_simple(c: &mut Criterion) {
    let mut scheduler = InstructionScheduler::new();
    let block = create_benchmark_ir_block(0x1000, 20);
    
    c.bench_function("instruction_scheduling_simple", |b| {
        b.iter(|| {
            scheduler.build_dependency_graph(black_box(&block.ops));
            let scheduled = scheduler.schedule(black_box(&block.ops));
            black_box(scheduled)
        })
    });
}

fn bench_instruction_scheduling_complex(c: &mut Criterion) {
    let mut scheduler = InstructionScheduler::new();
    let block = create_benchmark_ir_block(0x1000, 100);
    
    c.bench_function("instruction_scheduling_complex", |b| {
        b.iter(|| {
            scheduler.build_dependency_graph(black_box(&block.ops));
            let scheduled = scheduler.schedule(black_box(&block.ops));
            black_box(scheduled)
        })
    });
}

// ============================================================================
// 优化Pass性能基准
// ============================================================================

fn bench_optimization_passes_constant_folding(c: &mut Criterion) {
    let mut manager = OptimizationPassManager::new();
    let mut block = create_benchmark_ir_block(0x1000, 30);
    
    // 添加常量表达式
    for i in 1..=10 {
        block.ops.push(IROp::MovImm { dst: (i + 30) as u32, imm: i as u64 });
        block.ops.push(IROp::MovImm { dst: (i + 40) as u32, imm: (i * 2) as u64 });
        block.ops.push(IROp::Add {
            dst: (i + 50) as u32,
            src1: (i + 30) as u32,
            src2: (i + 40) as u32,
        });
    }
    
    c.bench_function("optimization_passes_constant_folding", |b| {
        b.iter(|| {
            let mut block_copy = block.clone();
            manager.run_optimizations(black_box(&mut block_copy));
            black_box(block_copy)
        })
    });
}

fn bench_optimization_passes_dead_code_elimination(c: &mut Criterion) {
    let mut manager = OptimizationPassManager::new();
    let mut block = create_benchmark_ir_block(0x1000, 50);
    
    // 添加死代码
    for i in 1..=20 {
        block.ops.push(IROp::MovImm { dst: (i + 100) as u32, imm: i as u64 });
        // 这些寄存器永远不会被使用
    }
    
    c.bench_function("optimization_passes_dead_code_elimination", |b| {
        b.iter(|| {
            let mut block_copy = block.clone();
            manager.run_optimizations(black_box(&mut block_copy));
            black_box(block_copy)
        })
    });
}

// ============================================================================
// GC性能基准
// ============================================================================

fn bench_gc_allocation_young_gen(c: &mut Criterion) {
    let config = create_benchmark_gc_config();
    let gc = UnifiedGc::new(config);
    
    c.bench_function("gc_allocation_young_gen", |b| {
        b.iter(|| {
            for i in 0..100 {
                let addr = 0x1000 + i * 0x100;
                gc.allocate_young(addr, 64);
            }
            black_box(())
        })
    });
}

fn bench_gc_allocation_old_gen(c: &mut Criterion) {
    let config = create_benchmark_gc_config();
    let gc = UnifiedGc::new(config);
    
    c.bench_function("gc_allocation_old_gen", |b| {
        b.iter(|| {
            for i in 0..50 {
                let addr = 0x1000 + i * 0x100;
                gc.allocate_old(addr, 128);
            }
            black_box(())
        })
    });
}

fn bench_gc_collection_young_gen(c: &mut Criterion) {
    let config = create_benchmark_gc_config();
    let gc = UnifiedGc::new(config);
    
    // 预分配对象
    for i in 0..200 {
        let addr = 0x1000 + i * 0x100;
        gc.allocate_young(addr, 64);
    }
    
    c.bench_function("gc_collection_young_gen", |b| {
        b.iter(|| {
            let roots: Vec<_> = (0..200).map(|i| 0x1000 + i * 0x100).collect();
            gc.collect_young(black_box(&roots));
            black_box(())
        })
    });
}

fn bench_gc_collection_full(c: &mut Criterion) {
    let config = create_benchmark_gc_config();
    let gc = UnifiedGc::new(config);
    
    // 预分配对象
    for i in 0..300 {
        let addr = 0x1000 + i * 0x100;
        gc.allocate(addr, 64);
    }
    
    c.bench_function("gc_collection_full", |b| {
        b.iter(|| {
            let roots: Vec<_> = (0..300).map(|i| 0x1000 + i * 0x100).collect();
            gc.collect_full(black_box(&roots));
            black_box(())
        })
    });
}

fn bench_gc_concurrent_marking(c: &mut Criterion) {
    let config = create_benchmark_gc_config();
    let gc = UnifiedGc::new(config);
    
    // 预分配对象
    for i in 0..500 {
        let addr = 0x1000 + i * 0x100;
        gc.allocate(addr, 64);
    }
    
    c.bench_function("gc_concurrent_marking", |b| {
        b.iter(|| {
            let roots: Vec<_> = (0..500).map(|i| 0x1000 + i * 0x100).collect();
            gc.start_concurrent_marking(black_box(&roots));
            black_box(())
        })
    });
}

// ============================================================================
// 缓存性能基准
// ============================================================================

fn bench_cache_lookup_lru(c: &mut Criterion) {
    let mut config = create_benchmark_cache_config();
    config.eviction_policy = EvictionPolicy::LRU;
    
    let hotspot_config = create_benchmark_hotspot_config();
    let cache = UnifiedCodeCache::new(config, hotspot_config);
    
    // 预填充缓存
    for i in 0..1000 {
        let addr = 0x1000 + i * 0x100;
        let code = vec![0x90; 64];
        cache.insert_sync(addr, code, false);
    }
    
    c.bench_function("cache_lookup_lru", |b| {
        b.iter(|| {
            for i in 0..1000 {
                let addr = 0x1000 + i * 0x100;
                let result = cache.lookup(black_box(addr));
                black_box(result);
            }
        })
    });
}

fn bench_cache_lookup_lfu(c: &mut Criterion) {
    let mut config = create_benchmark_cache_config();
    config.eviction_policy = EvictionPolicy::LFU;
    
    let hotspot_config = create_benchmark_hotspot_config();
    let cache = UnifiedCodeCache::new(config, hotspot_config);
    
    // 预填充缓存
    for i in 0..1000 {
        let addr = 0x1000 + i * 0x100;
        let code = vec![0x90; 64];
        cache.insert_sync(addr, code, false);
    }
    
    c.bench_function("cache_lookup_lfu", |b| {
        b.iter(|| {
            for i in 0..1000 {
                let addr = 0x1000 + i * 0x100;
                let result = cache.lookup(black_box(addr));
                black_box(result);
            }
        })
    });
}

fn bench_cache_insertion(c: &mut Criterion) {
    let config = create_benchmark_cache_config();
    let hotspot_config = create_benchmark_hotspot_config();
    let cache = UnifiedCodeCache::new(config, hotspot_config);
    
    c.bench_function("cache_insertion", |b| {
        b.iter(|| {
            for i in 0..100 {
                let addr = 0x1000 + i * 0x100;
                let code = vec![0x90; 64];
                cache.insert_sync(black_box(addr), code, false);
            }
        })
    });
}

fn bench_cache_eviction(c: &mut Criterion) {
    let mut config = create_benchmark_cache_config();
    config.max_entries = 100; // 小缓存以触发淘汰
    
    let hotspot_config = create_benchmark_hotspot_config();
    let cache = UnifiedCodeCache::new(config, hotspot_config);
    
    c.bench_function("cache_eviction", |b| {
        b.iter(|| {
            // 插入超过限制的条目
            for i in 0..200 {
                let addr = 0x1000 + i * 0x100;
                let code = vec![0x90; 64];
                cache.insert_sync(black_box(addr), code, false);
            }
        })
    });
}

// ============================================================================
// 热点检测性能基准
// ============================================================================

fn bench_hotspot_detection_basic(c: &mut Criterion) {
    let config = create_benchmark_hotspot_config();
    let detector = EwmaHotspotDetector::new(config);
    
    c.bench_function("hotspot_detection_basic", |b| {
        b.iter(|| {
            for i in 0..1000 {
                let addr = 0x1000 + i * 0x100;
                detector.record_execution(black_box(addr), 50);
            }
        })
    });
}

fn bench_hotspot_detection_multidimensional(c: &mut Criterion) {
    let config = create_benchmark_hotspot_config();
    let detector = EwmaHotspotDetector::new(config);
    
    c.bench_function("hotspot_detection_multidimensional", |b| {
        b.iter(|| {
            for i in 0..1000 {
                let addr = 0x1000 + i * 0x100;
                let complexity = 1.0 + (i % 5) as f64 * 0.5;
                detector.record_execution_with_complexity(black_box(addr), 50, complexity);
            }
        })
    });
}

fn bench_hotspot_detection_adaptive_threshold(c: &mut Criterion) {
    let config = create_benchmark_hotspot_config();
    let detector = EwmaHotspotDetector::new(config);
    
    c.bench_function("hotspot_detection_adaptive_threshold", |b| {
        b.iter(|| {
            for i in 0..500 {
                let addr = 0x1000 + i * 0x100;
                detector.record_execution(black_box(addr), 50);
                
                // 获取自适应阈值
                let _threshold = detector.get_adaptive_threshold(black_box(addr));
            }
        })
    });
}

// ============================================================================
// 集成性能基准
// ============================================================================

fn bench_integration_jit_gc_cache(c: &mut Criterion) {
    let gc_config = create_benchmark_gc_config();
    let cache_config = create_benchmark_cache_config();
    let hotspot_config = create_benchmark_hotspot_config();
    
    let gc = Arc::new(UnifiedGc::new(gc_config));
    let cache = Arc::new(UnifiedCodeCache::new(cache_config, hotspot_config));
    let mut jit = Jit::new();
    
    c.bench_function("integration_jit_gc_cache", |b| {
        b.iter(|| {
            for i in 0..100 {
                let addr = 0x1000 + i * 0x100;
                let block = create_benchmark_ir_block(addr, 20);
                
                // JIT编译
                let compiled = jit.compile_block(black_box(&block));
                
                // 缓存
                cache.insert_sync(black_box(addr), compiled.code.clone(), compiled.compile_time_ns);
                
                // GC分配
                gc.allocate(black_box(addr), block.ops.len() * 8);
                
                // 查找
                let _result = cache.lookup(black_box(addr));
                
                // 热点检测
                cache.record_execution(black_box(addr), 50, 1.0);
            }
        })
    });
}

fn bench_integration_memory_pressure(c: &mut Criterion) {
    let mut gc_config = create_benchmark_gc_config();
    gc_config.young_gen.initial_size = 1 * 1024 * 1024; // 1MB，小内存
    
    let gc_config = gc_config;
    let cache_config = create_benchmark_cache_config();
    let hotspot_config = create_benchmark_hotspot_config();
    
    let gc = Arc::new(UnifiedGc::new(gc_config));
    let cache = Arc::new(UnifiedCodeCache::new(cache_config, hotspot_config));
    let mut jit = Jit::new();
    
    c.bench_function("integration_memory_pressure", |b| {
        b.iter(|| {
            // 大量分配以触发内存压力
            for i in 0..200 {
                let addr = 0x1000 + i * 0x100;
                let block = create_benchmark_ir_block(addr, 30);
                
                let compiled = jit.compile_block(black_box(&block));
                cache.insert_sync(black_box(addr), compiled.code.clone(), compiled.compile_time_ns);
                gc.allocate(black_box(addr), block.ops.len() * 16); // 更大内存分配
                
                let _result = cache.lookup(black_box(addr));
                cache.record_execution(black_box(addr), 50, 1.0);
            }
            
            // 触发GC
            let roots: Vec<_> = (0..200).map(|i| 0x1000 + i * 0x100).collect();
            gc.collect_full(black_box(&roots));
        })
    });
}

// ============================================================================
// 吞吐量基准
// ============================================================================

fn bench_throughput_compilation(c: &mut Criterion) {
    let mut jit = Jit::new();
    let blocks: Vec<_> = (0..1000)
        .map(|i| create_benchmark_ir_block(0x1000 + i * 0x100, 25))
        .collect();
    
    let mut group = c.benchmark_group("throughput_compilation");
    group.throughput(Throughput::Elements(blocks.len() as u64));
    
    group.bench_function("throughput_compilation", |b| {
        b.iter(|| {
            for block in &blocks {
                let compiled = jit.compile_block(black_box(block));
                black_box(compiled);
            }
        })
    });
    
    group.finish();
}

fn bench_throughput_gc_allocation(c: &mut Criterion) {
    let config = create_benchmark_gc_config();
    let gc = UnifiedGc::new(config);
    
    let mut group = c.benchmark_group("throughput_gc_allocation");
    group.throughput(Throughput::Elements(10000));
    
    group.bench_function("throughput_gc_allocation", |b| {
        b.iter(|| {
            for i in 0..10000 {
                let addr = 0x1000 + i * 0x100;
                gc.allocate(black_box(addr), 64);
            }
        })
    });
    
    group.finish();
}

fn bench_throughput_cache_operations(c: &mut Criterion) {
    let config = create_benchmark_cache_config();
    let hotspot_config = create_benchmark_hotspot_config();
    let cache = UnifiedCodeCache::new(config, hotspot_config);
    
    let mut group = c.benchmark_group("throughput_cache_operations");
    group.throughput(Throughput::Elements(20000)); // 10000 inserts + 10000 lookups
    
    group.bench_function("throughput_cache_operations", |b| {
        b.iter(|| {
            // 插入
            for i in 0..10000 {
                let addr = 0x1000 + i * 0x100;
                let code = vec![0x90; 64];
                cache.insert_sync(black_box(addr), code, false);
            }
            
            // 查找
            for i in 0..10000 {
                let addr = 0x1000 + i * 0x100;
                let _result = cache.lookup(black_box(addr));
            }
        })
    });
    
    group.finish();
}

// ============================================================================
// 基准组定义
// ============================================================================

criterion_group!(
    jit_compilation_benches,
    bench_jit_compilation_small,
    bench_jit_compilation_medium,
    bench_jit_compilation_large,
    bench_jit_compilation_throughput
);

criterion_group!(
    register_allocation_benches,
    bench_register_allocation_linear_scan,
    bench_register_allocation_graph_coloring,
    bench_register_allocation_spill_heavy
);

criterion_group!(
    instruction_scheduling_benches,
    bench_instruction_scheduling_simple,
    bench_instruction_scheduling_complex
);

criterion_group!(
    optimization_benches,
    bench_optimization_passes_constant_folding,
    bench_optimization_passes_dead_code_elimination
);

criterion_group!(
    gc_benches,
    bench_gc_allocation_young_gen,
    bench_gc_allocation_old_gen,
    bench_gc_collection_young_gen,
    bench_gc_collection_full,
    bench_gc_concurrent_marking
);

criterion_group!(
    cache_benches,
    bench_cache_lookup_lru,
    bench_cache_lookup_lfu,
    bench_cache_insertion,
    bench_cache_eviction
);

criterion_group!(
    hotspot_benches,
    bench_hotspot_detection_basic,
    bench_hotspot_detection_multidimensional,
    bench_hotspot_detection_adaptive_threshold
);

criterion_group!(
    integration_benches,
    bench_integration_jit_gc_cache,
    bench_integration_memory_pressure
);

criterion_group!(
    throughput_benches,
    bench_throughput_compilation,
    bench_throughput_gc_allocation,
    bench_throughput_cache_operations
);

criterion_main!(
    jit_compilation_benches,
    register_allocation_benches,
    instruction_scheduling_benches,
    optimization_benches,
    gc_benches,
    cache_benches,
    hotspot_benches,
    integration_benches,
    throughput_benches
);