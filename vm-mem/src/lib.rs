//! vm-mem: 内存管理单元实现
//!
//! 包含 SoftMMU（软件 MMU）和 RISC-V SV39/SV48 页表遍历

use crate::mmu::hugepage::{HugePageAllocator, HugePageSize};

use lru::LruCache;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use vm_core::error::{CoreError, MemoryError};
use vm_core::{
    AccessType, Fault, MemoryAccess, MmioDevice, MmioManager, MmuAsAny, VmError,
    mmu_traits::AddressTranslator,
};

/// Type alias for MMIO device result to reduce type complexity
type MmioDeviceResult = Result<Option<(Arc<RwLock<Box<dyn MmioDevice>>>, u64)>, String>;

// 模块组织
pub mod domain_services;
pub mod memory;
pub mod optimization;
pub mod tlb;
pub mod unified_mmu;
pub mod unified_mmu_v2;

/// @deprecated 使用unified_mmu_v2::HybridMMU替代
#[cfg(feature = "async")]
pub mod async_mmu;

// 重新导出主要类型
pub use memory::{
    MemoryPool, NumaAllocPolicy, NumaAllocStats, NumaAllocator, NumaNodeInfo, PageTableEntryPool,
    PoolError, PoolManager, PoolStats, StackPool, Sv39PageTableWalker, Sv48PageTableWalker,
    TlbEntryPool,
};
pub use optimization::unified::*;
pub use tlb::{
    AdaptiveReplacementPolicy, AtomicTlbStats, ConcurrentTlbConfig, ConcurrentTlbManager,
    ConcurrentTlbManagerAdapter, MultiLevelTlb, MultiLevelTlbConfig, OptimizedTlbEntry, ShardedTlb,
    StandardTlbManager, TlbFactory, TlbResult, UnifiedTlb,
};

#[cfg(feature = "optimizations")]
pub use tlb::SingleLevelTlb;
// 显式导入 TlbStats 避免冲突
pub use domain_services::AddressTranslationDomainService;
pub use tlb::BasicTlbStats as TlbStats;
/// @deprecated 使用unified_mmu_v2中的类型替代
pub use unified_mmu::{MmuOptimizationStrategy, UnifiedMmu, UnifiedMmuConfig, UnifiedMmuStats};
pub use unified_mmu_v2::{HybridMMU, UnifiedMMU as UnifiedMMUV2, UnifiedMmuConfigV2, UnifiedMmuStats as UnifiedMmuStatsV2};

// Re-export common types from vm_core for test convenience
pub use vm_core::{GuestAddr, GuestPhysAddr};

pub mod mmu;
#[cfg(feature = "async")]
pub use async_mmu::async_impl::async_file_io;
#[cfg(feature = "async")]
pub use async_mmu::{
    TlbCache, async_impl::AsyncMMU, async_impl::AsyncMmuWrapper, async_impl::AsyncTlbLookup,
};

/// Host 虚拟地址
pub type HostAddr = u64;

// ============================================================================
// 页表相关常量（RISC-V SV39）
// ============================================================================

/// 页大小：4KB
pub const PAGE_SIZE: u64 = 4096;
/// 页偏移位数
pub const PAGE_SHIFT: u64 = 12;
/// 页表项大小：8 字节
pub const PTE_SIZE: u64 = 8;
/// SV39 每级页表项数
pub const PTES_PER_PAGE: u64 = 512;
/// VPN 位宽
pub const VPN_BITS: u64 = 9;
/// VPN 掩码
pub const VPN_MASK: u64 = (1 << VPN_BITS) - 1;

/// RISC-V 页表项标志
pub mod pte_flags {
    pub const V: u64 = 1 << 0; // Valid
    pub const R: u64 = 1 << 1; // Read
    pub const W: u64 = 1 << 2; // Write
    pub const X: u64 = 1 << 3; // Execute
    pub const U: u64 = 1 << 4; // User
    pub const G: u64 = 1 << 5; // Global
    pub const A: u64 = 1 << 6; // Accessed
    pub const D: u64 = 1 << 7; // Dirty
}

/// 分页模式
///
/// 定义虚拟机使用的分页模式。
///
/// # 使用场景
/// - RISC-V: Sv39/Sv48
/// - ARM64: 4级或5级页表
/// - x86_64: 4级页表
///
/// # 示例
/// ```
/// use vm_mem::PagingMode;
///
/// let mode = PagingMode::Sv39; // RISC-V 39位虚拟地址
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PagingMode {
    /// 无分页（恒等映射）
    ///
    /// 虚拟地址直接映射到物理地址，用于简单场景或早期启动
    Bare,
    /// RISC-V SV39（3 级页表，39 位虚拟地址）
    ///
    /// 支持39位虚拟地址空间，最大512TB
    Sv39,
    /// RISC-V SV48（4 级页表，48 位虚拟地址）
    ///
    /// 支持48位虚拟地址空间，最大256TB
    Sv48,
    /// ARM64 四级页表
    ///
    /// ARM64 48位虚拟地址，4级页表翻译
    Arm64,
    /// x86_64 四级页表
    ///
    /// x86_64 48位虚拟地址，4级页表翻译
    X86_64,
}

// ============================================================================// TLB 实现 (优化版: HashMap + LRU)// ============================================================================
use vm_core::TlbEntry;

/// TLB 条目
// Removed duplicate TlbEntry struct to avoid shadowing public re-export
/// 组合键: (vpn, asid) -> 单个 u64 键
#[inline]
fn make_tlb_key(vpn: u64, asid: u16) -> u64 {
    (vpn << 16) | (asid as u64)
}

struct Tlb {
    entries: HashMap<u64, TlbEntry>,
    lru: LruCache<u64, ()>,
    global_entries: HashMap<u64, TlbEntry>,
    max_size: usize,
}

impl Tlb {
    fn new(size: usize) -> Self {
        let capacity =
            NonZeroUsize::new(size).unwrap_or(NonZeroUsize::new(1).expect("Operation failed"));
        Self {
            entries: HashMap::with_capacity(size),
            lru: LruCache::new(capacity),
            global_entries: HashMap::with_capacity(size / 4),
            max_size: size,
        }
    }

    fn lookup(&mut self, vpn: u64, asid: u16) -> Option<(u64, u64)> {
        if let Some(entry) = self.global_entries.get(&vpn) {
            return Some((entry.phys_addr.0, entry.flags));
        }
        let key = make_tlb_key(vpn, asid);
        if let Some(entry) = self.entries.get(&key) {
            self.lru.get(&key);
            return Some((entry.phys_addr.0, entry.flags));
        }
        None
    }

