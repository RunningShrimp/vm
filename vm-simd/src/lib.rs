//! vm-simd - 平台向量运算辅助库
//! 为解释器和 JIT 提供可复用的 SIMD 优化路径。

/// 计算单个 64-bit packed 向量的逐元素加法（按 element_size 分段）。
pub fn vec_add(a: u64, b: u64, element_size: u8) -> u64 {
    platform::vec_add(a, b, element_size).unwrap_or_else(|| fallback_vec_binop(a, b, element_size, |x, y| x.wrapping_add(y)))
}

/// 计算逐元素减法。
pub fn vec_sub(a: u64, b: u64, element_size: u8) -> u64 {
    platform::vec_sub(a, b, element_size).unwrap_or_else(|| fallback_vec_binop(a, b, element_size, |x, y| x.wrapping_sub(y)))
}

/// 计算逐元素乘法。
pub fn vec_mul(a: u64, b: u64, element_size: u8) -> u64 {
    platform::vec_mul(a, b, element_size).unwrap_or_else(|| fallback_vec_binop(a, b, element_size, |x, y| x.wrapping_mul(y)))
}

/// 256-bit（4×u64）向量加法。
pub fn vec256_add(a: [u64; 4], b: [u64; 4], element_size: u8) -> [u64; 4] {
    if let Some(res) = platform::vec256_add(&a, &b, element_size) {
        res
    } else {
        combine_chunks(a, b, element_size, vec_add)
    }
}

/// 256-bit 逐元素减法。
pub fn vec256_sub(a: [u64; 4], b: [u64; 4], element_size: u8) -> [u64; 4] {
    if let Some(res) = platform::vec256_sub(&a, &b, element_size) {
        res
    } else {
        combine_chunks(a, b, element_size, vec_sub)
    }
}

/// 256-bit 逐元素乘法。
pub fn vec256_mul(a: [u64; 4], b: [u64; 4], element_size: u8) -> [u64; 4] {
    if let Some(res) = platform::vec256_mul(&a, &b, element_size) {
        res
    } else {
        combine_chunks(a, b, element_size, vec_mul)
    }
}

fn combine_chunks(
    a: [u64; 4],
    b: [u64; 4],
    element_size: u8,
    f: fn(u64, u64, u8) -> u64,
) -> [u64; 4] {
    [
        f(a[0], b[0], element_size),
        f(a[1], b[1], element_size),
        f(a[2], b[2], element_size),
        f(a[3], b[3], element_size),
    ]
}

fn fallback_vec_binop(
    a: u64,
    b: u64,
    element_size: u8,
    op: impl Fn(u64, u64) -> u64,
) -> u64 {
    let lane_bits = (element_size.max(1) as u64) * 8;
    let lanes = 64 / lane_bits;
    let mut acc = 0u64;
    for i in 0..lanes {
        let shift = i * lane_bits;
        let mask = ((1u128 << lane_bits) - 1) as u64;
        let av = (a >> shift) & mask;
        let bv = (b >> shift) & mask;
        let rv = op(av, bv) & mask;
        acc |= rv << shift;
    }
    acc
}

#[cfg(target_arch = "x86_64")]
mod platform {
    use super::*;
    use std::arch::x86_64::*;

    pub fn vec_add(a: u64, b: u64, element_size: u8) -> Option<u64> {
        if !is_x86_feature_detected!("sse2") {
            return None;
        }
        unsafe { sse_binop(a, b, element_size, BinOp::Add) }
    }

    pub fn vec_sub(a: u64, b: u64, element_size: u8) -> Option<u64> {
        if !is_x86_feature_detected!("sse2") {
            return None;
        }
        unsafe { sse_binop(a, b, element_size, BinOp::Sub) }
    }

    pub fn vec_mul(a: u64, b: u64, element_size: u8) -> Option<u64> {
        if !is_x86_feature_detected!("sse2") {
            return None;
        }
        unsafe { sse_mul(a, b, element_size) }
    }

    pub fn vec256_add(a: &[u64; 4], b: &[u64; 4], element_size: u8) -> Option<[u64; 4]> {
        if !is_x86_feature_detected!("avx2") {
            return None;
        }
        unsafe { avx_binop(a, b, element_size, BinOp::Add) }
    }

    pub fn vec256_sub(a: &[u64; 4], b: &[u64; 4], element_size: u8) -> Option<[u64; 4]> {
        if !is_x86_feature_detected!("avx2") {
            return None;
        }
        unsafe { avx_binop(a, b, element_size, BinOp::Sub) }
    }

