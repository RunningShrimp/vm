//! Round 37: 生产级优化系统集成示例
//!
//! 展示如何将AutoOptimizer和RealTimeMonitor集成到生产环境中

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use vm_core::optimization::{
    AutoOptimizer, OptimizationStrategy, PerformanceMetrics as AutoMetrics,
};
use vm_monitor::{RealTimeMetrics, RealTimeMonitor};

fn main() {
    println!("=== Round 37: 生产级优化系统集成 ===\n");

    // 1. 创建AutoOptimizer
    let optimizer = AutoOptimizer::new();
    println!("✓ AutoOptimizer已创建");

    // 2. 创建RealTimeMonitor
    let monitor = RealTimeMonitor::new();
    println!("✓ RealTimeMonitor已创建");

    // 3. 显示平台信息
    let platform = optimizer.platform();
    println!("\n📊 平台信息:");
    println!("  架构: {}", platform.architecture);
    println!("  核心数: {}", platform.core_count);
    println!("  NEON支持: {}", platform.supports_neon);
    println!("  AVX2支持: {}", platform.supports_avx2);

    // 4. 模拟生产工作负载
    println!("\n📈 模拟生产工作负载...");

    let now_ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64;

    // 模拟1000次操作
    for i in 0..1000 {
        // 模拟操作时间 (波动模式)
        let base_latency = 10000 + (i % 100) * 50; // 10-15us基准
        let spike = if i % 200 == 0 { 5000 } else { 0 }; // 每200次操作有一次延迟尖峰
        let latency = base_latency + spike;

        // 记录到AutoOptimizer
        let auto_metrics = AutoMetrics {
            timestamp_ns: now_ns + i * 1_000_000,
            operation_time_ns: latency,
            memory_used_bytes: 1024 * 10,
            cpu_usage_percent: 60.0,
            cache_hit_rate: Some(0.8),
        };
        optimizer.record_metrics(auto_metrics);

        // 记录到RealTimeMonitor
        let rt_metrics = RealTimeMetrics {
            timestamp_ns: now_ns + i * 1_000_000,
            operation_type: "vm_execution".to_string(),
            latency_ns: latency,
            memory_bytes: 1024 * 10,
            cpu_percent: 60.0,
            throughput_ops_per_sec: 1_000_000.0 / latency as f64,
        };
        monitor.record_metric(rt_metrics);

        // 每250次操作输出进度
        if (i + 1) % 250 == 0 {
            println!("  已记录 {} 次操作...", i + 1);
        }
    }

    println!("✓ 工作负载模拟完成");

    // 5. 获取优化策略
    println!("\n🔧 自动优化分析:");
    let strategy = optimizer.analyze_and_optimize();
    println!("  工作负载类型: {:?}", strategy.workload);
    println!("  SIMD优化: {}", strategy.enable_simd);
    println!("  NEON优化: {}", strategy.enable_neon);
    println!("  内存池: {}", strategy.enable_memory_pool);
    println!("  对象池: {}", strategy.enable_object_pool);
    println!("  TLB优化: {}", strategy.enable_tlb_optimization);
    println!("  JIT热点: {}", strategy.enable_jit_hotspot);
    println!("  内存对齐: {} 字节", strategy.memory_alignment);
    println!("  性能核心优先: {}", strategy.prefer_performance_cores);

    // 6. 获取性能统计窗口
    println!("\n📊 性能统计窗口:");
    if let Some(window) = monitor.current_window() {
        println!("  样本数: {}", window.sample_count);
        println!("  平均延迟: {:.0} ns", window.avg_latency_ns);
        println!("  P50延迟: {} ns", window.p50_latency_ns);
        println!("  P95延迟: {} ns", window.p95_latency_ns);
        println!("  P99延迟: {} ns", window.p99_latency_ns);
        println!("  最小延迟: {} ns", window.min_latency_ns);
        println!("  最大延迟: {} ns", window.max_latency_ns);
        println!("  标准差: {:.0} ns", window.std_dev_ns);
        println!("  吞吐量: {:.0} ops/s", window.total_throughput);
    }

    // 7. 检查性能异常
    println!("\n⚠️  性能异常检测:");
    let anomalies = monitor.recent_anomalies(10);
    if anomalies.is_empty() {
        println!("  ✓ 未检测到异常");
    } else {
        println!("  检测到 {} 个异常:", anomalies.len());
        for anomaly in anomalies.iter().take(5) {
            println!(
                "    - {:?}: 严重度 {:.2}",
                anomaly.anomaly_type, anomaly.severity
            );
            println!("      {}", anomaly.description);
            println!("      建议: {}", anomaly.suggested_action);
        }
    }

    // 8. 性能基线对比
    println!("\n📈 性能基线对比:");
    if let Some(current) = monitor.current_window() {
        if let Some(baseline) = monitor.baseline() {
            let latency_change = (current.avg_latency_ns - baseline.avg_latency_ns)
                / baseline.avg_latency_ns
                * 100.0;
            let throughput_change = (current.total_throughput - baseline.total_throughput)
                / baseline.total_throughput
                * 100.0;

            println!("  延迟变化: {:+.1}%", latency_change);
            println!("  吞吐量变化: {:+.1}%", throughput_change);

            if latency_change > 10.0 {
                println!("  ⚠️  延迟显著增加,建议启用更多优化");
            } else if latency_change < -10.0 {
                println!("  ✓ 延迟显著降低,优化效果良好");
            }
        }
    }

    // 9. 持续监控建议
    println!("\n💡 持续监控建议:");
    println!("  1. 在生产环境中定期调用monitor.record_metric()");
    println!("  2. 设置告警阈值,当异常检测触发时通知");
    println!("  3. 定期调用optimizer.analyze_and_optimize()调整策略");
    println!("  4. 监控性能趋势,及时发现回归");
    println!("  5. 记录优化前后的性能指标进行对比");

    // 10. 集成到应用
    println!("\n🔗 应用集成示例:");
    println!("  ```rust");
    println!("  // 在应用初始化时");
    println!("  let optimizer = AutoOptimizer::new();");
    println!("  let monitor = RealTimeMonitor::new();");
    println!();
    println!("  // 在关键操作后");
    println!("  let start = Instant::now();");
    println!("  // ... 执行操作 ...");
    println!("  let latency = start.elapsed().as_nanos() as u64;");
    println!();
    println!("  optimizer.record_metrics(AutoMetrics::new(latency));");
    println!("  monitor.record_metric(RealTimeMetrics {");
    println!("      timestamp_ns: now,");
    println!("      operation_type: \"critical_path\".to_string(),");
    println!("      latency_ns: latency,");
    println!("      ...");
    println!("  });");
    println!();
    println!("  // 定期分析 (每100次操作)");
    println!("  if op_count % 100 == 0 {");
    println!("      let strategy = optimizer.analyze_and_optimize();");
    println!("      apply_strategy(&strategy);");
    println!();
    println!("      let anomalies = monitor.recent_anomalies(10);");
    println!("      if !anomalies.is_empty() {");
    println!("          alert_team(&anomalies);");
    println!("      }");
    println!("  }");
    println!("  ```");

    println!("\n✅ Round 37集成演示完成!");
}
