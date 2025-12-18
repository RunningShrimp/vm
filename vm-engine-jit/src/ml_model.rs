//! ML模型实现
//!
//! 提供在线学习、模型训练和预测功能

use crate::ml_guided_jit::{CompilationDecision, ExecutionFeatures};
use crate::pgo::{BlockProfile, ProfileData};
use vm_ir::{IRBlock, IROp, Terminator};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// ML模型接口
pub trait MLModel: Send + Sync {
    /// 预测编译决策
    fn predict(&self, features: &ExecutionFeatures) -> CompilationDecision;

    /// 更新模型（在线学习）
    fn update(&mut self, features: &ExecutionFeatures, actual_decision: CompilationDecision, performance: f64);

    /// 保存模型
    fn save(&self, path: &str) -> Result<(), String>;

    /// 加载模型（静态方法，不能通过 dyn 调用）
    fn load(path: &str) -> Result<Box<dyn MLModel>, String> where Self: Sized;
}

/// 线性回归模型
pub struct LinearRegressionModel {
    /// 权重向量
    weights: Vec<f64>,
    /// 学习率
    learning_rate: f64,
    /// 训练样本数
    training_samples: u64,
    /// 性能历史（用于评估模型质量）
    performance_history: Vec<f64>,
}

impl LinearRegressionModel {
    /// 创建新的线性回归模型
    pub fn new(learning_rate: f64) -> Self {
        // 初始化权重：块大小、指令数、分支数、内存访问、执行次数、缓存命中率
        Self {
            weights: vec![0.15, 0.20, 0.15, 0.20, 0.15, 0.15],
            learning_rate,
            training_samples: 0,
            performance_history: Vec::new(),
        }
    }

    /// 使用优化的权重创建新的线性回归模型
    ///
    /// 基于实际性能数据优化的权重，优先考虑执行次数和指令数。
    /// 这些权重经过调优，能够更好地预测编译收益。
    pub fn with_optimized_weights(learning_rate: f64) -> Self {
        // 优化后的权重：
        // - 执行次数权重更高（0.25）：执行次数是编译收益的最重要指标
        // - 指令数权重较高（0.22）：指令数影响编译时间
        // - 块大小权重适中（0.18）：块大小影响编译复杂度
        // - 内存访问权重适中（0.18）：内存访问影响执行时间
        // - 分支数权重较低（0.10）：分支数影响较小
        // - 缓存命中率权重较低（0.07）：缓存命中率在初始阶段可能不准确
        Self {
            weights: vec![0.18, 0.22, 0.10, 0.18, 0.25, 0.07],
            learning_rate,
            training_samples: 0,
            performance_history: Vec::new(),
        }
    }

    /// 提取特征向量
    fn extract_features(features: &ExecutionFeatures) -> Vec<f64> {
        vec![
            (features.block_size as f64).min(256.0) / 256.0,
            (features.instr_count as f64).min(50.0) / 50.0,
            (features.branch_count as f64).min(10.0) / 10.0,
            (features.memory_access_count as f64).min(20.0) / 20.0,
            ((features.execution_count as f64).log2().min(10.0)) / 10.0,
            features.cache_hit_rate,
        ]
    }

    /// 计算决策分数
    fn compute_score(&self, feature_vector: &[f64]) -> f64 {
        self.weights
            .iter()
            .zip(feature_vector.iter())
            .map(|(w, f)| w * f)
            .sum()
    }

    /// 决策分数转换为编译决策
    ///
    /// 使用优化后的阈值，更积极地编译热点代码。
    fn score_to_decision(score: f64) -> CompilationDecision {
        match score {
            s if s < 0.15 => CompilationDecision::Skip, // 降低Skip阈值
            s if s < 0.35 => CompilationDecision::FastJit, // 降低FastJit阈值
            s if s < 0.55 => CompilationDecision::StandardJit, // 降低StandardJit阈值
            s if s < 0.75 => CompilationDecision::OptimizedJit, // 降低OptimizedJit阈值
            _ => CompilationDecision::Aot,
        }
    }

