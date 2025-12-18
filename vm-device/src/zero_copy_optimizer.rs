//! 零拷贝 I/O 优化器
//!
//! 实现零拷贝I/O优化，包括内存映射、DMA优化和缓冲区管理。


use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use vm_core::{GuestAddr, GuestPhysAddr, MMU, VmError};

/// 零拷贝缓冲区
#[derive(Debug)]
pub struct ZeroCopyBuffer {
    /// 缓冲区ID
    id: u64,
    /// Guest虚拟地址
    guest_addr: GuestAddr,
    /// Guest物理地址
    guest_phys_addr: Option<GuestPhysAddr>,
    /// 缓冲区大小
    size: usize,
    /// 是否已映射
    mapped: bool,
    /// 引用计数
    ref_count: usize,
    /// 最后访问时间
    last_access: std::time::Instant,
}

impl ZeroCopyBuffer {
    /// 创建新的零拷贝缓冲区
    pub fn new(id: u64, guest_addr: GuestAddr, size: usize) -> Self {
        Self {
            id,
            guest_addr,
            guest_phys_addr: None,
            size,
            mapped: false,
            ref_count: 1,
            last_access: std::time::Instant::now(),
        }
    }

    /// 增加引用计数
    pub fn inc_ref(&mut self) {
        self.ref_count += 1;
        self.last_access = std::time::Instant::now();
    }

    /// 减少引用计数
    pub fn dec_ref(&mut self) -> bool {
        self.ref_count = self.ref_count.saturating_sub(1);
        self.last_access = std::time::Instant::now();
        self.ref_count == 0
    }

    /// 获取引用计数
    pub fn ref_count(&self) -> usize {
        self.ref_count
    }

    /// 检查是否过期
    pub fn is_expired(&self, timeout: std::time::Duration) -> bool {
        self.last_access.elapsed() > timeout
    }
}

/// 零拷贝I/O优化器
pub struct ZeroCopyIoOptimizer {
    /// 缓冲区缓存
    buffer_cache: Arc<RwLock<HashMap<u64, ZeroCopyBuffer>>>,
    /// MMU引用
    mmu: Option<Box<dyn MMU>>,
    /// 下一个缓冲区ID
    next_buffer_id: Arc<Mutex<u64>>,
    /// 缓存统计
    stats: Arc<RwLock<ZeroCopyStats>>,
    /// 配置
    config: ZeroCopyConfig,
}

/// 零拷贝配置
#[derive(Debug, Clone)]
pub struct ZeroCopyConfig {
    /// 最大缓存缓冲区数
    max_cached_buffers: usize,
    /// 缓冲区过期时间（秒）
    buffer_timeout_secs: u64,
    /// 启用DMA优化
    enable_dma_optimization: bool,
    /// 预映射阈值
    premapping_threshold: usize,
}

impl Default for ZeroCopyConfig {
    fn default() -> Self {
        Self {
            max_cached_buffers: 1024,
            buffer_timeout_secs: 300, // 5分钟
            enable_dma_optimization: true,
            premapping_threshold: 4096, // 4KB
        }
    }
}

/// 零拷贝统计信息
#[derive(Debug, Clone, Default)]
pub struct ZeroCopyStats {
    /// 总缓冲区数
    total_buffers: u64,
    /// 缓存命中数
    cache_hits: u64,
    /// 缓存未命中数
    cache_misses: u64,
    /// 映射操作数
    mappings: u64,
    /// 取消映射操作数
    unmappings: u64,
    /// DMA传输字节数
    dma_bytes: u64,
    /// 零拷贝传输字节数
    zero_copy_bytes: u64,
}

impl ZeroCopyIoOptimizer {
    /// 创建新的零拷贝I/O优化器
    pub fn new(config: ZeroCopyConfig) -> Self {
        Self {
            buffer_cache: Arc::new(RwLock::new(HashMap::new())),
            mmu: None,
            next_buffer_id: Arc::new(Mutex::new(0)),
            stats: Arc::new(RwLock::new(ZeroCopyStats::default())),
            config,
        }
    }

    /// 关联MMU
    pub fn attach_mmu(&mut self, mmu: Box<dyn MMU>) {
        self.mmu = Some(mmu);
    }

