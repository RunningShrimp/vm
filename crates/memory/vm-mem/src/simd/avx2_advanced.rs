//! AVX2 高级指令实现
//!
//! 包括可变移位、聚集加载等高级 AVX2 指令

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

/// AVX2 可变左移 (VPSLLVD)
/// 每个元素根据对应的移位量进行左移
#[cfg(target_arch = "x86_64")]
pub unsafe fn vpsllvd_256(a: &[u32; 8], shift: &[u32; 8]) -> Option<[u32; 8]> {
    if !is_x86_feature_detected!("avx2") {
        return None;
    }

    unsafe {
        let va = _mm256_loadu_si256(a.as_ptr() as *const __m256i);
        let vshift = _mm256_loadu_si256(shift.as_ptr() as *const __m256i);
        let res = _mm256_sllv_epi32(va, vshift);
        let mut out = [0u32; 8];
        _mm256_storeu_si256(out.as_mut_ptr() as *mut __m256i, res);
        Some(out)
    }
}

/// AVX2 可变右移 (VPSRLVD)
/// 每个元素根据对应的移位量进行逻辑右移
#[cfg(target_arch = "x86_64")]
pub unsafe fn vpsrlvd_256(a: &[u32; 8], shift: &[u32; 8]) -> Option<[u32; 8]> {
    if !is_x86_feature_detected!("avx2") {
        return None;
    }

    unsafe {
        let va = _mm256_loadu_si256(a.as_ptr() as *const __m256i);
        let vshift = _mm256_loadu_si256(shift.as_ptr() as *const __m256i);
        let res = _mm256_srlv_epi32(va, vshift);
        let mut out = [0u32; 8];
        _mm256_storeu_si256(out.as_mut_ptr() as *mut __m256i, res);
        Some(out)
    }
}

/// AVX2 聚集加载单精度浮点 (VGATHERDPS)
/// 从内存中根据索引向量聚集加载单精度浮点数
#[cfg(target_arch = "x86_64")]
pub unsafe fn vgatherdps_256(base: *const f32, indices: &[u32; 8], scale: i32) -> Option<[f32; 8]> {
    if !is_x86_feature_detected!("avx2") {
        return None;
    }

    unsafe {
        let vidx = _mm256_loadu_si256(indices.as_ptr() as *const __m256i);
        let mask = _mm256_set1_epi32(-1); // 全1掩码，加载所有元素
        let res = _mm256_mask_i32gather_ps(_mm256_setzero_ps(), base, vidx, mask, scale);
        let mut out = [0f32; 8];
        _mm256_storeu_ps(out.as_mut_ptr(), res);
        Some(out)
    }
}

/// AVX2 聚集加载双精度浮点 (VGATHERDPD)
#[cfg(target_arch = "x86_64")]
pub unsafe fn vgatherdpd_256(base: *const f64, indices: &[u32; 4], scale: i32) -> Option<[f64; 4]> {
    if !is_x86_feature_detected!("avx2") {
        return None;
    }

    unsafe {
        let vidx = _mm_loadu_si128(indices.as_ptr() as *const __m128i);
        let mask = _mm_set1_epi64x(-1);
        let res = _mm256_mask_i32gather_pd(
            _mm256_setzero_pd(),
            base,
            vidx,
            _mm256_castsi256_pd(_mm256_broadcastsi128_si256(mask)),
            scale,
        );
        let mut out = [0f64; 4];
        _mm256_storeu_pd(out.as_mut_ptr(), res);
        Some(out)
    }
}

/// AVX2 聚集加载双字 (VPGATHERDD)
#[cfg(target_arch = "x86_64")]
pub unsafe fn vpgatherdd_256(base: *const i32, indices: &[u32; 8], scale: i32) -> Option<[i32; 8]> {
    if !is_x86_feature_detected!("avx2") {
        return None;
    }

    unsafe {
        let vidx = _mm256_loadu_si256(indices.as_ptr() as *const __m256i);
        let mask = _mm256_set1_epi32(-1);
        let res = _mm256_mask_i32gather_epi32(_mm256_setzero_si256(), base, vidx, mask, scale);
        let mut out = [0i32; 8];
        _mm256_storeu_si256(out.as_mut_ptr() as *mut __m256i, res);
        Some(out)
    }
}

#[cfg(not(target_arch = "x86_64"))]
mod fallback {
    /// AVX2 可变左移的跨平台 fallback 实现
    ///
    /// # Safety
    ///
    /// 此函数在非 x86_64 平台上返回 `None`，不执行任何实际操作。
    /// 调用此函数是安全的，无需满足任何特定条件。
    pub unsafe fn vpsllvd_256(_a: &[u32; 8], _shift: &[u32; 8]) -> Option<[u32; 8]> {
        None
    }

    /// AVX2 可变右移的跨平台 fallback 实现
    ///
    /// # Safety
    ///
    /// 此函数在非 x86_64 平台上返回 `None`，不执行任何实际操作。
    /// 调用此函数是安全的，无需满足任何特定条件。
    pub unsafe fn vpsrlvd_256(_a: &[u32; 8], _shift: &[u32; 8]) -> Option<[u32; 8]> {
        None
    }

    /// AVX2 聚集加载单精度浮点的跨平台 fallback 实现
    ///
    /// # Safety
    ///
    /// 此函数在非 x86_64 平台上返回 `None`，不执行任何实际操作。
    /// 调用此函数是安全的，无需满足任何特定条件。
    pub unsafe fn vgatherdps_256(
        _base: *const f32,
        _indices: &[u32; 8],
        _scale: i32,
    ) -> Option<[f32; 8]> {
        None
    }

    /// AVX2 聚集加载双精度浮点的跨平台 fallback 实现
    ///
    /// # Safety
    ///
    /// 此函数在非 x86_64 平台上返回 `None`，不执行任何实际操作。
    /// 调用此函数是安全的，无需满足任何特定条件。
    pub unsafe fn vgatherdpd_256(
        _base: *const f64,
        _indices: &[u32; 4],
        _scale: i32,
    ) -> Option<[f64; 4]> {
        None
    }

    /// AVX2 聚集加载双字的跨平台 fallback 实现
    ///
    /// # Safety
    ///
    /// 此函数在非 x86_64 平台上返回 `None`，不执行任何实际操作。
    /// 调用此函数是安全的，无需满足任何特定条件。
    pub unsafe fn vpgatherdd_256(
        _base: *const i32,
        _indices: &[u32; 8],
        _scale: i32,
    ) -> Option<[i32; 8]> {
        None
    }
}

#[cfg(not(target_arch = "x86_64"))]
pub use fallback::*;
