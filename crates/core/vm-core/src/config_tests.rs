// Unit tests for VM configuration
//
// Tests for VmConfig, validation, and defaults

use super::*;
use std::path::PathBuf;

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================
    // VmConfig Tests
    // ============================================================

    #[test]
    fn test_vmconfig_default() {
        let config = VmConfig::default();

        // Check default values
        assert_eq!(config.memory_size, 128 * 1024 * 1024); // 128 MB
        assert_eq!(config.vcpu_count, 1);
        assert_eq!(config.exec_mode, ExecMode::Interpreter);
    }

    #[test]
    fn test_vmconfig_builder() {
        let config = VmConfig {
            guest_arch: GuestArch::Riscv64,
            memory_size: 512 * 1024 * 1024,
            vcpu_count: 2,
            exec_mode: ExecMode::JIT,
            ..Default::default()
        };

        assert_eq!(config.guest_arch, GuestArch::Riscv64);
        assert_eq!(config.memory_size, 512 * 1024 * 1024);
        assert_eq!(config.vcpu_count, 2);
        assert_eq!(config.exec_mode, ExecMode::JIT);
    }

    #[test]
    fn test_vmconfig_memory_validation() {
        // Valid memory sizes
        assert!(VmConfig::validate_memory_size(1024).is_ok());           // 1 KB
        assert!(VmConfig::validate_memory_size(1024 * 1024).is_ok());   // 1 MB
        assert!(VmConfig::validate_memory_size(1024 * 1024 * 1024).is_ok()); // 1 GB

        // Invalid memory sizes (too small)
        let result = VmConfig::validate_memory_size(0);
        assert!(result.is_err());

        // Invalid memory sizes (too large)
        let result = VmConfig::validate_memory_size(1024 * 1024 * 1024 * 1024); // 1 TB
        assert!(result.is_err());
    }

    #[test]
    fn test_vmconfig_vcpu_validation() {
        // Valid vCPU counts
        assert!(VmConfig::validate_vcpu_count(1).is_ok());
        assert!(VmConfig::validate_vcpu_count(2).is_ok());
        assert!(VmConfig::validate_vcpu_count(128).is_ok());

        // Invalid vCPU counts
        let result = VmConfig::validate_vcpu_count(0);
        assert!(result.is_err());

        let result = VmConfig::validate_vcpu_count(1000);
        assert!(result.is_err());
    }

    #[test]
    fn test_exec_mode_display() {
        assert_eq!(format!("{:?}", ExecMode::Interpreter), "Interpreter");
        assert_eq!(format!("{:?}", ExecMode::JIT), "JIT");
        assert_eq!(format!("{:?}", ExecMode::HardwareAssisted), "HardwareAssisted");
    }

    #[test]
    fn test_guest_arch_display() {
        assert_eq!(format!("{:?}", GuestArch::Riscv64), "Riscv64");
        assert_eq!(format!("{:?}", GuestArch::X86_64), "X86_64");
        assert_eq!(format!("{:?}", GuestArch::Arm64), "Arm64");
    }

    // ============================================================
    // Architecture Compatibility Tests
    // ============================================================

    #[test]
    fn test_arch_compatibility_riscv() {
        let config = VmConfig {
            guest_arch: GuestArch::Riscv64,
            ..Default::default()
        };

        // RISC-V should work with all exec modes
        assert!(config.is_valid_combination());
    }

    #[test]
    fn test_arch_compatibility_x86_64() {
        let config = VmConfig {
            guest_arch: GuestArch::X86_64,
            exec_mode: ExecMode::Interpreter,
            ..Default::default()
        };

        // x86_64 should work with interpreter
        assert!(config.is_valid_combination());
    }

    #[test]
    fn test_arch_compatibility_arm64() {
        let config = VmConfig {
            guest_arch: GuestArch::Arm64,
            exec_mode: ExecMode::Interpreter,
            ..Default::default()
        };

        // ARM64 should work with interpreter
        assert!(config.is_valid_combination());
    }
}