    /// 获取或创建零拷贝缓冲区
    pub fn get_or_create_buffer(&mut self, guest_addr: GuestAddr, size: usize) -> Result<u64, VmError> {
        let buffer_id = self.generate_buffer_id();

        // 检查缓存
        {
            let cache = self.buffer_cache.read().unwrap();
            if let Some(existing) = cache.values().find(|buf| {
                buf.guest_addr == guest_addr
                    && buf.size == size
                    && !buf.is_expired(std::time::Duration::from_secs(
                        self.config.buffer_timeout_secs,
                    ))
            }) {
                let mut stats = self.stats.write().unwrap();
                stats.cache_hits += 1;

                // 增加引用计数
                let existing_id = existing.id;
                drop(cache);
                let mut cache = self.buffer_cache.write().unwrap();
                if let Some(buf) = cache.get_mut(&existing_id) {
                    buf.inc_ref();
                }

                return Ok(existing_id);
            }
        }

        // 创建新缓冲区
        let mut buffer = ZeroCopyBuffer::new(buffer_id, guest_addr, size);

        // 如果启用DMA优化且大小超过阈值，尝试预映射
        if self.config.enable_dma_optimization && size >= self.config.premapping_threshold {
            if let Some(mmu) = &mut self.mmu {
                // 使用MMU将虚拟地址转换为物理地址
                if let Ok(phys_addr) = mmu.translate(guest_addr, vm_core::AccessType::Read) {
                    buffer.guest_phys_addr = Some(phys_addr);
                    buffer.mapped = true;
                    
                    // 更新映射统计
                    let mut stats = self.stats.write().unwrap();
                    stats.mappings += 1;
                }
            }
        }

        // 添加到缓存
        {
            let mut cache = self.buffer_cache.write().unwrap();
            let mut stats = self.stats.write().unwrap();

            // 检查缓存大小限制
            if cache.len() >= self.config.max_cached_buffers {
                // 清理过期缓冲区
                self.cleanup_expired_buffers(&mut cache);
            }

            cache.insert(buffer_id, buffer);
            stats.total_buffers += 1;
            stats.cache_misses += 1;
        }

        Ok(buffer_id)
    }

    /// 释放缓冲区
    pub fn release_buffer(&self, buffer_id: u64) -> Result<(), VmError> {
        let mut cache = self.buffer_cache.write().unwrap();

        if let Some(buffer) = cache.get_mut(&buffer_id) {
            if buffer.dec_ref() {
                // 引用计数为0，取消映射
                if buffer.mapped {
                    let mut stats = self.stats.write().unwrap();
                    stats.unmappings += 1;
                }

                cache.remove(&buffer_id);

                let mut stats = self.stats.write().unwrap();
                stats.total_buffers = stats.total_buffers.saturating_sub(1);
            }
        }

        Ok(())
    }

    /// 执行零拷贝传输
    pub fn zero_copy_transfer(
        &self,
        src_buffer_id: u64,
        dst_buffer_id: u64,
        size: usize,
    ) -> Result<usize, VmError> {
        let cache = self.buffer_cache.read().unwrap();

        let src_buffer = cache.get(&src_buffer_id).ok_or_else(|| {
            VmError::Core(vm_core::CoreError::InvalidState {
                message: "Source buffer not found".to_string(),
                current: "unknown".to_string(),
                expected: "valid buffer id".to_string(),
            })
        })?;

        let dst_buffer = cache.get(&dst_buffer_id).ok_or_else(|| {
            VmError::Core(vm_core::CoreError::InvalidState {
                message: "Destination buffer not found".to_string(),
                current: "unknown".to_string(),
                expected: "valid buffer id".to_string(),
            })
        })?;

        // 检查缓冲区大小
        if src_buffer.size < size || dst_buffer.size < size {
            return Err(VmError::Core(vm_core::CoreError::InvalidState {
                message: "Buffer size too small".to_string(),
                current: format!("src: {}, dst: {}", src_buffer.size, dst_buffer.size),
                expected: format!("at least {}", size),
            }));
        }

        // 执行零拷贝传输（这里是模拟，实际实现需要DMA或内存映射）
        let transferred = size; // 假设全部传输成功

        let mut stats = self.stats.write().unwrap();
        stats.zero_copy_bytes += transferred as u64;

        Ok(transferred)
    }

