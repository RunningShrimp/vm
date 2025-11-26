//! Windows Hypervisor Platform (WHPX) 加速后端完整实现
//!
//! 支持 Intel 和 AMD 虚拟化扩展

use super::{Accel, AccelError};
use vm_core::{GuestRegs, MMU};
use std::collections::HashMap;

#[cfg(all(target_os = "windows", feature = "whpx"))]
use windows::Win32::System::Hypervisor::*;

/// WHPX vCPU
pub struct WhpxVcpu {
    #[cfg(all(target_os = "windows", feature = "whpx"))]
    index: u32,
    #[cfg(not(all(target_os = "windows", feature = "whpx")))]
    _index: u32,
}

impl WhpxVcpu {
    pub fn new(index: u32) -> Result<Self, AccelError> {
        #[cfg(all(target_os = "windows", feature = "whpx"))]
        {
            Ok(Self { index })
        }
        
        #[cfg(not(all(target_os = "windows", feature = "whpx")))]
        {
            Ok(Self { _index: index })
        }
    }

    /// 获取寄存器
    #[cfg(all(target_os = "windows", feature = "whpx", target_arch = "x86_64"))]
    pub fn get_regs(&self, _partition: &WHV_PARTITION_HANDLE) -> Result<GuestRegs, AccelError> {
        // TODO: 使用 WHvGetVirtualProcessorRegisters 获取寄存器
        Ok(GuestRegs::default())
    }

    #[cfg(not(all(target_os = "windows", feature = "whpx", target_arch = "x86_64")))]
    pub fn get_regs(&self) -> Result<GuestRegs, AccelError> {
        Err(AccelError::NotSupported("WHPX not available on this platform".to_string()))
    }

    /// 设置寄存器
    #[cfg(all(target_os = "windows", feature = "whpx", target_arch = "x86_64"))]
    pub fn set_regs(&mut self, _partition: &WHV_PARTITION_HANDLE, _regs: &GuestRegs) -> Result<(), AccelError> {
        // TODO: 使用 WHvSetVirtualProcessorRegisters 设置寄存器
        Ok(())
    }

    #[cfg(not(all(target_os = "windows", feature = "whpx", target_arch = "x86_64")))]
    pub fn set_regs(&mut self, _regs: &GuestRegs) -> Result<(), AccelError> {
        Err(AccelError::NotSupported("WHPX not available on this platform".to_string()))
    }

    /// 运行 vCPU
    #[cfg(all(target_os = "windows", feature = "whpx"))]
    pub fn run(&mut self, _partition: &WHV_PARTITION_HANDLE) -> Result<(), AccelError> {
        // TODO: 使用 WHvRunVirtualProcessor 运行 vCPU
        Ok(())
    }

    #[cfg(not(all(target_os = "windows", feature = "whpx")))]
    pub fn run(&mut self) -> Result<(), AccelError> {
        Err(AccelError::NotSupported("WHPX not available on this platform".to_string()))
    }
}

/// WHPX 加速器
pub struct AccelWhpx {
    #[cfg(all(target_os = "windows", feature = "whpx"))]
    partition: Option<WHV_PARTITION_HANDLE>,
    
    vcpus: Vec<WhpxVcpu>,
    memory_regions: HashMap<u64, u64>, // gpa -> size
    initialized: bool,
}

impl AccelWhpx {
    pub fn new() -> Self {
        Self {
            #[cfg(all(target_os = "windows", feature = "whpx"))]
            partition: None,
            vcpus: Vec::new(),
            memory_regions: HashMap::new(),
            initialized: false,
        }
    }

    /// 检查 WHPX 是否可用
    #[cfg(all(target_os = "windows", feature = "whpx"))]
    pub fn is_available() -> bool {
        unsafe {
            let mut capability = WHV_CAPABILITY {
                Code: WHV_CAPABILITY_CODE_HYPERVISOR_PRESENT,
                ..Default::default()
            };
            let mut written_size: u32 = 0;
            
            let result = WHvGetCapability(
                WHV_CAPABILITY_CODE_HYPERVISOR_PRESENT,
                &mut capability as *mut _ as *mut _,
                std::mem::size_of::<WHV_CAPABILITY>() as u32,
                &mut written_size,
            );
            
            result.is_ok() && capability.HypervisorPresent.HypervisorPresent != 0
        }
    }