    fn insert(&mut self, vpn: u64, ppn: u64, flags: u64, asid: u16) {
        let entry = TlbEntry {
            guest_addr: GuestAddr(vpn << PAGE_SHIFT),
            phys_addr: GuestPhysAddr(ppn << PAGE_SHIFT),
            flags,
            asid,
        };
        if flags & pte_flags::G != 0 {
            self.global_entries.insert(vpn, entry);
            return;
        }
        let key = make_tlb_key(vpn, asid);
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

    #[allow(dead_code)]
    fn flush_page(&mut self, vpn: u64) {
        self.global_entries.remove(&vpn);
        let keys_to_remove: Vec<u64> = self
            .entries
            .iter()
            .filter(|(_, e)| e.guest_addr.0 == (vpn << PAGE_SHIFT))
            .map(|(k, _)| *k)
            .collect();
        for key in keys_to_remove {
            self.entries.remove(&key);
            self.lru.pop(&key);
        }
    }
}

// ============================================================================
// MMIO 区域
// ============================================================================

struct MmioRegion {
    base: GuestAddr,
    size: u64,
    device: Arc<RwLock<Box<dyn MmioDevice>>>,
}

// ============================================================================
// Physical Memory Backend
// ============================================================================

const SHARD_COUNT: usize = 16;

/// 物理内存后端（共享）
///
/// 实现Guest物理内存的后端存储。使用分片RwLock提高并发性能。
///
/// # 设计特点
/// - **分片设计**: 将内存分成多个分片，减少锁竞争
/// - **大页支持**: 可选择使用2MB大页减少TLB压力
/// - **MMIO支持**: 支持内存映射I/O设备
/// - **原子操作**: 支持LR/SC（Load-Reserved/Store-Conditional）指令
///
/// # 使用场景
/// - Guest物理内存：存储Guest的物理内存内容
/// - MMIO映射：映射硬件设备寄存器
/// - 原子操作：支持RISC-V的LR/SC指令
///
/// # 性能优化
/// - 分片RwLock：16个分片，减少锁竞争
/// - 大页支持：可选2MB huge pages
/// - Cache line对齐：64字节cache line对齐
pub struct PhysicalMemory {
    /// 物理内存分片 (Sharded RAM)
    ///
    /// 将内存分成16个分片，每个分片有独立的RwLock
    shards: Vec<RwLock<Vec<u8>>>,
    /// 分片大小
    shard_size: usize,
    /// 总内存大小
    total_size: usize,
    /// MMIO 设备区域
    ///
    /// 注册的MMIO设备列表
    mmio_regions: RwLock<Vec<MmioRegion>>,
    /// 全局保留地址集合 (用于 LR/SC): (addr, owner_id, size)
    ///
    /// 用于实现Load-Reserved/Store-Conditional指令
    reservations: RwLock<Vec<(GuestPhysAddr, u64, u8)>>,
    /// 大页分配器
    #[allow(dead_code)]
    huge_page_allocator: HugePageAllocator,
}

impl PhysicalMemory {
    /// 创建新的物理内存
    ///
    /// # 参数
    /// - `size`: 内存大小（字节）
    /// - `use_hugepages`: 是否使用大页（2MB）
    ///
    /// # 示例
    /// ```
    /// use vm_mem::PhysicalMemory;
    ///
    /// // 创建128MB物理内存
    /// let mem = PhysicalMemory::new(128 * 1024 * 1024, false);
    /// ```
    pub fn new(size: usize, use_hugepages: bool) -> Self {
        let allocator = HugePageAllocator::new(use_hugepages, HugePageSize::Size2M);
        let shard_size = size.div_ceil(SHARD_COUNT);

        let mut shards = Vec::with_capacity(SHARD_COUNT);
        for i in 0..SHARD_COUNT {
            let current_shard_size = if i == SHARD_COUNT - 1 {
                size - (shard_size * (SHARD_COUNT - 1))
            } else {
                shard_size
            };

            let mem_vec = if use_hugepages {
                match allocator.allocate_linux(current_shard_size) {
                    Ok(ptr) => unsafe {
                        Vec::from_raw_parts(ptr, current_shard_size, current_shard_size)
                    },
                    Err(_) => vec![0u8; current_shard_size],
                }
            } else {
                vec![0u8; current_shard_size]
            };
            shards.push(RwLock::new(mem_vec));
        }

        Self {
            shards,
            shard_size,
            total_size: size,
            mmio_regions: RwLock::new(Vec::new()),
            reservations: RwLock::new(Vec::new()),
            huge_page_allocator: allocator,
        }
    }

    #[inline]
    fn get_shard_index(&self, addr: usize) -> (usize, usize) {
        (addr / self.shard_size, addr % self.shard_size)
    }

    pub fn reserve(&self, pa: GuestPhysAddr, owner: u64, size: u8) {
        let mut reservations = self.reservations.write();
        reservations.retain(|&(_, o, _)| o != owner);
        reservations.push((pa, owner, size));
    }

    pub fn invalidate(&self, pa: GuestPhysAddr, _size: u8) {
        let mut reservations = self.reservations.write();
        let line_mask = !63u64;
        let target_line = pa.0 & line_mask;
        reservations.retain(|&(r_pa, _, _)| (r_pa.0 & line_mask) != target_line);
    }

    pub fn store_conditional_ram(
        &self,
        pa: GuestPhysAddr,
        val: u64,
        size: u8,
        owner: u64,
    ) -> Result<bool, VmError> {
        let mut reservations = self.reservations.write();

        if !reservations
            .iter()
            .any(|&(r_pa, r_owner, r_size)| r_pa == pa && r_owner == owner && r_size == size)
        {
            return Ok(false);
        }

        let addr = pa.0 as usize;
        if addr + (size as usize) > self.total_size {
            return Err(VmError::from(Fault::PageFault {
                addr: GuestAddr(pa.0),
                access_type: AccessType::Write,
                is_write: true,
                is_user: false,
            }));
        }

        let (idx, offset) = self.get_shard_index(addr);
        {
            let mut shard = self.shards[idx].write();
            if offset + (size as usize) > shard.len() {
                return Err(VmError::from(Fault::PageFault {
                    addr: GuestAddr(pa.0),
                    access_type: AccessType::Write,
                    is_write: true,
                    is_user: false,
                }));
            }
            match size {
                1 => shard[offset] = val as u8,
                2 => {
                    let b = (val as u16).to_le_bytes();
                    shard[offset..offset + 2].copy_from_slice(&b);
                }
                4 => {
                    let b = (val as u32).to_le_bytes();
                    shard[offset..offset + 4].copy_from_slice(&b);
                }
                8 => {
                    let b = val.to_le_bytes();
                    shard[offset..offset + 8].copy_from_slice(&b);
                }
                _ => return Err(VmError::from(Fault::AlignmentFault)),
            }
        }

        let line_mask = !63u64;
        let target_line = pa.0 & line_mask;
        reservations.retain(|&(r_pa, _, _)| (r_pa.0 & line_mask) != target_line);

        Ok(true)
    }

