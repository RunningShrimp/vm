//! 内存管理接口定义

use crate::{
    CacheStats, GuestAddr, MemoryOrder, PageFlags, PageStats, VmComponent, VmError,
};
use vm_core::GuestPhysAddr;

/// 内存管理器配置
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemoryManagerConfig {
    /// 内存大小（字节）
    pub memory_size: usize,
    /// 页大小
    pub page_size: usize,
    /// 启用TLB
    pub enable_tlb: bool,
    /// TLB大小
    pub tlb_size: usize,
    /// 启用NUMA感知
    pub enable_numa: bool,
    /// 启用内存池
    pub enable_memory_pool: bool,
    /// 最大内存池大小
    pub max_pool_size: usize,
}

impl Default for MemoryManagerConfig {
    fn default() -> Self {
        Self {
            memory_size: 128 * 1024 * 1024, // 128MB
            page_size: 4096,
            enable_tlb: true,
            tlb_size: 1024,
            enable_numa: false,
            enable_memory_pool: true,
            max_pool_size: 64 * 1024 * 1024, // 64MB
        }
    }
}

/// 统一的内存管理接口
pub trait MemoryManager: VmComponent {
    /// 读取内存
    fn read_memory(&self, addr: GuestAddr, size: usize) -> Result<Vec<u8>, VmError>;

    /// 写入内存
    fn write_memory(&mut self, addr: GuestAddr, data: &[u8]) -> Result<(), VmError>;

    /// 原子读取
    fn read_atomic(&self, addr: GuestAddr, size: usize, order: MemoryOrder)
    -> Result<u64, VmError>;

    /// 原子写入
    fn write_atomic(
        &mut self,
        addr: GuestAddr,
        value: u64,
        size: usize,
        order: MemoryOrder,
    ) -> Result<(), VmError>;

    /// 原子比较交换
    fn compare_exchange(
        &mut self,
        addr: GuestAddr,
        expected: u64,
        desired: u64,
        size: usize,
        success: MemoryOrder,
        failure: MemoryOrder,
    ) -> Result<u64, VmError>;

    /// 异步内存操作
    async fn read_memory_async(&self, addr: GuestAddr, size: usize) -> Result<Vec<u8>, VmError>;
    async fn write_memory_async(&mut self, addr: GuestAddr, data: Vec<u8>) -> Result<(), VmError>;
}

/// 缓存管理接口
pub trait CacheManager {
    type Key;
    type Value;

    /// 获取缓存项
    fn get(&self, key: &Self::Key) -> Option<&Self::Value>;

    /// 设置缓存项
    fn set(&mut self, key: Self::Key, value: Self::Value);

    /// 删除缓存项
    fn remove(&mut self, key: &Self::Key) -> Option<Self::Value>;

    /// 清空缓存
    fn clear(&mut self);

    /// 获取缓存统计
    fn get_stats(&self) -> &CacheStats;
}

/// 页表管理接口
pub trait PageTableManager {
    /// 地址翻译
    fn translate(
        &self,
        vaddr: GuestAddr,
        access_type: vm_core::AccessType,
    ) -> Result<GuestPhysAddr, VmError>;

    /// 更新页表项
    fn update_entry(
        &mut self,
        vaddr: GuestAddr,
        paddr: GuestPhysAddr,
        flags: PageFlags,
    ) -> Result<(), VmError>;

    /// 刷新TLB
    fn flush_tlb(&mut self, vaddr: Option<GuestAddr>);

    /// 获取页表统计
    fn get_page_stats(&self) -> &PageStats;
}

/// TLB缓存管理器
pub struct TlbCacheManager {
    config: MemoryManagerConfig,
    stats: CacheStats,
}

impl TlbCacheManager {
    pub fn new(config: MemoryManagerConfig) -> Self {
        Self {
            config,
            stats: CacheStats::default(),
        }
    }
}

impl CacheManager for TlbCacheManager {
    type Key = (GuestAddr, u16); // (virtual_address, asid)
    type Value = (GuestPhysAddr, PageFlags);

    fn get(&self, _key: &Self::Key) -> Option<&Self::Value> {
        // 简化实现
        None
    }

    fn set(&mut self, _key: Self::Key, _value: Self::Value) {
        // 简化实现
    }

    fn remove(&mut self, _key: &Self::Key) -> Option<Self::Value> {
        // 简化实现
        None
    }

    fn clear(&mut self) {
        // 简化实现
    }

    fn get_stats(&self) -> &CacheStats {
        &self.stats
    }
}

/// 页表管理器实现
pub struct PageTableManagerImpl {
    config: MemoryManagerConfig,
    stats: PageStats,
}

impl PageTableManagerImpl {
    pub fn new(config: MemoryManagerConfig) -> Self {
        Self {
            config,
            stats: PageStats::default(),
        }
    }
}

impl PageTableManager for PageTableManagerImpl {
    fn translate(
        &self,
        vaddr: GuestAddr,
        _access_type: vm_core::AccessType,
    ) -> Result<GuestPhysAddr, VmError> {
        // 简化实现：直接映射
        if vaddr >= self.config.memory_size as u64 {
            return Err(VmError::Memory(vm_core::MemoryError::InvalidAddress(vaddr)));
        }
        Ok(vaddr) // 直接映射
    }

    fn update_entry(
        &mut self,
        _vaddr: GuestAddr,
        _paddr: GuestPhysAddr,
        _flags: PageFlags,
    ) -> Result<(), VmError> {
        // 简化实现
        Ok(())
    }

    fn flush_tlb(&mut self, _vaddr: Option<GuestAddr>) {
        self.stats.flushes += 1;
    }

    fn get_page_stats(&self) -> &PageStats {
        &self.stats
    }
}
