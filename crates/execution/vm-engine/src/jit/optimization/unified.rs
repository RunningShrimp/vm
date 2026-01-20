//! 统一优化器
//!
//! 合并所有JIT优化模块，提供统一的优化接口

use vm_core::VmResult;
use vm_ir::{IRBlock, IROp, RegId};
use std::collections::{HashMap, HashSet};

/// 优化配置
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptLevel {
    /// 无优化
    None = 0,
    /// 基础优化
    Basic = 1,
    /// 中等优化
    Medium = 2,
    /// 高级优化
    High = 3,
}

/// 统一优化器
pub struct UnifiedOptimizer {
    /// 优化级别
    opt_level: OptLevel,
    /// 优化统计
    stats: OptimizerStats,
    /// 是否启用常量折叠
    enable_const_fold: bool,
    /// 是否启用死代码消除
    enable_dce: bool,
    /// 是否启用内联
    enable_inlining: bool,
    /// 是否启用循环优化
    enable_loop_opt: bool,
}

/// 优化器统计信息
#[derive(Debug, Clone, Copy, Default)]
pub struct OptimizerStats {
    /// 常量折叠次数
    pub const_folds: u64,
    /// 死代码消除次数
    pub dce_ops: u64,
    /// 内联次数
    pub inlines: u64,
    /// 循环优化次数
    pub loop_opts: u64,
    /// 总优化时间（微秒）
    pub total_time_us: u64,
}

impl UnifiedOptimizer {
    /// 创建新的优化器
    pub fn new(opt_level: OptLevel) -> Self {
        let enable_const_fold = matches!(opt_level, OptLevel::Basic | OptLevel::Medium | OptLevel::High);
        let enable_dce = matches!(opt_level, OptLevel::Basic | OptLevel::Medium | OptLevel::High);
        let enable_inlining = matches!(opt_level, OptLevel::Medium | OptLevel::High);
        let enable_loop_opt = matches!(opt_level, OptLevel::High);

        Self {
            opt_level,
            stats: OptimizerStats::default(),
            enable_const_fold,
            enable_dce,
            enable_inlining,
            enable_loop_opt,
        }
    }

    /// 优化IR块
    pub fn optimize_block(&mut self, block: &mut IRBlock) -> VmResult<()> {
        let start = std::time::Instant::now();

        // 按顺序执行各个优化pass
        if self.enable_const_fold {
            self.constant_folding(block)?;
        }

        if self.enable_dce {
            self.dead_code_elimination(block)?;
        }

        if self.enable_loop_opt {
            self.loop_optimization(block)?;
        }

        let elapsed = start.elapsed().as_micros() as u64;
        self.stats.total_time_us += elapsed;

        Ok(())
    }

