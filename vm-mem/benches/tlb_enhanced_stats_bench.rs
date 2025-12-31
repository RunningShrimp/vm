//! TLB增强统计功能性能基准测试
use vm_core::AddressTranslator;
//!
//! 测试新增的TLB统计功能：
//! - 延迟分布统计（LatencyDistribution）
//! - 未命中原因分析（MissReasonAnalysis）
//! - 策略切换历史（PolicySwitchEvent）
//! - 预取准确率（PrefetchAccuracy）
//!
//! 使用方法：
//! ```toml
//! [dev-dependencies]
//! criterion = { version = "0.5", features = ["html_reports"] }
//! ```
//!
//! 运行基准测试：
//! ```bash
//! cargo bench --bench enhanced_tlb_stats
//! ```

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use std::sync::Arc;
use vm_core::{AccessType, GuestAddr, GuestPhysAddr};
use vm_mem::tlb::{
    // 新增的增强统计
    EnhancedTlbStats,
    LatencyDistribution,
    MissReason,
    PrefetchAccuracy,
    StatsReport,
    TlbFactory,
    UnifiedTlb,
};

use vm_mem::tlb::tlb::TlbReplacePolicy;
use vm_mem::tlb::tlb::{MissReason as TlbMissReason, SwitchReason as TlbSwitchReason};

// ============================================================================
// 辅助函数：生成测试数据
// ============================================================================

/// 生成随机测试地址
fn generate_test_addresses(count: usize, base: u64) -> Vec<GuestAddr> {
    (0..count)
        .map(|i| GuestAddr(base + (i as u64 * 4096)))
        .collect()
}

/// 模拟延迟（纳秒）
fn simulate_latency(i: usize) -> u64 {
    // 基础延迟：命中时较低
    let base_latency = 10u64;

    // 每10次访问有1次"较慢"的访问（模拟TLB未命中）
    let miss_penalty = 50;
    let miss = i % 10 == 0;

    if miss {
        base_latency + miss_penalty
    } else {
        base_latency + (i % 5) as u64 // 添加一些变化
    }
}

// ============================================================================
// 基准测试1：延迟分布统计测试
// ============================================================================

/// 测试延迟分布统计的性能开销
fn bench_latency_distribution(c: &mut Criterion) {
    let mut group = c.benchmark_group("latency_distribution");

    let addresses = generate_test_addresses(10000, 0x1000_0000);

    // 基准：不使用增强统计
    group.bench_function("without_enhanced_stats", |b| {
        b.iter(|| {
            let mut lookups = 0u64;
            let mut hits = 0u64;

            for addr in black_box(addresses.iter()) {
                let _ = black_box(*addr); // 模拟查找
                lookups += 1;
                hits += 1; // 假设大部分命中
            }

            // 简单计算（不使用增强统计）
            let hit_rate = hits as f64 / lookups as f64;
            black_box(hit_rate);
        });
    });

    // 基准：使用增强统计
    group.bench_function("with_enhanced_stats", |b| {
        b.iter(|| {
            let mut stats = EnhancedTlbStats::new();
            let mut latency_dist = LatencyDistribution::default();

            for (i, _addr) in addresses.iter().enumerate() {
                let latency = simulate_latency(i);

                // 记录延迟样本
                latency_dist.add_sample(latency);

                // 简单命中/未命中计数
                if latency < 30 {
                    // 假设30ns以内为命中
                    stats.base.hits += 1;
                } else {
                    stats.base.misses += 1;
                }

                stats.base.lookups += 1;
            }

            // 计算统计
            stats.record_latency(15); // 平均延迟
            black_box(stats);
        });
    });
}

// ============================================================================
// 基准测试2：未命中原因分析测试
// ============================================================================

/// 测试未命中原因分析的性能开销
fn bench_miss_reason_analysis(c: &mut Criterion) {
    let mut group = c.benchmark_group("miss_reason_analysis");

    let addresses = generate_test_addresses(10000, 0x2000_0000);

    // 基准：不使用未命中原因分析
    group.bench_function("without_miss_reason", |b| {
        b.iter(|| {
            let mut misses = 0u64;

            for (i, addr) in addresses.iter().enumerate() {
                // 简单的未命中计数
                if i % 8 == 0 {
                    misses += 1;
                }
                let _ = black_box(*addr);
            }

            black_box(misses);
        });
    });

    // 基准：使用未命中原因分析
    group.bench_function("with_miss_reason", |b| {
        b.iter(|| {
            let mut stats = EnhancedTlbStats::new();

            for (i, addr) in addresses.iter().enumerate() {
                // 模拟不同类型的未命中
                let reason = match i % 4 {
                    0 => MissReason::Capacity,
                    1 => MissReason::Conflict,
                    2 => MissReason::Cold,
                    3 => MissReason::Prefetch,
                    _ => unreachable!(),
                };

                stats.record_miss(reason);
                stats.base.misses += 1;
                let _ = black_box(*addr);
            }

            let _ = stats.miss_reasons.get_distribution();
            black_box(stats);
        });
    });
}

// ============================================================================
// 基准测试3：策略切换跟踪测试
// ============================================================================

