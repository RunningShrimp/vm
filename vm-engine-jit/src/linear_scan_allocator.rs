//! 线性扫描寄存器分配器
//!
//! 实现线性扫描算法，适用于小块代码（指令数 < 50）

use std::collections::HashMap;
use vm_ir::{IROp, RegId};
use crate::ir_utils;
use super::register_allocator::{RegisterAllocatorTrait, RegisterAllocation, RegisterAllocatorStats};

/// 线性扫描寄存器分配器
pub struct LinearScanAllocator {
    /// 寄存器生命周期
    reg_lifetimes: HashMap<RegId, (usize, usize)>, // (start, end)
    /// 寄存器溢出到内存的映射
    spilled_regs: HashMap<RegId, i32>,
    /// 下一个可用的栈偏移
    next_spill_offset: i32,
}

impl LinearScanAllocator {
    pub fn new() -> Self {
        Self {
            reg_lifetimes: HashMap::new(),
            spilled_regs: HashMap::new(),
            next_spill_offset: 0,
        }
    }
}

impl RegisterAllocatorTrait for LinearScanAllocator {
    fn analyze_lifetimes(&mut self, ops: &[IROp]) {
        for (idx, op) in ops.iter().enumerate() {
            let read_regs = ir_utils::IrAnalyzer::collect_read_regs(op);
            let written_regs = ir_utils::IrAnalyzer::collect_written_regs(op);

            for &reg in &read_regs {
                let lifetime = self.reg_lifetimes.entry(reg).or_insert((idx, idx));
                lifetime.1 = idx;
            }

            for &reg in &written_regs {
                self.reg_lifetimes.insert(reg, (idx, idx));
            }
        }
    }

    fn allocate_registers(&mut self, ops: &[IROp]) -> HashMap<RegId, RegisterAllocation> {
        let mut allocations = HashMap::new();
        let mut active_regs: Vec<(RegId, usize)> = Vec::new();
        let mut free_regs: Vec<u32> = (1..=31).collect();
        let mut reg_to_phys: HashMap<RegId, u32> = HashMap::new();

        for pos in 0..ops.len() {
            // 释放生命周期已结束的寄存器
            active_regs.retain(|(reg, end_pos)| {
                if *end_pos < pos {
                    if let Some(phys_reg) = reg_to_phys.remove(reg) {
                        free_regs.push(phys_reg);
                    }
                    false
                } else {
                    true
                }
            });

            let read_regs = ir_utils::IrAnalyzer::collect_read_regs(&ops[pos]);
            let written_regs = ir_utils::IrAnalyzer::collect_written_regs(&ops[pos]);

            // 确保读取的寄存器已分配
            for &reg in &read_regs {
                if !reg_to_phys.contains_key(&reg) {
                    if let Some(phys_reg) = free_regs.pop() {
                        reg_to_phys.insert(reg, phys_reg);
                        if let Some(&(_, end)) = self.reg_lifetimes.get(&reg) {
                            active_regs.push((reg, end));
                            allocations.insert(reg, RegisterAllocation::Register(phys_reg));
                        }
                    } else {
                        // 寄存器已满，溢出最早结束的寄存器
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

            // 确保写入的寄存器已分配
            for &reg in &written_regs {
                if !reg_to_phys.contains_key(&reg) {
                    if let Some(phys_reg) = free_regs.pop() {
                        reg_to_phys.insert(reg, phys_reg);
                        if let Some(&(_, end)) = self.reg_lifetimes.get(&reg) {
                            active_regs.push((reg, end));
                            allocations.insert(reg, RegisterAllocation::Register(phys_reg));
                        }
                    } else {
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

    fn get_stats(&self) -> RegisterAllocatorStats {
        RegisterAllocatorStats {
            spill_count: self.spilled_regs.len(),
            allocated_count: self.reg_lifetimes.len(),
            algorithm_used: "linear_scan".to_string(),
        }
    }
}

impl Default for LinearScanAllocator {
    fn default() -> Self {
        Self::new()
    }
}