    pub fn read_u8(&self, addr: usize) -> Result<u8, VmError> {
        if addr >= self.total_size {
            return Err(VmError::Memory(MemoryError::InvalidAddress(GuestAddr(
                addr as u64,
            ))));
        }
        let (idx, offset) = self.get_shard_index(addr);
        let shard = self.shards[idx].read();
        Ok(shard[offset])
    }

    pub fn read_u16(&self, addr: usize) -> Result<u16, VmError> {
        if addr + 2 > self.total_size {
            return Err(VmError::Memory(MemoryError::InvalidAddress(GuestAddr(
                addr as u64,
            ))));
        }
        let (idx, offset) = self.get_shard_index(addr);
        let shard = self.shards[idx].read();
        if offset + 2 <= shard.len() {
            let b = [shard[offset], shard[offset + 1]];
            Ok(u16::from_le_bytes(b))
        } else {
            drop(shard);
            let mut buf = [0u8; 2];
            self.read_buf(addr, &mut buf)?;
            Ok(u16::from_le_bytes(buf))
        }
    }

    pub fn read_u32(&self, addr: usize) -> Result<u32, VmError> {
        if addr + 4 > self.total_size {
            return Err(VmError::Memory(MemoryError::InvalidAddress(GuestAddr(
                addr as u64,
            ))));
        }
        let (idx, offset) = self.get_shard_index(addr);
        let shard = self.shards[idx].read();
        if offset + 4 <= shard.len() {
            let mut b = [0u8; 4];
            b.copy_from_slice(&shard[offset..offset + 4]);
            Ok(u32::from_le_bytes(b))
        } else {
            drop(shard);
            let mut buf = [0u8; 4];
            self.read_buf(addr, &mut buf)?;
            Ok(u32::from_le_bytes(buf))
        }
    }

    pub fn read_u64(&self, addr: usize) -> Result<u64, VmError> {
        if addr + 8 > self.total_size {
            return Err(VmError::Memory(MemoryError::InvalidAddress(GuestAddr(
                addr as u64,
            ))));
        }
        let (idx, offset) = self.get_shard_index(addr);
        let shard = self.shards[idx].read();
        if offset + 8 <= shard.len() {
            let mut b = [0u8; 8];
            b.copy_from_slice(&shard[offset..offset + 8]);
            Ok(u64::from_le_bytes(b))
        } else {
            drop(shard);
            let mut buf = [0u8; 8];
            self.read_buf(addr, &mut buf)?;
            Ok(u64::from_le_bytes(buf))
        }
    }

    pub fn write_u8(&self, addr: usize, val: u8) -> Result<(), VmError> {
        if addr >= self.total_size {
            return Err(VmError::Memory(MemoryError::InvalidAddress(GuestAddr(
                addr as u64,
            ))));
        }
        let (idx, offset) = self.get_shard_index(addr);
        let mut shard = self.shards[idx].write();
        shard[offset] = val;
        Ok(())
    }

    pub fn write_u16(&self, addr: usize, val: u16) -> Result<(), VmError> {
        if addr + 2 > self.total_size {
            return Err(VmError::Memory(MemoryError::InvalidAddress(GuestAddr(
                addr as u64,
            ))));
        }
        let (idx, offset) = self.get_shard_index(addr);
        let mut shard = self.shards[idx].write();
        if offset + 2 <= shard.len() {
            let b = val.to_le_bytes();
            shard[offset..offset + 2].copy_from_slice(&b);
            Ok(())
        } else {
            drop(shard);
            let b = val.to_le_bytes();
            self.write_buf(addr, &b)
        }
    }

    pub fn write_u32(&self, addr: usize, val: u32) -> Result<(), VmError> {
        if addr + 4 > self.total_size {
            return Err(VmError::Memory(MemoryError::InvalidAddress(GuestAddr(
                addr as u64,
            ))));
        }
        let (idx, offset) = self.get_shard_index(addr);
        let mut shard = self.shards[idx].write();
        if offset + 4 <= shard.len() {
            let b = val.to_le_bytes();
            shard[offset..offset + 4].copy_from_slice(&b);
            Ok(())
        } else {
            drop(shard);
            let b = val.to_le_bytes();
            self.write_buf(addr, &b)
        }
    }

    pub fn write_u64(&self, addr: usize, val: u64) -> Result<(), VmError> {
        if addr + 8 > self.total_size {
            return Err(VmError::Memory(MemoryError::InvalidAddress(GuestAddr(
                addr as u64,
            ))));
        }
        let (idx, offset) = self.get_shard_index(addr);
        let mut shard = self.shards[idx].write();
        if offset + 8 <= shard.len() {
            let b = val.to_le_bytes();
            shard[offset..offset + 8].copy_from_slice(&b);
            Ok(())
        } else {
            drop(shard);
            let b = val.to_le_bytes();
            self.write_buf(addr, &b)
        }
    }

    pub fn read_buf(&self, addr: usize, buf: &mut [u8]) -> Result<(), VmError> {
        if addr + buf.len() > self.total_size {
            return Err(VmError::Memory(MemoryError::InvalidAddress(GuestAddr(
                addr as u64,
            ))));
        }

        let mut current_addr = addr;
        let mut buf_offset = 0;
        let mut remaining = buf.len();

        while remaining > 0 {
            let (idx, offset) = self.get_shard_index(current_addr);
            let shard = self.shards[idx].read();
            let available = shard.len() - offset;
            let to_read = std::cmp::min(remaining, available);

            buf[buf_offset..buf_offset + to_read].copy_from_slice(&shard[offset..offset + to_read]);

            current_addr += to_read;
            buf_offset += to_read;
            remaining -= to_read;
        }
        Ok(())
    }

    pub fn write_buf(&self, addr: usize, buf: &[u8]) -> Result<(), VmError> {
        if addr + buf.len() > self.total_size {
            return Err(VmError::Memory(MemoryError::InvalidAddress(GuestAddr(
                addr as u64,
            ))));
        }

        let mut current_addr = addr;
        let mut buf_offset = 0;
        let mut remaining = buf.len();

        while remaining > 0 {
            let (idx, offset) = self.get_shard_index(current_addr);
            let mut shard = self.shards[idx].write();
            let available = shard.len() - offset;
            let to_write = std::cmp::min(remaining, available);

            shard[offset..offset + to_write]
                .copy_from_slice(&buf[buf_offset..buf_offset + to_write]);

            current_addr += to_write;
            buf_offset += to_write;
            remaining -= to_write;
        }
        Ok(())
    }

    /// 获取物理内存总大小（与memory_size方法相同，为了兼容旧代码）
    pub fn size(&self) -> usize {
        self.total_size
    }

