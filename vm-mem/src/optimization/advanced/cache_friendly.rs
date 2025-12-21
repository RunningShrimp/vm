//! 缓存友好的内存操作优化
//!
//! 实现高效的内存拷贝和操作，优化缓存使用

use crate::{GuestAddr, VmError};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use vm_core::error::MemoryError;

/// 内存拷贝策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyStrategy {
    /// 逐字节拷贝
    ByteByByte,
    /// 逐字拷贝（8字节）
    WordByWord,
    /// SIMD拷贝（16字节）
    Simd128,
    /// SIMD拷贝（32字节）
    Simd256,
    /// SIMD拷贝（512字节）
    Simd512,
    /// 自适应选择最优策略
    Adaptive,
}

/// 内存对齐信息
#[derive(Debug, Clone, Copy)]
pub struct AlignmentInfo {
    /// 源地址对齐
    pub src_alignment: u64,
    /// 目标地址对齐
    pub dst_alignment: u64,
    /// 大小对齐
    pub size_alignment: u64,
    /// 是否对齐
    pub is_aligned: bool,
}

impl AlignmentInfo {
    /// 检查地址是否对齐到指定边界
    pub fn is_aligned_to(addr: u64, alignment: u64) -> bool {
        addr & (alignment - 1) == 0
    }

    /// 计算对齐信息
    pub fn new(src: u64, dst: u64, size: usize) -> Self {
        let size_u64 = size as u64;

        // 计算各部分的对齐
        let src_alignment = src & 0xFF; // 低8位，最大256字节对齐
        let dst_alignment = dst & 0xFF;
        let size_alignment = size_u64 & 0xFF;

        // 检查是否完全对齐
        let is_aligned = Self::is_aligned_to(src, 64)
            && Self::is_aligned_to(dst, 64)
            && Self::is_aligned_to(size_u64, 64);

        Self {
            src_alignment,
            dst_alignment,
            size_alignment,
            is_aligned,
        }
    }

    /// 获取最佳拷贝策略
    pub fn best_copy_strategy(&self) -> CopyStrategy {
        if self.is_aligned {
            // 完全对齐，可以使用最大的SIMD
            #[cfg(target_arch = "x86_64")]
            {
                if is_x86_feature_detected!("avx512f") {
                    return CopyStrategy::Simd512;
                } else if is_x86_feature_detected!("avx2") {
                    return CopyStrategy::Simd256;
                } else if is_x86_feature_detected!("sse2") {
                    return CopyStrategy::Simd128;
                }
            }

            #[cfg(target_arch = "aarch64")]
            {
                // ARM64 NEON支持128位SIMD
                CopyStrategy::Simd128
            }

            // 默认使用字拷贝
            #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
            CopyStrategy::WordByWord
        } else {
            // 不完全对齐，检查部分对齐
            if Self::is_aligned_to(self.src_alignment, 8)
                && Self::is_aligned_to(self.dst_alignment, 8)
            {
                CopyStrategy::WordByWord
            } else {
                CopyStrategy::ByteByByte
            }
        }
    }
}

/// 内存拷贝配置
#[derive(Debug, Clone)]
pub struct MemoryCopyConfig {
    /// 默认拷贝策略
    pub default_strategy: CopyStrategy,
    /// 小块大小阈值（小于此值使用字节拷贝）
    pub small_block_threshold: usize,
    /// 中等块大小阈值（小于此值使用字拷贝）
    pub medium_block_threshold: usize,
    /// 大块大小阈值（小于此值使用SIMD128）
    pub large_block_threshold: usize,
    /// 超大块大小阈值（小于此值使用SIMD256）
    pub xlarge_block_threshold: usize,
    /// 是否启用预取
    pub enable_prefetch: bool,
    /// 预取距离（字节）
    pub prefetch_distance: usize,
    /// 是否使用非临时存储（避免缓存污染）
    pub use_non_temporal: bool,
    /// 非临时存储阈值（大于此值使用非临时存储）
    pub non_temporal_threshold: usize,
}

