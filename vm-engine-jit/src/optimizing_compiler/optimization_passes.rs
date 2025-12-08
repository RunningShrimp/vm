//! 优化Pass管理器实现
//!
//! 实现各种编译优化Pass，包括常量折叠、死代码消除等

use std::collections::HashMap;
use vm_ir::{IRBlock, IROp, RegId};

/// 优化Pass接口
pub trait OptimizationPass {
    /// 获取Pass名称
    fn name(&self) -> &'static str;
    
    /// 执行优化
    fn run(&mut self, block: &mut IRBlock) -> bool;
    
    /// 获取Pass统计信息
    fn get_stats(&self) -> OptimizationPassStats;
}

/// 优化Pass统计信息
#[derive(Debug, Clone, Default)]
pub struct OptimizationPassStats {
    /// 执行次数
    pub executions: u64,
    /// 优化次数（实际修改代码的次数）
    pub optimizations: u64,
    /// 平均执行时间（纳秒）
    pub avg_execution_time_ns: u64,
}

/// 优化Pass管理器
pub struct OptimizationPassManager {
    /// 注册的优化Pass
    passes: Vec<Box<dyn OptimizationPass>>,
    /// 全局统计信息
    stats: OptimizationManagerStats,
    /// 是否启用优化
    enabled: bool,
}

/// 优化管理器统计信息
#[derive(Debug, Clone, Default)]
pub struct OptimizationManagerStats {
    /// 总执行次数
    pub total_executions: u64,
    /// 总优化次数
    pub total_optimizations: u64,
    /// 各Pass执行统计
    pub pass_stats: HashMap<String, OptimizationPassStats>,
    /// 总执行时间（纳秒）
    pub total_execution_time_ns: u64,
}

impl OptimizationPassManager {
    /// 创建新的优化Pass管理器
    pub fn new() -> Self {
        let mut manager = Self {
            passes: Vec::new(),
            stats: OptimizationManagerStats::default(),
            enabled: true,
        };
        
        // 注册默认的优化Pass
        manager.register_default_passes();
        manager
    }
    
    /// 注册默认的优化Pass
    fn register_default_passes(&mut self) {
        // 按照执行顺序注册Pass
        self.add_pass(Box::new(ConstantFoldingPass::new()));
        self.add_pass(Box::new(DeadCodeEliminationPass::new()));
        self.add_pass(Box::new(CommonSubexpressionEliminationPass::new()));
        self.add_pass(Box::new(CopyPropagationPass::new()));
    }
    
    /// 添加优化Pass
    pub fn add_pass(&mut self, pass: Box<dyn OptimizationPass>) {
        self.passes.push(pass);
    }
    
    /// 启用/禁用优化
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
    
    /// 运行所有优化Pass
    pub fn run_optimizations(&mut self, block: &mut IRBlock) {
        if !self.enabled {
            return;
        }
        
        let start_time = std::time::Instant::now();
        self.stats.total_executions += 1;
        
        // 运行每个Pass
        for pass in &mut self.passes {
            let pass_start = std::time::Instant::now();
            let modified = pass.run(block);
            let pass_elapsed = pass_start.elapsed().as_nanos() as u64;
            
            // 更新Pass统计
            let mut pass_stats = pass.get_stats();
            pass_stats.executions += 1;
            if modified {
                pass_stats.optimizations += 1;
                self.stats.total_optimizations += 1;
            }
            // 修复平均时间计算：应该使用累加平均而不是简单平均
            pass_stats.avg_execution_time_ns =
                (pass_stats.avg_execution_time_ns * (pass_stats.executions - 1) + pass_elapsed) / pass_stats.executions;
            
            self.stats.pass_stats.insert(pass.name().to_string(), pass_stats);
        }
        
        // 更新总统计
        let elapsed = start_time.elapsed().as_nanos() as u64;
        // 修复平均时间计算：使用累加平均
        self.stats.total_execution_time_ns =
            (self.stats.total_execution_time_ns * (self.stats.total_executions - 1) + elapsed) / self.stats.total_executions;
    }
    
    /// 获取统计信息
    pub fn get_stats(&self) -> OptimizationManagerStats {
        self.stats.clone()
    }
}

