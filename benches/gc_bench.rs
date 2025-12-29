//! GC Performance Benchmark
//!
//! Benchmarks garbage collector performance:
//! - Pause time (ms)
//! - Throughput (allocations/second)
//! - Memory overhead (%)
//! - Collection frequency
//!
//! Test cases: High allocation rate, Large objects, Mixed workload
//!
//! Run: cargo bench --bench gc_bench

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Mock GC heap
#[derive(Debug, Clone)]
struct MockGcHeap {
    objects: Vec<MockObject>,
    heap_size: usize,
    used: usize,
    allocation_count: usize,
    collection_count: usize,
}

#[derive(Debug, Clone)]
struct MockObject {
    size: usize,
    data: Vec<u8>,
    marked: bool,
}

impl MockObject {
    fn new(size: usize) -> Self {
        Self {
            size,
            data: vec![0u8; size],
            marked: false,
        }
    }
}

impl MockGcHeap {
    fn new(heap_size: usize) -> Self {
        Self {
            objects: Vec::new(),
            heap_size,
            used: 0,
            allocation_count: 0,
            collection_count: 0,
        }
    }

    fn allocate(&mut self, size: usize) -> Option<u64> {
        if self.used + size > self.heap_size {
            self.collect();
            if self.used + size > self.heap_size {
                return None;
            }
        }

        let obj = MockObject::new(size);
        let id = self.objects.len() as u64;
        self.objects.push(obj);
        self.used += size;
        self.allocation_count += 1;
        Some(id)
    }

    fn collect(&mut self) {
        self.collection_count += 1;

        // Mark phase
        for obj in &mut self.objects {
            obj.marked = rand::random(); // Simulate reachability
        }

        // Sweep phase
        let mut i = 0;
        while i < self.objects.len() {
            if !self.objects[i].marked {
                let size = self.objects[i].size;
                self.objects.swap_remove(i);
                self.used -= size;
            } else {
                i += 1;
            }
        }

        // Reset marks
        for obj in &mut self.objects {
            obj.marked = false;
        }
    }

    fn get_usage_ratio(&self) -> f64 {
        self.used as f64 / self.heap_size as f64
    }

    fn get_memory_overhead(&self) -> f64 {
        let object_data_size: usize = self.objects.iter().map(|o| o.size).sum();
        let total_heap_size = self.used;
        if object_data_size > 0 {
            (total_heap_size - object_data_size) as f64 / object_data_size as f64 * 100.0
        } else {
            0.0
        }
    }
}

/// Generational GC heap
struct GenerationalHeap {
    young: MockGcHeap,
    old: MockGcHeap,
    young_size: usize,
    old_size: usize,
    promotion_count: usize,
}

impl GenerationalHeap {
    fn new(young_size: usize, old_size: usize) -> Self {
        Self {
            young: MockGcHeap::new(young_size),
            old: MockGcHeap::new(old_size),
            young_size,
            old_size,
            promotion_count: 0,
        }
    }

    fn allocate(&mut self, size: usize, is_old: bool) -> Option<u64> {
        if is_old {
            self.old.allocate(size)
        } else {
            self.young.allocate(size)
        }
    }

    fn collect_young(&mut self) {
        self.young.collect();

        // Promote surviving objects
        let surviving: Vec<_> = self.young.objects.drain(..).collect();
        for obj in surviving {
            if obj.marked {
                let id = self.old.objects.len() as u64;
                self.old.objects.push(obj);
                self.promotion_count += 1;
            }
        }
    }

    fn collect_full(&mut self) {
        self.collect_young();
        self.old.collect();
    }
}

