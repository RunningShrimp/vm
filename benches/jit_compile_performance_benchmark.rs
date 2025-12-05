//! JIT编译性能基准测试
//!
//! 测试JIT编译性能，包括同步编译、异步编译、编译队列等场景

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use vm_core::GuestAddr;
use vm_engine_jit::Jit;
use vm_ir::{IRBlock, IRBuilder, IROp, Terminator};

fn create_test_ir_block(addr: GuestAddr) -> IRBlock {
    let mut builder = IRBuilder::new(addr);
    // 创建一个包含多种操作的块
    builder.push(IROp::MovImm { dst: 1, imm: 42 });
    builder.push(IROp::Add {
        dst: 0,
        src1: 0,
        src2: 1,
    });
    builder.push(IROp::Sub {
        dst: 0,
        src1: 0,
        src2: 1,
    });
    builder.push(IROp::Mul {
        dst: 0,
        src1: 0,
        src2: 1,
    });
    builder.set_term(Terminator::Jmp { target: addr + 16 });
    builder.build()
}

fn create_complex_ir_block(addr: GuestAddr) -> IRBlock {
    let mut builder = IRBuilder::new(addr);
    // 创建一个更复杂的块
    for i in 0..20 {
        builder.push(IROp::MovImm { dst: i % 32, imm: i as u64 });
        if i > 0 {
            builder.push(IROp::Add {
                dst: 0,
                src1: 0,
                src2: (i - 1) % 32,
            });
        }
    }
    builder.set_term(Terminator::Jmp { target: addr + 320 });
    builder.build()
}

fn bench_sync_compile(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit_sync_compile");
    
    group.bench_function("simple_block", |b| {
        let mut jit = Jit::new();
        let block = create_test_ir_block(0x1000);
        b.iter(|| {
            black_box(jit.compile_only(black_box(&block)));
        });
    });
    
    group.bench_function("complex_block", |b| {
        let mut jit = Jit::new();
        let block = create_complex_ir_block(0x2000);
        b.iter(|| {
            black_box(jit.compile_only(black_box(&block)));
        });
    });
    
    group.finish();
}

fn bench_async_compile(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("jit_async_compile");
    
    group.bench_function("simple_block", |b| {
        let mut jit = Jit::new();
        let block = create_test_ir_block(0x1000);
        b.iter(|| {
            rt.block_on(async {
                let handle = jit.compile_async(black_box(block.clone()));
                black_box(handle.await);
            });
        });
    });
    
    group.bench_function("complex_block", |b| {
        let mut jit = Jit::new();
        let block = create_complex_ir_block(0x2000);
        b.iter(|| {
            rt.block_on(async {
                let handle = jit.compile_async(black_box(block.clone()));
                black_box(handle.await);
            });
        });
    });
    
    group.finish();
}

fn bench_concurrent_compile(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("jit_concurrent_compile");
    
    for count in [1, 4, 8, 16].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(count),
            count,
            |b, &count| {
                let jit = Arc::new(Mutex::new(Jit::new()));
                let blocks: Vec<_> = (0..count)
                    .map(|i| create_test_ir_block(0x1000 + i as u64 * 16))
                    .collect();
                
                b.iter(|| {
                    rt.block_on(async {
                        let mut handles = Vec::new();
                        for block in blocks.iter() {
                            let jit_clone = jit.clone();
                            let block_clone = block.clone();
                            let handle = tokio::spawn(async move {
                                let mut jit = jit_clone.lock().await;
                                jit.compile_async(block_clone).await
                            });
                            handles.push(handle);
                        }
                        
                        for handle in handles {
                            black_box(handle.await.unwrap());
                        }
                    });
                });
            },
        );
    }
    
    group.finish();
}

fn bench_compile_queue(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit_compile_queue");
    
    for queue_size in [10, 50, 100, 500].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(queue_size),
            queue_size,
            |b, &queue_size| {
                let mut jit = Jit::new();
                let blocks: HashMap<_, _> = (0..queue_size)
                    .map(|i| {
                        let addr = 0x1000 + i as u64 * 16;
                        (addr, create_test_ir_block(addr))
                    })
                    .collect();
                
                // 填充队列
                for (pc, _) in blocks.iter() {
                    jit.add_to_compile_queue(*pc, 100);
                }
                
                b.iter(|| {
                    black_box(jit.process_compile_queue(black_box(&blocks)));
                });
            },
        );
    }
    
    group.finish();
}

criterion_group!(
    benches,
    bench_sync_compile,
    bench_async_compile,
    bench_concurrent_compile,
    bench_compile_queue
);
criterion_main!(benches);


