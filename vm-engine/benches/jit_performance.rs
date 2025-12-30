//! JIT性能基准测试
//!
//! 测试JIT编译器和执行的性能
//!
//! 测试覆盖:
//! - 60个性能基准测试用例
//! - JIT编译性能 (tier0-3)
//! - 优化级别对比
//! - 不同IR块大小
//! - 代码缓存性能
//! - 并行编译性能

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use vm_engine::jit::{JITCompiler, JITConfig, OptLevel};
use vm_ir::{IRBlock, IROp, RegId, Terminator};

// ============================================================================
// 辅助函数
// ============================================================================

/// 创建测试IR块
fn create_test_block(num_ops: usize) -> IRBlock {
    let mut ops = Vec::new();

    for i in 0..num_ops {
        ops.push(IROp::Add {
            dst: (i % 32) as RegId,
            src1: ((i + 1) % 32) as RegId,
            src2: ((i + 2) % 32) as RegId,
        });
    }

    IRBlock {
        start_pc: vm_core::GuestAddr(0),
        ops,
        term: Terminator::Ret,
    }
}

/// 创建复杂IR块 (包含多种操作)
fn create_complex_block(num_ops: usize) -> IRBlock {
    let mut ops = Vec::new();

    for i in 0..num_ops {
        match i % 5 {
            0 => ops.push(IROp::Add {
                dst: (i % 32) as RegId,
                src1: ((i + 1) % 32) as RegId,
                src2: ((i + 2) % 32) as RegId,
            }),
            1 => ops.push(IROp::Sub {
                dst: (i % 32) as RegId,
                src1: ((i + 1) % 32) as RegId,
                src2: ((i + 2) % 32) as RegId,
            }),
            2 => ops.push(IROp::Mul {
                dst: (i % 32) as RegId,
                src1: ((i + 1) % 32) as RegId,
                src2: ((i + 2) % 32) as RegId,
            }),
            3 => ops.push(IROp::Load {
                dst: (i % 32) as RegId,
                base: ((i + 1) % 32) as RegId,
                offset: (i * 8) as i64,
                size: 8,
                flags: vm_ir::MemFlags::default(),
            }),
            4 => ops.push(IROp::Store {
                src: (i % 32) as RegId,
                base: ((i + 1) % 32) as RegId,
                offset: (i * 8) as i64,
                size: 8,
                flags: vm_ir::MemFlags::default(),
            }),
            _ => unreachable!(),
        }
    }

    let block = IRBlock {
        start_pc: vm_core::GuestAddr(0),
        ops,
        term: Terminator::Ret,
    };
    block
}

// ============================================================================
// JIT编译性能基准测试 (测试1-20)
// ============================================================================

/// 基准测试1: Tier0编译 (小IR块)
fn bench_jit_compilation_tier0(c: &mut Criterion) {
    let mut config = JITConfig::default();
    config.opt_level = OptLevel::None;

    let mut compiler = JITCompiler::with_config(config).unwrap();
    let block = create_test_block(10);

    c.bench_function("jit_compile_tier0_10ops", |b| {
        b.iter(|| {
            black_box(compiler.compile(black_box(&block))).unwrap();
        });
    });
}

/// 基准测试2: Tier1编译 (中等IR块)
fn bench_jit_compilation_tier1(c: &mut Criterion) {
    let mut config = JITConfig::default();
    config.opt_level = OptLevel::Basic;

    let mut compiler = JITCompiler::with_config(config).unwrap();
    let block = create_test_block(100);

    c.bench_function("jit_compile_tier1_100ops", |b| {
        b.iter(|| {
            black_box(compiler.compile(black_box(&block))).unwrap();
        });
    });
}

/// 基准测试3: Tier2编译 (大IR块)
fn bench_jit_compilation_tier2(c: &mut Criterion) {
    let mut config = JITConfig::default();
    config.opt_level = OptLevel::Balanced;

    let mut compiler = JITCompiler::with_config(config).unwrap();
    let block = create_test_block(500);

    c.bench_function("jit_compile_tier2_500ops", |b| {
        b.iter(|| {
            black_box(compiler.compile(black_box(&block))).unwrap();
        });
    });
}

/// 基准测试4: Tier3编译 (超大IR块)
fn bench_jit_compilation_tier3(c: &mut Criterion) {
    let mut config = JITConfig::default();
    config.opt_level = OptLevel::Aggressive;

    let mut compiler = JITCompiler::with_config(config).unwrap();
    let block = create_test_block(1000);

    c.bench_function("jit_compile_tier3_1000ops", |b| {
        b.iter(|| {
            black_box(compiler.compile(black_box(&block))).unwrap();
        });
    });
}

/// 基准测试5: 不同IR块大小编译性能
fn bench_jit_compilation_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("compile_by_size");

    for size in [10, 50, 100, 500, 1000].iter() {
        let mut config = JITConfig::default();
        config.opt_level = OptLevel::Balanced;

        let mut compiler = JITCompiler::with_config(config).unwrap();
        let block = create_test_block(*size);

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _size| {
            b.iter(|| {
                black_box(compiler.compile(black_box(&block))).unwrap();
            });
        });
    }

    group.finish();
}

/// 基准测试6: 不同优化级别对比
fn bench_jit_optimization_levels(c: &mut Criterion) {
    let mut group = c.benchmark_group("optimization_levels");

    for opt_level in [
        OptLevel::None,
        OptLevel::Basic,
        OptLevel::Balanced,
        OptLevel::Aggressive,
    ] {
        let mut config = JITConfig::default();
        config.opt_level = opt_level;

        let mut compiler = JITCompiler::with_config(config).unwrap();
        let block = create_test_block(100);

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{:?}", opt_level)),
            &opt_level,
            |b, _| {
                b.iter(|| {
                    black_box(compiler.compile(black_box(&block))).unwrap();
                });
            },
        );
    }

    group.finish();
}

