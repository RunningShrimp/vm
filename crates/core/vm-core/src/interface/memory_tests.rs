// Unit tests for memory management
//
// Tests for memory interfaces and MMU implementations

use super::*;

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================
    // GuestAddr Tests
    // ============================================================

    #[test]
    fn test_guestaddr_creation() {
        let addr = GuestAddr(0x1000);
        assert_eq!(addr.0, 0x1000);
    }

    #[test]
    fn test_guestaddr_arithmetic() {
        let addr1 = GuestAddr(0x1000);
        let addr2 = GuestAddr(addr1.0 + 0x100);
        assert_eq!(addr2.0, 0x1100);

        let addr3 = GuestAddr(addr2.0 - 0x50);
        assert_eq!(addr3.0, 0x10B0);
    }

    #[test]
    fn test_guestaddr_alignment() {
        let addr = GuestAddr(0x1003);

        // Test alignment check
        let aligned = GuestAddr(0x1000);
        assert_eq!(aligned.0 % 4, 0);
        assert_ne!(addr.0 % 4, 0);
    }

    #[test]
    fn test_guestaddr_page_offset() {
        let addr = GuestAddr(0x1234);
        let page_offset = addr.0 % 4096;
        assert_eq!(page_offset, 0x1234);
    }
}
