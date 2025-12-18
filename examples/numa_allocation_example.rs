//! NUMA感知内存分配示例
//!
//! 展示如何使用NUMA感知的内存分配器来优化多socket系统的内存访问性能。

use std::alloc::Layout;
use vm_mem::{
    init_global_numa_allocator, NumaAllocPolicy, NumaAllocator, NumaNodeInfo,
    GlobalNumaAllocator,
};

/// 全局NUMA分配器实例
#[global_allocator]
static GLOBAL_NUMA: GlobalNumaAllocator = GlobalNumaAllocator::new();

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("NUMA感知内存分配示例");
    
    // 初始化全局NUMA分配器
    init_global_numa_allocator(NumaAllocPolicy::Local)?;
    
    // 打印NUMA统计信息
    if let Some(stats) = vm_mem::global_numa_stats() {
        println!("NUMA分配器统计:");
        println!("  本地分配: {}", 
            stats.local_allocs.load(std::sync::atomic::Ordering::Relaxed));
        println!("  远程分配: {}", 
            stats.remote_allocs.load(std::sync::atomic::Ordering::Relaxed));
        println!("  失败分配: {}", 
            stats.failed_allocs.load(std::sync::atomic::Ordering::Relaxed));
    }
    
    // 示例1: 使用本地分配策略
    demo_local_allocation()?;
    
    // 示例2: 使用交错分配策略
    demo_interleaved_allocation()?;
    
    // 示例3: 使用绑定分配策略
    demo_bound_allocation()?;
    
    // 示例4: 多线程NUMA分配
    demo_multithreaded_allocation()?;
    
    Ok(())
}

/// 演示本地分配策略
fn demo_local_allocation() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== 本地分配策略示例 ===");
    
    // 创建测试NUMA节点
    let nodes = vec![
        NumaNodeInfo {
            node_id: 0,
            total_memory: 8 * 1024 * 1024 * 1024, // 8GB
            available_memory: 7 * 1024 * 1024 * 1024, // 7GB
            cpu_mask: 0xFF, // CPU 0-7
        },
        NumaNodeInfo {
            node_id: 1,
            total_memory: 8 * 1024 * 1024 * 1024, // 8GB
            available_memory: 7 * 1024 * 1024 * 1024, // 7GB
            cpu_mask: 0xFF00, // CPU 8-15
        },
    ];
    
    // 创建本地分配器
    let allocator = NumaAllocator::new(nodes, NumaAllocPolicy::Local);
    
    // 分配不同大小的内存块
    let sizes = [1024, 4096, 16384, 65536]; // 1KB, 4KB, 16KB, 64KB
    
    for &size in &sizes {
        let layout = Layout::from_size_align(size, 8)?;
        match allocator.allocate(layout) {
            Ok(ptr) => {
                println!("成功分配 {} 字节内存，地址: {:p}", size, ptr.as_ptr());
                // 使用内存...
                unsafe {
                    std::ptr::write_bytes(ptr.as_ptr(), 0xAA, size);
                }
                // 释放内存
                allocator.deallocate(ptr, size);
            }
            Err(e) => {
                println!("分配失败: {}", e);
            }
        }
    }
    
    // 打印统计信息
    let stats = allocator.stats();
    println!("本地分配统计:");
    println!("  本地分配: {}", 
        stats.local_allocs.load(std::sync::atomic::Ordering::Relaxed));
    println!("  远程分配: {}", 
        stats.remote_allocs.load(std::sync::atomic::Ordering::Relaxed));
    
    Ok(())
}

/// 演示交错分配策略
fn demo_interleaved_allocation() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== 交错分配策略示例 ===");
    
    // 创建测试NUMA节点
    let nodes = vec![
        NumaNodeInfo {
            node_id: 0,
            total_memory: 4 * 1024 * 1024 * 1024, // 4GB
            available_memory: 3 * 1024 * 1024 * 1024, // 3GB
            cpu_mask: 0xFF, // CPU 0-7
        },
        NumaNodeInfo {
            node_id: 1,
            total_memory: 4 * 1024 * 1024 * 1024, // 4GB
            available_memory: 3 * 1024 * 1024 * 1024, // 3GB
            cpu_mask: 0xFF00, // CPU 8-15
        },
        NumaNodeInfo {
            node_id: 2,
            total_memory: 4 * 1024 * 1024 * 1024, // 4GB
            available_memory: 3 * 1024 * 1024 * 1024, // 3GB
            cpu_mask: 0xFF0000, // CPU 16-23
        },
    ];
    
    // 创建交错分配器
    let allocator = NumaAllocator::new(nodes, NumaAllocPolicy::Interleave);
    
    // 分配多个内存块
    let block_count = 10;
    let block_size = 64 * 1024; // 64KB
    
    for i in 0..block_count {
        let layout = Layout::from_size_align(block_size, 8)?;
        match allocator.allocate(layout) {
            Ok(ptr) => {
                println!("交错分配块 {}: {} 字节，地址: {:p}", i, block_size, ptr.as_ptr());
                // 使用内存...
                unsafe {
                    std::ptr::write_bytes(ptr.as_ptr(), (i % 256) as u8, block_size);
                }
                // 释放内存
                allocator.deallocate(ptr, block_size);
            }
            Err(e) => {
                println!("分配失败: {}", e);
            }
        }
    }
    
    // 打印统计信息
    let stats = allocator.stats();
    println!("交错分配统计:");
    println!("  本地分配: {}", 
        stats.local_allocs.load(std::sync::atomic::Ordering::Relaxed));
    println!("  远程分配: {}", 
        stats.remote_allocs.load(std::sync::atomic::Ordering::Relaxed));
    
    Ok(())
}

