//! vm-simd 全面的SIMD运算测试
//!
//! 测试向量运算、饱和运算、位运算等核心功能

use vm_simd::*;

// ============================================================================
// 基础向量运算测试
// ============================================================================

#[test]
fn test_vec_add_different_sizes() {
    // 测试8位元素加法
    let a8 = 0x0102030405060708u64;
    let b8 = 0x0101010101010101u64;
    let result8 = vec_add(a8, b8, 1);
    assert_eq!(result8, 0x0203040506070809u64);

    // 测试16位元素加法
    let a16 = 0x0001000200030004u64;
    let b16 = 0x0001000100010001u64;
    let result16 = vec_add(a16, b16, 2);
    assert_eq!(result16, 0x0002000300040005u64);

    // 测试32位元素加法
    let a32 = 0x0000000100000002u64;
    let b32 = 0x0000000100000001u64;
    let result32 = vec_add(a32, b32, 4);
    assert_eq!(result32, 0x0000000200000003u64);

    // 测试64位元素加法
    let a64 = 0x0000000000000001u64;
    let b64 = 0x0000000000000001u64;
    let result64 = vec_add(a64, b64, 8);
    assert_eq!(result64, 0x0000000000000002u64);
}

#[test]
fn test_vec_sub_different_sizes() {
    // 测试8位元素减法
    let a8 = 0x0807060504030201u64;
    let b8 = 0x0101010101010101u64;
    let result8 = vec_sub(a8, b8, 1);
    assert_eq!(result8, 0x0706050403020100u64);

    // 测试16位元素减法
    let a16 = 0x0004000300020001u64;
    let b16 = 0x0001000100010001u64;
    let result16 = vec_sub(a16, b16, 2);
    assert_eq!(result16, 0x0003000200010000u64);

    // 测试32位元素减法
    let a32 = 0x0000000300000002u64;
    let b32 = 0x0000000100000001u64;
    let result32 = vec_sub(a32, b32, 4);
    assert_eq!(result32, 0x0000000200000001u64);

    // 测试64位元素减法
    let a64 = 0x0000000000000005u64;
    let b64 = 0x0000000000000003u64;
    let result64 = vec_sub(a64, b64, 8);
    assert_eq!(result64, 0x0000000000000002u64);
}

#[test]
fn test_vec_mul_different_sizes() {
    // 测试8位元素乘法
    let a8 = 0x0202020202020202u64;
    let b8 = 0x0303030303030303u64;
    let result8 = vec_mul(a8, b8, 1);
    assert_eq!(result8, 0x0606060606060606u64);

    // 测试16位元素乘法
    let a16 = 0x0002000200020002u64;
    let b16 = 0x0003000300030003u64;
    let result16 = vec_mul(a16, b16, 2);
    assert_eq!(result16, 0x0006000600060006u64);

    // 测试32位元素乘法
    let a32 = 0x0000000200000002u64;
    let b32 = 0x0000000300000003u64;
    let result32 = vec_mul(a32, b32, 4);
    assert_eq!(result32, 0x0000000600000006u64);
}

// ============================================================================
// 饱和运算测试
// ============================================================================

#[test]
fn test_vec_add_sat_u8() {
    // 测试无符号8位饱和加法
    let a = 0xFF00FF00FF00FF00u64;
    let b = 0x0101010101010101u64;
    let result = vec_add_sat_u(a, b, 1);

    // 0xFF + 0x01 应该饱和到 0xFF
    // 0x00 + 0x01 = 0x01
    assert_eq!(result, 0xFF01FF01FF01FF01u64);
}

#[test]
fn test_vec_add_sat_u16() {
    // 测试无符号16位饱和加法
    let a = 0xFFFF0000FFFF0000u64;
    let b = 0x0001000100010001u64;
    let result = vec_add_sat_u(a, b, 2);

    // 0xFFFF + 0x01 应该饱和到 0xFFFF
    assert_eq!(result, 0xFFFF0001FFFF0001u64);
}

#[test]
fn test_vec_sub_sat_u8() {
    // 测试无符号8位饱和减法
    let a = 0x0000000000000000u64;
    let b = 0x0101010101010101u64;
    let result = vec_sub_sat_u(a, b, 1);

    // 0x00 - 0x01 应该饱和到 0x00
    assert_eq!(result, 0x0000000000000000u64);
}

