//! SIMD memcpy 使用示例和集成指南
//!
//! 本示例展示如何在VM项目中使用SIMD优化的内存复制操作

use vm_mem::simd_memcpy::{memcpy_fast, memcpy_adaptive, memcpy_adaptive_with_threshold};

/// 示例1: 基础使用 - memcpy_fast
///
/// 这是最简单的使用方式，适合大多数场景
#[cfg(test)]
mod example1_basic_usage {
    use super::*;

    #[test]
    fn test_memcpy_fast() {
        let src = vec![42u8; 1024];
        let mut dst = vec![0u8; 1024];

        // 使用SIMD优化的memcpy
        memcpy_fast(&mut dst, &src);

        assert_eq!(dst, src);
        println!("✓ memcpy_fast 成功复制 {} 字节", dst.len());
    }

    #[test]
    fn test_memcpy_fast_different_sizes() {
        // 测试不同大小的复制
        for size in [64, 256, 1024, 4096, 16384].iter() {
            let src = vec![99u8; *size];
            let mut dst = vec![0u8; *size];

            memcpy_fast(&mut dst, &src);

            assert_eq!(dst, src);
            println!("✓ 成功复制 {} 字节", size);
        }
    }
}

/// 示例2: 自适应memcpy - memcpy_adaptive
///
/// 自动选择最优的复制策略
#[cfg(test)]
mod example2_adaptive {
    use super::*;

    #[test]
    fn test_memcpy_adaptive() {
        let src = vec![123u8; 2048];
        let mut dst = vec![0u8; 2048];

        // 自适应选择最佳SIMD路径
        memcpy_adaptive(&mut dst, &src);

        assert_eq!(dst, src);
        println!("✓ memcpy_adaptive 成功复制 {} 字节", dst.len());
    }

    #[test]
    fn test_memcpy_adaptive_vs_fast() {
        // 对比两种API的结果
        let src = vec![55u8; 5000];
        let mut dst1 = vec![0u8; 5000];
        let mut dst2 = vec![0u8; 5000];

        memcpy_fast(&mut dst1, &src);
        memcpy_adaptive(&mut dst2, &src);

        assert_eq!(dst1, src);
        assert_eq!(dst2, src);
        assert_eq!(dst1, dst2);
        println!("✓ 两种API结果一致");
    }
}

/// 示例3: 自定义阈值 - memcpy_adaptive_with_threshold
///
/// 根据实际工作负载调整阈值
#[cfg(test)]
mod example3_custom_threshold {
    use super::*;

    #[test]
    fn test_small_threshold() {
        // 小阈值: 更激进地使用SIMD
        let src = vec![77u8; 256];
        let mut dst = vec![0u8; 256];

        // 阈值设为128字节
        memcpy_adaptive_with_threshold(&mut dst, &src, 128);

        assert_eq!(dst, src);
        println!("✓ 小阈值 (128B) 适合频繁的小块复制");
    }

    #[test]
    fn test_large_threshold() {
        // 大阈值: 只对大块使用SIMD
        let src = vec![88u8; 4096];
        let mut dst = vec![0u8; 4096];

        // 阈值设为2048字节
        memcpy_adaptive_with_threshold(&mut dst, &src, 2048);

        assert_eq!(dst, src);
        println!("✓ 大阈值 (2KB) 减少小块的SIMD开销");
    }

    #[test]
    fn test_threshold_selection() {
        // 阈值选择建议:
        // - 小数据 (频繁): 64-256 字节
        // - 混合负载: 512-1024 字节 (默认)
        // - 大数据为主: 2048-4096 字节

        let sizes = [128, 512, 1024, 2048, 4096];
        let thresholds = [64, 256, 1024, 2048];

        for &threshold in thresholds.iter() {
            for &size in sizes.iter() {
                let src = vec![size as u8; size];
                let mut dst = vec![0u8; size];

                memcpy_adaptive_with_threshold(&mut dst, &src, threshold);

                assert_eq!(dst, src);
            }
            println!("✓ 阈值 {} 字节: 所有测试通过", threshold);
        }
    }
}

/// 示例4: 性能对比
///
/// 展示SIMD相对于标准库的性能优势
#[cfg(test)]
mod example4_performance {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_performance_comparison() {
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
        }
    }

    #[test]
    fn test_different_size_performance() {
        let sizes = [256, 1024, 4096, 16384, 65536];

        for size in sizes.iter() {
            let src = vec![99u8; *size];
            let mut dst = vec![0u8; *size];

            // SIMD复制
            let start = Instant::now();
            memcpy_fast(&mut dst, &src);
            let duration = start.elapsed();

            println!("大小: {:6} 字节, 耗时: {:8.3?}", size, duration);
        }
    }
}

