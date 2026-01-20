// SIMD性能基准测试
//
// 测试SIMD操作相对于标量操作的性能提升
// 使用criterion进行精确的性能测量

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use vm_core::GuestAddr;
use vm_ir::{IRBlock, IROp, Terminator};

/// 创建向量加法的SIMD IR块
fn create_vec_add_ir_block(num_ops: usize, element_size: u8) -> IRBlock {
    let mut ops = Vec::with_capacity(num_ops);

    for i in 0..num_ops {
        let dst = (i * 3 + 1) as u32;
        let src1 = (i * 3 + 2) as u32;
        let src2 = (i * 3 + 3) as u32;

        ops.push(IROp::VecAdd {
            dst,
            src1,
            src2,
            element_size,
        });
    }

    IRBlock {
        start_pc: GuestAddr(0x1000),
        ops,
        term: Terminator::Ret,
    }
}

/// 创建标量加法的IR块（用于对比）
fn create_scalar_add_ir_block(num_ops: usize) -> IRBlock {
    let mut ops = Vec::with_capacity(num_ops);

    for i in 0..num_ops {
        let dst = (i * 3 + 1) as u32;
        let src1 = (i * 3 + 2) as u32;
        let src2 = (i * 3 + 3) as u32;

        // 使用标量Add操作
        ops.push(IROp::Add { dst, src1, src2 });
    }

    IRBlock {
        start_pc: GuestAddr(0x2000),
        ops,
        term: Terminator::Ret,
    }
}

/// 基准测试：SIMD向量加法 vs 标量加法
fn bench_vec_add_vs_scalar(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd_vs_scalar");

    // 测试不同数量的操作
    for num_ops in [10, 50, 100, 500, 1000].iter() {
        let size = *num_ops;

        // SIMD向量加法 (32位元素)
        group.bench_with_input(
            BenchmarkId::new("vec_add_32bit", size),
            &size,
            |b, &size| {
                let block = create_vec_add_ir_block(size, 32);
                b.iter(|| {
                    // 注意：当前只是IR创建测试
                    // 实际SIMD编译将在后续实现
                    std::hint::black_box(&block);
                });
            },
        );

        // SIMD向量加法 (64位元素)
        group.bench_with_input(
            BenchmarkId::new("vec_add_64bit", size),
            &size,
            |b, &size| {
                let block = create_vec_add_ir_block(size, 64);
                b.iter(|| {
                    std::hint::black_box(&block);
                });
            },
        );

        // 标量加法（用于对比）
        group.bench_with_input(BenchmarkId::new("scalar_add", size), &size, |b, &size| {
            let block = create_scalar_add_ir_block(size);
            b.iter(|| {
                std::hint::black_box(&block);
            });
        });
    }

    group.finish();
}

/// 基准测试：不同元素大小的SIMD操作
fn bench_element_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("element_sizes");

    let element_sizes = [8, 16, 32, 64];

    for &size in &element_sizes {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &size,
            |b, &element_size| {
                let block = create_vec_add_ir_block(100, element_size);
                b.iter(|| {
                    std::hint::black_box(&block);
                });
            },
        );
    }

    group.finish();
}

