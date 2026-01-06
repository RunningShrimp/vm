//! 自动优化系统示例
//!
//! 展示如何使用AutoOptimizer进行智能优化

use vm_core::optimization::Auto_optimizer::{AutoOptimizer, PerformanceMetrics, WorkloadType};

fn main() {
    println!("=== Round 36: 自动优化系统示例 ===\n");

    // 1. 创建自动优化控制器
    let optimizer = AutoOptimizer::new();

    // 2. 显示平台信息
    let platform = optimizer.platform();
    println!("📊 平台信息:");
    println!("  架构: {}", platform.architecture);
    println!("  核心数: {}", platform.core_count);
    println!("  NEON支持: {}", platform.supports_neon);
    println!("  AVX2支持: {}", platform.supports_avx2);
    println!("  大小核: {}", platform.has_big_little_cores);
    println!();

    // 3. 模拟不同工作负载的性能指标
    println!("📈 模拟工作负载...");

    // 场景1: 计算密集型
    println!("\n1️⃣  计算密集型工作负载:");
    for i in 0..20 {
        let metrics = PerformanceMetrics {
            timestamp_ns: 0,
            operation_time_ns: 50000 + (i as i64 * 100), // ~50us操作
            memory_used_bytes: 1024,
            cpu_usage_percent: 95.0,
            cache_hit_rate: Some(0.85),
        };
        optimizer.record_metrics(metrics);
    }

    let strategy1 = optimizer.analyze_and_optimize();
    println!("  识别为: {:?}", strategy1.workload);
    println!("  SIMD优化: {}", strategy1.enable_simd);
    println!("  内存对齐: {} 字节", strategy1.memory_alignment);
    println!("  性能核心: {}", strategy1.prefer_performance_cores);

    // 场景2: 内存密集型
    println!("\n2️⃣  内存密集型工作负载:");
    for i in 0..20 {
        let metrics = PerformanceMetrics {
            timestamp_ns: 0,
            operation_time_ns: 8000 + (i as i64 * 50), // ~8us操作
            memory_used_bytes: 1024 * 1024, // 1MB
            cpu_usage_percent: 40.0,
            cache_hit_rate: Some(0.60),
        };
        optimizer.record_metrics(metrics);
    }

    let strategy2 = optimizer.analyze_and_optimize();
    println!("  识别为: {:?}", strategy2.workload);
    println!("  内存池: {}", strategy2.enable_memory_pool);
    println!("  SIMD优化: {}", strategy2.enable_simd);
    println!("  性能核心: {}", strategy2.prefer_performance_cores);

    // 场景3: 混合型工作负载
    println!("\n3️⃣  混合型工作负载:");
    for i in 0..20 {
        let metrics = PerformanceMetrics {
            timestamp_ns: 0,
            operation_time_ns: 10000 + (i as i64 * 10),
            memory_used_bytes: 1024 * 10,
            cpu_usage_percent: 60.0,
            cache_hit_rate: Some(0.75),
        };
        optimizer.record_metrics(metrics);
    }

    let strategy3 = optimizer.analyze_and_optimize();
    println!("  识别为: {:?}", strategy3.workload);
    println!("  全面优化: {}", strategy3.enable_simd && strategy3.enable_memory_pool);
    println!("  TLB优化: {}", strategy3.enable_tlb_optimization);
    println!("  JIT热点: {}", strategy3.enable_jit_hotspot);

    // 4. 总结
    println!("\n✅ 自动优化系统演示完成!");
    println!("\n关键特性:");
    println!("  ✓ 工作负载自动识别");
    println!("  ✓ 平台特性自动检测");
    println!("  ✓ 优化策略自动生成");
    println!("  ✓ 性能指标持续监控");
}
