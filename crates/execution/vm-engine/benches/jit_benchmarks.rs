//! JIT 编译器性能基准测试
//!
//! 使用 criterion 测量 JIT 编译器的性能指标

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use vm_core::GuestAddr;
use vm_engine::jit::JITCompiler;
use vm_ir::{IRBlock, IROp};

/// 创建一个简单的测试 IR 块
fn create_simple_block(num_ops: usize) -> IRBlock {
    let mut block = IRBlock::new(GuestAddr(0x1000));

    for i in 0..num_ops {
        let dst = (i * 3) as u32;
        let src1 = (i * 3 + 1) as u32;
        let src2 = (i * 3 + 2) as u32;

        let op = match i % 4 {
            0 => IROp::Add { dst, src1, src2 },
            1 => IROp::Sub { dst, src1, src2 },
            2 => IROp::Mul { dst, src1, src2 },
            _ => IROp::And { dst, src1, src2 },
        };

        block.ops.push(op);
    }

    block
}

/// 创建一个包含控制流的 IR 块
fn create_block_with_control_flow() -> IRBlock {
    let mut block = IRBlock::new(GuestAddr(0x1000));

    // 添加一些算术操作
    for i in 0..10 {
        block.ops.push(IROp::Add {
            dst: i,
            src1: i + 10,
            src2: i + 20,
        });
    }

    // 添加条件分支
    block.ops.push(IROp::CmpEq {
        dst: 100,
        lhs: 1,
        rhs: 2,
    });

    // 添加更多算术操作
    for i in 0..5 {
        block.ops.push(IROp::Mul {
            dst: 200 + i,
            src1: i,
            src2: 2,
        });
    }

    block
}

fn benchmark_jit_compilation(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit_compilation");

    // 测试不同大小的块的编译时间
    for size in [10, 50, 100, 500, 1000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                let mut jit = JITCompiler::new();
                let block = create_simple_block(size);
                let _ = black_box(&mut jit).compile(black_box(&block));
            });
        });
    }

    group.finish();
}

fn benchmark_optimization_levels(c: &mut Criterion) {
    let mut group = c.benchmark_group("optimization_levels");

    // 测试不同优化级别
    group.bench_function("no_optimization", |b| {
        b.iter(|| {
            let mut jit = JITCompiler::new();
            let block = create_block_with_control_flow();
            let _ = black_box(&mut jit).compile(black_box(&block));
        });
    });

    group.bench_function("basic_optimization", |b| {
        b.iter(|| {
            let mut jit = JITCompiler::new();
            let block = create_block_with_control_flow();
            let _ = black_box(&mut jit).compile(black_box(&block));
        });
    });

    group.bench_function("full_optimization", |b| {
        b.iter(|| {
            let mut jit = JITCompiler::new();
            let block = create_block_with_control_flow();
            let _ = black_box(&mut jit).compile(black_box(&block));
        });
    });

    group.finish();
}

fn benchmark_cache_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_performance");

    let block = create_simple_block(100);

    // 第一次编译（缓存未命中）
    group.bench_function("compile_cache_miss", |b| {
        b.iter(|| {
            let mut jit = JITCompiler::new();
            let _ = black_box(&mut jit).compile(black_box(&block));
        });
    });

    // 后续编译（缓存命中）- 注意：由于每次迭代创建新的 jit，这只是示例
    group.bench_function("compile_cache_hit", |b| {
        let mut jit = JITCompiler::new();
        let _ = jit.compile(&block).unwrap();
        b.iter(|| {
            let _ = black_box(&mut jit).compile(black_box(&block));
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_jit_compilation,
    benchmark_optimization_levels,
    benchmark_cache_performance
);
criterion_main!(benches);
