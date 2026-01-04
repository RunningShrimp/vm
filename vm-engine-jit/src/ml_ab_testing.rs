//! ML模型A/B测试框架
//!
//! 提供A/B测试能力，用于比较不同ML模型的性能。

use super::ml_model_enhanced::ExecutionFeaturesEnhanced;
use super::ml_random_forest::{CompilationDecision, RandomForestModel};
use rand::Rng;
use rand::SeedableRng;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

// ============================================================================
// 性能记录
// ============================================================================

/// 性能记录
#[derive(Clone, Debug)]
pub struct PerformanceRecord {
    /// 使用的模型名称
    pub model_name: String,
    /// 输入特征
    pub features: ExecutionFeaturesEnhanced,
    /// 决策
    pub decision: CompilationDecision,
    /// 时间戳
    pub timestamp: SystemTime,
    /// 执行时间（纳秒）
    pub execution_time_ns: Option<u64>,
    /// 是否成功
    pub success: bool,
}

// ============================================================================
// 模型比较结果
// ============================================================================

/// 模型对比结果
#[derive(Debug)]
pub struct ModelComparison {
    /// 模型A统计
    pub model_a: ModelPerformanceStats,
    /// 模型B统计
    pub model_b: ModelPerformanceStats,
    /// 获胜模型
    pub winner: String,
    /// 性能提升百分比
    pub improvement_percent: f64,
}

/// 模型性能统计
#[derive(Debug, Clone, Default)]
pub struct ModelPerformanceStats {
    /// 总预测次数
    pub total_predictions: u64,
    /// 成功预测次数
    pub successful_predictions: u64,
    /// 失败预测次数
    pub failed_predictions: u64,
    /// 平均执行时间（纳秒）
    pub avg_execution_time_ns: f64,
    /// 准确率（如果有标签）
    pub accuracy: f64,
    /// 决策分布
    pub decision_distribution: DecisionDistribution,
}

/// 决策分布
#[derive(Debug, Clone, Default)]
pub struct DecisionDistribution {
    pub skip_count: u64,
    pub compile_count: u64,
    pub compile_fast_count: u64,
    pub warmup_count: u64,
}

// ============================================================================
// A/B测试配置
// ============================================================================

/// A/B测试配置
#[derive(Debug, Clone)]
pub struct ABTestConfig {
    /// 流量分配（模型A的比例，0.0-1.0）
    pub traffic_split: f64,
    /// 是否记录详细日志
    pub enable_logging: bool,
    /// 最小样本数（用于统计显著性）
    pub min_samples: usize,
    /// 是否自动选择获胜模型
    pub auto_select_winner: bool,
}

impl Default for ABTestConfig {
    fn default() -> Self {
        Self {
            traffic_split: 0.5, // 50/50分配
            enable_logging: true,
            min_samples: 100,
            auto_select_winner: false,
        }
    }
}

// ============================================================================
// A/B测试器
// ============================================================================

/// ML模型A/B测试器
///
/// 用于比较两个ML模型的性能
pub struct ModelABTest {
    /// 模型A
    model_a: Box<dyn MLModel>,
    /// 模型B
    model_b: Box<dyn MLModel>,
    /// 配置
    config: ABTestConfig,
    /// 性能日志
    performance_log: Arc<Mutex<Vec<PerformanceRecord>>>,
    /// 随机数生成器
    rng: Arc<Mutex<rand::rngs::StdRng>>,
}

/// ML模型trait
pub trait MLModel: Send + Sync {
    /// 预测决策
    fn predict(&mut self, features: &ExecutionFeaturesEnhanced) -> CompilationDecision;

    /// 获取模型名称
    fn name(&self) -> String;
}

// 为RandomForestModel实现MLModel trait
impl MLModel for RandomForestModel {
    fn predict(&mut self, features: &ExecutionFeaturesEnhanced) -> CompilationDecision {
        RandomForestModel::predict(self, features)
    }

    fn name(&self) -> String {
        "RandomForest".to_string()
    }
}