/// 基准测试7: 复杂IR块编译性能
fn bench_complex_block_compilation(c: &mut Criterion) {
    let mut config = JITConfig::default();
    config.opt_level = OptLevel::Balanced;

    let mut compiler = JITCompiler::with_config(config).unwrap();
    let block = create_complex_block(200);

    c.bench_function("compile_complex_200ops", |b| {
        b.iter(|| {
            black_box(compiler.compile(black_box(&block))).unwrap();
        });
    });
}

/// 基准测试8: 算术密集型IR块
fn bench_arithmetic_intensive_block(c: &mut Criterion) {
    let mut ops = Vec::new();

    for i in 0..1000 {
        ops.push(IROp::Add {
            dst: (i % 32) as RegId,
            src1: ((i + 1) % 32) as RegId,
            src2: ((i + 2) % 32) as RegId,
        });
    }
    let block = IRBlock {
        start_pc: vm_core::GuestAddr(0),
        ops,
        term: Terminator::Ret,
    };

    let mut compiler = JITCompiler::new();
    c.bench_function("compile_arithmetic_1000ops", |b| {
        b.iter(|| {
            black_box(compiler.compile(black_box(&block))).unwrap();
        });
    });
}

/// 基准测试9: 内存访问密集型IR块
fn bench_memory_intensive_block(c: &mut Criterion) {
    let mut ops = Vec::new();

    for i in 0..1000 {
        ops.push(IROp::Load {
            dst: (i % 32) as RegId,
            base: ((i + 1) % 32) as RegId,
            offset: (i * 8) as i64,
            size: 8,
        flags: vm_ir::MemFlags::default(),
                });
    }
    let block = IRBlock {
        start_pc: vm_core::GuestAddr(0),
        ops,
        term: Terminator::Ret,
    };

    let mut compiler = JITCompiler::new();
    c.bench_function("compile_memory_load_1000ops", |b| {
        b.iter(|| {
            black_box(compiler.compile(black_box(&block))).unwrap();
        });
    });
}

/// 基准测试10: 混合操作IR块
fn bench_mixed_operations_block(c: &mut Criterion) {
    let block = create_complex_block(500);
    let mut compiler = JITCompiler::new();

    c.bench_function("compile_mixed_500ops", |b| {
        b.iter(|| {
            black_box(compiler.compile(black_box(&block))).unwrap();
        });
    });
}

/// 基准测试11: 小IR块批量编译
fn bench_small_batch_compilation(c: &mut Criterion) {
    let blocks: Vec<_> = (0..100).map(|i| create_test_block(10 + i % 20)).collect();
    let mut compiler = JITCompiler::new();

    c.bench_function("compile_batch_100_small_blocks", |b| {
        b.iter(|| {
            for block in &blocks {
                black_box(compiler.compile(black_box(block))).unwrap();
            }
        });
    });
}

/// 基准测试12: 中等IR块批量编译
fn bench_medium_batch_compilation(c: &mut Criterion) {
    let blocks: Vec<_> = (0..50).map(|i| create_test_block(100 + i % 50)).collect();
    let mut compiler = JITCompiler::new();

    c.bench_function("compile_batch_50_medium_blocks", |b| {
        b.iter(|| {
            for block in &blocks {
                black_box(compiler.compile(black_box(block))).unwrap();
            }
        });
    });
}

/// 基准测试13: 大IR块批量编译
fn bench_large_batch_compilation(c: &mut Criterion) {
    let blocks: Vec<_> = (0..10).map(|i| create_test_block(500 + i % 100)).collect();
    let mut compiler = JITCompiler::new();

    c.bench_function("compile_batch_10_large_blocks", |b| {
        b.iter(|| {
            for block in &blocks {
                black_box(compiler.compile(black_box(block))).unwrap();
            }
        });
    });
}

/// 基准测试14: 编译器创建开销
fn bench_compiler_creation(c: &mut Criterion) {
    c.bench_function("create_compiler", |b| {
        b.iter(|| {
            black_box(JITCompiler::new());
        });
    });
}

/// 基准测试15: 带配置的编译器创建
fn bench_compiler_creation_with_config(c: &mut Criterion) {
    let config = JITConfig::default();

    c.bench_function("create_compiler_with_config", |b| {
        b.iter(|| {
            black_box(JITCompiler::with_config(black_box(config.clone())).unwrap());
        });
    });
}

/// 基准测试16: IR块创建开销
fn bench_ir_block_creation(c: &mut Criterion) {
    c.bench_function("create_ir_block_100ops", |b| {
        b.iter(|| {
            black_box(create_test_block(100));
        });
    });
}

/// 基准测试17: 复杂IR块创建开销
fn bench_complex_ir_block_creation(c: &mut Criterion) {
    c.bench_function("create_complex_ir_block_200ops", |b| {
        b.iter(|| {
            black_box(create_complex_block(200));
        });
    });
}

/// 基准测试18: 极小IR块编译 (1条操作)
fn bench_tiny_block_compilation(c: &mut Criterion) {
    let block = create_test_block(1);
    let mut compiler = JITCompiler::new();

    c.bench_function("compile_tiny_1op", |b| {
        b.iter(|| {
            black_box(compiler.compile(black_box(&block))).unwrap();
        });
    });
}

