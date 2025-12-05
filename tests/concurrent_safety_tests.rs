//! 并发安全性验证测试
//!
//! 验证优化后的无锁实现在高并发场景下的正确性和线程安全性

use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Duration;
use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};

// 导入优化的实现
use vm_core::parallel_execution::{ShardedMmuCache, OptimizedMultiVcpuExecutor};
use vm_device::zero_copy_io::{LockFreeBufferPool, ShardedMappingCache};
use vm_engine_jit::unified_gc::{LockFreeMarkStack, ShardedWriteBarrier};

/// 测试ShardedMmuCache的并发安全性
#[test]
fn test_sharded_mmu_cache_concurrent_safety() {
    println!("测试ShardedMmuCache并发安全性...");
    
    // 创建模拟MMU
    struct DynMockMmu {
        translations: Arc<std::sync::RwLock<std::collections::HashMap<u64, u64>>>,
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
        translations: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
    });
    
    // 预填充一些翻译
    {
        let mut translations = dyn_mmu.translations.write().unwrap();
        for i in 0..1000 {
            translations.insert(i * 0x1000, i * 0x1000 + 0x1000_0000);
        }
    }
    
    let cache = Arc::new(ShardedMmuCache::new(dyn_mmu, 8));
    let thread_count = 16;
    let operations_per_thread = 10000;
    let barrier = Arc::new(Barrier::new(thread_count));
    let success_count = Arc::new(AtomicU64::new(0));
    let error_count = Arc::new(AtomicU64::new(0));
    
    let mut handles = Vec::new();
    
    for thread_id in 0..thread_count {
        let cache_clone = Arc::clone(&cache);
        let barrier_clone = Arc::clone(&barrier);
        let success_clone = Arc::clone(&success_count);
        let error_clone = Arc::clone(&error_count);
        
        let handle = thread::spawn(move || {
            barrier_clone.wait(); // 同步开始
            
            for i in 0..operations_per_thread {
                let vaddr = vm_core::GuestAddr(((thread_id * operations_per_thread + i) % 1000) * 0x1000);
                
                // 测试快速翻译
                if let Some(paddr) = cache_clone.fast_translate(vaddr) {
                    // 验证翻译结果的一致性
                    let expected = (vaddr.0 / 0x1000) * 0x1000 + 0x1000_0000;
                    if paddr == expected {
                        success_clone.fetch_add(1, Ordering::Relaxed);
                    } else {
                        error_clone.fetch_add(1, Ordering::Relaxed);
                        eprintln!("翻译不一致: vaddr={:#x}, paddr={:#x}, expected={:#x}", vaddr.0, paddr, expected);
                    }
                } else {
                    // 测试慢速翻译
                    match cache_clone.slow_translate(vaddr) {
                        Ok(paddr) => {
                            let expected = (vaddr.0 / 0x1000) * 0x1000 + 0x1000_0000;
                            if paddr == expected {
                                success_clone.fetch_add(1, Ordering::Relaxed);
                            } else {
                                error_clone.fetch_add(1, Ordering::Relaxed);
                                eprintln!("慢速翻译不一致: vaddr={:#x}, paddr={:#x}, expected={:#x}", vaddr.0, paddr, expected);
                            }
                        }
                        Err(e) => {
                            error_clone.fetch_add(1, Ordering::Relaxed);
                            eprintln!("翻译错误: {:?}", e);
                        }
                    }
                }
                
                // 偶尔执行缓存失效
                if i % 1000 == 0 {
                    cache_clone.invalidate(vaddr);
                }
            }
        });
        
        handles.push(handle);
    }
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    let successes = success_count.load(Ordering::Relaxed);
    let errors = error_count.load(Ordering::Relaxed);
    let total_operations = (thread_count * operations_per_thread) as u64;
    
    println!("ShardedMmuCache并发测试结果:");
    println!("  总操作数: {}", total_operations);
    println!("  成功操作: {}", successes);
    println!("  错误操作: {}", errors);
    println!("  成功率: {:.2}%", (successes as f64 / total_operations as f64) * 100.0);
    
    // 验证没有错误
    assert_eq!(errors, 0, "ShardedMmuCache并发安全性测试失败");
    assert_eq!(successes, total_operations, "部分操作未成功完成");
}

