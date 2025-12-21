//! 硬件加速执行引擎
//!
//! 使用 vm-accel 提供的硬件虚拟化能力执行代码。
//! 当前实现：最小化集成，支持初始化和内存映射，执行循环待完善。

use vm_accel::{Accel, AccelError, AccelKind, cpuinfo::CpuInfo, smmu::{SmmuVirtualizer, SmmuConfig, StreamId}};
use vm_core::{ExecResult, ExecStatus, ExecStats, ExecutionEngine, GuestAddr, HostAddr, MMU, VmError};
use vm_ir::IRBlock;
use std::sync::{Arc, Mutex};

/// 硬件加速执行引擎
///
/// 使用硬件虚拟化（KVM/HVF/WHPX）执行代码。
/// 当前实现状态：支持初始化和内存映射，执行循环待完善。
pub struct HardwareAccelEngine {
    /// 硬件加速器实例
    accel: Box<dyn Accel>,
    /// 加速器类型
    kind: AccelKind,
    /// 是否已初始化
    initialized: bool,
    /// vCPU ID（当前仅支持单 vCPU）
    vcpu_id: u32,
    /// SMMU 虚拟化实例（如果支持）
    smmu: Option<Arc<Mutex<SmmuVirtualizer>>>,
}

impl HardwareAccelEngine {
    /// 创建新的硬件加速执行引擎
    ///
    /// 自动选择最佳的硬件加速器后端。
    pub fn new() -> Result<Self, VmError> {
        let (kind, mut accel) = vm_accel::select();
        
        if kind == AccelKind::None {
            return Err(VmError::Core(vm_core::CoreError::NotSupported {
                feature: "Hardware acceleration".to_string(),
                module: "vm-cross-arch".to_string(),
            }));
        }
        
        // 初始化加速器
        accel.init().map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to initialize hardware accelerator: {:?}", e),
                module: "vm-cross-arch".to_string(),
            })
        })?;
        
        // 检测并初始化 SMMU（如果支持）
        let smmu = if CpuInfo::get().features.smmu {
            let config = SmmuConfig::default();
            let mut smmu_virt = SmmuVirtualizer::new(config);
            if smmu_virt.enable().is_ok() {
                tracing::info!("SMMU enabled for hardware acceleration");
                Some(Arc::new(Mutex::new(smmu_virt)))
            } else {
                tracing::warn!("Failed to enable SMMU, continuing without SMMU support");
                None
            }
        } else {
            None
        };
        
        Ok(Self {
            accel,
            kind,
            initialized: true,
            vcpu_id: 0,
            smmu,
        })
    }
    
    /// 创建 vCPU
    fn create_vcpu(&mut self) -> Result<(), VmError> {
        self.accel.create_vcpu(self.vcpu_id).map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to create vCPU: {:?}", e),
                module: "vm-cross-arch".to_string(),
            })
        })
    }
    
    /// 映射内存到硬件加速器
    fn map_memory(&mut self, mmu: &mut dyn MMU, guest_addr: GuestAddr, size: usize) -> Result<(), VmError> {
        // 获取物理地址
        let phys_addr = mmu.translate(guest_addr, vm_core::AccessType::Read)
            .map_err(|e| {
                VmError::Core(vm_core::CoreError::Internal {
                    message: format!("Failed to translate address: {:?}", e),
                    module: "vm-cross-arch".to_string(),
                })
            })?;
        
        // 从 MMU 读取内存数据
        let mut buffer = vec![0u8; size];
        mmu.read_bulk(phys_addr.into(), &mut buffer).map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to read memory: {:?}", e),
                module: "vm-cross-arch".to_string(),
            })
        })?;
        
        // 映射到硬件加速器
        // 注意：这里需要将 Rust 内存转换为 C 指针，实际实现需要更仔细的内存管理
        let host_ptr = buffer.as_ptr() as u64;
        let flags = 0x7; // Read | Write | Exec
        
        self.accel.map_memory(guest_addr.0, host_ptr, size as u64, flags).map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to map memory: {:?}", e),
                module: "vm-cross-arch".to_string(),
            })
        })?;
        
        // 防止 buffer 被释放（在实际实现中，应该使用更安全的内存管理）
        std::mem::forget(buffer);
        
        Ok(())
    }

    /// 获取 SMMU 虚拟化实例
    pub fn get_smmu(&self) -> Option<&Arc<Mutex<SmmuVirtualizer>>> {
        self.smmu.as_ref()
    }

    /// 配置设备的 SMMU StreamID
    pub fn configure_device_smmu(&self, device_id: u32, stream_id: StreamId, base_addr: GuestAddr) -> Result<(), VmError> {
        if let Some(smmu) = &self.smmu {
            let mut smmu_guard = smmu.lock().map_err(|e| {
                VmError::Core(vm_core::CoreError::Internal {
                    message: format!("Failed to lock SMMU: {:?}", e),
                    module: "vm-cross-arch".to_string(),
                })
            })?;
            
            smmu_guard.configure_stream(stream_id, base_addr)?;
            tracing::debug!("Configured SMMU for device {} with StreamID {}", device_id, stream_id.0);
            Ok(())
        } else {
            Err(VmError::Core(vm_core::CoreError::NotSupported {
                feature: "SMMU".to_string(),
                module: "vm-cross-arch".to_string(),
            }))
        }
    }

    /// 添加设备 DMA 地址转换
    pub fn add_device_translation(
        &self,
        stream_id: StreamId,
        device_addr: GuestAddr,
        host_addr: HostAddr,
        size: u64,
    ) -> Result<(), VmError> {
        if let Some(smmu) = &self.smmu {
            let mut smmu_guard = smmu.lock().map_err(|e| {
                VmError::Core(vm_core::CoreError::Internal {
                    message: format!("Failed to lock SMMU: {:?}", e),
                    module: "vm-cross-arch".to_string(),
                })
            })?;
            
            use vm_accel::smmu::TranslationFlags;
            let flags = TranslationFlags::default();
            smmu_guard.add_translation(stream_id, device_addr, host_addr, size, flags)?;
            tracing::debug!(
                "Added SMMU translation: StreamID {}: {:#x} -> {:#x} (size: {})",
                stream_id.0,
                device_addr.0,
                host_addr.0,
                size
            );
            Ok(())
        } else {
            Err(VmError::Core(vm_core::CoreError::NotSupported {
                feature: "SMMU".to_string(),
                module: "vm-cross-arch".to_string(),
            }))
        }
    }
}

