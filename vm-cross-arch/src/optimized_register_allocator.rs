//! 优化的寄存器分配器
//!
//! 实现高级寄存器分配算法，减少不必要的寄存器拷贝和移动

use super::{Architecture, RegisterClass, RegisterInfo, CallingConvention, InterferenceNode};
use std::collections::{HashMap, HashSet, VecDeque};
use vm_ir::RegId;

/// 寄存器生命周期
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterLifetime {
    /// 寄存器ID
    pub reg: RegId,
    /// 定义点（指令索引）
    pub def_point: usize,
    /// 最后使用点（指令索引）
    pub last_use: usize,
    /// 使用点列表
    pub use_points: Vec<usize>,
    /// 是否是循环中的寄存器
    pub in_loop: bool,
    /// 循环嵌套深度
    pub loop_depth: usize,
}

/// 寄存器拷贝记录
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterCopy {
    /// 源寄存器
    pub src: RegId,
    /// 目标寄存器
    pub dst: RegId,
    /// 拷贝发生的指令位置
    pub position: usize,
    /// 是否可以消除
    pub can_eliminate: bool,
    /// 消除原因
    pub eliminate_reason: Option<String>,
}

/// 临时寄存器使用记录
#[derive(Debug, Clone)]
pub struct TempRegisterUsage {
    /// 寄存器ID
    pub reg: RegId,
    /// 分配点
    pub allocated_at: usize,
    /// 释放点
    pub released_at: Option<usize>,
    /// 使用次数
    pub use_count: usize,
    /// 是否可以重用
    pub can_reuse: bool,
}

/// 优化的寄存器映射器
pub struct OptimizedRegisterMapper {
    /// 目标架构寄存器信息
    target_regs: Vec<RegisterInfo>,
    /// 调用约定
    calling_conv: CallingConvention,
    /// 当前映射关系：源寄存器ID -> 目标寄存器ID
    current_mapping: HashMap<RegId, RegId>,
    /// 已分配的目标寄存器
    allocated_targets: HashSet<RegId>,
    /// 寄存器生命周期
    lifetimes: Vec<RegisterLifetime>,
    /// 寄存器拷贝记录
    copies: Vec<RegisterCopy>,
    /// 临时寄存器使用记录
    temp_usages: Vec<TempRegisterUsage>,
    /// 可重用的临时寄存器池
    reusable_temps: VecDeque<RegId>,
    /// 下一个可用的临时寄存器
    next_temp: usize,
    /// 寄存器重用映射：源寄存器 -> 可重用的目标寄存器
    reuse_mapping: HashMap<RegId, RegId>,
    /// 优化统计
    optimization_stats: OptimizationStats,
}

/// 优化统计信息
#[derive(Debug, Clone, Default)]
pub struct OptimizationStats {
    /// 消除的寄存器拷贝数
    pub copies_eliminated: usize,
    /// 寄存器重用次数
    pub registers_reused: usize,
    /// 临时寄存器重用次数
    pub temps_reused: usize,
    /// 总拷贝数
    pub total_copies: usize,
    /// 总寄存器分配数
    pub total_allocations: usize,
}

impl OptimizedRegisterMapper {
    /// 创建新的优化寄存器映射器
    pub fn new(target_arch: Architecture) -> Self {
        let target_regs = Self::get_target_registers(target_arch);
        let calling_conv = Self::get_calling_convention(target_arch);

        Self {
            target_regs,
            calling_conv,
            current_mapping: HashMap::new(),
            allocated_targets: HashSet::new(),
            lifetimes: Vec::new(),
            copies: Vec::new(),
            temp_usages: Vec::new(),
            reusable_temps: VecDeque::new(),
            next_temp: 0,
            reuse_mapping: HashMap::new(),
            optimization_stats: OptimizationStats::default(),
        }
    }

