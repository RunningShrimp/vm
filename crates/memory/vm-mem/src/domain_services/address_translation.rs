//! 地址转换领域服务
//!
//! 提供虚拟地址到物理地址的转换逻辑，支持多种架构。

use vm_core::{AccessType, Fault, VmError};

use crate::GuestAddr;
use crate::mmu::{
    MmuArch, PAGE_SIZE_1G, PAGE_SIZE_2M, PAGE_SIZE_4K, PageTableFlags, PageWalkResult,
};

/// 地址转换领域服务
///
/// 负责执行虚拟地址到物理地址的转换，这是虚拟机内存管理的核心业务逻辑。
/// 支持多种架构的页表遍历。
pub struct AddressTranslationDomainService {
    /// Guest 内存访问接口
    memory: Box<dyn Fn(GuestAddr, usize) -> Result<Vec<u8>, VmError> + Send>,
    /// 架构类型
    arch: MmuArch,
}

impl AddressTranslationDomainService {
    /// 创建新的地址转换服务
    pub fn new<F>(arch: MmuArch, memory: F) -> Self
    where
        F: Fn(GuestAddr, usize) -> Result<Vec<u8>, VmError> + Send + 'static,
    {
        Self {
            memory: Box::new(memory),
            arch,
        }
    }

    /// 获取架构类型
    pub fn arch(&self) -> MmuArch {
        self.arch
    }

    /// GVA -> GPA 地址转换
    ///
    /// 根据架构类型选择相应的页表遍历算法。
    ///
    /// # 参数
    /// - `gva`: Guest虚拟地址
    /// - `cr3/ttbr`: 页表基址寄存器
    ///
    /// # 返回
    /// 页表遍历结果，包含物理地址、页面大小和标志
    pub fn translate(&self, gva: GuestAddr, cr3: GuestAddr) -> Result<PageWalkResult, VmError> {
        match self.arch {
            MmuArch::X86_64 => self.walk_x86_64(gva, cr3),
            MmuArch::AArch64 => self.walk_aarch64(gva, cr3),
            MmuArch::RiscVSv39 => self.walk_riscv_sv39(gva, cr3),
            MmuArch::RiscVSv48 => self.walk_riscv_sv48(gva, cr3),
        }
    }

    /// 读取页表项
    ///
    /// 从指定地址读取8字节的页表项。
    fn read_pte(&self, addr: GuestAddr) -> Result<u64, VmError> {
        let data = (self.memory)(addr, 8)?;
        Ok(u64::from_le_bytes([
            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
        ]))
    }

    /// x86_64 页表遍历
    ///
    /// 遍历4级页表（PML4 -> PDPT -> PD -> PT），支持4KB、2MB和1GB页面。
    fn walk_x86_64(&self, gva: GuestAddr, cr3: GuestAddr) -> Result<PageWalkResult, VmError> {
        let pml4_index = (gva >> 39) & 0x1FF;
        let pdpt_index = (gva >> 30) & 0x1FF;
        let pd_index = (gva >> 21) & 0x1FF;
        let pt_index = (gva >> 12) & 0x1FF;
        let offset = gva & 0xFFF;

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

        if pdpt_flags.huge_page {
            let gpa_val = (pdpte & !0x3FFFFFFF) | (gva & 0x3FFFFFFF);
            return Ok(PageWalkResult {
                gpa: GuestAddr(gpa_val),
                page_size: PAGE_SIZE_1G,
                flags: pdpt_flags,
            });
        }

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

        if pd_flags.huge_page {
            let gpa_val = (pde & !0x1FFFFF) | (gva & 0x1FFFFF);
            return Ok(PageWalkResult {
                gpa: GuestAddr(gpa_val),
                page_size: PAGE_SIZE_2M,
                flags: pd_flags,
            });
        }

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

        let gpa_val = (pte & !0xFFF) | offset;
        Ok(PageWalkResult {
            gpa: GuestAddr(gpa_val),
            page_size: PAGE_SIZE_4K,
            flags: pt_flags,
        })
    }

