// 马尔可夫链预测器
//
// 本模块实现了基于马尔可夫链的地址预测算法，用于TLB动态预热。
// 马尔可夫链通过状态转移矩阵来预测下一个可能的状态（访问模式）。

use super::access_pattern::PatternType;
use std::collections::HashMap;
use vm_core::GuestAddr;

/// 马尔可夫链预测器
///
/// 使用状态转移矩阵来预测下一个可能的访问模式，从而预测下一个访问地址。
pub struct MarkovPredictor {
    /// 状态转移矩阵：从状态A转移到状态B的概率
    transition_matrix: HashMap<(PatternType, PatternType), TransitionProbability>,
    /// 当前状态
    current_state: PatternType,
    /// N-gram模型阶数（默认2）
    n_gram: usize,
    /// 学习率（0.0-1.0，默认0.1）
    learning_rate: f64,
    /// 总预测次数
    pub total_predictions: u64,
    /// 准确预测次数
    pub correct_predictions: u64,
}

/// 状态转移概率
#[derive(Debug, Clone)]
struct TransitionProbability {
    /// 转移概率（0.0-1.0）
    probability: f64,
    /// 转移次数
    count: u64,
    /// 最后更新时间
    last_updated: u64,
}

impl TransitionProbability {
    fn new() -> Self {
        Self {
            probability: 0.0,
            count: 0,
            last_updated: 0,
        }
    }

    fn update(&mut self, probability: f64, timestamp: u64) {
        self.probability = probability;
        self.count += 1;
        self.last_updated = timestamp;
    }
}

impl MarkovPredictor {
    /// 创建新的马尔可夫链预测器
    ///
    /// # 参数
    /// - `n_gram`: N-gram模型阶数（默认2）
    /// - `learning_rate`: 学习率（0.0-1.0，默认0.1）
    ///
    /// # 示例
    /// ```ignore
    /// let predictor = MarkovPredictor::new(2, 0.1);
    /// ```
    pub fn new(n_gram: usize, learning_rate: f64) -> Self {
        Self {
            transition_matrix: HashMap::new(),
            current_state: PatternType::Random,
            n_gram,
            learning_rate,
            total_predictions: 0,
            correct_predictions: 0,
        }
    }

    /// 使用默认配置创建预测器
    #[deprecated(note = "Use Default trait instead")]
    #[allow(clippy::should_implement_trait)]
    pub fn default() -> Self {
        Self::new(2, 0.1)
    }
}

impl Default for MarkovPredictor {
    fn default() -> Self {
        Self::new(2, 0.1)
    }
}

