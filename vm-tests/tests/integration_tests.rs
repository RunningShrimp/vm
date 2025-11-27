/// 集成测试 - 虚拟机完整生命周期测试
///
/// 这些测试覆盖了虚拟机从初始化、启动、执行到停止的完整生命周期。
/// 测试目标是确保系统在各种场景下都能正确工作。

use vm_mem::SoftMmu;
use vm_core::{MMU, VirtualMachine, VmConfig, GuestArch, ExecMode};
use vm_ir::IRBlock;

/// VM 初始化测试
#[test]
fn test_vm_initialization() {
    let config = VmConfig {
        guest_arch: GuestArch::Riscv64,
        memory_size: 1024 * 1024,
        vcpu_count: 1,
        exec_mode: ExecMode::Interpreter,
        ..Default::default()
    };

    let mmu = SoftMmu::new(config.memory_size, false);
    let vm: VirtualMachine<IRBlock> = VirtualMachine::with_mmu(config.clone(), Box::new(mmu));

    // 验证 VM 配置
    assert_eq!(vm.config().guest_arch, GuestArch::Riscv64);
    assert_eq!(vm.config().memory_size, 1024 * 1024);
    assert_eq!(vm.config().vcpu_count, 1);
    assert_eq!(vm.config().exec_mode, ExecMode::Interpreter);
}

/// 内存映射测试
#[test]
fn test_memory_mapping() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);

    // 测试基本的读写操作
    let test_addr = 0x1000;
    let test_value = 0x12345678u64;

    // 写入
    mmu.write(test_addr, test_value, 8)
        .expect("Failed to write to memory");

    // 读取
    let read_value = mmu.read(test_addr, 8)
        .expect("Failed to read from memory");

    assert_eq!(read_value, test_value, "Memory value mismatch");
}

/// 批量内存操作测试
#[test]
fn test_bulk_memory_operations() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);

    let test_addr = 0x2000;
    let test_data = vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];

    // 批量写入
    for (i, &byte) in test_data.iter().enumerate() {
        mmu.write(test_addr + i as u64, byte as u64, 1)
            .expect("Failed to write to memory");
    }

    // 批量读取
    for (i, &expected) in test_data.iter().enumerate() {
        let value = mmu.read(test_addr + i as u64, 1)
            .expect("Failed to read from memory");
        assert_eq!(value, expected as u64, "Bulk read mismatch at offset {}", i);
    }
}

/// MMIO 设备映射测试
#[test]
fn test_mmio_device_mapping() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);

    use vm_device::block::{VirtioBlock, VirtioBlockMmio};

    // 创建设备
    let virtio = VirtioBlock::new();
    let mmio = VirtioBlockMmio::new(virtio);

    // 映射 MMIO 区域
    let mmio_base = 0x10000000;
    let mmio_size = 0x1000;
    mmu.map_mmio(mmio_base, mmio_size, Box::new(mmio));

    // 尝试写入 MMIO 区域（不应该崩溃）
    let result = mmu.write(mmio_base + 0x20, 0x1, 4);
    assert!(result.is_ok(), "MMIO write should succeed");
}

/// 虚拟机启动和停止测试
#[test]
fn test_vm_startup_shutdown() {
    let config = VmConfig {
        guest_arch: GuestArch::Riscv64,
        memory_size: 1024 * 1024,
        vcpu_count: 1,
        exec_mode: ExecMode::Interpreter,
        ..Default::default()
    };

    let mmu = SoftMmu::new(config.memory_size, false);
    let vm: VirtualMachine<IRBlock> = VirtualMachine::with_mmu(config, Box::new(mmu));

    // 获取 MMU 锁以确保能正确访问
    let mmu_ref = vm.mmu();
    let _mmu_guard = mmu_ref.lock();
    // 测试通过，无需崩溃或恐慌
}

/// 虚拟机内存布局测试
#[test]
fn test_vm_memory_layout() {
    let config = VmConfig {
        guest_arch: GuestArch::Riscv64,
        memory_size: 4 * 1024 * 1024, // 4MB
        vcpu_count: 1,
        exec_mode: ExecMode::Interpreter,
        ..Default::default()
    };

    let mmu = SoftMmu::new(config.memory_size, false);
    let vm: VirtualMachine<IRBlock> = VirtualMachine::with_mmu(config, Box::new(mmu));

    let mmu_ref = vm.mmu();
    let mut mmu = mmu_ref.lock().expect("Failed to acquire MMU lock");

    // 测试内存的不同区域
    // 低地址空间
    let addr1 = 0x0;
    mmu.write(addr1, 0x1111, 4).expect("Failed to write to low address");
    let val1 = mmu.read(addr1, 4).expect("Failed to read from low address");
    assert_eq!(val1, 0x1111);

    // 中间地址空间
    let addr2 = 0x100000;
    mmu.write(addr2, 0x2222, 4).expect("Failed to write to mid address");
    let val2 = mmu.read(addr2, 4).expect("Failed to read from mid address");
    assert_eq!(val2, 0x2222);

    // 高地址空间
    let addr3 = 0x200000;
    mmu.write(addr3, 0x3333, 4).expect("Failed to write to high address");
    let val3 = mmu.read(addr3, 4).expect("Failed to read from high address");
    assert_eq!(val3, 0x3333);
}

