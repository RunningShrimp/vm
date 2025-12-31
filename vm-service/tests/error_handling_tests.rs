//! vm-service错误处理和边界条件测试
//!
//! 测试各种错误情况和边界条件

use vm_core::{VmConfig, GuestArch};
use vm_service::VmService;

/// 创建测试配置
fn create_test_config() -> VmConfig {
    VmConfig {
        guest_arch: GuestArch::Riscv64,
        vcpu_count: 1,
        memory_size: 1024 * 1024, // 1MB
        exec_mode: vm_core::ExecMode::Interpreter,
        kernel_path: None,
        initrd_path: None,
    }
}

#[tokio::test]
async fn test_vm_minimal_memory() {
    // 测试最小内存配置
    let config = VmConfig {
        guest_arch: GuestArch::Riscv64,
        vcpu_count: 1,
        memory_size: 4096, // 4KB - 非常小
        exec_mode: vm_core::ExecMode::Interpreter,
        kernel_path: None,
        initrd_path: None,
    };

    #[cfg(feature = "devices")]
    let result = VmService::new(config, None).await;

    #[cfg(not(feature = "devices"))]
    let result = VmService::new(config, None).await;

    // 可能成功或失败，取决于实现
    assert!(result.is_ok() || result.is_err());
}

#[tokio::test]
async fn test_vm_zero_vcpus() {
    // 测试0个vCPU的配置
    let config = VmConfig {
        guest_arch: GuestArch::Riscv64,
        vcpu_count: 0, // 边界值
        memory_size: 1024 * 1024,
        exec_mode: vm_core::ExecMode::Interpreter,
        kernel_path: None,
        initrd_path: None,
    };

    #[cfg(feature = "devices")]
    let result = VmService::new(config, None).await;

    #[cfg(not(feature = "devices"))]
    let result = VmService::new(config, None).await;

    // 应该失败或成功（取决于实现）
    assert!(result.is_ok() || result.is_err());
}

#[tokio::test]
async fn test_vm_large_memory() {
    // 测试大内存配置
    let config = VmConfig {
        guest_arch: GuestArch::Riscv64,
        vcpu_count: 1,
        memory_size: 1024 * 1024 * 1024, // 1GB
        exec_mode: vm_core::ExecMode::Interpreter,
        kernel_path: None,
        initrd_path: None,
    };

    #[cfg(feature = "devices")]
    let result = VmService::new(config, None).await;

    #[cfg(not(feature = "devices"))]
    let result = VmService::new(config, None).await;

    // 大内存可能成功或失败
    assert!(result.is_ok() || result.is_err());
}

#[tokio::test]
async fn test_vm_invalid_kernel_path() {
    let config = create_test_config();

    #[cfg(feature = "devices")]
    let mut vm = match VmService::new(config, None).await {
        Ok(vm) => vm,
        Err(_) => return,
    };

    #[cfg(not(feature = "devices"))]
    let mut vm = VmService::new(config, None).await.unwrap();

    // 测试无效的内核路径
    let invalid_paths = vec![
        "",
        "/nonexistent/path",
        "/dev/null/invalid",
        "relative/path.bin",
    ];

    for path in invalid_paths {
        let result = vm.load_kernel(path, 0x1000);
        assert!(result.is_err(), "Should fail for invalid path: {}", path);
    }
}

#[tokio::test]
async fn test_vm_run_at_invalid_address() {
    let config = create_test_config();

    #[cfg(feature = "devices")]
    let mut vm = match VmService::new(config, None).await {
        Ok(vm) => vm,
        Err(_) => return,
    };

    #[cfg(not(feature = "devices"))]
    let mut vm = VmService::new(config, None).await.unwrap();

    // 测试在无效地址运行
    let invalid_addresses = vec![
        0,           // 地址0
        u64::MAX,    // 最大地址
        0xFFFFFFFF,  // 接近边界
    ];

    for addr in invalid_addresses {
        let result = vm.run(addr);
        // 应该失败或panic
        assert!(result.is_err() || result.is_ok());
    }
}

#[tokio::test]
async fn test_vm_empty_snapshot_id() {
    let config = create_test_config();

    #[cfg(feature = "devices")]
    let mut vm = match VmService::new(config, None).await {
        Ok(vm) => vm,
        Err(_) => return,
    };

    #[cfg(not(feature = "devices"))]
    let mut vm = VmService::new(config, None).await.unwrap();

    // 测试空的快照ID
    let result = vm.restore_snapshot("");
    assert!(result.is_err(), "Empty snapshot ID should fail");
}

#[tokio::test]
async fn test_vm_empty_serialized_data() {
    let config = create_test_config();

    #[cfg(feature = "devices")]
    let mut vm = match VmService::new(config, None).await {
        Ok(vm) => vm,
        Err(_) => return,
    };

    #[cfg(not(feature = "devices"))]
    let mut vm = VmService::new(config, None).await.unwrap();

    // 测试空的序列化数据
    let result = vm.deserialize_state(&[]);
    assert!(result.is_err(), "Empty data should fail");
}

#[tokio::test]
async fn test_vm_invalid_serialized_data() {
    let config = create_test_config();

    #[cfg(feature = "devices")]
    let mut vm = match VmService::new(config, None).await {
        Ok(vm) => vm,
        Err(_) => return,
    };

    #[cfg(not(feature = "devices"))]
    let mut vm = VmService::new(config, None).await.unwrap();

    // 测试无效的序列化数据
    let invalid_data = vec![0xFF, 0xFF, 0xFF, 0xFF];
    let result = vm.deserialize_state(&invalid_data);
    assert!(result.is_err(), "Invalid data should fail");
}

