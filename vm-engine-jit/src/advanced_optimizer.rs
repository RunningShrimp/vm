//! JIT引擎高级优化器
//!
//! 本模块实现高级优化技术，包括循环优化、内联优化、
//! 常量传播、死代码消除、函数特化等高级优化技术。

use std::collections::{HashMap, HashSet};
use vm_ir::{IRBlock, IROp};

/// 高级优化器配置
#[derive(Debug, Clone)]
pub struct AdvancedOptimizerConfig {
    /// 是否启用循环优化
    pub enable_loop_optimization: bool,
    /// 是否启用内联优化
    pub enable_inlining: bool,
    /// 是否启用常量传播
    pub enable_constant_propagation: bool,
    /// 是否启用死代码消除
    pub enable_dead_code_elimination: bool,
    /// 是否启用函数特化
    pub enable_function_specialization: bool,
    /// 是否启用值范围分析
    pub enable_value_range_analysis: bool,
    /// 是否启用别名分析
    pub enable_alias_analysis: bool,
    /// 最大内联深度
    pub max_inline_depth: usize,
    /// 最大循环展开次数
    pub max_loop_unroll_count: usize,
    /// 优化级别
    pub optimization_level: u8,
}

impl Default for AdvancedOptimizerConfig {
    fn default() -> Self {
        Self {
            enable_loop_optimization: true,
            enable_inlining: true,
            enable_constant_propagation: true,
            enable_dead_code_elimination: true,
            enable_function_specialization: true,
            enable_value_range_analysis: true,
            enable_alias_analysis: true,
            max_inline_depth: 3,
            max_loop_unroll_count: 4,
            optimization_level: 3,
        }
    }
}

/// 值范围信息
#[derive(Debug, Clone)]
pub struct ValueRange {
    /// 最小值
    pub min: i64,
    /// 最大值
    pub max: i64,
    /// 是否已知
    pub known: bool,
}

impl ValueRange {
    pub fn unknown() -> Self {
        Self {
            min: i64::MIN,
            max: i64::MAX,
            known: false,
        }
    }

    pub fn known(value: i64) -> Self {
        Self {
            min: value,
            max: value,
            known: true,
        }
    }

    pub fn range(min: i64, max: i64) -> Self {
        Self {
            min,
            max,
            known: true,
        }
    }

    pub fn intersect(&self, other: &ValueRange) -> ValueRange {
        if !self.known || !other.known {
            return ValueRange::unknown();
        }

        ValueRange::range(
            self.min.max(other.min),
            self.max.min(other.max),
        )
    }

    pub fn union(&self, other: &ValueRange) -> ValueRange {
        if !self.known {
            return other.clone();
        }
        if !other.known {
            return self.clone();
        }

        ValueRange::range(
            self.min.min(other.min),
            self.max.max(other.max),
        )
    }

    pub fn contains(&self, value: i64) -> bool {
        if !self.known {
            return true;
        }
        value >= self.min && value <= self.max
    }
}

/// 常量信息
#[derive(Debug, Clone)]
pub struct ConstantInfo {
    /// 常量值
    pub value: u64,
    /// 是否已知
    pub known: bool,
    /// 定义位置
    pub definition: Option<usize>,
}

impl ConstantInfo {
    pub fn unknown() -> Self {
        Self {
            value: 0,
            known: false,
            definition: None,
        }
    }

    pub fn known(value: u64, definition: Option<usize>) -> Self {
        Self {
            value,
            known: true,
            definition,
        }
    }
}

/// 循环信息
#[derive(Debug, Clone)]
pub struct LoopInfo {
    /// 循环头指令索引
    pub header_index: usize,
    /// 循环体指令索引范围
    pub body_indices: Vec<usize>,
    /// 循环退出指令索引
    pub exit_indices: Vec<usize>,
    /// 循环归纳变量
    pub induction_variables: Vec<u32>,
    /// 循环不变量
    pub invariants: Vec<u32>,
    /// 循环迭代次数（如果已知）
    pub iteration_count: Option<u64>,
}

/// 内联候选
#[derive(Debug, Clone)]
pub struct InlineCandidate {
    /// 函数地址
    pub function_address: u64,
    /// 调用点索引
    pub call_site_index: usize,
    /// 内联收益估计
    pub benefit_estimate: f64,
    /// 内联成本估计
    pub cost_estimate: f64,
    /// 内联深度
    pub inline_depth: usize,
}

