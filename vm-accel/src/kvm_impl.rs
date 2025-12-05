//! Linux KVM 加速后端完整实现
//!
//! 支持 Intel VT-x, AMD-V 和 ARM 虚拟化扩展

use super::{Accel, AccelError};
use std::collections::HashMap;
use vm_core::{GuestRegs, MMU, PlatformError, VmError};

#[cfg(feature = "kvm")]
use kvm_bindings::*;
#[cfg(feature = "kvm")]
use kvm_ioctls::{Kvm, VcpuExit, VcpuFd, VmFd};

/// KVM vCPU 包装
pub struct KvmVcpu {
    #[cfg(feature = "kvm")]
    fd: VcpuFd,
    id: u32,
    #[cfg(feature = "kvm")]
    run_mmap_size: usize,
}

impl KvmVcpu {
    #[cfg(feature = "kvm")]
    pub fn new(vm: &VmFd, id: u32) -> Result<Self, AccelError> {
        let vcpu = vm.create_vcpu(id as u64).map_err(|e| {
            VmError::Platform(PlatformError::ResourceAllocationFailed(format!(
                "KVM create_vcpu failed: {}",
                e
            )))
        })?;

        let run_mmap_size = vm.get_vcpu_mmap_size().map_err(|e| {
            VmError::Platform(PlatformError::ResourceAllocationFailed(format!(
                "Failed to get mmap size: {}",
                e
            )))
        })?;

        Ok(Self {
            fd: vcpu,
            id,
            run_mmap_size,
        })
    }

    #[cfg(not(feature = "kvm"))]
    pub fn new(_vm: &(), id: u32) -> Result<Self, AccelError> {
        Ok(Self { id })
    }

    /// 获取通用寄存器
    #[cfg(all(feature = "kvm", target_arch = "x86_64"))]
    pub fn get_regs(&self) -> Result<GuestRegs, AccelError> {
        let regs = self.fd.get_regs().map_err(|e| {
            VmError::Platform(PlatformError::AccessDenied(format!(
                "KVM get_regs failed: {}",
                e
            )))
        })?;

        let mut gpr = [0u64; 32];
        gpr[0] = regs.rax;
        gpr[1] = regs.rcx;
        gpr[2] = regs.rdx;
        gpr[3] = regs.rbx;
        gpr[4] = regs.rsp;
        gpr[5] = regs.rbp;
        gpr[6] = regs.rsi;
        gpr[7] = regs.rdi;
        gpr[8] = regs.r8;
        gpr[9] = regs.r9;
        gpr[10] = regs.r10;
        gpr[11] = regs.r11;
        gpr[12] = regs.r12;
        gpr[13] = regs.r13;
        gpr[14] = regs.r14;
        gpr[15] = regs.r15;

        Ok(GuestRegs {
            pc: regs.rip,
            sp: regs.rsp,
            fp: regs.rbp,
            gpr,
        })
    }

    #[cfg(all(feature = "kvm", target_arch = "aarch64"))]
    pub fn get_regs(&self) -> Result<GuestRegs, AccelError> {
        let mut regs = kvm_bindings::kvm_regs::default();
        self.fd.get_regs(&mut regs).map_err(|e| {
            VmError::Platform(PlatformError::AccessDenied(format!(
                "KVM get_regs failed: {}",
                e
            )))
        })?;

        let mut gpr = [0u64; 32];
        gpr[..31].copy_from_slice(&regs.regs[..31]);

        Ok(GuestRegs {
            pc: regs.pc,
            sp: regs.sp,
            fp: regs.regs[29], // x29 is FP on ARM64
            gpr,
        })
    }

    #[cfg(not(feature = "kvm"))]
    pub fn get_regs(&self) -> Result<GuestRegs, AccelError> {
        Err(VmError::Platform(PlatformError::UnsupportedOperation(
            "KVM feature not enabled".to_string(),
        )))
    }

    /// 设置通用寄存器
    #[cfg(all(feature = "kvm", target_arch = "x86_64"))]
    pub fn set_regs(&mut self, regs: &GuestRegs) -> Result<(), AccelError> {
        let kvm_regs = kvm_regs {
            rax: regs.gpr[0],
            rcx: regs.gpr[1],
            rdx: regs.gpr[2],
            rbx: regs.gpr[3],
            rsp: regs.sp,
            rbp: regs.fp,
            rsi: regs.gpr[6],
            rdi: regs.gpr[7],
            r8: regs.gpr[8],
            r9: regs.gpr[9],
            r10: regs.gpr[10],
            r11: regs.gpr[11],
            r12: regs.gpr[12],
            r13: regs.gpr[13],
            r14: regs.gpr[14],
            r15: regs.gpr[15],
            rip: regs.pc,
            rflags: 0x2, // Reserved bit must be 1
        };

        self.fd.set_regs(&kvm_regs).map_err(|e| {
            VmError::Platform(PlatformError::AccessDenied(format!(
                "KVM set_regs failed: {}",
                e
            )))
        })?;

        Ok(())
    }

