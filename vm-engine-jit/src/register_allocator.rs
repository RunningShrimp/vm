//! 寄存器分配器模块
//!
//! 实现统一的寄存器分配接口和多种分配策略：
//! - 线性扫描算法（快速，适用于小块）
//! - 图着色算法（更优，适用于大块）
//! - 自适应策略（根据块大小自动选择）

mod linear_scan_allocator;
mod graph_coloring_allocator;

pub use linear_scan_allocator::LinearScanAllocator;
pub use graph_coloring_allocator::GraphColoringAllocator;
pub use super::ir_utils;

use std::collections::{HashMap, HashSet, BTreeMap};
use vm_ir::{IROp, RegId};
use crate::ir_utils;

/// 寄存器分配结果
#[derive(Debug, Clone)]
pub enum RegisterAllocation {
    /// 分配到物理寄存器
    Register(RegId),
    /// 溢出到栈内存
    Stack(i32),
}

/// 寄存器分配器trait
///
/// 统一的寄存器分配接口，支持不同的分配策略
pub trait RegisterAllocatorTrait {
    /// 分析寄存器生命周期
    fn analyze_lifetimes(&mut self, ops: &[IROp]);
    
    /// 分配寄存器
    fn allocate_registers(&mut self, ops: &[IROp]) -> HashMap<RegId, RegisterAllocation>;
    
    /// 获取分配统计信息（可选）
    fn get_stats(&self) -> RegisterAllocatorStats {
        RegisterAllocatorStats::default()
    }
}

/// 寄存器分配器统计信息
#[derive(Debug, Clone, Default)]
pub struct RegisterAllocatorStats {
    /// 溢出次数
    pub spill_count: usize,
    /// 分配的寄存器数
    pub allocated_count: usize,
    /// 使用的算法
    pub algorithm_used: String,
}

/// 统一寄存器分配器（自适应策略）
///
/// 根据代码块大小自动选择最优算法：
/// - 小块（< threshold）：线性扫描（O(n)，快速）
/// - 大块（>= threshold）：图着色（O(n²)，更优）
pub struct RegisterAllocator {
    /// 寄存器使用情况
    used_regs: HashSet<RegId>,
    /// 寄存器生命周期
    reg_lifetimes: HashMap<RegId, (usize, usize)>, // (start, end)
    /// 寄存器溢出到内存的映射
    spilled_regs: HashMap<RegId, i32>, // offset from stack pointer
    /// 下一个可用的栈偏移
    next_spill_offset: i32,
    /// 小块阈值（指令数），小于此值使用线性扫描
    small_block_threshold: usize,
}

impl RegisterAllocatorTrait for RegisterAllocator {
    fn analyze_lifetimes(&mut self, ops: &[IROp]) {
        RegisterAllocator::analyze_lifetimes(self, ops);
    }

    fn allocate_registers(&mut self, ops: &[IROp]) -> HashMap<RegId, RegisterAllocation> {
        RegisterAllocator::allocate_registers(self, ops)
    }

    fn get_stats(&self) -> RegisterAllocatorStats {
        RegisterAllocatorStats {
            spill_count: self.spilled_regs.len(),
            allocated_count: self.reg_lifetimes.len(),
            algorithm_used: if self.small_block_threshold > 0 {
                "adaptive".to_string()
            } else {
                "graph_coloring".to_string()
            },
        }
    }
}

impl RegisterAllocator {
    pub fn new() -> Self {
        Self {
            used_regs: HashSet::new(),
            reg_lifetimes: HashMap::new(),
            spilled_regs: HashMap::new(),
            next_spill_offset: 0,
            small_block_threshold: 50, // 默认阈值：50条指令
        }
    }

    /// 设置小块阈值
    pub fn set_small_block_threshold(&mut self, threshold: usize) {
        self.small_block_threshold = threshold;
    }

    /// 分析寄存器生命周期
    pub fn analyze_lifetimes(&mut self, ops: &[IROp]) {
        for (idx, op) in ops.iter().enumerate() {
            // 收集读取的寄存器
            let read_regs = ir_utils::IrAnalyzer::collect_read_regs(op);
            // 收集写入的寄存器
            let written_regs = ir_utils::IrAnalyzer::collect_written_regs(op);

            // 更新寄存器生命周期
            for &reg in &read_regs {
                let lifetime = self.reg_lifetimes.entry(reg).or_insert((idx, idx));
                lifetime.1 = idx; // 延伸到当前指令
            }

            for &reg in &written_regs {
                self.reg_lifetimes.insert(reg, (idx, idx));
            }
        }
    }

