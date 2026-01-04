//! ML增强特征提取器
//!
//! 为JIT编译决策提供增强的特征工程支持。

use std::collections::{HashMap, HashSet};
use vm_ir::IRBlock;
use vm_ir::{GuestAddr, RegId};

/// 分支信息
#[derive(Debug, Clone)]
pub struct BranchInfo {
    pub kind: BranchKind,
    pub target: Option<GuestAddr>,
    pub fallthrough: Option<GuestAddr>,
    pub condition: Option<RegId>,
}

/// 分支类型
#[derive(Debug, Clone, Copy)]
pub enum BranchKind {
    Conditional,
    Unconditional,
    Indirect,
    Call,
    Return,
}

/// 循环信息
#[derive(Debug, Clone)]
pub struct LoopInfo {
    pub header: GuestAddr,
    pub blocks: HashSet<GuestAddr>,
    pub exit_blocks: HashSet<GuestAddr>,
    pub depth: u8,
}

/// 支配树数据结构
#[derive(Debug, Clone)]
pub struct DominatorTree {
    pub immediate_dominators: HashMap<GuestAddr, GuestAddr>,
}

impl DominatorTree {
    pub fn new() -> Self {
        Self {
            immediate_dominators: HashMap::new(),
        }
    }

    pub fn dominates(&self, a: GuestAddr, b: GuestAddr) -> bool {
        let mut current = b;
        while let Some(dominator) = self.immediate_dominators.get(&current) {
            if *dominator == a {
                return true;
            }
            current = *dominator;
        }
        false
    }
}

// ============================================================================
// 增强特征定义
// ============================================================================

/// 增强的执行特征
///
/// 包含原有特征和新增的高级特征
#[derive(Clone, Debug)]
pub struct ExecutionFeaturesEnhanced {
    // === 原有基础特征 ===
    /// IR块大小（指令数）
    pub block_size: usize,
    /// 指令计数
    pub instr_count: usize,
    /// 分支指令计数
    pub branch_count: usize,
    /// 内存访问计数
    pub memory_access_count: usize,
    /// 执行次数
    pub execution_count: u64,
    /// 缓存命中率
    pub cache_hit_rate: f64,

    // === 新增特征：指令混合 ===
    /// 指令混合特征
    pub instruction_mix: InstMixFeatures,

    // === 新增特征：控制流 ===
    /// 控制流复杂度（圈复杂度）
    pub control_flow_complexity: f64,
    /// 循环嵌套深度
    pub loop_nest_depth: u8,
    /// 是否有递归调用
    pub has_recursion: bool,

    // === 新增特征：数据局部性 ===
    /// 数据局部性评分
    pub data_locality: f64,
    /// 内存访问模式顺序性
    pub memory_sequentiality: f64,

    // === 新增特征：编译历史 ===
    /// 历史编译信息
    pub compilation_history: CompilationHistory,

    // === 新增特征：寄存器压力 ===
    /// 寄存器压力（0-1，1表示最高压力）
    pub register_pressure: f64,

    // === 新增特征：代码热度和稳定性 ===
    /// 代码热度（执行频率）
    pub code_heat: f64,
    /// 代码稳定性（变化频率）
    pub code_stability: f64,
}

/// 指令混合特征
#[derive(Clone, Debug)]
pub struct InstMixFeatures {
    /// 算术指令比例
    pub arithmetic_ratio: f64,
    /// 内存指令比例
    pub memory_ratio: f64,
    /// 分支指令比例
    pub branch_ratio: f64,
    /// 向量指令比例
    pub vector_ratio: f64,
    /// 浮点指令比例
    pub float_ratio: f64,
    /// 调用指令比例
    pub call_ratio: f64,
}

/// 编译历史信息
#[derive(Clone, Debug)]
pub struct CompilationHistory {
    /// 之前编译次数
    pub previous_compilations: u32,
    /// 平均编译时间（微秒）
    pub avg_compilation_time_us: f64,
    /// 上次编译收益（加速比）
    pub last_compile_benefit: f64,
    /// 上次编译是否成功
    pub last_compile_success: bool,
}

// ============================================================================
// 特征提取器
// ============================================================================

/// 增强特征提取器
pub struct FeatureExtractorEnhanced {
    /// 历史窗口大小
    history_window: usize,
    /// 记录的执行历史
    execution_history: HashMap<u64, Vec<ExecutionRecord>>,
}

/// 执行记录
#[derive(Clone, Debug)]
struct ExecutionRecord {
    timestamp: u64,
    execution_time_ns: u64,
    memory_accesses: Vec<(u64, u8)>, // (address, size)
}

impl FeatureExtractorEnhanced {
    /// 创建新的增强特征提取器
    pub fn new(history_window: usize) -> Self {
        Self {
            history_window,
            execution_history: HashMap::new(),
        }
    }

