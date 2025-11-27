//! vm-mem: 内存管理单元实现
//! 
//! 包含 SoftMMU（软件 MMU）和 RISC-V SV39/SV48 页表遍历

use vm_core::{MMU, MmioDevice, GuestAddr, GuestPhysAddr, AccessType, Fault};
use crate::mmu::hugepage::{HugePageAllocator, HugePageSize};
use lru::LruCache;
use std::collections::HashMap;
use std::num::NonZeroUsize;

pub mod tlb_manager;
pub mod page_table_walker;

pub mod tlb_manager;
pub mod page_table_walker;

/// Host 虚拟地址
pub type HostAddr = u64;

/// 内存错误类型
#[derive(Debug)]
pub enum MemoryError {
    /// 地址越界
    OutOfBounds,
    /// 访问权限错误
    PermissionDenied,
    /// 页表错误
    PageFault,
    /// 对齐错误
    UnalignedAccess,
}

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
    pub const V: u64 = 1 << 0;  // Valid
    pub const R: u64 = 1 << 1;  // Read
    pub const W: u64 = 1 << 2;  // Write
    pub const X: u64 = 1 << 3;  // Execute
    pub const U: u64 = 1 << 4;  // User
    pub const G: u64 = 1 << 5;  // Global
    pub const A: u64 = 1 << 6;  // Accessed
    pub const D: u64 = 1 << 7;  // Dirty
}

/// 分页模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PagingMode {
    /// 无分页（恒等映射）
    Bare,
    /// RISC-V SV39（3 级页表，39 位虚拟地址）
    Sv39,
    /// RISC-V SV48（4 级页表，48 位虚拟地址）
    Sv48,
    /// ARM64 四级页表
    Arm64,
    /// x86_64 四级页表
    X86_64,
}

// ============================================================================
// TLB 实现 (优化版: HashMap + LRU)
// TLB 实现 (优化版: HashMap + LRU)
// TLB 实现 (优化版: HashMap + LRU)
// ============================================================================

/// TLB 条目
#[derive(Clone, Copy)]
struct TlbEntry {
    #[allow(dead_code)]
    #[allow(dead_code)]
    vpn: u64,
    ppn: u64,
    flags: u64,
    asid: u16,
}

/// 组合键: (vpn, asid) -> 单个 u64 键
#[inline]
fn make_tlb_key(vpn: u64, asid: u16) -> u64 {
    // vpn 最多 44 位 (SV48), asid 16 位, 组合后不会溢出
    (vpn << 16) | (asid as u64)
}

struct Tlb {
    /// 主哈希表存储 TLB 条目
    entries: HashMap<u64, TlbEntry>,
    /// LRU 缓存用于跟踪访问顺序和驱逐
    lru: LruCache<u64, ()>,
    /// 全局页条目 (不受 ASID 影响)
    global_entries: HashMap<u64, TlbEntry>,
    /// 最大容量
    max_size: usize,
    /// 主哈希表存储 TLB 条目
    entries: HashMap<u64, TlbEntry>,
    /// LRU 缓存用于跟踪访问顺序和驱逐
    lru: LruCache<u64, ()>,
    /// 全局页条目 (不受 ASID 影响)
    global_entries: HashMap<u64, TlbEntry>,
    /// 最大容量
    max_size: usize,
}

impl Tlb {
    fn new(size: usize) -> Self {
        let capacity = NonZeroUsize::new(size).unwrap_or(NonZeroUsize::new(1).expect("Operation failed"));
        Self {
            entries: HashMap::with_capacity(size),
            lru: LruCache::new(capacity),
            global_entries: HashMap::with_capacity(size / 4),
            max_size: size,
            entries: HashMap::with_capacity(size),
            lru: LruCache::new(capacity),
            global_entries: HashMap::with_capacity(size / 4),
            max_size: size,
        }
    }

