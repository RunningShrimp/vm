//! 执行编排器模块
//!
//! 根据 host 和 guest 架构选择最优的执行路径

use vm_core::{ExecMode, GuestArch};

/// 执行路径
///
/// 定义虚拟机执行的不同路径，根据 host 和 guest 架构自动选择
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionPath {
    /// 硬件加速路径（同架构）
    ///
    /// 当 host 和 guest 架构相同时，使用硬件虚拟化加速（KVM/HVF/WHPX）
    HardwareAccel,
    /// 翻译路径（跨架构）
    ///
    /// 当 host 和 guest 架构不同时，使用二进制翻译 + JIT/解释器
    Translation,
    /// 混合模式
    ///
    /// 结合硬件加速和翻译的混合执行模式
    Hybrid,
}

/// 执行编排器
///
/// 根据 host 架构、guest 架构和执行模式选择最优的执行路径
pub struct ExecutionOrchestrator {
    /// Host 架构
    host_arch: GuestArch,
    /// Guest 架构
    guest_arch: GuestArch,
    /// 执行模式
    exec_mode: ExecMode,
}

impl ExecutionOrchestrator {
    /// 创建新的执行编排器
    pub fn new(host_arch: GuestArch, guest_arch: GuestArch, exec_mode: ExecMode) -> Self {
        Self {
            host_arch,
            guest_arch,
            exec_mode,
        }
    }

    /// 检测当前 host 架构
    pub fn detect_host_arch() -> GuestArch {
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
            log::warn!("Unknown host architecture, defaulting to RISC-V 64");
            GuestArch::Riscv64
        }
    }

    /// 选择执行路径
    ///
    /// 根据 host 和 guest 架构的匹配情况选择最优执行路径
    pub fn select_execution_path(&self) -> ExecutionPath {
        match (self.host_arch, self.guest_arch) {
            // 同架构：优先使用硬件加速
            (a, b) if a == b => {
                match self.exec_mode {
                    ExecMode::HardwareAssisted => ExecutionPath::HardwareAccel,
                    ExecMode::JIT => ExecutionPath::Hybrid, // JIT + 硬件加速
                    ExecMode::Interpreter => ExecutionPath::Translation, // 解释器模式
                }
            }
            // 跨架构：必须使用翻译路径
            _ => ExecutionPath::Translation,
        }
    }

    /// 检查是否需要跨架构翻译
    pub fn requires_translation(&self) -> bool {
        self.host_arch != self.guest_arch
    }

    /// 检查是否可以使用硬件加速
    pub fn can_use_hardware_accel(&self) -> bool {
        self.host_arch == self.guest_arch && self.exec_mode == ExecMode::HardwareAssisted
    }

    /// 获取推荐的执行模式
    pub fn recommended_exec_mode(&self) -> ExecMode {
        if self.requires_translation() {
            // 跨架构：使用 JIT 或解释器
            ExecMode::JIT
        } else {
            // 同架构：可以使用硬件加速
            ExecMode::HardwareAssisted
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_same_arch_hardware_accel() {
        let orchestrator = ExecutionOrchestrator::new(
            GuestArch::X86_64,
            GuestArch::X86_64,
            ExecMode::HardwareAssisted,
        );
        assert_eq!(
            orchestrator.select_execution_path(),
            ExecutionPath::HardwareAccel
        );
        assert!(!orchestrator.requires_translation());
        assert!(orchestrator.can_use_hardware_accel());
    }

    #[test]
    fn test_cross_arch_translation() {
        let orchestrator =
            ExecutionOrchestrator::new(GuestArch::X86_64, GuestArch::Arm64, ExecMode::JIT);
        assert_eq!(
            orchestrator.select_execution_path(),
            ExecutionPath::Translation
        );
        assert!(orchestrator.requires_translation());
        assert!(!orchestrator.can_use_hardware_accel());
    }

    #[test]
    fn test_same_arch_jit_hybrid() {
        let orchestrator =
            ExecutionOrchestrator::new(GuestArch::Arm64, GuestArch::Arm64, ExecMode::JIT);
        assert_eq!(orchestrator.select_execution_path(), ExecutionPath::Hybrid);
        assert!(!orchestrator.requires_translation());
    }

    #[test]
    fn test_host_arch_detection() {
        let host_arch = ExecutionOrchestrator::detect_host_arch();
        // 应该检测到当前编译目标的架构
        assert!(matches!(
            host_arch,
            GuestArch::X86_64 | GuestArch::Arm64 | GuestArch::Riscv64
        ));
    }
}
