// ARM SMMUv3 Implementation
//
// 本模块实现了完整的ARM SMMUv3功能，包括：
// - 地址转换单元（ATSU）
// - 多级页表遍历
// - TLB缓存管理
// - 中断和MSI管理
// - 命令队列处理

pub mod atsu;
pub mod error;
pub mod interrupt;
pub mod mmu;
pub mod tlb;

// 重新导出主要类型
pub use atsu::{AddressTranslator, TranslationResult, TranslationStage};
pub use error::{SmmuError, SmmuResult};
pub use interrupt::{
    InterruptController, InterruptController as InterruptManager, InterruptType, MsiMessage,
};
pub use mmu::{SmmuConfig, SmmuDevice, SmmuStats};
pub use tlb::{TlbCache, TlbEntry, TlbPolicy};

/// ARM SMMUv3 库版本
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// ARM SMMUv3 库描述
pub const DESCRIPTION: &str = "ARM SMMUv3 Implementation for Virtual Machine DMA Virtualization";

/// SMMUv3 基本常量
pub const STREAM_ID_MAX: u16 = 65535;
pub const TLB_ENTRY_MAX: usize = 256;
pub const PAGE_SIZE_4KB: u64 = 4096;
pub const PAGE_SIZE_16KB: u64 = 16384;
pub const PAGE_SIZE_64KB: u64 = 65536;

/// 访问权限
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessPermission {
    Read = 1 << 0,
    Write = 1 << 1,
    Execute = 1 << 2,
    ReadWrite = 1 << 0 | 1 << 1,                 // 组合权限
    ReadWriteExecute = 1 << 0 | 1 << 1 | 1 << 2, // 完全权限
}

impl AccessPermission {
    /// 组合多个访问权限
    pub fn combine(self, other: AccessPermission) -> u32 {
        self as u32 | other as u32
    }
}

/// 访问类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessType {
    Read,
    Write,
    Execute,
    Atomic,
}

/// 页大小
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageSize {
    Size4KB = 12,
    Size16KB = 14,
    Size64KB = 16,
}

impl PageSize {
    /// 获取页大小对应的位偏移
    pub fn shift(&self) -> u8 {
        *self as u8
    }

    /// 获取页大小
    pub fn size(&self) -> u64 {
        1u64 << self.shift()
    }
}

/// SMMUv3 版本信息
#[derive(Debug, Clone)]
pub struct SmmuVersion {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
}

impl SmmuVersion {
    pub fn new(major: u8, minor: u8, patch: u8) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    pub fn v3_2() -> Self {
        Self::new(3, 2, 0)
    }
}

impl std::fmt::Display for SmmuVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
        println!("SMMUv3 version: {}", VERSION);
    }

    #[test]
    fn test_page_size() {
        assert_eq!(PageSize::Size4KB.size(), 4096);
        assert_eq!(PageSize::Size16KB.size(), 16384);
        assert_eq!(PageSize::Size64KB.size(), 65536);
    }

    #[test]
    fn test_smmu_version() {
        let v = SmmuVersion::v3_2();
        assert_eq!(v.major, 3);
        assert_eq!(v.minor, 2);
        assert_eq!(v.to_string(), "3.2.0");
    }

    #[test]
    fn test_access_permission() {
        let perms = AccessPermission::Read as u8;
        assert_eq!(perms, 1);
    }

    #[test]
    fn test_constants() {
        assert_eq!(STREAM_ID_MAX, 65535);
        assert_eq!(TLB_ENTRY_MAX, 256);
        assert_eq!(PAGE_SIZE_4KB, 4096);
    }
}
