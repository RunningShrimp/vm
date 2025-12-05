use crate::{AccessType, GuestAddr, GuestPhysAddr, TlbEntry, TlbManager, VmError};
use parking_lot::RwLock;
use std::collections::HashMap;
/// Week 4 - TLB 异步优化
///
/// 实现高效的 TLB 异步操作，包括：
/// - 并发 TLB 访问
/// - 异步批量刷新
/// - 选择性失效
/// - TLB 一致性维护
use std::sync::Arc;

/// TLB 访问记录
#[derive(Clone, Debug)]
pub struct AccessRecord {
    /// 访问时间戳
    pub timestamp_us: u64,
    /// 访问类型
    pub access_type: AccessType,
    /// 访问频率
    pub frequency: u64,
}

/// TLB 一致性状态
#[derive(Clone, Debug, PartialEq, Eq, Copy)]
pub enum TLBConsistency {
    /// 有效
    Valid,
    /// 待刷新
    Pending,
    /// 无效
    Invalid,
}

/// 高性能异步 TLB 缓存
pub struct AsyncTLBCache {
    /// TLB 表项存储 (虚拟地址 -> (物理地址, 访问权限, 一致性状态))
    entries: Arc<RwLock<HashMap<GuestAddr, (GuestPhysAddr, AccessType, TLBConsistency)>>>,
    /// 访问记录 (用于 LRU)
    access_records: Arc<parking_lot::Mutex<HashMap<GuestAddr, AccessRecord>>>,
    /// TLB 容量
    capacity: usize,
    /// 预取队列大小
    prefetch_queue_size: usize,
    /// 统计信息
    stats: Arc<parking_lot::Mutex<TLBCacheStats>>,
}

/// TLB 缓存统计
#[derive(Clone, Debug, Default)]
pub struct TLBCacheStats {
    /// 命中次数
    pub hits: u64,
    /// 缺失次数
    pub misses: u64,
    /// 刷新次数
    pub flushes: u64,
    /// 批量刷新次数
    pub batch_flushes: u64,
    /// 预取次数
    pub prefetches: u64,
    /// 平均命中率
    pub hit_rate: f64,
}

