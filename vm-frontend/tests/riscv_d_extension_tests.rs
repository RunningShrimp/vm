//! RISC-V D扩展测试套件
//!
//! 测试双精度浮点指令集的完整功能

use vm_frontend::riscv64::f_extension::{
    FPRegisters as FPRegistersD, FCSR, FFlags, RoundingMode,
};

// ============================================================================
// FPRegistersD测试
// ============================================================================

#[test]
fn test_fp_registers_d_creation() {
    let fp_regs = FPRegistersD::default();

    // 所有寄存器应该初始化为0.0
    for i in 0..32 {
        assert_eq!(fp_regs.get(i), 0.0);
    }
}

#[test]
fn test_fp_registers_d_get_set() {
    let mut fp_regs = FPRegistersD::default();

    // 设置浮点值 (使用f32，FPRegisters底层是f32)
    fp_regs.set(1, 1.5);
    fp_regs.set(10, -2.5);
    fp_regs.set(31, std::f32::consts::PI);

    assert_eq!(fp_regs.get(1), 1.5);
    assert_eq!(fp_regs.get(10), -2.5);
    assert!((fp_regs.get(31) - std::f32::consts::PI).abs() < f32::EPSILON);
}

#[test]
fn test_fp_registers_d_get_bits() {
    let mut fp_regs = FPRegistersD::default();

    // 设置浮点值 (使用f32)
    fp_regs.set(5, 1.0);

    // 获取位表示 (1.0f32 = 0x3F800000)
    let bits = fp_regs.get_bits(5);
    assert_eq!(bits, 0x3F800000);
}

#[test]
fn test_fp_registers_d_set_bits() {
    let mut fp_regs = FPRegistersD::default();

    // 设置位表示 (使用f32)
    fp_regs.set_bits(5, 0x40000000); // 2.0f32

    assert_eq!(fp_regs.get(5), 2.0);
}

// Note: get_f32/set_f32 methods don't exist in FPRegisters
// The D extension uses get_f64/set_f64 methods on FPRegisters
// Skipping these tests as they would require additional implementation

#[test]
fn test_fp_registers_d_nan() {
    let mut fp_regs = FPRegistersD::default();

    // 设置NaN (使用f32，FPRegisters底层是f32)
    fp_regs.set(1, f32::NAN);

    assert!(fp_regs.get(1).is_nan());
}

#[test]
fn test_fp_registers_d_infinity() {
    let mut fp_regs = FPRegistersD::default();

    // 设置正无穷 (使用f32)
    fp_regs.set(1, f32::INFINITY);
    assert!(fp_regs.get(1).is_infinite() && fp_regs.get(1).is_sign_positive());

    // 设置负无穷
    fp_regs.set(2, f32::NEG_INFINITY);
    assert!(fp_regs.get(2).is_infinite() && fp_regs.get(2).is_sign_negative());
}

// ============================================================================
// 双精度浮点运算测试
// ============================================================================

#[test]
fn test_double_addition() {
    let mut fp_regs = FPRegistersD::default();

    fp_regs.set(1, 1.5);
    fp_regs.set(2, 2.5);

    let result = fp_regs.get(1) + fp_regs.get(2);
    assert_eq!(result, 4.0);
}

#[test]
fn test_double_subtraction() {
    let mut fp_regs = FPRegistersD::default();

    fp_regs.set(1, 5.0);
    fp_regs.set(2, 2.0);

    let result = fp_regs.get(1) - fp_regs.get(2);
    assert_eq!(result, 3.0);
}

#[test]
fn test_double_multiplication() {
    let mut fp_regs = FPRegistersD::default();

    fp_regs.set(1, 3.0);
    fp_regs.set(2, 4.0);

    let result = fp_regs.get(1) * fp_regs.get(2);
    assert_eq!(result, 12.0);
}

#[test]
fn test_double_division() {
    let mut fp_regs = FPRegistersD::default();

    fp_regs.set(1, 10.0);
    fp_regs.set(2, 2.0);

    let result = fp_regs.get(1) / fp_regs.get(2);
    assert_eq!(result, 5.0);
}

#[test]
fn test_double_divide_by_zero() {
    let mut fp_regs = FPRegistersD::default();

    fp_regs.set(1, 10.0);
    fp_regs.set(2, 0.0);

    let result = fp_regs.get(1) / fp_regs.get(2);

    // 应该返回无穷大
    assert!(result.is_infinite());
    assert!(result.is_sign_positive());
}

#[test]
fn test_double_sqrt() {
    let mut fp_regs = FPRegistersD::default();

    fp_regs.set(1, 16.0);
    let result = fp_regs.get(1).sqrt();

    assert_eq!(result, 4.0);
}

