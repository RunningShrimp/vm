//! 自适应GC测试套件
//!
//! 测试自适应垃圾回收调优的功能和性能

use vm_optimizers::gc_adaptive::{
    AdaptiveGCTuner, AdaptiveGCConfig, GCPerformanceMetrics, GCProblem, TuningAction,
};
use std::sync::atomic::Ordering;
use std::time::SystemTime;

// ============================================================================
// GCPerformanceMetrics测试
// ============================================================================

#[test]
fn test_gc_performance_metrics_creation() {
    let metrics = GCPerformanceMetrics {
        heap_size: 1024 * 1024,
        used_memory: 512 * 1024,
        fragmentation_rate: 0.2,
        avg_pause_time_ns: 1_000_000,
        p99_pause_time_ns: 5_000_000,
        throughput: 0.95,
        minor_gc_count: 10,
        major_gc_count: 2,
        promoted_objects: 100,
        collected_objects: 500,
    };

    assert_eq!(metrics.heap_size, 1024 * 1024);
    assert_eq!(metrics.used_memory, 512 * 1024);
    assert_eq!(metrics.fragmentation_rate, 0.2);
}

#[test]
fn test_gc_performance_metrics_compute_fragmentation() {
    let metrics = GCPerformanceMetrics {
        heap_size: 1000,
        used_memory: 700,
        fragmentation_rate: 0.0, // 将被计算
        avg_pause_time_ns: 0,
        p99_pause_time_ns: 0,
        throughput: 0.0,
        minor_gc_count: 0,
        major_gc_count: 0,
        promoted_objects: 0,
        collected_objects: 0,
    };

    let frag = metrics.compute_fragmentation();
    // 碎片率 = (1000 - 700) / 1000 = 0.3
    assert_eq!(frag, 0.3);
}

#[test]
fn test_gc_performance_metrics_zero_heap_fragmentation() {
    let metrics = GCPerformanceMetrics {
        heap_size: 0,
        used_memory: 0,
        fragmentation_rate: 0.0,
        avg_pause_time_ns: 0,
        p99_pause_time_ns: 0,
        throughput: 0.0,
        minor_gc_count: 0,
        major_gc_count: 0,
        promoted_objects: 0,
        collected_objects: 0,
    };

    let frag = metrics.compute_fragmentation();
    // 堆大小为0时应该返回0
    assert_eq!(frag, 0.0);
}

#[test]
fn test_gc_performance_metrics_is_high_memory_pressure() {
    let metrics = GCPerformanceMetrics {
        heap_size: 1000,
        used_memory: 950, // 95%使用率
        fragmentation_rate: 0.0,
        avg_pause_time_ns: 0,
        p99_pause_time_ns: 0,
        throughput: 0.0,
        minor_gc_count: 0,
        major_gc_count: 0,
        promoted_objects: 0,
        collected_objects: 0,
    };

    assert!(metrics.is_high_memory_pressure());
}

#[test]
fn test_gc_performance_metrics_is_not_high_memory_pressure() {
    let metrics = GCPerformanceMetrics {
        heap_size: 1000,
        used_memory: 800, // 80%使用率
        fragmentation_rate: 0.0,
        avg_pause_time_ns: 0,
        p99_pause_time_ns: 0,
        throughput: 0.0,
        minor_gc_count: 0,
        major_gc_count: 0,
        promoted_objects: 0,
        collected_objects: 0,
    };

    assert!(!metrics.is_high_memory_pressure());
}

#[test]
fn test_gc_performance_metrics_is_high_fragmentation() {
    let metrics = GCPerformanceMetrics {
        heap_size: 1000,
        used_memory: 500,
        fragmentation_rate: 0.4, // 40%碎片率
        avg_pause_time_ns: 0,
        p99_pause_time_ns: 0,
        throughput: 0.0,
        minor_gc_count: 0,
        major_gc_count: 0,
        promoted_objects: 0,
        collected_objects: 0,
    };

    assert!(metrics.is_high_fragmentation());
}

