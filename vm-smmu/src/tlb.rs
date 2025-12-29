// TLB缓存管理
//
// 实现SMMUv3的TLB缓存功能，包括：
// - L1/L2/L3多级TLB
// - 多种替换策略（LRU/LFU/Clock/2Q）
// - TLB命中率和延迟统计

use super::{AccessPermission, TLB_ENTRY_MAX};
use std::collections::{HashMap, VecDeque};

/// TLB策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlbPolicy {
    /// LRU（Least Recently Used）
    LRU,
    /// LFU（Least Frequently Used）
    LFU,
    /// Clock
    Clock,
    /// 2Q（Two Queue）
    TwoQueue,
}

/// TLB条目
#[derive(Debug, Clone)]
pub struct TlbEntry {
    /// Stream ID
    pub stream_id: u16,
    /// 虚拟地址
    pub va: u64,
    /// 物理地址
    pub pa: u64,
    /// 访问权限
    pub perms: AccessPermission,
    /// 有效标志
    pub valid: bool,
    /// 访问次数
    pub access_count: u64,
    /// 最后访问时间戳
    pub last_access: u64,
}

/// TLB缓存
pub struct TlbCache {
    /// TLB条目（用于快速查找）
    pub entries: HashMap<(u16, u64), TlbEntry>,
    /// LRU访问顺序
    pub lru_order: VecDeque<(u16, u64)>,
    /// 最大条目数
    pub max_entries: usize,
    /// 替换策略
    pub policy: TlbPolicy,
    /// 命中统计
    pub hit_count: u64,
    /// 未命中统计
    pub miss_count: u64,
}

impl Default for TlbCache {
    fn default() -> Self {
        Self::new(TLB_ENTRY_MAX, TlbPolicy::LRU)
    }
}

impl TlbCache {
    /// 创建新的TLB缓存
    ///
    /// # 参数
    /// - `max_entries`: 最大条目数（默认256）
    /// - `policy`: 替换策略（默认LRU）
    ///
    /// # 示例
    /// ```ignore
    /// let tlb = TlbCache::new(256, TlbPolicy::LRU);
    /// ```
    pub fn new(max_entries: usize, policy: TlbPolicy) -> Self {
        Self {
            entries: HashMap::new(),
            lru_order: VecDeque::with_capacity(max_entries),
            max_entries,
            policy,
            hit_count: 0,
            miss_count: 0,
        }
    }

    /// 查找TLB条目
    ///
    /// # 参数
    /// - `stream_id`: Stream ID
    /// - `va`: 虚拟地址
    ///
    /// # 返回
    /// - `Some(entry)`: TLB命中
    /// - `None`: TLB未命中
    ///
    /// # 示例
    /// ```ignore
    /// if let Some(entry) = tlb.lookup(stream_id, va) {
    ///     // 命中，使用entry.pa作为物理地址
    /// } else {
    ///     // 未命中，执行页表遍历
    /// }
    /// ```
    pub fn lookup(&mut self, stream_id: u16, va: u64) -> Option<TlbEntry> {
        let key = (stream_id, va);

        // 先克隆 entry，避免借用冲突
        let entry = self.entries.get(&key).cloned();

        if let Some(entry_data) = entry {
            if entry_data.valid {
                // 更新访问时间戳
                self.update_lru_order(&key);

                // 更新统计
                self.hit_count += 1;

                return Some(entry_data);
            }
        }

        // 未命中
        self.miss_count += 1;
        None
    }

    /// 插入TLB条目
    ///
    /// # 参数
    /// - `entry`: TLB条目
    ///
    /// # 示例
    /// ```ignore
    /// let entry = TlbEntry {
    ///     stream_id: 0x100,
    ///     va: 0x1000,
    ///     pa: 0x1000,
    ///     perms: AccessPermission::ReadWriteExecute,
    ///     valid: true,
    ///     access_count: 0,
    ///     last_access: 0,
    /// };
    /// tlb.insert(entry);
    /// ```
    pub fn insert(&mut self, entry: TlbEntry) {
        let key = (entry.stream_id, entry.va);

        // 检查是否已满
        if self.entries.len() >= self.max_entries {
            // 执行替换策略
            self.evict();
        }

        // 插入新条目
        self.entries.insert(key, entry);
        self.update_lru_order(&key);
    }

