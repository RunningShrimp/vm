//! 内存优化性能基准测试
//!
//! 本模块提供内存优化相关的性能基准测试，包括：
//! - NUMA感知内存分配性能测试
//! - 透明大页(THP)性能测试
//! - 内存对齐优化测试
//! - 内存访问模式性能测试
//! - 内存压力测试

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use std::alloc::Layout;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::thread;
use vm_mem::{
    init_global_numa_allocator, NumaAllocPolicy, NumaAllocator, NumaNodeInfo,
    init_global_thp_manager, allocate_with_thp, deallocate_with_thp, is_thp_address,
    get_thp_usage_stats, ThpPolicy, TransparentHugePageManager, ThpConfig,
};

/// 创建测试NUMA节点信息
fn create_test_numa_nodes() -> Vec<NumaNodeInfo> {
    vec![
        NumaNodeInfo {
            node_id: 0,
            total_memory: 8 * 1024 * 1024 * 1024, // 8GB
            available_memory: 7 * 1024 * 1024 * 1024, // 7GB
            cpu_mask: 0xFF, // CPU 0-7
        },
        NumaNodeInfo {
            node_id: 1,
            total_memory: 8 * 1024 * 1024 * 1024, // 8GB
            available_memory: 7 * 1024 * 1024 * 1024, // 7GB
            cpu_mask: 0xFF00, // CPU 8-15
        },
    ]
}

/// NUMA本地分配性能测试
fn bench_numa_local_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("numa_local_allocation");
    
    let sizes = [1024, 4096, 16384, 65536, 262144, 1048576]; // 1KB到1MB
    let nodes = create_test_numa_nodes();
    
    for size in &sizes {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(
            BenchmarkId::new("local_allocation", size),
            size,
            |b, &size| {
                let allocator = NumaAllocator::new(nodes.clone(), NumaAllocPolicy::Local);
                let layout = Layout::from_size_align(size, 8).unwrap();
                
                b.iter(|| {
                    let mut ptrs = Vec::new();
                    for _ in 0..100 {
                        match allocator.allocate(layout) {
                            Ok(ptr) => ptrs.push(ptr),
                            _ => {}
                        }
                    }
                    
                    // 使用内存
                    for ptr in &ptrs {
                        unsafe {
                            std::ptr::write_bytes(ptr.as_ptr(), 0xAA, size);
                        }
                    }
                    
                    // 释放内存
                    for ptr in ptrs {
                        allocator.deallocate(ptr, size);
                    }
                });
            },
        );
    }
    
    group.finish();
}

/// NUMA交错分配性能测试
fn bench_numa_interleave_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("numa_interleave_allocation");
    
    let sizes = [1024, 4096, 16384, 65536, 262144, 1048576]; // 1KB到1MB
    let nodes = create_test_numa_nodes();
    
    for size in &sizes {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(
            BenchmarkId::new("interleave_allocation", size),
            size,
            |b, &size| {
                let allocator = NumaAllocator::new(nodes.clone(), NumaAllocPolicy::Interleave);
                let layout = Layout::from_size_align(size, 8).unwrap();
                
                b.iter(|| {
                    let mut ptrs = Vec::new();
                    for _ in 0..100 {
                        match allocator.allocate(layout) {
                            Ok(ptr) => ptrs.push(ptr),
                            _ => {}
                        }
                    }
                    
                    // 使用内存
                    for ptr in &ptrs {
                        unsafe {
                            std::ptr::write_bytes(ptr.as_ptr(), 0xBB, size);
                        }
                    }
                    
                    // 释放内存
                    for ptr in ptrs {
                        allocator.deallocate(ptr, size);
                    }
                });
            },
        );
    }
    
    group.finish();
}