impl Default for OptimizationPassManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 常量折叠Pass
pub struct ConstantFoldingPass {
    stats: OptimizationPassStats,
}

impl ConstantFoldingPass {
    pub fn new() -> Self {
        Self {
            stats: OptimizationPassStats::default(),
        }
    }
}

impl OptimizationPass for ConstantFoldingPass {
    fn name(&self) -> &'static str {
        "constant_folding"
    }
    
    fn run(&mut self, block: &mut IRBlock) -> bool {
        let mut modified = false;
        
        // 简化实现：只处理基本的常量运算
        for i in 0..block.ops.len() {
            let op = &block.ops[i];
            
            match op {
                IROp::Add { dst, src1, src2 } => {
                    // 检查是否两个操作数都是常量
                    if let (Some(IROp::MovImm { imm: imm1, .. }), Some(IROp::MovImm { imm: imm2, .. })) =
                        (block.ops.get(*src1 as usize - 1), block.ops.get(*src2 as usize - 1)) {
                        // 执行常量折叠
                        let result = imm1 + imm2;
                        block.ops[i] = IROp::MovImm { dst: *dst, imm: result };
                        modified = true;
                    }
                }
                IROp::Mul { dst, src1, src2 } => {
                    if let (Some(IROp::MovImm { imm: imm1, .. }), Some(IROp::MovImm { imm: imm2, .. })) =
                        (block.ops.get(*src1 as usize - 1), block.ops.get(*src2 as usize - 1)) {
                        let result = imm1 * imm2;
                        block.ops[i] = IROp::MovImm { dst: *dst, imm: result };
                        modified = true;
                    }
                }
                _ => {}
            }
        }
        
        modified
    }
    
    fn get_stats(&self) -> OptimizationPassStats {
        self.stats.clone()
    }
}

/// 死代码消除Pass
pub struct DeadCodeEliminationPass {
    stats: OptimizationPassStats,
}

impl DeadCodeEliminationPass {
    pub fn new() -> Self {
        Self {
            stats: OptimizationPassStats::default(),
        }
    }
}

