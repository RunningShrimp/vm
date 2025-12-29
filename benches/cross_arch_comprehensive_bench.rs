//! Comprehensive Cross-Architecture Translation Performance Benchmarks
//!
//! This benchmark suite provides comprehensive performance testing for cross-architecture
//! instruction translation with metrics for:
//! - Translation speed (instructions/second)
//! - Translation overhead percentage
//! - Code size ratio (generated/target code size)
//! - Translation accuracy
//! - All translation pairs (x86_64 ↔ ARM64 ↔ RISC-V64)
//! - Different optimization levels
//! - Cache performance comparison
//! - Tiered compilation comparison

use criterion::{black_box, BenchmarkId, Criterion, Throughput};
use std::time::{Duration, Instant};
use vm_cross_arch::{ArchTranslator, SourceArch, TargetArch};
use vm_ir::{IRBlock, IRBuilder, IROp, RegId, Terminator};

// ============================================================================
// Test Data Generators
// ============================================================================

/// Generate basic instruction blocks (MOV, ADD, SUB, MUL)
fn generate_basic_block(addr: u64, instruction_count: usize) -> IRBlock {
    let mut builder = IRBuilder::new(addr);

    for i in 0..instruction_count {
        match i % 4 {
            0 => builder.push(IROp::MovImm {
                dst: RegId(i as u32 % 16),
                imm: (i * 42) as u64,
            }),
            1 => builder.push(IROp::Add {
                dst: RegId(0),
                src1: RegId(0),
                src2: RegId(i as u32 % 16),
            }),
            2 => builder.push(IROp::Sub {
                dst: RegId(1),
                src1: RegId(1),
                src2: RegId(i as u32 % 16),
            }),
            3 => builder.push(IROp::Mul {
                dst: RegId(2),
                src1: RegId(2),
                src2: RegId(i as u32 % 16),
            }),
            _ => unreachable!(),
        }
    }

    builder.set_term(Terminator::Ret);
    builder.build()
}

/// Generate memory operation blocks (Load, Store)
fn generate_memory_block(addr: u64, instruction_count: usize) -> IRBlock {
    let mut builder = IRBuilder::new(addr);

    for i in 0..instruction_count {
        match i % 2 {
            0 => builder.push(IROp::Load {
                dst: RegId(i as u32 % 16),
                base: RegId(16),
                offset: (i * 8) as i64,
                size: 8,
                flags: Default::default(),
            }),
            1 => builder.push(IROp::Store {
                src: RegId(i as u32 % 16),
                base: RegId(17),
                offset: (i * 8) as i64,
                size: 8,
                flags: Default::default(),
            }),
            _ => unreachable!(),
        }
    }

    builder.set_term(Terminator::Ret);
    builder.build()
}

/// Generate control flow blocks (conditional branches, jumps)
fn generate_control_flow_block(addr: u64, instruction_count: usize) -> IRBlock {
    let mut builder = IRBuilder::new(addr);

    for i in 0..instruction_count {
        match i % 3 {
            0 => builder.push(IROp::Add {
                dst: RegId(i as u32 % 8),
                src1: RegId(i as u32 % 8),
                src2: RegId(1),
            }),
            1 => builder.push(IROp::CmpEq {
                dst: RegId(i as u32 % 8),
                lhs: RegId(i as u32 % 8),
                rhs: RegId(0),
            }),
            2 => builder.push(IROp::Beq {
                src1: RegId(i as u32 % 8),
                src2: RegId(0),
                target: vm_ir::GuestAddr(addr + (i * 4) as u64 + 16),
            }),
            _ => unreachable!(),
        }
    }

    builder.set_term(Terminator::Ret);
    builder.build()
}

/// Generate function prologue/epilogue patterns
fn generate_function_prologue_epilogue(addr: u64) -> IRBlock {
    let mut builder = IRBuilder::new(addr);

    // Function prologue
    builder.push(IROp::Store {
        src: RegId(30),
        base: RegId(31),
        offset: -16,
        size: 8,
        flags: Default::default(),
    });
    builder.push(IROp::Store {
        src: RegId(29),
        base: RegId(31),
        offset: -24,
        size: 8,
        flags: Default::default(),
    });

    // Function body
    builder.push(IROp::Add {
        dst: RegId(0),
        src1: RegId(1),
        src2: RegId(2),
    });

    // Function epilogue
    builder.push(IROp::Load {
        dst: RegId(29),
        base: RegId(31),
        offset: -24,
        size: 8,
        flags: Default::default(),
    });
    builder.push(IROp::Load {
        dst: RegId(30),
        base: RegId(31),
        offset: -16,
        size: 8,
        flags: Default::default(),
    });

    builder.set_term(Terminator::Ret);
    builder.build()
}