    /// 分配寄存器（自适应策略）
    ///
    /// 根据代码块大小选择算法：
    /// - 小块（< threshold）：线性扫描（O(n)，快速）
    /// - 大块（>= threshold）：图着色（O(n²)，更优）
    pub fn allocate_registers(&mut self, ops: &[IROp]) -> HashMap<RegId, RegisterAllocation> {
        if ops.len() < self.small_block_threshold {
            // 小块：使用线性扫描
            let mut linear_allocator = LinearScanAllocator::new();
            linear_allocator.analyze_lifetimes(ops);
            linear_allocator.allocate_registers(ops)
        } else {
            // 大块：使用图着色
            let mut graph_allocator = GraphColoringAllocator::new();
            graph_allocator.analyze_lifetimes(ops);
            graph_allocator.allocate_registers(ops)
        }
    }

    /// 线性扫描寄存器分配（适用于小块）
    ///
    /// 算法步骤：
    /// 1. 按指令顺序扫描
    /// 2. 维护活跃寄存器集合
    /// 3. 当需要新寄存器时，如果寄存器已满，选择最早结束的寄存器溢出
    /// 4. 当寄存器生命周期结束时，释放寄存器
    fn allocate_registers_linear_scan(&mut self, ops: &[IROp]) -> HashMap<RegId, RegisterAllocation> {
        let mut allocations = HashMap::new();
        let mut active_regs: Vec<(RegId, usize)> = Vec::new(); // (reg, end_pos)
        let mut free_regs: Vec<u32> = (1..=31).collect(); // 可用寄存器池
        let mut reg_to_phys: HashMap<RegId, u32> = HashMap::new(); // 虚拟寄存器 -> 物理寄存器

        // 按位置扫描
        for pos in 0..ops.len() {
            // 1. 释放生命周期已结束的寄存器
            active_regs.retain(|(reg, end_pos)| {
                if *end_pos < pos {
                    // 生命周期结束，释放寄存器
                    if let Some(phys_reg) = reg_to_phys.remove(reg) {
                        free_regs.push(phys_reg);
                    }
                    false
                } else {
                    true
                }
            });

            // 2. 处理当前指令需要的寄存器
            let read_regs = ir_utils::IrAnalyzer::collect_read_regs(&ops[pos]);
            let written_regs = ir_utils::IrAnalyzer::collect_written_regs(&ops[pos]);

            // 2.1 确保读取的寄存器已分配
            for &reg in &read_regs {
                if !reg_to_phys.contains_key(&reg) {
                    // 需要分配寄存器
                    if let Some(phys_reg) = free_regs.pop() {
                        reg_to_phys.insert(reg, phys_reg);
                        if let Some(&(_, end)) = self.reg_lifetimes.get(&reg) {
                            active_regs.push((reg, end));
                            allocations.insert(reg, RegisterAllocation::Register(phys_reg));
                        }
                    } else {
                        // 寄存器已满，需要溢出
                        // 选择最早结束的寄存器溢出
                        if let Some((spill_reg, _)) = active_regs.iter().min_by_key(|(_, end)| *end) {
                            let spill_reg = *spill_reg;
                            let phys_reg = reg_to_phys.remove(&spill_reg).unwrap();
                            
                            // 溢出到栈
                            let offset = self.next_spill_offset;
                            self.next_spill_offset -= 8;
                            self.spilled_regs.insert(spill_reg, offset);
                            allocations.insert(spill_reg, RegisterAllocation::Stack(offset));
                            
                            // 重新分配
                            reg_to_phys.insert(reg, phys_reg);
                            if let Some(&(_, end)) = self.reg_lifetimes.get(&reg) {
                                active_regs.push((reg, end));
                                allocations.insert(reg, RegisterAllocation::Register(phys_reg));
                            }
                            
                            // 从active_regs中移除被溢出的寄存器
                            active_regs.retain(|(r, _)| *r != spill_reg);
                        } else {
                            // 无法分配，溢出
                            let offset = self.next_spill_offset;
                            self.next_spill_offset -= 8;
                            self.spilled_regs.insert(reg, offset);
                            allocations.insert(reg, RegisterAllocation::Stack(offset));
                        }
                    }
                }
            }

            // 2.2 确保写入的寄存器已分配
            for &reg in &written_regs {
                if !reg_to_phys.contains_key(&reg) {
                    // 需要分配寄存器
                    if let Some(phys_reg) = free_regs.pop() {
                        reg_to_phys.insert(reg, phys_reg);
                        if let Some(&(_, end)) = self.reg_lifetimes.get(&reg) {
                            active_regs.push((reg, end));
                            allocations.insert(reg, RegisterAllocation::Register(phys_reg));
                        }
                    } else {
                        // 寄存器已满，需要溢出
                        if let Some((spill_reg, _)) = active_regs.iter().min_by_key(|(_, end)| *end) {
                            let spill_reg = *spill_reg;
                            let phys_reg = reg_to_phys.remove(&spill_reg).unwrap();
                            
                            let offset = self.next_spill_offset;
                            self.next_spill_offset -= 8;
                            self.spilled_regs.insert(spill_reg, offset);
                            allocations.insert(spill_reg, RegisterAllocation::Stack(offset));
                            
                            reg_to_phys.insert(reg, phys_reg);
                            if let Some(&(_, end)) = self.reg_lifetimes.get(&reg) {
                                active_regs.push((reg, end));
                                allocations.insert(reg, RegisterAllocation::Register(phys_reg));
                            }
                            
                            active_regs.retain(|(r, _)| *r != spill_reg);
                        } else {
                            let offset = self.next_spill_offset;
                            self.next_spill_offset -= 8;
                            self.spilled_regs.insert(reg, offset);
                            allocations.insert(reg, RegisterAllocation::Stack(offset));
                        }
                    }
                }
            }
        }

        allocations
    }

