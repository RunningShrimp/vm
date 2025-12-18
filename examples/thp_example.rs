//! 透明大页(THP)使用示例
//!
//! 展示如何使用透明大页(THP)来减少TLB压力，提高内存访问性能。

use std::alloc::Layout;
use vm_mem::{
    init_global_thp_manager, allocate_with_thp, deallocate_with_thp, is_thp_address,
    get_thp_usage_stats, ThpPolicy, TransparentHugePageManager, ThpConfig,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("透明大页(THP)使用示例");
    
    // 初始化全局THP管理器
    init_global_thp_manager(ThpPolicy::Transparent)?;
    
    // 打印THP配置信息
    match TransparentHugePageManager::get_thp_config() {
        Ok(config) => {
            println!("THP配置:");
            println!("  策略: {:?}", config.enabled);
            println!("  碎片整理: {}", config.defrag);
            println!("  使用零页: {}", config.use_zero_page);
        }
        Err(e) => {
            println!("获取THP配置失败: {}", e);
        }
    }
    
    // 示例1: 基本THP分配
    demo_basic_thp_allocation()?;
    
    // 示例2: 比较THP与常规分配性能
    demo_performance_comparison()?;
    
    // 示例3: 检查THP使用统计
    demo_thp_usage_stats()?;
    
    // 示例4: 不同THP策略的效果
    demo_thp_policies()?;
    
    Ok(())
}

/// 演示基本THP分配
fn demo_basic_thp_allocation() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== 基本THP分配示例 ===");
    
    // 分配不同大小的内存块
    let sizes = [4096, 65536, 1048576, 16777216]; // 4KB, 64KB, 1MB, 16MB
    
    for &size in &sizes {
        match allocate_with_thp(size) {
            Ok(ptr) if !ptr.is_null() => {
                println!("成功分配 {} 字节内存，地址: {:p}", size, ptr);
                
                // 检查是否使用了THP
                let is_thp = is_thp_address(ptr);
                println!("  使用THP: {}", is_thp);
                
                // 使用内存...
                unsafe {
                    std::ptr::write_bytes(ptr, 0xCC, size);
                }
                
                // 释放内存
                deallocate_with_thp(ptr, size);
            }
            Ok(_) => {
                println!("分配失败: 返回空指针");
            }
            Err(e) => {
                println!("分配失败: {}", e);
            }
        }
    }
    
    Ok(())
}

/// 演示THP与常规分配的性能比较
fn demo_performance_comparison() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== THP性能比较示例 ===");
    
    use std::time::Instant;
    
    let block_size = 2 * 1024 * 1024; // 2MB
    let block_count = 100;
    
    // 测试THP分配性能
    println!("测试THP分配性能...");
    let thp_start = Instant::now();
    let mut thp_ptrs = Vec::new();
    
    for _ in 0..block_count {
        match allocate_with_thp(block_size) {
            Ok(ptr) if !ptr.is_null() => {
                thp_ptrs.push(ptr);
            }
            _ => {}
        }
    }
    
    let thp_alloc_time = thp_start.elapsed();
    
    // 使用内存
    for &ptr in &thp_ptrs {
        unsafe {
            std::ptr::write_bytes(ptr, 0xDD, block_size);
        }
    }
    
    // 释放内存
    for ptr in thp_ptrs {
        deallocate_with_thp(ptr, block_size);
    }
    
    // 测试常规分配性能
    println!("测试常规分配性能...");
    let regular_start = Instant::now();
    let mut regular_ptrs = Vec::new();
    
    for _ in 0..block_count {
        let layout = Layout::from_size_align(block_size, 8)?;
        let ptr = unsafe { std::alloc::alloc(layout) };
        if !ptr.is_null() {
            regular_ptrs.push(ptr);
        }
    }
    
    let regular_alloc_time = regular_start.elapsed();
    
    // 使用内存
    for &ptr in &regular_ptrs {
        unsafe {
            std::ptr::write_bytes(ptr, 0xEE, block_size);
        }
    }
    
    // 释放内存
    for ptr in regular_ptrs {
        unsafe {
            let layout = Layout::from_size_align(block_size, 8).unwrap();
            std::alloc::dealloc(ptr, layout);
        }
    }
    
    // 打印性能比较结果
    println!("性能比较结果:");
    println!("  THP分配时间: {:?}", thp_alloc_time);
    println!("  常规分配时间: {:?}", regular_alloc_time);
    
    if thp_alloc_time < regular_alloc_time {
        let speedup = regular_alloc_time.as_nanos() as f64 / thp_alloc_time.as_nanos() as f64;
        println!("  THP加速比: {:.2}x", speedup);
    } else {
        println!("  THP未显示性能优势");
    }
    
    Ok(())
}

/// 演示THP使用统计
fn demo_thp_usage_stats() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== THP使用统计示例 ===");
    
    match get_thp_usage_stats() {
        Ok(stats) => {
            println!("THP使用统计:");
            println!("  匿名大页: {}", stats.anon_huge_pages);
            println!("  共享内存大页: {}", stats.shmem_huge_pages);
            println!("  总大页: {}", stats.huge_pages_total);
            println!("  空闲大页: {}", stats.huge_pages_free);
            println!("  保留大页: {}", stats.huge_pages_reserved);
            println!("  大页大小: {} kB", stats.huge_page_size);
            
            if stats.huge_pages_total > 0 {
                let usage_ratio = (stats.huge_pages_total - stats.huge_pages_free) as f64 / stats.huge_pages_total as f64;
                println!("  大页使用率: {:.1}%", usage_ratio * 100.0);
            }
        }
        Err(e) => {
            println!("获取THP统计失败: {}", e);
        }
    }
    
    Ok(())
}

/// 演示不同THP策略的效果
fn demo_thp_policies() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== THP策略示例 ===");
    
    let policies = [
        (ThpPolicy::Always, "Always"),
        (ThpPolicy::Never, "Never"),
        (ThpPolicy::Madvise, "Madvise"),
        (ThpPolicy::Transparent, "Transparent"),
    ];
    
    let test_size = 4 * 1024 * 1024; // 4MB
    
    for (policy, name) in &policies {
        println!("\n测试策略: {}", name);
        
        // 创建THP管理器
        match TransparentHugePageManager::new(*policy) {
            Ok(manager) => {
                // 测试分配
                match manager.allocate_with_thp(test_size) {
                    Ok(ptr) if !ptr.is_null() => {
                        println!("  分配成功: {:p}", ptr);
                        
                        // 检查是否使用了THP
                        let is_thp = TransparentHugePageManager::is_thp_address(ptr);
                        println!("  使用THP: {}", is_thp);
                        
                        // 释放内存
                        manager.deallocate_thp(ptr, test_size);
                    }
                    Ok(_) => {
                        println!("  分配失败: 返回空指针");
                    }
                    Err(e) => {
                        println!("  分配失败: {}", e);
                    }
                }
                
                // 打印统计信息
                let stats = manager.stats();
                println!("  THP分配: {}", 
                    stats.thp_allocations.load(std::sync::atomic::Ordering::Relaxed));
                println!("  常规分配: {}", 
                    stats.normal_allocations.load(std::sync::atomic::Ordering::Relaxed));
            }
            Err(e) => {
                println!("  创建THP管理器失败: {}", e);
            }
        }
    }
    
    Ok(())
}