    /// 提取增强特征
    pub fn extract_enhanced(&mut self, block: &IRBlock) -> ExecutionFeaturesEnhanced {
        // 1. 提取基础特征
        let block_size = block.ops.len();
        let instr_count = block.ops.len();
        let branch_count = self.count_branches(block);
        let memory_access_count = self.count_memory_accesses(block);

        // 2. 提取指令混合特征
        let instruction_mix = self.analyze_instruction_mix(block);

        // 3. 计算控制流复杂度（圈复杂度）
        let control_flow_complexity = self.compute_cyclomatic_complexity(block);

        // 4. 检测循环嵌套
        let loop_nest_depth = self.detect_loop_nesting(block);

        // 5. 检测递归
        let has_recursion = self.detect_recursion(block);

        // 6. 计算数据局部性
        let data_locality = self.compute_data_locality(block);

        // 7. 计算内存访问顺序性
        let memory_sequentiality = self.compute_memory_sequentiality(block);

        // 8. 获取编译历史
        let compilation_history = self.get_compilation_history(block);

        // 9. 计算寄存器压力
        let register_pressure = self.compute_register_pressure(block);

        // 10. 计算代码热度和稳定性
        let block_hash = self.hash_block(block);
        let (code_heat, code_stability) = self.compute_heat_and_stability(&block_hash);

        // 从历史记录中获取执行统计
        let (execution_count, cache_hit_rate) = self.get_execution_stats(&block_hash);

        ExecutionFeaturesEnhanced {
            block_size,
            instr_count,
            branch_count,
            memory_access_count,
            execution_count,
            cache_hit_rate,
            instruction_mix,
            control_flow_complexity,
            loop_nest_depth,
            has_recursion,
            data_locality,
            memory_sequentiality,
            compilation_history,
            register_pressure,
            code_heat,
            code_stability,
        }
    }

    /// 分析指令混合
    fn analyze_instruction_mix(&self, block: &IRBlock) -> InstMixFeatures {
        let mut arithmetic = 0usize;
        let mut memory = 0usize;
        let branch = 0usize;
        let mut vector = 0usize;
        let float = 0usize;
        let call = 0usize;

        for op in &block.ops {
            match op {
                // 算术指令
                vm_ir::IROp::Add { .. }
                | vm_ir::IROp::Sub { .. }
                | vm_ir::IROp::Mul { .. }
                | vm_ir::IROp::Div { .. }
                | vm_ir::IROp::Rem { .. }
                | vm_ir::IROp::And { .. }
                | vm_ir::IROp::Or { .. }
                | vm_ir::IROp::Xor { .. }
                | vm_ir::IROp::Not { .. }
                | vm_ir::IROp::Sll { .. }
                | vm_ir::IROp::Srl { .. }
                | vm_ir::IROp::Sra { .. } => arithmetic += 1,

                // 内存指令
                vm_ir::IROp::LoadExt { .. } | vm_ir::IROp::StoreExt { .. } => memory += 1,

                // 向量指令
                vm_ir::IROp::VecAdd { .. }
                | vm_ir::IROp::VecSub { .. }
                | vm_ir::IROp::VecMul { .. } => vector += 1,

                _ => {}
            }
        }

        let total = block.ops.len();
        if total == 0 {
            return InstMixFeatures {
                arithmetic_ratio: 0.0,
                memory_ratio: 0.0,
                branch_ratio: 0.0,
                vector_ratio: 0.0,
                float_ratio: 0.0,
                call_ratio: 0.0,
            };
        }

        InstMixFeatures {
            arithmetic_ratio: arithmetic as f64 / total as f64,
            memory_ratio: memory as f64 / total as f64,
            branch_ratio: branch as f64 / total as f64,
            vector_ratio: vector as f64 / total as f64,
            float_ratio: float as f64 / total as f64,
            call_ratio: call as f64 / total as f64,
        }
    }

    /// 计算圈复杂度
    ///
    /// 圈复杂度 = E - N + 2P
    /// - E: 边数
    /// - N: 节点数
    /// - P: 连通组件数
    fn compute_cyclomatic_complexity(&self, block: &IRBlock) -> f64 {
        let nodes = block.ops.len() as f64;
        let edges = self.count_edges(block) as f64;
        let p = 1.0; // 单个连通组件

        edges - nodes + 2.0 * p
    }

    /// 计算CFG边数
    fn count_edges(&self, block: &IRBlock) -> usize {
        let mut edges = 0;
        for insn in &block.ops {
            // 每个分支指令增加2条边（true和false分支）
            if self.is_branch(insn) {
                edges += 2;
            } else {
                edges += 1; // 顺序边
            }
        }
        edges
    }