/// Benchmark GC pause time
fn bench_gc_pause_time(c: &mut Criterion) {
    let mut group = c.benchmark_group("gc/pause_time");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));

    let heap_sizes = [1024 * 1024, 10 * 1024 * 1024, 100 * 1024 * 1024]; // 1MB, 10MB, 100MB

    for &heap_size in &heap_sizes {
        group.throughput(Throughput::Bytes(heap_size as u64));
        group.bench_with_input(
            BenchmarkId::new("collect", format!("{}MB", heap_size / 1024 / 1024)),
            &heap_size,
            |b, &heap_size| {
                b.iter(|| {
                    let mut heap = MockGcHeap::new(heap_size);

                    // Allocate objects
                    for i in 0..1000 {
                        let _ = heap.allocate(1024);
                        if i % 100 == 0 {
                            heap.collect();
                        }
                    }

                    let start = std::time::Instant::now();
                    heap.collect();
                    let pause_time = start.elapsed();

                    black_box(pause_time)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark allocation throughput
fn bench_allocation_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("gc/allocation_throughput");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));

    let allocation_sizes = [64, 256, 1024, 4096];

    for &size in &allocation_sizes {
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &size,
            |b, &size| {
                let mut heap = MockGcHeap::new(100 * 1024 * 1024); // 100MB heap

                b.iter(|| {
                    let start = std::time::Instant::now();
                    for _ in 0..10000 {
                        let _ = heap.allocate(size);
                    }
                    let duration = start.elapsed();
                    black_box(duration)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark high allocation rate scenario
fn bench_high_allocation_rate(c: &mut Criterion) {
    let mut group = c.benchmark_group("gc/high_allocation_rate");

    let rates = [1000, 10000, 100000]; // allocations per second

    for &rate in &rates {
        group.throughput(Throughput::Elements(rate as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(rate),
            &rate,
            |b, &rate| {
                let mut heap = MockGcHeap::new(10 * 1024 * 1024);

                b.iter(|| {
                    for _ in 0..rate {
                        let _ = heap.allocate(1024);
                        if heap.get_usage_ratio() > 0.8 {
                            heap.collect();
                        }
                    }
                    black_box(heap.allocation_count)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark large object handling
fn bench_large_objects(c: &mut Criterion) {
    let mut group = c.benchmark_group("gc/large_objects");

    let large_sizes = [1024 * 1024, 10 * 1024 * 1024, 100 * 1024 * 1024]; // 1MB, 10MB, 100MB

    for &size in &large_sizes {
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(
            BenchmarkId::new("allocate", format!("{}MB", size / 1024 / 1024)),
            &size,
            |b, &size| {
                let mut heap = MockGcHeap::new(size * 2);

                b.iter(|| {
                    let start = std::time::Instant::now();
                    let id = heap.allocate(size);
                    let duration = start.elapsed();
                    black_box((id, duration))
                });
            },
        );
    }

    group.finish();
}

/// Benchmark mixed workload
fn bench_mixed_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("gc/mixed_workload");

    group.bench_function("small_objects", |b| {
        let mut heap = MockGcHeap::new(10 * 1024 * 1024);

        b.iter(|| {
            for _ in 0..10000 {
                let _ = heap.allocate(64);
            }
            heap.collect();
            black_box(heap.allocation_count)
        });
    });

    group.bench_function("medium_objects", |b| {
        let mut heap = MockGcHeap::new(10 * 1024 * 1024);

        b.iter(|| {
            for _ in 0..1000 {
                let _ = heap.allocate(1024);
            }
            heap.collect();
            black_box(heap.allocation_count)
        });
    });

    group.bench_function("mixed_sizes", |b| {
        let mut heap = MockGcHeap::new(10 * 1024 * 1024);

        b.iter(|| {
            for i in 0..1000 {
                let size = if i % 10 == 0 { 1024 } else { 64 };
                let _ = heap.allocate(size);
            }
            heap.collect();
            black_box(heap.allocation_count)
        });
    });

    group.finish();
}

/// Benchmark generational GC
fn bench_generational_gc(c: &mut Criterion) {
    let mut group = c.benchmark_group("gc/generational");

    group.bench_function("young_gen_collection", |b| {
        let mut heap = GenerationalHeap::new(10 * 1024 * 1024, 100 * 1024 * 1024);

        b.iter(|| {
            for _ in 0..10000 {
                let _ = heap.allocate(1024, false);
            }
            heap.collect_young();
            black_box(heap.promotion_count)
        });
    });

    group.bench_function("full_collection", |b| {
        let mut heap = GenerationalHeap::new(10 * 1024 * 1024, 100 * 1024 * 1024);

        b.iter(|| {
            for _ in 0..10000 {
                let _ = heap.allocate(1024, false);
            }
            heap.collect_full();
            black_box(heap.promotion_count)
        });
    });

    group.finish();
}

/// Benchmark memory overhead
fn bench_memory_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("gc/memory_overhead");

    let heap_sizes = [1024 * 1024, 10 * 1024 * 1024, 100 * 1024 * 1024];

    for &heap_size in &heap_sizes {
        group.bench_with_input(
            BenchmarkId::new("overhead", format!("{}MB", heap_size / 1024 / 1024)),
            &heap_size,
            |b, &heap_size| {
                b.iter(|| {
                    let mut heap = MockGcHeap::new(heap_size);

                    // Allocate objects with varying sizes
                    for i in 0..1000 {
                        let size = 1024 * (1 + (i % 10));
                        let _ = heap.allocate(size);
                    }

                    heap.collect();
                    let overhead = heap.get_memory_overhead();
                    black_box(overhead)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark concurrent allocation
fn bench_concurrent_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("gc/concurrent_allocation");

    let thread_counts = [1, 2, 4, 8];

    for &thread_count in &thread_counts {
        group.bench_with_input(
            BenchmarkId::new("threads", thread_count),
            &thread_count,
            |b, &thread_count| {
                b.iter(|| {
                    let heap = Arc::new(Mutex::new(MockGcHeap::new(100 * 1024 * 1024)));
                    let mut handles = Vec::new();

                    for _ in 0..thread_count {
                        let heap_clone = heap.clone();
                        let handle = std::thread::spawn(move || {
                            for _ in 0..1000 {
                                let mut h = heap_clone.lock().unwrap();
                                let _ = h.allocate(1024);
                            }
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }

                    let h = heap.lock().unwrap();
                    black_box(h.allocation_count)
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_gc_pause_time,
    bench_allocation_throughput,
    bench_high_allocation_rate,
    bench_large_objects,
    bench_mixed_workload,
    bench_generational_gc,
    bench_memory_overhead,
    bench_concurrent_allocation
);

criterion_main!(benches);