#[test]
fn test_vec_add_sat_s8() {
    // 测试有符号8位饱和加法
    // 0x7F (127) + 0x01 = 128 应该饱和到 0x7F (127)
    // 0x80 (-128) + 0x01 = -127 (0x81) 应该保持为 -127
    let a = 0x7F7F7F7F7F7F7F7Fu64;
    let b = 0x0101010101010101u64;
    let result = vec_add_sat_s(a, b, 1);

    // 正数溢出应该饱和到最大正值 0x7F
    // 0x7F + 0x01 = 128 -> 饱和到 127 (0x7F)
    // 0x7F (odd lanes) + 0x01 = 饱和到 0x7F
    // 0x7F (even lanes) + 0x01 = 饱和到 0x7F
    // All lanes should be 0x7F since we're adding 1 to max positive
    assert_eq!(result, 0x7F7F7F7F7F7F7F7Fu64);
}

#[test]
fn test_vec_sub_sat_s8() {
    // 测试有符号8位饱和减法
    // 0x80 (-128) - 0x01 = -129 应该饱和到 -128 (0x80)
    let a = 0x8080808080808080u64;
    let b = 0x0101010101010101u64;
    let result = vec_sub_sat_s(a, b, 1);

    // 负数下溢应该饱和到最小负值 0x80
    // All lanes: -128 - 1 = -129, saturates to -128 (0x80)
    assert_eq!(result, 0x8080808080808080u64);
}

#[test]
fn test_vec_mul_sat_u8() {
    // 测试无符号8位饱和乘法
    let a = 0xFF00000000000000u64;
    let b = 0x0200000000000000u64;
    let result = vec_mul_sat_u(a, b, 1);

    // 0xFF * 0x02 = 0x1FE (510) 应该饱和到 0xFF (255)
    assert_eq!(result >> 56, 0xFF);
}

// ============================================================================
// 位运算测试
// ============================================================================

#[test]
fn test_vec_and() {
    let a = 0xFFFFFFFFFFFFFFFFu64;
    let b = 0xAAAAAAAAAAAAAAAAu64;
    let result = vec_and(a, b);
    assert_eq!(result, 0xAAAAAAAAAAAAAAAAu64);

    let a = 0xFFFFFFFFFFFFFFFFu64;
    let b = 0x5555555555555555u64;
    let result = vec_and(a, b);
    assert_eq!(result, 0x5555555555555555u64);
}

#[test]
fn test_vec_or() {
    let a = 0xAAAAAAAAAAAAAAAAu64;
    let b = 0x5555555555555555u64;
    let result = vec_or(a, b);
    assert_eq!(result, 0xFFFFFFFFFFFFFFFFu64);

    let a = 0x0000000000000000u64;
    let b = 0xFFFFFFFFFFFFFFFFu64;
    let result = vec_or(a, b);
    assert_eq!(result, 0xFFFFFFFFFFFFFFFFu64);
}

#[test]
fn test_vec_xor() {
    let a = 0xFFFFFFFFFFFFFFFFu64;
    let b = 0xFFFFFFFFFFFFFFFFu64;
    let result = vec_xor(a, b);
    assert_eq!(result, 0x0000000000000000u64);

    let a = 0xAAAAAAAAAAAAAAAAu64;
    let b = 0x5555555555555555u64;
    let result = vec_xor(a, b);
    assert_eq!(result, 0xFFFFFFFFFFFFFFFFu64);
}

#[test]
fn test_vec_not() {
    let a = 0x0000000000000000u64;
    let result = vec_not(a);
    assert_eq!(result, 0xFFFFFFFFFFFFFFFFu64);

    let a = 0xFFFFFFFFFFFFFFFFu64;
    let result = vec_not(a);
    assert_eq!(result, 0x0000000000000000u64);

    let a = 0xAAAAAAAAAAAAAAAAu64;
    let result = vec_not(a);
    assert_eq!(result, 0x5555555555555555u64);
}

// ============================================================================
// 比较运算测试
// ============================================================================

#[test]
fn test_vec_cmpeq() {
    let a = 0x0101010101010101u64;
    let b = 0x0101010101010101u64;
    let result = vec_cmpeq(a, b, 1);
    // 所有元素相等，应该返回全1
    assert_eq!(result, 0xFFFFFFFFFFFFFFFFu64);

    let a = 0x0102030405060708u64;
    let b = 0x0102030405060708u64;
    let result = vec_cmpeq(a, b, 1);
    assert_eq!(result, 0xFFFFFFFFFFFFFFFFu64);

    let a = 0x0102030405060708u64;
    let b = 0x0807060504030201u64;
    let result = vec_cmpeq(a, b, 1);
    // 所有元素不相等，应该返回全0
    assert_eq!(result, 0x0000000000000000u64);
}

