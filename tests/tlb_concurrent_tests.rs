//! TLB并发安全测试
//!
//! 使用loom测试TLB并发访问、插入和刷新的并发安全性

#[cfg(loom)]
mod loom_tests {
    use loom::sync::Arc;
    use loom::thread;
    use vm_core::AccessType;
    use vm_mem::tlb_concurrent::{ConcurrentTlbConfig, ConcurrentTlbManager};

    /// 测试TLB并发查找的安全性
    #[test]
    fn test_tlb_concurrent_lookup() {
        loom::model(|| {
            let config = ConcurrentTlbConfig {
                sharded_capacity: 1000,
                shard_count: 4,
                fast_path_capacity: 100,
                enable_fast_path: true,
                enable_adaptive: false,
            };
            let tlb = Arc::new(ConcurrentTlbManager::new(config));
            let mut handles = Vec::new();

            // 创建多个线程并发查找
            for i in 0..8 {
                let tlb_clone = Arc::clone(&tlb);
                let handle = thread::spawn(move || {
                    for j in 0..100 {
                        let vpn = (i * 100 + j) as u64;
                        let asid = (i % 4) as u16;
                        let _ = tlb_clone.translate(vpn, asid, AccessType::Read);
                    }
                });
                handles.push(handle);
            }

            // 等待所有线程完成
            for handle in handles {
                handle.join().unwrap();
            }
        });
    }

    /// 测试TLB并发插入的安全性
    #[test]
    fn test_tlb_concurrent_insert() {
        loom::model(|| {
            let config = ConcurrentTlbConfig {
                sharded_capacity: 1000,
                shard_count: 4,
                fast_path_capacity: 100,
                enable_fast_path: true,
                enable_adaptive: false,
            };
            let tlb = Arc::new(ConcurrentTlbManager::new(config));
            let mut handles = Vec::new();

            // 创建多个线程并发插入
            for i in 0..8 {
                let tlb_clone = Arc::clone(&tlb);
                let handle = thread::spawn(move || {
                    for j in 0..100 {
                        let vpn = (i * 100 + j) as u64;
                        let ppn = vpn + 0x1000_0000;
                        let flags = 0x3; // Read + Write
                        let asid = (i % 4) as u16;
                        tlb_clone.insert(vpn, ppn, flags, asid);
                    }
                });
                handles.push(handle);
            }

            // 等待所有线程完成
            for handle in handles {
                handle.join().unwrap();
            }
        });
    }

    /// 测试TLB并发查找和插入的混合操作
    #[test]
    fn test_tlb_concurrent_mixed_operations() {
        loom::model(|| {
            let config = ConcurrentTlbConfig {
                sharded_capacity: 1000,
                shard_count: 4,
                fast_path_capacity: 100,
                enable_fast_path: true,
                enable_adaptive: false,
            };
            let tlb = Arc::new(ConcurrentTlbManager::new(config));
            let mut handles = Vec::new();

            // 创建查找线程
            for i in 0..4 {
                let tlb_clone = Arc::clone(&tlb);
                let handle = thread::spawn(move || {
                    for j in 0..100 {
                        let vpn = (i * 100 + j) as u64;
                        let asid = (i % 4) as u16;
                        let _ = tlb_clone.translate(vpn, asid, AccessType::Read);
                    }
                });
                handles.push(handle);
            }

            // 创建插入线程
            for i in 0..4 {
                let tlb_clone = Arc::clone(&tlb);
                let handle = thread::spawn(move || {
                    for j in 0..100 {
                        let vpn = (i * 100 + j) as u64;
                        let ppn = vpn + 0x1000_0000;
                        let flags = 0x3;
                        let asid = (i % 4) as u16;
                        tlb_clone.insert(vpn, ppn, flags, asid);
                    }
                });
                handles.push(handle);
            }

            // 等待所有线程完成
            for handle in handles {
                handle.join().unwrap();
            }
        });
    }

    /// 测试TLB并发刷新操作
    #[test]
    fn test_tlb_concurrent_flush() {
        loom::model(|| {
            let config = ConcurrentTlbConfig {
                sharded_capacity: 1000,
                shard_count: 4,
                fast_path_capacity: 100,
                enable_fast_path: true,
                enable_adaptive: false,
            };
            let tlb = Arc::new(ConcurrentTlbManager::new(config));
            let mut handles = Vec::new();

            // 先插入一些条目
            for i in 0..100 {
                let vpn = i as u64 * 0x1000;
                let ppn = vpn + 0x1000_0000;
                tlb.insert(vpn, ppn, 0x3, 0);
            }

            // 创建查找线程
            for _ in 0..4 {
                let tlb_clone = Arc::clone(&tlb);
                let handle = thread::spawn(move || {
                    for j in 0..50 {
                        let vpn = (j * 0x1000) as u64;
                        let _ = tlb_clone.translate(vpn, 0, AccessType::Read);
                    }
                });
                handles.push(handle);
            }

            // 创建刷新线程
            for _ in 0..2 {
                let tlb_clone = Arc::clone(&tlb);
                let handle = thread::spawn(move || {
                    for _ in 0..10 {
                        tlb_clone.flush_all();
                    }
                });
                handles.push(handle);
            }

            // 等待所有线程完成
            for handle in handles {
                handle.join().unwrap();
            }
        });
    }

