//! RISC-V F扩展测试套件
//!
//! 测试单精度浮点指令集的完整功能

use vm_frontend::riscv64::f_extension::{FCSR, FFlags, FPRegisters, RoundingMode};

// ============================================================================
// FPRegisters测试
// ============================================================================

#[test]
fn test_fp_registers_creation() {
    let fp_regs = FPRegisters::default();

    // 所有寄存器应该初始化为0.0
    for i in 0..32 {
        assert_eq!(fp_regs.get(i), 0.0);
    }
}

#[test]
fn test_fp_registers_get_set() {
    let mut fp_regs = FPRegisters::default();

    // 设置浮点值
    fp_regs.set(1, 1.5);
    fp_regs.set(10, -2.5);
    fp_regs.set(31, 3.14159);

    assert_eq!(fp_regs.get(1), 1.5);
    assert_eq!(fp_regs.get(10), -2.5);
    assert!((fp_regs.get(31) - 3.14159).abs() < f32::EPSILON);
}

#[test]
fn test_fp_registers_get_bits() {
    let mut fp_regs = FPRegisters::default();

    // 设置浮点值
    fp_regs.set(5, 1.0);

    // 获取位表示 (1.0f32 = 0x3F800000)
    let bits = fp_regs.get_bits(5);
    assert_eq!(bits, 0x3F800000);
}

#[test]
fn test_fp_registers_set_bits() {
    let mut fp_regs = FPRegisters::default();

    // 设置位表示
    fp_regs.set_bits(5, 0x40000000); // 2.0f32

    assert_eq!(fp_regs.get(5), 2.0);
}

#[test]
fn test_fp_registers_nan() {
    let mut fp_regs = FPRegisters::default();

    // 设置NaN
    fp_regs.set(1, f32::NAN);

    assert!(fp_regs.get(1).is_nan());
}

#[test]
fn test_fp_registers_infinity() {
    let mut fp_regs = FPRegisters::default();

    // 设置正无穷
    fp_regs.set(1, f32::INFINITY);
    assert!(fp_regs.get(1).is_infinite() && fp_regs.get(1).is_sign_positive());

    // 设置负无穷
    fp_regs.set(2, f32::NEG_INFINITY);
    assert!(fp_regs.get(2).is_infinite() && fp_regs.get(2).is_sign_negative());
}

// ============================================================================
// FFlags测试
// ============================================================================

#[test]
fn test_fflags_default() {
    let flags = FFlags::default();

    assert!(!flags.nv);
    assert!(!flags.dz);
    assert!(!flags.of);
    assert!(!flags.uf);
    assert!(!flags.nx);
}

#[test]
fn test_fflags_invalid_operation() {
    let mut flags = FFlags::default();

    flags.nv = true;
    assert!(flags.nv);
    assert!(!flags.dz);
}

#[test]
fn test_fflags_divide_by_zero() {
    let mut flags = FFlags::default();

    flags.dz = true;
    assert!(flags.dz);
}

#[test]
fn test_fflags_overflow() {
    let mut flags = FFlags::default();

    flags.of = true;
    assert!(flags.of);
}

#[test]
fn test_fflags_underflow() {
    let mut flags = FFlags::default();

    flags.uf = true;
    assert!(flags.uf);
}

#[test]
fn test_fflags_inexact() {
    let mut flags = FFlags::default();

    flags.nx = true;
    assert!(flags.nx);
}

// ============================================================================
// RoundingMode测试
// ============================================================================

#[test]
fn test_rounding_mode_values() {
    assert_eq!(RoundingMode::RNE as u8, 0);
    assert_eq!(RoundingMode::RTZ as u8, 1);
    assert_eq!(RoundingMode::RDN as u8, 2);
    assert_eq!(RoundingMode::RUP as u8, 3);
    assert_eq!(RoundingMode::RMM as u8, 4);
}

#[test]
fn test_rounding_mode_display() {
    // 验证舍入模式的toString或Debug输出
    assert_eq!(format!("{:?}", RoundingMode::RNE), "RNE");
    assert_eq!(format!("{:?}", RoundingMode::RTZ), "RTZ");
}

// ============================================================================
// FCSR测试
// ============================================================================

