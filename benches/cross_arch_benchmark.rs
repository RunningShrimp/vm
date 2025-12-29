//! Cross-Architecture Translation Performance Benchmarks
//!
//! This module provides comprehensive performance benchmarks for cross-architecture
//! instruction translation, including:
//! - Translation performance between different architectures
//! - Instruction parallelism optimization effectiveness
//! - Register allocation optimization tests
//! - Memory alignment optimization tests
//! - Block-level cache efficiency tests

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use std::time::{Duration, Instant};
use vm_cross_arch::{ArchTranslator, SourceArch, TargetArch};
use vm_ir::{IRBlock, IRBuilder, IROp, Terminator, RegId};

/// 创建测试IR块
fn create_test_ir_block(addr: u64, instruction_count: usize) -> IRBlock {
    let mut builder = IRBuilder::new(addr);
    
    for i in 0..instruction_count {
        match i % 8 {
            0 => {
                // MOV指令
                builder.push(IROp::MovImm { 
                    dst: RegId(i as u32 % 16), 
                    imm: (i * 42) as u64 
                });
            }
            1 => {
                // ADD指令
                builder.push(IROp::Add {
                    dst: RegId(0),
                    src1: RegId(0),
                    src2: RegId(i as u32 % 16),
                });
            }
            2 => {
                // SUB指令
                builder.push(IROp::Sub {
                    dst: RegId(1),
                    src1: RegId(1),
                    src2: RegId(i as u32 % 16),
                });
            }
            3 => {
                // MUL指令
                builder.push(IROp::Mul {
                    dst: RegId(2),
                    src1: RegId(2),
                    src2: RegId(i as u32 % 16),
                });
            }
            4 => {
                // 内存加载
                builder.push(IROp::Load {
                    dst: RegId(3),
                    base: RegId(5),
                    offset: (i * 8) as i64,
                    size: 8,
                    flags: 0,
                });
            }
            5 => {
                // 内存存储
                builder.push(IROp::Store {
                    src: RegId(8),
                    base: RegId(7),
                    offset: (i * 8) as i64,
                    size: 8,
                    flags: 0,
                });
            }
            6 => {
                // 条件跳转
                builder.push(IROp::Cmp {
                    dst: RegId(9),
                    src1: RegId(10),
                    src2: RegId(i as u32 % 16),
                });
            }
            7 => {
                // 逻辑运算
                builder.push(IROp::And {
                    dst: RegId(11),
                    src1: RegId(11),
                    src2: RegId(i as u32 % 16),
                });
            }
            _ => {}
        }
    }
    
    builder.set_term(Terminator::Jmp { 
        target: addr + (instruction_count * 16) as u64 
    });
    builder.build()
}

/// 创建复杂计算IR块
fn create_compute_ir_block(addr: u64, complexity: usize) -> IRBlock {
    let mut builder = IRBuilder::new(addr);
    
    // 初始化
    builder.push(IROp::MovImm { dst: RegId(0), imm: 1 });
    builder.push(IROp::MovImm { dst: RegId(1), imm: 1 });
    
    // 斐波那契数列计算
    for i in 2..complexity {
        builder.push(IROp::Add {
            dst: RegId(i as u32),
            src1: RegId((i-1) as u32),
            src2: RegId((i-2) as u32),
        });
    }
    
    // 矩阵乘法
    let matrix_size = (complexity as f64).sqrt() as usize;
    for i in 0..matrix_size {
        for j in 0..matrix_size {
            for k in 0..matrix_size {
                builder.push(IROp::Mul {
                    dst: RegId(20), // 临时寄存器
                    src1: RegId((i * matrix_size + k) as u32),
                    src2: RegId((k * matrix_size + j) as u32),
                });
                builder.push(IROp::Add {
                    dst: RegId((i * matrix_size + j) as u32),
                    src1: RegId((i * matrix_size + j) as u32),
                    src2: RegId(20),
                });
            }
        }
    }
    
    builder.set_term(Terminator::Jmp { 
        target: addr + (complexity * 16) as u64 
    });
    builder.build()
}

/// x86-64到ARM64转换性能测试
fn bench_x86_to_arm64_translation(c: &mut Criterion) {
    let mut group = c.benchmark_group("x86_to_arm64_translation");
    
    let instruction_counts = [10, 50, 100, 500, 1000];
    
    for count in &instruction_counts {
        group.throughput(Throughput::Elements(*count as u64));
        group.bench_with_input(
            BenchmarkId::new("translation", count),
            count,
            |b, &count| {
                let translator = ArchTranslator::new(SourceArch::X86_64, TargetArch::ARM64);
                let ir_block = create_test_ir_block(0x1000, count);
                
                b.iter(|| {
                    let _result = translator.translate_block(&ir_block);
                });
            },
        );
    }
    
    group.finish();
}

