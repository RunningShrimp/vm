//! 分支指令优化模块
//!
//! 实现分支预测、分支消除、条件执行等优化

use std::collections::HashMap;
use vm_ir::{IRBlock, IROp, Terminator, GuestAddr};

/// 分支预测统计信息
#[derive(Debug, Clone, Copy)]
pub struct BranchStats {
    /// 分支执行总次数
    pub total_executions: u64,
    /// 条件为真的次数
    pub true_count: u64,
    /// 条件为假的次数
    pub false_count: u64,
}

impl BranchStats {
    /// 创建新统计
    pub fn new() -> Self {
        Self {
            total_executions: 0,
            true_count: 0,
            false_count: 0,
        }
    }

    /// 获取分支被预测为真的概率
    pub fn true_probability(&self) -> f64 {
        if self.total_executions == 0 {
            0.5
        } else {
            self.true_count as f64 / self.total_executions as f64
        }
    }

    /// 获取分支偏向性（0=平衡，1=极度偏向真，-1=极度偏向假）
    pub fn bias(&self) -> f64 {
        self.true_probability() * 2.0 - 1.0
    }

    /// 更新统计信息
    pub fn update(&mut self, taken: bool) {
        self.total_executions += 1;
        if taken {
            self.true_count += 1;
        } else {
            self.false_count += 1;
        }
    }
}

/// 分支预测器
pub struct BranchPredictor {
    /// 分支统计信息 (PC -> 统计)
    stats: HashMap<GuestAddr, BranchStats>,
}

impl BranchPredictor {
    /// 创建新预测器
    pub fn new() -> Self {
        Self {
            stats: HashMap::new(),
        }
    }

    /// 记录分支执行结果
    pub fn record_branch(&mut self, pc: GuestAddr, taken: bool) {
        self.stats
            .entry(pc)
            .or_insert_with(BranchStats::new)
            .update(taken);
    }

    /// 获取分支统计
    pub fn get_stats(&self, pc: GuestAddr) -> Option<BranchStats> {
        self.stats.get(&pc).copied()
    }

    /// 预测分支方向
    pub fn predict(&self, pc: GuestAddr) -> bool {
        self.get_stats(pc)
            .map(|s| s.true_probability() >= 0.5)
            .unwrap_or(false)
    }

    /// 获取所有统计信息
    pub fn all_stats(&self) -> &HashMap<GuestAddr, BranchStats> {
        &self.stats
    }

    /// 清空统计信息
    pub fn clear(&mut self) {
        self.stats.clear();
    }
}

/// 分支消除优化
pub struct BranchElimination;

impl BranchElimination {
    /// 检查是否可以将分支转换为条件选择
    /// 
    /// 当分支的两个分支都只有一个操作且没有副作用时，
    /// 可以用 SELECT 指令替代
    pub fn can_convert_to_select(
        block: &IRBlock,
        true_block: &IRBlock,
        false_block: &IRBlock,
    ) -> bool {
        // 简化条件：两个分支都很短且没有复杂指令
        true_block.ops.len() <= 2
            && false_block.ops.len() <= 2
            && Self::all_ops_safe(&true_block.ops)
            && Self::all_ops_safe(&false_block.ops)
    }

    /// 检查所有操作是否安全（无副作用）
    fn all_ops_safe(ops: &[IROp]) -> bool {
        ops.iter().all(|op| match op {
            IROp::Add { .. }
            | IROp::Sub { .. }
            | IROp::Mul { .. }
            | IROp::Div { .. }
            | IROp::Rem { .. }
            | IROp::And { .. }
            | IROp::Or { .. }
            | IROp::Xor { .. }
            | IROp::Not { .. }
            | IROp::Sll { .. }
            | IROp::Srl { .. }
            | IROp::Sra { .. }
            | IROp::CmpEq { .. }
            | IROp::CmpNe { .. }
            | IROp::CmpLt { .. }
            | IROp::CmpGe { .. } => true,
            _ => false,
        })
    }
}

/// 条件执行优化
pub struct ConditionalExecution;

impl ConditionalExecution {
    /// 检查是否可以使用条件执行
    /// 
    /// 当分支长度短且没有相依性时，可以在两个路径上都编译代码，
    /// 然后根据条件选择结果
    pub fn can_use_conditional_execution(
        true_block: &IRBlock,
        false_block: &IRBlock,
    ) -> bool {
        // 限制大小：最多5条指令
        true_block.ops.len() <= 5 && false_block.ops.len() <= 5
    }

    /// 计算条件执行的收益
    /// 
    /// 返回预期的循环周期数节省（正数表示有收益）
    pub fn compute_benefit(
        true_len: usize,
        false_len: usize,
        branch_probability: f64,
    ) -> f64 {
        // 分支预测惩罚：约10个周期
        let branch_penalty = 10.0;
        // 条件执行额外开销：约2个周期
        let exec_overhead = 2.0;

        let true_cost = branch_len_to_cycles(true_len) as f64;
        let false_cost = branch_len_to_cycles(false_len) as f64;
        let expected_serial = branch_penalty + branch_probability * true_cost 
            + (1.0 - branch_probability) * false_cost;
        let conditional_cost = (true_len + false_len) as f64 + exec_overhead;

        expected_serial - conditional_cost
    }
}

/// 将分支长度转换为近似周期数
fn branch_len_to_cycles(len: usize) -> u64 {
    (len as u64) * 2 // 每条指令约2个周期
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_branch_stats() {
        let mut stats = BranchStats::new();
        stats.update(true);
        stats.update(true);
        stats.update(false);
        
        assert_eq!(stats.total_executions, 3);
        assert_eq!(stats.true_count, 2);
        assert_eq!(stats.true_probability(), 2.0 / 3.0);
    }

    #[test]
    fn test_branch_predictor() {
        let mut predictor = BranchPredictor::new();
        
        // 记录几次分支执行
        for _ in 0..7 {
            predictor.record_branch(0x1000, true);
        }
        for _ in 0..3 {
            predictor.record_branch(0x1000, false);
        }
        
        // 预测应该返回 true (70% 的情况下为真)
        assert!(predictor.predict(0x1000));
        
        // 获取统计
        let stats = predictor.get_stats(0x1000).unwrap();
        assert_eq!(stats.true_count, 7);
        assert_eq!(stats.false_count, 3);
    }

    #[test]
    fn test_conditional_execution_benefit() {
        let benefit = ConditionalExecution::compute_benefit(3, 2, 0.7);
        // 预期的串行执行：10 + 0.7*6 + 0.3*4 = 14.6 周期
        // 条件执行：5 + 2 = 7 周期
        // 收益：约7.6 周期
        assert!(benefit > 5.0, "benefit = {}", benefit);
    }
}
