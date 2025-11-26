//! vm-mem: 内存管理单元实现
//! 
//! 包含 SoftMMU（软件 MMU）和 RISC-V SV39/SV48 页表遍历

use vm_core::{MMU, MmioDevice, GuestAddr, GuestPhysAddr, AccessType, Fault};

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
// TLB 实现
// ============================================================================

/// TLB 条目
#[derive(Clone, Copy)]
struct TlbEntry {
    vpn: u64,
    ppn: u64,
    flags: u64,
    asid: u16,
}

/// TLB 缓存
struct Tlb {
    entries: Vec<Option<TlbEntry>>,
    size: usize,
    next_victim: usize,
}

impl Tlb {
    fn new(size: usize) -> Self {
        Self {
            entries: vec![None; size],
            size,
            next_victim: 0,
        }
    }

    fn lookup(&self, vpn: u64, asid: u16) -> Option<(u64, u64)> {
        for entry in &self.entries {
            if let Some(e) = entry {
                if e.vpn == vpn && (e.asid == asid || e.flags & pte_flags::G != 0) {
                    return Some((e.ppn, e.flags));
                }
            }
        }
        None
    }

    fn insert(&mut self, vpn: u64, ppn: u64, flags: u64, asid: u16) {
        let entry = TlbEntry { vpn, ppn, flags, asid };
        self.entries[self.next_victim] = Some(entry);
        self.next_victim = (self.next_victim + 1) % self.size;
    }

    fn flush(&mut self) {
        for entry in &mut self.entries {
            *entry = None;
        }
    }

    fn flush_asid(&mut self, asid: u16) {
        for entry in &mut self.entries {
            if let Some(e) = entry {
                if e.asid == asid && e.flags & pte_flags::G == 0 {
                    *entry = None;
                }
            }
        }
    }

    fn flush_page(&mut self, vpn: u64) {
        for entry in &mut self.entries {
            if let Some(e) = entry {
                if e.vpn == vpn {
                    *entry = None;
                }
            }
        }
    }
}

// ============================================================================
// MMIO 区域
// ============================================================================

struct MmioRegion {
    base: GuestAddr,
    size: u64,
    device: Box<dyn MmioDevice>,
}

// ============================================================================
// SoftMmu 实现
// ============================================================================

/// 软件 MMU 实现
pub struct SoftMmu {
    /// 物理内存
    mem: Vec<u8>,
    /// 指令 TLB
    itlb: Tlb,
    /// 数据 TLB
    dtlb: Tlb,
    /// MMIO 设备区域
    mmio_regions: Vec<MmioRegion>,
    /// 分页模式
    paging_mode: PagingMode,
    /// 页表基址寄存器（satp for RISC-V）
    page_table_base: GuestPhysAddr,
    /// 当前 ASID
    asid: u16,
    /// TLB 统计
    tlb_hits: u64,
    tlb_misses: u64,
}

impl SoftMmu {
    /// 创建默认大小（64KB）的 MMU
    pub fn new_default() -> Self {
        Self::new(64 * 1024)
    }

    /// 创建指定大小的 MMU
    pub fn new(size: usize) -> Self {
        Self {
            mem: vec![0u8; size],
            itlb: Tlb::new(64),
            dtlb: Tlb::new(128),
            mmio_regions: Vec::new(),
            paging_mode: PagingMode::Bare,
            page_table_base: 0,
            asid: 0,
            tlb_hits: 0,
            tlb_misses: 0,
        }
    }

