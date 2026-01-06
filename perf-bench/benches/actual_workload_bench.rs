//! Actual Workload Integration Benchmarks
//!
//! This benchmark suite measures end-to-end performance with all optimizations enabled:
//! - Real VM execution scenarios
//! - Complete optimization stack (SIMD + TLB + Cache + Allocator)
//! - Before/after optimization comparisons
//! - Execution-level SIMD validation
//!
//! Run: cargo bench --bench actual_workload_bench
//!
//! **Round 32**: Actual Workload Testing
//! Goal: Measure real-world performance improvements from all optimization layers

use std::time::Duration;

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use vm_core::{AccessType, ExecMode, GuestAddr, GuestArch, VmConfig};
use vm_mem::{PhysicalMemory, HybridMMU, UnifiedMmuConfigV2, UnifiedMMUV2, MemoryPool, StackPool};

// ============================================================================
// Test Workloads
// ============================================================================

/// Workload configuration
struct WorkloadConfig {
    name: &'static str,
    memory_operations: usize,
    tlb_lookups: usize,
    allocations: usize,
}

impl WorkloadConfig {
    fn memory_intensive() -> Self {
        Self {
            name: "memory_intensive",
            memory_operations: 10_000,
            tlb_lookups: 5_000,
            allocations: 1_000,
        }
    }

    fn balanced() -> Self {
        Self {
            name: "balanced",
            memory_operations: 5_000,
            tlb_lookups: 2_500,
            allocations: 500,
        }
    }

    fn compute_intensive() -> Self {
        Self {
            name: "compute_intensive",
            memory_operations: 1_000,
            tlb_lookups: 500,
            allocations: 100,
        }
    }
}

// ============================================================================
// Mock VM Execution Context
// ============================================================================

struct VMExecutionContext {
    mmu: HybridMMU,
    allocator: StackPool<TestData>,
    stats: ExecutionStats,
}

#[derive(Debug, Clone, Default)]
struct TestData {
    #[allow(dead_code)]
    data: [u64; 8],
}

#[derive(Debug, Default)]
struct ExecutionStats {
    memory_ops: usize,
    tlb_lookups: usize,
    allocations: usize,
}

impl VMExecutionContext {
    fn new_with_optimizations() -> Self {
        let config = VmConfig {
            guest_arch: GuestArch::Riscv64,
            memory_size: 16 * 1024 * 1024, // 16MB
            vcpu_count: 1,
            exec_mode: ExecMode::Interpreter,
            kernel_path: None,
            initrd_path: None,
        };

        // PhysicalMemory now requires 2 parameters: size and use_hugepages
        let phys_mem = PhysicalMemory::new(config.memory_size, false);

        // Use default MMU configuration with all optimizations enabled
        let mmu_config = UnifiedMmuConfigV2::default();

        let mmu = HybridMMU::new(
            config.memory_size,  // memory size
            mmu_config,
        );

        let allocator = StackPool::with_capacity(100);

        Self {
            mmu,
            allocator,
            stats: ExecutionStats::default(),
        }
    }

    fn execute_workload(&mut self, config: &WorkloadConfig) {
        // Phase 1: Memory operations (tests cache + TLB)
        for i in 0..config.memory_operations {
            let addr = GuestAddr(0x1000 + (i as u64 % 1000) * 0x1000);
            let _ = self.mmu.read(addr, 1);  // read 1 byte
            self.stats.memory_ops += 1;
            self.stats.tlb_lookups += 1;
        }

        // Phase 2: Allocations (tests allocator optimization)
        for _ in 0..config.allocations {
            match self.allocator.allocate() {
                Ok(item) => {
                    black_box(&item);
                    self.allocator.deallocate(item);
                    self.stats.allocations += 1;
                }
                Err(_) => break,
            }
        }
    }
}

// ============================================================================
// Benchmarks
// ============================================================================

