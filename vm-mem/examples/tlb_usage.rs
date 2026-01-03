//! # TLB (Translation Lookaside Buffer) 使用示例
//!
//! 本示例详细演示如何使用TLB来优化虚拟地址转换性能。
//!
//! ## 功能演示
//!
//! - ✅ 创建不同类型的TLB
//! - ✅ TLB查找和更新
//! - ✅ TLB失效操作
//! - ✅ TLB性能统计
//! - ✅ 多级TLB使用
//!
//! ## 运行
//!
//! ```bash
//! cargo run --example tlb_usage --features "vm-frontend/riscv64"
//! ```

use vm_core::{GuestAddr, GuestPhysAddr, AccessType, TlbEntry, TlbManager};
use vm_mem::{
    MultiLevelTlb, MultiLevelTlbConfig, ConcurrentTlbManager, ConcurrentTlbConfig,
    AdaptiveReplacementPolicy, TlbFactory
};
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("======================================");
    println!("  TLB使用示例");
    println!("======================================");
    println!();

    // 示例1: 基础TLB使用
    basic_tlb_usage()?;

    println!();
    println!("======================================");
    println!();

    // 示例2: 多级TLB使用
    multi_level_tlb_usage()?;

    println!();
    println!("======================================");
    println!();

    // 示例3: 并发TLB使用
    concurrent_tlb_usage()?;

    println!();
    println!("======================================");
    println!("  示例完成!");
    println!("======================================");

    Ok(())
}

/// 示例1: 基础TLB使用
///
/// 演示基本的TLB操作：查找、更新、失效
fn basic_tlb_usage() -> Result<(), Box<dyn std::error::Error>> {
    println!("示例1: 基础TLB使用");
    println!("----------------------");
    println!();

    // 创建TLB (使用MultiLevelTlb作为示例)
    let config = MultiLevelTlbConfig {
        l1_size: 16,
        l2_size: 64,
        l3_size: 256,
        asid_bits: 8,
    };
    let mut tlb = MultiLevelTlb::new(config);

    println!("✓ 创建TLB (L1=16, L2=64, L3=256条目)");
    println!();

    // 模拟TLB查找
    let test_addr = GuestAddr(0x1000);
    let asid = 1;

    println!("步骤1: TLB查找");
    println!("  查找地址: 0x{:x}, ASID: {}", test_addr, asid);
    match tlb.lookup(test_addr, asid, AccessType::Read) {
        Some(entry) => {
            println!("  ✓ TLB命中!");
            println!("    虚拟地址: 0x{:x}", entry.guest_addr);
            println!("    物理地址: 0x{:x}", entry.phys_addr);
            println!("    标志: 0x{:x}", entry.flags);
        }
        None => {
            println!("  ⚠ TLB未命中 (预期，初始状态)");
        }
    }
    println!();

    // 插入TLB条目
    println!("步骤2: 插入TLB条目");
    let entry = TlbEntry {
        guest_addr: test_addr,
        phys_addr: GuestPhysAddr(0x1000),
        flags: 0x7, // R+W+X
        asid,
    };
    tlb.update(entry);
    println!("  ✓ 插入条目: virt=0x{:x} -> phys=0x{:x}", test_addr, entry.phys_addr);
    println!();

    // 再次查找
    println!("步骤3: 再次查找");
    match tlb.lookup(test_addr, asid, AccessType::Read) {
        Some(entry) => {
            println!("  ✓ TLB命中!");
            println!("    虚拟地址: 0x{:x}", entry.guest_addr);
            println!("    物理地址: 0x{:x}", entry.phys_addr);
        }
        None => {
            println!("  ✗ TLB未命中 (不应发生)");
        }
    }
    println!();

    // TLB失效
    println!("步骤4: TLB失效");
    tlb.flush();
    println!("  ✓ TLB已清空");

    match tlb.lookup(test_addr, asid, AccessType::Read) {
        Some(_) => {
            println!("  ✗ TLB仍命中 (不应发生)");
        }
        None => {
            println!("  ✓ TLB未命中 (已清空)");
        }
    }
    println!();

    Ok(())
}

