//! 软件 TLB (Translation Lookaside Buffer)
//!
//! 缓存地址翻译结果，减少页表遍历开销

use crate::{GuestAddr, HostAddr};
use crate::mmu::{PageWalkResult, PageTableFlags};
use std::collections::HashMap;

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
    entries: Vec<Option<TlbEntry>>,
    /// 容量
    capacity: usize,
    /// 替换策略
    policy: TlbReplacePolicy,
    /// 下一个替换索引（用于 FIFO）
    next_replace: usize,
    /// 全局访问计数
    global_access: u64,
    /// 统计信息
    stats: TlbStats,
}

/// TLB 统计信息
#[derive(Debug, Clone, Default)]
pub struct TlbStats {
    /// 命中次数
    pub hits: u64,
    /// 未命中次数
    pub misses: u64,
    /// 失效次数
    pub flushes: u64,
}

impl TlbStats {
    /// 计算命中率
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
    /// 创建新的 TLB
    pub fn new(capacity: usize, policy: TlbReplacePolicy) -> Self {
        Self {
            entries: vec![None; capacity],
            capacity,
            policy,
            next_replace: 0,
            global_access: 0,
            stats: TlbStats::default(),
        }
    }

    /// 创建默认 TLB (1024 条目，LRU 策略)
    pub fn default() -> Self {
        Self::new(1024, TlbReplacePolicy::Lru)
    }

    /// 查找 TLB 条目
    pub fn lookup(&mut self, gva: GuestAddr, asid: u16) -> Option<&TlbEntry> {
        self.global_access += 1;

        for entry_opt in &mut self.entries {
            if let Some(entry) = entry_opt {
                if entry.asid == asid && entry.contains(gva) {
                    entry.access_count = self.global_access;
                    self.stats.hits += 1;
                    return Some(entry);
                }
            }
        }

        self.stats.misses += 1;
        None
    }

    /// 插入 TLB 条目
    pub fn insert(&mut self, walk_result: PageWalkResult, gva: GuestAddr, asid: u16) {
        let entry = TlbEntry {
            gva: gva & !(walk_result.page_size - 1), // 页对齐
            gpa: walk_result.gpa & !(walk_result.page_size - 1),
            page_size: walk_result.page_size,
            flags: walk_result.flags,
            access_count: self.global_access,
            asid,
        };

        // 查找空槽或选择替换目标
        let index = self.find_replace_index();
        self.entries[index] = Some(entry);
    }

    /// 查找替换索引
    fn find_replace_index(&mut self) -> usize {
        // 首先查找空槽
        for (i, entry) in self.entries.iter().enumerate() {
            if entry.is_none() {
                return i;
            }
        }

        // 根据策略选择替换目标
        match self.policy {
            TlbReplacePolicy::Random => {
                use std::collections::hash_map::RandomState;
                use std::hash::{BuildHasher, Hash, Hasher};
                let mut hasher = RandomState::new().build_hasher();
                self.global_access.hash(&mut hasher);
                (hasher.finish() as usize) % self.capacity
            }
            TlbReplacePolicy::Lru => {
                // 查找访问计数最小的条目
                let mut min_access = u64::MAX;
                let mut min_index = 0;
                
                for (i, entry_opt) in self.entries.iter().enumerate() {
                    if let Some(entry) = entry_opt {
                        if entry.access_count < min_access {
                            min_access = entry.access_count;
                            min_index = i;
                        }
                    }
                }
                
                min_index
            }
            TlbReplacePolicy::Fifo => {
                let index = self.next_replace;
                self.next_replace = (self.next_replace + 1) % self.capacity;
                index
            }
        }
    }

    /// 刷新整个 TLB
    pub fn flush_all(&mut self) {
        for entry in &mut self.entries {
            *entry = None;
        }
        self.stats.flushes += 1;
    }

    /// 刷新指定 ASID 的 TLB 条目
    pub fn flush_asid(&mut self, asid: u16) {
        for entry_opt in &mut self.entries {
            if let Some(entry) = entry_opt {
                if entry.asid == asid {
                    *entry_opt = None;
                }
            }
        }
        self.stats.flushes += 1;
    }

    /// 刷新指定地址的 TLB 条目 (invlpg)
    pub fn flush_page(&mut self, gva: GuestAddr, asid: u16) {
        for entry_opt in &mut self.entries {
            if let Some(entry) = entry_opt {
                if entry.asid == asid && entry.contains(gva) {
                    *entry_opt = None;
                    break;
                }
            }
        }
    }