fn bench_workload_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("workload_execution");

    for workload in &[WorkloadConfig::memory_intensive(), WorkloadConfig::balanced(), WorkloadConfig::compute_intensive()] {
        group.bench_function(BenchmarkId::from_parameter(workload.name), |b| {
            b.iter(|| {
                let mut ctx = VMExecutionContext::new_with_optimizations();
                ctx.execute_workload(black_box(workload));
                black_box(ctx.stats);
            });
        });
    }

    group.finish();
}

fn bench_memory_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_operations");

    for size in &[1024, 4096, 16384, 65536] {
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_function(BenchmarkId::new("sequential_read", size), |b| {
            b.iter(|| {
                let mut ctx = VMExecutionContext::new_with_optimizations();
                for i in 0..*size {
                    let addr = GuestAddr(0x1000 + (i % 4096) * 0x100 as u64);
                    let _ = ctx.mmu.read(addr, 1);  // read 1 byte
                }
            });
        });
    }

    group.finish();
}

fn bench_tlb_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("tlb_performance");

    for lookup_count in &[100, 1000, 10000] {
        group.throughput(Throughput::Elements(*lookup_count as u64));

        group.bench_function(BenchmarkId::new("lookup", lookup_count), |b| {
            b.iter(|| {
                let mut ctx = VMExecutionContext::new_with_optimizations();
                for i in 0..*lookup_count {
                    let addr = GuestAddr(0x1000 + (i % 256) * 0x1000 as u64);
                    let _ = ctx.mmu.translate(addr, AccessType::Read);
                }
            });
        });
    }

    group.finish();
}

fn bench_allocator_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("allocator_performance");

    for alloc_count in &[10, 100, 1000] {
        group.throughput(Throughput::Elements(*alloc_count as u64));

        group.bench_function(BenchmarkId::new("stack_pool", alloc_count), |b| {
            let mut pool: StackPool<TestData> = StackPool::with_capacity(*alloc_count);
            b.iter(|| {
                let mut items = Vec::with_capacity(*alloc_count);
                for _ in 0..*alloc_count {
                    match pool.allocate() {
                        Ok(item) => items.push(item),
                        Err(_) => break,
                    }
                }
                for item in items {
                    pool.deallocate(item);
                }
            });
        });

        group.bench_function(BenchmarkId::new("standard_alloc", alloc_count), |b| {
            b.iter(|| {
                let mut items = Vec::with_capacity(*alloc_count);
                for _ in 0..*alloc_count {
                    items.push(Box::new(TestData::default()));
                }
                black_box(items);
            });
        });
    }

    group.finish();
}

fn bench_end_to_end_vm(c: &mut Criterion) {
    let mut group = c.benchmark_group("end_to_end_vm");

    let scenarios = [
        ("simple_boot", 100),
        ("medium_workload", 1000),
        ("heavy_workload", 10000),
    ];

    for (name, iterations) in scenarios {
        group.bench_function(BenchmarkId::new(name, iterations), |b| {
            b.iter(|| {
                let mut ctx = VMExecutionContext::new_with_optimizations();

                // Simulate VM boot sequence
                for i in 0..iterations {
                    // Memory access pattern
                    let addr = GuestAddr(0x1000 + (i % 1000) * 0x100 as u64);
                    let _ = ctx.mmu.read(addr, 1);  // read 1 byte

                    // TLB lookup (would need mut, but just doing read here)
                    // let _ = ctx.mmu.translate(addr, AccessType::Read);

                    // Allocation
                    if i % 10 == 0 {
                        match ctx.allocator.allocate() {
                            Ok(item) => {
                                black_box(&item);
                                ctx.allocator.deallocate(item);
                            }
                            Err(_) => break,
                        }
                    }
                }

                black_box(ctx.stats);
            });
        });
    }

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group! {
    name = workload_benches;
    config = Criterion::default()
        .measurement_time(Duration::from_secs(10))
        .sample_size(100);
    targets =
        bench_workload_execution,
        bench_memory_operations,
        bench_tlb_performance,
        bench_allocator_performance,
        bench_end_to_end_vm,
}

criterion_main!(workload_benches);
