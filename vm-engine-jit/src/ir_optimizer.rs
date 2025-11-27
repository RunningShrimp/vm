//! 优化和预处理模块
//!
//! 为 Cranelift 转换提供优化 Pass 和预处理功能

use vm_ir::{IRBlock, IROp};
use std::collections::HashMap;

/// 优化管道配置
#[derive(Debug, Clone)]
pub struct OptimizationConfig {
    /// 启用常量折叠
    pub enable_const_folding: bool,
    /// 启用死代码消除
    pub enable_dce: bool,
    /// 启用指令合并
    pub enable_combine: bool,
    /// 优化级别 (0-3)
    pub opt_level: u8,
}

impl Default for OptimizationConfig {
    fn default() -> Self {
        Self {
            enable_const_folding: true,
            enable_dce: true,
            enable_combine: true,
            opt_level: 2,
        }
    }
}

/// 常量值
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConstValue {
    I64(i64),
    U64(u64),
    F32(f32),
    F64(f64),
    Unknown,
}

/// 常量传播分析
pub struct ConstantPropagation {
    values: HashMap<u32, ConstValue>,
}

impl ConstantPropagation {
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    /// 分析 IR 块中的常量值
    pub fn analyze(&mut self, block: &IRBlock) {
        for op in &block.ops {
            match op {
                IROp::MovImm { dst, imm } => {
                    self.values.insert(*dst, ConstValue::U64(*imm));
                }
                IROp::Add { dst, src1, src2 } => {
                    if let (Some(ConstValue::I64(v1)), Some(ConstValue::I64(v2))) =
                        (self.values.get(src1), self.values.get(src2))
                    {
                        self.values.insert(*dst, ConstValue::I64(v1 + v2));
                    } else {
                        self.values.insert(*dst, ConstValue::Unknown);
                    }
                }
                IROp::Sub { dst, src1, src2 } => {
                    if let (Some(ConstValue::I64(v1)), Some(ConstValue::I64(v2))) =
                        (self.values.get(src1), self.values.get(src2))
                    {
                        self.values.insert(*dst, ConstValue::I64(v1 - v2));
                    } else {
                        self.values.insert(*dst, ConstValue::Unknown);
                    }
                }
                IROp::Mul { dst, src1, src2 } => {
                    if let (Some(ConstValue::I64(v1)), Some(ConstValue::I64(v2))) =
                        (self.values.get(src1), self.values.get(src2))
                    {
                        self.values.insert(*dst, ConstValue::I64(v1 * v2));
                    } else {
                        self.values.insert(*dst, ConstValue::Unknown);
                    }
                }
                _ => {
                    // 其他操作设置为未知
                }
            }
        }
    }

    /// 查询寄存器的常量值
    pub fn query(&self, reg: u32) -> Option<ConstValue> {
        self.values.get(&reg).copied()
    }

    /// 清除分析结果
    pub fn clear(&mut self) {
        self.values.clear();
    }
}

/// 活性分析
pub struct LivenessAnalysis {
    /// 每个指令的活变量集合
    live_in: HashMap<usize, Vec<u32>>,
    live_out: HashMap<usize, Vec<u32>>,
}

impl LivenessAnalysis {
    pub fn new() -> Self {
        Self {
            live_in: HashMap::new(),
            live_out: HashMap::new(),
        }
    }

    /// 执行活性分析
    pub fn analyze(&mut self, block: &IRBlock) {
        let n = block.ops.len();
        
        // 初始化：末尾指令的 live_out 为空
        let mut live = Vec::new();

        // 从后往前扫描
        for i in (0..n).rev() {
            let op = &block.ops[i];
            
            // 记录这个指令的 live_in（在执行前的活变量）
            self.live_in.insert(i, live.clone());

            // 更新 live_out（当前指令使用的变量）
            let used_regs = self.extract_used_regs(op);
            for reg in used_regs {
                if !live.contains(&reg) {
                    live.push(reg);
                }
            }

            // 移除被定义的变量
            if let Some(dst) = self.extract_dest_reg(op) {
                live.retain(|&r| r != dst);
            }

            self.live_out.insert(i, live.clone());
        }
    }

    fn extract_used_regs(&self, op: &IROp) -> Vec<u32> {
        match op {
            IROp::Add { src1, src2, .. } => vec![*src1, *src2],
            IROp::Sub { src1, src2, .. } => vec![*src1, *src2],
            IROp::Mul { src1, src2, .. } => vec![*src1, *src2],
            IROp::Div { src1, src2, .. } => vec![*src1, *src2],
            IROp::And { src1, src2, .. } => vec![*src1, *src2],
            IROp::Or { src1, src2, .. } => vec![*src1, *src2],
            IROp::Xor { src1, src2, .. } => vec![*src1, *src2],
            IROp::Load { base, .. } => vec![*base],
            IROp::Store { src, base, .. } => vec![*src, *base],
            _ => Vec::new(),
        }
    }