    /// 获取统计信息
    pub fn stats(&self) -> &TlbStats {
        &self.stats
    }

    /// 重置统计信息
    pub fn reset_stats(&mut self) {
        self.stats = TlbStats::default();
    }

    /// 获取当前使用的条目数
    pub fn used_entries(&self) -> usize {
        self.entries.iter().filter(|e| e.is_some()).count()
    }

    /// 获取容量
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

/// 大页支持
pub mod hugepage {
    use super::*;

    /// 大页类型
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum HugePageSize {
        /// 2MB 大页
        Size2M,
        /// 1GB 大页
        Size1G,
    }

    impl HugePageSize {
        pub fn size(&self) -> u64 {
            match self {
                HugePageSize::Size2M => 2 * 1024 * 1024,
                HugePageSize::Size1G => 1024 * 1024 * 1024,
            }
        }

        pub fn alignment(&self) -> u64 {
            self.size()
        }
    }

    /// 大页分配器
    pub struct HugePageAllocator {
        /// 是否启用大页
        enabled: bool,
        /// 首选大页大小
        preferred_size: HugePageSize,
    }

    impl HugePageAllocator {
        /// 创建新的大页分配器
        pub fn new(enabled: bool, preferred_size: HugePageSize) -> Self {
            Self {
                enabled,
                preferred_size,
            }
        }

        /// 检查是否启用大页
        pub fn is_enabled(&self) -> bool {
            self.enabled
        }

        /// 获取首选大页大小
        pub fn preferred_size(&self) -> HugePageSize {
            self.preferred_size
        }

        /// 检查地址是否对齐
        pub fn is_aligned(&self, addr: u64) -> bool {
            addr % self.preferred_size.alignment() == 0
        }

        /// 对齐地址
        pub fn align_up(&self, addr: u64) -> u64 {
            let alignment = self.preferred_size.alignment();
            (addr + alignment - 1) & !(alignment - 1)
        }

        /// 对齐地址（向下）
        pub fn align_down(&self, addr: u64) -> u64 {
            let alignment = self.preferred_size.alignment();
            addr & !(alignment - 1)
        }

        /// 在 Linux 上分配大页
        #[cfg(target_os = "linux")]
        pub fn allocate_linux(&self, size: usize) -> Result<*mut u8, String> {
            if !self.enabled {
                return Err("Huge pages not enabled".to_string());
            }

            use std::ptr;
            
            // 使用 mmap 分配大页
            let flags = libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_HUGETLB;
            let prot = libc::PROT_READ | libc::PROT_WRITE;
            
            let addr = unsafe {
                libc::mmap(
                    ptr::null_mut(),
                    size,
                    prot,
                    flags,
                    -1,
                    0,
                )
            };

            if addr == libc::MAP_FAILED {
                Err("Failed to allocate huge pages".to_string())
            } else {
                Ok(addr as *mut u8)
            }
        }

        /// 在其他平台上的占位实现
        #[cfg(not(target_os = "linux"))]
        pub fn allocate_linux(&self, _size: usize) -> Result<*mut u8, String> {
            Err("Huge pages only supported on Linux".to_string())
        }
    }

    impl Default for HugePageAllocator {
        fn default() -> Self {
            Self::new(false, HugePageSize::Size2M)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mmu::PageTableFlags;

    #[test]
    fn test_tlb_lookup() {
        let mut tlb = SoftwareTlb::new(4, TlbReplacePolicy::Lru);
        
        // 插入条目
        let walk_result = PageWalkResult {
            gpa: 0x1000,
            page_size: 4096,
            flags: PageTableFlags::default(),
        };
        
        tlb.insert(walk_result, 0x2000, 0);
        
        // 查找应该命中
        let entry = tlb.lookup(0x2000, 0);
        assert!(entry.is_some());
        assert_eq!(tlb.stats().hits, 1);
        
        // 查找不存在的地址应该未命中
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

    #[test]
    fn test_hugepage_alignment() {
        let allocator = hugepage::HugePageAllocator::new(true, hugepage::HugePageSize::Size2M);
        
        assert!(allocator.is_aligned(0x200000));
        assert!(!allocator.is_aligned(0x1000));
        
        assert_eq!(allocator.align_up(0x1000), 0x200000);
        assert_eq!(allocator.align_down(0x201000), 0x200000);
    }
}