/// ARM64到RISC-V64转换性能测试
fn bench_arm64_to_riscv_translation(c: &mut Criterion) {
    let mut group = c.benchmark_group("arm64_to_riscv_translation");

    let instruction_counts = [10, 50, 100, 500, 1000];

    for count in &instruction_counts {
        group.throughput(Throughput::Elements(*count as u64));
        group.bench_with_input(
            BenchmarkId::new("translation", count),
            count,
            |b, &count| {
                let translator = ArchTranslator::new(SourceArch::ARM64, TargetArch::RiscV64);
                let ir_block = create_test_ir_block(0x1000, count);

                b.iter(|| {
                    let _result = translator.translate_block(&ir_block);
                });
            },
        );
    }

    group.finish();
}

/// RISC-V64到x86-64转换性能测试
fn bench_riscv_to_x86_translation(c: &mut Criterion) {
    let mut group = c.benchmark_group("riscv_to_x86_translation");
    
    let instruction_counts = [10, 50, 100, 500, 1000];
    
    for count in &instruction_counts {
        group.throughput(Throughput::Elements(*count as u64));
        group.bench_with_input(
            BenchmarkId::new("translation", count),
            count,
            |b, &count| {
                let translator = ArchTranslator::new(SourceArch::RiscV64, TargetArch::X86_64);
                let ir_block = create_test_ir_block(0x1000, count);
                
                b.iter(|| {
                    let _result = translator.translate_block(&ir_block);
                });
            },
        );
    }
    
    group.finish();
}

/// 指令并行优化效果测试
fn bench_instruction_parallelism_optimization(c: &mut Criterion) {
    let mut group = c.benchmark_group("instruction_parallelism_optimization");

    let complexities = [10, 50, 100, 500];

    for complexity in &complexities {
        // 无优化
        group.bench_with_input(
            BenchmarkId::new("no_optimization", complexity),
            complexity,
            |b, &complexity| {
                let translator = ArchTranslator::new(SourceArch::X86_64, TargetArch::ARM64);
                let ir_block = create_compute_ir_block(0x1000, *complexity);

                b.iter(|| {
                    let _result = translator.translate_block(&ir_block);
                });
            },
        );

        // 有优化
        group.bench_with_input(
            BenchmarkId::new("with_optimization", complexity),
            complexity,
            |b, &complexity| {
                let translator = ArchTranslator::with_all_optimizations(
                    SourceArch::X86_64,
                    TargetArch::ARM64,
                    None,
                    OptimizationConfig::all_enabled(),
                );
                let ir_block = create_compute_ir_block(0x1000, *complexity);

                b.iter(|| {
                    let _result = translator.translate_block(&ir_block);
                });
            },
        );
    }

    group.finish();
}

/// 寄存器分配优化效果测试
fn bench_register_allocation_optimization(c: &mut Criterion) {
    let mut group = c.benchmark_group("register_allocation_optimization");

    let complexities = [10, 50, 100, 500];

    for complexity in &complexities {
        // 无优化
        group.bench_with_input(
            BenchmarkId::new("no_optimization", complexity),
            complexity,
            |b, &complexity| {
                let translator = ArchTranslator::with_cache_and_optimization(
                    SourceArch::X86_64,
                    TargetArch::ARM64,
                    None,
                    false,
                );
                let ir_block = create_compute_ir_block(0x1000, *complexity);

                b.iter(|| {
                    let _result = translator.translate_block(&ir_block);
                });
            },
        );

        // 有优化
        group.bench_with_input(
            BenchmarkId::new("with_optimization", complexity),
            complexity,
            |b, &complexity| {
                let translator = ArchTranslator::with_cache_and_optimization(
                    SourceArch::X86_64,
                    TargetArch::ARM64,
                    None,
                    true,
                );
                let ir_block = create_compute_ir_block(0x1000, *complexity);

                b.iter(|| {
                    let _result = translator.translate_block(&ir_block);
                });
            },
        );
    }

    group.finish();
}