/// 虚拟机多个 vCPU 配置测试
#[test]
fn test_multi_vcpu_config() {
    let configs = vec![
        (1, "Single vCPU"),
        (2, "Dual vCPU"),
        (4, "Quad vCPU"),
    ];

    for (vcpu_count, desc) in configs {
        let config = VmConfig {
            guest_arch: GuestArch::Riscv64,
            memory_size: 1024 * 1024,
            vcpu_count,
            exec_mode: ExecMode::Interpreter,
            ..Default::default()
        };

        let mmu = SoftMmu::new(config.memory_size, false);
        let vm: VirtualMachine<IRBlock> = VirtualMachine::with_mmu(config.clone(), Box::new(mmu));

        assert_eq!(vm.config().vcpu_count, vcpu_count, "Failed for {}", desc);
    }
}

/// 虚拟机执行模式配置测试
#[test]
fn test_execution_modes() {
    let modes = vec![
        (ExecMode::Interpreter, "Interpreter"),
        (ExecMode::Jit, "JIT"),
    ];

    for (exec_mode, desc) in modes {
        let config = VmConfig {
            guest_arch: GuestArch::Riscv64,
            memory_size: 1024 * 1024,
            vcpu_count: 1,
            exec_mode,
            ..Default::default()
        };

        let mmu = SoftMmu::new(config.memory_size, false);
        let vm: VirtualMachine<IRBlock> = VirtualMachine::with_mmu(config.clone(), Box::new(mmu));

        assert_eq!(vm.config().exec_mode, exec_mode, "Failed for {}", desc);
    }
}

/// 虚拟机内核加载测试
#[test]
fn test_kernel_loading() {
    let config = VmConfig {
        guest_arch: GuestArch::Riscv64,
        memory_size: 1024 * 1024,
        vcpu_count: 1,
        exec_mode: ExecMode::Interpreter,
        ..Default::default()
    };

    let mmu = SoftMmu::new(config.memory_size, false);
    let mut vm: VirtualMachine<IRBlock> = VirtualMachine::with_mmu(config, Box::new(mmu));

    // 创建简单的测试代码
    let test_code = vec![0x00, 0x00, 0x00, 0x00];
    let entry_point = 0x0;

    // 加载内核
    let result = vm.load_kernel(&test_code, entry_point);
    assert!(result.is_ok(), "Failed to load kernel");
}

/// 虚拟机配置持久化测试
#[test]
fn test_vm_config_persistence() {
    let original_config = VmConfig {
        guest_arch: GuestArch::Riscv64,
        memory_size: 2 * 1024 * 1024,
        vcpu_count: 2,
        exec_mode: ExecMode::Jit,
        ..Default::default()
    };

    let mmu = SoftMmu::new(original_config.memory_size, false);
    let vm: VirtualMachine<IRBlock> = VirtualMachine::with_mmu(original_config.clone(), Box::new(mmu));

    let vm_config = vm.config();
    assert_eq!(vm_config.guest_arch, original_config.guest_arch);
    assert_eq!(vm_config.memory_size, original_config.memory_size);
    assert_eq!(vm_config.vcpu_count, original_config.vcpu_count);
    assert_eq!(vm_config.exec_mode, original_config.exec_mode);
}

/// 内存对齐测试
#[test]
fn test_memory_alignment() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);

    // 测试 1 字节对齐
    for i in 0..10 {
        let addr = 0x1000 + i;
        mmu.write(addr, i as u64, 1).expect("Failed 1-byte write");
        let val = mmu.read(addr, 1).expect("Failed 1-byte read");
        assert_eq!(val, i as u64);
    }

    // 测试 2 字节对齐
    for i in 0..5 {
        let addr = 0x2000 + i * 2;
        mmu.write(addr, (i * 0x1111) as u64, 2).expect("Failed 2-byte write");
        let val = mmu.read(addr, 2).expect("Failed 2-byte read");
        assert_eq!(val, (i * 0x1111) as u64);
    }

    // 测试 4 字节对齐
    for i in 0..5 {
        let addr = 0x3000 + i * 4;
        mmu.write(addr, (i * 0x11111111) as u64, 4).expect("Failed 4-byte write");
        let val = mmu.read(addr, 4).expect("Failed 4-byte read");
        assert_eq!(val, (i * 0x11111111) as u64);
    }

    // 测试 8 字节对齐
    for i in 0..5 {
        let addr = 0x4000 + i * 8;
        mmu.write(addr, (i as u64) * 0x1111111111111111, 8).expect("Failed 8-byte write");
        let val = mmu.read(addr, 8).expect("Failed 8-byte read");
        assert_eq!(val, (i as u64) * 0x1111111111111111);
    }
}