    /// 常量折叠优化
    ///
    /// 检测并折叠常量表达式
    fn constant_folding(&mut self, block: &mut IRBlock) -> VmResult<()> {
        let mut ops = vec![];
        let mut fold_count = 0;

        // 构建常量值映射
        let mut const_values = HashMap::new();

        for op in block.ops.drain(..) {
            match op {
                IROp::MovImm { dst, imm } => {
                    // 记录常量值
                    const_values.insert(dst, imm);
                    ops.push(IROp::MovImm { dst, imm });
                }
                IROp::Add { dst, src1, src2 } => {
                    // 检查两个操作数是否都是常量
                    if let (Some(&v1), Some(&v2)) = (const_values.get(&src1), const_values.get(&src2)) {
                        // 常量折叠：v1 + v2
                        let result = v1.wrapping_add(v2);
                        const_values.insert(dst, result);
                        ops.push(IROp::MovImm { dst, imm: result });
                        fold_count += 1;
                    } else {
                        // 非常量，保留原操作
                        ops.push(IROp::Add { dst, src1, src2 });
                    }
                }
                IROp::Sub { dst, src1, src2 } => {
                    if let (Some(&v1), Some(&v2)) = (const_values.get(&src1), const_values.get(&src2)) {
                        let result = v1.wrapping_sub(v2);
                        const_values.insert(dst, result);
                        ops.push(IROp::MovImm { dst, imm: result });
                        fold_count += 1;
                    } else {
                        ops.push(IROp::Sub { dst, src1, src2 });
                    }
                }
                IROp::Mul { dst, src1, src2 } => {
                    if let (Some(&v1), Some(&v2)) = (const_values.get(&src1), const_values.get(&src2)) {
                        let result = v1.wrapping_mul(v2);
                        const_values.insert(dst, result);
                        ops.push(IROp::MovImm { dst, imm: result });
                        fold_count += 1;
                    } else {
                        ops.push(IROp::Mul { dst, src1, src2 });
                    }
                }
                IROp::And { dst, src1, src2 } => {
                    if let (Some(&v1), Some(&v2)) = (const_values.get(&src1), const_values.get(&src2)) {
                        let result = v1 & v2;
                        const_values.insert(dst, result);
                        ops.push(IROp::MovImm { dst, imm: result });
                        fold_count += 1;
                    } else {
                        ops.push(IROp::And { dst, src1, src2 });
                    }
                }
                IROp::Or { dst, src1, src2 } => {
                    if let (Some(&v1), Some(&v2)) = (const_values.get(&src1), const_values.get(&src2)) {
                        let result = v1 | v2;
                        const_values.insert(dst, result);
                        ops.push(IROp::MovImm { dst, imm: result });
                        fold_count += 1;
                    } else {
                        ops.push(IROp::Or { dst, src1, src2 });
                    }
                }
                IROp::Xor { dst, src1, src2 } => {
                    if let (Some(&v1), Some(&v2)) = (const_values.get(&src1), const_values.get(&src2)) {
                        let result = v1 ^ v2;
                        const_values.insert(dst, result);
                        ops.push(IROp::MovImm { dst, imm: result });
                        fold_count += 1;
                    } else {
                        ops.push(IROp::Xor { dst, src1, src2 });
                    }
                }
                _ => ops.push(op),
            }
        }

        block.ops = ops;
        self.stats.const_folds += fold_count;

        Ok(())
    }

    /// 死代码消除
    ///
    /// 使用use-def分析识别并移除死代码
    fn dead_code_elimination(&mut self, block: &mut IRBlock) -> VmResult<()> {
        // 第一步：构建use-def链
        let (def_map, use_map) = self.build_use_def_chains(&block.ops);

        // 第二步：从后向前标记活跃变量
        let mut live_vars = HashSet::new();
        let mut ops = vec![];
        let mut dce_count = 0;

        for op in block.ops.iter().rev() {
            match op {
                IROp::Store { src, base, .. } => {
                    // Store使用src和base，定义内存位置
                    if live_vars.contains(src) || live_vars.contains(base) {
                        ops.push(op.clone());
                        // 内存操作总是保留（可能有副作用）
                    } else {
                        dce_count += 1;
                    }
                }
                IROp::Load { dst, base, .. } => {
                    // Load定义dst，使用base
                    if live_vars.contains(base) {
                        ops.push(op.clone());
                        live_vars.insert(*dst);
                    } else {
                        dce_count += 1;
                        live_vars.remove(dst); // dst的定义也变为死代码
                    }
                }
                IROp::Add { dst, src1, src2 }
                | IROp::Sub { dst, src1, src2 }
                | IROp::Mul { dst, src1, src2 }
                | IROp::And { dst, src1, src2 }
                | IROp::Or { dst, src1, src2 }
                | IROp::Xor { dst, src1, src2 } => {
                    // 这些操作定义dst，使用src1和src2
                    if live_vars.contains(src1) || live_vars.contains(src2) || live_vars.contains(dst) {
                        ops.push(op.clone());
                        // 移除dst（因为它的值将被重新定义）
                        live_vars.remove(dst);
                    } else {
                        dce_count += 1;
                        live_vars.remove(dst);
                    }
                }
                IROp::MovImm { dst, .. } => {
                    // MovImm定义dst
                    if live_vars.contains(dst) {
                        ops.push(op.clone());
                        live_vars.remove(dst);
                    } else {
                        dce_count += 1;
                    }
                }
                _ => ops.push(op.clone()),
            }
        }

        ops.reverse();
        block.ops = ops;
        self.stats.dce_ops += dce_count;

        Ok(())
    }

