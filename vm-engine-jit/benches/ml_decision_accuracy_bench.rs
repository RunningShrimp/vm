//! ML Decision Accuracy Benchmarks
//!
//! Comprehensive benchmarks for ML-based JIT compilation decision accuracy:
//! - Model prediction accuracy
//! - Feature extraction performance
//! - Prediction latency
//! - Model size and memory usage
//! - Feature importance analysis

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use std::time::Duration;
use vm_engine_jit::ml_model_enhanced::{
    ExecutionFeaturesEnhanced, InstMixFeatures, CompilationHistory,
};
use vm_engine_jit::ml_random_forest::{RandomForestModel, CompilationDecision};
use vm_ir::{IRBlock, IRBuilder, IROp, Terminator};

// Test parameters
const BLOCK_SIZES: &[usize] = &[10, 50, 100, 500, 1000];
const EXECUTION_COUNTS: &[u64] = &[1, 10, 100, 1000, 10000];
const NUM_TREES: &[usize] = &[5, 10, 20, 50, 100];
const MAX_DEPTHS: &[usize] = &[3, 5, 10, 15, 20];

/// Create test IR block with specific characteristics
fn create_test_block(size: usize, _complexity: f64) -> IRBlock {
    let mut builder = IRBuilder::new(0x1000);

    for i in 0..size {
        let dst = (i % 16) as u32;
        let src1 = ((i + 1) % 16) as u32;
        let src2 = ((i + 2) % 16) as u32;

        // Mix different operation types
        match i % 5 {
            0 => builder.push(IROp::MovImm { dst, imm: i as u64 }),
            1 => builder.push(IROp::Add { dst, src1, src2 }),
            2 => builder.push(IROp::Mul { dst, src1, src2 }),
            3 => builder.push(IROp::Load {
                dst,
                base: src1,
                offset: (i * 8) as i64,
                size: 8,
                flags: vm_ir::MemFlags::default(),
            }),
            _ => builder.push(IROp::Store {
                src: dst,
                base: src1,
                offset: (i * 8) as i64,
                size: 8,
                flags: vm_ir::MemFlags::default(),
            }),
        }
    }

    builder.set_term(Terminator::Ret);
    builder.build()
}

/// Create test features with specific execution counts
fn create_test_features(block_size: usize, exec_count: u64, complexity: f64) -> ExecutionFeaturesEnhanced {
    let branch_count = (block_size as f64 * complexity * 0.2) as usize;
    let memory_access_count = (block_size as f64 * complexity * 0.3) as usize;

    ExecutionFeaturesEnhanced {
        block_size,
        instr_count: block_size,
        branch_count,
        memory_access_count,
        execution_count: exec_count,
        cache_hit_rate: 0.85 + (complexity * 0.1),
        instruction_mix: InstMixFeatures {
            arithmetic_ratio: 0.4,
            memory_ratio: 0.3,
            branch_ratio: 0.2,
            vector_ratio: 0.1,
        },
        control_flow_complexity: complexity,
        loop_nest_depth: (complexity * 3.0) as u8,
        data_locality: 0.75 + (complexity * 0.2),
        compilation_history: CompilationHistory {
            previous_compilations: exec_count as u32 / 100,
            avg_compilation_time: 1000.0,
            last_compile_benefit: 0.8,
        },
        register_pressure: 8.0,
        code_heat: exec_count as f64 / 10000.0,
    }
}

/// Benchmark ML model prediction accuracy
fn bench_ml_prediction_accuracy(c: &mut Criterion) {
    let mut group = c.benchmark_group("ml_prediction_accuracy");
    group.sample_size(100);

    // Create a model
    let model = RandomForestModel::new(20, 10);

    // Test accuracy on synthetic data
    let test_data: Vec<_> = BLOCK_SIZES
        .iter()
        .flat_map(|&size| {
            EXECUTION_COUNTS.iter().map(move |&exec_count| {
                create_test_features(size, exec_count, 0.7)
            })
        })
        .collect();

    group.bench_function("accuracy", |b| {
        b.iter(|| {
            let correct = test_data
                .iter()
                .filter(|features| {
                    let predicted = model.predict(black_box(features));
                    // Simple heuristic: compile if exec_count > 100 and block_size > 50
                    let expected = if features.execution_count > 100 && features.block_size > 50 {
                        CompilationDecision::Compile
                    } else {
                        CompilationDecision::Skip
                    };
                    predicted == expected
                })
                .count();

            let accuracy = correct as f64 / test_data.len() as f64;
            black_box(accuracy);
        });
    });

    group.finish();
}

/// Benchmark feature extraction performance
fn bench_feature_extraction(c: &mut Criterion) {
    let mut group = c.benchmark_group("feature_extraction");

    for &size in BLOCK_SIZES {
        group.throughput(Throughput::Elements(size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter(|| {
                black_box(create_test_features(size, 1000, 0.5));
            });
        });
    }

    group.finish();
}

/// Benchmark ML prediction latency
fn bench_ml_prediction_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("ml_prediction_latency");

    // Create a model
    let model = RandomForestModel::new(20, 10);

    // Test different feature complexities
    for &size in [10, 50, 100, 500].iter() {
        group.bench_with_input(
            BenchmarkId::new("predict", size),
            &size,
            |b, &size| {
                let features = create_test_features(size, 1000, 0.5);

                b.iter(|| {
                    black_box(model.predict(black_box(&features)));
                });
            },
        );
    }

    group.finish();
}

/// Benchmark model size impact
fn bench_model_size_impact(c: &mut Criterion) {
    let mut group = c.benchmark_group("model_size_impact");

    for &num_trees in NUM_TREES {
        group.bench_with_input(
            BenchmarkId::new("memory_usage", num_trees),
            &num_trees,
            |b, &num_trees| {
                b.iter(|| {
                    let model = RandomForestModel::new(num_trees, 10);
                    black_box(std::mem::size_of_val(&model));
                });
            },
        );
    }

    group.finish();
}

/// Benchmark feature importance analysis
fn bench_feature_importance(c: &mut Criterion) {
    let mut group = c.benchmark_group("feature_importance");

    let model = RandomForestModel::new(20, 10);

    group.bench_function("analysis", |b| {
        b.iter(|| {
            black_box(model.feature_importance());
        });
    });

    group.finish();
}

/// Benchmark model creation time
fn bench_model_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("model_creation");

    for &num_trees in NUM_TREES {
        for &max_depth in MAX_DEPTHS.iter().take(3) {
            group.bench_with_input(
                BenchmarkId::new("trees", (num_trees, max_depth)),
                &(num_trees, max_depth),
                |b, &(num_trees, max_depth)| {
                    b.iter(|| {
                        black_box(RandomForestModel::new(num_trees, max_depth));
                    });
                },
            );
        }
    }

    group.finish();
}

criterion_group! {
    name = ml_benches;
    config = Criterion::default()
        .sample_size(20)
        .measurement_time(Duration::from_secs(10))
        .warm_up_time(Duration::from_secs(5));
    targets =
        bench_ml_prediction_accuracy,
        bench_feature_extraction,
        bench_ml_prediction_latency,
        bench_model_size_impact,
        bench_feature_importance,
        bench_model_creation,
}

criterion_main!(ml_benches);
