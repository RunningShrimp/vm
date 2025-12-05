//! MMU模糊测试
//!
//! 针对内存管理单元（MMU）的模糊测试，测试各种边界条件和异常情况

use vm_core::{AccessType, GuestAddr, MMU};
use vm_mem::SoftMmu;
use rand::Rng;

/// 模糊测试：MMU内存读写操作
#[test]
fn fuzz_mmu_read_write() {
    let mut rng = rand::thread_rng();
    let mut mmu = SoftMmu::new(1024 * 1024 * 1024, false); // 1GB内存

    for _ in 0..10000 {
        let addr = rng.gen_range(0..(1024 * 1024 * 1024));
        let size = [1, 2, 4, 8][rng.gen_range(0..4)];
        
        // 随机选择读写操作
        match rng.gen_range(0..2) {
            0 => {
                // 写入操作
                let value = rng.gen::<u64>();
                let _ = mmu.write(addr, value, size);
            }
            _ => {
                // 读取操作
                let _ = mmu.read(addr, size);
            }
        }
    }
}

/// 模糊测试：MMU批量操作
#[test]
fn fuzz_mmu_bulk_operations() {
    let mut rng = rand::thread_rng();
    let mut mmu = SoftMmu::new(1024 * 1024, false); // 1MB内存

    for _ in 0..1000 {
        let addr = rng.gen_range(0..(1024 * 1024 - 1024));
        let size = rng.gen_range(1..1024);
        
        match rng.gen_range(0..2) {
            0 => {
                // 批量写入
                let data: Vec<u8> = (0..size).map(|_| rng.gen()).collect();
                let _ = mmu.write_bulk(addr, &data);
            }
            _ => {
                // 批量读取
                let mut buf = vec![0u8; size];
                let _ = mmu.read_bulk(addr, &mut buf);
            }
        }
    }
}

/// 模糊测试：MMU地址翻译
#[test]
fn fuzz_mmu_translation() {
    let mut rng = rand::thread_rng();
    let mut mmu = SoftMmu::new(1024 * 1024 * 1024, true); // 启用分页

    for _ in 0..10000 {
        let va = rng.gen_range(0..(1024 * 1024 * 1024));
        let access = match rng.gen_range(0..3) {
            0 => AccessType::Read,
            1 => AccessType::Write,
            _ => AccessType::Exec,
        };

        // 翻译地址（可能失败，但不应该panic）
        let _ = mmu.translate(va, access);
    }
}

/// 模糊测试：MMU边界条件
#[test]
fn fuzz_mmu_edge_cases() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);
    
    // 测试边界地址
    let edge_addresses = [
        0u64,
        1,
        4095,  // 页边界前
        4096,  // 页边界
        4097,  // 页边界后
        1024 * 1024 - 1,  // 内存末尾前
        1024 * 1024,      // 内存末尾
    ];

    for &addr in &edge_addresses {
        // 测试不同大小的访问
        for size in [1, 2, 4, 8] {
            // 如果地址+大小超出范围，应该返回错误而不是panic
            if addr + (size as u64) <= 1024 * 1024 {
                let _ = mmu.write(addr, 0xDEADBEEF, size);
                let _ = mmu.read(addr, size);
            }
        }
    }
}

/// 模糊测试：MMU对齐访问
#[test]
fn fuzz_mmu_alignment() {
    let mut rng = rand::thread_rng();
    let mut mmu = SoftMmu::new(1024 * 1024, false);

    for _ in 0..1000 {
        let base_addr = rng.gen_range(0..(1024 * 1024 - 8));
        let size = [1, 2, 4, 8][rng.gen_range(0..4)];
        
        // 测试对齐和不对齐的访问
        let addr = if rng.gen_bool(0.5) {
            // 对齐地址
            (base_addr / (size as u64)) * (size as u64)
        } else {
            // 不对齐地址
            base_addr
        };

        if addr + (size as u64) <= 1024 * 1024 {
            let value = rng.gen::<u64>();
            let _ = mmu.write(addr, value, size);
            let _ = mmu.read(addr, size);
        }
    }
}

/// 模糊测试：MMU TLB刷新
#[test]
fn fuzz_mmu_tlb_flush() {
    let mut rng = rand::thread_rng();
    let mut mmu = SoftMmu::new(1024 * 1024 * 1024, true);

    for _ in 0..100 {
        // 执行一些内存操作
        for _ in 0..100 {
            let addr = rng.gen_range(0..(1024 * 1024 * 1024));
            let _ = mmu.translate(addr, AccessType::Read);
        }
        
        // 随机刷新TLB
        if rng.gen_bool(0.1) {
            mmu.flush_tlb();
        }
    }
}

/// 模糊测试：MMU并发访问
#[test]
fn fuzz_mmu_concurrent_access() {
    use std::sync::Arc;
    use std::thread;
    
    let mmu = Arc::new(std::sync::Mutex::new(SoftMmu::new(1024 * 1024, false)));
    let mut handles = Vec::new();

    for i in 0..10 {
        let mmu_clone = Arc::clone(&mmu);
        let handle = thread::spawn(move || {
            let mut rng = rand::thread_rng();
            for _ in 0..1000 {
                let mut mmu_guard = mmu_clone.lock().unwrap();
                let addr = rng.gen_range(0..(1024 * 1024 - 8));
                let size = [1, 2, 4, 8][rng.gen_range(0..4)];
                
                match rng.gen_range(0..2) {
                    0 => {
                        let _ = mmu_guard.write(addr, rng.gen(), size);
                    }
                    _ => {
                        let _ = mmu_guard.read(addr, size);
                    }
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

/// 模糊测试：MMU大块内存操作
#[test]
fn fuzz_mmu_large_blocks() {
    let mut rng = rand::thread_rng();
    let mut mmu = SoftMmu::new(1024 * 1024 * 1024, false); // 1GB

    for _ in 0..100 {
        let addr = rng.gen_range(0..(1024 * 1024 * 1024 - 1024 * 1024));
        let size = rng.gen_range(1024..(1024 * 1024)); // 1KB到1MB
        
        let data: Vec<u8> = (0..size).map(|_| rng.gen()).collect();
        let _ = mmu.write_bulk(addr, &data);
        
        let mut buf = vec![0u8; size];
        let _ = mmu.read_bulk(addr, &mut buf);
    }
}

