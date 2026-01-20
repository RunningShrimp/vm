//! 跨架构翻译缓存
//!
//! 使用 LRU 策略缓存已翻译的指令，减少重复翻译开销。
//! 目标：缓存命中率 > 80%

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use vm_core::GuestAddr;

/// 翻译缓存条目
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// 客户端地址（源架构）
    pub guest_addr: GuestAddr,
    /// 翻译后的指令字节
    pub translated_bytes: Vec<u8>,
    /// 指令长度（字节）
    pub instruction_length: usize,
    /// 翻译时间戳
    pub timestamp: Instant,
    /// 访问次数
    pub access_count: u64,
}

/// LRU 翻译缓存
///
/// 使用 LRU (Least Recently Used) 策略管理缓存条目。
pub struct TranslationCache {
    /// 主缓存：guest_addr -> CacheEntry
    cache: HashMap<GuestAddr, CacheEntry>,
    /// LRU 链表（用于驱逐）
    lru_list: VecDeque<GuestAddr>,
    /// 缓存容量
    capacity: usize,
    /// 统计信息
    stats: Arc<RwLock<CacheStats>>,
    /// 缓存预热时间
    warmup_time: Duration,
}

/// 缓存统计信息
#[derive(Debug, Default)]
pub struct CacheStats {
    /// 缓存命中次数
    pub hits: AtomicU64,
    /// 缓存未命中次数
    pub misses: AtomicU64,
    /// 缓存驱逐次数
    pub evictions: AtomicU64,
    /// 总插入次数
    pub inserts: AtomicU64,
    /// 预热开始时间
    pub warmup_start: Option<Instant>,
}

use std::sync::atomic::{AtomicU64, Ordering};

impl TranslationCache {
    /// 创建新的翻译缓存
    ///
    /// # 参数
    /// - `capacity`: 缓存容量（条目数）
    /// - `warmup_time`: 预热时间
    pub fn new(capacity: usize, warmup_time: Duration) -> Self {
        Self {
            cache: HashMap::new(),
            lru_list: VecDeque::with_capacity(capacity),
            capacity,
            stats: Arc::new(RwLock::new(CacheStats {
                warmup_start: Some(Instant::now()),
                ..Default::default()
            })),
            warmup_time,
        }
    }

    /// 使用默认配置创建
    ///
    /// 默认容量：10000 条目
    /// 默认预热时间：5 秒
    pub fn with_defaults() -> Self {
        Self::new(10_000, Duration::from_secs(5))
    }

    /// 查找缓存条目
    pub fn lookup(&mut self, guest_addr: GuestAddr) -> Option<CacheEntry> {
        if let Some(mut entry) = self.cache.remove(&guest_addr) {
            // 更新访问统计
            entry.access_count += 1;
            entry.timestamp = Instant::now();

            // 从 LRU 链表中移除旧条目（如果存在）
            self.lru_list.retain(|&addr| addr != guest_addr);

            // 重新插入（更新 LRU）
            self.cache.insert(guest_addr, entry.clone());
            self.lru_list.push_back(guest_addr);

            // 更新统计
            if let Ok(stats) = self.stats.write() {
                stats.hits.fetch_add(1, Ordering::Relaxed);
            }

            Some(entry)
        } else {
            if let Ok(stats) = self.stats.write() {
                stats.misses.fetch_add(1, Ordering::Relaxed);
            }
            None
        }
    }