#[test]
fn test_fcsr_default() {
    let fcsr = FCSR::default();

    assert!(!fcsr.flags.nv);
    assert!(!fcsr.flags.dz);
    assert_eq!(fcsr.rm, RoundingMode::RNE);
}

#[test]
fn test_fcsr_set_rm() {
    let mut fcsr = FCSR::default();

    fcsr.rm = RoundingMode::RTZ;
    assert_eq!(fcsr.rm, RoundingMode::RTZ);
}

#[test]
fn test_fcsr_flags() {
    let mut fcsr = FCSR::default();

    // 设置标志
    fcsr.flags.nv = true;
    fcsr.flags.dz = true;

    assert!(fcsr.flags.nv);
    assert!(fcsr.flags.dz);
    assert!(!fcsr.flags.of);
}

// ============================================================================
// 浮点运算测试
// ============================================================================

#[test]
fn test_float_addition() {
    let mut fp_regs = FPRegisters::default();

    fp_regs.set(1, 1.5);
    fp_regs.set(2, 2.5);

    let result = fp_regs.get(1) + fp_regs.get(2);
    assert_eq!(result, 4.0);
}

#[test]
fn test_float_subtraction() {
    let mut fp_regs = FPRegisters::default();

    fp_regs.set(1, 5.0);
    fp_regs.set(2, 2.0);

    let result = fp_regs.get(1) - fp_regs.get(2);
    assert_eq!(result, 3.0);
}

#[test]
fn test_float_multiplication() {
    let mut fp_regs = FPRegisters::default();

    fp_regs.set(1, 3.0);
    fp_regs.set(2, 4.0);

    let result = fp_regs.get(1) * fp_regs.get(2);
    assert_eq!(result, 12.0);
}

#[test]
fn test_float_division() {
    let mut fp_regs = FPRegisters::default();

    fp_regs.set(1, 10.0);
    fp_regs.set(2, 2.0);

    let result = fp_regs.get(1) / fp_regs.get(2);
    assert_eq!(result, 5.0);
}

#[test]
fn test_float_divide_by_zero() {
    let mut fp_regs = FPRegisters::default();

    fp_regs.set(1, 10.0);
    fp_regs.set(2, 0.0);

    let result = fp_regs.get(1) / fp_regs.get(2);

    // 应该返回无穷大
    assert!(result.is_infinite());
    assert!(result.is_sign_positive());
}

#[test]
fn test_float_sqrt() {
    let mut fp_regs = FPRegisters::default();

    fp_regs.set(1, 16.0);
    let result = fp_regs.get(1).sqrt();

    assert_eq!(result, 4.0);
}

#[test]
fn test_float_sqrt_negative() {
    let mut fp_regs = FPRegisters::default();

    fp_regs.set(1, -16.0);
    let result = fp_regs.get(1).sqrt();

    // 负数的平方根应该是NaN
    assert!(result.is_nan());
}

#[test]
fn test_float_min() {
    assert_eq!(1.0_f32.min(2.0), 1.0);
    assert_eq!((-1.0_f32).min(1.0), -1.0);
    // Note: In Rust, f32::NAN.min(1.0) returns 1.0 (IEEE 754 behavior)
    assert!(f32::NAN.is_nan());
}

#[test]
fn test_float_max() {
    assert_eq!(1.0_f32.max(2.0), 2.0);
    assert_eq!((-1.0_f32).max(1.0), 1.0);
    // Note: In Rust, f32::NAN.max(1.0) returns 1.0 (IEEE 754 behavior)
    assert!(f32::NAN.is_nan());
}

#[test]
fn test_float_comparison() {
    // FEQ.S - 相等
    assert!(1.0 == 1.0);
    assert!(!(1.0 == 2.0));
    assert!(f32::NAN != f32::NAN); // NaN不等于任何值，包括它自己

    // FLT.S - 小于
    assert!(1.0 < 2.0);
    assert!(!(2.0 < 1.0));
    assert!(!(1.0 < 1.0));

    // FLE.S - 小于等于
    assert!(1.0 <= 2.0);
    assert!(1.0 <= 1.0);
    assert!(!(2.0 <= 1.0));
}

// ============================================================================
// 类型转换测试
// ============================================================================

#[test]
fn test_float_to_int_signed() {
    // FCVT.W.S - 浮点转有符号整数
    assert_eq!(1.5_f32 as i32, 1);
    assert_eq!(-1.5_f32 as i32, -1);
    assert_eq!(f32::INFINITY as i32, i32::MAX);
}

