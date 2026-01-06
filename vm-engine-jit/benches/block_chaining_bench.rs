//! JIT块链接优化性能基准测试
//!
//! 测量块链接优化对JIT编译和执行性能的影响。

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use vm_core::GuestAddr;
use vm_engine_jit::block_chaining::BlockChainer;
use vm_ir::{IRBlock, IRBuilder, Terminator};

/// 创建线性块链（用于测试链式跳转）
fn create_linear_chain(length: usize) -> Vec<IRBlock> {
    let mut blocks = Vec::new();

    for i in 0..length {
        let addr = GuestAddr(0x1000 + (i as u64 * 0x100));
        let mut builder = IRBuilder::new(addr);

        if i < length - 1 {
            // 跳转到下一个块
            builder.set_term(Terminator::Jmp {
                target: GuestAddr(0x1000 + ((i + 1) as u64 * 0x100)),
            });
        } else {
            // 最后一个块返回
            builder.set_term(Terminator::Ret);
        }

        blocks.push(builder.build());
    }

    blocks
}

/// 创建复杂控制流块
fn create_complex_flow() -> Vec<IRBlock> {
    let mut blocks = Vec::new();

    // 主块
    let mut builder1 = IRBuilder::new(GuestAddr(0x1000));
    builder1.set_term(Terminator::CondJmp {
        cond: 1,
        target_true: GuestAddr(0x2000),
        target_false: GuestAddr(0x3000),
    });
    blocks.push(builder1.build());

    // True分支
    let mut builder2 = IRBuilder::new(GuestAddr(0x2000));
    builder2.set_term(Terminator::Jmp {
        target: GuestAddr(0x4000),
    });
    blocks.push(builder2.build());

    // False分支
    let mut builder3 = IRBuilder::new(GuestAddr(0x3000));
    builder3.set_term(Terminator::Jmp {
        target: GuestAddr(0x4000),
    });
    blocks.push(builder3.build());

    // 汇合块
    let mut builder4 = IRBuilder::new(GuestAddr(0x4000));
    builder4.set_term(Terminator::Ret);
    blocks.push(builder4.build());

    blocks
}

fn bench_block_analysis(c: &mut Criterion) {
    let mut group = c.benchmark_group("block_analysis");

    // 测试不同数量块的分析性能
    for size in [10, 50, 100, 500].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let blocks = create_linear_chain(size);
            let mut chainer = BlockChainer::new();

            b.iter(|| {
                for block in &blocks {
                    black_box(chainer.analyze_block(block).unwrap());
                }
            });
        });
    }

    group.finish();
}

fn bench_chain_building(c: &mut Criterion) {
    let mut group = c.benchmark_group("chain_building");

    // 测试不同链长度的构建性能
    for chain_length in [4, 8, 16, 32].iter() {
        group.bench_with_input(
            BenchmarkId::new("chain_length", chain_length),
            chain_length,
            |b, &chain_length| {
                let blocks = create_linear_chain(chain_length);
                let mut chainer = BlockChainer::with_config(chain_length, true);

                // 预先分析所有块
                for block in &blocks {
                    chainer.analyze_block(block).unwrap();
                }

                b.iter(|| {
                    let mut chainer_clone = chainer.clone(); // 注意：这里需要实际clone实现
                    black_box(chainer_clone.build_chains());
                });
            },
        );
    }

    group.finish();
}

fn bench_hot_path_optimization(c: &mut Criterion) {
    let mut group = c.benchmark_group("hot_path");

    // 测试热路径优化（高频率块）
    group.bench_function("with_hot_path", |b| {
        let blocks = create_linear_chain(10);
        let mut chainer = BlockChainer::with_config(16, true);

        // 模拟热路径：第一个块执行100次
        for _ in 0..100 {
            chainer.analyze_block(&blocks[0]).unwrap();
        }

        // 其他块执行1次
        for block in blocks.iter().skip(1) {
            chainer.analyze_block(block).unwrap();
        }

        b.iter(|| {
            let mut chainer_clone = chainer.clone();
            black_box(chainer_clone.build_chains());
        });
    });

    // 测试无热路径优化
    group.bench_function("without_hot_path", |b| {
        let blocks = create_linear_chain(10);
        let mut chainer = BlockChainer::with_config(16, false);

        for block in &blocks {
            chainer.analyze_block(block).unwrap();
        }

        b.iter(|| {
            let mut chainer_clone = chainer.clone();
            black_box(chainer_clone.build_chains());
        });
    });

    group.finish();
}

fn bench_complex_flow(c: &mut Criterion) {
    let mut group = c.benchmark_group("complex_flow");

    group.bench_function("conditional_branches", |b| {
        let blocks = create_complex_flow();
        let mut chainer = BlockChainer::new();

        for block in &blocks {
            chainer.analyze_block(block).unwrap();
        }

        b.iter(|| {
            let mut chainer_clone = chainer.clone();
            black_box(chainer_clone.build_chains());
        });
    });

    group.finish();
}

fn bench_lookup_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("lookup");

    let blocks = create_linear_chain(100);
    let mut chainer = BlockChainer::new();

    for block in &blocks {
        chainer.analyze_block(block).unwrap();
    }

    chainer.build_chains();

    group.bench_function("get_chain", |b| {
        b.iter(|| {
            black_box(chainer.get_chain(GuestAddr(0x1000)));
        });
    });

    group.bench_function("get_link", |b| {
        b.iter(|| {
            black_box(chainer.get_link(GuestAddr(0x1000), GuestAddr(0x2000)));
        });
    });

    group.bench_function("all_chains_iter", |b| {
        b.iter(|| {
            black_box(chainer.all_chains().count());
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_block_analysis,
    bench_chain_building,
    bench_hot_path_optimization,
    bench_complex_flow,
    bench_lookup_performance
);

criterion_main!(benches);
