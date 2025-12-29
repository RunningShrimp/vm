//! TLB刷新策略高级优化基准测试
//!
//! 比较基础刷新策略与高级优化策略的性能差异

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use vm_core::{AccessType, MMU};
use vm_mem::{
    AdaptiveFlushConfig, AdvancedTlbFlushConfig, AdvancedTlbFlushManager, FlushRequest, FlushScope,
    FlushStrategy, PerCpuTlbManager, PredictiveFlushConfig, SelectiveFlushConfig, TlbFlushConfig,
    TlbFlushManager,
};

/// 基准测试：基础刷新 vs 高级刷新
fn bench_basic_vs_advanced_flush(c: &mut Criterion) {
    let mut group = c.benchmark_group("basic_vs_advanced_flush");

    // 基础TLB管理器
    let tlb_manager = Arc::new(PerCpuTlbManager::with_default_config());
    let basic_config = TlbFlushConfig {
        strategy: FlushStrategy::Immediate,
        ..Default::default()
    };
    let basic_flush_manager = TlbFlushManager::new(basic_config, tlb_manager.clone());

    // 高级TLB管理器
    let advanced_config = AdvancedTlbFlushConfig::default();
    let advanced_flush_manager = AdvancedTlbFlushManager::new(advanced_config, tlb_manager.clone());

    // 测试地址
    let test_addresses: Vec<u64> = (0..1000).map(|i| i * 4096).collect();

    // 预热阶段
    for &addr in &test_addresses {
        basic_flush_manager.record_access(addr, 0);
        advanced_flush_manager.record_access(addr, 0, AccessType::Read);
    }

    // 基础刷新基准
    group.bench_function("basic_flush", |b| {
        b.iter(|| {
            for &addr in &test_addresses {
                let request = FlushRequest::new(
                    FlushScope::SinglePage,
                    black_box(addr),
                    black_box(addr),
                    0,
                    10,
                    0,
                );
                let _ = basic_flush_manager.request_flush(request);
            }
        })
    });

    // 高级刷新基准
    group.bench_function("advanced_flush", |b| {
        b.iter(|| {
            for &addr in &test_addresses {
                let request = FlushRequest::new(
                    FlushScope::SinglePage,
                    black_box(addr),
                    black_box(addr),
                    0,
                    10,
                    0,
                );
                let _ = advanced_flush_manager.request_flush(request);
            }
        })
    });

    group.finish();
}

/// 基准测试：不同刷新策略比较
fn bench_flush_strategies(c: &mut Criterion) {
    let mut group = c.benchmark_group("flush_strategies");

    let test_addresses: Vec<u64> = (0..100).map(|i| i * 4096).collect();

    // 测试不同策略
    let strategies = vec![
        ("immediate", FlushStrategy::Immediate),
        ("delayed", FlushStrategy::Delayed),
        ("batched", FlushStrategy::Batched),
        ("intelligent", FlushStrategy::Intelligent),
        ("adaptive", FlushStrategy::Adaptive),
    ];

    for (name, strategy) in strategies {
        let tlb_manager = Arc::new(PerCpuTlbManager::with_default_config());
        let config = TlbFlushConfig {
            strategy,
            ..Default::default()
        };
        let flush_manager = TlbFlushManager::new(config, tlb_manager);

        // 预热
        for &addr in &test_addresses {
            flush_manager.record_access(addr, 0);
        }

        group.bench_with_input(BenchmarkId::new("strategy", name), &strategy, |b, _| {
            b.iter(|| {
                for &addr in &test_addresses {
                    let request = FlushRequest::new(
                        FlushScope::SinglePage,
                        black_box(addr),
                        black_box(addr),
                        0,
                        10,
                        0,
                    );
                    let _ = flush_manager.request_flush(request);
                }
            })
        });
    }

    group.finish();
}

