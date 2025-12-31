//! Asynchronous TLB implementation module.

#![cfg(feature = "async")]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use crate::{AccessType, TlbEntry, VmError};

/// Trait defining the asynchronous interface for a Translation Lookaside Buffer (TLB).
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
        let entries = self.entries.write();

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
    /// # Arguments
    /// - `va`: The virtual address to translate.
    /// - `access`: The type of access (read, write, execute).
    ///
    /// # Returns
    /// - `Ok(paddr)`: The translated physical address if the translation was successful.
    /// - `Err(VmError)`: An error if the translation failed (e.g., page fault).
    async fn translate(&mut self, va: u64, access: AccessType) -> Result<u64, VmError>;

    /// Asynchronously update the TLB with a new translation entry.
    ///
    /// # Arguments
    /// - `va`: The virtual address to associate with the entry.
    /// - `entry`: The TLB entry containing the physical address and other metadata.
    ///
    /// # Returns
    /// - `Ok(())`: If the entry was successfully added to the TLB.
    /// - `Err(VmError)`: An error if the entry could not be added.
    async fn update(&mut self, va: u64, entry: TlbEntry) -> Result<(), VmError>;

    /// Asynchronously flush a specific TLB entry by virtual address.
    ///
    /// # Arguments
    /// - `va`: The virtual address of the entry to flush.
    ///
    /// # Returns
    /// - `Ok(())`: If the entry was successfully flushed.
    /// - `Err(VmError)`: An error if the entry could not be flushed.
    async fn flush(&mut self, va: u64) -> Result<(), VmError>;

    /// Asynchronously flush all TLB entries.
    ///
    /// # Returns
    /// - `Ok(())`: If all entries were successfully flushed.
    /// - `Err(VmError)`: An error if the TLB could not be flushed.
    async fn flush_all(&mut self) -> Result<(), VmError>;

    /// Asynchronously flush TLB entries by ASID.
    ///
    /// # Arguments
    /// - `asid`: The Address Space Identifier (ASID) of the entries to flush.
    ///
    /// # Returns
    /// - `Ok(())`: If the entries were successfully flushed.
    /// - `Err(VmError)`: An error if the entries could not be flushed.
    async fn flush_asid(&mut self, asid: u16) -> Result<(), VmError>;

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
        let entries = self.entries.write();
        entries.len() as f64 / self.capacity as f64
    }
}

/// A virtual page key used for TLB lookups.
///
/// This combines the virtual address and ASID to form a unique key for TLB entries.
#[derive(Eq, PartialEq, Hash, Clone, Debug)]
pub struct VirtPageKey {
    pub va: u64,
    pub asid: u16,
}

impl VirtPageKey {
    /// Create a new VirtPageKey.
    ///
    /// # Arguments
    /// - `va`: The virtual address (page-aligned).
    /// - `asid`: The Address Space Identifier.
    pub fn new(va: u64, asid: u16) -> Self {
        // Ensure the address is page-aligned.
        let page_aligned_va = va & !(0x1000 - 1);
        Self {
            va: page_aligned_va,
            asid,
        }
    }
}

impl From<u64> for VirtPageKey {
    /// Create a VirtPageKey from a virtual address with default ASID 0.
    ///
    /// # Arguments
    /// - `va`: The virtual address.
    fn from(va: u64) -> Self {
        Self::new(va, 0)
    }
}

/// A simple asynchronous TLB implementation.
pub struct AsyncSimpleTlb {
    entries: Arc<Mutex<HashMap<VirtPageKey, TlbEntry>>>,
}

impl AsyncSimpleTlb {
    /// Create a new simple asynchronous TLB.
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Acquire the lock with proper error handling.
    ///
    /// Returns a guard for the entries HashMap, or an error if the lock is poisoned.
    fn lock_entries(&self) -> Result<std::sync::MutexGuard<HashMap<VirtPageKey, TlbEntry>>, VmError> {
        self.entries.lock().map_err(|_| VmError::Memory(crate::MemoryError::PageTableError {
            message: "TLB lock is poisoned".to_string(),
            level: None,
        }))
    }
}