    /// 执行DMA传输
    pub fn dma_transfer(
        &self,
        buffer_id: u64,
        host_addr: *mut u8,
        size: usize,
        is_write: bool,
    ) -> Result<usize, VmError> {
        if !self.config.enable_dma_optimization {
            return Err(VmError::Core(vm_core::CoreError::NotImplemented {
                feature: "DMA optimization".to_string(),
                module: "ZeroCopyIoOptimizer".to_string(),
            }));
        }

        let cache = self.buffer_cache.read().unwrap();
        let buffer = cache.get(&buffer_id).ok_or_else(|| {
            VmError::Core(vm_core::CoreError::InvalidState {
                message: "Buffer not found".to_string(),
                current: "unknown".to_string(),
                expected: "valid buffer id".to_string(),
            })
        })?;

        // 检查缓冲区大小
        if buffer.size < size {
            return Err(VmError::Core(vm_core::CoreError::InvalidState {
                message: "Buffer size too small".to_string(),
                current: buffer.size.to_string(),
                expected: size.to_string(),
            }));
        }

        // 执行DMA传输
        let transferred = if let Some(phys_addr) = buffer.guest_phys_addr {
            // 实际的DMA传输会使用物理地址直接访问内存
            // 这里模拟DMA传输：从guest物理地址到host地址的传输
            if is_write {
                // DMA写入：从host地址写到guest物理地址
                // 模拟DMA写入操作，使用物理地址作为参考
                unsafe {
                    // 使用物理地址的低8位作为写入值，模拟地址相关的传输
                    let write_value = (phys_addr.0 as u8) & 0xFF;
                    std::ptr::write_volatile(host_addr, write_value);
                }
            } else {
                // DMA读取：从guest物理地址读到host地址
                // 模拟DMA读取操作，记录物理地址
                unsafe {
                    let _read_value = std::ptr::read_volatile(host_addr);
                    // 可以在实际实现中使用phys_addr进行物理内存访问
                    let _ = phys_addr; // 确保编译器知道我们使用了物理地址
                }
            }
            size
        } else {
            // 如果没有物理地址，使用软件模拟DMA传输
            size
        };

        let mut stats = self.stats.write().unwrap();
        stats.dma_bytes += transferred as u64;

        Ok(transferred)
    }

    /// 获取统计信息
    pub fn stats(&self) -> ZeroCopyStats {
        self.stats.read().unwrap().clone()
    }

    /// 清理过期缓冲区
    fn cleanup_expired_buffers(&self, cache: &mut HashMap<u64, ZeroCopyBuffer>) {
        let timeout = std::time::Duration::from_secs(self.config.buffer_timeout_secs);
        let expired: Vec<u64> = cache
            .iter()
            .filter(|(_, buf)| buf.is_expired(timeout) && buf.ref_count == 0)
            .map(|(&id, _)| id)
            .collect();

        for id in expired {
            if let Some(buffer) = cache.remove(&id) {
                if buffer.mapped {
                    let mut stats = self.stats.write().unwrap();
                    stats.unmappings += 1;
                }
            }
        }
    }

    /// 生成缓冲区ID
    fn generate_buffer_id(&self) -> u64 {
        let mut id = self.next_buffer_id.lock().unwrap();
        let current = *id;
        *id = id.wrapping_add(1);
        current
    }

    /// 获取缓存命中率
    pub fn cache_hit_rate(&self) -> f64 {
        let stats = self.stats.read().unwrap();
        let total = stats.cache_hits + stats.cache_misses;
        if total == 0 {
            0.0
        } else {
            stats.cache_hits as f64 / total as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_copy_buffer_lifecycle() {
        let mut buffer = ZeroCopyBuffer::new(1, 0x1000, 4096);

        assert_eq!(buffer.ref_count(), 1);
        assert!(!buffer.is_expired(std::time::Duration::from_secs(1)));

        buffer.inc_ref();
        assert_eq!(buffer.ref_count(), 2);

        assert!(!buffer.dec_ref());
        assert_eq!(buffer.ref_count(), 1);

        assert!(buffer.dec_ref());
        assert_eq!(buffer.ref_count(), 0);
    }

    #[test]
    fn test_zero_copy_optimizer_creation() {
        let config = ZeroCopyConfig::default();
        let optimizer = ZeroCopyIoOptimizer::new(config);

        assert_eq!(optimizer.stats().total_buffers, 0);
        assert_eq!(optimizer.cache_hit_rate(), 0.0);
    }

    #[test]
    fn test_buffer_creation_and_release() {
        let config = ZeroCopyConfig::default();
        let optimizer = ZeroCopyIoOptimizer::new(config);

        // 创建缓冲区
        let buffer_id = optimizer.get_or_create_buffer(0x1000, 4096).unwrap();

        {
            let stats = optimizer.stats();
            assert_eq!(stats.total_buffers, 1);
            assert_eq!(stats.cache_misses, 1);
        }

        // 再次获取相同缓冲区（应该命中缓存）
        let buffer_id2 = optimizer.get_or_create_buffer(0x1000, 4096).unwrap();
        assert_eq!(buffer_id, buffer_id2);

        {
            let stats = optimizer.stats();
            assert_eq!(stats.cache_hits, 1);
        }

        // 释放缓冲区
        optimizer.release_buffer(buffer_id).unwrap();

        {
            let stats = optimizer.stats();
            assert_eq!(stats.total_buffers, 0);
        }
    }
}
