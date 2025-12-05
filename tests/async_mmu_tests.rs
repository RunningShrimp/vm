/// Week 4 - 异步 MMU 测试
/// 
/// 测试异步地址翻译、TLB 操作和缓存功能

use vm_core::{AsyncMMU, AsyncTLB, AccessType, GuestAddr, GuestPhysAddr};

#[test]
fn test_async_tlb_creation() {
    let tlb = AsyncTLB::new(256);
    let stats = tlb.get_stats();
    assert_eq!(stats.total_lookups, 0);
    assert_eq!(stats.hits, 0);
    assert_eq!(stats.misses, 0);
}

#[test]
fn test_async_tlb_insert_lookup() {
    let tlb = AsyncTLB::new(16);
    
    // 创建 TLB 表项
    let entry = vm_core::async_mmu::TLBEntry {
        va: 0x1000,
        pa: 0x2000,
        access: AccessType::Read,
        dirty: false,
        last_access_us: 0,
    };
    
    tlb.insert(entry);
    
    // 查找应该成功
    let result = tlb.lookup(0x1000);
    assert!(result.is_some());
    assert_eq!(result.unwrap().pa, 0x2000);
}

#[test]
fn test_async_tlb_miss() {
    let tlb = AsyncTLB::new(16);
    
    // 查找未插入的地址
    let result = tlb.lookup(0x5000);
    assert!(result.is_none());
    
    let stats = tlb.get_stats();
    assert_eq!(stats.misses, 1);
}

#[test]
fn test_async_tlb_hit_rate() {
    let tlb = AsyncTLB::new(32);
    
    // 插入一个表项
    let entry = vm_core::async_mmu::TLBEntry {
        va: 0x1000,
        pa: 0x2000,
        access: AccessType::Read,
        dirty: false,
        last_access_us: 0,
    };
    tlb.insert(entry);
    
    // 进行查找
    tlb.lookup(0x1000);  // hit
    tlb.lookup(0x1000);  // hit
    tlb.lookup(0x5000);  // miss
    
    let stats = tlb.get_stats();
    assert_eq!(stats.hits, 2);
    assert_eq!(stats.misses, 1);
    
    let hit_rate = stats.hit_rate();
    assert!(hit_rate > 0.6 && hit_rate < 0.7);
}

#[test]
fn test_async_tlb_flush_single() {
    let tlb = AsyncTLB::new(32);
    
    // 插入表项
    let entry = vm_core::async_mmu::TLBEntry {
        va: 0x1000,
        pa: 0x2000,
        access: AccessType::Read,
        dirty: false,
        last_access_us: 0,
    };
    tlb.insert(entry);
    
    // 验证存在
    assert!(tlb.lookup(0x1000).is_some());
    
    // 刷新
    tlb.flush_va(0x1000);
    
    // 验证已删除
    assert!(tlb.lookup(0x1000).is_none());
}

#[test]
fn test_async_tlb_flush_all() {
    let tlb = AsyncTLB::new(32);
    
    // 插入多个表项
    for i in 0..5 {
        let entry = vm_core::async_mmu::TLBEntry {
            va: 0x1000 + (i * 0x1000) as u64,
            pa: 0x2000 + (i * 0x1000) as u64,
            access: AccessType::Read,
            dirty: false,
            last_access_us: 0,
        };
        tlb.insert(entry);
    }
    
    // 全部刷新
    tlb.flush_all();
    
    // 验证全部删除
    for i in 0..5 {
        let va = 0x1000 + (i * 0x1000) as u64;
        assert!(tlb.lookup(va).is_none());
    }
}

#[test]
fn test_async_tlb_capacity_eviction() {
    let tlb = AsyncTLB::new(4);
    
    // 插入 5 个表项，应该驱逐最旧的
    for i in 0..5 {
        let entry = vm_core::async_mmu::TLBEntry {
            va: 0x1000 + (i * 0x1000) as u64,
            pa: 0x2000 + (i * 0x1000) as u64,
            access: AccessType::Read,
            dirty: false,
            last_access_us: 0,
        };
        tlb.insert(entry);
    }
    
    let stats = tlb.get_stats();
    assert_eq!(stats.evictions, 1);  // 应该有 1 次驱逐
}

#[tokio::test]
async fn test_async_tlb_translate() {
    let tlb = AsyncTLB::new(32);
    
    // 异步地址翻译
    let result = tlb.translate_async(0x1000).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_async_tlb_batch_translate() {
    let tlb = AsyncTLB::new(32);
    
    let addresses = vec![0x1000, 0x2000, 0x3000];
    let result = tlb.batch_translate_async(&addresses).await;
    
    assert!(result.is_ok());
    let translated = result.unwrap();
    assert_eq!(translated.len(), 3);
}

#[tokio::test]
async fn test_async_tlb_prefetch() {
    let tlb = AsyncTLB::new(32);
    
    let addresses = vec![0x1000, 0x2000, 0x3000];
    let result = tlb.prefetch_async(&addresses).await;
    
    assert!(result.is_ok());
    
    // 处理预取队列
    tlb.process_prefetch_queue().await;
    
    let stats = tlb.get_stats();
    assert_eq!(stats.prefetch_hits, 3);
}

#[test]
fn test_async_tlb_stats_reset() {
    let tlb = AsyncTLB::new(32);
    
    // 进行一些操作
    let entry = vm_core::async_mmu::TLBEntry {
        va: 0x1000,
        pa: 0x2000,
        access: AccessType::Read,
        dirty: false,
        last_access_us: 0,
    };
    tlb.insert(entry);
    tlb.lookup(0x1000);
    
    let stats = tlb.get_stats();
    assert!(stats.total_lookups > 0);
    
    // 重置统计
    tlb.reset_stats();
    let stats = tlb.get_stats();
    assert_eq!(stats.total_lookups, 0);
    assert_eq!(stats.hits, 0);
}

#[test]
fn test_concurrent_tlb_operations() {
    use std::sync::Arc;
    use std::thread;
    
    let tlb = Arc::new(AsyncTLB::new(256));
    
    // 启动多个线程进行并发操作
    let mut handles = vec![];
    
    for thread_id in 0..4 {
        let tlb_clone = Arc::clone(&tlb);
        let handle = thread::spawn(move || {
            for i in 0..100 {
                let va = 0x1000 + (thread_id * 0x1000) as u64 + (i * 0x100) as u64;
                let pa = va + 0x1000;
                
                let entry = vm_core::async_mmu::TLBEntry {
                    va,
                    pa,
                    access: AccessType::Read,
                    dirty: false,
                    last_access_us: 0,
                };
                tlb_clone.insert(entry);
                
                let _ = tlb_clone.lookup(va);
            }
        });
        handles.push(handle);
    }
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    let stats = tlb.get_stats();
    assert!(stats.total_lookups > 0);
}
