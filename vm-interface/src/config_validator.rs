//! 配置验证框架
//!
//! 提供统一的配置验证机制，防止配置冲突和无效配置。

use vm_core::{CoreError, ExecMode, GuestArch, VmConfig, VmError};

/// 配置验证错误
#[derive(Debug, Clone)]
pub enum ConfigValidationError {
    /// 内存大小无效
    InvalidMemorySize { size: usize, min: usize, max: usize },
    /// vCPU数量无效
    InvalidVcpuCount { count: u32, min: u32, max: u32 },
    /// 架构不支持
    UnsupportedArchitecture { arch: GuestArch },
    /// 执行模式与架构不兼容
    IncompatibleExecMode { mode: ExecMode, arch: GuestArch },
    /// 配置冲突
    ConfigConflict {
        field1: String,
        field2: String,
        reason: String,
    },
    /// 必需字段缺失
    MissingRequiredField { field: String },
    /// 无效的文件路径
    InvalidFilePath { path: String, reason: String },
}

impl From<ConfigValidationError> for VmError {
    fn from(err: ConfigValidationError) -> Self {
        VmError::Core(CoreError::Config {
            message: format!("{:?}", err),
            path: None,
        })
    }
}

/// 配置验证器
pub struct ConfigValidator {
    /// 最小内存大小（字节）
    pub min_memory_size: usize,
    /// 最大内存大小（字节）
    pub max_memory_size: usize,
    /// 最小vCPU数量
    pub min_vcpu_count: u32,
    /// 最大vCPU数量
    pub max_vcpu_count: u32,
    /// 支持的架构列表
    pub supported_architectures: Vec<GuestArch>,
}

impl Default for ConfigValidator {
    fn default() -> Self {
        Self {
            min_memory_size: 1 * 1024 * 1024,          // 1MB
            max_memory_size: 256 * 1024 * 1024 * 1024, // 256GB
            min_vcpu_count: 1,
            max_vcpu_count: 256,
            supported_architectures: vec![GuestArch::Riscv64, GuestArch::Arm64, GuestArch::X86_64],
        }
    }
}

impl ConfigValidator {
    /// 创建新的配置验证器
    pub fn new() -> Self {
        Self::default()
    }

    /// 验证VM配置
    ///
    /// 返回Ok(())如果配置有效，否则返回ConfigValidationError
    pub fn validate(&self, config: &VmConfig) -> Result<(), ConfigValidationError> {
        // 验证内存大小
        if config.memory_size < self.min_memory_size {
            return Err(ConfigValidationError::InvalidMemorySize {
                size: config.memory_size,
                min: self.min_memory_size,
                max: self.max_memory_size,
            });
        }
        if config.memory_size > self.max_memory_size {
            return Err(ConfigValidationError::InvalidMemorySize {
                size: config.memory_size,
                min: self.min_memory_size,
                max: self.max_memory_size,
            });
        }

        // 验证vCPU数量
        if config.vcpu_count < self.min_vcpu_count {
            return Err(ConfigValidationError::InvalidVcpuCount {
                count: config.vcpu_count,
                min: self.min_vcpu_count,
                max: self.max_vcpu_count,
            });
        }
        if config.vcpu_count > self.max_vcpu_count {
            return Err(ConfigValidationError::InvalidVcpuCount {
                count: config.vcpu_count,
                min: self.min_vcpu_count,
                max: self.max_vcpu_count,
            });
        }

        // 验证架构支持
        if !self.supported_architectures.contains(&config.guest_arch) {
            return Err(ConfigValidationError::UnsupportedArchitecture {
                arch: config.guest_arch,
            });
        }

        // 验证执行模式与架构兼容性
        self.validate_exec_mode_compatibility(config.exec_mode, config.guest_arch)?;

        // 验证内核文件路径（如果提供）
        if let Some(ref kernel_path) = config.kernel_path {
            self.validate_file_path(kernel_path, "kernel_path")?;
        }

        // 验证initrd路径（如果提供）
        if let Some(ref initrd_path) = config.initrd_path {
            self.validate_file_path(initrd_path, "initrd_path")?;
        }

        // 验证块设备镜像路径（如果提供）
        if let Some(ref block_image) = config.virtio.block_image {
            self.validate_file_path(block_image, "block_image")?;
        }

        // 验证配置一致性
        self.validate_config_consistency(config)?;

        Ok(())
    }