    /// 测试TLB ASID刷新的并发安全性
    #[test]
    fn test_tlb_concurrent_flush_asid() {
        loom::model(|| {
            let config = ConcurrentTlbConfig {
                sharded_capacity: 1000,
                shard_count: 4,
                fast_path_capacity: 100,
                enable_fast_path: true,
                enable_adaptive: false,
            };
            let tlb = Arc::new(ConcurrentTlbManager::new(config));
            let mut handles = Vec::new();

            // 先插入一些条目（不同ASID）
            for asid in 0..4 {
                for i in 0..50 {
                    let vpn = (asid * 50 + i) as u64 * 0x1000;
                    let ppn = vpn + 0x1000_0000;
                    tlb.insert(vpn, ppn, 0x3, asid);
                }
            }

            // 创建查找线程
            for asid in 0..4 {
                let tlb_clone = Arc::clone(&tlb);
                let handle = thread::spawn(move || {
                    for j in 0..50 {
                        let vpn = (asid * 50 + j) as u64 * 0x1000;
                        let _ = tlb_clone.translate(vpn, asid, AccessType::Read);
                    }
                });
                handles.push(handle);
            }

            // 创建ASID刷新线程
            for asid in 0..2 {
                let tlb_clone = Arc::clone(&tlb);
                let handle = thread::spawn(move || {
                    for _ in 0..5 {
                        tlb_clone.flush_asid(asid);
                    }
                });
                handles.push(handle);
            }

            // 等待所有线程完成
            for handle in handles {
                handle.join().unwrap();
            }
        });
    }
}

/// 非loom环境的并发测试（使用标准库）
#[cfg(not(loom))]
mod std_tests {
    use std::sync::Arc;
    use std::thread;
    use vm_core::AccessType;
    use vm_mem::tlb_concurrent::{ConcurrentTlbConfig, ConcurrentTlbManager};

    /// 测试TLB并发查找的安全性（标准库版本）
    #[test]
    fn test_tlb_concurrent_lookup_std() {
        let config = ConcurrentTlbConfig {
            sharded_capacity: 1000,
            shard_count: 4,
            fast_path_capacity: 100,
            enable_fast_path: true,
            enable_adaptive: false,
        };
        let tlb = Arc::new(ConcurrentTlbManager::new(config));
        let mut handles = Vec::new();

        // 创建多个线程并发查找
        for i in 0..8 {
            let tlb_clone = Arc::clone(&tlb);
            let handle = thread::spawn(move || {
                for j in 0..100 {
                    let vpn = (i * 100 + j) as u64;
                    let asid = (i % 4) as u16;
                    let _ = tlb_clone.translate(vpn, asid, AccessType::Read);
                }
            });
            handles.push(handle);
        }

        // 等待所有线程完成
        for handle in handles {
            handle.join().unwrap();
        }
    }

    /// 测试TLB并发插入的安全性（标准库版本）
    #[test]
    fn test_tlb_concurrent_insert_std() {
        let config = ConcurrentTlbConfig {
            sharded_capacity: 1000,
            shard_count: 4,
            fast_path_capacity: 100,
            enable_fast_path: true,
            enable_adaptive: false,
        };
        let tlb = Arc::new(ConcurrentTlbManager::new(config));
        let mut handles = Vec::new();

        // 创建多个线程并发插入
        for i in 0..8 {
            let tlb_clone = Arc::clone(&tlb);
            let handle = thread::spawn(move || {
                for j in 0..100 {
                    let vpn = (i * 100 + j) as u64;
                    let ppn = vpn + 0x1000_0000;
                    let flags = 0x3;
                    let asid = (i % 4) as u16;
                    tlb_clone.insert(vpn, ppn, flags, asid);
                }
            });
            handles.push(handle);
        }

        // 等待所有线程完成
        for handle in handles {
            handle.join().unwrap();
        }
    }

    /// 测试TLB并发查找和插入的混合操作（标准库版本）
    #[test]
    fn test_tlb_concurrent_mixed_operations_std() {
        let config = ConcurrentTlbConfig {
            sharded_capacity: 1000,
            shard_count: 4,
            fast_path_capacity: 100,
            enable_fast_path: true,
            enable_adaptive: false,
        };
        let tlb = Arc::new(ConcurrentTlbManager::new(config));
        let mut handles = Vec::new();

        // 创建查找线程
        for i in 0..4 {
            let tlb_clone = Arc::clone(&tlb);
            let handle = thread::spawn(move || {
                for j in 0..100 {
                    let vpn = (i * 100 + j) as u64;
                    let asid = (i % 4) as u16;
                    let _ = tlb_clone.translate(vpn, asid, AccessType::Read);
                }
            });
            handles.push(handle);
        }

        // 创建插入线程
        for i in 0..4 {
            let tlb_clone = Arc::clone(&tlb);
            let handle = thread::spawn(move || {
                for j in 0..100 {
                    let vpn = (i * 100 + j) as u64;
                    let ppn = vpn + 0x1000_0000;
                    let flags = 0x3;
                    let asid = (i % 4) as u16;
                    tlb_clone.insert(vpn, ppn, flags, asid);
                }
            });
            handles.push(handle);
        }

        // 等待所有线程完成
        for handle in handles {
            handle.join().unwrap();
        }
    }
}


