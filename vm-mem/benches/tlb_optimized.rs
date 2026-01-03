//! TLB优化性能基准测试
use vm_core::AddressTranslator;
//!
//! 测试多级TLB和并发TLB的性能表现

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use vm_core::{AccessType, MMU};
use vm_mem::{MmuOptimizationStrategy, SoftMmu, UnifiedMmu, UnifiedMmuConfig};

/// 基准测试：原始TLB vs 多级TLB
fn bench_multilevel_vs_original(c: &mut Criterion) {
    let mut group = c.benchmark_group("multilevel_vs_original");

    // 原始SoftMMU
    let mut original_mmu = SoftMmu::new(64 * 1024 * 1024, false);

    // 多级TLB优化的MMU
    let config = UnifiedMmuConfig {
        strategy: MmuOptimizationStrategy::MultiLevel,
        ..Default::default()
    };
    let mut multilevel_mmu = UnifiedMmu::new(64 * 1024 * 1024, false, config);

    // 预热阶段
    for i in 0..1000 {
        let addr = (i * 4096) as u64;
        let _ = original_mmu.translate(addr, AccessType::Read);
        let _ = multilevel_mmu.translate(addr, AccessType::Read);
    }

    // 基准测试
    group.bench_function("original_tlb", |b| {
        b.iter(|| {
            for i in 0..1000 {
                let addr = black_box((i * 4096) as u64);
                let _ = original_mmu.translate(addr, AccessType::Read);
            }
        })
    });

    group.bench_function("multilevel_tlb", |b| {
        b.iter(|| {
            for i in 0..1000 {
                let addr = black_box((i * 4096) as u64);
                let _ = multilevel_mmu.translate(addr, AccessType::Read);
            }
        })
    });

    group.finish();
}

/// 基准测试：并发TLB性能
fn bench_concurrent_tlb(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_tlb");

    // 并发TLB配置
    let config = UnifiedMmuConfig {
        strategy: MmuOptimizationStrategy::Concurrent,
        ..Default::default()
    };
    let concurrent_mmu = Arc::new(UnifiedMmu::new(64 * 1024 * 1024, false, config));

    // 预热
    for i in 0..1000 {
        let addr = (i * 4096) as u64;
        let _ = concurrent_mmu.translate(addr, AccessType::Read);
    }

    // 单线程基准
    group.bench_function("concurrent_single_thread", |b| {
        b.iter(|| {
            for i in 0..1000 {
                let addr = black_box((i * 4096) as u64);
                let _ = concurrent_mmu.translate(addr, AccessType::Read);
            }
        })
    });

    // 多线程基准
    for thread_count in [2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::new("concurrent_multi_thread", thread_count),
            thread_count,
            |b, &thread_count| {
                b.iter(|| {
                    let mut handles = vec![];
                    let iterations_per_thread = 1000 / thread_count;

                    for t in 0..thread_count {
                        let mmu_clone = concurrent_mmu.clone();
                        let handle = thread::spawn(move || {
                            for i in 0..iterations_per_thread {
                                let addr = ((t * iterations_per_thread + i) * 4096) as u64;
                                let _ = mmu_clone.translate(addr, AccessType::Read);
                            }
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }
                })
            },
        );
    }

    group.finish();
}

/// 基准测试：混合策略性能
fn bench_hybrid_strategy(c: &mut Criterion) {
    let mut group = c.benchmark_group("hybrid_strategy");

    // 混合策略配置
    let config = UnifiedMmuConfig {
        strategy: MmuOptimizationStrategy::Hybrid,
        multilevel_tlb_config: MultiLevelTlbConfig::default(),
        concurrent_tlb_config: ConcurrentTlbConfig::default(),
        enable_prefetch: true,
        prefetch_window: 4,
        ..Default::default()
    };
    let mut hybrid_mmu = UnifiedMmu::new(64 * 1024 * 1024, false, config);

    // 预热
    for i in 0..1000 {
        let addr = (i * 4096) as u64;
        let _ = hybrid_mmu.translate(addr, AccessType::Read);
    }

    group.bench_function("hybrid_with_prefetch", |b| {
        b.iter(|| {
            // 顺序访问模式（测试预取效果）
            for i in 0..1000 {
                let addr = black_box((i * 4096) as u64);
                let _ = hybrid_mmu.translate(addr, AccessType::Read);
            }
        })
    });

    group.bench_function("hybrid_random_access", |b| {
        b.iter(|| {
            // 随机访问模式
            for i in 0..1000 {
                let addr = black_box(((i * 17) % 1000 * 4096) as u64);
                let _ = hybrid_mmu.translate(addr, AccessType::Read);
            }
        })
    });

    group.finish();
}

/// 基准测试：TLB命中率影响
fn bench_tlb_hit_rates(c: &mut Criterion) {
    let mut group = c.benchmark_group("tlb_hit_rates");

    // 测试不同的工作集大小
    for working_set_size in [64, 256, 1024, 4096].iter() {
        let config = UnifiedMmuConfig {
            strategy: MmuOptimizationStrategy::MultiLevel,
            ..Default::default()
        };
        let mut mmu = UnifiedMmu::new(64 * 1024 * 1024, false, config);

        // 预热工作集
        for i in 0..*working_set_size {
            let addr = (i * 4096) as u64;
            let _ = mmu.translate(addr, AccessType::Read);
        }

        group.bench_with_input(
            BenchmarkId::new("hit_rate_test", working_set_size),
            working_set_size,
            |b, &working_set_size| {
                b.iter(|| {
                    // 80%热地址，20%冷地址
                    for i in 0..1000 {
                        let addr = if i < 800 {
                            // 热地址：前20%的工作集
                            black_box(((i % (working_set_size / 5)) * 4096) as u64)
                        } else {
                            // 冷地址：随机访问
                            black_box(((i * 13) % working_set_size * 4096) as u64)
                        };
                        let _ = mmu.translate(addr, AccessType::Read);
                    }
                })
            },
        );
    }

    group.finish();
}

/// 基准测试：预取效果
fn bench_prefetch_effectiveness(c: &mut Criterion) {
    let mut group = c.benchmark_group("prefetch_effectiveness");

    // 无预取配置
    let config_no_prefetch = UnifiedMmuConfig {
        strategy: MmuOptimizationStrategy::MultiLevel,
        enable_prefetch: false,
        ..Default::default()
    };
    let mut mmu_no_prefetch = UnifiedMmu::new(64 * 1024 * 1024, false, config_no_prefetch);

    // 有预取配置
    let config_with_prefetch = UnifiedMmuConfig {
        strategy: MmuOptimizationStrategy::MultiLevel,
        enable_prefetch: true,
        prefetch_window: 8,
        ..Default::default()
    };
    let mut mmu_with_prefetch = UnifiedMmu::new(64 * 1024 * 1024, false, config_with_prefetch);

    // 顺序访问模式测试
    group.bench_function("no_prefetch_sequential", |b| {
        b.iter(|| {
            for i in 0..1000 {
                let addr = black_box((i * 4096) as u64);
                let _ = mmu_no_prefetch.translate(addr, AccessType::Read);
            }
        })
    });

    group.bench_function("with_prefetch_sequential", |b| {
        b.iter(|| {
            for i in 0..1000 {
                let addr = black_box((i * 4096) as u64);
                let _ = mmu_with_prefetch.translate(addr, AccessType::Read);
            }
        })
    });

    group.finish();
}

/// 基准测试：内存访问模式
fn bench_memory_access_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_access_patterns");

    let config = UnifiedMmuConfig {
        strategy: MmuOptimizationStrategy::Hybrid,
        enable_prefetch: true,
        ..Default::default()
    };
    let mut mmu = UnifiedMmu::new(64 * 1024 * 1024, false, config);

    // 预热
    for i in 0..1000 {
        let addr = (i * 4096) as u64;
        let _ = mmu.translate(addr, AccessType::Read);
    }

    // 顺序访问
    group.bench_function("sequential_access", |b| {
        b.iter(|| {
            for i in 0..1000 {
                let addr = black_box((i * 4096) as u64);
                let _ = mmu.translate(addr, AccessType::Read);
            }
        })
    });

    // 步长访问
    group.bench_function("stride_access", |b| {
        b.iter(|| {
            for i in 0..1000 {
                let addr = black_box(((i * 17) % 1000 * 4096) as u64);
                let _ = mmu.translate(addr, AccessType::Read);
            }
        })
    });

    // 随机访问
    group.bench_function("random_access", |b| {
        b.iter(|| {
            for i in 0..1000 {
                let addr = black_box(((i * 31) % 1000 * 4096) as u64);
                let _ = mmu.translate(addr, AccessType::Read);
            }
        })
    });

    group.finish();
}

