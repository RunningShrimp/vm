//! 统一MMU测试套件（迁移到v2）

use vm_core::{AccessType, GuestAddr};
use vm_mem::unified_mmu_v2::{HybridMMU, UnifiedMmuConfigV2, UnifiedMMU};

#[test]
fn test_unified_mmu_v2_creation() {
    let config = UnifiedMmuConfigV2::default();
    let mmu = HybridMMU::new(0x10000000, config);
    assert_eq!(mmu.memory_size(), 0x10000000);
}

#[test]
fn test_unified_mmu_v2_translate_bare() {
    let config = UnifiedMmuConfigV2::default();
    let mut mmu = HybridMMU::new(0x10000000, config);

    // Bare模式应该是恒等映射
    let va = GuestAddr(0x12345678);
    let result = mmu.translate(va, AccessType::Read);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), vm_core::GuestPhysAddr(0x12345678));
}

#[test]
fn test_unified_mmu_v2_read_write() {
    let config = UnifiedMmuConfigV2::default();
    let mut mmu = HybridMMU::new(0x10000000, config);

    let addr = GuestAddr(0x1000);
    let value = 0xDEADBEEF;

    // 写入
    let write_result = mmu.write(addr, value, 4);
    assert!(write_result.is_ok());

    // 读取
    let read_result = mmu.read(addr, 4);
    assert!(read_result.is_ok());
    assert_eq!(read_result.unwrap(), value);
}

#[test]
fn test_unified_mmu_v2_bulk_operations() {
    let config = UnifiedMmuConfigV2::default();
    let mut mmu = HybridMMU::new(0x10000000, config);

    let test_data: Vec<u8> = (0..256).map(|i| i as u8).collect();
    let addr = GuestAddr(0x1000);

    // 批量写入
    let write_result = mmu.write_bulk(addr, &test_data);
    assert!(write_result.is_ok());

    // 批量读取
    let mut read_buffer = vec![0u8; 256];
    let read_result = mmu.read_bulk(addr, &mut read_buffer);
    assert!(read_result.is_ok());

    // 验证数据
    assert_eq!(read_buffer, test_data);
}

#[test]
fn test_unified_mmu_v2_fetch_insn() {
    let config = UnifiedMmuConfigV2::default();
    let mut mmu = HybridMMU::new(0x10000000, config);

    let addr = GuestAddr(0x2000);
    let insn = 0x12345678;

    // 写入指令
    let write_result = mmu.write(addr, insn, 4);
    assert!(write_result.is_ok());

    // 取指令
    let fetch_result = mmu.fetch_insn(addr);
    assert!(fetch_result.is_ok());
    assert_eq!(fetch_result.unwrap(), insn);
}

#[test]
fn test_unified_mmu_v2_tlb_flush() {
    let config = UnifiedMmuConfigV2::default();
    let mut mmu = HybridMMU::new(0x10000000, config);

    // 进行一些翻译
    let _ = mmu.translate(GuestAddr(0x1000), AccessType::Read);
    let _ = mmu.translate(GuestAddr(0x2000), AccessType::Read);

    // 刷新TLB
    mmu.flush_tlb();

    // 验证仍然可以翻译
    let result = mmu.translate(GuestAddr(0x1000), AccessType::Read);
    assert!(result.is_ok());
}

#[test]
fn test_unified_mmu_v2_stats() {
    let config = UnifiedMmuConfigV2::default();
    let mut mmu = HybridMMU::new(0x10000000, config);

    // 执行一些操作
    let _ = mmu.translate(GuestAddr(0x1000), AccessType::Read);
    let _ = mmu.read(GuestAddr(0x100), 4);
    let _ = mmu.write(GuestAddr(0x200), 0x12345678, 4);

    // 获取统计信息
    let stats = mmu.stats();
    assert_eq!(stats.mmu_id, 1);
    assert_eq!(stats.memory_size_bytes, 0x10000000);
}

#[test]
fn test_unified_mmu_v2_dump_restore() {
    let config = UnifiedMmuConfigV2::default();
    let mut mmu1 = HybridMMU::new(1024 * 1024, config.clone());

    // 写入数据
    let _ = mmu1.write(GuestAddr(0x100), 0xABCD1234, 4);

    // 导出内存
    let dump = mmu1.dump_memory();
    assert_eq!(dump.len(), 1024 * 1024);

    // 恢复到新的MMU
    let mut mmu2 = HybridMMU::new(1024 * 1024, config);
    let restore_result = mmu2.restore_memory(&dump);
    assert!(restore_result.is_ok());

    // 验证数据
    let read_result = mmu2.read(GuestAddr(0x100), 4);
    assert!(read_result.is_ok());
    assert_eq!(read_result.unwrap(), 0xABCD1234);
}

#[test]
fn test_unified_mmu_v2_custom_config() {
    let config = UnifiedMmuConfigV2 {
        enable_multilevel_tlb: false,
        l1_itlb_capacity: 32,
        l1_dtlb_capacity: 64,
        l2_tlb_capacity: 256,
        l3_tlb_capacity: 1024,
        enable_concurrent_tlb: false,
        sharded_tlb_capacity: 2048,
        shard_count: 8,
        enable_fast_path: false,
        fast_path_capacity: 0,
        enable_page_table_cache: false,
        page_table_cache_size: 0,
        enable_prefetch: false,
        prefetch_window: 0,
        prefetch_history_window: 0,
        enable_adaptive_replacement: false,
        adaptive_threshold: 0.5,
        enable_stats: true,
        enable_monitoring: false,
        strict_align: false,
        use_hugepages: false,
    };

    let mmu = HybridMMU::new(0x10000000, config);
    assert_eq!(mmu.memory_size(), 0x10000000);
}

#[tokio::test]
#[cfg(feature = "async")]
async fn test_unified_mmu_v2_async_translate() {
    let config = UnifiedMmuConfigV2::default();
    let mut mmu = HybridMMU::new(0x10000000, config);

    // 异步翻译
    let result = mmu.translate_async(GuestAddr(0x1000), AccessType::Read).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), vm_core::GuestPhysAddr(0x1000));
}

#[tokio::test]
#[cfg(feature = "async")]
async fn test_unified_mmu_v2_async_read_write() {
    let config = UnifiedMmuConfigV2::default();
    let mmu = HybridMMU::new(0x10000000, config);

    let addr = GuestAddr(0x1000);
    let value = 0xCAFEBABE;

    // 异步写入
    let write_result = mmu.write_async(addr, value, 4).await;
    assert!(write_result.is_ok());

    // 异步读取
    let read_result = mmu.read_async(addr, 4).await;
    assert!(read_result.is_ok());
    assert_eq!(read_result.unwrap(), value);
}
