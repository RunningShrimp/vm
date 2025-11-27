//! 向量化优化模块
//!
//! 实现自动循环向量化、SLP 向量化、向量化数据布局等

use vm_ir::{IROp, RegId};
use std::collections::HashMap;

/// 向量化分析结果
#[derive(Debug, Clone)]
pub struct VectorizationAnalysis {
    /// 循环是否可以向量化
    pub can_vectorize: bool,
    /// 向量化宽度（元素数）
    pub vector_width: usize,
    /// 预期加速
    pub expected_speedup: f64,
    /// 障碍（如果不能向量化）
    pub barriers: Vec<String>,
}

/// 循环向量化器
pub struct LoopVectorizer {
    /// 目标向量宽度
    vector_width: usize,
}

impl LoopVectorizer {
    /// 创建新的循环向量化器
    pub fn new(vector_width: usize) -> Self {
        Self { vector_width }
    }

    /// 分析循环是否可以向量化
    pub fn analyze_loop(&self, ops: &[IROp]) -> VectorizationAnalysis {
        let mut barriers = Vec::new();

        // 检查是否存在减少操作（如求和）
        let has_reduction = Self::has_reduction(ops);
        
        // 检查是否存在数据依赖
        let has_bad_dependencies = Self::has_data_dependencies(ops);
        
        // 检查内存访问模式
        let has_strided_access = Self::has_strided_memory_access(ops);

        if has_bad_dependencies {
            barriers.push("Data dependencies".to_string());
        }

        if !has_strided_access {
            barriers.push("Non-strided memory access".to_string());
        }

        let can_vectorize = barriers.is_empty();
        let speedup = if can_vectorize {
            self.vector_width as f64 * 0.8 // 考虑开销
        } else {
            1.0
        };

        VectorizationAnalysis {
            can_vectorize,
            vector_width: self.vector_width,
            expected_speedup: speedup,
            barriers,
        }
    }

    /// 生成向量化的循环体
    pub fn vectorize_ops(&self, ops: &[IROp]) -> Vec<IROp> {
        let mut vectorized = Vec::new();

        for op in ops {
            if let Some(vec_op) = Self::convert_to_vector_op(op, self.vector_width) {
                vectorized.push(vec_op);
            } else {
                vectorized.push(op.clone());
            }
        }

        vectorized
    }

    fn convert_to_vector_op(op: &IROp, vector_width: usize) -> Option<IROp> {
        // 简化实现：使用向量操作替代标量操作
        match op {
            IROp::Add { dst, src1, src2 } => {
                Some(IROp::VecAdd {
                    dst: *dst,
                    src1: *src1,
                    src2: *src2,
                    element_type: "i64".to_string(),
                    element_count: vector_width,
                })
            }
            IROp::Sub { dst, src1, src2 } => {
                Some(IROp::VecSub {
                    dst: *dst,
                    src1: *src1,
                    src2: *src2,
                    element_type: "i64".to_string(),
                    element_count: vector_width,
                })
            }
            IROp::Mul { dst, src1, src2 } => {
                Some(IROp::VecMul {
                    dst: *dst,
                    src1: *src1,
                    src2: *src2,
                    element_type: "i64".to_string(),
                    element_count: vector_width,
                })
            }
            _ => None,
        }
    }

    fn has_reduction(ops: &[IROp]) -> bool {
        // 检查是否存在累积操作（如 sum += a[i]）
        ops.len() > 1 && ops.iter().any(|op| {
            matches!(op, IROp::Add { .. } | IROp::Mul { .. })
        })
    }

    fn has_data_dependencies(ops: &[IROp]) -> bool {
        // 简化检查：检查是否有寄存器依赖冲突
        let mut written = std::collections::HashSet::new();

        for op in ops {
            match op {
                IROp::Add { dst, src1, src2, .. }
                | IROp::Sub { dst, src1, src2, .. }
                | IROp::Mul { dst, src1, src2, .. } => {
                    if written.contains(src1) || written.contains(src2) {
                        return true; // 有真数据依赖
                    }
                    written.insert(*dst);
                }
                _ => {}
            }
        }

        false
    }

    fn has_strided_memory_access(ops: &[IROp]) -> bool {
        // 检查是否有规则的内存访问模式
        let loads: Vec<_> = ops.iter().filter(|op| matches!(op, IROp::Load { .. })).collect();
        loads.len() >= 1 // 至少有一个加载
    }
}

/// SLP 向量化（超字级并行）
pub struct SLPVectorizer {
    /// 最大向量宽度
    max_vector_width: usize,
}

impl SLPVectorizer {
    /// 创建新的 SLP 向量化器
    pub fn new(max_width: usize) -> Self {
        Self {
            max_vector_width: max_width,
        }
    }

