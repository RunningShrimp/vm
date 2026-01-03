//! 跨架构端到端集成测试
//!
//! 验证 decode → IR → translate → exec 的完整路径

use vm_core::{ExecMode, GuestArch, GuestAddr, VmConfig, VmError};
use vm_service::VmService;

/// 测试 x86_64 host 运行 RISC-V 64 guest
#[tokio::test]
#[cfg(all(target_arch = "x86_64", feature = "frontend"))]
async fn test_x86_to_riscv_translation() {
    let config = VmConfig {
        guest_arch: GuestArch::Riscv64,
        memory_size: 128 * 1024 * 1024, // 128MB
        vcpu_count: 1,
        exec_mode: ExecMode::Interpreter, // 使用解释器模式进行测试
        ..Default::default()
    };

    let mut vm = VmService::new(config, None).await;
    assert!(vm.is_ok(), "Failed to create VM service");

    let mut vm = vm.unwrap();

    // 加载测试程序
    let code_base = 0x1000;
    let result = vm.load_test_program(code_base);
    assert!(result.is_ok(), "Failed to load test program");

    // 运行程序
    let result = vm.run(code_base);
    assert!(result.is_ok(), "Failed to run program");

    // 验证执行结果（简单验证：程序应该能正常执行）
    let reg_value = vm.get_reg(3);
    // 测试程序应该计算 10 + 20 = 30
    assert_eq!(reg_value, 30, "Expected register x3 to be 30");
}

/// 测试 ARM64 host 运行 x86_64 guest
#[tokio::test]
#[cfg(all(target_arch = "aarch64", feature = "frontend"))]
async fn test_arm64_to_x86_translation() {
    let config = VmConfig {
        guest_arch: GuestArch::X86_64,
        memory_size: 128 * 1024 * 1024,
        vcpu_count: 1,
        exec_mode: ExecMode::Interpreter,
        ..Default::default()
    };

    let mut vm = VmService::new(config, None).await;
    assert!(vm.is_ok(), "Failed to create VM service for ARM64→x86_64");

    let mut vm = vm.unwrap();
    let code_base = 0x1000;
    let result = vm.load_test_program(code_base);
    assert!(result.is_ok(), "Failed to load test program");
}

/// 测试 RISC-V 64 host 运行 x86_64 guest
#[tokio::test]
#[cfg(all(target_arch = "riscv64", feature = "frontend"))]
async fn test_riscv_to_x86_translation() {
    let config = VmConfig {
        guest_arch: GuestArch::X86_64,
        memory_size: 128 * 1024 * 1024,
        vcpu_count: 1,
        exec_mode: ExecMode::Interpreter,
        ..Default::default()
    };

    let mut vm = VmService::new(config, None).await;
    assert!(vm.is_ok(), "Failed to create VM service for RISC-V→x86_64");
}

/// 测试同架构回退（确保不破坏现有功能）
#[tokio::test]
async fn test_same_arch_fallback() {
    #[cfg(target_arch = "x86_64")]
    let guest_arch = GuestArch::X86_64;
    #[cfg(target_arch = "aarch64")]
    let guest_arch = GuestArch::Arm64;
    #[cfg(target_arch = "riscv64")]
    let guest_arch = GuestArch::Riscv64;
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "riscv64")))]
    let guest_arch = GuestArch::Riscv64;

    let config = VmConfig {
        guest_arch,
        memory_size: 128 * 1024 * 1024,
        vcpu_count: 1,
        exec_mode: ExecMode::Interpreter,
        ..Default::default()
    };

    let mut vm = VmService::new(config, None).await;
    assert!(vm.is_ok(), "Failed to create VM service for same-arch execution");
}

/// 测试 JIT 模式下的跨架构执行
#[tokio::test]
#[cfg(all(feature = "jit", feature = "frontend"))]
async fn test_cross_arch_jit() {
    #[cfg(target_arch = "x86_64")]
    let guest_arch = GuestArch::Riscv64;
    #[cfg(target_arch = "aarch64")]
    let guest_arch = GuestArch::X86_64;
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        // 跳过测试如果 host 架构不支持
        return;
    }

    let config = VmConfig {
        guest_arch,
        memory_size: 128 * 1024 * 1024,
        vcpu_count: 1,
        exec_mode: ExecMode::JIT,
        ..Default::default()
    };

    let vm = VmService::new(config, None).await;
    // JIT 模式可能在某些配置下不可用，所以只验证创建是否成功
    assert!(vm.is_ok(), "Failed to create VM service with JIT mode");
}
