//! ARM NEON 高级指令实现
//!
//! 包括向量转换、排列等高级 NEON 指令

#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::*;

/// NEON 向量转换：XTN (Extract Narrow)
/// 将 128 位向量的高 64 位提取并截断为 64 位
///
/// # Safety
/// This function uses NEON intrinsics and is only safe to call on AArch64 targets
/// with NEON support. The caller must ensure the target architecture supports these
/// instructions.
#[cfg(target_arch = "aarch64")]
pub unsafe fn xtn_u8(a: u128) -> Option<u64> {
    unsafe {
        let va = vreinterpretq_u8_u64(vdupq_n_u64(a as u64));
        let v16 = vreinterpretq_u16_u8(va);
        let narrow = vmovn_u16(v16);
        Some(vget_lane_u64(vreinterpret_u64_u8(narrow), 0))
    }
}

/// NEON 向量转换：XTN2 (Extract Narrow, high half)
/// 将 128 位向量的高 64 位提取并截断，与低 64 位组合
///
/// # Safety
/// This function uses NEON intrinsics and is only safe to call on AArch64 targets
/// with NEON support. The caller must ensure the target architecture supports these
/// instructions.
#[cfg(target_arch = "aarch64")]
pub unsafe fn xtn2_u8(low: u64, high: u128) -> Option<u128> {
    unsafe {
        let vlow = vreinterpret_u8_u64(vdup_n_u64(low));
        let vhigh = vreinterpretq_u8_u64(vdupq_n_u64(high as u64));
        let narrow = vmovn_u16(vreinterpretq_u16_u8(vhigh));
        let combined = vcombine_u8(vlow, narrow);
        Some(vgetq_lane_u64(vreinterpretq_u64_u8(combined), 0) as u128)
    }
}

/// NEON 向量转换：UXTN (Unsigned Extract Narrow)
/// 无符号截断提取
///
/// # Safety
/// This function uses NEON intrinsics and is only safe to call on AArch64 targets
/// with NEON support. The caller must ensure the target architecture supports these
/// instructions.
#[cfg(target_arch = "aarch64")]
pub unsafe fn uxtn_u8(a: u128) -> Option<u64> {
    unsafe {
        let va = vreinterpretq_u16_u64(vdupq_n_u64(a as u64));
        let res = vmovn_u16(va);
        Some(vget_lane_u64(vreinterpret_u64_u8(res), 0))
    }
}

/// NEON 向量转换：UXTN2 (Unsigned Extract Narrow, high half)
///
/// # Safety
/// This function uses NEON intrinsics and is only safe to call on AArch64 targets
/// with NEON support. The caller must ensure the target architecture supports these
/// instructions.
#[cfg(target_arch = "aarch64")]
pub unsafe fn uxtn2_u8(low: u64, high: u128) -> Option<u128> {
    unsafe {
        let vlow = vreinterpret_u8_u64(vdup_n_u64(low));
        let vhigh = vreinterpretq_u16_u64(vdupq_n_u64(high as u64));
        let narrow = vmovn_u16(vhigh);
        let combined = vcombine_u8(vlow, narrow);
        Some(vgetq_lane_u64(vreinterpretq_u64_u8(combined), 0) as u128)
    }
}

/// NEON 表查找：TBL (Table Lookup)
/// 使用索引向量从查找表中提取元素
///
/// # Safety
/// This function uses NEON intrinsics and is only safe to call on AArch64 targets
/// with NEON support. The caller must ensure the target architecture supports these
/// instructions, and that the provided pointers are valid.
#[cfg(target_arch = "aarch64")]
pub unsafe fn tbl_u8(table: &[u8; 16], indices: &[u8; 16]) -> Option<[u8; 16]> {
    unsafe {
        let vtable = vld1q_u8(table.as_ptr());
        let vidx = vld1q_u8(indices.as_ptr());
        let res = vqtbl1q_u8(vtable, vidx);
        let mut out = [0u8; 16];
        vst1q_u8(out.as_mut_ptr(), res);
        Some(out)
    }
}

/// NEON 表查找扩展：TBX (Table Lookup Extended)
/// 类似于 TBL，但保留未匹配的元素
///
/// # Safety
/// This function uses NEON intrinsics and is only safe to call on AArch64 targets
/// with NEON support. The caller must ensure the target architecture supports these
/// instructions, and that the provided pointers are valid.
#[cfg(target_arch = "aarch64")]
pub unsafe fn tbx_u8(table: &[u8; 16], indices: &[u8; 16], default: &[u8; 16]) -> Option<[u8; 16]> {
    unsafe {
        let vtable = vld1q_u8(table.as_ptr());
        let vidx = vld1q_u8(indices.as_ptr());
        let vdefault = vld1q_u8(default.as_ptr());
        let res = vqtbx1q_u8(vdefault, vtable, vidx);
        let mut out = [0u8; 16];
        vst1q_u8(out.as_mut_ptr(), res);
        Some(out)
    }
}