impl Default for MemoryCopyConfig {
    fn default() -> Self {
        Self {
            default_strategy: CopyStrategy::Adaptive,
            small_block_threshold: 64,
            medium_block_threshold: 256,
            large_block_threshold: 2048,
            xlarge_block_threshold: 16384,
            enable_prefetch: true,
            prefetch_distance: 64,
            use_non_temporal: true,
            non_temporal_threshold: 65536, // 64KB
        }
    }
}

/// 内存拷贝统计信息
#[derive(Debug, Default)]
pub struct MemoryCopyStats {
    /// 总拷贝次数
    pub total_copies: AtomicU64,
    /// 字节拷贝次数
    pub byte_copies: AtomicU64,
    /// 字拷贝次数
    pub word_copies: AtomicU64,
    /// SIMD128拷贝次数
    pub simd128_copies: AtomicU64,
    /// SIMD256拷贝次数
    pub simd256_copies: AtomicU64,
    /// SIMD512拷贝次数
    pub simd512_copies: AtomicU64,
    /// 总拷贝字节数
    pub total_bytes: AtomicU64,
    /// 预取次数
    pub prefetch_count: AtomicU64,
    /// 非临时存储次数
    pub non_temporal_count: AtomicU64,
}

impl Clone for MemoryCopyStats {
    fn clone(&self) -> Self {
        Self {
            total_copies: AtomicU64::new(self.total_copies.load(Ordering::Relaxed)),
            byte_copies: AtomicU64::new(self.byte_copies.load(Ordering::Relaxed)),
            word_copies: AtomicU64::new(self.word_copies.load(Ordering::Relaxed)),
            simd128_copies: AtomicU64::new(self.simd128_copies.load(Ordering::Relaxed)),
            simd256_copies: AtomicU64::new(self.simd256_copies.load(Ordering::Relaxed)),
            simd512_copies: AtomicU64::new(self.simd512_copies.load(Ordering::Relaxed)),
            total_bytes: AtomicU64::new(self.total_bytes.load(Ordering::Relaxed)),
            prefetch_count: AtomicU64::new(self.prefetch_count.load(Ordering::Relaxed)),
            non_temporal_count: AtomicU64::new(self.non_temporal_count.load(Ordering::Relaxed)),
        }
    }
}

impl MemoryCopyStats {
    /// 获取统计信息快照
    pub fn snapshot(&self) -> MemoryCopyStatsSnapshot {
        MemoryCopyStatsSnapshot {
            total_copies: self.total_copies.load(Ordering::Relaxed),
            byte_copies: self.byte_copies.load(Ordering::Relaxed),
            word_copies: self.word_copies.load(Ordering::Relaxed),
            simd128_copies: self.simd128_copies.load(Ordering::Relaxed),
            simd256_copies: self.simd256_copies.load(Ordering::Relaxed),
            simd512_copies: self.simd512_copies.load(Ordering::Relaxed),
            total_bytes: self.total_bytes.load(Ordering::Relaxed),
            prefetch_count: self.prefetch_count.load(Ordering::Relaxed),
            non_temporal_count: self.non_temporal_count.load(Ordering::Relaxed),
        }
    }

    /// 重置统计信息
    pub fn reset(&self) {
        self.total_copies.store(0, Ordering::Relaxed);
        self.byte_copies.store(0, Ordering::Relaxed);
        self.word_copies.store(0, Ordering::Relaxed);
        self.simd128_copies.store(0, Ordering::Relaxed);
        self.simd256_copies.store(0, Ordering::Relaxed);
        self.simd512_copies.store(0, Ordering::Relaxed);
        self.total_bytes.store(0, Ordering::Relaxed);
        self.prefetch_count.store(0, Ordering::Relaxed);
        self.non_temporal_count.store(0, Ordering::Relaxed);
    }
}