/// 基准测试：TLB容量影响
fn bench_tlb_capacity_impact(c: &mut Criterion) {
    let mut group = c.benchmark_group("tlb_capacity_impact");

    for capacity in [128, 512, 1024, 2048, 4096].iter() {
        let config = UnifiedMmuConfig {
            strategy: MmuOptimizationStrategy::MultiLevel,
            ..Default::default()
        };
        let mut mmu = UnifiedMmu::new(64 * 1024 * 1024, false, config);

        // 预热到容量的80%
        for i in 0..(*capacity * 8 / 10) {
            let addr = (i * 4096) as u64;
            let _ = mmu.translate(addr, AccessType::Read);
        }

        group.bench_with_input(
            BenchmarkId::new("capacity_test", capacity),
            capacity,
            |b, &_capacity| {
                b.iter(|| {
                    // 在工作集内访问
                    for i in 0..1000 {
                        let addr = black_box(((i % (*capacity * 8 / 10)) * 4096) as u64);
                        let _ = mmu.translate(addr, AccessType::Read);
                    }
                })
            },
        );
    }

    group.finish();
}

/// 基准测试：延迟分析
fn bench_latency_analysis(c: &mut Criterion) {
    let mut group = c.benchmark_group("latency_analysis");

    let config = UnifiedMmuConfig::default();
    let mut mmu = UnifiedMmu::new(64 * 1024 * 1024, false, config);

    // 预热
    for i in 0..100 {
        let addr = (i * 4096) as u64;
        let _ = mmu.translate(addr, AccessType::Read);
    }

    // 测量单次翻译延迟
    group.bench_function("single_translation_latency", |b| {
        b.iter(|| {
            let addr = black_box(0x1000);
            let _ = mmu.translate(addr, AccessType::Read);
        })
    });

    // 测量批量翻译延迟
    group.bench_function("batch_translation_latency", |b| {
        b.iter(|| {
            for i in 0..100 {
                let addr = black_box((i * 4096) as u64);
                let _ = mmu.translate(addr, AccessType::Read);
            }
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_multilevel_vs_original,
    bench_concurrent_tlb,
    bench_hybrid_strategy,
    bench_tlb_hit_rates,
    bench_prefetch_effectiveness,
    bench_memory_access_patterns,
    bench_tlb_capacity_impact,
    bench_latency_analysis
);
criterion_main!(benches);
