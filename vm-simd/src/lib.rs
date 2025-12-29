//! vm-simd - 平台向量运算辅助库
//! 为解释器和 JIT 提供可复用的 SIMD 优化路径。
//!
//! ## 特性
//! - 自动检测并使用最佳 SIMD 指令集 (SSE2/AVX2/AVX-512/NEON)
//! - 64-bit、128-bit、256-bit、512-bit 向量支持
//! - 饱和算术运算
//! - 浮点向量运算
//! - 位运算和比较操作

/// Apple AMX 执行引擎
pub mod apple_amx;
/// AVX2 高级指令
pub mod avx2_advanced;
/// AVX512 指令
pub mod avx512;
/// GPU 加速 - CUDA/HIP 集成
pub mod gpu_accel;
/// GPU 计算流水线
pub mod gpu_pipeline;
/// Qualcomm Hexagon DSP 执行引擎
pub mod hexagon_dsp;
/// HiSilicon NPU 执行引擎
pub mod hisilicon_npu;
/// MediaTek APU 执行引擎
pub mod mediatek_apu;
/// ARM NEON 高级指令
pub mod neon_advanced;
/// RISC-V Vector 指令
pub mod riscv_vector;
/// ARM SVE 指令
pub mod sve;

pub mod opt;

pub use opt::SimdOptimizer;

/// 计算单个 64-bit packed 向量的逐元素加法（按 element_size 分段）。
pub fn vec_add(a: u64, b: u64, element_size: u8) -> u64 {
    platform::vec_add(a, b, element_size)
        .unwrap_or_else(|| fallback_vec_binop(a, b, element_size, |x, y| x.wrapping_add(y)))
}

/// 计算逐元素减法。
pub fn vec_sub(a: u64, b: u64, element_size: u8) -> u64 {
    platform::vec_sub(a, b, element_size)
        .unwrap_or_else(|| fallback_vec_binop(a, b, element_size, |x, y| x.wrapping_sub(y)))
}

/// 计算逐元素乘法。
pub fn vec_mul(a: u64, b: u64, element_size: u8) -> u64 {
    platform::vec_mul(a, b, element_size)
        .unwrap_or_else(|| fallback_vec_binop(a, b, element_size, |x, y| x.wrapping_mul(y)))
}

/// 饱和加法 (无符号)
pub fn vec_add_sat_u(a: u64, b: u64, element_size: u8) -> u64 {
    platform::vec_add_sat_u(a, b, element_size).unwrap_or_else(|| {
        let lane_bits = (element_size.max(1) as u64) * 8;
        let lanes = 64 / lane_bits;
        let mut acc = 0u64;
        let max_val = (1u128 << lane_bits) - 1;
        for i in 0..lanes {
            let shift = i * lane_bits;
            let mask = (1u128 << lane_bits) - 1;
            let av = ((a >> shift) as u128) & mask;
            let bv = ((b >> shift) as u128) & mask;
            let sum = av + bv;
            let rv = if sum > max_val { max_val } else { sum } as u64;
            acc |= rv << shift;
        }
        acc
    })
}

/// 饱和减法 (无符号)
pub fn vec_sub_sat_u(a: u64, b: u64, element_size: u8) -> u64 {
    platform::vec_sub_sat_u(a, b, element_size)
        .unwrap_or_else(|| fallback_vec_binop(a, b, element_size, |x, y| x.saturating_sub(y)))
}

/// 饱和加法 (有符号)
pub fn vec_add_sat_s(a: u64, b: u64, element_size: u8) -> u64 {
    let lane_bits = (element_size.max(1) as u64) * 8;
    let lanes = 64 / lane_bits;
    let mut acc = 0u64;
    let mask = ((1u128 << lane_bits) - 1) as u64;
    let sign_bit = 1u64 << (lane_bits - 1);
    let max_val = sign_bit - 1; // e.g., 0x7F for i8
    let min_val_bits = sign_bit; // e.g., 0x80 for i8 (represents -128)

    for i in 0..lanes {
        let shift = i * lane_bits;
        let av = (a >> shift) & mask;
        let bv = (b >> shift) & mask;
        let a_signed = if av & sign_bit != 0 {
            (av | !mask) as i64
        } else {
            av as i64
        };
        let b_signed = if bv & sign_bit != 0 {
            (bv | !mask) as i64
        } else {
            bv as i64
        };
        let sum = (a_signed as i128) + (b_signed as i128);
        let max_i = max_val as i128;
        let min_i = -(min_val_bits as i128);
        let clamped = if sum > max_i {
            max_i
        } else if sum < min_i {
            min_i
        } else {
            sum
        };
        let rv = (clamped as u64) & mask;
        acc |= rv << shift;
    }
    acc
}

/// 饱和减法 (有符号)
pub fn vec_sub_sat_s(a: u64, b: u64, element_size: u8) -> u64 {
    let lane_bits = (element_size.max(1) as u64) * 8;
    let lanes = 64 / lane_bits;
    let mut acc = 0u64;
    let mask = ((1u128 << lane_bits) - 1) as u64;
    let sign_bit = 1u64 << (lane_bits - 1);

    for i in 0..lanes {
        let shift = i * lane_bits;
        let av = (a >> shift) & mask;
        let bv = (b >> shift) & mask;
        let a_signed = if av & sign_bit != 0 {
            (av | !mask) as i64
        } else {
            av as i64
        };
        let b_signed = if bv & sign_bit != 0 {
            (bv | !mask) as i64
        } else {
            bv as i64
        };
        let mut diff = a_signed - b_signed;
        let max_val = (sign_bit - 1) as i64;
        let min_val = -(sign_bit as i64);
        if diff > max_val {
            diff = max_val;
        }
        if diff < min_val {
            diff = min_val;
        }
        let rv = (diff as u64) & mask;
        acc |= rv << shift;
    }
    acc
}

/// 饱和乘法 (无符号)
pub fn vec_mul_sat_u(a: u64, b: u64, element_size: u8) -> u64 {
    let lane_bits = (element_size.max(1) as u64) * 8;
    let lanes = 64 / lane_bits;
    let mut acc = 0u64;
    let mask = ((1u128 << lane_bits) - 1) as u64;
    let max_val = mask;

    for i in 0..lanes {
        let shift = i * lane_bits;
        let av = (a >> shift) & mask;
        let bv = (b >> shift) & mask;

        // 无符号饱和乘法
        let product = (av as u128) * (bv as u128);
        let rv = if product > max_val as u128 {
            max_val
        } else {
            product as u64
        };
        acc |= rv << shift;
    }
    acc
}

