//! Large-Scale TLB Optimization Benchmark
//!
//! This benchmark compares the performance of different TLB implementations
//! at various scales to measure the improvement from the optimized hash-based TLB.

use std::time::{Duration, Instant};

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use vm_core::{AccessType, GuestAddr};
use vm_mem::mmu::{PageTableFlags, PageWalkResult};
use vm_mem::tlb::core::{
    basic::SoftwareTlb, optimized_hash::OptimizedHashTlb, unified::MultiLevelTlb,
    MultiLevelTlbConfig, TlbReplacePolicy,
};

/// Benchmark: TLB lookup performance at different scales
fn bench_tlb_scale_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("tlb_scale_performance");

    // Test different page counts
    for page_count in [1, 10, 64, 128, 256].iter() {
        // ============ Basic TLB ============
        let mut basic_tlb = SoftwareTlb::new(*page_count, TlbReplacePolicy::Lru);

        // Pre-fill TLB
        for i in 0..*page_count {
            let gva = GuestAddr((i * 4096) as u64);
            let gpa = GuestAddr(((i + 0x1000) * 4096) as u64);
            basic_tlb.insert(
                PageWalkResult {
                    gpa,
                    page_size: 4096,
                    flags: PageTableFlags::from_bits_truncate(0x7),
                },
                gva,
                0,
            );
        }

        group.bench_with_input(
            BenchmarkId::new("basic_tlb", page_count),
            page_count,
            |b, page_count| {
                b.iter(|| {
                    // Measure lookup time for all entries
                    let start = Instant::now();
                    for i in 0..*page_count {
                        let gva = GuestAddr((i * 4096) as u64);
                        black_box(basic_tlb.lookup(gva, 0));
                    }
                    start.elapsed()
                })
            },
        );

        // ============ Optimized Hash TLB ============
        let hash_capacity = usize::next_power_of_two(*page_count);
        let mut hash_tlb = OptimizedHashTlb::new(hash_capacity);

        // Pre-fill TLB
        for i in 0..*page_count {
            let gva = GuestAddr((i * 4096) as u64);
            let gpa = GuestAddr(((i + 0x1000) * 4096) as u64);
            hash_tlb.insert(gva, gpa, 0x7, 0);
        }

        group.bench_with_input(
            BenchmarkId::new("optimized_hash_tlb", page_count),
            page_count,
            |b, page_count| {
                b.iter(|| {
                    // Measure lookup time for all entries
                    let start = Instant::now();
                    for i in 0..*page_count {
                        let gva = GuestAddr((i * 4096) as u64);
                        black_box(hash_tlb.translate(gva, 0, AccessType::Read));
                    }
                    start.elapsed()
                })
            },
        );

        // ============ Multi-Level TLB ============
        let config = MultiLevelTlbConfig {
            l1_capacity: (*page_count).min(64),
            l2_capacity: (*page_count).min(256),
            l3_capacity: *page_count,
            ..Default::default()
        };
        let mut multilevel_tlb = MultiLevelTlb::new(config);

        // Pre-fill TLB
        for i in 0..*page_count {
            let vpn = (i * 4096) >> 12;
            let ppn = ((i + 0x1000) * 4096) >> 12;
            multilevel_tlb.insert(vpn as u64, ppn as u64, 0x7, 0);
        }

        group.bench_with_input(
            BenchmarkId::new("multilevel_tlb", page_count),
            page_count,
            |b, page_count| {
                b.iter(|| {
                    // Measure lookup time for all entries
                    let start = Instant::now();
                    for i in 0..*page_count {
                        let gva = GuestAddr((i * 4096) as u64);
                        let vpn = gva.0 >> 12;
                        black_box(multilevel_tlb.translate(vpn, 0, AccessType::Read));
                    }
                    start.elapsed()
                })
            },
        );
    }

    group.finish();
}