/// 示例2: 多级TLB使用
///
/// 演示多级TLB的层次结构和性能优势
fn multi_level_tlb_usage() -> Result<(), Box<dyn std::error::Error>> {
    println!("示例2: 多级TLB使用");
    println!("----------------------");
    println!();

    let config = MultiLevelTlbConfig {
        l1_size: 16,   // L1 TLB: 快速但小
        l2_size: 64,   // L2 TLB: 中等速度和大小
        l3_size: 256,  // L3 TLB: 较慢但大
        asid_bits: 8,
    };
    let mut tlb = MultiLevelTlb::new(config);

    println!("✓ 创建多级TLB:");
    println!("  L1 TLB: {} 条目 (最快)", config.l1_size);
    println!("  L2 TLB: {} 条目 (中等)", config.l2_size);
    println!("  L3 TLB: {} 条目 (较大)", config.l3_size);
    println!();

    // 性能测试
    println!("步骤1: 性能测试");
    let iterations = 1000;

    // 插入一些条目
    for i in 0..64 {
        let entry = TlbEntry {
            guest_addr: GuestAddr(0x1000 + i * 0x1000),
            phys_addr: GuestPhysAddr(0x1000 + i * 0x1000),
            flags: 0x7,
            asid: 1,
        };
        tlb.update(entry);
    }

    // 测试查找性能
    let start = Instant::now();
    let mut hits = 0;
    for i in 0..iterations {
        let addr = GuestAddr(0x1000 + (i % 64) * 0x1000);
        if tlb.lookup(addr, 1, AccessType::Read).is_some() {
            hits += 1;
        }
    }
    let elapsed = start.elapsed();

    println!("  测试结果:");
    println!("    查找次数: {}", iterations);
    println!("    命中次数: {}", hits);
    println!("    命中率: {:.1}%", (hits as f64 / iterations as f64) * 100.0);
    println!("    总耗时: {:?} ({:.2} ns/次)", elapsed, elapsed.as_nanos() as f64 / iterations as f64);
    println!();

    // 显示统计信息
    println!("步骤2: TLB统计");
    if let Some(stats) = tlb.get_stats() {
        println!("  L1 TLB:");
        println!("    命中率: {:.1}%", stats.l1_hit_rate);
        println!("  L2 TLB:");
        println!("    命中率: {:.1}%", stats.l2_hit_rate);
        println!("  总体:");
        println!("    总查找次数: {}", stats.total_lookups);
        println!("    总命中次数: {}", stats.hits);
        println!("    总命中率: {:.1}%", stats.hit_rate);
    }
    println!();

    Ok(())
}

/// 示例3: 并发TLB使用
///
/// 演示并发安全的TLB实现，适用于多线程环境
fn concurrent_tlb_usage() -> Result<(), Box<dyn std::error::Error>> {
    println!("示例3: 并发TLB使用");
    println!("----------------------");
    println!();

    let config = ConcurrentTlbConfig {
        num_shards: 16,  // 16个分片减少锁竞争
        entries_per_shard: 64,
        asid_bits: 8,
        replacement_policy: AdaptiveReplacementPolicy::Arc,
    };
    let tlb = ConcurrentTlbManager::new(config);

    println!("✓ 创建并发TLB:");
    println!("  分片数: {}", config.num_shards);
    println!("  每分片条目: {}", config.entries_per_shard);
    println!("  总条目: {}", config.num_shards * config.entries_per_shard);
    println!("  替换策略: {:?}", config.replacement_policy);
    println!();

    // 并发测试
    println!("步骤1: 并发插入测试");
    let start = Instant::now();
    let mut handles = vec![];

    for thread_id in 0..4 {
        let tlb_clone = tlb.clone();
        let handle = std::thread::spawn(move || {
            for i in 0..16 {
                let entry = TlbEntry {
                    guest_addr: GuestAddr(0x1000 + (thread_id * 16 + i) * 0x1000),
                    phys_addr: GuestPhysAddr(0x1000 + (thread_id * 16 + i) * 0x1000),
                    flags: 0x7,
                    asid: thread_id as u16,
                };
                tlb_clone.update(entry);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let elapsed = start.elapsed();
    println!("  ✓ 4个线程并发插入64个条目");
    println!("    耗时: {:?}", elapsed);
    println!();

    // 并发查找测试
    println!("步骤2: 并发查找测试");
    let start = Instant::now();
    let mut handles = vec![];

    for thread_id in 0..4 {
        let tlb_clone = tlb.clone();
        let handle = std::thread::spawn(move || {
            let mut local_hits = 0;
            for i in 0..100 {
                let addr = GuestAddr(0x1000 + (thread_id * 16 + (i % 16)) * 0x1000);
                    if tlb_clone.lookup(addr, thread_id as u16, AccessType::Read).is_some() {
                        local_hits += 1;
                    }
                }
            local_hits
        });
        handles.push(handle);
    }

    let total_hits: u32 = handles.into_iter()
        .map(|h| h.join().unwrap())
        .sum();

    let elapsed = start.elapsed();
    println!("  ✓ 4个线程并发查找400次");
    println!("    总命中次数: {}", total_hits);
    println!("    命中率: {:.1}%", (total_hits as f64 / 400.0) * 100.0);
    println!("    耗时: {:?}", elapsed);
    println!();

    // 显示最终统计
    println!("步骤3: 最终TLB统计");
    if let Some(stats) = tlb.get_stats() {
        println!("  总查找次数: {}", stats.total_lookups);
        println!("  命中次数: {}", stats.hits);
        println!("  缺失次数: {}", stats.misses);
        println!("  命中率: {:.1}%", stats.hit_rate);
        println!("  当前条目数: {}", stats.current_entries);
        println!("  容量: {}", stats.capacity);
    }
    println!();

    Ok(())
}
