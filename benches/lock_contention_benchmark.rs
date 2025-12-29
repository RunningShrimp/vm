//! 锁竞争性能基准测试
//!
//! 比较原始锁实现与优化后的无锁实现的性能差异

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::Duration;
use std::sync::atomic::{AtomicU64, Ordering};

// 导入优化的实现
use vm_core::parallel_execution::{ShardedMmuCache, OptimizedMultiVcpuExecutor};
use vm_device::zero_copy_io::{LockFreeBufferPool, ShardedMappingCache};
use vm_engine::jit::unified_gc::{LockFreeMarkStack, ShardedWriteBarrier};

/// 模拟MMU实现
struct MockMmu {
    translations: Arc<RwLock<std::collections::HashMap<u64, u64>>>,
}

impl MockMmu {
    fn new() -> Self {
        Self {
            translations: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    fn add_translation(&self, vaddr: u64, paddr: u64) {
        let mut translations = self.translations.write().unwrap();
        translations.insert(vaddr, paddr);
    }

    fn translate_addr(&self, vaddr: u64) -> Option<u64> {
        let translations = self.translations.read().unwrap();
        translations.get(&vaddr).copied()
    }
}

/// 原始Mutex实现的MMU缓存
struct MutexMmuCache {
    cache: Arc<Mutex<std::collections::HashMap<u64, u64>>>,
    mmu: Arc<MockMmu>,
    hits: AtomicU64,
    misses: AtomicU64,
}

impl MutexMmuCache {
    fn new(mmu: Arc<MockMmu>) -> Self {
        Self {
            cache: Arc::new(Mutex::new(std::collections::HashMap::new())),
            mmu,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    fn translate(&self, vaddr: u64) -> u64 {
        // 尝试从缓存获取
        {
            let cache = self.cache.lock().unwrap();
            if let Some(&paddr) = cache.get(&vaddr) {
                self.hits.fetch_add(1, Ordering::Relaxed);
                return paddr;
            }
        }

        // 缓存未命中，查询MMU
        if let Some(paddr) = self.mmu.translate_addr(vaddr) {
            // 更新缓存
            {
                let mut cache = self.cache.lock().unwrap();
                cache.insert(vaddr, paddr);
            }
            self.misses.fetch_add(1, Ordering::Relaxed);
            paddr
        } else {
            0 // 简化实现
        }
    }

    fn get_hit_rate(&self) -> f64 {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total = hits + misses;
        
        if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64
        }
    }
}

/// 基准测试：MMU缓存性能
fn bench_mmu_cache_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("mmu_cache_performance");
    
    // 准备测试数据
    let mmu = Arc::new(MockMmu::new());
    for i in 0..1000 {
        mmu.add_translation(i * 0x1000, i * 0x1000 + 0x1000_0000);
    }

    // 测试原始Mutex实现
    group.bench_function("mutex_cache", |b| {
        let cache = MutexMmuCache::new(Arc::clone(&mmu));
        
        b.iter(|| {
            for i in 0..100 {
                let vaddr = (i % 1000) * 0x1000;
                black_box(cache.translate(vaddr));
            }
        });
    });

    // 测试优化的分片缓存
    group.bench_function("sharded_cache", |b| {
        // 创建模拟的dyn MMU
        struct DynMockMmu {
            mmu: Arc<MockMmu>,
        }
        
        impl vm_core::MMU for DynMockMmu {
            fn read(&self, _addr: vm_core::GuestAddr, _size: u8) -> Result<u64, vm_core::VmError> { Ok(0) }
            fn write(&mut self, _addr: vm_core::GuestAddr, _value: u64, _size: u8) -> Result<(), vm_core::VmError> { Ok(()) }
            fn read_bulk(&self, _addr: vm_core::GuestAddr, _buf: &mut [u8]) -> Result<(), vm_core::VmError> { Ok(()) }
            fn write_bulk(&mut self, _addr: vm_core::GuestAddr, _buf: &[u8]) -> Result<(), vm_core::VmError> { Ok(()) }
            fn translate_addr(&self, addr: vm_core::GuestAddr) -> Result<u64, vm_core::VmError> {
                Ok(self.mmu.translate_addr(addr.0).unwrap_or(0))
            }
            fn flush_tlb(&mut self) {}
            fn get_page_size(&self) -> u64 { 4096 }
            fn get_memory_size(&self) -> u64 { 0 }
        }
        
        let dyn_mmu = Arc::new(DynMockMmu { mmu: Arc::clone(&mmu) });
        let cache = ShardedMmuCache::new(dyn_mmu, 4);
        
        b.iter(|| {
            for i in 0..100 {
                let vaddr = vm_core::GuestAddr((i % 1000) * 0x1000);
                black_box(cache.fast_translate(vaddr));
            }
        });
    });

    group.finish();
}

/// 基准测试：缓冲区池性能
fn bench_buffer_pool_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer_pool_performance");

    // 测试原始Mutex缓冲区池
    group.bench_function("mutex_pool", |b| {
        use std::collections::VecDeque;
        
        struct MutexBufferPool {
            pool: Arc<Mutex<VecDeque<Vec<u8>>>>,
            buffer_size: usize,
        }
        
        impl MutexBufferPool {
            fn new(buffer_size: usize, pool_size: usize) -> Self {
                let mut pool = VecDeque::new();
                for _ in 0..pool_size {
                    pool.push_back(vec![0u8; buffer_size]);
                }
                
                Self {
                    pool: Arc::new(Mutex::new(pool)),
                    buffer_size,
                }
            }
            
            fn allocate(&self) -> Option<Vec<u8>> {
                let mut pool = self.pool.lock().unwrap();
                pool.pop_front()
            }
            
            fn release(&self, buffer: Vec<u8>) {
                let mut pool = self.pool.lock().unwrap();
                if pool.len() < 100 { // 限制池大小
                    pool.push_back(buffer);
                }
            }
        }
        
        let pool = MutexBufferPool::new(4096, 50);
        
        b.iter(|| {
            for _ in 0..10 {
                if let Some(buffer) = pool.allocate() {
                    black_box(buffer);
                    pool.release(vec![0u8; 4096]);
                }
            }
        });
    });

    // 测试无锁缓冲区池
    group.bench_function("lockfree_pool", |b| {
        let pool = LockFreeBufferPool::new(4096, 50);
        
        b.iter(|| {
            for _ in 0..10 {
                if let Some(buffer) = pool.allocate() {
                    black_box(buffer);
                    pool.release(buffer);
                }
            }
        });
    });

    group.finish();
}

/// 基准测试：并发访问性能
fn bench_concurrent_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_access");
    