#[test]
fn test_vec_cmpgt_u() {
    let a = 0x0202020202020202u64;
    let b = 0x0101010101010101u64;
    let result = vec_cmpgt_u(a, b, 1);
    // 所有a的元素大于b，应该返回全1
    assert_eq!(result, 0xFFFFFFFFFFFFFFFFu64);

    let a = 0x0101010101010101u64;
    let b = 0x0202020202020202u64;
    let result = vec_cmpgt_u(a, b, 1);
    // 所有a的元素小于b，应该返回全0
    assert_eq!(result, 0x0000000000000000u64);
}

#[test]
fn test_vec_min_u() {
    let a = 0x0505050505050505u64;
    let b = 0x0A0A0A0A0A0A0A0Au64;
    let result = vec_min_u(a, b, 1);
    // a的所有元素都小于b，应该返回a
    assert_eq!(result, 0x0505050505050505u64);

    let a = 0x0A0A0A0A0A0A0A0Au64;
    let b = 0x0505050505050505u64;
    let result = vec_min_u(a, b, 1);
    // a的所有元素都大于b，应该返回b
    assert_eq!(result, 0x0505050505050505u64);
}

#[test]
fn test_vec_max_u() {
    let a = 0x0505050505050505u64;
    let b = 0x0A0A0A0A0A0A0A0Au64;
    let result = vec_max_u(a, b, 1);
    // a的所有元素都小于b，应该返回b
    assert_eq!(result, 0x0A0A0A0A0A0A0A0Au64);

    let a = 0x0A0A0A0A0A0A0A0Au64;
    let b = 0x0505050505050505u64;
    let result = vec_max_u(a, b, 1);
    // a的所有元素都大于b，应该返回a
    assert_eq!(result, 0x0A0A0A0A0A0A0A0Au64);
}

// ============================================================================
// 移位运算测试
// ============================================================================

#[test]
fn test_vec_shl() {
    let a = 0x0101010101010101u64;

    // 左移1位
    let result = vec_shl(a, 1, 1);
    assert_eq!(result, 0x0202020202020202u64);

    // 左移4位
    let result = vec_shl(a, 4, 1);
    assert_eq!(result, 0x1010101010101010u64);

    // 左移7位
    let result = vec_shl(a, 7, 1);
    assert_eq!(result, 0x8080808080808080u64);
}

#[test]
fn test_vec_shr_u() {
    let a = 0x8080808080808080u64;

    // 右移1位
    let result = vec_shr_u(a, 1, 1);
    assert_eq!(result, 0x4040404040404040u64);

    // 右移4位
    let result = vec_shr_u(a, 4, 1);
    assert_eq!(result, 0x0808080808080808u64);

    // 右移7位
    let result = vec_shr_u(a, 7, 1);
    assert_eq!(result, 0x0101010101010101u64);
}

// ============================================================================
// 256位向量测试
// ============================================================================

#[test]
fn test_vec256_add() {
    let a = [1u64, 2, 3, 4];
    let b = [5u64, 6, 7, 8];
    let result = vec256_add(a, b, 8);
    assert_eq!(result, [6u64, 8, 10, 12]);
}

#[test]
fn test_vec256_sub() {
    let a = [10u64, 20, 30, 40];
    let b = [1u64, 2, 3, 4];
    let result = vec256_sub(a, b, 8);
    assert_eq!(result, [9u64, 18, 27, 36]);
}

#[test]
fn test_vec256_mul() {
    let a = [2u64, 3, 4, 5];
    let b = [3u64, 4, 5, 6];
    let result = vec256_mul(a, b, 8);
    assert_eq!(result, [6u64, 12, 20, 30]);
}

#[test]
fn test_vec256_add_sat_u() {
    let a = [0xFFu64, 0xFF, 0xFF, 0xFF];
    let b = [1u64, 1, 1, 1];
    let result = vec256_add_sat_u(a, b, 1);

    // 饱和到0xFF
    assert_eq!(result[0], 0xFF);
    assert_eq!(result[1], 0xFF);
    assert_eq!(result[2], 0xFF);
    assert_eq!(result[3], 0xFF);
}

// ============================================================================
// 512位向量测试
// ============================================================================