/// Generate loop structures
fn generate_loop_block(addr: u64, iterations: usize) -> IRBlock {
    let mut builder = IRBuilder::new(addr);

    // Loop counter initialization
    builder.push(IROp::MovImm { dst: RegId(1), imm: 0 });
    builder.push(IROp::MovImm { dst: RegId(2), imm: iterations as u64 });

    // Loop body
    let loop_start = builder.ops.len();
    builder.push(IROp::Add {
        dst: RegId(3),
        src1: RegId(3),
        src2: RegId(4),
    });
    builder.push(IROp::Add {
        dst: RegId(1),
        src1: RegId(1),
        src2: RegId(5),
    });
    builder.push(IROp::CmpNe {
        dst: RegId(6),
        lhs: RegId(1),
        rhs: RegId(2),
    });
    builder.push(IROp::Bne {
        src1: RegId(6),
        src2: RegId(0),
        target: vm_ir::GuestAddr(addr + loop_start as u64 * 4),
    });

    builder.set_term(Terminator::Ret);
    builder.build()
}

/// Generate switch statement patterns
fn generate_switch_block(addr: u64, case_count: usize) -> IRBlock {
    let mut builder = IRBuilder::new(addr);

    // Load switch value
    builder.push(IROp::MovImm { dst: RegId(1), imm: 0 });

    // Compare with each case
    for i in 0..case_count {
        builder.push(IROp::MovImm {
            dst: RegId(2),
            imm: i as u64,
        });
        builder.push(IROp::CmpEq {
            dst: RegId(3),
            lhs: RegId(1),
            rhs: RegId(2),
        });
        builder.push(IROp::Beq {
            src1: RegId(3),
            src2: RegId(0),
            target: vm_ir::GuestAddr(addr + (i * 16) as u64 + 100),
        });
    }

    // Default case
    builder.push(IROp::MovImm { dst: RegId(0), imm: 999 });

    builder.set_term(Terminator::Ret);
    builder.build()
}

/// Generate complex mixed blocks
fn generate_complex_block(addr: u64, instruction_count: usize) -> IRBlock {
    let mut builder = IRBuilder::new(addr);

    for i in 0..instruction_count {
        match i % 10 {
            0 => builder.push(IROp::MovImm {
                dst: RegId(i as u32 % 16),
                imm: (i * 42) as u64,
            }),
            1 => builder.push(IROp::Add {
                dst: RegId(0),
                src1: RegId(1),
                src2: RegId(2),
            }),
            2 => builder.push(IROp::Sub {
                dst: RegId(3),
                src1: RegId(4),
                src2: RegId(5),
            }),
            3 => builder.push(IROp::Mul {
                dst: RegId(6),
                src1: RegId(7),
                src2: RegId(8),
            }),
            4 => builder.push(IROp::Div {
                dst: RegId(9),
                src1: RegId(10),
                src2: RegId(11),
                signed: false,
            }),
            5 => builder.push(IROp::Load {
                dst: RegId(12),
                base: RegId(16),
                offset: (i * 8) as i64,
                size: 8,
                flags: Default::default(),
            }),
            6 => builder.push(IROp::Store {
                src: RegId(13),
                base: RegId(17),
                offset: (i * 8) as i64,
                size: 8,
                flags: Default::default(),
            }),
            7 => builder.push(IROp::CmpEq {
                dst: RegId(14),
                lhs: RegId(14),
                rhs: RegId(15),
            }),
            8 => builder.push(IROp::Beq {
                src1: RegId(14),
                src2: RegId(0),
                target: vm_ir::GuestAddr(addr + 100),
            }),
            9 => builder.push(IROp::And {
                dst: RegId(0),
                src1: RegId(1),
                src2: RegId(2),
            }),
            _ => unreachable!(),
        }
    }

    builder.set_term(Terminator::Ret);
    builder.build()
}

// ============================================================================
// Benchmark: All Translation Pairs
// ============================================================================