    /// O(1) 查找 - 先查全局页，再查普通条目
    fn lookup(&mut self, vpn: u64, asid: u16) -> Option<(u64, u64)> {
        // 首先检查全局页 (不受 ASID 影响)
        if let Some(entry) = self.global_entries.get(&vpn) {
            return Some((entry.ppn, entry.flags));
        }

        // 检查普通条目
        let key = make_tlb_key(vpn, asid);
        if let Some(entry) = self.entries.get(&key) {
            // 更新 LRU 顺序
            self.lru.get(&key);
            return Some((entry.ppn, entry.flags));
        }

    /// O(1) 查找 - 先查全局页，再查普通条目
    fn lookup(&mut self, vpn: u64, asid: u16) -> Option<(u64, u64)> {
        // 首先检查全局页 (不受 ASID 影响)
        if let Some(entry) = self.global_entries.get(&vpn) {
            return Some((entry.ppn, entry.flags));
        }

        // 检查普通条目
        let key = make_tlb_key(vpn, asid);
        if let Some(entry) = self.entries.get(&key) {
            // 更新 LRU 顺序
            self.lru.get(&key);
            return Some((entry.ppn, entry.flags));
        }

        None
    }

    /// O(1) 插入 - 带 LRU 驱逐
    /// O(1) 插入 - 带 LRU 驱逐
    fn insert(&mut self, vpn: u64, ppn: u64, flags: u64, asid: u16) {
        let entry = TlbEntry { vpn, ppn, flags, asid };

        // 全局页单独存储
        if flags & pte_flags::G != 0 {
            self.global_entries.insert(vpn, entry);
            return;
        }

        let key = make_tlb_key(vpn, asid);

        // LRU 驱逐: 如果已满且是新条目
        if !self.entries.contains_key(&key) && self.entries.len() >= self.max_size {
            if let Some((old_key, _)) = self.lru.pop_lru() {
                self.entries.remove(&old_key);
            }
        }

        self.entries.insert(key, entry);
        self.lru.put(key, ());
    }

    /// 刷新所有 TLB 条目
    fn flush(&mut self) {
        self.entries.clear();
        self.lru.clear();
        self.global_entries.clear();
    }

    /// 刷新指定 ASID 的非全局条目
    fn flush_asid(&mut self, target_asid: u16) {
        // 收集需要删除的键
        let keys_to_remove: Vec<u64> = self.entries
            .iter()
            .filter(|(_, e)| e.asid == target_asid)
            .map(|(k, _)| *k)
            .collect();

        for key in keys_to_remove {
            self.entries.remove(&key);
            self.lru.pop(&key);
        }
    }

    /// 刷新指定 VPN 的条目
    #[allow(dead_code)]
    #[allow(dead_code)]
    fn flush_page(&mut self, vpn: u64) {
        // 删除全局页
        self.global_entries.remove(&vpn);

        // 删除所有 ASID 下该 VPN 的条目
        let keys_to_remove: Vec<u64> = self.entries
            .iter()
            .filter(|(_, e)| e.vpn == vpn)
            .map(|(k, _)| *k)
            .collect();

        for key in keys_to_remove {
            self.entries.remove(&key);
            self.lru.pop(&key);
        // 删除全局页
        self.global_entries.remove(&vpn);

        // 删除所有 ASID 下该 VPN 的条目
        let keys_to_remove: Vec<u64> = self.entries
            .iter()
            .filter(|(_, e)| e.vpn == vpn)
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

use parking_lot::RwLock;
use std::sync::Arc;

// ... (keep imports)

// ============================================================================
// Physical Memory Backend
// ============================================================================

/// 物理内存后端（共享）
pub struct PhysicalMemory {
    /// 物理内存 (RAM)
    mem: RwLock<Vec<u8>>,
    /// MMIO 设备区域
    mmio_regions: RwLock<Vec<MmioRegion>>,
    /// 全局保留地址集合 (用于 LR/SC): (addr, owner_id, size)
    reservations: RwLock<Vec<(GuestPhysAddr, u64, u8)>>,
    /// 大页分配器
    #[allow(dead_code)]
    huge_page_allocator: HugePageAllocator,
}

impl PhysicalMemory {
    pub fn new(size: usize, use_hugepages: bool) -> Self {
        let allocator = HugePageAllocator::new(use_hugepages, HugePageSize::Size2M);
        let mem_vec = if use_hugepages {
            match allocator.allocate_linux(size) {
                Ok(ptr) => unsafe { Vec::from_raw_parts(ptr, size, size) },
                Err(_) => {
                    println!("Failed to allocate huge pages, falling back to standard pages.");
                    vec![0u8; size]
                }
            }
        } else {
            vec![0u8; size]
        };

        Self {
            mem: RwLock::new(mem_vec),
            mmio_regions: RwLock::new(Vec::new()),
            reservations: RwLock::new(Vec::new()),
            huge_page_allocator: allocator,
        }
    }

    pub fn reserve(&self, pa: GuestPhysAddr, owner: u64, size: u8) {
        let mut reservations = self.reservations.write();
        // Remove old reservation for this owner
        reservations.retain(|&(_, o, _)| o != owner);
        reservations.push((pa, owner, size));
    }

    pub fn invalidate(&self, pa: GuestPhysAddr, _size: u8) {
        let mut reservations = self.reservations.write();
        // Invalidate any reservation overlapping with the write
        // Simple cache line model (64 bytes) or exact overlap?
        // User said: "any write to the same address or same cache line"
        let line_mask = !63u64;
        let target_line = pa & line_mask;
        reservations.retain(|&(r_pa, _, _)| (r_pa & line_mask) != target_line);
    }

    pub fn store_conditional_ram(&self, pa: GuestPhysAddr, val: u64, size: u8, owner: u64) -> Result<bool, Fault> {
        let mut reservations = self.reservations.write();
        
        // Check if valid reservation exists for this owner
        // And matches address and size
        if !reservations.iter().any(|&(r_pa, r_owner, r_size)| r_pa == pa && r_owner == owner && r_size == size) {
            return Ok(false);
        }

        // Perform write
        {
             let mut mem = self.mem.write();
             let addr = pa as usize;
             if addr + (size as usize) > mem.len() {
                 return Err(Fault::AccessViolation { addr: pa, access: AccessType::Write });
             }
             match size {
                 1 => mem[addr] = val as u8,
                 2 => { let b = (val as u16).to_le_bytes(); mem[addr..addr+2].copy_from_slice(&b); }
                 4 => { let b = (val as u32).to_le_bytes(); mem[addr..addr+4].copy_from_slice(&b); }
                 8 => { let b = val.to_le_bytes(); mem[addr..addr+8].copy_from_slice(&b); }
                 _ => return Err(Fault::AlignmentFault { addr: pa, size }),
             }
        }

        // Invalidate ALL reservations on this line (including this one)
        let line_mask = !63u64;
        let target_line = pa & line_mask;
        reservations.retain(|&(r_pa, _, _)| (r_pa & line_mask) != target_line);
        
        Ok(true)
    }
    device: Arc<RwLock<Box<dyn MmioDevice>>>,
}

use parking_lot::RwLock;
use std::sync::Arc;

// ... (keep imports)

// ============================================================================
// Physical Memory Backend
// ============================================================================

/// 物理内存后端（共享）
pub struct PhysicalMemory {
    /// 物理内存 (RAM)
    mem: RwLock<Vec<u8>>,
    /// MMIO 设备区域
    mmio_regions: RwLock<Vec<MmioRegion>>,
    /// 全局保留地址集合 (用于 LR/SC): (addr, owner_id, size)
    reservations: RwLock<Vec<(GuestPhysAddr, u64, u8)>>,
    /// 大页分配器
    #[allow(dead_code)]
    huge_page_allocator: HugePageAllocator,
}

impl PhysicalMemory {
    pub fn new(size: usize, use_hugepages: bool) -> Self {
        let allocator = HugePageAllocator::new(use_hugepages, HugePageSize::Size2M);
        let mem_vec = if use_hugepages {
            match allocator.allocate_linux(size) {
                Ok(ptr) => unsafe { Vec::from_raw_parts(ptr, size, size) },
                Err(_) => {
                    println!("Failed to allocate huge pages, falling back to standard pages.");
                    vec![0u8; size]
                }
            }
        } else {
            vec![0u8; size]
        };

        Self {
            mem: RwLock::new(mem_vec),
            mmio_regions: RwLock::new(Vec::new()),
            reservations: RwLock::new(Vec::new()),
            huge_page_allocator: allocator,
        }
    }

    pub fn reserve(&self, pa: GuestPhysAddr, owner: u64, size: u8) {
        let mut reservations = self.reservations.write();
        // Remove old reservation for this owner
        reservations.retain(|&(_, o, _)| o != owner);
        reservations.push((pa, owner, size));
    }

    pub fn invalidate(&self, pa: GuestPhysAddr, _size: u8) {
        let mut reservations = self.reservations.write();
        // Invalidate any reservation overlapping with the write
        // Simple cache line model (64 bytes) or exact overlap?
        // User said: "any write to the same address or same cache line"
        let line_mask = !63u64;
        let target_line = pa & line_mask;
        reservations.retain(|&(r_pa, _, _)| (r_pa & line_mask) != target_line);
    }

    pub fn store_conditional_ram(&self, pa: GuestPhysAddr, val: u64, size: u8, owner: u64) -> Result<bool, Fault> {
        let mut reservations = self.reservations.write();
        
        // Check if valid reservation exists for this owner
        // And matches address and size
        if !reservations.iter().any(|&(r_pa, r_owner, r_size)| r_pa == pa && r_owner == owner && r_size == size) {
            return Ok(false);
        }

        // Perform write
        {
             let mut mem = self.mem.write();
             let addr = pa as usize;
             if addr + (size as usize) > mem.len() {
                 return Err(Fault::AccessViolation { addr: pa, access: AccessType::Write });
             }
             match size {
                 1 => mem[addr] = val as u8,
                 2 => { let b = (val as u16).to_le_bytes(); mem[addr..addr+2].copy_from_slice(&b); }
                 4 => { let b = (val as u32).to_le_bytes(); mem[addr..addr+4].copy_from_slice(&b); }
                 8 => { let b = val.to_le_bytes(); mem[addr..addr+8].copy_from_slice(&b); }
                 _ => return Err(Fault::AlignmentFault { addr: pa, size }),
             }
        }

        // Invalidate ALL reservations on this line (including this one)
        let line_mask = !63u64;
        let target_line = pa & line_mask;
        reservations.retain(|&(r_pa, _, _)| (r_pa & line_mask) != target_line);
        
        Ok(true)
    }
}

// ============================================================================
// SoftMmu 实现
// ============================================================================

use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_MMU_ID: AtomicU64 = AtomicU64::new(1);

/// 软件 MMU 实现 (Per-vCPU)
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_MMU_ID: AtomicU64 = AtomicU64::new(1);

/// 软件 MMU 实现 (Per-vCPU)
pub struct SoftMmu {
    /// 唯一 ID (用于 LR/SC owner 标识)
    id: u64,
    /// 共享物理内存后端
    phys_mem: Arc<PhysicalMemory>,
    /// 唯一 ID (用于 LR/SC owner 标识)
    id: u64,
    /// 共享物理内存后端
    phys_mem: Arc<PhysicalMemory>,
    /// 指令 TLB
    itlb: Tlb,
    /// 数据 TLB
    dtlb: Tlb,
    /// 分页模式
    paging_mode: PagingMode,
    /// 页表基址寄存器（satp for RISC-V）
    page_table_base: GuestPhysAddr,
    /// 当前 ASID
    asid: u16,
    /// TLB 统计
    tlb_hits: u64,
    tlb_misses: u64,
    /// 严格地址对齐检查
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
            tlb_hits: 0,
            tlb_misses: 0,
            strict_align: self.strict_align,
        }
    }
    /// 严格地址对齐检查
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
            tlb_hits: 0,
            tlb_misses: 0,
            strict_align: self.strict_align,
        }
    }
}