impl AsyncTLBCache {
    /// 创建新的 TLB 缓存
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            access_records: Arc::new(parking_lot::Mutex::new(HashMap::new())),
            capacity,
            prefetch_queue_size: 100,
            stats: Arc::new(parking_lot::Mutex::new(TLBCacheStats::default())),
        }
    }

    /// 查找 TLB 表项
    pub fn lookup(&self, va: GuestAddr) -> Option<(GuestPhysAddr, AccessType)> {
        let mut entries = self.entries.write();

        if let Some(&(pa, access, consistency)) = entries.get(&va) {
            if consistency == TLBConsistency::Valid {
                let mut stats = self.stats.lock();
                stats.hits += 1;
                return Some((pa, access));
            }
        }

        let mut stats = self.stats.lock();
        stats.misses += 1;
        None
    }

    /// 异步查找（带预取提示）
    pub async fn lookup_async_with_hint(
        &self,
        va: GuestAddr,
        prefetch_addrs: Option<&[GuestAddr]>,
    ) -> Option<(GuestPhysAddr, AccessType)> {
        let result = self.lookup(va);

        // 如果提供了预取地址，异步处理
        if let Some(addrs) = prefetch_addrs {
            if result.is_none() {
                // 可以在这里触发异步预取
                let _ = self.async_prefetch(addrs).await;
            }
        }

        result
    }

    /// 异步预取（优化版：智能预取策略）
    ///
    /// 预取策略：
    /// 1. 顺序访问模式：预取后续页面
    /// 2. 跨页访问模式：预取相邻页面
    /// 3. 热点检测：预取频繁访问的页面
    /// 4. 自适应窗口：根据命中率调整预取窗口大小
    pub async fn async_prefetch(&self, addresses: &[GuestAddr]) -> Result<(), VmError> {
        let mut records = self.access_records.lock();
        let mut entries = self.entries.write();
        let mut stats = self.stats.lock();

        // 计算当前命中率
        let total_accesses = stats.hits + stats.misses;
        let hit_rate = if total_accesses > 0 {
            stats.hits as f64 / total_accesses as f64
        } else {
            0.0
        };

        // 自适应预取窗口：命中率低时增加预取
        let prefetch_window = if hit_rate < 0.90 {
            4 // 命中率低，增加预取
        } else if hit_rate < 0.95 {
            2 // 命中率中等
        } else {
            1 // 命中率高，减少预取
        };

        let mut prefetched = 0;

        for &addr in addresses.iter().take(self.prefetch_queue_size) {
            // 策略1：顺序访问预取（+1, +2, +4页面）
            for offset in [0x1000, 0x2000, 0x4000] {
                let prefetch_addr = addr.wrapping_add(offset);

                // 检查是否已在TLB中
                if entries.contains_key(&prefetch_addr) {
                    continue;
                }

                // 添加到预取队列（标记为待预取）
                records.insert(
                    prefetch_addr,
                    AccessRecord {
                        timestamp_us: 0, // 预取标记
                        access_type: AccessType::Read,
                        frequency: 1,
                    },
                );

                prefetched += 1;
                if prefetched >= prefetch_window {
                    break;
                }
            }

            // 策略2：基于访问历史的预取
            if let Some(record) = records.get(&addr) {
                // 如果访问频率高，预取更多
                if record.frequency > 5 {
                    for offset in [0x1000, 0x2000] {
                        let prefetch_addr = addr.wrapping_add(offset);
                        if !entries.contains_key(&prefetch_addr) {
                            records.insert(
                                prefetch_addr,
                                AccessRecord {
                                    timestamp_us: 0,
                                    access_type: AccessType::Read,
                                    frequency: 1,
                                },
                            );
                        }
                    }
                }
            }
        }

        stats.prefetches += prefetched;

        Ok(())
    }

    /// 智能预取（基于访问模式）
    ///
    /// 分析最近的访问模式，预测下一个可能访问的地址
    pub fn smart_prefetch(&self, current_addr: GuestAddr) -> Vec<GuestAddr> {
        let records = self.access_records.lock();
        let entries = self.entries.read(); // 只需要读，不需要mut
        let mut prefetch_candidates = Vec::new();

        // 策略1：顺序访问检测
        // 如果最近访问的地址是连续的，预取后续地址
        let recent_addrs: Vec<GuestAddr> = records.iter().take(10).map(|(&addr, _)| addr).collect();

        if let Some(&last_addr) = recent_addrs.last() {
            // 检查是否是顺序访问
            if current_addr == last_addr + 0x1000 {
                // 顺序访问，预取后续页面
                for i in 1..=3 {
                    let prefetch_addr = current_addr + (i * 0x1000);
                    if !entries.contains_key(&prefetch_addr) {
                        prefetch_candidates.push(prefetch_addr);
                    }
                }
            }
        }

        // 策略2：热点页面预取
        // 找出访问频率最高的页面，预取其相邻页面
        let mut hot_pages: Vec<(&GuestAddr, &AccessRecord)> = records
            .iter()
            .filter(|(_, record)| record.frequency > 3)
            .collect();
        hot_pages.sort_by(|a, b| b.1.frequency.cmp(&a.1.frequency));

        for (hot_addr, _) in hot_pages.iter().take(3) {
            let hot_addr = **hot_addr;
            // 预取热点页面的相邻页面
            for offset in [0x1000u64, 0x2000u64] {
                // 向前和向后预取
                let prefetch_addr_forward = hot_addr.wrapping_add(offset);
                let prefetch_addr_backward = hot_addr.wrapping_sub(offset);

                for prefetch_addr in [prefetch_addr_forward, prefetch_addr_backward] {
                    if !entries.contains_key(&prefetch_addr)
                        && !prefetch_candidates.contains(&prefetch_addr)
                    {
                        prefetch_candidates.push(prefetch_addr);
                    }
                }
            }
        }

        prefetch_candidates
    }

    /// 插入 TLB 表项
    pub fn insert(&self, va: GuestAddr, pa: GuestPhysAddr, access: AccessType) {
        let mut entries = self.entries.write();

        // 容量检查
        if entries.len() >= self.capacity {
            // 移除 LRU 表项
            if let Some(lru_va) = self.find_lru_entry() {
                entries.remove(&lru_va);
            }
        }

        entries.insert(va, (pa, access, TLBConsistency::Valid));

        // 记录访问
        let mut records = self.access_records.lock();
        records.insert(
            va,
            AccessRecord {
                timestamp_us: 0,
                access_type: access,
                frequency: 1,
            },
        );
    }

    /// 查找 LRU 表项
    fn find_lru_entry(&self) -> Option<GuestAddr> {
        let records = self.access_records.lock();
        records
            .iter()
            .min_by_key(|(_, record)| record.timestamp_us)
            .map(|(&va, _)| va)
    }

    /// 刷新单个 TLB 表项
    pub fn flush_entry(&self, va: GuestAddr) {
        let mut entries = self.entries.write();
        entries.remove(&va);

        let mut stats = self.stats.lock();
        stats.flushes += 1;
    }

    /// 批量刷新 TLB 表项
    pub async fn batch_flush(&self, addresses: &[GuestAddr]) -> Result<(), VmError> {
        let mut entries = self.entries.write();

        for &va in addresses {
            entries.remove(&va);
        }

        let mut stats = self.stats.lock();
        stats.batch_flushes += 1;
        stats.flushes += addresses.len() as u64;

        Ok(())
    }

    /// 选择性刷新（根据条件）
    pub async fn selective_flush<F>(&self, predicate: F) -> Result<u64, VmError>
    where
        F: Fn(&GuestAddr) -> bool,
    {
        let mut entries = self.entries.write();
        let mut count = 0;

        let addresses: Vec<_> = entries.keys().filter(|va| predicate(va)).copied().collect();

        for va in addresses {
            entries.remove(&va);
            count += 1;
        }

        let mut stats = self.stats.lock();
        stats.flushes += count;

        Ok(count)
    }

    /// 刷新所有 TLB
    pub fn flush_all(&self) {
        let mut entries = self.entries.write();
        entries.clear();

        let mut stats = self.stats.lock();
        stats.flushes += 1;
    }

    /// 标记为待刷新
    pub fn mark_pending(&self, va: GuestAddr) {
        let mut entries = self.entries.write();
        if let Some((pa, access, _)) = entries.get(&va).copied() {
            entries.insert(va, (pa, access, TLBConsistency::Pending));
        }
    }

    /// 批量标记为待刷新
    pub async fn batch_mark_pending(&self, addresses: &[GuestAddr]) -> Result<(), VmError> {
        let mut entries = self.entries.write();

        for &va in addresses {
            if let Some((pa, access, _)) = entries.get(&va).copied() {
                entries.insert(va, (pa, access, TLBConsistency::Pending));
            }
        }

        Ok(())
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> TLBCacheStats {
        let mut stats = self.stats.lock().clone();

        // 计算命中率
        let total = stats.hits + stats.misses;
        if total > 0 {
            stats.hit_rate = stats.hits as f64 / total as f64;
        }

        stats
    }

    /// 重置统计信息
    pub fn reset_stats(&self) {
        let mut stats = self.stats.lock();
        *stats = TLBCacheStats::default();
    }

    /// 获取 TLB 使用率
    pub fn get_occupancy(&self) -> f64 {
        let mut entries = self.entries.write();
        entries.len() as f64 / self.capacity as f64
    }
}

