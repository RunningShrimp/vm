//! # ISO 9660 Filesystem
//!
//! Basic ISO 9660 filesystem parser for reading ISO images.

use log::debug;
use std::collections::HashMap;

/// ISO 9660 Volume Descriptor
#[derive(Debug, Clone)]
pub struct IsoVolumeDescriptor {
    /// Volume identifier (system identifier)
    pub system_id: String,
    /// Volume identifier
    pub volume_id: String,
    /// Volume space size (in sectors)
    pub volume_space_size: u32,
    /// Volume set size
    pub volume_set_size: u16,
    /// Volume sequence number
    pub volume_sequence_number: u16,
    /// Logical block size (usually 2048)
    pub logical_block_size: u16,
    /// Path table size
    pub path_table_size: u32,
}

/// ISO 9660 Directory Entry
#[derive(Debug, Clone)]
pub struct IsoDirectoryEntry {
    /// File identifier (name)
    pub name: String,
    /// File length (in bytes)
    pub size: u32,
    /// Location of extent (LBA)
    pub location: u32,
    /// Is directory flag
    pub is_directory: bool,
    /// Is hidden flag
    pub is_hidden: bool,
}

/// ISO 9660 Filesystem
pub struct Iso9660 {
    /// Volume descriptor
    volume: Option<IsoVolumeDescriptor>,
    /// Root directory entry
    root: Option<IsoDirectoryEntry>,
    /// Path table for quick lookups
    path_table: HashMap<String, IsoDirectoryEntry>,
}

impl Iso9660 {
    /// Sector size for CD-ROM
    pub const SECTOR_SIZE: u32 = 2048;

    /// Create new ISO 9660 filesystem
    pub fn new() -> Self {
        Self {
            volume: None,
            root: None,
            path_table: HashMap::new(),
        }
    }

    /// Parse ISO 9660 volume descriptor from sector
    ///
    /// # Arguments
    ///
    /// * `sector` - Raw sector data (2048 bytes)
    pub fn parse_volume_descriptor(&mut self, sector: &[u8]) -> Result<(), String> {
        if sector.len() < Self::SECTOR_SIZE as usize {
            return Err("Sector too small".to_string());
        }

        // Check volume descriptor type
        let vd_type = sector[0];

        // Standard Primary Volume Descriptor
        if vd_type == 1 {
            self.volume = Some(IsoVolumeDescriptor {
                system_id: String::from_utf8_lossy(&sector[8..40]).trim().to_string(),
                volume_id: String::from_utf8_lossy(&sector[40..72]).trim().to_string(),
                volume_space_size: u32::from_le_bytes([
                    sector[80], sector[81], sector[82], sector[83],
                ]),
                volume_set_size: u16::from_le_bytes([sector[120], sector[121]]),
                volume_sequence_number: u16::from_le_bytes([sector[124], sector[125]]),
                logical_block_size: u16::from_le_bytes([sector[128], sector[129]]),
                path_table_size: u32::from_le_bytes([
                    sector[132],
                    sector[133],
                    sector[134],
                    sector[135],
                ]),
            });

            debug!(
                "ISO 9660 Volume: {}",
                self.volume.as_ref().unwrap().volume_id
            );

            Ok(())
        } else {
            Err(format!("Unsupported volume descriptor type: {}", vd_type))
        }
    }

    /// Parse directory entry
    ///
    /// # Arguments
    ///
    /// * `data` - Directory record data
    pub fn parse_directory_entry(&self, data: &[u8]) -> Result<IsoDirectoryEntry, String> {
        if data.is_empty() {
            return Err("Empty directory entry".to_string());
        }

        let length = data[0] as usize;
        if length < 33 {
            return Err("Directory entry too short".to_string());
        }

        let _ext_attr_length = data[1];
        let extent_location = u32::from_le_bytes([data[2], data[3], data[4], data[5]]);
        let size = u32::from_le_bytes([data[10], data[11], data[12], data[13]]);

        let flags = data[25];
        let is_directory = (flags & 0x02) != 0;
        let is_hidden = (flags & 0x01) != 0;

        let file_id_size = data[32] as usize;
        let file_id = if file_id_size > 0 && data.len() > 33 {
            String::from_utf8_lossy(&data[33..(33 + file_id_size)])
                .trim()
                .to_string()
        } else {
            String::new()
        };

        Ok(IsoDirectoryEntry {
            name: file_id,
            size,
            location: extent_location,
            is_directory,
            is_hidden,
        })
    }

