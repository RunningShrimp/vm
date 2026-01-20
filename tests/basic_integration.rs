// Basic integration tests for VM project
//
// Tests basic functionality without complex dependencies

#[test]
fn test_guestaddr_creation() {
    use vm_core::GuestAddr;

    let addr = GuestAddr(0x1000);
    assert_eq!(addr.0, 0x1000);
}

#[test]
fn test_guestaddr_arithmetic() {
    use vm_core::GuestAddr;

    let addr1 = GuestAddr(0x1000);
    let addr2 = GuestAddr(addr1.0 + 0x100);
    assert_eq!(addr2.0, 0x1100);
}

#[test]
fn test_vmconfig_default() {
    use vm_core::VmConfig;

    let config = VmConfig::default();
    assert_eq!(config.memory_size, 128 * 1024 * 1024); // 128 MB
    assert_eq!(config.vcpu_count, 1);
}

#[test]
fn test_execmode_display() {
    use vm_core::ExecMode;

    assert_eq!(format!("{:?}", ExecMode::Interpreter), "Interpreter");
    assert_eq!(format!("{:?}", ExecMode::JIT), "JIT");
}

#[test]
fn test_guestarch_display() {
    use vm_core::GuestArch;

    assert_eq!(format!("{:?}", GuestArch::Riscv64), "Riscv64");
    assert_eq!(format!("{:?}", GuestArch::X86_64), "X86_64");
    assert_eq!(format!("{:?}", GuestArch::Arm64), "Arm64");
}
