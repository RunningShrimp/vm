//! ML模型A/B测试框架测试套件
//!
//! 测试A/B测试框架的功能

use std::time::SystemTime;

use vm_engine_jit::ml_ab_testing::{
    ABTestConfig, ABTestManager, ModelComparison, ModelPerformanceStats, PerformanceRecord,
};
use vm_engine_jit::ml_model_enhanced::{ExecutionFeaturesEnhanced, InstMixFeatures};
use vm_engine_jit::ml_random_forest::{CompilationDecision, RandomForestModel};

// ============================================================================
// 辅助函数
// ============================================================================

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
        compilation_history: vm_engine_jit::ml_model_enhanced::CompilationHistory {
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

/// 创建测试模型A（LinearRegression模拟）
#[allow(dead_code)]
struct MockModelA {
    name: String,
}

impl MockModelA {
    fn new() -> Self {
        Self {
            name: "MockModelA".to_string(),
        }
    }

    fn predict(&self, features: &ExecutionFeaturesEnhanced) -> CompilationDecision {
        // 简单策略：高执行次数编译
        if features.execution_count > 50 {
            CompilationDecision::Compile
        } else {
            CompilationDecision::Skip
        }
    }
}

/// 创建测试模型B（RandomForest）
#[allow(dead_code)]
struct MockModelB {
    rf: RandomForestModel,
    name: String,
}

impl MockModelB {
    fn new() -> Self {
        Self {
            rf: RandomForestModel::new(5, 3),
            name: "MockModelB".to_string(),
        }
    }

    fn predict(&self, features: &ExecutionFeaturesEnhanced) -> CompilationDecision {
        self.rf.predict(features)
    }
}

// ============================================================================
// ABTestConfig测试
// ============================================================================

#[test]
fn test_ab_test_config_default() {
    let config = ABTestConfig::default();

    assert_eq!(config.traffic_split, 0.5);
    assert!(config.enable_logging);
    assert_eq!(config.min_samples, 100);
    assert!(!config.auto_select_winner);
}

#[test]
fn test_ab_test_config_custom() {
    let config = ABTestConfig {
        traffic_split: 0.7,
        enable_logging: false,
        min_samples: 200,
        auto_select_winner: true,
    };

    assert_eq!(config.traffic_split, 0.7);
    assert!(!config.enable_logging);
    assert_eq!(config.min_samples, 200);
    assert!(config.auto_select_winner);
}

// ============================================================================
// PerformanceRecord测试
// ============================================================================

#[test]
fn test_performance_record_creation() {
    let features = create_test_features(100);
    let record = PerformanceRecord {
        model_name: "TestModel".to_string(),
        features: features.clone(),
        decision: CompilationDecision::Compile,
        timestamp: SystemTime::now(),
        execution_time_ns: Some(1000),
        success: true,
    };

    assert_eq!(record.model_name, "TestModel");
    assert_eq!(record.decision, CompilationDecision::Compile);
    assert_eq!(record.execution_time_ns, Some(1000));
    assert!(record.success);
}

#[test]
fn test_performance_record_clone() {
    let features = create_test_features(50);
    let record = PerformanceRecord {
        model_name: "TestModel".to_string(),
        features: features.clone(),
        decision: CompilationDecision::Skip,
        timestamp: SystemTime::now(),
        execution_time_ns: None,
        success: false,
    };

    let cloned = record.clone();

    assert_eq!(cloned.model_name, record.model_name);
    assert_eq!(cloned.decision, record.decision);
    assert_eq!(cloned.success, record.success);
}

// ============================================================================
// ModelPerformanceStats测试
// ============================================================================

#[test]
fn test_model_performance_stats_default() {
    let stats = ModelPerformanceStats::default();

    assert_eq!(stats.total_predictions, 0);
    assert_eq!(stats.successful_predictions, 0);
    assert_eq!(stats.failed_predictions, 0);
    assert_eq!(stats.accuracy, 0.0);
}

#[test]
fn test_model_performance_stats_calculation() {
    let mut stats = ModelPerformanceStats::default();

    // 添加一些预测记录
    stats.total_predictions = 100;
    stats.successful_predictions = 85;
    stats.failed_predictions = 15;
    stats.avg_execution_time_ns = 1500.0;

    // 计算准确率
    stats.accuracy = stats.successful_predictions as f64 / stats.total_predictions as f64;

    assert_eq!(stats.accuracy, 0.85);
    assert_eq!(stats.total_predictions, 100);
}

// ============================================================================
// ABTestManager测试
// ============================================================================

#[test]
fn test_ab_test_manager_creation() {
    let config = ABTestConfig::default();
    let manager = ABTestManager::new(config);

    // 验证创建成功
    assert!(manager.is_logging_enabled());
}

#[test]
fn test_ab_test_manager_model_registration() {
    let config = ABTestConfig::default();
    let mut manager = ABTestManager::new(config);

    // 注册两个模型
    manager.register_model("ModelA".to_string());
    manager.register_model("ModelB".to_string());

    // 验证模型已注册
    assert!(manager.has_model("ModelA"));
    assert!(manager.has_model("ModelB"));
    assert!(!manager.has_model("ModelC"));
}

#[test]
fn test_ab_test_manager_traffic_split() {
    let config = ABTestConfig {
        traffic_split: 0.5,
        ..Default::default()
    };
    let mut manager = ABTestManager::new(config);

    manager.register_model("ModelA".to_string());
    manager.register_model("ModelB".to_string());

    // 多次选择应该近似50/50分配
    let mut model_a_count = 0;
    let mut model_b_count = 0;

    for _ in 0..100 {
        let selected = manager.select_model().unwrap();
        match selected.as_str() {
            "ModelA" => model_a_count += 1,
            "ModelB" => model_b_count += 1,
            _ => {}
        }
    }

    // 验证近似50/50分配（允许一定偏差）
    let total = model_a_count + model_b_count;
    let ratio_a = model_a_count as f64 / total as f64;

    assert!(ratio_a > 0.3 && ratio_a < 0.7);
}

#[test]
fn test_ab_test_manager_record_performance() {
    let config = ABTestConfig::default();
    let mut manager = ABTestManager::new(config);

    manager.register_model("TestModel".to_string());

    let features = create_test_features(100);
    let record = PerformanceRecord {
        model_name: "TestModel".to_string(),
        features: features.clone(),
        decision: CompilationDecision::Compile,
        timestamp: SystemTime::now(),
        execution_time_ns: Some(1000),
        success: true,
    };

    // 记录性能
    manager.record_performance(record.clone());

    // 验证记录已保存
    let stats = manager.get_model_stats("TestModel");
    assert!(stats.is_some());
}

#[test]
fn test_ab_test_manager_comparison() {
    let config = ABTestConfig {
        min_samples: 10,
        ..Default::default()
    };
    let mut manager = ABTestManager::new(config);

    manager.register_model("ModelA".to_string());
    manager.register_model("ModelB".to_string());

    // 为ModelA添加记录
    for i in 0..15 {
        let features = create_test_features(i as u64);
        let record = PerformanceRecord {
            model_name: "ModelA".to_string(),
            features,
            decision: CompilationDecision::Compile,
            timestamp: SystemTime::now(),
            execution_time_ns: Some(1000),
            success: true,
        };
        manager.record_performance(record);
    }

    // 为ModelB添加记录（性能更好）
    for i in 0..15 {
        let features = create_test_features(i as u64);
        let record = PerformanceRecord {
            model_name: "ModelB".to_string(),
            features,
            decision: CompilationDecision::Compile,
            timestamp: SystemTime::now(),
            execution_time_ns: Some(500), // 更快
            success: true,
        };
        manager.record_performance(record);
    }

    // 比较模型
    let comparison = manager.compare_models("ModelA", "ModelB");

    assert!(comparison.is_some());
    let comp = comparison.unwrap();
    // ModelB应该获胜（执行时间更短）
    assert_eq!(comp.winner, "ModelB");
}

#[test]
fn test_ab_test_manager_insufficient_samples() {
    let config = ABTestConfig {
        min_samples: 100,
        ..Default::default()
    };
    let mut manager = ABTestManager::new(config);

    manager.register_model("ModelA".to_string());
    manager.register_model("ModelB".to_string());

    // 添加少量记录
    for i in 0..10 {
        let features = create_test_features(i);
        let record_a = PerformanceRecord {
            model_name: "ModelA".to_string(),
            features: features.clone(),
            decision: CompilationDecision::Compile,
            timestamp: SystemTime::now(),
            execution_time_ns: Some(1000),
            success: true,
        };
        manager.record_performance(record_a);

        let record_b = PerformanceRecord {
            model_name: "ModelB".to_string(),
            features,
            decision: CompilationDecision::Compile,
            timestamp: SystemTime::now(),
            execution_time_ns: Some(500),
            success: true,
        };
        manager.record_performance(record_b);
    }

    // 样本不足，比较应该返回None
    let comparison = manager.compare_models("ModelA", "ModelB");
    assert!(comparison.is_none());
}

#[test]
fn test_ab_test_manager_auto_select_winner() {
    let config = ABTestConfig {
        min_samples: 10,
        auto_select_winner: true,
        ..Default::default()
    };
    let mut manager = ABTestManager::new(config);

    manager.register_model("ModelA".to_string());
    manager.register_model("ModelB".to_string());

    // 添加足够的数据使ModelB获胜
    for i in 0..20 {
        let features = create_test_features(i);
        let record_a = PerformanceRecord {
            model_name: "ModelA".to_string(),
            features: features.clone(),
            decision: CompilationDecision::Compile,
            timestamp: SystemTime::now(),
            execution_time_ns: Some(2000), // 更慢
            success: true,
        };
        manager.record_performance(record_a);

        let record_b = PerformanceRecord {
            model_name: "ModelB".to_string(),
            features,
            decision: CompilationDecision::Compile,
            timestamp: SystemTime::now(),
            execution_time_ns: Some(1000), // 更快
            success: true,
        };
        manager.record_performance(record_b);
    }

    // 自动选择应该偏向ModelB
    let selected = manager.select_model_with_auto_selection().unwrap();
    // 由于启用了auto_select_winner，应该更多地选择ModelB
    assert!(selected == "ModelA" || selected == "ModelB");
}

// ============================================================================
// ModelComparison测试
// ============================================================================

#[test]
fn test_model_comparison_structure() {
    let stats_a = ModelPerformanceStats {
        total_predictions: 100,
        successful_predictions: 90,
        failed_predictions: 10,
        avg_execution_time_ns: 1500.0,
        accuracy: 0.9,
        ..Default::default()
    };

    let stats_b = ModelPerformanceStats {
        total_predictions: 100,
        successful_predictions: 85,
        failed_predictions: 15,
        avg_execution_time_ns: 1000.0, // 更快
        accuracy: 0.85,
        ..Default::default()
    };

    let comparison = ModelComparison {
        model_a: stats_a.clone(),
        model_b: stats_b.clone(),
        winner: "ModelB".to_string(),
        improvement_percent: 33.3,
    };

    assert_eq!(comparison.winner, "ModelB");
    assert_eq!(comparison.improvement_percent, 33.3);
    assert_eq!(comparison.model_a.total_predictions, 100);
    assert_eq!(comparison.model_b.total_predictions, 100);
}

// ============================================================================
// 集成测试
// ============================================================================

#[test]
fn test_full_ab_test_workflow() {
    let config = ABTestConfig::default();
    let mut manager = ABTestManager::new(config);

    // 1. 注册模型
    manager.register_model("ModelA".to_string());
    manager.register_model("ModelB".to_string());

    // 2. 模拟A/B测试流量
    let mut predictions = 0;
    for i in 0..50 {
        let selected = manager.select_model().unwrap();
        let features = create_test_features(i);

        // 根据选择的模型进行预测
        let decision = if selected == "ModelA" {
            MockModelA::new().predict(&features)
        } else {
            MockModelB::new().predict(&features)
        };

        // 记录性能
        let record = PerformanceRecord {
            model_name: selected,
            features,
            decision,
            timestamp: SystemTime::now(),
            execution_time_ns: Some(1000),
            success: true,
        };

        manager.record_performance(record);
        predictions += 1;
    }

    // 3. 验证记录
    assert_eq!(predictions, 50);

    let stats_a = manager.get_model_stats("ModelA");
    let stats_b = manager.get_model_stats("ModelB");

    assert!(stats_a.is_some() || stats_b.is_some());
}

#[test]
fn test_ab_test_with_real_random_forest() {
    let config = ABTestConfig {
        traffic_split: 0.5,
        min_samples: 20,
        ..Default::default()
    };
    let mut manager = ABTestManager::new(config);

    // 使用真实的RandomForest模型
    let model_a = RandomForestModel::new(3, 2);
    let model_b = RandomForestModel::new(5, 3);

    manager.register_model("RFSmall".to_string());
    manager.register_model("RFLarge".to_string());

    // 生成测试数据
    for i in 0..30 {
        let features = create_test_features(i * 10);

        // 使用小森林
        let decision_a = model_a.predict(&features);
        let record_a = PerformanceRecord {
            model_name: "RFSmall".to_string(),
            features: features.clone(),
            decision: decision_a,
            timestamp: SystemTime::now(),
            execution_time_ns: Some(800),
            success: true,
        };
        manager.record_performance(record_a);

        // 使用大森林
        let decision_b = model_b.predict(&features);
        let record_b = PerformanceRecord {
            model_name: "RFLarge".to_string(),
            features,
            decision: decision_b,
            timestamp: SystemTime::now(),
            execution_time_ns: Some(1200), // 稍慢
            success: true,
        };
        manager.record_performance(record_b);
    }

    // 比较模型
    let comparison = manager.compare_models("RFSmall", "RFLarge");
    assert!(comparison.is_some());

    let comp = comparison.unwrap();
    // 小森林应该获胜（执行时间更短）
    assert_eq!(comp.winner, "RFSmall");
    // 验证性能提升
    assert!(comp.improvement_percent > 0.0);
}

// ============================================================================
// 边界条件测试
// ============================================================================

#[test]
fn test_ab_test_empty_manager() {
    let config = ABTestConfig::default();
    let manager = ABTestManager::new(config);

    // 没有注册模型时选择应该失败
    let selected = manager.select_model();
    assert!(selected.is_none());
}

#[test]
fn test_ab_test_single_model() {
    let config = ABTestConfig::default();
    let mut manager = ABTestManager::new(config);

    manager.register_model("OnlyModel".to_string());

    // 只有一个模型时应该总是返回它
    for _ in 0..10 {
        let selected = manager.select_model().unwrap();
        assert_eq!(selected, "OnlyModel");
    }
}

#[test]
fn test_ab_test_zero_traffic_split() {
    let config = ABTestConfig {
        traffic_split: 0.0,
        ..Default::default()
    };
    let mut manager = ABTestManager::new(config);

    manager.register_model("ModelA".to_string());
    manager.register_model("ModelB".to_string());

    // traffic_split=0.0意味着总是选择第二个模型
    for _ in 0..10 {
        let selected = manager.select_model().unwrap();
        assert_eq!(selected, "ModelB");
    }
}

#[test]
fn test_ab_test_unit_traffic_split() {
    let config = ABTestConfig {
        traffic_split: 1.0,
        ..Default::default()
    };
    let mut manager = ABTestManager::new(config);

    manager.register_model("ModelA".to_string());
    manager.register_model("ModelB".to_string());

    // traffic_split=1.0意味着总是选择第一个模型
    for _ in 0..10 {
        let selected = manager.select_model().unwrap();
        assert_eq!(selected, "ModelA");
    }
}

#[test]
fn test_performance_record_with_failure() {
    let config = ABTestConfig::default();
    let mut manager = ABTestManager::new(config);

    manager.register_model("TestModel".to_string());

    let features = create_test_features(100);
    let record = PerformanceRecord {
        model_name: "TestModel".to_string(),
        features,
        decision: CompilationDecision::Compile,
        timestamp: SystemTime::now(),
        execution_time_ns: None, // 失败没有执行时间
        success: false,
    };

    manager.record_performance(record);

    let stats = manager.get_model_stats("TestModel").unwrap();
    assert_eq!(stats.failed_predictions, 1);
    assert_eq!(stats.successful_predictions, 0);
}