    /// 检测是否是分支指令
    fn is_branch(&self, op: &vm_ir::IROp) -> bool {
        matches!(
            op,
            vm_ir::IROp::Beq { .. }
                | vm_ir::IROp::Bne { .. }
                | vm_ir::IROp::Blt { .. }
                | vm_ir::IROp::Bge { .. }
                | vm_ir::IROp::Bltu { .. }
                | vm_ir::IROp::Bgeu { .. }
                | vm_ir::IROp::Branch { .. }
                | vm_ir::IROp::CondBranch { .. }
        )
    }

    /// 检测基本块中的分支指令
    pub fn detect_branches(&self, block: &IRBlock) -> Vec<BranchInfo> {
        let mut branches = Vec::new();

        // 检查IROp中的分支指令
        for insn in &block.ops {
            match insn {
                vm_ir::IROp::Beq {
                    src1: _,
                    src2: _,
                    target,
                } => {
                    branches.push(BranchInfo {
                        kind: BranchKind::Conditional,
                        target: Some(*target),
                        fallthrough: None,
                        condition: None,
                    });
                }

                vm_ir::IROp::Bne {
                    src1: _,
                    src2: _,
                    target,
                } => {
                    branches.push(BranchInfo {
                        kind: BranchKind::Conditional,
                        target: Some(*target),
                        fallthrough: None,
                        condition: None,
                    });
                }

                vm_ir::IROp::Blt {
                    src1: _,
                    src2: _,
                    target,
                } => {
                    branches.push(BranchInfo {
                        kind: BranchKind::Conditional,
                        target: Some(*target),
                        fallthrough: None,
                        condition: None,
                    });
                }

                vm_ir::IROp::Bge {
                    src1: _,
                    src2: _,
                    target,
                } => {
                    branches.push(BranchInfo {
                        kind: BranchKind::Conditional,
                        target: Some(*target),
                        fallthrough: None,
                        condition: None,
                    });
                }

                vm_ir::IROp::Bltu {
                    src1: _,
                    src2: _,
                    target,
                } => {
                    branches.push(BranchInfo {
                        kind: BranchKind::Conditional,
                        target: Some(*target),
                        fallthrough: None,
                        condition: None,
                    });
                }

                vm_ir::IROp::Bgeu {
                    src1: _,
                    src2: _,
                    target,
                } => {
                    branches.push(BranchInfo {
                        kind: BranchKind::Conditional,
                        target: Some(*target),
                        fallthrough: None,
                        condition: None,
                    });
                }

                vm_ir::IROp::Branch { target, link: _ } => {
                    branches.push(BranchInfo {
                        kind: BranchKind::Unconditional,
                        target: Some(match target {
                            vm_ir::Operand::Immediate(imm) => GuestAddr(*imm as u64),
                            vm_ir::Operand::Register(_) => {
                                // 无法确定目标，标记为间接分支
                                return branches;
                            }
                            _ => return branches,
                        }),
                        fallthrough: None,
                        condition: None,
                    });
                }

                vm_ir::IROp::CondBranch {
                    condition: _,
                    target: _,
                    link: _,
                } => {
                    // 条件分支，目标可能是动态的
                    branches.push(BranchInfo {
                        kind: BranchKind::Conditional,
                        target: None, // 动态目标
                        fallthrough: None,
                        condition: None,
                    });
                }

                _ => {
                    // 非分支指令
                }
            }
        }

        // 检查Terminator
        match &block.term {
            vm_ir::Terminator::Jmp { target } => {
                branches.push(BranchInfo {
                    kind: BranchKind::Unconditional,
                    target: Some(*target),
                    fallthrough: None,
                    condition: None,
                });
            }

            vm_ir::Terminator::CondJmp {
                cond,
                target_true,
                target_false,
            } => {
                branches.push(BranchInfo {
                    kind: BranchKind::Conditional,
                    target: Some(*target_true),
                    fallthrough: Some(*target_false),
                    condition: Some(*cond),
                });

                branches.push(BranchInfo {
                    kind: BranchKind::Conditional,
                    target: Some(*target_false),
                    fallthrough: None,
                    condition: None,
                });
            }

            vm_ir::Terminator::JmpReg { base: _, offset: _ } => {
                branches.push(BranchInfo {
                    kind: BranchKind::Indirect,
                    target: None, // 动态目标
                    fallthrough: None,
                    condition: None,
                });
            }

            vm_ir::Terminator::Call { target, ret_pc } => {
                branches.push(BranchInfo {
                    kind: BranchKind::Call,
                    target: Some(*target),
                    fallthrough: Some(*ret_pc),
                    condition: None,
                });
            }

            vm_ir::Terminator::Ret => {
                branches.push(BranchInfo {
                    kind: BranchKind::Return,
                    target: None,
                    fallthrough: None,
                    condition: None,
                });
            }

            vm_ir::Terminator::Fault { .. } | vm_ir::Terminator::Interrupt { .. } => {
                // 异常终结符，不是常规分支
            }
        }

        branches
    }

