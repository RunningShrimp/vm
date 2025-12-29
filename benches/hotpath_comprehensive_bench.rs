//! Comprehensive Hot Path Performance Benchmarks
//!
//! Benchmarks critical VM execution paths to measure and optimize performance.
//!
//! # Hot Paths Benchmarked
//!
//! 1. **Instruction Execution Loop** - Main dispatch overhead
//! 2. **Register File Access** - Read/write latency
//! 3. **Memory Operations** - Load/store patterns
//! 4. **TLB Lookup** - Cache hit/miss scenarios
//! 5. **JIT Compilation** - Compilation speed
//! 6. **Branch Prediction** - Conditional branch accuracy

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::Duration;

// Moduli for hotpath_optimizer
use vm_engine_interpreter::hotpath_optimizer::{
    optimized_arith, optimized_memory, optimized_regs, HotPathExecutor, HotPathStats,
};

// ============================================================================
// Register Access Benchmarks
// ============================================================================

fn bench_register_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("register_read");
    group.measurement_time(Duration::from_secs(10));

    let mut regs = [0u64; 32];
    for i in 0..32 {
        regs[i] = i as u64 * 100;
    }

    group.bench_function("sequential", |b| {
        b.iter(|| {
            for i in 0..32 {
                black_box(optimized_regs::get_reg_fast(&regs, i));
            }
        })
    });

    group.bench_function("random", |b| {
        let indices: Vec<u32> = (0..1000).map(|i| (i * 7) % 32).collect();
        b.iter(|| {
            for &idx in &indices {
                black_box(optimized_regs::get_reg_fast(&regs, idx));
            }
        })
    });

    group.bench_function("x0_access", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                black_box(optimized_regs::get_reg_fast(&regs, 0));
            }
        })
    });

    group.finish();
}

fn bench_register_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("register_write");
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("sequential", |b| {
        b.iter(|| {
            let mut regs = [0u64; 32];
            for i in 1..32 {
                // Skip x0
                optimized_regs::set_reg_fast(&mut regs, i, i as u64 * 100);
            }
            black_box(regs);
        })
    });

    group.bench_function("batch", |b| {
        let ops: Vec<(u32, u64)> = (1..32).map(|i| (i, i as u64 * 100)).collect();
        b.iter(|| {
            let mut regs = [0u64; 32];
            optimized_regs::batch_set(&mut regs, &ops);
            black_box(regs);
        })
    });

    group.finish();
}

// ============================================================================
// Arithmetic Operation Benchmarks
// ============================================================================

fn bench_arithmetic_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("arithmetic");
    group.measurement_time(Duration::from_secs(10));

    // Addition
    group.bench_function("add", |b| {
        b.iter(|| {
            let a: u64 = black_box(1000);
            let b: u64 = black_box(2000);
            black_box(a.wrapping_add(b));
        })
    });

    // Multiplication
    group.bench_function("mul", |b| {
        b.iter(|| {
            let a: u64 = black_box(1000);
            let b: u64 = black_box(2000);
            black_box(a.wrapping_mul(b));
        })
    });

    // Optimized multiply (fast path)
    group.bench_function("mul_fast", |b| {
        b.iter(|| {
            let a: u64 = black_box(1000);
            let b: u64 = black_box(2);
            black_box(optimized_arith::mul_fast(a, b));
        })
    });

    // Power-of-2 multiply
    group.bench_function("mul_power_of_2", |b| {
        b.iter(|| {
            let a: u64 = black_box(1000);
            let b: u64 = black_box(8);
            black_box(optimized_arith::mul_power_of_two(a, b));
        })
    });

    // Division
    group.bench_function("div", |b| {
        b.iter(|| {
            let a: u64 = black_box(10000);
            let b: u64 = black_box(100);
            black_box(a.wrapping_div(b));
        })
    });

    // Power-of-2 division
    group.bench_function("div_power_of_2", |b| {
        b.iter(|| {
            let a: u64 = black_box(10000);
            let b: u64 = black_box(8);
            black_box(optimized_arith::div_power_of_two(a, b));
        })
    });

    group.finish();
}

// ============================================================================
// Memory Operation Benchmarks
// ============================================================================