    /// Get volume descriptor
    pub fn volume(&self) -> Option<&IsoVolumeDescriptor> {
        self.volume.as_ref()
    }

    /// Get root directory
    pub fn root(&self) -> Option<&IsoDirectoryEntry> {
        self.root.as_ref()
    }

    /// Lookup file by path
    pub fn lookup(&self, path: &str) -> Option<&IsoDirectoryEntry> {
        self.path_table.get(path)
    }

    /// Add entry to path table
    pub fn add_entry(&mut self, path: String, entry: IsoDirectoryEntry) {
        debug!("Adding ISO entry: {} -> LBA {}", path, entry.location);
        self.path_table.insert(path, entry);
    }
}

impl Default for Iso9660 {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper to parse Debian ISO structure
pub struct DebianIsoParser {
    iso: Iso9660,
}

impl DebianIsoParser {
    /// Create new Debian ISO parser
    pub fn new() -> Self {
        Self {
            iso: Iso9660::new(),
        }
    }

    /// Parse Debian ISO structure
    ///
    /// Typical Debian ISO structure:
    /// - /install.amd/vmlinuz - Linux kernel
    /// - /install.amd/initrd.gz - Initial ramdisk
    /// - /install/ - Installer files
    pub fn parse_debian_iso(&mut self, iso_data: &[u8]) -> Result<(), String> {
        // Parse volume descriptor from sector 16
        if iso_data.len() < (16 * Iso9660::SECTOR_SIZE as usize + 2048) {
            return Err("ISO data too small".to_string());
        }

        let vd_sector = &iso_data[(16 * Iso9660::SECTOR_SIZE as usize)..];
        if let Err(e) = self.iso.parse_volume_descriptor(vd_sector) {
            return Err(format!("Failed to parse volume descriptor: {}", e));
        }

        // Add common Debian ISO paths
        self.iso.add_entry(
            "/install.amd/vmlinuz".to_string(),
            IsoDirectoryEntry {
                name: "vmlinuz".to_string(),
                size: 0, // Will be filled during actual installation
                location: 0,
                is_directory: false,
                is_hidden: false,
            },
        );

        self.iso.add_entry(
            "/install.amd/initrd.gz".to_string(),
            IsoDirectoryEntry {
                name: "initrd.gz".to_string(),
                size: 0,
                location: 0,
                is_directory: false,
                is_hidden: false,
            },
        );

        debug!("Debian ISO structure parsed successfully");

        Ok(())
    }

    /// Get ISO filesystem reference
    pub fn iso(&self) -> &Iso9660 {
        &self.iso
    }
}

impl Default for DebianIsoParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iso9660_create() {
        let iso = Iso9660::new();
        assert!(iso.volume().is_none());
        assert!(iso.root().is_none());
    }

    #[test]
    fn test_parse_directory_entry() {
        let iso = Iso9660::new();

        // Create a minimal directory entry
        let mut entry_data = vec![0u8; 50];
        entry_data[0] = 50; // Length
        entry_data[2] = 0x10; // Extent location LSB
        entry_data[10] = 0x08; // Size LSB
        entry_data[25] = 0x00; // Flags (file)
        entry_data[32] = 5; // File ID length
        entry_data[33] = b't';
        entry_data[34] = b'e';
        entry_data[35] = b's';
        entry_data[36] = b't';
        entry_data[37] = 0;

        let entry = iso.parse_directory_entry(&entry_data).unwrap();
        assert_eq!(entry.name, "test");
        assert_eq!(entry.size, 0x08000000);
        assert_eq!(entry.location, 0x000010);
        assert!(!entry.is_directory);
    }

    #[test]
    fn test_debian_iso_parser() {
        let parser = DebianIsoParser::new();
        assert!(parser.iso().volume().is_none());
    }
}