/// Benchmark: Average lookup latency per entry
fn bench_tlb_per_entry_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("tlb_per_entry_latency");

    for page_count in [1, 10, 64, 128, 256].iter() {
        let iterations = 1000;

        // ============ Basic TLB ============
        let mut basic_tlb = SoftwareTlb::new(*page_count, TlbReplacePolicy::Lru);

        for i in 0..*page_count {
            let gva = GuestAddr((i * 4096) as u64);
            let gpa = GuestAddr(((i + 0x1000) * 4096) as u64);
            basic_tlb.insert(
                PageWalkResult {
                    gpa,
                    page_size: 4096,
                    flags: PageTableFlags::from_bits_truncate(0x7),
                },
                gva,
                0,
            );
        }

        group.bench_with_input(
            BenchmarkId::new("basic_tlb", page_count),
            page_count,
            |b, page_count| {
                b.iter(|| {
                    let start = Instant::now();
                    for _ in 0..iterations {
                        for i in 0..*page_count {
                            let gva = GuestAddr((i * 4096) as u64);
                            black_box(basic_tlb.lookup(gva, 0));
                        }
                    }
                    let total_ns = start.elapsed().as_nanos() as f64;
                    total_ns / (iterations * *page_count) as f64
                })
            },
        );

        // ============ Optimized Hash TLB ============
        let hash_capacity = usize::next_power_of_two(*page_count);
        let mut hash_tlb = OptimizedHashTlb::new(hash_capacity);

        for i in 0..*page_count {
            let gva = GuestAddr((i * 4096) as u64);
            let gpa = GuestAddr(((i + 0x1000) * 4096) as u64);
            hash_tlb.insert(gva, gpa, 0x7, 0);
        }

        group.bench_with_input(
            BenchmarkId::new("optimized_hash_tlb", page_count),
            page_count,
            |b, page_count| {
                b.iter(|| {
                    let start = Instant::now();
                    for _ in 0..iterations {
                        for i in 0..*page_count {
                            let gva = GuestAddr((i * 4096) as u64);
                            black_box(hash_tlb.translate(gva, 0, AccessType::Read));
                        }
                    }
                    let total_ns = start.elapsed().as_nanos() as f64;
                    total_ns / (iterations * *page_count) as f64
                })
            },
        );
    }

    group.finish();
}

/// Benchmark: Mixed access pattern (sequential + random)
fn bench_tlb_mixed_access_pattern(c: &mut Criterion) {
    let mut group = c.benchmark_group("tlb_mixed_access");

    for page_count in [64, 128, 256].iter() {
        let hash_capacity = usize::next_power_of_two(*page_count);

        // ============ Sequential Access ============
        let mut hash_tlb = OptimizedHashTlb::new(hash_capacity);
        for i in 0..*page_count {
            let gva = GuestAddr((i * 4096) as u64);
            let gpa = GuestAddr(((i + 0x1000) * 4096) as u64);
            hash_tlb.insert(gva, gpa, 0x7, 0);
        }

        group.bench_with_input(
            BenchmarkId::new("sequential_access", page_count),
            page_count,
            |b, page_count| {
                b.iter(|| {
                    for i in 0..*page_count {
                        let gva = GuestAddr((i * 4096) as u64);
                        black_box(hash_tlb.translate(gva, 0, AccessType::Read));
                    }
                })
            },
        );

        // ============ Random Access ============
        group.bench_with_input(
            BenchmarkId::new("random_access", page_count),
            page_count,
            |b, page_count| {
                b.iter(|| {
                    for i in 0..*page_count {
                        // Use pseudo-random pattern
                        let idx = (i * 17) % *page_count;
                        let gva = GuestAddr((idx * 4096) as u64);
                        black_box(hash_tlb.translate(gva, 0, AccessType::Read));
                    }
                })
            },
        );

        // ============ Strided Access ============
        group.bench_with_input(
            BenchmarkId::new("strided_access", page_count),
            page_count,
            |b, page_count| {
                b.iter(|| {
                    for i in 0..*page_count {
                        // Access every 4th page
                        let idx = (i * 4) % *page_count;
                        let gva = GuestAddr((idx * 4096) as u64);
                        black_box(hash_tlb.translate(gva, 0, AccessType::Read));
                    }
                })
            },
        );
    }

    group.finish();
}

