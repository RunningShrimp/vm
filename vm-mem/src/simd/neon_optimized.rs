// ARM64 NEON优化的小向量操作
//
// Round 35优化1: NEON intrinsic深度使用
// 目的: 为小向量运算(<64元素)提供NEON优化版本
//
// 基于 Round 34 NEON性能测试结果:
// - 4元素: 2.75-4.60x 加速
// - 16元素: 4.17-4.60x 加速
// - 64元素: 1.33-2.13x 加速
//
// 预期提升: 2-4x (小向量场景)

use std::arch::aarch64::*;

/// NEON优化的4元素向量加法
///
/// # Safety
/// 调用者确保输入数组至少有4个元素
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub unsafe fn vec4_add_f32(a: &[f32; 4], b: &[f32; 4]) -> [f32; 4] {
    let a_vec = vld1q_f32(a.as_ptr());
    let b_vec = vld1q_f32(b.as_ptr());
    let result = vaddq_f32(a_vec, b_vec);

    let mut output = [0.0f32; 4];
    vst1q_f32(output.as_mut_ptr(), result);
    output
}

/// 标量fallback: 4元素向量加法
#[cfg(not(target_arch = "aarch64"))]
#[inline(always)]
pub fn vec4_add_f32(a: &[f32; 4], b: &[f32; 4]) -> [f32; 4] {
    [
        a[0] + b[0],
        a[1] + b[1],
        a[2] + b[2],
        a[3] + b[3],
    ]
}

/// NEON优化的4元素向量乘法
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub unsafe fn vec4_mul_f32(a: &[f32; 4], b: &[f32; 4]) -> [f32; 4] {
    let a_vec = vld1q_f32(a.as_ptr());
    let b_vec = vld1q_f32(b.as_ptr());
    let result = vmulq_f32(a_vec, b_vec);

    let mut output = [0.0f32; 4];
    vst1q_f32(output.as_mut_ptr(), result);
    output
}

/// 标量fallback: 4元素向量乘法
#[cfg(not(target_arch = "aarch64"))]
#[inline(always)]
pub fn vec4_mul_f32(a: &[f32; 4], b: &[f32; 4]) -> [f32; 4] {
    [
        a[0] * b[0],
        a[1] * b[1],
        a[2] * b[2],
        a[3] * b[3],
    ]
}

/// NEON优化的融合乘加: a*b+c
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub unsafe fn vec4_fma_f32(a: &[f32; 4], b: &[f32; 4], c: &[f32; 4]) -> [f32; 4] {
    let a_vec = vld1q_f32(a.as_ptr());
    let b_vec = vld1q_f32(b.as_ptr());
    let c_vec = vld1q_f32(c.as_ptr());

    // FMA: a*b + c
    let mul_result = vmulq_f32(a_vec, b_vec);
    let result = vaddq_f32(mul_result, c_vec);

    let mut output = [0.0f32; 4];
    vst1q_f32(output.as_mut_ptr(), result);
    output
}

/// 标量fallback: 融合乘加
#[cfg(not(target_arch = "aarch64"))]
#[inline(always)]
pub fn vec4_fma_f32(a: &[f32; 4], b: &[f32; 4], c: &[f32; 4]) -> [f32; 4] {
    [
        a[0] * b[0] + c[0],
        a[1] * b[1] + c[1],
        a[2] * b[2] + c[2],
        a[3] * b[3] + c[3],
    ]
}

/// NEON优化的点积运算
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub unsafe fn vec4_dot_f32(a: &[f32; 4], b: &[f32; 4]) -> f32 {
    let a_vec = vld1q_f32(a.as_ptr());
    let b_vec = vld1q_f32(b.as_ptr());

    let mul = vmulq_f32(a_vec, b_vec);

    // 水平求和
    let mut result = [0.0f32; 4];
    vst1q_f32(result.as_mut_ptr(), mul);
    result.iter().sum()
}

/// 标量fallback: 点积
#[cfg(not(target_arch = "aarch64"))]
#[inline(always)]
pub fn vec4_dot_f32(a: &[f32; 4], b: &[f32; 4]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3]
}

/// NEON优化的16元素向量加法 (使用循环展开)
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub unsafe fn vec16_add_f32(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    let mut result = [0.0f32; 16];

    // 处理4个向量块 (4×4=16)
    for i in 0..4 {
        let offset = i * 4;
        let a_vec = vld1q_f32(a[offset..].as_ptr());
        let b_vec = vld1q_f32(b[offset..].as_ptr());
        let sum = vaddq_f32(a_vec, b_vec);
        vst1q_f32(result[offset..].as_mut_ptr(), sum);
    }

    result
}