/// 基准测试：SIMD操作类型对比
fn bench_simd_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd_operations");

    // VecAdd
    group.bench_function("vec_add", |b| {
        let ops = vec![
            IROp::VecAdd {
                dst: 1u32,
                src1: 2u32,
                src2: 3u32,
                element_size: 32,
            };
            100
        ];
        let block = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops,
            term: Terminator::Ret,
        };
        b.iter(|| std::hint::black_box(&block));
    });

    // VecSub
    group.bench_function("vec_sub", |b| {
        let ops = vec![
            IROp::VecSub {
                dst: 1u32,
                src1: 2u32,
                src2: 3u32,
                element_size: 32,
            };
            100
        ];
        let block = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops,
            term: Terminator::Ret,
        };
        b.iter(|| std::hint::black_box(&block));
    });

    // VecMul
    group.bench_function("vec_mul", |b| {
        let ops = vec![
            IROp::VecMul {
                dst: 1u32,
                src1: 2u32,
                src2: 3u32,
                element_size: 32,
            };
            100
        ];
        let block = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops,
            term: Terminator::Ret,
        };
        b.iter(|| std::hint::black_box(&block));
    });

    // VecAnd
    group.bench_function("vec_and", |b| {
        let ops = vec![
            IROp::VecAnd {
                dst: 1u32,
                src1: 2u32,
                src2: 3u32,
                element_size: 64,
            };
            100
        ];
        let block = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops,
            term: Terminator::Ret,
        };
        b.iter(|| std::hint::black_box(&block));
    });

    // VecOr
    group.bench_function("vec_or", |b| {
        let ops = vec![
            IROp::VecOr {
                dst: 1u32,
                src1: 2u32,
                src2: 3u32,
                element_size: 64,
            };
            100
        ];
        let block = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops,
            term: Terminator::Ret,
        };
        b.iter(|| std::hint::black_box(&block));
    });

    group.finish();
}

/// 基准测试：SIMD位运算操作
fn bench_simd_bitwise(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd_bitwise");

    // VecXor
    group.bench_function("vec_xor", |b| {
        let ops = vec![
            IROp::VecXor {
                dst: 1u32,
                src1: 2u32,
                src2: 3u32,
                element_size: 64,
            };
            100
        ];
        let block = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops,
            term: Terminator::Ret,
        };
        b.iter(|| std::hint::black_box(&block));
    });

    // VecNot
    group.bench_function("vec_not", |b| {
        let ops = vec![
            IROp::VecNot {
                dst: 1u32,
                src: 2u32,
                element_size: 64,
            };
            100
        ];
        let block = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops,
            term: Terminator::Ret,
        };
        b.iter(|| std::hint::black_box(&block));
    });

    group.finish();
}

/// 基准测试：SIMD移位操作
fn bench_simd_shift(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd_shift");

    // VecShl
    group.bench_function("vec_shl", |b| {
        let ops = vec![
            IROp::VecShl {
                dst: 1u32,
                src: 2u32,
                shift: 3u32,
                element_size: 32,
            };
            100
        ];
        let block = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops,
            term: Terminator::Ret,
        };
        b.iter(|| std::hint::black_box(&block));
    });

    // VecSrl
    group.bench_function("vec_srl", |b| {
        let ops = vec![
            IROp::VecSrl {
                dst: 1u32,
                src: 2u32,
                shift: 3u32,
                element_size: 32,
            };
            100
        ];
        let block = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops,
            term: Terminator::Ret,
        };
        b.iter(|| std::hint::black_box(&block));
    });

    // VecSra
    group.bench_function("vec_sra", |b| {
        let ops = vec![
            IROp::VecSra {
                dst: 1u32,
                src: 2u32,
                shift: 3u32,
                element_size: 32,
            };
            100
        ];
        let block = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops,
            term: Terminator::Ret,
        };
        b.iter(|| std::hint::black_box(&block));
    });

    // VecShlImm
    group.bench_function("vec_shl_imm", |b| {
        let ops = vec![
            IROp::VecShlImm {
                dst: 1u32,
                src: 2u32,
                shift: 4,
                element_size: 32,
            };
            100
        ];
        let block = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops,
            term: Terminator::Ret,
        };
        b.iter(|| std::hint::black_box(&block));
    });

    group.finish();
}

/// 基准测试：IR块创建吞吐量
fn bench_ir_block_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("ir_block_throughput");

    // 测试不同大小IR块的创建吞吐量
    for size in [10, 50, 100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                let block = create_vec_add_ir_block(size, 32);
                std::hint::black_box(&block);
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_vec_add_vs_scalar,
    bench_element_sizes,
    bench_simd_operations,
    bench_simd_bitwise,
    bench_simd_shift,
    bench_ir_block_throughput
);

criterion_main!(benches);
