//! # AHCI Port Implementation
//!
//! Individual SATA port implementation with command processing.

use super::{AhciError, commands::AhciCommand};
use crate::block_device::BlockDevice;
use log::{debug, info};
use std::sync::{Arc, Mutex};

/// SATA port signature values
pub const SATA_SIG_SATA: u32 = 0x00000101; // SATA disk
pub const SATA_SIG_ATAPI: u32 = 0xEB140101; // SATAPI device
pub const SATA_SIG_SEMB: u32 = 0xC33C0101; // Enclosure management bridge
pub const SATA_SIG_PM: u32 = 0x96690101; // Port multiplier

/// AHCI port
///
/// Represents a single SATA port with attached device.
pub struct AhciPort {
    /// Port number (0-31)
    port_num: usize,
    /// Attached block device
    disk: Option<Box<dyn BlockDevice>>,
    /// Command List Base Address
    clb: u64,
    /// FIS Base Address
    fb: u64,
    /// Command register
    cmd: u32,
    /// SATA Status (SCR0: SStatus)
    ssts: u32,
    /// SATA Error (SCR1: SError)
    serr: u32,
    /// Command Issue
    ci: u32,
    /// Interrupt Status
    is: u32,
    /// Interrupt Enable
    ie: u32,
    /// Device signature
    sig: u32,
}

impl AhciPort {
    /// Create new AHCI port
    pub fn new(port_num: usize) -> Self {
        Self {
            port_num,
            disk: None,
            clb: 0,
            fb: 0,
            cmd: 0,      // Port start: 0, spin-up: 0, power-on: 0
            ssts: 0x113, // Device present, power on, PHY communication established
            serr: 0,
            ci: 0,
            is: 0,
            ie: 0,
            sig: SATA_SIG_SATA, // Default to SATA disk
        }
    }

    /// Attach disk device to port
    pub fn attach_disk(&mut self, disk: Box<dyn BlockDevice>) {
        info!("Attaching disk to port {}", self.port_num);
        self.disk = Some(disk);
        self.sig = SATA_SIG_SATA;
        self.ssts = 0x133; // Device detected and communication established
    }

    /// Check if port has disk attached
    pub fn has_disk(&self) -> bool {
        self.disk.is_some()
    }

    /// Process AHCI command
    pub fn process_command(&mut self, cmd: AhciCommand) -> Result<Vec<u8>, AhciError> {
        debug!("Processing command on port {}: {:?}", self.port_num, cmd);

        // Check if disk is attached
        let disk = self
            .disk
            .as_ref()
            .ok_or_else(|| AhciError::NoDiskAttached(self.port_num))?;

        // Execute command based on type
        match cmd {
            AhciCommand::ReadDma { lba, count } => self.read_dma(disk, lba, count),
            AhciCommand::WriteDma { lba, count, data } => self.write_dma(disk, lba, count, data),
            AhciCommand::Identify => self.identify(disk),
            AhciCommand::FlushCache => self.flush_cache(disk),
        }
    }

    /// Read DMA command
    fn read_dma(
        &self,
        disk: &Box<dyn BlockDevice>,
        lba: u64,
        count: u16,
    ) -> Result<Vec<u8>, AhciError> {
        debug!("Read DMA: LBA={}, count={}", lba, count);

        let sector_size = 512;
        let total_bytes = (count as usize) * sector_size;

        // Read sectors from disk
        let mut buffer = vec![0u8; total_bytes];
        disk.read(lba, &mut buffer)
            .map_err(|e| AhciError::DiskIo(e.to_string()))?;

        Ok(buffer)
    }

    /// Write DMA command
    fn write_dma(
        &self,
        disk: &Box<dyn BlockDevice>,
        lba: u64,
        count: u16,
        data: Vec<u8>,
    ) -> Result<Vec<u8>, AhciError> {
        debug!(
            "Write DMA: LBA={}, count={}, bytes={}",
            lba,
            count,
            data.len()
        );

        // Write sectors to disk
        disk.write(lba, &data)
            .map_err(|e| AhciError::DiskIo(e.to_string()))?;

        // Return empty result for write
        Ok(Vec::new())
    }

