//! JIT引擎综合性能基准测试
//!
//! 本模块提供全面的JIT引擎性能基准测试，包括编译性能、执行性能、
//! 内存使用、缓存效率、优化效果等多维度性能评估。

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use vm_core::{GuestAddr, VmError, MMU, AccessType};
use vm_engine_jit::core::{JITEngine, JITConfig};
use vm_ir::{IRBlock, IRBuilder, IROp, Terminator, MemFlags};

/// 创建基础测试IR块
fn create_basic_ir_block(addr: GuestAddr, instruction_count: usize) -> IRBlock {
    let mut builder = IRBuilder::new(addr);
    
    for i in 0..instruction_count {
        builder.push(IROp::MovImm { dst: (i % 16) as u32, imm: (i * 42) as u64 });
        builder.push(IROp::Add {
            dst: 0,
            src1: 0,
            src2: (i % 16) as u32,
        });
    }
    
    builder.set_term(Terminator::Jmp { target: addr + (instruction_count * 16) as u64 });
    builder.build()
}

/// 创建复杂计算IR块
fn create_compute_ir_block(addr: GuestAddr, complexity: usize) -> IRBlock {
    let mut builder = IRBuilder::new(addr);
    
    for i in 0..complexity {
        match i % 8 {
            0 => {
                builder.push(IROp::MovImm { dst: 1, imm: i as u64 });
                builder.push(IROp::Add {
                    dst: 0,
                    src1: 0,
                    src2: 1,
                });
            }
            1 => {
                builder.push(IROp::Sub {
                    dst: 2,
                    src1: 0,
                    src2: 1,
                });
            }
            2 => {
                builder.push(IROp::Mul {
                    dst: 3,
                    src1: 2,
                    src2: 1,
                });
            }
            3 => {
                builder.push(IROp::Div {
                    dst: 4,
                    src1: 3,
                    src2: 1,
                    signed: false,
                });
            }
            4 => {
                builder.push(IROp::Load {
                    dst: 5,
                    base: 0,
                    offset: (i * 8) as i64,
                    size: 8,
                    flags: MemFlags::default(),
                });
            }
            5 => {
                builder.push(IROp::Store {
                    base: 0,
                    offset: (i * 8) as i64,
                    size: 8,
                    src: 5,
                    flags: MemFlags::default(),
                });
            }
            6 => {
                builder.push(IROp::ShiftLeft {
                    dst: 6,
                    src: 0,
                    amount: 2,
                });
            }
            _ => {
                builder.push(IROp::ShiftRight {
                    dst: 7,
                    src: 6,
                    amount: 1,
                    signed: false,
                });
            }
        }
    }
    
    builder.set_term(Terminator::Jmp { target: addr + (complexity * 32) as u64 });
    builder.build()
}

/// 创建SIMD友好IR块
fn create_simd_ir_block(addr: GuestAddr, vector_length: usize) -> IRBlock {
    let mut builder = IRBuilder::new(addr);
    
    // 创建适合SIMD向量化的操作序列
    for i in 0..vector_length {
        // 连续的加法操作 - 适合SLP向量化
        builder.push(IROp::MovImm { dst: (i % 8) as u32, imm: (i * 10) as u64 });
        builder.push(IROp::Add {
            dst: (i % 8) as u32,
            src1: (i % 8) as u32,
            src2: ((i + 1) % 8) as u32,
        });
        builder.push(IROp::Mul {
            dst: (i % 8) as u32,
            src1: (i % 8) as u32,
            src2: 2,
        });
    }
    
    builder.set_term(Terminator::Jmp { target: addr + (vector_length * 24) as u64 });
    builder.build()
}

/// 创建内存密集型IR块
fn create_memory_intensive_ir_block(addr: GuestAddr, memory_ops: usize) -> IRBlock {
    let mut builder = IRBuilder::new(addr);
    
    for i in 0..memory_ops {
        // 交替的加载和存储操作
        builder.push(IROp::Load {
            dst: (i % 16) as u32,
            base: 0,
            offset: (i * 8) as i64,
            size: 8,
            flags: MemFlags::default(),
        });
        
        builder.push(IROp::Add {
            dst: (i % 16) as u32,
            src1: (i % 16) as u32,
            src2: 1,
        });
        
        builder.push(IROp::Store {
            base: 0,
            offset: (i * 8) as i64,
            size: 8,
            src: (i % 16) as u32,
            flags: MemFlags::default(),
        });
    }
    
    builder.set_term(Terminator::Jmp { target: addr + (memory_ops * 24) as u64 });
    builder.build()
}

/// 创建控制流密集型IR块
fn create_control_flow_ir_block(addr: GuestAddr, branch_count: usize) -> IRBlock {
    let mut builder = IRBuilder::new(addr);
    
    for i in 0..branch_count {
        builder.push(IROp::MovImm { dst: 1, imm: i as u64 });
        
        // 条件分支
        if i % 2 == 0 {
            builder.push(IROp::Cmp {
                dst: 2,
                src1: 0,
                src2: 1,
            });
            builder.push(IROp::JmpIf {
                condition: 2,
                target: addr + ((i + 1) * 16) as u64,
            });
        } else {
            builder.push(IROp::Jmp {
                target: addr + ((i + 1) * 16) as u64,
            });
        }
    }
    
    builder.set_term(Terminator::Ret);
    builder.build()
}