/// 内存拷贝统计信息快照
#[derive(Debug, Clone)]
pub struct MemoryCopyStatsSnapshot {
    pub total_copies: u64,
    pub byte_copies: u64,
    pub word_copies: u64,
    pub simd128_copies: u64,
    pub simd256_copies: u64,
    pub simd512_copies: u64,
    pub total_bytes: u64,
    pub prefetch_count: u64,
    pub non_temporal_count: u64,
}

impl MemoryCopyStatsSnapshot {
    /// 计算平均拷贝大小
    pub fn avg_copy_size(&self) -> f64 {
        if self.total_copies == 0 {
            0.0
        } else {
            self.total_bytes as f64 / self.total_copies as f64
        }
    }

    /// 计算SIMD使用率
    pub fn simd_usage_rate(&self) -> f64 {
        if self.total_copies == 0 {
            0.0
        } else {
            (self.simd128_copies + self.simd256_copies + self.simd512_copies) as f64
                / self.total_copies as f64
        }
    }
}

/// 高效内存拷贝器
pub struct FastMemoryCopier {
    /// 配置
    config: MemoryCopyConfig,
    /// 统计信息
    stats: Arc<MemoryCopyStats>,
}

impl FastMemoryCopier {
    /// 创建新的内存拷贝器
    pub fn new(config: MemoryCopyConfig) -> Self {
        Self {
            config,
            stats: Arc::new(MemoryCopyStats::default()),
        }
    }