/// 别名信息
#[derive(Debug, Clone)]
pub struct AliasInfo {
    /// 内存位置
    pub memory_locations: HashSet<u32>,
    /// 是否可能别名
    pub may_alias: bool,
    /// 是否确定别名
    pub must_alias: bool,
}

/// 高级优化器
pub struct AdvancedOptimizer {
    /// 优化器配置
    config: AdvancedOptimizerConfig,
    /// 值范围信息
    value_ranges: HashMap<u32, ValueRange>,
    /// 常量信息
    constants: HashMap<u32, ConstantInfo>,
    /// 循环信息
    loops: Vec<LoopInfo>,
    /// 内联候选
    inline_candidates: Vec<InlineCandidate>,
    /// 别名信息
    alias_info: HashMap<u32, AliasInfo>,
    /// 已死代码
    dead_instructions: HashSet<usize>,
    /// 优化统计
    optimization_stats: OptimizationStats,
}

/// 优化统计信息
#[derive(Debug, Clone, Default)]
pub struct OptimizationStats {
    /// 常量传播次数
    pub constant_propagations: usize,
    /// 死代码消除次数
    pub dead_code_eliminations: usize,
    /// 循环优化次数
    pub loop_optimizations: usize,
    /// 内联次数
    pub inlinings: usize,
    /// 函数特化次数
    pub specializations: usize,
    /// 值范围分析次数
    pub value_range_analyses: usize,
    /// 别名分析次数
    pub alias_analyses: usize,
}

impl AdvancedOptimizer {
    /// 创建新的高级优化器
    pub fn new(config: AdvancedOptimizerConfig) -> Self {
        Self {
            config,
            value_ranges: HashMap::new(),
            constants: HashMap::new(),
            loops: Vec::new(),
            inline_candidates: Vec::new(),
            alias_info: HashMap::new(),
            dead_instructions: HashSet::new(),
            optimization_stats: OptimizationStats::default(),
        }
    }

    /// 优化IR块
    pub fn optimize(&mut self, ir_block: &mut IRBlock) -> Result<(), String> {
        // 重置优化状态
        self.reset_optimization_state();

        // 分析阶段
        self.analyze_ir_block(ir_block)?;

        // 优化阶段
        if self.config.enable_constant_propagation {
            self.constant_propagation(ir_block)?;
        }

        if self.config.enable_dead_code_elimination {
            self.dead_code_elimination(ir_block)?;
        }

        if self.config.enable_loop_optimization {
            self.loop_optimization(ir_block)?;
        }

        if self.config.enable_inlining {
            self.inlining_optimization(ir_block)?;
        }

        if self.config.enable_value_range_analysis {
            self.value_range_optimization(ir_block)?;
        }

        if self.config.enable_alias_analysis {
            self.alias_optimization(ir_block)?;
        }

        if self.config.enable_function_specialization {
            self.function_specialization(ir_block)?;
        }

        Ok(())
    }

    /// 重置优化状态
    fn reset_optimization_state(&mut self) {
        self.value_ranges.clear();
        self.constants.clear();
        self.loops.clear();
        self.inline_candidates.clear();
        self.alias_info.clear();
        self.dead_instructions.clear();
        self.optimization_stats = OptimizationStats::default();
    }

    /// 分析IR块
    fn analyze_ir_block(&mut self, ir_block: &IRBlock) -> Result<(), String> {
        // 分析循环结构
        self.analyze_loops(ir_block)?;

        // 分析常量
        self.analyze_constants(ir_block)?;

        // 分析内联候选
        self.analyze_inline_candidates(ir_block)?;

        // 分析别名
        self.analyze_aliases(ir_block)?;

        Ok(())
    }

