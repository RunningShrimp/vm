//! Round 35-37 性能验证基准测试
//!
//! 验证ARM64优化和自动优化系统的实际性能提升
//!
//! 测试内容:
//! 1. NEON intrinsic优化 (Round 35)
//! 2. 16字节内存对齐 (Round 35)
//! 3. AutoOptimizer自动优化 (Round 36)
//! 4. RealTimeMonitor监控开销 (Round 37)

use std::time::{Duration, Instant};

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};

// Round 35: NEON优化测试
#[cfg(target_arch = "aarch64")]
use vm_mem::simd::neon_optimized::{vec_add_f32, vec_mul_f32, vec_fma_f32, vec_dot_f32};

// Round 36: AutoOptimizer
use vm_core::optimization::{AutoOptimizer, PerformanceMetrics, WorkloadType};

// Round 37: RealTimeMonitor
use vm_monitor::{RealTimeMetrics, RealTimeMonitor};

/// Round 35: NEON向量加法性能测试
#[cfg(target_arch = "aarch64")]
fn benchmark_neon_vec_add(c: &mut Criterion) {
    let mut group = c.benchmark_group("neon_vec_add");

    for size in [4, 16, 64, 256, 1024].iter() {
        let a: Vec<f32> = (0..*size).map(|i| i as f32).collect();
        let b: Vec<f32> = (0..*size).map(|i| i as f32 * 2.0).collect();

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                black_box(vec_add_f32(black_box(&a), black_box(&b)));
            });
        });
    }

    group.finish();
}

/// Round 35: 标量向量加法 (对比基准)
fn benchmark_scalar_vec_add(c: &mut Criterion) {
    let mut group = c.benchmark_group("scalar_vec_add");

    for size in [4, 16, 64, 256, 1024].iter() {
        let a: Vec<f32> = (0..*size).map(|i| i as f32).collect();
        let b: Vec<f32> = (0..*size).map(|i| i as f32 * 2.0).collect();

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                black_box(
                    a.iter()
                        .zip(b.iter())
                        .map(|(x, y)| x + y)
                        .collect::<Vec<f32>>()
                );
            });
        });
    }

    group.finish();
}

/// Round 35: NEON向量乘法性能测试
#[cfg(target_arch = "aarch64")]
fn benchmark_neon_vec_mul(c: &mut Criterion) {
    let mut group = c.benchmark_group("neon_vec_mul");

    for size in [4, 16, 64, 256].iter() {
        let a: Vec<f32> = (0..*size).map(|i| i as f32).collect();
        let b: Vec<f32> = (0..*size).map(|i| i as f32 * 2.0).collect();

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                black_box(vec_mul_f32(black_box(&a), black_box(&b)));
            });
        });
    }

    group.finish();
}

/// Round 35: NEON融合乘加 (FMA) 性能测试
#[cfg(target_arch = "aarch64")]
fn benchmark_neon_vec_fma(c: &mut Criterion) {
    let mut group = c.benchmark_group("neon_vec_fma");

    for size in [4, 16, 64, 256].iter() {
        let a: Vec<f32> = (0..*size).map(|i| i as f32).collect();
        let b: Vec<f32> = (0..*size).map(|i| i as f32 * 2.0).collect();
        let c: Vec<f32> = (0..*size).map(|i| i as f32 * 3.0).collect();

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                black_box(vec_fma_f32(black_box(&a), black_box(&b), black_box(&c)));
            });
        });
    }

    group.finish();
}

/// Round 35: NEON点积性能测试
#[cfg(target_arch = "aarch64")]
fn benchmark_neon_vec_dot(c: &mut Criterion) {
    let mut group = c.benchmark_group("neon_vec_dot");

    for size in [4, 16, 64, 256, 1024].iter() {
        let a: Vec<f32> = (0..*size).map(|i| i as f32).collect();
        let b: Vec<f32> = (0..*size).map(|i| i as f32 * 2.0).collect();

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                black_box(vec_dot_f32(black_box(&a), black_box(&b)));
            });
        });
    }

    group.finish();
}