fn bench_memory_operations(c: &mut Criterion) {
    use vm_mem::SoftMmu;
    use vm_core::AccessType;

    let mut group = c.benchmark_group("memory_ops");
    group.measurement_time(Duration::from_secs(10));

    let mut mmu = SoftMmu::new(64 * 1024 * 1024, true);

    // Initialize test data
    for i in 0..1000 {
        let _ = mmu.write(i * 8, 0xDEADBEEF_u64, 8);
    }

    // Sequential reads
    group.bench_function("seq_read_1k", |b| {
        b.iter(|| {
            for i in 0..1000 {
                let addr = black_box(i * 8);
                let _ = mmu.read(addr, 8);
            }
        })
    });

    // Random reads
    group.bench_function("random_read_1k", |b| {
        let addrs: Vec<u64> = (0..1000).map(|i| (i * 7 % 1000) * 8).collect();
        b.iter(|| {
            for &addr in &addrs {
                let _ = mmu.read(black_box(addr), 8);
            }
        })
    });

    // Sequential writes
    group.bench_function("seq_write_1k", |b| {
        b.iter(|| {
            for i in 0..1000 {
                let addr = black_box(i * 8);
                let _ = mmu.write(addr, 0xDEADBEEF, 8);
            }
        })
    });

    group.finish();
}

// ============================================================================
// TLB Lookup Benchmarks
// ============================================================================

fn bench_tlb_lookup(c: &mut Criterion) {
    use vm_mem::tlb::concurrent_tlb::ConcurrentTlbManager;
    use vm_mem::tlb::concurrent_tlb::ConcurrentTlbConfig;
    use vm_core::AccessType;

    let mut group = c.benchmark_group("tlb_lookup");
    group.measurement_time(Duration::from_secs(10));

    let config = ConcurrentTlbConfig::default();
    let tlb = ConcurrentTlbManager::new(config);

    // Pre-populate TLB
    for i in 0..1000 {
        tlb.insert(i, i + 0x1000, 0x5, i % 16);
    }

    group.bench_function("sequential_hit", |b| {
        b.iter(|| {
            for i in 0..1000 {
                black_box(tlb.translate(i, i % 16, AccessType::Read));
            }
        })
    });

    group.bench_function("random_hit", |b| {
        let indices: Vec<u64> = (0..1000).map(|i| (i * 7) % 1000).collect();
        b.iter(|| {
            for &idx in &indices {
                black_box(tlb.translate(idx, (idx % 16) as u16, AccessType::Read));
            }
        })
    });

    group.bench_function("miss", |b| {
        b.iter(|| {
            for i in 0..1000 {
                black_box(tlb.translate(i + 0x10000, i % 16, AccessType::Read));
            }
        })
    });

    group.finish();
}

// ============================================================================
// JIT Compilation Benchmarks
// ============================================================================

fn bench_jit_compilation(c: &mut Criterion) {
    use vm_engine_jit::core::{IRBlock, JITConfig, JITEngine};
    use vm_ir::IROp;

    let mut group = c.benchmark_group("jit_compilation");
    group.measurement_time(Duration::from_secs(10));

    for size in [10, 50, 100, 500, 1000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let mut ops = Vec::with_capacity(size);

            for i in 0..size {
                ops.push(IROp::MovImm {
                    dst: (i % 32) as u32,
                    imm: (i * 42) as u64,
                });
                ops.push(IROp::Add {
                    dst: (i % 32) as u32,
                    src1: (i % 16) as u32,
                    src2: ((i + 1) % 16) as u32,
                });
            }

            let ir_block = IRBlock {
                start_pc: vm_core::GuestAddr(0x1000),
                ops,
                term: vm_ir::Terminator::Ret,
            };

            let config = JITConfig::default();
            let mut engine = JITEngine::new(config);

            b.iter(|| {
                black_box(engine.compile(black_box(&ir_block)));
            });
        });
    }

    group.finish();
}

// ============================================================================
// Branch Prediction Benchmarks
// ============================================================================

fn bench_branch_prediction(c: &mut Criterion) {
    let mut group = c.benchmark_group("branch_prediction");
    group.measurement_time(Duration::from_secs(10));

    // Always-taken branch
    group.bench_function("always_taken", |b| {
        b.iter(|| {
            let mut sum = 0;
            for i in 0..1000 {
                if black_box(true) {
                    sum += i;
                }
            }
            black_box(sum);
        })
    });

    // Never-taken branch
    group.bench_function("never_taken", |b| {
        b.iter(|| {
            let mut sum = 0;
            for i in 0..1000 {
                if black_box(false) {
                    sum += i;
                }
            }
            black_box(sum);
        })
    });

    // Predictable pattern
    group.bench_function("predictable_pattern", |b| {
        b.iter(|| {
            let mut sum = 0;
            for i in 0..1000 {
                if i % 2 == 0 {
                    sum += i;
                }
            }
            black_box(sum);
        })
    });

    // Unpredictable random
    group.bench_function("unpredictable", |b| {
        let values: Vec<bool> = (0..1000).map(|i| (i * 7) % 2 == 0).collect();
        b.iter(|| {
            let mut sum = 0;
            for (i, &val) in values.iter().enumerate() {
                if black_box(val) {
                    sum += i;
                }
            }
            black_box(sum);
        })
    });

    group.finish();
}