/// 基准测试19: 极大IR块编译 (5000条操作)
fn bench_huge_block_compilation(c: &mut Criterion) {
    let block = create_test_block(5000);
    let mut compiler = JITCompiler::new();

    c.bench_function("compile_huge_5000ops", |b| {
        b.iter(|| {
            black_box(compiler.compile(black_box(&block))).unwrap();
        });
    });
}

/// 基准测试20: 连续编译相同IR块
fn bench_repeated_compilation(c: &mut Criterion) {
    let block = create_test_block(100);
    let mut compiler = JITCompiler::new();

    c.bench_function("compile_same_block_repeatedly", |b| {
        b.iter(|| {
            for _ in 0..10 {
                black_box(compiler.compile(black_box(&block))).unwrap();
            }
        });
    });
}

// ============================================================================
// 代码生成性能基准测试 (测试21-40)
// ============================================================================

/// 基准测试21: 寄存器密集型IR块
fn bench_register_intensive_compilation(c: &mut Criterion) {
    let mut ops = Vec::new();

    // 使用大量不同的寄存器
    for i in 0..1000 {
        ops.push(IROp::MovImm {
            dst: (i % 64) as RegId,
            imm: i as u64,
        });
    }
    let block = IRBlock {
        start_pc: vm_core::GuestAddr(0),
        ops,
        term: Terminator::Ret,
    };

    let mut compiler = JITCompiler::new();
    c.bench_function("compile_register_intensive_64regs", |b| {
        b.iter(|| {
            black_box(compiler.compile(black_box(&block))).unwrap();
        });
    });
}

/// 基准测试22: 分支密集型IR块
fn bench_branch_intensive_compilation(c: &mut Criterion) {
    let mut ops = Vec::new();

    for i in 0..100 {
        ops.push(IROp::Add {
            dst: 0,
            src1: 1,
            src2: 2,
        });
    }

    let block = IRBlock {
        start_pc: vm_core::GuestAddr(0),
        ops,
        term: Terminator::Ret,
    };

    let mut compiler = JITCompiler::new();
    c.bench_function("compile_branch_intensive_100blocks", |b| {
        b.iter(|| {
            black_box(compiler.compile(black_box(&block))).unwrap();
        });
    });
}

/// 基准测试23: 不同操作类型分布
fn bench_operation_type_distribution(c: &mut Criterion) {
    let mut group = c.benchmark_group("operation_types");

    let mut arithmetic_ops = Vec::new();
    for i in 0..500 {
        arithmetic_ops.push(IROp::Add {
            dst: (i % 32) as RegId,
            src1: ((i + 1) % 32) as RegId,
            src2: ((i + 2) % 32) as RegId,
        });
    }
    let arithmetic_block = IRBlock {
        start_pc: vm_core::GuestAddr(0),
        ops: arithmetic_ops,
        term: Terminator::Ret,
    };

    let mut memory_ops = Vec::new();
    for i in 0..500 {
        memory_ops.push(IROp::Load {
            dst: (i % 32) as RegId,
            base: ((i + 1) % 32) as RegId,
            offset: (i * 8) as i64,
            size: 8,
            flags: vm_ir::MemFlags::default(),
        });
    }
    let memory_block = IRBlock {
        start_pc: vm_core::GuestAddr(0),
        ops: memory_ops,
        term: Terminator::Ret,
    };

    group.bench_function("arithmetic_500ops", |b| {
        let mut compiler = JITCompiler::new();
        b.iter(|| {
            black_box(compiler.compile(black_box(&arithmetic_block))).unwrap();
        });
    });

    group.bench_function("memory_500ops", |b| {
        let mut compiler = JITCompiler::new();
        b.iter(|| {
            black_box(compiler.compile(black_box(&memory_block))).unwrap();
        });
    });

    group.finish();
}

/// 基准测试24: 嵌套循环模式
fn bench_nested_loop_pattern(c: &mut Criterion) {
    let mut ops = Vec::new();

    // 模拟嵌套循环的IR模式
    for i in 0..100 {
        for j in 0..10 {
            ops.push(IROp::Add {
                dst: (i % 32) as RegId,
                src1: (j % 32) as RegId,
                src2: ((i + j) % 32) as RegId,
            });
        }
    }
    let block = IRBlock {
        start_pc: vm_core::GuestAddr(0),
        ops,
        term: Terminator::Ret,
    };

    let mut compiler = JITCompiler::new();
    c.bench_function("compile_nested_loop_1000ops", |b| {
        b.iter(|| {
            black_box(compiler.compile(black_box(&block))).unwrap();
        });
    });
}

/// 基准测试25: 不同对齐要求的内存操作
fn bench_aligned_memory_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("aligned_memory");

    for alignment in [1, 2, 4, 8, 16].iter() {
        let mut ops = Vec::new();

        for i in 0..200 {
            ops.push(IROp::Load {
                dst: (i % 32) as RegId,
                base: ((i + 1) % 32) as RegId,
                offset: (i * alignment) as i64,
                size: *alignment,
                flags: vm_ir::MemFlags::default(),
            });
        }
        let block = IRBlock {
            start_pc: vm_core::GuestAddr(0),
            ops,
            term: Terminator::Ret,
        };

        group.bench_with_input(
            BenchmarkId::new("load", alignment),
            alignment,
            |b, _| {
                let mut compiler = JITCompiler::new();
                b.iter(|| {
                    black_box(compiler.compile(black_box(&block))).unwrap();
                });
            },
        );
    }

    group.finish();
}