impl OptimizationPass for DeadCodeEliminationPass {
    fn name(&self) -> &'static str {
        "dead_code_elimination"
    }
    
    fn run(&mut self, block: &mut IRBlock) -> bool {
        let mut modified = false;
        
        // 简化实现：移除没有使用的MovImm指令
        let mut used_regs = std::collections::HashSet::new();
        
        // 收集所有被使用的寄存器
        for op in &block.ops {
            match op {
                IROp::Add { src1, src2, .. } => {
                    used_regs.insert(*src1);
                    used_regs.insert(*src2);
                }
                IROp::Mul { src1, src2, .. } => {
                    used_regs.insert(*src1);
                    used_regs.insert(*src2);
                }
                IROp::Sub { src1, src2, .. } => {
                    used_regs.insert(*src1);
                    used_regs.insert(*src2);
                }
                IROp::Div { src1, src2, .. } => {
                    used_regs.insert(*src1);
                    used_regs.insert(*src2);
                }
                IROp::Rem { src1, src2, .. } => {
                    used_regs.insert(*src1);
                    used_regs.insert(*src2);
                }
                IROp::And { src1, src2, .. } => {
                    used_regs.insert(*src1);
                    used_regs.insert(*src2);
                }
                IROp::Or { src1, src2, .. } => {
                    used_regs.insert(*src1);
                    used_regs.insert(*src2);
                }
                IROp::Xor { src1, src2, .. } => {
                    used_regs.insert(*src1);
                    used_regs.insert(*src2);
                }
                IROp::Not { src, .. } => {
                    used_regs.insert(*src);
                }
                IROp::Sll { src, shreg, .. } => {
                    used_regs.insert(*src);
                    used_regs.insert(*shreg);
                }
                IROp::Srl { src, shreg, .. } => {
                    used_regs.insert(*src);
                    used_regs.insert(*shreg);
                }
                IROp::Sra { src, shreg, .. } => {
                    used_regs.insert(*src);
                    used_regs.insert(*shreg);
                }
                IROp::AddImm { src, .. } => {
                    used_regs.insert(*src);
                }
                IROp::MulImm { src, .. } => {
                    used_regs.insert(*src);
                }
                IROp::SllImm { src, .. } => {
                    used_regs.insert(*src);
                }
                IROp::SrlImm { src, .. } => {
                    used_regs.insert(*src);
                }
                IROp::SraImm { src, .. } => {
                    used_regs.insert(*src);
                }
                IROp::CmpEq { lhs, rhs, .. } => {
                    used_regs.insert(*lhs);
                    used_regs.insert(*rhs);
                }
                IROp::CmpNe { lhs, rhs, .. } => {
                    used_regs.insert(*lhs);
                    used_regs.insert(*rhs);
                }
                IROp::CmpLt { lhs, rhs, .. } => {
                    used_regs.insert(*lhs);
                    used_regs.insert(*rhs);
                }
                IROp::CmpLtU { lhs, rhs, .. } => {
                    used_regs.insert(*lhs);
                    used_regs.insert(*rhs);
                }
                IROp::CmpGe { lhs, rhs, .. } => {
                    used_regs.insert(*lhs);
                    used_regs.insert(*rhs);
                }
                IROp::CmpGeU { lhs, rhs, .. } => {
                    used_regs.insert(*lhs);
                    used_regs.insert(*rhs);
                }
                IROp::Select { cond, true_val, false_val, .. } => {
                    used_regs.insert(*cond);
                    used_regs.insert(*true_val);
                    used_regs.insert(*false_val);
                }
                IROp::Load { base, .. } => {
                    used_regs.insert(*base);
                }
                IROp::Store { src, base, .. } => {
                    used_regs.insert(*src);
                    used_regs.insert(*base);
                }
                IROp::AtomicRMW { base, src, .. } => {
                    used_regs.insert(*base);
                    used_regs.insert(*src);
                }
                IROp::AtomicRMWOrder { base, src, .. } => {
                    used_regs.insert(*base);
                    used_regs.insert(*src);
                }
                IROp::AtomicCmpXchg { base, expected, new, .. } => {
                    used_regs.insert(*base);
                    used_regs.insert(*expected);
                    used_regs.insert(*new);
                }
                IROp::AtomicCmpXchgOrder { base, expected, new, .. } => {
                    used_regs.insert(*base);
                    used_regs.insert(*expected);
                    used_regs.insert(*new);
                }
                IROp::AtomicLoadReserve { base, .. } => {
                    used_regs.insert(*base);
                }
                IROp::AtomicStoreCond { src, base, dst_flag, .. } => {
                    used_regs.insert(*src);
                    used_regs.insert(*base);
                    used_regs.insert(*dst_flag);
                }
                IROp::AtomicCmpXchgFlag { base, expected, new, .. } => {
                    used_regs.insert(*base);
                    used_regs.insert(*expected);
                    used_regs.insert(*new);
                }
                IROp::AtomicRmwFlag { base, src, .. } => {
                    used_regs.insert(*base);
                    used_regs.insert(*src);
                }
                IROp::VecAdd { src1, src2, .. } => {
                    used_regs.insert(*src1);
                    used_regs.insert(*src2);
                }
                IROp::VecSub { src1, src2, .. } => {
                    used_regs.insert(*src1);
                    used_regs.insert(*src2);
                }
                IROp::VecMul { src1, src2, .. } => {
                    used_regs.insert(*src1);
                    used_regs.insert(*src2);
                }
                IROp::VecAddSat { src1, src2, .. } => {
                    used_regs.insert(*src1);
                    used_regs.insert(*src2);
                }
                IROp::VecSubSat { src1, src2, .. } => {
                    used_regs.insert(*src1);
                    used_regs.insert(*src2);
                }
                IROp::VecMulSat { src1, src2, .. } => {
                    used_regs.insert(*src1);
                    used_regs.insert(*src2);
                }
                IROp::Fadd { src1, src2, .. } => {
                    used_regs.insert(*src1);
                    used_regs.insert(*src2);
                }
                IROp::Fsub { src1, src2, .. } => {
                    used_regs.insert(*src1);
                    used_regs.insert(*src2);
                }
                IROp::Fmul { src1, src2, .. } => {
                    used_regs.insert(*src1);
                    used_regs.insert(*src2);
                }
                IROp::Fdiv { src1, src2, .. } => {
                    used_regs.insert(*src1);
                    used_regs.insert(*src2);
                }
                IROp::Fsqrt { src, .. } => {
                    used_regs.insert(*src);
                }
                IROp::Fmin { src1, src2, .. } => {
                    used_regs.insert(*src1);
                    used_regs.insert(*src2);
                }
                IROp::Fmax { src1, src2, .. } => {
                    used_regs.insert(*src1);
                    used_regs.insert(*src2);
                }
                IROp::Fload { base, .. } => {
                    used_regs.insert(*base);
                }
                IROp::Fstore { src, base, .. } => {
                    used_regs.insert(*src);
                    used_regs.insert(*base);
                }
                IROp::Beq { src1, src2, .. } => {
                    used_regs.insert(*src1);
                    used_regs.insert(*src2);
                }
                IROp::Bne { src1, src2, .. } => {
                    used_regs.insert(*src1);
                    used_regs.insert(*src2);
                }
                IROp::Blt { src1, src2, .. } => {
                    used_regs.insert(*src1);
                    used_regs.insert(*src2);
                }
                IROp::Bge { src1, src2, .. } => {
                    used_regs.insert(*src1);
                    used_regs.insert(*src2);
                }
                IROp::Bltu { src1, src2, .. } => {
                    used_regs.insert(*src1);
                    used_regs.insert(*src2);
                }
                IROp::Bgeu { src1, src2, .. } => {
                    used_regs.insert(*src1);
                    used_regs.insert(*src2);
                }
                IROp::Atomic { base, src, .. } => {
                    used_regs.insert(*base);
                    used_regs.insert(*src);
                }
                IROp::Cpuid { leaf, subleaf, .. } => {
                    used_regs.insert(*leaf);
                    used_regs.insert(*subleaf);
                }
                IROp::CsrWrite { src, .. } => {
                    used_regs.insert(*src);
                }
                IROp::CsrSet { src, .. } => {
                    used_regs.insert(*src);
                }
                IROp::CsrClear { src, .. } => {
                    used_regs.insert(*src);
                }
                IROp::WritePstateFlags { src, .. } => {
                    used_regs.insert(*src);
                }
                IROp::EvalCondition { cond, .. } => {
                    used_regs.insert((*cond).into());
                }
                IROp::VendorLoad { base, .. } => {
                    used_regs.insert(*base);
                }
                IROp::VendorStore { src, base, .. } => {
                    used_regs.insert(*src);
                    used_regs.insert(*base);
                }
                IROp::VendorVectorOp { src1, src2, .. } => {
                    used_regs.insert(*src1);
                    used_regs.insert(*src2);
                }
                _ => {}
            }
        }
        
        // 移除死代码
        block.ops.retain(|op| {
            match op {
                IROp::MovImm { dst, .. } => {
                    // 如果目标寄存器没有被使用，则移除
                    !used_regs.contains(dst)
                }
                _ => true, // 保留其他指令
            }
        });
        
        modified
    }
    
    fn get_stats(&self) -> OptimizationPassStats {
        self.stats.clone()
    }
}

/// 公共子表达式消除Pass
pub struct CommonSubexpressionEliminationPass {
    stats: OptimizationPassStats,
}

impl CommonSubexpressionEliminationPass {
    pub fn new() -> Self {
        Self {
            stats: OptimizationPassStats::default(),
        }
    }
}

impl OptimizationPass for CommonSubexpressionEliminationPass {
    fn name(&self) -> &'static str {
        "common_subexpression_elimination"
    }
    
    fn run(&mut self, _block: &mut IRBlock) -> bool {
        // 简化实现：暂不实现
        false
    }
    
    fn get_stats(&self) -> OptimizationPassStats {
        self.stats.clone()
    }
}

/// 复制传播Pass
pub struct CopyPropagationPass {
    stats: OptimizationPassStats,
}

impl CopyPropagationPass {
    pub fn new() -> Self {
        Self {
            stats: OptimizationPassStats::default(),
        }
    }
}

impl OptimizationPass for CopyPropagationPass {
    fn name(&self) -> &'static str {
        "copy_propagation"
    }
    
    fn run(&mut self, _block: &mut IRBlock) -> bool {
        // 简化实现：暂不实现
        false
    }
    
    fn get_stats(&self) -> OptimizationPassStats {
        self.stats.clone()
    }
}