    /// 使用默认配置创建内存拷贝器
    pub fn with_default_config() -> Self {
        Self::new(MemoryCopyConfig::default())
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> MemoryCopyStatsSnapshot {
        self.stats.snapshot()
    }

    /// 重置统计信息
    pub fn reset_stats(&self) {
        self.stats.reset();
    }

    /// 高效内存拷贝
    ///
    /// # Safety
    ///
    /// 调用者必须确保：
    /// - `src` 指向有效的只读内存区域，大小至少为 `size` 字节
    /// - `dst` 指向有效的可写内存区域，大小至少为 `size` 字节
    /// - 源和目标内存区域不重叠
    /// - 指针在函数调用期间保持有效
    pub unsafe fn copy_memory(
        &self,
        src: *const u8,
        dst: *mut u8,
        size: usize,
    ) -> Result<(), VmError> {
        if src.is_null() || dst.is_null() {
            return Err(VmError::from(MemoryError::InvalidAddress(GuestAddr(0))));
        }

        if size == 0 {
            return Ok(());
        }

        // 更新统计信息
        self.stats.total_copies.fetch_add(1, Ordering::Relaxed);
        self.stats
            .total_bytes
            .fetch_add(size as u64, Ordering::Relaxed);

        // 计算对齐信息
        let src_addr = src as u64;
        let dst_addr = dst as u64;
        let alignment = AlignmentInfo::new(src_addr, dst_addr, size);

        // 选择拷贝策略
        let strategy = match self.config.default_strategy {
            CopyStrategy::Adaptive => self.select_strategy(&alignment, size),
            strategy => strategy,
        };

        // 执行拷贝
        match strategy {
            CopyStrategy::ByteByByte => {
                self.stats.byte_copies.fetch_add(1, Ordering::Relaxed);
                unsafe {
                    self.copy_byte_by_byte(src, dst, size);
                }
            }
            CopyStrategy::WordByWord => {
                self.stats.word_copies.fetch_add(1, Ordering::Relaxed);
                unsafe {
                    self.copy_word_by_word(src, dst, size);
                }
            }
            CopyStrategy::Simd128 => {
                self.stats.simd128_copies.fetch_add(1, Ordering::Relaxed);
                unsafe {
                    self.copy_simd128(src, dst, size);
                }
            }
            CopyStrategy::Simd256 => {
                self.stats.simd256_copies.fetch_add(1, Ordering::Relaxed);
                unsafe {
                    self.copy_simd256(src, dst, size);
                }
            }
            CopyStrategy::Simd512 => {
                self.stats.simd512_copies.fetch_add(1, Ordering::Relaxed);
                unsafe {
                    self.copy_simd512(src, dst, size);
                }
            }
            CopyStrategy::Adaptive => {
                // 自适应策略应该已经在前面被转换为具体策略了
                // 这里添加一个panic作为安全保障
                panic!("Adaptive strategy should have been converted to a concrete strategy");
            }
        }

        Ok(())
    }

    /// 选择最佳拷贝策略
    fn select_strategy(&self, alignment: &AlignmentInfo, size: usize) -> CopyStrategy {
        // 小块使用字节拷贝
        if size < self.config.small_block_threshold {
            return CopyStrategy::ByteByByte;
        }

        // 中等块使用字拷贝
        if size < self.config.medium_block_threshold {
            return CopyStrategy::WordByWord;
        }

        // 大块根据对齐情况选择SIMD
        if size < self.config.large_block_threshold {
            return if alignment.is_aligned {
                CopyStrategy::Simd128
            } else {
                CopyStrategy::WordByWord
            };
        }

        // 超大块根据对齐情况选择更大的SIMD
        if size < self.config.xlarge_block_threshold {
            return if alignment.is_aligned {
                CopyStrategy::Simd256
            } else {
                CopyStrategy::Simd128
            };
        }

        // 巨大块使用最大的SIMD
        if alignment.is_aligned {
            #[cfg(target_arch = "x86_64")]
            {
                if is_x86_feature_detected!("avx512f") {
                    return CopyStrategy::Simd512;
                }
            }
            CopyStrategy::Simd256
        } else {
            CopyStrategy::Simd128
        }
    }

    /// 逐字节拷贝
    unsafe fn copy_byte_by_byte(&self, src: *const u8, dst: *mut u8, size: usize) {
        unsafe {
            let src_slice = std::slice::from_raw_parts(src, size);
            let dst_slice = std::slice::from_raw_parts_mut(dst, size);
            dst_slice.copy_from_slice(src_slice);
        }
    }

    /// 逐字拷贝（8字节）
    unsafe fn copy_word_by_word(&self, src: *const u8, dst: *mut u8, size: usize) {
        let word_size = std::mem::size_of::<u64>();
        let word_count = size / word_size;
        let remainder = size % word_size;

        // 拷贝对齐的部分
        if word_count > 0 {
            unsafe {
                let src_words = std::slice::from_raw_parts(src as *const u64, word_count);
                let dst_words = std::slice::from_raw_parts_mut(dst as *mut u64, word_count);
                dst_words.copy_from_slice(src_words);
            }
        }

        // 拷贝剩余字节
        if remainder > 0 {
            unsafe {
                let src_remainder = src.add(word_count * word_size);
                let dst_remainder = dst.add(word_count * word_size);
                self.copy_byte_by_byte(src_remainder, dst_remainder, remainder);
            }
        }
    }

    /// SIMD128拷贝（16字节）
    unsafe fn copy_simd128(&self, src: *const u8, dst: *mut u8, size: usize) {
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("sse2") {
                return self.copy_simd128_x86(src, dst, size);
            }
        }

        #[cfg(target_arch = "aarch64")]
        {
            // ARM64架构默认支持NEON指令集，所以可以安全使用SIMD128
            unsafe {
                self.copy_simd128_arm(src, dst, size);
            }
        }

        // 对于其他架构，回退到字拷贝
        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        unsafe {
            self.copy_word_by_word(src, dst, size);
        }
    }

