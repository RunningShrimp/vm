//! VirtualMachineService 单元测试
//!
//! 验证DDD贫血模型合规性和服务层功能

use vm_core::{ExecMode, GuestAddr, GuestArch, VmConfig, vm_state::VirtualMachineState};
use vm_mem::SoftMmu;
use vm_service::vm_service::VirtualMachineService;

#[test]
fn test_vm_service_creation() {
    use vm_ir::IRBlock;
    type B = IRBlock;

    let config = VmConfig {
        guest_arch: GuestArch::Riscv64,
        memory_size: 128 * 1024 * 1024,
        vcpu_count: 1,
        exec_mode: ExecMode::Interpreter,
        ..Default::default()
    };

    let mmu = Box::new(SoftMmu::new(128 * 1024 * 1024, false));
    let service: VirtualMachineService<B> = VirtualMachineService::from_config(config, mmu);

    // 验证服务创建成功
    let state = service.state();
    let state_guard = state.lock().unwrap();
    assert_eq!(state_guard.state(), vm_core::VmLifecycleState::Created);
}

#[test]
fn test_vm_service_load_kernel() {
    use vm_ir::IRBlock;
    type B = IRBlock;

    let config = VmConfig::default();
    let mmu = Box::new(SoftMmu::new(128 * 1024 * 1024, false));
    let service: VirtualMachineService<B> = VirtualMachineService::from_config(config, mmu);

    let kernel_data = vec![0x13, 0x01, 0x00, 0x00]; // 示例RISC-V指令
    let result = service.load_kernel(&kernel_data, GuestAddr(0x1000));

    assert!(result.is_ok());
}

#[test]
fn test_vm_service_lifecycle() {
    use vm_ir::IRBlock;
    type B = IRBlock;

    let config = VmConfig::default();
    let mmu = Box::new(SoftMmu::new(128 * 1024 * 1024, false));
    let service: VirtualMachineService<B> = VirtualMachineService::from_config(config, mmu);

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
    use vm_ir::IRBlock;
    type B = IRBlock;

    // 设置独立的快照目录，避免测试之间的干扰
    unsafe {
        std::env::set_var("VM_SNAPSHOT_DIR", "./test_snapshots");
    }

    // 清理旧的测试快照目录
    let _ = std::fs::remove_dir_all("./test_snapshots");

    let config = VmConfig::default();
    let mmu = Box::new(SoftMmu::new(128 * 1024 * 1024, false));
    let service: VirtualMachineService<B> = VirtualMachineService::from_config(config, mmu);

    // 创建快照
    let snapshot_id = service
        .create_snapshot("test_snapshot".to_string(), "Test snapshot".to_string())
        .unwrap();

    assert!(!snapshot_id.is_empty());

    // 列出快照
    let snapshots = service.list_snapshots().unwrap();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0], snapshot_id);

    // 清理测试快照目录
    let _ = std::fs::remove_dir_all("./test_snapshots");
}

#[test]
fn test_vm_state_anemic_model() {
    use vm_ir::IRBlock;
    type B = IRBlock;

    // 验证VirtualMachineState是纯数据结构，不包含业务逻辑
    let config = VmConfig::default();
    let mmu = Box::new(SoftMmu::new(128 * 1024 * 1024, false));
    let state: VirtualMachineState<B> = VirtualMachineState::new(config, mmu);

    // 验证只有简单的访问方法
    assert_eq!(state.state(), vm_core::VmLifecycleState::Created);
    assert_eq!(state.vcpus.len(), 0);

    // 验证没有业务逻辑方法（通过编译检查）
    // 所有业务逻辑应该在VirtualMachineService中
}

#[cfg(feature = "smmu")]
#[test]
fn test_smmu_initialization() {
    use vm_ir::IRBlock;
    type B = IRBlock;

    let config = VmConfig::default();
    let mmu = Box::new(SoftMmu::new(128 * 1024 * 1024, false));
    let mut service: VirtualMachineService<B> = VirtualMachineService::from_config(config, mmu);

    // 测试 SMMU 初始化
    let result = service.init_smmu();
    assert!(result.is_ok(), "SMMU initialization should succeed");
}

#[cfg(feature = "smmu")]
#[test]
fn test_smmu_device_attachment() {
    use vm_ir::IRBlock;
    type B = IRBlock;

    let config = VmConfig::default();
    let mmu = Box::new(SoftMmu::new(128 * 1024 * 1024, false));
    let mut service: VirtualMachineService<B> = VirtualMachineService::from_config(config, mmu);

    // 初始化 SMMU
    service.init_smmu().unwrap();

    // 测试设备分配
    let bdf = "0000:01:00.0";
    let dma_start = 0x1000_0000;
    let dma_size = 0x1000;

    let stream_id = service.attach_device_to_smmu(bdf, dma_start, dma_size);
    assert!(stream_id.is_ok(), "Device attachment should succeed");

    let id = stream_id.unwrap();
    assert!(id > 0, "Stream ID should be non-zero");

    // 测试列出设备
    let devices = service.list_smmu_devices();
    assert!(devices.is_ok(), "Listing devices should succeed");

    let device_list = devices.unwrap();
    assert!(!device_list.is_empty(), "Device list should not be empty");
    assert!(
        device_list.contains(&bdf.to_string()),
        "Device list should contain the attached device"
    );

    // 测试 DMA 地址转换 (使用 DMA 范围内的地址)
    let guest_addr = vm_core::GuestAddr(0x1000_0800); // 在 DMA 范围内
    let translated = service.translate_device_dma(bdf, guest_addr, 0x100);
    if let Err(e) = &translated {
        eprintln!("DMA translation failed: {:?}", e);
    }
    assert!(translated.is_ok(), "DMA translation should succeed");
}

#[cfg(feature = "smmu")]
#[test]
fn test_smmu_device_detachment() {
    use vm_ir::IRBlock;
    type B = IRBlock;

    let config = VmConfig::default();
    let mmu = Box::new(SoftMmu::new(128 * 1024 * 1024, false));
    let mut service: VirtualMachineService<B> = VirtualMachineService::from_config(config, mmu);

    // 初始化 SMMU
    service.init_smmu().unwrap();

    // 附加设备
    let bdf = "0000:02:00.0";
    let _stream_id = service
        .attach_device_to_smmu(bdf, 0x2000_0000, 0x2000)
        .unwrap();

    // 分离设备
    let result = service.detach_device_from_smmu(bdf);
    if let Err(e) = &result {
        eprintln!("Device detachment failed: {:?}", e);
    }
    assert!(result.is_ok(), "Device detachment should succeed");

    // 验证设备已从列表中移除
    let devices = service.list_smmu_devices().unwrap();
    assert!(
        !devices.contains(&bdf.to_string()),
        "Device should be removed from list"
    );
}

#[cfg(feature = "smmu")]
#[test]
fn test_smmu_not_initialized_error() {
    use vm_ir::IRBlock;
    type B = IRBlock;

    let config = VmConfig::default();
    let mmu = Box::new(SoftMmu::new(128 * 1024 * 1024, false));
    let service: VirtualMachineService<B> = VirtualMachineService::from_config(config, mmu);

    // 不初始化 SMMU，直接尝试使用应该返回错误
    let result = service.attach_device_to_smmu("0000:01:00.0", 0x1000, 0x1000);
    assert!(result.is_err(), "Should fail when SMMU is not initialized");

    let err = result.unwrap_err();
    // 验证错误类型
    assert!(
        matches!(err, vm_core::VmError::Core(_)),
        "Should return InvalidState error"
    );
}
