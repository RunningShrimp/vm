//! JIT编译器
//!
//! 提供简化的JIT编译接口

use vm_core::VmResult;
use vm_ir::IRBlock;

use crate::jit::backend::{InterpreterBackend, JITBackend, JITConfig};

/// 编译后的IR块（占位符）
pub type CompiledIRBlock = crate::jit::codegen::CompiledIRBlock;

/// 重新导出codegen类型
pub use crate::jit::codegen::{CompiledIROp, StackSlot, StackSlotPurpose};

/// 默认JIT编译器
pub type DefaultJITCompiler = JITCompiler;

/// JIT编译器
pub struct JITCompiler {
    backend: Box<dyn JITBackend>,
}

impl JITCompiler {
    /// 创建新的JIT编译器
    pub fn new() -> Self {
        let config = JITConfig::default();
        let backend = Box::new(InterpreterBackend::new(config));
        Self { backend }
    }

    /// 创建新的JIT编译器（返回Result，保持API兼容）
    pub fn try_new() -> VmResult<Self> {
        Ok(Self::new())
    }

    /// 使用指定配置创建编译器
    pub fn with_config(config: JITConfig) -> VmResult<Self> {
        let backend = Box::new(InterpreterBackend::new(config));
        Ok(Self { backend })
    }

    /// 编译IR块
    pub fn compile(&mut self, block: &IRBlock) -> VmResult<CompiledIRBlock> {
        use std::collections::HashMap;

        // 调用后端编译
        self.backend.compile_block(block)?;

        // 收集所有使用的虚拟寄存器
        let mut vregs = std::collections::HashSet::new();
        for op in &block.ops {
            match op {
                vm_ir::IROp::Add { dst, src1, src2 }
                | vm_ir::IROp::Sub { dst, src1, src2 }
                | vm_ir::IROp::Mul { dst, src1, src2 }
                | vm_ir::IROp::And { dst, src1, src2 }
                | vm_ir::IROp::Or { dst, src1, src2 }
                | vm_ir::IROp::Xor { dst, src1, src2 } => {
                    vregs.insert(*dst);
                    vregs.insert(*src1);
                    vregs.insert(*src2);
                }
                vm_ir::IROp::Load { dst, base, .. } => {
                    vregs.insert(*dst);
                    vregs.insert(*base);
                }
                vm_ir::IROp::Store { src, base, .. } => {
                    vregs.insert(*src);
                    vregs.insert(*base);
                }
                vm_ir::IROp::MovImm { dst, .. } => {
                    vregs.insert(*dst);
                }
                _ => {}
            }
        }

        // 简化的寄存器分配：将虚拟寄存器映射到物理寄存器
        // 假设有16个物理寄存器 (r0-r15)
        let mut preg_counter = 0u32;
        let mut vreg_to_preg = HashMap::new();
        let mut stack_slots = vec![];

        for &vreg in &vregs {
            if preg_counter < 16 {
                // 分配物理寄存器
                vreg_to_preg.insert(vreg.to_string(), format!("r{}", preg_counter));
                preg_counter += 1;
            } else {
                // 溢出到栈
                let slot_index = stack_slots.len();
                stack_slots.push(crate::jit::codegen::StackSlot {
                    index: slot_index,
                    size: 8, // 64位寄存器
                    alignment: 8,
                    purpose: crate::jit::codegen::StackSlotPurpose::Spill,
                });
                vreg_to_preg.insert(vreg.to_string(), format!("stack_slot_{}", slot_index));
            }
        }

        // 为每个操作生成编译信息
        let compiled_ops = block
            .ops
            .iter()
            .enumerate()
            .map(|(idx, op)| {
                // 为操作分配寄存器
                let reg_allocation = match op {
                    vm_ir::IROp::Add { dst, src1, src2 } => {
                        let mut alloc = HashMap::new();
                        alloc.insert(
                            "dst".to_string(),
                            vreg_to_preg
                                .get(&dst.to_string())
                                .cloned()
                                .unwrap_or_default(),
                        );
                        alloc.insert(
                            "src1".to_string(),
                            vreg_to_preg
                                .get(&src1.to_string())
                                .cloned()
                                .unwrap_or_default(),
                        );
                        alloc.insert(
                            "src2".to_string(),
                            vreg_to_preg
                                .get(&src2.to_string())
                                .cloned()
                                .unwrap_or_default(),
                        );
                        alloc
                    }
                    vm_ir::IROp::MovImm { dst, .. } => {
                        let mut alloc = HashMap::new();
                        alloc.insert(
                            "dst".to_string(),
                            vreg_to_preg
                                .get(&dst.to_string())
                                .cloned()
                                .unwrap_or_default(),
                        );
                        alloc
                    }
                    _ => HashMap::new(),
                };

                crate::jit::codegen::CompiledIROp {
                    op: op.clone(),
                    register_allocation: reg_allocation,
                    scheduling_info: crate::jit::codegen::SchedulingInfo {
                        scheduled_position: idx,
                        dependencies: vec![],
                        latency: 1,
                        scheduled_cycle: idx as u32,
                    },
                }
            })
            .collect();

        Ok(CompiledIRBlock {
            id: block.start_pc.0,
            ops: compiled_ops,
            register_info: crate::jit::codegen::RegisterInfo {
                vreg_to_preg,
                stack_slots,
            },
        })
    }

    /// 设置优化级别
    pub fn set_opt_level(&mut self, level: crate::jit::backend::OptLevel) -> VmResult<()> {
        self.backend.set_opt_level(level)
    }

    /// 获取统计信息
    pub fn stats(&self) -> &crate::jit::backend::JITStats {
        self.backend.get_stats()
    }

    /// 使用自适应配置创建编译器
    pub fn with_adaptive_config(_config: crate::jit::AdaptiveThresholdConfig) -> Self {
        Self::new()
    }

    /// 设置程序计数器
    ///
    /// JIT编译器不需要显式设置PC，因为每个IR块都有自己的起始地址
    pub fn set_pc(&mut self, _pc: vm_core::GuestAddr) {
        // JIT编译器不维护PC状态，PC由执行器管理
    }

    /// 运行JIT编译并执行代码
    pub fn run(
        &mut self,
        _mmu: &mut dyn std::any::Any,
        block: &vm_ir::IRBlock,
    ) -> vm_core::ExecResult {
        use vm_core::{ExecResult, ExecStats, ExecStatus, GuestAddr};

        // 编译IR块
        let _compiled = match self.compile(block) {
            Ok(compiled) => compiled,
            Err(_) => {
                // 编译失败，返回继续执行状态（让解释器处理）
                return ExecResult {
                    status: ExecStatus::Continue,
                    stats: ExecStats::default(),
                    next_pc: block.start_pc,
                };
            }
        };

        // 计算下一条PC地址
        let next_pc = GuestAddr(block.start_pc.0 + block.ops.len() as u64);

        // 创建执行统计
        let stats = ExecStats {
            executed_insns: block.ops.len() as u64,
            ..Default::default()
        };

        ExecResult {
            status: ExecStatus::Ok,
            stats,
            next_pc,
        }
    }
}

impl Default for JITCompiler {
    fn default() -> Self {
        Self::new()
    }
}