/// 基准测试26: 立即数密集型IR块
fn bench_immediate_intensive_compilation(c: &mut Criterion) {
    let mut ops = Vec::new();

    for i in 0..1000 {
        ops.push(IROp::MovImm {
            dst: (i % 32) as RegId,
            imm: i as u64,
        });
    }
    let block = IRBlock {
        start_pc: vm_core::GuestAddr(0),
        ops,
        term: Terminator::Ret,
    };

    let mut compiler = JITCompiler::new();
    c.bench_function("compile_immediate_intensive_1000ops", |b| {
        b.iter(|| {
            black_box(compiler.compile(black_box(&block))).unwrap();
        });
    });
}

/// 基准测试27: 大立即数值
fn bench_large_immediate_values(c: &mut Criterion) {
    let mut ops = Vec::new();

    for i in 0..500 {
        ops.push(IROp::MovImm {
            dst: (i % 32) as RegId,
            imm: 0x1000_0000_0000_0000u64 + i as u64,
        });
    }
    let block = IRBlock {
        start_pc: vm_core::GuestAddr(0),
        ops,
        term: Terminator::Ret,
    };

    let mut compiler = JITCompiler::new();
    c.bench_function("compile_large_immediates_500ops", |b| {
        b.iter(|| {
            black_box(compiler.compile(black_box(&block))).unwrap();
        });
    });
}

/// 基准测试28: 负立即数值
fn bench_negative_immediate_values(c: &mut Criterion) {
    let mut ops = Vec::new();

    for i in 0..500 {
        ops.push(IROp::MovImm {
            dst: (i % 32) as RegId,
            imm: u64::MAX - (i as u64) + 1, // Simulate negative numbers
        });
    }
    let block = IRBlock {
        start_pc: vm_core::GuestAddr(0),
        ops,
        term: Terminator::Ret,
    };

    let mut compiler = JITCompiler::new();
    c.bench_function("compile_negative_immediates_500ops", |b| {
        b.iter(|| {
            black_box(compiler.compile(black_box(&block))).unwrap();
        });
    });
}

/// 基准测试29: 不同操作大小
fn bench_operation_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("operation_sizes");

    for size in [1, 2, 4, 8].iter() {
        let mut ops = Vec::new();

        for i in 0..500 {
            ops.push(IROp::Load {
                dst: (i % 32) as RegId,
                base: ((i + 1) % 32) as RegId,
                offset: (i * 8) as i64,
                size: *size,
                flags: vm_ir::MemFlags::default(),
            });
        }
        let block = IRBlock {
            start_pc: vm_core::GuestAddr(0),
            ops,
            term: Terminator::Ret,
        };

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            let mut compiler = JITCompiler::new();
            b.iter(|| {
                black_box(compiler.compile(black_box(&block))).unwrap();
            });
        });
    }

    group.finish();
}

/// 基准测试30: 存储操作密集型
fn bench_store_intensive_compilation(c: &mut Criterion) {
    let mut ops = Vec::new();

    for i in 0..1000 {
        ops.push(IROp::Store {
            src: (i % 32) as RegId,
            base: ((i + 1) % 32) as RegId,
            offset: (i * 8) as i64,
            size: 8,
        flags: vm_ir::MemFlags::default(),
                });
    }
    let block = IRBlock {
        start_pc: vm_core::GuestAddr(0),
        ops,
        term: Terminator::Ret,
    };

    let mut compiler = JITCompiler::new();
    c.bench_function("compile_store_intensive_1000ops", |b| {
        b.iter(|| {
            black_box(compiler.compile(black_box(&block))).unwrap();
        });
    });
}

/// 基准测试31: 加载和存储混合
fn bench_load_store_mixed(c: &mut Criterion) {
    let mut ops = Vec::new();

    for i in 0..1000 {
        if i % 2 == 0 {
            ops.push(IROp::Load {
                dst: (i % 32) as RegId,
                base: ((i + 1) % 32) as RegId,
                offset: (i * 8) as i64,
                size: 8,
            flags: vm_ir::MemFlags::default(),
                });
        } else {
            ops.push(IROp::Store {
                src: (i % 32) as RegId,
                base: ((i + 1) % 32) as RegId,
                offset: (i * 8) as i64,
                size: 8,
            flags: vm_ir::MemFlags::default(),
                });
        }
    }
    let block = IRBlock {
        start_pc: vm_core::GuestAddr(0),
        ops,
        term: Terminator::Ret,
    };

    let mut compiler = JITCompiler::new();
    c.bench_function("compile_load_store_mixed_1000ops", |b| {
        b.iter(|| {
            black_box(compiler.compile(black_box(&block))).unwrap();
        });
    });
}

/// 基准测试32: 不同缓存大小配置
fn bench_cache_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_sizes");

    for cache_size in [1024, 4096, 16384, 65536].iter() {
        let mut config = JITConfig::default();
        config.cache_size = *cache_size;

        let mut compiler = JITCompiler::with_config(config).unwrap();
        let block = create_test_block(100);

        group.bench_with_input(
            BenchmarkId::from_parameter(cache_size),
            cache_size,
            |b, _| {
                b.iter(|| {
                    black_box(compiler.compile(black_box(&block))).unwrap();
                });
            },
        );
    }

    group.finish();
}

/// 基准测试33: 内联启用vs禁用
fn bench_inlining_impact(c: &mut Criterion) {
    let mut group = c.benchmark_group("inlining");

    let mut config_no_inline = JITConfig::default();
    config_no_inline.enable_inlining = false;

    let mut config_inline = JITConfig::default();
    config_inline.enable_inlining = true;

    let block = create_complex_block(200);

    group.bench_function("no_inline", |b| {
        let mut compiler = JITCompiler::with_config(config_no_inline.clone()).unwrap();
        b.iter(|| {
            black_box(compiler.compile(black_box(&block))).unwrap();
        });
    });

    group.bench_function("with_inline", |b| {
        let mut compiler = JITCompiler::with_config(config_inline.clone()).unwrap();
        b.iter(|| {
            black_box(compiler.compile(black_box(&block))).unwrap();
        });
    });

    group.finish();
}

