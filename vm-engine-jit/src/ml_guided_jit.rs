//! 机器学习辅助 JIT 编译
//!
//! 使用简单的启发式 ML 模型预测编译策略和参数

use std::collections::HashMap;

/// 执行特征 (用于 ML 模型)
#[derive(Clone, Debug)]
pub struct ExecutionFeatures {
    /// 块大小 (字节数)
    pub block_size: u32,
    /// 指令数
    pub instr_count: u32,
    /// 分支数
    pub branch_count: u32,
    /// 记忆访问数
    pub memory_access_count: u32,
    /// 历史执行次数
    pub execution_count: u64,
    /// 平均块执行时间 (微秒)
    pub avg_block_time_us: u64,
    /// 缓存命中率
    pub cache_hit_rate: f64,
}

impl ExecutionFeatures {
    pub fn new(block_size: u32, instr_count: u32, branch_count: u32, memory_accesses: u32) -> Self {
        Self {
            block_size,
            instr_count,
            branch_count,
            memory_access_count: memory_accesses,
            execution_count: 0,
            avg_block_time_us: 0,
            cache_hit_rate: 0.0,
        }
    }

    /// 计算特征得分用于 ML 决策
    pub fn compute_score(&self) -> f64 {
        // 简单的启发式评分函数
        let size_score = (self.block_size as f64).min(256.0) / 256.0;
        let branch_score = (self.branch_count as f64).min(10.0) / 10.0;
        let memory_score = (self.memory_access_count as f64).min(20.0) / 20.0;
        let cache_score = self.cache_hit_rate;

        // 加权组合
        0.3 * size_score + 0.2 * branch_score + 0.25 * memory_score + 0.25 * cache_score
    }
}

/// 编译决策结果
#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub enum CompilationDecision {
    /// 跳过编译 - 解释执行
    Skip,
    /// 快速 JIT - O0
    FastJit,
    /// 标准 JIT - O1
    StandardJit,
    /// 优化 JIT - O2
    OptimizedJit,
    /// AOT 离线编译
    Aot,
}

impl CompilationDecision {
    pub fn as_str(&self) -> &'static str {
        match self {
            CompilationDecision::Skip => "skip",
            CompilationDecision::FastJit => "fast-jit",
            CompilationDecision::StandardJit => "standard-jit",
            CompilationDecision::OptimizedJit => "optimized-jit",
            CompilationDecision::Aot => "aot",
        }
    }
}

/// ML 辅助编译决策器
pub struct MLGuidedCompiler {
    /// 特征历史记录
    feature_history: HashMap<u64, Vec<ExecutionFeatures>>,
    /// 模型权重 (简单的线性模型)
    model_weights: ModelWeights,
    /// 预测缓存
    prediction_cache: HashMap<u64, CompilationDecision>,
}

/// 模型权重
#[derive(Clone, Debug)]
struct ModelWeights {
    /// 块大小权重
    block_size_weight: f64,
    /// 指令数权重
    instr_count_weight: f64,
    /// 分支数权重
    branch_count_weight: f64,
    /// 内存访问权重
    memory_access_weight: f64,
    /// 执行计数权重
    execution_count_weight: f64,
    /// 缓存命中率权重
    cache_hit_weight: f64,
}

impl Default for ModelWeights {
    fn default() -> Self {
        // 优化后的默认权重，与LinearRegressionModel::with_optimized_weights保持一致
        Self {
            block_size_weight: 0.18,
            instr_count_weight: 0.22,
            branch_count_weight: 0.10,
            memory_access_weight: 0.18,
            execution_count_weight: 0.25, // 执行次数是最重要的指标
            cache_hit_weight: 0.07, // 缓存命中率在初始阶段可能不准确
        }
    }
}

impl MLGuidedCompiler {
    /// 创建新的 ML 编译决策器
    pub fn new() -> Self {
        Self {
            feature_history: HashMap::new(),
            model_weights: ModelWeights::default(),
            prediction_cache: HashMap::new(),
        }
    }

    /// 更新特征数据
    pub fn update_features(&mut self, block_id: u64, features: ExecutionFeatures) {
        self.feature_history
            .entry(block_id)
            .or_insert_with(Vec::new)
            .push(features.clone());

        // 清除缓存以重新计算
        self.prediction_cache.remove(&block_id);
    }

    /// 根据特征预测编译决策
    pub fn predict_decision(
        &mut self,
        block_id: u64,
        features: &ExecutionFeatures,
    ) -> CompilationDecision {
        // 检查缓存
        if let Some(decision) = self.prediction_cache.get(&block_id) {
            return *decision;
        }

        let decision = self.compute_decision(block_id, features);
        self.prediction_cache.insert(block_id, decision);
        decision
    }