/// 标量fallback: 16元素向量加法
#[cfg(not(target_arch = "aarch64"))]
#[inline(always)]
pub fn vec16_add_f32(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    let mut result = [0.0f32; 16];
    for i in 0..16 {
        result[i] = a[i] + b[i];
    }
    result
}

/// NEON优化的16元素向量乘法
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub unsafe fn vec16_mul_f32(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    let mut result = [0.0f32; 16];

    for i in 0..4 {
        let offset = i * 4;
        let a_vec = vld1q_f32(a[offset..].as_ptr());
        let b_vec = vld1q_f32(b[offset..].as_ptr());
        let product = vmulq_f32(a_vec, b_vec);
        vst1q_f32(result[offset..].as_mut_ptr(), product);
    }

    result
}

/// 标量fallback: 16元素向量乘法
#[cfg(not(target_arch = "aarch64"))]
#[inline(always)]
pub fn vec16_mul_f32(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    let mut result = [0.0f32; 16];
    for i in 0..16 {
        result[i] = a[i] * b[i];
    }
    result
}

/// NEON优化的16元素点积
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub unsafe fn vec16_dot_f32(a: &[f32; 16], b: &[f32; 16]) -> f32 {
    let mut sum_vec = vdupq_n_f32(0.0);

    for i in 0..4 {
        let offset = i * 4;
        let a_vec = vld1q_f32(a[offset..].as_ptr());
        let b_vec = vld1q_f32(b[offset..].as_ptr());
        let mul = vmulq_f32(a_vec, b_vec);
        sum_vec = vaddq_f32(sum_vec, mul);
    }

    // 水平求和
    let mut result = [0.0f32; 4];
    vst1q_f32(result.as_mut_ptr(), sum_vec);
    result.iter().sum()
}

/// 标量fallback: 16元素点积
#[cfg(not(target_arch = "aarch64"))]
#[inline(always)]
pub fn vec16_dot_f32(a: &[f32; 16], b: &[f32; 16]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| x * y)
        .sum()
}

/// 批量向量加法 (自适应大小)
///
/// 自动选择最优策略:
/// - <=16元素: NEON intrinsic
/// - >16元素: 标量(编译器自动向量化)
#[inline(always)]
pub fn vec_add_f32(a: &[f32], b: &[f32]) -> Vec<f32> {
    assert_eq!(a.len(), b.len(), "Input vectors must have equal length");

    let len = a.len();

    if len == 4 {
        unsafe {
            let a_arr = [a[0], a[1], a[2], a[3]];
            let b_arr = [b[0], b[1], b[2], b[3]];
            vec4_add_f32(&a_arr, &b_arr).to_vec()
        }
    } else if len == 16 {
        unsafe {
            let mut a_arr = [0.0f32; 16];
            let mut b_arr = [0.0f32; 16];
            a_arr.copy_from_slice(a);
            b_arr.copy_from_slice(b);
            vec16_add_f32(&a_arr, &b_arr).to_vec()
        }
    } else if len < 64 {
        // 中等大小: 逐个元素处理
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| x + y)
            .collect()
    } else {
        // 大向量: 标量代码(编译器会自动向量化)
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| x + y)
            .collect()
    }
}

/// 批量向量乘法 (自适应大小)
#[inline(always)]
pub fn vec_mul_f32(a: &[f32], b: &[f32]) -> Vec<f32> {
    assert_eq!(a.len(), b.len(), "Input vectors must have equal length");

    let len = a.len();

    if len == 4 {
        unsafe {
            let a_arr = [a[0], a[1], a[2], a[3]];
            let b_arr = [b[0], b[1], b[2], b[3]];
            vec4_mul_f32(&a_arr, &b_arr).to_vec()
        }
    } else if len == 16 {
        unsafe {
            let mut a_arr = [0.0f32; 16];
            let mut b_arr = [0.0f32; 16];
            a_arr.copy_from_slice(a);
            b_arr.copy_from_slice(b);
            vec16_mul_f32(&a_arr, &b_arr).to_vec()
        }
    } else if len < 64 {
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| x * y)
            .collect()
    } else {
        // 大向量: 标量代码
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| x * y)
            .collect()
    }
}