#[test]
fn test_gc_performance_metrics_is_not_high_fragmentation() {
    let metrics = GCPerformanceMetrics {
        heap_size: 1000,
        used_memory: 500,
        fragmentation_rate: 0.2, // 20%碎片率
        avg_pause_time_ns: 0,
        p99_pause_time_ns: 0,
        throughput: 0.0,
        minor_gc_count: 0,
        major_gc_count: 0,
        promoted_objects: 0,
        collected_objects: 0,
    };

    assert!(!metrics.is_high_fragmentation());
}

#[test]
fn test_gc_performance_metrics_is_long_pause() {
    let metrics = GCPerformanceMetrics {
        heap_size: 1000,
        used_memory: 500,
        fragmentation_rate: 0.0,
        avg_pause_time_ns: 0,
        p99_pause_time_ns: 15_000_000, // 15ms
        throughput: 0.0,
        minor_gc_count: 0,
        major_gc_count: 0,
        promoted_objects: 0,
        collected_objects: 0,
    };

    // 阈值为10ms，15ms > 10ms
    assert!(metrics.is_long_pause(10));
}

#[test]
fn test_gc_performance_metrics_is_not_long_pause() {
    let metrics = GCPerformanceMetrics {
        heap_size: 1000,
        used_memory: 500,
        fragmentation_rate: 0.0,
        avg_pause_time_ns: 0,
        p99_pause_time_ns: 5_000_000, // 5ms
        throughput: 0.0,
        minor_gc_count: 0,
        major_gc_count: 0,
        promoted_objects: 0,
        collected_objects: 0,
    };

    // 阈值为10ms，5ms < 10ms
    assert!(!metrics.is_long_pause(10));
}

#[test]
fn test_gc_performance_metrics_is_low_throughput() {
    let metrics = GCPerformanceMetrics {
        heap_size: 1000,
        used_memory: 500,
        fragmentation_rate: 0.0,
        avg_pause_time_ns: 0,
        p99_pause_time_ns: 0,
        throughput: 0.7, // 70%
        minor_gc_count: 0,
        major_gc_count: 0,
        promoted_objects: 0,
        collected_objects: 0,
    };

    // 阈值为0.8，0.7 < 0.8
    assert!(metrics.is_low_throughput(0.8));
}

#[test]
fn test_gc_performance_metrics_is_not_low_throughput() {
    let metrics = GCPerformanceMetrics {
        heap_size: 1000,
        used_memory: 500,
        fragmentation_rate: 0.0,
        avg_pause_time_ns: 0,
        p99_pause_time_ns: 0,
        throughput: 0.9, // 90%
        minor_gc_count: 0,
        major_gc_count: 0,
        promoted_objects: 0,
        collected_objects: 0,
    };

    // 阈值为0.8，0.9 > 0.8
    assert!(!metrics.is_low_throughput(0.8));
}

// ============================================================================
// GCProblem测试
// ============================================================================

#[test]
fn test_gc_problem_display() {
    assert_eq!(format!("{}", GCProblem::None), "None");
    assert_eq!(format!("{}", GCProblem::HighFragmentation), "HighFragmentation");
    assert_eq!(format!("{}", GCProblem::LongPauseTime), "LongPauseTime");
    assert_eq!(format!("{}", GCProblem::LowThroughput), "LowThroughput");
    assert_eq!(
        format!("{}", GCProblem::HighMemoryPressure),
        "HighMemoryPressure"
    );
    assert_eq!(format!("{}", GCProblem::FrequentPromotion), "FrequentPromotion");
    assert_eq!(format!("{}", GCProblem::OOMRisk), "OOMRisk");
}

#[test]
fn test_gc_problem_equality() {
    assert_eq!(GCProblem::HighFragmentation, GCProblem::HighFragmentation);
    assert_ne!(GCProblem::HighFragmentation, GCProblem::LongPauseTime);
}

// ============================================================================
// AdaptiveGCConfig测试
// ============================================================================

#[test]
fn test_adaptive_gc_config_default() {
    let config = AdaptiveGCConfig::default();

    assert_eq!(config.heap_size, 256 * 1024 * 1024); // 256MB
    assert_eq!(config.nursery_ratio, 0.1); // 10%
    assert_eq!(config.gc_threshold, 0.8); // 80%
    assert_eq!(config.work_quota, 100);
    assert_eq!(config.min_work_quota, 10);
    assert_eq!(config.max_work_quota, 1000);
    assert_eq!(config.promotion_age, 3);
}