    /// 图着色寄存器分配（适用于大块）
    ///
    /// 图着色算法相比线性扫描的优势：
    /// - 全局视角：考虑所有寄存器的冲突关系
    /// - 更好的分配：减少10-20%的寄存器溢出
    /// - 更优的寄存器重用
    fn allocate_registers_graph_coloring(&mut self, ops: &[IROp]) -> HashMap<RegId, RegisterAllocation> {
        // 1. 构建冲突图（interference graph）
        let interference_graph = self.build_interference_graph(ops);

        // 2. 图着色分配
        let mut allocations = HashMap::new();
        let mut colored = HashMap::new();
        let mut spilled = Vec::new();

        // 可用物理寄存器数量（x1-x31，共31个）
        let k = 31;

        // 3. 简化图（simplify phase）
        let mut worklist = self.simplify_graph(&interference_graph, k);

        // 4. 选择阶段（select phase）- 反向分配颜色
        while let Some(reg) = worklist.pop() {
            let mut used_colors = HashSet::new();

            // 收集已分配给冲突寄存器的颜色
            if let Some(neighbors) = interference_graph.get(&reg) {
                for &neighbor in neighbors {
                    if let Some(color) = colored.get(&neighbor) {
                        used_colors.insert(*color);
                    }
                }
            }

            // 分配第一个可用的颜色
            if used_colors.len() < k {
                for color in 1..=k {
                    if !used_colors.contains(&(color as u32)) {
                        colored.insert(reg, color as u32);
                        allocations.insert(
                            reg,
                            RegisterAllocation::Register(color as u32),
                        );
                        break;
                    }
                }
            } else {
                // 无法分配寄存器，需要溢出
                spilled.push(reg);
            }
        }

        // 5. 处理溢出的寄存器
        for reg in spilled {
            let offset = self.next_spill_offset;
            self.next_spill_offset -= 8; // 每个寄存器8字节
            self.spilled_regs.insert(reg, offset);
            allocations.insert(reg, RegisterAllocation::Stack(offset));
        }

        allocations
    }

    /// 构建冲突图（interference graph）
    fn build_interference_graph(&self, ops: &[IROp]) -> HashMap<RegId, HashSet<RegId>> {
        let mut graph: HashMap<RegId, HashSet<RegId>> = HashMap::new();

        // 对于每个寄存器，找到所有与其冲突的寄存器
        for (reg, &(start, end)) in &self.reg_lifetimes {
            let mut conflicts = HashSet::new();

            // 检查所有与当前寄存器生命周期重叠的寄存器
            for (other_reg, &(other_start, other_end)) in &self.reg_lifetimes {
                if reg != other_reg {
                    // 如果生命周期重叠，则存在冲突
                    if !(end < other_start || other_end < start) {
                        conflicts.insert(*other_reg);
                    }
                }
            }

            graph.insert(*reg, conflicts);
        }

        graph
    }

    /// 简化图（simplify phase）
    /// 移除度数小于k的节点，直到图为空或只剩下高度数节点
    fn simplify_graph(
        &self,
        graph: &HashMap<RegId, HashSet<RegId>>,
        k: usize,
    ) -> Vec<RegId> {
        let mut worklist = Vec::new();
        let mut remaining = graph.clone();

        loop {
            let mut found = false;

            // 找到度数小于k的节点
            let candidates: Vec<RegId> = remaining
                .iter()
                .filter(|(_, neighbors)| neighbors.len() < k)
                .map(|(reg, _)| *reg)
                .collect();

            if candidates.is_empty() {
                break;
            }

            // 移除第一个候选节点
            if let Some(&reg) = candidates.first() {
                worklist.push(reg);
                remaining.remove(&reg);

                // 从其他节点的邻居中移除该节点
                for neighbors in remaining.values_mut() {
                    neighbors.remove(&reg);
                }

                found = true;
            }

            if !found {
                break;
            }
        }

        // 如果还有剩余节点，按度数排序后加入worklist
        let mut remaining_nodes: Vec<_> = remaining.keys().copied().collect();
        remaining_nodes.sort_by_key(|&reg| {
            remaining.get(&reg).map(|n| n.len()).unwrap_or(0)
        });
        worklist.extend(remaining_nodes);

        worklist
    }

}

impl Default for RegisterAllocator {
    fn default() -> Self {
        Self::new()
    }
}

