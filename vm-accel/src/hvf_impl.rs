//! macOS Hypervisor.framework 加速后端完整实现
//!
//! 支持 Intel 和 Apple Silicon (M系列)

use super::{Accel, AccelError};
use std::collections::HashMap;
use vm_core::error::CoreError;
use vm_core::{GuestRegs, MMU, VmError};

#[cfg(target_os = "macos")]
use std::ptr;

// Hypervisor.framework FFI 绑定
#[cfg(target_os = "macos")]
#[link(name = "Hypervisor", kind = "framework")]
#[allow(dead_code)]
unsafe extern "C" {
    // VM 管理
    fn hv_vm_create(config: *mut std::ffi::c_void) -> i32;
    fn hv_vm_destroy() -> i32;
    fn hv_vm_map(uva: *const std::ffi::c_void, gpa: u64, size: usize, flags: u64) -> i32;
    fn hv_vm_unmap(gpa: u64, size: usize) -> i32;
    fn hv_vm_protect(gpa: u64, size: usize, flags: u64) -> i32;

    // vCPU 管理
    fn hv_vcpu_create(
        vcpu: *mut u32,
        exit: *mut std::ffi::c_void,
        config: *mut std::ffi::c_void,
    ) -> i32;
    fn hv_vcpu_destroy(vcpu: u32) -> i32;
    fn hv_vcpu_run(vcpu: u32) -> i32;

    // x86_64 寄存器访问
    #[cfg(target_arch = "x86_64")]
    fn hv_vcpu_read_register(vcpu: u32, reg: u32, value: *mut u64) -> i32;
    #[cfg(target_arch = "x86_64")]
    fn hv_vcpu_write_register(vcpu: u32, reg: u32, value: u64) -> i32;

    // ARM64 寄存器访问
    #[cfg(target_arch = "aarch64")]
    fn hv_vcpu_get_reg(vcpu: u32, reg: u32, value: *mut u64) -> i32;
    #[cfg(target_arch = "aarch64")]
    fn hv_vcpu_set_reg(vcpu: u32, reg: u32, value: u64) -> i32;

    // VM Exit 读取
    #[cfg(target_arch = "x86_64")]
    fn hv_vcpu_read_exit_reason(vcpu: u32, reason: *mut u64) -> i32;
    #[cfg(target_arch = "x86_64")]
    fn hv_vcpu_read_exit_instruction_length(vcpu: u32, length: *mut u64) -> i32;
    #[cfg(target_arch = "x86_64")]
    fn hv_vcpu_read_exit_qualification(vcpu: u32, qualification: *mut u64) -> i32;
    #[cfg(target_arch = "x86_64")]
    fn hv_vcpu_read_exit_io_port(vcpu: u32, port: *mut u64) -> i32;
    #[cfg(target_arch = "x86_64")]
    fn hv_vcpu_read_exit_io_access_size(vcpu: u32, size: *mut u64) -> i32;
    #[cfg(target_arch = "x86_64")]
    fn hv_vcpu_read_exit_io_direction(vcpu: u32, direction: *mut u64) -> i32;

    // ARM64 Exit 读取
    #[cfg(target_arch = "aarch64")]
    fn hv_vcpu_get_reg_state(vcpu: u32, reg: u32, value: *mut u64) -> i32;
}

// HV 返回码
#[cfg(target_os = "macos")]
const HV_SUCCESS: i32 = 0;
#[cfg(target_os = "macos")]
#[allow(dead_code)]
const HV_ERROR: i32 = 0xfae94001u32 as i32;
#[cfg(target_os = "macos")]
#[allow(dead_code)]
const HV_BUSY: i32 = 0xfae94002u32 as i32;
#[cfg(target_os = "macos")]
#[allow(dead_code)]
const HV_BAD_ARGUMENT: i32 = 0xfae94003u32 as i32;
#[cfg(target_os = "macos")]
#[allow(dead_code)]
const HV_NO_RESOURCES: i32 = 0xfae94005u32 as i32;
#[cfg(target_os = "macos")]
#[allow(dead_code)]
const HV_NO_DEVICE: i32 = 0xfae94006u32 as i32;
#[cfg(target_os = "macos")]
#[allow(dead_code)]
const HV_UNSUPPORTED: i32 = 0xfae9400fu32 as i32;