/// 演示绑定分配策略
fn demo_bound_allocation() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== 绑定分配策略示例 ===");
    
    // 创建测试NUMA节点
    let nodes = vec![
        NumaNodeInfo {
            node_id: 0,
            total_memory: 8 * 1024 * 1024 * 1024, // 8GB
            available_memory: 7 * 1024 * 1024 * 1024, // 7GB
            cpu_mask: 0xFF, // CPU 0-7
        },
        NumaNodeInfo {
            node_id: 1,
            total_memory: 8 * 1024 * 1024 * 1024, // 8GB
            available_memory: 7 * 1024 * 1024 * 1024, // 7GB
            cpu_mask: 0xFF00, // CPU 8-15
        },
    ];
    
    // 创建绑定到节点1的分配器
    let allocator = NumaAllocator::new(nodes, NumaAllocPolicy::Bind(1));
    
    // 分配大内存块
    let block_size = 1024 * 1024; // 1MB
    
    let layout = Layout::from_size_align(block_size, 8)?;
    match allocator.allocate(layout) {
        Ok(ptr) => {
            println!("绑定分配: {} 字节，地址: {:p}，绑定到节点 1", 
                block_size, ptr.as_ptr());
            
            // 使用内存...
            unsafe {
                std::ptr::write_bytes(ptr.as_ptr(), 0xBB, block_size);
            }
            
            // 释放内存
            allocator.deallocate(ptr, block_size);
        }
        Err(e) => {
            println!("绑定分配失败: {}", e);
        }
    }
    
    // 打印统计信息
    let stats = allocator.stats();
    println!("绑定分配统计:");
    println!("  本地分配: {}", 
        stats.local_allocs.load(std::sync::atomic::Ordering::Relaxed));
    println!("  远程分配: {}", 
        stats.remote_allocs.load(std::sync::atomic::Ordering::Relaxed));
    
    Ok(())
}

/// 演示多线程NUMA分配
fn demo_multithreaded_allocation() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== 多线程NUMA分配示例 ===");
    
    use std::sync::Arc;
    use std::thread;
    
    // 创建测试NUMA节点
    let nodes = vec![
        NumaNodeInfo {
            node_id: 0,
            total_memory: 8 * 1024 * 1024 * 1024, // 8GB
            available_memory: 7 * 1024 * 1024 * 1024, // 7GB
            cpu_mask: 0xFF, // CPU 0-7
        },
        NumaNodeInfo {
            node_id: 1,
            total_memory: 8 * 1024 * 1024 * 1024, // 8GB
            available_memory: 7 * 1024 * 1024 * 1024, // 7GB
            cpu_mask: 0xFF00, // CPU 8-15
        },
    ];
    
    // 创建共享分配器
    let allocator = Arc::new(NumaAllocator::new(nodes, NumaAllocPolicy::Local));
    
    let thread_count = 4;
    let allocations_per_thread = 100;
    let block_size = 4096; // 4KB
    
    let mut handles = Vec::new();
    
    // 创建多个线程进行内存分配
    for thread_id in 0..thread_count {
        let allocator_clone = allocator.clone();
        let handle = thread::spawn(move || {
            println!("线程 {} 开始分配内存", thread_id);
            
            for i in 0..allocations_per_thread {
                let layout = Layout::from_size_align(block_size, 8).unwrap();
                match allocator_clone.allocate(layout) {
                    Ok(ptr) => {
                        // 使用内存...
                        unsafe {
                            std::ptr::write_bytes(ptr.as_ptr(), 
                                ((thread_id * 10 + i) % 256) as u8, block_size);
                        }
                        
                        // 释放内存
                        allocator_clone.deallocate(ptr, block_size);
                    }
                    Err(_) => {
                        eprintln!("线程 {} 分配 {} 失败", thread_id, i);
                    }
                }
            }
            
            println!("线程 {} 完成内存分配", thread_id);
        });
        
        handles.push(handle);
    }
    
    // 等待所有线程完成
    for handle in handles {
        handle.join().unwrap();
    }
    
    // 打印最终统计信息
    let stats = allocator.stats();
    println!("多线程分配统计:");
    println!("  本地分配: {}", 
        stats.local_allocs.load(std::sync::atomic::Ordering::Relaxed));
    println!("  远程分配: {}", 
        stats.remote_allocs.load(std::sync::atomic::Ordering::Relaxed));
    println!("  失败分配: {}", 
        stats.failed_allocs.load(std::sync::atomic::Ordering::Relaxed));
    
    Ok(())
}