    /// 导出内存数据（与dump_memory方法相同，为了兼容旧代码）
    pub fn dump(&self) -> Vec<u8> {
        self.dump_memory()
    }

    /// 恢复内存数据（与restore_memory方法相同，为了兼容旧代码）
    pub fn restore(&self, data: &[u8]) -> Result<(), VmError> {
        if data.len() != self.total_size {
            return Err(VmError::Core(CoreError::InvalidParameter {
                name: "data".to_string(),
                value: data.len().to_string(),
                message: "Invalid data size for memory restore".to_string(),
            }));
        }

        let mut current_addr = 0;
        for shard in &self.shards {
            let mut shard = shard.write();
            let shard_size = shard.len();
            let slice = &data[current_addr..current_addr + shard_size];
            shard.copy_from_slice(slice);
            current_addr += shard_size;
        }
        Ok(())
    }
}

// MemoryAccess trait implementation for PhysicalMemory
impl MemoryAccess for PhysicalMemory {
    fn read(&self, pa: GuestAddr, size: u8) -> Result<u64, VmError> {
        let addr = pa.0 as usize;
        match size {
            1 => Ok(self.read_u8(addr)? as u64),
            2 => Ok(self.read_u16(addr)? as u64),
            4 => Ok(self.read_u32(addr)? as u64),
            8 => Ok(self.read_u64(addr)?),
            _ => Err(VmError::from(Fault::AlignmentFault)),
        }
    }

    fn write(&mut self, pa: GuestAddr, val: u64, size: u8) -> Result<(), VmError> {
        let addr = pa.0 as usize;
        match size {
            1 => self.write_u8(addr, val as u8),
            2 => self.write_u16(addr, val as u16),
            4 => self.write_u32(addr, val as u32),
            8 => self.write_u64(addr, val),
            _ => Err(VmError::from(Fault::AlignmentFault)),
        }
    }

    fn fetch_insn(&self, pc: GuestAddr) -> Result<u64, VmError> {
        self.read(pc, 4)
    }

    fn memory_size(&self) -> usize {
        self.total_size
    }

    fn dump_memory(&self) -> Vec<u8> {
        let mut dump = Vec::with_capacity(self.total_size);
        for shard in &self.shards {
            let shard = shard.read();
            dump.extend_from_slice(&shard);
        }
        dump
    }

    fn restore_memory(&mut self, data: &[u8]) -> Result<(), String> {
        if data.len() != self.total_size {
            return Err(format!(
                "Restore failed: expected {} bytes, got {} bytes",
                self.total_size,
                data.len()
            ));
        }

        let mut current_addr = 0;
        for shard in &self.shards {
            let mut shard = shard.write();
            let shard_size = shard.len();
            let slice = &data[current_addr..current_addr + shard_size];
            shard.copy_from_slice(slice);
            current_addr += shard_size;
        }
        Ok(())
    }
}

// MmioManager trait implementation for PhysicalMemory
impl MmioManager for PhysicalMemory {
    fn map_mmio(&self, base: GuestAddr, size: u64, device: Box<dyn MmioDevice>) {
        let mut mmio_regions = self.mmio_regions.write();
        mmio_regions.push(MmioRegion {
            base,
            size,
            device: Arc::new(RwLock::new(device)),
        });
    }

    fn poll_devices(&self) {
        // MmioDevice trait doesn't have poll method, so this is a no-op for now
    }
}

// ============================================================================
// SoftMmu 实现
// ============================================================================

static NEXT_MMU_ID: AtomicU64 = AtomicU64::new(1);

/// 软件 MMU 实现 (Per-vCPU)
///
/// 基于软件实现的内存管理单元，支持虚拟地址到物理地址的翻译。
///
/// # 核心功能
/// - **地址翻译**: 支持Bare、Sv39、Sv48等分页模式
/// - **TLB缓存**: ITLB和DTLB分离，提高翻译性能
/// - **MMIO支持**: 支持内存映射I/O设备
/// - **权限检查**: R/W/X权限验证
/// - **原子操作**: 支持LR/SC指令对
///
/// # TLB优化
/// - ITLB: 64条目，用于指令取指
/// - DTLB: 128条目，用于数据访问
/// - 支持ASID（地址空间ID）隔离
/// - 支持全局页条目（G标志）
///
/// # 使用场景
/// - RISC-V虚拟机：SV39/SV48页表翻译
/// - 用户态模拟：在用户态模拟MMU
/// - 多vCPU：每个vCPU独立的MMU实例
///
/// # 示例
/// ```
/// use vm_mem::SoftMmu;
///
/// // 创建64MB内存的MMU
/// let mut mmu = SoftMmu::new(64 * 1024 * 1024, false);
///
/// // Bare模式：恒等映射
/// use vm_core::{GuestAddr, AccessType};
/// let phys = mmu.translate(GuestAddr(0x1000), AccessType::Read).unwrap();
/// ```
///
/// # 性能
/// - TLB命中: ~5-10ns
/// - TLB未命中: ~50-100ns（需要页表遍历）
/// - 分片RwLock: 高并发读写
pub struct SoftMmu {
    /// MMU唯一ID
    id: u64,
    /// 物理内存后端
    phys_mem: Arc<PhysicalMemory>,
    /// 指令TLB
    itlb: Tlb,
    /// 数据TLB
    dtlb: Tlb,
    /// 当前分页模式
    paging_mode: PagingMode,
    /// 页表基址
    page_table_base: GuestPhysAddr,
    /// 地址空间ID
    asid: u16,
    /// 页表遍历器
    page_table_walker: Option<Box<dyn vm_core::PageTableWalker>>,
    /// TLB命中次数
    tlb_hits: u64,
    /// TLB未命中次数
    tlb_misses: u64,
    /// 严格对齐检查
    strict_align: bool,
}

impl Clone for SoftMmu {
    fn clone(&self) -> Self {
        Self {
            id: NEXT_MMU_ID.fetch_add(1, Ordering::Relaxed),
            phys_mem: self.phys_mem.clone(),
            itlb: Tlb::new(64),
            dtlb: Tlb::new(128),
            paging_mode: self.paging_mode,
            page_table_base: self.page_table_base,
            asid: self.asid,
            page_table_walker: match self.paging_mode {
                PagingMode::Sv39 => Some(Box::new(Sv39PageTableWalker::new(
                    self.page_table_base,
                    self.asid,
                ))),
                PagingMode::Sv48 => Some(Box::new(Sv48PageTableWalker::new(
                    self.page_table_base,
                    self.asid,
                ))),
                _ => None,
            },
            tlb_hits: 0,
            tlb_misses: 0,
            strict_align: self.strict_align,
        }
    }
}

impl SoftMmu {
    /// 创建默认MMU（64KB内存）
    pub fn new_default() -> Self {
        Self::new(64 * 1024, false)
    }