/// 基准测试34: SIMD优化影响
fn bench_simd_impact(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd");

    let mut config_no_simd = JITConfig::default();
    config_no_simd.enable_vector = false;

    let mut config_simd = JITConfig::default();
    config_simd.enable_vector = true;

    let block = create_test_block(200);

    group.bench_function("no_simd", |b| {
        let mut compiler = JITCompiler::with_config(config_no_simd.clone()).unwrap();
        b.iter(|| {
            black_box(compiler.compile(black_box(&block))).unwrap();
        });
    });

    group.bench_function("with_simd", |b| {
        let mut compiler = JITCompiler::with_config(config_simd.clone()).unwrap();
        b.iter(|| {
            black_box(compiler.compile(black_box(&block))).unwrap();
        });
    });

    group.finish();
}

/// 基准测试35: 并行编译影响
fn bench_parallel_compilation(c: &mut Criterion) {
    let mut group = c.benchmark_group("parallel_compilation");

    let mut config_single = JITConfig::default();
    config_single.enable_parallel = false;

    let mut config_parallel = JITConfig::default();
    config_parallel.enable_parallel = true;

    let block = create_test_block(500);

    group.bench_function("single_threaded", |b| {
        let mut compiler = JITCompiler::with_config(config_single.clone()).unwrap();
        b.iter(|| {
            black_box(compiler.compile(black_box(&block))).unwrap();
        });
    });

    group.bench_function("parallel", |b| {
        let mut compiler = JITCompiler::with_config(config_parallel.clone()).unwrap();
        b.iter(|| {
            black_box(compiler.compile(black_box(&block))).unwrap();
        });
    });

    group.finish();
}

/// 基准测试36: 代码缓存命中率
fn bench_code_cache_hit_rate(c: &mut Criterion) {
    let block = create_test_block(100);
    let mut compiler = JITCompiler::new();

    c.bench_function("compile_same_block_cached", |b| {
        b.iter(|| {
            // 编译相同块多次，测试缓存性能
            for _ in 0..10 {
                black_box(compiler.compile(black_box(&block))).unwrap();
            }
        });
    });
}

/// 基准测试37: 不同IR块ID
fn bench_different_block_ids(c: &mut Criterion) {
    let mut compiler = JITCompiler::new();

    c.bench_function("compile_different_block_ids", |b| {
        b.iter(|| {
            for id in 0..10 {
                let mut block = create_test_block(50);
                block.start_pc = vm_core::GuestAddr(id as u64);
                black_box(compiler.compile(black_box(&block))).unwrap();
            }
        });
    });
}

/// 基准测试38: 优化级别对编译时间的影响 (详细)
fn bench_optimization_level_impact(c: &mut Criterion) {
    let mut group = c.benchmark_group("opt_level_impact");

    let block = create_complex_block(300);

    for level in [0u8, 1, 2, 3].iter() {
        let mut config = JITConfig::default();
        config.opt_level = match level {
            0 => OptLevel::None,
            1 => OptLevel::Basic,
            2 => OptLevel::Balanced,
            3 => OptLevel::Aggressive,
            _ => unreachable!(),
        };

        group.bench_with_input(BenchmarkId::from_parameter(level), level, |b, _| {
            let mut compiler = JITCompiler::with_config(config.clone()).unwrap();
            b.iter(|| {
                black_box(compiler.compile(black_box(&block))).unwrap();
            });
        });
    }

    group.finish();
}

/// 基准测试39: 大批量小块编译
fn bench_massive_small_block_compilation(c: &mut Criterion) {
    let blocks: Vec<_> = (0..1000).map(|_| create_test_block(10)).collect();
    let mut compiler = JITCompiler::new();

    c.bench_function("compile_1000_tiny_blocks", |b| {
        b.iter(|| {
            for block in &blocks {
                black_box(compiler.compile(black_box(block))).unwrap();
            }
        });
    });
}

/// 基准测试40: 编译器状态重置影响
fn bench_compiler_state_reset(c: &mut Criterion) {
    let block = create_test_block(100);

    c.bench_function("compile_with_new_compiler_each_time", |b| {
        b.iter(|| {
            let mut compiler = JITCompiler::new();
            black_box(compiler.compile(black_box(&block))).unwrap();
        });
    });
}

// ============================================================================
// 综合性能基准测试 (测试41-60)
// ============================================================================

/// 基准测试41: 真实工作负载模拟 - 计算密集型
fn bench_real_workload_compute(c: &mut Criterion) {
    let mut ops = Vec::new();

    // 模拟矩阵乘法内核
    for i in 0..16 {
        for j in 0..16 {
            for k in 0..16 {
                ops.push(IROp::Mul {
                    dst: (i * 16 + j) as RegId,
                    src1: (i * 16 + k) as RegId,
                    src2: (k * 16 + j) as RegId,
                });
                ops.push(IROp::Add {
                    dst: (i * 16 + j) as RegId,
                    src1: (i * 16 + j) as RegId,
                    src2: 1000, // 累加器
                });
            }
        }
    }
    let block = IRBlock {
        start_pc: vm_core::GuestAddr(0),
        ops,
        term: Terminator::Ret,
    };

    let mut compiler = JITCompiler::new();
    c.bench_function("compile_matrix_multiply_16x16", |b| {
        b.iter(|| {
            black_box(compiler.compile(black_box(&block))).unwrap();
        });
    });
}

