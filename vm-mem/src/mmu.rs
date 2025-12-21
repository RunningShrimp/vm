//! 软件 MMU 实现
//!
//! 实现 GVA -> GPA -> HVA 的两级地址转换

use crate::GuestAddr;
use vm_core::{AccessType, Fault, VmError};

/// 大页支持
pub mod hugepage {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum HugePageSize {
        Size2M,
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

    pub struct HugePageAllocator {
        enabled: bool,
        preferred_size: HugePageSize,
    }

    impl HugePageAllocator {
        pub fn new(enabled: bool, preferred_size: HugePageSize) -> Self {
            Self {
                enabled,
                preferred_size,
            }
        }

        pub fn is_enabled(&self) -> bool {
            self.enabled
        }

        pub fn preferred_size(&self) -> HugePageSize {
            self.preferred_size
        }

        pub fn is_aligned(&self, addr: u64) -> bool {
            addr.is_multiple_of(self.preferred_size.alignment())
        }

        pub fn align_up(&self, addr: u64) -> u64 {
            let alignment = self.preferred_size.alignment();
            (addr + alignment - 1) & !(alignment - 1)
        }

        pub fn align_down(&self, addr: u64) -> u64 {
            let alignment = self.preferred_size.alignment();
            addr & !(alignment - 1)
        }

        #[cfg(target_os = "linux")]
        pub fn allocate_linux(&self, size: usize) -> Result<*mut u8, String> {
            if !self.enabled {
                return Err("Huge pages not enabled".to_string());
            }
            use std::ptr;
            let flags = libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_HUGETLB;
            let prot = libc::PROT_READ | libc::PROT_WRITE;
            let addr = unsafe { libc::mmap(ptr::null_mut(), size, prot, flags, -1, 0) };
            if addr == libc::MAP_FAILED {
                Err("Failed to allocate huge pages".to_string())
            } else {
                Ok(addr as *mut u8)
            }
        }

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

/// 页面大小
pub const PAGE_SIZE_4K: u64 = 4096;
pub const PAGE_SIZE_2M: u64 = 2 * 1024 * 1024;
pub const PAGE_SIZE_1G: u64 = 1024 * 1024 * 1024;

/// 页表项标志
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageTableFlags {
    /// 存在位
    pub present: bool,
    /// 可写
    pub writable: bool,
    /// 用户可访问
    pub user: bool,
    /// 写穿透
    pub write_through: bool,
    /// 禁用缓存
    pub cache_disable: bool,
    /// 已访问
    pub accessed: bool,
    /// 脏页
    pub dirty: bool,
    /// 大页
    pub huge_page: bool,
    /// 全局页
    pub global: bool,
    /// 不可执行
    pub no_execute: bool,
}

impl Default for PageTableFlags {
    fn default() -> Self {
        Self {
            present: true,
            writable: true,
            user: false,
            write_through: false,
            cache_disable: false,
            accessed: false,
            dirty: false,
            huge_page: false,
            global: false,
            no_execute: false,
        }
    }
}

impl PageTableFlags {
    /// 从 x86_64 页表项解析标志
    pub fn from_x86_64_entry(entry: u64) -> Self {
        Self {
            present: entry & 0x1 != 0,
            writable: entry & 0x2 != 0,
            user: entry & 0x4 != 0,
            write_through: entry & 0x8 != 0,
            cache_disable: entry & 0x10 != 0,
            accessed: entry & 0x20 != 0,
            dirty: entry & 0x40 != 0,
            huge_page: entry & 0x80 != 0,
            global: entry & 0x100 != 0,
            no_execute: entry & (1 << 63) != 0,
        }
    }