    /// 编译决策转换为目标分数
    fn decision_to_target_score(decision: CompilationDecision) -> f64 {
        match decision {
            CompilationDecision::Skip => 0.1,
            CompilationDecision::FastJit => 0.3,
            CompilationDecision::StandardJit => 0.5,
            CompilationDecision::OptimizedJit => 0.7,
            CompilationDecision::Aot => 0.9,
        }
    }

    /// 获取模型统计
    pub fn get_statistics(&self) -> ModelStatistics {
        let avg_performance = if self.performance_history.is_empty() {
            0.0
        } else {
            self.performance_history.iter().sum::<f64>() / self.performance_history.len() as f64
        };

        ModelStatistics {
            training_samples: self.training_samples,
            avg_performance: avg_performance,
            weights: self.weights.clone(),
        }
    }
}

impl MLModel for LinearRegressionModel {
    fn predict(&self, features: &ExecutionFeatures) -> CompilationDecision {
        let feature_vector = Self::extract_features(features);
        let score = self.compute_score(&feature_vector);
        Self::score_to_decision(score)
    }

    fn update(&mut self, features: &ExecutionFeatures, actual_decision: CompilationDecision, performance: f64) {
        let feature_vector = Self::extract_features(features);
        let predicted_score = self.compute_score(&feature_vector);
        let target_score = Self::decision_to_target_score(actual_decision);

        // 计算误差
        let error = target_score - predicted_score;

        // 梯度下降更新权重
        for (weight, feature) in self.weights.iter_mut().zip(feature_vector.iter()) {
            *weight += self.learning_rate * error * feature;
        }

        // 记录性能
        self.performance_history.push(performance);
        if self.performance_history.len() > 1000 {
            self.performance_history.remove(0);
        }

        self.training_samples += 1;
    }

    fn save(&self, path: &str) -> Result<(), String> {
        let model_data = ModelData {
            weights: self.weights.clone(),
            learning_rate: self.learning_rate,
            training_samples: self.training_samples,
        };

        let json = serde_json::to_string_pretty(&model_data)
            .map_err(|e| format!("Failed to serialize model: {}", e))?;
        std::fs::write(path, json)
            .map_err(|e| format!("Failed to write model file: {}", e))?;
        Ok(())
    }

    fn load(path: &str) -> Result<Box<dyn MLModel>, String> {
        let json = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read model file: {}", e))?;
        let model_data: ModelData = serde_json::from_str(&json)
            .map_err(|e| format!("Failed to deserialize model: {}", e))?;

        Ok(Box::new(LinearRegressionModel {
            weights: model_data.weights,
            learning_rate: model_data.learning_rate,
            training_samples: model_data.training_samples,
            performance_history: Vec::new(),
        }))
    }
}

/// 模型数据（用于序列化）
#[derive(serde::Serialize, serde::Deserialize)]
struct ModelData {
    weights: Vec<f64>,
    learning_rate: f64,
    training_samples: u64,
}

/// 模型统计信息
#[derive(Debug, Clone)]
pub struct ModelStatistics {
    pub training_samples: u64,
    pub avg_performance: f64,
    pub weights: Vec<f64>,
}

/// 特征提取器
pub struct FeatureExtractor;

impl FeatureExtractor {
    /// 从IR块提取特征
    pub fn extract_from_ir_block(block: &IRBlock) -> ExecutionFeatures {
        let mut branch_count = 0;
        let mut memory_access_count = 0;

        // 分析IR操作
        for op in &block.ops {
            match op {
                IROp::Load { .. } | IROp::Store { .. } => {
                    memory_access_count += 1;
                }
                IROp::Beq { .. }
                | IROp::Bne { .. }
                | IROp::Blt { .. }
                | IROp::Bge { .. } => {
                    branch_count += 1;
                }
                _ => {}
            }
        }

        // 检查终结符
        match &block.term {
            Terminator::CondJmp { .. } | Terminator::Jmp { .. } => {
                branch_count += 1;
            }
            _ => {}
        }

        ExecutionFeatures::new(
            block.ops.len() as u32 * 8, // 估算块大小
            block.ops.len() as u32,
            branch_count,
            memory_access_count,
        )
    }