/// NUMA多线程分配性能测试
fn bench_numa_multithread_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("numa_multithread_allocation");
    
    let thread_counts = [1, 2, 4, 8];
    let nodes = create_test_numa_nodes();
    
    for thread_count in &thread_counts {
        group.bench_with_input(
            BenchmarkId::new("multithread_allocation", thread_count),
            thread_count,
            |b, &thread_count| {
                let allocator = Arc::new(NumaAllocator::new(nodes.clone(), NumaAllocPolicy::Local));
                let layout = Layout::from_size_align(4096, 8).unwrap();
                
                b.iter(|| {
                    let mut handles = Vec::new();
                    
                    for _ in 0..thread_count {
                        let allocator_clone = allocator.clone();
                        let handle = thread::spawn(move || {
                            let mut ptrs = Vec::new();
                            for _ in 0..100 {
                                match allocator_clone.allocate(layout) {
                                    Ok(ptr) => ptrs.push(ptr),
                                    _ => {}
                                }
                            }
                            
                            // 使用内存
                            for ptr in &ptrs {
                                unsafe {
                                    std::ptr::write_bytes(ptr.as_ptr(), 0xCC, 4096);
                                }
                            }
                            
                            // 释放内存
                            for ptr in ptrs {
                                allocator_clone.deallocate(ptr, 4096);
                            }
                            
                            ptrs.len()
                        });
                        handles.push(handle);
                    }
                    
                    let mut total_allocations = 0;
                    for handle in handles {
                        total_allocations += handle.join().unwrap();
                    }
                    
                    black_box(total_allocations);
                });
            },
        );
    }
    
    group.finish();
}

/// THP分配性能测试
fn bench_thp_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("thp_allocation");
    
    let sizes = [4096, 65536, 1048576, 16777216]; // 4KB到16MB
    
    // 初始化THP管理器
    init_global_thp_manager(ThpPolicy::Transparent).unwrap();
    
    for size in &sizes {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(
            BenchmarkId::new("thp_allocation", size),
            size,
            |b, &size| {
                b.iter(|| {
                    let mut ptrs = Vec::new();
                    for _ in 0..50 {
                        match allocate_with_thp(size) {
                            Ok(ptr) if !ptr.is_null() => ptrs.push(ptr),
                            _ => {}
                        }
                    }
                    
                    // 使用内存
                    for ptr in &ptrs {
                        unsafe {
                            std::ptr::write_bytes(ptr, 0xDD, size);
                        }
                    }
                    
                    // 释放内存
                    for ptr in ptrs {
                        deallocate_with_thp(ptr, size);
                    }
                });
            },
        );
    }
    
    group.finish();
}

/// THP与常规分配性能比较
fn bench_thp_vs_regular_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("thp_vs_regular_allocation");
    
    let sizes = [4096, 65536, 1048576, 16777216]; // 4KB到16MB
    
    // 初始化THP管理器
    init_global_thp_manager(ThpPolicy::Transparent).unwrap();
    
    for size in &sizes {
        group.throughput(Throughput::Bytes(*size as u64));
        
        // THP分配
        group.bench_with_input(
            BenchmarkId::new("thp", size),
            size,
            |b, &size| {
                b.iter(|| {
                    let mut ptrs = Vec::new();
                    for _ in 0..50 {
                        match allocate_with_thp(size) {
                            Ok(ptr) if !ptr.is_null() => ptrs.push(ptr),
                            _ => {}
                        }
                    }
                    
                    // 使用内存
                    for ptr in &ptrs {
                        unsafe {
                            std::ptr::write_bytes(ptr, 0xEE, size);
                        }
                    }
                    
                    // 释放内存
                    for ptr in ptrs {
                        deallocate_with_thp(ptr, size);
                    }
                });
            },
        );
        
        // 常规分配
        group.bench_with_input(
            BenchmarkId::new("regular", size),
            size,
            |b, &size| {
                b.iter(|| {
                    let mut ptrs = Vec::new();
                    for _ in 0..50 {
                        let layout = Layout::from_size_align(size, 8).unwrap();
                        let ptr = unsafe { std::alloc::alloc(layout) };
                        if !ptr.is_null() {
                            ptrs.push(ptr);
                        }
                    }
                    
                    // 使用内存
                    for ptr in &ptrs {
                        unsafe {
                            std::ptr::write_bytes(ptr, 0xFF, size);
                        }
                    }
                    
                    // 释放内存
                    for ptr in ptrs {
                        let layout = Layout::from_size_align(size, 8).unwrap();
                        unsafe { std::alloc::dealloc(ptr, layout) };
                    }
                });
            },
        );
    }
    
    group.finish();
}

