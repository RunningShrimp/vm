//! 跨架构翻译性能基准测试
//!
//! 测试RISC-V到x86的翻译性能，验证P1-2缓存效果

use std::time::Duration;

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use vm_core::GuestAddr;
use vm_ir::{IRBlock, IRBuilder, IROp, Terminator};

/// 创建RISC-V算术操作IR块
fn create_arithmetic_ir_block(size: usize) -> IRBlock {
    let mut builder = IRBuilder::new(GuestAddr(0x1000));

    for i in 0..size {
        let dst = (i % 16) as u32;
        let src1 = ((i + 1) % 16) as u32;
        let src2 = ((i + 2) % 16) as u32;

        match i % 4 {
            0 => builder.push(IROp::Add { dst, src1, src2 }),
            1 => builder.push(IROp::Sub { dst, src1, src2 }),
            2 => builder.push(IROp::Mul { dst, src1, src2 }),
            _ => builder.push(IROp::Div { dst, src1, src2 }),
        }
    }

    builder.set_term(Terminator::Ret);
    builder.build()
}

/// 创建RISC-V内存操作IR块
fn create_memory_ir_block(size: usize) -> IRBlock {
    let mut builder = IRBuilder::new(GuestAddr(0x1000));

    for i in 0..size {
        let dst = (i % 16) as u32;
        let base = ((i + 1) % 16) as u32;
        let offset = (i * 8) as i64;

        match i % 2 {
            0 => builder.push(IROp::Load {
                dst,
                base,
                offset,
                size: 8,
                flags: vm_ir::MemFlags::default(),
            }),
            _ => builder.push(IROp::Store {
                src: dst,
                base,
                offset,
                size: 8,
                flags: vm_ir::MemFlags::default(),
            }),
        }
    }

    builder.set_term(Terminator::Ret);
    builder.build()
}

/// 创建可融合的IR块（测试指令融合）
fn create_fusible_ir_block(size: usize) -> IRBlock {
    let mut builder = IRBuilder::new(GuestAddr(0x1000));

    for i in 0..size {
        let dst = (i % 16) as u32;
        let src = ((i + 1) % 16) as u32;

        // 创建可融合的模式：ADDI + LOAD
        builder.push(IROp::AddImm {
            dst,
            src,
            imm: i as i64,
        });
        builder.push(IROp::Load {
            dst: ((i + 2) % 16) as u32,
            base: dst,
            offset: 0,
            size: 8,
            flags: vm_ir::MemFlags::default(),
        });
    }

    builder.set_term(Terminator::Ret);
    builder.build()
}

/// 创建常量密集的IR块（测试常量传播）
fn create_constant_dense_ir_block(size: usize) -> IRBlock {
    let mut builder = IRBuilder::new(GuestAddr(0x1000));

    // 创建大量常量操作
    for i in 0..size {
        builder.push(IROp::MovImm {
            dst: (i % 16) as u32,
            imm: (i * 10) as u64,
        });
    }

    // 创建使用这些常量的操作
    for i in 0..size / 2 {
        builder.push(IROp::Add {
            dst: (i % 16) as u32,
            src1: (i % 16) as u32,
            src2: ((i + 1) % 16) as u32,
        });
    }

    builder.set_term(Terminator::Ret);
    builder.build()
}

/// 基础翻译性能基准测试
fn bench_basic_translation(c: &mut Criterion) {
    let mut group = c.benchmark_group("translation_basic");

    // 不同大小的IR块翻译
    for size in [10, 50, 100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let optimizer = TranslationOptimizer::new(1024);
            let ir_block = create_arithmetic_ir_block(size);

            b.iter(|| {
                black_box(optimizer.translate(
                    black_box(&ir_block),
                    GuestAddr(0x1000),
                    GuestAddr(0x1000 + size as u64 * 4),
                ));
            });
        });
    }

    group.finish();
}