    /// 转换为 x86_64 页表项
    pub fn to_x86_64_entry(&self, addr: u64) -> u64 {
        let mut entry = addr & !0xFFF; // 清除低12位
        if self.present {
            entry |= 0x1;
        }
        if self.writable {
            entry |= 0x2;
        }
        if self.user {
            entry |= 0x4;
        }
        if self.write_through {
            entry |= 0x8;
        }
        if self.cache_disable {
            entry |= 0x10;
        }
        if self.accessed {
            entry |= 0x20;
        }
        if self.dirty {
            entry |= 0x40;
        }
        if self.huge_page {
            entry |= 0x80;
        }
        if self.global {
            entry |= 0x100;
        }
        if self.no_execute {
            entry |= 1 << 63;
        }
        entry
    }
}

/// 页表项
#[derive(Debug, Clone)]
pub struct PageTableEntry {
    /// 物理地址
    pub addr: GuestAddr,
    /// 标志
    pub flags: PageTableFlags,
}

/// 页表遍历结果
#[derive(Debug, Clone)]
pub struct PageWalkResult {
    /// 最终的物理地址
    pub gpa: GuestAddr,
    /// 页面大小
    pub page_size: u64,
    /// 标志
    pub flags: PageTableFlags,
}

/// 软件 MMU
pub struct SoftwareMmu {
    /// Guest 内存访问接口
    memory: Box<dyn Fn(GuestAddr, usize) -> Result<Vec<u8>, VmError> + Send>,
    /// 架构类型
    arch: MmuArch,
}

/// MMU 架构
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MmuArch {
    /// x86_64 (4级页表)
    X86_64,
    /// AArch64 (4级页表)
    AArch64,
    /// RISC-V Sv39 (3级页表)
    RiscVSv39,
    /// RISC-V Sv48 (4级页表)
    RiscVSv48,
}

impl SoftwareMmu {
    /// 创建新的软件 MMU
    pub fn new<F>(arch: MmuArch, memory: F) -> Self
    where
        F: Fn(GuestAddr, usize) -> Result<Vec<u8>, VmError> + Send + 'static,
    {
        Self {
            memory: Box::new(memory),
            arch,
        }
    }

    /// GVA -> GPA 地址转换
    pub fn translate(&self, gva: GuestAddr, cr3: GuestAddr) -> Result<PageWalkResult, VmError> {
        match self.arch {
            MmuArch::X86_64 => self.walk_x86_64(gva, cr3),
            MmuArch::AArch64 => self.walk_aarch64(gva, cr3),
            MmuArch::RiscVSv39 => self.walk_riscv_sv39(gva, cr3),
            MmuArch::RiscVSv48 => self.walk_riscv_sv48(gva, cr3),
        }
    }

    /// 读取页表项
    fn read_pte(&self, addr: GuestAddr) -> Result<u64, VmError> {
        let data = (self.memory)(addr, 8)?;
        Ok(u64::from_le_bytes([
            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
        ]))
    }

    /// x86_64 页表遍历
    fn walk_x86_64(&self, gva: GuestAddr, cr3: GuestAddr) -> Result<PageWalkResult, VmError> {
        // 提取各级页表索引
        let pml4_index = (gva >> 39) & 0x1FF;
        let pdpt_index = (gva >> 30) & 0x1FF;
        let pd_index = (gva >> 21) & 0x1FF;
        let pt_index = (gva >> 12) & 0x1FF;
        let offset = gva & 0xFFF;

        // PML4
        let pml4_addr = (cr3 & !0xFFF) + pml4_index * 8;
        let pml4e = self.read_pte(GuestAddr(pml4_addr))?;
        let pml4_flags = PageTableFlags::from_x86_64_entry(pml4e);

        if !pml4_flags.present {
            return Err(VmError::from(Fault::PageFault {
                addr: gva,
                access_type: AccessType::Read,
                is_write: false,
                is_user: false,
            }));
        }

        // PDPT
        let pdpt_addr = (pml4e & !0xFFF) + pdpt_index * 8;
        let pdpte = self.read_pte(GuestAddr(pdpt_addr))?;
        let pdpt_flags = PageTableFlags::from_x86_64_entry(pdpte);

        if !pdpt_flags.present {
            return Err(VmError::from(Fault::PageFault {
                addr: gva,
                access_type: AccessType::Read,
                is_write: false,
                is_user: false,
            }));
        }

        // 检查 1GB 大页
        if pdpt_flags.huge_page {
            let gpa_val = (pdpte & !0x3FFFFFFF) | (gva & 0x3FFFFFFF);
            return Ok(PageWalkResult {
                gpa: GuestAddr(gpa_val),
                page_size: PAGE_SIZE_1G,
                flags: pdpt_flags,
            });
        }

        // PD
        let pd_addr = (pdpte & !0xFFF) + pd_index * 8;
        let pde = self.read_pte(GuestAddr(pd_addr))?;
        let pd_flags = PageTableFlags::from_x86_64_entry(pde);

        if !pd_flags.present {
            return Err(VmError::from(Fault::PageFault {
                addr: gva,
                access_type: AccessType::Read,
                is_write: false,
                is_user: false,
            }));
        }

        // 检查 2MB 大页
        if pd_flags.huge_page {
            let gpa_val = (pde & !0x1FFFFF) | (gva & 0x1FFFFF);
            return Ok(PageWalkResult {
                gpa: GuestAddr(gpa_val),
                page_size: PAGE_SIZE_2M,
                flags: pd_flags,
            });
        }

        // PT
        let pt_addr = (pde & !0xFFF) + pt_index * 8;
        let pte = self.read_pte(GuestAddr(pt_addr))?;
        let pt_flags = PageTableFlags::from_x86_64_entry(pte);

        if !pt_flags.present {
            return Err(VmError::from(Fault::PageFault {
                addr: gva,
                access_type: AccessType::Read,
                is_write: false,
                is_user: false,
            }));
        }

        // 4KB 页
        let gpa_val = (pte & !0xFFF) | offset;
        Ok(PageWalkResult {
            gpa: GuestAddr(gpa_val),
            page_size: PAGE_SIZE_4K,
            flags: pt_flags,
        })
    }