/// 点积运算 (自适应大小)
#[inline(always)]
pub fn vec_dot_f32(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len(), "Input vectors must have equal length");

    let len = a.len();

    if len == 4 {
        unsafe {
            let a_arr = [a[0], a[1], a[2], a[3]];
            let b_arr = [b[0], b[1], b[2], b[3]];
            vec4_dot_f32(&a_arr, &b_arr)
        }
    } else if len == 16 {
        unsafe {
            let mut a_arr = [0.0f32; 16];
            let mut b_arr = [0.0f32; 16];
            a_arr.copy_from_slice(a);
            b_arr.copy_from_slice(b);
            vec16_dot_f32(&a_arr, &b_arr)
        }
    } else if len <= 256 {
        // 中等大小: 使用点积算法
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| x * y)
            .sum()
    } else {
        // 大向量: 标量代码
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| x * y)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vec4_add_f32() {
        let a = [1.0, 2.0, 3.0, 4.0];
        let b = [5.0, 6.0, 7.0, 8.0];
        let result = unsafe { vec4_add_f32(&a, &b) };
        assert_eq!(result, [6.0, 8.0, 10.0, 12.0]);
    }

    #[test]
    fn test_vec4_mul_f32() {
        let a = [1.0, 2.0, 3.0, 4.0];
        let b = [5.0, 6.0, 7.0, 8.0];
        let result = unsafe { vec4_mul_f32(&a, &b) };
        assert_eq!(result, [5.0, 12.0, 21.0, 32.0]);
    }

    #[test]
    fn test_vec4_fma_f32() {
        let a = [2.0, 3.0, 4.0, 5.0];
        let b = [3.0, 4.0, 5.0, 6.0];
        let c = [1.0, 1.0, 1.0, 1.0];
        let result = unsafe { vec4_fma_f32(&a, &b, &c) };
        // a*b+c: [2*3+1, 3*4+1, 4*5+1, 5*6+1] = [7, 13, 21, 31]
        assert_eq!(result, [7.0, 13.0, 21.0, 31.0]);
    }

    #[test]
    fn test_vec4_dot_f32() {
        let a = [1.0, 2.0, 3.0, 4.0];
        let b = [5.0, 6.0, 7.0, 8.0];
        let result = unsafe { vec4_dot_f32(&a, &b) };
        assert_eq!(result, 70.0); // 1*5 + 2*6 + 3*7 + 4*8
    }

    #[test]
    fn test_vec16_add_f32() {
        let a: [f32; 16] = (0..16).map(|i| i as f32).collect::<Vec<_>>().try_into().unwrap();
        let b: [f32; 16] = (0..16).map(|i| (i + 16) as f32).collect::<Vec<_>>().try_into().unwrap();
        let result = unsafe { vec16_add_f32(&a, &b) };
        assert_eq!(result, [16.0, 18.0, 20.0, 22.0, 24.0, 26.0, 28.0, 30.0,
                           32.0, 34.0, 36.0, 38.0, 40.0, 42.0, 44.0, 46.0]);
    }

    #[test]
    fn test_vec16_mul_f32() {
        let a = [2.0; 16];
        let b = [3.0; 16];
        let result = unsafe { vec16_mul_f32(&a, &b) };
        assert!(result.iter().all(|&x| x == 6.0));
    }

    #[test]
    fn test_vec16_dot_f32() {
        let a = [1.0; 16];
        let b = [2.0; 16];
        let result = unsafe { vec16_dot_f32(&a, &b) };
        assert_eq!(result, 32.0); // 16个1*2的和
    }

    #[test]
    fn test_adaptive_vec_add() {
        let a = vec![1.0, 2.0, 3.0, 4.0];
        let b = vec![5.0, 6.0, 7.0, 8.0];
        let result = vec_add_f32(&a, &b);
        assert_eq!(result, vec![6.0, 8.0, 10.0, 12.0]);
    }

    #[test]
    fn test_adaptive_vec_mul() {
        let a = vec![1.0, 2.0, 3.0, 4.0];
        let b = vec![5.0, 6.0, 7.0, 8.0];
        let result = vec_mul_f32(&a, &b);
        assert_eq!(result, vec![5.0, 12.0, 21.0, 32.0]);
    }

    #[test]
    fn test_adaptive_vec_dot() {
        let a = vec![1.0, 2.0, 3.0, 4.0];
        let b = vec![5.0, 6.0, 7.0, 8.0];
        let result = vec_dot_f32(&a, &b);
        assert_eq!(result, 70.0);
    }
}