    /// x86 SIMD128拷贝实现
    #[cfg(target_arch = "x86_64")]
    unsafe fn copy_simd128_x86(&self, src: *const u8, dst: *mut u8, size: usize) {
        let simd_size = 16; // 128位 = 16字节
        let simd_count = size / simd_size;
        let remainder = size % simd_size;

        // 拷贝对齐的部分
        if simd_count > 0 {
            // 使用非临时存储（如果启用且大小足够大）
            let use_non_temporal =
                self.config.use_non_temporal && size >= self.config.non_temporal_threshold;

            if use_non_temporal {
                self.stats
                    .non_temporal_count
                    .fetch_add(1, Ordering::Relaxed);
                self.copy_simd128_non_temporal_x86(src, dst, simd_count);
            } else {
                self.copy_simd128_temporal_x86(src, dst, simd_count);
            }
        }

        // 拷贝剩余字节
        if remainder > 0 {
            unsafe {
                let src_remainder = src.add(simd_count * simd_size);
                let dst_remainder = dst.add(simd_count * simd_size);
                self.copy_word_by_word(src_remainder, dst_remainder, remainder);
            }
        }
    }

    /// x86 SIMD128时序拷贝
    #[cfg(target_arch = "x86_64")]
    unsafe fn copy_simd128_temporal_x86(&self, src: *const u8, dst: *mut u8, count: usize) {
        std::arch::x86_64::_mm_prefetch(src as *const i8, std::arch::x86_64::_MM_HINT_T0);

        for i in 0..count {
            let src_ptr = src.add(i * 16);
            let dst_ptr = dst.add(i * 16);

            let data =
                std::arch::x86_64::_mm_loadu_si128(src_ptr as *const std::arch::x86_64::__m128i);
            std::arch::x86_64::_mm_storeu_si128(dst_ptr as *mut std::arch::x86_64::__m128i, data);
        }
    }

    /// x86 SIMD128非时序拷贝
    #[cfg(target_arch = "x86_64")]
    unsafe fn copy_simd128_non_temporal_x86(&self, src: *const u8, dst: *mut u8, count: usize) {
        for i in 0..count {
            let src_ptr = src.add(i * 16);
            let dst_ptr = dst.add(i * 16);

            let data =
                std::arch::x86_64::_mm_loadu_si128(src_ptr as *const std::arch::x86_64::__m128i);
            std::arch::x86_64::_mm_stream_si128(dst_ptr as *mut std::arch::x86_64::__m128i, data);
        }

        // 内存屏障
        std::arch::x86_64::_mm_sfence();
    }

    /// ARM SIMD128拷贝实现
    #[cfg(target_arch = "aarch64")]
    unsafe fn copy_simd128_arm(&self, src: *const u8, dst: *mut u8, size: usize) {
        let simd_size = 16; // 128位 = 16字节
        let simd_count = size / simd_size;
        let remainder = size % simd_size;

        // 拷贝对齐的部分
        if simd_count > 0 {
            for i in 0..simd_count {
                unsafe {
                    let src_ptr = src.add(i * simd_size);
                    let dst_ptr = dst.add(i * simd_size);

                    let data = std::arch::aarch64::vld1q_u8(src_ptr);
                    std::arch::aarch64::vst1q_u8(dst_ptr, data);
                }
            }
        }

        // 拷贝剩余字节
        if remainder > 0 {
            unsafe {
                let src_remainder = src.add(simd_count * simd_size);
                let dst_remainder = dst.add(simd_count * simd_size);
                self.copy_word_by_word(src_remainder, dst_remainder, remainder);
            }
        }
    }

    /// SIMD256拷贝（32字节）
    unsafe fn copy_simd256(&self, src: *const u8, dst: *mut u8, size: usize) {
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("avx2") {
                return self.copy_simd256_x86(src, dst, size);
            }
        }

