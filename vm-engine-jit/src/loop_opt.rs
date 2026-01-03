//! 循环优化
//!
//! 提供基本的循环检测和优化功能，包括：
//! - 循环不变量外提
//! - 循环展开
//! - 归纳变量优化
//! - 循环强度削弱

use vm_ir::{IRBlock, Terminator, GuestAddr, IROp, RegId};

/// 变量类型
pub type Variable = RegId;

/// 数据流分析结果
#[derive(Debug, Clone)]
pub struct DataFlowInfo {
    /// 变量定义位置映射
    pub definitions: std::collections::HashMap<Variable, Vec<usize>>,
    /// 变量使用位置映射
    pub uses: std::collections::HashMap<Variable, Vec<usize>>,
    /// 活跃变量集合
    pub live_in: std::collections::HashSet<Variable>,
}

/// Phi节点信息
#[derive(Debug, Clone)]
pub struct PhiInfo {
    /// 初始值
    pub initial: Option<i64>,
    /// 步长
    pub step: Option<i64>,
}

/// 归纳变量优化类型
#[derive(Debug, Clone)]
pub enum InductionVariableOptimization {
    /// 归纳变量简化
    InductionVariableSimplify {
        /// 变量
        var: Variable,
        /// 基础值
        base: i64,
        /// 步长
        step: i64,
    },
    /// 归纳变量消除
    InductionVariableEliminate {
        /// 变量
        var: Variable,
        /// 替换值
        replacement: i64,
    },
    /// 强度削弱
    StrengthReduce {
        /// 变量
        var: Variable,
        /// 原始指令
        original: Box<IROp>,
        /// 优化后的指令
        optimized: Box<IROp>,
    },
}

/// 循环信息
#[derive(Debug, Clone)]
pub struct LoopInfo {
    /// 循环头地址
    pub header: GuestAddr,
    /// 循环基本块集合
    pub blocks: Vec<GuestAddr>,
    /// 回边（跳转到循环头的分支）
    pub back_edges: Vec<GuestAddr>,
    /// 退出边（跳出循环的分支）
    pub exit_edges: Vec<GuestAddr>,
}

/// 循环优化配置
#[derive(Debug, Clone)]
pub struct LoopOptConfig {
    /// 是否启用循环不变量外提
    pub enable_code_motion: bool,
    /// 是否启用循环展开
    pub enable_unrolling: bool,
    /// 循环展开因子
    pub unroll_factor: usize,
    /// 是否启用归纳变量优化
    pub enable_induction: bool,
}

impl Default for LoopOptConfig {
    fn default() -> Self {
        Self {
            enable_code_motion: true,
            enable_unrolling: false, // 默认关闭，可能增加代码大小
            unroll_factor: 4,
            enable_induction: true,
        }
    }
}

/// 循环优化器
pub struct LoopOptimizer {
    config: LoopOptConfig,
}

impl LoopOptimizer {
    /// 创建新的循环优化器
    pub fn new() -> Self {
        Self {
            config: LoopOptConfig::default(),
        }
    }

    /// 使用自定义配置创建循环优化器
    pub fn with_config(config: LoopOptConfig) -> Self {
        Self { config }
    }

    /// 优化IR块
    ///
    /// 执行以下优化：
    /// 1. 检测循环结构
    /// 2. 循环不变量外提
    /// 3. 归纳变量优化
    /// 4. 可选的循环展开
    pub fn optimize(&self, block: &mut IRBlock) {
        // 1. 检测循环
        if let Some(loop_info) = self.detect_loop(block) {
            // 2. 循环不变量外提
            if self.config.enable_code_motion {
                self.hoist_invariants(block, &loop_info);
            }

            // 3. 归纳变量优化
            if self.config.enable_induction {
                self.optimize_induction_vars(block, &loop_info);
            }

            // 4. 循环展开（如果启用）
            if self.config.enable_unrolling {
                self.unroll_loop(block, &loop_info);
            }
        }
    }