impl SoftMmu {
    /// 创建默认大小（64KB）的 MMU
    pub fn new_default() -> Self {
        Self::new(64 * 1024, false)
    }

    /// 创建指定大小的 MMU
    pub fn new(size: usize, use_hugepages: bool) -> Self {
        let mut mmu = Self {
            id: NEXT_MMU_ID.fetch_add(1, Ordering::Relaxed),
            phys_mem: Arc::new(PhysicalMemory::new(size, use_hugepages)),
        let mut mmu = Self {
            id: NEXT_MMU_ID.fetch_add(1, Ordering::Relaxed),
            phys_mem: Arc::new(PhysicalMemory::new(size, use_hugepages)),
            itlb: Tlb::new(64),
            dtlb: Tlb::new(128),
            paging_mode: PagingMode::Bare,
            page_table_base: 0,
            asid: 0,
            tlb_hits: 0,
            tlb_misses: 0,
            strict_align: false,
        };
        #[cfg(not(feature = "no_std"))]
        {
            if std::env::var("VM_STRICT_ALIGN").map(|v| v == "1" || v.eq_ignore_ascii_case("true")).unwrap_or(false) {
                mmu.strict_align = true;
            }
        }
        mmu
            strict_align: false,
        };
        #[cfg(not(feature = "no_std"))]
        {
            if std::env::var("VM_STRICT_ALIGN").map(|v| v == "1" || v.eq_ignore_ascii_case("true")).unwrap_or(false) {
                mmu.strict_align = true;
            }
        }
        mmu
    }

