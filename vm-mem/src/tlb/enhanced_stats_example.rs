//! TLB增强统计功能示例
//!
//! 展示如何使用新增的TLB统计功能：
//! - LatencyDistribution - 延迟分布统计
//! - MissReasonAnalysis - 未命中原因分析
//! - PolicySwitchEvent - 策略切换历史
//! - PrefetchAccuracy - 预取准确率
//! - EnhancedTlbStats - 综合增强统计

use super::unified_tlb::{
    EnhancedTlbStats, LatencyDistribution, MissReasonAnalysis, PolicySwitchEvent, PrefetchAccuracy,
};

// 从模块根导入枚举
use super::{MissReason, SwitchReason, TlbReplacePolicy};
use std::time::{Duration, Instant};

/// 示例1：延迟分布统计
pub fn example_latency_distribution() {
    println!("=== 延迟分布统计示例 ===\n");

    let mut latency_dist = LatencyDistribution::default();

    // 添加延迟样本（纳秒）
    let latencies = vec![10, 15, 20, 12, 18, 25, 30, 14, 16, 22];

    for latency in latencies {
        latency_dist.add_sample(latency);
    }

    // 计算标准差
    latency_dist.calculate_std_dev();

    // 计算分位数
    let samples = vec![10, 15, 20, 12, 18, 25, 30, 14, 16, 22];
    latency_dist.calculate_percentiles(&samples);

    println!("最小延迟: {} ns", latency_dist.min);
    println!("最大延迟: {} ns", latency_dist.max);
    println!("平均延迟: {:.2} ns", latency_dist.avg);
    println!("标准差: {:.2} ns", latency_dist.std_dev);
    println!("P50 (中位数): {} ns", latency_dist.percentiles.p50);
    println!("P90: {} ns", latency_dist.percentiles.p90);
    println!("P95: {} ns", latency_dist.percentiles.p95);
    println!("P99: {} ns", latency_dist.percentiles.p99);
    println!("P99.9: {} ns", latency_dist.percentiles.p99_9);
    println!("样本数量: {}\n", latency_dist.sample_count);
}

/// 示例2：未命中原因分析
pub fn example_miss_reason_analysis() {
    println!("=== 未命中原因分析示例 ===\n");

    let mut miss_analysis = MissReasonAnalysis::default();

    // 模拟各种未命中
    miss_analysis.record_miss(MissReason::Capacity); // 容量未命中
    miss_analysis.record_miss(MissReason::Capacity); // 容量未命中
    miss_analysis.record_miss(MissReason::Capacity); // 容量未命中
    miss_analysis.record_miss(MissReason::Conflict); // 冲突未命中
    miss_analysis.record_miss(MissReason::Conflict); // 冲突未命中
    miss_analysis.record_miss(MissReason::Cold); // 冷未命中
    miss_analysis.record_miss(MissReason::Prefetch); // 预取未命中
    miss_analysis.record_miss(MissReason::Cold); // 冷未命中

    // 获取未命中原因分布
    let distribution = miss_analysis.get_distribution();

    println!(
        "容量未命中: {} ({:.2}%)",
        miss_analysis.capacity_misses,
        distribution.capacity_rate * 100.0
    );
    println!(
        "冲突未命中: {} ({:.2}%)",
        miss_analysis.conflict_misses,
        distribution.conflict_rate * 100.0
    );
    println!(
        "冷未命中: {} ({:.2}%)",
        miss_analysis.cold_misses,
        distribution.cold_rate * 100.0
    );
    println!(
        "预取未命中: {} ({:.2}%)",
        miss_analysis.prefetch_misses,
        distribution.prefetch_rate * 100.0
    );
    println!("总未命中数: {}\n", miss_analysis.total_misses);

    // 分析建议
    if distribution.capacity_rate > 0.5 {
        println!("建议: 容量未命中占比较高 (>50%)，考虑增加TLB大小");
    }
    if distribution.conflict_rate > 0.3 {
        println!("建议: 冲突未命中占比较高 (>30%)，考虑使用更优化的替换策略");
    }
}