/// Benchmark: Cache locality impact
fn bench_tlb_cache_locality(c: &mut Criterion) {
    let mut group = c.benchmark_group("tlb_cache_locality");

    for page_count in [64, 256].iter() {
        let hash_capacity = usize::next_power_of_two(*page_count);

        // ============ Hot set (20% of pages, 80% of accesses) ============
        let mut hash_tlb = OptimizedHashTlb::new(hash_capacity);
        for i in 0..*page_count {
            let gva = GuestAddr((i * 4096) as u64);
            let gpa = GuestAddr(((i + 0x1000) * 4096) as u64);
            hash_tlb.insert(gva, gpa, 0x7, 0);
        }

        group.bench_with_input(
            BenchmarkId::new("hot_set_access", page_count),
            page_count,
            |b, page_count| {
                b.iter(|| {
                    // 80% hot, 20% cold
                    for i in 0..1000 {
                        let gva = if i < 800 {
                            let hot_idx = i % (*page_count / 5);
                            GuestAddr((hot_idx * 4096) as u64)
                        } else {
                            let cold_idx = (i * 13) % *page_count;
                            GuestAddr((cold_idx * 4096) as u64)
                        };
                        black_box(hash_tlb.translate(gva, 0, AccessType::Read));
                    }
                })
            },
        );

        // ============ Working set expansion ============
        group.bench_with_input(
            BenchmarkId::new("working_set_expansion", page_count),
            page_count,
            |b, page_count| {
                b.iter(|| {
                    // Gradually expand working set
                    for phase in 0..10 {
                        let set_size = (*page_count / 10) * (phase + 1);
                        for i in 0..set_size {
                            let gva = GuestAddr((i * 4096) as u64);
                            black_box(hash_tlb.translate(gva, 0, AccessType::Read));
                        }
                    }
                })
            },
        );
    }

    group.finish();
}

/// Manual performance measurement for detailed reporting
#[allow(dead_code)]
fn measure_detailed_performance() {
    println!("\n=== Detailed TLB Performance Measurement ===\n");

    for page_count in [1, 10, 64, 128, 256].iter() {
        println!("--- Scale: {} pages ---", page_count);

        // Measure Basic TLB
        let mut basic_tlb = SoftwareTlb::new(*page_count, TlbReplacePolicy::Lru);
        for i in 0..*page_count {
            let gva = GuestAddr((i * 4096) as u64);
            let gpa = GuestAddr(((i + 0x1000) * 4096) as u64);
            basic_tlb.insert(
                PageWalkResult {
                    gpa,
                    page_size: 4096,
                    flags: PageTableFlags::from_bits_truncate(0x7),
                },
                gva,
                0,
            );
        }

        let iterations = 10_000;
        let start = Instant::now();
        for _ in 0..iterations {
            for i in 0..*page_count {
                let gva = GuestAddr((i * 4096) as u64);
                black_box(basic_tlb.lookup(gva, 0));
            }
        }
        let basic_duration = start.elapsed();
        let basic_avg_ns = basic_duration.as_nanos() as f64 / (iterations * *page_count) as f64;

        // Measure Optimized Hash TLB
        let hash_capacity = usize::next_power_of_two(*page_count);
        let mut hash_tlb = OptimizedHashTlb::new(hash_capacity);
        for i in 0..*page_count {
            let gva = GuestAddr((i * 4096) as u64);
            let gpa = GuestAddr(((i + 0x1000) * 4096) as u64);
            hash_tlb.insert(gva, gpa, 0x7, 0);
        }

        let start = Instant::now();
        for _ in 0..iterations {
            for i in 0..*page_count {
                let gva = GuestAddr((i * 4096) as u64);
                black_box(hash_tlb.translate(gva, 0, AccessType::Read));
            }
        }
        let hash_duration = start.elapsed();
        let hash_avg_ns = hash_duration.as_nanos() as f64 / (iterations * *page_count) as f64;

        let speedup = basic_avg_ns / hash_avg_ns;
        let improvement = ((basic_avg_ns - hash_avg_ns) / basic_avg_ns) * 100.0;

        println!("Basic TLB:         {:.2} ns/lookup", basic_avg_ns);
        println!("Optimized Hash TLB: {:.2} ns/lookup", hash_avg_ns);
        println!("Speedup:           {:.2}x", speedup);
        println!("Improvement:       {:.1}%", improvement);
        println!();
    }
}

criterion_group!(
    benches,
    bench_tlb_scale_performance,
    bench_tlb_per_entry_latency,
    bench_tlb_mixed_access_pattern,
    bench_tlb_cache_locality
);
criterion_main!(benches);