/// NEON ZIP1 (Zip vectors, lower half)
/// 交错两个向量的低半部分
///
/// # Safety
/// This function uses NEON intrinsics and is only safe to call on AArch64 targets
/// with NEON support. The caller must ensure the target architecture supports these
/// instructions, and that the provided pointers are valid.
#[cfg(target_arch = "aarch64")]
pub unsafe fn zip1_u8(a: &[u8; 16], b: &[u8; 16]) -> Option<[u8; 16]> {
    unsafe {
        let va = vld1q_u8(a.as_ptr());
        let vb = vld1q_u8(b.as_ptr());
        let res = vzip1q_u8(va, vb);
        let mut out = [0u8; 16];
        vst1q_u8(out.as_mut_ptr(), res);
        Some(out)
    }
}

/// NEON ZIP2 (Zip vectors, upper half)
/// 交错两个向量的高半部分
///
/// # Safety
/// This function uses NEON intrinsics and is only safe to call on AArch64 targets
/// with NEON support. The caller must ensure the target architecture supports these
/// instructions, and that the provided pointers are valid.
#[cfg(target_arch = "aarch64")]
pub unsafe fn zip2_u8(a: &[u8; 16], b: &[u8; 16]) -> Option<[u8; 16]> {
    unsafe {
        let va = vld1q_u8(a.as_ptr());
        let vb = vld1q_u8(b.as_ptr());
        let res = vzip2q_u8(va, vb);
        let mut out = [0u8; 16];
        vst1q_u8(out.as_mut_ptr(), res);
        Some(out)
    }
}

/// NEON UZP1 (Unzip vectors, lower half)
/// 解交错两个向量，提取低半部分
///
/// # Safety
/// This function uses NEON intrinsics and is only safe to call on AArch64 targets
/// with NEON support. The caller must ensure the target architecture supports these
/// instructions, and that the provided pointers are valid.
#[cfg(target_arch = "aarch64")]
pub unsafe fn uzp1_u8(a: &[u8; 16], b: &[u8; 16]) -> Option<[u8; 16]> {
    unsafe {
        let va = vld1q_u8(a.as_ptr());
        let vb = vld1q_u8(b.as_ptr());
        let res = vuzp1q_u8(va, vb);
        let mut out = [0u8; 16];
        vst1q_u8(out.as_mut_ptr(), res);
        Some(out)
    }
}

/// NEON UZP2 (Unzip vectors, upper half)
/// 解交错两个向量，提取高半部分
///
/// # Safety
/// This function uses NEON intrinsics and is only safe to call on AArch64 targets
/// with NEON support. The caller must ensure the target architecture supports these
/// instructions, and that the provided pointers are valid.
#[cfg(target_arch = "aarch64")]
pub unsafe fn uzp2_u8(a: &[u8; 16], b: &[u8; 16]) -> Option<[u8; 16]> {
    unsafe {
        let va = vld1q_u8(a.as_ptr());
        let vb = vld1q_u8(b.as_ptr());
        let res = vuzp2q_u8(va, vb);
        let mut out = [0u8; 16];
        vst1q_u8(out.as_mut_ptr(), res);
        Some(out)
    }
}

#[cfg(not(target_arch = "aarch64"))]
mod fallback {
    /// NEON extract narrow (fallback implementation)
    ///
    /// # Safety
    /// This function is safe to call as it's a no-op fallback that returns None.
    pub unsafe fn xtn_u8(_a: u128) -> Option<u64> {
        None
    }

    /// NEON extract narrow high (fallback implementation)
    ///
    /// # Safety
    /// This function is safe to call as it's a no-op fallback that returns None.
    pub unsafe fn xtn2_u8(_low: u64, _high: u128) -> Option<u128> {
        None
    }

    /// NEON unsigned extract narrow (fallback implementation)
    ///
    /// # Safety
    /// This function is safe to call as it's a no-op fallback that returns None.
    pub unsafe fn uxtn_u8(_a: u128) -> Option<u64> {
        None
    }

    /// NEON unsigned extract narrow high (fallback implementation)
    ///
    /// # Safety
    /// This function is safe to call as it's a no-op fallback that returns None.
    pub unsafe fn uxtn2_u8(_low: u64, _high: u128) -> Option<u128> {
        None
    }

    /// NEON table lookup (fallback implementation)
    ///
    /// # Safety
    /// This function is safe to call as it's a no-op fallback that returns None.
    pub unsafe fn tbl_u8(_table: &[u8; 16], _indices: &[u8; 16]) -> Option<[u8; 16]> {
        None
    }

    /// NEON table lookup with default (fallback implementation)
    ///
    /// # Safety
    /// This function is safe to call as it's a no-op fallback that returns None.
    pub unsafe fn tbx_u8(
        _table: &[u8; 16],
        _indices: &[u8; 16],
        _default: &[u8; 16],
    ) -> Option<[u8; 16]> {
        None
    }
    pub unsafe fn zip1_u8(_a: &[u8; 16], _b: &[u8; 16]) -> Option<[u8; 16]> {
        None
    }
    pub unsafe fn zip2_u8(_a: &[u8; 16], _b: &[u8; 16]) -> Option<[u8; 16]> {
        None
    }
    pub unsafe fn uzp1_u8(_a: &[u8; 16], _b: &[u8; 16]) -> Option<[u8; 16]> {
        None
    }
    pub unsafe fn uzp2_u8(_a: &[u8; 16], _b: &[u8; 16]) -> Option<[u8; 16]> {
        None
    }
}

#[cfg(not(target_arch = "aarch64"))]
pub use fallback::*;