#[test]
fn test_adaptive_gc_config_custom() {
    let config = AdaptiveGCConfig {
        heap_size: 2 * 1024 * 1024 * 1024, // 2GB
        nursery_ratio: 0.3, // 30%
        gc_threshold: 0.7, // 70%
        work_quota: 200,
        min_work_quota: 20,
        max_work_quota: 2000,
        promotion_age: 5,
        promotion_ratio: 0.7,
        enable_compaction: false,
        target_pause_time_ms: 10,
        target_throughput: 0.9,
        compaction_threshold: 0.5,
    };

    assert_eq!(config.heap_size, 2 * 1024 * 1024 * 1024);
    assert_eq!(config.nursery_ratio, 0.3);
    assert_eq!(config.gc_threshold, 0.7);
    assert_eq!(config.work_quota, 200);
    assert_eq!(config.min_work_quota, 20);
    assert_eq!(config.max_work_quota, 2000);
    assert_eq!(config.promotion_age, 5);
    assert!(!config.enable_compaction);
    assert_eq!(config.target_pause_time_ms, 10);
}

// ============================================================================
// TuningAction测试
// ============================================================================

#[test]
fn test_tuning_action_creation() {
    let old_config = AdaptiveGCConfig::default();
    let new_config = AdaptiveGCConfig {
        work_quota: 200,
        ..old_config.clone()
    };

    let action = TuningAction {
        timestamp: SystemTime::now(),
        problem: GCProblem::LongPauseTime,
        old_config,
        new_config,
        reason: "Pause time too long".to_string(),
    };

    assert_eq!(action.problem, GCProblem::LongPauseTime);
    assert_eq!(action.reason, "Pause time too long");
}

// ============================================================================
// AdaptiveGC测试
// ============================================================================

#[test]
fn test_adaptive_gc_creation() {
    let config = AdaptiveGCConfig::default();
    let gc = AdaptiveGCTuner::new(config);

    assert_eq!(gc.config().heap_size, 256 * 1024 * 1024);
}

#[test]
fn test_adaptive_gc_diagnose_high_fragmentation() {
    let config = AdaptiveGCConfig::default();
    let mut gc = AdaptiveGCTuner::new(config);

    let metrics = GCPerformanceMetrics {
        heap_size: 1000,
        used_memory: 500,
        fragmentation_rate: 0.4, // 高碎片率
        avg_pause_time_ns: 0,
        p99_pause_time_ns: 0,
        throughput: 0.95,
        minor_gc_count: 10,
        major_gc_count: 2,
        promoted_objects: 100,
        collected_objects: 500,
    };

    let problem = gc.diagnose(&metrics);

    // 应该诊断出高碎片率问题
    assert_eq!(problem, GCProblem::HighFragmentation);
}

#[test]
fn test_adaptive_gc_diagnose_long_pause() {
    let config = AdaptiveGCConfig::default();
    let mut gc = AdaptiveGCTuner::new(config);

    let metrics = GCPerformanceMetrics {
        heap_size: 1000,
        used_memory: 700,
        fragmentation_rate: 0.2,
        avg_pause_time_ns: 0,
        p99_pause_time_ns: 15_000_000, // 15ms
        throughput: 0.95,
        minor_gc_count: 10,
        major_gc_count: 2,
        promoted_objects: 100,
        collected_objects: 500,
    };

    let problem = gc.diagnose(&metrics);

    // 应该诊断出长暂停时间问题
    assert_eq!(problem, GCProblem::LongPauseTime);
}

