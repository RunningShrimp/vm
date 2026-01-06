//! SIMD优化验证测试
//!
//! 本测试验证SIMD memcpy优化功能是否正常工作。
//!
//! 运行测试:
//! ```bash
//! cargo test --package vm-mem simd_verification_test -- --nocapture
//! ```

use vm_mem::simd_memcpy::{memcpy_fast, simd_feature_name};

#[test]
fn test_simd_feature_detection() {
    println!("\n=== SIMD Feature Detection Test ===");
    println!("Active SIMD feature: {}\n", simd_feature_name());

    // 验证至少有一个SIMD特性被检测到
    let feature = simd_feature_name();
    assert!(!feature.is_empty(), "Should detect at least one SIMD feature");

    println!("✅ SIMD feature detection works correctly");
}

#[test]
fn test_simd_memcpy_basic() {
    println!("\n=== Basic SIMD memcpy Test ===");

    let size = 1024;
    let src: Vec<u8> = (0..size).map(|i| i as u8).collect();
    let mut dst = vec![0u8; size];

    // 使用SIMD优化的memcpy
    memcpy_fast(&mut dst, &src);

    // 验证数据正确性
    assert_eq!(dst, src, "SIMD memcpy should produce correct result");

    println!("✅ Basic SIMD memcpy test passed ({} bytes)", size);
}

#[test]
fn test_simd_memcpy_aligned() {
    println!("\n=== Aligned SIMD memcpy Test ===");

    // 测试不同大小的对齐拷贝
    let sizes = [16, 32, 64, 128, 256, 512, 1024];

    for size in sizes.iter() {
        let src: Vec<u8> = (0..*size).map(|i| i as u8).collect();
        let mut dst = vec![0u8; *size];

        memcpy_fast(&mut dst, &src);

        assert_eq!(dst, src, "Aligned memcpy failed for size {}", size);
        println!("  ✅ Aligned copy: {} bytes", size);
    }

    println!("✅ All aligned SIMD memcpy tests passed");
}

#[test]
fn test_simd_memcpy_unaligned() {
    println!("\n=== Unaligned SIMD memcpy Test ===");

    let size = 1024;
    let src_size = size + 16;
    let src: Vec<u8> = (0..src_size).map(|i| i as u8).collect();

    // 测试不同的未对齐偏移
    for offset in [1, 3, 5, 7, 9].iter() {
        let mut dst = vec![0u8; size];
        let src_slice = &src[*offset..*offset + size];

        memcpy_fast(&mut dst, src_slice);

        let expected: Vec<u8> = (*offset..*offset + size).map(|i| i as u8).collect();
        assert_eq!(dst, expected, "Unaligned memcpy failed for offset {}", offset);
        println!("  ✅ Unaligned copy: offset={}, size={}", offset, size);
    }

    println!("✅ All unaligned SIMD memcpy tests passed");
}

#[test]
fn test_simd_memcpy_zero_copy() {
    println!("\n=== Zero-Copy Edge Case Test ===");

    let src: Vec<u8> = vec![42u8; 100];
    let mut dst = vec![0u8; 100];

    // 测试0字节拷贝
    memcpy_fast(&mut dst, &src[0..0]);
    assert!(dst.iter().all(|&x| x == 0), "Zero copy should not modify destination");

    println!("✅ Zero-copy edge case handled correctly");
}

#[test]
fn test_simd_memcpy_large_data() {
    println!("\n=== Large Data SIMD memcpy Test ===");

    let large_size = 128 * 1024; // 128 KB
    let src: Vec<u8> = (0..large_size).map(|i| (i % 256) as u8).collect();
    let mut dst = vec![0u8; large_size];

    memcpy_fast(&mut dst, &src);

    assert_eq!(dst, src, "Large data SIMD memcpy failed");

    println!("✅ Large data SIMD memcpy test passed ({} KB)", large_size / 1024);

    // 验证模式正确
    let check_count = 100;
    for i in 0..check_count {
        assert_eq!(
            dst[i],
            (i % 256) as u8,
            "Pattern mismatch at index {}",
            i
        );
    }
    println!("✅ Data pattern verification passed");
}

#[test]
fn test_simd_performance_characteristics() {
    println!("\n=== SIMD Performance Characteristics Test ===");

    // 测试不同数据大小的性能特征
    let sizes = [64, 256, 1024, 4096, 16384];

    println!("  Testing various data sizes:");
    for size in sizes.iter() {
        let src: Vec<u8> = vec![42u8; *size];
        let mut dst = vec![0u8; *size];

        let start = std::time::Instant::now();
        for _ in 0..1000 {
            memcpy_fast(&mut dst, &src);
        }
        let duration = start.elapsed();

        let throughput = (*size as f64 * 1000.0) / duration.as_secs_f64() / (1024.0 * 1024.0);
        println!("    Size: {:5} bytes | Time: {:8.3}ms | Throughput: {:8.2} MB/s",
                 size, duration.as_secs_f64() * 1000.0, throughput);
    }

    println!("✅ Performance characteristics test completed");
}