impl Default for HardwareAccelEngine {
    fn default() -> Self {
        // 如果硬件加速不可用，这个会失败
        // 调用者应该检查错误并回退到其他引擎
        Self::new().unwrap_or_else(|_| {
            // 创建一个占位实现（不应该到达这里，因为 new() 会返回错误）
            panic!("HardwareAccelEngine::default() called but hardware acceleration is not available")
        })
    }
}

impl ExecutionEngine<IRBlock> for HardwareAccelEngine {
    fn run(&mut self, mmu: &mut dyn MMU, block: &IRBlock) -> ExecResult {
        // 当前实现：最小化集成
        // TODO: 实现完整的硬件加速执行循环，包括：
        // 1. 将 IR 块转换为可以直接执行的机器码（或使用 JIT 编译）
        // 2. 设置 vCPU 寄存器
        // 3. 运行 vCPU 并处理 VM exits
        // 4. 处理异常和中断
        
        if !self.initialized {
            return ExecResult {
                status: ExecStatus::Error(VmError::Core(vm_core::CoreError::InvalidState {
                    message: "Hardware accelerator not initialized".to_string(),
                    current: "not_initialized".to_string(),
                    expected: "initialized".to_string(),
                })),
                stats: ExecStats::default(),
                next_pc: block.start_pc,
            };
        }
        
        // 尝试创建 vCPU（如果尚未创建）
        if let Err(e) = self.create_vcpu() {
            tracing::warn!(
                "Failed to create vCPU for hardware acceleration: {:?}, falling back to interpreter",
                e
            );
            return ExecResult {
                status: ExecStatus::Error(e),
                stats: ExecStats::default(),
                next_pc: block.start_pc,
            };
        }
        
        // 尝试映射内存（如果尚未映射）
        // 注意：这里只映射代码块所在的内存页
        let page_size = 4096;
        let aligned_addr = GuestAddr(block.start_pc.0 & !(page_size - 1));
        if let Err(e) = self.map_memory(mmu, aligned_addr, page_size) {
            tracing::warn!(
                "Failed to map memory for hardware acceleration: {:?}, falling back to interpreter",
                e
            );
            return ExecResult {
                status: ExecStatus::Error(e),
                stats: ExecStats::default(),
                next_pc: block.start_pc,
            };
        }
        
        // TODO: 实现真正的硬件加速执行
        // 当前实现：返回错误，指示需要回退到解释器
        tracing::warn!(
            pc = block.start_pc.0,
            "Hardware acceleration execution loop not yet implemented, caller should fallback to interpreter"
        );
        
        ExecResult {
            status: ExecStatus::Error(VmError::Core(vm_core::CoreError::NotSupported {
                feature: "Hardware acceleration execution loop".to_string(),
                module: "vm-cross-arch".to_string(),
            })),
            stats: ExecStats::default(),
            next_pc: block.start_pc,
        }
    }
}

impl Drop for HardwareAccelEngine {
    fn drop(&mut self) {
        if self.initialized {
            // 清理资源
            // 注意：Accel trait 没有 destroy_vcpu 方法，资源由 Accel 实现自行管理
            // 这里可以添加其他清理逻辑（如取消内存映射）
        }
    }
}