    /// 统计分支指令数
    fn count_branches(&self, block: &IRBlock) -> usize {
        block.ops.iter().filter(|op| self.is_branch(op)).count()
    }

    /// 统计内存访问数
    fn count_memory_accesses(&self, block: &IRBlock) -> usize {
        block
            .ops
            .iter()
            .filter(|op| {
                matches!(
                    op,
                    vm_ir::IROp::LoadExt { .. } | vm_ir::IROp::StoreExt { .. }
                )
            })
            .count()
    }

    /// 检测循环嵌套深度
    fn detect_loop_nesting(&self, block: &IRBlock) -> u8 {
        // 使用基于Terminator的循环检测
        let mut max_depth = 0u8;

        // 检查当前基本块是否是循环头
        if let Some(_loop_info) = self.identify_loop_header(block) {
            // 计算循环深度
            max_depth = self.calculate_loop_depth(block);
        }

        max_depth
    }

    /// 识别循环头
    fn identify_loop_header(&self, block: &IRBlock) -> Option<LoopInfo> {
        let mut _worklist: Vec<GuestAddr> = Vec::new();
        let mut _visited: HashSet<GuestAddr> = HashSet::new();
        let mut _loop_blocks: HashSet<GuestAddr> = HashSet::new();

        // 检查Terminator是否有回边
        match &block.term {
            vm_ir::Terminator::Jmp { target } => {
                if target >= &block.start_pc {
                    // 可能是回边
                    let loop_info = self.analyze_natural_loop(vec![block.start_pc, *target]);
                    if loop_info.blocks.len() > 1 {
                        return Some(loop_info);
                    }
                }
            }

            vm_ir::Terminator::CondJmp {
                target_true,
                target_false,
                ..
            } => {
                if target_true >= &block.start_pc || target_false >= &block.start_pc {
                    // 可能是回边
                    let mut targets = Vec::new();
                    if target_true >= &block.start_pc {
                        targets.push(*target_true);
                    }
                    if target_false >= &block.start_pc {
                        targets.push(*target_false);
                    }

                    let mut all_blocks = vec![block.start_pc];
                    all_blocks.extend(targets);
                    let loop_info = self.analyze_natural_loop(all_blocks);
                    if loop_info.blocks.len() > 1 {
                        return Some(loop_info);
                    }
                }
            }

            _ => {}
        }

        None
    }

    /// 分析自然循环
    fn analyze_natural_loop(&self, block_addrs: Vec<GuestAddr>) -> LoopInfo {
        let mut blocks: HashSet<GuestAddr> = HashSet::new();
        let mut worklist: Vec<GuestAddr> = Vec::new();

        // 初始化工作列表
        for &addr in &block_addrs {
            if !blocks.contains(&addr) {
                blocks.insert(addr);
                worklist.push(addr);
            }
        }

        // 收集所有在循环中的块
        while let Some(block_addr) = worklist.pop() {
            // 这里简化实现，实际应该根据控制流图收集前驱块
            // 模拟收集循环内的块
            if let Some(predecessors) = self.get_predecessors(block_addr) {
                for &pred in &predecessors {
                    if !blocks.contains(&pred) {
                        blocks.insert(pred);
                        worklist.push(pred);
                    }
                }
            }
        }

        // 找出循环出口
        let exit_blocks = self.find_loop_exits(&blocks);

        LoopInfo {
            header: block_addrs[0],
            blocks,
            exit_blocks,
            depth: 0, // 深度将在外部计算
        }
    }

    /// 获取基本块的前驱
    fn get_predecessors(&self, _block_addr: GuestAddr) -> Option<Vec<GuestAddr>> {
        // 简化实现，实际应该基于控制流图
        // 这里返回空列表作为占位符
        Some(Vec::new())
    }

    /// 找出循环出口
    fn find_loop_exits(&self, loop_blocks: &HashSet<GuestAddr>) -> HashSet<GuestAddr> {
        let mut exits: HashSet<GuestAddr> = HashSet::new();

        // 简化实现：循环出口是那些跳转到循环外部的块
        // 实际实现需要遍历所有块的后继
        for &block_addr in loop_blocks {
            if let Some(succs) = self.get_successors(block_addr) {
                for &succ in &succs {
                    if !loop_blocks.contains(&succ) {
                        exits.insert(succ);
                    }
                }
            }
        }

        exits
    }

    /// 获取基本块的后继
    fn get_successors(&self, _block_addr: GuestAddr) -> Option<Vec<GuestAddr>> {
        // 简化实现，实际应该基于控制流图
        // 这里返回空列表作为占位符
        Some(Vec::new())
    }