/// 饱和乘法 (有符号)
pub fn vec_mul_sat_s(a: u64, b: u64, element_size: u8) -> u64 {
    let lane_bits = (element_size.max(1) as u64) * 8;
    let lanes = 64 / lane_bits;
    let mut acc = 0u64;
    let mask = ((1u128 << lane_bits) - 1) as u64;
    let sign_bit = 1u64 << (lane_bits - 1);
    let max_val = (sign_bit - 1) as i64; // e.g., 0x7F for i8
    let min_val = -(sign_bit as i64); // e.g., -128 for i8

    for i in 0..lanes {
        let shift = i * lane_bits;
        let av = (a >> shift) & mask;
        let bv = (b >> shift) & mask;

        // 符号扩展到 i64
        let a_signed = if av & sign_bit != 0 {
            (av | !mask) as i64
        } else {
            av as i64
        };
        let b_signed = if bv & sign_bit != 0 {
            (bv | !mask) as i64
        } else {
            bv as i64
        };

        // 有符号饱和乘法
        let product = (a_signed as i128) * (b_signed as i128);
        let rv = if product > max_val as i128 {
            max_val as u64
        } else if product < min_val as i128 {
            (min_val as u64) & mask
        } else {
            (product as u64) & mask
        };
        acc |= rv << shift;
    }
    acc
}

/// 按位与
pub fn vec_and(a: u64, b: u64) -> u64 {
    a & b
}

/// 按位或
pub fn vec_or(a: u64, b: u64) -> u64 {
    a | b
}

/// 按位异或
pub fn vec_xor(a: u64, b: u64) -> u64 {
    a ^ b
}

/// 按位非
pub fn vec_not(a: u64) -> u64 {
    !a
}

/// 逐元素最小值 (无符号)
pub fn vec_min_u(a: u64, b: u64, element_size: u8) -> u64 {
    platform::vec_min_u(a, b, element_size)
        .unwrap_or_else(|| fallback_vec_binop(a, b, element_size, |x, y| x.min(y)))
}

/// 逐元素最大值 (无符号)
pub fn vec_max_u(a: u64, b: u64, element_size: u8) -> u64 {
    platform::vec_max_u(a, b, element_size)
        .unwrap_or_else(|| fallback_vec_binop(a, b, element_size, |x, y| x.max(y)))
}

/// 逐元素比较相等
pub fn vec_cmpeq(a: u64, b: u64, element_size: u8) -> u64 {
    fallback_vec_binop(a, b, element_size, |x, y| if x == y { !0u64 } else { 0 })
}

/// 逐元素比较大于 (无符号)
pub fn vec_cmpgt_u(a: u64, b: u64, element_size: u8) -> u64 {
    fallback_vec_binop(a, b, element_size, |x, y| if x > y { !0u64 } else { 0 })
}

/// 逐元素左移
pub fn vec_shl(a: u64, shift: u8, element_size: u8) -> u64 {
    fallback_vec_unop(a, element_size, |x| {
        x << (shift as u64 % (element_size as u64 * 8))
    })
}

/// 逐元素右移 (逻辑)
pub fn vec_shr_u(a: u64, shift: u8, element_size: u8) -> u64 {
    fallback_vec_unop(a, element_size, |x| {
        x >> (shift as u64 % (element_size as u64 * 8))
    })
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

/// 256-bit 向量饱和加法（无符号）
pub fn vec256_add_sat_u(a: [u64; 4], b: [u64; 4], element_size: u8) -> [u64; 4] {
    combine_chunks(a, b, element_size, vec_add_sat_u)
}

/// 256-bit 向量饱和加法（有符号）
pub fn vec256_add_sat_s(a: [u64; 4], b: [u64; 4], element_size: u8) -> [u64; 4] {
    combine_chunks(a, b, element_size, vec_add_sat_s)
}

/// 256-bit 向量饱和减法（无符号）
pub fn vec256_sub_sat_u(a: [u64; 4], b: [u64; 4], element_size: u8) -> [u64; 4] {
    combine_chunks(a, b, element_size, vec_sub_sat_u)
}

/// 256-bit 向量饱和减法（有符号）
pub fn vec256_sub_sat_s(a: [u64; 4], b: [u64; 4], element_size: u8) -> [u64; 4] {
    combine_chunks(a, b, element_size, vec_sub_sat_s)
}

/// 256-bit 向量饱和乘法（无符号）
pub fn vec256_mul_sat_u(a: [u64; 4], b: [u64; 4], element_size: u8) -> [u64; 4] {
    combine_chunks(a, b, element_size, vec_mul_sat_u)
}

/// 256-bit 向量饱和乘法（有符号）
pub fn vec256_mul_sat_s(a: [u64; 4], b: [u64; 4], element_size: u8) -> [u64; 4] {
    combine_chunks(a, b, element_size, vec_mul_sat_s)
}

/// 512-bit（8×u64）向量加法
pub fn vec512_add(a: [u64; 8], b: [u64; 8], element_size: u8) -> [u64; 8] {
    if let Some(res) = platform::vec512_add(&a, &b, element_size) {
        res
    } else {
        combine_chunks_512(a, b, element_size, vec_add)
    }
}

/// 512-bit（8×u64）向量减法
pub fn vec512_sub(a: [u64; 8], b: [u64; 8], element_size: u8) -> [u64; 8] {
    if let Some(res) = platform::vec512_sub(&a, &b, element_size) {
        res
    } else {
        combine_chunks_512(a, b, element_size, vec_sub)
    }
}

/// 512-bit（8×u64）向量乘法
pub fn vec512_mul(a: [u64; 8], b: [u64; 8], element_size: u8) -> [u64; 8] {
    if let Some(res) = platform::vec512_mul(&a, &b, element_size) {
        res
    } else {
        combine_chunks_512(a, b, element_size, vec_mul)
    }
}

// ============================================================
// 浮点向量运算
// ============================================================

/// 单精度浮点向量加法 (4 x f32)
pub fn vec_fadd_f32(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    platform::vec_fadd_f32(&a, &b)
        .unwrap_or_else(|| [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]])
}

/// 单精度浮点向量减法 (4 x f32)
pub fn vec_fsub_f32(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    platform::vec_fsub_f32(&a, &b)
        .unwrap_or_else(|| [a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]])
}

/// 单精度浮点向量乘法 (4 x f32)
pub fn vec_fmul_f32(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    platform::vec_fmul_f32(&a, &b)
        .unwrap_or_else(|| [a[0] * b[0], a[1] * b[1], a[2] * b[2], a[3] * b[3]])
}

/// 单精度浮点向量除法 (4 x f32)
pub fn vec_fdiv_f32(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    platform::vec_fdiv_f32(&a, &b)
        .unwrap_or_else(|| [a[0] / b[0], a[1] / b[1], a[2] / b[2], a[3] / b[3]])
}

/// 双精度浮点向量加法 (2 x f64)
pub fn vec_fadd_f64(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    platform::vec_fadd_f64(&a, &b).unwrap_or([a[0] + b[0], a[1] + b[1]])
}

/// 双精度浮点向量减法 (2 x f64)
pub fn vec_fsub_f64(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    platform::vec_fsub_f64(&a, &b).unwrap_or([a[0] - b[0], a[1] - b[1]])
}

