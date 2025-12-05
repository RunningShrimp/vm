//! 代码缓存并发安全测试
//!
//! 使用loom测试代码缓存的并发访问、插入和淘汰的并发安全性

#[cfg(loom)]
mod loom_tests {
    use loom::sync::Arc;
    use loom::thread;
    use vm_core::GuestAddr;
    use vm_engine_jit::unified_cache::{CacheConfig, UnifiedCodeCache};
    use vm_engine_jit::ewma_hotspot::EwmaHotspotConfig;
    use vm_engine_jit::CodePtr;

    /// 测试代码缓存并发查找的安全性
    #[test]
    fn test_code_cache_concurrent_lookup() {
        loom::model(|| {
            let cache_config = CacheConfig {
                max_hot_entries: 1000,
                max_cold_entries: 5000,
                ..Default::default()
            };
            let hotspot_config = EwmaHotspotConfig::default();
            let cache = Arc::new(UnifiedCodeCache::new(cache_config, hotspot_config));
            let mut handles = Vec::new();

            // 先插入一些条目
            for i in 0..100 {
                let addr = GuestAddr(i * 0x1000);
                let code_ptr = CodePtr::new(0x1000_0000 + i * 0x1000);
                let _ = cache.insert(addr, code_ptr, 1024, 1000);
            }

            // 创建多个线程并发查找
            for i in 0..8 {
                let cache_clone = Arc::clone(&cache);
                let handle = thread::spawn(move || {
                    for j in 0..100 {
                        let addr = GuestAddr(((i * 100 + j) % 100) * 0x1000);
                        let _ = cache_clone.lookup(addr);
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

    /// 测试代码缓存并发插入的安全性
    #[test]
    fn test_code_cache_concurrent_insert() {
        loom::model(|| {
            let cache_config = CacheConfig {
                max_hot_entries: 1000,
                max_cold_entries: 5000,
                ..Default::default()
            };
            let hotspot_config = EwmaHotspotConfig::default();
            let cache = Arc::new(UnifiedCodeCache::new(cache_config, hotspot_config));
            let mut handles = Vec::new();

            // 创建多个线程并发插入
            for i in 0..8 {
                let cache_clone = Arc::clone(&cache);
                let handle = thread::spawn(move || {
                    for j in 0..100 {
                        let addr = GuestAddr((i * 100 + j) * 0x1000);
                        let code_ptr = CodePtr::new(0x1000_0000 + (i * 100 + j) * 0x1000);
                        let _ = cache_clone.insert(addr, code_ptr, 1024, 1000);
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

    /// 测试代码缓存并发查找和插入的混合操作
    #[test]
    fn test_code_cache_concurrent_mixed_operations() {
        loom::model(|| {
            let cache_config = CacheConfig {
                max_hot_entries: 1000,
                max_cold_entries: 5000,
                ..Default::default()
            };
            let hotspot_config = EwmaHotspotConfig::default();
            let cache = Arc::new(UnifiedCodeCache::new(cache_config, hotspot_config));
            let mut handles = Vec::new();

            // 创建查找线程
            for i in 0..4 {
                let cache_clone = Arc::clone(&cache);
                let handle = thread::spawn(move || {
                    for j in 0..100 {
                        let addr = GuestAddr(((i * 100 + j) % 200) * 0x1000);
                        let _ = cache_clone.lookup(addr);
                    }
                });
                handles.push(handle);
            }

            // 创建插入线程
            for i in 0..4 {
                let cache_clone = Arc::clone(&cache);
                let handle = thread::spawn(move || {
                    for j in 0..100 {
                        let addr = GuestAddr((i * 100 + j) * 0x1000);
                        let code_ptr = CodePtr::new(0x1000_0000 + (i * 100 + j) * 0x1000);
                        let _ = cache_clone.insert(addr, code_ptr, 1024, 1000);
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

    /// 测试代码缓存并发淘汰的安全性
    #[test]
    fn test_code_cache_concurrent_eviction() {
        loom::model(|| {
            let cache_config = CacheConfig {
                max_hot_entries: 100,
                max_cold_entries: 200,
                ..Default::default()
            };
            let hotspot_config = EwmaHotspotConfig::default();
            let cache = Arc::new(UnifiedCodeCache::new(cache_config, hotspot_config));
            let mut handles = Vec::new();

            // 先插入一些条目（超过限制）
            for i in 0..500 {
                let addr = GuestAddr(i * 0x1000);
                let code_ptr = CodePtr::new(0x1000_0000 + i * 0x1000);
                let _ = cache.insert(addr, code_ptr, 1024);
            }

            // 创建查找线程
            for i in 0..4 {
                let cache_clone = Arc::clone(&cache);
                let handle = thread::spawn(move || {
                    for j in 0..100 {
                        let addr = GuestAddr(((i * 100 + j) % 500) * 0x1000);
                        let _ = cache_clone.lookup(addr);
                    }
                });
                handles.push(handle);
            }

            // 创建插入线程（触发淘汰）
            for i in 0..4 {
                let cache_clone = Arc::clone(&cache);
                let handle = thread::spawn(move || {
                    for j in 0..100 {
                        let addr = GuestAddr((500 + i * 100 + j) * 0x1000);
                        let code_ptr = CodePtr::new(0x1000_0000 + (500 + i * 100 + j) * 0x1000);
                        let _ = cache_clone.insert(addr, code_ptr, 1024, 1000);
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
    use vm_core::GuestAddr;
    use vm_engine_jit::unified_cache::{CacheConfig, UnifiedCodeCache};
    use vm_engine_jit::ewma_hotspot::EwmaHotspotConfig;
    use vm_engine_jit::CodePtr;

    /// 测试代码缓存并发查找的安全性（标准库版本）
    #[test]
    fn test_code_cache_concurrent_lookup_std() {
        let cache_config = CacheConfig {
            max_hot_entries: 1000,
            max_cold_entries: 5000,
            ..Default::default()
        };
        let hotspot_config = EwmaHotspotConfig::default();
        let cache = Arc::new(UnifiedCodeCache::new(cache_config, hotspot_config));

        // 先插入一些条目
        for i in 0..100 {
            let addr = GuestAddr(i * 0x1000);
            let code_ptr = CodePtr::new(0x1000_0000 + i * 0x1000);
            let _ = cache.insert(addr, code_ptr, 1024);
        }

        let mut handles = Vec::new();

        // 创建多个线程并发查找
        for i in 0..8 {
            let cache_clone = Arc::clone(&cache);
            let handle = thread::spawn(move || {
                for j in 0..100 {
                    let addr = GuestAddr(((i * 100 + j) % 100) * 0x1000);
                    let _ = cache_clone.get(addr);
                }
            });
            handles.push(handle);
        }

        // 等待所有线程完成
        for handle in handles {
            handle.join().unwrap();
        }
    }

    /// 测试代码缓存并发插入的安全性（标准库版本）
    #[test]
    fn test_code_cache_concurrent_insert_std() {
        let cache_config = CacheConfig {
            max_hot_entries: 1000,
            max_cold_entries: 5000,
            ..Default::default()
        };
        let hotspot_config = EwmaHotspotConfig::default();
        let cache = Arc::new(UnifiedCodeCache::new(cache_config, hotspot_config));
        let mut handles = Vec::new();

        // 创建多个线程并发插入
        for i in 0..8 {
            let cache_clone = Arc::clone(&cache);
            let handle = thread::spawn(move || {
                for j in 0..100 {
                    let addr = GuestAddr((i * 100 + j) * 0x1000);
                    let code_ptr = CodePtr::new(0x1000_0000 + (i * 100 + j) * 0x1000);
                    let _ = cache_clone.insert(addr, code_ptr, 1024);
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