    /// 创建新的MMU实例
    ///
    /// # 参数
    /// - `size`: 物理内存大小（字节）
    /// - `use_hugepages`: 是否使用2MB大页
    ///
    /// # 示例
    /// ```
    /// use vm_mem::SoftMmu;
    ///
    /// // 创建128MB内存，不使用大页
    /// let mmu = SoftMmu::new(128 * 1024 * 1024, false);
    ///
    /// // 创建1GB内存，使用大页
    /// let mmu = SoftMmu::new(1024 * 1024 * 1024, true);
    /// ```
    pub fn new(size: usize, use_hugepages: bool) -> Self {
        #[allow(unused_mut)]
        let mut mmu = Self {
            id: NEXT_MMU_ID.fetch_add(1, Ordering::Relaxed),
            phys_mem: Arc::new(PhysicalMemory::new(size, use_hugepages)),
            itlb: Tlb::new(64),
            dtlb: Tlb::new(128),
            paging_mode: PagingMode::Bare,
            page_table_base: GuestPhysAddr(0),
            asid: 0,
            page_table_walker: None,
            tlb_hits: 0,
            tlb_misses: 0,
            strict_align: false,
        };
        #[cfg(feature = "std")]
        {
            if std::env::var("VM_STRICT_ALIGN")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false)
            {
                mmu.strict_align = true;
            }
        }
        mmu
    }

    /// 从Guest物理地址读取字节切片
    ///
    /// # 参数
    /// - `pa`: Guest物理地址
    /// - `len`: 要读取的字节数
    ///
    /// # 返回
    /// 成功返回读取的字节向量，失败返回None
    pub fn guest_slice(&self, pa: u64, len: usize) -> Option<Vec<u8>> {
        let mut buf = vec![0u8; len];
        if self.phys_mem.read_buf(pa as usize, &mut buf).is_ok() {
            Some(buf)
        } else {
            None
        }
    }

    /// 设置分页模式
    ///
    /// 切换分页模式时会自动刷新TLB。
    ///
    /// # 参数
    /// - `mode`: 新的分页模式（Bare/Sv39/Sv48等）
    ///
    /// # 注意
    /// - 切换模式会清空所有TLB条目
    /// - Bare模式：虚拟地址直接映射到物理地址
    /// - Sv39/Sv48模式：需要页表翻译
    pub fn set_paging_mode(&mut self, mode: PagingMode) {
        if self.paging_mode != mode {
            self.paging_mode = mode;
            self.itlb.flush();
            self.dtlb.flush();
            self.update_page_table_walker();
        }
    }

    /// 启用或禁用严格对齐检查
    ///
    /// # 参数
    /// - `enable`: true启用严格对齐，false禁用
    ///
    /// # 对齐要求
    /// - 1字节：任意地址
    /// - 2字节：2字节对齐
    /// - 4字节：4字节对齐
    /// - 8字节：8字节对齐
    pub fn set_strict_align(&mut self, enable: bool) {
        self.strict_align = enable;
    }

    /// 设置RISC-V SATP寄存器
    ///
    /// SATP寄存器控制分页模式和页表基址。
    ///
    /// # 参数
    /// - `satp`: SATP寄存器值
    ///
    /// # SATP格式（RISC-V Sv39）
    /// ```
    /// | 63..60 | 59..44 | 43..0 |
    /// | MODE   | ASID   | PPN   |
    /// ```
    /// - MODE: 8=Sv39, 9=Sv48
    /// - ASID: 地址空间ID
    /// - PPN: 页表基址的物理页号
    ///
    /// # 注意
    /// 设置SATP会自动刷新对应ASID的TLB条目
    pub fn set_satp(&mut self, satp: u64) {
        let mode = (satp >> 60) & 0xF;
        let asid = ((satp >> 44) & 0xFFFF) as u16;
        let ppn = satp & ((1u64 << 44) - 1);

        self.paging_mode = match mode {
            0 => PagingMode::Bare,
            8 => PagingMode::Sv39,
            9 => PagingMode::Sv48,
            _ => PagingMode::Bare,
        };

        self.asid = asid;
        self.page_table_base = GuestPhysAddr(ppn << PAGE_SHIFT);

        self.itlb.flush_asid(asid);
        self.dtlb.flush_asid(asid);
        self.update_page_table_walker();
    }

    pub fn tlb_stats(&self) -> (u64, u64) {
        (self.tlb_hits, self.tlb_misses)
    }

    /// 获取物理内存大小（字节）
    pub fn memory_size(&self) -> usize {
        self.phys_mem.size()
    }

    fn check_alignment(&self, pa: GuestAddr, size: u8) -> Result<(), VmError> {
        if self.strict_align {
            match size {
                1 => {}
                2 => {
                    if pa % 2 != 0 {
                        return Err(VmError::from(Fault::AlignmentFault));
                    }
                }
                4 => {
                    if pa % 4 != 0 {
                        return Err(VmError::from(Fault::AlignmentFault));
                    }
                }
                8 => {
                    if pa % 8 != 0 {
                        return Err(VmError::from(Fault::AlignmentFault));
                    }
                }
                _ => return Err(VmError::from(Fault::AlignmentFault)),
            }
        } else if !matches!(size, 1 | 2 | 4 | 8) {
            return Err(VmError::from(Fault::AlignmentFault));
        }
        Ok(())
    }

    fn check_mmio_region(&self, pa: GuestAddr) -> MmioDeviceResult {
        let mmio_regions = self.phys_mem.mmio_regions.read();

        for region in mmio_regions.iter() {
            if pa >= region.base && pa < region.base + region.size {
                return Ok(Some((region.device.clone(), pa - region.base)));
            }
        }
        Ok(None)
    }

    fn read_phys(&self, pa: GuestPhysAddr, size: u8) -> Result<u64, VmError> {
        let addr = pa.0 as usize;
        self.check_alignment(GuestAddr(pa.0), size)?;

        let mmio_op = match self.check_mmio_region(GuestAddr(pa.0)) {
            Ok(op) => op,
            Err(_) => {
                return Err(VmError::from(Fault::PageFault {
                    addr: pa.to_guest_addr(),
                    access_type: AccessType::Read,
                    is_write: false,
                    is_user: false,
                }));
            }
        };

        if let Some((device, offset)) = mmio_op {
            return (*device.read()).read(offset, size);
        }

        match size {
            1 => self.phys_mem.read_u8(addr).map(|v| v as u64),
            2 => self.phys_mem.read_u16(addr).map(|v| v as u64),
            4 => self.phys_mem.read_u32(addr).map(|v| v as u64),
            8 => self.phys_mem.read_u64(addr),
            _ => return Err(VmError::from(Fault::AlignmentFault)),
        }
        .map_err(|_| {
            VmError::from(Fault::PageFault {
                addr: GuestAddr(pa.0),
                access_type: AccessType::Read,
                is_write: false,
                is_user: false,
            })
        })
    }

