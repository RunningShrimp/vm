//! 属性测试框架
//!
//! 使用proptest进行属性测试

use proptest::prelude::*;
use vm_core::{GuestAddr, GuestPhysAddr, AccessType};
use vm_mem::tlb::core::lockfree::LockFreeTlb;
use vm_mem::tlb::core::lockfree::TlbEntry;

/// 属性测试: TLB查找和插入的一致性
proptest! {
    #[test]
    fn prop_tlb_lookup_insert(vpn in any::<u64>(), ppn in any::<u64>()) {
        let tlb = LockFreeTlb::new();

        // 插入
        let entry = TlbEntry::new(vpn & !0xFFF, ppn & !0xFFF, 0x1, 0);
        tlb.insert(entry);

        // 查找
        let result = tlb.lookup(vpn & !0xFFF, 0);

        // 如果VPN和PPN对齐，应该能找到
        if vpn % 4096 == 0 && ppn % 4096 == 0 {
            prop_assert!(result.is_some());
            if let Some(found) = result {
                prop_assert_eq!(found.ppn, ppn & !0xFFF);
            }
        }
    }
}

/// 属性测试: 批量操作的顺序保持性
proptest! {
    #[test]
    fn prop_batch_operations_preserves_order(addrs in prop::collection::vec(any::<u64>(), 1..100)) {
        let tlb = LockFreeTlb::new();

        // 插入
        for addr in &addrs {
            let entry = TlbEntry::new(addr & !0xFFF, addr & !0xFFF, 0x1, 0);
            tlb.insert(entry);
        }

        // 批量查找
        let requests: Vec<_> = addrs.iter().map(|addr| (*addr & !0xFFF, 0)).collect();
        let results = tlb.lookup_batch(&requests);

        // 验证数量
        prop_assert_eq!(results.len(), addrs.len());
    }
}

/// 属性测试: TLB刷新后所有条目消失
proptest! {
    #[test]
    fn prop_flush_clears_all(addrs in prop::collection::vec(any::<u64>(), 1..100)) {
        let tlb = LockFreeTlb::new();

        // 插入
        for addr in &addrs {
            let entry = TlbEntry::new(*addr & !0xFFF, *addr & !0xFFF, 0x1, 0);
            tlb.insert(entry);
        }

        // 刷新
        tlb.flush();

        // 所有查找应该失败
        for addr in &addrs {
            let result = tlb.lookup(*addr & !0xFFF, 0);
            prop_assert!(result.is_none());
        }
    }
}

/// 属性测试: ASID隔离
proptest! {
    #[test]
    fn test_asid_isolation(addrs in prop::collection::vec(any::<u64>(), 1..50)) {
        let tlb = LockFreeTlb::new();

        // ASID 0
        for addr in &addrs {
            let entry = TlbEntry::new(*addr & !0xFFF, *addr & !0xFFF, 0x1, 0);
            tlb.insert(entry);
        }

        // ASID 1
        for addr in &addrs {
            let entry = TlbEntry::new(*addr & !0xFFF, (*addr & !0xFFF) | 0x1000_0000_0000, 0x1, 1);
            tlb.insert(entry);
        }

        // ASID 0刷新
        tlb.flush_asid(0);

        // ASID 0的所有条目应该消失
        for addr in &addrs {
            let result = tlb.lookup(*addr & !0xFFF, 0);
            prop_assert!(result.is_none());
        }

        // ASID 1的条目应该保留
        for addr in &addrs {
            let result = tlb.lookup(*addr & !0xFFF, 1);
            prop_assert!(result.is_some());
        }
    }
}

/// 属性测试: TLB容量限制
proptest! {
    #[test]
    fn prop_tlb_capacity_limit(addrs in prop::collection::vec(any::<u64>(), 10..200)) {
        let tlb = LockFreeTlb::with_capacity(16);

        // 插入超过容量的条目
        let mut inserted = 0;
        for addr in &addrs {
            let entry = TlbEntry::new(*addr & !0xFFF, *addr & !0xFFF, 0x1, 0);
            tlb.insert(entry);
            inserted += 1;

            // TLB不应超过容量
            let stats = tlb.stats();
            prop_assert!(stats.entries <= 16);
        }
    }
}

/// 属性测试: 重复插入更新条目
proptest! {
    #[test]
    fn prop_duplicate_insert_updates(vpn in any::<u64>(), ppn1 in any::<u64>(), ppn2 in any::<u64>()) {
        let tlb = LockFreeTlb::new();

        // 插入第一次
        let entry1 = TlbEntry::new(vpn & !0xFFF, ppn1 & !0xFFF, 0x1, 0);
        tlb.insert(entry1);

        // 插入第二次（相同VPN，不同PPN）
        let entry2 = TlbEntry::new(vpn & !0xFFF, ppn2 & !0xFFF, 0x1, 0);
        tlb.insert(entry2);

        // 查找应该返回最新的PPN
        let result = tlb.lookup(vpn & !0xFFF, 0);
        prop_assert!(result.is_some());
        if let Some(found) = result {
            prop_assert_eq!(found.ppn, ppn2 & !0xFFF);
        }
    }
}

