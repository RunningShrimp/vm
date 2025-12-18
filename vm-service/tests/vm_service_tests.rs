//! VirtualMachineService 单元测试
//!
//! 验证DDD贫血模型合规性和服务层功能

use vm_core::{ExecMode, GuestArch, VmConfig, VmLifecycleState, vm_state::VirtualMachineState};
use vm_mem::SoftMmu;
use vm_service::vm_service::VirtualMachineService;

#[test]
fn test_vm_service_creation() {
    let config = VmConfig {
        guest_arch: GuestArch::Riscv64,
        memory_size: 128 * 1024 * 1024,
        vcpu_count: 1,
        exec_mode: ExecMode::Interpreter,
        ..Default::default()
    };

    let mmu = Box::new(SoftMmu::new(128 * 1024 * 1024, false));
    let service = VirtualMachineService::from_config(config, mmu);

    // 验证服务创建成功
    let state = service.state();
    let state_guard = state.lock().unwrap();
    assert_eq!(state_guard.state(), vm_core::VmLifecycleState::Created);
}

#[test]
fn test_vm_service_load_kernel() {
    let config = VmConfig::default();
    let mmu = Box::new(SoftMmu::new(128 * 1024 * 1024, false));
    let service = VirtualMachineService::from_config(config, mmu);

    let kernel_data = vec![0x13, 0x01, 0x00, 0x00]; // 示例RISC-V指令
    let result = service.load_kernel(&kernel_data, 0x1000);

    assert!(result.is_ok());
}

#[test]
fn test_vm_service_lifecycle() {
    let config = VmConfig::default();
    let mmu = Box::new(SoftMmu::new(128 * 1024 * 1024, false));
    let service = VirtualMachineService::from_config(config, mmu);

    // 测试启动
    assert!(service.start().is_ok());
    {
        let state = service.state();
        let state_guard = state.lock().unwrap();
        assert_eq!(state_guard.state(), vm_core::VmLifecycleState::Running);
    }

    // 测试暂停
    assert!(service.pause().is_ok());
    {
        let state = service.state();
        let state_guard = state.lock().unwrap();
        assert_eq!(state_guard.state(), vm_core::VmLifecycleState::Paused);
    }

    // 测试停止
    assert!(service.stop().is_ok());
    {
        let state = service.state();
        let state_guard = state.lock().unwrap();
        assert_eq!(state_guard.state(), vm_core::VmLifecycleState::Stopped);
    }
}

#[test]
fn test_vm_service_snapshot() {
    let config = VmConfig::default();
    let mmu = Box::new(SoftMmu::new(128 * 1024 * 1024, false));
    let service = VirtualMachineService::from_config(config, mmu);

    // 创建快照
    let snapshot_id = service
        .create_snapshot("test_snapshot".to_string(), "Test snapshot".to_string())
        .unwrap();

    assert!(!snapshot_id.is_empty());

    // 列出快照
    let snapshots = service.list_snapshots().unwrap();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].id, snapshot_id);
}

#[test]
fn test_vm_state_anemic_model() {
    // 验证VirtualMachineState是纯数据结构，不包含业务逻辑
    let config = VmConfig::default();
    let mmu = Box::new(SoftMmu::new(128 * 1024 * 1024, false));
    let state = VirtualMachineState::new(config, mmu);

    // 验证只有简单的访问方法
    assert_eq!(state.state(), vm_core::VmLifecycleState::Created);
    assert_eq!(state.vcpus.len(), 0);

    // 验证没有业务逻辑方法（通过编译检查）
    // 所有业务逻辑应该在VirtualMachineService中
}
