//! Combined Workload Benchmarks - Simplified Integration Testing
//!
//! This benchmark suite combines the already-verified optimizations to measure
//! their combined effectiveness in realistic scenarios, avoiding complex API integration.
//!
//! **Round 33**: Simplified actual workload testing
//! Strategy: Use proven APIs, combine optimizations, focus on practical value
//!
//! Run: cargo bench --bench combined_workload_bench

use std::time::Duration;
use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use vm_mem::{StackPool, PhysicalMemory, UnifiedMmuConfigV2, HybridMMU, UnifiedMMUV2, MemoryPool};

// ============================================================================
// Test Data Structures
// ============================================================================

#[derive(Debug, Clone, Default)]
struct TestData {
    #[allow(dead_code)]
    data: [u64; 8],  // 64 bytes
}

// ============================================================================
// Benchmark 1: Allocator + Memory Access
// ============================================================================

fn bench_allocator_with_memory_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("alloc_with_memory");

    for alloc_count in &[10, 100, 1000] {
        group.bench_function(BenchmarkId::new("stack_pool", alloc_count), |b| {
            let mut pool: StackPool<TestData> = StackPool::with_capacity(*alloc_count);
            let _phys_mem = PhysicalMemory::new(1024 * 1024, false);
            let mmu_config = UnifiedMmuConfigV2::default();
            let mut mmu = HybridMMU::new(1024 * 1024, mmu_config);

            b.iter(|| {
                // Phase 1: Allocate objects
                let mut items = Vec::with_capacity(*alloc_count);
                for _ in 0..*alloc_count {
                    match pool.allocate() {
                        Ok(item) => items.push(item),
                        Err(_) => break,
                    }
                }

                // Phase 2: Access memory (simulating real usage)
                for i in 0..*alloc_count {
                    let addr = vm_core::GuestAddr(0x1000 + (i as u64 % 256) * 0x100);
                    let _ = black_box(mmu.read(addr, 1));
                }

                // Phase 3: Deallocate
                for item in items {
                    pool.deallocate(item);
                }
            });
        });
    }

    group.finish();
}

// ============================================================================
// Benchmark 2: TLB + Allocator Combined
// ============================================================================

fn bench_tlb_with_allocator(c: &mut Criterion) {
    let mut group = c.benchmark_group("tlb_with_alloc");

    for lookup_count in &[100, 1000, 10000] {
        group.throughput(Throughput::Elements(*lookup_count as u64));

        group.bench_function(BenchmarkId::new("combined", lookup_count), |b| {
            let mut pool: StackPool<TestData> = StackPool::with_capacity(100);
            let _phys_mem = PhysicalMemory::new(1024 * 1024, false);
            let mmu_config = UnifiedMmuConfigV2::default();
            let mut mmu = HybridMMU::new(1024 * 1024, mmu_config);

            b.iter(|| {
                for i in 0..*lookup_count {
                    // TLB lookup
                    let addr = vm_core::GuestAddr(0x1000 + (i as u64 % 256) * 0x1000);
                    let _ = black_box(mmu.translate(addr, vm_core::AccessType::Read));

                    // Trigger allocation every 10 lookups (simulating object creation)
                    if i % 10 == 0 {
                        match pool.allocate() {
                            Ok(item) => {
                                black_box(&item);
                                pool.deallocate(item);
                            }
                            Err(_) => break,
                        }
                    }
                }
            });
        });
    }

    group.finish();
}

// ============================================================================
// Benchmark 3: Memory Copy + Allocator
// ============================================================================

fn bench_memcpy_with_allocator(c: &mut Criterion) {
    let mut group = c.benchmark_group("memcpy_with_alloc");

    for size in &[1024, 4096, 16384] {
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_function(BenchmarkId::new("combined", size), |b| {
            let mut pool: StackPool<TestData> = StackPool::with_capacity(100);

            // Prepare source and destination
            let _src = vec![0xABu8; *size];
            let mut dst = vec![0u8; *size];

            b.iter(|| {
                // Phase 1: Allocate objects
                let mut items = Vec::with_capacity(10);
                for _ in 0..10 {
                    match pool.allocate() {
                        Ok(item) => items.push(item),
                        Err(_) => break,
                    }
                }

                // Phase 2: Memory copy (simulating cache operations)
                dst.copy_from_slice(&_src);
                black_box(&dst);

                // Phase 3: Deallocate
                for item in items {
                    pool.deallocate(item);
                }
            });
        });
    }

    group.finish();
}

// ============================================================================
// Benchmark 4: Sustained Workload
// ============================================================================

fn bench_sustained_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("sustained_workload");

    for duration_ms in &[10, 50, 100] {
        group.bench_function(BenchmarkId::new("mixed_ops", duration_ms), |b| {
            let mut pool: StackPool<TestData> = StackPool::with_capacity(100);
            let _phys_mem = PhysicalMemory::new(1024 * 1024, false);
            let mmu_config = UnifiedMmuConfigV2::default();
            let mut mmu = HybridMMU::new(1024 * 1024, mmu_config);

            b.iter(|| {
                // Simulate sustained mixed workload
                let start = std::time::Instant::now();
                let mut ops = 0;

                while start.elapsed().as_millis() < *duration_ms {
                    // Mix of operations
                    for i in 0..100 {
                        // Memory access
                        let addr = vm_core::GuestAddr(0x1000 + (i % 256) * 0x100);
                        let _ = mmu.read(addr, 1);

                        // Allocation every 10 ops
                        if i % 10 == 0 {
                            match pool.allocate() {
                                Ok(item) => {
                                    black_box(&item);
                                    pool.deallocate(item);
                                }
                                Err(_) => break,
                            }
                        }

                        ops += 1;
                    }
                }

                black_box(ops);
            });
        });
    }

    group.finish();
}

// ============================================================================
// Benchmark 5: Stress Test - High Frequency Operations
// ============================================================================

fn bench_high_frequency_stress(c: &mut Criterion) {
    let mut group = c.benchmark_group("stress_test");

    group.bench_function("rapid_alloc_free_cycle", |b| {
        let mut pool: StackPool<TestData> = StackPool::with_capacity(1000);

        b.iter(|| {
            // Rapid allocation/deallocation cycles
            for _ in 0..10000 {
                match pool.allocate() {
                    Ok(item) => {
                        black_box(&item);
                        pool.deallocate(item);
                    }
                    Err(_) => break,
                }
            }
        });
    });

    group.bench_function("rapid_memory_access", |b| {
        let _phys_mem = PhysicalMemory::new(1024 * 1024, false);
        let mmu_config = UnifiedMmuConfigV2::default();
        let mut mmu = HybridMMU::new(1024 * 1024, mmu_config);

        b.iter(|| {
            // Rapid memory accesses
            for i in 0..10000 {
                let addr = vm_core::GuestAddr(0x1000 + (i as u64 % 1024) * 0x100);
                let _ = black_box(mmu.read(addr, 1));
            }
        });
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group! {
    name = combined_benches;
    config = Criterion::default()
        .measurement_time(Duration::from_secs(10))
        .sample_size(100);
    targets =
        bench_allocator_with_memory_access,
        bench_tlb_with_allocator,
        bench_memcpy_with_allocator,
        bench_sustained_workload,
        bench_high_frequency_stress,
}

criterion_main!(combined_benches);
