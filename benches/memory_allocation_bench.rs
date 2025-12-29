//! Memory Allocation Performance Benchmark
//!
//! Benchmarks memory allocator performance:
//! - Allocation/deallocation speed
//! - Pool efficiency
//! - Fragmentation rate
//! - Concurrent allocation performance
//!
//! Test cases: Small allocs, Large allocs, Concurrent allocs
//!
//! Run: cargo bench --bench memory_allocation_bench

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Mock memory pool
#[derive(Debug)]
struct MemoryPool {
    blocks: Vec<Vec<u8>>,
    pool_size: usize,
    block_size: usize,
    allocated: usize,
}

impl MemoryPool {
    fn new(pool_size: usize, block_size: usize) -> Self {
        Self {
            blocks: Vec::new(),
            pool_size,
            block_size,
            allocated: 0,
        }
    }

    fn allocate(&mut self) -> Option<*mut u8> {
        if self.allocated + self.block_size > self.pool_size {
            return None;
        }

        let block = vec![0u8; self.block_size];
        let ptr = block.as_ptr() as *mut u8;
        self.blocks.push(block);
        self.allocated += self.block_size;
        Some(ptr)
    }

    fn deallocate(&mut self, ptr: *mut u8) {
        // In a real implementation, this would return the block to the pool
        self.allocated -= self.block_size;
    }

    fn get_usage_ratio(&self) -> f64 {
        self.allocated as f64 / self.pool_size as f64
    }
}

/// Simple allocator
struct SimpleAllocator {
    memory: Vec<u8>,
    allocated: Vec<(usize, usize)>, // (offset, size)
    capacity: usize,
}

impl SimpleAllocator {
    fn new(capacity: usize) -> Self {
        Self {
            memory: vec![0u8; capacity],
            allocated: Vec::new(),
            capacity,
        }
    }

    fn allocate(&mut self, size: usize) -> Option<usize> {
        // Simple first-fit allocation
        let mut offset = 0;

        for &(alloc_offset, alloc_size) in &self.allocated {
            if offset + size <= alloc_offset {
                // Found space
                self.allocated.push((offset, size));
                self.allocated.sort_by_key(|&(off, _)| off);
                return Some(offset);
            }
            offset = alloc_offset + alloc_size;
        }

        // Check if there's space at the end
        if offset + size <= self.capacity {
            self.allocated.push((offset, size));
            return Some(offset);
        }

        None
    }

    fn deallocate(&mut self, offset: usize) {
        self.allocated.retain(|&(off, _)| off != offset);
    }

    fn get_fragmentation(&self) -> f64 {
        if self.allocated.is_empty() {
            return 0.0;
        }

        let total_free = self.capacity - self.allocated.iter().map(|(_, size)| size).sum::<usize>();
        let largest_free = self.calculate_largest_free_block();

        if largest_free == 0 {
            return 100.0;
        }

        ((total_free - largest_free) as f64 / total_free as f64) * 100.0
    }

    fn calculate_largest_free_block(&self) -> usize {
        let mut largest = 0;
        let mut offset = 0;

        for &(alloc_offset, alloc_size) in &self.allocated {
            let free_size = alloc_offset - offset;
            largest = largest.max(free_size);
            offset = alloc_offset + alloc_size;
        }

        // Check free space at the end
        let free_size = self.capacity - offset;
        largest = largest.max(free_size);

        largest
    }
}

/// Arena allocator
struct ArenaAllocator {
    memory: Vec<u8>,
    offset: usize,
    capacity: usize,
}

impl ArenaAllocator {
    fn new(capacity: usize) -> Self {
        Self {
            memory: vec![0u8; capacity],
            offset: 0,
            capacity,
        }
    }

    fn allocate(&mut self, size: usize) -> Option<usize> {
        if self.offset + size > self.capacity {
            return None;
        }

        let alloc_offset = self.offset;
        self.offset += size;
        Some(alloc_offset)
    }

