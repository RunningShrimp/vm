//! # SATA/AHCI Controller
//!
//! Minimal AHCI (Advanced Host Controller Interface) implementation
//! for supporting basic disk I/O operations in the Debian installer.
//!
//! # Architecture
//!
//! The AHCI controller provides:
//! - SATA port management (up to 32 ports)
//! - Command queue processing
//! - FIS (Frame Information Structure) handling
//! - DMA data transfer
//!
//! # Usage
//!
//! ```rust
//! use vm_device::ahci::AhciController;
//! use vm_device::block::RawBlockDevice;
//!
//! // Create controller
//! let mut controller = AhciController::new();
//!
//! // Attach disk to port 0
//! let disk = RawBlockDevice::new("/path/to/disk.img")?;
//! controller.attach_disk(0, Box::new(disk))?;
//!
//! // Read sectors
//! let data = controller.read(0, 0, 1)?;  // LBA 0, 1 sector
//!
//! // Write sectors
//! controller.write(0, 0, &data)?;
//! ```

pub mod commands;
pub mod port;
pub mod regs;

use log::{debug, error, info, warn};
use std::sync::{Arc, Mutex};
use thiserror::Error;

pub use commands::AhciCommand;
pub use port::AhciPort;
pub use regs::AhciRegs;

/// AHCI controller errors
#[derive(Error, Debug)]
pub enum AhciError {
    #[error("Port {0} not implemented")]
    PortNotImplemented(usize),

    #[error("Port {0} has no disk attached")]
    NoDiskAttached(usize),

    #[error("Invalid LBA: {0}")]
    InvalidLba(u64),

    #[error("Invalid sector count: {0}")]
    InvalidSectorCount(usize),

    #[error("Disk I/O error: {0}")]
    DiskIo(String),

    #[error("Command error: {0}")]
    CommandError(String),

    #[error("DMA error: {0}")]
    DmaError(String),

    #[error("FIS error: {0}")]
    FisError(String),
}

/// AHCI controller
///
/// Manages SATA ports and handles command processing.
pub struct AhciController {
    /// AHCI registers
    regs: AhciRegs,
    /// SATA ports (AHCI supports up to 32 ports)
    ports: Vec<Option<AhciPort>>,
    /// HBA memory (ABAR) base address
    abar_base: u64,
    /// Number of implemented ports
    ports_implemented: u32,
    /// Global interrupt status
    interrupt_status: u32,
}

impl AhciController {
    /// Create new AHCI controller
    ///
    /// # Example
    ///
    /// ```rust
    /// let controller = AhciController::new();
    /// ```
    pub fn new() -> Self {
        let ports_implemented = 0x1; // Port 0 only for minimal implementation

        Self {
            regs: AhciRegs::new(),
            ports: (0..32).map(|_| None).collect(),
            abar_base: 0xFFFFF000, // Typical ABAR location
            ports_implemented,
            interrupt_status: 0,
        }
    }

    /// Initialize AHCI controller
    ///
    /// Enables AHCI mode, resets the controller, and starts command processing.
    pub fn init(&mut self) -> Result<(), AhciError> {
        info!("Initializing AHCI controller");

        // Set HBA to AHCI mode
        self.regs.ghc_set_hba_enable();

        // Reset controller
        self.reset()?;

        // Enable interrupts
        self.regs.ghc_set_interrupt_enable();

        info!(
            "AHCI controller initialized: {} ports implemented",
            self.ports_implemented
        );

        Ok(())
    }

    /// Reset AHCI controller
    pub fn reset(&mut self) -> Result<(), AhciError> {
        debug!("Resetting AHCI controller");

        // Set HBA reset bit
        self.regs.ghc_set_hba_reset();

        // Wait for reset to complete (in real hardware, this would timeout)
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Clear reset bit
        self.regs.ghc_clear_hba_reset();

        // Re-enable AHCI mode
        self.regs.ghc_set_hba_enable();

        debug!("AHCI controller reset complete");

        Ok(())
    }

    /// Attach a disk device to a port
    ///
    /// # Arguments
    ///
    /// * `port_num` - Port number (0-31)
    /// * `disk` - Block device to attach
    ///
    /// # Example
    ///
    /// ```rust
    /// let disk = RawBlockDevice::new("/path/to/disk.img")?;
    /// controller.attach_disk(0, Box::new(disk))?;
    /// ```
    pub fn attach_disk(
        &mut self,
        port_num: usize,
        disk: Box<dyn crate::block_device::BlockDevice>,
    ) -> Result<(), AhciError> {
        if port_num >= 32 {
            return Err(AhciError::PortNotImplemented(port_num));
        }

        if (self.ports_implemented & (1 << port_num)) == 0 {
            return Err(AhciError::PortNotImplemented(port_num));
        }

        info!("Attaching disk to port {}", port_num);

        let mut port = AhciPort::new(port_num);
        port.attach_disk(disk);

        self.ports[port_num] = Some(port);

        info!("Disk attached to port {} successfully", port_num);

        Ok(())
    }

    /// Read sectors from disk
    ///
    /// # Arguments
    ///
    /// * `port_num` - Port number
    /// * `lba` - Logical Block Address (sector number)
    /// * `count` - Number of sectors to read
    ///
    /// # Returns
    ///
    /// Vector of bytes containing the sector data
    ///
    /// # Example
    ///
    /// ```rust
    /// // Read first sector (MBR)
    /// let mbr = controller.read(0, 0, 1)?;
    /// ```
    pub fn read(&mut self, port_num: usize, lba: u64, count: usize) -> Result<Vec<u8>, AhciError> {
        debug!(
            "Reading {} sectors from LBA {} on port {}",
            count, lba, port_num
        );

        // Get port
        let port = self.get_port_mut(port_num)?;

        // Issue read command
        let cmd = AhciCommand::read_dma(lba, count as u16);
        let result = port.process_command(cmd)?;

        Ok(result)
    }