/// 并发 TLB 操作管理器
pub struct ConcurrentTLBManager {
    /// 主 TLB 缓存
    cache: Arc<AsyncTLBCache>,
    /// 待处理刷新操作
    pending_flushes: Arc<parking_lot::Mutex<Vec<(GuestAddr, TLBConsistency)>>>,
}

impl ConcurrentTLBManager {
    /// 创建新的并发 TLB 管理器
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: Arc::new(AsyncTLBCache::new(capacity)),
            pending_flushes: Arc::new(parking_lot::Mutex::new(Vec::new())),
        }
    }

    /// 异步查找
    pub async fn async_lookup(&self, va: GuestAddr) -> Option<(GuestPhysAddr, AccessType)> {
        self.cache.lookup(va)
    }

    /// 异步插入
    pub async fn async_insert(
        &self,
        va: GuestAddr,
        pa: GuestPhysAddr,
        access: AccessType,
    ) -> Result<(), VmError> {
        self.cache.insert(va, pa, access);
        Ok(())
    }

    /// 异步刷新
    pub async fn async_flush(&self, va: GuestAddr) -> Result<(), VmError> {
        self.cache.flush_entry(va);
        Ok(())
    }

    /// 处理待处理刷新
    pub async fn process_pending_flushes(&self) -> Result<usize, VmError> {
        let mut pending = self.pending_flushes.lock();
        let count = pending.len();

        for (va, _) in pending.drain(..) {
            self.cache.flush_entry(va);
        }

        Ok(count)
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> TLBCacheStats {
        self.cache.get_stats()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tlb_cache_creation() {
        let cache = AsyncTLBCache::new(256);
        assert_eq!(cache.capacity, 256);
        assert_eq!(cache.get_occupancy(), 0.0);
    }

    #[test]
    fn test_insert_and_lookup() {
        let cache = AsyncTLBCache::new(10);
        cache.insert(0x1000, 0x2000, AccessType::Read);

        let result = cache.lookup(0x1000);
        assert!(result.is_some());
        assert_eq!(result.unwrap().0, 0x2000);
    }

    #[test]
    fn test_flush_entry() {
        let cache = AsyncTLBCache::new(10);
        cache.insert(0x1000, 0x2000, AccessType::Read);

        cache.flush_entry(0x1000);
        assert!(cache.lookup(0x1000).is_none());
    }

    #[test]
    fn test_miss_tracking() {
        let cache = AsyncTLBCache::new(10);
        cache.lookup(0x5000); // miss
        cache.lookup(0x5000); // miss

        let stats = cache.get_stats();
        assert_eq!(stats.misses, 2);
    }

    #[test]
    fn test_hit_rate_calculation() {
        let cache = AsyncTLBCache::new(10);
        cache.insert(0x1000, 0x2000, AccessType::Read);

        cache.lookup(0x1000); // hit
        cache.lookup(0x1000); // hit
        cache.lookup(0x5000); // miss

        let stats = cache.get_stats();
        assert!(stats.hit_rate > 0.65 && stats.hit_rate < 0.75);
    }

    #[tokio::test]
    async fn test_batch_flush() {
        let cache = AsyncTLBCache::new(20);

        for i in 0..5 {
            cache.insert(
                0x1000 + (i * 0x1000) as u64,
                0x2000 + (i * 0x1000) as u64,
                AccessType::Read,
            );
        }

        let addresses = vec![0x1000, 0x2000, 0x3000];
        let result = cache.batch_flush(&addresses).await;
        assert!(result.is_ok());

        assert!(cache.lookup(0x1000).is_none());
        assert!(cache.lookup(0x2000).is_none());
    }

    #[tokio::test]
    async fn test_selective_flush() {
        let cache = AsyncTLBCache::new(20);

        cache.insert(0x1000, 0x2000, AccessType::Read);
        cache.insert(0x2000, 0x3000, AccessType::Store);
        cache.insert(0x3000, 0x4000, AccessType::Read);

        let count = cache.selective_flush(|va| *va < 0x2500).await;
        assert!(count.is_ok());
        assert_eq!(count.unwrap(), 2);
    }

    #[tokio::test]
    async fn test_concurrent_manager() {
        let manager = ConcurrentTLBManager::new(10);

        manager
            .async_insert(0x1000, 0x2000, AccessType::Read)
            .await
            .unwrap();
        let result = manager.async_lookup(0x1000).await;
        assert!(result.is_some());
    }

    #[test]
    fn test_occupancy() {
        let cache = AsyncTLBCache::new(10);
        cache.insert(0x1000, 0x2000, AccessType::Read);
        cache.insert(0x3000, 0x4000, AccessType::Read);

        let occupancy = cache.get_occupancy();
        assert!(occupancy > 0.1 && occupancy < 0.3);
    }
}

/// 异步TLB缓存适配器，实现TlbManager trait
///
/// 此适配器将AsyncTLBCache适配到TlbManager接口，使其可以与其他TLB实现互换使用。
///
/// # 适用场景
///
/// - **异步执行环境**: 当使用async/await进行异步内存访问时
/// - **批量刷新优化**: 需要异步批量刷新TLB条目的场景
/// - **选择性失效**: 需要细粒度控制TLB条目失效的场景
///
/// # 与其他TLB实现的对比
///
/// - `MultiLevelTlb` (vm-mem): 适用于高性能场景，支持多级缓存和预取
/// - `ConcurrentTlbManager` (vm-mem): 适用于高并发场景，使用无锁数据结构
/// - `AsyncTlbAdapter` (vm-core): 适用于异步场景，支持异步批量操作
pub struct AsyncTlbAdapter {
    cache: Arc<AsyncTLBCache>,
}

impl AsyncTlbAdapter {
    /// 创建新的异步TLB适配器
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: Arc::new(AsyncTLBCache::new(capacity)),
        }
    }

    /// 获取内部缓存引用
    pub fn cache(&self) -> &Arc<AsyncTLBCache> {
        &self.cache
    }
}