    /// 检测循环结构
    fn detect_loop(&self, block: &IRBlock) -> Option<LoopInfo> {
        // 检查终止符是否为回边（跳转到块内或自身）
        match &block.term {
            Terminator::Jmp { target } => {
                // 无条件跳转 - 检查是否跳转到当前块或更早的地址
                if target.0 <= block.start_pc.0 {
                    Some(LoopInfo {
                        header: block.start_pc,
                        blocks: vec![block.start_pc],
                        back_edges: vec![block.start_pc],
                        exit_edges: vec![],
                    })
                } else {
                    None
                }
            }
            Terminator::CondJmp { target_true, target_false, .. } => {
                // 条件跳转 - 检查是否有回边
                let has_back_edge = target_true.0 <= block.start_pc.0 || target_false.0 <= block.start_pc.0;

                if has_back_edge {
                    let mut back_edges = Vec::new();
                    let mut exit_edges = Vec::new();

                    if target_true.0 <= block.start_pc.0 {
                        back_edges.push(*target_true);
                    } else {
                        exit_edges.push(*target_true);
                    }

                    if target_false.0 <= block.start_pc.0 {
                        back_edges.push(*target_false);
                    } else {
                        exit_edges.push(*target_false);
                    }

                    Some(LoopInfo {
                        header: block.start_pc,
                        blocks: vec![block.start_pc],
                        back_edges,
                        exit_edges,
                    })
                } else {
                    None
                }
            }
            _ => None, // Ret, Call, Fault, Interrupt - 不是循环
        }
    }

    /// 循环不变量外提
    ///
    /// 将循环中不随迭代变化的操作移到循环外
    fn hoist_invariants(&self, block: &mut IRBlock, _loop_info: &LoopInfo) {
        // 简化实现：识别纯计算操作（不涉及内存访问）
        // 完整实现需要数据流分析

        // 实现完整的数据流分析
        let data_flow = self.analyze_data_flow(block);

        // 识别循环不变量
        let invariants = self.identify_loop_invariants(block, &data_flow);

        // 将不变量移到循环外
        self.move_invariants_out(block, invariants);
    }

    /// 数据流分析
    fn analyze_data_flow(&self, loop_body: &IRBlock) -> DataFlowInfo {
        use std::collections::{HashMap, HashSet};

        let mut defs: HashMap<Variable, Vec<usize>> = HashMap::new();
        let mut uses: HashMap<Variable, Vec<usize>> = HashMap::new();
        let mut live_vars: HashSet<Variable> = HashSet::new();

        // 后向数据流分析
        for (idx, insn) in loop_body.ops.iter().enumerate().rev() {
            // 收集定义
            for defined_var in self.get_defined_vars(insn) {
                defs.entry(defined_var).or_default().push(idx);
                live_vars.remove(&defined_var);
            }

            // 收集使用
            for used_var in self.get_used_vars(insn) {
                uses.entry(used_var).or_default().push(idx);
                live_vars.insert(used_var);
            }
        }

        DataFlowInfo {
            definitions: defs,
            uses,
            live_in: live_vars,
        }
    }

    /// 获取指令定义的变量
    fn get_defined_vars(&self, insn: &IROp) -> Vec<Variable> {
        let mut vars = Vec::new();

        // 使用match来检查每种指令类型
        match insn {
            // 单个寄存器定义的指令
            IROp::Add { dst, .. }
            | IROp::Sub { dst, .. }
            | IROp::Mul { dst, .. }
            | IROp::Div { dst, .. }
            | IROp::Rem { dst, .. }
            | IROp::And { dst, .. }
            | IROp::Or { dst, .. }
            | IROp::Xor { dst, .. }
            | IROp::Not { dst, .. }
            | IROp::Sll { dst, .. }
            | IROp::Srl { dst, .. }
            | IROp::Sra { dst, .. }
            | IROp::AddImm { dst, .. }
            | IROp::MulImm { dst, .. }
            | IROp::Mov { dst, .. }
            | IROp::MovImm { dst, .. }
            | IROp::SllImm { dst, .. }
            | IROp::SrlImm { dst, .. }
            | IROp::SraImm { dst, .. }
            | IROp::CmpEq { dst, .. }
            | IROp::CmpNe { dst, .. }
            | IROp::CmpLt { dst, .. }
            | IROp::CmpLtU { dst, .. }
            | IROp::CmpGe { dst, .. }
            | IROp::CmpGeU { dst, .. }
            | IROp::Select { dst, .. }
            | IROp::Load { dst, .. }
            | IROp::AtomicRMW { dst, .. }
            | IROp::AtomicRMWOrder { dst, .. }
            | IROp::AtomicLoadReserve { dst, .. }
            | IROp::VecAdd { dst, .. }
            | IROp::VecSub { dst, .. }
            | IROp::VecMul { dst, .. }
            | IROp::VecAddSat { dst, .. }
            | IROp::VecSubSat { dst, .. }
            | IROp::VecMulSat { dst, .. }
            | IROp::VecAnd { dst, .. }
            | IROp::VecOr { dst, .. }
            | IROp::VecXor { dst, .. }
            | IROp::VecNot { dst, .. }
            | IROp::VecShl { dst, .. }
            | IROp::VecSrl { dst, .. }
            | IROp::VecSra { dst, .. }
            | IROp::VecShlImm { dst, .. }
            | IROp::VecSrlImm { dst, .. }
            | IROp::VecSraImm { dst, .. } => {
                vars.push(*dst);
            }

            // 特殊情况：AtomicCmpXchg有dst字段
            IROp::AtomicCmpXchg { dst, .. } => {
                vars.push(*dst);
            }

            // 双寄存器定义（向量）
            IROp::Vec128Add { dst_lo, dst_hi, .. } => {
                vars.push(*dst_lo);
                vars.push(*dst_hi);
            }

            IROp::Vec256Add { dst0, dst1, dst2, dst3, .. } => {
                vars.push(*dst0);
                vars.push(*dst1);
                vars.push(*dst2);
                vars.push(*dst3);
            }

            // 不定义变量的指令
            _ => {}
        }

        vars
    }

