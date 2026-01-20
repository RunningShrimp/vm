//! iOS/iPadOS Virtualization.framework 加速后端实现
//!
//! 支持 Apple Silicon (M系列) 芯片
//! 注意: Virtualization.framework 在 iOS/iPadOS 上需要特殊权限

use std::collections::HashMap;

use vm_core::{GuestRegs, MMU};

use super::{Accel, AccelError};

// Virtualization.framework FFI 绑定 (iOS/iPadOS)
// 注意: 这些 API 在 iOS 15+ 和 iPadOS 15+ 上可用，但需要特殊的 entitlements

#[cfg(any(target_os = "ios", target_os = "tvos"))]
#[link(name = "Virtualization", kind = "framework")]
extern "C" {
    // VM 配置
    fn VZVirtualMachineConfiguration_new() -> *mut std::ffi::c_void;
    fn VZVirtualMachineConfiguration_setMemorySize(config: *mut std::ffi::c_void, size: u64);
    fn VZVirtualMachineConfiguration_setCPUCount(config: *mut std::ffi::c_void, count: u32);

    // VM 创建和运行
    fn VZVirtualMachine_new(config: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    fn VZVirtualMachine_start(vm: *mut std::ffi::c_void) -> bool;
    fn VZVirtualMachine_stop(vm: *mut std::ffi::c_void);
    fn VZVirtualMachine_pause(vm: *mut std::ffi::c_void) -> bool;
    fn VZVirtualMachine_resume(vm: *mut std::ffi::c_void) -> bool;
}

/// VZ vCPU (Virtualization.framework 不直接暴露 vCPU 接口)
pub struct VzVcpu {
    id: u32,
}

impl VzVcpu {
    pub fn new(id: u32) -> Result<Self, AccelError> {
        Ok(Self { id })
    }

    pub fn get_id(&self) -> u32 {
        self.id
    }

    /// 获取寄存器 (Virtualization.framework 不直接支持)
    pub fn get_regs(&self) -> Result<GuestRegs, AccelError> {
        // Virtualization.framework 是高层 API，不直接暴露寄存器访问
        // 需要通过 GDB stub 或其他机制
        Err(AccelError::NotSupported(
            "Direct register access not supported in Virtualization.framework".to_string(),
        ))
    }

    /// 设置寄存器 (Virtualization.framework 不直接支持)
    pub fn set_regs(&mut self, _regs: &GuestRegs) -> Result<(), AccelError> {
        Err(AccelError::NotSupported(
            "Direct register access not supported in Virtualization.framework".to_string(),
        ))
    }

    /// 运行 vCPU (Virtualization.framework 运行整个 VM)
    pub fn run(&mut self) -> Result<(), AccelError> {
        // Virtualization.framework 在 AccelVz 中运行整个 VM
        // 单个 vCPU 的 run 是空操作
        Ok(())
    }
}

// Implement VcpuOps trait for unified vCPU interface
impl crate::vcpu_common::VcpuOps for VzVcpu {
    fn get_id(&self) -> u32 {
        self.get_id()
    }

    fn run(&mut self) -> crate::vcpu_common::VcpuResult<crate::vcpu_common::VcpuExit> {
        self.run().map(|_| crate::vcpu_common::VcpuExit::Unknown)
    }

    fn get_regs(&self) -> crate::vcpu_common::VcpuResult<vm_core::GuestRegs> {
        self.get_regs()
    }

    fn set_regs(&mut self, regs: &vm_core::GuestRegs) -> crate::vcpu_common::VcpuResult<()> {
        self.set_regs(regs)
    }

    fn get_fpu_regs(&self) -> crate::vcpu_common::VcpuResult<crate::vcpu_common::FpuRegs> {
        // Virtualization.framework doesn't expose direct FPU register access
        Err(vm_core::VmError::Core(vm_core::CoreError::NotSupported {
            feature: "VZ FPU register access".to_string(),
            module: "vm-accel::vz".to_string(),
        }))
    }

    fn set_fpu_regs(
        &mut self,
        _regs: &crate::vcpu_common::FpuRegs,
    ) -> crate::vcpu_common::VcpuResult<()> {
        // Virtualization.framework doesn't expose direct FPU register access
        Err(vm_core::VmError::Core(vm_core::CoreError::NotSupported {
            feature: "VZ FPU register access".to_string(),
            module: "vm-accel::vz".to_string(),
        }))
    }
}

/// Virtualization.framework 加速器
pub struct AccelVz {
    #[cfg(any(target_os = "ios", target_os = "tvos"))]
    vm: Option<*mut std::ffi::c_void>,
    #[cfg(any(target_os = "ios", target_os = "tvos"))]
    config: Option<*mut std::ffi::c_void>,

    vcpus: Vec<VzVcpu>,
    memory_regions: HashMap<u64, u64>, // gpa -> size
    memory_size: u64,
    cpu_count: u32,
    initialized: bool,
}

impl AccelVz {
    pub fn new() -> Self {
        Self {
            #[cfg(any(target_os = "ios", target_os = "tvos"))]
            vm: None,
            #[cfg(any(target_os = "ios", target_os = "tvos"))]
            config: None,
            vcpus: Vec::new(),
            memory_regions: HashMap::new(),
            memory_size: 2 * 1024 * 1024 * 1024, // 默认 2GB
            cpu_count: 2,                        // 默认 2 核
            initialized: false,
        }
    }

    /// 检查 Virtualization.framework 是否可用
    #[cfg(any(target_os = "ios", target_os = "tvos"))]
    pub fn is_available() -> bool {
        // iOS 15+ 和 iPadOS 15+ 支持 Virtualization.framework
        // 但需要特殊的 entitlements: com.apple.developer.kernel.extended-virtual-addressing
        // 实际可用性需要运行时检查
        true
    }

    #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
    pub fn is_available() -> bool {
        false
    }

    /// 设置内存大小
    pub fn set_memory_size(&mut self, size: u64) {
        self.memory_size = size;
    }

    /// 设置 CPU 数量
    pub fn set_cpu_count(&mut self, count: u32) {
        self.cpu_count = count;
    }
}

impl Accel for AccelVz {
    fn init(&mut self) -> Result<(), AccelError> {
        #[cfg(any(target_os = "ios", target_os = "tvos"))]
        {
            if self.initialized {
                return Ok(());
            }

            if !Self::is_available() {
                return Err(AccelError::NotAvailable(
                    "Virtualization.framework not available".to_string(),
                ));
            }

            unsafe {
                // 创建 VM 配置
                let config = VZVirtualMachineConfiguration_new();
                if config.is_null() {
                    return Err(AccelError::InitFailed(
                        "Failed to create VM configuration".to_string(),
                    ));
                }

                // 设置内存大小
                VZVirtualMachineConfiguration_setMemorySize(config, self.memory_size);

                // 设置 CPU 数量
                VZVirtualMachineConfiguration_setCPUCount(config, self.cpu_count);

                // 创建 VM
                let vm = VZVirtualMachine_new(config);
                if vm.is_null() {
                    return Err(AccelError::InitFailed("Failed to create VM".to_string()));
                }

                self.config = Some(config);
                self.vm = Some(vm);
                self.initialized = true;

                log::info!("Virtualization.framework accelerator initialized successfully");
                log::info!(
                    "Memory: {} MB, CPUs: {}",
                    self.memory_size / (1024 * 1024),
                    self.cpu_count
                );
                Ok(())
            }
        }

        #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
        {
            Err(AccelError::NotSupported(
                "Virtualization.framework only available on iOS/iPadOS".to_string(),
            ))
        }
    }

    fn create_vcpu(&mut self, id: u32) -> Result<(), AccelError> {
        // Virtualization.framework 在配置时设置 CPU 数量，不需要单独创建 vCPU
        if id >= self.cpu_count {
            return Err(AccelError::CreateVcpuFailed(format!(
                "vCPU {} exceeds configured count {}",
                id, self.cpu_count
            )));
        }

        let vcpu = VzVcpu::new(id)?;
        self.vcpus.push(vcpu);

        log::info!("Created VZ vCPU {}", id);
        Ok(())
    }

    fn map_memory(
        &mut self,
        gpa: u64,
        _hva: u64,
        size: u64,
        _flags: u32,
    ) -> Result<(), AccelError> {
        // Virtualization.framework 使用高层内存管理
        // 内存映射在配置阶段完成，运行时不支持动态映射
        self.memory_regions.insert(gpa, size);
        log::debug!(
            "Registered memory region: GPA 0x{:x}, size 0x{:x}",
            gpa,
            size
        );
        Ok(())
    }

    fn unmap_memory(&mut self, gpa: u64, _size: u64) -> Result<(), AccelError> {
        self.memory_regions.remove(&gpa);
        log::debug!("Unregistered memory region: GPA 0x{:x}", gpa);
        Ok(())
    }

    fn run_vcpu(&mut self, vcpu_id: u32, _mmu: &mut dyn MMU) -> Result<(), AccelError> {
        #[cfg(any(target_os = "ios", target_os = "tvos"))]
        {
            let vm = self
                .vm
                .as_ref()
                .ok_or_else(|| AccelError::NotInitialized("VM not initialized".to_string()))?;

            // Virtualization.framework 运行整个 VM，而不是单个 vCPU
            if vcpu_id == 0 {
                unsafe {
                    if !VZVirtualMachine_start(*vm) {
                        return Err(AccelError::RunFailed("Failed to start VM".to_string()));
                    }
                }
                log::info!("VM started");
            }

            Ok(())
        }

        #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
        {
            Err(AccelError::NotSupported(
                "Virtualization.framework not available on this platform".to_string(),
            ))
        }
    }

    fn get_regs(&self, vcpu_id: u32) -> Result<GuestRegs, AccelError> {
        let vcpu = self
            .vcpus
            .get(vcpu_id as usize)
            .ok_or_else(|| AccelError::InvalidVcpuId(vcpu_id))?;
        vcpu.get_regs()
    }

    fn set_regs(&mut self, vcpu_id: u32, regs: &GuestRegs) -> Result<(), AccelError> {
        let vcpu = self
            .vcpus
            .get_mut(vcpu_id as usize)
            .ok_or_else(|| AccelError::InvalidVcpuId(vcpu_id))?;
        vcpu.set_regs(regs)
    }

    fn name(&self) -> &str {
        "Virtualization.framework"
    }
}

impl AccelVz {
    /// Create vCPU and return VcpuOps trait object
    ///
    /// This creates a new vCPU and returns it as a trait object.
    pub fn create_vcpu_ops(
        &mut self,
        id: u32,
    ) -> Result<Box<dyn crate::vcpu_common::VcpuOps>, AccelError> {
        if id >= self.cpu_count {
            return Err(AccelError::CreateVcpuFailed(format!(
                "vCPU {} exceeds configured count {}",
                id, self.cpu_count
            )));
        }

        let vcpu = VzVcpu::new(id)?;
        // Don't add to Vec - return directly
        log::info!("Created VZ vCPU {}", id);
        Ok(Box::new(vcpu))
    }
}

impl Drop for AccelVz {
    fn drop(&mut self) {
        #[cfg(any(target_os = "ios", target_os = "tvos"))]
        if let Some(vm) = self.vm {
            unsafe {
                VZVirtualMachine_stop(vm);
            }
            log::info!("VM stopped");
        }
    }
}

impl Default for AccelVz {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vz_availability() {
        println!(
            "Virtualization.framework available: {}",
            AccelVz::is_available()
        );
    }

    #[test]
    #[cfg(any(target_os = "ios", target_os = "tvos"))]
    fn test_vz_init() {
        let mut accel = AccelVz::new();
        // 注意: 实际测试需要正确的 entitlements
        if AccelVz::is_available() {
            let result = accel.init();
            println!("VZ init result: {:?}", result);
        }
    }
}