    /// 获取目标架构的寄存器信息
    fn get_target_registers(arch: Architecture) -> Vec<RegisterInfo> {
        // 复用原有实现
        match arch {
            Architecture::X86_64 => {
                vec![
                    // 通用寄存器
                    RegisterInfo { id: 0, class: RegisterClass::General, callee_saved: false, can_be_temp: true },  // RAX
                    RegisterInfo { id: 1, class: RegisterClass::General, callee_saved: false, can_be_temp: true },  // RCX
                    RegisterInfo { id: 2, class: RegisterClass::General, callee_saved: false, can_be_temp: true },  // RDX
                    RegisterInfo { id: 3, class: RegisterClass::General, callee_saved: false, can_be_temp: true },  // RBX
                    RegisterInfo { id: 4, class: RegisterClass::General, callee_saved: true, can_be_temp: false },  // RSP
                    RegisterInfo { id: 5, class: RegisterClass::General, callee_saved: true, can_be_temp: false },  // RBP
                    RegisterInfo { id: 6, class: RegisterClass::General, callee_saved: false, can_be_temp: true },  // RSI
                    RegisterInfo { id: 7, class: RegisterClass::General, callee_saved: false, can_be_temp: true },  // RDI
                    RegisterInfo { id: 8, class: RegisterClass::General, callee_saved: false, can_be_temp: true },  // R8
                    RegisterInfo { id: 9, class: RegisterClass::General, callee_saved: false, can_be_temp: true },  // R9
                    RegisterInfo { id: 10, class: RegisterClass::General, callee_saved: false, can_be_temp: true }, // R10
                    RegisterInfo { id: 11, class: RegisterClass::General, callee_saved: false, can_be_temp: true }, // R11
                    RegisterInfo { id: 12, class: RegisterClass::General, callee_saved: true, can_be_temp: true },  // R12
                    RegisterInfo { id: 13, class: RegisterClass::General, callee_saved: true, can_be_temp: true },  // R13
                    RegisterInfo { id: 14, class: RegisterClass::General, callee_saved: true, can_be_temp: true },  // R14
                    RegisterInfo { id: 15, class: RegisterClass::General, callee_saved: true, can_be_temp: true },  // R15
                    // 浮点寄存器（XMM）
                    RegisterInfo { id: 16, class: RegisterClass::Float, callee_saved: false, can_be_temp: true }, // XMM0
                    RegisterInfo { id: 17, class: RegisterClass::Float, callee_saved: false, can_be_temp: true }, // XMM1
                    RegisterInfo { id: 18, class: RegisterClass::Float, callee_saved: false, can_be_temp: true }, // XMM2
                    RegisterInfo { id: 19, class: RegisterClass::Float, callee_saved: false, can_be_temp: true }, // XMM3
                    RegisterInfo { id: 20, class: RegisterClass::Float, callee_saved: false, can_be_temp: true }, // XMM4
                    RegisterInfo { id: 21, class: RegisterClass::Float, callee_saved: false, can_be_temp: true }, // XMM5
                    RegisterInfo { id: 22, class: RegisterClass::Float, callee_saved: false, can_be_temp: true }, // XMM6
                    RegisterInfo { id: 23, class: RegisterClass::Float, callee_saved: true, can_be_temp: true },  // XMM7
                ]
            }
            Architecture::ARM64 => {
                let mut regs = Vec::new();
                // X0-X30: 通用寄存器
                for i in 0..31 {
                    regs.push(RegisterInfo {
                        id: i,
                        class: RegisterClass::General,
                        callee_saved: (i >= 19 && i <= 28), // X19-X28 are callee-saved
                        can_be_temp: i != 29 && i != 30, // X29(FP) and X30(LR) are special
                    });
                }
                // X31: SP/Zero register
                regs.push(RegisterInfo {
                    id: 31,
                    class: RegisterClass::Special,
                    callee_saved: true,
                    can_be_temp: false,
                });
                // 浮点/向量寄存器 V0-V31
                for i in 0..32 {
                    regs.push(RegisterInfo {
                        id: 32 + i,
                        class: RegisterClass::Vector,
                        callee_saved: (i >= 8 && i <= 15), // V8-V15 are callee-saved
                        can_be_temp: true,
                    });
                }
                regs
            }
            Architecture::RISCV64 => {
                let mut regs = Vec::new();
                // X0-X31: 通用寄存器
                for i in 0..32 {
                    regs.push(RegisterInfo {
                        id: i,
                        class: RegisterClass::General,
                        callee_saved: (i >= 8 && i <= 9) || (i >= 18 && i <= 27), // s0-s1, s2-s11 are callee-saved
                        can_be_temp: i != 0 && i != 2, // x0(zero) and x2(sp) are special
                    });
                }
                // 浮点寄存器 f0-f31
                for i in 0..32 {
                    regs.push(RegisterInfo {
                        id: 32 + i,
                        class: RegisterClass::Float,
                        callee_saved: (i >= 8 && i <= 9) || (i >= 18 && i <= 27), // fs0-fs1, fs2-fs11 are callee-saved
                        can_be_temp: true,
                    });
                }
                regs
            }
        }
    }