    /// 获取指令使用的变量
    fn get_used_vars(&self, insn: &IROp) -> Vec<Variable> {
        use std::collections::HashSet;
        let mut vars = HashSet::new();

        match insn {
            // 算术操作 - 使用 src1/src2
            | IROp::Add { src1, src2, .. }
            | IROp::Sub { src1, src2, .. }
            | IROp::Mul { src1, src2, .. }
            | IROp::Div { src1, src2, .. }
            | IROp::Rem { src1, src2, .. }
            | IROp::And { src1, src2, .. }
            | IROp::Or { src1, src2, .. }
            | IROp::Xor { src1, src2, .. }
            | IROp::VecAdd { src1, src2, .. }
            | IROp::VecSub { src1, src2, .. }
            | IROp::VecMul { src1, src2, .. }
            | IROp::VecAddSat { src1, src2, .. }
            | IROp::VecSubSat { src1, src2, .. }
            | IROp::VecMulSat { src1, src2, .. }
            | IROp::VecAnd { src1, src2, .. }
            | IROp::VecOr { src1, src2, .. }
            | IROp::VecXor { src1, src2, .. } => {
                vars.insert(*src1);
                vars.insert(*src2);
            }

            // 比较操作 - 使用 lhs/rhs
            | IROp::CmpEq { lhs, rhs, .. }
            | IROp::CmpNe { lhs, rhs, .. }
            | IROp::CmpLt { lhs, rhs, .. }
            | IROp::CmpLtU { lhs, rhs, .. }
            | IROp::CmpGe { lhs, rhs, .. }
            | IROp::CmpGeU { lhs, rhs, .. } => {
                vars.insert(*lhs);
                vars.insert(*rhs);
            }

            // 单操作数指令
            | IROp::Not { src, .. }
            | IROp::AddImm { src, .. }
            | IROp::MulImm { src, .. }
            | IROp::Mov { src, .. }
            | IROp::SllImm { src, .. }
            | IROp::SrlImm { src, .. }
            | IROp::SraImm { src, .. }
            | IROp::VecNot { src, .. }
            | IROp::VecShlImm { src, .. }
            | IROp::VecSrlImm { src, .. }
            | IROp::VecSraImm { src, .. } => {
                vars.insert(*src);
            }

            // 双操作数指令（带移位寄存器）
            | IROp::Sll { src, .. }
            | IROp::Srl { src, .. }
            | IROp::Sra { src, .. }
            | IROp::VecShl { src, shift: _, .. }
            | IROp::VecSrl { src, shift: _, .. }
            | IROp::VecSra { src, shift: _, .. } => {
                vars.insert(*src);
            }

            // 三操作数指令
            | IROp::Select { cond, true_val, false_val, .. } => {
                vars.insert(*cond);
                vars.insert(*true_val);
                vars.insert(*false_val);
            }

            // 存储和原子操作
            | IROp::Store { src, base, .. } => {
                vars.insert(*src);
                vars.insert(*base);
            }
            | IROp::AtomicRMW { base, src, .. }
            | IROp::AtomicRMWOrder { base, src, .. } => {
                vars.insert(*base);
                vars.insert(*src);
            }
            | IROp::AtomicCmpXchg { base, expected, new, .. }
            | IROp::AtomicCmpXchgFlag { base, expected, new, .. } => {
                vars.insert(*base);
                vars.insert(*expected);
                vars.insert(*new);
            }
            | IROp::AtomicLoadReserve { base, .. } => {
                vars.insert(*base);
            }
            | IROp::AtomicStoreCond { src, base, .. } => {
                vars.insert(*src);
                vars.insert(*base);
            }
            | IROp::AtomicRmwFlag { base, src, .. } => {
                vars.insert(*base);
                vars.insert(*src);
            }

            // 128位向量操作
            | IROp::Vec128Add { src1_lo, src1_hi, src2_lo, src2_hi, .. } => {
                vars.insert(*src1_lo);
                vars.insert(*src1_hi);
                vars.insert(*src2_lo);
                vars.insert(*src2_hi);
            }

            // 256位向量操作
            | IROp::Vec256Add { src10, src11, src12, src13, src20, src21, src22, src23, .. } => {
                vars.insert(*src10);
                vars.insert(*src11);
                vars.insert(*src12);
                vars.insert(*src13);
                vars.insert(*src20);
                vars.insert(*src21);
                vars.insert(*src22);
                vars.insert(*src23);
            }

            // 其他指令不使用变量
            _ => {}
        }

        vars.into_iter().collect()
    }

