//! 性能基线和回归测试
//!
//! 本模块定义性能基线,用于检测性能回归

use criterion::{black_box, Criterion};
use vm_engine::jit::core::{JITEngine, JITConfig};
use vm_ir::{IRBuilder, IROp, Terminator};
use std::time::Duration;

/// 性能基线结构
#[derive(Debug, Clone)]
pub struct PerformanceBaseline {
    /// 基线名称
    pub name: String,
    /// 平均时间（纳秒）
    pub avg_time_ns: u64,
    /// 标准差（纳秒）
    pub std_dev_ns: u64,
    /// 样本数
    pub sample_count: u64,
    /// 最小值（纳秒）
    pub min_ns: u64,
    /// 最大值（纳秒）
    pub max_ns: u64,
}

/// 定义性能基线
pub fn get_baselines() -> Vec<PerformanceBaseline> {
    vec![
        // JIT编译基线（100条指令）
        PerformanceBaseline {
            name: "jit_compile_100_instructions".to_string(),
            avg_time_ns: 50_000,     // 50微秒
            std_dev_ns: 10_000,      // 10微秒标准差
            sample_count: 100,
            min_ns: 30_000,
            max_ns: 80_000,
        },

        // JIT编译基线（1000条指令）
        PerformanceBaseline {
            name: "jit_compile_1000_instructions".to_string(),
            avg_time_ns: 500_000,    // 500微秒
            std_dev_ns: 100_000,     // 100微秒标准差
            sample_count: 100,
            min_ns: 300_000,
            max_ns: 800_000,
        },

        // TLB查找基线（缓存命中）
        PerformanceBaseline {
            name: "tlb_lookup_hit".to_string(),
            avg_time_ns: 50,          // 50纳秒
            std_dev_ns: 10,
            sample_count: 1000,
            min_ns: 30,
            max_ns: 100,
        },

        // TLB查找基线（缓存未命中）
        PerformanceBaseline {
            name: "tlb_lookup_miss".to_string(),
            avg_time_ns: 500,         // 500纳秒
            std_dev_ns: 100,
            sample_count: 1000,
            min_ns: 300,
            max_ns: 800,
        },

        // 翻译基线（100条指令）
        PerformanceBaseline {
            name: "translate_100_instructions".to_string(),
            avg_time_ns: 100_000,     // 100微秒
            std_dev_ns: 20_000,
            sample_count: 100,
            min_ns: 60_000,
            max_ns: 150_000,
        },

        // PGO编译基线（热路径）
        PerformanceBaseline {
            name: "pgo_compile_hot".to_string(),
            avg_time_ns: 1_000_000,   // 1毫秒
            std_dev_ns: 200_000,
            sample_count: 50,
            min_ns: 600_000,
            max_ns: 1_500_000,
        },

        // PGO编译基线（冷路径）
        PerformanceBaseline {
            name: "pgo_compile_cold".to_string(),
            avg_time_ns: 100_000,     // 100微秒
            std_dev_ns: 20_000,
            sample_count: 50,
            min_ns: 60_000,
            max_ns: 150_000,
        },
    ]
}

/// 性能回归检测阈值
#[derive(Debug, Clone)]
pub struct RegressionThreshold {
    /// 基线名称
    pub baseline_name: String,
    /// 允许的最大退化百分比（例如10表示允许10%的退化）
    pub max_regression_percent: f64,
    /// 允许的最大改善百分比（用于检测异常改善，可能是测试错误）
    pub max_improvement_percent: f64,
}

/// 获取回归检测阈值
pub fn get_regression_thresholds() -> Vec<RegressionThreshold> {
    vec![
        RegressionThreshold {
            baseline_name: "jit_compile_100_instructions".to_string(),
            max_regression_percent: 10.0,   // 允许10%退化
            max_improvement_percent: 50.0,   // 超过50%改善可能有问题
        },

        RegressionThreshold {
            baseline_name: "jit_compile_1000_instructions".to_string(),
            max_regression_percent: 10.0,
            max_improvement_percent: 50.0,
        },

        RegressionThreshold {
            baseline_name: "tlb_lookup_hit".to_string(),
            max_regression_percent: 15.0,   // TLB对延迟敏感，允许15%退化
            max_improvement_percent: 40.0,
        },

        RegressionThreshold {
            baseline_name: "tlb_lookup_miss".to_string(),
            max_regression_percent: 15.0,
            max_improvement_percent: 40.0,
        },

        RegressionThreshold {
            baseline_name: "translate_100_instructions".to_string(),
            max_regression_percent: 10.0,
            max_improvement_percent: 50.0,
        },

        RegressionThreshold {
            baseline_name: "pgo_compile_hot".to_string(),
            max_regression_percent: 15.0,   // PGO编译时间可能波动较大
            max_improvement_percent: 60.0,
        },

        RegressionThreshold {
            baseline_name: "pgo_compile_cold".to_string(),
            max_regression_percent: 10.0,
            max_improvement_percent: 50.0,
        },
    ]
}

