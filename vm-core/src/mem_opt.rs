//! Memory optimization utilities
//!
//! Provides optimized memory copy operations based on Round 2/3 benchmark results.

/// Adaptive memory copy that automatically selects the optimal strategy
///
/// Based on Round 2 benchmark results, this function chooses:
/// - **Small data (<4KB)**: Use standard library copy (optimized for small data)
/// - **Large data (≥4KB)**: Use standard library copy (already optimal)
///
/// # Performance Characteristics
/// - **<1KB**: Standard library is well-optimized
/// - **1-4KB**: Standard library with CPU-specific optimizations
/// - **≥4KB**: Standard library uses SIMD automatically
///
/// The key insight from Round 2 benchmarks:
/// - Explicit SIMD helps for small data (<4KB): +5-14%
/// - But for large data, standard library's memcpy is already optimal
///
/// Since vm-core cannot depend on vm-mem (circular dependency),
/// we use the standard library's copy_from_slice which is already
/// highly optimized and uses SIMD internally when beneficial.
///
/// # Example
/// ```rust
/// use vm_core::mem_opt::memcpy_adaptive;
///
/// let mut dst = vec![0u8; 2048];
/// let src = vec![42u8; 2048];
///
/// memcpy_adaptive(&mut dst, &src);
/// assert_eq!(dst, src);
/// ```
pub fn memcpy_adaptive(dst: &mut [u8], src: &[u8]) {
    assert_eq!(
        dst.len(),
        src.len(),
        "Destination and source slices must have the same length"
    );

    // For now, use the standard library copy which is already highly optimized
    // The standard library's copy_from_slice uses:
    // - SIMD instructions when available (AVX-512, AVX2, SSE2, NEON)
    // - Size-specific optimizations
    // - Platform-specific implementations
    //
    // Future optimization: If benchmarks show specific vm-core workloads
    // can benefit from custom SIMD, we can add target-specific implementations.
    dst.copy_from_slice(src);
}

/// Adaptive memory copy with custom threshold
///
/// Same as [`memcpy_adaptive`] but allows custom threshold configuration.
/// Currently delegates to standard library for optimal performance.
pub fn memcpy_adaptive_with_threshold(dst: &mut [u8], src: &[u8], _threshold: usize) {
    assert_eq!(
        dst.len(),
        src.len(),
        "Destination and source slices must have the same length"
    );

    // Delegate to standard library
    dst.copy_from_slice(src);
}

/// Get the default adaptive threshold (4096 bytes / 4KB)
///
/// This threshold is based on Round 2 benchmark results from vm-mem.
pub const fn adaptive_threshold() -> usize {
    4096
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memcpy_adaptive_small() {
        let src = vec![42u8; 2048]; // 2KB
        let mut dst = vec![0u8; 2048];

        memcpy_adaptive(&mut dst, &src);
        assert_eq!(dst, src);
    }

    #[test]
    fn test_memcpy_adaptive_large() {
        let src = vec![42u8; 8192]; // 8KB
        let mut dst = vec![0u8; 8192];

        memcpy_adaptive(&mut dst, &src);
        assert_eq!(dst, src);
    }

    #[test]
    fn test_memcpy_adaptive_threshold_boundary() {
        // Test exactly at threshold (4KB)
        let src = vec![42u8; 4096];
        let mut dst = vec![0u8; 4096];

        memcpy_adaptive(&mut dst, &src);
        assert_eq!(dst, src);

        // Test just below threshold
        let src = vec![42u8; 4095];
        let mut dst = vec![0u8; 4095];

        memcpy_adaptive(&mut dst, &src);
        assert_eq!(dst, src);

        // Test just above threshold
        let src = vec![42u8; 4097];
        let mut dst = vec![0u8; 4097];

        memcpy_adaptive(&mut dst, &src);
        assert_eq!(dst, src);
    }

    #[test]
    fn test_memcpy_adaptive_empty() {
        let src: Vec<u8> = vec![];
        let mut dst: Vec<u8> = vec![];

        memcpy_adaptive(&mut dst, &src);
        assert_eq!(dst, src);
    }

    #[test]
    fn test_memcpy_adaptive_with_custom_threshold() {
        let src = vec![42u8; 8192]; // 8KB
        let mut dst = vec![0u8; 8192];

        // Use 16KB threshold
        memcpy_adaptive_with_threshold(&mut dst, &src, 16384);
        assert_eq!(dst, src);

        // Use 4KB threshold
        memcpy_adaptive_with_threshold(&mut dst, &src, 4096);
        assert_eq!(dst, src);
    }

    #[test]
    fn test_adaptive_threshold_constant() {
        assert_eq!(adaptive_threshold(), 4096);
    }

    #[test]
    fn test_memcpy_adaptive_various_sizes() {
        for size in [0, 1, 1024, 2048, 4096, 8192, 16384].iter() {
            let src = vec![(*size % 256) as u8; *size];
            let mut dst = vec![0u8; *size];

            memcpy_adaptive(&mut dst, &src);
            assert_eq!(dst, src, "Failed for size {}", size);
        }
    }

    #[test]
    #[should_panic(expected = "Destination and source slices must have the same length")]
    fn test_length_mismatch() {
        let src = vec![1u8, 2, 3];
        let mut dst = vec![0u8; 2];
        memcpy_adaptive(&mut dst, &src);
    }
}
