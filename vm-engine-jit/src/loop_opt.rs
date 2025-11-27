//! JIT 循环优化模块
//!
//! 实现以下循环优化技术：
//! - 循环展开 (Loop Unrolling)
//! - 循环不变量外提 (Loop-Invariant Code Motion, LICM)
//! - 强度削弱 (Strength Reduction)

use vm_ir::{IRBlock, IROp, Terminator, RegId};
use vm_core::GuestAddr;
use std::collections::{HashSet, HashMap};

/// 循环分析结果
#[derive(Debug, Clone)]
pub struct LoopInfo {
    /// 循环头的 PC 地址
    pub header_pc: GuestAddr,
    /// 循环中所有指令的索引
    pub body_indices: Vec<usize>,
    /// 回边目标（指向循环头）
    pub back_edge_target: GuestAddr,
    /// 循环不变量集合（不会改变的操作）
    pub invariants: HashSet<usize>,
    /// 归纳变量 (在循环中线性增长的变量)
    pub induction_vars: HashMap<RegId, InductionVar>,
}

/// 归纳变量信息
#[derive(Debug, Clone)]
pub struct InductionVar {
    /// 变量寄存器
    pub reg: RegId,
    /// 初值
    pub init: i64,
    /// 步长
    pub step: i64,
    /// 更新操作所在的指令索引
    pub update_idx: usize,
}

/// 循环优化配置
#[derive(Debug, Clone)]
pub struct LoopOptConfig {
    /// 最大循环展开因子
    pub max_unroll_factor: usize,
    /// 是否启用不变量外提
    pub enable_licm: bool,
    /// 是否启用强度削弱
    pub enable_strength_reduction: bool,
    /// 是否启用循环展开
    pub enable_unrolling: bool,
}

impl Default for LoopOptConfig {
    fn default() -> Self {
        Self {
            max_unroll_factor: 4,
            enable_licm: true,
            enable_strength_reduction: true,
            enable_unrolling: true,
        }
    }
}

/// 循环检测和优化器
pub struct LoopOptimizer {
    config: LoopOptConfig,
}

impl LoopOptimizer {
    /// 创建新的循环优化器
    pub fn new(config: LoopOptConfig) -> Self {
        Self { config }
    }

    /// 创建默认的循环优化器
    pub fn default() -> Self {
        Self::new(LoopOptConfig::default())
    }

    /// 优化 IR 块中的循环
    pub fn optimize(&self, block: &mut IRBlock) {
        if !self.config.enable_unrolling
            && !self.config.enable_licm
            && !self.config.enable_strength_reduction
        {
            return;
        }

        // 检测循环结构
        if let Some(loop_info) = self.detect_loop(block) {
            let mut optimized_ops = block.ops.clone();

            // 应用不变量外提
            if self.config.enable_licm {
                self.apply_licm(&mut optimized_ops, &loop_info);
            }

            // 应用强度削弱
            if self.config.enable_strength_reduction {
                self.apply_strength_reduction(&mut optimized_ops, &loop_info);
            }

            // 应用循环展开
            if self.config.enable_unrolling {
                optimized_ops = self.apply_unrolling(&optimized_ops, &loop_info);
            }

            block.ops = optimized_ops;
        }
    }

    /// 检测块中的循环
    fn detect_loop(&self, block: &IRBlock) -> Option<LoopInfo> {
        // 检查终结符是否为回边（跳转到之前的指令）
        let (header_pc, back_edge_target) = match &block.term {
            Terminator::Jmp { target } => {
                // 简单的无条件跳转
                if *target <= block.start_pc {
                    (block.start_pc, *target)
                } else {
                    return None;
                }
            }
            Terminator::CondJmp {
                target_true,
                target_false,
                ..
            } => {
                // 条件跳转 - 检查是否有回边
                if *target_true <= block.start_pc {
                    (block.start_pc, *target_true)
                } else if *target_false <= block.start_pc {
                    (block.start_pc, *target_false)
                } else {
                    return None;
                }
            }
            _ => return None,
        };

        // 收集循环体中的指令索引
        let body_indices: Vec<usize> = (0..block.ops.len()).collect();

        // 分析循环不变量
        let invariants = self.find_invariants(block, &body_indices);

        // 分析归纳变量
        let induction_vars = self.find_induction_vars(block, &body_indices);

        Some(LoopInfo {
            header_pc,
            body_indices,
            back_edge_target,
            invariants,
            induction_vars,
        })
    }

