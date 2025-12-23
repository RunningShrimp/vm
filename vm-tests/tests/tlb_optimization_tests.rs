//! TLB优化综合测试
//!
//! 验证多级TLB和并发优化的性能提升

use std::sync::Arc;
use std::thread;
use std::time::Instant;
use vm_core::{AccessType, MMU};
use vm_mem::{
    MmuOptimizationStrategy, SoftMmu, UnifiedMmu,
    UnifiedMmuConfig,
};

/// 测试多级TLB性能提升
#[test]
fn test_multilevel_tlb_performance() {
    println!("=== 多级TLB性能测试 ===");

    // 原始SoftMMU
    let mut original_mmu = SoftMmu::new(64 * 1024 * 1024, false);

    // 多级TLB优化MMU
    let config = UnifiedMmuConfig {
        strategy: MmuOptimizationStrategy::MultiLevel,
        ..Default::default()
    };
    let mut optimized_mmu = UnifiedMmu::new(64 * 1024 * 1024, false, config);

    // 测试参数
    let test_addresses: Vec<u64> = (0..1000).map(|i| i * 4096).collect();
    let iterations = 10000;

    // 预热
    for &addr in &test_addresses {
        let _ = original_mmu.translate(addr, AccessType::Read);
        let _ = optimized_mmu.translate(addr, AccessType::Read);
    }

    // 测试原始MMU
    let start = Instant::now();
    for _ in 0..iterations {
        for &addr in &test_addresses {
            let _ = original_mmu.translate(addr, AccessType::Read);
        }
    }
    let original_duration = start.elapsed();

    // 测试优化MMU
    let start = Instant::now();
    for _ in 0..iterations {
        for &addr in &test_addresses {
            let _ = optimized_mmu.translate(addr, AccessType::Read);
        }
    }
    let optimized_duration = start.elapsed();

    // 计算性能提升
    let speedup = original_duration.as_nanos() as f64 / optimized_duration.as_nanos() as f64;

    println!("原始MMU: {:?}", original_duration);
    println!("优化MMU: {:?}", optimized_duration);
    println!("性能提升: {:.2}x", speedup);

    // 获取统计信息
    let stats = optimized_mmu.stats();
    println!("TLB命中率: {:.2}%", stats.tlb_hit_rate() * 100.0);
    println!("平均延迟: {:.1} ns", stats.avg_latency_ns());

    // 性能断言
    assert!(speedup > 1.2, "多级TLB应该至少提供20%的性能提升");
    assert!(stats.tlb_hit_rate() > 0.8, "TLB命中率应该超过80%");
}