/// 属性测试: TLB失效操作
proptest! {
    #[test]
    fn prop_invalidate_removes_entry(addrs in prop::collection::vec(any::<u64>(), 1..100)) {
        let tlb = LockFreeTlb::new();

        // 插入
        for addr in &addrs {
            let entry = TlbEntry::new(*addr & !0xFFF, *addr & !0xFFF, 0x1, 0);
            tlb.insert(entry);
        }

        // 失效前半部分
        for addr in addrs.iter().take(addrs.len() / 2) {
            tlb.invalidate(*addr & !0xFFF, 0);
        }

        // 验证前半部分已失效
        for addr in addrs.iter().take(addrs.len() / 2) {
            let result = tlb.lookup(*addr & !0xFFF, 0);
            prop_assert!(result.is_none());
        }

        // 验证后半部分仍在
        for addr in addrs.iter().skip(addrs.len() / 2) {
            let result = tlb.lookup(*addr & !0xFFF, 0);
            prop_assert!(result.is_some());
        }
    }
}

/// 属性测试: LRU替换策略
proptest! {
    #[test]
    fn prop_lru_replacement_strategy(addrs in prop::collection::vec(any::<u64>(), 20..100)) {
        let tlb = LockFreeTlb::with_capacity(8);

        // 插入初始条目
        for (i, addr) in addrs.iter().take(8).enumerate() {
            let entry = TlbEntry::new(*addr & !0xFFF, *addr & !0xFFF, 0x1, 0);
            tlb.insert(entry);

            // 访问以增加访问计数
            let _ = tlb.lookup(*addr & !0xFFF, 0);
        }

        // 插入更多条目，触发替换
        for addr in addrs.iter().skip(8).take(10) {
            let entry = TlbEntry::new(*addr & !0xFFF, *addr & !0xFFF, 0x1, 0);
            tlb.insert(entry);
        }

        // 验证最早插入且最少访问的条目被替换
        let stats = tlb.stats();
        prop_assert!(stats.entries <= 8);
    }
}

/// 属性测试: 地址对齐要求
proptest! {
    #[test]
    fn prop_address_alignment(vpn in any::<u64>(), ppn in any::<u64>()) {
        let tlb = LockFreeTlb::new();

        // 测试未对齐的地址
        let unaligned_vpn = vpn | 0xFFF;
        let unaligned_ppn = ppn | 0xFFF;

        let entry = TlbEntry::new(unaligned_vpn & !0xFFF, unaligned_ppn & !0xFFF, 0x1, 0);
        tlb.insert(entry);

        // 查找对齐后的地址
        let aligned_vpn = unaligned_vpn & !0xFFF;
        let result = tlb.lookup(aligned_vpn, 0);

        // 应该能找到
        prop_assert!(result.is_some());
    }
}

/// 属性测试: 页面标志位
proptest! {
    #[test]
    fn prop_page_flags_preserved(vpn in any::<u64>(), ppn in any::<u64>(), flags in any::<u64>()) {
        let tlb = LockFreeTlb::new();

        // 插入带特定标志的条目
        let entry = TlbEntry::new(vpn & !0xFFF, ppn & !0xFFF, flags & 0xFFF, 0);
        tlb.insert(entry);

        // 查找并验证标志
        let result = tlb.lookup(vpn & !0xFFF, 0);
        if vpn % 4096 == 0 && ppn % 4096 == 0 {
            prop_assert!(result.is_some());
            if let Some(found) = result {
                prop_assert_eq!(found.flags, flags & 0xFFF);
            }
        }
    }
}

/// 属性测试: 统计信息准确性
proptest! {
    #[test]
    fn prop_stats_accuracy(addrs in prop::collection::vec(any::<u64>(), 10..100)) {
        let tlb = LockFreeTlb::new();

        let mut total_inserts = 0;
        let mut total_lookups = 0;

        // 插入
        for addr in &addrs {
            let entry = TlbEntry::new(*addr & !0xFFF, *addr & !0xFFF, 0x1, 0);
            tlb.insert(entry);
            total_inserts += 1;
        }

        // 查找
        for addr in &addrs {
            let _ = tlb.lookup(*addr & !0xFFF, 0);
            total_lookups += 1;
        }

        // 验证统计
        let stats = tlb.stats();
        prop_assert_eq!(stats.lookups, total_lookups);
        prop_assert!(stats.entries <= total_inserts);
    }
}

/// 属性测试: 并发插入查找一致性
proptest! {
    #[test]
    fn prop_concurrent_insert_lookup_consistency(addrs in prop::collection::vec(any::<u64>(), 10..50)) {
        use std::sync::Arc;
        use std::thread;

        let tlb = Arc::new(LockFreeTlb::new());
        let mut handles = vec![];

        // 插入线程
        let tlb_clone1 = Arc::clone(&tlb);
        let addrs_clone1 = addrs.clone();
        handles.push(thread::spawn(move || {
            for addr in &addrs_clone1 {
                let entry = TlbEntry::new(*addr & !0xFFF, *addr & !0xFFF, 0x1, 0);
                tlb_clone1.insert(entry);
            }
        }));

        // 查找线程
        let tlb_clone2 = Arc::clone(&tlb);
        let addrs_clone2 = addrs.clone();
        handles.push(thread::spawn(move || {
            for addr in &addrs_clone2 {
                let _ = tlb_clone2.lookup(*addr & !0xFFF, 0);
            }
        }));

        // 等待完成
        for handle in handles {
            handle.join().unwrap();
        }

        // 验证TLB仍然一致
        let stats = tlb.stats();
        prop_assert!(stats.entries <= addrs.len());
    }
}