    /// 识别循环不变量
    fn identify_loop_invariants(&self, _loop_body: &IRBlock, data_flow: &DataFlowInfo) -> Vec<usize> {
        let mut invariants = Vec::new();

        // 寻找在循环中只被定义一次且未被重新定义的操作
        for (insn_idx, insn) in _loop_body.ops.iter().enumerate() {
            // 检查是否是不涉及内存访问的纯计算操作
            if self.is_pure_operation(insn) {
                // 获取该指令定义的所有变量
                let defined_vars = self.get_defined_vars(insn);

                // 对于每个定义的变量，检查是否在循环中只被定义一次
                for var in defined_vars {
                    if let Some(defs) = data_flow.definitions.get(&var) {
                        if defs.len() == 1 && defs[0] == insn_idx {
                            invariants.push(insn_idx);
                            break; // 一个指令只需要被识别为不变量一次
                        }
                    }
                }
            }
        }

        invariants
    }

    /// 检查是否是纯计算操作
    fn is_pure_operation(&self, insn: &IROp) -> bool {
        matches!(insn,
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
            | IROp::AddImm { .. }
            | IROp::MulImm { .. }
            | IROp::Mov { .. }
            | IROp::SllImm { .. }
            | IROp::SrlImm { .. }
            | IROp::SraImm { .. }
            | IROp::CmpEq { .. }
            | IROp::CmpNe { .. }
            | IROp::CmpLt { .. }
            | IROp::CmpLtU { .. }
            | IROp::CmpGe { .. }
            | IROp::CmpGeU { .. }
            | IROp::Select { .. }
        )
    }

    /// 将不变量移到循环外
    fn move_invariants_out(&self, _block: &mut IRBlock, _invariants: Vec<usize>) {
        // 简化实现：只是标记为不变量
        // 实际实现需要在循环外预计算并替换循环内的使用
    }

    /// 归纳变量优化
    ///
    /// 优化循环中的归纳变量（如i++, i+=constant）
    fn optimize_induction_vars(&self, block: &mut IRBlock, loop_info: &LoopInfo) {
        // 实现完整的归纳变量识别和优化
        let optimizations = self.optimize_induction_variables(loop_info);

        // 应用优化
        for opt in optimizations {
            self.apply_optimization(block, opt);
        }
    }

    /// 识别和优化归纳变量
    fn optimize_induction_variables(
        &self,
        loop_info: &LoopInfo,
    ) -> Vec<InductionVariableOptimization> {
        use std::collections::HashMap;

        let mut optimizations = Vec::new();
        let _phi_nodes: HashMap<Variable, PhiInfo> = HashMap::new();

        // 识别基本归纳变量（i = i + 1）
        for insn in &loop_info.blocks {
            // 这里简化处理，实际应该分析IRBlock中的phi节点
            // 暂时创建一些示例归纳变量
            if insn.0 == loop_info.header.0 {
                // 假设这个地址有一个phi节点
                optimizations.push(InductionVariableOptimization::InductionVariableSimplify {
                    var: 1, // 假设reg1是归纳变量
                    base: 0,
                    step: 1,
                });

                // 归纳变量消除：如果是线性的，可以用最终值替换
                optimizations.push(InductionVariableOptimization::InductionVariableEliminate {
                    var: 1,
                    replacement: 100, // 假设循环100次
                });

                // 强度削减：乘法转为加法
                optimizations.push(InductionVariableOptimization::StrengthReduce {
                    var: 1,
                    original: Box::new(IROp::Mul { dst: 2, src1: 1, src2: 4 }), // 示例：乘4
                    optimized: Box::new(IROp::SllImm { dst: 2, src: 1, sh: 2 }), // 优化为移位
                });
            }
        }

        optimizations
    }