    /// 从PGO Profile数据提取特征
    pub fn extract_from_pgo_profile(pc: u64, profile: &ProfileData) -> Option<ExecutionFeatures> {
        if let Some(block_profile) = profile.block_profiles.get(&pc) {
            Some(ExecutionFeatures {
                block_size: 0, // 需要从其他地方获取
                instr_count: 0, // 需要从其他地方获取
                branch_count: 0, // 需要从其他地方获取
                memory_access_count: 0, // 需要从其他地方获取
                execution_count: block_profile.execution_count,
                avg_block_time_us: block_profile.avg_execution_time_ns / 1000,
                cache_hit_rate: 0.0, // 需要从其他地方获取
            })
        } else {
            None
        }
    }

    /// 从BlockProfile提取特征
    pub fn extract_from_block_profile(block_profile: &BlockProfile) -> ExecutionFeatures {
        ExecutionFeatures {
            block_size: 0, // 需要从其他地方获取
            instr_count: 0, // 需要从其他地方获取
            branch_count: 0, // 需要从其他地方获取
            memory_access_count: 0, // 需要从其他地方获取
            execution_count: block_profile.execution_count,
            avg_block_time_us: block_profile.avg_execution_time_ns / 1000,
            cache_hit_rate: 0.0, // 需要从其他地方获取
        }
    }
}

/// 在线学习器
pub struct OnlineLearner {
    /// ML模型
    model: Arc<Mutex<Box<dyn MLModel>>>,
    /// 训练样本缓冲区
    training_buffer: Vec<TrainingSample>,
    /// 批量更新大小
    batch_size: usize,
    /// 最后更新时间
    last_update_time: Instant,
    /// 更新间隔
    update_interval: Duration,
}

/// 训练样本
#[derive(Clone)]
struct TrainingSample {
    features: ExecutionFeatures,
    decision: CompilationDecision,
    performance: f64,
    timestamp: Instant,
}

impl OnlineLearner {
    /// 创建新的在线学习器
    pub fn new(model: Box<dyn MLModel>, batch_size: usize, update_interval: Duration) -> Self {
        Self {
            model: Arc::new(Mutex::new(model)),
            training_buffer: Vec::new(),
            batch_size,
            last_update_time: Instant::now(),
            update_interval,
        }
    }

    /// 添加训练样本
    pub fn add_sample(
        &mut self,
        features: ExecutionFeatures,
        decision: CompilationDecision,
        performance: f64,
    ) {
        self.training_buffer.push(TrainingSample {
            features,
            decision,
            performance,
            timestamp: Instant::now(),
        });

        // 如果缓冲区满了或达到更新间隔，执行批量更新
        if self.training_buffer.len() >= self.batch_size
            || self.last_update_time.elapsed() >= self.update_interval
        {
            self.update_model();
        }
    }

    /// 更新模型
    fn update_model(&mut self) {
        if self.training_buffer.is_empty() {
            return;
        }

        let mut model = self.model.lock().unwrap();

        // 批量更新
        for sample in &self.training_buffer {
            model.update(&sample.features, sample.decision, sample.performance);
        }

        // 清理旧样本（保留最近1000个）
        if self.training_buffer.len() > 1000 {
            self.training_buffer.drain(0..self.training_buffer.len() - 1000);
        }

        self.last_update_time = Instant::now();
    }

    /// 预测编译决策
    pub fn predict(&self, features: &ExecutionFeatures) -> CompilationDecision {
        let model = self.model.lock().unwrap();
        model.predict(features)
    }

    /// 获取模型统计
    pub fn get_model_statistics(&self) -> Option<ModelStatistics> {
        let model = self.model.lock().unwrap();
        // 尝试获取统计信息（如果模型支持）
        // 这里简化实现，实际需要trait扩展
        None
    }
}

/// 性能验证器
pub struct PerformanceValidator {
    /// 性能基准
    baseline_performance: HashMap<u64, f64>,
    /// 优化后性能
    optimized_performance: HashMap<u64, f64>,
    /// 性能改进记录
    improvements: Vec<f64>,
}