    /// AArch64 页表遍历
    ///
    /// 遍历4级页表（L0 -> L1 -> L2 -> L3），支持4KB、2MB和1GB页面。
    fn walk_aarch64(&self, gva: GuestAddr, ttbr: GuestAddr) -> Result<PageWalkResult, VmError> {
        let l0_index = (gva >> 39) & 0x1FF;
        let l1_index = (gva >> 30) & 0x1FF;
        let l2_index = (gva >> 21) & 0x1FF;
        let l3_index = (gva >> 12) & 0x1FF;
        let offset = gva & 0xFFF;

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

        if l1e & 0x3 == 0x1 {
            let gpa_val = (l1e & !0x3FFFFFFF) | (gva & 0x3FFFFFFF);
            return Ok(PageWalkResult {
                gpa: GuestAddr(gpa_val),
                page_size: PAGE_SIZE_1G,
                flags: PageTableFlags::default(),
            });
        }

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

        if l2e & 0x3 == 0x1 {
            let gpa_val = (l2e & !0x1FFFFF) | (gva & 0x1FFFFF);
            return Ok(PageWalkResult {
                gpa: GuestAddr(gpa_val),
                page_size: PAGE_SIZE_2M,
                flags: PageTableFlags::default(),
            });
        }

        let l3_addr = (l2e & !0xFFF) + l3_index * 8;
        let l3e = self.read_pte(GuestAddr(l3_addr))?;

        if l3e & 0x1 == 0 {
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
    ///
    /// 遍历3级页表，支持4KB和2MB页面。
    fn walk_riscv_sv39(&self, gva: GuestAddr, satp: GuestAddr) -> Result<PageWalkResult, VmError> {
        let vpn2 = (gva >> 30) & 0x1FF;
        let vpn1 = (gva >> 21) & 0x1FF;
        let vpn0 = (gva >> 12) & 0x1FF;
        let offset = gva & 0xFFF;

        let ppn = (satp >> 44) & 0xFFF_FFFF_FFFF;
        let pgtbl_base = ppn << 12;

        let l2_addr = pgtbl_base + vpn2 * 8;
        let l2e = self.read_pte(GuestAddr(l2_addr))?;

        if l2e & 0x1 == 0 {
            return Err(VmError::from(Fault::PageFault {
                addr: gva,
                access_type: AccessType::Read,
                is_write: false,
                is_user: false,
            }));
        }

        if l2e & 0xE != 0x8 {
            let gpa_val = ((l2e >> 10) << 12) | (gva & 0x3FFF_FFFF);
            return Ok(PageWalkResult {
                gpa: GuestAddr(gpa_val),
                page_size: PAGE_SIZE_1G,
                flags: PageTableFlags::default(),
            });
        }

        let l1_addr = ((l2e >> 10) << 12) + vpn1 * 8;
        let l1e = self.read_pte(GuestAddr(l1_addr))?;

        if l1e & 0x1 == 0 {
            return Err(VmError::from(Fault::PageFault {
                addr: gva,
                access_type: AccessType::Read,
                is_write: false,
                is_user: false,
            }));
        }

        if l1e & 0xE != 0x8 {
            let gpa_val = ((l1e >> 10) << 12) | (gva & 0x1F_FFFF);
            return Ok(PageWalkResult {
                gpa: GuestAddr(gpa_val),
                page_size: PAGE_SIZE_2M,
                flags: PageTableFlags::default(),
            });
        }

        let l0_addr = ((l1e >> 10) << 12) + vpn0 * 8;
        let l0e = self.read_pte(GuestAddr(l0_addr))?;

        if l0e & 0x1 == 0 {
            return Err(VmError::from(Fault::PageFault {
                addr: gva,
                access_type: AccessType::Read,
                is_write: false,
                is_user: false,
            }));
        }

        let gpa_val = ((l0e >> 10) << 12) | offset;
        Ok(PageWalkResult {
            gpa: GuestAddr(gpa_val),
            page_size: PAGE_SIZE_4K,
            flags: PageTableFlags::default(),
        })
    }

    /// RISC-V Sv48 页表遍历
    ///
    /// 遍历4级页表，支持4KB、2MB和1GB页面。
    fn walk_riscv_sv48(&self, gva: GuestAddr, satp: GuestAddr) -> Result<PageWalkResult, VmError> {
        let vpn3 = (gva >> 39) & 0x1FF;
        let vpn2 = (gva >> 30) & 0x1FF;
        let vpn1 = (gva >> 21) & 0x1FF;
        let vpn0 = (gva >> 12) & 0x1FF;
        let offset = gva & 0xFFF;

        let ppn = (satp >> 44) & 0xFFF_FFFF_FFFF;
        let pgtbl_base = ppn << 12;

        let l3_addr = pgtbl_base + vpn3 * 8;
        let l3e = self.read_pte(GuestAddr(l3_addr))?;

        if l3e & 0x1 == 0 {
            return Err(VmError::from(Fault::PageFault {
                addr: gva,
                access_type: AccessType::Read,
                is_write: false,
                is_user: false,
            }));
        }

        let l2_addr = ((l3e >> 10) << 12) + vpn2 * 8;
        let l2e = self.read_pte(GuestAddr(l2_addr))?;

        if l2e & 0x1 == 0 {
            return Err(VmError::from(Fault::PageFault {
                addr: gva,
                access_type: AccessType::Read,
                is_write: false,
                is_user: false,
            }));
        }

        if l2e & 0xE != 0x8 {
            let gpa_val = ((l2e >> 10) << 12) | (gva & 0x3FFF_FFFF);
            return Ok(PageWalkResult {
                gpa: GuestAddr(gpa_val),
                page_size: PAGE_SIZE_1G,
                flags: PageTableFlags::default(),
            });
        }

        let l1_addr = ((l2e >> 10) << 12) + vpn1 * 8;
        let l1e = self.read_pte(GuestAddr(l1_addr))?;

        if l1e & 0x1 == 0 {
            return Err(VmError::from(Fault::PageFault {
                addr: gva,
                access_type: AccessType::Read,
                is_write: false,
                is_user: false,
            }));
        }

        if l1e & 0xE != 0x8 {
            let gpa_val = ((l1e >> 10) << 12) | (gva & 0x1F_FFFF);
            return Ok(PageWalkResult {
                gpa: GuestAddr(gpa_val),
                page_size: PAGE_SIZE_2M,
                flags: PageTableFlags::default(),
            });
        }

        let l0_addr = ((l1e >> 10) << 12) + vpn0 * 8;
        let l0e = self.read_pte(GuestAddr(l0_addr))?;

        if l0e & 0x1 == 0 {
            return Err(VmError::from(Fault::PageFault {
                addr: gva,
                access_type: AccessType::Read,
                is_write: false,
                is_user: false,
            }));
        }

        let gpa_val = ((l0e >> 10) << 12) | offset;
        Ok(PageWalkResult {
            gpa: GuestAddr(gpa_val),
            page_size: PAGE_SIZE_4K,
            flags: PageTableFlags::default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_address_translation_service_creation() {
        let memory =
            |_addr: GuestAddr, size: usize| -> Result<Vec<u8>, VmError> { Ok(vec![0u8; size]) };
        let service = AddressTranslationDomainService::new(MmuArch::X86_64, memory);
        assert_eq!(service.arch(), MmuArch::X86_64);
    }
}