// ============================================================================
// 集成指南
// ============================================================================
//
// # 如何在VM项目中集成SIMD memcpy
//
// ## 1. 内存管理集成
//
// 在 vm-mem 的内存操作中使用:
/// ```rust
/// use vm_mem::simd_memcpy::memcpy_fast;
///
/// pub struct MemoryManager {
///     // ...
/// }
///
/// impl MemoryManager {
///     pub fn copy_memory(&mut self, dst: usize, src: usize, size: usize) {
///         unsafe {
///             let dst_ptr = self.get_mut_ptr(dst);
///             let src_ptr = self.get_ptr(src);
///
///             // 方法1: 使用安全包装器
///             let dst_slice = std::slice::from_raw_parts_mut(dst_ptr, size);
///             let src_slice = std::slice::from_raw_parts(src_ptr, size);
///             memcpy_fast(dst_slice, src_slice);
///
///             // 方法2: 使用raw函数 (需要确保不重叠)
///             // vm_mem::simd_memcpy::memcpy_raw(dst_ptr, src_ptr, size);
///         }
///     }
/// }
/// ```
///
/// ## 2. JIT编译器集成
///
/// 在代码生成中使用SIMD memcpy:
/// ```rust
/// use vm_mem::simd_memcpy::memcpy_fast;
///
/// pub fn generate_memcpy_code(dst: usize, src: usize, size: usize) {
///     // 对于小size,生成内联复制指令
///     if size < 128 {
///         // 生成mov指令
///     } else {
///         // 对于大size,调用SIMD memcpy
///         call_simd_memcpy(dst, src, size);
///     }
/// }
/// ```
///
/// ## 3. 翻译管道集成
///
/// 在跨架构翻译中使用:
/// ```rust
/// use vm_mem::simd_memcpy::memcpy_adaptive;
///
/// pub fn translate_memory_op(operation: &MemoryOperation) -> Result<()> {
///     match operation {
///         MemoryOperation::Copy { dst, src, size } => {
///             // 使用自适应SIMD复制
///             memcpy_adaptive(dst, src);
///             Ok(())
///         }
///         // ...
///     }
/// }
/// ```
///
/// ## 4. 设备仿真集成
///
/// 在设备DMA操作中使用:
/// ```rust
/// use vm_mem::simd_memcpy::memcpy_fast;
///
/// pub struct DmaController {
///     // ...
/// }
///
/// impl DmaController {
///     pub fn do_dma_transfer(&mut self, dst: &[u8], src: &mut [u8]) {
///         // DMA传输使用SIMD优化
///         memcpy_fast(src, dst);
///     }
/// }
/// ```
///
/// # 最佳实践
///
/// ## 选择合适的API
///
/// - **memcpy_fast**: 适合大多数场景，推荐使用
/// - **memcpy_adaptive**: 需要自动优化时使用
/// - **memcpy_adaptive_with_threshold**: 需要精细控制时使用
///
/// ## 阈值选择
///
/// - **64-256字节**: 频繁的小块复制
/// - **512-1024字节**: 默认，混合负载
/// - **2048-4096字节**: 大数据为主
///
/// ## 性能考虑
///
/// - SIMD对大块复制 (>1KB) 优势明显
/// - 小块复制 (<64字节) 标准库可能更快
/// - 使用自适应API可以自动选择最佳策略
///
/// # 预期性能提升
///
/// | CPU架构 | SIMD指令 | 提升倍数 |
/// |---------|---------|---------|
/// | x86_64 (AVX-512) | 512-bit | **8-10x** |
/// | x86_64 (AVX2) | 256-bit | **5-7x** |
/// | ARM64 (NEON) | 128-bit | **4-6x** |
/// | 其他架构 | Fallback | **1x** (标准库) |
///
/// # 注意事项
///
/// 1. **内存重叠**: SIMD memcpy假设内存不重叠
/// 2. **对齐**: 自然对齐的内存性能更好
/// 3. **大小**: 性能提升与数据块大小相关
/// 4. **兼容性**: 所有架构都有fallback实现
///
/// # 示例代码
///
/// See `simd_memcpy_example.rs` for complete usage examples.
