//! JIT 核心性能基准测试套件
//!
//! 测量 JIT 引擎的核心性能指标，包括：
//! - 编译延迟
//! - 代码缓存命中率
//! - 可执行内存分配
//! - Safepoint 开销

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use std::sync::Arc;
use std::time::Duration;
use vm_core::GuestAddr;
use vm_engine_jit::{
    executable_memory::ExecutableMemory,
    safepoint::{SafepointManager, SafepointPoller},
    Jit, JitContext,
};
use vm_ir::{IRBlock, IRBuilder, IROp, Terminator};

/// 创建不同大小的 IR 块用于基准测试
fn create_benchmark_block(op_count: usize) -> IRBlock {
    let mut builder = IRBuilder::new(GuestAddr(0x1000));

    for i in 0..op_count {
        match i % 4 {
            0 => builder.push(IROp::Add {
                dst: (i % 16) as u8,
                src1: ((i + 1) % 16) as u8,
                src2: ((i + 2) % 16) as u8,
            }),
            1 => builder.push(IROp::Sub {
                dst: (i % 16) as u8,
                src1: ((i + 1) % 16) as u8,
                src2: ((i + 2) % 16) as u8,
            }),
            2 => builder.push(IROp::And {
                dst: (i % 16) as u8,
                src1: ((i + 1) % 16) as u8,
                src2: ((i + 2) % 16) as u8,
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
        group.bench_with_input(
            BenchmarkId::new("ops", op_count),
            &block,
            |b, block| {
                b.iter(|| {
                    let mut jit = Jit::new();
                    black_box(jit.compile_only(block))
                });
            },
        );
    }

    group.finish();
}

/// 基准测试：代码缓存命中
fn bench_code_cache_hit(c: &mut Criterion) {
    let mut group = c.benchmark_group("Code Cache Hit");
    group.measurement_time(Duration::from_secs(3));

    let block = create_benchmark_block(100);
    let mut jit = Jit::new();

    // 预热：首次编译
    jit.compile_only(&block);

    group.bench_function("cache_hit", |b| {
        b.iter(|| {
            black_box(jit.compile_only(&block))
        });
    });

    group.finish();
}

/// 基准测试：可执行内存分配
fn bench_executable_memory_alloc(c: &mut Criterion) {
    let mut group = c.benchmark_group("Executable Memory");
    group.measurement_time(Duration::from_secs(3));

    for size in [4096, 16384, 65536, 262144].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(
            BenchmarkId::new("alloc", size),
            size,
            |b, &size| {
                b.iter(|| {
                    let mem = ExecutableMemory::allocate(size).unwrap();
                    black_box(mem)
                });
            },
        );
    }

    group.finish();
}

/// 基准测试：W^X 切换
fn bench_wx_toggle(c: &mut Criterion) {
    let mut group = c.benchmark_group("W^X Toggle");
    group.measurement_time(Duration::from_secs(3));

    let mut mem = ExecutableMemory::allocate(4096).unwrap();
    
    // 写入一些代码
    #[cfg(target_arch = "x86_64")]
    let code = vec![0x90; 1000]; // NOP sled
    
    #[cfg(target_arch = "aarch64")]
    let code = vec![0x1F, 0x20, 0x03, 0xD5].repeat(250); // NOP sled
    
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    let code = vec![0x00; 1000];
    
    mem.write(&code).unwrap();

    group.bench_function("write_to_execute", |b| {
        b.iter(|| {
            mem.make_executable().unwrap();
            mem.make_writable().unwrap();
        });
    });

    group.finish();
}

/// 基准测试：Safepoint 轮询开销
fn bench_safepoint_polling(c: &mut Criterion) {
    let mut group = c.benchmark_group("Safepoint Polling");
    group.measurement_time(Duration::from_secs(3));

    let manager = Arc::new(SafepointManager::new());

    // 无安全点请求时的轮询开销
    group.bench_function("poll_no_request", |b| {
        let mut poller = SafepointPoller::new(manager.clone(), 1000);
        b.iter(|| {
            for _ in 0..1000 {
                black_box(poller.poll());
            }
        });
    });

    // 仅检查开销（不进入安全点）
    group.bench_function("check_only", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                black_box(manager.check_safepoint());
            }
        });
    });

    group.finish();
}

/// 基准测试：JIT 上下文创建
fn bench_jit_context(c: &mut Criterion) {
    let mut group = c.benchmark_group("JIT Context");
    group.measurement_time(Duration::from_secs(2));

    group.bench_function("create_default", |b| {
        b.iter(|| {
            black_box(JitContext::default())
        });
    });

    group.finish();
}

/// 基准测试：多块编译
fn bench_multi_block_compile(c: &mut Criterion) {
    let mut group = c.benchmark_group("Multi Block Compile");
    group.measurement_time(Duration::from_secs(5));

    let blocks: Vec<IRBlock> = (0..100)
        .map(|i| {
            let mut builder = IRBuilder::new(GuestAddr(0x1000 + (i * 0x100) as u64));
            for _ in 0..20 {
                builder.push(IROp::Add {
                    dst: 0,
                    src1: 1,
                    src2: 2,
                });
            }
            builder.set_term(Terminator::Ret);
            builder.build()
        })
        .collect();

    group.bench_function("compile_100_blocks", |b| {
        b.iter(|| {
            let mut jit = Jit::new();
            for block in &blocks {
                black_box(jit.compile_only(block));
            }
        });
    });

    // 测试缓存效率
    group.bench_function("cached_100_blocks", |b| {
        let mut jit = Jit::new();
        // 预热
        for block in &blocks {
            jit.compile_only(block);
        }

        b.iter(|| {
            for block in &blocks {
                black_box(jit.compile_only(block));
            }
        });
    });

    group.finish();
}

criterion_group!(
    name = jit_benches;
    config = Criterion::default()
        .sample_size(100)
        .warm_up_time(Duration::from_millis(500));
    targets =
        bench_jit_compile_latency,
        bench_code_cache_hit,
        bench_executable_memory_alloc,
        bench_wx_toggle,
        bench_safepoint_polling,
        bench_jit_context,
        bench_multi_block_compile
);

criterion_main!(jit_benches);

