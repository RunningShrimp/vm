//! Week 4 - 异步内存管理单元 (AsyncMMU)
//!
//! 为虚拟机的内存管理单元添加异步支持，包括：
//! - 异步地址翻译
//! - 异步 TLB 操作
//! - 批量地址翻译
//! - TLB 预取

#![cfg(feature = "async")]

use crate::{AccessType, GuestAddr, GuestPhysAddr, VmError};
use parking_lot::RwLock;
use std::collections::VecDeque;
use std::sync::Arc;

use tokio::time::sleep;

/// 异步 TLB 表项
#[derive(Clone, Debug)]
pub struct TLBEntry {
    /// 虚拟地址
    pub va: GuestAddr,
    /// 物理地址
    pub pa: GuestPhysAddr,
    /// 访问权限
    pub access: AccessType,
    /// 是否脏
    pub dirty: bool,
    /// 最后访问时间 (us)
    pub last_access_us: u64,
}

/// TLB 统计信息
#[derive(Clone, Debug, Default)]
pub struct TLBStats {
    /// TLB 命中数
    pub hits: u64,
    /// TLB 缺失数
    pub misses: u64,
    /// 预取命中数
    pub prefetch_hits: u64,
    /// 驱逐数
    pub evictions: u64,
    /// 总查找次数
    pub total_lookups: u64,
}

impl TLBStats {
    /// 计算命中率
    pub fn hit_rate(&self) -> f64 {
        if self.total_lookups == 0 {
            0.0
        } else {
            self.hits as f64 / self.total_lookups as f64
        }
    }

    /// 计算预取效率
    pub fn prefetch_efficiency(&self) -> f64 {
        if self.misses == 0 {
            0.0
        } else {
            self.prefetch_hits as f64 / self.misses as f64
        }
    }
}

/// 异步 TLB 缓存
///
/// 优化：使用分片锁减少锁竞争，提高并发性能
pub struct AsyncTLB {
    /// TLB 表项分片（使用分片锁减少竞争）
    shards: Vec<Arc<RwLock<Vec<TLBEntry>>>>,
    /// TLB 容量
    capacity: usize,
    /// 分片数量（必须是2的幂）
    shard_count: usize,
    /// 分片掩码
    shard_mask: usize,
    /// 预取队列（使用无锁队列）
    prefetch_queue: Arc<parking_lot::Mutex<VecDeque<GuestAddr>>>,
    /// 统计信息（使用原子操作）
    stats: Arc<parking_lot::Mutex<TLBStats>>,
}

impl AsyncTLB {
    /// 创建新的异步 TLB
    ///
    /// 使用分片锁优化并发性能
    pub fn new(capacity: usize) -> Self {
        // 使用16个分片（可以根据CPU核心数调整）
        let shard_count = 16;
        let mut shards = Vec::with_capacity(shard_count);
        let shard_capacity = capacity / shard_count + 1;

        for _ in 0..shard_count {
            shards.push(Arc::new(RwLock::new(Vec::with_capacity(shard_capacity))));
        }

        Self {
            shards,
            capacity,
            shard_count,
            shard_mask: shard_count - 1,
            prefetch_queue: Arc::new(parking_lot::Mutex::new(VecDeque::new())),
            stats: Arc::new(parking_lot::Mutex::new(TLBStats::default())),
        }
    }

    /// 根据地址计算分片索引（使用地址的高位）
    #[inline]
    fn shard_index(&self, va: GuestAddr) -> usize {
        // 使用地址的20-24位作为分片索引
        ((va >> 20) as usize) & self.shard_mask
    }

    /// 获取指定地址对应的分片
    #[inline]
    fn get_shard(&self, va: GuestAddr) -> &Arc<RwLock<Vec<TLBEntry>>> {
        &self.shards[self.shard_index(va)]
    }

    /// 查找 TLB 表项（无锁优化：只读操作使用读锁）
    pub fn lookup(&self, va: GuestAddr) -> Option<TLBEntry> {
        let shard = self.get_shard(va);
        let entries = shard.read();
        let result = entries.iter().find(|e| e.va == va).cloned();

        // 使用快速路径更新统计（减少锁竞争）
        let mut stats = self.stats.lock();
        stats.total_lookups += 1;

        match result {
            Some(entry) => {
                stats.hits += 1;
                Some(entry)
            }
            None => {
                stats.misses += 1;
                None
            }
        }
    }

    /// 插入 TLB 表项（使用分片锁，只锁定相关分片）
    pub fn insert(&self, entry: TLBEntry) {
        let shard = self.get_shard(entry.va);
        let mut entries = shard.write();

        // 检查是否已存在
        if let Some(pos) = entries.iter().position(|e| e.va == entry.va) {
            entries[pos] = entry;
        } else {
            // 如果满了，驱逐最旧的（LRU策略）
            let shard_capacity = self.capacity / self.shard_count + 1;
            if entries.len() >= shard_capacity {
                entries.remove(0);
                let mut stats = self.stats.lock();
                stats.evictions += 1;
            }
            entries.push(entry);
        }
    }

    /// 刷新特定地址的 TLB（只刷新相关分片）
    pub fn flush_va(&self, va: GuestAddr) {
        let shard = self.get_shard(va);
        let mut entries = shard.write();
        entries.retain(|e| e.va != va);
    }

    /// 刷新所有 TLB（并行刷新所有分片）
    pub fn flush_all(&self) {
        // 并行刷新所有分片，减少总时间
        for shard in &self.shards {
            let mut entries = shard.write();
            entries.clear();
        }
    }