impl ModelABTest {
    /// 创建新的A/B测试器
    ///
    /// # 参数
    /// - `model_a`: 模型A
    /// - `model_b`: 模型B
    /// - `config`: 测试配置
    pub fn new(model_a: Box<dyn MLModel>, model_b: Box<dyn MLModel>, config: ABTestConfig) -> Self {
        let rng = rand::rngs::StdRng::from_entropy();
        Self {
            model_a,
            model_b,
            config,
            performance_log: Arc::new(Mutex::new(Vec::new())),
            rng: Arc::new(Mutex::new(rng)),
        }
    }

    /// 预测并记录性能
    ///
    /// 根据流量分配选择模型A或B，并记录结果
    pub fn predict_and_log(&mut self, features: ExecutionFeaturesEnhanced) -> CompilationDecision {
        // 随机选择模型
        let use_model_a = {
            let mut rng = self.rng.lock().unwrap();
            rng.r#gen::<f64>() < self.config.traffic_split
        };

        let (decision, model_name) = if use_model_a {
            let decision = self.model_a.predict(&features);
            (decision, self.model_a.name())
        } else {
            let decision = self.model_b.predict(&features);
            (decision, self.model_b.name())
        };

        // 记录性能数据
        if self.config.enable_logging {
            let record = PerformanceRecord {
                model_name: model_name.clone(),
                features: features.clone(),
                decision,
                timestamp: SystemTime::now(),
                execution_time_ns: None,
                success: true,
            };

            let mut log = self.performance_log.lock().unwrap();
            log.push(record);
        }

        decision
    }

    /// 预测并记录执行时间
    pub fn predict_and_log_with_timing<F>(
        &mut self,
        features: ExecutionFeaturesEnhanced,
        exec_fn: F,
    ) -> CompilationDecision
    where
        F: FnOnce() -> Result<(), ()>,
    {
        let start = std::time::Instant::now();

        // 预测决策
        let decision = self.predict_and_log(features.clone());

        // 执行并测量时间
        let result = exec_fn();
        let elapsed = start.elapsed();

        // 更新记录
        if self.config.enable_logging {
            let mut log = self.performance_log.lock().unwrap();
            if let Some(last) = log.last_mut() {
                last.execution_time_ns = Some(elapsed.as_nanos() as u64);
                last.success = result.is_ok();
            }
        }

        decision
    }

    /// 评估模型性能
    ///
    /// 比较模型A和模型B的性能指标
    pub fn evaluate(&self) -> ModelComparison {
        let log = self.performance_log.lock().unwrap();

        let mut model_a_stats = ModelPerformanceStats::default();
        let mut model_b_stats = ModelPerformanceStats::default();

        // 分类统计
        for record in log.iter() {
            let stats = if record.model_name == self.model_a.name() {
                &mut model_a_stats
            } else {
                &mut model_b_stats
            };

            stats.total_predictions += 1;

            if record.success {
                stats.successful_predictions += 1;
            } else {
                stats.failed_predictions += 1;
            }

            // 统计执行时间
            if let Some(time) = record.execution_time_ns {
                let count = stats.total_predictions as f64;
                let avg = stats.avg_execution_time_ns;
                stats.avg_execution_time_ns = (avg * (count - 1.0) + time as f64) / count;
            }

            // 统计决策分布
            match record.decision {
                CompilationDecision::Skip => {
                    stats.decision_distribution.skip_count += 1;
                }
                CompilationDecision::Compile => {
                    stats.decision_distribution.compile_count += 1;
                }
                CompilationDecision::CompileFast => {
                    stats.decision_distribution.compile_fast_count += 1;
                }
                CompilationDecision::Warmup => {
                    stats.decision_distribution.warmup_count += 1;
                }
            }
        }

        // 计算准确率（简化：使用成功率）
        model_a_stats.accuracy = if model_a_stats.total_predictions > 0 {
            model_a_stats.successful_predictions as f64 / model_a_stats.total_predictions as f64
        } else {
            0.0
        };

        model_b_stats.accuracy = if model_b_stats.total_predictions > 0 {
            model_b_stats.successful_predictions as f64 / model_b_stats.total_predictions as f64
        } else {
            0.0
        };

        // 确定获胜模型（基于准确率）
        let (winner, improvement) = if model_a_stats.accuracy > model_b_stats.accuracy {
            let improvement = if model_b_stats.accuracy > 0.0 {
                (model_a_stats.accuracy - model_b_stats.accuracy) / model_b_stats.accuracy * 100.0
            } else {
                0.0
            };
            (self.model_a.name(), improvement)
        } else {
            let improvement = if model_a_stats.accuracy > 0.0 {
                (model_b_stats.accuracy - model_a_stats.accuracy) / model_a_stats.accuracy * 100.0
            } else {
                0.0
            };
            (self.model_b.name(), improvement)
        };

        ModelComparison {
            model_a: model_a_stats,
            model_b: model_b_stats,
            winner,
            improvement_percent: improvement,
        }
    }

