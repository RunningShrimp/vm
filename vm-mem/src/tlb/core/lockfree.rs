//! 无锁TLB实现
//!
//! 使用DashMap实现无锁TLB，支持高并发访问
//!
//! 性能目标：
//! - 读取性能提升 ≥ 30%
//! - 无数据竞争
//! - 支持高并发场景

use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;


/// TLB条目
#[derive(Debug, Clone, Copy)]
pub struct TlbEntry {
    /// 虚拟页号
    pub vpn: u64,
    /// 物理页号
    pub ppn: u64,
    /// 标志位
    pub flags: u64,
    /// 地址空间ID
    pub asid: u16,
    /// 时间戳
    pub timestamp: u64,
}

impl TlbEntry {
    pub fn new(vpn: u64, ppn: u64, flags: u64, asid: u16) -> Self {
        Self {
            vpn,
            ppn,
            flags,
            asid,
            timestamp: 0,
        }
    }
}

/// TLB键（用于DashMap）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct TlbKey {
    vpn: u64,
    asid: u16,
}

/// TLB值（用于DashMap）
#[derive(Debug, Clone, Copy)]
struct TlbValue {
    ppn: u64,
    flags: u64,
    timestamp: u64,
}

impl TlbValue {
    fn from_entry(entry: &TlbEntry) -> Self {
        Self {
            ppn: entry.ppn,
            flags: entry.flags,
            timestamp: entry.timestamp,
        }
    }
}

/// TLB统计信息
#[derive(Debug, Default)]
pub struct AtomicTlbStats {
    hits: AtomicU64,
    misses: AtomicU64,
    inserts: AtomicU64,
    flushes: AtomicU64,
}

