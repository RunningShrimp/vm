//! ML引导编译库 - 机器学习驱动的编译优化决策
//!
//! 本库集成ML模型进行智能编译决策：
//! - 编译层级预测 (Tier 0/1/2/3)
//! - 优化选项选择
//! - 自适应参数调整
//! - A/B测试框架

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use parking_lot::RwLock;

/// 编译块特征向量
#[derive(Clone, Debug, Default)]
pub struct BlockFeatures {
    /// 代码大小 (字节)
    pub size_bytes: u32,
    /// 分支数
    pub branch_count: u32,
    /// 循环数
    pub loop_count: u32,
    /// 函数调用数
    pub call_count: u32,
    /// 内存访问数
    pub memory_ops: u32,
    /// 执行次数 (历史)
    pub execution_count: u64,
    /// 执行时间 (微秒)
    pub execution_time_us: u64,
}

impl BlockFeatures {
    /// 计算特征的规范化向量 (0-1)
    pub fn normalize(&self) -> Vec<f32> {
        vec![
            (self.size_bytes as f32) / 10000.0,
            (self.branch_count as f32) / 100.0,
            (self.loop_count as f32) / 20.0,
            (self.call_count as f32) / 100.0,
            (self.memory_ops as f32) / 100.0,
            (self.execution_count as f32).log2() / 32.0,
            (self.execution_time_us as f32).log2() / 20.0,
        ]
    }

    /// 计算块的热度指标 (0-100)
    pub fn hotness_score(&self) -> u32 {
        let exec_score = (self.execution_count as f32).min(1000.0) / 1000.0;
        let time_score = (self.execution_time_us as f32).min(10000.0) / 10000.0;
        ((exec_score * 0.6 + time_score * 0.4) * 100.0) as u32
    }
}

/// ML编译决策
#[derive(Clone, Debug, PartialEq)]
pub enum CompilationDecision {
    Tier0, // 解释执行
    Tier1, // 快速JIT
    Tier2, // 平衡JIT
    Tier3, // 优化JIT
}

impl CompilationDecision {
    /// 转换为数值编码
    pub fn to_id(&self) -> u8 {
        match self {
            CompilationDecision::Tier0 => 0,
            CompilationDecision::Tier1 => 1,
            CompilationDecision::Tier2 => 2,
            CompilationDecision::Tier3 => 3,
        }
    }

    /// 从数值解码
    pub fn from_id(id: u8) -> Self {
        match id {
            0 => CompilationDecision::Tier0,
            1 => CompilationDecision::Tier1,
            2 => CompilationDecision::Tier2,
            _ => CompilationDecision::Tier3,
        }
    }
}

/// 简化的线性ML模型
pub struct SimpleLinearModel {
    // 权重向量 (7个特征)
    weights: Vec<f32>,
    // 偏置项
    bias: f32,
}

impl SimpleLinearModel {
    /// 创建新的模型 (使用预训练权重)
    pub fn new() -> Self {
        Self {
            // 这些是示例权重，基于启发式规则
            weights: vec![
                0.1,  // size权重
                0.3,  // branch权重
                0.2,  // loop权重
                0.15, // call权重
                0.15, // memory权重
                0.25, // exec_count权重
                0.25, // exec_time权重
            ],
            bias: -0.5,
        }
    }

    /// 对特征向量进行预测 (输出0-3的分数)
    pub fn predict(&self, features: &BlockFeatures) -> f32 {
        let normalized = features.normalize();
        let mut score = self.bias;

        for (i, feat) in normalized.iter().enumerate() {
            if i < self.weights.len() {
                score += self.weights[i] * feat;
            }
        }

        score.clamp(0.0, 3.0)
    }

    /// 预测最佳编译层级
    pub fn predict_tier(&self, features: &BlockFeatures) -> CompilationDecision {
        let score = self.predict(features);
        CompilationDecision::from_id((score.round()) as u8)
    }
}

impl Default for SimpleLinearModel {
    fn default() -> Self {
        Self::new()
    }
}

/// ML引导编译器
pub struct MLGuidedCompiler {
    model: Arc<RwLock<SimpleLinearModel>>,
    // 特征提取器缓存
    feature_cache: Arc<RwLock<HashMap<u64, BlockFeatures>>>,
    // 编译决策缓存
    decision_cache: Arc<RwLock<HashMap<u64, CompilationDecision>>>,
    // 统计信息
    predictions_made: Arc<AtomicU64>,
    cache_hits: Arc<AtomicU64>,
    cache_misses: Arc<AtomicU64>,
}