    /// 计算编译决策的核心逻辑
    fn compute_decision(
        &self,
        _block_id: u64,
        features: &ExecutionFeatures,
    ) -> CompilationDecision {
        // 计算综合评分
        let normalized_block_size = (features.block_size as f64).min(256.0) / 256.0;
        let normalized_instr = (features.instr_count as f64).min(50.0) / 50.0;
        let normalized_branches = (features.branch_count as f64).min(10.0) / 10.0;
        let normalized_memory = (features.memory_access_count as f64).min(20.0) / 20.0;
        let normalized_exec = ((features.execution_count as f64).log2().min(10.0)) / 10.0;

        let score = self.model_weights.block_size_weight * normalized_block_size
            + self.model_weights.instr_count_weight * normalized_instr
            + self.model_weights.branch_count_weight * normalized_branches
            + self.model_weights.memory_access_weight * normalized_memory
            + self.model_weights.execution_count_weight * normalized_exec
            + self.model_weights.cache_hit_weight * features.cache_hit_rate;

        // 基于评分做决策（优化后的阈值，更积极地编译热点代码）
        // 降低Skip阈值，提高FastJit和StandardJit的使用率
        match score {
            s if s < 0.15 => CompilationDecision::Skip, // 降低Skip阈值（0.2 -> 0.15）
            s if s < 0.35 => CompilationDecision::FastJit, // 降低FastJit阈值（0.4 -> 0.35）
            s if s < 0.55 => CompilationDecision::StandardJit, // 降低StandardJit阈值（0.6 -> 0.55）
            s if s < 0.75 => CompilationDecision::OptimizedJit, // 降低OptimizedJit阈值（0.8 -> 0.75）
            _ => CompilationDecision::Aot,
        }
    }

    /// 根据执行次数建议编译策略
    pub fn suggest_compilation_strategy(&self, execution_count: u64) -> CompilationDecision {
        match execution_count {
            0..=10 => CompilationDecision::Skip,
            11..=50 => CompilationDecision::FastJit,
            51..=200 => CompilationDecision::StandardJit,
            201..=1000 => CompilationDecision::OptimizedJit,
            _ => CompilationDecision::Aot,
        }
    }

    /// 估计编译收益 (相对于解释执行的性能提升)
    pub fn estimate_benefit(&self, features: &ExecutionFeatures) -> f64 {
        // 基于特征估计性能改进百分比
        let branch_benefit = (features.branch_count as f64) * 0.05; // 每个分支 5% 收益
        let memory_benefit = (features.memory_access_count as f64) * 0.02; // 每个内存访问 2% 收益
        let cache_penalty = (1.0 - features.cache_hit_rate) * 0.20; // 缓存命中率低会减少收益

        (branch_benefit + memory_benefit - cache_penalty)
            .max(0.0)
            .min(2.0) // 最多 200% 改进
    }

    /// 使用PGO数据增强特征
    pub fn enhance_features_with_pgo(
        &mut self,
        block_id: u64,
        features: &mut ExecutionFeatures,
        profile: &crate::pgo::ProfileData,
    ) {
        if let Some(block_profile) = profile.block_profiles.get(&block_id) {
            features.execution_count = block_profile.execution_count;
            features.avg_block_time_us = block_profile.avg_execution_time_ns / 1000;

            // 从分支profile更新缓存命中率估算
            if let Some(branch_profile) = profile.branch_profiles.get(&block_id) {
                // 使用分支预测准确率作为缓存命中率的代理
                features.cache_hit_rate = branch_profile.taken_probability;
            }
        }
    }

    /// 估计编译成本 (相对)
    pub fn estimate_compile_cost(&self, features: &ExecutionFeatures) -> f64 {
        // 基于块大小和指令数估计编译成本
        let size_cost = (features.block_size as f64) / 256.0;
        let instr_cost = (features.instr_count as f64) / 50.0;

        (size_cost * 0.6 + instr_cost * 0.4).min(1.0)
    }

    /// 收集统计数据用于模型改进
    pub fn get_statistics(&self) -> MLStatistics {
        let mut total_blocks = 0;
        let mut total_features = 0;
        let mut avg_block_size = 0.0;
        let mut avg_exec_count = 0.0;

        for features_list in self.feature_history.values() {
            total_blocks += 1;
            for features in features_list {
                total_features += 1;
                avg_block_size += features.block_size as f64;
                avg_exec_count += features.execution_count as f64;
            }
        }

        if total_features > 0 {
            avg_block_size /= total_features as f64;
            avg_exec_count /= total_features as f64;
        }

        MLStatistics {
            total_blocks,
            total_features,
            avg_block_size,
            avg_execution_count: avg_exec_count,
            cached_predictions: self.prediction_cache.len(),
        }
    }