/// 检查性能回归
pub fn check_regression(
    baseline_name: &str,
    measured_time_ns: u64,
) -> Result<(), String> {
    let baselines = get_baselines();
    let thresholds = get_regression_thresholds();

    // 查找基线
    let baseline = baselines
        .iter()
        .find(|b| b.name == baseline_name)
        .ok_or_else(|| format!("Baseline not found: {}", baseline_name))?;

    // 查找阈值
    let threshold = thresholds
        .iter()
        .find(|t| t.baseline_name == baseline_name)
        .ok_or_else(|| format!("Threshold not found: {}", baseline_name))?;

    // 计算与基线的差异百分比
    let baseline_time = baseline.avg_time_ns as f64;
    let measured_time = measured_time_ns as f64;
    let diff_percent = ((measured_time - baseline_time) / baseline_time) * 100.0;

    // 检查性能回归
    if diff_percent > threshold.max_regression_percent {
        return Err(format!(
            "PERFORMANCE REGRESSION: {} is {:.2}% slower than baseline \
             (baseline: {:.2}μs, measured: {:.2}μs)",
            baseline_name,
            diff_percent,
            baseline_time / 1000.0,
            measured_time / 1000.0
        ));
    }

    // 检查异常改善（可能是测试错误）
    if diff_percent < -threshold.max_improvement_percent {
        return Err(format!(
            "UNUSUAL IMPROVEMENT: {} is {:.2}% faster than baseline \
             (baseline: {:.2}μs, measured: {:.2}μs) - verify test correctness",
            baseline_name,
            diff_percent.abs(),
            baseline_time / 1000.0,
            measured_time / 1000.0
        ));
    }

    Ok(())
}

/// 运行基线测试
pub fn run_baseline_tests(c: &mut Criterion) {
    let mut group = c.benchmark_group("baseline");

    // JIT编译基线测试
    group.bench_function("jit_compile_100", |b| {
        let mut builder = IRBuilder::new(0x1000);
        for i in 0..100 {
            builder.push(IROp::Add {
                dst: (i % 16) as u32,
                src1: ((i + 1) % 16) as u32,
                src2: ((i + 2) % 16) as u32,
            });
        }
        builder.set_term(Terminator::Ret);
        let ir_block = builder.build();

        let config = JITConfig::default();
        let mut jit_engine = JITEngine::new(config);

        b.iter(|| {
            black_box(jit_engine.compile(black_box(&ir_block)));
        });
    });

    group.bench_function("jit_compile_1000", |b| {
        let mut builder = IRBuilder::new(0x1000);
        for i in 0..1000 {
            builder.push(IROp::Add {
                dst: (i % 16) as u32,
                src1: ((i + 1) % 16) as u32,
                src2: ((i + 2) % 16) as u32,
            });
        }
        builder.set_term(Terminator::Ret);
        let ir_block = builder.build();

        let config = JITConfig::default();
        let mut jit_engine = JITEngine::new(config);

        b.iter(|| {
            black_box(jit_engine.compile(black_box(&ir_block)));
        });
    });

    group.finish();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regression_detection_pass() {
        // 测试正常情况（不应该检测到回归）
        let result = check_regression("jit_compile_100_instructions", 55_000);
        assert!(result.is_ok(), "Should not detect regression for 10% slower");
    }

    #[test]
    fn test_regression_detection_fail() {
        // 测试性能回归
        let result = check_regression("jit_compile_100_instructions", 60_000);
        assert!(result.is_err(), "Should detect regression for 20% slower");
    }

    #[test]
    fn test_unusual_improvement() {
        // 测试异常改善
        let result = check_regression("jit_compile_100_instructions", 20_000);
        assert!(result.is_err(), "Should detect unusual improvement");
    }

    #[test]
    fn test_baseline_not_found() {
        let result = check_regression("nonexistent_baseline", 50_000);
        assert!(result.is_err(), "Should fail when baseline not found");
    }
}
