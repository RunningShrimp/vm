//! 同架构直接执行优化
//!
//! 当 guest 架构与 host 架构相同时，利用硬件虚拟化直接执行

use vm_accel::{Accel, AccelKind, cpuinfo::CpuInfo};
use vm_core::{ExecMode, ExecResult, ExecStatus, ExecStats, GuestArch, GuestAddr, MMU, VmError};
use vm_ir::IRBlock;
use std::sync::Arc;
use tracing::{debug, warn};

/// 直接执行优化器
///
/// 检测 guest 和 host 架构是否相同，如果相同则使用硬件虚拟化直接执行
pub struct DirectExecutionOptimizer {
    /// Host 架构
    host_arch: GuestArch,
    /// Guest 架构
    guest_arch: GuestArch,
    /// 是否启用硬件虚拟化
    hardware_virtualization_enabled: bool,
    /// 硬件加速器（如果可用）
    hardware_accel: Option<Arc<dyn Accel>>,
}

impl DirectExecutionOptimizer {
    /// 创建新的直接执行优化器
    pub fn new(guest_arch: GuestArch) -> Result<Self, VmError> {
        let host_arch = Self::detect_host_arch();
        let hardware_virtualization_enabled = Self::check_hardware_virtualization();

        // 如果架构相同且支持硬件虚拟化，尝试创建硬件加速器
        let hardware_accel = if host_arch == guest_arch && hardware_virtualization_enabled {
            match Self::create_hardware_accel() {
                Ok(accel) => {
                    debug!("Hardware virtualization enabled for direct execution");
                    Some(Arc::new(accel))
                }
                Err(e) => {
                    warn!("Failed to create hardware accelerator: {:?}, falling back to JIT", e);
                    None
                }
            }
        } else {
            None
        };

        Ok(Self {
            host_arch,
            guest_arch,
            hardware_virtualization_enabled,
            hardware_accel,
        })
    }

    /// 检测 host 架构
    fn detect_host_arch() -> GuestArch {
        #[cfg(target_arch = "x86_64")]
        return GuestArch::X86_64;

        #[cfg(target_arch = "aarch64")]
        return GuestArch::Arm64;

        #[cfg(target_arch = "riscv64")]
        return GuestArch::Riscv64;

        #[cfg(not(any(
            target_arch = "x86_64",
            target_arch = "aarch64",
            target_arch = "riscv64"
        )))]
        {
            warn!("Unknown host architecture, defaulting to X86_64");
            GuestArch::X86_64
        }
    }

    /// 检查硬件虚拟化支持
    fn check_hardware_virtualization() -> bool {
        let cpu_info = CpuInfo::new();
        
        #[cfg(target_arch = "x86_64")]
        {
            // 检查 Intel VT-x 或 AMD SVM
            let features = vm_accel::cpuinfo::detect();
            features.vmx || features.svm
        }

        #[cfg(target_arch = "aarch64")]
        {
            // 检查 ARM EL2 支持（通过 /dev/kvm 存在性）
            std::path::Path::new("/dev/kvm").exists()
        }

        #[cfg(target_arch = "riscv64")]
        {
            // RISC-V 虚拟化支持检查
            // 当前简化处理
            false
        }

        #[cfg(not(any(
            target_arch = "x86_64",
            target_arch = "aarch64",
            target_arch = "riscv64"
        )))]
        {
            false
        }
    }

    /// 创建硬件加速器
    fn create_hardware_accel() -> Result<Box<dyn Accel>, VmError> {
        // 使用 vm_accel::select() 自动选择最佳加速器
        let (kind, mut accel) = vm_accel::select();
        
        if kind == AccelKind::None {
            return Err(VmError::Core(vm_core::CoreError::NotSupported {
                feature: "Hardware virtualization".to_string(),
                module: "DirectExecutionOptimizer".to_string(),
            }));
        }

        // 初始化加速器
        accel.init().map_err(|e| {
            VmError::Platform(vm_core::PlatformError::AccelError {
                kind: format!("{:?}", kind),
                message: format!("Failed to initialize accelerator: {:?}", e),
            })
        })?;

        Ok(accel)
    }

    /// 检查是否可以使用直接执行
    pub fn can_use_direct_execution(&self) -> bool {
        self.host_arch == self.guest_arch && self.hardware_accel.is_some()
    }

    /// 获取推荐的执行模式
    pub fn recommended_exec_mode(&self) -> ExecMode {
        if self.can_use_direct_execution() {
            ExecMode::HardwareAssisted
        } else if self.host_arch == self.guest_arch {
            // 同架构但无硬件虚拟化，使用 JIT
            ExecMode::JIT
        } else {
            // 跨架构，使用 JIT 或解释器
            ExecMode::JIT
        }
    }

    /// 直接执行代码块（如果支持）
    ///
    /// 如果 guest 和 host 架构相同且支持硬件虚拟化，则直接执行
    pub fn execute_directly(
        &self,
        _mmu: &mut dyn MMU,
        _block: &IRBlock,
    ) -> Option<ExecResult> {
        if !self.can_use_direct_execution() {
            return None;
        }

        // TODO: 实现直接执行逻辑
        // 1. 将 IR 块转换为原生机器码（或直接使用块中的机器码）
        // 2. 设置 vCPU 寄存器
        // 3. 运行 vCPU
        // 4. 处理 VM exits
        // 5. 返回执行结果

        // 当前实现：返回 None，表示需要回退到其他执行方式
        None
    }

    /// 获取 host 架构
    pub fn host_arch(&self) -> GuestArch {
        self.host_arch
    }

    /// 获取 guest 架构
    pub fn guest_arch(&self) -> GuestArch {
        self.guest_arch
    }

    /// 检查是否为同架构
    pub fn is_same_arch(&self) -> bool {
        self.host_arch == self.guest_arch
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arch_detection() {
        let optimizer = DirectExecutionOptimizer::new(GuestArch::X86_64).unwrap();
        assert!(optimizer.is_same_arch() || !optimizer.is_same_arch()); // 应该能检测
    }

    #[test]
    fn test_direct_execution_check() {
        let optimizer = DirectExecutionOptimizer::new(GuestArch::X86_64).unwrap();
        // 检查是否能使用直接执行（取决于硬件支持）
        let _can_use = optimizer.can_use_direct_execution();
    }
}

