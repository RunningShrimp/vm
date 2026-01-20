//! NUMA性能基准测试
//!
//! 测试NUMA分配器和跨节点访问的性能指标

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::alloc::Layout;
use vm_core::AddressTranslator;

use vm_mem::{NumaAllocPolicy, NumaAllocator, NumaNodeInfo};

// Use std::hint::black_box instead of criterion's deprecated version
use std::hint::black_box;

fn create_test_nodes() -> Vec<NumaNodeInfo> {
    vec![
        NumaNodeInfo {
            node_id: 0,
            total_memory: 8 * 1024 * 1024 * 1024,     // 8GB
            available_memory: 7 * 1024 * 1024 * 1024, // 7GB
            cpu_mask: 0xFF,                           // CPU 0-7
        },
        NumaNodeInfo {
            node_id: 1,
            total_memory: 8 * 1024 * 1024 * 1024,     // 8GB
            available_memory: 7 * 1024 * 1024 * 1024, // 7GB
            cpu_mask: 0xFF00,                         // CPU 8-15
        },
    ]
}

/// 基准测试: NUMA分配性能 (不同策略)
fn bench_numa_allocation_policies(c: &mut Criterion) {
    let mut group = c.benchmark_group("numa_allocation_policies");

    let nodes = create_test_nodes();

    for policy in [
        NumaAllocPolicy::Local,
        NumaAllocPolicy::Interleave,
        NumaAllocPolicy::Preferred(0),
    ]
    .iter()
    {
        group.bench_with_input(
            BenchmarkId::new(format!("{:?}", policy), nodes.len()),
            policy,
            |b, &policy| {
                let allocator = NumaAllocator::new(nodes.clone(), policy);
                let layout = Layout::from_size_align(1024, 8).unwrap();

                b.iter(|| {
                    let ptr = allocator.allocate(layout);
                    if let Ok(ptr) = ptr {
                        black_box(ptr);
                        // 避免内存泄漏，分配后立即释放
                        unsafe {
                            allocator.deallocate(ptr, 1024);
                        }
                    }
                });
            },
        );
    }

    group.finish();
}

/// 基准测试: 不同大小的NUMA分配
fn bench_numa_allocation_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("numa_allocation_sizes");

    let nodes = create_test_nodes();
    let allocator = NumaAllocator::new(nodes.clone(), NumaAllocPolicy::Local);

    for size in [64, 256, 1024, 4096, 16384, 65536].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let layout = Layout::from_size_align(size, 8).unwrap();

            b.iter(|| {
                let ptr = allocator.allocate(layout);
                if let Ok(ptr) = ptr {
                    black_box(ptr);
                    unsafe {
                        allocator.deallocate(ptr, size);
                    }
                }
            });
        });
    }

    group.finish();
}

/// 基准测试: NUMA批量分配
fn bench_numa_batch_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("numa_batch_allocation");

    let nodes = create_test_nodes();

    for batch_size in [10, 50, 100, 500, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &batch_size| {
                b.iter(|| {
                    let allocator = NumaAllocator::new(nodes.clone(), NumaAllocPolicy::Local);
                    let mut ptrs = Vec::with_capacity(batch_size);

                    for _i in 0..batch_size {
                        let layout = Layout::from_size_align(1024, 8).unwrap();
                        if let Ok(ptr) = allocator.allocate(layout) {
                            ptrs.push((ptr, 1024));
                        }
                    }

                    // 清理
                    let count = ptrs.len();
                    for (ptr, size) in ptrs {
                        unsafe {
                            allocator.deallocate(ptr, size);
                        }
                    }

                    black_box(count);
                });
            },
        );
    }

    group.finish();
}

/// 基准测试: NUMA跨节点访问模拟
fn bench_numa_cross_node_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("numa_cross_node_access");

    let nodes = create_test_nodes();

    // Local vs Remote访问对比
    for access_type in ["local", "remote"].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(access_type),
            access_type,
            |b, &_access_type| {
                let allocator = NumaAllocator::new(nodes.clone(), NumaAllocPolicy::Local);

                // 模拟分配
                let layout = Layout::from_size_align(1024, 8).unwrap();
                let ptrs: Vec<_> = (0..100)
                    .filter_map(|_| allocator.allocate(layout).ok())
                    .collect();

                b.iter(|| {
                    // 模拟内存访问
                    for ptr in &ptrs {
                        black_box(ptr.as_ptr());
                    }
                });

                // 清理
                for ptr in ptrs {
                    unsafe {
                        allocator.deallocate(ptr, 1024);
                    }
                }
            },
        );
    }

    group.finish();
}

/// 基准测试: NUMA分配和释放循环
fn bench_numa_alloc_dealloc_cycle(c: &mut Criterion) {
    let mut group = c.benchmark_group("numa_alloc_dealloc_cycle");

    let nodes = create_test_nodes();

    for iterations in [100, 1000, 10000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(iterations),
            iterations,
            |b, &iterations| {
                b.iter(|| {
                    let allocator = NumaAllocator::new(nodes.clone(), NumaAllocPolicy::Local);
                    let layout = Layout::from_size_align(1024, 8).unwrap();
                    let mut ptrs = Vec::new();

                    // 分配
                    for _ in 0..iterations {
                        if let Ok(ptr) = allocator.allocate(layout) {
                            ptrs.push(ptr);
                        }
                    }

                    // 释放
                    for ptr in ptrs {
                        unsafe {
                            allocator.deallocate(ptr, 1024);
                        }
                    }

                    black_box(iterations);
                });
            },
        );
    }

    group.finish();
}

/// 基准测试: NUMA碎片化场景
fn bench_numa_fragmentation(c: &mut Criterion) {
    let mut group = c.benchmark_group("numa_fragmentation");

    let nodes = create_test_nodes();

    for pattern in ["sequential", "random"].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(pattern),
            pattern,
            |b, &_pattern| {
                let allocator = NumaAllocator::new(nodes.clone(), NumaAllocPolicy::Local);
                let layout = Layout::from_size_align(1024, 8).unwrap();
                let mut ptrs = Vec::new();

                // 分配1000次
                for _ in 0..1000 {
                    if let Ok(ptr) = allocator.allocate(layout) {
                        ptrs.push(ptr);
                    }
                }

                // 随机释放一半
                use std::collections::HashSet;
                let mut to_free = HashSet::new();
                while to_free.len() < 500 {
                    let idx = (ptrs.len() as f64 * rand::random::<f64>()) as usize;
                    to_free.insert(idx);
                }

                for idx in to_free {
                    unsafe {
                        allocator.deallocate(ptrs[idx], 1024);
                    }
                }

                // 再分配500次
                b.iter(|| {
                    for _ in 0..500 {
                        let _ = allocator.allocate(layout);
                    }
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_numa_allocation_policies,
    bench_numa_allocation_sizes,
    bench_numa_batch_allocation,
    bench_numa_cross_node_access,
    bench_numa_alloc_dealloc_cycle,
    bench_numa_fragmentation,
);

criterion_main!(benches);