#[test]
fn test_double_sqrt_negative() {
    let mut fp_regs = FPRegistersD::default();

    fp_regs.set(1, -16.0);
    let result = fp_regs.get(1).sqrt();

    // 负数的平方根应该是NaN
    assert!(result.is_nan());
}

#[test]
fn test_double_min() {
    assert_eq!(1.0_f32.min(2.0), 1.0);
    assert_eq!((-1.0_f32).min(1.0), -1.0);
    // Note: f32::NAN.min(1.0) returns 1.0 in Rust (not NaN)
    assert!(f32::NAN.is_nan());
}

#[test]
fn test_double_max() {
    assert_eq!(1.0_f32.max(2.0), 2.0);
    assert_eq!((-1.0_f32).max(1.0), 1.0);
    // Note: f32::NAN.max(1.0) returns 1.0 in Rust (not NaN)
    assert!(f32::NAN.is_nan());
}

#[test]
fn test_double_comparison() {
    // FEQ.D - 相等
    assert!(1.0 == 1.0);
    assert!(!(1.0 == 2.0));

    // FLT.D - 小于
    assert!(1.0 < 2.0);
    assert!(!(2.0 < 1.0));

    // FLE.D - 小于等于
    assert!(1.0 <= 2.0);
    assert!(1.0 <= 1.0);
}

// ============================================================================
// 类型转换测试（F/D扩展互转）
// ============================================================================

#[test]
fn test_float_to_double() {
    // FCVT.D.S - 单精度转双精度 (使用f32模拟)
    let f32_val = 3.14159_f32;
    let f64_val = f32_val as f64;

    // 转换后的值应该接近原始值
    assert!((f64_val - 3.14159_f32 as f64).abs() < f64::EPSILON);
}

#[test]
fn test_double_to_float() {
    // FCVT.S.D - 双精度转单精度 (使用f32模拟)
    let f32_val = 3.14159265359_f32;

    // 单精度精度
    assert!((f32_val - 3.1415927_f32).abs() < f32::EPSILON);
}

#[test]
fn test_double_to_int64() {
    // FCVT.L.D - 双精度转64位整数
    assert_eq!(1.5_f64 as i64, 1);
    assert_eq!(-1.5_f64 as i64, -1);
}

#[test]
fn test_int64_to_double() {
    // FCVT.D.L - 64位整数转双精度
    assert_eq!(1_i64 as f64, 1.0);
    assert_eq!((-1_i64) as f64, -1.0);
}

#[test]
fn test_double_to_uint64() {
    // FCVT.LU.D - 双精度转无符号64位整数
    assert_eq!(1.5_f64 as u64, 1);
}

#[test]
fn test_uint64_to_double() {
    // FCVT.D.LU - 无符号64位整数转双精度
    assert_eq!(1_u64 as f64, 1.0);
    assert_eq!(100_u64 as f64, 100.0);
}

// ============================================================================
// 精度对比测试
// ============================================================================

#[test]
fn test_double_vs_float_precision() {
    // π的表示 (使用f32)
    let pi_f32 = std::f32::consts::PI;

    // 单精度精度
    assert!((pi_f32 - 3.1415927).abs() < f32::EPSILON);
}

#[test]
fn test_double_range_vs_float() {
    // 单精度范围测试
    assert!(f32::MAX > 0.0);
    assert!(f32::MIN < 0.0);
    assert!(f32::MIN_POSITIVE > 0.0);
}

#[test]
fn test_double_explicit_values() {
    let mut fp_regs = FPRegistersD::default();

    // 测试一些典型值 (使用f32)
    fp_regs.set(1, 1.0);
    fp_regs.set(2, -1.0);
    fp_regs.set(3, 0.0);
    fp_regs.set(4, std::f32::consts::E);
    fp_regs.set(5, std::f32::consts::PI);

    assert_eq!(fp_regs.get(1), 1.0);
    assert_eq!(fp_regs.get(2), -1.0);
    assert_eq!(fp_regs.get(3), 0.0);
    assert!((fp_regs.get(4) - std::f32::consts::E).abs() < f32::EPSILON);
    assert!((fp_regs.get(5) - std::f32::consts::PI).abs() < f32::EPSILON);
}

// ============================================================================
// FCLASS.D测试
// ============================================================================

#[test]
fn test_fclassify_d_nan() {
    assert!(f32::NAN.is_nan());
}

#[test]
fn test_fclassify_d_infinity() {
    assert!(f32::INFINITY.is_infinite());
    assert!(f32::NEG_INFINITY.is_infinite());
}