    fn extract_dest_reg(&self, op: &IROp) -> Option<u32> {
        match op {
            IROp::Add { dst, .. }
            | IROp::Sub { dst, .. }
            | IROp::Mul { dst, .. }
            | IROp::Div { dst, .. }
            | IROp::Load { dst, .. }
            | IROp::MovImm { dst, .. } => Some(*dst),
            _ => None,
        }
    }

    /// 获取指定指令的活变量
    pub fn live_in(&self, idx: usize) -> Option<&[u32]> {
        self.live_in.get(&idx).map(|v| v.as_slice())
    }

    pub fn live_out(&self, idx: usize) -> Option<&[u32]> {
        self.live_out.get(&idx).map(|v| v.as_slice())
    }
}

/// 死代码消除
pub struct DeadCodeElimination;

impl DeadCodeElimination {
    pub fn eliminate(block: &mut IRBlock) {
        let mut live_analysis = LivenessAnalysis::new();
        live_analysis.analyze(block);

        // 标记需要保留的指令
        let mut keep = vec![false; block.ops.len()];

        // 终结符的源寄存器必须保留
        match &block.term {
            vm_ir::Terminator::Ret { value } => {
                if let Some(reg) = value {
                    Self::mark_dependencies(&block.ops, *reg, &mut keep);
                }
            }
            vm_ir::Terminator::CondJmp { cond, .. } => {
                Self::mark_dependencies(&block.ops, *cond, &mut keep);
            }
            _ => {}
        }

        // 只保留有副作用或被使用的指令
        for i in 0..block.ops.len() {
            let op = &block.ops[i];
            if Self::has_side_effects(op) || keep[i] {
                keep[i] = true;
            }
        }

        block.ops.retain_mut(|_| {
            let should_keep = keep[block.ops.len()];
            keep.pop();
            should_keep
        });
    }

    fn mark_dependencies(ops: &[IROp], reg: u32, keep: &mut [bool]) {
        // 反向查找定义该寄存器的指令
        for (i, op) in ops.iter().enumerate().rev() {
            if let Some(dst) = Self::extract_dest(op) {
                if dst == reg {
                    keep[i] = true;
                    // 递归标记该指令的依赖
                    let used = Self::extract_sources(op);
                    for src_reg in used {
                        Self::mark_dependencies(ops, src_reg, keep);
                    }
                    break;
                }
            }
        }
    }

    fn has_side_effects(op: &IROp) -> bool {
        matches!(
            op,
            IROp::Store { .. }
            | IROp::SysCall
            | IROp::AtomicRMW { .. }
            | IROp::AtomicCmpXchg { .. }
        )
    }

    fn extract_dest(op: &IROp) -> Option<u32> {
        match op {
            IROp::Add { dst, .. }
            | IROp::Sub { dst, .. }
            | IROp::Mul { dst, .. }
            | IROp::MovImm { dst, .. }
            | IROp::Load { dst, .. } => Some(*dst),
            _ => None,
        }
    }

    fn extract_sources(op: &IROp) -> Vec<u32> {
        match op {
            IROp::Add { src1, src2, .. }
            | IROp::Sub { src1, src2, .. }
            | IROp::Mul { src1, src2, .. } => vec![*src1, *src2],
            IROp::Load { base, .. } | IROp::Store { base, .. } => vec![*base],
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_propagation() {
        let mut cp = ConstantPropagation::new();
        // 创建虚拟 block 用于测试
        let block = IRBlock {
            start_pc: vm_core::GuestAddr::from(0u64),
            ops: vec![IROp::MovImm { dst: 0, imm: 42 }],
            term: vm_ir::Terminator::Ret { value: Some(0) },
        };
        
        cp.analyze(&block);
        assert_eq!(cp.query(0), Some(ConstValue::U64(42)));
    }

    #[test]
    fn test_liveness_analysis() {
        let block = IRBlock {
            start_pc: vm_core::GuestAddr::from(0u64),
            ops: vec![
                IROp::MovImm { dst: 0, imm: 10 },
                IROp::MovImm { dst: 1, imm: 20 },
                IROp::Add { dst: 2, src1: 0, src2: 1 },
            ],
            term: vm_ir::Terminator::Ret { value: Some(2) },
        };

        let mut la = LivenessAnalysis::new();
        la.analyze(&block);
        
        // 最后一条指令的 live_in 应该包含其源操作数
        assert!(la.live_in(2).unwrap_or(&[]).contains(&0));
        assert!(la.live_in(2).unwrap_or(&[]).contains(&1));
    }
}