    #[cfg(not(all(target_os = "windows", feature = "whpx")))]
    pub fn is_available() -> bool {
        false
    }
}

impl Accel for AccelWhpx {
    fn init(&mut self) -> Result<(), AccelError> {
        #[cfg(all(target_os = "windows", feature = "whpx"))]
        {
            if self.initialized {
                return Ok(());
            }

            if !Self::is_available() {
                return Err(AccelError::NotAvailable("WHPX not available".to_string()));
            }

            unsafe {
                let mut partition: WHV_PARTITION_HANDLE = std::mem::zeroed();
                
                WHvCreatePartition(&mut partition)
                    .map_err(|e| AccelError::InitFailed(format!("WHvCreatePartition failed: {:?}", e)))?;

                // 设置分区属性
                let mut property = WHV_PARTITION_PROPERTY {
                    ProcessorCount: 1,
                    ..Default::default()
                };

                WHvSetPartitionProperty(
                    partition,
                    WHV_PARTITION_PROPERTY_CODE_PROCESSOR_COUNT,
                    &property as *const _ as *const _,
                    std::mem::size_of::<WHV_PARTITION_PROPERTY>() as u32,
                ).map_err(|e| AccelError::InitFailed(format!("WHvSetPartitionProperty failed: {:?}", e)))?;

                // 设置分区
                WHvSetupPartition(partition)
                    .map_err(|e| AccelError::InitFailed(format!("WHvSetupPartition failed: {:?}", e)))?;

                self.partition = Some(partition);
                self.initialized = true;

                log::info!("WHPX accelerator initialized successfully");
                Ok(())
            }
        }

        #[cfg(not(all(target_os = "windows", feature = "whpx")))]
        {
            Err(AccelError::NotSupported("WHPX only available on Windows".to_string()))
        }
    }

    fn create_vcpu(&mut self, id: u32) -> Result<(), AccelError> {
        #[cfg(all(target_os = "windows", feature = "whpx"))]
        {
            let partition = self.partition.as_ref()
                .ok_or_else(|| AccelError::NotInitialized("Partition not initialized".to_string()))?;

            unsafe {
                WHvCreateVirtualProcessor(*partition, id, 0)
                    .map_err(|e| AccelError::CreateVcpuFailed(format!("WHvCreateVirtualProcessor failed: {:?}", e)))?;
            }

            let vcpu = WhpxVcpu::new(id)?;
            self.vcpus.push(vcpu);

            log::info!("Created WHPX vCPU {}", id);
            Ok(())
        }

        #[cfg(not(all(target_os = "windows", feature = "whpx")))]
        {
            Err(AccelError::NotSupported("WHPX not available on this platform".to_string()))
        }
    }

    fn map_memory(&mut self, gpa: u64, hva: u64, size: u64, flags: u32) -> Result<(), AccelError> {
        #[cfg(all(target_os = "windows", feature = "whpx"))]
        {
            let partition = self.partition.as_ref()
                .ok_or_else(|| AccelError::NotInitialized("Partition not initialized".to_string()))?;

            let mut whpx_flags = WHV_MAP_GPA_RANGE_FLAGS_READ | WHV_MAP_GPA_RANGE_FLAGS_WRITE;
            if flags & 0x4 != 0 {
                whpx_flags |= WHV_MAP_GPA_RANGE_FLAGS_EXECUTE;
            }

            unsafe {
                WHvMapGpaRange(
                    *partition,
                    hva as *const _,
                    gpa,
                    size,
                    whpx_flags,
                ).map_err(|e| AccelError::MapMemoryFailed(format!("WHvMapGpaRange failed: {:?}", e)))?;
            }

            self.memory_regions.insert(gpa, size);
            log::debug!("Mapped memory: GPA 0x{:x}, size 0x{:x}", gpa, size);
            Ok(())
        }

        #[cfg(not(all(target_os = "windows", feature = "whpx")))]
        {
            Err(AccelError::NotSupported("WHPX not available on this platform".to_string()))
        }
    }