/// 内存访问模式性能测试
fn bench_memory_access_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_access_patterns");
    
    // 初始化THP管理器
    init_global_thp_manager(ThpPolicy::Transparent).unwrap();
    
    let size = 64 * 1024 * 1024; // 64MB
    let ptr = allocate_with_thp(size).unwrap();
    
    // 顺序访问
    group.bench_function("sequential_access", |b| {
        b.iter(|| {
            let mut sum = 0u64;
            for i in (0..size).step_by(8) {
                unsafe {
                    sum += *(ptr.add(i) as *const u64);
                }
            }
            black_box(sum);
        });
    });
    
    // 随机访问
    group.bench_function("random_access", |b| {
        let mut indices: Vec<usize> = (0..size/8).collect();
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        // 简单的伪随机打乱
        for i in 0..indices.len() {
            let mut hasher = DefaultHasher::new();
            (i * 2654435761).hash(&mut hasher);
            let j = hasher.finish() as usize % indices.len();
            indices.swap(i, j);
        }
        
        b.iter(|| {
            let mut sum = 0u64;
            for &i in &indices {
                unsafe {
                    sum += *(ptr.add(i * 8) as *const u64);
                }
            }
            black_box(sum);
        });
    });
    
    // 跳跃访问（模拟缓存未命中）
    group.bench_function("strided_access", |b| {
        b.iter(|| {
            let mut sum = 0u64;
            let stride = 4096; // 4KB，模拟缓存行大小
            for i in (0..size).step_by(stride) {
                unsafe {
                    sum += *(ptr.add(i) as *const u64);
                }
            }
            black_box(sum);
        });
    });
    
    deallocate_with_thp(ptr, size);
    group.finish();
}

/// 内存压力测试
fn bench_memory_pressure(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_pressure");
    
    // 初始化NUMA和THP
    init_global_numa_allocator(NumaAllocPolicy::Local).unwrap();
    init_global_thp_manager(ThpPolicy::Transparent).unwrap();
    
    let allocation_sizes = [1024 * 1024, 4 * 1024 * 1024, 16 * 1024 * 1024]; // 1MB, 4MB, 16MB
    
    for size in &allocation_sizes {
        group.bench_with_input(
            BenchmarkId::new("allocation_pressure", size),
            size,
            |b, &size| {
                b.iter(|| {
                    let mut ptrs = Vec::new();
                    let start = Instant::now();
                    
                    // 分配内存直到失败或达到限制
                    for _ in 0..100 {
                        match allocate_with_thp(size) {
                            Ok(ptr) if !ptr.is_null() => {
                                ptrs.push(ptr);
                                // 使用内存
                                unsafe {
                                    std::ptr::write_bytes(ptr, 0x55, size);
                                }
                            }
                            _ => break,
                        }
                    }
                    
                    let allocation_time = start.elapsed();
                    
                    // 释放内存
                    for ptr in ptrs {
                        deallocate_with_thp(ptr, size);
                    }
                    
                    black_box(allocation_time);
                });
            },
        );
    }
    
    group.finish();
}

/// 内存对齐性能测试
fn bench_memory_alignment(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_alignment");
    
    // 初始化THP管理器
    init_global_thp_manager(ThpPolicy::Transparent).unwrap();
    
    let size = 1024 * 1024; // 1MB
    let alignments = [1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096];
    
    for alignment in &alignments {
        group.bench_with_input(
            BenchmarkId::new("aligned_access", alignment),
            alignment,
            |b, &alignment| {
                // 分配对齐内存
                let layout = Layout::from_size_align(size + alignment, alignment).unwrap();
                let ptr = unsafe { std::alloc::alloc(layout) };
                let aligned_ptr = ((ptr as usize + alignment - 1) & !(alignment - 1)) as *mut u8;
                
                // 初始化内存
                unsafe {
                    std::ptr::write_bytes(aligned_ptr, 0x77, size);
                }
                
                b.iter(|| {
                    let mut sum = 0u64;
                    for i in (0..size).step_by(8) {
                        unsafe {
                            sum += *(aligned_ptr.add(i) as *const u64);
                        }
                    }
                    black_box(sum);
                });
                
                unsafe { std::alloc::dealloc(ptr, layout) };
            },
        );
    }
    
    group.finish();
}

criterion_group!(
    benches,
    bench_numa_local_allocation,
    bench_numa_interleave_allocation,
    bench_numa_multithread_allocation,
    bench_thp_allocation,
    bench_thp_vs_regular_allocation,
    bench_memory_access_patterns,
    bench_memory_pressure,
    bench_memory_alignment
);

criterion_main!(benches);