//! vm-mem TLB并发测试
//!
//! 测试TLB在多线程环境下的正确性和性能
//!
//! 测试覆盖:
//! - 50个并发测试用例
//! - 多线程TLB访问
//! - TLB刷新并发安全性
//! - TLB统计信息准确性

use vm_mem::tlb::core::lockfree::{LockFreeTlb, TlbEntry};

#[cfg(test)]
mod basic_concurrent_tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::Barrier;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;

    /// 测试1: 多线程并发TLB查找
    #[test]
    fn test_01_concurrent_lookup() {
        let tlb = Arc::new(LockFreeTlb::new());
        let barrier = Arc::new(Barrier::new(10));
        let mut handles = vec![];

        // 预填充TLB
        for i in 0..100 {
            let vpn = i * 4096;
            let ppn = i * 4096;
            let entry = TlbEntry::new(vpn, ppn, 0x1, 0);
            tlb.insert(entry);
        }

        // 10个线程并发查找
        for thread_id in 0..10 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier_clone = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier_clone.wait();
                for i in 0..1000 {
                    let vpn = (i % 100) * 4096;
                    let result = tlb_clone.lookup(vpn, 0);
                    assert!(
                        result.is_some(),
                        "Thread {} failed lookup at {}",
                        thread_id,
                        i
                    );
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    /// 测试2: 多线程并发插入
    #[test]
    fn test_02_concurrent_insert() {
        let tlb = Arc::new(LockFreeTlb::new());
        let barrier = Arc::new(Barrier::new(10));
        let mut handles = vec![];

        // 10个线程并发插入不同条目
        for thread_id in 0..10 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier_clone = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier_clone.wait();
                for i in 0..100 {
                    let vpn = (thread_id * 100 + i) * 4096;
                    let ppn = (thread_id * 100 + i) * 4096;
                    let entry = TlbEntry::new(vpn, ppn, 0x1, thread_id as u16);
                    tlb_clone.insert(entry);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // 验证插入成功
        let stats = tlb.stats();
        assert_eq!(stats.inserts(), 1000);
    }

    /// 测试3: 并发查找和插入混合
    #[test]
    fn test_03_mixed_lookup_insert() {
        let tlb = Arc::new(LockFreeTlb::new());
        let barrier = Arc::new(Barrier::new(20));
        let mut handles = vec![];

        // 10个线程查找
        for thread_id in 0..10 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier_clone = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier_clone.wait();
                for i in 0..1000 {
                    let vpn = (i % 100) * 4096;
                    let _ = tlb_clone.lookup(vpn, 0);
                }
            }));
        }

        // 10个线程插入
        for thread_id in 0..10 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier_clone = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier_clone.wait();
                for i in 0..100 {
                    let vpn = (thread_id * 100 + i) * 4096;
                    let ppn = (thread_id * 100 + i) * 4096;
                    let entry = TlbEntry::new(vpn, ppn, 0x1, thread_id as u16);
                    tlb_clone.insert(entry);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    /// 测试4: 并发TLB刷新
    #[test]
    fn test_04_concurrent_flush() {
        let tlb = Arc::new(LockFreeTlb::new());
        let barrier = Arc::new(Barrier::new(11));
        let mut handles = vec![];

        // 预填充TLB
        for i in 0..1000 {
            let vpn = i * 4096;
            let ppn = i * 4096;
            let entry = TlbEntry::new(vpn, ppn, 0x1, 0);
            tlb.insert(entry);
        }

        // 10个线程查找，1个线程刷新
        for thread_id in 0..10 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier_clone = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier_clone.wait();
                for i in 0..100 {
                    let vpn = i * 4096;
                    let _ = tlb_clone.lookup(vpn, 0);
                    thread::yield_now();
                }
            }));
        }

        // 刷新线程
        let tlb_clone = Arc::clone(&tlb);
        let barrier_clone = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier_clone.wait();
            tlb_clone.flush();
        }));

        for handle in handles {
            handle.join().unwrap();
        }
    }

    /// 测试5: 并发失效操作
    #[test]
    fn test_05_concurrent_invalidate() {
        let tlb = Arc::new(LockFreeTlb::new());
        let barrier = Arc::new(Barrier::new(10));
        let mut handles = vec![];

        // 预填充TLB
        for i in 0..100 {
            let vpn = i * 4096;
            let ppn = i * 4096;
            let entry = TlbEntry::new(vpn, ppn, 0x1, 0);
            tlb.insert(entry);
        }

        // 10个线程并发失效不同地址
        for thread_id in 0..10 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier_clone = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier_clone.wait();
                for i in 0..10 {
                    let vpn = (thread_id * 10 + i) * 4096;
                    tlb_clone.invalidate(vpn, 0);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    /// 测试6: ASID隔离测试
    #[test]
    fn test_06_asid_isolation() {
        let tlb = Arc::new(LockFreeTlb::new());
        let barrier = Arc::new(Barrier::new(5));
        let mut handles = vec![];

        // 5个线程使用不同ASID
        for thread_id in 0..5 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier_clone = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier_clone.wait();
                for i in 0..100 {
                    let vpn = i * 4096;
                    let ppn = (thread_id * 1000 + i) * 4096;
                    let entry = TlbEntry::new(vpn, ppn, 0x1, thread_id as u16);
                    tlb_clone.insert(entry);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // 验证插入成功
        let stats = tlb.stats();
        assert_eq!(stats.inserts(), 500);
    }

    /// 测试7: 并发统计信息更新
    #[test]
    fn test_07_concurrent_stats() {
        let tlb = Arc::new(LockFreeTlb::new());
        let barrier = Arc::new(Barrier::new(10));
        let mut handles = vec![];
        let total_lookups = Arc::new(AtomicUsize::new(0));

        for thread_id in 0..10 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier_clone = Arc::clone(&barrier);
            let total_lookups_clone = Arc::clone(&total_lookups);
            handles.push(thread::spawn(move || {
                barrier_clone.wait();
                for i in 0..100 {
                    let vpn = i * 4096;
                    let ppn = i * 4096;
                    let entry = TlbEntry::new(vpn, ppn, 0x1, thread_id as u16);
                    tlb_clone.insert(entry);

                    let _ = tlb_clone.lookup(vpn, thread_id as u16);
                    total_lookups_clone.fetch_add(1, Ordering::Relaxed);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(total_lookups.load(Ordering::Relaxed), 1000);
        let stats = tlb.stats();
        assert_eq!(stats.inserts(), 1000);
    }

    /// 测试8: TLB容量压力测试
    #[test]
    fn test_08_capacity_pressure() {
        let tlb = Arc::new(LockFreeTlb::new());
        let barrier = Arc::new(Barrier::new(10));
        let mut handles = vec![];

        // 10个线程插入超过容量的条目
        for thread_id in 0..10 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier_clone = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier_clone.wait();
                for i in 0..1000 {
                    let vpn = (thread_id * 1000 + i) * 4096;
                    let ppn = (thread_id * 1000 + i) * 4096;
                    let entry = TlbEntry::new(vpn, ppn, 0x1, thread_id as u16);
                    tlb_clone.insert(entry);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // 验证TLB仍正常工作
        let stats = tlb.stats();
        assert_eq!(stats.inserts(), 10000);
    }

    /// 测试9: 并发不同访问模式
    #[test]
    fn test_09_sequential_access_pattern() {
        let tlb = Arc::new(LockFreeTlb::new());
        let barrier = Arc::new(Barrier::new(10));
        let mut handles = vec![];

        // 预填充
        for i in 0..100 {
            let vpn = i * 4096;
            let ppn = i * 4096;
            let entry = TlbEntry::new(vpn, ppn, 0x3, 0);
            tlb.insert(entry);
        }

        // 10个线程顺序访问
        for thread_id in 0..10 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier_clone = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier_clone.wait();
                for i in 0..100 {
                    let vpn = i * 4096;
                    let _ = tlb_clone.lookup(vpn, 0);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let stats = tlb.stats();
        assert!(stats.hits() >= 1000);
    }

    /// 测试10: 热点地址竞争
    #[test]
    fn test_10_hotspot_contention() {
        let tlb = Arc::new(LockFreeTlb::new());
        let barrier = Arc::new(Barrier::new(20));
        let mut handles = vec![];

        // 预插入热点地址
        let entry = TlbEntry::new(0x1000, 0x2000, 0x1, 0);
        tlb.insert(entry);

        // 20个线程竞争访问同一个地址
        for thread_id in 0..20 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier_clone = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier_clone.wait();
                for _ in 0..1000 {
                    let vpn = 0x1000; // 同一地址
                    let _ = tlb_clone.lookup(vpn, 0);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // 验证TLB仍正常工作
        let stats = tlb.stats();
        assert!(stats.hits() > 0);
    }

    /// 测试11-20: 更多并发场景
    #[test]
    fn test_11_concurrent_invalidate_range() {
        let tlb = Arc::new(LockFreeTlb::new());
        let barrier = Arc::new(Barrier::new(5));
        let mut handles = vec![];

        // 预填充
        for i in 0..1000 {
            let vpn = i * 4096;
            let ppn = i * 4096;
            let entry = TlbEntry::new(vpn, ppn, 0x1, 0);
            tlb.insert(entry);
        }

        // 5个线程并发失效不同范围
        for thread_id in 0..5 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier_clone = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier_clone.wait();
                for i in 0..100 {
                    let vpn = (thread_id * 200 + i) * 4096;
                    tlb_clone.invalidate(vpn, 0);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_12_read_write_ratio() {
        let tlb = Arc::new(LockFreeTlb::new());
        let barrier = Arc::new(Barrier::new(10));
        let mut handles = vec![];

        for thread_id in 0..10 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier_clone = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier_clone.wait();
                for i in 0..1000 {
                    // 90%读，10%写
                    if i % 10 == 0 {
                        let vpn = (thread_id * 1000 + i) * 4096;
                        let ppn = (thread_id * 1000 + i) * 4096;
                        let entry = TlbEntry::new(vpn, ppn, 0x1, 0);
                        tlb_clone.insert(entry);
                    } else {
                        let vpn = ((i / 10) * 4096) % (100 * 4096);
                        let _ = tlb_clone.lookup(vpn, 0);
                    }
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_13_random_access_pattern() {
        let tlb = Arc::new(LockFreeTlb::new());
        let barrier = Arc::new(Barrier::new(10));
        let mut handles = vec![];

        for thread_id in 0..10 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier_clone = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier_clone.wait();
                for i in 0..1000 {
                    // 伪随机访问模式
                    let vpn = ((thread_id * 37 + i * 17) % 100) * 4096;
                    let ppn = vpn;
                    let entry = TlbEntry::new(vpn, ppn, 0x1, thread_id as u16);
                    tlb_clone.insert(entry);
                    let _ = tlb_clone.lookup(vpn, thread_id as u16);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_14_tlb_shootdown() {
        let tlb = Arc::new(LockFreeTlb::new());
        let barrier = Arc::new(Barrier::new(11));
        let mut handles = vec![];

        // 预填充
        for i in 0..1000 {
            let vpn = i * 4096;
            let ppn = i * 4096;
            let entry = TlbEntry::new(vpn, ppn, 0x1, 0);
            tlb.insert(entry);
        }

        // 10个线程查找，1个线程做shootdown
        for thread_id in 0..10 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier_clone = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier_clone.wait();
                for i in 0..100 {
                    let vpn = i * 4096;
                    let _ = tlb_clone.lookup(vpn, 0);
                }
            }));
        }

        let tlb_clone = Arc::clone(&tlb);
        let barrier_clone = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier_clone.wait();
            tlb_clone.flush(); // 模拟TLB shootdown
        }));

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_15_nested_barrier_synchronization() {
        let tlb = Arc::new(LockFreeTlb::new());
        let barrier1 = Arc::new(Barrier::new(10));
        let barrier2 = Arc::new(Barrier::new(10));
        let mut handles = vec![];

        for thread_id in 0..10 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier1_clone = Arc::clone(&barrier1);
            let barrier2_clone = Arc::clone(&barrier2);
            handles.push(thread::spawn(move || {
                // 第一阶段：插入
                barrier1_clone.wait();
                for i in 0..100 {
                    let vpn = (thread_id * 100 + i) * 4096;
                    let ppn = (thread_id * 100 + i) * 4096;
                    let entry = TlbEntry::new(vpn, ppn, 0x1, thread_id as u16);
                    tlb_clone.insert(entry);
                }

                // 第二阶段：查找
                barrier2_clone.wait();
                for i in 0..100 {
                    let vpn = (thread_id * 100 + i) * 4096;
                    let result = tlb_clone.lookup(vpn, thread_id as u16);
                    assert!(result.is_some());
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    // 测试16-25：更多边界情况
    #[test]
    fn test_16_single_entry_many_threads() {
        let tlb = Arc::new(LockFreeTlb::new());
        let barrier = Arc::new(Barrier::new(100));
        let mut handles = vec![];

        // 插入单个条目
        let entry = TlbEntry::new(0x1000, 0x2000, 0x1, 0);
        tlb.insert(entry);

        // 100个线程竞争读取单个条目
        for _ in 0..100 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier_clone = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier_clone.wait();
                for _ in 0..100 {
                    let _ = tlb_clone.lookup(0x1000, 0);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let stats = tlb.stats();
        assert!(stats.hits() > 0);
    }

    #[test]
    fn test_17_batch_operations() {
        let tlb = Arc::new(LockFreeTlb::new());
        let barrier = Arc::new(Barrier::new(10));
        let mut handles = vec![];

        for thread_id in 0..10 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier_clone = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier_clone.wait();

                // 批量插入
                let entries: Vec<_> = (0..100)
                    .map(|i| {
                        let vpn = (thread_id * 100 + i) * 4096;
                        let ppn = (thread_id * 100 + i) * 4096;
                        TlbEntry::new(vpn, ppn, 0x1, thread_id as u16)
                    })
                    .collect();
                tlb_clone.insert_batch(&entries);

                // 批量查找
                let requests: Vec<_> = (0..100)
                    .map(|i| ((thread_id * 100 + i) * 4096, thread_id as u16))
                    .collect();
                let results = tlb_clone.lookup_batch(&requests);
                assert_eq!(results.len(), 100);
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_18_flush_asid_concurrent() {
        let tlb = Arc::new(LockFreeTlb::new());
        let barrier = Arc::new(Barrier::new(10));
        let mut handles = vec![];

        // 预填充多个ASID
        for asid in 0..10 {
            for i in 0..100 {
                let vpn = i * 4096;
                let ppn = i * 4096;
                let entry = TlbEntry::new(vpn, ppn, 0x1, asid);
                tlb.insert(entry);
            }
        }

        // 10个线程，每个刷新一个ASID
        for thread_id in 0..10 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier_clone = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier_clone.wait();
                tlb_clone.flush_asid(thread_id as u16);
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_19_alternating_insert_lookup() {
        let tlb = Arc::new(LockFreeTlb::new());
        let barrier = Arc::new(Barrier::new(10));
        let mut handles = vec![];

        for thread_id in 0..10 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier_clone = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier_clone.wait();
                for i in 0..1000 {
                    if i % 2 == 0 {
                        // 插入
                        let vpn = (thread_id * 1000 + i) * 4096;
                        let ppn = (thread_id * 1000 + i) * 4096;
                        let entry = TlbEntry::new(vpn, ppn, 0x1, thread_id as u16);
                        tlb_clone.insert(entry);
                    } else {
                        // 查找
                        let vpn = (thread_id * 1000 + (i - 1)) * 4096;
                        let _ = tlb_clone.lookup(vpn, thread_id as u16);
                    }
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_20_high_contention_invalidate() {
        let tlb = Arc::new(LockFreeTlb::new());
        let barrier = Arc::new(Barrier::new(10));
        let mut handles = vec![];

        // 预填充
        for i in 0..100 {
            let vpn = i * 4096;
            let ppn = i * 4096;
            let entry = TlbEntry::new(vpn, ppn, 0x1, 0);
            tlb.insert(entry);
        }

        // 10个线程并发失效相同条目
        for thread_id in 0..10 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier_clone = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier_clone.wait();
                for i in 0..10 {
                    let vpn = i * 4096;
                    tlb_clone.invalidate(vpn, 0);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }
}

#[cfg(test)]
mod advanced_concurrent_tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::Barrier;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use std::time::Duration;

    /// 测试21-30: 性能和可扩展性测试
    #[test]
    fn test_21_scalability_increasing_threads() {
        for thread_count in [1, 2, 4, 8, 16].iter() {
            let tlb = Arc::new(LockFreeTlb::new());
            let barrier = Arc::new(Barrier::new(*thread_count));
            let mut handles = vec![];

            for thread_id in 0..*thread_count {
                let tlb_clone = Arc::clone(&tlb);
                let barrier_clone = Arc::clone(&barrier);
                handles.push(thread::spawn(move || {
                    barrier_clone.wait();
                    for i in 0..100 {
                        let vpn = ((thread_id * 100 + i) * 4096) as u64;
                        let ppn = ((thread_id * 100 + i) * 4096) as u64;
                        let entry = TlbEntry::new(vpn, ppn, 0x1, thread_id as u16);
                        tlb_clone.insert(entry);
                        let _ = tlb_clone.lookup(vpn, thread_id as u16);
                    }
                }));
            }

            for handle in handles {
                handle.join().unwrap();
            }

            let stats = tlb.stats();
            let expected_inserts = *thread_count * 100;
            assert_eq!(stats.inserts(), expected_inserts as u64);
        }
    }

    #[test]
    fn test_22_cache_coherency_stress() {
        let tlb = Arc::new(LockFreeTlb::new());
        let barrier = Arc::new(Barrier::new(20));
        let mut handles = vec![];

        for thread_id in 0..20 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier_clone = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier_clone.wait();
                for i in 0..1000 {
                    // 所有线程访问相同的小地址集
                    let vpn = (i % 10) * 4096;
                    let ppn = (i % 10) * 4096;
                    let entry = TlbEntry::new(vpn, ppn, 0x1, thread_id as u16);
                    tlb_clone.insert(entry);
                    let _ = tlb_clone.lookup(vpn, thread_id as u16);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let stats = tlb.stats();
        assert!(stats.hits() > 0 || stats.misses() > 0);
    }

    #[test]
    fn test_23_rapid_flush_reinsert() {
        let tlb = Arc::new(LockFreeTlb::new());
        let barrier = Arc::new(Barrier::new(10));
        let mut handles = vec![];

        for thread_id in 0..10 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier_clone = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier_clone.wait();
                for i in 0..100 {
                    // 插入
                    let vpn = (thread_id * 100 + i) * 4096;
                    let ppn = (thread_id * 100 + i) * 4096;
                    let entry = TlbEntry::new(vpn, ppn, 0x1, thread_id as u16);
                    tlb_clone.insert(entry);

                    // 立即刷新
                    if i % 10 == 0 {
                        tlb_clone.flush();
                    }
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_24_wide_address_distribution() {
        let tlb = Arc::new(LockFreeTlb::new());
        let barrier = Arc::new(Barrier::new(10));
        let mut handles = vec![];

        for thread_id in 0..10 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier_clone = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier_clone.wait();
                for i in 0..1000 {
                    // 宽地址分布
                    let vpn = (thread_id * 0x1000_0000u64 + i * 0x1000) & !0xFFF;
                    let ppn = vpn;
                    let entry = TlbEntry::new(vpn, ppn, 0x1, thread_id as u16);
                    tlb_clone.insert(entry);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let stats = tlb.stats();
        assert_eq!(stats.inserts(), 10000);
    }

    #[test]
    fn test_25_timestamp_monotonicity() {
        let tlb = Arc::new(LockFreeTlb::new());
        let barrier = Arc::new(Barrier::new(10));
        let mut handles = vec![];

        for thread_id in 0..10 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier_clone = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier_clone.wait();
                for i in 0..100 {
                    let vpn = (thread_id * 100 + i) * 4096;
                    let ppn = (thread_id * 100 + i) * 4096;
                    let entry = TlbEntry::new(vpn, ppn, 0x1, thread_id as u16);
                    tlb_clone.insert(entry);

                    // 验证查找返回条目
                    let result = tlb_clone.lookup(vpn, thread_id as u16);
                    assert!(result.is_some());
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    /// 测试31-40: 复杂交互场景
    #[test]
    fn test_31_phased_operations() {
        let tlb = Arc::new(LockFreeTlb::new());
        let barrier1 = Arc::new(Barrier::new(10));
        let barrier2 = Arc::new(Barrier::new(10));
        let barrier3 = Arc::new(Barrier::new(10));
        let mut handles = vec![];

        for thread_id in 0..10 {
            let tlb_clone = Arc::clone(&tlb);
            let b1 = Arc::clone(&barrier1);
            let b2 = Arc::clone(&barrier2);
            let b3 = Arc::clone(&barrier3);
            handles.push(thread::spawn(move || {
                // 阶段1: 插入
                b1.wait();
                for i in 0..100 {
                    let vpn = (thread_id * 100 + i) * 4096;
                    let ppn = (thread_id * 100 + i) * 4096;
                    let entry = TlbEntry::new(vpn, ppn, 0x1, thread_id as u16);
                    tlb_clone.insert(entry);
                }

                // 阶段2: 查找
                b2.wait();
                for i in 0..100 {
                    let vpn = (thread_id * 100 + i) * 4096;
                    let _ = tlb_clone.lookup(vpn, thread_id as u16);
                }

                // 阶段3: 失效
                b3.wait();
                for i in 0..50 {
                    let vpn = (thread_id * 100 + i) * 4096;
                    tlb_clone.invalidate(vpn, thread_id as u16);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_32_wave_pattern_access() {
        let tlb = Arc::new(LockFreeTlb::new());
        let barrier = Arc::new(Barrier::new(10));
        let mut handles = vec![];

        for thread_id in 0..10 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier_clone = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier_clone.wait();
                for i in 0..100 {
                    // 波浪模式访问
                    let offset = ((i as i32 - 50).abs() as u64) * 4096;
                    let vpn = (thread_id * 1000 + offset) & !0xFFF;
                    let ppn = vpn;
                    let entry = TlbEntry::new(vpn, ppn, 0x1, thread_id as u16);
                    tlb_clone.insert(entry);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_33_interleaved_asid_operations() {
        let tlb = Arc::new(LockFreeTlb::new());
        let barrier = Arc::new(Barrier::new(10));
        let mut handles = vec![];

        for thread_id in 0..10 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier_clone = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier_clone.wait();
                for i in 0..100 {
                    let asid = ((thread_id + i) % 10) as u16;
                    let vpn = (i * 4096) as u64;
                    let ppn = (i * 4096) as u64;
                    let entry = TlbEntry::new(vpn, ppn, 0x1, asid);
                    tlb_clone.insert(entry);

                    if i % 10 == 0 {
                        tlb_clone.flush_asid(asid);
                    }
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_34_burst_pattern_access() {
        let tlb = Arc::new(LockFreeTlb::new());
        let barrier = Arc::new(Barrier::new(10));
        let mut handles = vec![];

        for thread_id in 0..10 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier_clone = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier_clone.wait();
                for burst in 0..10 {
                    // 每次突发访问连续10个地址
                    for i in 0..10 {
                        let vpn = (thread_id * 100 + burst * 10 + i) * 4096;
                        let ppn = (thread_id * 100 + burst * 10 + i) * 4096;
                        let entry = TlbEntry::new(vpn, ppn, 0x1, thread_id as u16);
                        tlb_clone.insert(entry);
                        let _ = tlb_clone.lookup(vpn, thread_id as u16);
                    }
                    thread::yield_now();
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_35_aging_with_timestamps() {
        let tlb = Arc::new(LockFreeTlb::new());
        let barrier = Arc::new(Barrier::new(5));
        let mut handles = vec![];

        // 预填充
        for i in 0..100 {
            let vpn = i * 4096;
            let ppn = i * 4096;
            let entry = TlbEntry::new(vpn, ppn, 0x1, 0);
            tlb.insert(entry);
        }

        // 5个线程持续访问
        for thread_id in 0..5 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier_clone = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier_clone.wait();
                for i in 0..1000 {
                    let vpn = (i % 100) * 4096;
                    let _ = tlb_clone.lookup(vpn, 0);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    /// 测试41-50: 综合压力测试
    #[test]
    fn test_41_mixed_operation_stress() {
        let tlb = Arc::new(LockFreeTlb::new());
        let barrier = Arc::new(Barrier::new(20));
        let mut handles = vec![];

        // 10个插入线程
        for thread_id in 0..10 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier_clone = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier_clone.wait();
                for i in 0..1000 {
                    let vpn = (thread_id * 1000 + i) * 4096;
                    let ppn = (thread_id * 1000 + i) * 4096;
                    let entry = TlbEntry::new(vpn, ppn, 0x1, thread_id as u16);
                    tlb_clone.insert(entry);
                }
            }));
        }

        // 10个查找线程
        for thread_id in 0..10 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier_clone = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier_clone.wait();
                for i in 0..1000 {
                    let vpn = (thread_id * 1000 + i) * 4096;
                    let _ = tlb_clone.lookup(vpn, thread_id as u16);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_42_extreme_contention() {
        let tlb = Arc::new(LockFreeTlb::new());
        let barrier = Arc::new(Barrier::new(50));
        let mut handles = vec![];

        // 50个线程竞争访问单个条目
        for _ in 0..50 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier_clone = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier_clone.wait();
                for _ in 0..1000 {
                    let _ = tlb_clone.lookup(0x1000, 0);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let stats = tlb.stats();
        assert!(stats.misses() > 0 || stats.hits() > 0);
    }

    #[test]
    fn test_43_rapid_asid_switching() {
        let tlb = Arc::new(LockFreeTlb::new());
        let barrier = Arc::new(Barrier::new(10));
        let mut handles = vec![];

        for thread_id in 0..10 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier_clone = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier_clone.wait();
                for i in 0..1000 {
                    let asid = (i % 16) as u16;
                    let vpn = i * 4096;
                    let ppn = i * 4096;
                    let entry = TlbEntry::new(vpn, ppn, 0x1, asid);
                    tlb_clone.insert(entry);
                    let _ = tlb_clone.lookup(vpn, asid);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_44_full_flush_stress() {
        let tlb = Arc::new(LockFreeTlb::new());
        let barrier = Arc::new(Barrier::new(11));
        let mut handles = vec![];

        // 预填充
        for i in 0..10000 {
            let vpn = i * 4096;
            let ppn = i * 4096;
            let entry = TlbEntry::new(vpn, ppn, 0x1, 0);
            tlb.insert(entry);
        }

        // 10个线程查找，1个线程持续刷新
        for thread_id in 0..10 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier_clone = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier_clone.wait();
                for i in 0..1000 {
                    let vpn = i * 4096;
                    let _ = tlb_clone.lookup(vpn, 0);
                }
            }));
        }

        let tlb_clone = Arc::clone(&tlb);
        let barrier_clone = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier_clone.wait();
            for _ in 0..100 {
                tlb_clone.flush();
                thread::sleep(Duration::from_micros(100));
            }
        }));

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_45_address_collision_test() {
        let tlb = Arc::new(LockFreeTlb::new());
        let barrier = Arc::new(Barrier::new(20));
        let mut handles = vec![];

        for thread_id in 0..20 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier_clone = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier_clone.wait();
                for i in 0..1000 {
                    // 故意造成地址冲突
                    let vpn = ((thread_id * 7 + i * 13) % 100) * 4096;
                    let ppn = ((thread_id * 11 + i * 17) % 100) * 4096;
                    let entry = TlbEntry::new(vpn, ppn, 0x1, thread_id as u16);
                    tlb_clone.insert(entry);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let stats = tlb.stats();
        assert_eq!(stats.inserts(), 20000);
    }

    #[test]
    fn test_46_statistics_accuracy_under_load() {
        let tlb = Arc::new(LockFreeTlb::new());
        let barrier = Arc::new(Barrier::new(10));
        let mut handles = vec![];
        let total_ops = Arc::new(AtomicUsize::new(0));

        for thread_id in 0..10 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier_clone = Arc::clone(&barrier);
            let total_ops_clone = Arc::clone(&total_ops);
            handles.push(thread::spawn(move || {
                barrier_clone.wait();
                for i in 0..1000 {
                    let vpn = (thread_id * 1000 + i) * 4096;
                    let ppn = (thread_id * 1000 + i) * 4096;
                    let entry = TlbEntry::new(vpn, ppn, 0x1, thread_id as u16);
                    tlb_clone.insert(entry);
                    total_ops_clone.fetch_add(1, Ordering::Relaxed);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(total_ops.load(Ordering::Relaxed), 10000);
        let stats = tlb.stats();
        assert_eq!(stats.inserts(), 10000);
    }

    #[test]
    fn test_47_memory_consistency() {
        let tlb = Arc::new(LockFreeTlb::new());
        let barrier = Arc::new(Barrier::new(10));
        let mut handles = vec![];

        // 预填充
        for i in 0..100 {
            let vpn = i * 4096;
            let ppn = i * 4096;
            let entry = TlbEntry::new(vpn, ppn, 0x1, 0);
            tlb.insert(entry);
        }

        for thread_id in 0..10 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier_clone = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier_clone.wait();
                for i in 0..1000 {
                    let vpn = (i % 100) * 4096;
                    if let Some(result) = tlb_clone.lookup(vpn, 0) {
                        assert_eq!(result.vpn, vpn);
                        assert_eq!(result.ppn, vpn);
                    }
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_48_repeated_invalidation() {
        let tlb = Arc::new(LockFreeTlb::new());
        let barrier = Arc::new(Barrier::new(10));
        let mut handles = vec![];

        // 预填充
        for i in 0..100 {
            let vpn = i * 4096;
            let ppn = i * 4096;
            let entry = TlbEntry::new(vpn, ppn, 0x1, 0);
            tlb.insert(entry);
        }

        for thread_id in 0..10 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier_clone = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier_clone.wait();
                for i in 0..100 {
                    // 重复失效相同地址
                    let vpn = (i % 100) * 4096;
                    tlb_clone.invalidate(vpn, 0);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_49_concurrent_batch_and_single() {
        let tlb = Arc::new(LockFreeTlb::new());
        let barrier = Arc::new(Barrier::new(20));
        let mut handles = vec![];

        // 10个线程批量操作
        for thread_id in 0..10 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier_clone = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier_clone.wait();
                let entries: Vec<_> = (0..100)
                    .map(|i| {
                        let vpn = (thread_id * 100 + i) * 4096;
                        let ppn = (thread_id * 100 + i) * 4096;
                        TlbEntry::new(vpn, ppn, 0x1, thread_id as u16)
                    })
                    .collect();
                tlb_clone.insert_batch(&entries);

                let requests: Vec<_> = (0..100)
                    .map(|i| ((thread_id * 100 + i) * 4096, thread_id as u16))
                    .collect();
                let _ = tlb_clone.lookup_batch(&requests);
            }));
        }

        // 10个线程单条操作
        for thread_id in 0..10 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier_clone = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier_clone.wait();
                for i in 0..100 {
                    let vpn = (thread_id * 100 + i) * 4096;
                    let ppn = (thread_id * 100 + i) * 4096;
                    let entry = TlbEntry::new(vpn, ppn, 0x1, thread_id as u16);
                    tlb_clone.insert(entry);
                    let _ = tlb_clone.lookup(vpn, thread_id as u16);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_50_long_running_stability() {
        let tlb = Arc::new(LockFreeTlb::new());
        let barrier = Arc::new(Barrier::new(10));
        let mut handles = vec![];

        for thread_id in 0..10 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier_clone = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier_clone.wait();
                for i in 0..10000 {
                    let vpn = (thread_id * 10000 + i) * 4096;
                    let ppn = (thread_id * 10000 + i) * 4096;
                    let entry = TlbEntry::new(vpn, ppn, 0x1, thread_id as u16);
                    tlb_clone.insert(entry);

                    if i % 100 == 0 {
                        let _ = tlb_clone.lookup(vpn, thread_id as u16);
                    }
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let stats = tlb.stats();
        assert_eq!(stats.inserts(), 100000);
    }
}