/// Round 35: 16字节对齐内存性能测试
#[cfg(target_arch = "aarch64")]
fn benchmark_aligned_memory(c: &mut Criterion) {
    use vm_mem::memory::aligned::SimdAlignedVector4;

    let mut group = c.benchmark_group("aligned_memory");

    // 测试对齐 vs 未对齐的加法性能
    let v1_aligned = SimdAlignedVector4::new([1.0, 2.0, 3.0, 4.0]);
    let v2_aligned = SimdAlignedVector4::new([5.0, 6.0, 7.0, 8.0]);

    let v1_unaligned: [f32; 4] = [1.0, 2.0, 3.0, 4.0];
    let v2_unaligned: [f32; 4] = [5.0, 6.0, 7.0, 8.0];

    group.bench_function("aligned_add_neon", |b| {
        b.iter(|| {
            black_box(v1_aligned.add_neon(black_box(&v2_aligned)));
        });
    });

    group.bench_function("unaligned_add_scalar", |b| {
        b.iter(|| {
            let mut result = [0.0f32; 4];
            for i in 0..4 {
                result[i] = v1_unaligned[i] + v2_unaligned[i];
            }
            black_box(result);
        });
    });

    group.finish();
}

/// Round 36: AutoOptimizer性能开销测试
fn benchmark_auto_optimizer(c: &mut Criterion) {
    let mut group = c.benchmark_group("auto_optimizer");

    // 测试指标记录开销
    let optimizer = AutoOptimizer::new();

    group.bench_function("record_metrics", |b| {
        b.iter(|| {
            let metrics = PerformanceMetrics {
                timestamp_ns: 0,
                operation_time_ns: 10_000,
                memory_used_bytes: 1024,
                cpu_usage_percent: 60.0,
                cache_hit_rate: Some(0.8),
            };
            black_box(optimizer.record_metrics(black_box(metrics)));
        });
    });

    // 测试分析开销
    group.bench_function("analyze_and_optimize", |b| {
        // 预先记录100个指标
        for i in 0..100 {
            let metrics = PerformanceMetrics {
                timestamp_ns: i * 1_000_000,
                operation_time_ns: 10_000 + i as u64 * 100,
                memory_used_bytes: 1024,
                cpu_usage_percent: 60.0,
                cache_hit_rate: Some(0.8),
            };
            optimizer.record_metrics(metrics);
        }

        b.iter(|| {
            black_box(optimizer.analyze_and_optimize());
        });
    });

    group.finish();
}

/// Round 37: RealTimeMonitor性能开销测试
fn benchmark_real_time_monitor(c: &mut Criterion) {
    let mut group = c.benchmark_group("real_time_monitor");

    // 测试指标记录开销
    let monitor = RealTimeMonitor::new();

    group.bench_function("record_metric", |b| {
        b.iter(|| {
            let metrics = RealTimeMetrics {
                timestamp_ns: 0,
                operation_type: "test".to_string(),
                latency_ns: 10_000,
                memory_bytes: 1024,
                cpu_percent: 60.0,
                throughput_ops_per_sec: 100_000.0,
            };
            black_box(monitor.record_metric(black_box(metrics)));
        });
    });

    // 测试100条记录的开销 (触发窗口更新)
    group.bench_function("record_100_metrics", |b| {
        b.iter(|| {
            for i in 0..100 {
                let metrics = RealTimeMetrics {
                    timestamp_ns: i * 1_000_000,
                    operation_type: "test".to_string(),
                    latency_ns: 10_000 + i * 100,
                    memory_bytes: 1024,
                    cpu_percent: 60.0,
                    throughput_ops_per_sec: 100_000.0,
                };
                monitor.record_metric(metrics);
            }
        });
    });

    group.finish();
}

/// Round 37: 监控系统开销对比 (有监控 vs 无监控)
fn benchmark_monitoring_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("monitoring_overhead");

    // 无监控的基准操作
    group.bench_function("without_monitor", |b| {
        b.iter(|| {
            let mut result = 0.0f64;
            for i in 0..1000 {
                result += (i as f64).sqrt();
            }
            black_box(result);
        });
    });

    // 有监控的操作
    let monitor = RealTimeMonitor::new();
    group.bench_function("with_monitor", |b| {
        b.iter(|| {
            let start = Instant::now();
            let mut result = 0.0f64;
            for i in 0..1000 {
                result += (i as f64).sqrt();
            }
            let latency = start.elapsed().as_nanos() as u64;

            let metrics = RealTimeMetrics {
                timestamp_ns: 0,
                operation_type: "computation".to_string(),
                latency_ns: latency,
                memory_bytes: 0,
                cpu_percent: 0.0,
                throughput_ops_per_sec: 0.0,
            };
            monitor.record_metric(metrics);

            black_box(result);
        });
    });

    group.finish();
}