#[test]
fn test_vec512_add() {
    let a = [1u64, 2, 3, 4, 5, 6, 7, 8];
    let b = [1u64, 1, 1, 1, 1, 1, 1, 1];
    let result = vec512_add(a, b, 8);
    assert_eq!(result, [2u64, 3, 4, 5, 6, 7, 8, 9]);
}

#[test]
fn test_vec512_sub() {
    let a = [10u64, 20, 30, 40, 50, 60, 70, 80];
    let b = [1u64, 2, 3, 4, 5, 6, 7, 8];
    let result = vec512_sub(a, b, 8);
    assert_eq!(result, [9u64, 18, 27, 36, 45, 54, 63, 72]);
}

#[test]
fn test_vec512_mul() {
    let a = [2u64, 2, 2, 2, 2, 2, 2, 2];
    let b = [3u64, 3, 3, 3, 3, 3, 3, 3];
    let result = vec512_mul(a, b, 8);
    assert_eq!(result, [6u64, 6, 6, 6, 6, 6, 6, 6]);
}

// ============================================================================
// 浮点运算测试
// ============================================================================

#[test]
fn test_vec_fadd_f32() {
    let a = [1.0f32, 2.0, 3.0, 4.0];
    let b = [0.5f32, 0.5, 0.5, 0.5];
    let result = vec_fadd_f32(a, b);
    assert_eq!(result, [1.5f32, 2.5, 3.5, 4.5]);
}

#[test]
fn test_vec_fsub_f32() {
    let a = [5.0f32, 10.0, 15.0, 20.0];
    let b = [1.0f32, 2.0, 3.0, 4.0];
    let result = vec_fsub_f32(a, b);
    assert_eq!(result, [4.0f32, 8.0, 12.0, 16.0]);
}

#[test]
fn test_vec_fmul_f32() {
    let a = [2.0f32, 3.0, 4.0, 5.0];
    let b = [3.0f32, 4.0, 5.0, 6.0];
    let result = vec_fmul_f32(a, b);
    assert_eq!(result, [6.0f32, 12.0, 20.0, 30.0]);
}

#[test]
fn test_vec_fdiv_f32() {
    let a = [10.0f32, 20.0, 30.0, 40.0];
    let b = [2.0f32, 4.0, 5.0, 8.0];
    let result = vec_fdiv_f32(a, b);
    assert_eq!(result, [5.0f32, 5.0, 6.0, 5.0]);
}

#[test]
fn test_vec_fadd_f64() {
    let a = [1.0f64, 2.0];
    let b = [0.5f64, 0.5];
    let result = vec_fadd_f64(a, b);
    assert_eq!(result, [1.5f64, 2.5]);
}

#[test]
fn test_vec_fsub_f64() {
    let a = [10.0f64, 20.0];
    let b = [3.0f64, 5.0];
    let result = vec_fsub_f64(a, b);
    assert_eq!(result, [7.0f64, 15.0]);
}

#[test]
fn test_vec_fmul_f64() {
    let a = [2.5f64, 3.0];
    let b = [4.0f64, 5.0];
    let result = vec_fmul_f64(a, b);
    assert_eq!(result, [10.0f64, 15.0]);
}

#[test]
fn test_vec_fdiv_f64() {
    let a = [20.0f64, 30.0];
    let b = [4.0f64, 6.0];
    let result = vec_fdiv_f64(a, b);
    assert_eq!(result, [5.0f64, 5.0]);
}

#[test]
fn test_vec_fma_f32() {
    let a = [2.0f32, 3.0, 4.0, 5.0];
    let b = [3.0f32, 4.0, 5.0, 6.0];
    let c = [1.0f32, 1.0, 1.0, 1.0];
    let result = vec_fma_f32(a, b, c);

    // a * b + c
    assert_eq!(result, [7.0f32, 13.0, 21.0, 31.0]);
}

#[test]
fn test_vec_fma_f64() {
    let a = [2.0f64, 3.0];
    let b = [3.0f64, 4.0];
    let c = [1.0f64, 1.0];
    let result = vec_fma_f64(a, b, c);

    // a * b + c
    assert_eq!(result, [7.0f64, 13.0]);
}

// ============================================================================
// 边界条件测试
// ============================================================================

#[test]
fn test_vec_add_overflow() {
    // 测试溢出情况（使用包装语义）
    let a = 0xFFFFFFFFFFFFFFFFu64;
    let b = 0x0000000000000001u64;
    let result = vec_add(a, b, 8);
    assert_eq!(result, 0x0000000000000000u64);
}

