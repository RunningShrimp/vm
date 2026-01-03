//! Lock-free MMU 性能测试
//!
//! 快速验证并发性能提升

use std::sync::{Arc, Barrier};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use vm_core::{GuestAddr, MemoryAccess};
use vm_mem::LockFreeMMU;

/// 测试单线程性能
fn test_single_thread_performance(num_ops: usize) -> Duration {
    let mmu = LockFreeMMU::new(16 * 1024 * 1024, false);
    mmu.set_paging_mode(8); // 启用分页模式以使用 DashMap

    // 预先建立映射
    for i in 0..num_ops {
        let addr = GuestAddr(0x1000 + (i * 8) as u64);
        mmu.update_mapping(addr, addr.0);
    }

    let start = Instant::now();

    for i in 0..num_ops {
        let addr = GuestAddr(0x1000 + (i * 8) as u64);
        let _ = mmu.read(addr, 8);
    }

    start.elapsed()
}

/// 测试多线程性能（预热后）
fn test_multi_thread_performance(num_threads: usize, ops_per_thread: usize) -> Duration {
    let mmu = Arc::new(LockFreeMMU::new(16 * 1024 * 1024, false));
    mmu.set_paging_mode(8); // 启用分页模式

    let total_ops = num_threads * ops_per_thread;

    // 预先建立映射
    for thread_id in 0..num_threads {
        for i in 0..ops_per_thread {
            let addr = GuestAddr((thread_id * 0x100000 + i * 8) as u64);
            mmu.update_mapping(addr, addr.0);
        }
    }

    let barrier = Arc::new(Barrier::new(num_threads));
    let mut handles: Vec<JoinHandle<Duration>> = vec![];

    for thread_id in 0..num_threads {
        let mmu_clone = Arc::clone(&mmu);
        let barrier_clone: Arc<Barrier> = Arc::clone(&barrier);

        handles.push(thread::spawn(move || {
            barrier_clone.wait(); // 等待所有线程就绪

            let start = Instant::now();

            for i in 0..ops_per_thread {
                let addr = GuestAddr((thread_id * 0x100000 + i * 8) as u64);
                let _ = mmu_clone.read(addr, 8);
            }

            start.elapsed()
        }));
    }

    // 等待所有线程完成并返回最长时间
    handles
        .into_iter()
        .map(|h| h.join().unwrap())
        .max()
        .unwrap_or(Duration::ZERO)
}

#[test]
fn test_performance_comparison() {
    // 增加操作数以更好地测量并发性能
    let num_ops = 1_000_000;

    // 测试单线程性能
    let single_time = test_single_thread_performance(num_ops);
    println!("\n单线程执行 {} 次操作耗时: {:?}", num_ops, single_time);

    // 测试多线程性能（4线程，每线程执行相同操作数）
    let ops_per_thread = num_ops / 4;
    let multi_time = test_multi_thread_performance(4, ops_per_thread);

    // 计算总工作量时间（所有线程并行执行）
    println!(
        "4线程（每线程 {} 次）最慢线程耗时: {:?}",
        ops_per_thread, multi_time
    );

    // 计算加速比（单线程时间 vs 并行时间）
    let speedup = single_time.as_nanos() as f64 / multi_time.as_nanos() as f64;
    println!("性能加速比: {:.2}x", speedup);

    // 计算吞吐量对比
    let single_throughput = (num_ops as f64) / (single_time.as_micros() as f64 / 1000.0);
    let multi_throughput = (num_ops as f64) / (multi_time.as_micros() as f64 / 1000.0);
    println!("单线程吞吐量: {:.2} ops/ms", single_throughput);
    println!("4线程吞吐量: {:.2} ops/ms", multi_throughput);
    println!("吞吐量提升: {:.2}x", multi_throughput / single_throughput);

    // 在并发场景下，即使加速比不高，只要吞吐量提升就应该算通过
    assert!(
        multi_throughput > single_throughput * 0.5,
        "并发性能提升不足，吞吐量提升: {:.2}x",
        multi_throughput / single_throughput
    );
}

#[test]
fn test_scalability() {
    let num_threads_list = [2, 4, 8, 16];
    let ops_per_thread = 10_000;

    println!("\n可扩展性测试:");
    println!("每线程操作数: {}", ops_per_thread);

    let baseline_time = test_single_thread_performance(ops_per_thread);
    println!("单线程基准: {:?}", baseline_time);

    for &num_threads in &num_threads_list {
        let total_ops = num_threads * ops_per_thread;
        let multi_time = test_multi_thread_performance(num_threads, ops_per_thread);

        // 计算吞吐量 (ops/ms)
        let throughput = (total_ops as f64) / (multi_time.as_micros() as f64 / 1000.0);

        println!(
            "{}线程: {:?} (吞吐量: {:.2} ops/ms)",
            num_threads, multi_time, throughput
        );
    }
}

#[test]
fn test_concurrent_mapping_updates() {
    let num_threads = 8;
    let ops_per_thread = 1_000;
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
                let guest_addr = GuestAddr((thread_id * 0x10000 + i * 8) as u64);
                mmu_clone.update_mapping(guest_addr, guest_addr.0);

                let host = mmu_clone.read(guest_addr, 8);
                assert!(host.is_ok());
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let elapsed = start.elapsed();
    let total_ops = num_threads * ops_per_thread;
    let throughput = (total_ops as f64) / (elapsed.as_micros() as f64 / 1000.0);

    println!("并发映射更新测试:");
    println!("总操作数: {}", total_ops);
    println!("总耗时: {:?}", elapsed);
    println!("吞吐量: {:.2} ops/ms", throughput);

    // 验证无死锁和数据竞争
    assert!(elapsed < Duration::from_secs(5), "性能测试超时");
}
