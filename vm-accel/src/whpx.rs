//! # Windows Hypervisor Platform (WHPX) 加速后端
//!
//! 提供 Windows 平台的硬件虚拟化加速支持。
//!
//! ## 平台要求
//!
//! - Windows 10 版本 1803 或更高版本
//! - 启用 Hyper-V 平台功能
//! - Intel VT-x 或 AMD-V 虚拟化扩展
//!
//! ## 启用方式
//!
//! 1. 在 Windows 功能中启用 "Windows 虚拟机监控程序平台"
//! 2. 编译时启用 `whpx` feature
//!
//! ```toml
//! vm-accel = { path = "../vm-accel", features = ["whpx"] }
//! ```
//!
//! ## 主要功能
//!
//! - 分区管理 (Partition)
//! - vCPU 创建和管理
//! - 物理内存映射 (GPA -> HVA)
//! - 寄存器读写
//!
//! ## 使用示例
//!
//! ```rust,ignore
//! use vm_accel::whpx::AccelWhpx;
//! use vm_accel::Accel;
//!
//! let mut accel = AccelWhpx::new();
//! accel.init()?;
//! accel.create_vcpu(0)?;
//! accel.map_memory(0, hva, size, flags)?;
//! accel.run_vcpu(0, &mut mmu)?;
//! ```