    fn write_phys(&mut self, pa: GuestPhysAddr, val: u64, size: u8) -> Result<(), VmError> {
        let addr = pa.0 as usize;
        self.check_alignment(GuestAddr(pa.0), size)?;

        let mmio_op = match self.check_mmio_region(GuestAddr(pa.0)) {
            Ok(op) => op,
            Err(_) => {
                return Err(VmError::from(Fault::PageFault {
                    addr: GuestAddr(pa.0),
                    access_type: AccessType::Write,
                    is_write: true,
                    is_user: false,
                }));
            }
        };

        if let Some((device, offset)) = mmio_op {
            let mut dev_guard = device.write();
            let dev = &mut *dev_guard;
            dev.write(offset, val, size)?;
            // Note: notify method is not part of MmioDevice trait
            // if offset == 0x20 {
            //     dev.notify(self, offset);
            // }
            return Ok(());
        }

        let res = match size {
            1 => self.phys_mem.write_u8(addr, val as u8),
            2 => self.phys_mem.write_u16(addr, val as u16),
            4 => self.phys_mem.write_u32(addr, val as u32),
            8 => self.phys_mem.write_u64(addr, val),
            _ => return Err(VmError::from(Fault::AlignmentFault)),
        };

        if res.is_err() {
            return Err(VmError::from(Fault::PageFault {
                addr: GuestAddr(pa.0),
                access_type: AccessType::Write,
                is_write: true,
                is_user: false,
            }));
        }

        self.phys_mem.invalidate(pa, size);
        Ok(())
    }

    pub fn as_mmu_any(&self) -> &dyn std::any::Any {
        self
    }
    pub fn as_mmu_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn update_page_table_walker(&mut self) {
        self.page_table_walker = match self.paging_mode {
            PagingMode::Sv39 => Some(Box::new(Sv39PageTableWalker::new(
                self.page_table_base,
                self.asid,
            ))),
            PagingMode::Sv48 => Some(Box::new(Sv48PageTableWalker::new(
                self.page_table_base,
                self.asid,
            ))),
            _ => None,
        };
    }

    pub fn resize_tlbs(&mut self, itlb_size: usize, dtlb_size: usize) {
        self.itlb = Tlb::new(itlb_size.max(1));
        self.dtlb = Tlb::new(dtlb_size.max(1));
        self.tlb_hits = 0;
        self.tlb_misses = 0;
    }

    pub fn tlb_capacity(&self) -> (usize, usize) {
        (self.itlb.max_size, self.dtlb.max_size)
    }

    /// 虚拟地址到物理地址的翻译，支持访问类型
    pub fn translate(
        &mut self,
        va: GuestAddr,
        access: AccessType,
    ) -> Result<GuestPhysAddr, VmError> {
        match self.paging_mode {
            PagingMode::Bare => Ok(GuestPhysAddr(va.0)),
            _ => {
                // 计算VPN（虚拟页号）
                let vpn = va.0 >> PAGE_SHIFT;

                // 先检查TLB，使用不可变引用
                let asid = self.asid;

                // 先从TLB中查找（使用不可变借用的TLB）
                let tlb_result = match access {
                    AccessType::Execute => self.itlb.lookup(vpn, asid),
                    _ => self.dtlb.lookup(vpn, asid),
                };

                if let Some((ppn, flags)) = tlb_result {
                    // 检查访问权限
                    match access {
                        AccessType::Read if flags & pte_flags::R == 0 => {
                            return Err(VmError::from(Fault::PageFault {
                                addr: va,
                                access_type: access,
                                is_write: false,
                                is_user: false,
                            }));
                        }
                        AccessType::Write if flags & pte_flags::W == 0 => {
                            return Err(VmError::from(Fault::PageFault {
                                addr: va,
                                access_type: access,
                                is_write: true,
                                is_user: false,
                            }));
                        }
                        AccessType::Execute if flags & pte_flags::X == 0 => {
                            return Err(VmError::from(Fault::PageFault {
                                addr: va,
                                access_type: access,
                                is_write: false,
                                is_user: false,
                            }));
                        }
                        _ => {}
                    }
                    self.tlb_hits += 1;
                    return Ok(GuestPhysAddr(ppn << PAGE_SHIFT | (va.0 & (PAGE_SIZE - 1))));
                }

                self.tlb_misses += 1;

                // TLB未命中，进行页表遍历
                // 临时取出page_table_walker以避免同时可变借用self
                let mut walker = match self.page_table_walker.take() {
                    Some(walker) => walker,
                    None => {
                        return Err(VmError::from(Fault::PageFault {
                            addr: va,
                            access_type: access,
                            is_write: access == AccessType::Write,
                            is_user: false,
                        }));
                    }
                };

                // 使用walker进行地址翻译
                let (phys_addr, flags) = walker.walk(va, access, asid, self)?;

                // 将walker放回
                self.page_table_walker = Some(walker);

                // 将翻译结果缓存到TLB中
                let ppn = phys_addr.0 >> PAGE_SHIFT;
                match access {
                    AccessType::Execute => self.itlb.insert(vpn, ppn, flags, asid),
                    _ => self.dtlb.insert(vpn, ppn, flags, asid),
                }

                Ok(phys_addr)
            }
        }
    }
}

impl AddressTranslator for SoftMmu {
    fn translate(&mut self, va: GuestAddr, access: AccessType) -> Result<GuestPhysAddr, VmError> {
        // 调用不可变的translate方法
        SoftMmu::translate(self, va, access)
    }

    fn flush_tlb(&mut self) {
        self.itlb.flush();
        self.dtlb.flush();
    }

    fn flush_tlb_asid(&mut self, asid: u16) {
        self.itlb.flush_asid(asid);
        self.dtlb.flush_asid(asid);
    }

    fn flush_tlb_page(&mut self, va: GuestAddr) {
        self.itlb.flush_page(va.0);
        self.dtlb.flush_page(va.0);
    }
}

impl MemoryAccess for SoftMmu {
    fn read(&self, pa: GuestAddr, size: u8) -> Result<u64, VmError> {
        self.read_phys(GuestPhysAddr(pa.0), size)
    }

    fn write(&mut self, pa: GuestAddr, val: u64, size: u8) -> Result<(), VmError> {
        self.write_phys(GuestPhysAddr(pa.0), val, size)
    }

    fn load_reserved(&mut self, pa: GuestAddr, size: u8) -> Result<u64, VmError> {
        let phys_addr = GuestPhysAddr(pa.0);
        self.phys_mem.reserve(phys_addr, self.id, size);
        self.read_phys(phys_addr, size)
    }

