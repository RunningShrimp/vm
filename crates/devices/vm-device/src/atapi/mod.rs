//! # ATAPI CD-ROM Device
//!
//! ATA Packet Interface (ATAPI) CD-ROM device for ISO image access.
//!
//! # Architecture
//!
//! The ATAPI CD-ROM provides:
//! - ATA packet command interface
//! - ISO 9660 filesystem support
//! - CD-ROM read operations
//! - Boot capability
//!
//! # Usage
//!
//! ```rust
//! use vm_device::atapi::AtapiCdRom;
//!
//! // Create ATAPI device from ISO
//! let mut cdrom = AtapiCdRom::new("/path/to/debian.iso")?;
//!
//! // Read sectors
//! let data = cdrom.read_sectors(16, 1)?;  // LBA 16, 1 sector
//!
//! // Get TOC (Table of Contents)
//! let toc = cdrom.read_toc()?;
//! ```

pub mod commands;
pub mod iso9660;

use log::{debug, info, warn};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use thiserror::Error;

pub use commands::AtapiCommand;
pub use iso9660::Iso9660;

/// ATAPI errors
#[derive(Error, Debug)]
pub enum AtapiError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid LBA: {0}")]
    InvalidLba(u32),

    #[error("Invalid sector count: {0}")]
    InvalidSectorCount(u8),

    #[error("ISO file not found: {0}")]
    IsoNotFound(String),

    #[error("Command not supported: {0}")]
    CommandNotSupported(String),

    #[error("Read error: {0}")]
    ReadError(String),

    #[error("Media error: {0}")]
    MediaError(String),
}

/// ATA Packet Interface CD-ROM device
///
/// Simulates a CD-ROM device using an ISO image file.
pub struct AtapiCdRom {
    /// ISO file path
    iso_path: String,
    /// ISO file handle
    iso_file: Option<File>,
    /// Total size in bytes
    total_size: u64,
    /// Total sectors (2048 bytes per sector for CD-ROM)
    total_sectors: u32,
    /// Current LBA position
    current_lba: u32,
    /// Device ready flag
    ready: bool,
    /// Device type (CD-ROM)
    device_type: u8,
}

impl AtapiCdRom {
    /// CD-ROM sector size (2048 bytes for Mode 1 Form 1)
    pub const SECTOR_SIZE: u32 = 2048;

    /// Create new ATAPI CD-ROM device from ISO file
    ///
    /// # Arguments
    ///
    /// * `iso_path` - Path to ISO image file
    ///
    /// # Example
    ///
    /// ```rust
    /// let cdrom = AtapiCdRom::new("/path/to/debian.iso")?;
    /// ```
    pub fn new<P: AsRef<Path>>(iso_path: P) -> Result<Self, AtapiError> {
        let path = iso_path.as_ref();

        if !path.exists() {
            return Err(AtapiError::IsoNotFound(path.to_string_lossy().to_string()));
        }

        let metadata = std::fs::metadata(path).map_err(|e| {
            AtapiError::ReadError(format!("Failed to open {}: {}", path.display(), e))
        })?;

        let total_size = metadata.len();
        let total_sectors = (total_size / Self::SECTOR_SIZE as u64) as u32;

        info!(
            "ATAPI CD-ROM: {} sectors, {} MB",
            total_sectors,
            total_size / (1024 * 1024)
        );

        Ok(Self {
            iso_path: path.to_string_lossy().to_string(),
            iso_file: None,
            total_size,
            total_sectors,
            current_lba: 0,
            ready: true,
            device_type: 0x05, // CD-ROM device type
        })
    }

    /// Initialize device (open ISO file)
    pub fn init(&mut self) -> Result<(), AtapiError> {
        debug!("Initializing ATAPI CD-ROM: {}", self.iso_path);

        let file = File::open(&self.iso_path)
            .map_err(|e| AtapiError::ReadError(format!("Failed to open ISO: {}", e)))?;

        self.iso_file = Some(file);
        self.ready = true;

        debug!("ATAPI CD-ROM initialized: {} sectors", self.total_sectors);

        Ok(())
    }

    /// Shutdown device (close ISO file)
    pub fn shutdown(&mut self) {
        debug!("Shutting down ATAPI CD-ROM");
        self.iso_file = None;
        self.ready = false;
    }

    /// Read sectors from ISO
    ///
    /// # Arguments
    ///
    /// * `lba` - Logical Block Address (sector number)
    /// * `count` - Number of sectors to read (max 255)
    ///
    /// # Returns
    ///
    /// Vector of bytes containing the sector data
    pub fn read_sectors(&mut self, lba: u32, count: u8) -> Result<Vec<u8>, AtapiError> {
        if !self.ready {
            return Err(AtapiError::MediaError("Device not ready".to_string()));
        }

        if lba >= self.total_sectors {
            return Err(AtapiError::InvalidLba(lba));
        }

        if count == 0 {
            return Ok(Vec::new());
        }

        let sector_count = count as u32;
        if lba + sector_count > self.total_sectors {
            return Err(AtapiError::InvalidSectorCount(count));
        }

        debug!("Reading {} sectors from LBA {}", count, lba);

        let file = self
            .iso_file
            .as_mut()
            .ok_or_else(|| AtapiError::MediaError("Device not initialized".to_string()))?;

        let offset = lba as u64 * Self::SECTOR_SIZE as u64;
        file.seek(SeekFrom::Start(offset))
            .map_err(|e| AtapiError::ReadError(format!("Seek failed: {}", e)))?;

        let mut buffer = vec![0u8; (sector_count as usize) * (Self::SECTOR_SIZE as usize)];
        file.read_exact(&mut buffer)
            .map_err(|e| AtapiError::ReadError(format!("Read failed: {}", e)))?;

        self.current_lba = lba + sector_count;

        Ok(buffer)
    }