    pub fn vec256_mul(a: &[u64; 4], b: &[u64; 4], element_size: u8) -> Option<[u64; 4]> {
        if !is_x86_feature_detected!("avx2") {
            return None;
        }
        unsafe { avx_mul(a, b, element_size) }
    }

    #[derive(Clone, Copy)]
    enum BinOp {
        Add,
        Sub,
    }

    #[target_feature(enable = "sse2")]
    unsafe fn sse_binop(a: u64, b: u64, element_size: u8, op: BinOp) -> Option<u64> {
        let va = _mm_cvtsi64_si128(a as i64);
        let vb = _mm_cvtsi64_si128(b as i64);
        let res = match element_size {
            1 => match op {
                BinOp::Add => _mm_add_epi8(va, vb),
                BinOp::Sub => _mm_sub_epi8(va, vb),
            },
            2 => match op {
                BinOp::Add => _mm_add_epi16(va, vb),
                BinOp::Sub => _mm_sub_epi16(va, vb),
            },
            4 => match op {
                BinOp::Add => _mm_add_epi32(va, vb),
                BinOp::Sub => _mm_sub_epi32(va, vb),
            },
            8 => match op {
                BinOp::Add => _mm_add_epi64(va, vb),
                BinOp::Sub => _mm_sub_epi64(va, vb),
            },
            _ => return None,
        };
        Some(_mm_cvtsi128_si64(res) as u64)
    }

    #[target_feature(enable = "sse2")]
    unsafe fn sse_mul(a: u64, b: u64, element_size: u8) -> Option<u64> {
        let va = _mm_cvtsi64_si128(a as i64);
        let vb = _mm_cvtsi64_si128(b as i64);
        let res = match element_size {
            2 => _mm_mullo_epi16(va, vb),
            4 => _mm_mullo_epi32(va, vb),
            _ => return None,
        };
        Some(_mm_cvtsi128_si64(res) as u64)
    }

    #[target_feature(enable = "avx2")]
    unsafe fn avx_binop(a: &[u64; 4], b: &[u64; 4], element_size: u8, op: BinOp) -> Option<[u64; 4]> {
        let va = _mm256_loadu_si256(a.as_ptr() as *const __m256i);
        let vb = _mm256_loadu_si256(b.as_ptr() as *const __m256i);
        let res = match element_size {
            1 => match op {
                BinOp::Add => _mm256_add_epi8(va, vb),
                BinOp::Sub => _mm256_sub_epi8(va, vb),
            },
            2 => match op {
                BinOp::Add => _mm256_add_epi16(va, vb),
                BinOp::Sub => _mm256_sub_epi16(va, vb),
            },
            4 => match op {
                BinOp::Add => _mm256_add_epi32(va, vb),
                BinOp::Sub => _mm256_sub_epi32(va, vb),
            },
            8 => match op {
                BinOp::Add => _mm256_add_epi64(va, vb),
                BinOp::Sub => _mm256_sub_epi64(va, vb),
            },
            _ => return None,
        };
        let mut out = [0u64; 4];
        _mm256_storeu_si256(out.as_mut_ptr() as *mut __m256i, res);
        Some(out)
    }

    #[target_feature(enable = "avx2")]
    unsafe fn avx_mul(a: &[u64; 4], b: &[u64; 4], element_size: u8) -> Option<[u64; 4]> {
        let va = _mm256_loadu_si256(a.as_ptr() as *const __m256i);
        let vb = _mm256_loadu_si256(b.as_ptr() as *const __m256i);
        let res = match element_size {
            2 => _mm256_mullo_epi16(va, vb),
            4 => _mm256_mullo_epi32(va, vb),
            _ => return None,
        };
        let mut out = [0u64; 4];
        _mm256_storeu_si256(out.as_mut_ptr() as *mut __m256i, res);
        Some(out)
    }
}

#[cfg(not(target_arch = "x86_64"))]
mod platform {
    pub fn vec_add(_a: u64, _b: u64, _element_size: u8) -> Option<u64> { None }
    pub fn vec_sub(_a: u64, _b: u64, _element_size: u8) -> Option<u64> { None }
    pub fn vec_mul(_a: u64, _b: u64, _element_size: u8) -> Option<u64> { None }
    pub fn vec256_add(_a: &[u64; 4], _b: &[u64; 4], _element_size: u8) -> Option<[u64; 4]> { None }
    pub fn vec256_sub(_a: &[u64; 4], _b: &[u64; 4], _element_size: u8) -> Option<[u64; 4]> { None }
    pub fn vec256_mul(_a: &[u64; 4], _b: &[u64; 4], _element_size: u8) -> Option<[u64; 4]> { None }
}
