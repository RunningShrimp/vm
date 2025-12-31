//! ML模型测试套件
//!
//! 测试RandomForest和A/B测试框架的功能

use vm_engine_jit::ml_model_enhanced::{
    CompilationHistory, ExecutionFeaturesEnhanced, FeatureExtractorEnhanced, InstMixFeatures,
};
use vm_engine_jit::ml_random_forest::{CompilationDecision, RandomForestModel, TreeNode};
use vm_ir::{IRBlock, IROp, Terminator};

// ============================================================================
// 辅助函数
// ============================================================================

/// 创建测试IR块
fn create_test_block(name: &str, num_ops: usize) -> IRBlock {
    IRBlock {
        name: name.to_string(),
        instructions: (0..num_ops)
            .map(|_| IROp::Nop)
            .collect(),
        terminator: Terminator::Ret { value: None },
    }
}

/// 创建复杂测试块
fn create_complex_block(name: &str, num_ops: usize) -> IRBlock {
    IRBlock {
        name: name.to_string(),
        instructions: (0..num_ops)
            .map(|i| {
                match i % 5 {
                    0 => IROp::IntAdd {
                        dest: vm_ir::Value::Register(1),
                        lhs: vm_ir::Value::Register(0),
                        rhs: vm_ir::Value::Immediate(1),
                    },
                    1 => IROp::Load {
                        dest: vm_ir::Value::Register(2),
                        addr: vm_ir::Value::Register(0),
                        size: vm_ir::MemSize::U64,
                    },
                    2 => IROp::Store {
                        addr: vm_ir::Value::Register(0),
                        value: vm_ir::Value::Register(2),
                        size: vm_ir::MemSize::U64,
                    },
                    3 => IROp::IntMul {
                        dest: vm_ir::Value::Register(3),
                        lhs: vm_ir::Value::Register(1),
                        rhs: vm_ir::Value::Immediate(2),
                    },
                    _ => IROp::BranchEqual {
                        lhs: vm_ir::Value::Register(1),
                        rhs: vm_ir::Value::Immediate(0),
                        target: "loop_end".to_string(),
                    },
                }
            })
            .collect(),
        terminator: Terminator::Ret { value: None },
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
// 决策树测试
// ============================================================================

#[test]
fn test_decision_tree_leaf_node() {
    let leaf = TreeNode::Leaf {
        decision: CompilationDecision::Compile,
        confidence: 0.95,
    };

    let features = create_test_features(100);
    let decision = leaf.predict(&features);

    assert_eq!(decision, CompilationDecision::Compile);
}

#[test]
fn test_decision_tree_internal_node() {
    // 创建简单的决策树：execution_count < 50 -> Compile, else Skip
    let tree = TreeNode::Internal {
        feature: "execution_count".to_string(),
        threshold: 50.0,
        left: Box::new(TreeNode::Leaf {
            decision: CompilationDecision::Compile,
            confidence: 0.9,
        }),
        right: Box::new(TreeNode::Leaf {
            decision: CompilationDecision::Skip,
            confidence: 0.8,
        }),
    };

    // 测试左分支（低执行次数）
    let features_low = create_test_features(10);
    assert_eq!(tree.predict(&features_low), CompilationDecision::Compile);

    // 测试右分支（高执行次数）
    let features_high = create_test_features(100);
    assert_eq!(tree.predict(&features_high), CompilationDecision::Skip);
}

#[test]
fn test_decision_tree_feature_extraction() {
    let features = create_test_features(100);

    // 测试各种特征提取
    assert_eq!(TreeNode::extract_feature(&features, "execution_count"), 100.0);
    assert_eq!(TreeNode::extract_feature(&features, "block_size"), 100.0);
    assert_eq!(TreeNode::extract_feature(&features, "branch_count"), 5.0);
    assert_eq!(TreeNode::extract_feature(&features, "cache_hit_rate"), 0.85);
    assert_eq!(TreeNode::extract_feature(&features, "arithmetic_ratio"), 0.4);
}

#[test]
fn test_decision_tree_feature_importance() {
    // 创建复杂树
    let tree = TreeNode::Internal {
        feature: "execution_count".to_string(),
        threshold: 50.0,
        left: Box::new(TreeNode::Internal {
            feature: "block_size".to_string(),
            threshold: 100.0,
            left: Box::new(TreeNode::Leaf {
                decision: CompilationDecision::Compile,
                confidence: 0.9,
            }),
            right: Box::new(TreeNode::Leaf {
                decision: CompilationDecision::CompileFast,
                confidence: 0.8,
            }),
        }),
        right: Box::new(TreeNode::Leaf {
            decision: CompilationDecision::Skip,
            confidence: 0.7,
        }),
    };

    let importance = tree.feature_importance();

    // execution_count应该有最高重要性（根节点）
    assert!(importance.get("execution_count").unwrap_or(&0.0) > &0.0);
    assert!(importance.get("block_size").unwrap_or(&0.0) > &0.0);
}

// ============================================================================
// RandomForest测试
// ============================================================================

#[test]
fn test_random_forest_creation() {
    let rf = RandomForestModel::new(10, 5);

    // 验证树的数量
    assert_eq!(rf.num_trees, 10);
}

#[test]
fn test_random_forest_prediction() {
    let rf = RandomForestModel::new(5, 3);

    // 测试低执行次数（应该编译）
    let features_low = create_test_features(5);
    let decision_low = rf.predict(&features_low);
    assert!(matches!(decision_low, CompilationDecision::Compile));

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
    assert!(confidence >= 0.0 && confidence <= 1.0);

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
    let extractor = FeatureExtractorEnhanced::new(100);

    // 验证创建成功
    assert_eq!(extractor.history_window, 100);
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