    /// 获取性能日志
    pub fn get_log(&self) -> Vec<PerformanceRecord> {
        self.performance_log.lock().unwrap().clone()
    }

    /// 清空日志
    pub fn clear_log(&mut self) {
        self.performance_log.lock().unwrap().clear();
    }

    /// 获取日志大小
    pub fn log_size(&self) -> usize {
        self.performance_log.lock().unwrap().len()
    }

    /// 调整流量分配
    ///
    /// 将更多流量分配给获胜模型
    pub fn adjust_traffic_to_winner(&mut self) {
        let comparison = self.evaluate();

        // 简单策略：如果模型A更好，分配70%流量给A
        if comparison.winner == self.model_a.name() {
            self.config.traffic_split = 0.7;
        } else {
            self.config.traffic_split = 0.3;
        }
    }

    /// 自动选择获胜模型
    ///
    /// 如果启用了自动选择且样本数足够，切换到获胜模型
    pub fn auto_select_winner_if_needed(&mut self) -> Option<Box<dyn MLModel>> {
        if !self.config.auto_select_winner {
            return None;
        }

        let log_size = self.log_size();
        if log_size < self.config.min_samples {
            return None;
        }

        let _comparison = self.evaluate();

        // 返回获胜模型的克隆（如果实现支持）
        // 简化实现：只返回当前使用的模型
        None
    }
}

// ============================================================================
// A/B测试管理器（高级封装）
// ============================================================================

/// A/B测试管理器
///
/// 提供更简单的API来管理多个模型的A/B测试
pub struct ABTestManager {
    /// 已注册的模型
    models: Vec<String>,
    /// 配置
    config: ABTestConfig,
    /// 性能记录（按模型名称分类）
    performance_records: Arc<Mutex<HashMap<String, Vec<PerformanceRecord>>>>,
    /// 随机数生成器
    rng: Arc<Mutex<rand::rngs::StdRng>>,
}

impl ABTestManager {
    /// 创建新的A/B测试管理器
    pub fn new(config: ABTestConfig) -> Self {
        let rng = rand::rngs::StdRng::from_entropy();
        Self {
            models: Vec::new(),
            config,
            performance_records: Arc::new(Mutex::new(HashMap::new())),
            rng: Arc::new(Mutex::new(rng)),
        }
    }

    /// 注册模型
    pub fn register_model(&mut self, model_name: String) {
        if !self.models.contains(&model_name) {
            self.models.push(model_name);
        }
    }

    /// 检查是否有模型
    pub fn has_model(&self, model_name: &str) -> bool {
        self.models.contains(&model_name.to_string())
    }

    /// 检查是否启用日志
    pub fn is_logging_enabled(&self) -> bool {
        self.config.enable_logging
    }

    /// 选择模型（根据流量分配）
    pub fn select_model(&self) -> Option<String> {
        if self.models.is_empty() {
            return None;
        }

        // 如果只有一个模型，直接返回
        if self.models.len() == 1 {
            return self.models.first().cloned();
        }

        // 根据traffic_split选择模型
        let use_first = {
            let mut rng = self.rng.lock().unwrap();
            rng.r#gen::<f64>() < self.config.traffic_split
        };

        if use_first && self.models.len() > 0 {
            self.models.get(0).cloned()
        } else if self.models.len() > 1 {
            self.models.get(1).cloned()
        } else {
            None
        }
    }