/// 基准测试：基础编译性能
fn bench_basic_compilation(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit_basic_compilation");
    
    for instruction_count in [100, 500, 1000, 5000, 10000].iter() {
        group.throughput(Throughput::Elements(*instruction_count as u64));
        group.bench_with_input(
            BenchmarkId::new("compile_instructions", instruction_count),
            instruction_count,
            |b, &instruction_count| {
                let mut jit = JITEngine::new(JITConfig::default());
                let block = create_basic_ir_block(0x1000, instruction_count);
                
                b.iter(|| {
                    black_box(jit.compile(black_box(&block)).unwrap());
                });
            },
        );
    }
    
    group.finish();
}

/// 基准测试：复杂计算编译性能
fn bench_complex_compilation(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit_complex_compilation");
    
    for complexity in [100, 500, 1000, 2000].iter() {
        group.throughput(Throughput::Elements(*complexity as u64));
        group.bench_with_input(
            BenchmarkId::new("compile_complex", complexity),
            complexity,
            |b, &complexity| {
                let mut jit = JITEngine::new(JITConfig::default());
                let block = create_compute_ir_block(0x2000, complexity);
                
                b.iter(|| {
                    black_box(jit.compile(black_box(&block)).unwrap());
                });
            },
        );
    }
    
    group.finish();
}

/// 基准测试：SIMD优化性能
fn bench_simd_optimization(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit_simd_optimization");
    
    // 测试非SIMD版本
    group.bench_function("no_simd", |b| {
        let mut config = JITConfig::default();
        config.enable_simd = false;
        let mut jit = JITEngine::new(config);
        let block = create_simd_ir_block(0x3000, 1000);
        
        b.iter(|| {
            black_box(jit.compile(black_box(&block)).unwrap());
        });
    });
    
    // 测试SIMD版本
    group.bench_function("with_simd", |b| {
        let mut config = JITConfig::default();
        config.enable_simd = true;
        let mut jit = JITEngine::new(config);
        let block = create_simd_ir_block(0x3000, 1000);
        
        b.iter(|| {
            black_box(jit.compile(black_box(&block)).unwrap());
        });
    });
    
    group.finish();
}

/// 基准测试：内存密集型编译
fn bench_memory_intensive_compilation(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit_memory_intensive_compilation");
    
    for memory_ops in [100, 500, 1000, 2000].iter() {
        group.throughput(Throughput::Elements(*memory_ops as u64));
        group.bench_with_input(
            BenchmarkId::new("compile_memory_intensive", memory_ops),
            memory_ops,
            |b, &memory_ops| {
                let mut jit = JITEngine::new(JITConfig::default());
                let block = create_memory_intensive_ir_block(0x4000, memory_ops);
                
                b.iter(|| {
                    black_box(jit.compile(black_box(&block)).unwrap());
                });
            },
        );
    }
    
    group.finish();
}

/// 基准测试：控制流密集型编译
fn bench_control_flow_compilation(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit_control_flow_compilation");
    
    for branch_count in [50, 100, 200, 500].iter() {
        group.throughput(Throughput::Elements(*branch_count as u64));
        group.bench_with_input(
            BenchmarkId::new("compile_control_flow", branch_count),
            branch_count,
            |b, &branch_count| {
                let mut jit = JITEngine::new(JITConfig::default());
                let block = create_control_flow_ir_block(0x5000, branch_count);
                
                b.iter(|| {
                    black_box(jit.compile(black_box(&block)).unwrap());
                });
            },
        );
    }
    
    group.finish();
}

/// 基准测试：代码缓存性能
fn bench_code_cache_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit_code_cache_performance");
    
    // 测试缓存命中
    group.bench_function("cache_hit", |b| {
        let mut jit = JITEngine::new(JITConfig::default());
        let block = create_basic_ir_block(0x6000, 1000);
        
        // 预先编译以填充缓存
        let _ = jit.compile(&block).unwrap();
        
        b.iter(|| {
            black_box(jit.compile(black_box(&block)).unwrap());
        });
    });
    
    // 测试缓存未命中
    group.bench_function("cache_miss", |b| {
        let mut jit = JITEngine::new(JITConfig::default());
        
        b.iter(|| {
            let addr = 0x7000 + (rand::random::<u64>() % 10000);
            let block = create_basic_ir_block(addr, 1000);
            black_box(jit.compile(black_box(&block)).unwrap());
        });
    });
    
    group.finish();
}