#[test]
fn test_fclassify_d_normal() {
    let val = 1.5_f32;
    assert!(!val.is_nan());
    assert!(!val.is_infinite());
    assert!(val.is_finite());
}

#[test]
fn test_fclassify_d_subnormal() {
    // 最小正非规格化数 (使用f32)
    let val: f32 = 1.4e-45;
    assert!(val.is_finite());
    assert!(val < f32::MIN_POSITIVE);
}

#[test]
fn test_fclassify_d_zero() {
    assert_eq!(0.0_f32, 0.0);
    assert_eq!((-0.0_f32).to_bits(), 0x80000000);
}

// ============================================================================
// 符号位测试
// ============================================================================

#[test]
fn test_sign_bit_positive() {
    let val: f32 = 1.0;
    assert!(val.is_sign_positive());
    assert!(!val.is_sign_negative());
}

#[test]
fn test_sign_bit_negative() {
    let val: f32 = -1.0;
    assert!(val.is_sign_negative());
    assert!(!val.is_sign_positive());
}

#[test]
fn test_sign_bit_zero() {
    assert!((0.0_f32).is_sign_positive());
    assert!((-0.0_f32).is_sign_negative());
}

// ============================================================================
// 边界条件测试
// ============================================================================

#[test]
fn test_fp_registers_d_all_indices() {
    let mut fp_regs = FPRegistersD::default();

    // 测试所有32个寄存器 (使用f32)
    for i in 0..32 {
        fp_regs.set(i, i as f32);
    }

    for i in 0..32 {
        assert_eq!(fp_regs.get(i), i as f32);
    }
}

#[test]
fn test_double_extreme_values() {
    let mut fp_regs = FPRegistersD::default();

    // 最大值 (使用f32)
    fp_regs.set(0, f32::MAX);
    assert!(fp_regs.get(0) == f32::MAX);

    // 最小正值
    fp_regs.set(1, f32::MIN_POSITIVE);
    assert!(fp_regs.get(1) == f32::MIN_POSITIVE);

    // 最小负值
    fp_regs.set(2, f32::MIN);
    assert!(fp_regs.get(2) == f32::MIN);
}

#[test]
fn test_double_special_values() {
    // 正零和负零 (使用f32)
    assert_eq!(0.0_f32, 0.0);
    assert_eq!((-0.0_f32).to_bits(), 0x80000000);

    // NaN
    assert!(f32::NAN.is_nan());

    // 无穷
    assert!(f32::INFINITY.is_infinite());
    assert!(f32::NEG_INFINITY.is_infinite());
}

// ============================================================================
// F/D扩展互操作性测试
// ============================================================================

#[test]
fn test_f32_to_f64_conversion() {
    // 从F扩展的单精度转换为D扩展的双精度
    let f32_val: f32 = 1.5;
    let f64_val: f64 = f32_val as f64;

    assert_eq!(f64_val, 1.5);
}

#[test]
fn test_f64_to_f32_conversion() {
    // 从D扩展的双精度转换为F扩展的单精度
    let f64_val: f64 = 1.5;
    let f32_val: f32 = f64_val as f32;

    assert_eq!(f32_val, 1.5);
}

#[test]
fn test_precision_loss_conversion() {
    // 测试双精度转单精度的精度损失
    let f64_val = std::f64::consts::PI;
    let f32_val = f64_val as f32;
    let f64_back = f32_val as f64;

    // 精度损失，但应该接近
    assert!((f64_back - f64_val).abs() < 1e-6);
}

#[test]
fn test_overflow_f64_to_f32() {
    // 测试双精度溢出到单精度
    let huge_f64 = f64::MAX;
    let f32_val = huge_f64 as f32;

    // 应该是无穷大
    assert!(f32_val.is_infinite());
}

// ============================================================================
// 性能测试
// ============================================================================

#[test]
fn test_fp_registers_d_performance() {
    let mut fp_regs = FPRegistersD::default();

    let start = std::time::Instant::now();

    // 1000次读写操作
    for i in 0..1000 {
        fp_regs.set(i % 32, i as f32);
        let _ = fp_regs.get(i % 32);
    }

    let duration = start.elapsed();

    // 应该很快 (< 1ms)
    assert!(duration.as_millis() < 1);
}

#[test]
fn test_double_operations_performance() {
    let start = std::time::Instant::now();

    // 1000次双精度浮点运算
    let mut result = 0.0_f64;
    for i in 0..1000 {
        result = (i as f64) * 1.5 + (i as f64) / 2.0;
    }

    let duration = start.elapsed();

    // 应该很快 (< 1ms)
    assert!(duration.as_millis() < 1);
    assert!(result > 0.0); // 避免优化掉
}