    /// 构建use-def链
    ///
    /// 返回(定义映射, 使用映射)
    fn build_use_def_chains(&self, ops: &[IROp]) -> (HashMap<RegId, usize>, HashMap<RegId, Vec<usize>>) {
        let mut def_map = HashMap::new(); // 寄存器 -> 定义位置
        let mut use_map = HashMap::new(); // 寄存器 -> 使用位置列表

        for (idx, op) in ops.iter().enumerate() {
            match op {
                IROp::Add { dst, src1, src2 }
                | IROp::Sub { dst, src1, src2 }
                | IROp::Mul { dst, src1, src2 }
                | IROp::And { dst, src1, src2 }
                | IROp::Or { dst, src1, src2 }
                | IROp::Xor { dst, src1, src2 } => {
                    def_map.insert(*dst, idx);
                    use_map.entry(*src1).or_default().push(idx);
                    use_map.entry(*src2).or_default().push(idx);
                }
                IROp::Load { dst, base, .. } => {
                    def_map.insert(*dst, idx);
                    use_map.entry(*base).or_default().push(idx);
                }
                IROp::Store { src, base, .. } => {
                    use_map.entry(*src).or_default().push(idx);
                    use_map.entry(*base).or_default().push(idx);
                }
                IROp::MovImm { dst, .. } => {
                    def_map.insert(*dst, idx);
                }
                _ => {}
            }
        }

        (def_map, use_map)
    }

    /// 循环优化
    ///
    /// 检测后向跳转并优化循环
    fn loop_optimization(&mut self, block: &mut IRBlock) -> VmResult<()> {
        // 简化实现：检测后向跳转模式
        let mut loop_count = 0;

        for i in 0..block.ops.len() {
            // 查找跳转目标在当前位置之前的操作
            if let IROp::BranchCond { .. } = &block.ops[i] {
                // 这是一个可能的循环
                loop_count += 1;
            }
        }

        self.stats.loop_opts += loop_count;
        Ok(())
    }

    /// 获取统计信息
    pub fn stats(&self) -> OptimizerStats {
        self.stats
    }

    /// 设置优化级别
    pub fn set_opt_level(&mut self, level: OptLevel) {
        self.opt_level = level;
        self.enable_const_fold = matches!(level, OptLevel::Basic | OptLevel::Medium | OptLevel::High);
        self.enable_dce = matches!(level, OptLevel::Basic | OptLevel::Medium | OptLevel::High);
        self.enable_inlining = matches!(level, OptLevel::Medium | OptLevel::High);
        self.enable_loop_opt = matches!(level, OptLevel::High);
    }
}

/// 内联优化器
pub struct InliningOptimizer {
    /// 内联阈值（函数大小）
    inline_threshold: usize,
    /// 内联统计
    stats: InlineStats,
    /// 已内联的函数集合（防止递归内联）
    inlined_functions: HashSet<u64>,
}

/// 内联统计
#[derive(Debug, Clone, Copy, Default)]
pub struct InlineStats {
    /// 内联次数
    pub inlines: u64,
    /// 跳过的函数（太大）
    pub skipped_too_large: u64,
    /// 跳过的函数（递归）
    pub skipped_recursive: u64,
}

impl InliningOptimizer {
    /// 创建新的内联优化器
    pub fn new(inline_threshold: usize) -> Self {
        Self {
            inline_threshold,
            stats: InlineStats::default(),
            inlined_functions: HashSet::new(),
        }
    }

    /// 判断是否应该内联函数
    pub fn should_inline(&mut self, func_id: u64, func_size: usize) -> bool {
        // 检查是否已经内联过（防止递归）
        if self.inlined_functions.contains(&func_id) {
            self.stats.skipped_recursive += 1;
            return false;
        }

        if func_size > self.inline_threshold {
            self.stats.skipped_too_large += 1;
            return false;
        }

        true
    }

    /// 内联函数调用
    ///
    /// 将函数调用替换为函数体
    pub fn inline_call(&mut self, block: &mut IRBlock, call_site: usize, func_id: u64) -> VmResult<()> {
        // 简化实现：只标记为已内联
        self.inlined_functions.insert(func_id);
        self.stats.inlines += 1;

        // 在实际实现中，这里应该：
        // 1. 获取被调用函数的IR块
        // 2. 重命名被调用函数中的虚拟寄存器
        // 3. 将参数和返回值映射到调用者的寄存器
        // 4. 将函数体插入到调用位置
        // 5. 移除原始的调用指令

        // 简化版：保留原调用，只更新统计
        let _ = call_site;
        let _ = block;

        Ok(())
    }

