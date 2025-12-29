//! SIMD Tests
//!
//! Tests for platform-specific SIMD functions

#[cfg(target_arch = "x86_64")]
mod x86_64_tests {
    /// Test AVX2 SIMD addition
    #[test]
    fn test_avx2_add_i32x8() {
        let a = [1, 2, 3, 4, 5, 6, 7, 8];
        let b = [10, 20, 30, 40, 50, 60, 70, 80];

        let result = vm_accel::add_i32x8(a, b);

        assert_eq!(result, [11, 22, 33, 44, 55, 66, 77, 88]);
        println!("AVX2 addition test passed");
    }

    /// Test AVX2 with zeros
    #[test]
    fn test_avx2_add_zeros() {
        let a = [0, 0, 0, 0, 0, 0, 0, 0];
        let b = [1, 2, 3, 4, 5, 6, 7, 8];

        let result = vm_accel::add_i32x8(a, b);

        assert_eq!(result, [1, 2, 3, 4, 5, 6, 7, 8]);
        println!("AVX2 zero addition test passed");
    }

    /// Test AVX2 with negative numbers
    #[test]
    fn test_avx2_add_negative() {
        let a = [-1, -2, -3, -4, -5, -6, -7, -8];
        let b = [1, 2, 3, 4, 5, 6, 7, 8];

        let result = vm_accel::add_i32x8(a, b);

        assert_eq!(result, [0, 0, 0, 0, 0, 0, 0, 0]);
        println!("AVX2 negative addition test passed");
    }

    /// Test AVX2 with large numbers
    #[test]
    fn test_avx2_add_large() {
        let a = [1_000_000, 2_000_000, 3_000_000, 4_000_000, 5_000_000, 6_000_000, 7_000_000, 8_000_000];
        let b = [1, 2, 3, 4, 5, 6, 7, 8];

        let result = vm_accel::add_i32x8(a, b);

        assert_eq!(result, [1_000_001, 2_000_002, 3_000_003, 4_000_004, 5_000_005, 6_000_006, 7_000_007, 8_000_008]);
        println!("AVX2 large number addition test passed");
    }

    /// Test AVX2 fallback path
    #[test]
    fn test_avx2_fallback_path() {
        // This test always runs, but uses AVX2 if available
        let a = [1, 1, 1, 1, 1, 1, 1, 1];
        let b = [2, 2, 2, 2, 2, 2, 2, 2];

        let result = vm_accel::add_i32x8(a, b);

        assert_eq!(result, [3, 3, 3, 3, 3, 3, 3, 3]);

        if std::is_x86_feature_detected!("avx2") {
            println!("Using AVX2 path");
        } else {
            println!("Using fallback path");
        }
    }

    /// Test CPU feature detection at runtime
    #[test]
    fn test_x86_feature_detection() {
        if std::is_x86_feature_detected!("avx2") {
            println!("AVX2 is available");
        } else {
            println!("AVX2 is not available");
        }

        if std::is_x86_feature_detected!("avx512f") {
            println!("AVX-512 is available");
        } else {
            println!("AVX-512 is not available");
        }
    }
}

#[cfg(target_arch = "aarch64")]
mod aarch64_tests {
    /// Test NEON SIMD addition
    #[test]
    fn test_neon_add_i32x4() {
        let a = [1, 2, 3, 4];
        let b = [10, 20, 30, 40];

        let result = vm_accel::add_i32x4(a, b);

        assert_eq!(result, [11, 22, 33, 44]);
        println!("NEON addition test passed");
    }

    /// Test NEON with zeros
    #[test]
    fn test_neon_add_zeros() {
        let a = [0, 0, 0, 0];
        let b = [1, 2, 3, 4];

        let result = vm_accel::add_i32x4(a, b);

        assert_eq!(result, [1, 2, 3, 4]);
        println!("NEON zero addition test passed");
    }

    /// Test NEON with negative numbers
    #[test]
    fn test_neon_add_negative() {
        let a = [-1, -2, -3, -4];
        let b = [1, 2, 3, 4];

        let result = vm_accel::add_i32x4(a, b);

        assert_eq!(result, [0, 0, 0, 0]);
        println!("NEON negative addition test passed");
    }

    /// Test NEON with large numbers
    #[test]
    fn test_neon_add_large() {
        let a = [1_000_000, 2_000_000, 3_000_000, 4_000_000];
        let b = [1, 2, 3, 4];

        let result = vm_accel::add_i32x4(a, b);

        assert_eq!(result, [1_000_001, 2_000_002, 3_000_003, 4_000_004]);
        println!("NEON large number addition test passed");
    }

    /// Test NEON is always available on ARM64
    #[test]
    fn test_neon_always_available() {
        let a = [1, 2, 3, 4];
        let b = [5, 6, 7, 8];

        let result = vm_accel::add_i32x4(a, b);

        assert_eq!(result, [6, 8, 10, 12]);
        println!("NEON is always available on ARM64");
    }
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
mod other_tests {
    /// Test for other architectures
    #[test]
    fn test_unsupported_arch() {
        println!("SIMD tests not implemented for this architecture");
    }
}
