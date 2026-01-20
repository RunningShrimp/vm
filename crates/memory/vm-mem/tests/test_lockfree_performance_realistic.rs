//! Lock-free MMU 现实性能测试
//!
//! 测试目标：
//! - 验证并发场景下的正确性（无死锁、无数据竞争）
//! - 测试可扩展性（吞吐量随线程数变化）
//! - 验证长时间运行的稳定性

use std::sync::Arc;
use std::sync::Barrier;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use vm_core::{GuestAddr, MemoryAccess};
use vm_mem::LockFreeMMU;

/// 测试并发场景的正确性和稳定性
#[test]
fn test_concurrent_stress() {
    let num_threads = 16;
    let ops_per_thread = 10_000;
    let mmu = Arc::new(LockFreeMMU::new(16 * 1024 * 1024, false));
    mmu.set_paging_mode(8);

    let barrier = Arc::new(Barrier::new(num_threads));
    let mut handles = vec![];

    let start = Instant::now();

    for thread_id in 0..num_threads {
        let mmu_clone = Arc::clone(&mmu);
        let barrier_clone: Arc<Barrier> = Arc::clone(&barrier);

        handles.push(thread::spawn(move || {
            barrier_clone.wait();

            for i in 0..ops_per_thread {
                let guest_addr = GuestAddr((thread_id * 0x100000 + i * 8) as u64);
                mmu_clone.update_mapping(guest_addr, guest_addr.0);

                let host = mmu_clone.read(guest_addr, 8);
                assert!(host.is_ok(), "读取失败: thread={}, i={}", thread_id, i);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let elapsed = start.elapsed();
    let total_ops = num_threads * ops_per_thread;

    println!("并发压力测试:");
    println!("  线程数: {}", num_threads);
    println!("  每线程操作数: {}", ops_per_thread);
    println!("  总操作数: {}", total_ops);
    println!("  总耗时: {:?}", elapsed);
    println!(
        "  平均吞吐量: {:.2} ops/ms",
        (total_ops as f64) / (elapsed.as_micros() as f64 / 1000.0)
    );

    // 验证无死锁（在合理时间内完成）
    assert!(elapsed < Duration::from_secs(10), "测试超时，可能存在死锁");

    // 验证统计信息
    let stats = mmu.stats();
    println!("  总翻译次数: {}", stats.translations);
    println!("  TLB命中数: {}", stats.tlb_hits);
    println!("  TLB未命中数: {}", stats.tlb_misses);
    println!("  TLB命中率: {:.2}%", mmu.hit_rate() * 100.0);
}

/// 测试可扩展性
#[test]
fn test_scalability() {
    let num_threads_list = [1, 2, 4, 8, 16];
    let ops_per_thread = 5_000;

    println!("\n可扩展性测试:");
    println!("每线程操作数: {}", ops_per_thread);

    for &num_threads in &num_threads_list {
        let mmu = Arc::new(LockFreeMMU::new(16 * 1024 * 1024, false));
        mmu.set_paging_mode(8);

        // 预先建立映射
        for thread_id in 0..num_threads {
            for i in 0..ops_per_thread {
                let addr = GuestAddr((thread_id * 0x100000 + i * 8) as u64);
                mmu.update_mapping(addr, addr.0);
            }
        }

        let barrier = Arc::new(Barrier::new(num_threads));
        let mut handles = vec![];

        let start = Instant::now();

        for thread_id in 0..num_threads {
            let mmu_clone = Arc::clone(&mmu);
            let barrier_clone: Arc<Barrier> = Arc::clone(&barrier);

            handles.push(thread::spawn(move || {
                barrier_clone.wait();

                for i in 0..ops_per_thread {
                    let addr = GuestAddr((thread_id * 0x100000 + i * 8) as u64);
                    let _ = mmu_clone.read(addr, 8);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let elapsed = start.elapsed();
        let total_ops = num_threads * ops_per_thread;
        let throughput = (total_ops as f64) / (elapsed.as_micros() as f64 / 1000.0);

        println!(
            "{}线程: {:>8.3?} (吞吐量: {:>8.2} ops/ms)",
            num_threads, elapsed, throughput
        );
    }
}

/// 测试长时间运行的稳定性
#[test]
fn test_long_running_stability() {
    let num_threads = 4;
    let ops_per_thread = 100_000;
    let mmu = Arc::new(LockFreeMMU::new(16 * 1024 * 1024, false));
    mmu.set_paging_mode(8);

    let barrier = Arc::new(Barrier::new(num_threads));
    let mut handles = vec![];

    let start = Instant::now();

    for thread_id in 0..num_threads {
        let mmu_clone = Arc::clone(&mmu);
        let barrier_clone: Arc<Barrier> = Arc::clone(&barrier);

        handles.push(thread::spawn(move || {
            barrier_clone.wait();

            for i in 0..ops_per_thread {
                let guest_addr = GuestAddr((thread_id * 0x100000 + i * 8) as u64);

                // 混合读写操作
                if i % 10 == 0 {
                    mmu_clone.update_mapping(guest_addr, guest_addr.0);
                }

                let host = mmu_clone.read(guest_addr, 8);
                assert!(
                    host.is_ok(),
                    "长时间运行测试失败: thread={}, i={}",
                    thread_id,
                    i
                );
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let elapsed = start.elapsed();
    let total_ops = num_threads * ops_per_thread;

    println!("\n长时间运行稳定性测试:");
    println!("  每线程操作数: {}", ops_per_thread);
    println!("  总操作数: {}", total_ops);
    println!("  总耗时: {:?}", elapsed);
    println!(
        "  平均吞吐量: {:.2} ops/ms",
        (total_ops as f64) / (elapsed.as_micros() as f64 / 1000.0)
    );

    // 验证稳定性
    assert!(elapsed < Duration::from_secs(30), "长时间运行测试超时");

    // 验证统计信息的一致性
    let stats = mmu.stats();
    assert_eq!(stats.translations as usize, total_ops, "翻译次数不匹配");
}

/// 测试 TLB 性能
#[test]
fn test_tlb_performance() {
    let mmu = LockFreeMMU::new(16 * 1024 * 1024, false);
    mmu.set_paging_mode(8);
    mmu.update_mapping(GuestAddr(0x1000), 0x2000);

    // 测试 TLB 命中性能（热缓存）
    let start = Instant::now();
    for _ in 0..100_000 {
        let _ = mmu.read(GuestAddr(0x1000), 8);
    }
    let hot_cache_time = start.elapsed();

    // 测试 TLB 未命中性能（冷缓存）
    let start = Instant::now();
    for i in 0..100_000 {
        let addr = GuestAddr(0x1000 + i * 8);
        mmu.update_mapping(addr, addr.0);
        let _ = mmu.read(addr, 8);
    }
    let cold_cache_time = start.elapsed();

    let stats = mmu.stats();
    let hit_rate = mmu.hit_rate();

    println!("\nTLB 性能测试:");
    println!("  热缓存 (100k次): {:?}", hot_cache_time);
    println!("  冷缓存 (100k次): {:?}", cold_cache_time);
    println!("  TLB命中率: {:.2}%", hit_rate * 100.0);
    println!("  总翻译次数: {}", stats.translations);
    println!("  TLB命中数: {}", stats.tlb_hits);
    println!("  TLB未命中数: {}", stats.tlb_misses);

    // 验证 TLB 有效
    assert!(hit_rate > 0.0, "TLB命中率应该大于0");
}