/// 综合性能测试: 计算密集型工作负载
fn benchmark_compute_intensive_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("compute_intensive");

    let optimizer = AutoOptimizer::new();
    let monitor = RealTimeMonitor::new();

    // 纯计算
    group.bench_function("pure_computation", |b| {
        b.iter(|| {
            let mut result = 0.0f64;
            for i in 0..10000 {
                result += (i as f64).sin().cos().tan();
            }
            black_box(result);
        });
    });

    // 计算优化
    #[cfg(target_arch = "aarch64")]
    group.bench_function("computation_with_neon", |b| {
        b.iter(|| {
            let size = 1024;
            let a: Vec<f32> = (0..size).map(|i| i as f32).collect();
            let b: Vec<f32> = (0..size).map(|i| i as f32 * 2.0).collect();
            let c: Vec<f32> = (0..size).map(|i| i as f32 * 3.0).collect();

            black_box(vec_fma_f32(&a, &b, &c));
        });
    });

    // 计算监控
    group.bench_function("computation_with_monitoring", |b| {
        b.iter(|| {
            let start = Instant::now();
            let mut result = 0.0f64;
            for i in 0..10000 {
                result += (i as f64).sin().cos().tan();
            }
            let latency = start.elapsed().as_nanos() as u64;

            let metrics = RealTimeMetrics {
                timestamp_ns: 0,
                operation_type: "compute".to_string(),
                latency_ns: latency,
                memory_bytes: 0,
                cpu_percent: 100.0,
                throughput_ops_per_sec: 10_000.0 / (latency as f64 / 1_000_000_000.0),
            };
            monitor.record_metric(metrics);

            black_box(result);
        });
    });

    group.finish();
}

/// 综合性能测试: 内存密集型工作负载
fn benchmark_memory_intensive_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_intensive");

    let monitor = RealTimeMonitor::new();

    // 内存拷贝
    group.bench_function("memory_copy_1kb", |b| {
        let data: Vec<u8> = (0..1024).map(|i| i as u8).collect();

        b.iter(|| {
            let mut copy = vec![0u8; 1024];
            copy.copy_from_slice(black_box(&data));
            black_box(copy);
        });
    });

    group.bench_function("memory_copy_with_monitoring", |b| {
        let data: Vec<u8> = (0..1024).map(|i| i as u8).collect();

        b.iter(|| {
            let start = Instant::now();
            let mut copy = vec![0u8; 1024];
            copy.copy_from_slice(black_box(&data));
            let latency = start.elapsed().as_nanos() as u64;

            let metrics = RealTimeMetrics {
                timestamp_ns: 0,
                operation_type: "memory_copy".to_string(),
                latency_ns: latency,
                memory_bytes: 1024,
                cpu_percent: 20.0,
                throughput_ops_per_sec: 1.0,
            };
            monitor.record_metric(metrics);

            black_box(copy);
        });
    });

    group.finish();
}

#[cfg(target_arch = "aarch64")]
criterion_group!(
    name = benches;
    config = Criterion::default().measurement_time(Duration::from_secs(10));
    targets =
        benchmark_neon_vec_add,
        benchmark_scalar_vec_add,
        benchmark_neon_vec_mul,
        benchmark_neon_vec_fma,
        benchmark_neon_vec_dot,
        benchmark_aligned_memory,
        benchmark_auto_optimizer,
        benchmark_real_time_monitor,
        benchmark_monitoring_overhead,
        benchmark_compute_intensive_workload,
        benchmark_memory_intensive_workload
);

#[cfg(not(target_arch = "aarch64"))]
criterion_group!(
    name = benches;
    config = Criterion::default().measurement_time(Duration::from_secs(10));
    targets =
        benchmark_scalar_vec_add,
        benchmark_auto_optimizer,
        benchmark_real_time_monitor,
        benchmark_monitoring_overhead,
        benchmark_compute_intensive_workload,
        benchmark_memory_intensive_workload
);

criterion_main!(benches);