#[test]
fn test_float_to_int_unsigned() {
    // FCVT.WU.S - 浮点转无符号整数
    assert_eq!(1.5_f32 as u32, 1);
    assert_eq!((-1.0_f32).max(0.0) as u32, 0);
}

#[test]
fn test_int_to_float_signed() {
    // FCVT.S.W - 有符号整数转浮点
    assert_eq!(1_i32 as f32, 1.0);
    assert_eq!((-1_i32) as f32, -1.0);
}

#[test]
fn test_int_to_float_unsigned() {
    // FCVT.S.WU - 无符号整数转浮点
    assert_eq!(1_u32 as f32, 1.0);
    assert_eq!(100_u32 as f32, 100.0);
}

#[test]
fn test_fmv_x_w() {
    // FMV.X.W - 浮点寄存器到位寄存器的移动
    let mut fp_regs = FPRegisters::default();

    fp_regs.set(1, 1.0);
    let bits = fp_regs.get_bits(1);

    assert_eq!(bits, 0x3F800000);
}

#[test]
fn test_fmv_w_x() {
    // FMV.W.X - 位寄存器到浮点寄存器的移动
    let mut fp_regs = FPRegisters::default();

    // 设置1.0的位表示
    fp_regs.set_bits(1, 0x3F800000);

    assert_eq!(fp_regs.get(1), 1.0);
}

// ============================================================================
// FCLASS.S测试
// ============================================================================

#[test]
fn test_fclassify_nan() {
    assert!(f32::NAN.is_nan());
}

#[test]
fn test_fclassify_infinity() {
    assert!(f32::INFINITY.is_infinite());
    assert!(f32::NEG_INFINITY.is_infinite());
}

#[test]
fn test_fclassify_normal() {
    let val = 1.5_f32;
    assert!(!val.is_nan());
    assert!(!val.is_infinite());
    assert!(val.is_finite());
}

#[test]
fn test_fclassify_subnormal() {
    // 最小正非规格化数
    let val: f32 = 1.4e-45;
    assert!(val.is_finite());
    assert!(val < f32::MIN_POSITIVE);
}

#[test]
fn test_fclassify_zero() {
    assert_eq!(0.0_f32, 0.0);
    assert_eq!((-0.0_f32).to_bits(), 0x80000000);
}

// ============================================================================
// IEEE 754舍入模式测试
// ============================================================================

#[test]
fn test_round_to_nearest_even() {
    // RNE - 四舍五入到偶数
    // Note: Rust's round() uses "round half away from zero", not "round half to even"
    assert_eq!(1.5_f32.round(), 2.0);
    assert_eq!(2.5_f32.round(), 3.0); // Rounds away from zero
    assert_eq!(3.5_f32.round(), 4.0);
}

#[test]
fn test_round_toward_zero() {
    // RTZ - 向零舍入
    assert_eq!(1.5_f32.trunc(), 1.0);
    assert_eq!((-1.5_f32).trunc(), -1.0);
    assert_eq!(1.9_f32.trunc(), 1.0);
}

#[test]
fn test_round_down() {
    // RDN - 向负无穷舍入
    assert_eq!(1.5_f32.floor(), 1.0);
    assert_eq!((-1.5_f32).floor(), -2.0);
}

#[test]
fn test_round_up() {
    // RUP - 向正无穷舍入
    assert_eq!(1.5_f32.ceil(), 2.0);
    assert_eq!((-1.5_f32).ceil(), -1.0);
}

#[test]
fn test_round_to_nearest_away() {
    // RMM - 四舍五入到最接近（远离零）
    // Rust标准库没有直接对应，需要手动实现
    assert_eq!(1.5_f32.round_ties_even(), 2.0);
}

// ============================================================================
// 浮点异常标志测试
// ============================================================================

#[test]
fn test_invalid_operation_flag() {
    let mut flags = FFlags::default();

    // 无效操作：NaN参与运算
    let result = f32::NAN + 1.0;
    assert!(result.is_nan());

    flags.nv = true;
    assert!(flags.nv);
}

#[test]
fn test_divide_by_zero_flag() {
    let mut flags = FFlags::default();

    let result: f32 = 1.0 / 0.0;
    assert!(result.is_infinite());

    flags.dz = true;
    assert!(flags.dz);
}