        // 回退到SIMD128
        unsafe {
            self.copy_simd128(src, dst, size);
        }
    }

    /// x86 SIMD256拷贝实现
    #[cfg(target_arch = "x86_64")]
    unsafe fn copy_simd256_x86(&self, src: *const u8, dst: *mut u8, size: usize) {
        let simd_size = 32; // 256位 = 32字节
        let simd_count = size / simd_size;
        let remainder = size % simd_size;

        // 拷贝对齐的部分
        if simd_count > 0 {
            // 使用非临时存储（如果启用且大小足够大）
            let use_non_temporal =
                self.config.use_non_temporal && size >= self.config.non_temporal_threshold;

            if use_non_temporal {
                self.stats
                    .non_temporal_count
                    .fetch_add(1, Ordering::Relaxed);
                self.copy_simd256_non_temporal_x86(src, dst, simd_count);
            } else {
                self.copy_simd256_temporal_x86(src, dst, simd_count);
            }
        }

        // 拷贝剩余字节
        if remainder > 0 {
            unsafe {
                let src_remainder = src.add(simd_count * simd_size);
                let dst_remainder = dst.add(simd_count * simd_size);
                self.copy_simd128(src_remainder, dst_remainder, remainder);
            }
        }
    }

    /// x86 SIMD256时序拷贝
    #[cfg(target_arch = "x86_64")]
    unsafe fn copy_simd256_temporal_x86(&self, src: *const u8, dst: *mut u8, count: usize) {
        std::arch::x86_64::_mm_prefetch(src as *const i8, std::arch::x86_64::_MM_HINT_T0);

        for i in 0..count {
            let src_ptr = src.add(i * 32);
            let dst_ptr = dst.add(i * 32);

            let data =
                std::arch::x86_64::_mm256_loadu_si256(src_ptr as *const std::arch::x86_64::__m256i);
            std::arch::x86_64::_mm256_storeu_si256(
                dst_ptr as *mut std::arch::x86_64::__m256i,
                data,
            );
        }
    }

    /// x86 SIMD256非时序拷贝
    #[cfg(target_arch = "x86_64")]
    unsafe fn copy_simd256_non_temporal_x86(&self, src: *const u8, dst: *mut u8, count: usize) {
        for i in 0..count {
            let src_ptr = src.add(i * 32);
            let dst_ptr = dst.add(i * 32);

            let data =
                std::arch::x86_64::_mm256_loadu_si256(src_ptr as *const std::arch::x86_64::__m256i);
            std::arch::x86_64::_mm256_stream_si256(
                dst_ptr as *mut std::arch::x86_64::__m256i,
                data,
            );
        }

        // 内存屏障
        std::arch::x86_64::_mm_sfence();
    }

    /// SIMD512拷贝（64字节）
    unsafe fn copy_simd512(&self, src: *const u8, dst: *mut u8, size: usize) {
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("avx512f") {
                return self.copy_simd512_x86(src, dst, size);
            }
        }

        // 回退到SIMD256
        unsafe {
            self.copy_simd256(src, dst, size);
        }
    }

    /// x86 SIMD512拷贝实现
    #[cfg(target_arch = "x86_64")]
    unsafe fn copy_simd512_x86(&self, src: *const u8, dst: *mut u8, size: usize) {
        let simd_size = 64; // 512位 = 64字节
        let simd_count = size / simd_size;
        let remainder = size % simd_size;

        // 拷贝对齐的部分
        if simd_count > 0 {
            // 使用非临时存储（如果启用且大小足够大）
            let use_non_temporal =
                self.config.use_non_temporal && size >= self.config.non_temporal_threshold;

            if use_non_temporal {
                self.stats
                    .non_temporal_count
                    .fetch_add(1, Ordering::Relaxed);
                self.copy_simd512_non_temporal_x86(src, dst, simd_count);
            } else {
                self.copy_simd512_temporal_x86(src, dst, simd_count);
            }
        }

        // 拷贝剩余字节
        if remainder > 0 {
            unsafe {
                let src_remainder = src.add(simd_count * simd_size);
                let dst_remainder = dst.add(simd_count * simd_size);
                self.copy_simd256(src_remainder, dst_remainder, remainder);
            }
        }
    }

    /// x86 SIMD512时序拷贝
    #[cfg(target_arch = "x86_64")]
    unsafe fn copy_simd512_temporal_x86(&self, src: *const u8, dst: *mut u8, count: usize) {
        std::arch::x86_64::_mm_prefetch(src as *const i8, std::arch::x86_64::_MM_HINT_T0);

        for i in 0..count {
            let src_ptr = src.add(i * 64);
            let dst_ptr = dst.add(i * 64);

            let data =
                std::arch::x86_64::_mm512_loadu_si512(src_ptr as *const std::arch::x86_64::__m512i);
            std::arch::x86_64::_mm512_storeu_si512(
                dst_ptr as *mut std::arch::x86_64::__m512i,
                data,
            );
        }
    }

    /// x86 SIMD512非时序拷贝
    #[cfg(target_arch = "x86_64")]
    unsafe fn copy_simd512_non_temporal_x86(&self, src: *const u8, dst: *mut u8, count: usize) {
        for i in 0..count {
            let src_ptr = src.add(i * 64);
            let dst_ptr = dst.add(i * 64);

            let data =
                std::arch::x86_64::_mm512_loadu_si512(src_ptr as *const std::arch::x86_64::__m512i);
            std::arch::x86_64::_mm512_stream_si512(
                dst_ptr as *mut std::arch::x86_64::__m512i,
                data,
            );
        }

        // 内存屏障
        std::arch::x86_64::_mm_sfence();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alignment_info() {
        // 测试对齐检测
        assert!(AlignmentInfo::is_aligned_to(0x1000, 0x1000));
        assert!(AlignmentInfo::is_aligned_to(0x2000, 0x1000));
        assert!(!AlignmentInfo::is_aligned_to(0x1001, 0x1000));

        // 测试对齐信息计算
        let alignment = AlignmentInfo::new(0x1000, 0x2000, 1024);
        assert!(alignment.is_aligned);

        let alignment = AlignmentInfo::new(0x1001, 0x2000, 1024);
        assert!(!alignment.is_aligned);
    }

    #[test]
    fn test_copy_strategy_selection() {
        let alignment = AlignmentInfo::new(0x1000, 0x2000, 1024);
        let strategy = alignment.best_copy_strategy();
        assert_ne!(strategy, CopyStrategy::ByteByByte);

        let alignment = AlignmentInfo::new(0x1001, 0x2001, 10);
        let strategy = alignment.best_copy_strategy();
        assert_eq!(strategy, CopyStrategy::ByteByByte);
    }

    #[test]
    fn test_memory_copier_creation() {
        let copier = FastMemoryCopier::with_default_config();
        let stats = copier.get_stats();
        assert_eq!(stats.total_copies, 0);
    }

    #[test]
    fn test_small_copy() {
        let copier = FastMemoryCopier::with_default_config();
        let src = vec![1u8; 10];
        let mut dst = vec![0u8; 10];

        unsafe {
            copier
                .copy_memory(src.as_ptr(), dst.as_mut_ptr(), 10)
                .unwrap();
        }

        assert_eq!(src, dst);

        let stats = copier.get_stats();
        assert_eq!(stats.total_copies, 1);
        assert_eq!(stats.total_bytes, 10);
    }

    #[test]
    fn test_large_copy() {
        let copier = FastMemoryCopier::with_default_config();
        let src = vec![2u8; 10000];
        let mut dst = vec![0u8; 10000];

        unsafe {
            copier
                .copy_memory(src.as_ptr(), dst.as_mut_ptr(), 10000)
                .unwrap();
        }

        assert_eq!(src, dst);

        let stats = copier.get_stats();
        assert_eq!(stats.total_copies, 1);
        assert_eq!(stats.total_bytes, 10000);
    }
}