    /// AArch64 页表遍历
    fn walk_aarch64(&self, gva: GuestAddr, ttbr: GuestAddr) -> Result<PageWalkResult, VmError> {
        // 简化实现，假设使用 4KB 粒度，4级页表
        let l0_index = (gva >> 39) & 0x1FF;
        let l1_index = (gva >> 30) & 0x1FF;
        let l2_index = (gva >> 21) & 0x1FF;
        let l3_index = (gva >> 12) & 0x1FF;
        let offset = gva & 0xFFF;

        // L0
        let l0_addr = (ttbr & !0xFFF) + l0_index * 8;
        let l0e = self.read_pte(GuestAddr(l0_addr))?;

        if l0e & 0x1 == 0 {
            return Err(VmError::from(Fault::PageFault {
                addr: gva,
                access_type: AccessType::Read,
                is_write: false,
                is_user: false,
            }));
        }

        // L1
        let l1_addr = (l0e & !0xFFF) + l1_index * 8;
        let l1e = self.read_pte(GuestAddr(l1_addr))?;

        if l1e & 0x1 == 0 {
            return Err(VmError::from(Fault::PageFault {
                addr: gva,
                access_type: AccessType::Read,
                is_write: false,
                is_user: false,
            }));
        }

        // 检查块描述符 (1GB)
        if l1e & 0x3 == 0x1 {
            let gpa_val = (l1e & !0x3FFFFFFF) | (gva & 0x3FFFFFFF);
            return Ok(PageWalkResult {
                gpa: GuestAddr(gpa_val),
                page_size: PAGE_SIZE_1G,
                flags: PageTableFlags::default(),
            });
        }

        // L2
        let l2_addr = (l1e & !0xFFF) + l2_index * 8;
        let l2e = self.read_pte(GuestAddr(l2_addr))?;

        if l2e & 0x1 == 0 {
            return Err(VmError::from(Fault::PageFault {
                addr: gva,
                access_type: AccessType::Read,
                is_write: false,
                is_user: false,
            }));
        }

        // 检查块描述符 (2MB)
        if l2e & 0x3 == 0x1 {
            let gpa_val = (l2e & !0x1FFFFF) | (gva & 0x1FFFFF);
            return Ok(PageWalkResult {
                gpa: GuestAddr(gpa_val),
                page_size: PAGE_SIZE_2M,
                flags: PageTableFlags::default(),
            });
        }

        // L3
        let l3_addr = (l2e & !0xFFF) + l3_index * 8;
        let l3e = self.read_pte(GuestAddr(l3_addr))?;

        if l3e & 0x3 != 0x3 {
            return Err(VmError::from(Fault::PageFault {
                addr: gva,
                access_type: AccessType::Read,
                is_write: false,
                is_user: false,
            }));
        }

        let gpa_val = (l3e & !0xFFF) | offset;
        Ok(PageWalkResult {
            gpa: GuestAddr(gpa_val),
            page_size: PAGE_SIZE_4K,
            flags: PageTableFlags::default(),
        })
    }