    fn reset(&mut self) {
        self.offset = 0;
    }
}

/// Benchmark small allocations
fn bench_small_allocations(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory/small_allocations");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));

    let sizes = [8, 16, 32, 64, 128, 256];

    for &size in &sizes {
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &size,
            |b, &size| {
                let mut allocator = SimpleAllocator::new(10 * 1024 * 1024);

                b.iter(|| {
                    let mut allocations = Vec::new();
                    for _ in 0..1000 {
                        if let Some(offset) = allocator.allocate(size) {
                            allocations.push(offset);
                        }
                    }
                    for offset in allocations {
                        allocator.deallocate(offset);
                    }
                    black_box(allocations.len())
                });
            },
        );
    }

    group.finish();
}

/// Benchmark large allocations
fn bench_large_allocations(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory/large_allocations");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));

    let sizes = [
        1024 * 1024,      // 1MB
        10 * 1024 * 1024, // 10MB
        100 * 1024 * 1024, // 100MB
    ];

    for &size in &sizes {
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(
            BenchmarkId::new("size", format!("{}MB", size / 1024 / 1024)),
            &size,
            |b, &size| {
                let mut allocator = SimpleAllocator::new(size * 2);

                b.iter(|| {
                    let mut allocations = Vec::new();
                    for _ in 0..10 {
                        if let Some(offset) = allocator.allocate(size) {
                            allocations.push(offset);
                        }
                    }
                    for offset in allocations {
                        allocator.deallocate(offset);
                    }
                    black_box(allocations.len())
                });
            },
        );
    }

    group.finish();
}

/// Benchmark pool allocation
fn bench_pool_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory/pool_allocation");

    let block_sizes = [64, 256, 1024, 4096];

    for &block_size in &block_sizes {
        group.bench_with_input(
            BenchmarkId::from_parameter(block_size),
            &block_size,
            |b, &block_size| {
                b.iter(|| {
                    let mut pool = MemoryPool::new(10 * 1024 * 1024, block_size);

                    let mut allocations = Vec::new();
                    for _ in 0..1000 {
                        if let Some(ptr) = pool.allocate() {
                            allocations.push(ptr);
                        }
                    }

                    for ptr in allocations {
                        pool.deallocate(ptr);
                    }

                    black_box(allocations.len())
                });
            },
        );
    }

    group.finish();
}

/// Benchmark arena allocator
fn bench_arena_allocator(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory/arena_allocator");

    group.bench_function("allocate", |b| {
        let mut arena = ArenaAllocator::new(10 * 1024 * 1024);

        b.iter(|| {
            let mut allocations = Vec::new();
            for _ in 0..1000 {
                if let Some(offset) = arena.allocate(1024) {
                    allocations.push(offset);
                }
            }
            arena.reset();
            black_box(allocations.len())
        });
    });

    group.finish();
}