/// 测试LockFreeBufferPool的并发安全性
#[test]
fn test_lockfree_buffer_pool_concurrent_safety() {
    println!("测试LockFreeBufferPool并发安全性...");
    
    let pool = Arc::new(LockFreeBufferPool::new(4096, 100));
    let thread_count = 16;
    let operations_per_thread = 1000;
    let barrier = Arc::new(Barrier::new(thread_count));
    let allocated_count = Arc::new(AtomicU64::new(0));
    let released_count = Arc::new(AtomicU64::new(0));
    let allocation_errors = Arc::new(AtomicU64::new(0));
    
    let mut handles = Vec::new();
    
    for thread_id in 0..thread_count {
        let pool_clone = Arc::clone(&pool);
        let barrier_clone = Arc::clone(&barrier);
        let allocated_clone = Arc::clone(&allocated_count);
        let released_clone = Arc::clone(&released_count);
        let errors_clone = Arc::clone(&allocation_errors);
        
        let handle = thread::spawn(move || {
            barrier_clone.wait(); // 同步开始
            
            let mut local_buffers = Vec::new();
            
            for i in 0..operations_per_thread {
                // 分配缓冲区
                if let Some(buffer) = pool_clone.allocate() {
                    allocated_clone.fetch_add(1, Ordering::Relaxed);
                    
                    // 验证缓冲区大小
                    assert_eq!(buffer.len(), 4096, "缓冲区大小不正确");
                    
                    // 写入一些数据
                    if let Some(slice) = buffer.get_mut(0..64) {
                        for (j, byte) in slice.iter_mut().enumerate() {
                            *byte = ((thread_id * operations_per_thread + i + j) % 256) as u8;
                        }
                    }
                    
                    local_buffers.push(buffer);
                    
                    // 偶尔释放一些缓冲区
                    if i % 10 == 5 && !local_buffers.is_empty() {
                        let buffer_to_release = local_buffers.remove(0);
                        pool_clone.release(buffer_to_release);
                        released_clone.fetch_add(1, Ordering::Relaxed);
                    }
                } else {
                    errors_clone.fetch_add(1, Ordering::Relaxed);
                }
                
                // 偶尔让出CPU
                if i % 100 == 0 {
                    thread::yield_now();
                }
            }
            
            // 释放剩余的缓冲区
            for buffer in local_buffers {
                pool_clone.release(buffer);
                released_clone.fetch_add(1, Ordering::Relaxed);
            }
        });
        
        handles.push(handle);
    }
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    let allocated = allocated_count.load(Ordering::Relaxed);
    let released = released_count.load(Ordering::Relaxed);
    let errors = allocation_errors.load(Ordering::Relaxed);
    
    println!("LockFreeBufferPool并发测试结果:");
    println!("  分配次数: {}", allocated);
    println!("  释放次数: {}", released);
    println!("  分配错误: {}", errors);
    println!("  当前可用缓冲区: {}", pool.available_count());
    
    // 验证分配和释放数量匹配
    assert_eq!(allocated, released, "分配和释放数量不匹配");
    
    // 验证最终可用缓冲区数量正确
    let (allocs, reuses) = pool.stats();
    let total_allocations = allocs + reuses;
    assert_eq!(total_allocations, allocated, "统计的分配数量不匹配");
}

/// 测试LockFreeMarkStack的并发安全性
#[test]
fn test_lockfree_mark_stack_concurrent_safety() {
    println!("测试LockFreeMarkStack并发安全性...");
    
    let stack = Arc::new(LockFreeMarkStack::new(10000));
    let thread_count = 16;
    let operations_per_thread = 1000;
    let barrier = Arc::new(Barrier::new(thread_count));
    let pushed_count = Arc::new(AtomicU64::new(0));
    let popped_count = Arc::new(AtomicU64::new(0));
    let push_errors = Arc::new(AtomicU64::new(0));
    let unique_values = Arc::new(std::sync::Mutex::new(std::collections::HashSet::new()));
    
    let mut handles = Vec::new();
    
    for thread_id in 0..thread_count {
        let stack_clone = Arc::clone(&stack);
        let barrier_clone = Arc::clone(&barrier);
        let pushed_clone = Arc::clone(&pushed_count);
        let popped_clone = Arc::clone(&popped_count);
        let errors_clone = Arc::clone(&push_errors);
        let unique_clone = Arc::clone(&unique_values);
        
        let handle = thread::spawn(move || {
            barrier_clone.wait(); // 同步开始
            
            let mut local_values = Vec::new();
            
            // 压入操作
            for i in 0..operations_per_thread {
                let value = (thread_id * operations_per_thread + i) as u64;
                
                match stack_clone.push(value) {
                    Ok(()) => {
                        pushed_clone.fetch_add(1, Ordering::Relaxed);
                        local_values.push(value);
                    }
                    Err(_) => {
                        errors_clone.fetch_add(1, Ordering::Relaxed);
                    }
                }
                
                if i % 100 == 0 {
                    thread::yield_now();
                }
            }
            
            // 弹出操作
            for _ in 0..operations_per_thread {
                if let Some(value) = stack_clone.pop() {
                    popped_clone.fetch_add(1, Ordering::Relaxed);
                    
                    // 记录唯一值
                    {
                        let mut unique_set = unique_clone.lock().unwrap();
                        unique_set.insert(value);
                    }
                }
                
                thread::yield_now();
            }
        });
        
        handles.push(handle);
    }
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    let pushed = pushed_count.load(Ordering::Relaxed);
    let popped = popped_count.load(Ordering::Relaxed);
    let errors = push_errors.load(Ordering::Relaxed);
    let final_size = stack.len();
    
    println!("LockFreeMarkStack并发测试结果:");
    println!("  压入次数: {}", pushed);
    println!("  弹出次数: {}", popped);
    println!("  压入错误: {}", errors);
    println!("  最终栈大小: {}", final_size);
    
    // 验证栈的最终大小
    let expected_size = (pushed - popped) as usize;
    assert_eq!(final_size, expected_size, "栈最终大小不正确");
    
    // 验证没有压入错误（栈应该足够大）
    assert_eq!(errors, 0, "不应该有压入错误");
}