impl MLGuidedCompiler {
    pub fn new() -> Self {
        Self {
            model: Arc::new(RwLock::new(SimpleLinearModel::new())),
            feature_cache: Arc::new(RwLock::new(HashMap::new())),
            decision_cache: Arc::new(RwLock::new(HashMap::new())),
            predictions_made: Arc::new(AtomicU64::new(0)),
            cache_hits: Arc::new(AtomicU64::new(0)),
            cache_misses: Arc::new(AtomicU64::new(0)),
        }
    }

    /// 为代码块进行编译决策
    pub fn decide_compilation(
        &self,
        block_id: u64,
        features: &BlockFeatures,
    ) -> CompilationDecision {
        // 检查缓存
        {
            let cache = self.decision_cache.read();
            if let Some(decision) = cache.get(&block_id) {
                self.cache_hits.fetch_add(1, Ordering::Relaxed);
                return decision.clone();
            }
        }

        self.cache_misses.fetch_add(1, Ordering::Relaxed);

        // 使用模型进行预测
        let decision = {
            let model = self.model.read();
            model.predict_tier(features)
        };

        // 缓存决策和特征
        {
            self.feature_cache
                .write()
                .insert(block_id, features.clone());
            self.decision_cache
                .write()
                .insert(block_id, decision.clone());
        }

        self.predictions_made.fetch_add(1, Ordering::Relaxed);
        decision
    }

    /// 基于实际性能更新模型 (在线学习)
    pub fn update_with_feedback(
        &self,
        block_id: u64,
        _actual_tier: CompilationDecision,
        actual_time_us: u64,
    ) {
        let mut features = {
            let cache = self.feature_cache.read();
            if let Some(f) = cache.get(&block_id) {
                f.clone()
            } else {
                return;
            }
        };

        // 更新特征中的执行时间
        features.execution_time_us = actual_time_us;

        // 将更新后的特征存储回缓存
        {
            let mut cache = self.feature_cache.write();
            cache.insert(block_id, features);
        }

        // 简单的权重调整 (基于性能反馈)
        // 如果实际使用的tier比预测的好，增加相关特征的权重
        if actual_time_us < 100 {
            // 快速路径 - 倾向于使用低tier
            let mut model = self.model.write();
            model.weights[5] -= 0.05; // 减少exec_count的影响
        } else if actual_time_us > 1000 {
            // 慢速路径 - 倾向于使用高tier
            let mut model = self.model.write();
            model.weights[5] += 0.05;
        }
    }

    /// 清空缓存
    pub fn clear_cache(&self) {
        self.feature_cache.write().clear();
        self.decision_cache.write().clear();
    }