impl AtomicTlbStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_hit(&self) {
        self.hits.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_miss(&self) {
        self.misses.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_insert(&self) {
        self.inserts.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_flush(&self) {
        self.flushes.fetch_add(1, Ordering::Relaxed);
    }

    pub fn hits(&self) -> u64 {
        self.hits.load(Ordering::Relaxed)
    }

    pub fn misses(&self) -> u64 {
        self.misses.load(Ordering::Relaxed)
    }

    pub fn inserts(&self) -> u64 {
        self.inserts.load(Ordering::Relaxed)
    }

    pub fn flushes(&self) -> u64 {
        self.flushes.load(Ordering::Relaxed)
    }

    pub fn total_accesses(&self) -> u64 {
        self.hits() + self.misses()
    }

    pub fn hit_rate(&self) -> f64 {
        let total = self.total_accesses();
        if total == 0 {
            return 0.0;
        }
        self.hits() as f64 / total as f64
    }
}

/// TLB分片（减少竞争）
struct Shard {
    entries: DashMap<TlbKey, TlbValue>,
    stats: Arc<AtomicTlbStats>,
}

impl Shard {
    fn new(stats: Arc<AtomicTlbStats>) -> Self {
        Self {
            entries: DashMap::new(),
            stats,
        }
    }
}

/// 无锁TLB
pub struct LockFreeTlb {
    shards: Vec<Arc<Shard>>,
    shard_mask: usize,
    timestamp: AtomicU64,
}

impl LockFreeTlb {
    /// 创建新的无锁TLB
    ///
    /// # 参数
    /// - `shard_count`: 分片数量，必须是2的幂（默认16）
    pub fn new() -> Self {
        Self::with_shards(16)
    }

    pub fn with_shards(shard_count: usize) -> Self {
        assert!(shard_count.is_power_of_two(), "shard_count must be power of 2");

        let stats = Arc::new(AtomicTlbStats::new());
        let shards = (0..shard_count)
            .map(|_| Arc::new(Shard::new(Arc::clone(&stats))))
            .collect();

        Self {
            shards,
            shard_mask: shard_count - 1,
            timestamp: AtomicU64::new(0),
        }
    }

    /// 计算分片索引
    #[inline]
    fn shard_index(&self, vpn: u64, _asid: u16) -> usize {
        // 使用VPN和ASID的哈希来选择分片
        let hash = vpn.wrapping_mul(0x9e3779b97f4a7c15u64);
        ((hash >> 20) as usize) & self.shard_mask
    }

    /// 查找TLB条目（无锁）
    pub fn lookup(&self, vpn: u64, asid: u16) -> Option<TlbEntry> {
        let shard_idx = self.shard_index(vpn, asid);
        let shard = &self.shards[shard_idx];

        let key = TlbKey { vpn, asid };

        if let Some(value) = shard.entries.get(&key) {
            shard.stats.record_hit();
            Some(TlbEntry {
                vpn,
                ppn: value.ppn,
                flags: value.flags,
                asid,
                timestamp: value.timestamp,
            })
        } else {
            shard.stats.record_miss();
            None
        }
    }

    /// 插入TLB条目（无锁）
    pub fn insert(&self, entry: TlbEntry) {
        let shard_idx = self.shard_index(entry.vpn, entry.asid);
        let shard = &self.shards[shard_idx];

        let key = TlbKey {
            vpn: entry.vpn,
            asid: entry.asid,
        };

        let mut value = TlbValue::from_entry(&entry);
        value.timestamp = self.timestamp.fetch_add(1, Ordering::Relaxed);

        shard.entries.insert(key, value);
        shard.stats.record_insert();
    }

    /// 批量查找（优化）
    pub fn lookup_batch(&self, requests: &[(u64, u16)]) -> Vec<Option<TlbEntry>> {
        requests
            .iter()
            .map(|&(vpn, asid)| self.lookup(vpn, asid))
            .collect()
    }

    /// 批量插入（优化）
    pub fn insert_batch(&self, entries: &[TlbEntry]) {
        for entry in entries {
            self.insert(*entry);
        }
    }

    /// 刷新指定ASID的所有条目
    pub fn flush_asid(&self, asid: u16) {
        for shard in &self.shards {
            // 收集需要删除的键
            let keys_to_remove: Vec<_> = shard
                .entries
                .iter()
                .filter(|entry| entry.key().asid == asid)
                .map(|entry| *entry.key())
                .collect();

            // 删除条目
            for key in keys_to_remove {
                shard.entries.remove(&key);
            }
        }

        // 记录刷新
        self.shards[0].stats.record_flush();
    }

    /// 刷新所有条目
    pub fn flush(&self) {
        for shard in &self.shards {
            shard.entries.clear();
        }

        self.shards[0].stats.record_flush();
    }

    /// 使单个条目失效
    pub fn invalidate(&self, vpn: u64, asid: u16) -> bool {
        let shard_idx = self.shard_index(vpn, asid);
        let shard = &self.shards[shard_idx];

        let key = TlbKey { vpn, asid };
        shard.entries.remove(&key).is_some()
    }

    /// 获取统计信息
    pub fn stats(&self) -> &AtomicTlbStats {
        &self.shards[0].stats
    }
}

impl Default for LockFreeTlb {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_lookup() {
        let tlb = LockFreeTlb::new();

        let entry = TlbEntry::new(0x1000, 0x2000, 0x1, 0);
        tlb.insert(entry);

        let result = tlb.lookup(0x1000, 0);
        assert!(result.is_some());
        assert_eq!(result.unwrap().ppn, 0x2000);
    }

    #[test]
    fn test_miss() {
        let tlb = LockFreeTlb::new();
        let result = tlb.lookup(0x1000, 0);
        assert!(result.is_none());
    }

    #[test]
    fn test_flush() {
        let tlb = LockFreeTlb::new();

        for i in 0..100 {
            let entry = TlbEntry::new(i * 4096, i * 4096, 0x1, 0);
            tlb.insert(entry);
        }

        tlb.flush();

        for i in 0..100 {
            let result = tlb.lookup(i * 4096, 0);
            assert!(result.is_none());
        }
    }

    #[test]
    fn test_batch_operations() {
        let tlb = LockFreeTlb::new();

        let entries: Vec<_> = (0..10)
            .map(|i| TlbEntry::new(i * 4096, i * 4096, 0x1, 0))
            .collect();

        tlb.insert_batch(&entries);

        let requests: Vec<_> = (0..10).map(|i| (i * 4096, 0)).collect();
        let results = tlb.lookup_batch(&requests);

        assert_eq!(results.len(), 10);
        for result in results {
            assert!(result.is_some());
        }
    }

    #[test]
    fn test_concurrent_access() {
        use std::thread;

        let tlb = Arc::new(LockFreeTlb::new());
        let barrier = Arc::new(std::sync::Barrier::new(10));
        let mut handles = vec![];

        // 10个线程并发访问
        for thread_id in 0..10 {
            let tlb_clone = Arc::clone(&tlb);
            let barrier_clone = Arc::clone(&barrier);

            handles.push(thread::spawn(move || {
                barrier_clone.wait();

                for i in 0..100 {
                    let entry = TlbEntry::new(
                        (thread_id * 100 + i) * 4096,
                        (thread_id * 100 + i) * 4096,
                        0x1,
                        thread_id as u16,
                    );
                    tlb_clone.insert(entry);

                    let result = tlb_clone.lookup(entry.vpn, entry.asid);
                    assert!(result.is_some());
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // 验证统计信息
        let stats = tlb.stats();
        assert_eq!(stats.inserts(), 1000);
        assert!(stats.hits() >= 1000);
    }
}