    /// 获取目标架构的调用约定
    fn get_calling_convention(arch: Architecture) -> CallingConvention {
        match arch {
            Architecture::X86_64 => CallingConvention {
                // System V AMD64 ABI
                arg_regs: vec![5, 4, 1, 0, 2, 3], // RDI, RSI, RDX, RCX, R8, R9
                ret_regs: vec![0], // RAX
                caller_saved: vec![0, 1, 2, 8, 9, 10, 11], // RAX, RCX, RDX, R8-R11
                callee_saved: vec![3, 12, 13, 14, 15], // RBX, R12-R15
                stack_ptr: 4, // RSP
                base_ptr: Some(5), // RBP
            },
            Architecture::ARM64 => CallingConvention {
                // AAPCS64
                arg_regs: (0..8).collect(), // X0-X7
                ret_regs: vec![0], // X0
                caller_saved: (0..15).collect(), // X0-X14
                callee_saved: (19..28).collect(), // X19-X28
                stack_ptr: 31, // SP (X31)
                base_ptr: Some(29), // FP (X29)
            },
            Architecture::RISCV64 => CallingConvention {
                // RISC-V calling convention
                arg_regs: (10..17).collect(), // a0-a7 (x10-x17)
                ret_regs: vec![10, 11], // a0-a1 (x10-x11)
                caller_saved: (5..7).collect(), // t0-t1 (x5-x6) and t2-t6 (x27-x31)
                callee_saved: (8..9).collect(), // s0-s1 (x8-x9) and s2-s11 (x18-x27)
                stack_ptr: 2, // sp (x2)
                base_ptr: Some(8), // fp (x8)
            },
        }
    }

    /// 分析寄存器生命周期
    pub fn analyze_lifetimes(&mut self, instructions: &[vm_ir::IROp]) {
        self.lifetimes.clear();
        let mut use_counts: HashMap<RegId, usize> = HashMap::new();
        let mut def_points: HashMap<RegId, usize> = HashMap::new();
        let mut last_uses: HashMap<RegId, usize> = HashMap::new();

        // 第一遍：收集定义点和使用点
        for (idx, insn) in instructions.iter().enumerate() {
            // 分析寄存器使用
            let mut used_regs = Vec::new();
            let mut defined_regs = Vec::new();

            match insn {
                vm_ir::IROp::Add { dst, src1, src2 } |
                vm_ir::IROp::Sub { dst, src1, src2 } |
                vm_ir::IROp::Mul { dst, src1, src2 } |
                vm_ir::IROp::Div { dst, src1, src2, .. } |
                vm_ir::IROp::Rem { dst, src1, src2, .. } |
                vm_ir::IROp::And { dst, src1, src2 } |
                vm_ir::IROp::Or { dst, src1, src2 } |
                vm_ir::IROp::Xor { dst, src1, src2 } => {
                    used_regs.push(*src1);
                    used_regs.push(*src2);
                    defined_regs.push(*dst);
                }
                vm_ir::IROp::Sll { dst, src, shreg } |
                vm_ir::IROp::Srl { dst, src, shreg } |
                vm_ir::IROp::Sra { dst, src, shreg } => {
                    used_regs.push(*src);
                    used_regs.push(*shreg);
                    defined_regs.push(*dst);
                }
                vm_ir::IROp::Load { dst, .. } |
                vm_ir::IROp::MovImm { dst, .. } => {
                    defined_regs.push(*dst);
                }
                vm_ir::IROp::Store { src, .. } => {
                    used_regs.push(*src);
                }
                vm_ir::IROp::CmpEq { dst, lhs, rhs } |
                vm_ir::IROp::CmpNe { dst, lhs, rhs } |
                vm_ir::IROp::CmpLt { dst, lhs, rhs } |
                vm_ir::IROp::CmpLtU { dst, lhs, rhs } |
                vm_ir::IROp::CmpGe { dst, lhs, rhs } |
                vm_ir::IROp::CmpGeU { dst, lhs, rhs } => {
                    used_regs.push(*lhs);
                    used_regs.push(*rhs);
                    defined_regs.push(*dst);
                }
                vm_ir::IROp::Mov { dst, src } => {
                    used_regs.push(*src);
                    defined_regs.push(*dst);
                }
                _ => {}
            }

            // 记录使用点
            for &reg in &used_regs {
                *use_counts.entry(reg).or_insert(0) += 1;
                last_uses.insert(reg, idx);
            }

            // 记录定义点
            for &reg in &defined_regs {
                def_points.entry(reg).or_insert(idx);
            }
        }

        // 创建生命周期记录
        for (&reg, &_use_count) in &use_counts {
            let def_point = def_points.get(&reg).copied().unwrap_or(0);
            let last_use = last_uses.get(&reg).copied().unwrap_or(0);

            self.lifetimes.push(RegisterLifetime {
                reg,
                def_point,
                last_use,
                use_points: Vec::new(), // 可以进一步细化
                in_loop: false, // 需要循环检测
                loop_depth: 0,
            });
        }
    }

