//! SIMD功能快速验证
//!
//! 独立程序，验证SIMD memcpy功能
//!
//! 运行:
//! ```bash
//! cargo run --bin simd_quick_verify --package vm-mem
//! ```

use std::time::Instant;
use vm_mem::simd_memcpy::{memcpy_fast, simd_feature_name};

fn main() {
    println!("=== SIMD优化功能验证 ===\n");

    // 1. 特性检测
    println!("1. SIMD特性检测");
    println!("   Active SIMD feature: {}\n", simd_feature_name());

    // 2. 基础功能测试
    println!("2. 基础功能测试");

    let size = 1024;
    let src: Vec<u8> = (0..size).map(|i| i as u8).collect();
    let mut dst = vec![0u8; size];

    memcpy_fast(&mut dst, &src);

    if dst == src {
        println!("   ✅ 基础拷贝测试通过 ({} bytes)\n", size);
    } else {
        println!("   ❌ 基础拷贝测试失败\n");
        return;
    }

    // 3. 对齐拷贝测试
    println!("3. 对齐拷贝测试");

    let aligned_sizes = [16, 32, 64, 128, 256, 512, 1024];
    let mut aligned_passed = 0;

    for size in aligned_sizes.iter() {
        let src: Vec<u8> = (0..*size).map(|i| i as u8).collect();
        let mut dst = vec![0u8; *size];

        memcpy_fast(&mut dst, &src);

        if dst == src {
            aligned_passed += 1;
        }
    }

    println!("   ✅ 对齐拷贝: {}/{} 测试通过\n", aligned_passed, aligned_sizes.len());

    // 4. 未对齐拷贝测试
    println!("4. 未对齐拷贝测试");

    let size = 1024;
    let src_size = size + 16;
    let src: Vec<u8> = (0..src_size).map(|i| i as u8).collect();
    let offsets = [1, 3, 5, 7, 9];
    let mut unaligned_passed = 0;

    for offset in offsets.iter() {
        let mut dst = vec![0u8; size];
        let src_slice = &src[*offset..*offset + size];

        memcpy_fast(&mut dst, src_slice);

        let expected: Vec<u8> = (*offset..*offset + size).map(|i| i as u8).collect();
        if dst == expected {
            unaligned_passed += 1;
        }
    }

    println!("   ✅ 未对齐拷贝: {}/{} 测试通过\n", unaligned_passed, offsets.len());

    // 5. 性能测试
    println!("5. 性能特征测试");

    let test_sizes = [
        (64, "小数据"),
        (1024, "中等数据"),
        (16384, "大数据"),
        (65536, "大数据+"),
    ];

    println!("   数据大小    | 迭代次数 | 总时间   | 吞吐量");
    println!("   -----------|----------|----------|-----------");

    for (size, _label) in test_sizes.iter() {
        let src: Vec<u8> = vec![42u8; *size];
        let mut dst = vec![0u8; *size];

        let iterations = if *size < 1000 { 10000 } else { 1000 };

        let start = Instant::now();
        for _ in 0..iterations {
            memcpy_fast(&mut dst, &src);
        }
        let duration = start.elapsed();

        let total_bytes = (*size * iterations) as f64;
        let throughput_mb = total_bytes / duration.as_secs_f64() / (1024.0 * 1024.0);

        println!("   {:9}  | {:8} | {:8.3}ms | {:8.2} MB/s",
                 size, iterations, duration.as_secs_f64() * 1000.0, throughput_mb);
    }

    println!();

    // 6. 总结
    println!("=== 测试总结 ===");
    println!("✅ SIMD特性检测: 通过");
    println!("✅ 基础功能测试: 通过");
    println!("✅ 对齐拷贝测试: {}/{} 通过", aligned_passed, aligned_sizes.len());
    println!("✅ 未对齐拷贝测试: {}/{} 通过", unaligned_passed, offsets.len());
    println!("✅ 性能测试: 完成");

    if aligned_passed == aligned_sizes.len() && unaligned_passed == offsets.len() {
        println!("\n🎉 所有SIMD功能测试通过！");
        println!("SIMD优化工作正常，可以投入使用。");
    } else {
        println!("\n⚠️  部分测试失败，请检查实现。");
    }
}