/// 示例3：策略切换历史
pub fn example_policy_switch_history() {
    println!("=== 策略切换历史示例 ===\n");

    let mut switch_events: Vec<PolicySwitchEvent> = Vec::new();

    // 模拟策略切换
    switch_events.push(PolicySwitchEvent {
        timestamp: Instant::now(),
        from_policy: TlbReplacePolicy::Lru,
        to_policy: TlbReplacePolicy::AdaptiveLru,
        reason: SwitchReason::LowHitRate,
    });

    std::thread::sleep(Duration::from_millis(10));

    switch_events.push(PolicySwitchEvent {
        timestamp: Instant::now(),
        from_policy: TlbReplacePolicy::AdaptiveLru,
        to_policy: TlbReplacePolicy::Clock,
        reason: SwitchReason::PatternChange,
    });

    std::thread::sleep(Duration::from_millis(10));

    switch_events.push(PolicySwitchEvent {
        timestamp: Instant::now(),
        from_policy: TlbReplacePolicy::AdaptiveLru,
        to_policy: TlbReplacePolicy::Clock,
        reason: SwitchReason::PeriodicEvaluation,
    });

    println!("策略切换次数: {}", switch_events.len());

    for (i, event) in switch_events.iter().enumerate() {
        println!("\n切换 #{}:", i + 1);
        println!("  从: {:?}", event.from_policy);
        println!("  到: {:?}", event.to_policy);
        println!("  原因: {:?}", event.reason);
        println!("  时间: {:?}", event.timestamp);
    }
}

/// 示例4：预取准确率
pub fn example_prefetch_accuracy() {
    println!("=== 预取准确率示例 ===\n");

    let mut prefetch_acc = PrefetchAccuracy::default();

    // 模拟预取操作
    let prefetch_results = vec![
        true, true, false, true, true, false, true, true, false, true,
    ];

    for success in prefetch_results {
        prefetch_acc.record_prefetch(success);
    }

    println!("预取总次数: {}", prefetch_acc.total_prefetches);
    println!("成功命中次数: {}", prefetch_acc.successful_hits);
    println!("准确率: {:.2}%", prefetch_acc.accuracy * 100.0);

    // 分析建议
    if prefetch_acc.accuracy > 0.8 {
        println!("评价: 预取准确率良好 (>80%)");
    } else if prefetch_acc.accuracy > 0.6 {
        println!("评价: 预取准确率中等 (60-80%)，考虑优化预取算法");
    } else {
        println!("评价: 预取准确率较低 (<60%)，需要重新设计预取策略");
    }
}