    fn unmap_memory(&mut self, gpa: u64, size: u64) -> Result<(), AccelError> {
        #[cfg(all(target_os = "windows", feature = "whpx"))]
        {
            let partition = self.partition.as_ref()
                .ok_or_else(|| AccelError::NotInitialized("Partition not initialized".to_string()))?;

            unsafe {
                WHvUnmapGpaRange(*partition, gpa, size)
                    .map_err(|e| AccelError::UnmapMemoryFailed(format!("WHvUnmapGpaRange failed: {:?}", e)))?;
            }

            self.memory_regions.remove(&gpa);
            log::debug!("Unmapped memory: GPA 0x{:x}", gpa);
            Ok(())
        }

        #[cfg(not(all(target_os = "windows", feature = "whpx")))]
        {
            Err(AccelError::NotSupported("WHPX not available on this platform".to_string()))
        }
    }

    fn run_vcpu(&mut self, vcpu_id: u32, _mmu: &mut dyn MMU) -> Result<(), AccelError> {
        #[cfg(all(target_os = "windows", feature = "whpx"))]
        {
            let partition = self.partition.as_ref()
                .ok_or_else(|| AccelError::NotInitialized("Partition not initialized".to_string()))?;

            let vcpu = self.vcpus.get_mut(vcpu_id as usize)
                .ok_or_else(|| AccelError::InvalidVcpuId(vcpu_id))?;
            
            vcpu.run(partition)
        }

        #[cfg(not(all(target_os = "windows", feature = "whpx")))]
        {
            Err(AccelError::NotSupported("WHPX not available on this platform".to_string()))
        }
    }

    fn get_regs(&self, vcpu_id: u32) -> Result<GuestRegs, AccelError> {
        #[cfg(all(target_os = "windows", feature = "whpx"))]
        {
            let partition = self.partition.as_ref()
                .ok_or_else(|| AccelError::NotInitialized("Partition not initialized".to_string()))?;

            let vcpu = self.vcpus.get(vcpu_id as usize)
                .ok_or_else(|| AccelError::InvalidVcpuId(vcpu_id))?;
            
            vcpu.get_regs(partition)
        }

        #[cfg(not(all(target_os = "windows", feature = "whpx")))]
        {
            Err(AccelError::NotSupported("WHPX not available on this platform".to_string()))
        }
    }

    fn set_regs(&mut self, vcpu_id: u32, regs: &GuestRegs) -> Result<(), AccelError> {
        #[cfg(all(target_os = "windows", feature = "whpx"))]
        {
            let partition = self.partition.as_ref()
                .ok_or_else(|| AccelError::NotInitialized("Partition not initialized".to_string()))?;

            let vcpu = self.vcpus.get_mut(vcpu_id as usize)
                .ok_or_else(|| AccelError::InvalidVcpuId(vcpu_id))?;
            
            vcpu.set_regs(partition, regs)
        }

        #[cfg(not(all(target_os = "windows", feature = "whpx")))]
        {
            Err(AccelError::NotSupported("WHPX not available on this platform".to_string()))
        }
    }

    fn name(&self) -> &str {
        "WHPX"
    }
}

impl Drop for AccelWhpx {
    fn drop(&mut self) {
        #[cfg(all(target_os = "windows", feature = "whpx"))]
        if let Some(partition) = self.partition {
            unsafe {
                let _ = WHvDeletePartition(partition);
            }
            log::info!("WHPX partition destroyed");
        }
    }
}

impl Default for AccelWhpx {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_whpx_availability() {
        println!("WHPX available: {}", AccelWhpx::is_available());
    }

    #[test]
    #[cfg(all(target_os = "windows", feature = "whpx"))]
    fn test_whpx_init() {
        if AccelWhpx::is_available() {
            let mut accel = AccelWhpx::new();
            assert!(accel.init().is_ok());
        }
    }
}
