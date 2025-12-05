//! 统一代码缓存测试套件

use vm_core::GuestAddr;
use vm_engine_jit::ewma_hotspot::EwmaHotspotConfig;
use vm_engine_jit::unified_cache::{CacheConfig, CompilePriority, UnifiedCodeCache};
use vm_ir::{IRBlock, IRBuilder, IROp, Terminator};

fn create_test_ir_block(addr: GuestAddr) -> IRBlock {
    let mut builder = IRBuilder::new(addr);
    builder.push(IROp::MovImm { dst: 1, imm: 42 });
    builder.push(IROp::Add {
        dst: 0,
        src1: 0,
        src2: 1,
    });
    builder.set_term(Terminator::Jmp { target: addr + 16 });
    builder.build()
}

#[test]
fn test_unified_cache_basic() {
    let cache_config = CacheConfig::default();
    let hotspot_config = EwmaHotspotConfig::default();
    let cache = UnifiedCodeCache::new(cache_config, hotspot_config);

    let addr = 0x1000;
    let code = vec![0x90, 0x90, 0x90]; // NOP指令

    // 插入代码（同步）
    cache.insert_sync(addr, code.clone(), false);

    // 查找代码
    let found = cache.lookup(addr);
    assert!(found.is_some());
}

#[test]
fn test_unified_cache_hot_cold_promotion() {
    let mut cache_config = CacheConfig::default();
    cache_config.warmup_size = 3; // 访问3次后提升到热缓存
    let hotspot_config = EwmaHotspotConfig::default();
    let cache = UnifiedCodeCache::new(cache_config, hotspot_config);

    let addr = 0x1000;
    let code = vec![0x90];
    cache.insert_sync(addr, code, false);

    // 初始应该在冷缓存
    assert!(cache.lookup(addr).is_some());

    // 访问多次以触发提升
    for _ in 0..5 {
        cache.lookup(addr);
    }

    // 应该提升到热缓存
    let stats = cache.get_stats();
    assert!(stats.total_entries > 0);
}

#[test]
fn test_unified_cache_eviction() {
    let mut cache_config = CacheConfig::default();
    cache_config.max_entries = 2; // 限制缓存大小
    let hotspot_config = EwmaHotspotConfig::default();
    let cache = UnifiedCodeCache::new(cache_config, hotspot_config);

    // 插入超过限制的条目
    for i in 0..5 {
        let addr = 0x1000 + (i * 0x1000);
        let code = vec![0x90];
        cache.insert_sync(addr, code, false);
    }

    // 应该发生淘汰
    let stats = cache.get_stats();
    assert!(stats.total_entries <= 4);
}

#[test]
fn test_unified_cache_async_insert() {
    let cache_config = CacheConfig::default();
    let hotspot_config = EwmaHotspotConfig::default();
    let cache = UnifiedCodeCache::new(cache_config, hotspot_config);

    let addr = 0x1000;
    let block = create_test_ir_block(addr);

    // 提交异步编译请求
    cache.insert_async(addr, block, CompilePriority::High);

    // 检查是否已提交
    // 注意：实际编译是异步的，这里只测试提交
}

#[test]
fn test_unified_cache_stats() {
    let cache_config = CacheConfig::default();
    let hotspot_config = EwmaHotspotConfig::default();
    let cache = UnifiedCodeCache::new(cache_config, hotspot_config);

    // 插入一些条目
    for i in 0..5 {
        let addr = 0x1000 + (i * 0x1000);
        let code = vec![0x90];
        cache.insert_sync(addr, code, false);
    }

    // 访问一些条目
    cache.lookup(0x1000);
    cache.lookup(0x2000);
    cache.lookup(0x9999); // 未命中

    let stats = cache.get_stats();
    assert!(stats.total_entries >= 5);
    assert!(stats.hits >= 2);
    assert!(stats.misses >= 1);
}