/// Benchmark fragmentation
fn bench_fragmentation(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory/fragmentation");

    let patterns = ["sequential", "random", "mixed"];

    for &pattern in &patterns {
        group.bench_with_input(
            BenchmarkId::from_parameter(pattern),
            &pattern,
            |b, &pattern| {
                let mut allocator = SimpleAllocator::new(10 * 1024 * 1024);

                b.iter(|| {
                    let mut allocations = Vec::new();

                    match pattern {
                        "sequential" => {
                            for i in 0..1000 {
                                let size = 1024 * (1 + (i % 10));
                                if let Some(offset) = allocator.allocate(size) {
                                    allocations.push((offset, size));
                                }
                            }
                        }
                        "random" => {
                            for _ in 0..1000 {
                                let size = 1024 * (1 + (rand::random::<usize>() % 10));
                                if let Some(offset) = allocator.allocate(size) {
                                    allocations.push((offset, size));
                                }
                            }
                        }
                        "mixed" => {
                            for i in 0..1000 {
                                if i % 3 == 0 {
                                    // Allocate
                                    let size = 1024 * (1 + (i % 10));
                                    if let Some(offset) = allocator.allocate(size) {
                                        allocations.push((offset, size));
                                    }
                                } else if !allocations.is_empty() {
                                    // Deallocate
                                    let idx = rand::random::<usize>() % allocations.len();
                                    let (offset, _) = allocations.remove(idx);
                                    allocator.deallocate(offset);
                                }
                            }
                        }
                        _ => {}
                    }

                    // Free all
                    for (offset, _) in allocations {
                        allocator.deallocate(offset);
                    }

                    let fragmentation = allocator.get_fragmentation();
                    black_box(fragmentation)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark concurrent allocation
fn bench_concurrent_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory/concurrent_allocation");

    let thread_counts = [1, 2, 4, 8];

    for &thread_count in &thread_counts {
        group.bench_with_input(
            BenchmarkId::new("threads", thread_count),
            &thread_count,
            |b, &thread_count| {
                b.iter(|| {
                    let allocator = Arc::new(Mutex::new(SimpleAllocator::new(100 * 1024 * 1024)));
                    let mut handles = Vec::new();

                    for _ in 0..thread_count {
                        let alloc = allocator.clone();
                        let handle = std::thread::spawn(move || {
                            let mut allocations = Vec::new();
                            for _ in 0..1000 {
                                let mut a = alloc.lock().unwrap();
                                if let Some(offset) = a.allocate(1024) {
                                    allocations.push(offset);
                                }
                            }
                            allocations
                        });
                        handles.push(handle);
                    }

                    let mut all_allocations = Vec::new();
                    for handle in handles {
                        all_allocations.extend(handle.join().unwrap());
                    }

                    // Free all
                    let mut alloc = allocator.lock().unwrap();
                    for offset in all_allocations {
                        alloc.deallocate(offset);
                    }

                    black_box(all_allocations.len())
                });
            },
        );
    }

    group.finish();
}

/// Benchmark allocation/deallocation speed
fn bench_allocation_speed(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory/allocation_speed");

    group.bench_function("allocate_only", |b| {
        let mut allocator = SimpleAllocator::new(100 * 1024 * 1024);

        b.iter(|| {
            for _ in 0..10000 {
                let _ = allocator.allocate(1024);
            }
            // Reset for next iteration
            allocator.allocated.clear();
        });
    });

    group.bench_function("allocate_deallocate", |b| {
        let mut allocator = SimpleAllocator::new(100 * 1024 * 1024);

        b.iter(|| {
            let mut allocations = Vec::new();
            for _ in 0..10000 {
                if let Some(offset) = allocator.allocate(1024) {
                    allocations.push(offset);
                }
            }
            for offset in allocations {
                allocator.deallocate(offset);
            }
        });
    });

    group.finish();
}

/// Benchmark pool efficiency
fn bench_pool_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory/pool_efficiency");

    let pool_sizes = [1024 * 1024, 10 * 1024 * 1024, 100 * 1024 * 1024];

    for &pool_size in &pool_sizes {
        group.bench_with_input(
            BenchmarkId::new("pool_size", format!("{}MB", pool_size / 1024 / 1024)),
            &pool_size,
            |b, &pool_size| {
                b.iter(|| {
                    let mut pool = MemoryPool::new(pool_size, 4096);
                    let mut allocations = Vec::new();

                    // Allocate until 80% full
                    while pool.get_usage_ratio() < 0.8 {
                        if let Some(ptr) = pool.allocate() {
                            allocations.push(ptr);
                        } else {
                            break;
                        }
                    }

                    black_box((allocations.len(), pool.get_usage_ratio()))
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_small_allocations,
    bench_large_allocations,
    bench_pool_allocation,
    bench_arena_allocator,
    bench_fragmentation,
    bench_concurrent_allocation,
    bench_allocation_speed,
    bench_pool_efficiency
);

criterion_main!(benches);