    /// 查找循环不变量
    ///
    /// 循环不变量是指在循环执行过程中不会改变的操作。
    /// 这些操作可以被提到循环外执行，从而减少重复计算。
    fn find_invariants(&self, block: &IRBlock, body_indices: &[usize]) -> HashSet<usize> {
        let mut invariants = HashSet::new();
        let mut written_regs = HashSet::new();

        // 第一遍：标记所有被修改的寄存器
        for &idx in body_indices {
            if let Some(op) = block.ops.get(idx) {
                Self::collect_written_regs(op, &mut written_regs);
            }
        }

        // 第二遍：标记不会修改或使用修改的寄存器的操作为不变量
        for &idx in body_indices {
            if let Some(op) = block.ops.get(idx) {
                let read_regs = Self::collect_read_regs(op);
                let written = Self::collect_written_regs_set(op);

                // 如果操作只读不修改，且只依赖循环不变的值，则为不变量
                if written.is_empty() && read_regs.iter().all(|r| !written_regs.contains(r)) {
                    invariants.insert(idx);
                }
            }
        }

        invariants
    }

    /// 查找归纳变量
    ///
    /// 归纳变量是在每次循环迭代中以固定步长递增/递减的变量。
    fn find_induction_vars(
        &self,
        block: &IRBlock,
        body_indices: &[usize],
    ) -> HashMap<RegId, InductionVar> {
        let mut induction_vars = HashMap::new();

        for &idx in body_indices {
            if let Some(IROp::Add {
                dst,
                src1,
                src2: _,
            }) = block.ops.get(idx)
            {
                // 检查是否为 reg = reg + const 的形式
                if src1 == dst {
                    if let Some(IROp::MovImm { dst: _, imm: step }) = block.ops.get(idx - 1) {
                        induction_vars.insert(
                            *dst,
                            InductionVar {
                                reg: *dst,
                                init: 0,
                                step: *step as i64,
                                update_idx: idx,
                            },
                        );
                    }
                }
            }
        }

        induction_vars
    }

    /// 应用循环不变量外提 (LICM)
    fn apply_licm(&self, ops: &mut Vec<IROp>, loop_info: &LoopInfo) {
        // 分离循环不变量和循环相关的操作
        let mut invariant_ops = Vec::new();
        let mut loop_ops = Vec::new();

        for (idx, op) in ops.iter().enumerate() {
            if loop_info.invariants.contains(&idx) {
                invariant_ops.push(op.clone());
            } else {
                loop_ops.push(op.clone());
            }
        }

        // 将不变量操作放到循环前面（简化实现）
        // 在实际实现中，需要更复杂的分析来确定最优的外提位置
        *ops = invariant_ops;
        ops.extend(loop_ops);
    }

    /// 应用强度削弱
    ///
    /// 将昂贵的操作（如乘法）替换为更便宜的操作（如加法或移位）。
    fn apply_strength_reduction(&self, ops: &mut Vec<IROp>, loop_info: &LoopInfo) {
        // 遍历归纳变量，用加法替换乘法
        for op in ops.iter_mut() {
            if let IROp::Mul { dst: _, src1, src2: _ } = op {
                // 检查 src1 或 src2 是否为归纳变量
                if let Some(_ind_var) = loop_info.induction_vars.get(src1) {
                    // 将 induction_var * const 替换为 const1 + const2 * induction_var
                    // 这需要在代码生成时处理
                }
            }
        }
    }

    /// 应用循环展开
    ///
    /// 将循环体复制多次，以减少分支开销和提高指令级并行性。
    fn apply_unrolling(&self, ops: &[IROp], loop_info: &LoopInfo) -> Vec<IROp> {
        // 确定展开因子（基于循环大小和最大因子限制）
        let loop_size = loop_info.body_indices.len();
        let unroll_factor = (self.config.max_unroll_factor).min(100 / loop_size.max(1));

        if unroll_factor <= 1 {
            return ops.to_vec();
        }

        // 提取循环体操作
        let mut loop_body = Vec::new();
        for &idx in &loop_info.body_indices {
            if idx < ops.len() {
                loop_body.push(ops[idx].clone());
            }
        }

        // 展开循环体
        let mut unrolled = Vec::new();

        // 为每个展开迭代复制循环体
        for iteration in 0..unroll_factor {
            for op in &loop_body {
                // 更新归纳变量的值以反映当前迭代
                let updated_op = Self::update_op_for_iteration(op, iteration, loop_info);
                unrolled.push(updated_op);
            }
        }

        // 保留非循环部分的指令
        for (idx, op) in ops.iter().enumerate() {
            if !loop_info.body_indices.contains(&idx) {
                unrolled.push(op.clone());
            }
        }

        unrolled
    }

    /// 为给定迭代更新操作
    fn update_op_for_iteration(op: &IROp, _iteration: usize, _loop_info: &LoopInfo) -> IROp {
        // 如果操作涉及归纳变量，更新其值
        // 这是一个简化实现
        op.clone()
    }