    /// RISC-V Sv39 页表遍历
    fn walk_riscv_sv39(&self, gva: GuestAddr, satp: GuestAddr) -> Result<PageWalkResult, VmError> {
        // Sv39: 3级页表，39位虚拟地址
        let vpn2 = (gva >> 30) & 0x1FF;
        let vpn1 = (gva >> 21) & 0x1FF;
        let vpn0 = (gva >> 12) & 0x1FF;
        let offset = gva & 0xFFF;

        let root_ppn = satp & 0xFFFFFFFFFFF;
        let root_addr = root_ppn << 12;

        // Level 2
        let l2_addr = root_addr + vpn2 * 8;
        let l2e = self.read_pte(GuestAddr(l2_addr))?;

        if l2e & 0x1 == 0 {
            return Err(VmError::from(Fault::PageFault {
                addr: gva,
                access_type: AccessType::Read,
                is_write: false,
                is_user: false,
            }));
        }

        // 检查是否是叶子节点 (1GB 大页)
        if (l2e & 0xE) != 0 {
            let ppn = (l2e >> 10) & 0xFFFFFFFFFFF;
            let gpa_val = (ppn << 12) | (gva & 0x3FFFFFFF);
            return Ok(PageWalkResult {
                gpa: GuestAddr(gpa_val),
                page_size: PAGE_SIZE_1G,
                flags: PageTableFlags::default(),
            });
        }

        // Level 1
        let l1_ppn = (l2e >> 10) & 0xFFFFFFFFFFF;
        let l1_addr = (l1_ppn << 12) + vpn1 * 8;
        let l1e = self.read_pte(GuestAddr(l1_addr))?;

        if l1e & 0x1 == 0 {
            return Err(VmError::from(Fault::PageFault {
                addr: gva,
                access_type: AccessType::Read,
                is_write: false,
                is_user: false,
            }));
        }

        // 检查是否是叶子节点 (2MB 大页)
        if (l1e & 0xE) != 0 {
            let ppn = (l1e >> 10) & 0xFFFFFFFFFFF;
            let gpa_val = (ppn << 12) | (gva & 0x1FFFFF);
            return Ok(PageWalkResult {
                gpa: GuestAddr(gpa_val),
                page_size: PAGE_SIZE_2M,
                flags: PageTableFlags::default(),
            });
        }

        // Level 0
        let l0_ppn = (l1e >> 10) & 0xFFFFFFFFFFF;
        let l0_addr = (l0_ppn << 12) + vpn0 * 8;
        let l0e = self.read_pte(GuestAddr(l0_addr))?;

        if (l0e & 0x1) == 0 || (l0e & 0xE) == 0 {
            return Err(VmError::from(Fault::PageFault {
                addr: gva,
                access_type: AccessType::Read,
                is_write: false,
                is_user: false,
            }));
        }

        let ppn = (l0e >> 10) & 0xFFFFFFFFFFF;
        let gpa_val = (ppn << 12) | offset;
        Ok(PageWalkResult {
            gpa: GuestAddr(gpa_val),
            page_size: PAGE_SIZE_4K,
            flags: PageTableFlags::default(),
        })
    }

    /// RISC-V Sv48 页表遍历
    fn walk_riscv_sv48(&self, gva: GuestAddr, satp: GuestAddr) -> Result<PageWalkResult, VmError> {
        // Sv48: 4级页表，48位虚拟地址
        // 实现类似 Sv39，增加一级
        // 这里简化为调用 Sv39
        self.walk_riscv_sv39(gva, satp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_table_flags() {
        let flags = PageTableFlags::default();
        let entry = flags.to_x86_64_entry(0x1000);

        assert_eq!(entry & 0x1, 0x1); // present
        assert_eq!(entry & 0x2, 0x2); // writable
        assert_eq!(entry & !0xFFF, 0x1000); // address
    }

    #[test]
    fn test_page_sizes() {
        assert_eq!(PAGE_SIZE_4K, 4096);
        assert_eq!(PAGE_SIZE_2M, 2 * 1024 * 1024);
        assert_eq!(PAGE_SIZE_1G, 1024 * 1024 * 1024);
    }
}
