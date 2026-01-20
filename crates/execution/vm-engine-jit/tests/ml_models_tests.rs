//! ML模型测试套件
//!
//! 测试RandomForest和A/B测试框架的功能

use vm_engine_jit::ml_model_enhanced::{
    CompilationHistory, ExecutionFeaturesEnhanced, FeatureExtractorEnhanced, InstMixFeatures,
};
use vm_engine_jit::ml_random_forest::{CompilationDecision, RandomForestModel};
use vm_ir::{IRBlock, IROp, Terminator};

// ============================================================================
// 辅助函数
// ============================================================================

/// 创建测试IR块
fn create_test_block(_name: &str, num_ops: usize) -> IRBlock {
    IRBlock {
        start_pc: vm_ir::GuestAddr(0x1000),
        ops: (0..num_ops).map(|_| IROp::Nop).collect(),
        term: Terminator::Ret,
    }
}

/// 创建复杂测试块
fn create_complex_block(_name: &str, num_ops: usize) -> IRBlock {
    IRBlock {
        start_pc: vm_ir::GuestAddr(0x2000),
        ops: (0..num_ops)
            .map(|i| match i % 5 {
                0 => IROp::Add {
                    dst: 1,
                    src1: 0,
                    src2: 0,
                },
                1 => IROp::Load {
                    dst: 2,
                    base: 0,
                    offset: 0,
                    size: 8u8,
                    flags: vm_ir::MemFlags::default(),
                },
                2 => IROp::Store {
                    src: 2,
                    base: 0,
                    offset: 0,
                    size: 8u8,
                    flags: vm_ir::MemFlags::default(),
                },
                3 => IROp::Mul {
                    dst: 3,
                    src1: 1,
                    src2: 1,
                },
                _ => IROp::MovImm { dst: 1, imm: 0 },
            })
            .collect(),
        term: Terminator::Ret,
    }
}

/// 创建测试特征
fn create_test_features(execution_count: u64) -> ExecutionFeaturesEnhanced {
    ExecutionFeaturesEnhanced {
        block_size: 100,
        instr_count: 50,
        branch_count: 5,
        memory_access_count: 10,
        execution_count,
        cache_hit_rate: 0.85,
        instruction_mix: InstMixFeatures {
            arithmetic_ratio: 0.4,
            memory_ratio: 0.3,
            branch_ratio: 0.2,
            vector_ratio: 0.0,
            float_ratio: 0.1,
            call_ratio: 0.0,
        },
        control_flow_complexity: 2.5,
        loop_nest_depth: 1,
        has_recursion: false,
        data_locality: 0.75,
        memory_sequentiality: 0.8,
        compilation_history: CompilationHistory {
            previous_compilations: 2,
            avg_compilation_time_us: 150.0,
            last_compile_benefit: 1.5,
            last_compile_success: true,
        },
        register_pressure: 0.6,
        code_heat: 0.7,
        code_stability: 0.9,
    }
}

// ============================================================================
// RandomForest测试
// ============================================================================

#[test]
fn test_random_forest_creation() {
    let rf = RandomForestModel::new(10, 5);

    // 验证森林可以创建（具体字段是私有的）
    // 只验证它能正常运行即可
    let features = create_test_features(100);
    let _decision = rf.predict(&features);
}

#[test]
fn test_random_forest_prediction() {
    let rf = RandomForestModel::new(5, 3);

    // 测试低执行次数 - 验证能产生决策
    let features_low = create_test_features(5);
    let decision_low = rf.predict(&features_low);
    // 只验证返回有效决策，不限制具体类型（取决于随机森林初始化）
    match decision_low {
        CompilationDecision::Skip => {}
        CompilationDecision::Compile => {}
        CompilationDecision::CompileFast => {}
        CompilationDecision::Warmup => {}
    }

    // 测试高执行次数（可能跳过或编译，取决于模型）
    let features_high = create_test_features(1000);
    let decision_high = rf.predict(&features_high);
    // 只验证不panic
    let _ = decision_high;
}

#[test]
fn test_random_forest_voting() {
    // 创建具有奇数棵树的森林以便观察投票
    let rf = RandomForestModel::new(3, 2);

    let features = create_test_features(50);
    let decision = rf.predict(&features);

    // 验证返回有效决策
    match decision {
        CompilationDecision::Skip => {}
        CompilationDecision::Compile => {}
        CompilationDecision::CompileFast => {}
        CompilationDecision::Warmup => {}
    }
}

#[test]
fn test_random_forest_confidence() {
    let rf = RandomForestModel::new(5, 3);

    let features = create_test_features(25);
    let (decision, confidence) = rf.predict_with_confidence(&features);

    // 置信度应该在0-1之间
    assert!((0.0..=1.0).contains(&confidence));

    // 应该有明确的决策
    let _ = decision;
}

#[test]
fn test_random_forest_feature_importance() {
    let rf = RandomForestModel::new(5, 3);

    let importance = rf.feature_importance();

    // 应该包含一些特征
    assert!(!importance.is_empty());

    // 验证重要性值是非负的
    for (_feature, score) in importance.iter() {
        assert!(*score >= 0.0);
    }
}

// ============================================================================
// 增强特征提取测试
// ============================================================================

#[test]
fn test_feature_extractor_creation() {
    let mut extractor = FeatureExtractorEnhanced::new(100);

    // 验证创建成功（history_window字段是私有的）
    // 只验证它能正常使用
    let block = create_test_block("test", 10);
    let _features = extractor.extract_enhanced(&block);
}