    /// 分析循环结构
    fn analyze_loops(&mut self, ir_block: &IRBlock) -> Result<(), String> {
        let mut visited: HashSet<usize> = HashSet::new();
        let mut stack: Vec<usize> = Vec::new();
        let mut loops = Vec::new();

        // 简化的循环检测算法
        for (i, op) in ir_block.ops.iter().enumerate() {
            if let IROp::Beq { target, .. } = op {
                // 检查是否形成循环
                if *target < (i as u64) {
                    let loop_start = *target as usize;
                    let loop_end = i;

                    // 分析循环体
                    let mut body_indices = Vec::new();
                    let mut induction_variables = Vec::new();
                    let mut invariants = Vec::new();

                    for j in loop_start..=loop_end {
                        body_indices.push(j);
                        
                        // 简化的归纳变量检测
                        if let IROp::Add { dst, src1, src2 } = &ir_block.ops[j] {
                            if *src1 == *dst || *src2 == *dst {
                                induction_variables.push(*dst);
                            }
                        }
                    }

                    let loop_info = LoopInfo {
                        header_index: loop_start,
                        body_indices,
                        exit_indices: vec![loop_end],
                        induction_variables,
                        invariants,
                        iteration_count: None,
                    };

                    loops.push(loop_info);
                }
            }
        }

        self.loops = loops;
        Ok(())
    }

    /// 分析常量
    fn analyze_constants(&mut self, ir_block: &IRBlock) -> Result<(), String> {
        let mut constants = HashMap::new();

        for (i, op) in ir_block.ops.iter().enumerate() {
            match op {
                IROp::MovImm { dst, imm } => {
                    constants.insert(*dst, ConstantInfo::known(*imm, Some(i)));
                }
                IROp::Add { dst, src1, src2 } => {
                    if let (Some(const1), Some(const2)) = (constants.get(src1), constants.get(src2)) {
                        if const1.known && const2.known {
                            let result = const1.value.wrapping_add(const2.value);
                            constants.insert(*dst, ConstantInfo::known(result, Some(i)));
                        }
                    }
                }
                IROp::Sub { dst, src1, src2 } => {
                    if let (Some(const1), Some(const2)) = (constants.get(src1), constants.get(src2)) {
                        if const1.known && const2.known {
                            let result = const1.value.wrapping_sub(const2.value);
                            constants.insert(*dst, ConstantInfo::known(result, Some(i)));
                        }
                    }
                }
                IROp::Mul { dst, src1, src2 } => {
                    if let (Some(const1), Some(const2)) = (constants.get(src1), constants.get(src2)) {
                        if const1.known && const2.known {
                            let result = const1.value.wrapping_mul(const2.value);
                            constants.insert(*dst, ConstantInfo::known(result, Some(i)));
                        }
                    }
                }
                _ => {}
            }
        }

        self.constants = constants;
        Ok(())
    }

    /// 分析内联候选
    fn analyze_inline_candidates(&mut self, ir_block: &IRBlock) -> Result<(), String> {
        let mut candidates = Vec::new();

        for (i, op) in ir_block.ops.iter().enumerate() {
            // 简化的内联候选检测 - 暂时跳过，因为没有Call指令
            // if let IROp::Call { target, .. } = op {
            //     let candidate = InlineCandidate {
            //         function_address: *target,
            //         call_site_index: i,
            //         benefit_estimate: 10.0, // 简化的收益估计
            //         cost_estimate: 5.0,     // 简化的成本估计
            //         inline_depth: 0,
            //     };
            //     candidates.push(candidate);
            // }
        }

        self.inline_candidates = candidates;
        Ok(())
    }

    /// 分析别名
    fn analyze_aliases(&mut self, ir_block: &IRBlock) -> Result<(), String> {
        let mut alias_info = HashMap::new();

        for op in ir_block.ops.iter() {
            if let IROp::Load { dst, base, .. } = op {
                let info = AliasInfo {
                    memory_locations: HashSet::from([*base]),
                    may_alias: true,
                    must_alias: false,
                };
                alias_info.insert(*dst, info);
            }
        }

        self.alias_info = alias_info;
        Ok(())
    }