#[test]
fn test_adaptive_gc_diagnose_low_throughput() {
    let config = AdaptiveGCConfig::default();
    let mut gc = AdaptiveGCTuner::new(config);

    let metrics = GCPerformanceMetrics {
        heap_size: 1000,
        used_memory: 700,
        fragmentation_rate: 0.2,
        avg_pause_time_ns: 0,
        p99_pause_time_ns: 5_000_000,
        throughput: 0.7, // 低吞吐量
        minor_gc_count: 10,
        major_gc_count: 2,
        promoted_objects: 100,
        collected_objects: 500,
    };

    let problem = gc.diagnose(&metrics);

    // 应该诊断出低吞吐量问题
    assert_eq!(problem, GCProblem::LowThroughput);
}

#[test]
fn test_adaptive_gc_diagnose_high_memory_pressure() {
    let config = AdaptiveGCConfig::default();
    let mut gc = AdaptiveGCTuner::new(config);

    let metrics = GCPerformanceMetrics {
        heap_size: 1000,
        used_memory: 950, // 95%使用率
        fragmentation_rate: 0.2,
        avg_pause_time_ns: 0,
        p99_pause_time_ns: 5_000_000,
        throughput: 0.95,
        minor_gc_count: 10,
        major_gc_count: 2,
        promoted_objects: 100,
        collected_objects: 500,
    };

    let problem = gc.diagnose(&metrics);

    // 95%使用率应该被诊断为OOM风险
    assert_eq!(problem, GCProblem::OOMRisk);
}

#[test]
fn test_adaptive_gc_diagnose_none() {
    let config = AdaptiveGCConfig::default();
    let mut gc = AdaptiveGCTuner::new(config);

    let metrics = GCPerformanceMetrics {
        heap_size: 1000,
        used_memory: 700,
        fragmentation_rate: 0.2,
        avg_pause_time_ns: 0,
        p99_pause_time_ns: 5_000_000,
        throughput: 0.95,
        minor_gc_count: 10,
        major_gc_count: 2,
        promoted_objects: 100,
        collected_objects: 500,
    };

    let problem = gc.diagnose(&metrics);

    // 所有指标正常，应该诊断出无问题
    assert_eq!(problem, GCProblem::None);
}

#[test]
fn test_adaptive_gc_tune_high_fragmentation() {
    let config = AdaptiveGCConfig::default();
    let mut gc = AdaptiveGCTuner::new(config);

    let metrics = GCPerformanceMetrics {
        heap_size: 1000,
        used_memory: 500,
        fragmentation_rate: 0.4, // 高碎片率
        avg_pause_time_ns: 0,
        p99_pause_time_ns: 0,
        throughput: 0.95,
        minor_gc_count: 10,
        major_gc_count: 2,
        promoted_objects: 100,
        collected_objects: 500,
    };

    gc.record_metrics(metrics);
    let _action = gc.tune();

    // 缩小新生代比例
    let new_config = gc.config();
    assert!(new_config.nursery_ratio < 0.2);
}

#[test]
fn test_adaptive_gc_tune_long_pause() {
    let config = AdaptiveGCConfig {
        work_quota: 100,
        ..Default::default()
    };
    let mut gc = AdaptiveGCTuner::new(config);

    let metrics = GCPerformanceMetrics {
        heap_size: 1000,
        used_memory: 700,
        fragmentation_rate: 0.2,
        avg_pause_time_ns: 0,
        p99_pause_time_ns: 15_000_000, // 长暂停
        throughput: 0.95,
        minor_gc_count: 10,
        major_gc_count: 2,
        promoted_objects: 100,
        collected_objects: 500,
    };

    gc.record_metrics(metrics);
    let _action = gc.tune();

    // 减小工作配额
    let new_config = gc.config();
    assert!(new_config.work_quota < 100);
}

#[test]
fn test_adaptive_gc_tune_low_throughput() {
    let config = AdaptiveGCConfig {
        gc_threshold: 0.8,
        work_quota: 100,
        ..Default::default()
    };
    let mut gc = AdaptiveGCTuner::new(config);

    let metrics = GCPerformanceMetrics {
        heap_size: 1000,
        used_memory: 700,
        fragmentation_rate: 0.2,
        avg_pause_time_ns: 0,
        p99_pause_time_ns: 5_000_000,
        throughput: 0.7, // 低吞吐量
        minor_gc_count: 10,
        major_gc_count: 2,
        promoted_objects: 100,
        collected_objects: 500,
    };

    gc.record_metrics(metrics);
    let _action = gc.tune();

    // 延迟GC触发，增加工作配额
    let new_config = gc.config();
    assert!(new_config.gc_threshold > 0.8);
    assert!(new_config.work_quota > 100);
}