#[test]
fn test_vec_sub_underflow() {
    // 测试下溢情况（使用包装语义）
    let a = 0x0000000000000000u64;
    let b = 0x0000000000000001u64;
    let result = vec_sub(a, b, 8);
    assert_eq!(result, 0xFFFFFFFFFFFFFFFFu64);
}

#[test]
fn test_vec_mul_overflow() {
    // 测试乘法溢出
    let a = 0xFFFFFFFFFFFFFFFFu64;
    let b = 0x0000000000000002u64;
    let result = vec_mul(a, b, 8);

    // 由于是64位乘法，会发生溢出
    // 0xFFFFFFFFFFFFFFFF * 2 = 0x1FFFFFFFFFFFFFFFE
    // 在64位空间中，结果是 0xFFFFFFFFFFFFFFFE
    assert_eq!(result, 0xFFFFFFFFFFFFFFFEu64);
}

#[test]
fn test_zero_values() {
    // 测试零值操作
    let a = 0u64;
    let b = 0xFFFFFFFFFFFFFFFFu64;

    assert_eq!(vec_add(a, b, 8), b);
    assert_eq!(vec_sub(a, b, 8), 1u64);
    assert_eq!(vec_mul(a, b, 8), 0u64);
    assert_eq!(vec_and(a, b), 0u64);
    assert_eq!(vec_or(a, b), b);
    assert_eq!(vec_xor(a, b), b);
}

#[test]
fn test_all_ones_values() {
    // 测试全1值操作
    let a = 0xFFFFFFFFFFFFFFFFu64;
    let b = 0xFFFFFFFFFFFFFFFFu64;

    assert_eq!(vec_and(a, b), a);
    assert_eq!(vec_or(a, b), a);
    assert_eq!(vec_xor(a, b), 0u64);
    assert_eq!(vec_not(a), 0u64);
}

// ============================================================================
// 浮点特殊值测试
// ============================================================================

#[test]
fn test_f32_infinity() {
    let a = [f32::INFINITY, 1.0, 2.0, 3.0];
    let b = [1.0f32, 1.0, 1.0, 1.0];
    let result = vec_fadd_f32(a, b);

    // INFINITY + 1.0 = INFINITY
    assert_eq!(result[0], f32::INFINITY);
    assert_eq!(result[1], 2.0);
    assert_eq!(result[2], 3.0);
    assert_eq!(result[3], 4.0);
}

#[test]
fn test_f32_nan() {
    let a = [f32::NAN, 1.0, 2.0, 3.0];
    let b = [1.0f32, 1.0, 1.0, 1.0];
    let result = vec_fadd_f32(a, b);

    // NaN + 1.0 = NaN
    assert!(result[0].is_nan());
    assert_eq!(result[1], 2.0);
    assert_eq!(result[2], 3.0);
    assert_eq!(result[3], 4.0);
}

#[test]
fn test_f64_infinity() {
    let a = [f64::INFINITY, 1.0];
    let b = [1.0f64, 1.0];
    let result = vec_fadd_f64(a, b);

    // INFINITY + 1.0 = INFINITY
    assert_eq!(result[0], f64::INFINITY);
    assert_eq!(result[1], 2.0);
}

#[test]
fn test_f64_nan() {
    let a = [f64::NAN, 1.0];
    let b = [1.0f64, 1.0];
    let result = vec_fadd_f64(a, b);

    // NaN + 1.0 = NaN
    assert!(result[0].is_nan());
    assert_eq!(result[1], 2.0);
}

// ============================================================================
// 性能测试（简单的基准测试）
// ============================================================================

#[test]
fn test_performance_vec_add() {
    // 简单的性能验证：确保SIMD操作至少不比标量操作慢太多
    let iterations = 1000;
    let a = 0x0102030405060708u64;
    let b = 0x0101010101010101u64;

    for _ in 0..iterations {
        let _result = vec_add(a, b, 1);
    }

    // 如果能执行到这里，说明操作可以正常完成
    // Test passes if no panic occurs
}

#[test]
fn test_performance_vec_fadd_f32() {
    // 简单的性能验证
    let iterations = 1000;
    let a = [1.0f32, 2.0, 3.0, 4.0];
    let b = [0.5f32, 0.5, 0.5, 0.5];

    for _ in 0..iterations {
        let _result = vec_fadd_f32(a, b);
    }

    // 如果能执行到这里，说明操作可以正常完成
    // Test passes if no panic occurs
}