/// 基准测试：并发编译性能
fn bench_concurrent_compilation(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit_concurrent_compilation");
    
    for thread_count in [1, 2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::new("concurrent_compile", thread_count),
            thread_count,
            |b, &thread_count| {
                b.iter(|| {
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    rt.block_on(async {
                        let jit = Arc::new(Mutex::new(JITEngine::new(JITConfig::default())));
                        let mut handles = Vec::new();
                        
                        for i in 0..thread_count {
                            let jit_clone = jit.clone();
                            let handle = tokio::spawn(async move {
                                let block = create_basic_ir_block(0x8000 + i as u64 * 0x1000, 1000);
                                let mut jit = jit_clone.lock().await;
                                jit.compile(&block).unwrap()
                            });
                            handles.push(handle);
                        }
                        
                        for handle in handles {
                            black_box(handle.await.unwrap());
                        }
                    });
                });
            },
        );
    }
    
    group.finish();
}

/// 基准测试：优化器性能
fn bench_optimizer_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit_optimizer_performance");
    
    // 测试无优化
    group.bench_function("no_optimization", |b| {
        let mut config = JITConfig::default();
        config.enable_optimization = false;
        let mut jit = JITEngine::new(config);
        let block = create_compute_ir_block(0x9000, 1000);
        
        b.iter(|| {
            black_box(jit.compile(black_box(&block)).unwrap());
        });
    });
    
    // 测试基础优化
    group.bench_function("basic_optimization", |b| {
        let mut config = JITConfig::default();
        config.enable_optimization = true;
        config.optimization_level = 1;
        let mut jit = JITEngine::new(config);
        let block = create_compute_ir_block(0x9000, 1000);
        
        b.iter(|| {
            black_box(jit.compile(black_box(&block)).unwrap());
        });
    });
    
    // 测试激进优化
    group.bench_function("aggressive_optimization", |b| {
        let mut config = JITConfig::default();
        config.enable_optimization = true;
        config.optimization_level = 3;
        let mut jit = JITEngine::new(config);
        let block = create_compute_ir_block(0x9000, 1000);
        
        b.iter(|| {
            black_box(jit.compile(black_box(&block)).unwrap());
        });
    });
    
    group.finish();
}

/// 基准测试：热点检测性能
fn bench_hotspot_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit_hotspot_detection");
    
    // 测试热点检测开销
    group.bench_function("hotspot_detection_overhead", |b| {
        let mut config = JITConfig::default();
        config.enable_hotspot_detection = true;
        let mut jit = JITEngine::new(config);
        let block = create_basic_ir_block(0xA000, 1000);
        
        b.iter(|| {
            // 模拟多次执行以触发热点检测
            for _ in 0..150 {
                black_box(jit.compile(black_box(&block)).unwrap());
            }
        });
    });
    
    // 测试无热点检测
    group.bench_function("no_hotspot_detection", |b| {
        let mut config = JITConfig::default();
        config.enable_hotspot_detection = false;
        let mut jit = JITEngine::new(config);
        let block = create_basic_ir_block(0xA000, 1000);
        
        b.iter(|| {
            // 相同的执行次数，但无热点检测
            for _ in 0..150 {
                black_box(jit.compile(black_box(&block)).unwrap());
            }
        });
    });
    
    group.finish();
}

/// 基准测试：内存使用效率
fn bench_memory_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit_memory_efficiency");
    
    for block_count in [10, 50, 100, 500].iter() {
        group.bench_with_input(
            BenchmarkId::new("memory_efficiency", block_count),
            block_count,
            |b, &block_count| {
                b.iter(|| {
                    let mut jit = JITEngine::new(JITConfig::default());
                    let mut blocks = Vec::new();
                    
                    // 创建并编译多个块
                    for i in 0..block_count {
                        let block = create_basic_ir_block(0xB000 + i as u64 * 0x1000, 1000);
                        blocks.push(block);
                    }
                    
                    for block in blocks.iter() {
                        black_box(jit.compile(black_box(block)).unwrap());
                    }
                });
            },
        );
    }
    
    group.finish();
}

/// 基准测试：自适应阈值调整
fn bench_adaptive_threshold(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit_adaptive_threshold");
    
    // 测试固定阈值
    group.bench_function("fixed_threshold", |b| {
        let mut config = JITConfig::default();
        config.enable_adaptive_threshold = false;
        config.compilation_threshold = 100;
        let mut jit = JITEngine::new(config);
        let block = create_basic_ir_block(0xC000, 1000);
        
        b.iter(|| {
            black_box(jit.compile(black_box(&block)).unwrap());
        });
    });
    
    // 测试自适应阈值
    group.bench_function("adaptive_threshold", |b| {
        let mut config = JITConfig::default();
        config.enable_adaptive_threshold = true;
        let mut jit = JITEngine::new(config);
        let block = create_basic_ir_block(0xC000, 1000);
        
        b.iter(|| {
            black_box(jit.compile(black_box(&block)).unwrap());
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_basic_compilation,
    bench_complex_compilation,
    bench_simd_optimization,
    bench_memory_intensive_compilation,
    bench_control_flow_compilation,
    bench_code_cache_performance,
    bench_concurrent_compilation,
    bench_optimizer_performance,
    bench_hotspot_detection,
    bench_memory_efficiency,
    bench_adaptive_threshold
);
criterion_main!(benches);