    /// 生成诊断报告
    pub fn diagnostic_report(&self) -> String {
        let stats = self.get_statistics();

        format!(
            r#"=== ML-Guided Compilation Report ===
Total Blocks Tracked: {}
Total Features Recorded: {}
Average Block Size: {:.1} bytes
Average Execution Count: {:.0}
Cached Predictions: {}

Model Weights:
  Block Size: {:.2}
  Instruction Count: {:.2}
  Branch Count: {:.2}
  Memory Access: {:.2}
  Execution Count: {:.2}
  Cache Hit Rate: {:.2}
"#,
            stats.total_blocks,
            stats.total_features,
            stats.avg_block_size,
            stats.avg_execution_count,
            stats.cached_predictions,
            self.model_weights.block_size_weight,
            self.model_weights.instr_count_weight,
            self.model_weights.branch_count_weight,
            self.model_weights.memory_access_weight,
            self.model_weights.execution_count_weight,
            self.model_weights.cache_hit_weight,
        )
    }
}

impl Default for MLGuidedCompiler {
    fn default() -> Self {
        Self::new()
    }
}

/// ML 统计数据
#[derive(Debug, Clone, Default)]
pub struct MLStatistics {
    pub total_blocks: usize,
    pub total_features: usize,
    pub avg_block_size: f64,
    pub avg_execution_count: f64,
    pub cached_predictions: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ml_compiler_creation() {
        let compiler = MLGuidedCompiler::new();
        let stats = compiler.get_statistics();
        assert_eq!(stats.total_blocks, 0);
    }

    #[test]
    fn test_feature_update() {
        let mut compiler = MLGuidedCompiler::new();
        let features = ExecutionFeatures::new(100, 20, 2, 5);

        compiler.update_features(0x1000, features);
        let stats = compiler.get_statistics();
        assert_eq!(stats.total_blocks, 1);
        assert_eq!(stats.total_features, 1);
    }

    #[test]
    fn test_decision_prediction() {
        let mut compiler = MLGuidedCompiler::new();

        // 小块，低执行频率
        let small_features = ExecutionFeatures::new(50, 10, 1, 2);
        let decision = compiler.predict_decision(0x1000, &small_features);
        assert_eq!(decision, CompilationDecision::Skip);

        // 大块，高执行频率
        let mut large_features = ExecutionFeatures::new(256, 50, 10, 20);
        large_features.execution_count = 10000;
        large_features.cache_hit_rate = 0.95;
        let decision = compiler.predict_decision(0x2000, &large_features);
        assert!(matches!(
            decision,
            CompilationDecision::OptimizedJit | CompilationDecision::Aot
        ));
    }

    #[test]
    fn test_execution_strategy_suggestion() {
        let compiler = MLGuidedCompiler::new();

        assert_eq!(
            compiler.suggest_compilation_strategy(5),
            CompilationDecision::Skip
        );
        assert_eq!(
            compiler.suggest_compilation_strategy(30),
            CompilationDecision::FastJit
        );
        assert_eq!(
            compiler.suggest_compilation_strategy(100),
            CompilationDecision::StandardJit
        );
        assert_eq!(
            compiler.suggest_compilation_strategy(500),
            CompilationDecision::OptimizedJit
        );
        assert_eq!(
            compiler.suggest_compilation_strategy(5000),
            CompilationDecision::Aot
        );
    }

    #[test]
    fn test_benefit_estimation() {
        let compiler = MLGuidedCompiler::new();

        let low_benefit = ExecutionFeatures::new(50, 10, 0, 0);
        let benefit1 = compiler.estimate_benefit(&low_benefit);
        assert!(benefit1 < 0.1);

        let high_benefit = ExecutionFeatures::new(256, 50, 20, 20);
        let benefit2 = compiler.estimate_benefit(&high_benefit);
        assert!(benefit2 > benefit1);
    }

    #[test]
    fn test_feature_scoring() {
        let features = ExecutionFeatures::new(100, 20, 2, 5);
        let score = features.compute_score();
        assert!(score >= 0.0 && score <= 1.0);
    }

    #[test]
    fn test_diagnostic_report() {
        let mut compiler = MLGuidedCompiler::new();
        let features = ExecutionFeatures::new(100, 20, 2, 5);

        compiler.update_features(0x1000, features);
        let report = compiler.diagnostic_report();

        assert!(report.contains("ML-Guided Compilation Report"));
        assert!(report.contains("Model Weights"));
    }
}