/// 基准测试：预测性刷新效果
fn bench_predictive_flush(c: &mut Criterion) {
    let mut group = c.benchmark_group("predictive_flush");

    let test_addresses: Vec<u64> = (0..1000).map(|i| i * 4096).collect();

    // 无预测性刷新
    let tlb_manager1 = Arc::new(PerCpuTlbManager::with_default_config());
    let config1 = AdvancedTlbFlushConfig {
        predictive_config: PredictiveFlushConfig {
            enabled: false,
            ..Default::default()
        },
        ..Default::default()
    };
    let flush_manager1 = AdvancedTlbFlushManager::new(config1, tlb_manager1);

    // 有预测性刷新
    let tlb_manager2 = Arc::new(PerCpuTlbManager::with_default_config());
    let config2 = AdvancedTlbFlushConfig {
        predictive_config: PredictiveFlushConfig {
            enabled: true,
            prediction_window: 4,
            accuracy_threshold: 0.5,
            max_predictive_flushes: 2,
            history_size: 64,
        },
        ..Default::default()
    };
    let flush_manager2 = AdvancedTlbFlushManager::new(config2, tlb_manager2);

    // 预热 - 创建顺序访问模式
    for &addr in &test_addresses {
        flush_manager1.record_access(addr, 0, AccessType::Read);
        flush_manager2.record_access(addr, 0, AccessType::Read);
    }

    // 无预测性刷新基准
    group.bench_function("without_predictive", |b| {
        b.iter(|| {
            for &addr in &test_addresses.iter().step_by(10) {
                let request = FlushRequest::new(
                    FlushScope::SinglePage,
                    black_box(addr),
                    black_box(addr),
                    0,
                    10,
                    0,
                );
                let _ = flush_manager1.request_flush(request);
            }
        })
    });

    // 有预测性刷新基准
    group.bench_function("with_predictive", |b| {
        b.iter(|| {
            for &addr in &test_addresses.iter().step_by(10) {
                let request = FlushRequest::new(
                    FlushScope::SinglePage,
                    black_box(addr),
                    black_box(addr),
                    0,
                    10,
                    0,
                );
                let _ = flush_manager2.request_flush(request);
            }
        })
    });

    group.finish();
}

/// 基准测试：选择性刷新效果
fn bench_selective_flush(c: &mut Criterion) {
    let mut group = c.benchmark_group("selective_flush");

    let test_addresses: Vec<u64> = (0..1000).map(|i| i * 4096).collect();

    // 无选择性刷新
    let tlb_manager1 = Arc::new(PerCpuTlbManager::with_default_config());
    let config1 = AdvancedTlbFlushConfig {
        selective_config: SelectiveFlushConfig {
            enabled: false,
            ..Default::default()
        },
        ..Default::default()
    };
    let flush_manager1 = AdvancedTlbFlushManager::new(config1, tlb_manager1);

    // 有选择性刷新
    let tlb_manager2 = Arc::new(PerCpuTlbManager::with_default_config());
    let config2 = AdvancedTlbFlushConfig {
        selective_config: SelectiveFlushConfig {
            enabled: true,
            hot_page_threshold: 10,
            cold_page_threshold: 2,
            frequency_weight: 0.6,
            recency_weight: 0.3,
            size_weight: 0.1,
        },
        ..Default::default()
    };
    let flush_manager2 = AdvancedTlbFlushManager::new(config2, tlb_manager2);

    // 创建热点和冷页面
    let hot_addresses: Vec<u64> = (0..100).map(|i| i * 4096).collect();
    let cold_addresses: Vec<u64> = (100..1000).map(|i| i * 4096).collect();

    // 预热 - 热点页面频繁访问
    for &addr in &hot_addresses {
        for _ in 0..20 {
            flush_manager1.record_access(addr, 0, AccessType::Read);
            flush_manager2.record_access(addr, 0, AccessType::Read);
        }
    }

    // 预热 - 冷页面少量访问
    for &addr in &cold_addresses {
        for _ in 0..2 {
            flush_manager1.record_access(addr, 0, AccessType::Read);
            flush_manager2.record_access(addr, 0, AccessType::Read);
        }
    }

    // 无选择性刷新基准
    group.bench_function("without_selective", |b| {
        b.iter(|| {
            let request = FlushRequest::new(
                FlushScope::PageRange,
                black_box(0),
                black_box(1000 * 4096),
                0,
                10,
                0,
            );
            let _ = flush_manager1.request_flush(request);
        })
    });

    // 有选择性刷新基准
    group.bench_function("with_selective", |b| {
        b.iter(|| {
            let request = FlushRequest::new(
                FlushScope::PageRange,
                black_box(0),
                black_box(1000 * 4096),
                0,
                10,
                0,
            );
            let _ = flush_manager2.request_flush(request);
        })
    });

    group.finish();
}

