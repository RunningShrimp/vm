//! 页表遍历器实现 - 从 SoftMmu 中分离出来

use vm_core::{AccessType, Fault, GuestAddr, GuestPhysAddr, MMU, PageTableWalker, VmError};

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
        va: GuestAddr,
        access: AccessType,
        asid: u16,
        mmu: &mut dyn MMU,
    ) -> Result<(GuestPhysAddr, u64), VmError> {
        const PAGE_SIZE: u64 = 4096;
        const PAGE_SHIFT: u64 = 12;
        const VPN_BITS: u64 = 9;
        const VPN_MASK: u64 = (1 << VPN_BITS) - 1;
        const PTE_SIZE: u64 = 8;

        // PTE 标志
        const V: u64 = 1 << 0;
        const R: u64 = 1 << 1;
        const W: u64 = 1 << 2;
        const X: u64 = 1 << 3;
        const A: u64 = 1 << 6;
        const D: u64 = 1 << 7;

        let vpn = [
            (va >> 12) & VPN_MASK, // VPN[0]
            (va >> 21) & VPN_MASK, // VPN[1]
            (va >> 30) & VPN_MASK, // VPN[2]
        ];
        let offset = va & (PAGE_SIZE - 1);

        let mut pte_addr = self.page_table_base;
        let mut level = 2i32;

        loop {
            // 计算当前级别 PTE 地址
            pte_addr = pte_addr + vpn[level as usize] * PTE_SIZE;

            // 使用注入的MMU读取页表项
            let pte = mmu.read(pte_addr, 8)?;

            // 检查有效位
            if pte & V == 0 {
                return Err(VmError::from(Fault::PageFault { addr: va, access }));
            }

            let r = pte & R;
            let w = pte & W;
            let x = pte & X;

            // 如果 R=0 且 W=1，这是保留组合，产生页错误
            if r == 0 && w != 0 {
                return Err(VmError::from(Fault::PageFault { addr: va, access }));
            }

            // 叶子节点：R 或 X 位被设置
            if r != 0 || x != 0 {
                // 检查权限
                let required = match access {
                    AccessType::Read => R,
                    AccessType::Write => W,
                    AccessType::Exec => X,
                };

                if pte & required == 0 {
                    return Err(VmError::from(Fault::PageFault { addr: va, access }));
                }

                // 计算物理地址
                let ppn = (pte >> 10) & ((1u64 << 44) - 1);

                // 超级页对齐检查
                let pa = if level > 0 {
                    let shift = PAGE_SHIFT + (level as u64) * VPN_BITS;
                    (ppn << PAGE_SHIFT) | (va & ((1u64 << shift) - 1))
                } else {
                    (ppn << PAGE_SHIFT) | offset
                };

                return Ok((pa, pte));
            }

            // 非叶子节点：继续遍历
            if level == 0 {
                return Err(VmError::from(Fault::PageFault { addr: va, access }));
            }

            // 下一级页表地址
            let ppn = (pte >> 10) & ((1u64 << 44) - 1);
            pte_addr = ppn << PAGE_SHIFT;
            level -= 1;
        }
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
        va: GuestAddr,
        access: AccessType,
        asid: u16,
        mmu: &mut dyn MMU,
    ) -> Result<(GuestPhysAddr, u64), VmError> {
        const PAGE_SIZE: u64 = 4096;
        const PAGE_SHIFT: u64 = 12;
        const VPN_BITS: u64 = 9;
        const VPN_MASK: u64 = (1 << VPN_BITS) - 1;
        const PTE_SIZE: u64 = 8;

        const V: u64 = 1 << 0;
        const R: u64 = 1 << 1;
        const W: u64 = 1 << 2;
        const X: u64 = 1 << 3;

        let vpn = [
            (va >> 12) & VPN_MASK,
            (va >> 21) & VPN_MASK,
            (va >> 30) & VPN_MASK,
            (va >> 39) & VPN_MASK,
        ];
        let offset = va & (PAGE_SIZE - 1);

        let mut pte_addr = self.page_table_base;
        let mut level = 3i32;

        loop {
            pte_addr = pte_addr + vpn[level as usize] * PTE_SIZE;
            let pte = mmu.read(pte_addr, 8)?;

            if pte & V == 0 {
                return Err(VmError::from(Fault::PageFault { addr: va, access }));
            }

            let r = pte & R;
            let w = pte & W;
            let x = pte & X;

            if r == 0 && w != 0 {
                return Err(VmError::from(Fault::PageFault { addr: va, access }));
            }

            if r != 0 || x != 0 {
                let required = match access {
                    AccessType::Read => R,
                    AccessType::Write => W,
                    AccessType::Exec => X,
                };

                if pte & required == 0 {
                    return Err(VmError::from(Fault::PageFault { addr: va, access }));
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
                return Err(VmError::from(Fault::PageFault { addr: va, access }));
            }

            let ppn = (pte >> 10) & ((1u64 << 44) - 1);
            pte_addr = ppn << PAGE_SHIFT;
            level -= 1;
        }
    }
}