    pub fn guest_slice(&self, pa: u64, len: usize) -> Option<Vec<u8>> {
        let mem = self.phys_mem.mem.read();
    pub fn guest_slice(&self, pa: u64, len: usize) -> Option<Vec<u8>> {
        let mem = self.phys_mem.mem.read();
        let start = pa as usize;
        let end = start.checked_add(len)?;
        if end <= mem.len() { Some(mem[start..end].to_vec()) } else { None }
        if end <= mem.len() { Some(mem[start..end].to_vec()) } else { None }
    }
    
    // guest_slice_mut removed as it's unsafe with RwLock
    
    // guest_slice_mut removed as it's unsafe with RwLock

    /// 设置分页模式
    pub fn set_paging_mode(&mut self, mode: PagingMode) {
        if self.paging_mode != mode {
            self.paging_mode = mode;
            self.itlb.flush();
            self.dtlb.flush();
        }
    }

    pub fn set_strict_align(&mut self, enable: bool) {
        self.strict_align = enable;
    }

    pub fn set_strict_align(&mut self, enable: bool) {
        self.strict_align = enable;
    }

    /// 设置页表基址（RISC-V satp 寄存器）
    pub fn set_satp(&mut self, satp: u64) {
        // satp 格式 (SV39): MODE[63:60] | ASID[59:44] | PPN[43:0]
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
        self.page_table_base = ppn << PAGE_SHIFT;

        // 切换地址空间时刷新非全局 TLB
        self.itlb.flush_asid(asid);
        self.dtlb.flush_asid(asid);
    }

    /// 获取 TLB 统计
    pub fn tlb_stats(&self) -> (u64, u64) {
        (self.tlb_hits, self.tlb_misses)
    }

    /// 读取物理内存（内部使用）
    fn read_phys(&self, pa: GuestPhysAddr, size: u8) -> Result<u64, Fault> {
        let addr = pa as usize;
        if self.strict_align {
            match size {
                1 => {}
                2 => if pa % 2 != 0 { return Err(Fault::AlignmentFault { addr: pa, size }); },
                4 => if pa % 4 != 0 { return Err(Fault::AlignmentFault { addr: pa, size }); },
                8 => if pa % 8 != 0 { return Err(Fault::AlignmentFault { addr: pa, size }); },
                _ => return Err(Fault::AlignmentFault { addr: pa, size }),
            }
        } else if !matches!(size, 1|2|4|8) {
            return Err(Fault::AlignmentFault { addr: pa, size });
        }
        if self.strict_align {
            match size {
                1 => {}
                2 => if pa % 2 != 0 { return Err(Fault::AlignmentFault { addr: pa, size }); },
                4 => if pa % 4 != 0 { return Err(Fault::AlignmentFault { addr: pa, size }); },
                8 => if pa % 8 != 0 { return Err(Fault::AlignmentFault { addr: pa, size }); },
                _ => return Err(Fault::AlignmentFault { addr: pa, size }),
            }
        } else if !matches!(size, 1|2|4|8) {
            return Err(Fault::AlignmentFault { addr: pa, size });
        }

        // 检查 MMIO 区域
        let mmio_op = {
            let mmio_regions = self.phys_mem.mmio_regions.read();
            let mut op = None;
            for region in mmio_regions.iter() {
                if pa >= region.base && pa < region.base + region.size {
                    op = Some((region.device.clone(), pa - region.base));
                    break;
                }
            }
            op
        };
        let mmio_op = {
            let mmio_regions = self.phys_mem.mmio_regions.read();
            let mut op = None;
            for region in mmio_regions.iter() {
                if pa >= region.base && pa < region.base + region.size {
                    op = Some((region.device.clone(), pa - region.base));
                    break;
                }
            }
            op
        };

        if let Some((device, offset)) = mmio_op {
            return Ok(device.read().read(offset, size));
        }

        let mem = self.phys_mem.mem.read();
        if let Some((device, offset)) = mmio_op {
            return Ok(device.read().read(offset, size));
        }

        let mem = self.phys_mem.mem.read();
        // 边界检查
        if addr + (size as usize) > mem.len() {
        if addr + (size as usize) > mem.len() {
            return Err(Fault::AccessViolation {
                addr: pa,
                access: AccessType::Read,
            });
        }

        // 读取内存
        let val = match size {
            1 => mem[addr] as u64,
            2 => u16::from_le_bytes([mem[addr], mem[addr + 1]]) as u64,
            1 => mem[addr] as u64,
            2 => u16::from_le_bytes([mem[addr], mem[addr + 1]]) as u64,
            4 => u32::from_le_bytes([
                mem[addr],
                mem[addr + 1],
                mem[addr + 2],
                mem[addr + 3],
                mem[addr],
                mem[addr + 1],
                mem[addr + 2],
                mem[addr + 3],
            ]) as u64,
            8 => u64::from_le_bytes([
                mem[addr],
                mem[addr + 1],
                mem[addr + 2],
                mem[addr + 3],
                mem[addr + 4],
                mem[addr + 5],
                mem[addr + 6],
                mem[addr + 7],
                mem[addr],
                mem[addr + 1],
                mem[addr + 2],
                mem[addr + 3],
                mem[addr + 4],
                mem[addr + 5],
                mem[addr + 6],
                mem[addr + 7],
            ]),
            _ => {
                return Err(Fault::AlignmentFault { addr: pa, size });
            }
        };

        Ok(val)
    }

    /// 写入物理内存（内部使用）
    fn write_phys(&mut self, pa: GuestPhysAddr, val: u64, size: u8) -> Result<(), Fault> {
        let addr = pa as usize;
        if self.strict_align {
            match size {
                1 => {}
                2 => if pa % 2 != 0 { return Err(Fault::AlignmentFault { addr: pa, size }); },
                4 => if pa % 4 != 0 { return Err(Fault::AlignmentFault { addr: pa, size }); },
                8 => if pa % 8 != 0 { return Err(Fault::AlignmentFault { addr: pa, size }); },
                _ => return Err(Fault::AlignmentFault { addr: pa, size }),
            }
        } else if !matches!(size, 1|2|4|8) {
            return Err(Fault::AlignmentFault { addr: pa, size });
        }
        if self.strict_align {
            match size {
                1 => {}
                2 => if pa % 2 != 0 { return Err(Fault::AlignmentFault { addr: pa, size }); },
                4 => if pa % 4 != 0 { return Err(Fault::AlignmentFault { addr: pa, size }); },
                8 => if pa % 8 != 0 { return Err(Fault::AlignmentFault { addr: pa, size }); },
                _ => return Err(Fault::AlignmentFault { addr: pa, size }),
            }
        } else if !matches!(size, 1|2|4|8) {
            return Err(Fault::AlignmentFault { addr: pa, size });
        }

        // 检查 MMIO 区域
        let mmio_op = {
            let mmio_regions = self.phys_mem.mmio_regions.read();
            let mut op = None;
            for region in mmio_regions.iter() {
                if pa >= region.base && pa < region.base + region.size {
                    op = Some((region.device.clone(), pa - region.base));
                    break;
                }
            }
            op
        };

        if let Some((device, offset)) = mmio_op {
            let mut dev = device.write();
            // First perform register write
            dev.write(offset, val, size);
            // If queue notify, invoke device processing
            if offset == 0x20 {
                dev.notify(self, offset);
            }
            return Ok(());
        let mmio_op = {
            let mmio_regions = self.phys_mem.mmio_regions.read();
            let mut op = None;
            for region in mmio_regions.iter() {
                if pa >= region.base && pa < region.base + region.size {
                    op = Some((region.device.clone(), pa - region.base));
                    break;
                }
            }
            op
        };

        if let Some((device, offset)) = mmio_op {
            let mut dev = device.write();
            // First perform register write
            dev.write(offset, val, size);
            // If queue notify, invoke device processing
            if offset == 0x20 {
                dev.notify(self, offset);
            }
            return Ok(());
        }

        let mut mem = self.phys_mem.mem.write();
        let mut mem = self.phys_mem.mem.write();
        // 边界检查
        if addr + (size as usize) > mem.len() {
        if addr + (size as usize) > mem.len() {
            return Err(Fault::AccessViolation {
                addr: pa,
                access: AccessType::Write,
            });
        }

        // 写入内存
        match size {
            1 => mem[addr] = val as u8,
            1 => mem[addr] = val as u8,
            2 => {
                let bytes = (val as u16).to_le_bytes();
                mem[addr] = bytes[0];
                mem[addr + 1] = bytes[1];
                mem[addr] = bytes[0];
                mem[addr + 1] = bytes[1];
            }
            4 => {
                let bytes = (val as u32).to_le_bytes();
                mem[addr] = bytes[0];
                mem[addr + 1] = bytes[1];
                mem[addr + 2] = bytes[2];
                mem[addr + 3] = bytes[3];
                mem[addr] = bytes[0];
                mem[addr + 1] = bytes[1];
                mem[addr + 2] = bytes[2];
                mem[addr + 3] = bytes[3];
            }
            8 => {
                let bytes = val.to_le_bytes();
                for i in 0..8 {
                    mem[addr + i] = bytes[i];
                    mem[addr + i] = bytes[i];
                }
            }
            _ => {
                return Err(Fault::AlignmentFault { addr: pa, size });
            }
        }

        self.phys_mem.invalidate(pa, size);

        self.phys_mem.invalidate(pa, size);

        Ok(())
    }



    /// RISC-V SV39 页表遍历
    fn walk_sv39(&mut self, va: GuestAddr, access: AccessType) -> Result<(GuestPhysAddr, u64), Fault> {
        let vpn = [
            (va >> 12) & VPN_MASK,  // VPN[0]
            (va >> 21) & VPN_MASK,  // VPN[1]
            (va >> 30) & VPN_MASK,  // VPN[2]
        ];
        let offset = va & (PAGE_SIZE - 1);

        let mut pte_addr = self.page_table_base;
        let mut level = 2i32;

        loop {
            // 计算当前级别 PTE 地址
            pte_addr = pte_addr + vpn[level as usize] * PTE_SIZE;

            // 读取 PTE
            let pte = self.read_phys(pte_addr, 8)?;

            // 检查有效位
            if pte & pte_flags::V == 0 {
                return Err(Fault::PageFault { addr: va, access });
            }

            let r = pte & pte_flags::R;
            let w = pte & pte_flags::W;
            let x = pte & pte_flags::X;

            // 如果 R=0 且 W=1，这是保留组合，产生页错误
            if r == 0 && w != 0 {
                return Err(Fault::PageFault { addr: va, access });
            }

            // 叶子节点：R 或 X 位被设置
            if r != 0 || x != 0 {
                // 检查权限
                let required = match access {
                    AccessType::Read => pte_flags::R,
                    AccessType::Write => pte_flags::W,
                    AccessType::Exec => pte_flags::X,
                };

                if pte & required == 0 {
                    return Err(Fault::PageFault { addr: va, access });
                }

                // 计算物理地址
                let ppn = (pte >> 10) & ((1u64 << 44) - 1);

                // 超级页对齐检查
                let pa = if level > 0 {
                    // 超级页：低位 VPN 必须为 0
                    for i in 0..level as usize {
                        if vpn[i] != 0 {
                            // 检查 PPN 对应位是否为 0
                            let ppn_part = (ppn >> (i * 9)) & VPN_MASK;
                            if ppn_part != 0 {
                                return Err(Fault::PageFault { addr: va, access });
                            }
                        }
                    }
                    // 超级页物理地址
                    let shift = PAGE_SHIFT + (level as u64) * VPN_BITS;
                    (ppn << PAGE_SHIFT) | (va & ((1u64 << shift) - 1))
                } else {
                    // 4KB 页
                    (ppn << PAGE_SHIFT) | offset
                };

                return Ok((pa, pte));
            }

            // 非叶子节点：继续遍历
            if level == 0 {
                return Err(Fault::PageFault { addr: va, access });
            }

            // 下一级页表地址
            let ppn = (pte >> 10) & ((1u64 << 44) - 1);
            pte_addr = ppn << PAGE_SHIFT;
            level -= 1;
        }
    }

    /// RISC-V SV48 页表遍历
    fn walk_sv48(&mut self, va: GuestAddr, access: AccessType) -> Result<(GuestPhysAddr, u64), Fault> {
        let vpn = [
            (va >> 12) & VPN_MASK,  // VPN[0]
            (va >> 21) & VPN_MASK,  // VPN[1]
            (va >> 30) & VPN_MASK,  // VPN[2]
            (va >> 39) & VPN_MASK,  // VPN[3]
        ];
        let offset = va & (PAGE_SIZE - 1);

        let mut pte_addr = self.page_table_base;
        let mut level = 3i32;

        loop {
            pte_addr = pte_addr + vpn[level as usize] * PTE_SIZE;
            let pte = self.read_phys(pte_addr, 8)?;

            if pte & pte_flags::V == 0 {
                return Err(Fault::PageFault { addr: va, access });
            }

            let r = pte & pte_flags::R;
            let w = pte & pte_flags::W;
            let x = pte & pte_flags::X;

            if r == 0 && w != 0 {
                return Err(Fault::PageFault { addr: va, access });
            }

            if r != 0 || x != 0 {
                let required = match access {
                    AccessType::Read => pte_flags::R,
                    AccessType::Write => pte_flags::W,
                    AccessType::Exec => pte_flags::X,
                };

                if pte & required == 0 {
                    return Err(Fault::PageFault { addr: va, access });
                }

                let ppn = (pte >> 10) & ((1u64 << 44) - 1);
                let pa = if level > 0 {
                    let shift = PAGE_SHIFT + (level as u64) * VPN_BITS;
                    (ppn << PAGE_SHIFT) | (va & ((1u64 << shift) - 1))
                } else {
                    (ppn << PAGE_SHIFT) | offset
                };

                return Ok((pa, pte));
            }

            if level == 0 {
                return Err(Fault::PageFault { addr: va, access });
            }

            let ppn = (pte >> 10) & ((1u64 << 44) - 1);
            pte_addr = ppn << PAGE_SHIFT;
            level -= 1;
        }
    }
}

impl MMU for SoftMmu {
    fn translate(&mut self, va: GuestAddr, access: AccessType) -> Result<GuestPhysAddr, Fault> {
        // Bare 模式：恒等映射
        if self.paging_mode == PagingMode::Bare {
            return Ok(va);
        }

        // 查询 TLB
        let vpn = va >> PAGE_SHIFT;
        let asid = self.asid;
        
        // 根据访问类型选择 TLB 并查找
        let tlb_result = match access {
            AccessType::Exec => self.itlb.lookup(vpn, asid),
            _ => self.dtlb.lookup(vpn, asid),
        let asid = self.asid;
        
        // 根据访问类型选择 TLB 并查找
        let tlb_result = match access {
            AccessType::Exec => self.itlb.lookup(vpn, asid),
            _ => self.dtlb.lookup(vpn, asid),
        };

        if let Some((ppn, flags)) = tlb_result {
        if let Some((ppn, flags)) = tlb_result {
            self.tlb_hits += 1;

            // 检查权限
            let required = match access {
                AccessType::Read => pte_flags::R,
                AccessType::Write => pte_flags::W,
                AccessType::Exec => pte_flags::X,
            };

            if flags & required == 0 {
                return Err(Fault::PageFault { addr: va, access });
            }

            let offset = va & (PAGE_SIZE - 1);
            return Ok((ppn << PAGE_SHIFT) | offset);
        }

        self.tlb_misses += 1;

        // TLB 未命中，执行页表遍历
        let (pa, flags) = match self.paging_mode {
            PagingMode::Sv39 => self.walk_sv39(va, access)?,
            PagingMode::Sv48 => self.walk_sv48(va, access)?,
            _ => return Ok(va), // 其他模式暂不支持，恒等映射
        };

        // 插入 TLB
        let ppn = pa >> PAGE_SHIFT;
        match access {
            AccessType::Exec => self.itlb.insert(vpn, ppn, flags, asid),
            _ => self.dtlb.insert(vpn, ppn, flags, asid),
        };
        match access {
            AccessType::Exec => self.itlb.insert(vpn, ppn, flags, asid),
            _ => self.dtlb.insert(vpn, ppn, flags, asid),
        };

        Ok(pa)
    }

    fn fetch_insn(&self, pc: GuestAddr) -> Result<u64, Fault> {
        self.read_phys(pc, 4)
    }

    fn read(&self, pa: GuestAddr, size: u8) -> Result<u64, Fault> {
        self.read_phys(pa, size)
    }

    fn load_reserved(&mut self, pa: GuestAddr, size: u8) -> Result<u64, Fault> {
        let phys_addr = self.translate(pa, AccessType::Read)?;
        self.phys_mem.reserve(phys_addr, self.id, size);
        self.read_phys(phys_addr, size)
    }

    fn load_reserved(&mut self, pa: GuestAddr, size: u8) -> Result<u64, Fault> {
        let phys_addr = self.translate(pa, AccessType::Read)?;
        self.phys_mem.reserve(phys_addr, self.id, size);
        self.read_phys(phys_addr, size)
    }

    fn write(&mut self, pa: GuestAddr, val: u64, size: u8) -> Result<(), Fault> {
        self.write_phys(pa, val, size)
    }

    fn store_conditional(&mut self, pa: GuestAddr, val: u64, size: u8) -> Result<bool, Fault> {
        let phys_addr = self.translate(pa, AccessType::Write)?;
        self.phys_mem.store_conditional_ram(phys_addr, val, size, self.id)
    }

    fn invalidate_reservation(&mut self, pa: GuestAddr, size: u8) {
        // Best effort translation - if it fails, we can't invalidate specific PA
        // But if it fails, likely no valid reservation exists for that VA anyway.
        if let Ok(phys_addr) = self.translate(pa, AccessType::Write) {
            self.phys_mem.invalidate(phys_addr, size);
        }
    }

    fn store_conditional(&mut self, pa: GuestAddr, val: u64, size: u8) -> Result<bool, Fault> {
        let phys_addr = self.translate(pa, AccessType::Write)?;
        self.phys_mem.store_conditional_ram(phys_addr, val, size, self.id)
    }

    fn invalidate_reservation(&mut self, pa: GuestAddr, size: u8) {
        // Best effort translation - if it fails, we can't invalidate specific PA
        // But if it fails, likely no valid reservation exists for that VA anyway.
        if let Ok(phys_addr) = self.translate(pa, AccessType::Write) {
            self.phys_mem.invalidate(phys_addr, size);
        }
    }

    fn read_bulk(&self, pa: GuestAddr, buf: &mut [u8]) -> Result<(), Fault> {
        // 检查 MMIO 重叠
        {
            let mmio_regions = self.phys_mem.mmio_regions.read();
            for region in mmio_regions.iter() {
                let region_end = region.base + region.size;
                let req_end = pa + buf.len() as u64;
                if pa < region_end && req_end > region.base {
                    drop(mmio_regions);
                    // 重叠，回退到逐字节读取
                    for (i, byte) in buf.iter_mut().enumerate() {
                        *byte = self.read(pa + i as u64, 1)? as u8;
                    }
                    return Ok(());
                }
        {
            let mmio_regions = self.phys_mem.mmio_regions.read();
            for region in mmio_regions.iter() {
                let region_end = region.base + region.size;
                let req_end = pa + buf.len() as u64;
                if pa < region_end && req_end > region.base {
                    drop(mmio_regions);
                    // 重叠，回退到逐字节读取
                    for (i, byte) in buf.iter_mut().enumerate() {
                        *byte = self.read(pa + i as u64, 1)? as u8;
                    }
                    return Ok(());
                }
            }
        }

        // RAM 读取
        let mem = self.phys_mem.mem.read();
        let mem = self.phys_mem.mem.read();
        let addr = pa as usize;
        let len = buf.len();
        if addr.checked_add(len).map_or(false, |end| end <= mem.len()) {
            buf.copy_from_slice(&mem[addr..addr+len]);
        if addr.checked_add(len).map_or(false, |end| end <= mem.len()) {
            buf.copy_from_slice(&mem[addr..addr+len]);
            Ok(())
        } else {
            Err(Fault::AccessViolation { addr: pa, access: AccessType::Read })
        }
    }

    fn write_bulk(&mut self, pa: GuestAddr, buf: &[u8]) -> Result<(), Fault> {
        // 检查 MMIO 重叠
        {
            let mmio_regions = self.phys_mem.mmio_regions.read();
            for region in mmio_regions.iter() {
                let region_end = region.base + region.size;
                let req_end = pa + buf.len() as u64;
                if pa < region_end && req_end > region.base {
                    drop(mmio_regions);
                    // 重叠，回退到逐字节写入
                    for (i, &byte) in buf.iter().enumerate() {
                        self.write(pa + i as u64, byte as u64, 1)?;
                    }
                    return Ok(());
                }
        {
            let mmio_regions = self.phys_mem.mmio_regions.read();
            for region in mmio_regions.iter() {
                let region_end = region.base + region.size;
                let req_end = pa + buf.len() as u64;
                if pa < region_end && req_end > region.base {
                    drop(mmio_regions);
                    // 重叠，回退到逐字节写入
                    for (i, &byte) in buf.iter().enumerate() {
                        self.write(pa + i as u64, byte as u64, 1)?;
                    }
                    return Ok(());
                }
            }
        }

        // RAM 写入
        let mut mem = self.phys_mem.mem.write();
        let mut mem = self.phys_mem.mem.write();
        let addr = pa as usize;
        let len = buf.len();
        if addr.checked_add(len).map_or(false, |end| end <= mem.len()) {
            mem[addr..addr+len].copy_from_slice(buf);
        if addr.checked_add(len).map_or(false, |end| end <= mem.len()) {
            mem[addr..addr+len].copy_from_slice(buf);
            Ok(())
        } else {
            Err(Fault::AccessViolation { addr: pa, access: AccessType::Write })
        }
    }

    fn map_mmio(&mut self, base: GuestAddr, size: u64, device: Box<dyn MmioDevice>) {
        let mut mmio_regions = self.phys_mem.mmio_regions.write();
        mmio_regions.push(MmioRegion { 
            base, 
            size, 
            device: Arc::new(RwLock::new(device)) 
        });
        let mut mmio_regions = self.phys_mem.mmio_regions.write();
        mmio_regions.push(MmioRegion { 
            base, 
            size, 
            device: Arc::new(RwLock::new(device)) 
        });
    }

    fn flush_tlb(&mut self) {
        self.itlb.flush();
        self.dtlb.flush();
    }

    fn memory_size(&self) -> usize {
        self.phys_mem.mem.read().len()
        self.phys_mem.mem.read().len()
    }

    fn dump_memory(&self) -> Vec<u8> {
        self.phys_mem.mem.read().clone()
        self.phys_mem.mem.read().clone()
    }

    fn restore_memory(&mut self, data: &[u8]) -> Result<(), String> {
        let mut mem = self.phys_mem.mem.write();
        if data.len() != mem.len() {
        let mut mem = self.phys_mem.mem.write();
        if data.len() != mem.len() {
            return Err("Memory size mismatch".to_string());
        }
        mem.copy_from_slice(data);
        mem.copy_from_slice(data);
        Ok(())
    }

    fn poll_devices(&mut self) {
        let devices: Vec<Arc<RwLock<Box<dyn MmioDevice>>>> = {
            let mmio_regions = self.phys_mem.mmio_regions.read();
            mmio_regions.iter().map(|r| r.device.clone()).collect()
        };
        for dev_lock in devices {
            let mut dev = dev_lock.write();
            dev.poll(self);
        }
    }

    fn poll_devices(&mut self) {
        let devices: Vec<Arc<RwLock<Box<dyn MmioDevice>>>> = {
            let mmio_regions = self.phys_mem.mmio_regions.read();
            mmio_regions.iter().map(|r| r.device.clone()).collect()
        };
        for dev_lock in devices {
            let mut dev = dev_lock.write();
            dev.poll(self);
        }
    }

    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
}

impl SoftMmu {
    pub fn as_mmu_any(&self) -> &dyn std::any::Any { self }
    pub fn as_mmu_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    pub fn resize_tlbs(&mut self, itlb_size: usize, dtlb_size: usize) {
        self.itlb = Tlb::new(itlb_size.max(1));
        self.dtlb = Tlb::new(dtlb_size.max(1));
        self.tlb_hits = 0;
        self.tlb_misses = 0;
    }

    pub fn tlb_capacity(&self) -> (usize, usize) {
        (self.itlb.max_size, self.dtlb.max_size)
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
    ) -> Result<(), Fault> {
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
                    mmu.write_phys(new_table + i * PTE_SIZE, 0, 8)?;
                }
                // 写入页表项（非叶子）
                let new_pte = ((new_table >> PAGE_SHIFT) << 10) | pte_flags::V;
                mmu.write_phys(pte_addr, new_pte, 8)?;
                table_addr = new_table;
            } else {
                // 已有页表
                let ppn = (pte >> 10) & ((1u64 << 44) - 1);
                table_addr = ppn << PAGE_SHIFT;
            }
        }

        // 写入叶子 PTE
        let pte_addr = table_addr + vpn[0] * PTE_SIZE;
        let ppn = pa >> PAGE_SHIFT;
        let pte = (ppn << 10) | flags | pte_flags::V;
        mmu.write_phys(pte_addr, pte, 8)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bare_mode() {
        let mut mmu = SoftMmu::new(1024 * 1024, false);
        let mut mmu = SoftMmu::new(1024 * 1024, false);

        // Bare 模式恒等映射
        assert_eq!(
            mmu.translate(0x1000, AccessType::Read).expect("Operation failed"),
            0x1000
        );

        // 写入和读取
        mmu.write(0x100, 0xDEADBEEF, 4).expect("Operation failed");
        assert_eq!(mmu.read(0x100, 4).expect("Operation failed"), 0xDEADBEEF);
    }

    #[test]
    fn test_sv39_simple() {
        let mem_size = 16 * 1024 * 1024; // 16MB
        let mut mmu = SoftMmu::new(mem_size, false);
        let mut mmu = SoftMmu::new(mem_size, false);

        // 设置 SV39 分页
        let root_table = 0x100000; // 根页表在 1MB
        let mut builder = PageTableBuilder::new(root_table + PAGE_SIZE);

        // 初始化根页表
        for i in 0..PTES_PER_PAGE {
            mmu.write_phys(root_table + i * PTE_SIZE, 0, 8).expect("Operation failed");
        }

        // 映射虚拟地址 0x1000 -> 物理地址 0x2000
        let va = 0x1000u64;
        let pa = 0x200000u64; // 2MB
        let flags = pte_flags::R | pte_flags::W | pte_flags::X | pte_flags::A | pte_flags::D;
        builder.map_page_sv39(&mut mmu, va, pa, flags, root_table).expect("Operation failed");

        // 设置 satp
        let satp = (8u64 << 60) | (root_table >> PAGE_SHIFT); // MODE=Sv39, PPN
        mmu.set_satp(satp);

        // 测试地址翻译
        let translated = mmu.translate(va + 0x100, AccessType::Read).expect("Operation failed");
        assert_eq!(translated, pa + 0x100);
    }

    #[test]
    fn test_tlb_hit() {
        let mut mmu = SoftMmu::new(1024 * 1024, false);
        let mut mmu = SoftMmu::new(1024 * 1024, false);

        // 第一次访问（TLB miss）
        mmu.translate(0x1000, AccessType::Read).expect("Operation failed");
        let (hits1, _misses1) = mmu.tlb_stats();

        // 第二次访问（应该 TLB hit，但 Bare 模式不使用 TLB）
        mmu.translate(0x1000, AccessType::Read).expect("Operation failed");
        let (hits2, _misses2) = mmu.tlb_stats();

        // Bare 模式不使用 TLB，统计应该不变
        assert_eq!(hits1, hits2);
    }

    #[test]
    fn test_alignment_checks() {
        let mut mmu = SoftMmu::new(1024 * 1024, false);
        mmu.set_strict_align(true);
        mmu.write(0x100, 0xAA, 1).expect("Operation failed");
        assert!(mmu.read(0x100, 1).is_ok());
        assert!(matches!(mmu.read(0x101, 2), Err(Fault::AlignmentFault { .. })));
        assert!(matches!(mmu.read(0x102, 4), Err(Fault::AlignmentFault { .. })));
        assert!(matches!(mmu.read(0x104, 8), Err(Fault::AlignmentFault { .. })));
        assert!(matches!(mmu.write(0x101, 0xBB, 2), Err(Fault::AlignmentFault { .. })));
        assert!(matches!(mmu.write(0x102, 0xCC, 4), Err(Fault::AlignmentFault { .. })));
        assert!(matches!(mmu.write(0x104, 0xDD, 8), Err(Fault::AlignmentFault { .. })));
    }

    #[test]
    fn test_lr_sc_success() {
        let mut mmu = SoftMmu::new(1024 * 1024, false);
        // Initialize memory at addr
        let addr = 0x2000u64;
        mmu.write(addr, 0x11, 1).expect("Operation failed");
        // Load-reserved
        let v = mmu.load_reserved(addr, 1).expect("Operation failed");
        assert_eq!(v, 0x11);
        // Store-conditional should succeed and write new value
        let ok = mmu.store_conditional(addr, 0x22, 1).expect("Operation failed");
        assert!(ok);
        assert_eq!(mmu.read(addr, 1).expect("Operation failed"), 0x22);
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
        mmu1.write(addr1, 0x33, 1).expect("Operation failed");
        mmu2.write(addr2, 0x44, 1).expect("Operation failed");

        // mmu1 reserves addr1
        let _ = mmu1.load_reserved(addr1, 1).expect("Operation failed");

        // mmu2 writes to same cache line -> should invalidate mmu1's reservation
        mmu2.write(addr2, 0x55, 1).expect("Operation failed");

        // mmu1 store_conditional should fail
        let ok = mmu1.store_conditional(addr1, 0x66, 1).expect("Operation failed");
        assert!(!ok);
        // Memory remains unchanged at addr1
        assert_eq!(mmu1.read(addr1, 1).expect("Operation failed"), 0x33);
    }
}

pub mod mmu;
pub mod tlb;
pub mod asm_opt;