    /// Identify device command
    fn identify(&self, disk: &Box<dyn BlockDevice>) -> Result<Vec<u8>, AhciError> {
        debug!("Identify device on port {}", self.port_num);

        // Get disk size
        let sectors = disk.sectors();

        // Create IDENTIFY DEVICE response (512 bytes)
        let mut identify_data = vec![0u8; 512];

        // Word 0: General configuration
        identify_data[0] = 0x00; // Non-removable, SATA device
        identify_data[1] = 0x00;

        // Word 1: Cylinders (default to 16383 for LBA)
        identify_data[2] = 0x7F; // 16383 = 0x3FFF
        identify_data[3] = 0x3F;

        // Word 3: Heads (default to 16)
        identify_data[6] = 0x10;
        identify_data[7] = 0x00;

        // Word 6: Sectors per track (default to 63)
        identify_data[12] = 0x3F;
        identify_data[13] = 0x00;

        // Word 60-61: Total addressable sectors (LBA48)
        let lba28 = std::cmp::min(sectors, 0x0FFFFFFF) as u32;
        identify_data[120] = (lba28 & 0xFF) as u8;
        identify_data[121] = ((lba28 >> 8) & 0xFF) as u8;
        identify_data[122] = ((lba28 >> 16) & 0xFF) as u8;
        identify_data[123] = ((lba28 >> 24) & 0xFF) as u8;

        // Word 100-103: Maximum LBA (LBA48)
        let lba48 = sectors;
        identify_data[200] = (lba48 & 0xFF) as u8;
        identify_data[201] = ((lba48 >> 8) & 0xFF) as u8;
        identify_data[202] = ((lba48 >> 16) & 0xFF) as u8;
        identify_data[203] = ((lba48 >> 24) & 0xFF) as u8;
        identify_data[204] = ((lba48 >> 32) & 0xFF) as u8;
        identify_data[205] = ((lba48 >> 40) & 0xFF) as u8;
        identify_data[206] = ((lba48 >> 48) & 0xFF) as u8;
        identify_data[207] = ((lba48 >> 56) & 0xFF) as u8;

        // Word 83: Command set supported
        identify_data[166] = 0x7F; // LBA48 supported
        identify_data[167] = 0x00;

        // Word 169: Data set management (TRIM support)
        identify_data[338] = 0x00; // No TRIM for now

        Ok(identify_data)
    }

    /// Flush cache command
    fn flush_cache(&self, disk: &Box<dyn BlockDevice>) -> Result<Vec<u8>, AhciError> {
        debug!("Flush cache on port {}", self.port_num);

        disk.flush().map_err(|e| AhciError::DiskIo(e.to_string()))?;

        Ok(Vec::new())
    }

    /// Get disk size in sectors
    pub fn get_disk_size(&self) -> Result<u64, AhciError> {
        let disk = self
            .disk
            .as_ref()
            .ok_or_else(|| AhciError::NoDiskAttached(self.port_num))?;

        Ok(disk.sectors())
    }

    /// Check if port has pending interrupt
    pub fn has_pending_interrupt(&self) -> bool {
        self.is != 0
    }

    /// Clear interrupt
    pub fn clear_interrupt(&mut self) {
        self.is = 0;
    }

    /// Get port registers for MMIO
    pub fn get_cmd(&self) -> u32 {
        self.cmd
    }

    /// Get SATA status
    pub fn get_ssts(&self) -> u32 {
        self.ssts
    }

    /// Get device signature
    pub fn get_sig(&self) -> u32 {
        self.sig
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::test::MockBlockDevice;

    #[test]
    fn test_ahci_port_create() {
        let port = AhciPort::new(0);
        assert_eq!(port.port_num, 0);
        assert!(!port.has_disk());
        assert_eq!(port.get_ssts(), 0x113);
    }

    #[test]
    fn test_ahci_port_attach_disk() {
        let mut port = AhciPort::new(0);
        let disk = Box::new(MockBlockDevice::new(1024));
        port.attach_disk(disk);
        assert!(port.has_disk());
        assert_eq!(port.get_sig(), SATA_SIG_SATA);
        assert_eq!(port.get_ssts(), 0x133);
    }

    #[test]
    fn test_ahci_port_no_disk() {
        let port = AhciPort::new(0);
        let cmd = AhciCommand::ReadDma { lba: 0, count: 1 };
        assert!(port.process_command(cmd).is_err());
    }
}