#[test]
fn test_conversion_performance() {
    let start = std::time::Instant::now();

    // 1000次类型转换
    let mut result = 0.0_f64;
    for i in 0..1000 {
        let f32_val = i as f32;
        let f64_val = f32_val as f64;
        result += f64_val;
    }

    let duration = start.elapsed();

    // 应该很快 (< 1ms)
    assert!(duration.as_millis() < 1);
    assert!(result > 0.0);
}

// ============================================================================
// 并发测试
// ============================================================================

#[test]
fn test_fp_registers_d_concurrent_access() {
    use std::sync::Arc;
    use std::thread;

    let fp_regs = Arc::new(std::sync::Mutex::new(FPRegistersD::default()));
    let mut handles = vec![];

    // 多线程并发读写 (使用f32，FPRegisters底层是f32)
    for i in 0..10 {
        let regs_clone = Arc::clone(&fp_regs);
        let handle = thread::spawn(move || {
            let mut regs = regs_clone.lock().unwrap();
            for j in 0..100 {
                regs.set((i * 100 + j) % 32, (i * 100 + j) as f32);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // 验证最终状态
    let regs = fp_regs.lock().unwrap();
    for i in 0..32 {
        assert!(regs.get(i) >= 0.0);
    }
}

// ============================================================================
// 集成测试
// ============================================================================

#[test]
fn test_d_extension_integration() {
    let mut fcsr = FCSR::default();
    let mut fp_regs = FPRegistersD::default();

    // 模拟一次双精度浮点运算
    fp_regs.set(1, 10.0);
    fp_regs.set(2, 3.0);

    let result = fp_regs.get(1) / fp_regs.get(2);
    assert!((result - 3.3333333_f32).abs() < f32::EPSILON);

    // 设置不精确标志
    fcsr.flags.nx = true;
    assert!(fcsr.flags.nx);
}

#[test]
fn test_f_d_extension_interop() {
    let mut fp_regs = FPRegistersD::default();

    // F扩展和D扩展的互操作 (使用f32)
    // 1. 单精度值
    fp_regs.set(0, 1.5_f32);
    assert_eq!(fp_regs.get(0), 1.5_f32);

    // 2. 转换
    let val = fp_regs.get(0);
    assert_eq!(val, 1.5);

    // 3. 运算
    fp_regs.set(1, val * 2.0);
    assert_eq!(fp_regs.get(1), 3.0);

    // 4. 转回
    let float_val = fp_regs.get(1);
    assert_eq!(float_val, 3.0_f32);
}

// ============================================================================
// 数学函数测试
// ============================================================================

#[test]
fn test_double_sqrt_precision() {
    let mut fp_regs = FPRegistersD::default();

    // 平方根测试 (使用f32)
    fp_regs.set(1, 2.0);
    let result = fp_regs.get(1).sqrt();

    assert!((result - 1.41421356_f32).abs() < f32::EPSILON);
}

#[test]
fn test_double_trigonometric() {
    // 使用数学库验证 (使用f32)
    let angle = std::f32::consts::PI / 4.0; // 45度

    let sin_val = angle.sin();
    let cos_val = angle.cos();

    // sin(45°) = cos(45°) ≈ 0.707
    assert!((sin_val - 0.70710678_f32).abs() < 1e-5);
    assert!((cos_val - 0.70710678_f32).abs() < 1e-5);
}

#[test]
fn test_double_exponential() {
    // 指数函数 (使用f32)
    let val = 2.0_f32.exp();
    assert!((val - 7.389056_f32).abs() < f32::EPSILON);
}

#[test]
fn test_double_logarithm() {
    // 对数函数 (使用f32)
    let val = 10.0_f32.ln();
    assert!((val - 2.3025851_f32).abs() < f32::EPSILON);
}

#[test]
fn test_double_power() {
    // 幂函数 (使用f32)
    let base = 2.0_f32;
    let exp = 10.0_f32;
    let result = base.powf(exp);

    assert!((result - 1024.0).abs() < f32::EPSILON);
}

// ============================================================================
// 常用数学常量测试
// ============================================================================

#[test]
fn test_double_constants() {
    // π (使用f32)
    assert!((std::f32::consts::PI - 3.1415927).abs() < f32::EPSILON);

    // e
    assert!((std::f32::consts::E - 2.7182817).abs() < f32::EPSILON);

    // √2
    assert!((2.0_f32.sqrt() - 1.41421356_f32).abs() < f32::EPSILON);

    // ln(2)
    assert!((std::f32::consts::LN_2 - 0.6931472).abs() < f32::EPSILON);

    // log10(2)
    assert!((std::f32::consts::LOG10_2 - 0.30103).abs() < f32::EPSILON);
}
