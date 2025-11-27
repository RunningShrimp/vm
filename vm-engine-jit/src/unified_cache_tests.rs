use super::unified_cache::*;
use vm_core::GuestAddr;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[test]
fn test_cache_sharding_basic() {
    // 创建配置
    let config = UnifiedCacheConfig {
        max_size: 1024 * 1024, // 1MB
        shard_count: 16, // 16个分片
        hot_threshold: 10,
        eviction_policy: EvictionPolicy::LRU_LFU,
        ..Default::default()
    };
    
    // 创建缓存
    let cache = UnifiedCache::new(config);
    
    // 测试基本的插入和查找
    let addr1 = GuestAddr(0x1000);
    let addr2 = GuestAddr(0x2000);
    
    let code1 = vec![0x90, 0x90, 0x90]; // nop指令
    let code2 = vec![0x55, 0x48, 0x89, 0xe5]; // push rbp; mov rbp, rsp
    
    cache.insert(addr1, code1.clone());
    cache.insert(addr2, code2.clone());
    
    assert_eq!(cache.lookup(addr1).is_some(), true);
    assert_eq!(cache.lookup(addr2).is_some(), true);
    
    // 测试不在缓存中的地址
    let addr3 = GuestAddr(0x3000);
    assert_eq!(cache.lookup(addr3).is_none(), true);
}

#[test]
fn test_cache_concurrent_access() {
    // 创建配置
    let config = UnifiedCacheConfig {
        max_size: 1024 * 1024, // 1MB
        shard_count: 16, // 16个分片
        hot_threshold: 5,
        eviction_policy: EvictionPolicy::LRU,
        ..Default::default()
    };
    
    // 创建线程安全的缓存
    let cache = Arc::new(UnifiedCache::new(config));
    
    // 同时运行多个线程进行插入和查找
    let mut handles = Vec::new();
    
    for thread_id in 0..8 {
        let cache_clone = Arc::clone(&cache);
        
        let handle = thread::spawn(move || {
            for i in 0..100 {
                let addr = GuestAddr((thread_id * 1000 + i) as u64);
                let code = vec![0x90; 10]; // nop指令
                
                // 插入
                cache_clone.insert(addr, code.clone());
                
                // 查找
                let result = cache_clone.lookup(addr);
                assert!(result.is_some());
                
                // 模拟一些工作
                thread::sleep(Duration::from_nanos(100));
            }
        });
        
        handles.push(handle);
    }
    
    // 等待所有线程完成
    for handle in handles {
        handle.join().unwrap();
    }
    
    println!("Concurrent access test passed!");
}

#[test]
fn test_cache_eviction() {
    // 创建一个小缓存来测试驱逐
    let config = UnifiedCacheConfig {
        max_size: 100, // 100字节
        shard_count: 4, // 4个分片
        hot_threshold: 3,
        eviction_policy: EvictionPolicy::LRU,
        ..Default::default()
    };
    
    let cache = UnifiedCache::new(config);
    
    // 插入超过缓存大小的条目
    for i in 0..20 {
        let addr = GuestAddr(i as u64);
        let code = vec![0x90; 10]; // 10字节每个条目
        cache.insert(addr, code);
    }
    
    // 一些条目应该被驱逐了
    // 检查最近的条目是否还在
    assert_eq!(cache.lookup(GuestAddr(19)).is_some(), true);
    // 检查最早的条目是否被驱逐
    assert_eq!(cache.lookup(GuestAddr(0)).is_none(), true);
    
    println!("Eviction test passed!");
}

#[test]
fn test_cache_hot_promotion() {
    // 创建配置
    let config = UnifiedCacheConfig {
        max_size: 1000,
        shard_count: 4,
        hot_threshold: 5, // 5次访问后变为热门
        eviction_policy: EvictionPolicy::LRU_LFU,
        ..Default::default()
    };
    
    let cache = UnifiedCache::new(config);
    
    let addr = GuestAddr(0x1000);
    let code = vec![0x90; 10];
    
    cache.insert(addr, code);
    
    // 访问多次以使其变为热门
    for _ in 0..5 {
        cache.lookup(addr);
    }
    
    // 插入其他条目来触发驱逐
    for i in 0..100 {
        let other_addr = GuestAddr(0x2000 + i);
        cache.insert(other_addr, vec![0x90; 10]);
    }
    
    // 热门条目应该还在
    assert_eq!(cache.lookup(addr).is_some(), true);
    
    println!("Hot promotion test passed!");
}

#[test]
fn test_cache_metrics() {
    // 创建配置
    let config = UnifiedCacheConfig {
        max_size: 1024 * 1024,
        shard_count: 8,
        hot_threshold: 10,
        eviction_policy: EvictionPolicy::LRU,
        ..Default::default()
    };
    
    let cache = UnifiedCache::new(config);
    
    // 测试缓存命中率
    let addr = GuestAddr(0x1000);
    let code = vec![0x90; 10];
    
    // 第一次查找应该miss
    cache.lookup(addr);
    
    // 插入后查找应该hit
    cache.insert(addr, code);
    cache.lookup(addr);
    cache.lookup(addr);
    
    // 获取统计信息
    let stats = cache.get_stats();
    
    // 应该有1次miss，2次hit
    assert_eq!(stats.misses, 1);
    assert_eq!(stats.hits >= 2, true);
    
    println!("Cache metrics test passed!");
}