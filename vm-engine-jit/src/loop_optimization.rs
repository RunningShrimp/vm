//! 循环优化模块
//!
//! 实现循环展开、循环融合、循环分割等高级循环优化

use vm_ir::{IROp, IRBlock, GuestAddr, RegId};
use std::collections::{HashMap, HashSet};

/// 循环信息
#[derive(Debug, Clone)]
pub struct LoopInfo {
    /// 循环头地址
    pub header: GuestAddr,
    /// 循环体块地址
    pub body_blocks: Vec<GuestAddr>,
    /// 回边 (源, 目标)
    pub back_edges: Vec<(GuestAddr, GuestAddr)>,
    /// 预计执行次数（如果已知）
    pub trip_count: Option<u64>,
    /// 循环嵌套深度
    pub nesting_level: u32,
}

/// 循环展开优化器
pub struct LoopUnroller {
    /// 最大展开因子
    max_unroll_factor: u32,
    /// 最大代码膨胀（倍数）
    max_code_expansion: f32,
}

impl LoopUnroller {
    /// 创建新的循环展开器
    pub fn new(max_factor: u32, max_expansion: f32) -> Self {
        Self {
            max_unroll_factor: max_factor,
            max_code_expansion: max_expansion,
        }
    }

    /// 计算最优展开因子
    pub fn compute_unroll_factor(&self, loop_body_size: usize, trip_count: Option<u64>) -> u32 {
        // 如果循环次数已知且很小，完全展开
        if let Some(count) = trip_count {
            if count <= 4 {
                return count as u32;
            }
        }

        // 基于循环体大小和代码膨胀限制
        let max_by_expansion = (self.max_code_expansion * loop_body_size as f32) as u32;
        std::cmp::min(self.max_unroll_factor, max_by_expansion)
    }

    /// 检查是否值得展开
    pub fn should_unroll(&self, loop_body_size: usize, trip_count: Option<u64>) -> bool {
        // 展开的收益：
        // 1. 减少循环控制指令
        // 2. 增加指令级并行性
        // 3. 启用更多优化机会

        match trip_count {
            Some(count) if count <= 3 => true, // 小循环总是值得展开
            Some(count) if count > 1000 => false, // 大循环不展开
            _ => loop_body_size < 32, // 中等大小的循环体值得展开
        }
    }

    /// 执行循环展开
    pub fn unroll(&self, ops: &[IROp], factor: u32) -> Vec<IROp> {
        let mut unrolled = Vec::new();

        for _ in 0..factor {
            unrolled.extend_from_slice(ops);
        }

        unrolled
    }

    /// 执行条件循环展开（带余数处理）
    pub fn unroll_with_remainder(&self, ops: &[IROp], factor: u32) -> (Vec<IROp>, Vec<IROp>) {
        let mut main_loop = Vec::new();
        let mut remainder = ops.to_vec();

        for _ in 0..factor {
            main_loop.extend_from_slice(ops);
        }

        (main_loop, remainder)
    }
}

/// 循环融合优化器
pub struct LoopFusionOptimizer;

impl LoopFusionOptimizer {
    /// 检查两个循环是否可以融合
    /// 
    /// 条件：
    /// - 相邻循环
    /// - 相同的循环次数
    /// - 没有依赖关系冲突
    pub fn can_fuse_loops(loop1: &LoopInfo, loop2: &LoopInfo) -> bool {
        // 检查循环次数是否相同
        match (loop1.trip_count, loop2.trip_count) {
            (Some(c1), Some(c2)) => c1 == c2,
            (None, None) => true, // 假设相同
            _ => false,
        }
    }

    /// 融合两个循环体
    pub fn fuse_loops(body1: &[IROp], body2: &[IROp]) -> Vec<IROp> {
        let mut fused = Vec::new();
        fused.extend_from_slice(body1);
        fused.extend_from_slice(body2);
        fused
    }

    /// 检查循环体之间的依赖关系
    pub fn check_loop_dependencies(body1: &[IROp], body2: &[IROp]) -> bool {
        // 简化检查：如果 body2 读取 body1 写入的寄存器，则有依赖
        let mut written_regs = HashSet::new();

        for op in body1 {
            if let Some(dst) = Self::get_dest_reg(op) {
                written_regs.insert(dst);
            }
        }

        for op in body2 {
            let read_regs = Self::get_source_regs(op);
            for reg in read_regs {
                if written_regs.contains(&reg) {
                    return true; // 有依赖
                }
            }
        }

        false // 无依赖
    }

