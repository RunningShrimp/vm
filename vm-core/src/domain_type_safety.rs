//! Domain Type Safety Enhancements
//!
//! Provides additional type safety for domain types with validation and conversion methods.

use crate::{CoreError, GuestAddr, GuestPhysAddr, MemoryError, VmError, VmResult};

/// Extension trait for GuestAddr to add type-safe operations
pub trait GuestAddrExt {
    fn checked_add(self, offset: u64) -> VmResult<GuestAddr>;
    fn checked_sub(self, offset: u64) -> VmResult<GuestAddr>;
    fn is_aligned(self, alignment: u64) -> bool;
    fn align_down(self, alignment: u64) -> GuestAddr;
    fn align_up(self, alignment: u64) -> VmResult<GuestAddr>;
    fn page_index(self, page_size: u64) -> u64;
    fn page_offset(self, page_size: u64) -> u64;
}

impl GuestAddrExt for GuestAddr {
    fn checked_add(self, offset: u64) -> VmResult<GuestAddr> {
        self.0.checked_add(offset)
            .map(GuestAddr)
            .ok_or(VmError::Memory(MemoryError::InvalidAddress(self)))
    }

    fn checked_sub(self, offset: u64) -> VmResult<GuestAddr> {
        self.0.checked_sub(offset)
            .map(GuestAddr)
            .ok_or(VmError::Memory(MemoryError::InvalidAddress(self)))
    }

    fn is_aligned(self, alignment: u64) -> bool {
        alignment.is_power_of_two() && self.0 % alignment == 0
    }

    fn align_down(self, alignment: u64) -> GuestAddr {
        GuestAddr(self.0 & !(alignment - 1))
    }

    fn align_up(self, alignment: u64) -> VmResult<GuestAddr> {
        if !alignment.is_power_of_two() {
            return Err(VmError::Memory(MemoryError::AlignmentError {
                addr: self,
                required: (alignment as u8).ilog2() as u64,
                size: 64,
            }));
        }
        let aligned = (self.0 + alignment - 1) & !(alignment - 1);
        if aligned < self.0 {
            return Err(VmError::Memory(MemoryError::InvalidAddress(GuestAddr(aligned))));
        }
        Ok(GuestAddr(aligned))
    }

    fn page_index(self, page_size: u64) -> u64 {
        self.0 / page_size
    }

    fn page_offset(self, page_size: u64) -> u64 {
        self.0 % page_size
    }
}

/// Extension trait for GuestPhysAddr to add type-safe operations
pub trait GuestPhysAddrExt {
    fn is_aligned(self, alignment: u64) -> bool;
    fn align_down(self, alignment: u64) -> GuestPhysAddr;
    fn align_up(self, alignment: u64) -> VmResult<GuestPhysAddr>;
    fn page_index(self, page_size: u64) -> u64;
    fn page_offset(self, page_size: u64) -> u64;
}

impl GuestPhysAddrExt for GuestPhysAddr {
    fn is_aligned(self, alignment: u64) -> bool {
        alignment.is_power_of_two() && self.0 % alignment == 0
    }

    fn align_down(self, alignment: u64) -> GuestPhysAddr {
        GuestPhysAddr(self.0 & !(alignment - 1))
    }

    fn align_up(self, alignment: u64) -> VmResult<GuestPhysAddr> {
        if !alignment.is_power_of_two() {
            return Err(VmError::Memory(MemoryError::AllocationFailed {
                message: "Invalid alignment".to_string(),
                size: None,
            }));
        }
        let aligned = (self.0 + alignment - 1) & !(alignment - 1);
        if aligned < self.0 {
            return Err(VmError::Memory(MemoryError::AllocationFailed {
                message: "Address overflow".to_string(),
                size: None,
            }));
        }
        Ok(GuestPhysAddr(aligned))
    }

    fn page_index(self, page_size: u64) -> u64 {
        self.0 / page_size
    }

    fn page_offset(self, page_size: u64) -> u64 {
        self.0 % page_size
    }
}

/// Validated page size
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageSize(u64);

impl PageSize {
    pub const KB4: PageSize = PageSize(0x1000);
    pub const KB64: PageSize = PageSize(0x10000);
    pub const MB2: PageSize = PageSize(0x200000);
    pub const MB4: PageSize = PageSize(0x400000);

    pub fn new(size: u64) -> VmResult<Self> {
        if !size.is_power_of_two() {
            return Err(VmError::Core(CoreError::InvalidConfig {
                message: "Page size must be power of two".to_string(),
                field: "size".to_string(),
            }));
        }
        if size < 0x1000 {
            return Err(VmError::Core(CoreError::InvalidConfig {
                message: "Page size too small".to_string(),
                field: "size".to_string(),
            }));
        }
        Ok(PageSize(size))
    }

    pub fn as_u64(self) -> u64 {
        self.0
    }

    pub fn shift(self) -> u64 {
        self.0.trailing_zeros() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guest_addr_checked_add() {
        let addr = GuestAddr(0x1000);
        assert_eq!(addr.checked_add(0x100).unwrap(), GuestAddr(0x1100));
    }

    #[test]
    fn test_guest_addr_checked_add_overflow() {
        let addr = GuestAddr(u64::MAX);
        assert!(addr.checked_add(1).is_err());
    }

    #[test]
    fn test_guest_addr_is_aligned() {
        let addr = GuestAddr(0x1000);
        assert!(addr.is_aligned(0x1000));
        assert!(!addr.is_aligned(0x2000));
    }

    #[test]
    fn test_guest_addr_align_down() {
        let addr = GuestAddr(0x1A00);
        assert_eq!(addr.align_down(0x1000), GuestAddr(0x1000));
    }

    #[test]
    fn test_guest_addr_align_up() {
        let addr = GuestAddr(0x1A00);
        assert_eq!(addr.align_up(0x1000).unwrap(), GuestAddr(0x2000));
    }

    #[test]
    fn test_guest_addr_page_index() {
        let addr = GuestAddr(0x5000);
        assert_eq!(addr.page_index(0x1000), 5);
    }

    #[test]
    fn test_guest_addr_page_offset() {
        let addr = GuestAddr(0x5A00);
        assert_eq!(addr.page_offset(0x1000), 0xA00);
    }

    #[test]
    fn test_page_size_valid() {
        assert!(PageSize::new(0x1000).is_ok());
        assert!(PageSize::new(0x2000).is_ok());
    }

    #[test]
    fn test_page_size_invalid() {
        assert!(PageSize::new(0x100).is_err());
        assert!(PageSize::new(0x1001).is_err());
    }

    #[test]
    fn test_page_size_constants() {
        assert_eq!(PageSize::KB4.as_u64(), 0x1000);
        assert_eq!(PageSize::KB64.as_u64(), 0x10000);
        assert_eq!(PageSize::MB2.as_u64(), 0x200000);
        assert_eq!(PageSize::MB4.as_u64(), 0x400000);
    }
}