impl TlbManager for AsyncTlbAdapter {
    fn lookup(&mut self, addr: GuestAddr, _asid: u16, access: AccessType) -> Option<TlbEntry> {
        // AsyncTLBCache的lookup方法不接收asid和access参数
        // 我们需要先查找，然后检查权限
        if let Some((phys_addr, cached_access)) = self.cache.lookup(addr) {
            // 检查访问权限是否匹配
            if cached_access == access || access == AccessType::Read {
                Some(TlbEntry {
                    guest_addr: addr,
                    phys_addr,
                    flags: match cached_access {
                        AccessType::Read => 1 << 1,  // R bit
                        AccessType::Write => 1 << 2, // W bit
                        AccessType::Exec => 1 << 3,  // X bit
                    },
                    asid: _asid,
                })
            } else {
                None
            }
        } else {
            None
        }
    }

    fn update(&mut self, entry: TlbEntry) {
        // 从flags推断访问类型（简化实现）
        let access = if (entry.flags & (1 << 3)) != 0 {
            AccessType::Exec
        } else if (entry.flags & (1 << 2)) != 0 {
            AccessType::Write
        } else {
            AccessType::Read
        };
        self.cache.insert(entry.guest_addr, entry.phys_addr, access);
    }

    fn flush(&mut self) {
        self.cache.flush_all();
    }

    fn flush_asid(&mut self, asid: u16) {
        // AsyncTLBCache不直接支持ASID，我们需要刷新所有条目
        // 这是一个限制，但在某些场景下可以接受
        self.cache.flush_all();
    }
}