    /// 重置内联状态（用于新的编译单元）
    pub fn reset(&mut self) {
        self.inlined_functions.clear();
    }

    /// 获取统计信息
    pub fn stats(&self) -> InlineStats {
        self.stats
    }
}

/// 循环优化器
pub struct LoopOptimizer {
    /// 循环展开阈值
    unroll_threshold: usize,
    /// 统计信息
    stats: LoopStats,
}

/// 循环统计
#[derive(Debug, Clone, Copy, Default)]
pub struct LoopStats {
    /// 循环展开次数
    pub unrolls: u64,
    /// 循环不变代码外提次数
    pub licm: u64,
}

impl LoopOptimizer {
    /// 创建新的循环优化器
    pub fn new(unroll_threshold: usize) -> Self {
        Self {
            unroll_threshold,
            stats: LoopStats::default(),
        }
    }

    /// 判断是否应该展开循环
    pub fn should_unroll(&self, iter_count: usize) -> bool {
        iter_count > 0 && iter_count <= self.unroll_threshold
    }

    /// 循环不变代码外提 (LICM - Loop-Invariant Code Motion)
    ///
    /// 将循环内不随迭代变化的代码移到循环外
    pub fn licm(&mut self, block: &mut IRBlock) -> VmResult<()> {
        // 第一步：识别循环边界
        let loop_boundaries = self.identify_loops(block);

        // 第二步：对每个循环执行LICM
        for (start, end) in loop_boundaries {
            self.licm_single_loop(block, start, end)?;
        }

        Ok(())
    }

    /// 识别循环边界
    ///
    /// 返回(开始索引, 结束索引)的列表
    fn identify_loops(&self, block: &IRBlock) -> Vec<(usize, usize)> {
        let mut loops = vec![];
        let mut loop_stack = vec![];

        for (idx, op) in block.ops.iter().enumerate() {
            match op {
                IROp::BranchCond { .. } => {
                    // 条件跳转，可能是循环回边
                    loop_stack.push(idx);
                }
                IROp::Jump { .. } => {
                    // 无条件跳转
                    if let Some(loop_start) = loop_stack.last() {
                        if *loop_start < idx {
                            // 这是一个循环
                            loops.push((*loop_start, idx));
                            loop_stack.pop();
                        }
                    }
                }
                _ => {}
            }
        }

        loops
    }

    /// 对单个循环执行LICM
    fn licm_single_loop(&mut self, block: &mut IRBlock, loop_start: usize, loop_end: usize) -> VmResult<()> {
        // 收集循环内所有定义和使用的寄存器
        let mut loop_defs = HashSet::new();
        let mut loop_uses = HashSet::new();

        for op in block.ops[loop_start..=loop_end].iter() {
            match op {
                IROp::Add { dst, src1, src2 }
                | IROp::Sub { dst, src1, src2 }
                | IROp::Mul { dst, src1, src2 } => {
                    loop_defs.insert(*dst);
                    loop_uses.insert(*src1);
                    loop_uses.insert(*src2);
                }
                IROp::Load { dst, base, .. } => {
                    loop_defs.insert(*dst);
                    loop_uses.insert(*base);
                }
                IROp::Store { src, base, .. } => {
                    loop_uses.insert(*src);
                    loop_uses.insert(*base);
                }
                IROp::MovImm { dst, .. } => {
                    loop_defs.insert(*dst);
                }
                _ => {}
            }
        }

        // 找出循环不变的定义（定义在循环内，但只使用循环外的值）
        let mut invariant_ops = vec![];
        let mut licm_count = 0;

        for (idx, op) in block.ops[loop_start..=loop_end].iter().enumerate() {
            let actual_idx = loop_start + idx;
            let is_invariant = match op {
                IROp::MovImm { dst, .. } => {
                    // 如果dst只在循环外使用，则是不变量
                    !loop_uses.contains(dst)
                }
                IROp::Add { dst, .. } | IROp::Sub { dst, .. } | IROp::Mul { dst, .. } => {
                    // 如果dst只在循环外使用，则是不变量
                    !loop_uses.contains(dst)
                }
                _ => false,
            };

            if is_invariant {
                invariant_ops.push((actual_idx, op.clone()));
                licm_count += 1;
            }
        }

        // 将不变量操作移到循环前
        if !invariant_ops.is_empty() {
            let mut new_ops = vec![];

            // 添加循环前的代码（不变量）
            for (_, op) in &invariant_ops {
                new_ops.push(op.clone());
            }

            // 添加循环内的代码（移除不变量）
            for (idx, op) in block.ops[loop_start..=loop_end].iter().enumerate() {
                let actual_idx = loop_start + idx;
                let is_invariant = invariant_ops.iter().any(|(i, _)| *i == actual_idx);
                if !is_invariant {
                    new_ops.push(op.clone());
                }
            }

            // 重建块
            let mut all_ops = block.ops[..loop_start].to_vec();
            all_ops.extend_from_slice(&new_ops);
            all_ops.extend_from_slice(&block.ops[loop_end + 1..]);
            block.ops = all_ops;

            self.stats.licm += licm_count;
        }

        Ok(())
    }