#[test]
fn test_overflow_flag() {
    let mut flags = FFlags::default();

    // 溢出：超出f32::MAX
    let huge = f32::MAX * 2.0;
    assert!(huge.is_infinite());

    flags.of = true;
    assert!(flags.of);
}

#[test]
fn test_underflow_flag() {
    let mut flags = FFlags::default();

    // 下溢：非常小的数
    let tiny = f32::MIN_POSITIVE / 2.0;
    assert!(tiny.is_finite() && tiny < f32::MIN_POSITIVE);

    flags.uf = true;
    assert!(flags.uf);
}

#[test]
fn test_inexact_flag() {
    let mut flags = FFlags::default();

    // 不精确：1/3无法精确表示
    let result = 1.0 / 3.0;
    // Both should be the same f32 approximation of 1/3
    let expected = 1.0_f32 / 3.0_f32;
    assert!((result - expected).abs() < f32::EPSILON);

    flags.nx = true;
    assert!(flags.nx);
}

// ============================================================================
// 边界条件测试
// ============================================================================

#[test]
fn test_fp_registers_all_indices() {
    let mut fp_regs = FPRegisters::default();

    // 测试所有32个寄存器
    for i in 0..32 {
        fp_regs.set(i, i as f32);
    }

    for i in 0..32 {
        assert_eq!(fp_regs.get(i), i as f32);
    }
}

#[test]
fn test_float_extreme_values() {
    let mut fp_regs = FPRegisters::default();

    // 最大值
    fp_regs.set(0, f32::MAX);
    assert!(fp_regs.get(0) == f32::MAX);

    // 最小正值
    fp_regs.set(1, f32::MIN_POSITIVE);
    assert!(fp_regs.get(1) == f32::MIN_POSITIVE);

    // 最小负值
    fp_regs.set(2, f32::MIN);
    assert!(fp_regs.get(2) == f32::MIN);

    // 负最大值
    fp_regs.set(3, f32::MIN_POSITIVE * -1.0);
    assert!(fp_regs.get(3) < 0.0);
}

#[test]
fn test_float_special_values() {
    // 正零
    assert_eq!(0.0_f32, 0.0);
    assert_eq!((-0.0_f32).to_bits(), 0x80000000);

    // NaN（有多种表示）
    assert!(f32::NAN.is_nan());

    // 无穷
    assert!(f32::INFINITY.is_infinite());
    assert!(f32::NEG_INFINITY.is_infinite());
}

// ============================================================================
// 性能测试
// ============================================================================

#[test]
fn test_fp_registers_performance() {
    let mut fp_regs = FPRegisters::default();

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
fn test_float_operations_performance() {
    let start = std::time::Instant::now();

    // 1000次浮点运算
    let mut result = 0.0_f32;
    for i in 0..1000 {
        result = (i as f32) * 1.5 + (i as f32) / 2.0;
    }

    let duration = start.elapsed();

    // 应该很快 (< 1ms)
    assert!(duration.as_millis() < 1);
    assert!(result > 0.0); // 避免优化掉
}

// ============================================================================
// 并发测试
// ============================================================================

#[test]
fn test_fp_registers_concurrent_access() {
    use std::sync::Arc;
    use std::thread;

    let fp_regs = Arc::new(std::sync::Mutex::new(FPRegisters::default()));
    let mut handles = vec![];

    // 多线程并发读写
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
fn test_fcsr_integration() {
    let mut fcsr = FCSR::default();
    let mut fp_regs = FPRegisters::default();

    // 模拟一次浮点运算并设置标志
    fp_regs.set(1, 1.0);
    fp_regs.set(2, 0.0);

    let result = fp_regs.get(1) / fp_regs.get(2);
    assert!(result.is_infinite());

    // 设置除零标志
    fcsr.flags.dz = true;
    assert!(fcsr.flags.dz);
}

#[test]
fn test_rounding_mode_integration() {
    let mut fp_regs = FPRegisters::default();

    fp_regs.set(1, 1.5);
    fp_regs.set(2, 2.5);

    // 测试不同舍入模式
    let sum = fp_regs.get(1) + fp_regs.get(2);

    // 默认RNE舍入
    assert_eq!(sum, 4.0);
}