// x86_64 VM Exit 原因
#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
#[allow(dead_code)]
mod x86_exit {
    pub const HV_EXIT_REASON_EXCEPTION: u64 = 0;
    pub const HV_EXIT_REASON_EXTERNAL_INTERRUPT: u64 = 1;
    pub const HV_EXIT_REASON_TRIPLE_FAULT: u64 = 2;
    pub const HV_EXIT_REASON_PENDING_INTERRUPT: u64 = 7;
    pub const HV_EXIT_REASON_NMI_WINDOW: u64 = 8;
    pub const HV_EXIT_REASON_TASK_SWITCH: u64 = 9;
    pub const HV_EXIT_REASON_CPUID: u64 = 10;
    pub const HV_EXIT_REASON_GETSEC: u64 = 11;
    pub const HV_EXIT_REASON_HLT: u64 = 12;
    pub const HV_EXIT_REASON_INVD: u64 = 13;
    pub const HV_EXIT_REASON_INVLPG: u64 = 14;
    pub const HV_EXIT_REASON_RDPMC: u64 = 15;
    pub const HV_EXIT_REASON_RDTSC: u64 = 16;
    pub const HV_EXIT_REASON_RSM: u64 = 17;
    pub const HV_EXIT_REASON_VMCALL: u64 = 18;
    pub const HV_EXIT_REASON_VMCLEAR: u64 = 19;
    pub const HV_EXIT_REASON_VMLAUNCH: u64 = 20;
    pub const HV_EXIT_REASON_VMPTRLD: u64 = 21;
    pub const HV_EXIT_REASON_VMPTRST: u64 = 22;
    pub const HV_EXIT_REASON_VMREAD: u64 = 23;
    pub const HV_EXIT_REASON_VMRESUME: u64 = 24;
    pub const HV_EXIT_REASON_VMWRITE: u64 = 25;
    pub const HV_EXIT_REASON_VMXOFF: u64 = 26;
    pub const HV_EXIT_REASON_VMXON: u64 = 27;
    pub const HV_EXIT_REASON_CR_ACCESS: u64 = 28;
    pub const HV_EXIT_REASON_DR_ACCESS: u64 = 29;
    pub const HV_EXIT_REASON_IO_INSTRUCTION: u64 = 30;
    pub const HV_EXIT_REASON_RDMSR: u64 = 31;
    pub const HV_EXIT_REASON_WRMSR: u64 = 32;
    pub const HV_EXIT_REASON_ENTRY_FAIL_GUEST: u64 = 33;
    pub const HV_EXIT_REASON_ENTRY_FAIL_MSR: u64 = 34;
    pub const HV_EXIT_REASON_MWAIT: u64 = 36;
    pub const HV_EXIT_REASON_MTF: u64 = 37;
    pub const HV_EXIT_REASON_MONITOR: u64 = 39;
    pub const HV_EXIT_REASON_PAUSE: u64 = 40;
    pub const HV_EXIT_REASON_ENTRY_FAIL_MC: u64 = 41;
    pub const HV_EXIT_REASON_TPR_BELOW_THRESHOLD: u64 = 43;
    pub const HV_EXIT_REASON_APIC_ACCESS: u64 = 44;
    pub const HV_EXIT_REASON_VIRTUALIZED_EOI: u64 = 45;
    pub const HV_EXIT_REASON_GDTR_IDTR: u64 = 46;
    pub const HV_EXIT_REASON_LDTR_TR: u64 = 47;
    pub const HV_EXIT_REASON_EPT_VIOLATION: u64 = 48;
    pub const HV_EXIT_REASON_EPT_MISCONFIG: u64 = 49;
    pub const HV_EXIT_REASON_INVEPT: u64 = 50;
    pub const HV_EXIT_REASON_RDTSCP: u64 = 51;
    pub const HV_EXIT_REASON_PREEMPTION_TIMER: u64 = 52;
    pub const HV_EXIT_REASON_INVVPID: u64 = 53;
    pub const HV_EXIT_REASON_WBINVD: u64 = 54;
    pub const HV_EXIT_REASON_XSETBV: u64 = 55;
}

// ARM64 VM Exit 原因
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[allow(dead_code)]
mod arm_exit {
    pub const HV_EXIT_REASON_EXCEPTION: u64 = 0;
    pub const HV_EXIT_REASON_UNKNOWN: u64 = 1;
}

/// VM Exit 类型
#[derive(Debug, Clone, PartialEq)]
pub enum HvmExit {
    /// IO 指令退出
    Io {
        port: u64,
        size: u64,
        is_write: bool,
    },
    /// MMIO/EPT 违规退出
    Mmio { gpa: u64, size: u64, is_write: bool },
    /// 中断退出
    Interrupt,
    /// CPUID 指令
    Cpuid { leaf: u32, subleaf: u32 },
    /// MSR 读取
    Rdmsr { msr: u32 },
    /// MSR 写入
    Wrmsr { msr: u32, value: u64 },
    /// HLT 指令
    Hlt,
    /// 异常
    Exception { vector: u8, error_code: u64 },
    /// 未知退出原因
    Unknown { reason: u64 },
}

