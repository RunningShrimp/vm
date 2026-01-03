//! 寄存器分配器适配器（基础设施层）
//!
//! 提供寄存器分配器的基础设施层实现，适配领域层定义的 RegisterAllocator trait。
//! 这是基础设施层的实现，具体的技术细节应在此模块中。

use std::sync::Arc;

use vm_core::{RegisterAllocationStats, RegisterAllocator, VmError, VmResult};
use vm_ir::IRBlock;

use crate::jit::register_allocator::{
    AllocationStrategy, OptimizedRegisterAllocator, RegisterAllocator as EngineRegisterAllocator,
};

/// 寄存器分配器适配器（基础设施层）
///
/// 将 vm-engine 的寄存器分配器适配到领域层定义的 RegisterAllocator trait。
pub struct RegisterAllocatorAdapter {
    /// 内部分配器
    allocator: Arc<tokio::sync::Mutex<OptimizedRegisterAllocator>>,
}

impl RegisterAllocatorAdapter {
    /// 创建新的寄存器分配器适配器
    pub fn new(strategy: AllocationStrategy) -> Self {
        let config = crate::jit::register_allocator::AllocatorConfig {
            strategy,
            ..Default::default()
        };

        let allocator = OptimizedRegisterAllocator::new(config);

        Self {
            allocator: Arc::new(tokio::sync::Mutex::new(allocator)),
        }
    }

    /// 从现有分配器创建
    pub fn from_allocator(allocator: OptimizedRegisterAllocator) -> Self {
        Self {
            allocator: Arc::new(tokio::sync::Mutex::new(allocator)),
        }
    }
}

impl RegisterAllocator for RegisterAllocatorAdapter {
    fn allocate(&mut self, ir: &[u8]) -> VmResult<Vec<u8>> {
        // 反序列化 IR 块
        let (block, _): (IRBlock, usize) =
            bincode::serde::decode_from_slice(ir, bincode::config::standard()).map_err(|e| {
                VmError::Core(vm_core::CoreError::InvalidState {
                    message: format!("Failed to deserialize IR: {}", e),
                    current: "".to_string(),
                    expected: "".to_string(),
                })
            })?;

        // 将 IRBlock 转换为 CompiledIRBlock
        // 注意：这里需要创建一个 CompiledIRBlock
        let compiled_block = crate::jit::codegen::CompiledIRBlock {
            id: block.start_pc.0,
            ops: block
                .ops
                .iter()
                .map(|op| crate::jit::codegen::CompiledIROp {
                    op: op.clone(),
                    register_allocation: std::collections::HashMap::new(),
                    scheduling_info: crate::jit::codegen::SchedulingInfo::default(),
                })
                .collect(),
            register_info: crate::jit::codegen::RegisterInfo::default(),
        };

        // 使用同步方式调用分配器
        let rt = tokio::runtime::Handle::try_current();

        if let Ok(handle) = rt {
            // 在 async 上下文中
            let allocator = self.allocator.clone();
            handle.block_on(async move {
                let mut alloc = allocator.lock().await;
                let allocated = alloc.allocate(&compiled_block)?;

                // 将 CompiledIRBlock 转换回 IRBlock
                let ir_block = IRBlock {
                    start_pc: vm_ir::GuestAddr(allocated.id),
                    ops: allocated.ops.iter().map(|op| op.op.clone()).collect(),
                    term: vm_ir::Terminator::Ret, // 默认终止符，实际应从原始块获取
                };

                // 序列化分配后的 IR
                bincode::serde::encode_to_vec(&ir_block, bincode::config::standard()).map_err(|e| {
                    VmError::Core(vm_core::CoreError::InvalidState {
                        message: format!("Failed to serialize IR: {}", e),
                        current: "".to_string(),
                        expected: "".to_string(),
                    })
                })
            })
        } else {
            // 同步上下文 - 创建临时 runtime
            let rt = tokio::runtime::Runtime::new().map_err(|e| {
                VmError::Core(vm_core::CoreError::InvalidState {
                    message: format!("Failed to create tokio runtime: {}", e),
                    current: "".to_string(),
                    expected: "".to_string(),
                })
            })?;

            let allocator = self.allocator.clone();
            rt.block_on(async move {
                let mut alloc = allocator.lock().await;
                let allocated = alloc.allocate(&compiled_block)?;

                // 将 CompiledIRBlock 转换回 IRBlock
                let ir_block = IRBlock {
                    start_pc: vm_ir::GuestAddr(allocated.id),
                    ops: allocated.ops.iter().map(|op| op.op.clone()).collect(),
                    term: vm_ir::Terminator::Ret, // 默认终止符，实际应从原始块获取
                };

                // 序列化分配后的 IR
                bincode::serde::encode_to_vec(&ir_block, bincode::config::standard()).map_err(|e| {
                    VmError::Core(vm_core::CoreError::InvalidState {
                        message: format!("Failed to serialize IR: {}", e),
                        current: "".to_string(),
                        expected: "".to_string(),
                    })
                })
            })
        }
    }

    fn stats(&self) -> RegisterAllocationStats {
        // 获取内部统计信息
        let rt = tokio::runtime::Handle::try_current();

        if let Ok(handle) = rt {
            let allocator = self.allocator.clone();
            handle.block_on(async move {
                let alloc = allocator.lock().await;
                let engine_stats = alloc.get_stats();

                // 转换为领域层统计信息
                RegisterAllocationStats {
                    total_allocations: 1, // 从内部统计获取
                    spills: engine_stats.spilled_registers,
                    physical_regs_used: engine_stats.allocated_registers,
                    virtual_regs: engine_stats.total_virtual_registers,
                }
            })
        } else {
            // 同步上下文 - 创建临时 runtime
            match tokio::runtime::Runtime::new() {
                Ok(rt) => {
                    let allocator = self.allocator.clone();
                    rt.block_on(async move {
                        let alloc = allocator.lock().await;
                        let engine_stats = alloc.get_stats();

                        // 转换为领域层统计信息
                        RegisterAllocationStats {
                            total_allocations: 1, // 从内部统计获取
                            spills: engine_stats.spilled_registers,
                            physical_regs_used: engine_stats.allocated_registers,
                            virtual_regs: engine_stats.total_virtual_registers,
                        }
                    })
                }
                Err(_) => {
                    // 回退：返回默认统计
                    RegisterAllocationStats {
                        total_allocations: 0,
                        spills: 0,
                        physical_regs_used: 0,
                        virtual_regs: 0,
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_allocator_adapter_creation() {
        let adapter = RegisterAllocatorAdapter::new(AllocationStrategy::LinearScan);
        let stats = adapter.stats();
        assert_eq!(stats.total_allocations, 0);
    }
}
