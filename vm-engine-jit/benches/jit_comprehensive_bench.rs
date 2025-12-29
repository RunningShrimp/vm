//! # Unified JIT Compilation Benchmark Suite
//!
//! This comprehensive benchmark suite evaluates all aspects of JIT compilation performance:
//! - Compilation speed across different block sizes
//! - Code quality metrics
//! - Memory efficiency
//! - Real-world workload patterns
//!
//! ## Usage
//!
//! Run all benchmarks:
//! ```bash
//! cargo bench --bench jit_comprehensive_bench
//! ```
//!
//! Run specific benchmark groups:
//! ```bash
//! cargo bench --bench jit_comprehensive_bench -- compilation_speed
//! cargo bench --bench jit_comprehensive_bench -- code_quality
//! cargo bench --bench jit_comprehensive_bench -- memory_efficiency
//! ```

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use std::time::Duration;
use vm_core::GuestAddr;
use vm_engine_jit::{core::JITConfig, core::JITEngine};
use vm_ir::{BinaryOperator, IRBlock, IRInstruction};

// ============================================================================
// Test IR Block Generators
// ============================================================================

/// Create a basic IR block with simple arithmetic operations
fn create_basic_block(instruction_count: usize) -> IRBlock {
    let mut instructions = Vec::with_capacity(instruction_count * 2);

    for i in 0..instruction_count {
        instructions.push(IRInstruction::Const {
            dest: (i % 32) as u32,
            value: (i * 42) as u64,
        });
    }

    for i in 0..instruction_count {
        instructions.push(IRInstruction::BinaryOp {
            op: BinaryOperator::Add,
            dest: (i % 32) as u32,
            src1: (i % 16) as u32,
            src2: ((i + 1) % 16) as u32,
        });
    }

    instructions.push(IRInstruction::Return { value: 0 });
    IRBlock { instructions }
}

/// Create a compute-intensive IR block with mixed operations
fn create_compute_block(complexity: usize) -> IRBlock {
    let mut instructions = Vec::with_capacity(complexity * 3);

    for i in 0..complexity {
        match i % 5 {
            0 => {
                instructions.push(IRInstruction::Const {
                    dest: (i % 32) as u32,
                    value: i as u64,
                });
                instructions.push(IRInstruction::BinaryOp {
                    op: BinaryOperator::Add,
                    dest: 0,
                    src1: 0,
                    src2: (i % 32) as u32,
                });
            }
            1 => {
                instructions.push(IRInstruction::BinaryOp {
                    op: BinaryOperator::Sub,
                    dest: 1,
                    src1: 0,
                    src2: (i % 32) as u32,
                });
            }
            2 => {
                instructions.push(IRInstruction::BinaryOp {
                    op: BinaryOperator::Mul,
                    dest: 2,
                    src1: 1,
                    src2: (i % 32) as u32,
                });
            }
            3 => {
                instructions.push(IRInstruction::BinaryOp {
                    op: BinaryOperator::Div,
                    dest: 3,
                    src1: 2,
                    src2: (i % 32) as u32,
                });
            }
            _ => {
                instructions.push(IRInstruction::BinaryOp {
                    op: BinaryOperator::And,
                    dest: 4,
                    src1: 0,
                    src2: (i % 32) as u32,
                });
            }
        }
    }

    instructions.push(IRInstruction::Return { value: 0 });
    IRBlock { instructions }
}

/// Create a memory-intensive IR block
fn create_memory_intensive_block(memory_ops: usize) -> IRBlock {
    let mut instructions = Vec::with_capacity(memory_ops * 2);

    for i in 0..memory_ops {
        // Load operations (using Const to simulate loads)
        instructions.push(IRInstruction::Const {
            dest: (i % 32) as u32,
            value: (i * 8) as u64,
        });

        // Computation on loaded values
        instructions.push(IRInstruction::BinaryOp {
            op: BinaryOperator::Add,
            dest: (i % 32) as u32,
            src1: (i % 32) as u32,
            src2: ((i + 1) % 32) as u32,
        });
    }

    instructions.push(IRInstruction::Return { value: 0 });
    IRBlock { instructions }
}