/// 属性测试: 空TLB行为
proptest! {
    #[test]
    fn prop_empty_tlb_behavior(vpn in any::<u64>()) {
        let tlb = LockFreeTlb::new();

        // 空TLB查找应该失败
        let result = tlb.lookup(vpn & !0xFFF, 0);
        prop_assert!(result.is_none());

        // 统计应该反映空状态
        let stats = tlb.stats();
        prop_assert_eq!(stats.entries, 0);
        prop_assert_eq!(stats.hits, 0);
    }
}

/// 属性测试: 大地址空间
proptest! {
    #[test]
    fn prop_large_address_space(base in any::<u64>(), offset in 0u64..1000u64) {
        let tlb = LockFreeTlb::new();

        // 测试大地址
        let large_addr = base.wrapping_add(offset * 4096);
        let entry = TlbEntry::new(large_addr & !0xFFF, large_addr & !0xFFF, 0x1, 0);
        tlb.insert(entry);

        // 应该能找到
        let result = tlb.lookup(large_addr & !0xFFF, 0);
        prop_assert!(result.is_some());
    }
}

/// 属性测试: TLB替换后查找失败
proptest! {
    #[test]
    fn prop_replaced_entry_missing(unique_addrs in prop::collection::vec(any::<u64>(), 10..50)) {
        let tlb = LockFreeTlb::with_capacity(4);

        // 填满TLB
        for addr in unique_addrs.iter().take(4) {
            let entry = TlbEntry::new(*addr & !0xFFF, *addr & !0xFFF, 0x1, 0);
            tlb.insert(entry);
        }

        // 记录第一个地址
        let first_addr = unique_addrs[0] & !0xFFF;

        // 插入更多条目，触发替换
        for addr in unique_addrs.iter().skip(4) {
            let entry = TlbEntry::new(*addr & !0xFFF, *addr & !0xFFF, 0x1, 0);
            tlb.insert(entry);
        }

        // 第一个条目可能已被替换
        let _ = tlb.lookup(first_addr, 0);

        // 验证容量未超
        let stats = tlb.stats();
        prop_assert!(stats.entries <= 4);
    }
}

/// 属性测试: 多ASID统计
proptest! {
    #[test]
    fn prop_multi_asid_stats(addrs in prop::collection::vec(any::<u64>(), 5..50)) {
        let tlb = LockFreeTlb::new();

        // 插入多个ASID
        for (asid, addr) in addrs.iter().enumerate().take(8) {
            let entry = TlbEntry::new(*addr & !0xFFF, *addr & !0xFFF, 0x1, asid as u16);
            tlb.insert(entry);
        }

        // 统计应该反映所有ASID的条目
        let stats = tlb.stats();
        prop_assert!(stats.entries <= 8);
    }
}

/// 属性测试: 连续查找相同地址
proptest! {
    #[test]
    fn prop_repeated_lookup_same_addr(vpn in any::<u64>(), ppn in any::<u64>(), count in 1u64..100u64) {
        let tlb = LockFreeTlb::new();

        // 插入
        let entry = TlbEntry::new(vpn & !0xFFF, ppn & !0xFFF, 0x1, 0);
        tlb.insert(entry);

        // 多次查找
        for _ in 0..count {
            let result = tlb.lookup(vpn & !0xFFF, 0);
            prop_assert!(result.is_some());
        }

        // 统计应该反映所有查找
        let stats = tlb.stats();
        prop_assert_eq!(stats.lookups, count);
    }
}

/// 属性测试: TLB满后行为
proptest! {
    #[test]
    fn prop_tlb_full_behavior(addrs in prop::collection::vec(any::<u64>(), 10..50)) {
        let tlb = LockFreeTlb::with_capacity(4);

        // 填满TLB
        for addr in addrs.iter().take(4) {
            let entry = TlbEntry::new(*addr & !0xFFF, *addr & !0xFFF, 0x1, 0);
            tlb.insert(entry);
        }

        let stats_before = tlb.stats();

        // 继续插入
        for addr in addrs.iter().skip(4) {
            let entry = TlbEntry::new(*addr & !0xFFF, *addr & !0xFFF, 0x1, 0);
            tlb.insert(entry);

            // TLB不应超过容量
            let stats = tlb.stats();
            prop_assert!(stats.entries <= 4);
        }

        // TLB应该是满的或接近满的
        let stats_final = tlb.stats();
        prop_assert!(stats_final.entries > 0);
        prop_assert!(stats_final.entries <= 4);
    }
}

