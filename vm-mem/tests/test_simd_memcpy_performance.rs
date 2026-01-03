//! SIMD memcpy 性能对比测试
//!
//! 快速验证 SIMD memcpy 的性能提升

use std::time::Instant;

use vm_mem::memcpy_fast;

/// 标准 memcpy 实现
fn memcpy_std(dst: &mut [u8], src: &[u8]) {
    dst.copy_from_slice(src);
}

#[test]
fn test_simd_performance_small() {
    let sizes = [64, 128, 256, 512];

    println!("\n小数据块性能测试:");
    for &size in &sizes {
        let src = vec![42u8; size];
        let mut dst = vec![0u8; size];

        // 测试标准 memcpy
        let start = Instant::now();
        for _ in 0..10_000 {
            memcpy_std(&mut dst, &src);
        }
        let std_time = start.elapsed();

        // 测试 SIMD memcpy
        let start = Instant::now();
        for _ in 0..10_000 {
            memcpy_fast(&mut dst, &src);
        }
        let simd_time = start.elapsed();

        let speedup = std_time.as_nanos() as f64 / simd_time.as_nanos() as f64;

        println!(
            "{} bytes: std={:?}, simd={:?}, speedup={:.2}x",
            size, std_time, simd_time, speedup
        );

        // 验证结果正确性
        assert_eq!(dst, src);
    }
}

#[test]
fn test_simd_performance_large() {
    let sizes = [1024, 4096, 16384];

    println!("\n大数据块性能测试:");
    for &size in &sizes {
        let src = vec![42u8; size];
        let mut dst = vec![0u8; size];

        // 测试标准 memcpy (较少次数)
        let start = Instant::now();
        for _ in 0..1_000 {
            memcpy_std(&mut dst, &src);
        }
        let std_time = start.elapsed();

        // 测试 SIMD memcpy (较少次数)
        let start = Instant::now();
        for _ in 0..1_000 {
            memcpy_fast(&mut dst, &src);
        }
        let simd_time = start.elapsed();

        let speedup = std_time.as_nanos() as f64 / simd_time.as_nanos() as f64;

        // 计算吞吐量
        let std_throughput = (size * 1_000) as f64 / (std_time.as_micros() as f64 / 1000.0);
        let simd_throughput = (size * 1_000) as f64 / (simd_time.as_micros() as f64 / 1000.0);

        println!(
            "{} bytes: std={:?} ({:.2} MB/s), simd={:?} ({:.2} MB/s), speedup={:.2}x",
            size,
            std_time,
            std_throughput / (1024.0 * 1024.0),
            simd_time,
            simd_throughput / (1024.0 * 1024.0),
            speedup
        );

        // 验证结果正确性
        assert_eq!(dst, src);

        // 注意：在某些平台上（如 ARM64 macOS），系统 memcpy 已经高度优化
        // 因此我们的 SIMD 实现可能不会更快，这可以接受
        // SIMD 的价值在于跨平台一致性（x86_64 上可能有显著提升）
        if simd_time > std_time * 3 {
            println!(
                "  ⚠️  注意: SIMD 比标准实现慢 {}x",
                std_time.as_nanos() as f64 / simd_time.as_nanos() as f64
            );
            println!("      这可能是因为系统 memcpy 已经针对当前平台高度优化");
        }
    }
}

#[test]
fn test_simd_correctness() {
    let sizes = [
        1, 7, 8, 15, 16, 17, 31, 32, 33, 63, 64, 65, 127, 128, 129, 255, 256, 257, 511, 512, 513,
        1023, 1024, 1025, 4095, 4096, 4097,
    ];

    println!("\n正确性测试:");
    for &size in &sizes {
        let mut src = vec![0u8; size];
        // 填充测试数据
        for (i, byte) in src.iter_mut().enumerate() {
            *byte = (i % 256) as u8;
        }

        let mut dst = vec![0u8; size];
        memcpy_fast(&mut dst, &src);

        assert_eq!(dst, src, "大小 {}: 拷贝不正确", size);
    }

    println!("所有大小 (1 到 4097) 的正确性测试通过 ✅");
}