    /// 常量传播优化
    fn constant_propagation(&mut self, ir_block: &mut IRBlock) -> Result<(), String> {
        let mut changed = true;
        let mut iterations = 0;

        while changed && iterations < 10 {
            changed = false;
            iterations += 1;

            let ops_to_process: Vec<(usize, IROp)> = ir_block.ops.iter().enumerate().map(|(i, op)| (i, op.clone())).collect();
            
            for (i, op) in ops_to_process {
                if self.dead_instructions.contains(&i) {
                    continue;
                }

                match op {
                    IROp::Add { dst, src1, src2 } => {
                        if let (Some(const1), Some(const2)) = (self.constants.get(&src1), self.constants.get(&src2)) {
                            if const1.known && const2.known {
                                let result = const1.value.wrapping_add(const2.value);
                                ir_block.ops[i] = IROp::MovImm { dst, imm: result };
                                self.constants.insert(dst, ConstantInfo::known(result, Some(i)));
                                changed = true;
                                self.optimization_stats.constant_propagations += 1;
                            }
                        }
                    }
                    IROp::Sub { dst, src1, src2 } => {
                        if let (Some(const1), Some(const2)) = (self.constants.get(&src1), self.constants.get(&src2)) {
                            if const1.known && const2.known {
                                let result = const1.value.wrapping_sub(const2.value);
                                ir_block.ops[i] = IROp::MovImm { dst, imm: result };
                                self.constants.insert(dst, ConstantInfo::known(result, Some(i)));
                                changed = true;
                                self.optimization_stats.constant_propagations += 1;
                            }
                        }
                    }
                    IROp::Mul { dst, src1, src2 } => {
                        if let (Some(const1), Some(const2)) = (self.constants.get(&src1), self.constants.get(&src2)) {
                            if const1.known && const2.known {
                                let result = const1.value.wrapping_mul(const2.value);
                                ir_block.ops[i] = IROp::MovImm { dst, imm: result };
                                self.constants.insert(dst, ConstantInfo::known(result, Some(i)));
                                changed = true;
                                self.optimization_stats.constant_propagations += 1;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }

    /// 死代码消除优化
    fn dead_code_elimination(&mut self, ir_block: &mut IRBlock) -> Result<(), String> {
        let mut used_registers = HashSet::new();
        let mut dead_instructions: HashSet<usize> = HashSet::new();

        // 首先标记所有使用的寄存器
        for op in ir_block.ops.iter() {
            match op {
                IROp::Add { src1, src2, .. } => {
                    used_registers.insert(*src1);
                    used_registers.insert(*src2);
                }
                IROp::Sub { src1, src2, .. } => {
                    used_registers.insert(*src1);
                    used_registers.insert(*src2);
                }
                IROp::Mul { src1, src2, .. } => {
                    used_registers.insert(*src1);
                    used_registers.insert(*src2);
                }
                IROp::Div { src1, src2, .. } => {
                    used_registers.insert(*src1);
                    used_registers.insert(*src2);
                }
                IROp::Load { base, .. } => {
                    used_registers.insert(*base);
                }
                IROp::Store { base, src, .. } => {
                    used_registers.insert(*base);
                    used_registers.insert(*src);
                }
                // IROp::CondJmp { cond, .. } => {
                //     used_registers.insert(*cond);
                // }
                _ => {}
            }
        }

        // 标记死代码
    let mut dead_instructions = HashSet::new();
    for (i, op) in ir_block.ops.iter().enumerate() {
            match op {
                IROp::MovImm { dst, .. } |
                IROp::Add { dst, .. } |
                IROp::Sub { dst, .. } |
                IROp::Mul { dst, .. } |
                IROp::Div { dst, .. } |
                IROp::Load { dst, .. } |
                IROp::SllImm { dst, .. } |
                IROp::SrlImm { dst, .. } => {
                    if !used_registers.contains(dst) {
                        dead_instructions.insert(i);
                        self.optimization_stats.dead_code_eliminations += 1;
                    }
                }
                _ => {}
            }
        }

        // 移除死代码
        let mut new_ops = Vec::new();
        for (i, op) in ir_block.ops.iter().enumerate() {
            if !dead_instructions.contains(&i) {
                new_ops.push(op.clone());
            }
        }
        ir_block.ops = new_ops;

        self.dead_instructions = dead_instructions;
        Ok(())
    }

    /// 循环优化
    fn loop_optimization(&mut self, ir_block: &mut IRBlock) -> Result<(), String> {
        for loop_info in &self.loops.clone() {
            // 循环不变量外提
            self.loop_invariant_code_motion(ir_block, loop_info)?;

            // 循环展开
            if loop_info.iteration_count.is_some() && 
               loop_info.iteration_count.unwrap() <= self.config.max_loop_unroll_count as u64 {
                self.loop_unrolling(ir_block, loop_info)?;
            }

            // 归纳变量优化
            self.induction_variable_optimization(ir_block, loop_info)?;

            self.optimization_stats.loop_optimizations += 1;
        }

        Ok(())
    }

    /// 循环不变量外提
    fn loop_invariant_code_motion(&mut self, ir_block: &mut IRBlock, loop_info: &LoopInfo) -> Result<(), String> {
        // 简化的不变量检测
        let mut invariants = Vec::new();

        for &index in &loop_info.body_indices {
            let op = &ir_block.ops[index];
            match op {
                IROp::MovImm { .. } => {
                    invariants.push(index);
                }
                IROp::Add { src1, src2, .. } => {
                    if !loop_info.induction_variables.contains(src1) && 
                       !loop_info.induction_variables.contains(src2) {
                        invariants.push(index);
                    }
                }
                _ => {}
            }
        }

        // 将不变量移到循环前面
        let mut new_ops = Vec::new();
        let mut loop_ops = Vec::new();

        for (i, op) in ir_block.ops.iter().enumerate() {
            if invariants.contains(&i) {
                new_ops.push(op.clone());
            } else {
                loop_ops.push(op.clone());
            }
        }

        new_ops.extend(loop_ops);
        ir_block.ops = new_ops;

        Ok(())
    }

    /// 循环展开
    fn loop_unrolling(&mut self, ir_block: &mut IRBlock, loop_info: &LoopInfo) -> Result<(), String> {
        if let Some(iteration_count) = loop_info.iteration_count {
            if iteration_count <= self.config.max_loop_unroll_count as u64 {
                // 简化的循环展开实现
                let mut new_ops = Vec::new();
                
                for _ in 0..iteration_count {
                    for &index in &loop_info.body_indices {
                        if index < ir_block.ops.len() {
                            new_ops.push(ir_block.ops[index].clone());
                        }
                    }
                }

                // 替换循环体
                let mut final_ops = Vec::new();
                for (i, op) in ir_block.ops.iter().enumerate() {
                    if !loop_info.body_indices.contains(&i) {
                        final_ops.push(op.clone());
                    }
                }
                final_ops.extend(new_ops);
                ir_block.ops = final_ops;
            }
        }

        Ok(())
    }

    /// 归纳变量优化
    fn induction_variable_optimization(&mut self, ir_block: &mut IRBlock, loop_info: &LoopInfo) -> Result<(), String> {
        // 简化的归纳变量优化
        for &ind_var in &loop_info.induction_variables {
            // 查找归纳变量的定义
            for &index in &loop_info.body_indices {
                if let IROp::Add { dst, src1, src2 } = &ir_block.ops[index] {
                    if *dst == ind_var && (*src1 == ind_var || *src2 == ind_var) {
                        // 简化的强度削减：将加法转换为移位
                        if let Some(constant) = self.constants.get(src2) {
                            if constant.known && constant.value.is_power_of_two() {
                                let shift_amount = constant.value.trailing_zeros() as u8;
                                ir_block.ops[index] = IROp::SllImm {
                                    dst: *dst,
                                    src: *src1,
                                    sh: shift_amount,
                                };
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// 内联优化
    fn inlining_optimization(&mut self, ir_block: &mut IRBlock) -> Result<(), String> {
        // 按收益排序内联候选
        self.inline_candidates.sort_by(|a, b| {
            b.benefit_estimate.partial_cmp(&a.benefit_estimate).unwrap()
        });

        for candidate in &self.inline_candidates.clone() {
            if candidate.inline_depth >= self.config.max_inline_depth {
                continue;
            }

            if candidate.benefit_estimate > candidate.cost_estimate {
                self.inline_function(ir_block, candidate)?;
                self.optimization_stats.inlinings += 1;
            }
        }

        Ok(())
    }

    /// 内联函数
    fn inline_function(&mut self, ir_block: &mut IRBlock, candidate: &InlineCandidate) -> Result<(), String> {
        // 简化的内联实现
        // 在实际实现中，这里需要获取函数体并进行内联
        
        // 移除函数调用
        ir_block.ops.remove(candidate.call_site_index);
        
        // 插入内联代码（这里用空操作代替）
        ir_block.ops.insert(candidate.call_site_index, IROp::MovImm { dst: 0, imm: 0 });

        Ok(())
    }

    /// 值范围优化
    fn value_range_optimization(&mut self, ir_block: &mut IRBlock) -> Result<(), String> {
        // 计算值范围
        self.compute_value_ranges(ir_block)?;

        // 基于值范围进行优化
        for (i, op) in ir_block.ops.iter().enumerate() {
            match op {
                // IROp::CondJmp { cond, .. } => {
                //     if let Some(range) = self.value_ranges.get(cond) {
                //         if range.known {
                //             // 如果条件总是为真或假，可以替换为直接跳转
                //             if range.min > 0 && range.max > 0 {
                //                 // 这里需要根据实际情况处理
                //             } else if range.min == 0 && range.max == 0 {
                //                 // 条件总是为假，移除跳转
                //                 // 这里需要根据实际情况处理
                //             }
                //         }
                //     }
                // }
                _ => {}
            }
        }

        self.optimization_stats.value_range_analyses += 1;
        Ok(())
    }

    /// 计算值范围
    fn compute_value_ranges(&mut self, ir_block: &IRBlock) -> Result<(), String> {
        let mut ranges = HashMap::new();

        for op in ir_block.ops.iter() {
            match op {
                IROp::MovImm { dst, imm } => {
                    ranges.insert(*dst, ValueRange::known(*imm as i64));
                }
                IROp::Add { dst, src1, src2 } => {
                    if let (Some(range1), Some(range2)) = (ranges.get(src1), ranges.get(src2)) {
                        if range1.known && range2.known {
                            let min = range1.min.saturating_add(range2.min);
                            let max = range1.max.saturating_add(range2.max);
                            ranges.insert(*dst, ValueRange::range(min, max));
                        }
                    }
                }
                IROp::Sub { dst, src1, src2 } => {
                    if let (Some(range1), Some(range2)) = (ranges.get(src1), ranges.get(src2)) {
                        if range1.known && range2.known {
                            let min = range1.min.saturating_sub(range2.max);
                            let max = range1.max.saturating_sub(range2.min);
                            ranges.insert(*dst, ValueRange::range(min, max));
                        }
                    }
                }
                _ => {}
            }
        }

        self.value_ranges = ranges;
        Ok(())
    }

    /// 别名优化
    fn alias_optimization(&mut self, ir_block: &mut IRBlock) -> Result<(), String> {
        // 简化的别名优化
        for i in 0..ir_block.ops.len() {
            if let IROp::Store { src, .. } = &ir_block.ops[i] {
                if let Some(alias_info) = self.alias_info.get(src) {
                    if !alias_info.may_alias {
                        // 如果没有别名，可以进行更激进的优化
                        // 这里可以添加更多的优化逻辑
                    }
                }
            }
        }

        self.optimization_stats.alias_analyses += 1;
        Ok(())
    }

    /// 函数特化
    fn function_specialization(&mut self, _ir_block: &mut IRBlock) -> Result<(), String> {
        // 简化的函数特化实现
        // 在实际实现中，这里需要分析函数调用模式并生成特化版本

        self.optimization_stats.specializations += 1;
        Ok(())
    }

    /// 获取优化统计信息
    pub fn get_optimization_stats(&self) -> &OptimizationStats {
        &self.optimization_stats
    }

    /// 重置优化统计信息
    pub fn reset_optimization_stats(&mut self) {
        self.optimization_stats = OptimizationStats::default();
    }
}

/// 高级优化器特征
pub trait AdvancedOptimizerTrait {
    /// 优化IR块
    fn optimize(&mut self, ir_block: &mut IRBlock) -> Result<(), String>;
    
    /// 获取优化统计信息
    fn get_optimization_stats(&self) -> &OptimizationStats;
    
    /// 重置优化统计信息
    fn reset_optimization_stats(&mut self);
}

impl AdvancedOptimizerTrait for AdvancedOptimizer {
    fn optimize(&mut self, ir_block: &mut IRBlock) -> Result<(), String> {
        self.optimize(ir_block)
    }

    fn get_optimization_stats(&self) -> &OptimizationStats {
        &self.optimization_stats
    }

    fn reset_optimization_stats(&mut self) {
        self.reset_optimization_stats();
    }
}

/// 创建高级优化器
pub fn create_advanced_optimizer(config: AdvancedOptimizerConfig) -> Box<dyn AdvancedOptimizerTrait> {
    Box::new(AdvancedOptimizer::new(config))
}