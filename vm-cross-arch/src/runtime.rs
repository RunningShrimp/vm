//! 跨架构运行时支持
//!
//! 提供自动检测host架构、guest架构，并选择合适的执行策略

use super::Architecture;
use std::fmt;
use vm_core::{ExecMode, GuestArch, VmError};

/// Host架构检测结果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostArch {
    X86_64,
    ARM64,
    RISCV64,
    Unknown,
}

impl HostArch {
    /// 自动检测当前host架构
    pub fn detect() -> Self {
        #[cfg(target_arch = "x86_64")]
        return HostArch::X86_64;

        #[cfg(target_arch = "aarch64")]
        return HostArch::ARM64;

        #[cfg(target_arch = "riscv64")]
        return HostArch::RISCV64;

        #[cfg(not(any(
            target_arch = "x86_64",
            target_arch = "aarch64",
            target_arch = "riscv64"
        )))]
        return HostArch::Unknown;
    }

    /// 转换为Architecture枚举
    pub fn to_architecture(self) -> Option<Architecture> {
        match self {
            HostArch::X86_64 => Some(Architecture::X86_64),
            HostArch::ARM64 => Some(Architecture::ARM64),
            HostArch::RISCV64 => Some(Architecture::RISCV64),
            HostArch::Unknown => None,
        }
    }

    /// 获取架构名称
    pub fn name(self) -> &'static str {
        match self {
            HostArch::X86_64 => "x86_64",
            HostArch::ARM64 => "arm64",
            HostArch::RISCV64 => "riscv64",
            HostArch::Unknown => "unknown",
        }
    }
}

impl fmt::Display for HostArch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Guest架构到Architecture的转换
// Note: From implementations for GuestArch/Architecture are omitted due to orphan rule.
// Both types are defined in external crates (vm-core and vm-error).
// Conversion is handled inline where needed using match expressions.
/// 跨架构执行策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossArchStrategy {
    /// 同架构执行（可以使用硬件虚拟化加速）
    Native,
    /// 跨架构执行（需要二进制翻译）
    CrossArch,
    /// 不支持（架构不匹配且无法转换）
    Unsupported,
}

/// 跨架构执行配置
#[derive(Debug, Clone)]
pub struct CrossArchConfig {
    /// Host架构
    pub host_arch: HostArch,
    /// Guest架构
    pub guest_arch: GuestArch,
    /// 执行策略
    pub strategy: CrossArchStrategy,
    /// 是否启用硬件加速（仅同架构时可用）
    pub enable_hardware_accel: bool,
    /// 是否启用JIT编译（跨架构时推荐）
    pub enable_jit: bool,
    /// 是否启用解释器（作为fallback）
    pub enable_interpreter: bool,
}

impl CrossArchConfig {
    /// 自动检测并创建配置
    pub fn auto_detect(guest_arch: GuestArch) -> Result<Self, VmError> {
        let host_arch = HostArch::detect();

        if host_arch == HostArch::Unknown {
            return Err(VmError::Platform(vm_core::PlatformError::UnsupportedArch {
                arch: host_arch.name().to_string(),
                supported: vec![
                    "x86_64".to_string(),
                    "arm64".to_string(),
                    "riscv64".to_string(),
                ],
            }));
        }

        let guest_arch_enum: Architecture = match guest_arch {
            vm_core::GuestArch::X86_64 => Architecture::X86_64,
            vm_core::GuestArch::Arm64 => Architecture::ARM64,
            vm_core::GuestArch::Riscv64 => Architecture::RISCV64,
            vm_core::GuestArch::PowerPC64 => {
                return Err(VmError::Core(vm_core::CoreError::NotSupported {
                    feature: "PowerPC64 architecture".to_string(),
                    module: "runtime".to_string(),
                }));
            }
        };
        let host_arch_enum = host_arch.to_architecture().ok_or_else(|| {
            VmError::Core(vm_core::CoreError::NotSupported {
                feature: format!("Unknown host architecture: {:?}", host_arch),
                module: "CrossArchRuntime".to_string(),
            })
        })?;

        // 判断是否为跨架构
        let is_cross_arch = guest_arch_enum != host_arch_enum;

        let strategy = if is_cross_arch {
            CrossArchStrategy::CrossArch
        } else {
            CrossArchStrategy::Native
        };

        // 根据策略设置执行选项
        let enable_hardware_accel = !is_cross_arch; // 仅同架构支持硬件加速
        let enable_jit = true; // JIT总是可用（会自动编译到host架构）
        let enable_interpreter = true; // 解释器作为fallback

        Ok(Self {
            host_arch,
            guest_arch,
            strategy,
            enable_hardware_accel,
            enable_jit,
            enable_interpreter,
        })
    }

    /// 获取推荐的执行模式
    pub fn recommended_exec_mode(&self) -> ExecMode {
        match self.strategy {
            CrossArchStrategy::Native => {
                // 同架构：优先使用硬件加速或JIT
                if self.enable_hardware_accel {
                    ExecMode::HardwareAssisted
                } else {
                    ExecMode::JIT // JIT编译模式
                }
            }
            CrossArchStrategy::CrossArch => {
                // 跨架构：使用JIT编译模式
                ExecMode::JIT
            }
            CrossArchStrategy::Unsupported => {
                // 不支持：只能使用解释器
                ExecMode::Interpreter
            }
        }
    }

    /// 检查是否支持跨架构执行
    pub fn is_supported(&self) -> bool {
        self.strategy != CrossArchStrategy::Unsupported
    }

    /// 获取架构信息字符串
    pub fn arch_info(&self) -> String {
        format!(
            "Host: {} → Guest: {}",
            self.host_arch,
            self.guest_arch.name()
        )
    }
}

impl fmt::Display for CrossArchConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [Strategy: {:?}]", self.arch_info(), self.strategy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_host_arch_detection() {
        let host = HostArch::detect();
        assert_ne!(host, HostArch::Unknown);
        println!("Detected host architecture: {}", host);
    }

    #[test]
    fn test_cross_arch_config_native() {
        // 测试同架构配置（使用实际检测的host架构）
        let host = HostArch::detect();
        let guest = match host {
            HostArch::X86_64 => GuestArch::X86_64,
            HostArch::ARM64 => GuestArch::Arm64,
            HostArch::RISCV64 => GuestArch::Riscv64,
            HostArch::Unknown => panic!("Cannot run test on unknown host architecture"),
        };

        let config =
            CrossArchConfig::auto_detect(guest).expect("Failed to auto-detect cross-arch config");
        assert_eq!(config.strategy, CrossArchStrategy::Native);
        assert!(config.enable_hardware_accel);
    }

    #[test]
    fn test_cross_arch_config_cross() {
        // 测试跨架构配置
        let host = HostArch::detect();
        let guest = if host == HostArch::X86_64 {
            GuestArch::Arm64
        } else {
            GuestArch::X86_64
        };

        let config =
            CrossArchConfig::auto_detect(guest).expect("Failed to auto-detect cross-arch config");
        assert_eq!(config.strategy, CrossArchStrategy::CrossArch);
        assert!(!config.enable_hardware_accel);
    }
}
