//! # AHCI Commands
//!
//! Command structures for AHCI operations.

/// AHCI command types
#[derive(Debug, Clone)]
pub enum AhciCommand {
    /// Read DMA (FIS type: H2D_REGISTER)
    ReadDma {
        /// Logical Block Address
        lba: u64,
        /// Sector count (1-65536)
        count: u16,
    },

    /// Write DMA (FIS type: H2D_REGISTER)
    WriteDma {
        /// Logical Block Address
        lba: u64,
        /// Sector count (1-65536)
        count: u16,
        /// Data to write
        data: Vec<u8>,
    },

    /// Identify device
    Identify,

    /// Flush cache
    FlushCache,
}

impl AhciCommand {
    /// Create read DMA command
    pub fn read_dma(lba: u64, count: u16) -> Self {
        Self::ReadDma { lba, count }
    }

    /// Create write DMA command
    pub fn write_dma(lba: u64, count: u16, data: Vec<u8>) -> Self {
        Self::WriteDma { lba, count, data }
    }

    /// Create identify command
    pub fn identify() -> Self {
        Self::Identify
    }

    /// Create flush cache command
    pub fn flush_cache() -> Self {
        Self::FlushCache
    }

    /// Check if command is read operation
    pub fn is_read(&self) -> bool {
        matches!(self, Self::ReadDma { .. })
    }

    /// Check if command is write operation
    pub fn is_write(&self) -> bool {
        matches!(self, Self::WriteDma { .. })
    }

    /// Get LBA if applicable
    pub fn lba(&self) -> Option<u64> {
        match self {
            Self::ReadDma { lba, .. } => Some(*lba),
            Self::WriteDma { lba, .. } => Some(*lba),
            _ => None,
        }
    }

    /// Get sector count if applicable
    pub fn count(&self) -> Option<u16> {
        match self {
            Self::ReadDma { count, .. } => Some(*count),
            Self::WriteDma { count, .. } => Some(*count),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_dma_command() {
        let cmd = AhciCommand::read_dma(0x1000, 10);
        assert!(cmd.is_read());
        assert!(!cmd.is_write());
        assert_eq!(cmd.lba(), Some(0x1000));
        assert_eq!(cmd.count(), Some(10));
    }

    #[test]
    fn test_write_dma_command() {
        let data = vec![0u8; 5120];
        let cmd = AhciCommand::write_dma(0x2000, 10, data);
        assert!(!cmd.is_read());
        assert!(cmd.is_write());
        assert_eq!(cmd.lba(), Some(0x2000));
        assert_eq!(cmd.count(), Some(10));
    }

    #[test]
    fn test_identify_command() {
        let cmd = AhciCommand::identify();
        assert!(!cmd.is_read());
        assert!(!cmd.is_write());
        assert_eq!(cmd.lba(), None);
        assert_eq!(cmd.count(), None);
    }

    #[test]
    fn test_flush_cache_command() {
        let cmd = AhciCommand::flush_cache();
        assert!(!cmd.is_read());
        assert!(!cmd.is_write());
        assert_eq!(cmd.lba(), None);
        assert_eq!(cmd.count(), None);
    }
}