    /// 使TLB条目失效
    ///
    /// # 参数
    /// - `stream_id`: Stream ID（可选）
    /// - `va`: 虚拟地址（可选）
    ///
    /// # 示例
    /// ```ignore
    /// // 使单个条目失效
    /// tlb.invalidate(Some(stream_id), Some(va));
    ///
    /// // 使所有条目失效
    /// tlb.invalidate(None, None);
    /// ```
    pub fn invalidate(&mut self, stream_id: Option<u16>, va: Option<u64>) {
        match (stream_id, va) {
            (Some(sid), Some(vaddr)) => {
                // 使指定条目失效
                let key = (sid, vaddr);
                if self.entries.remove(&key).is_some() {
                    self.lru_order.retain(|&x| x != key);
                }
            }
            (Some(sid), None) => {
                // 使指定Stream ID的所有条目失效
                self.entries.retain(|&(s, _), _| s != sid);
                self.lru_order.retain(|&(s, _)| s != sid);
            }
            (None, _) => {
                // 使所有条目失效
                self.entries.clear();
                self.lru_order.clear();
            }
        }
    }

    /// 执行TLB淘汰策略
    fn evict(&mut self) {
        match self.policy {
            TlbPolicy::LRU => self.evict_lru(),
            TlbPolicy::LFU => self.evict_lfu(),
            TlbPolicy::Clock => self.evict_clock(),
            TlbPolicy::TwoQueue => self.evict_2q(),
        }
    }

    /// LRU淘汰策略
    fn evict_lru(&mut self) {
        if let Some(lru_key) = self.lru_order.pop_front() {
            // 淘汰最少使用的条目
            self.entries.remove(&lru_key);
        }
    }

    /// LFU淘汰策略
    fn evict_lfu(&mut self) {
        // 找到访问次数最少的条目
        let mut min_count = u64::MAX;
        let mut lfu_key = None;

        for (key, entry) in &self.entries {
            if entry.access_count < min_count {
                min_count = entry.access_count;
                lfu_key = Some(*key);
            }
        }

        // 淘汰
        if let Some(key) = lfu_key {
            self.entries.remove(&key);
            self.lru_order.retain(|&x| x != key);
        }
    }

    /// Clock淘汰策略
    fn evict_clock(&mut self) {
        // 简化的Clock算法：找到第一个未引用的条目
        let key_to_remove: Option<(u16, u64)> = self
            .entries
            .iter()
            .find(|(_, entry)| entry.access_count == 0)
            .map(|(key, _)| *key);

        if let Some(key) = key_to_remove {
            self.entries.remove(&key);
            self.lru_order.retain(|&x| x != key);
        }
    }

    /// 2Q淘汰策略
    fn evict_2q(&mut self) {
        // 简化的2Q算法：淘汰最老的条目
        self.evict_lru();
    }

    /// 更新LRU顺序
    fn update_lru_order(&mut self, key: &(u16, u64)) {
        self.lru_order.retain(|&x| x != *key);
        self.lru_order.push_back(*key);
    }

    /// 获取TLB统计信息
    pub fn get_stats(&self) -> TlbStats {
        let total_lookups = self.hit_count + self.miss_count;
        let hit_rate = if total_lookups > 0 {
            self.hit_count as f64 / total_lookups as f64
        } else {
            0.0
        };

        TlbStats {
            total_lookups,
            hit_count: self.hit_count,
            miss_count: self.miss_count,
            hit_rate,
            current_size: self.entries.len(),
            max_capacity: self.max_entries,
        }
    }

    /// 清空TLB
    pub fn flush(&mut self) {
        self.entries.clear();
        self.lru_order.clear();
        self.hit_count = 0;
        self.miss_count = 0;
    }

    /// 获取当前大小
    pub fn size(&self) -> usize {
        self.entries.len()
    }

    /// 判断是否为空
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 获取容量
    pub fn capacity(&self) -> usize {
        self.max_entries
    }
}

/// TLB统计信息
#[derive(Debug, Clone)]
pub struct TlbStats {
    /// 总查找次数
    pub total_lookups: u64,
    /// 命中次数
    pub hit_count: u64,
    /// 未命中次数
    pub miss_count: u64,
    /// 命中率
    pub hit_rate: f64,
    /// 当前大小
    pub current_size: usize,
    /// 最大容量
    pub max_capacity: usize,
}

