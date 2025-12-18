//! AVX512 指令实现
//!
//! 包括 ZMM 寄存器支持、掩码寄存器、压缩/解压缩等

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

/// AVX512 掩码寄存器 (k1-k7)
/// 每个掩码寄存器是 8 位，可以控制 8 个元素的掩码
#[derive(Debug, Clone, Copy)]
pub struct Avx512Mask {
    mask: u8,
}

impl Avx512Mask {
    pub fn new(mask: u8) -> Self {
        Self { mask }
    }

    pub fn all_true() -> Self {
        Self { mask: 0xFF }
    }

    pub fn all_false() -> Self {
        Self { mask: 0x00 }
    }

    pub fn get(&self) -> u8 {
        self.mask
    }

    pub fn set(&mut self, mask: u8) {
        self.mask = mask;
    }

    /// 检查第 i 位是否设置 (i in 0..8)
    pub fn test(&self, i: usize) -> bool {
        (self.mask >> i) & 1 != 0
    }

    /// 设置第 i 位
    pub fn set_bit(&mut self, i: usize, value: bool) {
        if value {
            self.mask |= 1 << i;
        } else {
            self.mask &= !(1 << i);
        }
    }
}

/// AVX512 ZMM 寄存器 (512位)
/// 使用 8 个 u64 表示 512 位
pub type ZmmRegister = [u64; 8];

/// AVX512 单精度浮点向量加法 (VADDPS ZMM)
/// 512位向量，16个单精度浮点数
#[cfg(target_arch = "x86_64")]
pub unsafe fn vaddps_zmm(a: &[f32; 16], b: &[f32; 16], mask: Avx512Mask) -> Option<[f32; 16]> {
    if !is_x86_feature_detected!("avx512f") {
        // 回退到 AVX2: 分成两个 256 位操作
        return avx2_fallback_addps(a, b);
    }

    unsafe {
        // 注意: Rust 标准库对 AVX512 支持有限，这里使用内联汇编或回退实现
        // 实际实现需要使用内联汇编或第三方库如 stdarch
        avx2_fallback_addps(a, b)
    }
}

#[cfg(target_arch = "x86_64")]
unsafe fn avx2_fallback_addps(a: &[f32; 16], b: &[f32; 16]) -> Option<[f32; 16]> {
    if !is_x86_feature_detected!("avx") {
        return None;
    }

    unsafe {
        // 分成两个 256 位操作
        let lo_a = _mm256_loadu_ps(a.as_ptr());
        let lo_b = _mm256_loadu_ps(b.as_ptr());
        let hi_a = _mm256_loadu_ps(a.as_ptr().add(8));
        let hi_b = _mm256_loadu_ps(b.as_ptr().add(8));

        let lo_res = _mm256_add_ps(lo_a, lo_b);
        let hi_res = _mm256_add_ps(hi_a, hi_b);

        let mut out = [0f32; 16];
        _mm256_storeu_ps(out.as_mut_ptr(), lo_res);
        _mm256_storeu_ps(out.as_mut_ptr().add(8), hi_res);
        Some(out)
    }
}

/// AVX512 FMA (融合乘加) ZMM
/// res = a * b + c (16个单精度浮点数)
#[cfg(target_arch = "x86_64")]
pub unsafe fn vfma_zmm(
    a: &[f32; 16],
    b: &[f32; 16],
    c: &[f32; 16],
    mask: Avx512Mask,
) -> Option<[f32; 16]> {
    if !is_x86_feature_detected!("avx512f") {
        return avx2_fallback_fma(a, b, c);
    }

    unsafe {
        // 回退到 AVX2 FMA
        avx2_fallback_fma(a, b, c)
    }
}

#[cfg(target_arch = "x86_64")]
unsafe fn avx2_fallback_fma(a: &[f32; 16], b: &[f32; 16], c: &[f32; 16]) -> Option<[f32; 16]> {
    if !is_x86_feature_detected!("fma") {
        // 如果没有 FMA，使用乘加分离
        let mut out = [0f32; 16];
        for i in 0..16 {
            out[i] = a[i] * b[i] + c[i];
        }
        return Some(out);
    }

    unsafe {
        let lo_a = _mm256_loadu_ps(a.as_ptr());
        let lo_b = _mm256_loadu_ps(b.as_ptr());
        let lo_c = _mm256_loadu_ps(c.as_ptr());
        let hi_a = _mm256_loadu_ps(a.as_ptr().add(8));
        let hi_b = _mm256_loadu_ps(b.as_ptr().add(8));
        let hi_c = _mm256_loadu_ps(c.as_ptr().add(8));

        let lo_res = _mm256_fmadd_ps(lo_a, lo_b, lo_c);
        let hi_res = _mm256_fmadd_ps(hi_a, hi_b, hi_c);

        let mut out = [0f32; 16];
        _mm256_storeu_ps(out.as_mut_ptr(), lo_res);
        _mm256_storeu_ps(out.as_mut_ptr().add(8), hi_res);
        Some(out)
    }
}

/// AVX512 排列单精度浮点 (VPERMPS ZMM)
/// 根据索引向量重新排列元素
#[cfg(target_arch = "x86_64")]
pub unsafe fn vpermps_zmm(
    a: &[f32; 16],
    indices: &[u32; 16],
    mask: Avx512Mask,
) -> Option<[f32; 16]> {
    if !is_x86_feature_detected!("avx512f") {
        return avx2_fallback_permps(a, indices);
    }

    unsafe {
        // 回退到 AVX2
        avx2_fallback_permps(a, indices)
    }
}

#[cfg(target_arch = "x86_64")]
unsafe fn avx2_fallback_permps(a: &[f32; 16], indices: &[u32; 16]) -> Option<[f32; 16]> {
    unsafe {
        let mut out = [0f32; 16];
        for i in 0..16 {
            let idx = (indices[i] & 0xF) as usize; // 只取低4位
            out[i] = a[idx];
        }
        Some(out)
    }
}

/// AVX512 压缩指令 (压缩掩码为字节)
/// 将掩码寄存器的8位压缩为字节
pub fn compress_mask(mask: Avx512Mask) -> u8 {
    mask.get()
}

/// AVX512 解压缩指令 (从字节解压缩为掩码)
pub fn decompress_mask(byte: u8) -> Avx512Mask {
    Avx512Mask::new(byte)
}

#[cfg(not(target_arch = "x86_64"))]
mod fallback {
    use super::Avx512Mask;

    pub unsafe fn vaddps_zmm(
        _a: &[f32; 16],
        _b: &[f32; 16],
        _mask: Avx512Mask,
    ) -> Option<[f32; 16]> {
        None
    }
    pub unsafe fn vfma_zmm(
        _a: &[f32; 16],
        _b: &[f32; 16],
        _c: &[f32; 16],
        _mask: Avx512Mask,
    ) -> Option<[f32; 16]> {
        None
    }
    pub unsafe fn vpermps_zmm(
        _a: &[f32; 16],
        _indices: &[u32; 16],
        _mask: Avx512Mask,
    ) -> Option<[f32; 16]> {
        None
    }
}

#[cfg(not(target_arch = "x86_64"))]
pub use fallback::*;