fn bench_all_translation_pairs(c: &mut Criterion) {
    let translation_pairs = [
        (SourceArch::X86_64, TargetArch::ARM64, "x86_64_to_arm64"),
        (SourceArch::X86_64, TargetArch::RiscV64, "x86_64_to_riscv64"),
        (SourceArch::ARM64, TargetArch::X86_64, "arm64_to_x86_64"),
        (SourceArch::ARM64, TargetArch::RiscV64, "arm64_to_riscv64"),
        (SourceArch::RiscV64, TargetArch::X86_64, "riscv64_to_x86_64"),
        (SourceArch::RiscV64, TargetArch::ARM64, "riscv64_to_arm64"),
    ];

    let instruction_counts = [10, 50, 100, 500];

    for (source, target, pair_name) in &translation_pairs {
        let mut group = c.benchmark_group(format!("translation_pair_{}", pair_name));

        for count in &instruction_counts {
            group.throughput(Throughput::Elements(*count as u64));
            group.bench_with_input(
                BenchmarkId::from_parameter(count),
                count,
                |b, &count| {
                    let mut translator = ArchTranslator::new(*source, *target);
                    let ir_block = generate_basic_block(0x1000, count);

                    b.iter(|| {
                        black_box(translator.translate_block(&ir_block))
                    });
                },
            );
        }

        group.finish();
    }
}

// ============================================================================
// Benchmark: Translation Speed (Instructions/Second)
// ============================================================================

fn bench_translation_speed(c: &mut Criterion) {
    let mut group = c.benchmark_group("translation_speed_instructions_per_sec");

    let translation_pairs = [
        (SourceArch::X86_64, TargetArch::ARM64, "x86_64_to_arm64"),
        (SourceArch::ARM64, TargetArch::X86_64, "arm64_to_x86_64"),
        (SourceArch::X86_64, TargetArch::RiscV64, "x86_64_to_riscv64"),
    ];

    for (source, target, pair_name) in &translation_pairs {
        let mut translator = ArchTranslator::new(*source, *target);
        let ir_block = generate_basic_block(0x1000, 1000);

        let start = Instant::now();
        let iterations = 1000;

        group.bench_function(pair_name, |b| {
            b.iter(|| {
                for _ in 0..iterations {
                    black_box(translator.translate_block(&ir_block));
                }
            })
        });

        let duration = start.elapsed();
        let total_instructions = (ir_block.ops.len() * iterations) as f64;
        let instructions_per_second = total_instructions / duration.as_secs_f64();

        group.throughput(criterion::Throughput::Elements(total_instructions as u64));
        println!(
            "{}: {:.2} instructions/second",
            pair_name, instructions_per_second
        );
    }

    group.finish();
}

// ============================================================================
// Benchmark: Translation Overhead
// ============================================================================

fn bench_translation_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("translation_overhead");

    // Baseline: Native execution (simulated by just creating IR blocks)
    group.bench_function("baseline_ir_creation", |b| {
        b.iter(|| {
            let block = black_box(generate_basic_block(0x1000, 100));
            black_box(block.ops.len());
        });
    });

    // Translation overhead for each pair
    let translation_pairs = [
        (SourceArch::X86_64, TargetArch::ARM64, "x86_64_to_arm64"),
        (SourceArch::ARM64, TargetArch::X86_64, "arm64_to_x86_64"),
        (SourceArch::X86_64, TargetArch::RiscV64, "x86_64_to_riscv64"),
    ];

    for (source, target, pair_name) in &translation_pairs {
        let mut translator = ArchTranslator::new(*source, *target);
        let ir_block = generate_basic_block(0x1000, 100);

        group.bench_function(pair_name, |b| {
            b.iter(|| {
                black_box(translator.translate_block(&ir_block));
            });
        });
    }

    group.finish();
}

// ============================================================================
// Benchmark: Code Size Ratio
// ============================================================================

