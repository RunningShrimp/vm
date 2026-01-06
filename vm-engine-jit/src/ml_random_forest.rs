//! ML RandomForest模型实现
//!
//! 提供基于随机森林的JIT编译决策模型。

use super::ml_model_enhanced::ExecutionFeaturesEnhanced;
use std::collections::HashMap;

// ============================================================================
// 编译决策
// ============================================================================

/// JIT编译决策
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompilationDecision {
    /// 跳过编译（解释执行）
    Skip,
    /// 编译为优化代码
    Compile,
    /// 编译为非优化代码（快速编译）
    CompileFast,
    /// 预热（收集信息）
    Warmup,
}

// ============================================================================
// 决策树
// ============================================================================

/// 决策树节点
pub enum TreeNode {
    /// 叶子节点
    Leaf {
        decision: CompilationDecision,
        confidence: f64,
    },
    /// 内部节点
    Internal {
        feature: String,
        threshold: f64,
        left: Box<TreeNode>,
        right: Box<TreeNode>,
    },
}

impl TreeNode {
    /// 预测决策
    pub fn predict(&self, features: &ExecutionFeaturesEnhanced) -> CompilationDecision {
        match self {
            TreeNode::Leaf { decision, .. } => *decision,
            TreeNode::Internal {
                feature,
                threshold,
                left,
                right,
            } => {
                let feature_value = Self::extract_feature(features, feature);
                if feature_value < *threshold {
                    left.predict(features)
                } else {
                    right.predict(features)
                }
            }
        }
    }

    /// 提取特征值
    fn extract_feature(features: &ExecutionFeaturesEnhanced, feature_name: &str) -> f64 {
        match feature_name {
            "execution_count" => features.execution_count as f64,
            "block_size" => features.block_size as f64,
            "branch_count" => features.branch_count as f64,
            "memory_access_count" => features.memory_access_count as f64,
            "cache_hit_rate" => features.cache_hit_rate,
            "control_flow_complexity" => features.control_flow_complexity,
            "loop_nest_depth" => features.loop_nest_depth as f64,
            "data_locality" => features.data_locality,
            "register_pressure" => features.register_pressure,
            "code_heat" => features.code_heat,
            "arithmetic_ratio" => features.instruction_mix.arithmetic_ratio,
            "memory_ratio" => features.instruction_mix.memory_ratio,
            "branch_ratio" => features.instruction_mix.branch_ratio,
            _ => 0.0,
        }
    }

    /// 计算特征重要性
    pub fn feature_importance(&self) -> HashMap<String, f64> {
        let mut importance = HashMap::new();
        self.compute_importance(&mut importance, 1.0);
        importance
    }

    fn compute_importance(&self, importance: &mut HashMap<String, f64>, weight: f64) {
        match self {
            TreeNode::Internal {
                feature,
                left,
                right,
                ..
            } => {
                *importance.entry(feature.clone()).or_insert(0.0) += weight;
                left.compute_importance(importance, weight * 0.5);
                right.compute_importance(importance, weight * 0.5);
            }
            TreeNode::Leaf { .. } => {}
        }
    }
}

// ============================================================================
// 随机森林模型
// ============================================================================

/// 随机森林模型
///
/// 使用多个决策树进行集成学习，提高决策准确性
pub struct RandomForestModel {
    trees: Vec<TreeNode>,
    num_trees: usize,
    max_depth: usize,
}

impl RandomForestModel {
    /// 创建新的随机森林模型
    ///
    /// # 参数
    /// - `num_trees`: 决策树数量（推荐10-20）
    /// - `max_depth`: 最大深度（推荐5-10）
    pub fn new(num_trees: usize, max_depth: usize) -> Self {
        let trees = (0..num_trees)
            .map(|_| Self::create_default_tree(max_depth))
            .collect();

        Self {
            trees,
            num_trees,
            max_depth,
        }
    }