    /// 在基本块内应用 SLP 向量化
    pub fn vectorize_block(&self, ops: &[IROp]) -> Vec<IROp> {
        let mut vectorized = Vec::new();
        let mut i = 0;

        while i < ops.len() {
            // 尝试找到可以并行化的操作序列
            if let Some((group_size, vec_op)) = self.find_vectorizable_sequence(&ops[i..]) {
                vectorized.push(vec_op);
                i += group_size;
            } else {
                vectorized.push(ops[i].clone());
                i += 1;
            }
        }

        vectorized
    }

    fn find_vectorizable_sequence(&self, ops: &[IROp]) -> Option<(usize, IROp)> {
        // 检查是否有多个独立的相同操作
        if ops.len() < 2 {
            return None;
        }

        let op = &ops[0];
        let mut group_size = 1;

        for next_op in &ops[1..] {
            if Self::same_operation_type(op, next_op) && group_size < self.max_vector_width {
                group_size += 1;
            } else {
                break;
            }
        }

        if group_size >= 2 {
            // 可以向量化
            Some((group_size, ops[0].clone()))
        } else {
            None
        }
    }

    fn same_operation_type(op1: &IROp, op2: &IROp) -> bool {
        match (op1, op2) {
            (IROp::Add { .. }, IROp::Add { .. }) => true,
            (IROp::Sub { .. }, IROp::Sub { .. }) => true,
            (IROp::Mul { .. }, IROp::Mul { .. }) => true,
            (IROp::Load { .. }, IROp::Load { .. }) => true,
            _ => false,
        }
    }
}

/// 向量化数据布局优化器
pub struct VectorizationDataLayout;

impl VectorizationDataLayout {
    /// 计算最优的数据布局（AoS vs SoA）
    /// 
    /// AoS (Array of Structures): [x0, y0, z0, x1, y1, z1, ...]
    /// SoA (Structure of Arrays): [x0, x1, x2, ..., y0, y1, y2, ..., z0, z1, z2, ...]
    pub fn analyze_layout(num_elements: usize, element_size: usize, vector_width: usize) -> DataLayoutRecommendation {
        let aos_efficiency = Self::compute_aos_efficiency(num_elements, element_size, vector_width);
        let soa_efficiency = Self::compute_soa_efficiency(num_elements, element_size, vector_width);

        if soa_efficiency > aos_efficiency {
            DataLayoutRecommendation {
                recommendation: "SoA".to_string(),
                efficiency_gain: soa_efficiency - aos_efficiency,
            }
        } else {
            DataLayoutRecommendation {
                recommendation: "AoS".to_string(),
                efficiency_gain: aos_efficiency - soa_efficiency,
            }
        }
    }

    fn compute_aos_efficiency(num_elements: usize, element_size: usize, vector_width: usize) -> f64 {
        let stride = element_size;
        let vector_size = vector_width * 8;

        if stride == vector_size {
            1.0 // 完全对齐
        } else {
            0.5 // 次优对齐
        }
    }

    fn compute_soa_efficiency(num_elements: usize, _element_size: usize, _vector_width: usize) -> f64 {
        if num_elements % 16 == 0 {
            0.95 // 很好的对齐
        } else {
            0.85
        }
    }
}

/// 数据布局推荐
#[derive(Debug)]
pub struct DataLayoutRecommendation {
    pub recommendation: String,
    pub efficiency_gain: f64,
}

/// 向量化代价模型
pub struct VectorizationCostModel;

impl VectorizationCostModel {
    /// 估计向量化的成本
    pub fn estimate_cost(
        ops: &[IROp],
        vector_width: usize,
        has_unaligned_access: bool,
    ) -> VectorizationCost {
        let scalar_cost = Self::compute_scalar_cost(ops);
        let vector_cost = Self::compute_vector_cost(ops, vector_width, has_unaligned_access);

        VectorizationCost {
            scalar_cost,
            vector_cost,
            speedup: (scalar_cost as f64) / (vector_cost as f64),
        }
    }

    fn compute_scalar_cost(ops: &[IROp]) -> u32 {
        ops.len() as u32 * 2 // 每个操作约 2 个周期
    }

    fn compute_vector_cost(ops: &[IROp], vector_width: usize, has_unaligned: bool) -> u32 {
        let base_cost = (ops.len() as u32) * 2 / (vector_width as u32);
        let unaligned_penalty = if has_unaligned { 5 } else { 0 };
        base_cost + unaligned_penalty
    }
}

/// 向量化成本
#[derive(Debug)]
pub struct VectorizationCost {
    pub scalar_cost: u32,
    pub vector_cost: u32,
    pub speedup: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loop_vectorizer() {
        let vectorizer = LoopVectorizer::new(4);
        let analysis = vectorizer.analyze_loop(&[]);
        assert_eq!(analysis.vector_width, 4);
    }

    #[test]
    fn test_slp_vectorizer() {
        let slp = SLPVectorizer::new(8);
        let ops = vec![];
        let vectorized = slp.vectorize_block(&ops);
        assert_eq!(vectorized.len(), 0);
    }

    #[test]
    fn test_data_layout_recommendation() {
        let rec = VectorizationDataLayout::analyze_layout(1024, 16, 4);
        assert!(!rec.recommendation.is_empty());
    }
}