/// 内存对齐优化效果测试
fn bench_memory_alignment_optimization(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_alignment_optimization");

    let complexities = [10, 50, 100, 500];

    for complexity in &complexities {
        // 无优化
        group.bench_with_input(
            BenchmarkId::new("no_optimization", complexity),
            complexity,
            |b, &complexity| {
                let translator = ArchTranslator::with_cache_and_optimization(
                    SourceArch::X86_64,
                    TargetArch::ARM64,
                    None,
                    false,
                );
                let ir_block = create_compute_ir_block(0x1000, *complexity);

                b.iter(|| {
                    let _result = translator.translate_block(&ir_block);
                });
            },
        );

        // 有优化
        group.bench_with_input(
            BenchmarkId::new("with_optimization", complexity),
            complexity,
            |b, &complexity| {
                let translator = ArchTranslator::with_cache_optimization_and_memory(
                    SourceArch::X86_64,
                    TargetArch::ARM64,
                    None,
                    false,
                    true,
                );
                let ir_block = create_compute_ir_block(0x1000, *complexity);

                b.iter(|| {
                    let _result = translator.translate_block(&ir_block);
                });
            },
        );
    }

    group.finish();
}

/// 块级缓存效率测试
fn bench_block_cache_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("block_cache_efficiency");

    let cache_sizes = [1024, 4096, 16384, 65536]; // 1K, 4K, 16K, 64K条目

    for cache_size in &cache_sizes {
        // 缓存命中
        group.bench_with_input(
            BenchmarkId::new("cache_hit", cache_size),
            cache_size,
            |b, &cache_size| {
                let translator = ArchTranslator::with_cache(
                    SourceArch::X86_64,
                    TargetArch::ARM64,
                    Some(*cache_size),
                );
                let ir_block = create_test_ir_block(0x1000, 100);

                // 预热缓存
                let _ = translator.translate_block(&ir_block);

                b.iter(|| {
                    let _result = translator.translate_block(&ir_block);
                });
            },
        );

        // 缓存未命中
        group.bench_with_input(
            BenchmarkId::new("cache_miss", cache_size),
            cache_size,
            |b, &cache_size| {
                let translator = ArchTranslator::with_cache(
                    SourceArch::X86_64,
                    TargetArch::ARM64,
                    Some(*cache_size),
                );

                b.iter(|| {
                    // 每次使用不同的IR块，确保缓存未命中
                    let ir_block = create_test_ir_block(0x1000 + b.iter.count() as u64, 100);
                    let _result = translator.translate_block(&ir_block);
                });
            },
        );
    }

    group.finish();
}

/// 多线程转换性能测试
fn bench_multithread_translation(c: &mut Criterion) {
    let mut group = c.benchmark_group("multithread_translation");
    
    let thread_counts = [1, 2, 4, 8];
    
    for thread_count in &thread_counts {
        group.bench_with_input(
            BenchmarkId::new("multithread", thread_count),
            thread_count,
            |b, &thread_count| {
                b.iter(|| {
                    let mut handles = Vec::new();
                    
                    for _ in 0..*thread_count {
                        let handle = std::thread::spawn(|| {
                            let translator = ArchTranslator::new(SourceArch::X86_64, TargetArch::ARM64);
                            let mut total_time = Duration::new(0, 0);
                            
                            for i in 0..10 {
                                let ir_block = create_test_ir_block(0x1000 + i as u64 * 0x1000, 100);
                                let start = Instant::now();
                                let _result = translator.translate_block(&ir_block);
                                total_time += start.elapsed();
                            }
                            
                            total_time
                        });
                        handles.push(handle);
                    }
                    
                    let mut total_time = Duration::new(0, 0);
                    for handle in handles {
                        total_time += handle.join().unwrap();
                    }
                    
                    black_box(total_time);
                });
            },
        );
    }
    
    group.finish();
}

/// 转换压力测试
fn bench_translation_stress(c: &mut Criterion) {
    let mut group = c.benchmark_group("translation_stress");
    
    let instruction_counts = [100, 500, 1000, 5000];
    
    for count in &instruction_counts {
        group.bench_with_input(
            BenchmarkId::new("stress_test", count),
            count,
            |b, &count| {
                let translator = ArchTranslator::new(SourceArch::X86_64, TargetArch::ARM64);
                
                b.iter(|| {
                    for i in 0..10 {
                        let ir_block = create_test_ir_block(0x1000 + i as u64 * 0x1000, *count);
                        let _result = translator.translate_block(&ir_block);
                    }
                });
            },
        );
    }
    
    group.finish();
}

criterion_group!(
    benches,
    bench_x86_to_arm64_translation,
    bench_arm64_to_riscv_translation,
    bench_riscv_to_x86_translation,
    bench_instruction_parallelism_optimization,
    bench_register_allocation_optimization,
    bench_memory_alignment_optimization,
    bench_block_cache_efficiency,
    bench_multithread_translation,
    bench_translation_stress
);

criterion_main!(benches);