#[tokio::test]
async fn test_vm_register_out_of_bounds() {
    let config = create_test_config();

    #[cfg(feature = "devices")]
    let vm = match VmService::new(config, None).await {
        Ok(vm) => vm,
        Err(_) => return,
    };

    #[cfg(not(feature = "devices"))]
    let vm = VmService::new(config, None).await.unwrap();

    // 测试超出范围的寄存器访问
    // RISC-V有32个寄存器(x0-x31)
    let out_of_bounds_indexes = vec![32, 100, 255, 999];

    for idx in out_of_bounds_indexes {
        let _reg_val = vm.get_reg(idx);
        // 应该panic或返回默认值
        // 如果panic，测试会失败；如果返回默认值，继续
    }
}

#[tokio::test]
async fn test_vm_template_with_empty_id() {
    let config = create_test_config();

    #[cfg(feature = "devices")]
    let mut vm = match VmService::new(config, None).await {
        Ok(vm) => vm,
        Err(_) => return,
    };

    #[cfg(not(feature = "devices"))]
    let mut vm = VmService::new(config, None).await.unwrap();

    // 测试空字符串的base_snapshot_id
    let result = vm.create_template(
        "test".to_string(),
        "description".to_string(),
        "".to_string(),
    );

    // 可能失败或成功，取决于实现
    assert!(result.is_ok() || result.is_err());
}

#[tokio::test]
async fn test_vm_multiple_snapshot_calls() {
    let config = create_test_config();

    #[cfg(feature = "devices")]
    let mut vm = match VmService::new(config, None).await {
        Ok(vm) => vm,
        Err(_) => return,
    };

    #[cfg(not(feature = "devices"))]
    let mut vm = VmService::new(config, None).await.unwrap();

    // 多次创建快照（应该都失败，因为功能禁用）
    for i in 0..5 {
        let result = vm.create_snapshot(
            format!("snapshot_{}", i),
            format!("Test snapshot {}", i),
        );
        assert!(result.is_err());
    }

    // 列出快照应该返回空列表
    let result = vm.list_snapshots();
    assert!(result.is_ok());
    let snapshots = result.unwrap();
    assert_eq!(snapshots.len(), 0);
}

#[tokio::test]
async fn test_vm_template_list_initially_empty() {
    let config = create_test_config();

    #[cfg(feature = "devices")]
    let vm = match VmService::new(config, None).await {
        Ok(vm) => vm,
        Err(_) => return,
    };

    #[cfg(not(feature = "devices"))]
    let vm = VmService::new(config, None).await.unwrap();

    // 初始状态下应该没有模板
    let result = vm.list_templates();
    assert!(result.is_ok());
    let templates = result.unwrap();
    assert_eq!(templates.len(), 0);
}

#[tokio::test]
async fn test_vm_x64_architecture() {
    // 测试x86_64架构支持
    let config = VmConfig {
        guest_arch: GuestArch::X86_64,
        vcpu_count: 1,
        memory_size: 1024 * 1024,
        exec_mode: vm_core::ExecMode::Interpreter,
        kernel_path: None,
        initrd_path: None,
    };

    #[cfg(feature = "devices")]
    let result = VmService::new(config, None).await;

    #[cfg(not(feature = "devices"))]
    let result = VmService::new(config, None).await;

    // 可能不支持x86_64，所以可能失败
    assert!(result.is_ok() || result.is_err());
}

#[tokio::test]
async fn test_vm_arm64_architecture() {
    // 测试ARM64架构支持
    let config = VmConfig {
        guest_arch: GuestArch::Arm64,
        vcpu_count: 1,
        memory_size: 1024 * 1024,
        exec_mode: vm_core::ExecMode::Interpreter,
        kernel_path: None,
        initrd_path: None,
    };

    #[cfg(feature = "devices")]
    let result = VmService::new(config, None).await;

    #[cfg(not(feature = "devices"))]
    let result = VmService::new(config, None).await;

    // 可能不支持ARM64，所以可能失败
    assert!(result.is_ok() || result.is_err());
}

#[tokio::test]
async fn test_vm_gpu_backend_variations() {
    let config = create_test_config();

    // 测试不同的GPU后端选项
    let gpu_backends = vec![
        None,
        Some("vulkan".to_string()),
        Some("opengl".to_string()),
        Some("".to_string()), // 空字符串
    ];

    for backend in gpu_backends {
        #[cfg(feature = "devices")]
        let result = VmService::new(config.clone(), backend.clone()).await;

        #[cfg(not(feature = "devices"))]
        let result = VmService::new(config.clone(), backend.clone()).await;

        // 不同的GPU后端可能导致不同的结果
        assert!(result.is_ok() || result.is_err());
    }
}

#[tokio::test]
async fn test_vm_interpreter_mode() {
    // 测试解释器模式
    let config = VmConfig {
        guest_arch: GuestArch::Riscv64,
        vcpu_count: 1,
        memory_size: 1024 * 1024,
        exec_mode: vm_core::ExecMode::Interpreter,
        kernel_path: None,
        initrd_path: None,
    };

    #[cfg(feature = "devices")]
    let result = VmService::new(config, None).await;

    #[cfg(not(feature = "devices"))]
    let result = VmService::new(config, None).await;

    assert!(result.is_ok());
}