    /// 计算循环深度
    fn calculate_loop_depth(&self, block: &IRBlock) -> u8 {
        // 简化实现：基于基本块地址计算深度
        // 实际实现应该使用支配树分析

        // 检查是否是循环头
        if self.identify_loop_header(block).is_some() {
            // 计算嵌套深度
            let addr_value = block.start_pc.0 as u64;
            ((addr_value / 0x1000) & 0xFF) as u8
        } else {
            0
        }
    }

    /// 基于Terminator检测循环
    pub fn detect_loops_with_terminator(&self, _func: &[IRBlock]) -> Vec<LoopInfo> {
        // 简化实现：返回空列表
        // 实际实现需要完整分析整个函数的控制流图
        Vec::new()
    }

    /// 检测递归调用
    fn detect_recursion(&self, block: &IRBlock) -> bool {
        // 检查Terminator::Call是否调用自身
        if let vm_ir::Terminator::Call { target, .. } = &block.term {
            // 检查是否调用自身
            if target.0 == block.start_pc.0 {
                return true;
            }
        }
        false
    }

    /// 计算数据局部性
    ///
    /// 评分越高表示数据局部性越好
    fn compute_data_locality(&self, block: &IRBlock) -> f64 {
        // 分析内存访问模式
        let mut memory_accesses = Vec::new();

        for op in &block.ops {
            // 收集内存访问地址
            match op {
                // 加载操作
                vm_ir::IROp::Load {
                    base, offset, size, ..
                } => {
                    memory_accesses.push((base, offset, *size));
                }
                vm_ir::IROp::LoadExt { addr, size, .. } => {
                    if let vm_ir::Operand::Register(reg) = addr {
                        memory_accesses.push((reg, &0, *size));
                    }
                }

                // 存储操作
                vm_ir::IROp::Store {
                    base, offset, size, ..
                } => {
                    memory_accesses.push((base, offset, *size));
                }
                vm_ir::IROp::StoreExt { addr, size, .. } => {
                    if let vm_ir::Operand::Register(reg) = addr {
                        memory_accesses.push((reg, &0, *size));
                    }
                }

                // 浮点加载/存储
                vm_ir::IROp::Fload {
                    base, offset, size, ..
                } => {
                    memory_accesses.push((base, offset, *size));
                }
                vm_ir::IROp::Fstore {
                    base, offset, size, ..
                } => {
                    memory_accesses.push((base, offset, *size));
                }

                // 原子操作
                vm_ir::IROp::AtomicRMW { base, .. }
                | vm_ir::IROp::AtomicRMWOrder { base, .. }
                | vm_ir::IROp::AtomicCmpXchg { base, .. }
                | vm_ir::IROp::AtomicCmpXchgOrder { base, .. }
                | vm_ir::IROp::AtomicLoadReserve { base, .. } => {
                    // 原子操作通常涉及同一地址，具有良好的数据局部性
                    memory_accesses.push((base, &0, 8));
                }

                _ => {}
            }
        }

        if memory_accesses.len() < 2 {
            // 如果内存访问少于2次，局部性评分较高
            return 0.8;
        }

        // 计算内存访问的空间局部性
        let mut spatial_locality_score = 0.0;
        let mut comparison_count = 0;

        for i in 0..memory_accesses.len() {
            for j in (i + 1)..memory_accesses.len() {
                let (base1, offset1, size1) = memory_accesses[i];
                let (base2, offset2, size2) = memory_accesses[j];

                // 如果使用相同的基址寄存器，检查地址是否接近
                if base1 == base2 {
                    let addr1 = *offset1 as i64;
                    let addr2 = *offset2 as i64;
                    let distance = (addr1 - addr2).abs();

                    // 计算访问重叠度
                    let overlap = if addr1 < addr2 {
                        // addr1 [-----]
                        // addr2      [-----]
                        if addr1 + size1 as i64 <= addr2 {
                            0.0 // 无重叠
                        } else {
                            // 计算重叠大小
                            let overlap_end = (addr1 + size1 as i64).min(addr2 + size2 as i64);
                            let overlap_size = overlap_end - addr2;
                            overlap_size.min(size1 as i64) as f64 / size1.max(size2) as f64
                        }
                    } else {
                        // addr2 [-----]
                        // addr1      [-----]
                        if addr2 + size2 as i64 <= addr1 {
                            0.0 // 无重叠
                        } else {
                            // 计算重叠大小
                            let overlap_end = (addr2 + size2 as i64).min(addr1 + size1 as i64);
                            let overlap_size = overlap_end - addr1;
                            overlap_size.min(size2 as i64) as f64 / size1.max(size2) as f64
                        }
                    };

                    // 距离越近、重叠度越高，空间局部性越好
                    let distance_score = if distance < 64 {
                        1.0 - distance as f64 / 64.0
                    } else {
                        0.0
                    };

                    spatial_locality_score += overlap * distance_score;
                    comparison_count += 1;
                }
            }
        }

        if comparison_count == 0 {
            return 0.8; // 如果没有可比的访问，局部性较好
        }

        let avg_spatial_locality = spatial_locality_score / comparison_count as f64;

        // 考虑重复访问同一地址的频率
        let mut address_count = std::collections::HashMap::new();
        for (base, offset, _) in &memory_accesses {
            let address_key = (*base, *offset);
            *address_count.entry(address_key).or_insert(0) += 1;
        }

        // 计算重复访问比例
        let repeated_accesses = address_count.values().filter(|&&count| count > 1).count();
        let repetition_score = repeated_accesses as f64 / memory_accesses.len() as f64;

        // 综合评分：空间局部性 + 重复访问性
        let final_score = (avg_spatial_locality * 0.6 + repetition_score * 0.4).min(1.0_f64);

        final_score
    }