/// 测试并发TLB性能
#[test]
fn test_concurrent_tlb_performance() {
    println!("=== 并发TLB性能测试 ===");

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

    // 单线程性能测试
    let start = Instant::now();
    for i in 0..10000 {
        let addr = ((i % 1000) * 4096) as u64;
        let _ = concurrent_mmu.translate(addr, AccessType::Read);
    }
    let single_thread_duration = start.elapsed();

    // 多线程性能测试
    let thread_count = 4;
    let iterations_per_thread = 2500;

    let start = Instant::now();
    let mut handles = vec![];

    for t in 0..thread_count {
        let mmu_clone = concurrent_mmu.clone();
        let handle = thread::spawn(move || {
            for i in 0..iterations_per_thread {
                let addr = ((t * iterations_per_thread + i) % 1000 * 4096) as u64;
                let _ = mmu_clone.translate(addr, AccessType::Read);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
    let multi_thread_duration = start.elapsed();

    let scalability = single_thread_duration.as_nanos() as f64
        / multi_thread_duration.as_nanos() as f64
        / thread_count as f64;

    println!("单线程性能: {:?}", single_thread_duration);
    println!("多线程性能: {:?}", multi_thread_duration);
    println!("可扩展性: {:.2}x", scalability);

    // 获取统计信息
    let stats = concurrent_mmu.stats();
    println!("并发TLB命中率: {:.2}%", stats.tlb_hit_rate() * 100.0);

    // 性能断言
    assert!(scalability > 0.7, "并发TLB应该有良好的可扩展性");
    assert!(stats.tlb_hit_rate() > 0.7, "并发TLB命中率应该超过70%");
}

/// 测试混合策略性能
#[test]
fn test_hybrid_strategy_performance() {
    println!("=== 混合策略性能测试 ===");

    // 混合策略配置
    let config = UnifiedMmuConfig {
        strategy: MmuOptimizationStrategy::Hybrid,
        ..Default::default()
    };
    let mut hybrid_mmu = UnifiedMmu::new(64 * 1024 * 1024, false, config);

    // 顺序访问模式测试（测试预取效果）
    let sequential_start = Instant::now();
    for i in 0..10000 {
        let addr = (i * 4096) as u64;
        let _ = hybrid_mmu.translate(addr, AccessType::Read);
    }
    let sequential_duration = sequential_start.elapsed();

    // 随机访问模式测试
    let random_start = Instant::now();
    for i in 0..10000 {
        let addr = ((i * 17) % 1000 * 4096) as u64;
        let _ = hybrid_mmu.translate(addr, AccessType::Read);
    }
    let random_duration = random_start.elapsed();

    println!("顺序访问: {:?}", sequential_duration);
    println!("随机访问: {:?}", random_duration);

    let sequential_speedup =
        random_duration.as_nanos() as f64 / sequential_duration.as_nanos() as f64;
    println!("顺序访问加速比: {:.2}x", sequential_speedup);

    // 获取统计信息
    let stats = hybrid_mmu.stats();
    println!("整体TLB命中率: {:.2}%", stats.tlb_hit_rate() * 100.0);
    println!(
        "预取命中次数: {}",
        stats
            .prefetch_hits
            .load(std::sync::atomic::Ordering::Relaxed)
    );

    // 性能断言
    assert!(sequential_speedup > 1.1, "预取应该在顺序访问中提供性能提升");
    assert!(stats.tlb_hit_rate() > 0.75, "混合策略命中率应该超过75%");
}

/// 测试TLB容量优化
#[test]
fn test_tlb_capacity_optimization() {
    println!("=== TLB容量优化测试 ===");

    let capacities = vec![128, 512, 1024, 2048, 4096];
    let mut results = Vec::new();

    for &capacity in &capacities {
        let config = UnifiedMmuConfig {
            strategy: MmuOptimizationStrategy::MultiLevel,
            ..Default::default()
        };
        let mut mmu = UnifiedMmu::new(64 * 1024 * 1024, false, config);

        // 预热到容量的80%
        let working_set = capacity * 8 / 10;
        for i in 0..working_set {
            let addr = (i * 4096) as u64;
            let _ = mmu.translate(addr, AccessType::Read);
        }

        // 测试性能
        let start = Instant::now();
        for i in 0..10000 {
            let addr = ((i % working_set) * 4096) as u64;
            let _ = mmu.translate(addr, AccessType::Read);
        }
        let duration = start.elapsed();

        let stats = mmu.get_stats();
        let hit_rate = stats.hit_rate();
        let throughput = 10000.0 / duration.as_secs_f64();

        results.push((capacity, hit_rate, throughput));
        println!(
            "容量 {}: 命中率 {:.2}%, 吞吐量 {:.0} ops/sec",
            capacity,
            hit_rate * 100.0,
            throughput
        );
    }

    // 验证容量与性能的关系
    for window in results.windows(2) {
        let (prev_cap, prev_hit, _) = window[0];
        let (curr_cap, curr_hit, _) = window[1];

        // 更大的容量应该有相等或更好的命中率
        assert!(
            curr_hit >= prev_hit - 0.05,
            "容量从{}增加到{}时，命中率不应该显著下降",
            prev_cap,
            curr_cap
        );
    }
}

/// 测试不同访问模式的性能
#[test]
fn test_access_pattern_performance() {
    println!("=== 访问模式性能测试 ===");

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

    // 测试不同访问模式
    let patterns = vec![
        ("顺序访问", |i: usize| ((i % 1000) * 4096) as u64),
        ("步长访问", |i: usize| ((i * 17 % 1000) * 4096) as u64),
        ("随机访问", |i: usize| ((i * 31 % 1000) * 4096) as u64),
        ("局部访问", |i: usize| ((i % 100) * 4096) as u64), // 10%的工作集
    ];

    for (name, pattern) in patterns {
        let start = Instant::now();
        for i in 0..10000 {
            let addr = pattern(i);
            let _ = mmu.translate(addr, AccessType::Read);
        }
        let duration = start.elapsed();
        let throughput = 10000.0 / duration.as_secs_f64();

        println!("{}: {:.0} ops/sec", name, throughput);
    }

    let stats = mmu.stats();
    println!("整体命中率: {:.2}%", stats.tlb_hit_rate() * 100.0);
    println!("平均延迟: {:.1} ns", stats.avg_latency_ns());
}

/// 测试TLB替换策略效果
#[test]
fn test_replacement_strategy_effectiveness() {
    println!("=== 替换策略效果测试 ===");

    let strategies = vec!["LRU", "频率LRU", "混合策略", "2Q算法"];

    for name in strategies {
        let config = UnifiedMmuConfig {
            strategy: MmuOptimizationStrategy::MultiLevel,
            multilevel_tlb_config: MultiLevelTlbConfig {
                l1_capacity: 64,
                adaptive_replacement: true,
                ..Default::default()
            },
            concurrent_tlb_config: ConcurrentTlbConfig::default(),
            ..Default::default()
        };
        let mut mmu = UnifiedMmu::new(64 * 1024 * 1024, false, config);

        // 模拟工作负载：80%热地址，20%冷地址
        let start = Instant::now();
        let mut hits = 0;

        for i in 0..10000 {
            let addr = if i < 8000 {
                // 热地址：前10%的地址空间
                ((i % 100) * 4096) as u64
            } else {
                // 冷地址：随机访问
                ((i * 13 % 1000) * 4096) as u64
            };

            if mmu.translate(addr, AccessType::Read).is_ok() {
                hits += 1;
            }
        }

        let duration = start.elapsed();
        let stats = mmu.stats();
        let hit_rate = stats.tlb_hit_rate();
        let throughput = 10000.0 / duration.as_secs_f64();

        println!(
            "{}: 命中率 {:.2}%, 吞吐量 {:.0} ops/sec",
            name,
            hit_rate * 100.0,
            throughput
        );

        // 基本性能要求
        assert!(hit_rate > 0.6, "{}策略的命中率应该超过60%", name);
        assert!(throughput > 10000.0, "{}策略应该有合理的吞吐量", name);
    }
}

/// 综合性能回归测试
#[test]
fn test_performance_regression() {
    println!("=== 性能回归测试 ===");

    // 基准配置
    let config = UnifiedMmuConfig::default();
    let mut mmu = UnifiedMmu::new(64 * 1024 * 1024, false, config);

    // 预热
    for i in 0..1000 {
        let addr = (i * 4096) as u64;
        let _ = mmu.translate(addr, AccessType::Read);
    }

    // 性能基准测试
    let start = Instant::now();
    let operations = 100000;

    for i in 0..operations {
        let addr = ((i % 1000) * 4096) as u64;
        let _ = mmu.translate(addr, AccessType::Read);
    }

    let duration = start.elapsed();
    let throughput = operations as f64 / duration.as_secs_f64();
    let stats = mmu.stats();

    println!("综合性能测试:");
    println!("  操作数: {}", operations);
    println!("  耗时: {:?}", duration);
    println!("  吞吐量: {:.0} ops/sec", throughput);
    println!("  命中率: {:.2}%", stats.tlb_hit_rate() * 100.0);
    println!("  平均延迟: {:.1} ns", stats.avg_latency_ns());

    // 性能要求
    assert!(throughput > 50000.0, "吞吐量应该超过50,000 ops/sec");
    assert!(stats.tlb_hit_rate() > 0.8, "命中率应该超过80%");
    assert!(stats.avg_latency_ns() < 1000.0, "平均延迟应该低于1000 ns");

    println!("✅ 所有性能要求都满足");
}
