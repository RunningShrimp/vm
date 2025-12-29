//! Linux KVM 加速后端完整实现
//!
//! 支持 Intel VT-x, AMD-V 和 ARM 虚拟化扩展
//! 优化后的版本: 将架构特定代码分离到独立模块,减少feature gates

use super::{Accel, AccelError};
use std::collections::HashMap;
use vm_core::{GuestRegs, MMU, PlatformError, VmError};

// ============================================================================
// 辅助宏 - 减少条件编译重复
// ============================================================================

/// 宏：统一架构特定的 vCPU 操作委托
///
/// 这个宏消除了在每个方法中重复编写条件编译的需要。
/// 它自动为不同架构生成适当的 match 分支。
#[cfg(feature = "kvm")]
macro_rules! kvm_vcpu_delegate {
    // 无参数方法调用
    ($self:ident, $method:ident) => {
        match $self {
            #[cfg(target_arch = "x86_64")]
            Self::X86_64(v) => v.$method(),
            #[cfg(target_arch = "aarch64")]
            Self::Aarch64(v) => v.$method(),
        }
    };

    // 单参数方法调用
    ($self:ident, $method:ident, $arg:expr) => {
        match $self {
            #[cfg(target_arch = "x86_64")]
            Self::X86_64(v) => v.$method($arg),
            #[cfg(target_arch = "aarch64")]
            Self::Aarch64(v) => v.$method($arg),
        }
    };

    // 可变借用版本
    ($self:ident, $method:ident, mut $arg:ident, $type:ty) => {
        match $self {
            #[cfg(target_arch = "x86_64")]
            Self::X86_64(ref mut v) => v.$method($arg),
            #[cfg(target_arch = "aarch64")]
            Self::Aarch64(ref mut v) => v.$method($arg),
        }
    };
}

// ============================================================================
// 架构特定模块 - 使用模块级条件编译减少重复的feature gates
// ============================================================================

/// x86_64 架构特定实现
#[cfg(all(feature = "kvm", target_arch = "x86_64"))]
mod kvm_x86_64 {
    pub use kvm_bindings::*;
    pub use kvm_ioctls::{Kvm, VcpuExit, VcpuFd, VmFd};

    use super::*;

    /// x86_64 vCPU 实现
    pub struct KvmVcpuX86_64 {
        pub fd: VcpuFd,
        pub id: u32,
        pub run_mmap_size: usize,
    }

    impl KvmVcpuX86_64 {
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

        pub fn set_regs(&mut self, regs: &GuestRegs) -> Result<(), AccelError> {
            let kvm_regs = kvm_bindings::kvm_regs {
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
                rflags: 0x2, // Reserved
            };

            self.fd.set_regs(&kvm_regs).map_err(|e| {
                VmError::Platform(PlatformError::AccessDenied(format!(
                    "KVM set_regs failed: {}",
                    e
                )))
            })?;

            Ok(())
        }
    }
}

/// ARM64 架构特定实现
#[cfg(all(feature = "kvm", target_arch = "aarch64"))]
mod kvm_aarch64 {
    pub use kvm_bindings::*;
    pub use kvm_ioctls::{Kvm, VcpuExit, VcpuFd, VmFd};

    use super::*;

    /// ARM64 vCPU 实现
    pub struct KvmVcpuAarch64 {
        pub fd: VcpuFd,
        pub id: u32,
        pub run_mmap_size: usize,
    }

    impl KvmVcpuAarch64 {
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

        pub fn get_regs(&self) -> Result<GuestRegs, AccelError> {
            let regs = self.fd.get_regs().map_err(|e| {
                VmError::Platform(PlatformError::AccessDenied(format!(
                    "KVM get_regs failed: {}",
                    e
                )))
            })?;

            let mut gpr = [0u64; 32];
            // ARM64 寄存器映射
            gpr[0] = regs.regs[0];
            gpr[1] = regs.regs[1];
            gpr[2] = regs.regs[2];
            gpr[3] = regs.regs[3];
            gpr[4] = regs.regs[4];
            gpr[5] = regs.regs[5];
            gpr[6] = regs.regs[6];
            gpr[7] = regs.regs[7];
            gpr[8] = regs.regs[8];
            gpr[9] = regs.regs[9];
            gpr[10] = regs.regs[10];
            gpr[11] = regs.regs[11];
            gpr[12] = regs.regs[12];
            gpr[13] = regs.regs[13];
            gpr[14] = regs.regs[14];
            gpr[15] = regs.regs[15];
            gpr[16] = regs.regs[16];
            gpr[17] = regs.regs[17];
            gpr[18] = regs.regs[18];
            gpr[19] = regs.regs[19];
            gpr[20] = regs.regs[20];
            gpr[21] = regs.regs[21];
            gpr[22] = regs.regs[22];
            gpr[23] = regs.regs[23];
            gpr[24] = regs.regs[24];
            gpr[25] = regs.regs[25];
            gpr[26] = regs.regs[26];
            gpr[27] = regs.regs[27];
            gpr[28] = regs.regs[28];
            gpr[29] = regs.regs[29];
            gpr[30] = regs.regs[30];

            Ok(GuestRegs {
                pc: regs.regs[31], // PC
                sp: regs.sp,
                fp: regs.regs[29], // FP
                gpr,
            })
        }