    /// 识别并消除不必要的寄存器拷贝
    pub fn eliminate_copies(&mut self, instructions: &[vm_ir::IROp]) -> Vec<vm_ir::IROp> {
        let mut optimized = Vec::new();
        let mut copy_map: HashMap<(RegId, RegId), usize> = HashMap::new();

        for (idx, insn) in instructions.iter().enumerate() {
            match insn {
                vm_ir::IROp::Mov { dst, src } => {
                    // 记录拷贝
                    self.copies.push(RegisterCopy {
                        src: *src,
                        dst: *dst,
                        position: idx,
                        can_eliminate: false,
                        eliminate_reason: None,
                    });
                    self.optimization_stats.total_copies += 1;

                    // 检查是否可以消除这个拷贝
                    if self.can_eliminate_copy(*src, *dst, &copy_map) {
                        // 标记为可消除
                        if let Some(copy) = self.copies.last_mut() {
                            copy.can_eliminate = true;
                            copy.eliminate_reason = Some("Redundant copy".to_string());
                        }
                        self.optimization_stats.copies_eliminated += 1;

                        // 更新拷贝映射
                        copy_map.insert((*src, *dst), idx);
                        
                        // 不添加到优化后的指令列表
                        continue;
                    }
                }
                _ => {}
            }

            optimized.push(insn.clone());
        }

        optimized
    }

    /// 检查是否可以消除寄存器拷贝
    fn can_eliminate_copy(&self, src: RegId, dst: RegId, copy_map: &HashMap<(RegId, RegId), usize>) -> bool {
        // 如果源和目标相同，可以消除
        if src == dst {
            return true;
        }

        // 如果已经存在相同的拷贝，可以消除
        if copy_map.contains_key(&(src, dst)) {
            return true;
        }

        // 如果源寄存器在拷贝后不再使用，可以消除
        if let Some(lifetime) = self.lifetimes.iter().find(|lt| lt.reg == src) {
            // 如果源寄存器的最后使用点就在当前指令附近，可以考虑消除拷贝
            // 这里简化实现，检查源寄存器是否只被使用一次（定义后立即使用）
            if lifetime.use_points.len() == 1 && lifetime.def_point + 1 == lifetime.last_use {
                return true;
            }
        }

        false
    }

    /// 优化的寄存器分配
    pub fn allocate_registers(&mut self, instructions: &[vm_ir::IROp]) -> Result<(), String> {
        // 分析寄存器生命周期
        self.analyze_lifetimes(instructions);

        // 消除不必要的拷贝
        let optimized_instructions = self.eliminate_copies(instructions);

        // 构建活跃范围
        let live_ranges = self.build_live_ranges(&optimized_instructions);

        // 构建冲突图
        let mut nodes = self.build_interference_graph(&live_ranges);

        // 按优先级排序（冲突多的先分配）
        nodes.sort_by(|a, b| b.priority.cmp(&a.priority));

        // 尝试为每个节点分配寄存器
        for node in &mut nodes {
            // 尝试重用寄存器
            if let Some(reused_reg) = self.try_reuse_register(node) {
                self.current_mapping.insert(node.reg, reused_reg);
                self.allocated_targets.insert(reused_reg);
                node.allocated = true;
                self.optimization_stats.registers_reused += 1;
                continue;
            }

            // 尝试分配新寄存器
            if let Some(target_reg) = self.find_available_register(node) {
                self.current_mapping.insert(node.reg, target_reg);
                self.allocated_targets.insert(target_reg);
                node.allocated = true;
                self.optimization_stats.total_allocations += 1;
            } else {
                // 无法分配，需要溢出到栈
                return Err(format!("Failed to allocate register for source register {}", node.reg));
            }
        }

        Ok(())
    }