/// 指令融合性能基准测试
fn bench_instruction_fusion(c: &mut Criterion) {
    let mut group = c.benchmark_group("instruction_fusion");

    // 测试融合功能
    group.bench_function("fusion_enabled", |b| {
        let mut fusion = InstructionFusion::new();
        let ir_block = create_fusible_ir_block(1000);

        b.iter(|| {
            black_box(fusion.fuse_block(black_box(&ir_block)));
        });
    });

    // 测试无融合
    group.bench_function("fusion_disabled", |b| {
        let ir_block = create_fusible_ir_block(1000);

        b.iter(|| {
            // 直接处理，不进行融合
            black_box(ir_block.ops.len());
        });
    });

    // 不同融合模式
    let mut fusion = InstructionFusion::new();
    let ir_block = create_fusible_ir_block(500);
    let fusion_result = fusion.fuse_block(&ir_block).unwrap();

    group.bench_function("analyze_fusion_result", |b| {
        b.iter(|| {
            black_box(&fusion_result);
        });
    });

    group.finish();
}

/// 常量传播性能基准测试
fn bench_constant_propagation(c: &mut Criterion) {
    let mut group = c.benchmark_group("constant_propagation");

    // 常量密集型代码
    for size in [100, 500, 1000, 5000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(
            BenchmarkId::new("constant_dense", size),
            size,
            |b, &size| {
                let optimizer = TranslationOptimizer::new(1024);
                let ir_block = create_constant_dense_ir_block(size);

                b.iter(|| {
                    // 测试常量传播优化
                    black_box(optimizer.constant_propagation(black_box(&ir_block)));
                });
            },
        );
    }

    group.finish();
}

/// 死代码消除性能基准测试
fn bench_dead_code_elimination(c: &mut Criterion) {
    let mut group = c.benchmark_group("dead_code_elimination");

    // 创建包含死代码的IR块
    let mut builder = IRBuilder::new(GuestAddr(0x1000));

    // 有用的代码
    for i in 0..500 {
        builder.push(IROp::Add {
            dst: (i % 16) as u32,
            src1: ((i + 1) % 16) as u32,
            src2: ((i + 2) % 16) as u32,
        });
    }

    // 死代码（从未使用的寄存器）
    for i in 500..1000 {
        builder.push(IROp::MovImm {
            dst: (i % 16) as u32,
            imm: i as u64,
        });
    }

    builder.set_term(Terminator::Ret);
    let ir_block_with_dce = builder.build();

    group.bench_function("with_dead_code", |b| {
        let optimizer = TranslationOptimizer::new(1024);

        b.iter(|| {
            black_box(optimizer.dead_code_elimination(black_box(&ir_block_with_dce)));
        });
    });

    // 无死代码的IR块
    let clean_ir_block = create_arithmetic_ir_block(1000);
    group.bench_function("without_dead_code", |b| {
        let optimizer = TranslationOptimizer::new(1024);

        b.iter(|| {
            black_box(optimizer.dead_code_elimination(black_box(&clean_ir_block)));
        });
    });

    group.finish();
}

/// 翻译缓存性能基准测试
fn bench_translation_cache(c: &mut Criterion) {
    let mut group = c.benchmark_group("translation_cache");

    // 缓存未命中
    group.bench_function("cache_miss", |b| {
        let optimizer = TranslationOptimizer::new(1024);
        let mut addr = 0x1000;

        b.iter(|| {
            let ir_block = create_arithmetic_ir_block(100);
            black_box(optimizer.translate(black_box(&ir_block), addr, addr + 1000));
            addr += 0x1000;
        });
    });

    // 缓存命中
    group.bench_function("cache_hit", |b| {
        let optimizer = TranslationOptimizer::new(1024);
        let ir_block = create_arithmetic_ir_block(100);
        let addr = GuestAddr(0x1000);

        // 预热缓存
        let _ = optimizer.translate(&ir_block, addr, addr + 1000);

        b.iter(|| {
            black_box(optimizer.translate(black_box(&ir_block), addr, addr + 1000));
        });
    });

    group.finish();
}

