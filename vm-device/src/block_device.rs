//! # Generic Block Device Trait
//!
//! Common interface for all block devices (SATA, NVMe, VirtIO, etc.)

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use thiserror::Error;

/// Block device errors
#[derive(Error, Debug)]
pub enum BlockDeviceError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid LBA: {0}")]
    InvalidLba(u64),

    #[error("Invalid sector count: {0}")]
    InvalidSectorCount(usize),

    #[error("Device not ready")]
    NotReady,

    #[error("Write protected")]
    WriteProtected,

    #[error("Device error: {0}")]
    DeviceError(String),
}

/// Generic block device trait
///
/// All disk-like devices must implement this trait.
pub trait BlockDevice: Send + Sync {
    /// Read sectors from device
    ///
    /// # Arguments
    ///
    /// * `lba` - Logical Block Address (sector number)
    /// * `buffer` - Buffer to store read data (must be multiple of 512 bytes)
    fn read(&self, lba: u64, buffer: &mut [u8]) -> Result<(), BlockDeviceError>;

    /// Write sectors to device
    ///
    /// # Arguments
    ///
    /// * `lba` - Logical Block Address (sector number)
    /// * `data` - Data to write (must be multiple of 512 bytes)
    fn write(&self, lba: u64, data: &[u8]) -> Result<(), BlockDeviceError>;

    /// Flush device cache
    fn flush(&self) -> Result<(), BlockDeviceError>;

    /// Get device size in sectors
    fn sectors(&self) -> u64;

    /// Get sector size (always 512 for ATA)
    fn sector_size(&self) -> usize {
        512
    }

    /// Get device size in bytes
    fn size(&self) -> u64 {
        self.sectors() * self.sector_size() as u64
    }
}

/// Raw block device implementation (file-based)
///
/// Basic block device that reads/writes to a disk image file.
pub struct RawBlockDevice {
    file_path: String,
    sectors: u64,
}

impl RawBlockDevice {
    /// Create new raw block device from file
    ///
    /// # Arguments
    ///
    /// * `path` - Path to disk image file
    ///
    /// # Example
    ///
    /// ```rust
    /// let device = RawBlockDevice::new("/path/to/disk.img")?;
    /// ```
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, BlockDeviceError> {
        let path = path.as_ref();
        let metadata = std::fs::metadata(path).map_err(|e| {
            BlockDeviceError::DeviceError(format!("Failed to open {}: {}", path.display(), e))
        })?;

        let file_size = metadata.len();
        let sectors = file_size / 512;

        Ok(Self {
            file_path: path.to_string_lossy().to_string(),
            sectors,
        })
    }

    /// Create new raw block device with specified size
    ///
    /// # Arguments
    ///
    /// * `path` - Path to disk image file
    /// * `size_gb` - Size in GB
    pub fn with_size<P: AsRef<Path>>(path: P, size_gb: u64) -> Result<Self, BlockDeviceError> {
        let path = path.as_ref();
        let size_bytes = size_gb * 1024 * 1024 * 1024;
        let sectors = size_bytes / 512;

        // Create file if it doesn't exist
        if !path.exists() {
            let file = File::create(path).map_err(|e| {
                BlockDeviceError::DeviceError(format!("Failed to create {}: {}", path.display(), e))
            })?;
            file.set_len(size_bytes)
                .map_err(|e| BlockDeviceError::DeviceError(format!("Failed to set size: {}", e)))?;
        }

        Ok(Self {
            file_path: path.to_string_lossy().to_string(),
            sectors,
        })
    }
}

impl BlockDevice for RawBlockDevice {
    fn read(&self, lba: u64, buffer: &mut [u8]) -> Result<(), BlockDeviceError> {
        if lba >= self.sectors {
            return Err(BlockDeviceError::InvalidLba(lba));
        }

        if buffer.len() % 512 != 0 {
            return Err(BlockDeviceError::InvalidSectorCount(buffer.len()));
        }

        let sector_count = buffer.len() / 512;
        if lba + sector_count as u64 > self.sectors {
            return Err(BlockDeviceError::InvalidSectorCount(sector_count));
        }

        let mut file = File::open(&self.file_path)?;
        file.seek(SeekFrom::Start(lba * 512))?;
        file.read_exact(buffer)?;

        Ok(())
    }