// ============================================================================
// Combined Hot Path Benchmark
// ============================================================================

fn bench_combined_hot_path(c: &mut Criterion) {
    let mut group = c.benchmark_group("combined_hot_path");
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("mixed_operations", |b| {
        b.iter(|| {
            let mut regs = [0u64; 32];
            regs[1] = 100;
            regs[2] = 200;

            // Arithmetic
            for i in 0..100 {
                let a = optimized_regs::get_reg_fast(&regs, 1);
                let b = optimized_regs::get_reg_fast(&regs, 2);
                let result = optimized_arith::mul_fast(a, b);
                optimized_regs::set_reg_fast(&mut regs, (i % 31 + 1) as u32, result);
            }

            // More arithmetic
            for i in 0..100 {
                let a = optimized_regs::get_reg_fast(&regs, 1);
                let result = a.wrapping_mul(2); // Power of 2
                optimized_regs::set_reg_fast(&mut regs, (i % 31 + 1) as u32, result);
            }

            black_box(regs);
        })
    });

    group.bench_function("register_heavy", |b| {
        b.iter(|| {
            let mut regs = [0u64; 32];

            // Heavy register access pattern
            for i in 0..1000 {
                let src1 = (i % 32) as u32;
                let src2 = ((i + 1) % 32) as u32;
                let dst = ((i + 2) % 32) as u32;

                let a = optimized_regs::get_reg_fast(&regs, src1);
                let b = optimized_regs::get_reg_fast(&regs, src2);
                let result = a.wrapping_add(b);
                optimized_regs::set_reg_fast(&mut regs, dst, result);
            }

            black_box(regs);
        })
    });

    group.finish();
}

// ============================================================================
// Hot Path Executor Benchmarks
// ============================================================================

fn bench_hot_path_executor(c: &mut Criterion) {
    let mut group = c.benchmark_group("hot_path_executor");
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("arith_operations", |b| {
        b.iter(|| {
            let mut executor = HotPathExecutor::new();
            let mut regs = [0u64; 32];
            regs[1] = 10;
            regs[2] = 4;

            let op = vm_ir::IROp::MulImm {
                dst: 3,
                src: 1,
                imm: 4,
            };

            for _ in 0..1000 {
                black_box(executor.execute_arith(&mut regs, black_box(&op)));
            }

            black_box(executor.get_stats());
        })
    });

    group.finish();
}

// ============================================================================
// Instruction Dispatch Benchmarks
// ============================================================================

fn bench_instruction_dispatch(c: &mut Criterion) {
    let mut group = c.benchmark_group("instruction_dispatch");
    group.measurement_time(Duration::from_secs(10));

    let ops = vec![
        vm_ir::IROp::Add {
            dst: 3,
            src1: 1,
            src2: 2,
        },
        vm_ir::IROp::Sub {
            dst: 4,
            src1: 1,
            src2: 2,
        },
        vm_ir::IROp::Mul {
            dst: 5,
            src1: 1,
            src2: 2,
        },
        vm_ir::IROp::MovImm {
            dst: 6,
            imm: 42,
        },
    ];

    group.bench_function("dispatch_small_set", |b| {
        b.iter(|| {
            let mut regs = [0u64; 32];
            regs[1] = 10;
            regs[2] = 20;

            for i in 0..1000 {
                let op = &ops[i % ops.len()];
                match op {
                    vm_ir::IROp::Add { dst, src1, src2 } => {
                        let v = regs[*src1 as usize].wrapping_add(regs[*src2 as usize]);
                        regs[*dst as usize] = v;
                    }
                    vm_ir::IROp::Sub { dst, src1, src2 } => {
                        let v = regs[*src1 as usize].wrapping_sub(regs[*src2 as usize]);
                        regs[*dst as usize] = v;
                    }
                    vm_ir::IROp::Mul { dst, src1, src2 } => {
                        let v = regs[*src1 as usize].wrapping_mul(regs[*src2 as usize]);
                        regs[*dst as usize] = v;
                    }
                    vm_ir::IROp::MovImm { dst, imm } => {
                        regs[*dst as usize] = *imm;
                    }
                    _ => {}
                }
            }

            black_box(regs);
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_register_read,
    bench_register_write,
    bench_arithmetic_operations,
    bench_memory_operations,
    bench_tlb_lookup,
    bench_jit_compilation,
    bench_branch_prediction,
    bench_combined_hot_path,
    bench_hot_path_executor,
    bench_instruction_dispatch
);

criterion_main!(benches);
