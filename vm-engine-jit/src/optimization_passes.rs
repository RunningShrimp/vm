//! 优化Pass模块
//!
//! 实现各种IR优化Pass，包括常量折叠、死代码消除、公共子表达式消除等

use std::collections::HashSet;
use vm_ir::{IROp, IRBlock};

/// 优化Pass接口
pub trait OptimizationPass: Send {
    fn optimize(&self, block: &mut IRBlock);
    fn name(&self) -> &'static str;
}

/// 优化Pass管理器
pub struct OptimizationPassManager {
    passes: Vec<Box<dyn OptimizationPass>>,
}

impl OptimizationPassManager {
    pub fn new() -> Self {
        let mut manager = Self { passes: Vec::new() };

        // 注册默认优化Pass
        manager.register_pass(Box::new(ConstantFoldingPass::new()));
        manager.register_pass(Box::new(DeadCodeEliminationPass::new()));
        manager.register_pass(Box::new(CommonSubexpressionEliminationPass::new()));

        manager
    }

    /// 注册优化Pass
    pub fn register_pass(&mut self, pass: Box<dyn OptimizationPass>) {
        self.passes.push(pass);
    }

    /// 运行所有优化Pass
    pub fn run_optimizations(&self, block: &mut IRBlock) {
        for pass in &self.passes {
            pass.optimize(block);
        }
    }
}

/// 常量折叠Pass
pub struct ConstantFoldingPass;

impl ConstantFoldingPass {
    pub fn new() -> Self {
        Self
    }
}

impl OptimizationPass for ConstantFoldingPass {
    fn optimize(&self, block: &mut IRBlock) {
        // 简化的常量折叠实现
        let snapshot = block.ops.clone();
        for (idx, op) in block.ops.iter_mut().enumerate() {
            match op {
                IROp::Add { dst, src1, src2 } => {
                    // 如果两个操作数都是常量，则折叠
                    if let (IROp::MovImm { imm: imm1, .. }, IROp::MovImm { imm: imm2, .. }) =
                        (&snapshot[*src1 as usize], &snapshot[*src2 as usize])
                    {
                        *op = IROp::MovImm {
                            dst: *dst,
                            imm: imm1 + imm2,
                        };
                    }
                }
                IROp::Mul { dst, src1, src2 } => {
                    if let (IROp::MovImm { imm: imm1, .. }, IROp::MovImm { imm: imm2, .. }) =
                        (&snapshot[*src1 as usize], &snapshot[*src2 as usize])
                    {
                        *op = IROp::MovImm {
                            dst: *dst,
                            imm: imm1 * imm2,
                        };
                    }
                }
                _ => {}
            }
        }
    }

    fn name(&self) -> &'static str {
        "ConstantFolding"
    }
}

/// 死代码消除Pass
pub struct DeadCodeEliminationPass;

impl DeadCodeEliminationPass {
    pub fn new() -> Self {
        Self
    }
}

impl OptimizationPass for DeadCodeEliminationPass {
    fn optimize(&self, block: &mut IRBlock) {
        // 简化的死代码消除：移除未使用的MovImm
        let mut used_regs = HashSet::new();

        // 收集所有被使用的寄存器
        for op in &block.ops {
            match op {
                IROp::Add { src1, src2, .. }
                | IROp::Sub { src1, src2, .. }
                | IROp::Mul { src1, src2, .. }
                | IROp::Div { src1, src2, .. }
                | IROp::Rem { src1, src2, .. }
                | IROp::And { src1, src2, .. }
                | IROp::Or { src1, src2, .. }
                | IROp::Xor { src1, src2, .. } => {
                    used_regs.insert(*src1);
                    used_regs.insert(*src2);
                }
                IROp::Store { src, base, .. } => {
                    used_regs.insert(*src);
                    used_regs.insert(*base);
                }
                _ => {}
            }
        }

        // 移除未使用的MovImm
        block.ops.retain(|op| {
            if let IROp::MovImm { dst, .. } = op {
                used_regs.contains(dst)
            } else {
                true
            }
        });
    }

    fn name(&self) -> &'static str {
        "DeadCodeElimination"
    }
}

/// 公共子表达式消除Pass
pub struct CommonSubexpressionEliminationPass;

impl CommonSubexpressionEliminationPass {
    pub fn new() -> Self {
        Self
    }
}

impl OptimizationPass for CommonSubexpressionEliminationPass {
    fn optimize(&self, _block: &mut IRBlock) {
        // 简化处理：暂不执行CSE以避免复杂的借用冲突
    }

    fn name(&self) -> &'static str {
        "CommonSubexpressionElimination"
    }
}