    #[cfg(all(feature = "kvm", target_arch = "aarch64"))]
    pub fn set_regs(&mut self, regs: &GuestRegs) -> Result<(), AccelError> {
        let mut kvm_regs = kvm_bindings::kvm_regs::default();
        kvm_regs.regs[..31].copy_from_slice(&regs.gpr[..31]);
        kvm_regs.sp = regs.sp;
        kvm_regs.pc = regs.pc;
        kvm_regs.pstate = 0x3c5; // EL1h, DAIF masked

        self.fd.set_regs(&kvm_regs).map_err(|e| {
            VmError::Platform(PlatformError::AccessDenied(format!(
                "KVM set_regs failed: {}",
                e
            )))
        })?;

        Ok(())
    }

    #[cfg(not(feature = "kvm"))]
    pub fn set_regs(&mut self, _regs: &GuestRegs) -> Result<(), AccelError> {
        Err(VmError::Platform(PlatformError::UnsupportedOperation(
            "KVM feature not enabled".to_string(),
        )))
    }

    /// 运行 vCPU
    #[cfg(feature = "kvm")]
    pub fn run(&mut self) -> Result<VcpuExit, AccelError> {
        self.fd.run().map_err(|e| {
            VmError::Platform(PlatformError::ExecutionFailed(format!(
                "KVM vcpu run failed: {}",
                e
            )))
        })
    }

    #[cfg(not(feature = "kvm"))]
    pub fn run(&mut self) -> Result<(), AccelError> {
        Err(VmError::Platform(PlatformError::UnsupportedOperation(
            "KVM feature not enabled".to_string(),
        )))
    }
}

/// KVM 加速器
pub struct AccelKvm {
    #[cfg(feature = "kvm")]
    kvm: Option<Kvm>,
    #[cfg(feature = "kvm")]
    vm: Option<VmFd>,

    vcpus: Vec<KvmVcpu>,
    memory_regions: HashMap<u32, (u64, u64)>, // slot -> (gpa, size)
    next_slot: u32,
}

impl AccelKvm {
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "kvm")]
            kvm: None,
            #[cfg(feature = "kvm")]
            vm: None,
            vcpus: Vec::new(),
            memory_regions: HashMap::new(),
            next_slot: 0,
        }
    }

    /// 检查 KVM 是否可用
    #[cfg(feature = "kvm")]
    pub fn is_available() -> bool {
        std::path::Path::new("/dev/kvm").exists()
    }

    #[cfg(not(feature = "kvm"))]
    pub fn is_available() -> bool {
        false
    }
}

impl Accel for AccelKvm {
    fn init(&mut self) -> Result<(), AccelError> {
        #[cfg(feature = "kvm")]
        {
            if !Self::is_available() {
                return Err(VmError::Platform(PlatformError::HardwareUnavailable(
                    "KVM device /dev/kvm not found".to_string(),
                )));
            }

            let kvm = Kvm::new().map_err(|e| {
                VmError::Platform(PlatformError::InitializationFailed(format!(
                    "Failed to open KVM: {}",
                    e
                )))
            })?;

            // 检查 KVM API 版本
            let api_version = kvm.get_api_version();
            if api_version != 12 {
                return Err(VmError::Platform(PlatformError::InitializationFailed(
                    format!("Unsupported KVM API version: {}", api_version),
                )));
            }

            // 创建 VM
            let vm = kvm.create_vm().map_err(|e| {
                VmError::Platform(PlatformError::ResourceAllocationFailed(format!(
                    "Failed to create VM: {}",
                    e
                )))
            })?;

            self.kvm = Some(kvm);
            self.vm = Some(vm);

            log::info!("KVM accelerator initialized successfully");
            Ok(())
        }

        #[cfg(not(feature = "kvm"))]
        {
            Err(VmError::Platform(PlatformError::UnsupportedOperation(
                "KVM feature not enabled".to_string(),
            )))
        }
    }