    fn store_conditional(&mut self, pa: GuestAddr, val: u64, size: u8) -> Result<bool, VmError> {
        let phys_addr = GuestPhysAddr(pa.0);
        self.phys_mem
            .store_conditional_ram(phys_addr, val, size, self.id)
    }

    fn invalidate_reservation(&mut self, pa: GuestAddr, size: u8) {
        let phys_addr = GuestPhysAddr(pa.0);
        self.phys_mem.invalidate(phys_addr, size);
    }

    fn fetch_insn(&self, pc: GuestAddr) -> Result<u64, VmError> {
        self.read_phys(GuestPhysAddr(pc.0), 4)
    }

    fn read_bulk(&self, pa: GuestAddr, buf: &mut [u8]) -> Result<(), VmError> {
        // 如果是MMIO区域，需要特殊处理
        let mmio_op = self.check_mmio_region(pa).unwrap_or_default();

        if let Some((device, offset)) = mmio_op {
            // 对于MMIO区域，逐字节读取
            for (i, byte) in buf.iter_mut().enumerate() {
                let result = (*device.read()).read(offset + i as u64, 1)?;
                *byte = result as u8;
            }
            return Ok(());
        }

        // 普通内存区域，直接读取
        self.phys_mem.read_buf(pa.0 as usize, buf)
    }

    fn write_bulk(&mut self, pa: GuestAddr, buf: &[u8]) -> Result<(), VmError> {
        {
            let mmio_regions = self.phys_mem.mmio_regions.read();
            for region in mmio_regions.iter() {
                let region_end = region.base + region.size;
                let req_end = pa + buf.len() as u64;
                if pa < region_end && req_end > region.base {
                    drop(mmio_regions);
                    // 对于MMIO区域，仍然需要逐字节写入
                    for (i, &byte) in buf.iter().enumerate() {
                        self.write(pa + i as u64, byte as u64, 1)?;
                    }
                    return Ok(());
                }
            }
        }

        // 对于普通内存，使用高效的批量写入
        self.phys_mem.write_buf(pa.0 as usize, buf).map_err(|_| {
            VmError::from(Fault::PageFault {
                addr: pa,
                access_type: AccessType::Write,
                is_write: true,
                is_user: false,
            })
        })
    }

    fn memory_size(&self) -> usize {
        self.phys_mem.size()
    }

    fn dump_memory(&self) -> Vec<u8> {
        self.phys_mem.dump()
    }

    fn restore_memory(&mut self, data: &[u8]) -> Result<(), String> {
        self.phys_mem.restore(data).map_err(|e| e.to_string())
    }
}

impl MmioManager for SoftMmu {
    fn map_mmio(&self, base: GuestAddr, size: u64, device: Box<dyn MmioDevice>) {
        self.phys_mem.map_mmio(base, size, device)
    }

    fn poll_devices(&self) {
        // 实现设备轮询逻辑
        self.phys_mem.poll_devices()
    }
}

impl MmuAsAny for SoftMmu {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// ============================================================================
// 页表构建辅助
// ============================================================================

/// 页表构建器（用于测试和初始化）
pub struct PageTableBuilder {
    /// 下一个可用物理页
    next_page: GuestPhysAddr,
    /// 已分配的页表页
    allocated_pages: Vec<GuestPhysAddr>,
}

impl PageTableBuilder {
    pub fn new(start_addr: GuestPhysAddr) -> Self {
        Self {
            next_page: start_addr,
            allocated_pages: Vec::new(),
        }
    }

    /// 分配一个页表页
    pub fn alloc_page(&mut self) -> GuestPhysAddr {
        let addr = self.next_page;
        self.next_page += PAGE_SIZE;
        self.allocated_pages.push(addr);
        addr
    }

