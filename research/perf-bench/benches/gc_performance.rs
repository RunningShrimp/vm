//! Garbage Collection Performance Benchmarks
//!
//! Comprehensive benchmarks for GC performance:
//! - Minor GC pause time
//! - Major GC pause time
//! - Throughput (bytes reclaimed/ms)
//! - Allocation rate
//!
//! Run: cargo bench --bench gc_performance

use std::time::Duration;

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};

// Mock GC implementation
struct MockGarbageCollector {
    young_generation: Vec<Vec<u8>>,
    old_generation: Vec<Vec<u8>>,
    young_gen_size: usize,
    old_gen_size: usize,
    allocation_count: u64,
    collection_count: u64,
}

impl MockGarbageCollector {
    fn new(young_size: usize, old_size: usize) -> Self {
        Self {
            young_generation: Vec::new(),
            old_generation: Vec::new(),
            young_gen_size: young_size,
            old_gen_size: old_size,
            allocation_count: 0,
            collection_count: 0,
        }
    }

    /// Allocate object in young generation
    fn allocate(&mut self, size: usize) -> bool {
        self.allocation_count += 1;

        // Calculate current young gen usage
        let current_usage: usize = self.young_generation.iter().map(|obj| obj.len()).sum();

        if current_usage + size > self.young_gen_size {
            // Trigger minor GC
            self.minor_gc();
        }

        // Allocate object
        self.young_generation.push(vec![0u8; size]);
        true
    }

    /// Minor GC: Collect young generation
    fn minor_gc(&mut self) {
        self.collection_count += 1;

        // Simulate survivor promotion
        let survivor_ratio = 0.1; // 10% survive to old generation
        let survivors = (self.young_generation.len() as f64 * survivor_ratio) as usize;

        // Promote survivors to old generation
        for obj in self.young_generation.drain(..survivors) {
            let current_old_usage: usize = self.old_generation.iter().map(|o| o.len()).sum();

            // Check if old gen needs collection
            if current_old_usage + obj.len() > self.old_gen_size {
                self.major_gc();
            }

            self.old_generation.push(obj);
        }

        // Clear remaining young gen (simulated collection)
        self.young_generation.clear();
    }

    /// Major GC: Collect old generation
    fn major_gc(&mut self) {
        self.collection_count += 1;

        // Simulate major GC work - more expensive than minor GC
        let retain_ratio = 0.8; // 80% survive in old gen
        let retain_count = (self.old_generation.len() as f64 * retain_ratio) as usize;

        // Keep only "live" objects
        self.old_generation.truncate(retain_count);

        // Simulate compaction work
        let total_size: usize = self.old_generation.iter().map(|obj| obj.len()).sum();
        black_box(total_size);
    }

    /// Get statistics
    fn get_stats(&self) -> GCStats {
        let young_usage: usize = self.young_generation.iter().map(|obj| obj.len()).sum();
        let old_usage: usize = self.old_generation.iter().map(|obj| obj.len()).sum();

        GCStats {
            young_gen_usage: young_usage,
            old_gen_usage: old_usage,
            allocation_count: self.allocation_count,
            collection_count: self.collection_count,
        }
    }
}

#[derive(Debug, Clone)]
struct GCStats {
    young_gen_usage: usize,
    old_gen_usage: usize,
    allocation_count: u64,
    collection_count: u64,
}