    for thread_count in [1, 2, 4, 8, 16].iter() {
        // 测试原始Mutex实现
        group.bench_with_input(
            BenchmarkId::new("mutex_concurrent", thread_count),
            thread_count,
            |b, &thread_count| {
                let counter = Arc::new(AtomicU64::new(0));
                let mutex_counter = Arc::new(Mutex::new(0u64));
                
                b.iter(|| {
                    let mut handles = Vec::new();
                    
                    for _ in 0..*thread_count {
                        let counter_clone = Arc::clone(&counter);
                        let mutex_counter_clone = Arc::clone(&mutex_counter);
                        
                        let handle = thread::spawn(move || {
                            for i in 0..1000 {
                                // 原子操作
                                counter_clone.fetch_add(1, Ordering::Relaxed);
                                
                                // Mutex操作
                                let mut guard = mutex_counter_clone.lock().unwrap();
                                *guard += 1;
                                black_box(i);
                            }
                        });
                        
                        handles.push(handle);
                    }
                    
                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            }
        );

        // 测试无锁实现
        group.bench_with_input(
            BenchmarkId::new("lockfree_concurrent", thread_count),
            thread_count,
            |b, &thread_count| {
                let counter = Arc::new(AtomicU64::new(0));
                
                b.iter(|| {
                    let mut handles = Vec::new();
                    
                    for _ in 0..*thread_count {
                        let counter_clone = Arc::clone(&counter);
                        
                        let handle = thread::spawn(move || {
                            for i in 0..1000 {
                                counter_clone.fetch_add(1, Ordering::Relaxed);
                                black_box(i);
                            }
                        });
                        
                        handles.push(handle);
                    }
                    
                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            }
        );
    }

    group.finish();
}

/// 基准测试：GC标记栈性能
fn bench_gc_mark_stack(c: &mut Criterion) {
    let mut group = c.benchmark_group("gc_mark_stack");

    // 测试原始Mutex标记栈
    group.bench_function("mutex_mark_stack", |b| {
        use std::collections::VecDeque;
        
        let stack = Arc::new(Mutex::new(VecDeque::new()));
        
        b.iter(|| {
            // 压入操作
            {
                let mut s = stack.lock().unwrap();
                for i in 0..100 {
                    s.push_back(i);
                }
            }
            
            // 弹出操作
            {
                let mut s = stack.lock().unwrap();
                for _ in 0..100 {
                    black_box(s.pop_front());
                }
            }
        });
    });

    // 测试无锁标记栈
    group.bench_function("lockfree_mark_stack", |b| {
        let stack = LockFreeMarkStack::new(1000);
        
        b.iter(|| {
            // 压入操作
            for i in 0..100 {
                let _ = stack.push(i);
            }
            
            // 弹出操作
            for _ in 0..100 {
                black_box(stack.pop());
            }
        });
    });

    group.finish();
}

/// 压力测试：高并发场景
fn stress_test_high_concurrency() {
    println!("开始高并发压力测试...");
    
    let thread_count = 32;
    let operations_per_thread = 10000;
    
    // 测试无锁缓冲区池
    println!("测试无锁缓冲区池...");
    let pool = Arc::new(LockFreeBufferPool::new(4096, 100));
    let success_count = Arc::new(AtomicU64::new(0));
    let failure_count = Arc::new(AtomicU64::new(0));
    
    let start = std::time::Instant::now();
    
    let mut handles = Vec::new();
    for _ in 0..thread_count {
        let pool_clone = Arc::clone(&pool);
        let success_clone = Arc::clone(&success_count);
        let failure_clone = Arc::clone(&failure_count);
        
        let handle = thread::spawn(move || {
            for i in 0..operations_per_thread {
                if let Some(buffer) = pool_clone.allocate() {
                    success_clone.fetch_add(1, Ordering::Relaxed);
                    // 模拟使用缓冲区
                    thread::sleep(Duration::from_nanos(100));
                    pool_clone.release(buffer);
                } else {
                    failure_clone.fetch_add(1, Ordering::Relaxed);
                }
                
                if i % 1000 == 0 {
                    thread::yield_now();
                }
            }
        });
        
        handles.push(handle);
    }
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    let elapsed = start.elapsed();
    let successes = success_count.load(Ordering::Relaxed);
    let failures = failure_count.load(Ordering::Relaxed);
    
    println!("无锁缓冲区池压力测试结果:");
    println!("  线程数: {}", thread_count);
    println!("  每线程操作数: {}", operations_per_thread);
    println!("  成功分配: {}", successes);
    println!("  失败分配: {}", failures);
    println!("  总耗时: {:?}", elapsed);
    println!("  吞吐量: {:.2} ops/sec", (successes + failures) as f64 / elapsed.as_secs_f64());
    
    // 测试分片MMU缓存
    println!("\n测试分片MMU缓存...");
    
    struct DynMockMmu {
        translations: Arc<RwLock<std::collections::HashMap<u64, u64>>>,
    }
    
    impl vm_core::MMU for DynMockMmu {
        fn read(&self, _addr: vm_core::GuestAddr, _size: u8) -> Result<u64, vm_core::VmError> { Ok(0) }
        fn write(&mut self, _addr: vm_core::GuestAddr, _value: u64, _size: u8) -> Result<(), vm_core::VmError> { Ok(()) }
        fn read_bulk(&self, _addr: vm_core::GuestAddr, _buf: &mut [u8]) -> Result<(), vm_core::VmError> { Ok(()) }
        fn write_bulk(&mut self, _addr: vm_core::GuestAddr, _buf: &[u8]) -> Result<(), vm_core::VmError> { Ok(()) }
        fn translate_addr(&self, addr: vm_core::GuestAddr) -> Result<u64, vm_core::VmError> {
            let translations = self.translations.read().unwrap();
            Ok(translations.get(&addr.0).copied().unwrap_or(addr.0 + 0x1000_0000))
        }
        fn flush_tlb(&mut self) {}
        fn get_page_size(&self) -> u64 { 4096 }
        fn get_memory_size(&self) -> u64 { 0 }
    }
    
    let dyn_mmu = Arc::new(DynMockMmu {
        translations: Arc::new(RwLock::new(std::collections::HashMap::new())),
    });
    
    // 预填充一些翻译
    {
        let mut translations = dyn_mmu.translations.write().unwrap();
        for i in 0..1000 {
            translations.insert(i * 0x1000, i * 0x1000 + 0x1000_0000);
        }
    }
    
    let cache = Arc::new(ShardedMmuCache::new(dyn_mmu, 8));
    let translation_count = Arc::new(AtomicU64::new(0));
    
    let start = std::time::Instant::now();
    
    let mut handles = Vec::new();
    for _ in 0..thread_count {
        let cache_clone = Arc::clone(&cache);
        let translation_clone = Arc::clone(&translation_count);
        
        let handle = thread::spawn(move || {
            for i in 0..operations_per_thread {
                let vaddr = vm_core::GuestAddr((i % 1000) * 0x1000);
                
                // 尝试快速翻译
                if let Some(_paddr) = cache_clone.fast_translate(vaddr) {
                    translation_clone.fetch_add(1, Ordering::Relaxed);
                } else {
                    // 慢速翻译
                    let _ = cache_clone.slow_translate(vaddr);
                }
                
                if i % 1000 == 0 {
                    thread::yield_now();
                }
            }
        });
        
        handles.push(handle);
    }
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    let elapsed = start.elapsed();
    let translations = translation_count.load(Ordering::Relaxed);
    
    println!("分片MMU缓存压力测试结果:");
    println!("  线程数: {}", thread_count);
    println!("  每线程操作数: {}", operations_per_thread);
    println!("  翻译次数: {}", translations);
    println!("  总耗时: {:?}", elapsed);
    println!("  吞吐量: {:.2} translations/sec", translations as f64 / elapsed.as_secs_f64());
}

criterion_group!(
    benches,
    bench_mmu_cache_performance,
    bench_buffer_pool_performance,
    bench_concurrent_access,
    bench_gc_mark_stack
);

criterion_main!(benches);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stress_scenarios() {
        // 运行压力测试
        stress_test_high_concurrency();
    }
    
    #[test]
    fn test_correctness() {
        // 测试无锁实现的正确性
        let pool = LockFreeBufferPool::new(1024, 10);
        
        // 分配所有缓冲区
        let mut buffers = Vec::new();
        for _ in 0..10 {
            if let Some(buffer) = pool.allocate() {
                buffers.push(buffer);
            } else {
                panic!("Expected buffer allocation to succeed");
            }
        }
        
        // 下一个分配应该失败
        assert!(pool.allocate().is_none());
        
        // 释放一个缓冲区
        if !buffers.is_empty() {
            pool.release(buffers.remove(0));
        }
        
        // 现在分配应该成功
        assert!(pool.allocate().is_some());
    }
}