    fn create_vcpu(&mut self, id: u32) -> Result<(), AccelError> {
        #[cfg(feature = "kvm")]
        {
            let vm = self.vm.as_ref().ok_or_else(|| {
                VmError::Platform(PlatformError::InitializationFailed(
                    "VM not initialized".to_string(),
                ))
            })?;

            let vcpu = KvmVcpu::new(vm, id)?;
            self.vcpus.push(vcpu);

            log::info!("Created KVM vCPU {}", id);
            Ok(())
        }

        #[cfg(not(feature = "kvm"))]
        {
            Err(VmError::Platform(PlatformError::UnsupportedOperation(
                "KVM feature not enabled".to_string(),
            )))
        }
    }

    fn map_memory(&mut self, gpa: u64, hva: u64, size: u64, _flags: u32) -> Result<(), AccelError> {
        #[cfg(feature = "kvm")]
        {
            let vm = self.vm.as_mut().ok_or_else(|| {
                VmError::Platform(PlatformError::InitializationFailed(
                    "VM not initialized".to_string(),
                ))
            })?;

            let slot = self.next_slot;
            self.next_slot += 1;

            let mem_region = kvm_userspace_memory_region {
                slot,
                flags: 0,
                guest_phys_addr: gpa,
                memory_size: size,
                userspace_addr: hva,
            };

            unsafe {
                vm.set_user_memory_region(mem_region).map_err(|e| {
                    VmError::Platform(PlatformError::MemoryMappingFailed(format!(
                        "KVM set_user_memory_region failed: {}",
                        e
                    )))
                })?;
            }

            self.memory_regions.insert(slot, (gpa, size));

            log::debug!(
                "Mapped memory: GPA 0x{:x}, size 0x{:x}, slot {}",
                gpa,
                size,
                slot
            );
            Ok(())
        }

        #[cfg(not(feature = "kvm"))]
        {
            Err(VmError::Platform(PlatformError::UnsupportedOperation(
                "KVM feature not enabled".to_string(),
            )))
        }
    }

    fn unmap_memory(&mut self, gpa: u64, size: u64) -> Result<(), AccelError> {
        #[cfg(feature = "kvm")]
        {
            let vm = self.vm.as_mut().ok_or_else(|| {
                VmError::Platform(PlatformError::InitializationFailed(
                    "VM not initialized".to_string(),
                ))
            })?;

            // 查找对应的 slot
            let slot = self
                .memory_regions
                .iter()
                .find(|(_, &(region_gpa, region_size))| region_gpa == gpa && region_size == size)
                .map(|(&slot, _)| slot)
                .ok_or_else(|| {
                    VmError::Platform(PlatformError::InvalidParameter(format!(
                        "Memory region not found: GPA 0x{:x}",
                        gpa
                    )))
                })?;

            let mem_region = kvm_userspace_memory_region {
                slot,
                flags: 0,
                guest_phys_addr: gpa,
                memory_size: 0, // size = 0 表示删除
                userspace_addr: 0,
            };

            unsafe {
                vm.set_user_memory_region(mem_region).map_err(|e| {
                    VmError::Platform(PlatformError::MemoryMappingFailed(format!(
                        "KVM unmap failed: {}",
                        e
                    )))
                })?;
            }

            self.memory_regions.remove(&slot);

            log::debug!("Unmapped memory: GPA 0x{:x}, slot {}", gpa, slot);
            Ok(())
        }

        #[cfg(not(feature = "kvm"))]
        {
            Err(VmError::Platform(PlatformError::UnsupportedOperation(
                "KVM feature not enabled".to_string(),
            )))
        }
    }

    /// 处理I/O输出（端口I/O写入）
    fn handle_io_out(&self, port: u16, data: &[u8], mmu: &mut dyn MMU) -> Result<(), AccelError> {
        // 根据端口号路由到相应设备
        match port {
            0x3F8..=0x3FF => {
                // COM1 串口 - 写入数据
                // 简化实现：输出到日志
                if let Ok(text) = std::str::from_utf8(data) {
                    log::info!("COM1 output: {}", text);
                } else {
                    log::debug!("COM1 write to port 0x{:x}: {:?}", port, data);
                }
            }
            0x2F8..=0x2FF => {
                // COM2 串口
                if let Ok(text) = std::str::from_utf8(data) {
                    log::info!("COM2 output: {}", text);
                } else {
                    log::debug!("COM2 write to port 0x{:x}: {:?}", port, data);
                }
            }
            0x60..=0x64 => {
                // 键盘控制器
                log::trace!("Keyboard controller write to port 0x{:x}: {:?}", port, data);
            }
            0x70..=0x71 => {
                // RTC (Real-Time Clock)
                log::trace!("RTC write to port 0x{:x}: {:?}", port, data);
            }
            0xCF8..=0xCFF => {
                // PCI配置空间
                log::trace!("PCI config write to port 0x{:x}: {:?}", port, data);
            }
            _ => {
                // 未知端口
                log::warn!("Unhandled I/O OUT port: 0x{:x}, data: {:?}", port, data);
            }
        }
        
        Ok(())
    }