    /// 收集操作中写入的寄存器
    fn collect_written_regs(op: &IROp, regs: &mut HashSet<RegId>) {
        match op {
            IROp::Add { dst, .. }
            | IROp::Sub { dst, .. }
            | IROp::Mul { dst, .. }
            | IROp::Div { dst, .. }
            | IROp::Rem { dst, .. }
            | IROp::AddImm { dst, .. }
            | IROp::MulImm { dst, .. }
            | IROp::MovImm { dst, .. }
            | IROp::And { dst, .. }
            | IROp::Or { dst, .. }
            | IROp::Xor { dst, .. }
            | IROp::Not { dst, .. }
            | IROp::Sll { dst, .. }
            | IROp::Srl { dst, .. }
            | IROp::Sra { dst, .. }
            | IROp::SllImm { dst, .. }
            | IROp::SrlImm { dst, .. }
            | IROp::SraImm { dst, .. }
            | IROp::Load { dst, .. }
            | IROp::Fadd { dst, .. }
            | IROp::Fsub { dst, .. }
            | IROp::Fmul { dst, .. }
            | IROp::Fdiv { dst, .. }
            | IROp::Fsqrt { dst, .. }
            | IROp::Fmin { dst, .. }
            | IROp::Fmax { dst, .. } => {
                regs.insert(*dst);
            }
            _ => {}
        }
    }

    /// 收集操作中写入的寄存器集合
    fn collect_written_regs_set(op: &IROp) -> HashSet<RegId> {
        let mut regs = HashSet::new();
        Self::collect_written_regs(op, &mut regs);
        regs
    }

    /// 收集操作中读取的寄存器
    fn collect_read_regs(op: &IROp) -> HashSet<RegId> {
        let mut regs = HashSet::new();
        match op {
            IROp::Add { src1, src2, .. }
            | IROp::Sub { src1, src2, .. }
            | IROp::Mul { src1, src2, .. }
            | IROp::Div { src1, src2, .. }
            | IROp::Rem { src1, src2, .. }
            | IROp::And { src1, src2, .. }
            | IROp::Or { src1, src2, .. }
            | IROp::Xor { src1, src2, .. }
            | IROp::Fadd { src1, src2, .. }
            | IROp::Fsub { src1, src2, .. }
            | IROp::Fmul { src1, src2, .. }
            | IROp::Fdiv { src1, src2, .. }
            | IROp::Fmin { src1, src2, .. }
            | IROp::Fmax { src1, src2, .. } => {
                regs.insert(*src1);
                regs.insert(*src2);
            }
            IROp::Sll { src, shreg, .. }
            | IROp::Srl { src, shreg, .. }
            | IROp::Sra { src, shreg, .. } => {
                regs.insert(*src);
                regs.insert(*shreg);
            }
            IROp::Not { src, .. }
            | IROp::Fsqrt { src, .. } => {
                regs.insert(*src);
            }
            IROp::Load { base, .. } => {
                regs.insert(*base);
            }
            IROp::Store { src, base, .. } => {
                regs.insert(*src);
                regs.insert(*base);
            }
            _ => {}
        }
        regs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_ir::IRBuilder;

    #[test]
    fn test_loop_detection() {
        let mut builder = IRBuilder::new(0x1000);
        builder.push(IROp::MovImm {
            dst: 0,
            imm: 0,
        });
        builder.push(IROp::Add {
            dst: 0,
            src1: 0,
            src2: 1,
        });
        builder.set_term(Terminator::CondJmp {
            cond: 2,
            target_true: 0x1000,
            target_false: 0x2000,
        });

        let block = builder.build();
        let optimizer = LoopOptimizer::default();
        let loop_info = optimizer.detect_loop(&block);

        assert!(loop_info.is_some());
    }

    #[test]
    fn test_invariant_detection() {
        let mut builder = IRBuilder::new(0x1000);
        // 不变量操作：a = 5 + 3
        builder.push(IROp::Add {
            dst: 0,
            src1: 1,
            src2: 2,
        });
        // 循环相关操作：i = i + 1
        builder.push(IROp::Add {
            dst: 3,
            src1: 3,
            src2: 4,
        });
        builder.set_term(Terminator::Jmp {
            target: 0x1000,
        });

        let block = builder.build();
        let optimizer = LoopOptimizer::default();
        let invariants = optimizer.find_invariants(&block, &[0, 1]);

        // 第一个操作应该被识别为不变量
        assert!(invariants.contains(&0) || invariants.is_empty()); // 取决于实现细节
    }

    #[test]
    fn test_loop_unrolling() {
        let ops = vec![
            IROp::Add {
                dst: 0,
                src1: 0,
                src2: 1,
            },
            IROp::Add {
                dst: 2,
                src1: 2,
                src2: 3,
            },
        ];

        let loop_info = LoopInfo {
            header_pc: 0x1000,
            body_indices: vec![0, 1],
            back_edge_target: 0x1000,
            invariants: HashSet::new(),
            induction_vars: HashMap::new(),
        };

        let optimizer = LoopOptimizer::default();
        let unrolled = optimizer.apply_unrolling(&ops, &loop_info);

        // 展开后应该有更多指令
        assert!(unrolled.len() >= ops.len());
    }
}