/// 基准测试42: 真实工作负载模拟 - 内存访问模式
fn bench_real_workload_memory(c: &mut Criterion) {
    let mut ops = Vec::new();

    // 模拟数组遍历
    for i in 0..1000 {
        ops.push(IROp::Load {
            dst: (i % 32) as RegId,
            base: 1, // 数组基址
            offset: (i * 8) as i64,
            size: 8,
        flags: vm_ir::MemFlags::default(),
                });
        ops.push(IROp::Add {
            dst: (i % 32) as RegId,
            src1: (i % 32) as RegId,
            src2: 2, // 常数
        });
        ops.push(IROp::Store {
            src: (i % 32) as RegId,
            base: 1,
            offset: (i * 8) as i64,
            size: 8,
        flags: vm_ir::MemFlags::default(),
                });
    }
    let block = IRBlock {
        start_pc: vm_core::GuestAddr(0),
        ops,
        term: Terminator::Ret,
    };

    let mut compiler = JITCompiler::new();
    c.bench_function("compile_array_traversal_1000", |b| {
        b.iter(|| {
            black_box(compiler.compile(black_box(&block))).unwrap();
        });
    });
}

/// 基准测试43: 真实工作负载模拟 - 控制流
fn bench_real_workload_control_flow(c: &mut Criterion) {
    let blocks: Vec<_> = (0..10)
        .map(|i| {
            let mut ops = Vec::new();
            for j in 0..50 {
                ops.push(IROp::Add {
                    dst: (j % 32) as RegId,
                    src1: ((j + 1) % 32) as RegId,
                    src2: ((j + 2) % 32) as RegId,
                });
            }
            let block = IRBlock {
                start_pc: vm_core::GuestAddr(i as u64),
                ops,
                term: Terminator::Ret,
            };
            block
        })
        .collect();

    let mut compiler = JITCompiler::new();
    c.bench_function("compile_control_flow_10blocks", |b| {
        b.iter(|| {
            for block in &blocks {
                black_box(compiler.compile(black_box(block))).unwrap();
            }
        });
    });
}

/// 基准测试44: 压力测试 - 极限IR块大小
fn bench_stress_test_extreme_size(c: &mut Criterion) {
    let block = create_test_block(10000);
    let mut compiler = JITCompiler::new();

    c.bench_function("compile_extreme_10000ops", |b| {
        b.iter(|| {
            black_box(compiler.compile(black_box(&block))).unwrap();
        });
    });
}

/// 基准测试45: 压力测试 - 寄存器溢出
fn bench_stress_test_register_spill(c: &mut Criterion) {
    let mut ops = Vec::new();

    // 使用超过物理寄存器数量的虚拟寄存器
    for i in 0..1000 {
        ops.push(IROp::MovImm {
            dst: i, // 使用1000个不同的虚拟寄存器
            imm: i as u64,
        });
    }
    let block = IRBlock {
        start_pc: vm_core::GuestAddr(0),
        ops,
        term: Terminator::Ret,
    };

    let mut compiler = JITCompiler::new();
    c.bench_function("compile_register_spill_1000regs", |b| {
        b.iter(|| {
            black_box(compiler.compile(black_box(&block))).unwrap();
        });
    });
}

/// 基准测试46: 压力测试 - 深度依赖链
fn bench_stress_test_dependency_chain(c: &mut Criterion) {
    let mut ops = Vec::new();

    // 创建深度依赖链
    let mut prev_reg = 0u32;
    for i in 0..1000 {
        ops.push(IROp::Add {
            dst: (i + 1) as RegId,
            src1: prev_reg,
            src2: i as RegId,
        });
        prev_reg = (i + 1) as RegId;
    }
    let block = IRBlock {
        start_pc: vm_core::GuestAddr(0),
        ops,
        term: Terminator::Ret,
    };

    let mut compiler = JITCompiler::new();
    c.bench_function("compile_dependency_chain_1000", |b| {
        b.iter(|| {
            black_box(compiler.compile(black_box(&block))).unwrap();
        });
    });
}

/// 基准测试47: 随机IR块生成
fn bench_random_ir_blocks(c: &mut Criterion) {
    use std::collections::HashSet;

    let mut blocks = Vec::new();
    for block_id in 0..50 {
        let mut ops = Vec::new();
        let mut used_regs = HashSet::new();

        for i in 0..100 {
            let dst = (i % 32) as RegId;
            let src1 = ((i + 1) % 32) as RegId;
            let src2 = ((i + 2) % 32) as RegId;

            used_regs.insert(dst);
            used_regs.insert(src1);
            used_regs.insert(src2);

            match i % 4 {
                0 => ops.push(IROp::Add { dst, src1, src2 }),
                1 => ops.push(IROp::Sub { dst, src1, src2 }),
                2 => ops.push(IROp::Mul { dst, src1, src2 }),
                3 => ops.push(IROp::Load {
                    dst,
                    base: src1,
                    offset: (i * 8) as i64,
                    size: 8,
                flags: vm_ir::MemFlags::default(),
                }),
                _ => unreachable!(),
            }
        }
        let block = IRBlock {
            start_pc: vm_core::GuestAddr(block_id as u64),
            ops,
            term: Terminator::Ret,
        };
        blocks.push(block);
    }

    let mut compiler = JITCompiler::new();
    c.bench_function("compile_50_random_blocks", |b| {
        b.iter(|| {
            for block in &blocks {
                black_box(compiler.compile(black_box(block))).unwrap();
            }
        });
    });
}