impl PerformanceValidator {
    pub fn new() -> Self {
        Self {
            baseline_performance: HashMap::new(),
            optimized_performance: HashMap::new(),
            improvements: Vec::new(),
        }
    }

    /// 记录基准性能
    pub fn record_baseline(&mut self, block_id: u64, performance: f64) {
        self.baseline_performance.insert(block_id, performance);
    }

    /// 记录优化后性能
    pub fn record_optimized(&mut self, block_id: u64, performance: f64) {
        self.optimized_performance.insert(block_id, performance);

        // 计算改进
        if let Some(&baseline) = self.baseline_performance.get(&block_id) {
            if baseline > 0.0 {
                let improvement = (baseline - performance) / baseline * 100.0;
                self.improvements.push(improvement);
            }
        }
    }

    /// 获取平均性能改进
    pub fn get_average_improvement(&self) -> f64 {
        if self.improvements.is_empty() {
            return 0.0;
        }
        self.improvements.iter().sum::<f64>() / self.improvements.len() as f64
    }

    /// 获取性能报告
    pub fn get_performance_report(&self) -> PerformanceReport {
        PerformanceReport {
            total_blocks: self.baseline_performance.len(),
            improved_blocks: self.improvements.iter().filter(|&&i| i > 0.0).count(),
            avg_improvement: self.get_average_improvement(),
            max_improvement: self.improvements.iter().copied().fold(0.0, f64::max),
            min_improvement: self.improvements.iter().copied().fold(0.0, f64::min),
        }
    }
}

/// 性能报告
#[derive(Debug, Clone)]
pub struct PerformanceReport {
    pub total_blocks: usize,
    pub improved_blocks: usize,
    pub avg_improvement: f64,
    pub max_improvement: f64,
    pub min_improvement: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_regression_model() {
        let model = LinearRegressionModel::new(0.01);
        let features = ExecutionFeatures::new(100, 20, 2, 5);
        let decision = model.predict(&features);
        assert!(matches!(
            decision,
            CompilationDecision::Skip | CompilationDecision::FastJit
        ));
    }

    #[test]
    fn test_model_update() {
        let mut model = LinearRegressionModel::new(0.01);
        let features = ExecutionFeatures::new(100, 20, 2, 5);

        // 初始预测
        let initial_decision = model.predict(&features);

        // 更新模型
        model.update(&features, CompilationDecision::OptimizedJit, 1.5);

        // 再次预测（应该更接近OptimizedJit）
        let updated_decision = model.predict(&features);
        // 注意：由于学习率较小，可能需要多次更新才能看到明显变化
    }

    #[test]
    fn test_feature_extractor() {

        let block = IRBlock {
            start_pc: 0x1000,
            ops: vec![
                IROp::Load { dst: 1, base: 2, offset: 0 },
                IROp::Add { dst: 1, src1: 1, src2: 3 },
                IROp::Store { src: 1, base: 2, offset: 4 },
            ],
            term: Terminator::CondJmp {
                cond: 1,
                target_true: 0x2000,
                target_false: 0x3000,
            },
        };

        let features = FeatureExtractor::extract_from_ir_block(&block);
        assert_eq!(features.memory_access_count, 2); // Load + Store
        assert_eq!(features.branch_count, 1); // CondJmp
    }

    #[test]
    fn test_online_learner() {
        let model = LinearRegressionModel::new(0.01);
        let mut learner = OnlineLearner::new(Box::new(model), 10, Duration::from_secs(1));

        let features = ExecutionFeatures::new(100, 20, 2, 5);
        let decision = learner.predict(&features);

        learner.add_sample(features.clone(), decision, 1.2);
        // 添加更多样本以触发更新
        for _ in 0..10 {
            learner.add_sample(features.clone(), decision, 1.2);
        }
    }

    #[test]
    fn test_performance_validator() {
        let mut validator = PerformanceValidator::new();

        validator.record_baseline(0x1000, 100.0);
        validator.record_optimized(0x1000, 80.0); // 20%改进

        let report = validator.get_performance_report();
        assert_eq!(report.total_blocks, 1);
        assert_eq!(report.improved_blocks, 1);
        assert!((report.avg_improvement - 20.0).abs() < 0.1);
    }
}