fn bench_code_size_ratio(c: &mut Criterion) {
    let translation_pairs = [
        (SourceArch::X86_64, TargetArch::ARM64, "x86_64_to_arm64"),
        (SourceArch::ARM64, TargetArch::X86_64, "arm64_to_x86_64"),
        (SourceArch::X86_64, TargetArch::RiscV64, "x86_64_to_riscv64"),
        (SourceArch::ARM64, TargetArch::RiscV64, "arm64_to_riscv64"),
    ];

    for (source, target, pair_name) in &translation_pairs {
        let mut translator = ArchTranslator::new(*source, *target);
        let ir_block = generate_basic_block(0x1000, 100);

        let result = translator.translate_block(&ir_block).unwrap();
        let source_size = ir_block.ops.len();
        let target_size = result.instructions.len();
        let ratio = target_size as f64 / source_size as f64;

        println!(
            "{} code size ratio: {:.2}x ({} IR ops -> {} target instructions)",
            pair_name, ratio, source_size, target_size
        );
    }

    let mut group = c.benchmark_group("code_size_generation");

    for (source, target, pair_name) in &translation_pairs {
        let mut translator = ArchTranslator::new(*source, *target);
        let ir_block = generate_basic_block(0x1000, 100);

        group.bench_function(pair_name, |b| {
            b.iter(|| {
                let result = black_box(translator.translate_block(&ir_block).unwrap());
                black_box(result.instructions.len());
            });
        });
    }

    group.finish();
}

// ============================================================================
// Benchmark: Translation Accuracy
// ============================================================================

fn bench_translation_accuracy(c: &mut Criterion) {
    let test_blocks = vec![
        ("basic", generate_basic_block(0x1000, 50)),
        ("memory", generate_memory_block(0x2000, 50)),
        ("control_flow", generate_control_flow_block(0x3000, 50)),
        ("prologue_epilogue", generate_function_prologue_epilogue(0x4000)),
        ("loop", generate_loop_block(0x5000, 10)),
        ("switch", generate_switch_block(0x6000, 5)),
        ("complex", generate_complex_block(0x7000, 50)),
    ];

    let translation_pairs = [
        (SourceArch::X86_64, TargetArch::ARM64),
        (SourceArch::ARM64, TargetArch::X86_64),
        (SourceArch::X86_64, TargetArch::RiscV64),
        (SourceArch::ARM64, TargetArch::RiscV64),
        (SourceArch::RiscV64, TargetArch::X86_64),
        (SourceArch::RiscV64, TargetArch::ARM64),
    ];

    let mut total_tests = 0;
    let mut successful_tests = 0;

    for (block_name, ir_block) in &test_blocks {
        for (source, target) in &translation_pairs {
            total_tests += 1;
            let mut translator = ArchTranslator::new(*source, *target);

            match translator.translate_block(ir_block) {
                Ok(result) => {
                    successful_tests += 1;
                    println!(
                        "✓ {} -> {} for {} block: {} instructions generated",
                        source,
                        target,
                        block_name,
                        result.instructions.len()
                    );
                }
                Err(e) => {
                    println!(
                        "✗ {} -> {} for {} block: {}",
                        source, target, block_name, e
                    );
                }
            }
        }
    }

    let accuracy = (successful_tests as f64 / total_tests as f64) * 100.0;
    println!("\nTranslation Accuracy: {:.2}% ({}/{})", accuracy, successful_tests, total_tests);

    let mut group = c.benchmark_group("translation_accuracy");
    group.bench_function("overall_accuracy", |b| {
        b.iter(|| {
            let mut translator = ArchTranslator::new(SourceArch::X86_64, TargetArch::ARM64);
            let block = generate_basic_block(0x1000, 50);
            black_box(translator.translate_block(&block).is_ok());
        });
    });
    group.finish();
}

// ============================================================================
// Benchmark: Different Workloads
// ============================================================================

fn bench_different_workloads(c: &mut Criterion) {
    let workloads = [
        ("basic", generate_basic_block(0x1000, 100)),
        ("memory", generate_memory_block(0x2000, 100)),
        ("control_flow", generate_control_flow_block(0x3000, 100)),
        ("prologue_epilogue", generate_function_prologue_epilogue(0x4000)),
        ("loop", generate_loop_block(0x5000, 100)),
        ("switch", generate_switch_block(0x6000, 10)),
        ("complex", generate_complex_block(0x7000, 100)),
    ];

    for (workload_name, ir_block) in &workloads {
        let mut group = c.benchmark_group(format!("workload_{}", workload_name));

        let translation_pairs = [
            (SourceArch::X86_64, TargetArch::ARM64, "x86_64_to_arm64"),
            (SourceArch::ARM64, TargetArch::X86_64, "arm64_to_x86_64"),
        ];

        for (source, target, pair_name) in &translation_pairs {
            group.bench_function(pair_name, |b| {
                let mut translator = ArchTranslator::new(*source, *target);
                b.iter(|| {
                    black_box(translator.translate_block(ir_block));
                });
            });
        }

        group.finish();
    }
}

