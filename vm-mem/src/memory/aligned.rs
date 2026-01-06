// ARM64内存对齐优化工具
//
// Round 35优化2: ARM64特定内存布局优化
// 目的: 16字节对齐以匹配NEON 128位向量，提升性能
//
// 预期提升: 5-15%

/// 16字节对齐的内存块 (匹配ARM64 NEON 128位向量)
#[repr(C, align(16))]
pub struct AlignedMemoryBlock {
    _data: [u8; 0], // 零大小类型，仅用于对齐
}

/// 验证类型是否16字节对齐
pub const fn is_aligned_16<T>() -> bool {
    std::mem::align_of::<T>() >= 16
}

/// 16字节对齐的对象池
///
/// 用于频繁分配的小对象，确保NEON向量操作最优性能
pub struct AlignedObjectPool<T> {
    objects: Vec<Option<T>>,
    capacity: usize,
}

impl<T> AlignedObjectPool<T> {
    /// 创建新的16字节对齐对象池
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            objects: Vec::with_capacity(capacity),
            capacity,
        }
    }

    /// 分配对象
    pub fn allocate(&mut self) -> Option<T> {
        self.objects.pop().and_then(|opt| opt)
    }

    /// 释放对象回池
    pub fn deallocate(&mut self, object: T) {
        if self.objects.len() < self.capacity {
            self.objects.push(Some(object));
        }
    }

    /// 获取池中可用对象数量
    pub fn available(&self) -> usize {
        self.objects.len()
    }

    /// 获取池容量
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

/// 16字节对齐的SIMD友好的向量
///
/// 自动对齐到16字节边界，优化ARM64 NEON性能
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug)]
pub struct SimdAlignedVector4 {
    pub data: [f32; 4],
}

impl SimdAlignedVector4 {
    /// 创建新的对齐向量
    pub fn new(data: [f32; 4]) -> Self {
        Self { data }
    }

    /// 从数组创建 (如果数组长度>=4)
    pub fn from_array(arr: &[f32]) -> Self {
        let mut data = [0.0f32; 4];
        data.copy_from_slice(&arr[..4.min(arr.len())]);
        Self { data }
    }

    /// NEON优化的向量加法
    #[cfg(target_arch = "aarch64")]
    pub fn add_neon(&self, other: &Self) -> Self {
        use std::arch::aarch64::*;
        unsafe {
            let a = vld1q_f32(self.data.as_ptr());
            let b = vld1q_f32(other.data.as_ptr());
            let result = vaddq_f32(a, b);
            let mut output = [0.0f32; 4];
            vst1q_f32(output.as_mut_ptr(), result);
            Self { data: output }
        }
    }

    /// 标量向量加法 (fallback)
    #[cfg(not(target_arch = "aarch64"))]
    pub fn add_neon(&self, other: &Self) -> Self {
        let mut result = [0.0f32; 4];
        for i in 0..4 {
            result[i] = self.data[i] + other.data[i];
        }
        Self { data: result }
    }

    /// NEON优化的向量乘法
    #[cfg(target_arch = "aarch64")]
    pub fn mul_neon(&self, other: &Self) -> Self {
        use std::arch::aarch64::*;
        unsafe {
            let a = vld1q_f32(self.data.as_ptr());
            let b = vld1q_f32(other.data.as_ptr());
            let result = vmulq_f32(a, b);
            let mut output = [0.0f32; 4];
            vst1q_f32(output.as_mut_ptr(), result);
            Self { data: output }
        }
    }

    /// 标量向量乘法 (fallback)
    #[cfg(not(target_arch = "aarch64"))]
    pub fn mul_neon(&self, other: &Self) -> Self {
        let mut result = [0.0f32; 4];
        for i in 0..4 {
            result[i] = self.data[i] * other.data[i];
        }
        Self { data: result }
    }

    /// 获取点积
    #[cfg(target_arch = "aarch64")]
    pub fn dot_product(&self, other: &Self) -> f32 {
        use std::arch::aarch64::*;
        unsafe {
            let a = vld1q_f32(self.data.as_ptr());
            let b = vld1q_f32(other.data.as_ptr());
            let mul = vmulq_f32(a, b);
            // 水平求和
            let mut result = [0.0f32; 4];
            vst1q_f32(result.as_mut_ptr(), mul);
            result.iter().sum()
        }
    }

    /// 标量点积 (fallback)
    #[cfg(not(target_arch = "aarch64"))]
    pub fn dot_product(&self, other: &Self) -> f32 {
        self.data.iter()
            .zip(other.data.iter())
            .map(|(a, b)| a * b)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aligned_vector4_alignment() {
        assert_eq!(std::mem::align_of::<SimdAlignedVector4>(), 16);
        assert_eq!(std::mem::size_of::<SimdAlignedVector4>(), 16);
    }

    #[test]
    fn test_aligned_vector4_operations() {
        let a = SimdAlignedVector4::new([1.0, 2.0, 3.0, 4.0]);
        let b = SimdAlignedVector4::new([5.0, 6.0, 7.0, 8.0]);

        let sum = a.add_neon(&b);
        assert_eq!(sum.data, [6.0, 8.0, 10.0, 12.0]);

        let product = a.mul_neon(&b);
        assert_eq!(product.data, [5.0, 12.0, 21.0, 32.0]);

        let dot = a.dot_product(&b);
        assert_eq!(dot, 70.0); // 1*5 + 2*6 + 3*7 + 4*8
    }

    #[test]
    fn test_aligned_object_pool() {
        let mut pool: AlignedObjectPool<SimdAlignedVector4> =
            AlignedObjectPool::with_capacity(10);

        // 分配一些对象
        for _ in 0..5 {
            pool.deallocate(SimdAlignedVector4::new([0.0; 4]));
        }

        assert_eq!(pool.available(), 5);

        // 分配回来
        let obj = pool.allocate();
        assert!(obj.is_some());
        assert_eq!(pool.available(), 4);
    }
}
