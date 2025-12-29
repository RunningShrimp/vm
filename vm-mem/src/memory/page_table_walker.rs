//! 页表遍历器实现 - 从 SoftMmu 中分离出来

use vm_core::error::VmError;
use vm_core::{AccessType, Fault, GuestAddr, GuestPhysAddr, MMU, PageTableWalker};

/// RISC-V SV39 页表遍历器
pub struct Sv39PageTableWalker {
    /// 根页表物理地址
    pub page_table_base: GuestPhysAddr,
    /// 当前 ASID
    pub asid: u16,
}

impl Sv39PageTableWalker {
    pub fn new(base: GuestPhysAddr, asid: u16) -> Self {
        Self {
            page_table_base: base,
            asid,
        }
    }
}

impl PageTableWalker for Sv39PageTableWalker {
    fn walk(
        &mut self,
        addr: GuestAddr,
        _access: AccessType,
        asid: u16,
        _mmu: &mut dyn MMU,
    ) -> Result<(GuestPhysAddr, u64), VmError> {
        // 简化的页表遍历实现 - 直接返回地址转换结果
        // 在实际实现中，这里应该遍历页表结构

        // 检查ASID匹配
        if asid != self.asid {
            return Err(VmError::from(Fault::PageFault {
                addr,
                access_type: _access,
                is_write: matches!(_access, AccessType::Write),
                is_user: false,
            }));
        }

        // 简化的地址转换：假设虚拟地址直接映射到物理地址
        // 在实际实现中，这里应该遍历多级页表
        let phys_addr = GuestPhysAddr(addr.0 & 0xFFFFFFFF); // 简单的地址掩码
        let flags = 0b111; // 简化的标志：可读写执行

        Ok((phys_addr, flags))
    }
}

/// RISC-V SV48 页表遍历器
pub struct Sv48PageTableWalker {
    /// 根页表物理地址
    pub page_table_base: GuestPhysAddr,
    /// 当前 ASID
    pub asid: u16,
}

impl Sv48PageTableWalker {
    pub fn new(base: GuestPhysAddr, asid: u16) -> Self {
        Self {
            page_table_base: base,
            asid,
        }
    }
}

impl PageTableWalker for Sv48PageTableWalker {
    fn walk(
        &mut self,
        addr: GuestAddr,
        _access: AccessType,
        _asid: u16,
        _mmu: &mut dyn MMU,
    ) -> Result<(GuestPhysAddr, u64), VmError> {
        const PAGE_SHIFT: u64 = 12;
        const PAGE_SIZE: u64 = 1 << PAGE_SHIFT;
        const PAGE_MASK: u64 = !(PAGE_SIZE - 1);
        const VPN_BITS: u64 = 9;
        const VPN_MASK: u64 = (1 << VPN_BITS) - 1;
        const PTE_SIZE: u64 = 8;

        const V: u64 = 1 << 0;
        const R: u64 = 1 << 1;
        const W: u64 = 1 << 2;
        const X: u64 = 1 << 3;

        let vpn = [
            (addr.0 >> PAGE_SHIFT) & VPN_MASK,
            (addr.0 >> (PAGE_SHIFT + VPN_BITS)) & VPN_MASK,
            (addr.0 >> (PAGE_SHIFT + 2 * VPN_BITS)) & VPN_MASK,
            (addr.0 >> (PAGE_SHIFT + 3 * VPN_BITS)) & VPN_MASK,
        ];
        let offset = addr.0 & (PAGE_SIZE - 1);

        let mut pte_addr = self.page_table_base;
        let level = 3i32;

        // 执行页面表遍历（简化实现）
        pte_addr = GuestPhysAddr(pte_addr.0 + vpn[level as usize] * PTE_SIZE);

        // 使用计算出的物理地址和偏移量
        let phys_addr = GuestPhysAddr(pte_addr.0 & !PAGE_MASK | offset);
        let flags = V | R | W | X; // 简化的标志：可读写执行
        Ok((phys_addr, flags))
    }
}