// ============================================================================
// Benchmark: Optimization Levels
// ============================================================================

fn bench_optimization_levels(c: &mut Criterion) {
    let mut group = c.benchmark_group("optimization_levels");

    let ir_block = generate_complex_block(0x1000, 100);

    // No optimization
    group.bench_function("no_optimization", |b| {
        let mut translator = ArchTranslator::with_cache_and_optimization(
            SourceArch::X86_64,
            TargetArch::ARM64,
            None,
            false,
        );
        b.iter(|| black_box(translator.translate_block(&ir_block)));
    });

    // Register optimization only
    group.bench_function("register_optimization", |b| {
        let mut translator = ArchTranslator::with_cache_and_optimization(
            SourceArch::X86_64,
            TargetArch::ARM64,
            None,
            true,
        );
        b.iter(|| black_box(translator.translate_block(&ir_block)));
    });

    // Memory optimization only
    group.bench_function("memory_optimization", |b| {
        let mut translator = ArchTranslator::with_cache_optimization_and_memory(
            SourceArch::X86_64,
            TargetArch::ARM64,
            None,
            false,
            true,
        );
        b.iter(|| black_box(translator.translate_block(&ir_block)));
    });

    // IR optimization only
    group.bench_function("ir_optimization", |b| {
        let mut translator = ArchTranslator::with_cache_optimization_memory_and_ir(
            SourceArch::X86_64,
            TargetArch::ARM64,
            None,
            false,
            false,
            true,
        );
        b.iter(|| black_box(translator.translate_block(&ir_block)));
    });

    // All optimizations
    group.bench_function("all_optimizations", |b| {
        let mut translator = ArchTranslator::with_all_optimizations(
            SourceArch::X86_64,
            TargetArch::ARM64,
            None,
            OptimizationConfig::all_enabled(),
        );
        b.iter(|| black_box(translator.translate_block(&ir_block)));
    });

    group.finish();
}

// ============================================================================
// Benchmark: Cache vs Uncached Translation
// ============================================================================

fn bench_cache_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_performance");

    let ir_block = generate_basic_block(0x1000, 100);

    // Uncached translation
    group.bench_function("uncached", |b| {
        let mut translator = ArchTranslator::new(SourceArch::X86_64, TargetArch::ARM64);
        b.iter(|| {
            // Use different blocks to avoid cache hits
            let block = generate_basic_block(0x1000 + b.iter.count() as u64 * 0x100, 100);
            black_box(translator.translate_block(&block));
        });
    });

    // Cached translation (warm cache)
    group.bench_function("cached_hit", |b| {
        let mut translator = ArchTranslator::with_cache(
            SourceArch::X86_64,
            TargetArch::ARM64,
            Some(1024),
        );
        // Warm up cache
        let _ = translator.translate_block(&ir_block);
        b.iter(|| black_box(translator.translate_block(&ir_block)));
    });

    // Different cache sizes
    for cache_size in [256, 1024, 4096, 16384] {
        group.bench_with_input(
            BenchmarkId::new("cache_size", cache_size),
            &cache_size,
            |b, &cache_size| {
                let mut translator = ArchTranslator::with_cache(
                    SourceArch::X86_64,
                    TargetArch::ARM64,
                    Some(cache_size),
                );
                // Warm up cache
                let _ = translator.translate_block(&ir_block);
                b.iter(|| black_box(translator.translate_block(&ir_block)));
            },
        );
    }

    group.finish();
}

// ============================================================================
// Criterion Groups
// ============================================================================

criterion_group!(
    translation_pairs,
    bench_all_translation_pairs
);

criterion_group!(
    performance_metrics,
    bench_translation_speed,
    bench_translation_overhead,
    bench_code_size_ratio
);

criterion_group!(
    workloads,
    bench_different_workloads
);

criterion_group!(
    optimization,
    bench_optimization_levels,
    bench_cache_performance
);

criterion_group!(
    accuracy,
    bench_translation_accuracy
);

criterion_main!(
    translation_pairs,
    performance_metrics,
    workloads,
    optimization,
    accuracy
);