#[test]
fn test_adaptive_gc_get_stats() {
    let config = AdaptiveGCConfig::default();
    let gc = AdaptiveGCTuner::new(config);

    let stats = gc.stats();

    // 初始统计应该为0
    assert_eq!(stats.tuning_count.load(Ordering::Relaxed), 0);
    assert_eq!(stats.problem_detection_count.load(Ordering::Relaxed), 0);
    assert_eq!(stats.config_change_count.load(Ordering::Relaxed), 0);
}

#[test]
fn test_adaptive_gc_get_tuning_history() {
    let config = AdaptiveGCConfig::default();
    let mut gc = AdaptiveGCTuner::new(config);

    let metrics = GCPerformanceMetrics {
        heap_size: 1000,
        used_memory: 500,
        fragmentation_rate: 0.4,
        avg_pause_time_ns: 0,
        p99_pause_time_ns: 0,
        throughput: 0.95,
        minor_gc_count: 10,
        major_gc_count: 2,
        promoted_objects: 100,
        collected_objects: 500,
    };

    gc.record_metrics(metrics);
    let _action = gc.tune();

    // 应该有一条调优记录
    let history = gc.tuning_history();
    assert!(history.len() > 0);
}

// ============================================================================
// 边界条件测试
// ============================================================================

#[test]
fn test_gc_performance_metrics_all_zeros() {
    let metrics = GCPerformanceMetrics {
        heap_size: 0,
        used_memory: 0,
        fragmentation_rate: 0.0,
        avg_pause_time_ns: 0,
        p99_pause_time_ns: 0,
        throughput: 0.0,
        minor_gc_count: 0,
        major_gc_count: 0,
        promoted_objects: 0,
        collected_objects: 0,
    };

    assert_eq!(metrics.compute_fragmentation(), 0.0);
    assert!(!metrics.is_high_memory_pressure());
    assert!(!metrics.is_high_fragmentation());
}

#[test]
fn test_gc_performance_metrics_full_usage() {
    let metrics = GCPerformanceMetrics {
        heap_size: 1000,
        used_memory: 1000, // 100%使用
        fragmentation_rate: 0.0,
        avg_pause_time_ns: 0,
        p99_pause_time_ns: 0,
        throughput: 0.0,
        minor_gc_count: 0,
        major_gc_count: 0,
        promoted_objects: 0,
        collected_objects: 0,
    };

    // 100%使用率应该触发高内存压力
    assert!(metrics.is_high_memory_pressure());
    // 碎片率为0
    assert_eq!(metrics.compute_fragmentation(), 0.0);
}

#[test]
fn test_gc_performance_metrics_extreme_pause() {
    let metrics = GCPerformanceMetrics {
        heap_size: 1000,
        used_memory: 700,
        fragmentation_rate: 0.2,
        avg_pause_time_ns: 0,
        p99_pause_time_ns: u64::MAX, // 极端暂停时间
        throughput: 0.95,
        minor_gc_count: 0,
        major_gc_count: 0,
        promoted_objects: 0,
        collected_objects: 0,
    };

    // 即使极端值也应该正常处理
    assert!(metrics.is_long_pause(10));
}

#[test]
fn test_adaptive_gc_config_boundary_values() {
    let config = AdaptiveGCConfig {
        heap_size: usize::MAX,
        nursery_ratio: 1.0,
        gc_threshold: 1.0,
        work_quota: usize::MAX,
        min_work_quota: 1,
        max_work_quota: usize::MAX,
        promotion_age: u8::MAX,
        promotion_ratio: 1.0,
        target_pause_time_ms: u64::MAX,
        target_throughput: 1.0,
        enable_compaction: true,
        compaction_threshold: 1.0,
    };

    // 边界值应该正常创建
    let gc = AdaptiveGCTuner::new(config);
    assert_eq!(gc.config().heap_size, usize::MAX);
}