/// 双精度浮点向量乘法 (2 x f64)
pub fn vec_fmul_f64(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    platform::vec_fmul_f64(&a, &b).unwrap_or([a[0] * b[0], a[1] * b[1]])
}

/// 双精度浮点向量除法 (2 x f64)
pub fn vec_fdiv_f64(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    platform::vec_fdiv_f64(&a, &b).unwrap_or([a[0] / b[0], a[1] / b[1]])
}

/// FMA: a * b + c (单精度)
pub fn vec_fma_f32(a: [f32; 4], b: [f32; 4], c: [f32; 4]) -> [f32; 4] {
    platform::vec_fma_f32(&a, &b, &c).unwrap_or_else(|| {
        [
            a[0].mul_add(b[0], c[0]),
            a[1].mul_add(b[1], c[1]),
            a[2].mul_add(b[2], c[2]),
            a[3].mul_add(b[3], c[3]),
        ]
    })
}

/// FMA: a * b + c (双精度)
pub fn vec_fma_f64(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> [f64; 2] {
    platform::vec_fma_f64(&a, &b, &c)
        .unwrap_or_else(|| [a[0].mul_add(b[0], c[0]), a[1].mul_add(b[1], c[1])])
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

fn combine_chunks_512(
    a: [u64; 8],
    b: [u64; 8],
    element_size: u8,
    f: fn(u64, u64, u8) -> u64,
) -> [u64; 8] {
    [
        f(a[0], b[0], element_size),
        f(a[1], b[1], element_size),
        f(a[2], b[2], element_size),
        f(a[3], b[3], element_size),
        f(a[4], b[4], element_size),
        f(a[5], b[5], element_size),
        f(a[6], b[6], element_size),
        f(a[7], b[7], element_size),
    ]
}

fn fallback_vec_binop(a: u64, b: u64, element_size: u8, op: impl Fn(u64, u64) -> u64) -> u64 {
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

#[allow(dead_code)]
fn fallback_vec_binop_signed(
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

fn fallback_vec_unop(a: u64, element_size: u8, op: impl Fn(u64) -> u64) -> u64 {
    let lane_bits = (element_size.max(1) as u64) * 8;
    let lanes = 64 / lane_bits;
    let mut acc = 0u64;
    for i in 0..lanes {
        let shift = i * lane_bits;
        let mask = ((1u128 << lane_bits) - 1) as u64;
        let av = (a >> shift) & mask;
        let rv = op(av) & mask;
        acc |= rv << shift;
    }
    acc
}

#[cfg(target_arch = "x86_64")]
mod platform {
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

    pub fn vec_add_sat_u(a: u64, b: u64, element_size: u8) -> Option<u64> {
        if !is_x86_feature_detected!("sse2") {
            return None;
        }
        unsafe { sse_sat_op(a, b, element_size, SatOp::AddU) }
    }

    pub fn vec_sub_sat_u(a: u64, b: u64, element_size: u8) -> Option<u64> {
        if !is_x86_feature_detected!("sse2") {
            return None;
        }
        unsafe { sse_sat_op(a, b, element_size, SatOp::SubU) }
    }

    pub fn vec_min_u(a: u64, b: u64, element_size: u8) -> Option<u64> {
        if !is_x86_feature_detected!("sse2") {
            return None;
        }
        unsafe { sse_minmax(a, b, element_size, true) }
    }

    pub fn vec_max_u(a: u64, b: u64, element_size: u8) -> Option<u64> {
        if !is_x86_feature_detected!("sse2") {
            return None;
        }
        unsafe { sse_minmax(a, b, element_size, false) }
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

    pub fn vec512_add(a: &[u64; 8], b: &[u64; 8], element_size: u8) -> Option<[u64; 8]> {
        // 优先使用 AVX512，如果没有则回退到 AVX2
        if is_x86_feature_detected!("avx512f") {
            unsafe { avx512_binop(a, b, element_size, BinOp::Add) }
        } else if is_x86_feature_detected!("avx2") {
            unsafe { avx512_binop(a, b, element_size, BinOp::Add) }
        } else {
            None
        }
    }

    pub fn vec512_sub(a: &[u64; 8], b: &[u64; 8], element_size: u8) -> Option<[u64; 8]> {
        if is_x86_feature_detected!("avx512f") {
            unsafe { avx512_binop(a, b, element_size, BinOp::Sub) }
        } else if is_x86_feature_detected!("avx2") {
            unsafe { avx512_binop(a, b, element_size, BinOp::Sub) }
        } else {
            None
        }
    }

    pub fn vec512_mul(a: &[u64; 8], b: &[u64; 8], element_size: u8) -> Option<[u64; 8]> {
        if is_x86_feature_detected!("avx512f") {
            unsafe { avx512_mul(a, b, element_size) }
        } else if is_x86_feature_detected!("avx2") {
            unsafe { avx512_mul(a, b, element_size) }
        } else {
            None
        }
    }

    // 浮点向量操作
    pub fn vec_fadd_f32(a: &[f32; 4], b: &[f32; 4]) -> Option<[f32; 4]> {
        if !is_x86_feature_detected!("sse") {
            return None;
        }
        unsafe {
            let va = _mm_loadu_ps(a.as_ptr());
            let vb = _mm_loadu_ps(b.as_ptr());
            let res = _mm_add_ps(va, vb);
            let mut out = [0f32; 4];
            _mm_storeu_ps(out.as_mut_ptr(), res);
            Some(out)
        }
    }

    pub fn vec_fsub_f32(a: &[f32; 4], b: &[f32; 4]) -> Option<[f32; 4]> {
        if !is_x86_feature_detected!("sse") {
            return None;
        }
        unsafe {
            let va = _mm_loadu_ps(a.as_ptr());
            let vb = _mm_loadu_ps(b.as_ptr());
            let res = _mm_sub_ps(va, vb);
            let mut out = [0f32; 4];
            _mm_storeu_ps(out.as_mut_ptr(), res);
            Some(out)
        }
    }

    pub fn vec_fmul_f32(a: &[f32; 4], b: &[f32; 4]) -> Option<[f32; 4]> {
        if !is_x86_feature_detected!("sse") {
            return None;
        }
        unsafe {
            let va = _mm_loadu_ps(a.as_ptr());
            let vb = _mm_loadu_ps(b.as_ptr());
            let res = _mm_mul_ps(va, vb);
            let mut out = [0f32; 4];
            _mm_storeu_ps(out.as_mut_ptr(), res);
            Some(out)
        }
    }

    pub fn vec_fdiv_f32(a: &[f32; 4], b: &[f32; 4]) -> Option<[f32; 4]> {
        if !is_x86_feature_detected!("sse") {
            return None;
        }
        unsafe {
            let va = _mm_loadu_ps(a.as_ptr());
            let vb = _mm_loadu_ps(b.as_ptr());
            let res = _mm_div_ps(va, vb);
            let mut out = [0f32; 4];
            _mm_storeu_ps(out.as_mut_ptr(), res);
            Some(out)
        }
    }

    pub fn vec_fadd_f64(a: &[f64; 2], b: &[f64; 2]) -> Option<[f64; 2]> {
        if !is_x86_feature_detected!("sse2") {
            return None;
        }
        unsafe {
            let va = _mm_loadu_pd(a.as_ptr());
            let vb = _mm_loadu_pd(b.as_ptr());
            let res = _mm_add_pd(va, vb);
            let mut out = [0f64; 2];
            _mm_storeu_pd(out.as_mut_ptr(), res);
            Some(out)
        }
    }

    pub fn vec_fsub_f64(a: &[f64; 2], b: &[f64; 2]) -> Option<[f64; 2]> {
        if !is_x86_feature_detected!("sse2") {
            return None;
        }
        unsafe {
            let va = _mm_loadu_pd(a.as_ptr());
            let vb = _mm_loadu_pd(b.as_ptr());
            let res = _mm_sub_pd(va, vb);
            let mut out = [0f64; 2];
            _mm_storeu_pd(out.as_mut_ptr(), res);
            Some(out)
        }
    }

    pub fn vec_fmul_f64(a: &[f64; 2], b: &[f64; 2]) -> Option<[f64; 2]> {
        if !is_x86_feature_detected!("sse2") {
            return None;
        }
        unsafe {
            let va = _mm_loadu_pd(a.as_ptr());
            let vb = _mm_loadu_pd(b.as_ptr());
            let res = _mm_mul_pd(va, vb);
            let mut out = [0f64; 2];
            _mm_storeu_pd(out.as_mut_ptr(), res);
            Some(out)
        }
    }

    pub fn vec_fdiv_f64(a: &[f64; 2], b: &[f64; 2]) -> Option<[f64; 2]> {
        if !is_x86_feature_detected!("sse2") {
            return None;
        }
        unsafe {
            let va = _mm_loadu_pd(a.as_ptr());
            let vb = _mm_loadu_pd(b.as_ptr());
            let res = _mm_div_pd(va, vb);
            let mut out = [0f64; 2];
            _mm_storeu_pd(out.as_mut_ptr(), res);
            Some(out)
        }
    }

    pub fn vec_fma_f32(a: &[f32; 4], b: &[f32; 4], c: &[f32; 4]) -> Option<[f32; 4]> {
        if !is_x86_feature_detected!("fma") {
            return None;
        }
        unsafe {
            let va = _mm_loadu_ps(a.as_ptr());
            let vb = _mm_loadu_ps(b.as_ptr());
            let vc = _mm_loadu_ps(c.as_ptr());
            let res = _mm_fmadd_ps(va, vb, vc);
            let mut out = [0f32; 4];
            _mm_storeu_ps(out.as_mut_ptr(), res);
            Some(out)
        }
    }

    pub fn vec_fma_f64(a: &[f64; 2], b: &[f64; 2], c: &[f64; 2]) -> Option<[f64; 2]> {
        if !is_x86_feature_detected!("fma") {
            return None;
        }
        unsafe {
            let va = _mm_loadu_pd(a.as_ptr());
            let vb = _mm_loadu_pd(b.as_ptr());
            let vc = _mm_loadu_pd(c.as_ptr());
            let res = _mm_fmadd_pd(va, vb, vc);
            let mut out = [0f64; 2];
            _mm_storeu_pd(out.as_mut_ptr(), res);
            Some(out)
        }
    }

    #[derive(Clone, Copy)]
    enum BinOp {
        Add,
        Sub,
    }

    #[derive(Clone, Copy)]
    enum SatOp {
        AddU,
        SubU,
    }

    /// 使用SSE2指令执行向量二元运算（加法或减法）
    ///
    /// # Safety
    ///
    /// 调用此函数必须满足以下条件：
    /// - CPU必须支持SSE2指令集（由`#[target_feature(enable = "sse2")]`保证）
    /// - `element_size`参数必须是1、2、4或8之一
    ///
    /// 违反这些条件将导致未定义行为（UB）。
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

    /// 使用SSE2指令执行饱和算术运算（无符号饱和加法或减法）
    ///
    /// # Safety
    ///
    /// 调用此函数必须满足以下条件：
    /// - CPU必须支持SSE2指令集（由`#[target_feature(enable = "sse2")]`保证）
    /// - `element_size`参数必须是1或2之一
    ///
    /// 违反这些条件将导致未定义行为（UB）。
    #[target_feature(enable = "sse2")]
    unsafe fn sse_sat_op(a: u64, b: u64, element_size: u8, op: SatOp) -> Option<u64> {
        let va = _mm_cvtsi64_si128(a as i64);
        let vb = _mm_cvtsi64_si128(b as i64);
        let res = match element_size {
            1 => match op {
                SatOp::AddU => _mm_adds_epu8(va, vb),
                SatOp::SubU => _mm_subs_epu8(va, vb),
            },
            2 => match op {
                SatOp::AddU => _mm_adds_epu16(va, vb),
                SatOp::SubU => _mm_subs_epu16(va, vb),
            },
            _ => return None,
        };
        Some(_mm_cvtsi128_si64(res) as u64)
    }

    /// 使用SSE2指令执行逐元素最小值或最大值计算（无符号）
    ///
    /// # Safety
    ///
    /// 调用此函数必须满足以下条件：
    /// - CPU必须支持SSE2指令集（由`#[target_feature(enable = "sse2")]`保证）
    /// - `element_size`参数必须是1
    ///
    /// 违反这些条件将导致未定义行为（UB）。
    #[target_feature(enable = "sse2")]
    unsafe fn sse_minmax(a: u64, b: u64, element_size: u8, is_min: bool) -> Option<u64> {
        let va = _mm_cvtsi64_si128(a as i64);
        let vb = _mm_cvtsi64_si128(b as i64);
        let res = match element_size {
            1 => {
                if is_min {
                    _mm_min_epu8(va, vb)
                } else {
                    _mm_max_epu8(va, vb)
                }
            }
            _ => return None,
        };
        Some(_mm_cvtsi128_si64(res) as u64)
    }

    /// 使用SSE2指令执行逐元素乘法运算
    ///
    /// # Safety
    ///
    /// 调用此函数必须满足以下条件：
    /// - CPU必须支持SSE2指令集（由`#[target_feature(enable = "sse2")]`保证）
    /// - `element_size`参数必须是2或4之一
    ///
    /// 违反这些条件将导致未定义行为（UB）。
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

    /// 使用AVX2指令执行256位向量二元运算（加法或减法）
    ///
    /// # Safety
    ///
    /// 调用此函数必须满足以下条件：
    /// - CPU必须支持AVX2指令集（由`#[target_feature(enable = "avx2")]`保证）
    /// - `element_size`参数必须是1、2、4或8之一
    /// - `a`和`b`必须是指向有效内存区域的指针，至少包含4个u64元素（32字节）
    ///
    /// 违反这些条件将导致未定义行为（UB）。
    #[target_feature(enable = "avx2")]
    unsafe fn avx_binop(
        a: &[u64; 4],
        b: &[u64; 4],
        element_size: u8,
        op: BinOp,
    ) -> Option<[u64; 4]> {
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

    /// 使用AVX2指令执行256位向量乘法运算
    ///
    /// # Safety
    ///
    /// 调用此函数必须满足以下条件：
    /// - CPU必须支持AVX2指令集（由`#[target_feature(enable = "avx2")]`保证）
    /// - `element_size`参数必须是2或4之一
    /// - `a`和`b`必须是指向有效内存区域的指针，至少包含4个u64元素（32字节）
    ///
    /// 违反这些条件将导致未定义行为（UB）。
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

    /// 使用两个AVX2指令模拟512位向量二元运算（加法或减法）
    ///
    /// # Safety
    ///
    /// 调用此函数必须满足以下条件：
    /// - CPU必须支持AVX2指令集（由`#[target_feature(enable = "avx2")]`保证）
    /// - `element_size`参数必须是1、2、4或8之一
    /// - `a`和`b`必须是指向有效内存区域的指针，至少包含8个u64元素（64字节）
    ///
    /// 违反这些条件将导致未定义行为（UB）。
    ///
    /// 注意：真正的AVX-512实现需要使用内联汇编或第三方库
    #[target_feature(enable = "avx2")]
    unsafe fn avx512_binop(
        a: &[u64; 8],
        b: &[u64; 8],
        element_size: u8,
        op: BinOp,
    ) -> Option<[u64; 8]> {
        // 将 512-bit 向量分成两个 256-bit 向量
        let lo_a = [a[0], a[1], a[2], a[3]];
        let lo_b = [b[0], b[1], b[2], b[3]];
        let hi_a = [a[4], a[5], a[6], a[7]];
        let hi_b = [b[4], b[5], b[6], b[7]];

        // 使用 AVX2 处理低 256 位和高 256 位
        let lo = avx_binop(&lo_a, &lo_b, element_size, op)?;
        let hi = avx_binop(&hi_a, &hi_b, element_size, op)?;
        Some([lo[0], lo[1], lo[2], lo[3], hi[0], hi[1], hi[2], hi[3]])
    }

    /// 使用两个AVX2指令模拟512位向量乘法运算
    ///
    /// # Safety
    ///
    /// 调用此函数必须满足以下条件：
    /// - CPU必须支持AVX2指令集（由`#[target_feature(enable = "avx2")]`保证）
    /// - `element_size`参数必须是2或4之一
    /// - `a`和`b`必须是指向有效内存区域的指针，至少包含8个u64元素（64字节）
    ///
    /// 违反这些条件将导致未定义行为（UB）。
    #[target_feature(enable = "avx2")]
    unsafe fn avx512_mul(a: &[u64; 8], b: &[u64; 8], element_size: u8) -> Option<[u64; 8]> {
        let lo_a = [a[0], a[1], a[2], a[3]];
        let lo_b = [b[0], b[1], b[2], b[3]];
        let hi_a = [a[4], a[5], a[6], a[7]];
        let hi_b = [b[4], b[5], b[6], b[7]];

        let lo = avx_mul(&lo_a, &lo_b, element_size)?;
        let hi = avx_mul(&hi_a, &hi_b, element_size)?;
        Some([lo[0], lo[1], lo[2], lo[3], hi[0], hi[1], hi[2], hi[3]])
    }
}

#[cfg(target_arch = "aarch64")]
mod platform {
    use std::arch::aarch64::*;

    pub fn vec_add(a: u64, b: u64, element_size: u8) -> Option<u64> {
        unsafe { neon_binop(a, b, element_size, NeonOp::Add) }
    }

    pub fn vec_sub(a: u64, b: u64, element_size: u8) -> Option<u64> {
        unsafe { neon_binop(a, b, element_size, NeonOp::Sub) }
    }

    pub fn vec_mul(a: u64, b: u64, element_size: u8) -> Option<u64> {
        unsafe { neon_mul(a, b, element_size) }
    }

    pub fn vec_add_sat_u(a: u64, b: u64, element_size: u8) -> Option<u64> {
        unsafe { neon_sat_op(a, b, element_size, true, true) }
    }

    pub fn vec_sub_sat_u(a: u64, b: u64, element_size: u8) -> Option<u64> {
        unsafe { neon_sat_op(a, b, element_size, false, true) }
    }

    pub fn vec_min_u(a: u64, b: u64, element_size: u8) -> Option<u64> {
        unsafe { neon_minmax(a, b, element_size, true) }
    }

    pub fn vec_max_u(a: u64, b: u64, element_size: u8) -> Option<u64> {
        unsafe { neon_minmax(a, b, element_size, false) }
    }

    pub fn vec256_add(a: &[u64; 4], b: &[u64; 4], element_size: u8) -> Option<[u64; 4]> {
        // Process as two 128-bit operations
        let lo_a = [a[0], a[1]];
        let lo_b = [b[0], b[1]];
        let hi_a = [a[2], a[3]];
        let hi_b = [b[2], b[3]];

        unsafe {
            let lo = neon_128_binop(&lo_a, &lo_b, element_size, NeonOp::Add)?;
            let hi = neon_128_binop(&hi_a, &hi_b, element_size, NeonOp::Add)?;
            Some([lo[0], lo[1], hi[0], hi[1]])
        }
    }

    pub fn vec256_sub(a: &[u64; 4], b: &[u64; 4], element_size: u8) -> Option<[u64; 4]> {
        let lo_a = [a[0], a[1]];
        let lo_b = [b[0], b[1]];
        let hi_a = [a[2], a[3]];
        let hi_b = [b[2], b[3]];

        unsafe {
            let lo = neon_128_binop(&lo_a, &lo_b, element_size, NeonOp::Sub)?;
            let hi = neon_128_binop(&hi_a, &hi_b, element_size, NeonOp::Sub)?;
            Some([lo[0], lo[1], hi[0], hi[1]])
        }
    }

    pub fn vec256_mul(a: &[u64; 4], b: &[u64; 4], element_size: u8) -> Option<[u64; 4]> {
        let lo_a = [a[0], a[1]];
        let lo_b = [b[0], b[1]];
        let hi_a = [a[2], a[3]];
        let hi_b = [b[2], b[3]];

        unsafe {
            let lo = neon_128_mul(&lo_a, &lo_b, element_size)?;
            let hi = neon_128_mul(&hi_a, &hi_b, element_size)?;
            Some([lo[0], lo[1], hi[0], hi[1]])
        }
    }

    pub fn vec512_add(a: &[u64; 8], b: &[u64; 8], element_size: u8) -> Option<[u64; 8]> {
        // SVE 支持可变长度向量，但我们可以使用多个 NEON 128-bit 操作来模拟
        // 将 512-bit 分成 4 个 128-bit 向量
        unsafe {
            let v0 = neon_128_binop(&[a[0], a[1]], &[b[0], b[1]], element_size, NeonOp::Add)?;
            let v1 = neon_128_binop(&[a[2], a[3]], &[b[2], b[3]], element_size, NeonOp::Add)?;
            let v2 = neon_128_binop(&[a[4], a[5]], &[b[4], b[5]], element_size, NeonOp::Add)?;
            let v3 = neon_128_binop(&[a[6], a[7]], &[b[6], b[7]], element_size, NeonOp::Add)?;
            Some([v0[0], v0[1], v1[0], v1[1], v2[0], v2[1], v3[0], v3[1]])
        }
    }

    pub fn vec512_sub(a: &[u64; 8], b: &[u64; 8], element_size: u8) -> Option<[u64; 8]> {
        unsafe {
            let v0 = neon_128_binop(&[a[0], a[1]], &[b[0], b[1]], element_size, NeonOp::Sub)?;
            let v1 = neon_128_binop(&[a[2], a[3]], &[b[2], b[3]], element_size, NeonOp::Sub)?;
            let v2 = neon_128_binop(&[a[4], a[5]], &[b[4], b[5]], element_size, NeonOp::Sub)?;
            let v3 = neon_128_binop(&[a[6], a[7]], &[b[6], b[7]], element_size, NeonOp::Sub)?;
            Some([v0[0], v0[1], v1[0], v1[1], v2[0], v2[1], v3[0], v3[1]])
        }
    }

    pub fn vec512_mul(a: &[u64; 8], b: &[u64; 8], element_size: u8) -> Option<[u64; 8]> {
        unsafe {
            let v0 = neon_128_mul(&[a[0], a[1]], &[b[0], b[1]], element_size)?;
            let v1 = neon_128_mul(&[a[2], a[3]], &[b[2], b[3]], element_size)?;
            let v2 = neon_128_mul(&[a[4], a[5]], &[b[4], b[5]], element_size)?;
            let v3 = neon_128_mul(&[a[6], a[7]], &[b[6], b[7]], element_size)?;
            Some([v0[0], v0[1], v1[0], v1[1], v2[0], v2[1], v3[0], v3[1]])
        }
    }

    // 浮点向量操作
    pub fn vec_fadd_f32(a: &[f32; 4], b: &[f32; 4]) -> Option<[f32; 4]> {
        unsafe {
            let va = vld1q_f32(a.as_ptr());
            let vb = vld1q_f32(b.as_ptr());
            let res = vaddq_f32(va, vb);
            let mut out = [0f32; 4];
            vst1q_f32(out.as_mut_ptr(), res);
            Some(out)
        }
    }

    pub fn vec_fsub_f32(a: &[f32; 4], b: &[f32; 4]) -> Option<[f32; 4]> {
        unsafe {
            let va = vld1q_f32(a.as_ptr());
            let vb = vld1q_f32(b.as_ptr());
            let res = vsubq_f32(va, vb);
            let mut out = [0f32; 4];
            vst1q_f32(out.as_mut_ptr(), res);
            Some(out)
        }
    }

    pub fn vec_fmul_f32(a: &[f32; 4], b: &[f32; 4]) -> Option<[f32; 4]> {
        unsafe {
            let va = vld1q_f32(a.as_ptr());
            let vb = vld1q_f32(b.as_ptr());
            let res = vmulq_f32(va, vb);
            let mut out = [0f32; 4];
            vst1q_f32(out.as_mut_ptr(), res);
            Some(out)
        }
    }

    pub fn vec_fdiv_f32(a: &[f32; 4], b: &[f32; 4]) -> Option<[f32; 4]> {
        unsafe {
            let va = vld1q_f32(a.as_ptr());
            let vb = vld1q_f32(b.as_ptr());
            let res = vdivq_f32(va, vb);
            let mut out = [0f32; 4];
            vst1q_f32(out.as_mut_ptr(), res);
            Some(out)
        }
    }

    pub fn vec_fadd_f64(a: &[f64; 2], b: &[f64; 2]) -> Option<[f64; 2]> {
        unsafe {
            let va = vld1q_f64(a.as_ptr());
            let vb = vld1q_f64(b.as_ptr());
            let res = vaddq_f64(va, vb);
            let mut out = [0f64; 2];
            vst1q_f64(out.as_mut_ptr(), res);
            Some(out)
        }
    }

    pub fn vec_fsub_f64(a: &[f64; 2], b: &[f64; 2]) -> Option<[f64; 2]> {
        unsafe {
            let va = vld1q_f64(a.as_ptr());
            let vb = vld1q_f64(b.as_ptr());
            let res = vsubq_f64(va, vb);
            let mut out = [0f64; 2];
            vst1q_f64(out.as_mut_ptr(), res);
            Some(out)
        }
    }

    pub fn vec_fmul_f64(a: &[f64; 2], b: &[f64; 2]) -> Option<[f64; 2]> {
        unsafe {
            let va = vld1q_f64(a.as_ptr());
            let vb = vld1q_f64(b.as_ptr());
            let res = vmulq_f64(va, vb);
            let mut out = [0f64; 2];
            vst1q_f64(out.as_mut_ptr(), res);
            Some(out)
        }
    }

    pub fn vec_fdiv_f64(a: &[f64; 2], b: &[f64; 2]) -> Option<[f64; 2]> {
        unsafe {
            let va = vld1q_f64(a.as_ptr());
            let vb = vld1q_f64(b.as_ptr());
            let res = vdivq_f64(va, vb);
            let mut out = [0f64; 2];
            vst1q_f64(out.as_mut_ptr(), res);
            Some(out)
        }
    }

    pub fn vec_fma_f32(a: &[f32; 4], b: &[f32; 4], c: &[f32; 4]) -> Option<[f32; 4]> {
        unsafe {
            let va = vld1q_f32(a.as_ptr());
            let vb = vld1q_f32(b.as_ptr());
            let vc = vld1q_f32(c.as_ptr());
            let res = vfmaq_f32(vc, va, vb);
            let mut out = [0f32; 4];
            vst1q_f32(out.as_mut_ptr(), res);
            Some(out)
        }
    }

    pub fn vec_fma_f64(a: &[f64; 2], b: &[f64; 2], c: &[f64; 2]) -> Option<[f64; 2]> {
        unsafe {
            let va = vld1q_f64(a.as_ptr());
            let vb = vld1q_f64(b.as_ptr());
            let vc = vld1q_f64(c.as_ptr());
            let res = vfmaq_f64(vc, va, vb);
            let mut out = [0f64; 2];
            vst1q_f64(out.as_mut_ptr(), res);
            Some(out)
        }
    }

    #[derive(Clone, Copy)]
    enum NeonOp {
        Add,
        Sub,
    }

    /// 使用NEON指令执行64位向量二元运算（加法或减法）
    ///
    /// # Safety
    ///
    /// 调用此函数必须满足以下条件：
    /// - CPU必须支持ARM NEON指令集
    /// - `element_size`参数必须是1、2、4或8之一
    ///
    /// 违反这些条件将导致未定义行为（UB）。
    unsafe fn neon_binop(a: u64, b: u64, element_size: u8, op: NeonOp) -> Option<u64> {
        match element_size {
            1 => unsafe {
                let va = vreinterpret_u8_u64(vdup_n_u64(a));
                let vb = vreinterpret_u8_u64(vdup_n_u64(b));
                let res = match op {
                    NeonOp::Add => vadd_u8(va, vb),
                    NeonOp::Sub => vsub_u8(va, vb),
                };
                Some(vget_lane_u64(vreinterpret_u64_u8(res), 0))
            },
            2 => unsafe {
                let va = vreinterpret_u16_u64(vdup_n_u64(a));
                let vb = vreinterpret_u16_u64(vdup_n_u64(b));
                let res = match op {
                    NeonOp::Add => vadd_u16(va, vb),
                    NeonOp::Sub => vsub_u16(va, vb),
                };
                Some(vget_lane_u64(vreinterpret_u64_u16(res), 0))
            },
            4 => unsafe {
                let va = vreinterpret_u32_u64(vdup_n_u64(a));
                let vb = vreinterpret_u32_u64(vdup_n_u64(b));
                let res = match op {
                    NeonOp::Add => vadd_u32(va, vb),
                    NeonOp::Sub => vsub_u32(va, vb),
                };
                Some(vget_lane_u64(vreinterpret_u64_u32(res), 0))
            },
            8 => match op {
                NeonOp::Add => Some(a.wrapping_add(b)),
                NeonOp::Sub => Some(a.wrapping_sub(b)),
            },
            _ => None,
        }
    }

    /// 使用NEON指令执行64位向量乘法运算
    ///
    /// # Safety
    ///
    /// 调用此函数必须满足以下条件：
    /// - CPU必须支持ARM NEON指令集
    /// - `element_size`参数必须是1、2或4之一
    ///
    /// 违反这些条件将导致未定义行为（UB）。
    unsafe fn neon_mul(a: u64, b: u64, element_size: u8) -> Option<u64> {
        match element_size {
            1 => unsafe {
                let va = vreinterpret_u8_u64(vdup_n_u64(a));
                let vb = vreinterpret_u8_u64(vdup_n_u64(b));
                let res = vmul_u8(va, vb);
                Some(vget_lane_u64(vreinterpret_u64_u8(res), 0))
            },
            2 => unsafe {
                let va = vreinterpret_u16_u64(vdup_n_u64(a));
                let vb = vreinterpret_u16_u64(vdup_n_u64(b));
                let res = vmul_u16(va, vb);
                Some(vget_lane_u64(vreinterpret_u64_u16(res), 0))
            },
            4 => unsafe {
                let va = vreinterpret_u32_u64(vdup_n_u64(a));
                let vb = vreinterpret_u32_u64(vdup_n_u64(b));
                let res = vmul_u32(va, vb);
                Some(vget_lane_u64(vreinterpret_u64_u32(res), 0))
            },
            _ => None,
        }
    }

    /// 使用NEON指令执行饱和算术运算（无符号饱和加法或减法）
    ///
    /// # Safety
    ///
    /// 调用此函数必须满足以下条件：
    /// - CPU必须支持ARM NEON指令集
    /// - `element_size`参数必须是1或2之一
    ///
    /// 违反这些条件将导致未定义行为（UB）。
    unsafe fn neon_sat_op(
        a: u64,
        b: u64,
        element_size: u8,
        is_add: bool,
        _is_unsigned: bool,
    ) -> Option<u64> {
        match element_size {
            1 => unsafe {
                let va = vreinterpret_u8_u64(vdup_n_u64(a));
                let vb = vreinterpret_u8_u64(vdup_n_u64(b));
                let res = if is_add {
                    vqadd_u8(va, vb)
                } else {
                    vqsub_u8(va, vb)
                };
                Some(vget_lane_u64(vreinterpret_u64_u8(res), 0))
            },
            2 => unsafe {
                let va = vreinterpret_u16_u64(vdup_n_u64(a));
                let vb = vreinterpret_u16_u64(vdup_n_u64(b));
                let res = if is_add {
                    vqadd_u16(va, vb)
                } else {
                    vqsub_u16(va, vb)
                };
                Some(vget_lane_u64(vreinterpret_u64_u16(res), 0))
            },
            _ => None,
        }
    }

    unsafe fn neon_minmax(a: u64, b: u64, element_size: u8, is_min: bool) -> Option<u64> {
        match element_size {
            1 => unsafe {
                let va = vreinterpret_u8_u64(vdup_n_u64(a));
                let vb = vreinterpret_u8_u64(vdup_n_u64(b));
                let res = if is_min {
                    vmin_u8(va, vb)
                } else {
                    vmax_u8(va, vb)
                };
                Some(vget_lane_u64(vreinterpret_u64_u8(res), 0))
            },
            2 => unsafe {
                let va = vreinterpret_u16_u64(vdup_n_u64(a));
                let vb = vreinterpret_u16_u64(vdup_n_u64(b));
                let res = if is_min {
                    vmin_u16(va, vb)
                } else {
                    vmax_u16(va, vb)
                };
                Some(vget_lane_u64(vreinterpret_u64_u16(res), 0))
            },
            4 => unsafe {
                let va = vreinterpret_u32_u64(vdup_n_u64(a));
                let vb = vreinterpret_u32_u64(vdup_n_u64(b));
                let res = if is_min {
                    vmin_u32(va, vb)
                } else {
                    vmax_u32(va, vb)
                };
                Some(vget_lane_u64(vreinterpret_u64_u32(res), 0))
            },
            _ => None,
        }
    }

    unsafe fn neon_128_binop(
        a: &[u64; 2],
        b: &[u64; 2],
        element_size: u8,
        op: NeonOp,
    ) -> Option<[u64; 2]> {
        match element_size {
            1 => unsafe {
                let va = vld1q_u8(a.as_ptr() as *const u8);
                let vb = vld1q_u8(b.as_ptr() as *const u8);
                let res = match op {
                    NeonOp::Add => vaddq_u8(va, vb),
                    NeonOp::Sub => vsubq_u8(va, vb),
                };
                let mut out = [0u64; 2];
                vst1q_u8(out.as_mut_ptr() as *mut u8, res);
                Some(out)
            },
            2 => unsafe {
                let va = vld1q_u16(a.as_ptr() as *const u16);
                let vb = vld1q_u16(b.as_ptr() as *const u16);
                let res = match op {
                    NeonOp::Add => vaddq_u16(va, vb),
                    NeonOp::Sub => vsubq_u16(va, vb),
                };
                let mut out = [0u64; 2];
                vst1q_u16(out.as_mut_ptr() as *mut u16, res);
                Some(out)
            },
            4 => unsafe {
                let va = vld1q_u32(a.as_ptr() as *const u32);
                let vb = vld1q_u32(b.as_ptr() as *const u32);
                let res = match op {
                    NeonOp::Add => vaddq_u32(va, vb),
                    NeonOp::Sub => vsubq_u32(va, vb),
                };
                let mut out = [0u64; 2];
                vst1q_u32(out.as_mut_ptr() as *mut u32, res);
                Some(out)
            },
            8 => unsafe {
                let va = vld1q_u64(a.as_ptr());
                let vb = vld1q_u64(b.as_ptr());
                let res = match op {
                    NeonOp::Add => vaddq_u64(va, vb),
                    NeonOp::Sub => vsubq_u64(va, vb),
                };
                let mut out = [0u64; 2];
                vst1q_u64(out.as_mut_ptr(), res);
                Some(out)
            },
            _ => None,
        }
    }

    unsafe fn neon_128_mul(a: &[u64; 2], b: &[u64; 2], element_size: u8) -> Option<[u64; 2]> {
        match element_size {
            1 => unsafe {
                let va = vld1q_u8(a.as_ptr() as *const u8);
                let vb = vld1q_u8(b.as_ptr() as *const u8);
                let res = vmulq_u8(va, vb);
                let mut out = [0u64; 2];
                vst1q_u8(out.as_mut_ptr() as *mut u8, res);
                Some(out)
            },
            2 => unsafe {
                let va = vld1q_u16(a.as_ptr() as *const u16);
                let vb = vld1q_u16(b.as_ptr() as *const u16);
                let res = vmulq_u16(va, vb);
                let mut out = [0u64; 2];
                vst1q_u16(out.as_mut_ptr() as *mut u16, res);
                Some(out)
            },
            4 => unsafe {
                let va = vld1q_u32(a.as_ptr() as *const u32);
                let vb = vld1q_u32(b.as_ptr() as *const u32);
                let res = vmulq_u32(va, vb);
                let mut out = [0u64; 2];
                vst1q_u32(out.as_mut_ptr() as *mut u32, res);
                Some(out)
            },
            _ => None,
        }
    }
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
mod platform {
    pub fn vec_add(_a: u64, _b: u64, _element_size: u8) -> Option<u64> {
        None
    }
    pub fn vec_sub(_a: u64, _b: u64, _element_size: u8) -> Option<u64> {
        None
    }
    pub fn vec_mul(_a: u64, _b: u64, _element_size: u8) -> Option<u64> {
        None
    }
    pub fn vec_add_sat_u(_a: u64, _b: u64, _element_size: u8) -> Option<u64> {
        None
    }
    pub fn vec_sub_sat_u(_a: u64, _b: u64, _element_size: u8) -> Option<u64> {
        None
    }
    pub fn vec_min_u(_a: u64, _b: u64, _element_size: u8) -> Option<u64> {
        None
    }
    pub fn vec_max_u(_a: u64, _b: u64, _element_size: u8) -> Option<u64> {
        None
    }
    pub fn vec256_add(_a: &[u64; 4], _b: &[u64; 4], _element_size: u8) -> Option<[u64; 4]> {
        None
    }
    pub fn vec256_sub(_a: &[u64; 4], _b: &[u64; 4], _element_size: u8) -> Option<[u64; 4]> {
        None
    }
    pub fn vec256_mul(_a: &[u64; 4], _b: &[u64; 4], _element_size: u8) -> Option<[u64; 4]> {
        None
    }
    pub fn vec512_add(_a: &[u64; 8], _b: &[u64; 8], _element_size: u8) -> Option<[u64; 8]> {
        None
    }
    pub fn vec512_sub(_a: &[u64; 8], _b: &[u64; 8], _element_size: u8) -> Option<[u64; 8]> {
        None
    }
    pub fn vec512_mul(_a: &[u64; 8], _b: &[u64; 8], _element_size: u8) -> Option<[u64; 8]> {
        None
    }
    pub fn vec_fadd_f32(_a: &[f32; 4], _b: &[f32; 4]) -> Option<[f32; 4]> {
        None
    }
    pub fn vec_fsub_f32(_a: &[f32; 4], _b: &[f32; 4]) -> Option<[f32; 4]> {
        None
    }
    pub fn vec_fmul_f32(_a: &[f32; 4], _b: &[f32; 4]) -> Option<[f32; 4]> {
        None
    }
    pub fn vec_fdiv_f32(_a: &[f32; 4], _b: &[f32; 4]) -> Option<[f32; 4]> {
        None
    }
    pub fn vec_fadd_f64(_a: &[f64; 2], _b: &[f64; 2]) -> Option<[f64; 2]> {
        None
    }
    pub fn vec_fsub_f64(_a: &[f64; 2], _b: &[f64; 2]) -> Option<[f64; 2]> {
        None
    }
    pub fn vec_fmul_f64(_a: &[f64; 2], _b: &[f64; 2]) -> Option<[f64; 2]> {
        None
    }
    pub fn vec_fdiv_f64(_a: &[f64; 2], _b: &[f64; 2]) -> Option<[f64; 2]> {
        None
    }
    pub fn vec_fma_f32(_a: &[f32; 4], _b: &[f32; 4], _c: &[f32; 4]) -> Option<[f32; 4]> {
        None
    }
    pub fn vec_fma_f64(_a: &[f64; 2], _b: &[f64; 2], _c: &[f64; 2]) -> Option<[f64; 2]> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vec_add_u8() {
        let a = 0x0102030405060708u64;
        let b = 0x0101010101010101u64;
        let result = vec_add(a, b, 1);
        assert_eq!(result, 0x0203040506070809u64);
    }

    #[test]
    fn test_vec_add_sat_u8() {
        let a = 0xFF00FF00FF00FF00u64;
        let b = 0x0101010101010101u64;
        let result = vec_add_sat_u(a, b, 1);
        // 0xFF + 0x01 saturates to 0xFF, 0x00 + 0x01 = 0x01
        assert_eq!(result, 0xFF01FF01FF01FF01u64);
    }

    #[test]
    fn test_vec_fadd_f32() {
        let a = [1.0f32, 2.0, 3.0, 4.0];
        let b = [0.5f32, 0.5, 0.5, 0.5];
        let result = vec_fadd_f32(a, b);
        assert_eq!(result, [1.5f32, 2.5, 3.5, 4.5]);
    }

    #[test]
    fn test_vec_fma_f32() {
        let a = [2.0f32, 2.0, 2.0, 2.0];
        let b = [3.0f32, 3.0, 3.0, 3.0];
        let c = [1.0f32, 1.0, 1.0, 1.0];
        let result = vec_fma_f32(a, b, c);
        // 2*3+1 = 7
        assert_eq!(result, [7.0f32, 7.0, 7.0, 7.0]);
    }
}
