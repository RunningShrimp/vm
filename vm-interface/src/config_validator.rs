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
    InvalidVcpuCount {
        count: usize,
        min: usize,
        max: usize,
    },
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
    pub min_vcpu_count: usize,
    /// 最大vCPU数量
    pub max_vcpu_count: usize,
    /// 支持的架构列表
    pub supported_architectures: Vec<GuestArch>,
}

impl Default for ConfigValidator {
    fn default() -> Self {
        Self {
            min_memory_size: 1024 * 1024,              // 1MB
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

        // 块设备镜像路径验证已移除，因为VmConfig中没有virtio字段

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
        // HardwareAssisted模式需要特定架构支持
        if matches!(mode, ExecMode::HardwareAssisted) {
            // 这里可以根据实际支持的架构进行调整
            match arch {
                GuestArch::Riscv64
                | GuestArch::Arm64
                | GuestArch::X86_64
                | GuestArch::PowerPC64 => {
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
    fn validate_config_consistency(&self, _config: &VmConfig) -> Result<(), ConfigValidationError> {
        // VmConfig中没有enable_accel和jit_threshold字段，所以简化验证
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

        // 测试有效内存大小
        let config = VmConfig {
            memory_size: 128 * 1024 * 1024, // 128MB
            ..Default::default()
        };
        assert!(validator.validate(&config).is_ok());

        // 测试内存太小
        let config = VmConfig {
            memory_size: 1024, // 1KB
            ..Default::default()
        };
        assert!(validator.validate(&config).is_err());

        // 测试内存太大
        let config = VmConfig {
            memory_size: 512 * 1024 * 1024 * 1024, // 512GB
            ..Default::default()
        };
        assert!(validator.validate(&config).is_err());
    }

    #[test]
    fn test_validate_vcpu_count() {
        let validator = ConfigValidator::default();

        // 测试有效vCPU数量
        let config = VmConfig {
            vcpu_count: 4,
            ..Default::default()
        };
        assert!(validator.validate(&config).is_ok());

        // 测试vCPU数量为0
        let config = VmConfig {
            vcpu_count: 0,
            ..Default::default()
        };
        assert!(validator.validate(&config).is_err());

        // 测试vCPU数量太大
        let config = VmConfig {
            vcpu_count: 1000,
            ..Default::default()
        };
        assert!(validator.validate(&config).is_err());
    }

    #[test]
    fn test_validate_and_fix() {
        let validator = ConfigValidator::default();

        // 设置无效的内存大小
        let config = VmConfig {
            memory_size: 512, // 太小
            ..Default::default()
        };
        let fixed_config = validator
            .validate_and_fix(config)
            .expect("validate_and_fix should succeed for fixable config");
        assert_eq!(fixed_config.memory_size, validator.min_memory_size);
    }
}