/// 示例5：综合增强统计
pub fn example_enhanced_stats() {
    println!("=== 综合增强统计示例 ===\n");

    let mut stats = EnhancedTlbStats::new();

    // 模拟TLB操作
    for i in 0..100 {
        // 模拟查找
        stats.base.lookups += 1;

        // 模拟命中/未命中
        if i % 3 == 0 {
            stats.base.misses += 1;

            // 记录未命中原因
            match i % 4 {
                0 => stats.miss_reasons.record_miss(MissReason::Capacity),
                1 => stats.miss_reasons.record_miss(MissReason::Conflict),
                2 => stats.miss_reasons.record_miss(MissReason::Cold),
                _ => stats.miss_reasons.record_miss(MissReason::Prefetch),
            }
        } else {
            stats.base.hits += 1;

            // 记录延迟（纳秒）
            let latency = 10 + (i % 20) as u64;
            stats.latency_distribution.add_sample(latency);
        }

        // 模拟预取
        if i % 10 == 0 {
            stats.prefetch_accuracy.record_prefetch(i % 3 != 0);
        }
    }

    // 计算统计
    stats.latency_distribution.calculate_std_dev();
    let samples: Vec<u64> = (10..30).collect();
    stats.latency_distribution.calculate_percentiles(&samples);

    // 打印统计信息
    println!("基础统计:");
    println!("  查找次数: {}", stats.base.lookups);
    println!("  命中次数: {}", stats.base.hits);
    println!("  未命中次数: {}", stats.base.misses);
    println!("  命中率: {:.2}%", stats.base.hit_rate() * 100.0);

    println!("\n延迟分布:");
    println!("  最小延迟: {} ns", stats.latency_distribution.min);
    println!("  最大延迟: {} ns", stats.latency_distribution.max);
    println!("  平均延迟: {:.2} ns", stats.latency_distribution.avg);
    println!("  P50: {} ns", stats.latency_distribution.percentiles.p50);
    println!("  P95: {} ns", stats.latency_distribution.percentiles.p95);
    println!("  P99: {} ns", stats.latency_distribution.percentiles.p99);

    println!("\n未命中原因:");
    let miss_dist = stats.miss_reasons.get_distribution();
    println!(
        "  容量未命中: {} ({:.2}%)",
        stats.miss_reasons.capacity_misses,
        miss_dist.capacity_rate * 100.0
    );
    println!(
        "  冲突未命中: {} ({:.2}%)",
        stats.miss_reasons.conflict_misses,
        miss_dist.conflict_rate * 100.0
    );
    println!(
        "  冷未命中: {} ({:.2}%)",
        stats.miss_reasons.cold_misses,
        miss_dist.cold_rate * 100.0
    );
    println!(
        "  预取未命中: {} ({:.2}%)",
        stats.miss_reasons.prefetch_misses,
        miss_dist.prefetch_rate * 100.0
    );

    println!("\n预取准确率:");
    println!("  预取次数: {}", stats.prefetch_accuracy.total_prefetches);
    println!("  成功次数: {}", stats.prefetch_accuracy.successful_hits);
    println!("  准确率: {:.2}%", stats.prefetch_accuracy.accuracy * 100.0);
}

/// 示例6：策略优化建议
pub fn example_optimization_suggestions() {
    println!("=== 策略优化建议示例 ===\n");

    let mut stats = EnhancedTlbStats::new();

    // 场景1：高容量未命中
    stats.miss_reasons.record_miss(MissReason::Capacity);
    stats.miss_reasons.record_miss(MissReason::Capacity);
    stats.miss_reasons.record_miss(MissReason::Capacity);
    stats.miss_reasons.record_miss(MissReason::Conflict);
    stats.miss_reasons.record_miss(MissReason::Conflict);
    stats.base.lookups = 100;
    stats.base.hits = 93;
    stats.base.misses = 7;

    println!(
        "场景1: 高容量未命中 (容量未命中率 {:.2}%)",
        stats.miss_reasons.capacity_misses as f64 / stats.miss_reasons.total_misses as f64 * 100.0
    );
    let hit_rate = stats.base.hit_rate() * 100.0;
    println!("  命中率: {:.2}%", hit_rate);

    let dist = stats.miss_reasons.get_distribution();
    if dist.capacity_rate > 0.4 && hit_rate < 95.0 {
        println!("  建议操作:");
        println!("    1. 增加TLB容量");
        println!("    2. 考虑使用多级TLB (L1 + L2)");
        println!("    3. 优化页面颜色分配");
    }

    println!("\n场景2: 低命中率 + 高冲突未命中");
    let mut stats2 = EnhancedTlbStats::new();
    stats2.miss_reasons.record_miss(MissReason::Conflict);
    stats2.miss_reasons.record_miss(MissReason::Conflict);
    stats2.miss_reasons.record_miss(MissReason::Conflict);
    stats2.miss_reasons.record_miss(MissReason::Cold);
    stats2.base.lookups = 100;
    stats2.base.hits = 85;
    stats2.base.misses = 15;

    let hit_rate2 = stats2.base.hit_rate() * 100.0;
    let dist2 = stats2.miss_reasons.get_distribution();
    println!("  命中率: {:.2}%", hit_rate2);
    println!("  冲突未命中率: {:.2}%", dist2.conflict_rate * 100.0);

    if dist2.conflict_rate > 0.5 && hit_rate2 < 90.0 {
        println!("  建议操作:");
        println!("    1. 更换TLB替换策略 (从LRU改为LFU或Tree-based)");
        println!("    2. 启用TLB分区 (按进程或地址空间)");
        println!("    3. 考虑使用自适应替换策略");
    }

    println!("\n场景3: 高预取准确率但命中率低");
    let mut stats3 = EnhancedTlbStats::new();
    for _ in 0..20 {
        stats3.prefetch_accuracy.record_prefetch(true);
    }
    stats3.prefetch_accuracy.record_prefetch(false);
    stats3.base.lookups = 100;
    stats3.base.hits = 70;
    stats3.base.misses = 30;

    let hit_rate3 = stats3.base.hit_rate() * 100.0;
    println!(
        "  预取准确率: {:.2}%",
        stats3.prefetch_accuracy.accuracy * 100.0
    );
    println!("  命中率: {:.2}%", hit_rate3);

    if stats3.prefetch_accuracy.accuracy > 0.9 && hit_rate3 < 80.0 {
        println!("  建议操作:");
        println!("    1. 检查预取时机是否正确");
        println!("    2. 增加预取距离或调整预取策略");
        println!("    3. 考虑使用stride-based预取");
    }
}