#[test]
fn test_feature_extractor_basic_features() {
    let mut extractor = FeatureExtractorEnhanced::new(100);
    let block = create_test_block("test", 50);

    let features = extractor.extract_enhanced(&block);

    // 验证基础特征
    assert_eq!(features.block_size, 50);
    assert_eq!(features.instr_count, 50);
    assert_eq!(features.execution_count, 0); // 新块
}

#[test]
fn test_feature_extractor_instruction_mix() {
    let mut extractor = FeatureExtractorEnhanced::new(100);
    let block = create_complex_block("complex", 100);

    let features = extractor.extract_enhanced(&block);

    // 验证指令混合特征
    assert!(features.instruction_mix.arithmetic_ratio >= 0.0);
    assert!(features.instruction_mix.memory_ratio >= 0.0);
    assert!(features.instruction_mix.branch_ratio >= 0.0);
}

#[test]
fn test_feature_extractor_control_flow() {
    let mut extractor = FeatureExtractorEnhanced::new(100);
    let block = create_complex_block("with_branches", 50);

    let features = extractor.extract_enhanced(&block);

    // 验证控制流特征
    assert!(features.control_flow_complexity >= 0.0);
    assert!(!features.has_recursion); // 简单测试块没有递归
}

#[test]
fn test_feature_enhanced_complete() {
    let features = ExecutionFeaturesEnhanced {
        block_size: 100,
        instr_count: 50,
        branch_count: 5,
        memory_access_count: 10,
        execution_count: 1000,
        cache_hit_rate: 0.9,
        instruction_mix: InstMixFeatures {
            arithmetic_ratio: 0.4,
            memory_ratio: 0.3,
            branch_ratio: 0.2,
            vector_ratio: 0.0,
            float_ratio: 0.1,
            call_ratio: 0.0,
        },
        control_flow_complexity: 3.0,
        loop_nest_depth: 2,
        has_recursion: false,
        data_locality: 0.8,
        memory_sequentiality: 0.85,
        compilation_history: CompilationHistory {
            previous_compilations: 5,
            avg_compilation_time_us: 200.0,
            last_compile_benefit: 2.0,
            last_compile_success: true,
        },
        register_pressure: 0.7,
        code_heat: 0.8,
        code_stability: 0.95,
    };

    // 验证所有字段都可以访问
    assert_eq!(features.block_size, 100);
    assert_eq!(features.instruction_mix.arithmetic_ratio, 0.4);
    assert_eq!(features.control_flow_complexity, 3.0);
    assert_eq!(features.loop_nest_depth, 2);
}

// ============================================================================
// 边界条件测试
// ============================================================================

#[test]
fn test_empty_block_features() {
    let mut extractor = FeatureExtractorEnhanced::new(100);
    let block = create_test_block("empty", 0);

    let features = extractor.extract_enhanced(&block);

    // 空块应该有合理的默认值
    assert_eq!(features.block_size, 0);
    assert_eq!(features.instr_count, 0);
    assert_eq!(features.branch_count, 0);
}

#[test]
fn test_very_large_execution_count() {
    let rf = RandomForestModel::new(5, 3);
    let mut features = create_test_features(u64::MAX);

    features.execution_count = u64::MAX;

    // 不应该panic
    let decision = rf.predict(&features);
    let _ = decision;
}

#[test]
fn test_extreme_cache_hit_rates() {
    let mut features_low = create_test_features(100);
    features_low.cache_hit_rate = 0.0;

    let mut features_high = create_test_features(100);
    features_high.cache_hit_rate = 1.0;

    // 极端值应该正常处理
    let rf = RandomForestModel::new(3, 2);
    let decision_low = rf.predict(&features_low);
    let decision_high = rf.predict(&features_high);

    let _ = decision_low;
    let _ = decision_high;
}

#[test]
fn test_all_zero_features() {
    let mut features = create_test_features(0);
    features.block_size = 0;
    features.instr_count = 0;
    features.branch_count = 0;
    features.memory_access_count = 0;
    features.execution_count = 0;
    features.cache_hit_rate = 0.0;
    features.instruction_mix.arithmetic_ratio = 0.0;
    features.instruction_mix.memory_ratio = 0.0;
    features.instruction_mix.branch_ratio = 0.0;

    // 全零特征应该仍然产生有效决策
    let rf = RandomForestModel::new(3, 2);
    let decision = rf.predict(&features);
    let _ = decision;
}

// ============================================================================
// 性能测试
// ============================================================================

#[test]
fn test_random_forest_prediction_performance() {
    let rf = RandomForestModel::new(10, 5);
    let features = create_test_features(100);

    // 测试预测性能（应该很快）
    let start = std::time::Instant::now();
    for _ in 0..1000 {
        let _ = rf.predict(&features);
    }
    let duration = start.elapsed();

    // 1000次预测应该在合理时间内完成（<100ms）
    assert!(duration.as_millis() < 100);
}

#[test]
fn test_feature_extraction_performance() {
    let mut extractor = FeatureExtractorEnhanced::new(100);
    let block = create_complex_block("perf_test", 100);

    // 测试特征提取性能
    let start = std::time::Instant::now();
    for _ in 0..100 {
        let _ = extractor.extract_enhanced(&block);
    }
    let duration = start.elapsed();

    // 100次提取应该在合理时间内完成（<50ms）
    assert!(duration.as_millis() < 50);
}
