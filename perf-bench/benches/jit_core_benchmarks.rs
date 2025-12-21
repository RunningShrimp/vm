use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::time::Duration;
use vm_core::{GuestAddr};
use vm_engine_jit::Jit;
use vm_ir::{IRBlock, IRBuilder, IROp, MemFlags, Terminator};

/// 创建不同大小的 IR 块用于基准测试
fn create_benchmark_block(op_count: usize) -> IRBlock {
    let mut builder = IRBuilder::new(GuestAddr(0x1000));

    for i in 0..op_count {
        match i % 4 {
            0 => builder.push(IROp::Add {
                dst: (i % 16) as u32,
                src1: ((i + 1) % 16) as u32,
                src2: ((i + 2) % 16) as u32,
            }),
            1 => builder.push(IROp::Sub {
                dst: (i % 16) as u32,
                src1: ((i + 1) % 16) as u32,
                src2: ((i + 2) % 16) as u32,
            }),
            2 => builder.push(IROp::And {
                dst: (i % 16) as u32,
                src1: ((i + 1) % 16) as u32,
                src2: ((i + 2) % 16) as u32,
            }),
            _ => builder.push(IROp::Nop),
        }
    }

    builder.set_term(Terminator::Ret);
    builder.build()
}

/// 基准测试：JIT 编译延迟
fn bench_jit_compile_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("JIT Compile Latency");
    group.measurement_time(Duration::from_secs(5));

    for op_count in [10, 50, 100, 500, 1000].iter() {
        let block = create_benchmark_block(*op_count);

        group.throughput(Throughput::Elements(*op_count as u64));
        group.bench_with_input(BenchmarkId::new("ops", op_count), &block, |b, block| {
            b.iter(|| {
                let mut jit = Jit::new();
                std::hint::black_box(jit.compile_only(block))
            });
        });
    }

    group.finish();
}

/// 基准测试：JIT 执行性能
fn bench_jit_execution_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("JIT Execution Performance");
    group.measurement_time(Duration::from_secs(5));

    for op_count in [10, 50, 100].iter() {
        let block = create_benchmark_block(*op_count);

        group.throughput(Throughput::Elements(*op_count as u64));
        group.bench_with_input(BenchmarkId::new("ops", op_count), &block, |b, block| {
            let mut jit = Jit::new();
            let compiled = jit.compile_only(block);

            b.iter(|| {
                std::hint::black_box(compiled.0);
            });
        });
    }

    group.finish();
}

/// 基准测试：内存操作性能
fn bench_memory_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("Memory Operations");
    group.measurement_time(Duration::from_secs(3));

    let mut jit = Jit::new();
    let mut builder = IRBuilder::new(GuestAddr(0x1000));

    // 创建包含内存操作的 IR 块
    for i in 0..20 {
        builder.push(IROp::Store {
            src: i as u32,
            base: 0,
            offset: 0x2000 + i * 4,
            size: 4,
            flags: MemFlags::default(),
        });
        builder.push(IROp::Load {
            dst: i as u32,
            base: 0,
            offset: 0x2000 + i * 4,
            size: 4,
            flags: MemFlags::default(),
        });
    }
    builder.set_term(Terminator::Ret);
    let block = builder.build();

    group.bench_function("memory_ops", |b| {
        b.iter(|| {
            std::hint::black_box(jit.compile_only(&block));
        });
    });

    group.finish();
}

/// 基准测试：JIT 编译器优化
fn bench_jit_compiler_optimizations(c: &mut Criterion) {
    let mut group = c.benchmark_group("JIT Compiler Optimizations");
    group.measurement_time(Duration::from_secs(3));

    // 创建一个包含冗余操作的 IR 块
    let mut builder = IRBuilder::new(GuestAddr(0x1000));
    for i in 0..50 {
        builder.push(IROp::MovImm { dst: i as u32, imm: 42 });
        builder.push(IROp::MovImm { dst: i as u32, imm: 42 }); // 冗余操作
        builder.push(IROp::Add {
            dst: i as u32,
            src1: i as u32,
            src2: 0,
        }); // 空操作
    }
    builder.set_term(Terminator::Ret);
    let block = builder.build();

    group.bench_function("redundant_ops", |b| {
        let mut jit = Jit::new();
        b.iter(|| {
            std::hint::black_box(jit.compile_only(&block));
        });
    });

    group.finish();
}

/// 基准测试：并发 JIT 编译
fn bench_concurrent_jit_compilation(c: &mut Criterion) {
    let mut group = c.benchmark_group("Concurrent JIT Compilation");
    group.measurement_time(Duration::from_secs(5));

    let blocks: Vec<_> = (0..10).map(|_| create_benchmark_block(100)).collect();

    group.bench_function("parallel_compile", |b| {
        b.iter(|| {
            let handles: Vec<_> = blocks.iter().map(|block| {
                let block_clone = block.clone();
                std::thread::spawn(move || {
                    let mut jit = Jit::new();
                    std::hint::black_box(jit.compile_only(&block_clone))
                })
            }).collect();

            for handle in handles {
                std::hint::black_box(handle.join().unwrap());
            }
        });
    });

    group.finish();
}

/// 基准测试：JIT 缓存性能
fn bench_jit_cache_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("JIT Cache Performance");
    group.measurement_time(Duration::from_secs(3));

    let block = create_benchmark_block(50);
    let mut jit = Jit::new();

    // 预热缓存
    let _ = jit.compile_only(&block);

    group.bench_function("cache_hit", |b| {
        b.iter(|| {
            std::hint::black_box(jit.compile_only(&block));
        });
    });

    group.finish();
}

/// 基准测试：代码生成质量
fn bench_code_generation_quality(c: &mut Criterion) {
    let mut group = c.benchmark_group("Code Generation Quality");
    group.measurement_time(Duration::from_secs(3));

    let mut jit = Jit::new();
    let mut builder = IRBuilder::new(GuestAddr(0x1000));

    // 创建复杂的算术运算
    for i in 0..10 {
        builder.push(IROp::Add {
            dst: (i % 16) as u32,
            src1: ((i + 1) % 16) as u32,
            src2: ((i + 2) % 16) as u32,
        });
        builder.push(IROp::Sub {
            dst: ((i + 3) % 16) as u32,
            src1: ((i + 4) % 16) as u32,
            src2: ((i + 5) % 16) as u32,
        });
    }
    builder.set_term(Terminator::Ret);
    let block = builder.build();

    group.bench_function("control_flow", |b| {
        b.iter(|| {
            std::hint::black_box(jit.compile_only(&block));
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_jit_compile_latency,
    bench_jit_execution_performance,
    bench_memory_operations,
    bench_jit_compiler_optimizations,
    bench_concurrent_jit_compilation,
    bench_jit_cache_performance,
    bench_code_generation_quality
);
criterion_main!(benches);