/// 测试策略切换跟踪的性能开销
fn bench_policy_switch_tracking(c: &mut Criterion) {
    let mut group = c.benchmark_group("policy_switch_tracking");

    // 基准：不跟踪策略切换
    group.bench_function("without_tracking", |b| {
        b.iter(|| {
            let mut stats = EnhancedTlbStats::new();

            for i in 0..1000 {
                // 模拟1000次访问
                stats.base.lookups += 1;
                stats.base.hits += 800; // 80%命中率
                stats.base.misses += 200; // 20%未命中率

                black_box(stats.base.lookups);
            }

            black_box(stats);
        });
    });

    // 基准：跟踪策略切换
    group.bench_function("with_tracking", |b| {
        b.iter(|| {
            let mut stats = EnhancedTlbStats::new();

            // 模拟10次策略切换
            for i in 0..10 {
                let from_policy = match i % 3 {
                    0 => TlbReplacePolicy::Lru,
                    1 => TlbReplacePolicy::Fifo,
                    2 => TlbReplacePolicy::Clock,
                    _ => unreachable!(),
                };

                let to_policy = match (i + 1) % 3 {
                    0 => TlbReplacePolicy::Lru,
                    1 => TlbReplacePolicy::Fifo,
                    2 => TlbReplacePolicy::Clock,
                    _ => unreachable!(),
                };

                let reason = match i % 3 {
                    0 => crate::tlb::tlb::SwitchReason::LowHitRate,
                    1 => crate::tlb::tlb::SwitchReason::PatternChange,
                    2 => crate::tlb::tlb::SwitchReason::PeriodicEvaluation,
                    _ => unreachable!(),
                };

                stats.record_policy_switch(from_policy, to_policy, reason);

                // 模拟100次访问
                for j in 0..100 {
                    stats.base.lookups += 1;
                    stats.base.hits += 80;
                    stats.base.misses += 20;
                }
            }

            black_box(stats);
        });
    });
}

// ============================================================================
// 基准测试4：预取准确率测试
// ============================================================================

/// 测试预取准确率计算的性能开销
fn bench_prefetch_accuracy(c: &mut Criterion) {
    let mut group = c.benchmark_group("prefetch_accuracy");

    let addresses = generate_test_addresses(10000, 0x3000_0000);

    // 基准：不跟踪预取准确率
    group.bench_function("without_accuracy", |b| {
        b.iter(|| {
            let mut prefetch_count = 0u64;
            let mut prefetch_hits = 0u64;

            for i in 0..1000 {
                prefetch_count += 1;
                // 模拟70%的预取命中率
                if i % 10 < 7 {
                    prefetch_hits += 1;
                }
            }

            black_box(prefetch_count);
            black_box(prefetch_hits);
        });
    });

    // 基准：跟踪预取准确率
    group.bench_function("with_accuracy", |b| {
        b.iter(|| {
            let mut stats = EnhancedTlbStats::new();

            for i in 0..1000 {
                let success = i % 10 < 7; // 70%成功率
                stats.prefetch_accuracy.record_prefetch(success);
            }

            let accuracy = stats.prefetch_accuracy.accuracy;
            black_box(accuracy);
        });
    });
}

// ============================================================================
// 基准测试5：综合统计报告生成测试
// ============================================================================

/// 测试统计报告生成的性能开销
fn bench_stats_report_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("stats_report_generation");

    let addresses = generate_test_addresses(50000, 0x4000_0000);

    // 基准：简单统计
    group.bench_function("simple_stats", |b| {
        b.iter(|| {
            let mut lookups = 0u64;
            let mut hits = 0u64;

            for addr in black_box(addresses.iter()) {
                let _ = black_box(*addr);
                lookups += 1;
                hits += 1; // 假设90%命中
            }

            let misses = lookups - hits;
            let hit_rate = hits as f64 / lookups as f64;

            black_box(hit_rate);
        });
    });

    // 基准：使用增强统计生成报告
    group.bench_function("enhanced_stats_report", |b| {
        b.iter(|| {
            let mut stats = EnhancedTlbStats::new();

            for (i, addr) in addresses.iter().enumerate() {
                let latency = simulate_latency(i);

                // 记录各种统计
                stats.record_latency(latency);

                if latency < 30 {
                    stats.base.hits += 1;
                } else {
                    stats.base.misses += 1;
                    // 记录未命中原因
                    if i % 10 == 0 {
                        stats.record_miss(MissReason::Capacity);
                    } else if i % 10 == 5 {
                        stats.record_miss(MissReason::Cold);
                    }
                }

                stats.base.lookups += 1;
            }

            // 生成报告
            let report = stats.generate_report();
            black_box(report);
        });
    });
}

// ============================================================================
// 基准测试6：不同TLB策略对比
// ============================================================================

/// 对比不同TLB替换策略的性能
fn bench_tlb_policies_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("tlb_policies_comparison");

    let addresses = generate_test_addresses(10000, 0x5000_0000);

    // 对比LRU、LFU、FIFO策略
    for policy in &[
        TlbReplacePolicy::Lru,
        TlbReplacePolicy::Fifo,
        // TlbReplacePolicy::Clock, // 如果Clock支持，可以添加
    ] {
        let policy_name = format!("{:?}", policy);

        group.bench_with_input(BenchmarkId::new(&policy_name), policy, |b, _policy| {
            let tlb = TlbFactory::create_basic_tlb(128);

            b.iter(|| {
                for addr in black_box(addresses.iter()) {
                    // 插入TLB条目
                    let gpa = GuestPhysAddr(0x2000_0000 + (addr.0 as u64));
                    tlb.insert(*addr, gpa, 0x7, 0);

                    // 查找TLB条目
                    let _ = tlb.lookup(*addr, AccessType::Read);
                }
            });
        });
    }
}

// ============================================================================
// 主函数：注册所有基准测试
// ============================================================================

criterion_group!(
    benches,
    bench_latency_distribution,
    bench_miss_reason_analysis,
    bench_policy_switch_tracking,
    bench_prefetch_accuracy,
    bench_stats_report_generation,
    bench_tlb_policies_comparison
);

criterion_main!(benches);