use super::{Accel, AccelError};
use std::collections::HashMap;
use vm_core::{GuestRegs, MMU};

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
    pub fn get_regs(&self, partition: &WHV_PARTITION_HANDLE) -> Result<GuestRegs, AccelError> {
        unsafe {
            let mut register_names = [
                WHV_REGISTER_NAME_RAX,
                WHV_REGISTER_NAME_RBX,
                WHV_REGISTER_NAME_RCX,
                WHV_REGISTER_NAME_RDX,
                WHV_REGISTER_NAME_RSI,
                WHV_REGISTER_NAME_RDI,
                WHV_REGISTER_NAME_RBP,
                WHV_REGISTER_NAME_RSP,
                WHV_REGISTER_NAME_RIP,
            ];
            let mut register_values = vec![WHV_REGISTER_VALUE::default(); register_names.len()];

            WHvGetVirtualProcessorRegisters(
                *partition,
                self.index,
                &register_names,
                &mut register_values,
            )
            .map_err(|e| AccelError::PlatformError(format!("Failed to get registers: {:?}", e)))?;

            let mut regs = GuestRegs::default();
            // 填充寄存器值
            // regs.x[0] = register_values[0].Reg64;
            // ...
            Ok(regs)
        }
    }

    #[cfg(not(all(target_os = "windows", feature = "whpx", target_arch = "x86_64")))]
    pub fn get_regs(&self) -> Result<GuestRegs, AccelError> {
        Err(AccelError::NotSupported(
            "WHPX not available on this platform".to_string(),
        ))
    }

    /// 设置寄存器
    #[cfg(all(target_os = "windows", feature = "whpx", target_arch = "x86_64"))]
    pub fn set_regs(
        &mut self,
        partition: &WHV_PARTITION_HANDLE,
        regs: &GuestRegs,
    ) -> Result<(), AccelError> {
        unsafe {
            // 准备寄存器名称和值数组
            // x86_64寄存器映射：
            // gpr[0] = RAX, gpr[1] = RCX, gpr[2] = RDX, gpr[3] = RBX
            // gpr[4] = RSP (但使用regs.sp), gpr[5] = RBP (但使用regs.fp)
            // gpr[6] = RSI, gpr[7] = RDI
            // gpr[8-15] = R8-R15
            // pc = RIP, sp = RSP, fp = RBP
            let register_names = [
                WHV_REGISTER_NAME_RAX,
                WHV_REGISTER_NAME_RCX,
                WHV_REGISTER_NAME_RDX,
                WHV_REGISTER_NAME_RBX,
                WHV_REGISTER_NAME_RSI,
                WHV_REGISTER_NAME_RDI,
                WHV_REGISTER_NAME_RBP,
                WHV_REGISTER_NAME_RSP,
                WHV_REGISTER_NAME_RIP,
                WHV_REGISTER_NAME_R8,
                WHV_REGISTER_NAME_R9,
                WHV_REGISTER_NAME_R10,
                WHV_REGISTER_NAME_R11,
                WHV_REGISTER_NAME_R12,
                WHV_REGISTER_NAME_R13,
                WHV_REGISTER_NAME_R14,
                WHV_REGISTER_NAME_R15,
            ];

            let mut register_values = vec![WHV_REGISTER_VALUE::default(); register_names.len()];

            // 设置通用寄存器
            register_values[0].Reg64 = regs.gpr[0]; // RAX
            register_values[1].Reg64 = regs.gpr[1]; // RCX
            register_values[2].Reg64 = regs.gpr[2]; // RDX
            register_values[3].Reg64 = regs.gpr[3]; // RBX
            register_values[4].Reg64 = regs.gpr[6]; // RSI
            register_values[5].Reg64 = regs.gpr[7]; // RDI
            register_values[6].Reg64 = regs.fp;    // RBP (使用fp字段)
            register_values[7].Reg64 = regs.sp;     // RSP (使用sp字段)
            register_values[8].Reg64 = regs.pc;     // RIP (使用pc字段)
            register_values[9].Reg64 = regs.gpr[8];  // R8
            register_values[10].Reg64 = regs.gpr[9]; // R9
            register_values[11].Reg64 = regs.gpr[10]; // R10
            register_values[12].Reg64 = regs.gpr[11]; // R11
            register_values[13].Reg64 = regs.gpr[12]; // R12
            register_values[14].Reg64 = regs.gpr[13]; // R13
            register_values[15].Reg64 = regs.gpr[14]; // R14
            register_values[16].Reg64 = regs.gpr[15]; // R15

            // 调用 WHvSetVirtualProcessorRegisters API
            WHvSetVirtualProcessorRegisters(
                *partition,
                self.index,
                &register_names,
                &register_values,
            )
            .map_err(|e| {
                AccelError::PlatformError(format!("Failed to set registers: {:?}", e))
            })?;

            Ok(())
        }
    }

    #[cfg(not(all(target_os = "windows", feature = "whpx", target_arch = "x86_64")))]
    pub fn set_regs(&mut self, _regs: &GuestRegs) -> Result<(), AccelError> {
        Err(AccelError::NotSupported(
            "WHPX not available on this platform".to_string(),
        ))
    }

    /// 运行 vCPU
    #[cfg(all(target_os = "windows", feature = "whpx"))]
    pub fn run(&mut self, partition: &WHV_PARTITION_HANDLE) -> Result<(), AccelError> {
        unsafe {
            let mut exit_context = WHV_RUN_VP_EXIT_CONTEXT::default();

            // 调用 WHvRunVirtualProcessor 运行 vCPU
            let result = WHvRunVirtualProcessor(
                *partition,
                self.index,
                &mut exit_context,
                std::mem::size_of::<WHV_RUN_VP_EXIT_CONTEXT>() as u32,
            );

            // 检查运行结果
            result.map_err(|e| {
                AccelError::RunFailed(format!("WHvRunVirtualProcessor failed: {:?}", e))
            })?;

            // 处理退出原因
            match exit_context.ExitReason {
                WHvRunVpExitReasonX64Halt => {
                    log::debug!("vCPU {} halted", self.index);
                    Ok(())
                }
                WHvRunVpExitReasonX64IoPortAccess => {
                    log::debug!("vCPU {} I/O port access", self.index);
                    // I/O端口访问由上层处理
                    Ok(())
                }
                WHvRunVpExitReasonMemoryAccess => {
                    log::debug!("vCPU {} memory access", self.index);
                    // MMIO访问由上层处理
                    Ok(())
                }
                WHvRunVpExitReasonX64InterruptWindow => {
                    log::debug!("vCPU {} interrupt window", self.index);
                    // 中断窗口，继续运行
                    Ok(())
                }
                WHvRunVpExitReasonX64Exception => {
                    log::warn!("vCPU {} exception", self.index);
                    // 异常处理
                    Ok(())
                }
                _ => {
                    log::warn!("vCPU {} unhandled exit reason: {}", self.index, exit_context.ExitReason);
                    Ok(())
                }
            }
        }
    }

    #[cfg(not(all(target_os = "windows", feature = "whpx")))]
    pub fn run(&mut self) -> Result<(), AccelError> {
        Err(AccelError::NotSupported(
            "WHPX not available on this platform".to_string(),
        ))
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

                WHvCreatePartition(&mut partition).map_err(|e| {
                    AccelError::InitFailed(format!("WHvCreatePartition failed: {:?}", e))
                })?;

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
                )
                .map_err(|e| {
                    AccelError::InitFailed(format!("WHvSetPartitionProperty failed: {:?}", e))
                })?;

                // 设置分区
                WHvSetupPartition(partition).map_err(|e| {
                    AccelError::InitFailed(format!("WHvSetupPartition failed: {:?}", e))
                })?;

                self.partition = Some(partition);
                self.initialized = true;

                log::info!("WHPX accelerator initialized successfully");
                Ok(())
            }
        }

        #[cfg(not(all(target_os = "windows", feature = "whpx")))]
        {
            Err(AccelError::NotSupported(
                "WHPX only available on Windows".to_string(),
            ))
        }
    }

    fn create_vcpu(&mut self, id: u32) -> Result<(), AccelError> {
        #[cfg(all(target_os = "windows", feature = "whpx"))]
        {
            let partition = self.partition.as_ref().ok_or_else(|| {
                AccelError::NotInitialized("Partition not initialized".to_string())
            })?;

            unsafe {
                WHvCreateVirtualProcessor(*partition, id, 0).map_err(|e| {
                    AccelError::CreateVcpuFailed(format!(
                        "WHvCreateVirtualProcessor failed: {:?}",
                        e
                    ))
                })?;
            }

            let vcpu = WhpxVcpu::new(id)?;
            self.vcpus.push(vcpu);

            log::info!("Created WHPX vCPU {}", id);
            Ok(())
        }

        #[cfg(not(all(target_os = "windows", feature = "whpx")))]
        {
            Err(AccelError::NotSupported(
                "WHPX not available on this platform".to_string(),
            ))
        }
    }

    fn map_memory(&mut self, gpa: u64, hva: u64, size: u64, flags: u32) -> Result<(), AccelError> {
        #[cfg(all(target_os = "windows", feature = "whpx"))]
        {
            let partition = self.partition.as_ref().ok_or_else(|| {
                AccelError::NotInitialized("Partition not initialized".to_string())
            })?;

            let mut whpx_flags = WHV_MAP_GPA_RANGE_FLAGS_READ | WHV_MAP_GPA_RANGE_FLAGS_WRITE;
            if flags & 0x4 != 0 {
                whpx_flags |= WHV_MAP_GPA_RANGE_FLAGS_EXECUTE;
            }

            unsafe {
                WHvMapGpaRange(*partition, hva as *const _, gpa, size, whpx_flags).map_err(
                    |e| AccelError::MapMemoryFailed(format!("WHvMapGpaRange failed: {:?}", e)),
                )?;
            }

            self.memory_regions.insert(gpa, size);
            log::debug!("Mapped memory: GPA 0x{:x}, size 0x{:x}", gpa, size);
            Ok(())
        }

        #[cfg(not(all(target_os = "windows", feature = "whpx")))]
        {
            Err(AccelError::NotSupported(
                "WHPX not available on this platform".to_string(),
            ))
        }
    }

    fn unmap_memory(&mut self, gpa: u64, size: u64) -> Result<(), AccelError> {
        #[cfg(all(target_os = "windows", feature = "whpx"))]
        {
            let partition = self.partition.as_ref().ok_or_else(|| {
                AccelError::NotInitialized("Partition not initialized".to_string())
            })?;

            unsafe {
                WHvUnmapGpaRange(*partition, gpa, size).map_err(|e| {
                    AccelError::UnmapMemoryFailed(format!("WHvUnmapGpaRange failed: {:?}", e))
                })?;
            }

            self.memory_regions.remove(&gpa);
            log::debug!("Unmapped memory: GPA 0x{:x}", gpa);
            Ok(())
        }

        #[cfg(not(all(target_os = "windows", feature = "whpx")))]
        {
            Err(AccelError::NotSupported(
                "WHPX not available on this platform".to_string(),
            ))
        }
    }

    fn run_vcpu(&mut self, vcpu_id: u32, _mmu: &mut dyn MMU) -> Result<(), AccelError> {
        #[cfg(all(target_os = "windows", feature = "whpx"))]
        {
            let partition = self.partition.as_ref().ok_or_else(|| {
                AccelError::NotInitialized("Partition not initialized".to_string())
            })?;

            let vcpu = self
                .vcpus
                .get_mut(vcpu_id as usize)
                .ok_or_else(|| AccelError::InvalidVcpuId(vcpu_id))?;

            vcpu.run(partition)
        }

        #[cfg(not(all(target_os = "windows", feature = "whpx")))]
        {
            Err(AccelError::NotSupported(
                "WHPX not available on this platform".to_string(),
            ))
        }
    }

    fn get_regs(&self, vcpu_id: u32) -> Result<GuestRegs, AccelError> {
        #[cfg(all(target_os = "windows", feature = "whpx"))]
        {
            let partition = self.partition.as_ref().ok_or_else(|| {
                AccelError::NotInitialized("Partition not initialized".to_string())
            })?;

            let vcpu = self
                .vcpus
                .get(vcpu_id as usize)
                .ok_or_else(|| AccelError::InvalidVcpuId(vcpu_id))?;

            vcpu.get_regs(partition)
        }

        #[cfg(not(all(target_os = "windows", feature = "whpx")))]
        {
            Err(AccelError::NotSupported(
                "WHPX not available on this platform".to_string(),
            ))
        }
    }

    fn set_regs(&mut self, vcpu_id: u32, regs: &GuestRegs) -> Result<(), AccelError> {
        #[cfg(all(target_os = "windows", feature = "whpx"))]
        {
            let partition = self.partition.as_ref().ok_or_else(|| {
                AccelError::NotInitialized("Partition not initialized".to_string())
            })?;

            let vcpu = self
                .vcpus
                .get_mut(vcpu_id as usize)
                .ok_or_else(|| AccelError::InvalidVcpuId(vcpu_id))?;

            vcpu.set_regs(partition, regs)
        }

        #[cfg(not(all(target_os = "windows", feature = "whpx")))]
        {
            Err(AccelError::NotSupported(
                "WHPX not available on this platform".to_string(),
            ))
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
use crate::event::{AccelEvent, AccelEventSource};
use std::time::{Duration, Instant};
impl AccelEventSource for AccelWhpx {
    fn poll_event(&mut self) -> Option<AccelEvent> {
        None
    }
}
