//! 配置验证器测试

use vm_core::{ExecMode, GuestArch, VmConfig};
use vm_interface::config_validator::ConfigValidator;

#[test]
fn test_validate_valid_config() {
    let validator = ConfigValidator::default();
    let config = VmConfig {
        guest_arch: GuestArch::Riscv64,
        memory_size: 128 * 1024 * 1024,
        vcpu_count: 1,
        exec_mode: ExecMode::Interpreter,
        ..Default::default()
    };

    assert!(validator.validate(&config).is_ok());
}

#[test]
fn test_validate_invalid_memory_size() {
    let validator = ConfigValidator::default();

    // 内存太小
    let config = VmConfig {
        memory_size: 1024,
        ..Default::default()
    };
    assert!(validator.validate(&config).is_err());

    // 内存太大
    let config = VmConfig {
        memory_size: 512 * 1024 * 1024 * 1024,
        ..Default::default()
    };
    assert!(validator.validate(&config).is_err());
}

#[test]
fn test_validate_invalid_vcpu_count() {
    let validator = ConfigValidator::default();

    // vCPU数量为0
    let config = VmConfig {
        vcpu_count: 0,
        ..Default::default()
    };
    assert!(validator.validate(&config).is_err());

    // vCPU数量太大
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