        pub fn set_regs(&mut self, regs: &GuestRegs) -> Result<(), AccelError> {
            let mut kvm_regs = kvm_bindings::kvm_regs::default();

            // ARM64 寄存器反向映射
            kvm_regs.regs[0] = regs.gpr[0];
            kvm_regs.regs[1] = regs.gpr[1];
            kvm_regs.regs[2] = regs.gpr[2];
            kvm_regs.regs[3] = regs.gpr[3];
            kvm_regs.regs[4] = regs.gpr[4];
            kvm_regs.regs[5] = regs.gpr[5];
            kvm_regs.regs[6] = regs.gpr[6];
            kvm_regs.regs[7] = regs.gpr[7];
            kvm_regs.regs[8] = regs.gpr[8];
            kvm_regs.regs[9] = regs.gpr[9];
            kvm_regs.regs[10] = regs.gpr[10];
            kvm_regs.regs[11] = regs.gpr[11];
            kvm_regs.regs[12] = regs.gpr[12];
            kvm_regs.regs[13] = regs.gpr[13];
            kvm_regs.regs[14] = regs.gpr[14];
            kvm_regs.regs[15] = regs.gpr[15];
            kvm_regs.regs[16] = regs.gpr[16];
            kvm_regs.regs[17] = regs.gpr[17];
            kvm_regs.regs[18] = regs.gpr[18];
            kvm_regs.regs[19] = regs.gpr[19];
            kvm_regs.regs[20] = regs.gpr[20];
            kvm_regs.regs[21] = regs.gpr[21];
            kvm_regs.regs[22] = regs.gpr[22];
            kvm_regs.regs[23] = regs.gpr[23];
            kvm_regs.regs[24] = regs.gpr[24];
            kvm_regs.regs[25] = regs.gpr[25];
            kvm_regs.regs[26] = regs.gpr[26];
            kvm_regs.regs[27] = regs.gpr[27];
            kvm_regs.regs[28] = regs.gpr[28];
            kvm_regs.regs[29] = regs.gpr[29];
            kvm_regs.regs[30] = regs.gpr[30];
            kvm_regs.regs[31] = regs.pc;

            self.fd.set_regs(&kvm_regs).map_err(|e| {
                VmError::Platform(PlatformError::AccessDenied(format!(
                    "KVM set_regs failed: {}",
                    e
                )))
            })?;

            Ok(())
        }
    }
}

/// KVM 通用功能实现
#[cfg(feature = "kvm")]
mod kvm_common {
    pub use kvm_bindings::{KVM_MEM_LOG_DIRTY_PAGES, kvm_irq_level, kvm_userspace_memory_region};
    pub use kvm_ioctls::{Kvm, VmFd};

    use super::*;

    /// 统一的 vCPU 接口
    pub enum KvmVcpuUnified {
        #[cfg(target_arch = "x86_64")]
        X86_64(crate::kvm_impl::kvm_x86_64::KvmVcpuX86_64),
        #[cfg(target_arch = "aarch64")]
        Aarch64(crate::kvm_impl::kvm_aarch64::KvmVcpuAarch64),
    }

    impl KvmVcpuUnified {
        pub fn id(&self) -> u32 {
            kvm_vcpu_delegate!(self, id)
        }

        pub fn get_regs(&self) -> Result<GuestRegs, AccelError> {
            kvm_vcpu_delegate!(self, get_regs)
        }

        pub fn set_regs(&mut self, regs: &GuestRegs) -> Result<(), AccelError> {
            match self {
                #[cfg(target_arch = "x86_64")]
                Self::X86_64(v) => v.set_regs(regs),
                #[cfg(target_arch = "aarch64")]
                Self::Aarch64(v) => v.set_regs(regs),
            }
        }
    }
}

/// KVM 未启用时的存根实现
#[cfg(not(feature = "kvm"))]
mod kvm_stub {
    use super::*;

    /// 存根 vCPU
    pub struct KvmVcpuStub {
        pub id: u32,
    }

    impl KvmVcpuStub {
        pub fn new(_vm: &(), id: u32) -> Result<Self, AccelError> {
            Ok(Self { id })
        }

        pub fn get_regs(&self) -> Result<GuestRegs, AccelError> {
            Err(VmError::Platform(PlatformError::UnsupportedOperation(
                "KVM feature not enabled".to_string(),
            )))
        }

        pub fn set_regs(&mut self, _regs: &GuestRegs) -> Result<(), AccelError> {
            Err(VmError::Platform(PlatformError::UnsupportedOperation(
                "KVM feature not enabled".to_string(),
            )))
        }

        pub fn id(&self) -> u32 {
            self.id
        }
    }
}

// ============================================================================
// 重新导出统一接口
// ============================================================================

#[cfg(feature = "kvm")]
use kvm_common::{KVM_MEM_LOG_DIRTY_PAGES, kvm_irq_level, kvm_userspace_memory_region};

#[cfg(feature = "kvm")]
use kvm_ioctls::{Kvm, VcpuExit, VmFd};

/// 统一的 vCPU 抽象
enum KvmVcpu {
    #[cfg(feature = "kvm")]
    X86_64(kvm_x86_64::KvmVcpuX86_64),
    #[cfg(feature = "kvm")]
    Aarch64(kvm_aarch64::KvmVcpuAarch64),
    #[cfg(not(feature = "kvm"))]
    Stub(kvm_stub::KvmVcpuStub),
}