    /// 处理MMIO读取（内存映射I/O读取）
    fn handle_mmio_read(&self, addr: u64, data: &mut [u8], mmu: &mut dyn MMU) -> Result<(), AccelError> {
        // MMIO地址通过MMU路由到相应设备
        // 常见MMIO地址范围：
        // 0x0200_0000-0x0200_1000: CLINT (Core Local Interruptor)
        // 0x0C00_0000-0x1000_0000: PLIC (Platform Level Interrupt Controller)
        // 0x1000_0000-0x1000_2000: VirtIO设备
        
        // 根据地址范围确定访问大小
        let size = data.len() as u8;
        
        // 通过MMU读取MMIO地址
        match mmu.read(addr, size) {
            Ok(value) => {
                // 将读取的值写入data缓冲区
                match size {
                    1 => data[0] = value as u8,
                    2 => {
                        data[0] = value as u8;
                        data[1] = (value >> 8) as u8;
                    }
                    4 => {
                        data[0] = value as u8;
                        data[1] = (value >> 8) as u8;
                        data[2] = (value >> 16) as u8;
                        data[3] = (value >> 24) as u8;
                    }
                    8 => {
                        data[0] = value as u8;
                        data[1] = (value >> 8) as u8;
                        data[2] = (value >> 16) as u8;
                        data[3] = (value >> 24) as u8;
                        data[4] = (value >> 32) as u8;
                        data[5] = (value >> 40) as u8;
                        data[6] = (value >> 48) as u8;
                        data[7] = (value >> 56) as u8;
                    }
                    _ => {
                        // 对于其他大小，使用字节数组转换
                        let bytes = value.to_le_bytes();
                        let copy_len = std::cmp::min(data.len(), bytes.len());
                        data[..copy_len].copy_from_slice(&bytes[..copy_len]);
                    }
                }
                log::trace!("MMIO read from 0x{:x}, size {}, value 0x{:x}", addr, size, value);
                Ok(())
            }
            Err(e) => {
                log::warn!("MMIO read failed at 0x{:x}: {:?}", addr, e);
                // 读取失败时填充0
                data.fill(0);
                Err(VmError::Platform(PlatformError::MemoryAccessFailed(format!(
                    "MMIO read failed: {:?}",
                    e
                ))).into())
            }
        }
    }

    /// 处理MMIO写入（内存映射I/O写入）
    fn handle_mmio_write(&self, addr: u64, data: &[u8], mmu: &mut dyn MMU) -> Result<(), AccelError> {
        // MMIO地址通过MMU路由到相应设备
        let size = data.len() as u8;
        
        // 将data缓冲区转换为u64值
        let value = match size {
            1 => data[0] as u64,
            2 => (data[0] as u64) | ((data[1] as u64) << 8),
            4 => {
                (data[0] as u64)
                    | ((data[1] as u64) << 8)
                    | ((data[2] as u64) << 16)
                    | ((data[3] as u64) << 24)
            }
            8 => {
                (data[0] as u64)
                    | ((data[1] as u64) << 8)
                    | ((data[2] as u64) << 16)
                    | ((data[3] as u64) << 24)
                    | ((data[4] as u64) << 32)
                    | ((data[5] as u64) << 40)
                    | ((data[6] as u64) << 48)
                    | ((data[7] as u64) << 56)
            }
            _ => {
                // 对于其他大小，使用字节数组转换
                let mut bytes = [0u8; 8];
                let copy_len = std::cmp::min(data.len(), bytes.len());
                bytes[..copy_len].copy_from_slice(&data[..copy_len]);
                u64::from_le_bytes(bytes)
            }
        };
        
        // 通过MMU写入MMIO地址
        match mmu.write(addr, value, size) {
            Ok(_) => {
                log::trace!("MMIO write to 0x{:x}, size {}, value 0x{:x}", addr, size, value);
                Ok(())
            }
            Err(e) => {
                log::warn!("MMIO write failed at 0x{:x}: {:?}", addr, e);
                Err(VmError::Platform(PlatformError::MemoryAccessFailed(format!(
                    "MMIO write failed: {:?}",
                    e
                ))).into())
            }
        }
    }