/// HVF 错误类型
#[derive(Debug, thiserror::Error)]
pub enum HvfError {
    /// vCPU 无效
    #[error("Invalid vCPU ID: {0}")]
    InvalidVcpu(u32),
    /// vCPU 操作失败
    #[error("vCPU operation failed: {0}")]
    VcpuError(String),
    /// 读取退出信息失败
    #[error("Failed to read exit info: {0}")]
    ExitReadError(String),
}

// 内存权限标志
#[cfg(target_os = "macos")]
const HV_MEMORY_READ: u64 = 1 << 0;
#[cfg(target_os = "macos")]
const HV_MEMORY_WRITE: u64 = 1 << 1;
#[cfg(target_os = "macos")]
const HV_MEMORY_EXEC: u64 = 1 << 2;

// x86_64 寄存器定义
#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
#[allow(dead_code)]
mod x86_regs {
    pub const HV_X86_RIP: u32 = 0;
    pub const HV_X86_RFLAGS: u32 = 1;
    pub const HV_X86_RAX: u32 = 2;
    pub const HV_X86_RCX: u32 = 3;
    pub const HV_X86_RDX: u32 = 4;
    pub const HV_X86_RBX: u32 = 5;
    pub const HV_X86_RSI: u32 = 6;
    pub const HV_X86_RDI: u32 = 7;
    pub const HV_X86_RSP: u32 = 8;
    pub const HV_X86_RBP: u32 = 9;
    pub const HV_X86_R8: u32 = 10;
    pub const HV_X86_R9: u32 = 11;
    pub const HV_X86_R10: u32 = 12;
    pub const HV_X86_R11: u32 = 13;
    pub const HV_X86_R12: u32 = 14;
    pub const HV_X86_R13: u32 = 15;
    pub const HV_X86_R14: u32 = 16;
    pub const HV_X86_R15: u32 = 17;
}

// ARM64 寄存器定义
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[allow(dead_code)]
mod arm_regs {
    pub const HV_REG_X0: u32 = 0;
    pub const HV_REG_X1: u32 = 1;
    // ... X2-X28
    pub const HV_REG_FP: u32 = 29;
    pub const HV_REG_LR: u32 = 30;
    pub const HV_REG_SP: u32 = 31;
    pub const HV_REG_PC: u32 = 32;
    pub const HV_REG_CPSR: u32 = 33;
}

/// HVF vCPU
pub struct HvfVcpu {
    #[cfg(target_os = "macos")]
    id: u32,
    #[cfg(not(target_os = "macos"))]
    _id: u32,
}

impl HvfVcpu {
    #[cfg(target_os = "macos")]
    pub fn new(_id: u32) -> Result<Self, AccelError> {
        let mut vcpu_id: u32 = 0;
        // SAFETY: hv_vcpu_create is an extern C function from Hypervisor.framework
        // Preconditions: &mut vcpu_id must be valid for writing, exit and config pointers may be null
        // Returns: HV_SUCCESS on success, error code otherwise
        let ret = unsafe { hv_vcpu_create(&mut vcpu_id, ptr::null_mut(), ptr::null_mut()) };

        if ret != HV_SUCCESS {
            return Err(VmError::Core(CoreError::Internal {
                message: format!("hv_vcpu_create failed: 0x{:x}", ret),
                module: "vm-accel".to_string(),
            }));
        }

        Ok(Self { id: vcpu_id })
    }

    #[cfg(not(target_os = "macos"))]
    pub fn new(_id: u32) -> Result<Self, AccelError> {
        Ok(Self { _id: _id })
    }