    /// 插入缓存条目
    pub fn insert(
        &mut self,
        guest_addr: GuestAddr,
        translated_bytes: Vec<u8>,
        instruction_length: usize,
    ) {
        // 检查是否需要驱逐
        if self.cache.len() >= self.capacity && !self.cache.contains_key(&guest_addr) {
            self.evict_lru();
        }

        let entry = CacheEntry {
            guest_addr,
            translated_bytes,
            instruction_length,
            timestamp: Instant::now(),
            access_count: 1,
        };

        self.cache.insert(guest_addr, entry.clone());
        self.lru_list.push_back(guest_addr);

        if let Ok(stats) = self.stats.write() {
            stats.inserts.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// 驱逐最久未使用的条目
    fn evict_lru(&mut self) {
        if let Some(old_addr) = self.lru_list.pop_front() {
            self.cache.remove(&old_addr);
            if let Ok(stats) = self.stats.write() {
                stats.evictions.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// 批量预热缓存
    ///
    /// 预加载常用的指令翻译结果。
    pub fn warmup(&mut self, entries: Vec<(GuestAddr, Vec<u8>, usize)>) {
        for (addr, bytes, len) in entries {
            if self.cache.len() < self.capacity {
                self.insert(addr, bytes, len);
            }
        }
    }

    /// 清空缓存
    pub fn clear(&mut self) {
        self.cache.clear();
        self.lru_list.clear();
    }

    /// 获取缓存命中率
    pub fn hit_rate(&self) -> f64 {
        let stats = self.stats.read().unwrap();
        let hits = stats.hits.load(Ordering::Relaxed);
        let misses = stats.misses.load(Ordering::Relaxed);
        let total = hits + misses;

        if total == 0 {
            return 0.0;
        }

        hits as f64 / total as f64
    }

    /// 检查是否已预热
    pub fn is_warmed_up(&self) -> bool {
        let stats = self.stats.read().unwrap();
        if let Some(start) = stats.warmup_start {
            start.elapsed() >= self.warmup_time
        } else {
            false
        }
    }

    /// 获取缓存大小
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// 检查缓存是否为空
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> CacheStatsSnapshot {
        let stats = self.stats.read().unwrap();
        CacheStatsSnapshot {
            hits: stats.hits.load(Ordering::Relaxed),
            misses: stats.misses.load(Ordering::Relaxed),
            evictions: stats.evictions.load(Ordering::Relaxed),
            inserts: stats.inserts.load(Ordering::Relaxed),
            hit_rate: self.hit_rate(),
            size: self.cache.len(),
            capacity: self.capacity,
        }
    }
}

/// 缓存统计快照
#[derive(Debug, Clone)]
pub struct CacheStatsSnapshot {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub inserts: u64,
    pub hit_rate: f64,
    pub size: usize,
    pub capacity: usize,
}

#[cfg(test)]
mod tests {
    use vm_core::GuestAddr;

    use super::*;

    #[test]
    fn test_cache_creation() {
        let cache = TranslationCache::with_defaults();
        assert_eq!(cache.capacity, 10_000);
        assert!(cache.is_empty());
    }

    #[test]
    fn test_cache_insert_and_lookup() {
        let mut cache = TranslationCache::new(10, Duration::from_secs(1));

        // 插入条目
        cache.insert(GuestAddr(0x1000), vec![0x90, 0x90], 2);
        cache.insert(GuestAddr(0x1002), vec![0xB8, 0x01, 0x00, 0x00, 0x00], 5);

        // 查找条目
        let entry1 = cache.lookup(GuestAddr(0x1000));
        assert!(entry1.is_some());
        assert_eq!(entry1.unwrap().instruction_length, 2);

        let entry2 = cache.lookup(GuestAddr(0x1002));
        assert!(entry2.is_some());
        assert_eq!(entry2.unwrap().instruction_length, 5);

        // 未命中
        let entry3 = cache.lookup(GuestAddr(0x2000));
        assert!(entry3.is_none());
    }

    #[test]
    fn test_cache_hit_rate() {
        let mut cache = TranslationCache::new(10, Duration::from_secs(1));

        // 插入 1 个条目
        cache.insert(GuestAddr(0x1000), vec![0x90], 1);

        // 查找 4 次命中，1 次未命中
        for _ in 0..4 {
            cache.lookup(GuestAddr(0x1000));
        }
        cache.lookup(GuestAddr(0x2000));

        // 命中率应该是 80%
        let hit_rate = cache.hit_rate();
        assert!((hit_rate - 0.8).abs() < 0.01);

        // 验证统计信息
        let stats = cache.get_stats();
        assert_eq!(stats.hits, 4);
        assert_eq!(stats.misses, 1);
        assert!((stats.hit_rate - 0.8).abs() < 0.01);
    }

    #[test]
    fn test_cache_eviction() {
        let mut cache = TranslationCache::new(3, Duration::from_secs(1));

        // 插入 3 个条目（达到容量上限）
        cache.insert(GuestAddr(0x1000), vec![0x90], 1);
        cache.insert(GuestAddr(0x1001), vec![0x90], 1);
        cache.insert(GuestAddr(0x1002), vec![0x90], 1);

        assert_eq!(cache.len(), 3);

        // 插入第 4 个条目（应该驱逐最旧的）
        cache.insert(GuestAddr(0x1003), vec![0x90], 1);

        assert_eq!(cache.len(), 3);

        // 第一个条目应该被驱逐
        assert!(cache.lookup(GuestAddr(0x1000)).is_none());

        // 其他条目应该仍然存在
        assert!(cache.lookup(GuestAddr(0x1001)).is_some());
        assert!(cache.lookup(GuestAddr(0x1002)).is_some());
        assert!(cache.lookup(GuestAddr(0x1003)).is_some());

        // 验证驱逐统计
        let stats = cache.get_stats();
        assert_eq!(stats.evictions, 1);
    }

    #[test]
    fn test_cache_warmup() {
        let mut cache = TranslationCache::with_defaults();

        let entries = vec![
            (GuestAddr(0x1000), vec![0x90], 1),
            (GuestAddr(0x1001), vec![0x90], 1),
            (GuestAddr(0x1002), vec![0x90], 1),
        ];

        cache.warmup(entries);

        assert_eq!(cache.len(), 3);
        assert!(cache.lookup(GuestAddr(0x1000)).is_some());
        assert!(cache.lookup(GuestAddr(0x1001)).is_some());
        assert!(cache.lookup(GuestAddr(0x1002)).is_some());
    }

    #[test]
    fn test_cache_clear() {
        let mut cache = TranslationCache::new(10, Duration::from_secs(1));

        cache.insert(GuestAddr(0x1000), vec![0x90], 1);
        cache.insert(GuestAddr(0x1001), vec![0x90], 1);

        assert_eq!(cache.len(), 2);

        cache.clear();

        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());
    }

    #[test]
    fn test_cache_warmed_up() {
        let cache = TranslationCache::new(10, Duration::from_millis(100));

        // 初始状态：未预热
        assert!(!cache.is_warmed_up());

        // 等待预热时间
        std::thread::sleep(Duration::from_millis(150));

        // 应该已经预热
        assert!(cache.is_warmed_up());
    }

    #[test]
    fn test_lru_behavior() {
        let mut cache = TranslationCache::new(3, Duration::from_secs(1));

        // 插入 3 个条目
        cache.insert(GuestAddr(0x1000), vec![0x90], 1);
        cache.insert(GuestAddr(0x1001), vec![0x90], 1);
        cache.insert(GuestAddr(0x1002), vec![0x90], 1);

        // 访问 0x1000（使其变为最近使用）
        cache.lookup(GuestAddr(0x1000));

        // 插入第 4 个条目（应该驱逐 0x1001，因为它是 LRU）
        cache.insert(GuestAddr(0x1003), vec![0x90], 1);

        // 0x1000 应该仍在缓存中（最近访问过）
        assert!(cache.lookup(GuestAddr(0x1000)).is_some());

        // 0x1001 应该被驱逐
        assert!(cache.lookup(GuestAddr(0x1001)).is_none());

        // 0x1002 和 0x1003 应该仍在缓存中
        assert!(cache.lookup(GuestAddr(0x1002)).is_some());
        assert!(cache.lookup(GuestAddr(0x1003)).is_some());
    }
}