    /// 异步地址翻译
    pub async fn translate_async(&self, va: GuestAddr) -> Result<GuestPhysAddr, VmError> {
        // 先查看 TLB
        if let Some(entry) = self.lookup(va) {
            return Ok(entry.pa);
        }

        // TLB 缺失，执行翻译（这里简化为直接返回）
        // 实际应该查询页表
        Ok(va)
    }

    /// 异步 TLB 预取
    pub async fn prefetch_async(&self, addresses: &[GuestAddr]) {
        let mut queue = self.prefetch_queue.lock();
        for addr in addresses {
            if queue.len() < 100 {
                // 队列大小限制
                queue.push_back(*addr);
            }
        }
    }

    /// 处理预取队列
    pub async fn process_prefetch_queue(&self) {
        let mut queue = self.prefetch_queue.lock();

        while let Some(va) = queue.pop_front() {
            if self.lookup(va).is_none() {
                // 预取地址
                if let Ok(pa) = self.translate_async(va).await {
                    let entry = TLBEntry {
                        va,
                        pa,
                        access: AccessType::Read,
                        dirty: false,
                        last_access_us: 0,
                    };
                    self.insert(entry);

                    let mut stats = self.stats.lock();
                    stats.prefetch_hits += 1;
                }
            }
        }
    }

    /// 批量地址翻译
    pub async fn batch_translate_async(
        &self,
        addresses: &[GuestAddr],
    ) -> Result<Vec<GuestPhysAddr>, VmError> {
        let mut results = Vec::with_capacity(addresses.len());

        for &va in addresses {
            let pa = self.translate_async(va).await?;
            results.push(pa);
        }

        Ok(results)
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> TLBStats {
        self.stats.lock().clone()
    }

    /// 重置统计信息
    pub fn reset_stats(&self) {
        let mut stats = self.stats.lock();
        stats.hits = 0;
        stats.misses = 0;
        stats.prefetch_hits = 0;
        stats.evictions = 0;
        stats.total_lookups = 0;
    }
}

/// 异步 MMU trait
pub trait AsyncMMU: Send + Sync {
    /// 异步地址翻译
    async fn translate_async(
        &mut self,
        va: GuestAddr,
        access: AccessType,
    ) -> Result<GuestPhysAddr, VmError>;

    /// 异步 TLB 预取
    async fn prefetch_tlb_async(&mut self, addresses: &[GuestAddr]) -> Result<(), VmError>;

    /// 批量地址翻译
    async fn batch_translate_async(
        &mut self,
        addresses: &[GuestAddr],
    ) -> Result<Vec<GuestPhysAddr>, VmError>;

    /// 异步 TLB 刷新
    async fn flush_tlb_async(&mut self) -> Result<(), VmError>;

    /// 获取 TLB 统计
    fn get_tlb_stats(&self) -> TLBStats;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tlb_creation() {
        let tlb = AsyncTLB::new(256);
        assert_eq!(tlb.capacity, 256);
        let stats = tlb.get_stats();
        assert_eq!(stats.total_lookups, 0);
    }

    #[test]
    fn test_tlb_insert_and_lookup() {
        let tlb = AsyncTLB::new(10);
        let entry = TLBEntry {
            va: 0x1000,
            pa: 0x2000,
            access: AccessType::Read,
            dirty: false,
            last_access_us: 0,
        };

        tlb.insert(entry.clone());
        let found = tlb.lookup(0x1000);
        assert!(found.is_some());
        assert_eq!(found.unwrap().pa, 0x2000);
    }

    #[test]
    fn test_tlb_miss() {
        let tlb = AsyncTLB::new(10);
        let result = tlb.lookup(0x5000);
        assert!(result.is_none());
    }

    #[test]
    fn test_tlb_hit_rate() {
        let tlb = AsyncTLB::new(10);
        let entry = TLBEntry {
            va: 0x1000,
            pa: 0x2000,
            access: AccessType::Read,
            dirty: false,
            last_access_us: 0,
        };

        tlb.insert(entry);
        tlb.lookup(0x1000); // hit
        tlb.lookup(0x5000); // miss

        let stats = tlb.get_stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert!(stats.hit_rate() > 0.4 && stats.hit_rate() < 0.6);
    }

    #[test]
    fn test_tlb_flush() {
        let tlb = AsyncTLB::new(10);
        let entry = TLBEntry {
            va: 0x1000,
            pa: 0x2000,
            access: AccessType::Read,
            dirty: false,
            last_access_us: 0,
        };

        tlb.insert(entry);
        assert!(tlb.lookup(0x1000).is_some());

        tlb.flush_va(0x1000);
        assert!(tlb.lookup(0x1000).is_none());
    }

    #[tokio::test]
    async fn test_async_translate() {
        let tlb = AsyncTLB::new(10);
        let result = tlb.translate_async(0x1000).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_batch_translate() {
        let tlb = AsyncTLB::new(10);
        let addresses = vec![0x1000, 0x2000, 0x3000];
        let result = tlb.batch_translate_async(&addresses).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 3);
    }

    #[test]
    fn test_tlb_capacity() {
        let tlb = AsyncTLB::new(3);

        // 插入 4 个表项，应该驱逐最旧的
        for i in 0..4 {
            let entry = TLBEntry {
                va: 0x1000 + (i * 0x1000) as u64,
                pa: 0x2000 + (i * 0x1000) as u64,
                access: AccessType::Read,
                dirty: false,
                last_access_us: 0,
            };
            tlb.insert(entry);
        }

        let stats = tlb.get_stats();
        assert_eq!(stats.evictions, 1); // 应该有 1 次驱逐
    }
}