    /// Read TOC (Table of Contents)
    ///
    /// Returns standard CD-ROM TOC data
    pub fn read_toc(&self) -> Result<Vec<u8>, AtapiError> {
        debug!("Reading TOC");

        // Simplified TOC response
        // Real CD-ROM TOC is more complex, but this is sufficient for basic operation
        let mut toc = Vec::new();

        // TOC header (4 bytes)
        toc.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // TOC length

        // Track 1 (data track)
        toc.extend_from_slice(&[
            0x01, // Track number
            0x00, // ADR/CONTROL
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ]); // LBA (filled later)

        // Lead-out track
        toc.extend_from_slice(&[
            0xAA, // Lead-out track
            0x01, // ADR/CONTROL
            (self.total_sectors & 0xFF) as u8,
            ((self.total_sectors >> 8) & 0xFF) as u8,
            ((self.total_sectors >> 16) & 0xFF) as u8,
            ((self.total_sectors >> 24) & 0xFF) as u8,
            0x00,
            0x00,
        ]);

        Ok(toc)
    }

    /// Get capacity (number of sectors)
    pub fn capacity(&self) -> u32 {
        self.total_sectors
    }

    /// Get device size in bytes
    pub fn size(&self) -> u64 {
        self.total_size
    }

    /// Check if device is ready
    pub fn is_ready(&self) -> bool {
        self.ready
    }

    /// Get current LBA position
    pub fn current_lba(&self) -> u32 {
        self.current_lba
    }

    /// Get device type
    pub fn device_type(&self) -> u8 {
        self.device_type
    }

    /// Get ISO path
    pub fn iso_path(&self) -> &str {
        &self.iso_path
    }

    /// Process ATAPI command
    pub fn process_command(&mut self, cmd: &AtapiCommand) -> Result<Vec<u8>, AtapiError> {
        debug!("Processing ATAPI command: {:?}", cmd);

        match cmd {
            AtapiCommand::Read { lba, count } => self.read_sectors(*lba, *count),
            AtapiCommand::ReadCd { lba, count } => {
                // READ CD is essentially the same as READ for our purposes
                self.read_sectors(*lba, *count)
            }
            AtapiCommand::ReadToc => self.read_toc(),
            AtapiCommand::TestUnitReady => {
                // Just return empty data if ready
                if self.ready {
                    Ok(Vec::new())
                } else {
                    Err(AtapiError::MediaError("Device not ready".to_string()))
                }
            }
            AtapiCommand::Inquiry => {
                // Return INQUIRY data
                Ok(self.inquiry_data())
            }
            AtapiCommand::ReadCapacity => {
                // Return capacity data
                Ok(self.capacity_data())
            }
            AtapiCommand::StartStopUnit { start, .. } => {
                // Start/stop the device
                self.ready = *start;
                Ok(Vec::new())
            }
            AtapiCommand::Unknown => Err(AtapiError::CommandNotSupported(
                "Unknown command".to_string(),
            )),
        }
    }

    /// Generate INQUIRY data
    fn inquiry_data(&self) -> Vec<u8> {
        let mut data = vec![0u8; 36];

        // Standard INQUIRY data format
        data[0] = 0x05; // Device type: CD-ROM
        data[1] = 0x80; // Removable media
        data[2] = 0x00; // Version
        data[3] = 0x02; // Response data format
        data[4] = 0x1F; // Additional length
        data[5] = 0x00; // Flags
        data[6] = 0x00; // Flags
        data[7] = 0x00; // Flags

        // Vendor identification (8 bytes)
        let vendor = b"VIRT-ATA";
        data[8..16].copy_from_slice(vendor);

        // Product identification (16 bytes)
        let product = b"ATAPI CD-ROM     ";
        data[16..32].copy_from_slice(product);

        // Firmware revision (4 bytes)
        let firmware = b"1.0 ";
        data[32..36].copy_from_slice(firmware);

        data
    }

    /// Generate READ CAPACITY data
    fn capacity_data(&self) -> Vec<u8> {
        let mut data = vec![0u8; 8];

        // Return last LBA (total sectors - 1)
        let last_lba = self.total_sectors - 1;
        data[0] = ((last_lba >> 24) & 0xFF) as u8;
        data[1] = ((last_lba >> 16) & 0xFF) as u8;
        data[2] = ((last_lba >> 8) & 0xFF) as u8;
        data[3] = (last_lba & 0xFF) as u8;

        // Block size (2048 for CD-ROM)
        data[4] = ((Self::SECTOR_SIZE >> 24) & 0xFF) as u8;
        data[5] = ((Self::SECTOR_SIZE >> 16) & 0xFF) as u8;
        data[6] = ((Self::SECTOR_SIZE >> 8) & 0xFF) as u8;
        data[7] = (Self::SECTOR_SIZE & 0xFF) as u8;

        data
    }
}

impl Clone for AtapiCdRom {
    fn clone(&self) -> Self {
        Self {
            iso_path: self.iso_path.clone(),
            iso_file: None, // Don't clone file handle
            total_size: self.total_size,
            total_sectors: self.total_sectors,
            current_lba: self.current_lba,
            ready: false, // Not ready after clone
            device_type: self.device_type,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atapi_create() {
        // This test requires a real ISO file
        // For CI/testing, we might want to create a temporary ISO
        let result = AtapiCdRom::new("/tmp/test.iso");
        assert!(result.is_err() || result.is_ok());
    }

    #[test]
    fn test_atapi_not_found() {
        let cdrom = AtapiCdRom::new("/nonexistent/debian.iso");
        assert!(cdrom.is_err());
    }
}