    /// 尝试重用寄存器
    fn try_reuse_register(&mut self, node: &InterferenceNode) -> Option<RegId> {
        // 检查是否有可重用的寄存器
        if let Some(&reusable_reg) = self.reuse_mapping.get(&node.reg) {
            // 检查是否与已分配的寄存器冲突
            if !self.allocated_targets.contains(&reusable_reg) {
                return Some(reusable_reg);
            }
        }

        // 检查临时寄存器池
        if let Some(temp_reg) = self.reusable_temps.pop_front() {
            self.optimization_stats.temps_reused += 1;
            return Some(temp_reg);
        }

        None
    }

    /// 构建活跃范围
    fn build_live_ranges(&self, _instructions: &[vm_ir::IROp]) -> Vec<(RegId, (usize, usize))> {
        let mut live_ranges = Vec::new();

        for lifetime in &self.lifetimes {
            live_ranges.push((lifetime.reg, (lifetime.def_point, lifetime.last_use)));
        }

        live_ranges
    }

    /// 构建寄存器冲突图
    fn build_interference_graph(&self, live_ranges: &[(RegId, (usize, usize))]) -> Vec<InterferenceNode> {
        let mut nodes: HashMap<RegId, InterferenceNode> = HashMap::new();

        // 初始化所有节点
        for &(reg, (_start, _end)) in live_ranges {
            let class = self.get_register_class(reg);
            nodes.insert(reg, InterferenceNode {
                reg,
                conflicts: HashSet::new(),
                class,
                priority: 0,
                allocated: false,
            });
        }

        // 构建冲突边
        for i in 0..live_ranges.len() {
            let (reg_i, (start_i, end_i)) = live_ranges[i];
            for j in (i + 1)..live_ranges.len() {
                let (reg_j, (start_j, end_j)) = live_ranges[j];

                // 如果两个寄存器的活跃范围重叠，则它们冲突
                if start_i <= end_j && start_j <= end_i {
                    nodes.get_mut(&reg_i).unwrap().conflicts.insert(reg_j);
                    nodes.get_mut(&reg_j).unwrap().conflicts.insert(reg_i);
                }
            }
        }

        // 计算优先级（基于冲突数量）
        for node in nodes.values_mut() {
            node.priority = node.conflicts.len() as u32;
        }

        nodes.into_values().collect()
    }

    /// 获取寄存器类
    fn get_register_class(&self, reg: RegId) -> RegisterClass {
        // 简化实现：根据寄存器编号判断类
        if reg < 16 {
            RegisterClass::General
        } else if reg < 24 {
            RegisterClass::Float
        } else {
            RegisterClass::Vector
        }
    }

    /// 查找可用的目标寄存器
    fn find_available_register(&self, node: &InterferenceNode) -> Option<RegId> {
        // 获取相同类的可用寄存器
        let candidates: Vec<&RegisterInfo> = self.target_regs
            .iter()
            .filter(|r| r.class == node.class && !self.allocated_targets.contains(&r.id))
            .collect();

        // 优先选择调用者保存的寄存器（减少保存/恢复开销）
        for candidate in &candidates {
            if !candidate.callee_saved && candidate.can_be_temp {
                return Some(candidate.id);
            }
        }

        // 其次选择被调用者保存的寄存器
        for candidate in &candidates {
            if candidate.callee_saved && candidate.can_be_temp {
                return Some(candidate.id);
            }
        }

        // 最后选择任何可用的寄存器
        candidates.first().map(|r| r.id)
    }

    /// 映射源寄存器到目标寄存器
    pub fn map_register(&self, source_reg: RegId) -> RegId {
        self.current_mapping.get(&source_reg).copied().unwrap_or(0)
    }