#[async_trait]
impl AsyncTranslationLookasideBuffer for AsyncSimpleTlb {
    async fn translate(&mut self, va: u64, access: AccessType) -> Result<u64, VmError> {
        let entry = self
            .lock_entries()?
            .get(&VirtPageKey::from(va))
            .cloned();

        match entry {
            Some(entry) => {
                // Check access permissions.
                // Note: For simplicity, we'll assume all entries have full permissions.
                // In a real implementation, you'd check the flags field.

                // Calculate the physical address.
                let offset = va - (va & !(0x1000 - 1));
                Ok(entry.phys_addr + offset)
            }
            None => {
                // Return a page not found error.
                Err(crate::VmError::Memory(crate::MemoryError::PageTableError {
                    message: "Page not found in TLB".to_string(),
                    level: None,
                }))
            }
        }
    }

    async fn update(&mut self, va: u64, entry: TlbEntry) -> Result<(), VmError> {
        let key = VirtPageKey::new(entry.guest_addr, entry.asid);
        self.lock_entries()?.insert(key, entry);
        Ok(())
    }

    async fn flush(&mut self, va: u64) -> Result<(), VmError> {
        self.lock_entries()?.remove(&VirtPageKey::from(va));
        Ok(())
    }

    async fn flush_all(&mut self) -> Result<(), VmError> {
        self.lock_entries()?.clear();
        Ok(())
    }

    async fn flush_asid(&mut self, asid: u16) -> Result<(), VmError> {
        self.lock_entries()?
            .retain(|key, _| key.asid != asid);
        Ok(())
    }

    async fn flush_range(&mut self, start_va: u64, end_va: u64) -> Result<(), VmError> {
        let start_key = VirtPageKey::from(start_va);
        let end_key = VirtPageKey::from(end_va);

        self.lock_entries()?
            .retain(|key, _| key.va < start_key.va || key.va > end_key.va);

        Ok(())
    }
}

/// Architecture-specific TLB implementations can be added here.
/// For example:
/// - X86_64AsyncTlb
/// - Arm64AsyncTlb
/// - Riscv64AsyncTlb

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AccessType;

    #[tokio::test]
    async fn test_async_simple_tlb_translate() {
        // Create a new asynchronous TLB.
        let mut tlb = AsyncSimpleTlb::new();

        // Define a test translation entry.
        let va = 0x12345000;
        let entry = TlbEntry {
            guest_addr: va,
            phys_addr: 0x56789000,
            flags: 0x00000003, // Read/Write permissions.
            asid: 0,
        };

        // Update the TLB with the entry.
        tlb.update(va, entry).await.expect("Failed to update TLB");

        // Translate the virtual address.
        let pa = tlb.translate(va, AccessType::Read).await.expect("Failed to translate address");

        // Verify the translation result.
        assert_eq!(pa, 0x56789000);
    }

    #[tokio::test]
    async fn test_async_simple_tlb_flush() {
        // Create a new asynchronous TLB.
        let mut tlb = AsyncSimpleTlb::new();

        // Define a test translation entry.
        let va = 0x12345000;
        let entry = TlbEntry {
            guest_addr: va,
            phys_addr: 0x56789000,
            flags: 0x00000003, // Read/Write permissions.
            asid: 0,
        };

        // Update the TLB with the entry.
        tlb.update(va, entry).await.expect("Failed to update TLB");

        // Flush the entry.
        tlb.flush(va).await.expect("Failed to flush TLB entry");

        // Attempt to translate the virtual address.
        let result = tlb.translate(va, AccessType::Read).await;

        // Verify that the translation fails.
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_async_simple_tlb_flush_all() {
        // Create a new asynchronous TLB.
        let mut tlb = AsyncSimpleTlb::new();

        // Define multiple test translation entries.
        let entries = vec![
            (0x12345000, 0x56789000, 0),
            (0x23456000, 0x67890000, 0),
            (0x34567000, 0x78901000, 1),
        ];

        // Update the TLB with the entries.
        for (va, pa, asid) in entries {
            let entry = TlbEntry {
                guest_addr: va,
                phys_addr: pa,
                flags: 0x00000003, // Read/Write permissions.
                asid,
            };
            tlb.update(va, entry).await.expect("Failed to update TLB");
        }

        // Flush all entries.
        tlb.flush_all().await.expect("Failed to flush all TLB entries");

        // Attempt to translate the virtual addresses.
        for (va, _, _) in entries {
            let result = tlb.translate(va, AccessType::Read).await;
            // Verify that the translation fails.
            assert!(result.is_err());
        }
    }
}
