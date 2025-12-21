//! VM服务扩展
//!
//! 为VM服务提供跨架构自动检测和配置功能

use super::CrossArchConfig;
use vm_core::{ExecMode, GuestArch, VmConfig, VmError};

/// VM配置扩展
///
/// 提供自动检测和配置跨架构执行的功能
pub trait VmConfigExt {
    /// 自动检测host架构并调整配置
    fn auto_detect_cross_arch(&mut self) -> Result<CrossArchConfig, VmError>;

    /// 根据跨架构配置调整VM配置
    fn apply_cross_arch_config(&mut self, config: &CrossArchConfig);
}

impl VmConfigExt for VmConfig {
    /// 自动检测host架构并调整配置
    fn auto_detect_cross_arch(&mut self) -> Result<CrossArchConfig, VmError> {
        let cross_config = CrossArchConfig::auto_detect(self.guest_arch)?;

        // 自动调整VM配置
        self.apply_cross_arch_config(&cross_config);

        Ok(cross_config)
    }

    /// 根据跨架构配置调整VM配置
    fn apply_cross_arch_config(&mut self, config: &CrossArchConfig) {
        // 根据策略调整执行模式
        if self.exec_mode == ExecMode::HardwareAssisted
            && config.strategy != super::CrossArchStrategy::Native
        {
            // 跨架构不支持硬件加速，回退到JIT模式
            self.exec_mode = ExecMode::JIT;
        }

        // 根据当前模式和推荐模式进行调整（当需要时）
    }
}

/// 创建自动配置的VM配置
///
/// 根据guest架构自动检测host架构并创建合适的配置
pub fn create_auto_vm_config(
    guest_arch: GuestArch,
    memory_size: Option<usize>,
) -> Result<(VmConfig, CrossArchConfig), VmError> {
    let mut config = VmConfig {
        guest_arch,
        memory_size: memory_size.unwrap_or(128 * 1024 * 1024), // 默认128MB
        vcpu_count: 1,
        exec_mode: ExecMode::Interpreter, // 默认解释器模式
        ..Default::default()
    };

    // 自动检测并配置
    let cross_config = config.auto_detect_cross_arch()?;

    Ok((config, cross_config))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auto_vm_config() {
        let (config, cross_config) = create_auto_vm_config(GuestArch::X86_64, None).unwrap();

        // Verify configuration
        assert_eq!(config.guest_arch, GuestArch::X86_64);
        assert!(cross_config.is_supported());
    }
}