    /// 分配临时寄存器
    pub fn allocate_temp(&mut self) -> Option<RegId> {
        // 首先尝试从可重用池中获取
        if let Some(temp_reg) = self.reusable_temps.pop_front() {
            self.allocated_targets.insert(temp_reg);
            self.temp_usages.push(TempRegisterUsage {
                reg: temp_reg,
                allocated_at: 0, // 需要实际跟踪
                released_at: None,
                use_count: 0,
                can_reuse: true,
            });
            return Some(temp_reg);
        }

        // 分配新的临时寄存器
        if self.next_temp < self.calling_conv.caller_saved.len() {
            let reg = self.calling_conv.caller_saved[self.next_temp];
            self.next_temp += 1;
            self.allocated_targets.insert(reg);
            self.temp_usages.push(TempRegisterUsage {
                reg,
                allocated_at: 0, // 需要实际跟踪
                released_at: None,
                use_count: 0,
                can_reuse: true,
            });
            Some(reg)
        } else {
            None
        }
    }

    /// 释放临时寄存器
    pub fn release_temp(&mut self, reg: RegId) {
        self.allocated_targets.remove(&reg);
        
        // 更新使用记录
        if let Some(usage) = self.temp_usages.iter_mut().find(|u| u.reg == reg) {
            usage.released_at = Some(0); // 需要实际跟踪
            usage.use_count += 1;
            
            // 如果可以重用，添加到重用池
            if usage.can_reuse {
                self.reusable_temps.push_back(reg);
            }
        }
    }

    /// 重置所有映射
    pub fn reset(&mut self) {
        self.current_mapping.clear();
        self.allocated_targets.clear();
        self.lifetimes.clear();
        self.copies.clear();
        self.temp_usages.clear();
        self.reusable_temps.clear();
        self.next_temp = 0;
        self.reuse_mapping.clear();
        self.optimization_stats = OptimizationStats::default();
    }

    /// 获取当前映射统计信息
    pub fn get_stats(&self) -> super::RegisterAllocationStats {
        super::RegisterAllocationStats {
            total_mappings: self.current_mapping.len(),
            allocated_targets: self.allocated_targets.len(),
            temp_allocated: self.next_temp,
            available_temps: self.reusable_temps.len(),
        }
    }

    /// 获取优化统计信息
    pub fn get_optimization_stats(&self) -> &OptimizationStats {
        &self.optimization_stats
    }

    /// 设置寄存器重用映射
    pub fn set_reuse_mapping(&mut self, src: RegId, target: RegId) {
        self.reuse_mapping.insert(src, target);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_ir::{IRBuilder, IROp};

    #[test]
    fn test_optimized_register_mapper() {
        let mut mapper = OptimizedRegisterMapper::new(Architecture::ARM64);
        
        // 创建测试指令
        let mut builder = IRBuilder::new(0x1000);
        builder.push(IROp::Const { dst: 0, value: 42 });
        builder.push(IROp::Mov { dst: 1, src: 0 });
        builder.push(IROp::Add { dst: 2, src1: 1, src2: 0 });
        let instructions = builder.build().ops;

        // 分析生命周期
        mapper.analyze_lifetimes(&instructions);
        
        // 验证生命周期分析
        assert_eq!(mapper.lifetimes.len(), 3); // 三个寄存器被使用
        
        // 消除拷贝
        let optimized = mapper.eliminate_copies(&instructions);
        
        // 验证拷贝消除
        assert!(optimized.len() <= instructions.len());
        
        // 获取优化统计
        let stats = mapper.get_optimization_stats();
        assert!(stats.total_copies > 0);
    }

    #[test]
    fn test_temp_register_reuse() {
        let mut mapper = OptimizedRegisterMapper::new(Architecture::ARM64);
        
        // 分配临时寄存器
        let temp1 = mapper.allocate_temp();
        assert!(temp1.is_some());
        
        // 释放临时寄存器
        mapper.release_temp(temp1.unwrap());
        
        // 再次分配，应该重用之前的寄存器
        let temp2 = mapper.allocate_temp();
        assert!(temp2.is_some());
        
        // 验证重用统计
        let stats = mapper.get_optimization_stats();
        assert!(stats.temps_reused > 0);
    }
}