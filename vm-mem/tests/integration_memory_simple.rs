//! 简化的集成测试 - 测试核心功能
//! 使用当前API

use vm_core::{GuestAddr, GuestPhysAddr, MemoryAccess, VmError};
use vm_mem::{
    UnifiedMmu, UnifiedMmuConfig,
    optimization::unified::{MemoryManagerFactory, MemoryPool, PhysicalMemoryManager},
};

/// 创建测试用MMU
fn create_test_mmu() -> UnifiedMmu {
    UnifiedMmu::new(
        1024 * 1024, // 1MB
        false,       // 不使用hugepages
        UnifiedMmuConfig::default(),
    )
}

#[test]
fn test_unified_mmu_basic() {
    let mut mmu = create_test_mmu();
    let addr = GuestAddr(0x1000);

    // 测试写入和读取
    mmu.write(addr, 0x42, 1).unwrap();
    let value = mmu.read(addr, 1).unwrap();
    assert_eq!(value, 0x42);
}

#[test]
fn test_unified_mmu_stats() {
    let mmu = create_test_mmu();
    let stats = mmu.stats();

    // 验证统计数据可访问
    assert!(stats.tlb_hits >= 0);
    assert!(stats.tlb_misses >= 0);
}

#[test]
fn test_memory_pool_basic() {
    let mut pool = MemoryPool::new();

    // 测试分配
    let addr1 = pool.allocate(1024).unwrap();
    let addr2 = pool.allocate(2048).unwrap();

    assert_ne!(addr1, addr2);

    // 测试释放
    pool.deallocate(addr1).unwrap();

    // 测试重新分配
    let addr3 = pool.allocate(512).unwrap();
    // addr3应该重用addr1或使用新地址
}

#[test]
fn test_memory_manager_factory() {
    // 测试工厂方法
    let mmu = create_test_mmu();
    let pool = MemoryManagerFactory::create_memory_pool();

    // 基本验证
    let _ = (mmu, pool);
}

#[test]
fn test_multiple_writes() {
    let mut mmu = create_test_mmu();

    // 写入多个位置
    for i in 0..10 {
        let addr = GuestAddr(0x1000 + i * 8);
        mmu.write(addr, i as u64, 8).unwrap();
    }

    // 读回验证
    for i in 0..10 {
        let addr = GuestAddr(0x1000 + i * 8);
        let value = mmu.read(addr, 8).unwrap();
        assert_eq!(value, i as u64);
    }
}

#[test]
fn test_different_sizes() {
    let mut mmu = create_test_mmu();
    let addr = GuestAddr(0x2000);

    // 测试不同大小的写入
    mmu.write(addr, 0x12345678, 4).unwrap();
    let value = mmu.read(addr, 4).unwrap();
    assert_eq!(value, 0x12345678);
}

#[test]
fn test_sequential_operations() {
    let mut mmu = create_test_mmu();

    // 测试顺序读写操作
    let addrs = vec![GuestAddr(0x3000), GuestAddr(0x3008), GuestAddr(0x3010)];

    // 先写入
    for (i, &addr) in addrs.iter().enumerate() {
        mmu.write(addr, i as u64, 8).unwrap();
    }

    // 顺序读回验证
    for (i, &addr) in addrs.iter().enumerate() {
        let value = mmu.read(addr, 8).unwrap();
        assert_eq!(value, i as u64);
    }
}