/// Benchmark: Minor GC pause time
fn bench_minor_gc_pause(c: &mut Criterion) {
    let mut group = c.benchmark_group("gc_minor_pause");
    group.measurement_time(Duration::from_secs(10));

    for heap_size in [1024, 4096, 16384, 65536].iter() {
        group.bench_with_input(
            BenchmarkId::new("heap_size", heap_size),
            heap_size,
            |b, &heap_size| {
                b.iter(|| {
                    let mut gc = MockGarbageCollector::new(heap_size, heap_size * 10);

                    // Fill young generation
                    for _ in 0..100 {
                        gc.allocate(1024);
                    }

                    // Measure minor GC
                    black_box(gc.minor_gc());
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Major GC pause time
fn bench_major_gc_pause(c: &mut Criterion) {
    let mut group = c.benchmark_group("gc_major_pause");
    group.measurement_time(Duration::from_secs(10));

    for heap_size in [1024, 4096, 16384, 65536].iter() {
        group.bench_with_input(
            BenchmarkId::new("heap_size", heap_size),
            heap_size,
            |b, &heap_size| {
                b.iter(|| {
                    let mut gc = MockGarbageCollector::new(heap_size, heap_size * 10);

                    // Fill old generation
                    for _ in 0..1000 {
                        gc.allocate(1024);
                        gc.minor_gc(); // Promote objects
                    }

                    // Measure major GC
                    black_box(gc.major_gc());
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Allocation throughput
fn bench_allocation_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("gc_allocation_throughput");
    group.measurement_time(Duration::from_secs(10));

    for object_size in [64, 256, 1024, 4096].iter() {
        group.throughput(Throughput::Bytes(*object_size as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(object_size),
            object_size,
            |b, &object_size| {
                b.iter(|| {
                    let mut gc = MockGarbageCollector::new(1024 * 1024, 10 * 1024 * 1024);

                    for _ in 0..1000 {
                        black_box(gc.allocate(object_size));
                    }

                    gc.get_stats()
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: GC throughput (bytes reclaimed per ms)
fn bench_gc_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("gc_throughput");
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("bytes_reclaimed_per_ms", |b| {
        b.iter(|| {
            let mut gc = MockGarbageCollector::new(1024 * 1024, 10 * 1024 * 1024);

            // Allocate and fill memory
            let allocated_before = gc.get_stats().young_gen_usage;
            for _ in 0..1000 {
                gc.allocate(1024);
            }
            let allocated_after = gc.get_stats().young_gen_usage;

            // Perform GC
            gc.minor_gc();

            let allocated_after_gc = gc.get_stats().young_gen_usage;
            let reclaimed = allocated_after - allocated_after_gc;

            black_box(reclaimed);
        });
    });

    group.finish();
}

/// Benchmark: Generational collection efficiency
fn bench_generational_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("gc_generational");
    group.measurement_time(Duration::from_secs(10));

    // Minor-only (mostly short-lived objects)
    group.bench_function("minor_only", |b| {
        b.iter(|| {
            let mut gc = MockGarbageCollector::new(1024 * 1024, 10 * 1024 * 1024);

            for i in 0..10000 {
                gc.allocate(1024);

                // Most objects die quickly
                if i % 100 == 0 {
                    gc.minor_gc();
                }
            }

            black_box(gc.get_stats());
        });
    });

    // Mixed major/minor (some long-lived objects)
    group.bench_function("mixed_generational", |b| {
        b.iter(|| {
            let mut gc = MockGarbageCollector::new(1024 * 1024, 10 * 1024 * 1024);

            for i in 0..10000 {
                gc.allocate(1024);

                if i % 100 == 0 {
                    gc.minor_gc();
                }

                // Some objects survive to old gen
                if i % 1000 == 0 {
                    // Allocate long-lived objects
                    for _ in 0..10 {
                        gc.allocate(4096);
                    }
                }
            }

            black_box(gc.get_stats());
        });
    });

    group.finish();
}

/// Benchmark: Collection frequency
fn bench_collection_frequency(c: &mut Criterion) {
    let mut group = c.benchmark_group("gc_frequency");
    group.measurement_time(Duration::from_secs(10));

    for allocation_rate in [100, 500, 1000, 5000].iter() {
        group.bench_with_input(
            BenchmarkId::new("allocations_per_gc", allocation_rate),
            allocation_rate,
            |b, &allocation_rate| {
                b.iter(|| {
                    let mut gc = MockGarbageCollector::new(1024 * 1024, 10 * 1024 * 1024);

                    for i in 0..allocation_rate * 10 {
                        gc.allocate(1024);

                        if i % allocation_rate == 0 && i != 0 {
                            gc.minor_gc();
                        }
                    }

                    black_box(gc.get_stats());
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Live data ratio impact
fn bench_live_data_ratio(c: &mut Criterion) {
    let mut group = c.benchmark_group("gc_live_data_ratio");
    group.measurement_time(Duration::from_secs(10));

    for live_ratio in [10, 30, 50, 70, 90].iter() {
        group.bench_with_input(
            BenchmarkId::new("live_ratio_percent", live_ratio),
            live_ratio,
            |b, &live_ratio| {
                b.iter(|| {
                    let mut gc = MockGarbageCollector::new(1024 * 1024, 10 * 1024 * 1024);

                    // Allocate objects
                    for i in 0..1000 {
                        gc.allocate(1024);
                    }

                    // Simulate liveness by marking certain objects as live
                    let live_count = (1000 * live_ratio / 100) as usize;
                    gc.young_generation.truncate(live_count);

                    // Measure GC time
                    black_box(gc.minor_gc());
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Heap size impact
fn bench_heap_size_impact(c: &mut Criterion) {
    let mut group = c.benchmark_group("gc_heap_size");
    group.measurement_time(Duration::from_secs(10));

    for heap_mb in [1, 4, 16, 64, 256].iter() {
        group.bench_with_input(
            BenchmarkId::new("heap_size_mb", heap_mb),
            heap_mb,
            |b, &heap_mb| {
                let heap_size = heap_mb * 1024 * 1024;

                b.iter(|| {
                    let mut gc = MockGarbageCollector::new(heap_size, heap_size * 10);

                    // Fill 50% of heap
                    let alloc_count = (heap_size / 2) / 1024;
                    for _ in 0..alloc_count {
                        gc.allocate(1024);
                    }

                    // Measure GC time
                    black_box(gc.minor_gc());
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_minor_gc_pause,
    bench_major_gc_pause,
    bench_allocation_throughput,
    bench_gc_throughput,
    bench_generational_efficiency,
    bench_collection_frequency,
    bench_live_data_ratio,
    bench_heap_size_impact
);
criterion_main!(benches);