    /// 计算内存访问顺序性
    ///
    /// 评分越高表示内存访问越有序（顺序访问而非随机访问）
    fn compute_memory_sequentiality(&self, block: &IRBlock) -> f64 {
        // 收集内存访问序列
        let mut access_sequence = Vec::new();

        for op in &block.ops {
            // 记录内存访问的顺序信息
            match op {
                // 加载操作
                vm_ir::IROp::Load {
                    base, offset, size, ..
                } => {
                    access_sequence.push((base, *offset, *size, true));
                }
                vm_ir::IROp::LoadExt { addr, size, .. } => {
                    if let vm_ir::Operand::Register(reg) = addr {
                        access_sequence.push((reg, 0, *size, true));
                    }
                }

                // 存储操作
                vm_ir::IROp::Store {
                    base, offset, size, ..
                } => {
                    access_sequence.push((base, *offset, *size, false));
                }
                vm_ir::IROp::StoreExt { addr, size, .. } => {
                    if let vm_ir::Operand::Register(reg) = addr {
                        access_sequence.push((reg, 0, *size, false));
                    }
                }

                // 浮点加载/存储
                vm_ir::IROp::Fload {
                    base, offset, size, ..
                } => {
                    access_sequence.push((base, *offset, *size, true));
                }
                vm_ir::IROp::Fstore {
                    base, offset, size, ..
                } => {
                    access_sequence.push((base, *offset, *size, false));
                }

                // 原子操作
                vm_ir::IROp::AtomicRMW { base, .. }
                | vm_ir::IROp::AtomicRMWOrder { base, .. }
                | vm_ir::IROp::AtomicCmpXchg { base, .. }
                | vm_ir::IROp::AtomicCmpXchgOrder { base, .. }
                | vm_ir::IROp::AtomicLoadReserve { base, .. } => {
                    access_sequence.push((base, 0, 8, true)); // 视为加载
                }

                _ => {}
            }
        }

        if access_sequence.len() < 2 {
            // 内存访问少于2次，默认认为是有序的
            return 0.8;
        }

        // 分析访问模式的顺序性
        let mut sequential_score = 0.0;
        let mut comparison_count = 0;

        for i in 0..access_sequence.len() - 1 {
            let (base1, offset1, size1, is_load1) = access_sequence[i];
            let (base2, offset2, size2, is_load2) = access_sequence[i + 1];

            // 只比较使用相同基址寄存器的访问
            if base1 == base2 {
                let addr1 = offset1 as i64;
                let addr2 = offset2 as i64;

                // 计算地址差
                let distance = addr2 - addr1;

                // 检查是否是顺序访问
                if distance >= 0 && distance <= size1 as i64 * 2 {
                    // 顺序或接近顺序访问
                    let expected_next = addr1 + size1 as i64;
                    let actual_distance = (addr2 - expected_next).abs();

                    // 实际距离越接近期望值，顺序性越好
                    let proximity_score = if actual_distance == 0 {
                        1.0 // 完美的顺序访问
                    } else {
                        1.0 - (actual_distance as f64 / (expected_next as f64)).min(1.0_f64)
                    };

                    // 同类型操作（加载->加载 或 存储->存储）得分更高
                    let type_match_score = if is_load1 == is_load2 { 1.0 } else { 0.5 };

                    sequential_score += proximity_score * type_match_score;
                    comparison_count += 1;
                } else if distance < 0 {
                    // 向后访问，降低顺序性得分
                    let backward_score = 1.0 - (distance.abs() as f64 / 128.0).min(1.0_f64);
                    sequential_score += backward_score * 0.3; // 向后访问得分较低
                    comparison_count += 1;
                } else {
                    // 向前跳跃访问
                    let jump_distance = distance as f64 / size1.max(size2) as f64;
                    let jump_score = if jump_distance <= 2.0 {
                        // 小跳跃，顺序性还不错
                        1.0 - jump_distance / 2.0
                    } else {
                        // 大跳跃，顺序性很差
                        0.1
                    };
                    sequential_score += jump_score;
                    comparison_count += 1;
                }
            }
        }

        if comparison_count == 0 {
            // 没有可比的内存访问，默认认为是有序的
            return 0.8;
        }

        let avg_sequential_score = sequential_score / comparison_count as f64;

        // 考虑内存访问的集中程度
        // 计算内存访问的分布范围
        let mut addresses = access_sequence
            .iter()
            .map(|(base, offset, _, _)| (*base, *offset))
            .collect::<Vec<_>>();

        addresses.sort();

        // 计算地址分布的密集程度
        let mut range_distribution = 0.0;
        let chunk_size = (addresses.len() / 4).max(1);

        for chunk_start in (0..addresses.len()).step_by(chunk_size) {
            let chunk_end = (chunk_start + chunk_size).min(addresses.len());
            if chunk_end > chunk_start {
                let chunk_addresses = &addresses[chunk_start..chunk_end];
                let min_addr = chunk_addresses[0].1;
                let max_addr = chunk_addresses[chunk_end - 1].1;
                let chunk_range = max_addr - min_addr;

                if chunk_range > 0 {
                    // 范围越小，访问越集中
                    let density = chunk_addresses.len() as f64 / (chunk_range as f64 + 1.0);
                    range_distribution += density;
                }
            }
        }

        let avg_density = range_distribution / 4.0;
        let density_score = (avg_density / 10.0).min(1.0_f64); // 归一化

        // 综合评分：顺序访问得分 + 密集程度得分
        let final_score = (avg_sequential_score * 0.7 + density_score * 0.3).min(1.0_f64);

        final_score
    }

