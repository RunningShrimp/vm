//! 智能寄存器分配器
//!
//! 实现基于图着色算法的寄存器分配，考虑架构特定的调用约定

use super::Architecture;
use std::collections::{HashMap, HashSet};
use vm_ir::RegId;

/// 寄存器类定义
#[derive(Debug, Clone, PartialEq)]
pub enum RegisterClass {
    /// 通用寄存器
    General,
    /// 浮点寄存器
    Float,
    /// 向量寄存器
    Vector,
    /// 特殊寄存器（如栈指针、程序计数器等）
    Special,
}

/// 寄存器信息
#[derive(Debug, Clone)]
pub struct RegisterInfo {
    /// 寄存器ID
    pub id: RegId,
    /// 寄存器类
    pub class: RegisterClass,
    /// 是否被调用者保存（callee-saved）
    pub callee_saved: bool,
    /// 是否可作为临时寄存器
    pub can_be_temp: bool,
}

/// 调用约定定义
#[derive(Debug, Clone)]
pub struct CallingConvention {
    /// 参数寄存器（按顺序）
    pub arg_regs: Vec<RegId>,
    /// 返回值寄存器
    pub ret_regs: Vec<RegId>,
    /// 调用者保存寄存器
    pub caller_saved: Vec<RegId>,
    /// 被调用者保存寄存器
    pub callee_saved: Vec<RegId>,
    /// 栈指针寄存器
    pub stack_ptr: RegId,
    /// 基址指针寄存器
    pub base_ptr: Option<RegId>,
}

/// 寄存器冲突图节点
#[derive(Debug, Clone)]
pub struct InterferenceNode {
    /// 寄存器ID
    pub reg: RegId,
    /// 与此寄存器冲突的其他寄存器
    pub conflicts: HashSet<RegId>,
    /// 寄存器类
    pub class: RegisterClass,
    /// 优先级（用于着色时的启发式）
    pub priority: u32,
    /// 是否已分配
    pub allocated: bool,
}

/// 智能寄存器映射器
pub struct SmartRegisterMapper {
    /// 目标架构寄存器信息
    target_regs: Vec<RegisterInfo>,
    /// 调用约定
    calling_conv: CallingConvention,
    /// 当前映射关系：源寄存器ID -> 目标寄存器ID
    current_mapping: HashMap<RegId, RegId>,
    /// 已分配的目标寄存器
    allocated_targets: HashSet<RegId>,
    /// 临时寄存器池
    temp_pool: Vec<RegId>,
    /// 下一个可用的临时寄存器
    next_temp: usize,
}

impl SmartRegisterMapper {
    /// 创建新的智能寄存器映射器
    pub fn new(target_arch: Architecture) -> Self {
        let target_regs = Self::get_target_registers(target_arch);
        let calling_conv = Self::get_calling_convention(target_arch);

        // 初始化临时寄存器池（使用调用者保存的寄存器）
        let temp_pool: Vec<RegId> = calling_conv
            .caller_saved
            .iter()
            .filter(|&&reg| target_regs.iter().any(|r| r.id == reg && r.can_be_temp))
            .copied()
            .collect();

        Self {
            target_regs,
            calling_conv,
            current_mapping: HashMap::new(),
            allocated_targets: HashSet::new(),
            temp_pool,
            next_temp: 0,
        }
    }