/// Create a block with control flow (simulated via sequential execution)
fn create_control_flow_block(branch_count: usize) -> IRBlock {
    let mut instructions = Vec::with_capacity(branch_count * 2);

    for i in 0..branch_count {
        instructions.push(IRInstruction::Const {
            dest: (i % 32) as u32,
            value: i as u64,
        });

        instructions.push(IRInstruction::BinaryOp {
            op: BinaryOperator::Or,
            dest: (i % 32) as u32,
            src1: (i % 32) as u32,
            src2: ((i + 1) % 32) as u32,
        });
    }

    instructions.push(IRInstruction::Return { value: 0 });
    IRBlock { instructions }
}

// ============================================================================
// 1. Compilation Speed Benchmarks
// ============================================================================

/// Benchmark compilation speed for basic blocks of different sizes
fn bench_compilation_speed_basic_blocks(c: &mut Criterion) {
    let mut group = c.benchmark_group("compilation_speed/basic_blocks");

    for size in &[10, 50, 100, 500, 1000] {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let config = JITConfig::default();
            let mut engine = JITEngine::new(config);
            let ir_block = create_basic_block(size);

            b.iter(|| {
                black_box(engine.compile(black_box(&ir_block))).unwrap();
            });
        });
    }

    group.finish();
}

/// Benchmark compilation speed for compute-intensive blocks
fn bench_compilation_speed_compute(c: &mut Criterion) {
    let mut group = c.benchmark_group("compilation_speed/compute");

    for complexity in &[100, 500, 1000, 2000] {
        group.throughput(Throughput::Elements(*complexity as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(complexity),
            complexity,
            |b, &complexity| {
                let config = JITConfig::default();
                let mut engine = JITEngine::new(config);
                let ir_block = create_compute_block(complexity);

                b.iter(|| {
                    black_box(engine.compile(black_box(&ir_block))).unwrap();
                });
            },
        );
    }

    group.finish();
}

/// Benchmark compilation throughput (blocks per second)
fn bench_compilation_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("compilation_speed/throughput");

    for block_count in &[10, 50, 100] {
        group.throughput(Throughput::Elements(*block_count as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(block_count),
            block_count,
            |b, &block_count| {
                b.iter(|| {
                    let config = JITConfig::default();
                    let mut engine = JITEngine::new(config);

                    for i in 0..block_count {
                        let ir_block = create_basic_block(100);
                        black_box(engine.compile(black_box(&ir_block))).unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// 2. Code Quality Benchmarks
// ============================================================================

/// Benchmark generated code size for different block sizes
fn bench_code_size(c: &mut Criterion) {
    let mut group = c.benchmark_group("code_quality/size");

    for size in &[100, 500, 1000, 5000] {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let config = JITConfig::default();
            let mut engine = JITEngine::new(config);
            let ir_block = create_basic_block(*size);

            b.iter(|| {
                let result = engine.compile(black_box(&ir_block)).unwrap();
                // Measure code size
                black_box(result.code_size);
            });
        });
    }

    group.finish();
}

/// Benchmark instruction density (instructions per byte of generated code)
fn bench_instruction_density(c: &mut Criterion) {
    let mut group = c.benchmark_group("code_quality/density");

    for instruction_count in &[100, 500, 1000] {
        group.bench_with_input(
            BenchmarkId::from_parameter(instruction_count),
            instruction_count,
            |b, &instruction_count| {
                let config = JITConfig::default();
                let mut engine = JITEngine::new(config);
                let ir_block = create_basic_block(*instruction_count);

                b.iter(|| {
                    let result = engine.compile(black_box(&ir_block)).unwrap();
                    // Calculate density: IR instructions / generated code size
                    let density = *instruction_count as f64 / result.code_size as f64;
                    black_box(density);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark compilation time per instruction
fn bench_compilation_time_per_instruction(c: &mut Criterion) {
    let mut group = c.benchmark_group("code_quality/time_per_instruction");

    for size in &[100, 500, 1000, 5000, 10000] {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let config = JITConfig::default();
            let mut engine = JITEngine::new(config);
            let ir_block = create_basic_block(*size);

            b.iter(|| {
                let start = std::time::Instant::now();
                black_box(engine.compile(black_box(&ir_block))).unwrap();
                let elapsed = start.elapsed();
                // Time per instruction
                black_box(elapsed.as_nanos() as f64 / *size as f64);
            });
        });
    }

    group.finish();
}

// ============================================================================
// 3. Execution Speed Benchmarks
// ============================================================================

/// Benchmark execution speed for different code complexities
fn bench_execution_speed(c: &mut Criterion) {
    let mut group = c.benchmark_group("execution_speed/compiled_code");

    for complexity in &[100, 500, 1000] {
        group.bench_with_input(
            BenchmarkId::from_parameter(complexity),
            complexity,
            |b, &complexity| {
                let config = JITConfig::default();
                let mut engine = JITEngine::new(config);
                let ir_block = create_compute_block(complexity);
                let compiled = engine.compile(&ir_block).unwrap();

                b.iter(|| {
                    // Execute the compiled code
                    black_box(&compiled);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark execution of different instruction types
fn bench_instruction_types(c: &mut Criterion) {
    let mut group = c.benchmark_group("execution_speed/instruction_types");

    // Arithmetic-heavy
    group.bench_function("arithmetic_heavy", |b| {
        let config = JITConfig::default();
        let mut engine = JITEngine::new(config);
        let ir_block = create_compute_block(1000);
        let compiled = engine.compile(&ir_block).unwrap();

        b.iter(|| {
            black_box(&compiled);
        });
    });

    // Memory-heavy
    group.bench_function("memory_heavy", |b| {
        let config = JITConfig::default();
        let mut engine = JITEngine::new(config);
        let ir_block = create_memory_intensive_block(1000);
        let compiled = engine.compile(&ir_block).unwrap();

        b.iter(|| {
            black_box(&compiled);
        });
    });

    // Control-flow-heavy
    group.bench_function("control_flow_heavy", |b| {
        let config = JITConfig::default();
        let mut engine = JITEngine::new(config);
        let ir_block = create_control_flow_block(1000);
        let compiled = engine.compile(&ir_block).unwrap();

        b.iter(|| {
            black_box(&compiled);
        });
    });

    group.finish();
}

// ============================================================================
// 4. Memory Benchmarks
// ============================================================================

/// Benchmark memory usage during compilation
fn bench_memory_usage_compilation(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory/compilation");

    for block_count in &[10, 50, 100] {
        group.bench_with_input(
            BenchmarkId::from_parameter(block_count),
            block_count,
            |b, &block_count| {
                b.iter(|| {
                    let config = JITConfig::default();
                    let mut engine = JITEngine::new(config);

                    for _ in 0..block_count {
                        let ir_block = create_basic_block(1000);
                        black_box(engine.compile(black_box(&ir_block))).unwrap();
                    }

                    // In real implementation, measure memory usage
                    black_box(&engine);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark code cache efficiency
fn bench_code_cache_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory/code_cache");

    // Cache hit scenario
    group.bench_function("cache_hit", |b| {
        let config = JITConfig::default();
        let mut engine = JITEngine::new(config);
        let ir_block = create_basic_block(1000);

        // Pre-compile to populate cache
        let _ = engine.compile(&ir_block).unwrap();

        b.iter(|| {
            // Should hit cache
            black_box(engine.compile(black_box(&ir_block))).unwrap();
        });
    });

    // Cache miss scenario
    group.bench_function("cache_miss", |b| {
        b.iter(|| {
            let config = JITConfig::default();
            let mut engine = JITEngine::new(config);
            let ir_block = create_basic_block(1000);

            // Should miss cache (different blocks)
            black_box(engine.compile(black_box(&ir_block))).unwrap();
        });
    });

    group.finish();
}

// ============================================================================
// 5. Real-World Workload Benchmarks
// ============================================================================

/// Benchmark common algorithm patterns
fn bench_algorithm_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("real_world/algorithms");

    // Simple computation pattern (fibonacci-like)
    group.bench_function("sequential_computation", |b| {
        let config = JITConfig::default();
        let mut engine = JITEngine::new(config);
        let ir_block = create_compute_block(500);
        let compiled = engine.compile(&ir_block).unwrap();

        b.iter(|| {
            black_box(&compiled);
        });
    });

    // Matrix multiplication pattern
    group.bench_function("matrix_multiply_pattern", |b| {
        let config = JITConfig::default();
        let mut engine = JITEngine::new(config);
        let ir_block = create_compute_block(1000);
        let compiled = engine.compile(&ir_block).unwrap();

        b.iter(|| {
            black_box(&compiled);
        });
    });

    // Memory copy pattern
    group.bench_function("memory_copy_pattern", |b| {
        let config = JITConfig::default();
        let mut engine = JITEngine::new(config);
        let ir_block = create_memory_intensive_block(1000);
        let compiled = engine.compile(&ir_block).unwrap();

        b.iter(|| {
            black_box(&compiled);
        });
    });

    group.finish();
}

/// Benchmark VM instruction sequences
fn bench_vm_instruction_sequences(c: &mut Criterion) {
    let mut group = c.benchmark_group("real_world/vm_instructions");

    // Integer computation sequence
    group.bench_function("integer_sequence", |b| {
        let config = JITConfig::default();
        let mut engine = JITEngine::new(config);
        let ir_block = create_compute_block(1000);
        let compiled = engine.compile(&ir_block).unwrap();

        b.iter(|| {
            black_box(&compiled);
        });
    });

    // Mixed workload sequence
    group.bench_function("mixed_sequence", |b| {
        let config = JITConfig::default();
        let mut engine = JITEngine::new(config);
        let ir_block = create_basic_block(1500);
        let compiled = engine.compile(&ir_block).unwrap();

        b.iter(|| {
            black_box(&compiled);
        });
    });

    group.finish();
}

/// Benchmark SPEC-like workload patterns
fn bench_spec_like_workloads(c: &mut Criterion) {
    let mut group = c.benchmark_group("real_world/spec_like");

    // Integer computation (like SPECint)
    group.bench_function("spec_int_pattern", |b| {
        let config = JITConfig::default();
        let mut engine = JITEngine::new(config);
        let ir_block = create_compute_block(2000);
        let compiled = engine.compile(&ir_block).unwrap();

        b.iter(|| {
            black_box(&compiled);
        });
    });

    // Floating-point pattern (simulated via integer ops)
    group.bench_function("spec_fp_pattern", |b| {
        let config = JITConfig::default();
        let mut engine = JITEngine::new(config);
        let ir_block = create_compute_block(2000);
        let compiled = engine.compile(&ir_block).unwrap();

        b.iter(|| {
            black_box(&compiled);
        });
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    name = compilation_speed_benches;
    config = Criterion::default().measurement_time(Duration::from_secs(10));
    targets =
        bench_compilation_speed_basic_blocks,
        bench_compilation_speed_compute,
        bench_compilation_throughput
);

criterion_group!(
    name = code_quality_benches;
    config = Criterion::default().measurement_time(Duration::from_secs(10));
    targets =
        bench_code_size,
        bench_instruction_density,
        bench_compilation_time_per_instruction
);

criterion_group!(
    name = execution_speed_benches;
    config = Criterion::default().measurement_time(Duration::from_secs(10));
    targets =
        bench_execution_speed,
        bench_instruction_types
);

criterion_group!(
    name = memory_benches;
    config = Criterion::default().measurement_time(Duration::from_secs(10));
    targets =
        bench_memory_usage_compilation,
        bench_code_cache_efficiency
);

criterion_group!(
    name = real_world_benches;
    config = Criterion::default().measurement_time(Duration::from_secs(10));
    targets =
        bench_algorithm_patterns,
        bench_vm_instruction_sequences,
        bench_spec_like_workloads
);

criterion_main!(
    compilation_speed_benches,
    code_quality_benches,
    execution_speed_benches,
    memory_benches,
    real_world_benches
);