/// 运行所有示例
pub fn run_all_examples() {
    println!("╔════════════════════════════════════════════╗");
    println!("║   TLB增强统计功能示例演示                    ║");
    println!("╚════════════════════════════════════════════╝\n");

    example_latency_distribution();
    println!("────────────────────────────────────────────────────\n");

    example_miss_reason_analysis();
    println!("────────────────────────────────────────────────────\n");

    example_policy_switch_history();
    println!("────────────────────────────────────────────────────\n");

    example_prefetch_accuracy();
    println!("────────────────────────────────────────────────────\n");

    example_enhanced_stats();
    println!("────────────────────────────────────────────────────\n");

    example_optimization_suggestions();
    println!("────────────────────────────────────────────────────\n");

    println!("╔════════════════════════════════════════════╗");
    println!("║   所有示例演示完成！                             ║");
    println!("╚════════════════════════════════════════════╝");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latency_distribution() {
        let mut dist = LatencyDistribution::default();
        dist.add_sample(10);
        dist.add_sample(20);
        dist.add_sample(30);

        assert_eq!(dist.min, 10);
        assert_eq!(dist.max, 30);
        assert_eq!(dist.sample_count, 3);
    }

    #[test]
    fn test_miss_reason_analysis() {
        let mut analysis = MissReasonAnalysis::default();
        analysis.record_miss(MissReason::Capacity);
        analysis.record_miss(MissReason::Conflict);

        assert_eq!(analysis.total_misses, 2);
        assert_eq!(analysis.capacity_misses, 1);
        assert_eq!(analysis.conflict_misses, 1);
    }

    #[test]
    fn test_prefetch_accuracy() {
        let mut accuracy = PrefetchAccuracy::default();
        accuracy.record_prefetch(true);
        accuracy.record_prefetch(false);
        accuracy.record_prefetch(true);

        assert_eq!(accuracy.total_prefetches, 3);
        assert_eq!(accuracy.successful_hits, 2);
        assert_eq!(accuracy.accuracy, 2.0 / 3.0);
    }

    #[test]
    fn test_enhanced_stats() {
        let stats = EnhancedTlbStats::new();
        assert_eq!(stats.base.lookups, 0);
        assert_eq!(stats.base.hits, 0);
        assert_eq!(stats.base.misses, 0);
    }
}
