#![allow(unused_variables)]
#![allow(dead_code)]

//! 软件 TLB (Translation Lookaside Buffer)
//!
//! 缓存地址翻译结果，减少页表遍历开销

use crate::{GuestAddr, HostAddr};
use crate::mmu::{PageWalkResult, PageTableFlags};
use std::collections::{HashMap, VecDeque};

/// TLB 条目
#[derive(Debug, Clone)]
pub struct TlbEntry {
    /// Guest 虚拟地址（页对齐）
    pub gva: GuestAddr,
    /// Guest 物理地址
    pub gpa: GuestAddr,
    /// 页面大小
    pub page_size: u64,
    /// 页表标志
    pub flags: PageTableFlags,
    /// 访问计数（用于 LRU）
    pub access_count: u64,
    /// ASID (Address Space ID)
    pub asid: u16,
}

impl TlbEntry {
    /// 检查地址是否在此条目范围内
    pub fn contains(&self, gva: GuestAddr) -> bool {
        let page_base = self.gva & !(self.page_size - 1);
        let gva_base = gva & !(self.page_size - 1);
        page_base == gva_base
    }

    /// 翻译地址
    pub fn translate(&self, gva: GuestAddr) -> GuestAddr {
        let offset = gva & (self.page_size - 1);
        self.gpa + offset
    }
}

/// TLB 替换策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlbReplacePolicy {
    /// 随机替换
    Random,
    /// 最近最少使用 (LRU)
    Lru,
    /// 先进先出 (FIFO)
    Fifo,
}

/// 软件 TLB
pub struct SoftwareTlb {
    /// TLB 条目
    entries: HashMap<(GuestAddr, u16), TlbEntry>,
    /// LRU 队列，存储键以实现高效的 LRU 替换
    lru_queue: VecDeque<(GuestAddr, u16)>,
    /// 容量
    capacity: usize,
    /// 替换策略
    policy: TlbReplacePolicy,
    /// 全局访问计数
    global_access: u64,
    /// 统计信息
    stats: TlbStats,
}

/// TLB 统计信息
#[derive(Debug, Clone, Default)]
pub struct TlbStats {
    pub hits: u64,
    pub misses: u64,
    pub flushes: u64,
}

impl TlbStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

impl SoftwareTlb {
    pub fn new(capacity: usize, policy: TlbReplacePolicy) -> Self {
        Self {
            entries: HashMap::with_capacity(capacity),
            lru_queue: VecDeque::with_capacity(capacity),
            capacity,
            policy,
            global_access: 0,
            stats: TlbStats::default(),
        }
    }

    pub fn default() -> Self {
        Self::new(4096, TlbReplacePolicy::Lru)
    }

    fn update_lru(&mut self, key: &(GuestAddr, u16)) {
        if let Some(pos) = self.lru_queue.iter().position(|x| x == key) {
            if let Some(k) = self.lru_queue.remove(pos) {
                self.lru_queue.push_back(k);
            }
        }
    }

    pub fn lookup(&mut self, gva: GuestAddr, asid: u16) -> Option<&TlbEntry> {
        self.global_access += 1;
        // 假设页大小为 4096 进行页对齐
        let page_base = gva & !(4096 - 1);
        let key = (page_base, asid);

        if self.entries.contains_key(&key) {
            self.stats.hits += 1;
            self.update_lru(&key);
            // 我们不能在这里返回可变引用，因为它会与 update_lru 中的可变借用冲突
            // 所以我们再次获取它
            self.entries.get(&key)
        } else {
            self.stats.misses += 1;
            None
        }
    }

    pub fn insert(&mut self, walk_result: PageWalkResult, gva: GuestAddr, asid: u16) {
        let entry = TlbEntry {
            gva: gva & !(walk_result.page_size - 1),
            gpa: walk_result.gpa & !(walk_result.page_size - 1),
            page_size: walk_result.page_size,
            flags: walk_result.flags,
            access_count: self.global_access,
            asid,
        };

        let key = (entry.gva, entry.asid);

        if self.entries.len() >= self.capacity {
            if let Some(lru_key) = self.lru_queue.pop_front() {
                self.entries.remove(&lru_key);
            }
        }
        self.entries.insert(key, entry);
        self.lru_queue.push_back(key);
    }

    pub fn flush_all(&mut self) {
        self.entries.clear();
        self.lru_queue.clear();
        self.stats.flushes += 1;
    }

    pub fn flush_asid(&mut self, asid: u16) {
        self.entries.retain(|_key, entry| entry.asid != asid);
        self.lru_queue.retain(|(_, entry_asid)| *entry_asid != asid);
        self.stats.flushes += 1;
    }

    pub fn flush_page(&mut self, gva: GuestAddr, asid: u16) {
        let page_base = gva & !(4096 - 1);
        let key = (page_base, asid);
        if self.entries.remove(&key).is_some() {
            self.lru_queue.retain(|k| *k != key);
        }
    }

    pub fn stats(&self) -> &TlbStats {
        &self.stats
    }

    pub fn reset_stats(&mut self) {
        self.stats = TlbStats::default();
    }

    pub fn used_entries(&self) -> usize {
        self.entries.len()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use crate::mmu::PageTableFlags;

    #[test]
    fn test_tlb_lookup() {
        let mut tlb = SoftwareTlb::new(4, TlbReplacePolicy::Lru);
        let walk_result = PageWalkResult {
            gpa: 0x1000,
            page_size: 4096,
            flags: PageTableFlags::default(),
        };
        tlb.insert(walk_result, 0x2000, 0);
        let entry = tlb.lookup(0x2000, 0);
        assert!(entry.is_some());
        assert_eq!(tlb.stats().hits, 1);
        let entry = tlb.lookup(0x3000, 0);
        assert!(entry.is_none());
        assert_eq!(tlb.stats().misses, 1);
    }

    #[test]
    fn test_tlb_flush() {
        let mut tlb = SoftwareTlb::new(4, TlbReplacePolicy::Lru);
        let walk_result = PageWalkResult {
            gpa: 0x1000,
            page_size: 4096,
            flags: PageTableFlags::default(),
        };
        tlb.insert(walk_result, 0x2000, 0);
        assert_eq!(tlb.used_entries(), 1);
        tlb.flush_all();
        assert_eq!(tlb.used_entries(), 0);
    }


}