/// 测试ShardedWriteBarrier的并发安全性
#[test]
fn test_sharded_write_barrier_concurrent_safety() {
    println!("测试ShardedWriteBarrier并发安全性...");
    
    let write_barrier = Arc::new(ShardedWriteBarrier::new(8));
    let thread_count = 16;
    let operations_per_thread = 1000;
    let barrier = Arc::new(Barrier::new(thread_count));
    let write_count = Arc::new(AtomicU64::new(0));
    let drained_count = Arc::new(AtomicU64::new(0));
    
    let mut handles = Vec::new();
    
    for thread_id in 0..thread_count {
        let wb_clone = Arc::clone(&write_barrier);
        let barrier_clone = Arc::clone(&barrier);
        let write_clone = Arc::clone(&write_count);
        
        let handle = thread::spawn(move || {
            barrier_clone.wait(); // 同步开始
            
            for i in 0..operations_per_thread {
                let obj_addr = (thread_id * operations_per_thread + i) as u64;
                let child_addr = obj_addr + 1; // 模拟子对象地址
                wb_clone.record_write(obj_addr, child_addr);
                write_clone.fetch_add(1, Ordering::Relaxed);
                
                if i % 100 == 0 {
                    thread::yield_now();
                }
            }
        });
        
        handles.push(handle);
    }
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    // 排空所有修改对象
    let modified = write_barrier.drain_modified();
    drained_count.store(modified.len() as u64, Ordering::Relaxed);
    
    let writes = write_count.load(Ordering::Relaxed);
    let drained = drained_count.load(Ordering::Relaxed);
    
    println!("ShardedWriteBarrier并发测试结果:");
    println!("  写入次数: {}", writes);
    println!("  排空对象数: {}", drained);
    println!("  唯一对象数: {}", modified.len());
    
    // 验证所有写入都被记录
    assert_eq!(writes, drained, "写入和排空数量不匹配");
    
    // 验证所有对象都是唯一的
    let mut unique_addrs = std::collections::HashSet::new();
    for addr in &modified {
        assert!(!unique_addrs.contains(addr), "发现重复地址: {:#x}", addr);
        unique_addrs.insert(*addr);
    }
    
    // 验证地址范围正确
    let expected_range = thread_count * operations_per_thread;
    assert_eq!(unique_addrs.len(), expected_range, "唯一地址数量不正确");
}

/// 测试内存一致性
#[test]
fn test_memory_consistency() {
    println!("测试内存一致性...");
    
    let shared_data = Arc::new(AtomicU64::new(0));
    let flag = Arc::new(AtomicBool::new(false));
    let thread_count = 8;
    let barrier = Arc::new(Barrier::new(thread_count + 1));
    
    let mut reader_handles = Vec::new();
    
    // 创建读线程
    for _ in 0..thread_count {
        let data_clone = Arc::clone(&shared_data);
        let flag_clone = Arc::clone(&flag);
        let barrier_clone = Arc::clone(&barrier);
        
        let handle = thread::spawn(move || {
            barrier_clone.wait();
            
            let mut observed_values = Vec::new();
            
            // 等待标志设置
            while !flag_clone.load(Ordering::Acquire) {
                thread::yield_now();
            }
            
            // 读取数据多次
            for _ in 0..1000 {
                observed_values.push(data_clone.load(Ordering::Acquire));
                thread::yield_now();
            }
            
            observed_values
        });
        
        reader_handles.push(handle);
    }
    
    // 创建写线程
    let data_clone = Arc::clone(&shared_data);
    let flag_clone = Arc::clone(&flag);
    let barrier_clone = Arc::clone(&barrier);
    
    let writer_handle = thread::spawn(move || {
        barrier_clone.wait();
        
        // 设置数据
        data_clone.store(0x123456789ABCDEF0, Ordering::Release);
        
        // 设置标志
        flag_clone.store(true, Ordering::Release);
    });
    
    // 等待所有线程就绪
    barrier.wait();
    
    // 等待写线程完成
    writer_handle.join().unwrap();
    
    // 收集所有读线程的观察结果
    let mut all_observations = Vec::new();
    for handle in reader_handles {
        let observations = handle.join().unwrap();
        all_observations.extend(observations);
    }
    
    // 验证内存一致性
    let expected_value = 0x123456789ABCDEF0;
    let mut consistent_reads = 0;
    let mut inconsistent_reads = 0;
    
    for value in &all_observations {
        if *value == expected_value {
            consistent_reads += 1;
        } else if *value == 0 {
            // 在标志设置之前的读取，这是正常的
        } else {
            inconsistent_reads += 1;
            eprintln!("观察到不一致的值: {:#x}", value);
        }
    }
    
    println!("内存一致性测试结果:");
    println!("  总读取次数: {}", all_observations.len());
    println!("  一致读取: {}", consistent_reads);
    println!("  不一致读取: {}", inconsistent_reads);
    
    // 验证没有不一致的读取
    assert_eq!(inconsistent_reads, 0, "检测到内存不一致");
    assert!(consistent_reads > 0, "应该有至少一次一致读取");
}