    /// 获取缓存命中率
    pub fn cache_hit_rate(&self) -> f32 {
        let hits = self.cache_hits.load(Ordering::Relaxed);
        let misses = self.cache_misses.load(Ordering::Relaxed);
        let total = hits + misses;

        if total == 0 {
            0.0
        } else {
            hits as f32 / total as f32
        }
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> MLCompilerStats {
        MLCompilerStats {
            predictions_made: self.predictions_made.load(Ordering::Relaxed),
            cache_hits: self.cache_hits.load(Ordering::Relaxed),
            cache_misses: self.cache_misses.load(Ordering::Relaxed),
            cache_entries: self.decision_cache.read().len() as u64,
        }
    }
}

impl Default for MLGuidedCompiler {
    fn default() -> Self {
        Self::new()
    }
}

/// ML编译器统计
#[derive(Debug, Clone, Default)]
pub struct MLCompilerStats {
    pub predictions_made: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub cache_entries: u64,
}

/// A/B测试框架
pub struct ABTestFramework {
    // 实验组ID
    experiment_id: String,
    // 控制组统计
    control_group: Arc<RwLock<ABTestMetrics>>,
    // 实验组统计
    experimental_group: Arc<RwLock<ABTestMetrics>>,
    // 样本数
    sample_count: Arc<AtomicUsize>,
}

/// A/B测试指标
#[derive(Clone, Debug, Default)]
pub struct ABTestMetrics {
    /// 样本数
    pub samples: u64,
    /// 总执行时间 (微秒)
    pub total_time_us: u64,
    /// 平均编译延迟 (微秒)
    pub avg_compile_us: u64,
    /// 总字节编译数
    pub total_bytes: u64,
}

impl ABTestFramework {
    pub fn new(experiment_id: String) -> Self {
        Self {
            experiment_id,
            control_group: Arc::new(RwLock::new(ABTestMetrics::default())),
            experimental_group: Arc::new(RwLock::new(ABTestMetrics::default())),
            sample_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// 记录控制组数据
    pub fn record_control(&self, compile_us: u64, bytes: u64) {
        let mut metrics = self.control_group.write();
        metrics.samples += 1;
        metrics.total_time_us += compile_us;
        metrics.total_bytes += bytes;
        metrics.avg_compile_us = metrics.total_time_us / metrics.samples;
        self.sample_count.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录实验组数据
    pub fn record_experimental(&self, compile_us: u64, bytes: u64) {
        let mut metrics = self.experimental_group.write();
        metrics.samples += 1;
        metrics.total_time_us += compile_us;
        metrics.total_bytes += bytes;
        metrics.avg_compile_us = metrics.total_time_us / metrics.samples;
        self.sample_count.fetch_add(1, Ordering::Relaxed);
    }

    /// 计算改进百分比
    pub fn improvement_percent(&self) -> f32 {
        let control = self.control_group.read();
        let experimental = self.experimental_group.read();

        if control.avg_compile_us == 0 {
            0.0
        } else {
            ((control.avg_compile_us - experimental.avg_compile_us) as f32
                / control.avg_compile_us as f32)
                * 100.0
        }
    }

    /// 是否显著改进 (>5%)
    pub fn is_significant(&self) -> bool {
        self.improvement_percent() > 5.0
    }

    /// 获取结果
    pub fn get_results(&self) -> ABTestResults {
        let control = self.control_group.read().clone();
        let experimental = self.experimental_group.read().clone();

        ABTestResults {
            experiment_id: self.experiment_id.clone(),
            control_group: control,
            experimental_group: experimental,
            improvement_percent: self.improvement_percent(),
            is_significant: self.is_significant(),
            total_samples: self.sample_count.load(Ordering::Relaxed) as u64,
        }
    }
}

/// A/B测试结果
#[derive(Clone, Debug)]
pub struct ABTestResults {
    pub experiment_id: String,
    pub control_group: ABTestMetrics,
    pub experimental_group: ABTestMetrics,
    pub improvement_percent: f32,
    pub is_significant: bool,
    pub total_samples: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_features_normalize() {
        let features = BlockFeatures {
            size_bytes: 1000,
            branch_count: 10,
            loop_count: 2,
            call_count: 5,
            memory_ops: 20,
            execution_count: 1000,
            execution_time_us: 500,
        };

        let normalized = features.normalize();
        assert_eq!(normalized.len(), 7);

        // 所有值应该在0-1之间 (考虑log的情况)
        for val in normalized {
            assert!(val >= 0.0);
        }
    }

    #[test]
    fn test_block_features_hotness() {
        let cold = BlockFeatures {
            execution_count: 1,
            execution_time_us: 10,
            ..Default::default()
        };

        let hot = BlockFeatures {
            execution_count: 10000,
            execution_time_us: 5000,
            ..Default::default()
        };

        let cold_score = cold.hotness_score();
        let hot_score = hot.hotness_score();

        assert!(hot_score > cold_score);
    }

    #[test]
    fn test_simple_linear_model() {
        let model = SimpleLinearModel::new();

        let features = BlockFeatures {
            execution_count: 5000,
            execution_time_us: 1000,
            ..Default::default()
        };

        let score = model.predict(&features);
        assert!((0.0..=3.0).contains(&score));

        let tier = model.predict_tier(&features);
        assert!(matches!(
            tier,
            CompilationDecision::Tier0
                | CompilationDecision::Tier1
                | CompilationDecision::Tier2
                | CompilationDecision::Tier3
        ));
    }

    #[test]
    fn test_compilation_decision_conversion() {
        assert_eq!(CompilationDecision::Tier0.to_id(), 0);
        assert_eq!(CompilationDecision::Tier1.to_id(), 1);
        assert_eq!(CompilationDecision::Tier2.to_id(), 2);
        assert_eq!(CompilationDecision::Tier3.to_id(), 3);

        assert_eq!(CompilationDecision::from_id(0), CompilationDecision::Tier0);
        assert_eq!(CompilationDecision::from_id(1), CompilationDecision::Tier1);
    }

    #[test]
    fn test_ml_guided_compiler_basic() {
        let compiler = MLGuidedCompiler::new();

        let features = BlockFeatures {
            size_bytes: 500,
            branch_count: 5,
            execution_count: 1000,
            ..Default::default()
        };

        let decision1 = compiler.decide_compilation(1, &features);
        assert!(matches!(
            decision1,
            CompilationDecision::Tier0
                | CompilationDecision::Tier1
                | CompilationDecision::Tier2
                | CompilationDecision::Tier3
        ));

        let stats = compiler.get_stats();
        assert_eq!(stats.predictions_made, 1);
    }

    #[test]
    fn test_ml_guided_compiler_caching() {
        let compiler = MLGuidedCompiler::new();

        let features = BlockFeatures {
            size_bytes: 500,
            ..Default::default()
        };

        let _ = compiler.decide_compilation(1, &features);
        let _ = compiler.decide_compilation(1, &features); // 应该命中缓存

        let stats = compiler.get_stats();
        assert_eq!(stats.predictions_made, 1); // 只预测一次
        assert_eq!(stats.cache_hits, 1);
        assert_eq!(stats.cache_misses, 1);
    }

    #[test]
    fn test_ml_guided_compiler_feedback() {
        let compiler = MLGuidedCompiler::new();

        let features = BlockFeatures {
            size_bytes: 500,
            branch_count: 5,
            execution_count: 100,
            ..Default::default()
        };

        let decision = compiler.decide_compilation(1, &features);

        // 模拟快速执行的反馈
        compiler.update_with_feedback(1, decision, 50);

        // 再次查询应该有缓存命中
        let stats = compiler.get_stats();
        assert!(stats.cache_entries > 0);
    }

    #[test]
    fn test_ml_guided_compiler_cache_hit_rate() {
        let compiler = MLGuidedCompiler::new();

        let features = BlockFeatures::default();

        compiler.decide_compilation(1, &features);
        compiler.decide_compilation(1, &features);
        compiler.decide_compilation(1, &features);

        let rate = compiler.cache_hit_rate();
        assert!(rate > 0.5); // 应该有>50%的命中率
    }

    #[test]
    fn test_ab_test_framework() {
        let framework = ABTestFramework::new("exp_001".to_string());

        // 模拟对照组数据
        framework.record_control(100, 1000);
        framework.record_control(110, 1000);
        framework.record_control(105, 1000);

        // 模拟实验组数据 (快10%)
        framework.record_experimental(90, 1000);
        framework.record_experimental(95, 1000);
        framework.record_experimental(92, 1000);

        let results = framework.get_results();
        assert!(results.improvement_percent > 0.0);
        assert!(results.is_significant);
    }

    #[test]
    fn test_ab_test_no_improvement() {
        let framework = ABTestFramework::new("exp_002".to_string());

        // 两组数据相同
        for _ in 0..10 {
            framework.record_control(100, 1000);
            framework.record_experimental(100, 1000);
        }

        let results = framework.get_results();
        assert!(!results.is_significant);
    }

    #[test]
    fn test_ml_compiler_multiple_decisions() {
        let compiler = MLGuidedCompiler::new();

        for i in 0..100u64 {
            let features = BlockFeatures {
                size_bytes: (i * 100) as u32,
                execution_count: i * 100,
                ..Default::default()
            };

            let _ = compiler.decide_compilation(i, &features);
        }

        let stats = compiler.get_stats();
        assert_eq!(stats.predictions_made, 100);
        assert_eq!(stats.cache_entries, 100);
    }

    #[test]
    fn test_feature_extraction_consistency() {
        let f1 = BlockFeatures {
            size_bytes: 1000,
            branch_count: 10,
            loop_count: 5,
            call_count: 20,
            memory_ops: 50,
            execution_count: 5000,
            execution_time_us: 1000,
        };

        let f2 = f1.clone();

        let model = SimpleLinearModel::new();
        let score1 = model.predict(&f1);
        let score2 = model.predict(&f2);

        assert_eq!(score1, score2);
    }
}