    /// 选择模型（支持自动选择获胜模型）
    pub fn select_model_with_auto_selection(&self) -> Option<String> {
        // 如果启用了自动选择且样本足够，优先选择获胜模型
        if self.config.auto_select_winner {
            let records = self.performance_records.lock().unwrap();

            // 检查是否所有模型都有足够的样本
            let min_samples = self.config.min_samples;
            let all_have_enough = self.models.iter().all(|name| {
                records
                    .get(name)
                    .map(|recs| recs.len() >= min_samples)
                    .unwrap_or(false)
            });

            if all_have_enough {
                // 找到平均执行时间最短的模型
                let mut best_model = None;
                let mut best_time = None;

                for model_name in &self.models {
                    if let Some(model_records) = records.get(model_name) {
                        let total_time: u64 = model_records
                            .iter()
                            .filter_map(|r| r.execution_time_ns)
                            .sum();
                        let count = model_records.len();
                        if count > 0 {
                            let avg_time = total_time / count as u64;

                            if best_time.is_none() || Some(avg_time) < best_time {
                                best_time = Some(avg_time);
                                best_model = Some(model_name.clone());
                            }
                        }
                    }
                }

                if let Some(best) = best_model {
                    return Some(best);
                }
            }
        }

        // 回退到普通选择
        self.select_model()
    }

    /// 记录性能
    pub fn record_performance(&self, record: PerformanceRecord) {
        let mut records = self.performance_records.lock().unwrap();
        records
            .entry(record.model_name.clone())
            .or_insert_with(Vec::new)
            .push(record);
    }

    /// 获取模型统计
    pub fn get_model_stats(&self, model_name: &str) -> Option<ModelPerformanceStats> {
        let records = self.performance_records.lock().unwrap();
        let model_records = records.get(model_name)?;

        let mut stats = ModelPerformanceStats::default();

        for record in model_records.iter() {
            stats.total_predictions += 1;

            if record.success {
                stats.successful_predictions += 1;
            } else {
                stats.failed_predictions += 1;
            }

            // 统计执行时间
            if let Some(time) = record.execution_time_ns {
                let count = stats.total_predictions as f64;
                let avg = stats.avg_execution_time_ns;
                stats.avg_execution_time_ns = (avg * (count - 1.0) + time as f64) / count;
            }

            // 统计决策分布
            match record.decision {
                CompilationDecision::Skip => {
                    stats.decision_distribution.skip_count += 1;
                }
                CompilationDecision::Compile => {
                    stats.decision_distribution.compile_count += 1;
                }
                CompilationDecision::CompileFast => {
                    stats.decision_distribution.compile_fast_count += 1;
                }
                CompilationDecision::Warmup => {
                    stats.decision_distribution.warmup_count += 1;
                }
            }
        }

        // 计算准确率
        stats.accuracy = if stats.total_predictions > 0 {
            stats.successful_predictions as f64 / stats.total_predictions as f64
        } else {
            0.0
        };

        Some(stats)
    }