    /// 验证执行模式与架构的兼容性
    fn validate_exec_mode_compatibility(
        &self,
        mode: ExecMode,
        arch: GuestArch,
    ) -> Result<(), ConfigValidationError> {
        // Accelerated模式需要特定架构支持
        if matches!(mode, ExecMode::Accelerated) {
            // 这里可以根据实际支持的架构进行调整
            match arch {
                GuestArch::Riscv64 | GuestArch::Arm64 | GuestArch::X86_64 => {
                    // 所有架构都支持加速模式
                }
            }
        }

        Ok(())
    }

    /// 验证文件路径
    fn validate_file_path(
        &self,
        path: &str,
        field_name: &str,
    ) -> Result<(), ConfigValidationError> {
        use std::path::Path;

        let path_obj = Path::new(path);

        // 检查路径是否为空
        if path.is_empty() {
            return Err(ConfigValidationError::InvalidFilePath {
                path: path.to_string(),
                reason: format!("{} cannot be empty", field_name),
            });
        }

        // 检查文件是否存在（如果路径是绝对路径）
        if path_obj.is_absolute() && !path_obj.exists() {
            return Err(ConfigValidationError::InvalidFilePath {
                path: path.to_string(),
                reason: format!("File does not exist: {}", path),
            });
        }

        Ok(())
    }

    /// 验证配置一致性
    fn validate_config_consistency(&self, config: &VmConfig) -> Result<(), ConfigValidationError> {
        // 检查：如果启用了硬件加速，但执行模式不是Accelerated
        if config.enable_accel && !matches!(config.exec_mode, ExecMode::Accelerated) {
            return Err(ConfigValidationError::ConfigConflict {
                field1: "enable_accel".to_string(),
                field2: "exec_mode".to_string(),
                reason: "Hardware acceleration enabled but exec_mode is not Accelerated"
                    .to_string(),
            });
        }

        // 检查：JIT阈值合理性
        if config.jit_threshold == 0 {
            return Err(ConfigValidationError::ConfigConflict {
                field1: "jit_threshold".to_string(),
                field2: "exec_mode".to_string(),
                reason: "JIT threshold cannot be zero".to_string(),
            });
        }

        Ok(())
    }

    /// 验证并修复配置
    ///
    /// 尝试修复无效配置，返回修复后的配置
    pub fn validate_and_fix(
        &self,
        mut config: VmConfig,
    ) -> Result<VmConfig, ConfigValidationError> {
        // 修复内存大小
        if config.memory_size < self.min_memory_size {
            config.memory_size = self.min_memory_size;
        } else if config.memory_size > self.max_memory_size {
            config.memory_size = self.max_memory_size;
        }

        // 修复vCPU数量
        if config.vcpu_count < self.min_vcpu_count {
            config.vcpu_count = self.min_vcpu_count;
        } else if config.vcpu_count > self.max_vcpu_count {
            config.vcpu_count = self.max_vcpu_count;
        }

        // 验证修复后的配置
        self.validate(&config)?;

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_memory_size() {
        let validator = ConfigValidator::default();
        let mut config = VmConfig::default();

        // 测试有效内存大小
        config.memory_size = 128 * 1024 * 1024; // 128MB
        assert!(validator.validate(&config).is_ok());

        // 测试内存太小
        config.memory_size = 1024; // 1KB
        assert!(validator.validate(&config).is_err());

        // 测试内存太大
        config.memory_size = 512 * 1024 * 1024 * 1024; // 512GB
        assert!(validator.validate(&config).is_err());
    }

    #[test]
    fn test_validate_vcpu_count() {
        let validator = ConfigValidator::default();
        let mut config = VmConfig::default();

        // 测试有效vCPU数量
        config.vcpu_count = 4;
        assert!(validator.validate(&config).is_ok());

        // 测试vCPU数量为0
        config.vcpu_count = 0;
        assert!(validator.validate(&config).is_err());

        // 测试vCPU数量太大
        config.vcpu_count = 1000;
        assert!(validator.validate(&config).is_err());
    }

    #[test]
    fn test_validate_and_fix() {
        let validator = ConfigValidator::default();
        let mut config = VmConfig::default();

        // 设置无效的内存大小
        config.memory_size = 512; // 太小
        let fixed_config = validator.validate_and_fix(config).unwrap();
        assert_eq!(fixed_config.memory_size, validator.min_memory_size);
    }
}

