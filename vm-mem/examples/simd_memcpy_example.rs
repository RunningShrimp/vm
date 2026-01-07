//! SIMD memcpy 使用示例
//!
//! 展示如何在VM项目中使用SIMD优化的内存复制操作

use vm_mem::simd_memcpy::{memcpy_adaptive, memcpy_adaptive_with_threshold, memcpy_fast};

fn main() {
    println!("VM SIMD memcpy 使用示例\n");

    // 示例1: 基础使用
    example1_basic();

    // 示例2: 不同大小
    example2_different_sizes();

    // 示例3: 性能对比
    example3_performance();

    println!("\n所有示例运行完成！");
}

fn example1_basic() {
    println!("=== 示例1: 基础使用 ===");

    let src = vec![42u8; 1024];
    let mut dst = vec![0u8; 1024];

    // 使用SIMD优化的memcpy
    memcpy_fast(&mut dst, &src);

    assert_eq!(dst, src);
    println!("✓ 成功复制 1024 字节");
}

fn example2_different_sizes() {
    println!("\n=== 示例2: 不同大小复制 ===");

    let sizes = [64, 256, 1024, 4096];

    for &size in sizes.iter() {
        let src = vec![size as u8; size];
        let mut dst = vec![0u8; size];

        memcpy_fast(&mut dst, &src);

        assert_eq!(dst, src);
        println!("✓ 成功复制 {:6} 字节", size);
    }
}

fn example3_performance() {
    println!("\n=== 示例3: 性能对比 ===");

    use std::time::Instant;

    let size = 1024 * 1024; // 1MB
    let iterations = 100;

    let src = vec![42u8; size];
    let mut dst = vec![0u8; size];

    // 测试标准库复制
    let start = Instant::now();
    for _ in 0..iterations {
        dst.copy_from_slice(&src);
    }
    let std_time = start.elapsed();

    // 测试SIMD复制
    let start = Instant::now();
    for _ in 0..iterations {
        memcpy_fast(&mut dst, &src);
    }
    let simd_time = start.elapsed();

    println!("标准库: {:?}", std_time);
    println!("SIMD:   {:?}", simd_time);

    if simd_time < std_time {
        let speedup = std_time.as_nanos() as f64 / simd_time.as_nanos() as f64;
        println!("✓ SIMD快 {:.2}倍", speedup);
    } else {
        println!("ℹ️  在此测试大小下，SIMD未显示优势（正常现象）");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memcpy_fast() {
        let src = vec![99u8; 2048];
        let mut dst = vec![0u8; 2048];

        memcpy_fast(&mut dst, &src);
        assert_eq!(dst, src);
    }

    #[test]
    fn test_memcpy_adaptive() {
        let src = vec![55u8; 4096];
        let mut dst = vec![0u8; 4096];

        memcpy_adaptive(&mut dst, &src);
        assert_eq!(dst, src);
    }

    #[test]
    fn test_memcpy_with_threshold() {
        let src = vec![77u8; 1024];
        let mut dst = vec![0u8; 1024];

        memcpy_adaptive_with_threshold(&mut dst, &src, 256);
        assert_eq!(dst, src);
    }
}