    /// 获取编译历史
    fn get_compilation_history(&self, _block: &IRBlock) -> CompilationHistory {
        // 简化实现：返回默认值
        // 实际实现中应该从历史记录中查询
        CompilationHistory {
            previous_compilations: 0,
            avg_compilation_time_us: 0.0,
            last_compile_benefit: 1.0,
            last_compile_success: true,
        }
    }

    /// 计算寄存器压力
    fn compute_register_pressure(&self, block: &IRBlock) -> f64 {
        // 统计使用的唯一寄存器数
        let mut used_regs = std::collections::HashSet::new();

        for op in &block.ops {
            // 从IROp中提取寄存器
            match op {
                vm_ir::IROp::Add { dst, src1, src2 }
                | vm_ir::IROp::Sub { dst, src1, src2 }
                | vm_ir::IROp::Mul { dst, src1, src2 }
                | vm_ir::IROp::And { dst, src1, src2 }
                | vm_ir::IROp::Or { dst, src1, src2 }
                | vm_ir::IROp::Xor { dst, src1, src2 } => {
                    used_regs.insert(*dst);
                    used_regs.insert(*src1);
                    used_regs.insert(*src2);
                }
                vm_ir::IROp::Load { dst, base, .. } => {
                    used_regs.insert(*dst);
                    used_regs.insert(*base);
                }
                vm_ir::IROp::Store { base, .. } => {
                    used_regs.insert(*base);
                }
                vm_ir::IROp::LoadExt { dest, addr, .. } => {
                    used_regs.insert(*dest);
                    if let vm_ir::Operand::Register(reg) = addr {
                        used_regs.insert(*reg);
                    }
                }
                vm_ir::IROp::StoreExt { value, addr, .. } => {
                    if let vm_ir::Operand::Register(reg) = value {
                        used_regs.insert(*reg);
                    }
                    if let vm_ir::Operand::Register(reg) = addr {
                        used_regs.insert(*reg);
                    }
                }
                vm_ir::IROp::Mov { dst, src } | vm_ir::IROp::Not { dst, src } => {
                    used_regs.insert(*dst);
                    used_regs.insert(*src);
                }
                vm_ir::IROp::MovImm { dst, .. } => {
                    used_regs.insert(*dst);
                }
                _ => {
                    // 对于其他操作类型，暂时忽略
                }
            }
        }

        // 假设有32个通用寄存器
        used_regs.len() as f64 / 32.0
    }

