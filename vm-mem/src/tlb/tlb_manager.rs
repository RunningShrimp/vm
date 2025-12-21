//! TLB 管理器实现 - 从 SoftMmu 中分离出来

use lru::LruCache;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use vm_core::{AccessType, GuestAddr, TlbEntry};

/// TLB 管理器 trait
pub trait TlbManager {
    /// 查询地址对应的 TLB 条目
    fn lookup(&mut self, addr: GuestAddr, asid: u16, access: AccessType) -> Option<TlbEntry>;

    /// 更新或插入 TLB 条目
    fn update(&mut self, entry: TlbEntry);

    /// 清空 TLB
    fn flush(&mut self);

    /// 清除特定 ASID 的条目
    fn flush_asid(&mut self, asid: u16);
}

/// TLB 管理器的标准实现，使用 HashMap + LRU 替换策略
pub struct StandardTlbManager {
    /// 主哈希表存储 TLB 条目
    entries: HashMap<u64, TlbEntry>,
    /// LRU 缓存用于跟踪访问顺序和驱逐
    lru: LruCache<u64, ()>,
    /// 全局页条目 (不受 ASID 影响)
    global_entries: HashMap<u64, TlbEntry>,
    /// 最大容量
    max_size: usize,
    /// 统计：TLB 命中数
    pub hits: u64,
    /// 统计：TLB 缺失数
    pub misses: u64,
}

impl StandardTlbManager {
    /// 创建一个新的 TLB 管理器，指定容量
    pub fn new(capacity: usize) -> Self {
        let lru_capacity =
            NonZeroUsize::new(capacity).unwrap_or(NonZeroUsize::new(1).expect("Operation failed"));
        Self {
            entries: HashMap::with_capacity(capacity),
            lru: LruCache::new(lru_capacity),
            global_entries: HashMap::with_capacity(capacity / 4),
            max_size: capacity,
            hits: 0,
            misses: 0,
        }
    }

    /// 组合键: (vpn, asid) -> u64 键
    #[inline]
    fn make_key(addr: GuestAddr, asid: u16) -> u64 {
        // vpn 最多 44 位 (SV48), asid 16 位, 组合后不会溢出
        (addr.0 << 16) | (asid as u64)
    }

    /// 获取 TLB 统计
    pub fn stats(&self) -> (u64, u64) {
        (self.hits, self.misses)
    }
}

impl TlbManager for StandardTlbManager {
    /// 查找TLB条目（性能优化版本）
    ///
    /// 优化点：
    /// 1. 优先检查全局页（通常更少，查找更快）
    /// 2. 使用 get 而不是 contains_key + get（减少一次查找）
    /// 3. 内联函数减少调用开销
    #[inline]
    fn lookup(&mut self, addr: GuestAddr, asid: u16, _access: AccessType) -> Option<TlbEntry> {
        // 首先检查全局页 (不受 ASID 影响，通常数量较少)
        if let Some(entry) = self.global_entries.get(&addr.0) {
            self.hits += 1;
            return Some(*entry);
        }

        // 检查普通条目（优化：直接使用 get，避免 contains_key）
        let key = Self::make_key(addr, asid);
        if let Some(entry) = self.entries.get(&key) {
            // 更新 LRU（仅在命中时更新，减少开销）
            self.lru.get(&key);
            self.hits += 1;
            return Some(*entry);
        }

        self.misses += 1;
        None
    }

    fn update(&mut self, entry: TlbEntry) {
        // 全局页单独存储
        if entry.flags & (1u64 << 5) != 0 {
            // 全局标志 (G bit)
            self.global_entries.insert(entry.guest_addr.0, entry);
            return;
        }

        let key = Self::make_key(entry.guest_addr, entry.asid);

        // LRU 驱逐: 如果已满且是新条目
        if !self.entries.contains_key(&key)
            && self.entries.len() >= self.max_size
            && let Some((old_key, _)) = self.lru.pop_lru()
        {
            self.entries.remove(&old_key);
        }

        self.entries.insert(key, entry);
        self.lru.put(key, ());
    }

    fn flush(&mut self) {
        self.entries.clear();
        self.lru.clear();
        self.global_entries.clear();
    }

    fn flush_asid(&mut self, target_asid: u16) {
        // 收集需要删除的键
        let keys_to_remove: Vec<u64> = self
            .entries
            .iter()
            .filter(|(_, e)| e.asid == target_asid)
            .map(|(k, _)| *k)
            .collect();

        for key in keys_to_remove {
            self.entries.remove(&key);
            self.lru.pop(&key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GuestPhysAddr;

    #[test]
    fn test_tlb_lookup() {
        let mut tlb = StandardTlbManager::new(64);

        let entry = TlbEntry {
            guest_addr: GuestAddr(0x1000),
            phys_addr: GuestPhysAddr(0x2000),
            flags: 0x3, // R | V
            asid: 0,
        };

        tlb.update(entry);

        let result = tlb.lookup(GuestAddr(0x1000), 0, AccessType::Read);
        assert!(result.is_some());
        assert_eq!(
            result.expect("Operation failed").phys_addr,
            GuestPhysAddr(0x2000)
        );
        assert_eq!(tlb.stats().0, 1); // 1 hit
    }

    #[test]
    fn test_tlb_miss() {
        let mut tlb = StandardTlbManager::new(64);

        let result = tlb.lookup(GuestAddr(0x1000), 0, AccessType::Read);
        assert!(result.is_none());
        assert_eq!(tlb.stats().1, 1); // 1 miss
    }

    #[test]
    fn test_tlb_flush_asid() {
        let mut tlb = StandardTlbManager::new(64);

        let entry1 = TlbEntry {
            guest_addr: GuestAddr(0x1000),
            phys_addr: GuestPhysAddr(0x2000),
            flags: 0x3,
            asid: 1,
        };
        let entry2 = TlbEntry {
            guest_addr: GuestAddr(0x1000),
            phys_addr: GuestPhysAddr(0x3000),
            flags: 0x3,
            asid: 2,
        };

        tlb.update(entry1);
        tlb.update(entry2);

        tlb.flush_asid(1);

        // ASID 1 的条目应该被删除
        assert!(tlb.lookup(GuestAddr(0x1000), 1, AccessType::Read).is_none());
        // ASID 2 的条目应该仍然存在
        assert!(tlb.lookup(GuestAddr(0x1000), 2, AccessType::Read).is_some());
    }
}