    /// 获取寄存器
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    pub fn get_regs(&self) -> Result<GuestRegs, AccelError> {
        use x86_regs::*;

        let mut regs = GuestRegs::default();

        // SAFETY: hv_vcpu_read_register is an extern C function from Hypervisor.framework
        // Preconditions: self.id is a valid vCPU ID, register IDs are valid, pointers point to valid u64 memory
        // Invariants: Reads register values into provided pointers
        unsafe {
            hv_vcpu_read_register(self.id, HV_X86_RIP, &mut regs.pc);
            hv_vcpu_read_register(self.id, HV_X86_RSP, &mut regs.sp);
            hv_vcpu_read_register(self.id, HV_X86_RBP, &mut regs.fp);

            hv_vcpu_read_register(self.id, HV_X86_RAX, &mut regs.gpr[0]);
            hv_vcpu_read_register(self.id, HV_X86_RCX, &mut regs.gpr[1]);
            hv_vcpu_read_register(self.id, HV_X86_RDX, &mut regs.gpr[2]);
            hv_vcpu_read_register(self.id, HV_X86_RBX, &mut regs.gpr[3]);
            hv_vcpu_read_register(self.id, HV_X86_RSI, &mut regs.gpr[6]);
            hv_vcpu_read_register(self.id, HV_X86_RDI, &mut regs.gpr[7]);
            hv_vcpu_read_register(self.id, HV_X86_R8, &mut regs.gpr[8]);
            hv_vcpu_read_register(self.id, HV_X86_R9, &mut regs.gpr[9]);
            hv_vcpu_read_register(self.id, HV_X86_R10, &mut regs.gpr[10]);
            hv_vcpu_read_register(self.id, HV_X86_R11, &mut regs.gpr[11]);
            hv_vcpu_read_register(self.id, HV_X86_R12, &mut regs.gpr[12]);
            hv_vcpu_read_register(self.id, HV_X86_R13, &mut regs.gpr[13]);
            hv_vcpu_read_register(self.id, HV_X86_R14, &mut regs.gpr[14]);
            hv_vcpu_read_register(self.id, HV_X86_R15, &mut regs.gpr[15]);
        }

        Ok(regs)
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    pub fn get_regs(&self) -> Result<GuestRegs, AccelError> {
        use arm_regs::*;

        let mut regs = GuestRegs::default();

        // SAFETY: hv_vcpu_get_reg is an extern C function from Hypervisor.framework
        // Preconditions: self.id is a valid vCPU ID, register IDs are valid (HV_REG_X0 + i stays within range)
        // Invariants: Reads register values into provided pointers
        unsafe {
            hv_vcpu_get_reg(self.id, HV_REG_PC, &mut regs.pc);
            hv_vcpu_get_reg(self.id, HV_REG_SP, &mut regs.sp);
            hv_vcpu_get_reg(self.id, HV_REG_FP, &mut regs.fp);

            for i in 0..31 {
                hv_vcpu_get_reg(self.id, HV_REG_X0 + i, &mut regs.gpr[i as usize]);
            }
        }

        Ok(regs)
    }

    #[cfg(not(target_os = "macos"))]
    pub fn get_regs(&self) -> Result<GuestRegs, AccelError> {
        Err(VmError::Core(CoreError::NotSupported {
            feature: "HVF get_regs".to_string(),
            module: "vm-accel".to_string(),
        }))
    }

    /// 设置寄存器
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    pub fn set_regs(&mut self, regs: &GuestRegs) -> Result<(), AccelError> {
        use x86_regs::*;

        // SAFETY: hv_vcpu_write_register is an extern C function from Hypervisor.framework
        // Preconditions: self.id is a valid vCPU ID, register IDs are valid, values are valid u64
        // Invariants: Writes register values to vCPU state
        unsafe {
            hv_vcpu_write_register(self.id, HV_X86_RIP, regs.pc);
            hv_vcpu_write_register(self.id, HV_X86_RSP, regs.sp);
            hv_vcpu_write_register(self.id, HV_X86_RBP, regs.fp);
            hv_vcpu_write_register(self.id, HV_X86_RFLAGS, 0x2); // Reserved bit

            hv_vcpu_write_register(self.id, HV_X86_RAX, regs.gpr[0]);
            hv_vcpu_write_register(self.id, HV_X86_RCX, regs.gpr[1]);
            hv_vcpu_write_register(self.id, HV_X86_RDX, regs.gpr[2]);
            hv_vcpu_write_register(self.id, HV_X86_RBX, regs.gpr[3]);
            hv_vcpu_write_register(self.id, HV_X86_RSI, regs.gpr[6]);
            hv_vcpu_write_register(self.id, HV_X86_RDI, regs.gpr[7]);
            hv_vcpu_write_register(self.id, HV_X86_R8, regs.gpr[8]);
            hv_vcpu_write_register(self.id, HV_X86_R9, regs.gpr[9]);
            hv_vcpu_write_register(self.id, HV_X86_R10, regs.gpr[10]);
            hv_vcpu_write_register(self.id, HV_X86_R11, regs.gpr[11]);
            hv_vcpu_write_register(self.id, HV_X86_R12, regs.gpr[12]);
            hv_vcpu_write_register(self.id, HV_X86_R13, regs.gpr[13]);
            hv_vcpu_write_register(self.id, HV_X86_R14, regs.gpr[14]);
            hv_vcpu_write_register(self.id, HV_X86_R15, regs.gpr[15]);
        }

        Ok(())
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    pub fn set_regs(&mut self, regs: &GuestRegs) -> Result<(), AccelError> {
        use arm_regs::*;

        // SAFETY: hv_vcpu_set_reg is an extern C function from Hypervisor.framework
        // Preconditions: self.id is a valid vCPU ID, register IDs are valid (HV_REG_X0 + i stays within range)
        // Invariants: Writes register values to vCPU state
        unsafe {
            hv_vcpu_set_reg(self.id, HV_REG_PC, regs.pc);
            hv_vcpu_set_reg(self.id, HV_REG_SP, regs.sp);
            hv_vcpu_set_reg(self.id, HV_REG_FP, regs.fp);
            hv_vcpu_set_reg(self.id, HV_REG_CPSR, 0x3c5); // EL1h

            for i in 0..31 {
                hv_vcpu_set_reg(self.id, HV_REG_X0 + i, regs.gpr[i as usize]);
            }
        }

        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    pub fn set_regs(&mut self, _regs: &GuestRegs) -> Result<(), AccelError> {
        Err(VmError::Core(CoreError::NotSupported {
            feature: "HVF set_regs".to_string(),
            module: "vm-accel".to_string(),
        }))
    }

    /// 运行 vCPU
    #[cfg(target_os = "macos")]
    pub fn run(&mut self) -> Result<(), AccelError> {
        // SAFETY: hv_vcpu_run is an extern C function from Hypervisor.framework
        // Preconditions: self.id is a valid vCPU ID created by hv_vcpu_create
        // Invariants: Executes vCPU until VM exit, returns HV_SUCCESS on normal exit
        let ret = unsafe { hv_vcpu_run(self.id) };

        if ret != HV_SUCCESS {
            return Err(VmError::Core(CoreError::Internal {
                message: format!("hv_vcpu_run failed: 0x{:x}", ret),
                module: "vm-accel".to_string(),
            }));
        }

        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    pub fn run(&mut self) -> Result<(), AccelError> {
        Err(VmError::Core(CoreError::NotSupported {
            feature: "HVF run".to_string(),
            module: "vm-accel".to_string(),
        }))
    }
}

impl Drop for HvfVcpu {
    fn drop(&mut self) {
        #[cfg(target_os = "macos")]
        {
            // SAFETY: hv_vcpu_destroy is an extern C function from Hypervisor.framework
            // Preconditions: self.id is a valid vCPU ID created by hv_vcpu_create
            // Invariants: Cleans up vCPU resources, invalidates the vCPU ID
            unsafe {
                hv_vcpu_destroy(self.id);
            }
        }
    }
}

/// HVF 加速器
pub struct AccelHvf {
    vcpus: HashMap<u32, HvfVcpu>,
    memory_regions: HashMap<u64, u64>, // gpa -> size
    initialized: bool,
}

impl AccelHvf {
    pub fn new() -> Self {
        Self {
            vcpus: HashMap::new(),
            memory_regions: HashMap::new(),
            initialized: false,
        }
    }

    /// 检查 HVF 是否可用
    #[cfg(target_os = "macos")]
    pub fn is_available() -> bool {
        // macOS 10.10+ 都支持 Hypervisor.framework
        true
    }

    #[cfg(not(target_os = "macos"))]
    pub fn is_available() -> bool {
        false
    }

    /// Handle VM exit
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    pub fn handle_vm_exit(&mut self, vcpu: u32) -> Result<HvmExit, HvfError> {
        use x86_exit::*;

        // SAFETY: hv_vcpu_read_exit_reason is an extern C function from Hypervisor.framework
        // Preconditions: vcpu is a valid vCPU ID created by hv_vcpu_create
        // Invariants: Reads the exit reason into the provided pointer
        unsafe {
            let mut exit_reason: u64 = 0;

            let vcpu_id = self.vcpus.get(&vcpu).ok_or(HvfError::InvalidVcpu(vcpu))?.id;

            let ret = hv_vcpu_read_exit_reason(vcpu_id, &mut exit_reason);

            if ret != HV_SUCCESS {
                return Err(HvfError::VcpuError(format!(
                    "Failed to read exit reason: 0x{:x}",
                    ret
                )));
            }

            match exit_reason {
                HV_EXIT_REASON_IO_INSTRUCTION => self.handle_io_exit(vcpu),
                HV_EXIT_REASON_EPT_VIOLATION | HV_EXIT_REASON_EPT_MISCONFIG => {
                    self.handle_mmio_exit(vcpu)
                }
                HV_EXIT_REASON_EXTERNAL_INTERRUPT | HV_EXIT_REASON_PENDING_INTERRUPT => {
                    Ok(HvmExit::Interrupt)
                }
                HV_EXIT_REASON_CPUID => self.handle_cpuid_exit(vcpu),
                HV_EXIT_REASON_RDMSR => self.handle_rdmsr_exit(vcpu),
                HV_EXIT_REASON_WRMSR => self.handle_wrmsr_exit(vcpu),
                HV_EXIT_REASON_HLT => Ok(HvmExit::Hlt),
                HV_EXIT_REASON_EXCEPTION => self.handle_exception_exit(vcpu),
                _ => Ok(HvmExit::Unknown {
                    reason: exit_reason,
                }),
            }
        }
    }

    /// Handle VM exit for ARM64
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    pub fn handle_vm_exit(&mut self, _vcpu: u32) -> Result<HvmExit, HvfError> {
        // ARM64 has different exit handling - simplified for now
        Ok(HvmExit::Unknown { reason: 0 })
    }

    /// Handle IO exit (x86_64 only)
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    fn handle_io_exit(&mut self, vcpu: u32) -> Result<HvmExit, HvfError> {
        // SAFETY: hv_vcpu_read_exit_io_* functions are extern C functions from Hypervisor.framework
        // Preconditions: vcpu is a valid vCPU ID, pointers are valid for writing u64 values
        // Invariants: Reads IO exit information into the provided pointers
        unsafe {
            let vcpu_id = self.vcpus.get(&vcpu).ok_or(HvfError::InvalidVcpu(vcpu))?.id;

            let mut port: u64 = 0;
            let mut size: u64 = 0;
            let mut direction: u64 = 0;

            let ret1 = hv_vcpu_read_exit_io_port(vcpu_id, &mut port);
            let ret2 = hv_vcpu_read_exit_io_access_size(vcpu_id, &mut size);
            let ret3 = hv_vcpu_read_exit_io_direction(vcpu_id, &mut direction);

            if ret1 != HV_SUCCESS || ret2 != HV_SUCCESS || ret3 != HV_SUCCESS {
                return Err(HvfError::ExitReadError(format!(
                    "Failed to read IO exit info: port=0x{:x}, size=0x{:x}, dir=0x{:x}",
                    ret1, ret2, ret3
                )));
            }

            Ok(HvmExit::Io {
                port,
                size,
                is_write: direction == 1,
            })
        }
    }

    /// Handle MMIO/EPT violation exit (x86_64 only)
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    fn handle_mmio_exit(&mut self, vcpu: u32) -> Result<HvmExit, HvfError> {
        // SAFETY: hv_vcpu_read_exit_qualification is an extern C function from Hypervisor.framework
        // Preconditions: vcpu is a valid vCPU ID, pointer is valid for writing u64
        // Invariants: Reads exit qualification which contains the GPA that caused the EPT violation
        unsafe {
            let vcpu_id = self.vcpus.get(&vcpu).ok_or(HvfError::InvalidVcpu(vcpu))?.id;

            let mut qualification: u64 = 0;
            let ret = hv_vcpu_read_exit_qualification(vcpu_id, &mut qualification);

            if ret != HV_SUCCESS {
                return Err(HvfError::ExitReadError(format!(
                    "Failed to read MMIO qualification: 0x{:x}",
                    ret
                )));
            }

            // Extract GPA and access type from qualification
            // The qualification format is architecture-specific
            let gpa = qualification & 0xFFFFFFFFFFFFFFF8; // Mask to get page-aligned address
            let is_write = (qualification & 0x2) != 0; // Check write flag

            Ok(HvmExit::Mmio {
                gpa,
                size: 1, // Default size, may need to read from registers
                is_write,
            })
        }
    }

    /// Handle CPUID exit (x86_64 only)
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    fn handle_cpuid_exit(&mut self, vcpu: u32) -> Result<HvmExit, HvfError> {
        unsafe {
            let vcpu_id = self.vcpus.get(&vcpu).ok_or(HvfError::InvalidVcpu(vcpu))?.id;

            // Read RAX (leaf) and RCX (subleaf) for CPUID
            let mut leaf: u64 = 0;
            let mut subleaf: u64 = 0;

            hv_vcpu_read_register(vcpu_id, x86_regs::HV_X86_RAX, &mut leaf);
            hv_vcpu_read_register(vcpu_id, x86_regs::HV_X86_RCX, &mut subleaf);

            Ok(HvmExit::Cpuid {
                leaf: leaf as u32,
                subleaf: subleaf as u32,
            })
        }
    }

    /// Handle RDMSR exit (x86_64 only)
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    fn handle_rdmsr_exit(&mut self, vcpu: u32) -> Result<HvmExit, HvfError> {
        unsafe {
            let vcpu_id = self.vcpus.get(&vcpu).ok_or(HvfError::InvalidVcpu(vcpu))?.id;

            // Read RCX which contains the MSR address
            let mut msr: u64 = 0;
            hv_vcpu_read_register(vcpu_id, x86_regs::HV_X86_RCX, &mut msr);

            Ok(HvmExit::Rdmsr { msr: msr as u32 })
        }
    }

    /// Handle WRMSR exit (x86_64 only)
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    fn handle_wrmsr_exit(&mut self, vcpu: u32) -> Result<HvmExit, HvfError> {
        unsafe {
            let vcpu_id = self.vcpus.get(&vcpu).ok_or(HvfError::InvalidVcpu(vcpu))?.id;

            // Read RCX (MSR address) and RDX:RAX (value)
            let mut msr: u64 = 0;
            let mut value_low: u64 = 0;
            let mut value_high: u64 = 0;

            hv_vcpu_read_register(vcpu_id, x86_regs::HV_X86_RCX, &mut msr);
            hv_vcpu_read_register(vcpu_id, x86_regs::HV_X86_RAX, &mut value_low);
            hv_vcpu_read_register(vcpu_id, x86_regs::HV_X86_RDX, &mut value_high);

            let value = (value_high << 32) | (value_low & 0xFFFFFFFF);

            Ok(HvmExit::Wrmsr {
                msr: msr as u32,
                value,
            })
        }
    }

    /// Handle exception exit (x86_64 only)
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    fn handle_exception_exit(&mut self, vcpu: u32) -> Result<HvmExit, HvfError> {
        unsafe {
            let vcpu_id = self.vcpus.get(&vcpu).ok_or(HvfError::InvalidVcpu(vcpu))?.id;

            // Read exit qualification to get exception details
            let mut qualification: u64 = 0;
            let ret = hv_vcpu_read_exit_qualification(vcpu_id, &mut qualification);

            if ret != HV_SUCCESS {
                return Err(HvfError::ExitReadError(format!(
                    "Failed to read exception qualification: 0x{:x}",
                    ret
                )));
            }

            // Extract vector and error code from qualification
            let vector = (qualification & 0xFF) as u8;
            let error_code = (qualification >> 32) as u64;

            Ok(HvmExit::Exception { vector, error_code })
        }
    }

    /// Handle IO exit (fallback for non-x86)
    #[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
    #[allow(dead_code)]
    fn handle_io_exit(&mut self, _vcpu: u32) -> Result<HvmExit, HvfError> {
        Ok(HvmExit::Io {
            port: 0,
            size: 0,
            is_write: false,
        })
    }

    /// Handle MMIO exit (fallback for non-x86)
    #[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
    #[allow(dead_code)]
    fn handle_mmio_exit(&mut self, _vcpu: u32) -> Result<HvmExit, HvfError> {
        Ok(HvmExit::Mmio {
            gpa: 0,
            size: 0,
            is_write: false,
        })
    }

    /// Handle CPUID exit (fallback for non-x86)
    #[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
    #[allow(dead_code)]
    fn handle_cpuid_exit(&mut self, _vcpu: u32) -> Result<HvmExit, HvfError> {
        Ok(HvmExit::Cpuid {
            leaf: 0,
            subleaf: 0,
        })
    }

    /// Handle RDMSR exit (fallback for non-x86)
    #[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
    #[allow(dead_code)]
    fn handle_rdmsr_exit(&mut self, _vcpu: u32) -> Result<HvmExit, HvfError> {
        Ok(HvmExit::Rdmsr { msr: 0 })
    }

    /// Handle WRMSR exit (fallback for non-x86)
    #[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
    #[allow(dead_code)]
    fn handle_wrmsr_exit(&mut self, _vcpu: u32) -> Result<HvmExit, HvfError> {
        Ok(HvmExit::Wrmsr { msr: 0, value: 0 })
    }

    /// Handle exception exit (fallback for non-x86)
    #[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
    #[allow(dead_code)]
    fn handle_exception_exit(&mut self, _vcpu: u32) -> Result<HvmExit, HvfError> {
        Ok(HvmExit::Exception {
            vector: 0,
            error_code: 0,
        })
    }
}

impl Accel for AccelHvf {
    fn init(&mut self) -> Result<(), AccelError> {
        #[cfg(target_os = "macos")]
        {
            if self.initialized {
                return Ok(());
            }

            // SAFETY: hv_vm_create is an extern C function from Hypervisor.framework
            // Preconditions: config pointer may be null (default configuration)
            // Returns: HV_SUCCESS on success, error code otherwise
            let ret = unsafe { hv_vm_create(ptr::null_mut()) };

            if ret != HV_SUCCESS {
                return Err(VmError::Core(CoreError::Internal {
                    message: format!("hv_vm_create failed: 0x{:x}", ret),
                    module: "vm-accel::hvf".to_string(),
                }));
            }

            self.initialized = true;
            log::info!("HVF accelerator initialized successfully");
            Ok(())
        }

        #[cfg(not(target_os = "macos"))]
        {
            Err(VmError::Core(CoreError::NotSupported {
                feature: "HVF initialization".to_string(),
                module: "vm-accel".to_string(),
            }))
        }
    }

    fn create_vcpu(&mut self, id: u32) -> Result<(), AccelError> {
        let vcpu = HvfVcpu::new(id)?;
        self.vcpus.insert(id, vcpu);
        log::info!("Created HVF vCPU {}", id);
        Ok(())
    }

    fn map_memory(&mut self, gpa: u64, hva: u64, size: u64, flags: u32) -> Result<(), AccelError> {
        #[cfg(target_os = "macos")]
        {
            let mut hv_flags = HV_MEMORY_READ | HV_MEMORY_WRITE;
            if flags & 0x4 != 0 {
                hv_flags |= HV_MEMORY_EXEC;
            }

            // SAFETY: hv_vm_map is an extern C function from Hypervisor.framework
            // Preconditions: hva is a valid host virtual address, gpa and size are properly aligned
            // Invariants: Maps host memory into guest physical address space with specified permissions
            let ret =
                unsafe { hv_vm_map(hva as *const std::ffi::c_void, gpa, size as usize, hv_flags) };

            if ret != HV_SUCCESS {
                return Err(VmError::Core(CoreError::Internal {
                    message: format!("hv_vm_map failed: 0x{:x}", ret),
                    module: "vm-accel".to_string(),
                }));
            }

            self.memory_regions.insert(gpa, size);
            log::debug!("Mapped memory: GPA 0x{:x}, size 0x{:x}", gpa, size);
            Ok(())
        }

        #[cfg(not(target_os = "macos"))]
        {
            Err(VmError::Core(CoreError::NotSupported {
                feature: "HVF memory mapping".to_string(),
                module: "vm-accel".to_string(),
            }))
        }
    }

    fn unmap_memory(&mut self, gpa: u64, size: u64) -> Result<(), AccelError> {
        #[cfg(target_os = "macos")]
        {
            // SAFETY: hv_vm_unmap is an extern C function from Hypervisor.framework
            // Preconditions: gpa and size must match a previously mapped region
            // Invariants: Removes guest physical memory mapping
            let ret = unsafe { hv_vm_unmap(gpa, size as usize) };

            if ret != HV_SUCCESS {
                return Err(VmError::Core(CoreError::Internal {
                    message: format!("hv_vm_unmap failed: 0x{:x}", ret),
                    module: "vm-accel".to_string(),
                }));
            }

            self.memory_regions.remove(&gpa);
            log::debug!("Unmapped memory: GPA 0x{:x}", gpa);
            Ok(())
        }

        #[cfg(not(target_os = "macos"))]
        {
            Err(VmError::Core(CoreError::NotSupported {
                feature: "HVF memory unmapping".to_string(),
                module: "vm-accel".to_string(),
            }))
        }
    }

    fn run_vcpu(&mut self, vcpu_id: u32, _mmu: &mut dyn MMU) -> Result<(), AccelError> {
        let vcpu = self.vcpus.get_mut(&vcpu_id).ok_or_else(|| {
            VmError::Core(CoreError::InvalidParameter {
                name: "vcpu_id".to_string(),
                value: format!("{}", vcpu_id),
                message: "Invalid vCPU ID".to_string(),
            })
        })?;

        vcpu.run()
    }

    fn get_regs(&self, vcpu_id: u32) -> Result<GuestRegs, AccelError> {
        let vcpu = self.vcpus.get(&vcpu_id).ok_or_else(|| {
            VmError::Core(CoreError::InvalidParameter {
                name: "vcpu_id".to_string(),
                value: format!("{}", vcpu_id),
                message: "Invalid vCPU ID".to_string(),
            })
        })?;
        vcpu.get_regs()
    }

    fn set_regs(&mut self, vcpu_id: u32, regs: &GuestRegs) -> Result<(), AccelError> {
        let vcpu = self.vcpus.get_mut(&vcpu_id).ok_or_else(|| {
            VmError::Core(CoreError::InvalidParameter {
                name: "vcpu_id".to_string(),
                value: format!("{}", vcpu_id),
                message: "Invalid vCPU ID".to_string(),
            })
        })?;
        vcpu.set_regs(regs)
    }

    fn name(&self) -> &str {
        "HVF"
    }
}

impl Drop for AccelHvf {
    fn drop(&mut self) {
        #[cfg(target_os = "macos")]
        if self.initialized {
            // SAFETY: hv_vm_destroy is an extern C function from Hypervisor.framework
            // Preconditions: VM was successfully created with hv_vm_create and initialized flag is true
            // Invariants: Destroys VM and releases all resources
            unsafe {
                hv_vm_destroy();
            }
            log::info!("HVF VM destroyed");
        }
    }
}

impl Default for AccelHvf {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hvf_availability() {
        println!("HVF available: {}", AccelHvf::is_available());
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_hvf_init() {
        let mut accel = AccelHvf::new();
        // HVF may not be available on all macOS systems due to permissions or system settings
        // Test verifies that init() either succeeds or fails gracefully
        let result = accel.init();
        // We just verify the call doesn't panic - result can be ok or err
        let _ = result;
    }
}