    /// 设置分页模式
    pub fn set_paging_mode(&mut self, mode: PagingMode) {
        if self.paging_mode != mode {
            self.paging_mode = mode;
            self.itlb.flush();
            self.dtlb.flush();
        }
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

        // 检查 MMIO 区域
        for region in &self.mmio_regions {
            if pa >= region.base && pa < region.base + region.size {
                return Ok(region.device.read(pa - region.base, size));
            }
        }

        // 边界检查
        if addr + (size as usize) > self.mem.len() {
            return Err(Fault::AccessViolation {
                addr: pa,
                access: AccessType::Read,
            });
        }

        // 读取内存
        let val = match size {
            1 => self.mem[addr] as u64,
            2 => u16::from_le_bytes([self.mem[addr], self.mem[addr + 1]]) as u64,
            4 => u32::from_le_bytes([
                self.mem[addr],
                self.mem[addr + 1],
                self.mem[addr + 2],
                self.mem[addr + 3],
            ]) as u64,
            8 => u64::from_le_bytes([
                self.mem[addr],
                self.mem[addr + 1],
                self.mem[addr + 2],
                self.mem[addr + 3],
                self.mem[addr + 4],
                self.mem[addr + 5],
                self.mem[addr + 6],
                self.mem[addr + 7],
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

        // 检查 MMIO 区域
        for region in &mut self.mmio_regions {
            if pa >= region.base && pa < region.base + region.size {
                region.device.write(pa - region.base, val, size);
                return Ok(());
            }
        }

        // 边界检查
        if addr + (size as usize) > self.mem.len() {
            return Err(Fault::AccessViolation {
                addr: pa,
                access: AccessType::Write,
            });
        }

        // 写入内存
        match size {
            1 => self.mem[addr] = val as u8,
            2 => {
                let bytes = (val as u16).to_le_bytes();
                self.mem[addr] = bytes[0];
                self.mem[addr + 1] = bytes[1];
            }
            4 => {
                let bytes = (val as u32).to_le_bytes();
                self.mem[addr] = bytes[0];
                self.mem[addr + 1] = bytes[1];
                self.mem[addr + 2] = bytes[2];
                self.mem[addr + 3] = bytes[3];
            }
            8 => {
                let bytes = val.to_le_bytes();
                for i in 0..8 {
                    self.mem[addr + i] = bytes[i];
                }
            }
            _ => {
                return Err(Fault::AlignmentFault { addr: pa, size });
            }
        }

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
        let tlb = match access {
            AccessType::Exec => &self.itlb,
            _ => &self.dtlb,
        };

        if let Some((ppn, flags)) = tlb.lookup(vpn, self.asid) {
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
        let tlb = match access {
            AccessType::Exec => &mut self.itlb,
            _ => &mut self.dtlb,
        };
        tlb.insert(vpn, ppn, flags, self.asid);

        Ok(pa)
    }

    fn fetch_insn(&self, pc: GuestAddr) -> Result<u64, Fault> {
        self.read_phys(pc, 4)
    }

    fn read(&self, pa: GuestAddr, size: u8) -> Result<u64, Fault> {
        self.read_phys(pa, size)
    }

    fn write(&mut self, pa: GuestAddr, val: u64, size: u8) -> Result<(), Fault> {
        self.write_phys(pa, val, size)
    }

    fn map_mmio(&mut self, base: GuestAddr, size: u64, device: Box<dyn MmioDevice>) {
        self.mmio_regions.push(MmioRegion { base, size, device });
    }

    fn flush_tlb(&mut self) {
        self.itlb.flush();
        self.dtlb.flush();
    }

    fn memory_size(&self) -> usize {
        self.mem.len()
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
        let mut mmu = SoftMmu::new(1024 * 1024);

        // Bare 模式恒等映射
        assert_eq!(
            mmu.translate(0x1000, AccessType::Read).unwrap(),
            0x1000
        );

        // 写入和读取
        mmu.write(0x100, 0xDEADBEEF, 4).unwrap();
        assert_eq!(mmu.read(0x100, 4).unwrap(), 0xDEADBEEF);
    }

    #[test]
    fn test_sv39_simple() {
        let mem_size = 16 * 1024 * 1024; // 16MB
        let mut mmu = SoftMmu::new(mem_size);

        // 设置 SV39 分页
        let root_table = 0x100000; // 根页表在 1MB
        let mut builder = PageTableBuilder::new(root_table + PAGE_SIZE);

        // 初始化根页表
        for i in 0..PTES_PER_PAGE {
            mmu.write_phys(root_table + i * PTE_SIZE, 0, 8).unwrap();
        }

        // 映射虚拟地址 0x1000 -> 物理地址 0x2000
        let va = 0x1000u64;
        let pa = 0x200000u64; // 2MB
        let flags = pte_flags::R | pte_flags::W | pte_flags::X | pte_flags::A | pte_flags::D;
        builder.map_page_sv39(&mut mmu, va, pa, flags, root_table).unwrap();

        // 设置 satp
        let satp = (8u64 << 60) | (root_table >> PAGE_SHIFT); // MODE=Sv39, PPN
        mmu.set_satp(satp);

        // 测试地址翻译
        let translated = mmu.translate(va + 0x100, AccessType::Read).unwrap();
        assert_eq!(translated, pa + 0x100);
    }

    #[test]
    fn test_tlb_hit() {
        let mut mmu = SoftMmu::new(1024 * 1024);

        // 第一次访问（TLB miss）
        mmu.translate(0x1000, AccessType::Read).unwrap();
        let (hits1, misses1) = mmu.tlb_stats();

        // 第二次访问（应该 TLB hit，但 Bare 模式不使用 TLB）
        mmu.translate(0x1000, AccessType::Read).unwrap();
        let (hits2, _misses2) = mmu.tlb_stats();

        // Bare 模式不使用 TLB，统计应该不变
        assert_eq!(hits1, hits2);
    }
}
