//! macOS Hypervisor.framework FFI bindings
//!
//! This module contains all FFI declarations for macOS Hypervisor.framework,
//! providing direct access to the virtualization APIs without going through
//! external bindings crates.

#[cfg(target_os = "macos")]
#[link(name = "Hypervisor", kind = "framework")]
#[allow(dead_code)]
unsafe extern "C" {
    // VM Management Functions

    /// Create a new VM instance
    ///
    /// # Arguments
    ///
    /// * `config` - Pointer to VM configuration (can be null for defaults)
    ///
    /// # Returns
    ///
    /// HV_SUCCESS on success, error code on failure
    fn hv_vm_create(config: *mut std::ffi::c_void) -> i32;

    /// Destroy the current VM instance
    fn hv_vm_destroy() -> i32;

    /// Map a region of guest physical memory
    ///
    /// # Arguments
    ///
    /// * `uva` - User virtual address (host pointer)
    /// * `gpa` - Guest physical address
    /// * `size` - Size of the mapping in bytes
    /// * `flags` - Memory protection flags (read, write, execute)
    fn hv_vm_map(uva: *const std::ffi::c_void, gpa: u64, size: usize, flags: u64) -> i32;

    /// Unmap a region of guest physical memory
    fn hv_vm_unmap(gpa: u64, size: usize) -> i32;

    /// Change memory protection flags
    fn hv_vm_protect(gpa: u64, size: usize, flags: u64) -> i32;

    // vCPU Management Functions

    /// Create a new virtual CPU
    ///
    /// # Arguments
    ///
    /// * `vcpu` - Output parameter that receives the vCPU ID
    /// * `exit` - Pointer to exit info structure
    /// * `config` - Pointer to vCPU configuration
    fn hv_vcpu_create(
        vcpu: *mut u32,
        exit: *mut std::ffi::c_void,
        config: *mut std::ffi::c_void,
    ) -> i32;

    /// Destroy a virtual CPU
    fn hv_vcpu_destroy(vcpu: u32) -> i32;

    /// Run the virtual CPU until it exits
    fn hv_vcpu_run(vcpu: u32) -> i32;

    // x86_64 Register Access

    #[cfg(target_arch = "x86_64")]
    /// Read a vCPU register
    ///
    /// # Arguments
    ///
    /// * `vcpu` - vCPU ID
    /// * `reg` - Register number (HV_X86_REG_* constants)
    /// * `value` - Output parameter that receives the register value
    fn hv_vcpu_read_register(vcpu: u32, reg: u32, value: *mut u64) -> i32;

    #[cfg(target_arch = "x86_64")]
    /// Write a vCPU register
    ///
    /// # Arguments
    ///
    /// * `vcpu` - vCPU ID
    /// * `reg` - Register number (HV_X86_REG_* constants)
    /// * `value` - Value to write
    fn hv_vcpu_write_register(vcpu: u32, reg: u32, value: u64) -> i32;

    // ARM64 Register Access

    #[cfg(target_arch = "aarch64")]
    /// Get an ARM64 vCPU register
    fn hv_vcpu_get_reg(vcpu: u32, reg: u32, value: *mut u64) -> i32;

    #[cfg(target_arch = "aarch64")]
    /// Set an ARM64 vCPU register
    fn hv_vcpu_set_reg(vcpu: u32, reg: u32, value: u64) -> i32;

    // VM Exit Information (x86_64)

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

    // ARM64 Exit Information

    #[cfg(target_arch = "aarch64")]
    fn hv_vcpu_get_reg_state(vcpu: u32, reg: u32, value: *mut u64) -> i32;
}

// Return codes

#[cfg(target_os = "macos")]
pub const HV_SUCCESS: i32 = 0;
#[cfg(target_os = "macos")]
#[allow(dead_code)]
pub const HV_ERROR: i32 = 0xfae94001u32 as i32;
#[cfg(target_os = "macos")]
#[allow(dead_code)]
pub const HV_BUSY: i32 = 0xfae94002u32 as i32;
#[cfg(target_os = "macos")]
#[allow(dead_code)]
pub const HV_BAD_ARGUMENT: i32 = 0xfae94003u32 as i32;
#[cfg(target_os = "macos")]
#[allow(dead_code)]
pub const HV_NO_RESOURCES: i32 = 0xfae94005u32 as i32;
#[cfg(target_os = "macos")]
#[allow(dead_code)]
pub const HV_NO_DEVICE: i32 = 0xfae94006u32 as i32;
#[cfg(target_os = "macos")]
#[allow(dead_code)]
pub const HV_UNSUPPORTED: i32 = 0xfae9400fu32 as i32;

// Memory mapping flags

#[cfg(target_os = "macos")]
#[allow(dead_code)]
pub const HV_MEMORY_READ: u64 = 0x01;
#[cfg(target_os = "macos")]
#[allow(dead_code)]
pub const HV_MEMORY_WRITE: u64 = 0x02;
#[cfg(target_os = "macos")]
#[allow(dead_code)]
pub const HV_MEMORY_EXECUTE: u64 = 0x04;