impl MarkovPredictor {
    /// 基于当前状态预测下一个访问地址
    ///
    /// # 参数
    /// - `current_addr`: 当前访问的地址
    /// - `prediction_count`: 预测的地址数量（默认3个）
    ///
    /// # 返回
    /// 预测的地址列表（按概率排序）
    pub fn predict(&mut self, current_addr: u64, prediction_count: usize) -> Vec<GuestAddr> {
        self.total_predictions += 1;

        // 获取所有可能的下一个状态
        let transitions: Vec<_> = self
            .transition_matrix
            .iter()
            .filter(|((from_state, _), _)| *from_state == self.current_state)
            .map(|((from, _to), prob)| (_to, *from, prob))
            .collect();

        if transitions.is_empty() {
            // 没有转移数据，无法预测
            return vec![];
        }

        // 按概率排序
        let mut sorted_transitions = transitions;
        sorted_transitions.sort_by(|a, b| {
            b.2.probability
                .partial_cmp(&a.2.probability)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // 根据最可能的模式生成预测
        let page_size = 4096; // 4KB页大小
        let mut predictions = Vec::new();

        for (pattern, _, _) in sorted_transitions.iter().take(prediction_count) {
            let addrs = self.generate_addresses_for_pattern(current_addr, **pattern, page_size);
            predictions.extend(addrs);
            if predictions.len() >= prediction_count {
                break;
            }
        }

        predictions.truncate(prediction_count);
        predictions
    }

    /// 为特定模式生成预测地址
    fn generate_addresses_for_pattern(
        &self,
        current_addr: u64,
        pattern: PatternType,
        page_size: u64,
    ) -> Vec<GuestAddr> {
        match pattern {
            PatternType::Sequential => {
                // 顺序访问：下一个页面
                let next_page = (current_addr / page_size + 1) * page_size;
                vec![GuestAddr(next_page)]
            }
            PatternType::Loop => {
                // 循环访问：根据历史返回可能的地址
                // 这里简化为返回当前页面的开始
                vec![GuestAddr(current_addr)]
            }
            PatternType::Stride => {
                // 步进访问：根据常见步长预测
                let next_page = (current_addr / page_size + 1) * page_size;
                vec![GuestAddr(next_page), GuestAddr(next_page + page_size)]
            }
            PatternType::Random => {
                // 随机访问：无法预测
                vec![]
            }
        }
    }

    /// 更新模型（记录实际转移）
    ///
    /// # 参数
    /// - `next_pattern`: 实际的下一个模式
    /// - `predicted`: 是否预测正确
    pub fn update(&mut self, next_pattern: PatternType, predicted: bool) {
        let from_state = self.current_state;
        let to_state = next_pattern;

        // 更新转移矩阵
        let key = (from_state, to_state);
        let current_prob = self
            .transition_matrix
            .get(&key)
            .map(|t| t.probability)
            .unwrap_or(0.1); // 初始概率

        // 应用学习率
        let new_prob = current_prob + (1.0 - current_prob) * self.learning_rate;

        // 更新或创建转移概率
        self.transition_matrix
            .entry(key)
            .or_insert_with(TransitionProbability::new)
            .update(new_prob, self.total_predictions);

        // 更新当前状态
        self.current_state = to_state;

        // 记录预测准确性
        if predicted {
            self.correct_predictions += 1;
        }
    }

    /// 基于多个历史状态预测（高阶马尔可夫链）
    ///
    /// # 参数
    /// - `history`: 最近的历史模式列表
    /// - `current_addr`: 当前访问的地址
    /// - `prediction_count`: 预测的地址数量
    ///
    /// # 返回
    /// 预测的地址列表
    pub fn predict_with_history(
        &mut self,
        history: &[PatternType],
        current_addr: u64,
        prediction_count: usize,
    ) -> Vec<GuestAddr> {
        if history.len() < self.n_gram {
            // 历史数据不足，使用简单预测
            return self.predict(current_addr, prediction_count);
        }

        // 使用最近的N个状态进行预测
        let recent_states = &history[history.len() - self.n_gram..];

        // 查找匹配的历史序列
        let matches: Vec<_> = self
            .transition_matrix
            .iter()
            .filter(|((from, _), _)| recent_states.contains(from))
            .collect();

        if matches.is_empty() {
            // 没有匹配，使用简单预测
            return self.predict(current_addr, prediction_count);
        }

        // 计算最可能的下一个状态
        let mut next_states = HashMap::new();
        for ((_, to), prob) in matches {
            *next_states.entry(to).or_insert(0.0) += prob.probability;
        }

        // 按概率排序
        let mut sorted_states: Vec<_> = next_states.into_iter().collect();
        sorted_states.sort_by(|a, b| {
            b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
        });

        // 生成预测
        let page_size = 4096;
        let mut predictions = Vec::new();

        for (pattern, _) in sorted_states.iter().take(prediction_count) {
            let addrs = self.generate_addresses_for_pattern(current_addr, **pattern, page_size);
            predictions.extend(addrs);
            if predictions.len() >= prediction_count {
                break;
            }
        }

        predictions.truncate(prediction_count);
        predictions
    }

    /// 获取预测准确率
    pub fn get_accuracy(&self) -> f64 {
        if self.total_predictions == 0 {
            return 0.0;
        }
        self.correct_predictions as f64 / self.total_predictions as f64
    }

    /// 获取转移矩阵统计信息
    pub fn get_transition_stats(&self) -> TransitionStats {
        let mut stats = HashMap::new();
        let mut total_transitions = 0u64;

        for ((from, _to), prob) in &self.transition_matrix {
            *stats.entry(from).or_insert(0u64) += prob.count;
            total_transitions += prob.count;
        }

        let from_count = stats.len();
        let avg_transitions = if from_count > 0 {
            total_transitions as f64 / from_count as f64
        } else {
            0.0
        };

        let to_count = stats.len();
        TransitionStats {
            from_count,
            to_count,
            total_transitions,
            avg_transitions,
            accuracy: self.get_accuracy(),
        }
    }

    /// 清空转移矩阵
    pub fn clear(&mut self) {
        self.transition_matrix.clear();
        self.current_state = PatternType::Random;
        self.total_predictions = 0;
        self.correct_predictions = 0;
    }

    /// 获取当前状态
    pub fn current_state(&self) -> PatternType {
        self.current_state
    }

    /// 设置学习率
    pub fn set_learning_rate(&mut self, rate: f64) {
        self.learning_rate = rate.clamp(0.0, 1.0);
    }

    /// 获取学习率
    pub fn learning_rate(&self) -> f64 {
        self.learning_rate
    }

    /// 获取预测准确率
    pub fn prediction_accuracy(&self) -> f64 {
        self.get_accuracy()
    }
}

/// 转移统计信息
#[derive(Debug, Clone)]
pub struct TransitionStats {
    /// 起始状态数量
    pub from_count: usize,
    /// 目标状态数量
    pub to_count: usize,
    /// 总转移次数
    pub total_transitions: u64,
    /// 平均每个状态的转移次数
    pub avg_transitions: f64,
    /// 预测准确率
    pub accuracy: f64,
}

impl std::fmt::Display for TransitionStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "马尔可夫链转移统计")?;
        writeln!(f, "  起始状态数: {}", self.from_count)?;
        writeln!(f, "  目标状态数: {}", self.to_count)?;
        writeln!(f, "  总转移次数: {}", self.total_transitions)?;
        writeln!(f, "  平均转移数: {:.2}", self.avg_transitions)?;
        writeln!(f, "  预测准确率: {:.2}%", self.accuracy * 100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::super::access_pattern::PatternType;
    use super::*;

    #[test]
    fn test_markov_predictor_creation() {
        let predictor = MarkovPredictor::new(2, 0.1);
        assert_eq!(predictor.current_state(), PatternType::Random);
        assert_eq!(predictor.get_accuracy(), 0.0);
    }

    #[test]
    fn test_markov_predictor_default() {
        let predictor = MarkovPredictor::default();
        assert_eq!(predictor.n_gram, 2);
        assert_eq!(predictor.learning_rate, 0.1);
    }

    #[test]
    fn test_predict_no_transitions() {
        let mut predictor = MarkovPredictor::new(2, 0.1);
        let predictions = predictor.predict(0x1000, 3);
        assert_eq!(predictions.len(), 0);
    }

    #[test]
    fn test_predict_with_transitions() {
        let mut predictor = MarkovPredictor::new(2, 0.1);

        // 记录一些转移
        predictor.update(PatternType::Sequential, false);
        predictor.update(PatternType::Sequential, true);
        predictor.update(PatternType::Sequential, false);

        // 预测（ Sequential -> Sequential）
        let predictions = predictor.predict(0x1000, 3);
        assert!(predictions.len() > 0);
    }

    #[test]
    fn test_update_accuracy() {
        let mut predictor = MarkovPredictor::new(2, 0.1);

        // 先调用predict来增加total_predictions
        let _predictions = predictor.predict(0x1000, 3);
        assert_eq!(predictor.total_predictions, 1);

        // 更新并记录准确性 - update只更新correct_predictions，不增加total_predictions
        predictor.update(PatternType::Sequential, true);
        // 现在total_predictions = 1, correct_predictions = 1
        assert!((predictor.get_accuracy() - 1.0).abs() < 0.01);

        // 再次predict以增加total_predictions
        let _predictions = predictor.predict(0x1000, 3);
        // 现在total_predictions = 2

        predictor.update(PatternType::Random, false);
        // correct_predictions仍然是1，因为predicted=false
        // total_predictions = 2, correct_predictions = 1
        assert!((predictor.get_accuracy() - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_learning_rate() {
        let mut predictor = MarkovPredictor::new(2, 0.1);
        assert_eq!(predictor.learning_rate(), 0.1);

        predictor.set_learning_rate(0.5);
        assert_eq!(predictor.learning_rate(), 0.5);

        predictor.set_learning_rate(1.5);
        assert_eq!(predictor.learning_rate(), 1.0); // 限制为最大值1.0
    }

    #[test]
    fn test_predict_with_history() {
        let mut predictor = MarkovPredictor::new(2, 0.1);

        let history = vec![
            PatternType::Sequential,
            PatternType::Sequential,
            PatternType::Loop,
        ];

        // 记录转移
        predictor.update(PatternType::Sequential, false);
        predictor.update(PatternType::Loop, false);

        let predictions = predictor.predict_with_history(&history, 0x1000, 3);
        // 应该返回一些预测（或空，如果数据不足）
        assert!(predictions.len() <= 3);
    }

    #[test]
    fn test_transition_stats() {
        let mut predictor = MarkovPredictor::new(2, 0.1);

        // 记录一些转移
        predictor.update(PatternType::Sequential, false);
        predictor.update(PatternType::Sequential, true);
        predictor.update(PatternType::Loop, false);
        predictor.update(PatternType::Random, true);

        let stats = predictor.get_transition_stats();
        assert!(stats.from_count > 0);
        assert!(stats.total_transitions > 0);
        assert!(stats.avg_transitions > 0.0);
    }

    #[test]
    fn test_clear() {
        let mut predictor = MarkovPredictor::new(2, 0.1);

        // 使用predict方法来增加total_predictions
        let predictions = predictor.predict(0x1000, 3);
        // predict会增加total_predictions
        assert!(predictor.total_predictions > 0);

        // 记录一些转移
        predictor.update(PatternType::Sequential, false);
        predictor.update(PatternType::Loop, true);

        assert!(predictor.transition_matrix.len() > 0);

        // 清空
        predictor.clear();

        assert_eq!(predictor.transition_matrix.len(), 0);
        assert_eq!(predictor.total_predictions, 0);
        assert_eq!(predictor.get_accuracy(), 0.0);
    }
}