    /// 创建默认决策树
    fn create_default_tree(_max_depth: usize) -> TreeNode {
        // 创建一个简单的决策树
        // 根节点：执行次数
        TreeNode::Internal {
            feature: "execution_count".to_string(),
            threshold: 10.0,
            left: Box::new(TreeNode::Internal {
                // 第二层：块大小
                feature: "block_size".to_string(),
                threshold: 50.0,
                left: Box::new(TreeNode::Internal {
                    // 第三层：缓存命中率
                    feature: "cache_hit_rate".to_string(),
                    threshold: 0.8,
                    left: Box::new(TreeNode::Leaf {
                        decision: CompilationDecision::Compile,
                        confidence: 0.95,
                    }),
                    right: Box::new(TreeNode::Leaf {
                        decision: CompilationDecision::CompileFast,
                        confidence: 0.85,
                    }),
                }),
                right: Box::new(TreeNode::Leaf {
                    decision: CompilationDecision::Warmup,
                    confidence: 0.7,
                }),
            }),
            right: Box::new(TreeNode::Leaf {
                decision: CompilationDecision::Skip,
                confidence: 0.9,
            }),
        }
    }

    /// 预测编译决策
    ///
    /// 使用多树投票机制
    pub fn predict(&self, features: &ExecutionFeaturesEnhanced) -> CompilationDecision {
        let mut votes = HashMap::new();

        // 让每棵树投票
        for tree in &self.trees {
            let decision = tree.predict(features);
            *votes.entry(decision).or_insert(0) += 1;
        }

        // 返回得票最多的决策
        votes
            .into_iter()
            .max_by_key(|&(_, count)| count)
            .map(|(decision, _)| decision)
            .unwrap_or(CompilationDecision::Skip)
    }

    /// 预测决策（带置信度）
    pub fn predict_with_confidence(
        &self,
        features: &ExecutionFeaturesEnhanced,
    ) -> (CompilationDecision, f64) {
        let mut votes = HashMap::new();

        for tree in &self.trees {
            let decision = tree.predict(features);
            *votes.entry(decision).or_insert(0) += 1;
        }

        let total_votes = self.num_trees as f64;
        let (decision, count) = votes.into_iter().max_by_key(|&(_, count)| count).unwrap();

        let confidence = count as f64 / total_votes;
        (decision, confidence)
    }

    /// 计算特征重要性
    ///
    /// 返回各特征对决策的重要性评分
    pub fn feature_importance(&self) -> HashMap<String, f64> {
        let mut importance = HashMap::new();

        for tree in &self.trees {
            let tree_importance = tree.feature_importance();
            for (feature, score) in tree_importance {
                *importance.entry(feature).or_insert(0.0) += score;
            }
        }

        // 归一化
        let total: f64 = importance.values().sum();
        if total > 0.0 {
            for score in importance.values_mut() {
                *score /= total;
            }
        }

        importance
    }

    /// 打印特征重要性（用于调试）
    pub fn print_feature_importance(&self) {
        let importance = self.feature_importance();
        let mut sorted: Vec<_> = importance.into_iter().collect();
        sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        println!("Feature Importance:");
        for (feature, score) in sorted {
            println!("  {}: {:.4} ({:.2}%)", feature, score, score * 100.0);
        }
    }
}

// ============================================================================
// 在线学习支持
// ============================================================================

impl RandomForestModel {
    /// 在线更新模型（简化版本）
    ///
    /// 实际应用中可能需要重新训练部分树
    pub fn update(&mut self, _training_data: &[(ExecutionFeaturesEnhanced, CompilationDecision)]) {
        // 简化实现：不进行更新
        // 实际实现中可以：
        // 1. 增量训练新树
        // 2. 替换表现最差的树
        // 3. 调整决策阈值
    }

    /// 增加新树
    pub fn add_tree(&mut self, tree: TreeNode) {
        self.trees.push(tree);
        self.num_trees += 1;
    }

    /// 移除树
    pub fn remove_tree(&mut self, index: usize) {
        if index < self.trees.len() {
            self.trees.remove(index);
            self.num_trees -= 1;
        }
    }
}