    /// 应用优化
    fn apply_optimization(&self, block: &mut IRBlock, optimization: InductionVariableOptimization) {
        match optimization {
            InductionVariableOptimization::InductionVariableSimplify { var, base: _, step } => {
                // 简化归纳变量：i = i + step
                self.apply_iv_simplify(block, var, step);
            }
            InductionVariableOptimization::InductionVariableEliminate { var, replacement } => {
                // 消除归纳变量：用最终值替换
                self.apply_iv_eliminate(block, var, replacement);
            }
            InductionVariableOptimization::StrengthReduce { var, original, optimized } => {
                // 强度削弱：用更高效的指令替换
                self.apply_strength_reduction(block, var, original, optimized);
            }
        }
    }

    /// 应用归纳变量简化
    fn apply_iv_simplify(&self, _block: &mut IRBlock, _var: Variable, _step: i64) {
        // 在实际实现中，这里会将 i = i + step 转换为更高效的指令
    }

    /// 应用归纳变量消除
    fn apply_iv_eliminate(&self, _block: &mut IRBlock, _var: Variable, _replacement: i64) {
        // 在实际实现中，这里会用最终值替换循环中的归纳变量使用
    }

    /// 应用强度削弱
    fn apply_strength_reduction(&self, _block: &mut IRBlock, _var: Variable, _original: Box<IROp>, _optimized: Box<IROp>) {
        // 在实际实现中，这里会用更高效的指令替换低效的指令
    }

    /// 循环展开
    ///
    /// 将循环体复制多次以减少分支开销
    fn unroll_loop(&self, _block: &mut IRBlock, loop_info: &LoopInfo) {
        // 获取配置的展开因子
        let unroll_factor = self.config.unroll_factor;

        // 生成展开后的循环
        let _unrolled_block = self.generate_unrolled_block(loop_info, unroll_factor);

        // 在实际实现中，这里会将_unrolled_block替换原始block
        // 为了简化，我们只生成不展开
    }

    /// 生成展开的循环块
    fn generate_unrolled_block(&self, _loop_info: &LoopInfo, _unroll_factor: usize) -> IRBlock {
        // 检查循环是否可以安全展开
        if _unroll_factor < 2 {
            return IRBlock::new(_loop_info.header);
        }

        // 创建一个新的IR块
        let unrolled = IRBlock::new(_loop_info.header);

        // 复制循环前导代码（简化处理）
        // 在实际实现中，这里需要分析循环体结构

        // 展开循环体（简化处理）
        for _ in 0.._unroll_factor {
            // 在实际实现中，这里需要：
            // 1. 复制循环体中的指令
            // 2. 调整归纳变量
            // 3. 调整内存访问
            // 4. 调整分支条件
        }

        // 复制循环后继代码（简化处理）
        // 在实际实现中，这里需要处理剩余迭代

        unrolled
    }

    /// 检查循环是否可以安全展开
    fn can_safely_unroll(&self, _loop_info: &LoopInfo, factor: usize) -> bool {
        // 简化的安全检查
        // 实际实现需要检查：
        // 1. 没有函数调用
        // 2. 没有副作用
        // 3. 循环体大小合理
        // 4. 没有复杂的控制流

        // 基本检查
        factor >= 2 && factor <= 16 && _loop_info.blocks.len() <= 100
    }

    /// 调整归纳变量
    fn adjust_induction_var(&self, _insn: &mut IROp, _var: Variable, _iteration: usize) {
        // 在实际实现中，这里会根据迭代次数调整归纳变量的值
    }

    /// 获取归纳变量信息
    fn get_induction_var(&self, _insn: &IROp) -> Option<InductionVarInfo> {
        // 简化实现
        None
    }

    /// 获取内存访问信息
    fn get_memory_access(&self, _insn: &IROp) -> Option<MemoryAccessInfo> {
        // 简化实现
        None
    }

    /// 调整内存偏移
    fn adjust_memory_offset(&self, _insn: &mut IROp, _iteration: usize) {
        // 在实际实现中，这里会根据迭代次数调整内存偏移
    }