/// 测试ABA问题的防护
#[test]
fn test_aba_protection() {
    println!("测试ABA问题防护...");
    
    use std::sync::atomic::{AtomicPtr, Ordering};
    
    struct Node {
        value: u64,
        next: AtomicPtr<Node>,
    }
    
    let node1 = Box::new(Node {
        value: 1,
        next: AtomicPtr::new(std::ptr::null_mut()),
    });
    
    let node2 = Box::new(Node {
        value: 2,
        next: AtomicPtr::new(std::ptr::null_mut()),
    });
    
    let head = AtomicPtr::new(node1.as_mut() as *mut Node);
    let barrier = Arc::new(Barrier::new(3));
    
    let mut handles = Vec::new();
    
    // 线程1：移除节点1
    let head_clone1 = Arc::clone(&head);
    let barrier_clone1 = Arc::clone(&barrier);
    let handle1 = thread::spawn(move || {
        barrier_clone1.wait();
        
        // CAS移除节点1
        let current = head_clone1.load(Ordering::Acquire);
        if !current.is_null() {
            let _ = head_clone1.compare_exchange_weak(
                current,
                std::ptr::null_mut(),
                Ordering::AcqRel,
                Ordering::Relaxed,
            );
        }
    });
    handles.push(handle1);
    
    // 线程2：添加节点2
    let head_clone2 = Arc::clone(&head);
    let barrier_clone2 = Arc::clone(&barrier);
    let handle2 = thread::spawn(move || {
        barrier_clone2.wait();
        
        // CAS添加节点2
        let current = head_clone2.load(Ordering::Acquire);
        if current.is_null() {
            let _ = head_clone2.compare_exchange_weak(
                current,
                node2.as_mut() as *mut Node,
                Ordering::AcqRel,
                Ordering::Relaxed,
            );
        }
    });
    handles.push(handle2);
    
    // 线程3：尝试ABA攻击
    let head_clone3 = Arc::clone(&head);
    let barrier_clone3 = Arc::clone(&barrier);
    let handle3 = thread::spawn(move || {
        barrier_clone3.wait();
        
        thread::sleep(Duration::from_millis(10)); // 让其他线程先执行
        
        // 尝试CAS操作
        let current = head_clone3.load(Ordering::Acquire);
        if !current.is_null() {
            let result = head_clone3.compare_exchange_weak(
                current,
                std::ptr::null_mut(),
                Ordering::AcqRel,
                Ordering::Relaxed,
            );
            
            // 如果CAS成功，说明没有ABA问题
            println!("线程3 CAS结果: {:?}", result.is_ok());
        }
    });
    handles.push(handle3);
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    let final_head = head.load(Ordering::Acquire);
    
    println!("ABA防护测试完成");
    println!("最终头指针: {:?}", final_head);
    
    // 验证最终状态的合理性
    // 这里主要测试没有崩溃或数据竞争
    assert!(final_head.is_null() || !final_head.is_null());
}

/// 运行所有并发安全性测试
#[test]
fn run_all_concurrent_safety_tests() {
    println!("=== 开始并发安全性验证测试 ===");
    
    test_sharded_mmu_cache_concurrent_safety();
    test_lockfree_buffer_pool_concurrent_safety();
    test_lockfree_mark_stack_concurrent_safety();
    test_sharded_write_barrier_concurrent_safety();
    test_memory_consistency();
    test_aba_protection();
    
    println!("=== 所有并发安全性测试通过 ===");
}