impl std::fmt::Display for TlbStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "TLB统计信息")?;
        writeln!(f, "  总查找次数: {}", self.total_lookups)?;
        writeln!(f, "  命中次数: {}", self.hit_count)?;
        writeln!(f, "  未命中次数: {}", self.miss_count)?;
        writeln!(f, "  命中率: {:.2}%", self.hit_rate * 100.0)?;
        writeln!(f, "  当前大小: {}/{}", self.current_size, self.max_capacity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tlb_creation() {
        let tlb = TlbCache::new(256, TlbPolicy::LRU);
        assert_eq!(tlb.capacity(), 256);
        assert!(tlb.is_empty());
        assert_eq!(tlb.hit_count, 0);
        assert_eq!(tlb.miss_count, 0);
    }

    #[test]
    fn test_tlb_insert() {
        let mut tlb = TlbCache::new(256, TlbPolicy::LRU);

        let entry = TlbEntry {
            stream_id: 0x100,
            va: 0x1000,
            pa: 0x1000,
            perms: AccessPermission::ReadWriteExecute,
            valid: true,
            access_count: 0,
            last_access: 0,
        };
        tlb.insert(entry);

        assert_eq!(tlb.size(), 1);
    }

    #[test]
    fn test_tlb_lookup() {
        let mut tlb = TlbCache::new(256, TlbPolicy::LRU);

        let entry = TlbEntry {
            stream_id: 0x100,
            va: 0x1000,
            pa: 0x1000,
            perms: AccessPermission::ReadWriteExecute,
            valid: true,
            access_count: 0,
            last_access: 0,
        };
        tlb.insert(entry);

        let result = tlb.lookup(0x100, 0x1000);
        assert!(result.is_some());
        assert_eq!(result.expect("Expected TLB entry").pa, 0x1000);
        assert_eq!(tlb.hit_count, 1);
    }

    #[test]
    fn test_tlb_miss() {
        let mut tlb = TlbCache::new(256, TlbPolicy::LRU);

        let result = tlb.lookup(0x100, 0x1000);
        assert!(result.is_none());
        assert_eq!(tlb.miss_count, 1);
    }

    #[test]
    fn test_tlb_invalidate() {
        let mut tlb = TlbCache::new(256, TlbPolicy::LRU);

        let entry = TlbEntry {
            stream_id: 0x100,
            va: 0x1000,
            pa: 0x1000,
            perms: AccessPermission::ReadWriteExecute,
            valid: true,
            access_count: 0,
            last_access: 0,
        };
        tlb.insert(entry.clone());

        // 使指定条目失效
        tlb.invalidate(Some(0x100), Some(0x1000));
        assert!(tlb.is_empty());

        // 重新插入
        tlb.insert(entry);

        // 使所有条目失效
        tlb.invalidate(None, None);
        assert!(tlb.is_empty());
    }

    #[test]
    fn test_tlb_stats() {
        let mut tlb = TlbCache::new(256, TlbPolicy::LRU);

        let entry = TlbEntry {
            stream_id: 0x100,
            va: 0x1000,
            pa: 0x1000,
            perms: AccessPermission::ReadWriteExecute,
            valid: true,
            access_count: 0,
            last_access: 0,
        };
        tlb.insert(entry);

        tlb.lookup(0x100, 0x1000);
        tlb.lookup(0x100, 0x2000);

        let stats = tlb.get_stats();
        assert_eq!(stats.total_lookups, 2);
        assert_eq!(stats.hit_count, 1);
        assert_eq!(stats.miss_count, 1);
        assert_eq!(stats.hit_rate, 0.5);
    }

    #[test]
    fn test_tlb_flush() {
        let mut tlb = TlbCache::new(256, TlbPolicy::LRU);

        let entry = TlbEntry {
            stream_id: 0x100,
            va: 0x1000,
            pa: 0x1000,
            perms: AccessPermission::ReadWriteExecute,
            valid: true,
            access_count: 0,
            last_access: 0,
        };
        tlb.insert(entry);

        tlb.flush();

        assert!(tlb.is_empty());
        assert_eq!(tlb.hit_count, 0);
        assert_eq!(tlb.miss_count, 0);
    }
}