    fn write(&self, lba: u64, data: &[u8]) -> Result<(), BlockDeviceError> {
        if lba >= self.sectors {
            return Err(BlockDeviceError::InvalidLba(lba));
        }

        if data.len() % 512 != 0 {
            return Err(BlockDeviceError::InvalidSectorCount(data.len()));
        }

        let sector_count = data.len() / 512;
        if lba + sector_count as u64 > self.sectors {
            return Err(BlockDeviceError::InvalidSectorCount(sector_count));
        }

        let mut file = File::options().write(true).open(&self.file_path)?;
        file.seek(SeekFrom::Start(lba * 512))?;
        file.write_all(data)?;
        file.flush()?;

        Ok(())
    }

    fn flush(&self) -> Result<(), BlockDeviceError> {
        // No-op for file-based device
        Ok(())
    }

    fn sectors(&self) -> u64 {
        self.sectors
    }
}

#[cfg(test)]
pub mod test {
    use super::*;

    /// Mock block device for testing
    pub struct MockBlockDevice {
        data: Vec<u8>,
        sectors: u64,
    }

    impl MockBlockDevice {
        pub fn new(sectors: u64) -> Self {
            Self {
                data: vec![0u8; (sectors * 512) as usize],
                sectors,
            }
        }

        pub fn with_data(sectors: u64, data: Vec<u8>) -> Self {
            Self { data, sectors }
        }
    }

    impl BlockDevice for MockBlockDevice {
        fn read(&self, lba: u64, buffer: &mut [u8]) -> Result<(), BlockDeviceError> {
            if lba >= self.sectors {
                return Err(BlockDeviceError::InvalidLba(lba));
            }

            let offset = (lba * 512) as usize;
            let end = offset + buffer.len();
            buffer.copy_from_slice(&self.data[offset..end]);

            Ok(())
        }

        fn write(&self, _lba: u64, _data: &[u8]) -> Result<(), BlockDeviceError> {
            // Mock is immutable, return error
            Err(BlockDeviceError::WriteProtected)
        }

        fn flush(&self) -> Result<(), BlockDeviceError> {
            Ok(())
        }

        fn sectors(&self) -> u64 {
            self.sectors
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_raw_block_device_create() {
        // Create temporary test file
        let test_path = "/tmp/test_block_device.img";
        let mut file = File::create(test_path).unwrap();
        file.set_len(1024 * 512).unwrap(); // 1024 sectors
        drop(file);

        let device = RawBlockDevice::new(test_path).unwrap();
        assert_eq!(device.sectors(), 1024);
        assert_eq!(device.sector_size(), 512);

        // Cleanup
        std::fs::remove_file(test_path).ok();
    }

    #[test]
    fn test_raw_block_device_read_write() {
        let test_path = "/tmp/test_block_device_rw.img";
        let mut file = File::create(test_path).unwrap();
        file.set_len(1024 * 512).unwrap();
        drop(file);

        let device = RawBlockDevice::new(test_path).unwrap();

        // Write data
        let write_data = vec![0xABu8; 512];
        device.write(0, &write_data).unwrap();

        // Read back
        let mut read_data = vec![0u8; 512];
        device.read(0, &mut read_data).unwrap();

        assert_eq!(read_data, write_data);

        // Cleanup
        std::fs::remove_file(test_path).ok();
    }

    #[test]
    fn test_mock_block_device() {
        let device = MockBlockDevice::new(100);
        assert_eq!(device.sectors(), 100);

        let mut buffer = vec![0u8; 512];
        device.read(0, &mut buffer).unwrap();
        assert!(device.write(0, &buffer).is_err());
    }
}
