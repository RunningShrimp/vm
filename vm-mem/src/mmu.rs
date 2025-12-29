//! 软件 MMU 实现
//!
//! 实现 GVA -> GPA -> HVA 的两级地址转换
//! 地址转换逻辑已委托给 AddressTranslationDomainService

use crate::GuestAddr;
use crate::domain_services::AddressTranslationDomainService;
use vm_core::VmError;

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
///
/// 此MMU实现将地址转换逻辑委托给 AddressTranslationDomainService。
/// 符合DDD贫血模型原则，将业务逻辑放在领域服务中。
pub struct SoftwareMmu {
    /// 地址转换领域服务
    translation_service: AddressTranslationDomainService,
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
    ///
    /// 地址转换逻辑委托给 AddressTranslationDomainService。
    pub fn new<F>(arch: MmuArch, memory: F) -> Self
    where
        F: Fn(GuestAddr, usize) -> Result<Vec<u8>, VmError> + Send + 'static,
    {
        Self {
            translation_service: AddressTranslationDomainService::new(arch, memory),
        }
    }

    /// GVA -> GPA 地址转换
    ///
    /// 委托给 AddressTranslationDomainService 执行地址转换。
    pub fn translate(&self, gva: GuestAddr, cr3: GuestAddr) -> Result<PageWalkResult, VmError> {
        self.translation_service.translate(gva, cr3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_table_flags() {
        let flags = PageTableFlags::default();
        let entry = flags.to_x86_64_entry(0x1000);

        assert_eq!(entry & 0x1, 0x1);
        assert_eq!(entry & 0x2, 0x2);
        assert_eq!(entry & !0xFFF, 0x1000);
    }

    #[test]
    fn test_page_sizes() {
        assert_eq!(PAGE_SIZE_4K, 4096);
        assert_eq!(PAGE_SIZE_2M, 2 * 1024 * 1024);
        assert_eq!(PAGE_SIZE_1G, 1024 * 1024 * 1024);
    }
}