    /// 获取目标架构的寄存器信息
    fn get_target_registers(arch: Architecture) -> Vec<RegisterInfo> {
        match arch {
            Architecture::X86_64 => {
                vec![
                    // 通用寄存器
                    RegisterInfo {
                        id: 0,
                        class: RegisterClass::General,
                        callee_saved: false,
                        can_be_temp: true,
                    }, // RAX
                    RegisterInfo {
                        id: 1,
                        class: RegisterClass::General,
                        callee_saved: false,
                        can_be_temp: true,
                    }, // RCX
                    RegisterInfo {
                        id: 2,
                        class: RegisterClass::General,
                        callee_saved: false,
                        can_be_temp: true,
                    }, // RDX
                    RegisterInfo {
                        id: 3,
                        class: RegisterClass::General,
                        callee_saved: false,
                        can_be_temp: true,
                    }, // RBX
                    RegisterInfo {
                        id: 4,
                        class: RegisterClass::General,
                        callee_saved: true,
                        can_be_temp: false,
                    }, // RSP
                    RegisterInfo {
                        id: 5,
                        class: RegisterClass::General,
                        callee_saved: true,
                        can_be_temp: false,
                    }, // RBP
                    RegisterInfo {
                        id: 6,
                        class: RegisterClass::General,
                        callee_saved: false,
                        can_be_temp: true,
                    }, // RSI
                    RegisterInfo {
                        id: 7,
                        class: RegisterClass::General,
                        callee_saved: false,
                        can_be_temp: true,
                    }, // RDI
                    RegisterInfo {
                        id: 8,
                        class: RegisterClass::General,
                        callee_saved: false,
                        can_be_temp: true,
                    }, // R8
                    RegisterInfo {
                        id: 9,
                        class: RegisterClass::General,
                        callee_saved: false,
                        can_be_temp: true,
                    }, // R9
                    RegisterInfo {
                        id: 10,
                        class: RegisterClass::General,
                        callee_saved: false,
                        can_be_temp: true,
                    }, // R10
                    RegisterInfo {
                        id: 11,
                        class: RegisterClass::General,
                        callee_saved: false,
                        can_be_temp: true,
                    }, // R11
                    RegisterInfo {
                        id: 12,
                        class: RegisterClass::General,
                        callee_saved: true,
                        can_be_temp: true,
                    }, // R12
                    RegisterInfo {
                        id: 13,
                        class: RegisterClass::General,
                        callee_saved: true,
                        can_be_temp: true,
                    }, // R13
                    RegisterInfo {
                        id: 14,
                        class: RegisterClass::General,
                        callee_saved: true,
                        can_be_temp: true,
                    }, // R14
                    RegisterInfo {
                        id: 15,
                        class: RegisterClass::General,
                        callee_saved: true,
                        can_be_temp: true,
                    }, // R15
                    // 浮点寄存器（XMM）
                    RegisterInfo {
                        id: 16,
                        class: RegisterClass::Float,
                        callee_saved: false,
                        can_be_temp: true,
                    }, // XMM0
                    RegisterInfo {
                        id: 17,
                        class: RegisterClass::Float,
                        callee_saved: false,
                        can_be_temp: true,
                    }, // XMM1
                    RegisterInfo {
                        id: 18,
                        class: RegisterClass::Float,
                        callee_saved: false,
                        can_be_temp: true,
                    }, // XMM2
                    RegisterInfo {
                        id: 19,
                        class: RegisterClass::Float,
                        callee_saved: false,
                        can_be_temp: true,
                    }, // XMM3
                    RegisterInfo {
                        id: 20,
                        class: RegisterClass::Float,
                        callee_saved: false,
                        can_be_temp: true,
                    }, // XMM4
                    RegisterInfo {
                        id: 21,
                        class: RegisterClass::Float,
                        callee_saved: false,
                        can_be_temp: true,
                    }, // XMM5
                    RegisterInfo {
                        id: 22,
                        class: RegisterClass::Float,
                        callee_saved: false,
                        can_be_temp: true,
                    }, // XMM6
                    RegisterInfo {
                        id: 23,
                        class: RegisterClass::Float,
                        callee_saved: true,
                        can_be_temp: true,
                    }, // XMM7
                ]
            }
            Architecture::ARM64 => {
                let mut regs = Vec::new();
                // X0-X30: 通用寄存器
                for i in 0..31 {
                    regs.push(RegisterInfo {
                        id: i,
                        class: RegisterClass::General,
                        callee_saved: (19..=28).contains(&i), // X19-X28 are callee-saved
                        can_be_temp: i != 29 && i != 30,      // X29(FP) and X30(LR) are special
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
                        callee_saved: (8..=15).contains(&i), // V8-V15 are callee-saved
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
                        callee_saved: (8..=9).contains(&i) || (18..=27).contains(&i), // s0-s1, s2-s11 are callee-saved
                        can_be_temp: i != 0 && i != 2, // x0(zero) and x2(sp) are special
                    });
                }
                // 浮点寄存器 f0-f31
                for i in 0..32 {
                    regs.push(RegisterInfo {
                        id: 32 + i,
                        class: RegisterClass::Float,
                        callee_saved: (8..=9).contains(&i) || (18..=27).contains(&i), // fs0-fs1, fs2-fs11 are callee-saved
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
                ret_regs: vec![0],                // RAX
                caller_saved: vec![0, 1, 2, 8, 9, 10, 11], // RAX, RCX, RDX, R8-R11
                callee_saved: vec![3, 12, 13, 14, 15], // RBX, R12-R15
                stack_ptr: 4,                     // RSP
                base_ptr: Some(5),                // RBP
            },
            Architecture::ARM64 => CallingConvention {
                // AAPCS64
                arg_regs: (0..8).collect(),       // X0-X7
                ret_regs: vec![0],                // X0
                caller_saved: (0..15).collect(),  // X0-X14
                callee_saved: (19..28).collect(), // X19-X28
                stack_ptr: 31,                    // SP (X31)
                base_ptr: Some(29),               // FP (X29)
            },
            Architecture::RISCV64 => CallingConvention {
                // RISC-V calling convention
                arg_regs: (10..17).collect(),   // a0-a7 (x10-x17)
                ret_regs: vec![10, 11],         // a0-a1 (x10-x11)
                caller_saved: (5..7).collect(), // t0-t1 (x5-x6) and t2-t6 (x27-x31)
                callee_saved: (8..9).collect(), // s0-s1 (x8-x9) and s2-s11 (x18-x27)
                stack_ptr: 2,                   // sp (x2)
                base_ptr: Some(8),              // fp (x8)
            },
        }
    }

    /// 构建寄存器冲突图
    pub fn build_interference_graph(
        &self,
        live_ranges: &[(RegId, (usize, usize))],
    ) -> Vec<InterferenceNode> {
        let mut nodes: HashMap<RegId, InterferenceNode> = HashMap::new();

        // 初始化所有节点
        for &(reg, (_start, _end)) in live_ranges {
            let class = self.get_register_class(reg);
            nodes.insert(
                reg,
                InterferenceNode {
                    reg,
                    conflicts: HashSet::new(),
                    class,
                    priority: 0,
                    allocated: false,
                },
            );
        }

        // 构建冲突边
        for (i, &(reg_i, (start_i, end_i))) in live_ranges.iter().enumerate() {
            for &(reg_j, (start_j, end_j)) in &live_ranges[i + 1..] {
                // 如果两个寄存器的活跃范围重叠，则它们冲突
                if start_i <= end_j && start_j <= end_i {
                    if let Some(node_i) = nodes.get_mut(&reg_i) {
                        node_i.conflicts.insert(reg_j);
                    }
                    if let Some(node_j) = nodes.get_mut(&reg_j) {
                        node_j.conflicts.insert(reg_i);
                    }
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

    /// 使用图着色算法分配寄存器
    pub fn allocate_registers(
        &mut self,
        live_ranges: &[(RegId, (usize, usize))],
    ) -> Result<(), String> {
        let mut nodes = self.build_interference_graph(live_ranges);

        // 按优先级排序（冲突多的先分配）
        nodes.sort_by(|a, b| b.priority.cmp(&a.priority));

        // 尝试为每个节点分配寄存器
        for node in &mut nodes {
            if let Some(target_reg) = self.find_available_register(node) {
                self.current_mapping.insert(node.reg, target_reg);
                self.allocated_targets.insert(target_reg);
                node.allocated = true;
            } else {
                // 无法分配，需要溢出到栈
                return Err(format!(
                    "Failed to allocate register for source register {}",
                    node.reg
                ));
            }
        }

        Ok(())
    }

    /// 查找可用的目标寄存器
    fn find_available_register(&self, node: &InterferenceNode) -> Option<RegId> {
        // 获取相同类的可用寄存器
        let candidates: Vec<&RegisterInfo> = self
            .target_regs
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
        if self.next_temp < self.temp_pool.len() {
            let reg = self.temp_pool[self.next_temp];
            self.next_temp += 1;
            self.allocated_targets.insert(reg);
            Some(reg)
        } else {
            // 尝试从调用者保存寄存器中找未使用的
            for &reg in &self.calling_conv.caller_saved {
                if !self.allocated_targets.contains(&reg) {
                    self.allocated_targets.insert(reg);
                    return Some(reg);
                }
            }
            None
        }
    }

    /// 释放临时寄存器
    pub fn release_temp(&mut self, reg: RegId) {
        self.allocated_targets.remove(&reg);
    }

    /// 重置所有映射
    pub fn reset(&mut self) {
        self.current_mapping.clear();
        self.allocated_targets.clear();
        self.next_temp = 0;
    }

    /// 获取当前映射统计信息
    pub fn get_stats(&self) -> RegisterAllocationStats {
        RegisterAllocationStats {
            total_mappings: self.current_mapping.len(),
            allocated_targets: self.allocated_targets.len(),
            temp_allocated: self.next_temp,
            available_temps: self.temp_pool.len() - self.next_temp,
        }
    }
}

/// 寄存器分配统计信息
#[derive(Debug, Clone)]
pub struct RegisterAllocationStats {
    /// 总映射数
    pub total_mappings: usize,
    /// 已分配的目标寄存器数
    pub allocated_targets: usize,
    /// 已分配的临时寄存器数
    pub temp_allocated: usize,
    /// 可用的临时寄存器数
    pub available_temps: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smart_register_mapper() {
        let mapper = SmartRegisterMapper::new(Architecture::ARM64);
        let stats = mapper.get_stats();
        assert_eq!(stats.total_mappings, 0);
        assert_eq!(stats.allocated_targets, 0);
    }

    #[test]
    fn test_temp_allocation() {
        let mut mapper = SmartRegisterMapper::new(Architecture::ARM64);
        let temp1 = mapper.allocate_temp();
        assert!(temp1.is_some());

        let temp2 = mapper.allocate_temp();
        assert!(temp2.is_some());

        if let Some(temp) = temp1 {
            mapper.release_temp(temp);
        }
        let temp3 = mapper.allocate_temp();
        assert!(temp3.is_some());
    }

    #[test]
    fn test_interference_graph() {
        let mapper = SmartRegisterMapper::new(Architecture::ARM64);
        let live_ranges = vec![
            (0, (0, 10)),  // 寄存器0在指令0-10活跃
            (1, (5, 15)),  // 寄存器1在指令5-15活跃（与0冲突）
            (2, (20, 30)), // 寄存器2在指令20-30活跃（与0、1不冲突）
        ];

        let nodes = mapper.build_interference_graph(&live_ranges);
        assert_eq!(nodes.len(), 3);

        // 验证冲突关系
        let node0 = nodes
            .iter()
            .find(|n| n.reg == 0)
            .expect("Node 0 should exist");
        let node1 = nodes
            .iter()
            .find(|n| n.reg == 1)
            .expect("Node 1 should exist");
        let node2 = nodes
            .iter()
            .find(|n| n.reg == 2)
            .expect("Node 2 should exist");

        assert!(node0.conflicts.contains(&1));
        assert!(node1.conflicts.contains(&0));
        assert!(!node0.conflicts.contains(&2));
        assert!(!node2.conflicts.contains(&0));
        assert!(!node1.conflicts.contains(&2));
        assert!(!node2.conflicts.contains(&1));
    }
}