    /// 比较两个模型
    pub fn compare_models(
        &self,
        model_a_name: &str,
        model_b_name: &str,
    ) -> Option<ModelComparison> {
        let stats_a = self.get_model_stats(model_a_name)?;
        let stats_b = self.get_model_stats(model_b_name)?;

        // 检查样本数是否足够
        if stats_a.total_predictions < self.config.min_samples as u64
            || stats_b.total_predictions < self.config.min_samples as u64
        {
            return None;
        }

        // 基于平均执行时间确定获胜者（时间越短越好）
        let (winner, improvement) = if stats_a.avg_execution_time_ns < stats_b.avg_execution_time_ns
        {
            let improvement = if stats_b.avg_execution_time_ns > 0.0 {
                (stats_b.avg_execution_time_ns - stats_a.avg_execution_time_ns)
                    / stats_b.avg_execution_time_ns
                    * 100.0
            } else {
                0.0
            };
            (model_a_name.to_string(), improvement)
        } else {
            let improvement = if stats_a.avg_execution_time_ns > 0.0 {
                (stats_a.avg_execution_time_ns - stats_b.avg_execution_time_ns)
                    / stats_a.avg_execution_time_ns
                    * 100.0
            } else {
                0.0
            };
            (model_b_name.to_string(), improvement)
        };

        Some(ModelComparison {
            model_a: stats_a.clone(),
            model_b: stats_b.clone(),
            winner,
            improvement_percent: improvement,
        })
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ml_model_enhanced::{CompilationHistory, InstMixFeatures};

    fn create_test_features() -> ExecutionFeaturesEnhanced {
        ExecutionFeaturesEnhanced {
            block_size: 100,
            instr_count: 100,
            branch_count: 10,
            memory_access_count: 20,
            execution_count: 50,
            cache_hit_rate: 0.85,
            instruction_mix: InstMixFeatures {
                arithmetic_ratio: 0.3,
                memory_ratio: 0.2,
                branch_ratio: 0.1,
                vector_ratio: 0.0,
                float_ratio: 0.0,
                call_ratio: 0.0,
            },
            control_flow_complexity: 5.0,
            loop_nest_depth: 2,
            has_recursion: false,
            data_locality: 0.8,
            memory_sequentiality: 0.9,
            compilation_history: CompilationHistory {
                previous_compilations: 5,
                avg_compilation_time_us: 100.0,
                last_compile_benefit: 2.0,
                last_compile_success: true,
            },
            register_pressure: 0.5,
            code_heat: 0.7,
            code_stability: 0.9,
        }
    }

    // 简单测试模型
    struct SimpleModelA;

    impl MLModel for SimpleModelA {
        fn predict(&mut self, _features: &ExecutionFeaturesEnhanced) -> CompilationDecision {
            CompilationDecision::Compile
        }

        fn name(&self) -> String {
            "ModelA".to_string()
        }
    }

    struct SimpleModelB;

    impl MLModel for SimpleModelB {
        fn predict(&mut self, _features: &ExecutionFeaturesEnhanced) -> CompilationDecision {
            CompilationDecision::Skip
        }

        fn name(&self) -> String {
            "ModelB".to_string()
        }
    }

    #[test]
    fn test_ab_test_creation() {
        let model_a = Box::new(SimpleModelA);
        let model_b = Box::new(SimpleModelB);
        let config = ABTestConfig::default();

        let ab_test = ModelABTest::new(model_a, model_b, config);

        assert_eq!(ab_test.log_size(), 0);
    }

    #[test]
    fn test_predict_and_log() {
        let model_a = Box::new(SimpleModelA);
        let model_b = Box::new(SimpleModelB);
        let config = ABTestConfig {
            traffic_split: 1.0, // 总是使用模型A
            ..Default::default()
        };

        let mut ab_test = ModelABTest::new(model_a, model_b, config);

        let features = create_test_features();
        let decision = ab_test.predict_and_log(features);

        assert_eq!(decision, CompilationDecision::Compile);
        assert_eq!(ab_test.log_size(), 1);
    }

    #[test]
    fn test_evaluate() {
        let model_a = Box::new(SimpleModelA);
        let model_b = Box::new(SimpleModelB);
        let config = ABTestConfig::default();

        let mut ab_test = ModelABTest::new(model_a, model_b, config);

        // 生成一些测试数据
        for _ in 0..10 {
            let features = create_test_features();
            ab_test.predict_and_log(features);
        }

        let comparison = ab_test.evaluate();

        assert_eq!(comparison.model_a.total_predictions, 10);
        assert_eq!(comparison.model_b.total_predictions, 10);
    }

    #[test]
    fn test_clear_log() {
        let model_a = Box::new(SimpleModelA);
        let model_b = Box::new(SimpleModelB);
        let config = ABTestConfig::default();

        let mut ab_test = ModelABTest::new(model_a, model_b, config);

        let features = create_test_features();
        ab_test.predict_and_log(features);

        assert_eq!(ab_test.log_size(), 1);

        ab_test.clear_log();
        assert_eq!(ab_test.log_size(), 0);
    }

    #[test]
    fn test_decision_distribution() {
        let model_a = Box::new(SimpleModelA);
        let model_b = Box::new(SimpleModelB);
        let config = ABTestConfig {
            traffic_split: 1.0, // 总是使用模型A
            ..Default::default()
        };

        let mut ab_test = ModelABTest::new(model_a, model_b, config);

        // 生成测试数据
        for _ in 0..5 {
            let features = create_test_features();
            ab_test.predict_and_log(features);
        }

        let comparison = ab_test.evaluate();

        // 模型A应该有5个Compile决策
        assert_eq!(comparison.model_a.decision_distribution.compile_count, 5);
    }
}