/// 不同类型操作的翻译性能
fn bench_operation_translation(c: &mut Criterion) {
    let mut group = c.benchmark_group("operation_translation");

    // 算术操作
    group.bench_function("arithmetic", |b| {
        let optimizer = TranslationOptimizer::new(1024);
        let ir_block = create_arithmetic_ir_block(1000);

        b.iter(|| {
            black_box(optimizer.translate(
                black_box(&ir_block),
                GuestAddr(0x1000),
                GuestAddr(0x2000),
            ));
        });
    });

    // 内存操作
    group.bench_function("memory", |b| {
        let optimizer = TranslationOptimizer::new(1024);
        let ir_block = create_memory_ir_block(1000);

        b.iter(|| {
            black_box(optimizer.translate(
                black_box(&ir_block),
                GuestAddr(0x1000),
                GuestAddr(0x2000),
            ));
        });
    });

    // 分支操作
    group.bench_function("branch", |b| {
        let mut builder = IRBuilder::new(GuestAddr(0x1000));
        for i in 0..500 {
            builder.push(IROp::CmpEq {
                dst: (i % 16) as u32,
                lhs: ((i + 1) % 16) as u32,
                rhs: ((i + 2) % 16) as u32,
            });
            builder.push(IROp::Beq {
                src1: ((i + 1) % 16) as u32,
                src2: ((i + 2) % 16) as u32,
                target: GuestAddr(0x1000 + (i * 4) as u64),
            });
        }
        builder.set_term(Terminator::Ret);
        let ir_block = builder.build();

        let optimizer = TranslationOptimizer::new(1024);

        b.iter(|| {
            black_box(optimizer.translate(
                black_box(&ir_block),
                GuestAddr(0x1000),
                GuestAddr(0x2000),
            ));
        });
    });

    group.finish();
}

/// 综合优化性能测试
fn bench_combined_optimizations(c: &mut Criterion) {
    let mut group = c.benchmark_group("combined_optimizations");

    // 无优化
    group.bench_function("no_optimization", |b| {
        let mut optimizer = TranslationOptimizer::new(1024);
        optimizer.set_optimizations(false, false, false, false);

        let ir_block = create_constant_dense_ir_block(500);

        b.iter(|| {
            black_box(optimizer.translate(
                black_box(&ir_block),
                GuestAddr(0x1000),
                GuestAddr(0x2000),
            ));
        });
    });

    // 仅缓存
    group.bench_function("cache_only", |b| {
        let mut optimizer = TranslationOptimizer::new(1024);
        optimizer.set_optimizations(true, false, false, false);

        let ir_block = create_constant_dense_ir_block(500);

        b.iter(|| {
            black_box(optimizer.translate(
                black_box(&ir_block),
                GuestAddr(0x1000),
                GuestAddr(0x2000),
            ));
        });
    });

    // 缓存 + 指令融合
    group.bench_function("cache_fusion", |b| {
        let mut optimizer = TranslationOptimizer::new(1024);
        optimizer.set_optimizations(true, true, false, false);

        let ir_block = create_fusible_ir_block(500);

        b.iter(|| {
            black_box(optimizer.translate(
                black_box(&ir_block),
                GuestAddr(0x1000),
                GuestAddr(0x2000),
            ));
        });
    });

    // 所有优化
    group.bench_function("all_optimizations", |b| {
        let optimizer = TranslationOptimizer::new(1024);

        let ir_block = create_constant_dense_ir_block(500);

        b.iter(|| {
            black_box(optimizer.translate(
                black_box(&ir_block),
                GuestAddr(0x1000),
                GuestAddr(0x2000),
            ));
        });
    });

    group.finish();
}

/// 配置基准测试
fn configure_criterion() -> Criterion {
    Criterion::default()
        .warm_up_time(Duration::from_secs(3))
        .measurement_time(Duration::from_secs(10))
        .sample_size(100)
}

criterion_group! {
    name = translation_benches;
    config = configure_criterion();
    targets =
        bench_basic_translation,
        bench_instruction_fusion,
        bench_constant_propagation,
        bench_dead_code_elimination,
        bench_translation_cache,
        bench_operation_translation,
        bench_combined_optimizations
}

criterion_main!(translation_benches);