    /// 处理I/O输入（端口I/O读取）
    fn handle_io_in(&self, port: u16, data: &mut [u8], mmu: &mut dyn MMU) -> Result<(), AccelError> {
        // 根据端口号路由到相应设备
        // 常见端口映射：
        // 0x3F8-0x3FF: COM1 (串口)
        // 0x2F8-0x2FF: COM2 (串口)
        // 0x60-0x64: 键盘控制器
        // 0x70-0x71: RTC
        // 0xCF8-0xCFF: PCI配置空间
        
        match port {
            0x3F8..=0x3FF => {
                // COM1 串口 - 读取数据
                // 简化实现：返回0表示无数据
                data.fill(0);
                log::trace!("COM1 read from port 0x{:x}", port);
            }
            0x2F8..=0x2FF => {
                // COM2 串口
                data.fill(0);
                log::trace!("COM2 read from port 0x{:x}", port);
            }
            0x60..=0x64 => {
                // 键盘控制器
                data.fill(0);
                log::trace!("Keyboard controller read from port 0x{:x}", port);
            }
            0x70..=0x71 => {
                // RTC (Real-Time Clock)
                data.fill(0);
                log::trace!("RTC read from port 0x{:x}", port);
            }
            0xCF8..=0xCFF => {
                // PCI配置空间
                data.fill(0);
                log::trace!("PCI config read from port 0x{:x}", port);
            }
            _ => {
                // 未知端口，返回0
                log::warn!("Unhandled I/O IN port: 0x{:x}", port);
                data.fill(0);
            }
        }
        
        Ok(())
    }

    fn run_vcpu(&mut self, vcpu_id: u32, _mmu: &mut dyn MMU) -> Result<(), AccelError> {
        #[cfg(feature = "kvm")]
        {
            let vcpu = self.vcpus.get_mut(vcpu_id as usize).ok_or_else(|| {
                VmError::Platform(PlatformError::InvalidParameter(format!(
                    "Invalid vCPU ID: {}",
                    vcpu_id
                )))
            })?;

            match vcpu.run()? {
                VcpuExit::Hlt => {
                    log::debug!("vCPU {} halted", vcpu_id);
                    Ok(())
                }
                VcpuExit::Shutdown => {
                    log::info!("vCPU {} shutdown", vcpu_id);
                    Ok(())
                }
                VcpuExit::IoIn(port, data) => {
                    log::debug!("I/O IN: port 0x{:x}, size {}", port, data.len());
                    self.handle_io_in(port, data, _mmu)
                }
                VcpuExit::IoOut(port, data) => {
                    log::debug!("I/O OUT: port 0x{:x}, data {:?}", port, data);
                    self.handle_io_out(port, data, _mmu)
                }
                VcpuExit::MmioRead(addr, data) => {
                    log::debug!("MMIO READ: addr 0x{:x}, size {}", addr, data.len());
                    self.handle_mmio_read(addr, data, _mmu)
                }
                VcpuExit::MmioWrite(addr, data) => {
                    log::debug!("MMIO WRITE: addr 0x{:x}, data {:?}", addr, data);
                    self.handle_mmio_write(addr, data, _mmu)
                }
                exit => {
                    log::warn!("Unhandled vCPU exit: {:?}", exit);
                    Err(VmError::Platform(PlatformError::ExecutionFailed(format!(
                        "Unhandled exit: {:?}",
                        exit
                    ))))
                }
            }
        }

        #[cfg(not(feature = "kvm"))]
        {
            Err(VmError::Platform(PlatformError::UnsupportedOperation(
                "KVM feature not enabled".to_string(),
            )))
        }
    }

    fn get_regs(&self, vcpu_id: u32) -> Result<GuestRegs, AccelError> {
        let vcpu = self.vcpus.get(vcpu_id as usize).ok_or_else(|| {
            VmError::Platform(PlatformError::InvalidParameter(format!(
                "Invalid vCPU ID: {}",
                vcpu_id
            )))
        })?;
        vcpu.get_regs()
    }

    fn set_regs(&mut self, vcpu_id: u32, regs: &GuestRegs) -> Result<(), AccelError> {
        let vcpu = self.vcpus.get_mut(vcpu_id as usize).ok_or_else(|| {
            VmError::Platform(PlatformError::InvalidParameter(format!(
                "Invalid vCPU ID: {}",
                vcpu_id
            )))
        })?;
        vcpu.set_regs(regs)
    }

    fn name(&self) -> &str {
        "KVM"
    }
}

impl Default for AccelKvm {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kvm_availability() {
        println!("KVM available: {}", AccelKvm::is_available());
    }

    #[test]
    #[cfg(feature = "kvm")]
    fn test_kvm_init() {
        if AccelKvm::is_available() {
            let mut accel = AccelKvm::new();
            assert!(accel.init().is_ok());
        }
    }
}