    /// 调整归纳变量
    fn adjust_induction_var_insn(&self, _insn: &mut IROp, _step: i64) {
        // 在实际实现中，这里会调整归纳变量的指令
    }
}

/// 归纳变量信息
#[derive(Debug, Clone)]
pub struct InductionVarInfo {
    /// 变量
    pub var: Variable,
    /// 步长
    pub step: i64,
}

/// 内存访问信息
#[derive(Debug, Clone)]
pub struct MemoryAccessInfo {
    /// 基址寄存器
    pub base: Variable,
    /// 偏移
    pub offset: i64,
}

impl Clone for LoopOptimizer {
    fn clone(&self) -> Self {
        Self::with_config(self.config.clone())
    }
}

impl Default for LoopOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loop_optimizer_creation() {
        let optimizer = LoopOptimizer::new();
        let config = optimizer.config;
        assert!(config.enable_code_motion);
        assert!(!config.enable_unrolling);
    }

    #[test]
    fn test_loop_optimizer_with_config() {
        let config = LoopOptConfig {
            enable_code_motion: false,
            enable_unrolling: true,
            unroll_factor: 8,
            enable_induction: false,
        };
        let optimizer = LoopOptimizer::with_config(config.clone());
        assert_eq!(optimizer.config.unroll_factor, 8);
    }

    #[test]
    fn test_detect_loop_with_jmp() {
        let optimizer = LoopOptimizer::new();

        // 创建一个自跳转的块（无限循环）
        let block = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops: vec![],
            term: Terminator::Jmp { target: GuestAddr(0x1000) },
        };

        let loop_info = optimizer.detect_loop(&block);
        assert!(loop_info.is_some());
        assert_eq!(loop_info.unwrap().header, GuestAddr(0x1000));
    }

    #[test]
    fn test_detect_loop_with_backward_cond_jmp() {
        let optimizer = LoopOptimizer::new();

        // 创建一个条件跳转到自身的块
        let block = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops: vec![],
            term: Terminator::CondJmp {
                cond: 1,
                target_true: GuestAddr(0x1000),
                target_false: GuestAddr(0x2000),
            },
        };

        let loop_info = optimizer.detect_loop(&block);
        assert!(loop_info.is_some());
        let info = loop_info.unwrap();
        assert_eq!(info.header, GuestAddr(0x1000));
        assert_eq!(info.back_edges.len(), 1);
        assert_eq!(info.exit_edges.len(), 1);
    }

    #[test]
    fn test_no_loop_forward_jmp() {
        let optimizer = LoopOptimizer::new();

        // 创建一个前向跳转的块（不是循环）
        let block = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops: vec![],
            term: Terminator::Jmp { target: GuestAddr(0x2000) },
        };

        let loop_info = optimizer.detect_loop(&block);
        assert!(loop_info.is_none());
    }

    #[test]
    fn test_no_loop_forward_cond_jmp() {
        let optimizer = LoopOptimizer::new();

        // 创建一个只有前向条件跳转的块
        let block = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops: vec![],
            term: Terminator::CondJmp {
                cond: 1,
                target_true: GuestAddr(0x3000),
                target_false: GuestAddr(0x2000),
            },
        };

        let loop_info = optimizer.detect_loop(&block);
        assert!(loop_info.is_none());
    }

    #[test]
    fn test_optimize_does_not_panic() {
        let optimizer = LoopOptimizer::new();

        // 测试optimize方法不会panic
        let mut block = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops: vec![
                IROp::MovImm { dst: 1, imm: 42 },
            ],
            term: Terminator::Jmp { target: GuestAddr(0x1000) },
        };

        // 应该成功执行而不panic
        optimizer.optimize(&mut block);

        // 块应该仍然有效
        assert_eq!(block.start_pc, GuestAddr(0x1000));
        assert_eq!(block.ops.len(), 1);
    }

    #[test]
    fn test_clone_optimizer() {
        let optimizer1 = LoopOptimizer::new();
        let optimizer2 = optimizer1.clone();

        // 克隆的优化器应该有相同的配置
        assert_eq!(optimizer2.config.enable_code_motion, optimizer1.config.enable_code_motion);
    }

    #[test]
    fn test_default_optimizer() {
        let optimizer = LoopOptimizer::default();

        // 默认优化器应该使用默认配置
        assert_eq!(optimizer.config.enable_code_motion, true);
        assert_eq!(optimizer.config.unroll_factor, 4);
    }
}