// x86_64 register numbers

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
#[allow(dead_code)]
pub mod x86_regs {
    pub const HV_X86_RAX: u32 = 0;
    pub const HV_X86_RCX: u32 = 1;
    pub const HV_X86_RDX: u32 = 2;
    pub const HV_X86_RBX: u32 = 3;
    pub const HV_X86_RSP: u32 = 4;
    pub const HV_X86_RBP: u32 = 5;
    pub const HV_X86_RSI: u32 = 6;
    pub const HV_X86_RDI: u32 = 7;
    pub const HV_X86_R8: u32 = 8;
    pub const HV_X86_R9: u32 = 9;
    pub const HV_X86_R10: u32 = 10;
    pub const HV_X86_R11: u32 = 11;
    pub const HV_X86_R12: u32 = 12;
    pub const HV_X86_R13: u32 = 13;
    pub const HV_X86_R14: u32 = 14;
    pub const HV_X86_R15: u32 = 15;
    pub const HV_X86_RIP: u32 = 16;
    pub const HV_X86_RFLAGS: u32 = 17;
}

// x86_64 VM exit reasons

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
pub mod x86_exit {
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
    pub const HV_EXIT_REASONMWAIT: u64 = 36;
    pub const HV_EXIT_REASON_MTF: u64 = 37;
    pub const HV_EXIT_REASON_MONITOR: u64 = 39;
    pub const HV_EXIT_REASON_PAUSE: u64 = 40;
    pub const HV_EXIT_REASON_ENTRY_FAIL_MC: u64 = 41;
    pub const HV_EXIT_REASON_TPR_BELOW_THRESHOLD: u64 = 43;
    pub const HV_EXIT_REASON_APIC_ACCESS: u64 = 44;
    pub const HV_EXIT_REASON_VIRTUALIZED_EOI: u64 = 45;
    pub const HV_EXIT_REASON_WINDOW_INDICATORS: u64 = 46;
    pub const HV_EXIT_REASON_XSETBV: u64 = 55;
    pub const HV_EXIT_REASON_APIC_WRITE: u64 = 56;
    pub const HV_EXIT_REASON_INVPCID: u64 = 58;
    pub const HV_EXIT_REASON_PML_FULL: u64 = 62;
    pub const HV_EXIT_REASON_XSAVES: u64 = 63;
    pub const HV_EXIT_REASON_XRSTORS: u64 = 64;
}

// ARM64 register numbers

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[allow(dead_code)]
pub mod arm_regs {
    pub const HV_ARM_REG_X0: u32 = 0;
    pub const HV_ARM_REG_X1: u32 = 1;
    pub const HV_ARM_REG_X2: u32 = 2;
    pub const HV_ARM_REG_X3: u32 = 3;
    pub const HV_ARM_REG_X4: u32 = 4;
    pub const HV_ARM_REG_X5: u32 = 5;
    pub const HV_ARM_REG_X6: u32 = 6;
    pub const HV_ARM_REG_X7: u32 = 7;
    pub const HV_ARM_REG_X8: u32 = 8;
    pub const HV_ARM_REG_X9: u32 = 9;
    pub const HV_ARM_REG_X10: u32 = 10;
    pub const HV_ARM_REG_X11: u32 = 11;
    pub const HV_ARM_REG_X12: u32 = 12;
    pub const HV_ARM_REG_X13: u32 = 13;
    pub const HV_ARM_REG_X14: u32 = 14;
    pub const HV_ARM_REG_X15: u32 = 15;
    pub const HV_ARM_REG_X16: u32 = 16;
    pub const HV_ARM_REG_X17: u32 = 17;
    pub const HV_ARM_REG_X18: u32 = 18;
    pub const HV_ARM_REG_X19: u32 = 19;
    pub const HV_ARM_REG_X20: u32 = 20;
    pub const HV_ARM_REG_X21: u32 = 21;
    pub const HV_ARM_REG_X22: u32 = 22;
    pub const HV_ARM_REG_X23: u32 = 23;
    pub const HV_ARM_REG_X24: u32 = 24;
    pub const HV_ARM_REG_X25: u32 = 25;
    pub const HV_ARM_REG_X26: u32 = 26;
    pub const HV_ARM_REG_X27: u32 = 27;
    pub const HV_ARM_REG_X28: u32 = 28;
    pub const HV_ARM_REG_X29: u32 = 29; // FP
    pub const HV_ARM_REG_X30: u32 = 30; // LR
    pub const HV_ARM_REG_PC: u32 = 31;
    pub const HV_ARM_REG_SP: u32 = 32;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hvf_constants() {
        #[cfg(target_os = "macos")]
        {
            assert_eq!(HV_SUCCESS, 0);
        }
    }
}
