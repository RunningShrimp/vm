// 性能基准测试 - 异步执行引擎
//
// 这个文件包含用于测试VM执行性能的基准测试
// 运行: cargo bench --bench async_performance_benchmark

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};

// 模拟的基准测试场景
fn execute_simple_instructions(iterations: u64) -> u64 {
    let mut result = 0u64;
    for i in 0..iterations {
        result = result.wrapping_add(i);
        result = result.wrapping_mul(i.wrapping_add(1));
    }
    result
}

fn async_execution_basic(c: &mut Criterion) {
    c.bench_function("simple_execution_1k", |b| {
        b.iter(|| execute_simple_instructions(black_box(1000)))
    });
    
    c.bench_function("simple_execution_10k", |b| {
        b.iter(|| execute_simple_instructions(black_box(10000)))
    });
    
    c.bench_function("simple_execution_100k", |b| {
        b.iter(|| execute_simple_instructions(black_box(100000)))
    });
}

fn async_execution_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("execution_scaling");
    
    for iterations in [1000, 10000, 100000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(iterations),
            iterations,
            |b, &iterations| {
                b.iter(|| execute_simple_instructions(black_box(iterations)))
            },
        );
    }
    group.finish();
}

fn lock_contention_simulation(lock_count: u32, operations: u32) -> u64 {
    use std::sync::Arc;
    use parking_lot::Mutex;
    
    let counter = Arc::new(Mutex::new(0u64));
    let mut handles = vec![];
    
    for _ in 0..lock_count {
        let counter_clone = Arc::clone(&counter);
        let handle = std::thread::spawn(move || {
            for _ in 0..operations {
                let mut val = counter_clone.lock();
                *val = val.wrapping_add(1);
            }
        });
        handles.push(handle);
    }
    
    for handle in handles {
        let _ = handle.join();
    }
    
    *counter.lock()
}

fn lock_contention_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("lock_contention");
    
    for thread_count in [2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::new("threads", thread_count),
            thread_count,
            |b, &thread_count| {
                b.iter(|| lock_contention_simulation(black_box(thread_count), black_box(100)))
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    async_execution_basic,
    async_execution_scaling,
    lock_contention_benchmark
);

criterion_main!(benches);