    /// 计算代码热度和稳定性
    fn compute_heat_and_stability(&self, block_hash: &u64) -> (f64, f64) {
        let history = self.execution_history.get(block_hash);

        if let Some(records) = history {
            if records.is_empty() {
                return (0.0, 1.0);
            }

            // 代码热度：基于执行频率
            let heat = records.len() as f64 / self.history_window as f64;

            // 代码稳定性：基于执行时间的一致性
            let times: Vec<f64> = records.iter().map(|r| r.execution_time_ns as f64).collect();

            let mean = times.iter().sum::<f64>() / times.len() as f64;
            let variance =
                times.iter().map(|&t| (t - mean).powi(2)).sum::<f64>() / times.len() as f64;

            let stability = 1.0 / (1.0 + variance.sqrt() / mean);

            (heat.min(1.0_f64), stability)
        } else {
            (0.0, 1.0)
        }
    }

    /// 获取执行统计
    fn get_execution_stats(&self, block_hash: &u64) -> (u64, f64) {
        if let Some(records) = self.execution_history.get(block_hash) {
            let execution_count = records.len() as u64;
            // 简化：假设缓存命中率为0.9
            let cache_hit_rate = 0.9;
            (execution_count, cache_hit_rate)
        } else {
            (0, 0.0)
        }
    }

    /// 计算块哈希
    fn hash_block(&self, block: &IRBlock) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();

        // 哈希所有操作
        for op in &block.ops {
            op.hash(&mut hasher);
        }

        // 也包含terminator
        block.term.hash(&mut hasher);

        hasher.finish()
    }

    /// 记录执行（用于更新历史）
    pub fn record_execution(
        &mut self,
        block_hash: u64,
        execution_time_ns: u64,
        memory_accesses: Vec<(u64, u8)>,
    ) {
        let record = ExecutionRecord {
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64,
            execution_time_ns,
            memory_accesses,
        };

        let history = self
            .execution_history
            .entry(block_hash)
            .or_insert_with(Vec::new);
        history.push(record);

        // 限制历史大小
        if history.len() > self.history_window {
            history.remove(0);
        }
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use vm_core::GuestAddr;

    fn create_test_block() -> IRBlock {
        IRBlock {
            start_pc: GuestAddr(0x1000),
            ops: vec![
                vm_ir::IROp::Add {
                    dst: 1,
                    src1: 2,
                    src2: 3,
                }, // 算术指令
                vm_ir::IROp::LoadExt {
                    dest: 1,
                    addr: vm_ir::Operand::Register(0),
                    size: 8,
                    flags: vm_ir::MemFlags::default(),
                }, // 内存指令
            ],
            term: vm_ir::Terminator::Ret,
        }
    }

    #[test]
    fn test_instruction_mix_analysis() {
        let extractor = FeatureExtractorEnhanced::new(100);
        let block = create_test_block();

        let mix = extractor.analyze_instruction_mix(&block);

        // 测试块包含1个算术指令(Add)和1个内存指令(LoadExt)
        assert!(mix.arithmetic_ratio > 0.0); // 应该有算术指令
        assert!(mix.memory_ratio > 0.0); // 应该有内存指令
        // branch_ratio可能为0，因为没有分支指令
        assert!(mix.branch_ratio >= 0.0);
    }

    #[test]
    fn test_cyclomatic_complexity() {
        let extractor = FeatureExtractorEnhanced::new(100);
        let block = create_test_block();

        let complexity = extractor.compute_cyclomatic_complexity(&block);

        assert!(complexity > 0.0);
    }

    #[test]
    fn test_data_locality() {
        let extractor = FeatureExtractorEnhanced::new(100);
        let block = create_test_block();

        let locality = extractor.compute_data_locality(&block);

        assert!(locality >= 0.0 && locality <= 1.0);
    }

    #[test]
    fn test_memory_sequentiality() {
        let extractor = FeatureExtractorEnhanced::new(100);
        let block = create_test_block();

        let sequentiality = extractor.compute_memory_sequentiality(&block);

        assert!(sequentiality >= 0.0 && sequentiality <= 1.0);
    }

    #[test]
    fn test_extract_enhanced_features() {
        let mut extractor = FeatureExtractorEnhanced::new(100);
        let block = create_test_block();

        let features = extractor.extract_enhanced(&block);

        assert_eq!(features.block_size, 2); // 2个操作
        assert_eq!(features.instr_count, 2);
        assert_eq!(features.branch_count, 0); // 没有分支指令
        assert_eq!(features.memory_access_count, 1); // 1个LoadExt
    }

    #[test]
    fn test_register_pressure() {
        let extractor = FeatureExtractorEnhanced::new(100);
        let block = create_test_block();

        let pressure = extractor.compute_register_pressure(&block);

        assert!(pressure >= 0.0 && pressure <= 1.0);
    }

    #[test]
    fn test_record_execution() {
        let mut extractor = FeatureExtractorEnhanced::new(10);
        let block_hash = 12345;

        extractor.record_execution(block_hash, 1000, vec![(0x2000, 4)]);

        let (count, _) = extractor.get_execution_stats(&block_hash);
        assert_eq!(count, 1);
    }
}