/// 基准测试：自适应刷新效果
fn bench_adaptive_flush(c: &mut Criterion) {
    let mut group = c.benchmark_group("adaptive_flush");

    let test_addresses: Vec<u64> = (0..1000).map(|i| i * 4096).collect();

    // 无自适应刷新
    let tlb_manager1 = Arc::new(PerCpuTlbManager::with_default_config());
    let config1 = AdvancedTlbFlushConfig {
        adaptive_config: AdaptiveFlushConfig {
            enabled: false,
            ..Default::default()
        },
        ..Default::default()
    };
    let flush_manager1 = AdvancedTlbFlushManager::new(config1, tlb_manager1);

    // 有自适应刷新
    let tlb_manager2 = Arc::new(PerCpuTlbManager::with_default_config());
    let config2 = AdvancedTlbFlushConfig {
        adaptive_config: AdaptiveFlushConfig {
            enabled: true,
            monitoring_window: 50,
            performance_threshold: 0.1,
            strategy_switch_interval: 1,
            min_samples: 10,
        },
        ..Default::default()
    };
    let flush_manager2 = AdvancedTlbFlushManager::new(config2, tlb_manager2);

    // 预热
    for &addr in &test_addresses {
        flush_manager1.record_access(addr, 0, AccessType::Read);
        flush_manager2.record_access(addr, 0, AccessType::Read);
    }

    // 无自适应刷新基准
    group.bench_function("without_adaptive", |b| {
        b.iter(|| {
            for &addr in &test_addresses.iter().step_by(10) {
                let request = FlushRequest::new(
                    FlushScope::SinglePage,
                    black_box(addr),
                    black_box(addr),
                    0,
                    10,
                    0,
                );
                let _ = flush_manager1.request_flush(request);
            }
        })
    });

    // 有自适应刷新基准
    group.bench_function("with_adaptive", |b| {
        b.iter(|| {
            for &addr in &test_addresses.iter().step_by(10) {
                let request = FlushRequest::new(
                    FlushScope::SinglePage,
                    black_box(addr),
                    black_box(addr),
                    0,
                    10,
                    0,
                );
                let _ = flush_manager2.request_flush(request);
            }
        })
    });

    group.finish();
}

/// 基准测试：并发刷新性能
fn bench_concurrent_flush(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_flush");

    // 基础刷新管理器
    let tlb_manager1 = Arc::new(PerCpuTlbManager::with_default_config());
    let basic_config = TlbFlushConfig::default();
    let basic_flush_manager = Arc::new(TlbFlushManager::new(basic_config, tlb_manager1));

    // 高级刷新管理器
    let tlb_manager2 = Arc::new(PerCpuTlbManager::with_default_config());
    let advanced_config = AdvancedTlbFlushConfig::default();
    let advanced_flush_manager =
        Arc::new(AdvancedTlbFlushManager::new(advanced_config, tlb_manager2));

    // 单线程基准
    group.bench_function("basic_single_thread", |b| {
        b.iter(|| {
            for i in 0..100 {
                let addr = i * 4096;
                basic_flush_manager.record_access(addr, 0);

                let request = FlushRequest::new(
                    FlushScope::SinglePage,
                    black_box(addr),
                    black_box(addr),
                    0,
                    10,
                    0,
                );
                let _ = basic_flush_manager.request_flush(request);
            }
        })
    });

    group.bench_function("advanced_single_thread", |b| {
        b.iter(|| {
            for i in 0..100 {
                let addr = i * 4096;
                advanced_flush_manager.record_access(addr, 0, AccessType::Read);

                let request = FlushRequest::new(
                    FlushScope::SinglePage,
                    black_box(addr),
                    black_box(addr),
                    0,
                    10,
                    0,
                );
                let _ = advanced_flush_manager.request_flush(request);
            }
        })
    });

    // 多线程基准
    for thread_count in [2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::new("basic_multi_thread", thread_count),
            thread_count,
            |b, &thread_count| {
                b.iter(|| {
                    let mut handles = Vec::new();

                    for thread_id in 0..thread_count {
                        let flush_manager = basic_flush_manager.clone();

                        let handle = thread::spawn(move || {
                            for i in 0..100 {
                                let addr = thread_id * 100 * 4096 + i * 4096;
                                flush_manager.record_access(addr, thread_id as u16);

                                let request = FlushRequest::new(
                                    FlushScope::SinglePage,
                                    black_box(addr),
                                    black_box(addr),
                                    thread_id as u16,
                                    10,
                                    thread_id,
                                );
                                let _ = flush_manager.request_flush(request);
                            }
                        });

                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("advanced_multi_thread", thread_count),
            thread_count,
            |b, &thread_count| {
                b.iter(|| {
                    let mut handles = Vec::new();

                    for thread_id in 0..thread_count {
                        let flush_manager = advanced_flush_manager.clone();

                        let handle = thread::spawn(move || {
                            for i in 0..100 {
                                let addr = thread_id * 100 * 4096 + i * 4096;
                                flush_manager.record_access(
                                    addr,
                                    thread_id as u16,
                                    AccessType::Read,
                                );

                                let request = FlushRequest::new(
                                    FlushScope::SinglePage,
                                    black_box(addr),
                                    black_box(addr),
                                    thread_id as u16,
                                    10,
                                    thread_id,
                                );
                                let _ = flush_manager.request_flush(request);
                            }
                        });

                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_basic_vs_advanced_flush,
    bench_flush_strategies,
    bench_predictive_flush,
    bench_selective_flush,
    bench_adaptive_flush,
    bench_concurrent_flush
);
criterion_main!(benches);