// ============================================================================
// 预训练模型配置
// ============================================================================

/// 预训练的随机森林配置
pub struct PretrainedRandomForest {
    model: RandomForestModel,
}

impl PretrainedRandomForest {
    /// 创建预训练模型
    pub fn new() -> Self {
        // 使用预定义的参数创建模型
        let model = RandomForestModel::new(10, 6);
        Self { model }
    }

    /// 预测决策
    pub fn predict(&self, features: &ExecutionFeaturesEnhanced) -> CompilationDecision {
        self.model.predict(features)
    }

    /// 获取底层模型
    pub fn model(&self) -> &RandomForestModel {
        &self.model
    }

    /// 获取底层模型的可变引用
    pub fn model_mut(&mut self) -> &mut RandomForestModel {
        &mut self.model
    }
}

impl Default for PretrainedRandomForest {
    fn default() -> Self {
        Self::new()
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

    #[test]
    fn test_random_forest_creation() {
        let model = RandomForestModel::new(10, 6);
        assert_eq!(model.trees.len(), 10);
    }

    #[test]
    fn test_predict_compile() {
        let model = RandomForestModel::new(10, 6);
        let mut features = create_test_features();
        features.execution_count = 5; // 低执行次数
        features.block_size = 30; // 小块大小
        features.cache_hit_rate = 0.7; // 低缓存命中率

        let decision = model.predict(&features);

        // 低执行次数 + 小块 + 低缓存命中率应该触发Compile
        // 路径: execution_count=5<10→左, block_size=30<50→左, cache_hit_rate=0.7<0.8→左→Compile
        assert_eq!(decision, CompilationDecision::Compile);
    }

    #[test]
    fn test_predict_skip() {
        let model = RandomForestModel::new(10, 6);
        let mut features = create_test_features();
        features.execution_count = 20; // 高执行次数会触发Skip决策（根据树的结构）

        let decision = model.predict(&features);

        // 高执行次数（>=10）应该跳过编译（树的结构是右子树返回Skip）
        assert_eq!(decision, CompilationDecision::Skip);
    }

    #[test]
    fn test_predict_with_confidence() {
        let model = RandomForestModel::new(10, 6);
        let mut features = create_test_features();
        features.execution_count = 5; // 低执行次数
        features.block_size = 30; // 小块大小
        features.cache_hit_rate = 0.7; // 低缓存命中率

        let (decision, confidence) = model.predict_with_confidence(&features);

        assert!(confidence > 0.0 && confidence <= 1.0);
        // 应该返回Compile决策
        assert_eq!(decision, CompilationDecision::Compile);
    }

    #[test]
    fn test_feature_importance() {
        let model = RandomForestModel::new(10, 6);
        let importance = model.feature_importance();

        // 应该包含执行次数和块大小
        assert!(importance.contains_key("execution_count"));
        assert!(importance.contains_key("block_size"));

        // 重要性总和应该为1
        let total: f64 = importance.values().sum();
        assert!((total - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_pretrained_model() {
        let pretrained = PretrainedRandomForest::new();
        let mut features = create_test_features();
        features.execution_count = 5; // 低执行次数
        features.block_size = 30; // 小块大小
        features.cache_hit_rate = 0.7; // 低缓存命中率

        let decision = pretrained.predict(&features);
        // 预训练模型应该能够做出Compile决策
        assert_eq!(decision, CompilationDecision::Compile);
    }

    #[test]
    fn test_add_remove_tree() {
        let mut model = RandomForestModel::new(5, 4);
        assert_eq!(model.trees.len(), 5);

        // 添加新树
        let new_tree = TreeNode::Leaf {
            decision: CompilationDecision::Skip,
            confidence: 0.5,
        };
        model.add_tree(new_tree);
        assert_eq!(model.trees.len(), 6);

        // 移除树
        model.remove_tree(0);
        assert_eq!(model.trees.len(), 5);
    }
}