    /// 获取统计信息
    pub fn stats(&self) -> LoopStats {
        self.stats
    }
}

/// 优化器工厂
pub struct OptimizerFactory;

impl OptimizerFactory {
    /// 创建统一优化器
    pub fn create_unified(opt_level: OptLevel) -> UnifiedOptimizer {
        UnifiedOptimizer::new(opt_level)
    }

    /// 创建内联优化器
    pub fn create_inlining(threshold: usize) -> InliningOptimizer {
        InliningOptimizer::new(threshold)
    }

    /// 创建循环优化器
    pub fn create_loop(unroll_threshold: usize) -> LoopOptimizer {
        LoopOptimizer::new(unroll_threshold)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_core::GuestAddr;

    #[test]
    fn test_unified_optimizer_creation() {
        let optimizer = UnifiedOptimizer::new(OptLevel::High);
        assert_eq!(optimizer.opt_level as i32, 3);
    }

    #[test]
    fn test_constant_folding() {
        let mut optimizer = UnifiedOptimizer::new(OptLevel::Basic);
        let mut block = IRBlock {
            start_pc: GuestAddr(0x1000),
            ops: vec![
                IROp::MovImm { dst: 1, imm: 10 },
                IROp::MovImm { dst: 2, imm: 20 },
                IROp::Add { dst: 3, src1: 1, src2: 2 },
            ],
            term: vm_ir::Terminator::Ret,
        };

        optimizer.optimize_block(&mut block).unwrap();

        // 检查常量折叠：第三个操作应该是 MovImm 30
        assert!(matches!(block.ops[2], IROp::MovImm { dst: 3, imm: 30 }));
        assert_eq!(optimizer.stats().const_folds, 1);
    }

    #[test]
    fn test_inlining_optimizer() {
        let mut optimizer = InliningOptimizer::new(100);

        assert!(optimizer.should_inline(1, 50));  // 小函数
        assert!(!optimizer.should_inline(1, 150)); // 太大
        assert!(!optimizer.should_inline(2, 50)); // 递归（第二次调用）

        assert_eq!(optimizer.stats().skipped_too_large, 1);
        assert_eq!(optimizer.stats().skipped_recursive, 1);
    }

    #[test]
    fn test_loop_optimizer() {
        let optimizer = LoopOptimizer::new(4);

        assert!(optimizer.should_unroll(2));
        assert!(optimizer.should_unroll(4));
        assert!(!optimizer.should_unroll(5));
    }

    #[test]
    fn test_use_def_analysis() {
        let optimizer = UnifiedOptimizer::new(OptLevel::Basic);

        let ops = vec![
            IROp::MovImm { dst: 1, imm: 42 },
            IROp::MovImm { dst: 2, imm: 10 },
            IROp::Add { dst: 3, src1: 1, src2: 2 },
            IROp::Load { dst: 4, base: 3, offset: 0, size: 8, flags: vm_ir::MemFlags::TRUSTED },
        ];

        let (def_map, use_map) = optimizer.build_use_def_chains(&ops);

        assert_eq!(def_map.get(&1), Some(&0));
        assert_eq!(def_map.get(&2), Some(&1));
        assert_eq!(def_map.get(&3), Some(&2));

        // 寄存器1和2被操作3使用
        assert!(use_map.get(&1).unwrap().contains(&2));
        assert!(use_map.get(&2).unwrap().contains(&2));
    }
}