    /// Write sectors to disk
    ///
    /// # Arguments
    ///
    /// * `port_num` - Port number
    /// * `lba` - Logical Block Address (sector number)
    /// * `data` - Data to write (must be multiple of 512 bytes)
    ///
    /// # Example
    ///
    /// ```rust
    /// // Write to first sector
    /// controller.write(0, 0, &my_mbr_data)?;
    /// ```
    pub fn write(&mut self, port_num: usize, lba: u64, data: &[u8]) -> Result<(), AhciError> {
        if data.len() % 512 != 0 {
            return Err(AhciError::InvalidSectorCount(data.len()));
        }

        let count = data.len() / 512;

        debug!(
            "Writing {} sectors to LBA {} on port {}",
            count, lba, port_num
        );

        // Get port
        let port = self.get_port_mut(port_num)?;

        // Issue write command
        let cmd = AhciCommand::write_dma(lba, count as u16, data.to_vec());
        port.process_command(cmd)?;

        Ok(())
    }

    /// Get mutable reference to port
    fn get_port_mut(&mut self, port_num: usize) -> Result<&mut AhciPort, AhciError> {
        if port_num >= 32 {
            return Err(AhciError::PortNotImplemented(port_num));
        }

        self.ports[port_num]
            .as_mut()
            .ok_or_else(|| AhciError::NoDiskAttached(port_num))
    }

    /// Get reference to port
    fn get_port(&self, port_num: usize) -> Result<&AhciPort, AhciError> {
        if port_num >= 32 {
            return Err(AhciError::PortNotImplemented(port_num));
        }

        self.ports[port_num]
            .as_ref()
            .ok_or_else(|| AhciError::NoDiskAttached(port_num))
    }

    /// Get disk size in sectors
    pub fn get_disk_size(&self, port_num: usize) -> Result<u64, AhciError> {
        let port = self.get_port(port_num)?;
        Ok(port.get_disk_size()?)
    }

    /// Check if port has disk attached
    pub fn has_disk(&self, port_num: usize) -> bool {
        if port_num >= 32 {
            return false;
        }

        self.ports[port_num]
            .as_ref()
            .map(|p| p.has_disk())
            .unwrap_or(false)
    }

    /// Get AHCI registers for MMIO
    pub fn regs(&self) -> &AhciRegs {
        &self.regs
    }

    /// Get mutable AHCI registers for MMIO
    pub fn regs_mut(&mut self) -> &mut AhciRegs {
        &mut self.regs
    }

    /// Get ABAR (AHCI Base Memory Register) address
    pub fn abar_base(&self) -> u64 {
        self.abar_base
    }

    /// Get number of implemented ports
    pub fn ports_implemented(&self) -> u32 {
        self.ports_implemented
    }

    /// Handle interrupt
    pub fn handle_interrupt(&mut self) {
        // Clear interrupt status
        self.interrupt_status = 0;

        // Check each port for pending interrupts
        for (i, port) in self.ports.iter_mut().enumerate() {
            if let Some(p) = port {
                if p.has_pending_interrupt() {
                    debug!("Clearing interrupt on port {}", i);
                    p.clear_interrupt();
                }
            }
        }
    }

    /// Get controller statistics
    pub fn get_stats(&self) -> AhciStats {
        let mut attached_disks = 0;
        for port in &self.ports {
            if port.as_ref().map(|p| p.has_disk()).unwrap_or(false) {
                attached_disks += 1;
            }
        }

        AhciStats {
            ports_implemented: self.ports_implemented,
            attached_disks,
            abar_base: self.abar_base,
            hba_enabled: self.regs.ghc_hba_enabled(),
            interrupt_enabled: self.regs.ghc_interrupt_enabled(),
        }
    }
}

impl Default for AhciController {
    fn default() -> Self {
        Self::new()
    }
}

/// AHCI controller statistics
#[derive(Debug, Clone)]
pub struct AhciStats {
    /// Number of implemented ports
    pub ports_implemented: u32,
    /// Number of attached disks
    pub attached_disks: u32,
    /// ABAR base address
    pub abar_base: u64,
    /// HBA enabled flag
    pub hba_enabled: bool,
    /// Interrupt enabled flag
    pub interrupt_enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ahci_controller_create() {
        let controller = AhciController::new();
        assert_eq!(controller.ports_implemented(), 0x1); // Port 0 only
        assert!(!controller.has_disk(0));
    }

    #[test]
    fn test_ahci_controller_init() {
        let mut controller = AhciController::new();
        assert!(controller.init().is_ok());
        assert!(controller.regs().ghc_hba_enabled());
    }

    #[test]
    fn test_ahci_controller_reset() {
        let mut controller = AhciController::new();
        controller.init().unwrap();
        assert!(controller.reset().is_ok());
        // HBA should remain enabled after reset
        assert!(controller.regs().ghc_hba_enabled());
    }

    #[test]
    fn test_ahci_port_not_implemented() {
        let controller = AhciController::new();
        assert!(!controller.has_disk(32)); // Port 32 doesn't exist
    }
}