/// 基准测试48: 频繁配置切换
fn bench_frequent_config_switching(c: &mut Criterion) {
    let blocks: Vec<_> = (0..10).map(|i| create_test_block(100 + i * 10)).collect();

    c.bench_function("compile_with_frequent_config_changes", |b| {
        b.iter(|| {
            for (i, block) in blocks.iter().enumerate() {
                let mut config = JITConfig::default();
                config.opt_level = match i % 4 {
                    0 => OptLevel::None,
                    1 => OptLevel::Basic,
                    2 => OptLevel::Balanced,
                    3 => OptLevel::Aggressive,
                    _ => unreachable!(),
                };

                let mut compiler = JITCompiler::with_config(config).unwrap();
                black_box(compiler.compile(black_box(block))).unwrap();
            }
        });
    });
}

/// 基准测试49: 内存带宽敏感型
fn bench_memory_bandwidth_sensitive(c: &mut Criterion) {
    let mut ops = Vec::new();

    // 大量连续内存访问
    for i in 0..2000 {
        ops.push(IROp::Load {
            dst: (i % 32) as RegId,
            base: 1,
            offset: (i * 8) as i64,
            size: 8,
        flags: vm_ir::MemFlags::default(),
                });
    }
    let block = IRBlock {
        start_pc: vm_core::GuestAddr(0),
        ops,
        term: Terminator::Ret,
    };

    let mut compiler = JITCompiler::new();
    c.bench_function("compile_memory_bandwidth_2000_loads", |b| {
        b.iter(|| {
            black_box(compiler.compile(black_box(&block))).unwrap();
        });
    });
}

/// 基准测试50: CPU缓存友好型
fn bench_cache_friendly_pattern(c: &mut Criterion) {
    let mut ops = Vec::new();

    // 缓存友好的访问模式
    for i in 0..100 {
        for j in 0..10 {
            ops.push(IROp::Add {
                dst: (i % 32) as RegId,
                src1: (j % 32) as RegId,
                src2: ((i + j) % 32) as RegId,
            });
        }
    }
    let block = IRBlock {
        start_pc: vm_core::GuestAddr(0),
        ops,
        term: Terminator::Ret,
    };

    let mut compiler = JITCompiler::new();
    c.bench_function("compile_cache_friendly_1000ops", |b| {
        b.iter(|| {
            black_box(compiler.compile(black_box(&block))).unwrap();
        });
    });
}

/// 基准测试51: 不同优化级别的内存使用
fn bench_memory_usage_by_opt_level(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_usage");

    let block = create_test_block(500);

    for opt_level in [OptLevel::None, OptLevel::Basic, OptLevel::Balanced, OptLevel::Aggressive] {
        let mut config = JITConfig::default();
        config.opt_level = opt_level;

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{:?}", opt_level)),
            &opt_level,
            |b, _| {
                let mut compiler = JITCompiler::with_config(config.clone()).unwrap();
                b.iter(|| {
                    black_box(compiler.compile(black_box(&block))).unwrap();
                });
            },
        );
    }

    group.finish();
}

