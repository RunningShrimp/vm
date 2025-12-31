//! 基础集成测试 - 最小化可用版本
//! 只测试最核心的功能，避免API兼容性问题

use vm_core::{GuestAddr, MemoryAccess};
use vm_mem::UnifiedMmu;

#[test]
fn test_basic_mmu_operations() {
    let mut mmu = UnifiedMmu::new(
        1024 * 1024, // 1MB
        false,
        vm_mem::unified_mmu::UnifiedMmuConfig::default(),
    );

    let addr = GuestAddr(0x1000);

    // 写入
    assert!(mmu.write(addr, 0x42, 1).is_ok());

    // 读回
    let result = mmu.read(addr, 1);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0x42);
}

#[test]
fn test_multiple_operations() {
    let mut mmu = UnifiedMmu::new(
        1024 * 1024,
        false,
        vm_mem::unified_mmu::UnifiedMmuConfig::default(),
    );

    // 多次写入
    for i in 0..10 {
        let addr = GuestAddr(0x2000 + i * 8);
        assert!(mmu.write(addr, i as u64, 8).is_ok());
    }

    // 验证
    for i in 0..10 {
        let addr = GuestAddr(0x2000 + i * 8);
        let result = mmu.read(addr, 8);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), i as u64);
    }
}

#[test]
fn test_stats_access() {
    let mmu = UnifiedMmu::new(
        1024 * 1024,
        false,
        vm_mem::unified_mmu::UnifiedMmuConfig::default(),
    );

    let stats = mmu.stats();
    // 验证可访问 - 使用load()读取AtomicU64
    let _ = (
        stats.tlb_hits.load(std::sync::atomic::Ordering::Relaxed),
        stats.tlb_misses.load(std::sync::atomic::Ordering::Relaxed),
    );
}