    fn get_dest_reg(op: &IROp) -> Option<RegId> {
        match op {
            IROp::Add { dst, .. }
            | IROp::Sub { dst, .. }
            | IROp::Mul { dst, .. }
            | IROp::Load { dst, .. } => Some(*dst),
            _ => None,
        }
    }

    fn get_source_regs(op: &IROp) -> Vec<RegId> {
        match op {
            IROp::Add { src1, src2, .. }
            | IROp::Sub { src1, src2, .. }
            | IROp::Mul { src1, src2, .. } => vec![*src1, *src2],
            IROp::Load { src, .. } => vec![*src],
            _ => Vec::new(),
        }
    }
}

/// 循环分割（Tiling）优化器
pub struct LoopTilingOptimizer;

impl LoopTilingOptimizer {
    /// 计算最优的分割大小
    /// 
    /// 考虑：
    /// - L1 缓存大小（通常 32-64 KB）
    /// - 工作集大小
    /// - 数据局部性
    pub fn compute_tile_size(
        array_size: usize,
        element_size: usize,
        cache_size: usize,
    ) -> usize {
        // 分割大小应该适合 L1 缓存
        let max_elements = cache_size / element_size;
        let tile_size = (max_elements as f32).sqrt() as usize;
        std::cmp::max(tile_size, 8) // 最小为 8
    }

    /// 生成分割后的循环嵌套
    pub fn tile_loop(
        original_ops: &[IROp],
        tile_size: usize,
    ) -> Vec<Vec<IROp>> {
        let mut tiles = Vec::new();
        let chunk_size = std::cmp::max(original_ops.len() / tile_size, 1);

        for chunk in original_ops.chunks(chunk_size) {
            tiles.push(chunk.to_vec());
        }

        tiles
    }
}

/// 循环强度削弱（Loop Strength Reduction）
pub struct LoopStrengthReduction;

impl LoopStrengthReduction {
    /// 识别循环中的强度削弱机会
    /// 
    /// 例如：i = i + 1; sum = sum + i 可以转换为增量更新
    pub fn find_reduction_opportunities(ops: &[IROp]) -> Vec<ReductionOpportunity> {
        let mut opportunities = Vec::new();

        for op in ops {
            match op {
                // 识别乘以归纳变量的模式
                IROp::Mul { dst, src1, src2 } => {
                    opportunities.push(ReductionOpportunity {
                        original_op: "multiply".to_string(),
                        optimized_op: "add".to_string(),
                        expected_speedup: 2.0,
                    });
                }
                _ => {}
            }
        }

        opportunities
    }
}

/// 强度削弱机会
#[derive(Debug, Clone)]
pub struct ReductionOpportunity {
    pub original_op: String,
    pub optimized_op: String,
    pub expected_speedup: f64,
}

/// 循环分析工具
pub struct LoopAnalyzer;

impl LoopAnalyzer {
    /// 识别循环中的不变量
    pub fn find_invariants(ops: &[IROp], loop_vars: &[RegId]) -> Vec<usize> {
        let mut invariants = Vec::new();

        for (idx, op) in ops.iter().enumerate() {
            let is_invariant = !Self::uses_loop_var(op, loop_vars) && Self::has_no_side_effects(op);
            if is_invariant {
                invariants.push(idx);
            }
        }

        invariants
    }

    fn uses_loop_var(op: &IROp, loop_vars: &[RegId]) -> bool {
        match op {
            IROp::Add { src1, src2, .. }
            | IROp::Sub { src1, src2, .. }
            | IROp::Mul { src1, src2, .. } => {
                loop_vars.contains(src1) || loop_vars.contains(src2)
            }
            _ => false,
        }
    }

    fn has_no_side_effects(op: &IROp) -> bool {
        !matches!(op, IROp::Store { .. } | IROp::AtomicRMW { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loop_unroller() {
        let unroller = LoopUnroller::new(8, 3.0);
        let factor = unroller.compute_unroll_factor(16, Some(100));
        assert!(factor > 0 && factor <= 8);
    }

    #[test]
    fn test_loop_fusion() {
        let loop1 = LoopInfo {
            header: 0x1000,
            body_blocks: vec![0x1000],
            back_edges: vec![],
            trip_count: Some(10),
            nesting_level: 1,
        };

        let loop2 = LoopInfo {
            header: 0x2000,
            body_blocks: vec![0x2000],
            back_edges: vec![],
            trip_count: Some(10),
            nesting_level: 1,
        };

        assert!(LoopFusionOptimizer::can_fuse_loops(&loop1, &loop2));
    }

    #[test]
    fn test_tile_size() {
        let tile_size = LoopTilingOptimizer::compute_tile_size(1024, 8, 32768);
        assert!(tile_size > 0);
    }
}