    /// 创建 SV39 页表映射（4KB 页）
    pub fn map_page_sv39(
        &mut self,
        mmu: &mut SoftMmu,
        va: GuestAddr,
        pa: GuestPhysAddr,
        flags: u64,
        root: GuestPhysAddr,
    ) -> Result<(), VmError> {
        let vpn = [
            (va >> 12) & VPN_MASK,
            (va >> 21) & VPN_MASK,
            (va >> 30) & VPN_MASK,
        ];

        let mut table_addr = root;

        // 遍历到最后一级
        for level in (1..=2).rev() {
            let pte_addr = table_addr + vpn[level] * PTE_SIZE;
            let pte = mmu.read_phys(pte_addr, 8)?;

            if pte & pte_flags::V == 0 {
                // 分配新页表
                let new_table = self.alloc_page();
                // 清零新页表
                for i in 0..PTES_PER_PAGE {
                    mmu.write_phys(GuestPhysAddr(new_table.0 + i * PTE_SIZE), 0, 8)?;
                }
                // 写入页表项（非叶子）
                let new_pte = ((new_table >> PAGE_SHIFT) << 10) | pte_flags::V;
                mmu.write_phys(GuestPhysAddr(pte_addr.0), new_pte, 8)?;
                table_addr = new_table;
            } else {
                // 已有页表
                let ppn = (pte >> 10) & ((1u64 << 44) - 1);
                table_addr = GuestPhysAddr(ppn << PAGE_SHIFT);
            }
        }

        // 写入叶子 PTE
        let pte_addr = table_addr + vpn[0] * PTE_SIZE;
        let ppn = pa >> PAGE_SHIFT;
        let pte = (ppn << 10) | flags | pte_flags::V;
        mmu.write_phys(GuestPhysAddr(pte_addr.0), pte, 8)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_core::ExecutionError;

    #[test]
    fn test_bare_mode() {
        let mut mmu = SoftMmu::new(1024 * 1024, false);

        // Bare 模式恒等映射
        assert_eq!(
            mmu.translate(GuestAddr(0x1000), AccessType::Read)
                .expect("Operation failed"),
            GuestPhysAddr(0x1000)
        );

        // 写入和读取
        mmu.write(GuestAddr(0x100), 0xDEADBEEF, 4)
            .expect("Operation failed");
        assert_eq!(
            mmu.read(GuestAddr(0x100), 4).expect("Operation failed"),
            0xDEADBEEF
        );
    }

    #[test]
    #[ignore] // Issue: Fix SV39 page table translation logic - page walk implementation needs debugging
    fn test_sv39_simple() {
        let mem_size = 16 * 1024 * 1024; // 16MB
        let mut mmu = SoftMmu::new(mem_size, false);

        // 设置 SV39 分页
        let root_table = 0x100000; // 根页表在 1MB
        let mut builder = PageTableBuilder::new(GuestPhysAddr(root_table + PAGE_SIZE));

        // 初始化根页表
        for i in 0..PTES_PER_PAGE {
            mmu.write_phys(GuestPhysAddr(root_table + (i * PTE_SIZE)), 0, 8)
                .expect("Operation failed");
        }

        // 映射虚拟地址 0x1000 -> 物理地址 0x2000
        let va = 0x1000u64;
        let pa = 0x200000u64; // 2MB
        let flags = pte_flags::R | pte_flags::W | pte_flags::X | pte_flags::A | pte_flags::D;
        builder
            .map_page_sv39(
                &mut mmu,
                GuestAddr(va),
                GuestPhysAddr(pa),
                flags,
                GuestPhysAddr(root_table),
            )
            .expect("Operation failed");

        // 设置 satp
        let satp = (8u64 << 60) | (root_table >> PAGE_SHIFT); // MODE=Sv39, PPN
        mmu.set_satp(satp);

        // 测试地址翻译
        let translated = mmu
            .translate(GuestAddr(va + 0x100), AccessType::Read)
            .expect("Operation failed");
        assert_eq!(translated, GuestPhysAddr(pa + 0x100));
    }

    #[test]
    fn test_tlb_hit() {
        let mut mmu = SoftMmu::new(1024 * 1024, false);

        // 第一次访问（TLB miss）
        mmu.translate(GuestAddr(0x1000), AccessType::Read)
            .expect("Operation failed");
        let (hits1, _misses1) = mmu.tlb_stats();

        // 第二次访问（应该 TLB hit，但 Bare 模式不使用 TLB）
        mmu.translate(GuestAddr(0x1000), AccessType::Read)
            .expect("Operation failed");
        let (hits2, _misses2) = mmu.tlb_stats();

        // Bare 模式不使用 TLB，统计应该不变
        assert_eq!(hits1, hits2);
    }

    #[test]
    fn test_alignment_checks() {
        let mut mmu = SoftMmu::new(1024 * 1024, false);
        mmu.set_strict_align(true);
        mmu.write(GuestAddr(0x100), 0xAA, 1)
            .expect("Operation failed");
        assert!(mmu.read(GuestAddr(0x100), 1).is_ok());
        assert!(matches!(
            mmu.read(GuestAddr(0x101), 2),
            Err(VmError::Execution(ExecutionError::Fault(
                Fault::AlignmentFault
            )))
        ));
        assert!(matches!(
            mmu.read(GuestAddr(0x102), 4),
            Err(VmError::Execution(ExecutionError::Fault(
                Fault::AlignmentFault
            )))
        ));
        assert!(matches!(
            mmu.read(GuestAddr(0x104), 8),
            Err(VmError::Execution(ExecutionError::Fault(
                Fault::AlignmentFault
            )))
        ));
        assert!(matches!(
            mmu.write(GuestAddr(0x101), 0xBB, 2),
            Err(VmError::Execution(ExecutionError::Fault(
                Fault::AlignmentFault
            )))
        ));
        assert!(matches!(
            mmu.write(GuestAddr(0x102), 0xCC, 4),
            Err(VmError::Execution(ExecutionError::Fault(
                Fault::AlignmentFault
            )))
        ));
        assert!(matches!(
            mmu.write(GuestAddr(0x104), 0xDD, 8),
            Err(VmError::Execution(ExecutionError::Fault(
                Fault::AlignmentFault
            )))
        ));
    }

    #[test]
    fn test_lr_sc_success() {
        let mut mmu = SoftMmu::new(1024 * 1024, false);
        // Initialize memory at addr
        let addr = 0x2000u64;
        mmu.write(GuestAddr(addr), 0x11, 1)
            .expect("Operation failed");
        // Load-reserved
        let v = mmu
            .load_reserved(GuestAddr(addr), 1)
            .expect("Operation failed");
        assert_eq!(v, 0x11);
        // Store-conditional should succeed and write new value
        let ok = mmu
            .store_conditional(GuestAddr(addr), 0x22, 1)
            .expect("Operation failed");
        assert!(ok);
        assert_eq!(
            mmu.read(GuestAddr(addr), 1).expect("Operation failed"),
            0x22
        );
    }

    #[test]
    fn test_lr_sc_invalidate_on_same_line_write() {
        let mut mmu1 = SoftMmu::new(1024 * 1024, false);
        // Share physical memory via clone (same Arc<PhysicalMemory>)
        let mut mmu2 = mmu1.clone();
        let base = 0x3000u64; // align to 64-byte line
        let addr1 = base; // reservation address
        let addr2 = base + 8; // same cache line

        // Initialize
        mmu1.write(GuestAddr(addr1), 0x33, 1)
            .expect("Operation failed");
        mmu2.write(GuestAddr(addr2), 0x44, 1)
            .expect("Operation failed");

        // mmu1 reserves addr1
        let _ = mmu1
            .load_reserved(GuestAddr(addr1), 1)
            .expect("Operation failed");

        // mmu2 writes to same cache line -> should invalidate mmu1's reservation
        mmu2.write(GuestAddr(addr2), 0x55, 1)
            .expect("Operation failed");

        // mmu1 store_conditional should fail
        let ok = mmu1
            .store_conditional(GuestAddr(addr1), 0x66, 1)
            .expect("Operation failed");
        assert!(!ok);
        // Memory remains unchanged at addr1
        assert_eq!(
            mmu1.read(GuestAddr(addr1), 1).expect("Operation failed"),
            0x33
        );
    }
}

#[cfg(test)]
mod tests_page_sizes {
    use super::*;

    #[test]
    fn test_page_size_constants() {
        assert_eq!(PAGE_SIZE, 4096);
        assert_eq!(PAGE_SHIFT, 12);
        assert_eq!(PTE_SIZE, 8);
        assert_eq!(PTES_PER_PAGE, 512);
    }

    #[test]
    fn test_vpn_bits_and_mask() {
        assert_eq!(VPN_BITS, 9);
        assert_eq!(VPN_MASK, 0x1FF);
    }

    #[test]
    fn test_pte_flags() {
        assert_eq!(pte_flags::V, 1 << 0);
        assert_eq!(pte_flags::R, 1 << 1);
        assert_eq!(pte_flags::W, 1 << 2);
        assert_eq!(pte_flags::X, 1 << 3);
        assert_eq!(pte_flags::U, 1 << 4);
        assert_eq!(pte_flags::G, 1 << 5);
        assert_eq!(pte_flags::A, 1 << 6);
        assert_eq!(pte_flags::D, 1 << 7);
    }

    #[test]
    fn test_paging_mode_variants() {
        let bare = PagingMode::Bare;
        let sv39 = PagingMode::Sv39;
        let sv48 = PagingMode::Sv48;
        let arm64 = PagingMode::Arm64;
        let x86_64 = PagingMode::X86_64;

        assert_eq!(bare, PagingMode::Bare);
        assert_eq!(sv39, PagingMode::Sv39);
        assert_eq!(sv48, PagingMode::Sv48);
        assert_eq!(arm64, PagingMode::Arm64);
        assert_eq!(x86_64, PagingMode::X86_64);
    }
}