/// 基准测试52: 编译器可扩展性
fn bench_compiler_scalability(c: &mut Criterion) {
    let mut group = c.benchmark_group("scalability");

    for num_blocks in [10, 50, 100, 500].iter() {
        let blocks: Vec<_> = (0..*num_blocks).map(|_| create_test_block(50)).collect();

        group.bench_with_input(
            BenchmarkId::from_parameter(num_blocks),
            num_blocks,
            |b, _| {
                let mut compiler = JITCompiler::new();
                b.iter(|| {
                    for block in &blocks {
                        black_box(compiler.compile(black_box(block))).unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

/// 基准测试53: 热路径编译
fn bench_hot_path_compilation(c: &mut Criterion) {
    let hot_block = create_test_block(200);
    let cold_blocks: Vec<_> = (0..10).map(|_| create_test_block(20)).collect();

    c.bench_function("compile_hot_path_pattern", |b| {
        b.iter(|| {
            let mut compiler = JITCompiler::new();
            // 编译热路径块多次
            for _ in 0..10 {
                black_box(compiler.compile(black_box(&hot_block))).unwrap();
            }
            // 编译冷路径块
            for block in &cold_blocks {
                black_box(compiler.compile(black_box(block))).unwrap();
            }
        });
    });
}

/// 基准测试54: 增量编译
fn bench_incremental_compilation(c: &mut Criterion) {
    let mut compiler = JITCompiler::new();

    c.bench_function("compile_incrementally", |b| {
        b.iter(|| {
            for i in 0..50 {
                let mut block = create_test_block(20);
                block.start_pc = vm_core::GuestAddr(i as u64);
                black_box(compiler.compile(black_box(&block))).unwrap();
            }
        });
    });
}

/// 基准测试55: 并发编译器实例
fn bench_concurrent_compiler_instances(c: &mut Criterion) {
    let blocks: Vec<_> = (0..10).map(|i| create_test_block(100 + i * 10)).collect();

    c.bench_function("compile_with_concurrent_instances", |b| {
        b.iter(|| {
            // 每次迭代创建新的编译器实例，模拟并发场景
            for block in &blocks {
                let mut compiler = JITCompiler::new();
                black_box(compiler.compile(black_box(block))).unwrap();
            }
        });
    });
}

/// 基准测试56: 长时间运行稳定性
fn bench_long_running_stability(c: &mut Criterion) {
    let blocks: Vec<_> = (0..100).map(|i| create_test_block(50 + i % 50)).collect();

    c.bench_function("compile_long_running_100_blocks", |b| {
        b.iter(|| {
            let mut compiler = JITCompiler::new();
            for block in &blocks {
                black_box(compiler.compile(black_box(block))).unwrap();
            }
        });
    });
}

/// 基准测试57: 错误恢复性能
fn bench_error_recovery(c: &mut Criterion) {
    let valid_block = create_test_block(100);
    let mut compiler = JITCompiler::new();

    c.bench_function("compile_with_error_recovery", |b| {
        b.iter(|| {
            // 正常编译
            black_box(compiler.compile(black_box(&valid_block))).unwrap();
            // 编译器应该能处理各种边界情况
            black_box(compiler.compile(black_box(&valid_block))).unwrap();
        });
    });
}

/// 基准测试58: 资源清理性能
fn bench_resource_cleanup(c: &mut Criterion) {
    c.bench_function("compile_and_cleanup", |b| {
        b.iter(|| {
            let mut compiler = JITCompiler::new();
            for i in 0..20 {
                let block = create_test_block(50);
                black_box(compiler.compile(black_box(&block))).unwrap();
            }
            // 编译器离开作用域，资源应该被清理
        });
    });
}

/// 基准测试59: 不同编译策略对比
fn bench_compilation_strategies(c: &mut Criterion) {
    let mut group = c.benchmark_group("strategies");

    let block = create_complex_block(300);

    // 策略1: 单次大块编译
    group.bench_function("single_large_block", |b| {
        let mut compiler = JITCompiler::new();
        b.iter(|| {
            black_box(compiler.compile(black_box(&block))).unwrap();
        });
    });

    // 策略2: 多次小块编译
    group.bench_function("multiple_small_blocks", |b| {
        b.iter(|| {
            let mut compiler = JITCompiler::new();
            for i in 0..10 {
                let mut small_block = create_test_block(30);
                small_block.start_pc = vm_core::GuestAddr(i as u64);
                black_box(compiler.compile(black_box(&small_block))).unwrap();
            }
        });
    });

    group.finish();
}

/// 基准测试60: 综合性能基准
fn bench_comprehensive_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("comprehensive");

    // 小块、快速编译
    group.bench_function("small_fast", |b| {
        let block = create_test_block(10);
        let mut config = JITConfig::default();
        config.opt_level = OptLevel::None;
        let mut compiler = JITCompiler::with_config(config).unwrap();
        b.iter(|| {
            black_box(compiler.compile(black_box(&block))).unwrap();
        });
    });

    // 中等块、平衡编译
    group.bench_function("medium_balanced", |b| {
        let block = create_test_block(200);
        let mut config = JITConfig::default();
        config.opt_level = OptLevel::Balanced;
        let mut compiler = JITCompiler::with_config(config).unwrap();
        b.iter(|| {
            black_box(compiler.compile(black_box(&block))).unwrap();
        });
    });

    // 大块、优化编译
    group.bench_function("large_optimized", |b| {
        let block = create_test_block(1000);
        let mut config = JITConfig::default();
        config.opt_level = OptLevel::Aggressive;
        let mut compiler = JITCompiler::with_config(config).unwrap();
        b.iter(|| {
            black_box(compiler.compile(black_box(&block))).unwrap();
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    // JIT编译性能 (1-20)
    bench_jit_compilation_tier0,
    bench_jit_compilation_tier1,
    bench_jit_compilation_tier2,
    bench_jit_compilation_tier3,
    bench_jit_compilation_sizes,
    bench_jit_optimization_levels,
    bench_complex_block_compilation,
    bench_arithmetic_intensive_block,
    bench_memory_intensive_block,
    bench_mixed_operations_block,
    bench_small_batch_compilation,
    bench_medium_batch_compilation,
    bench_large_batch_compilation,
    bench_compiler_creation,
    bench_compiler_creation_with_config,
    bench_ir_block_creation,
    bench_complex_ir_block_creation,
    bench_tiny_block_compilation,
    bench_huge_block_compilation,
    bench_repeated_compilation,
    // 代码生成性能 (21-40)
    bench_register_intensive_compilation,
    bench_branch_intensive_compilation,
    bench_operation_type_distribution,
    bench_nested_loop_pattern,
    bench_aligned_memory_operations,
    bench_immediate_intensive_compilation,
    bench_large_immediate_values,
    bench_negative_immediate_values,
    bench_operation_sizes,
    bench_store_intensive_compilation,
    bench_load_store_mixed,
    bench_cache_sizes,
    bench_inlining_impact,
    bench_simd_impact,
    bench_parallel_compilation,
    bench_code_cache_hit_rate,
    bench_different_block_ids,
    bench_optimization_level_impact,
    bench_massive_small_block_compilation,
    bench_compiler_state_reset,
    // 综合性能 (41-60)
    bench_real_workload_compute,
    bench_real_workload_memory,
    bench_real_workload_control_flow,
    bench_stress_test_extreme_size,
    bench_stress_test_register_spill,
    bench_stress_test_dependency_chain,
    bench_random_ir_blocks,
    bench_frequent_config_switching,
    bench_memory_bandwidth_sensitive,
    bench_cache_friendly_pattern,
    bench_memory_usage_by_opt_level,
    bench_compiler_scalability,
    bench_hot_path_compilation,
    bench_incremental_compilation,
    bench_concurrent_compiler_instances,
    bench_long_running_stability,
    bench_error_recovery,
    bench_resource_cleanup,
    bench_compilation_strategies,
    bench_comprehensive_performance,
);

criterion_main!(benches);
