//! ISO 9660 文件系统解析
//!
//! 支持读取 ISO 镜像文件的目录结构和文件内容
//!
//! This module uses standardized error handling with conversions to VmError.

use std::io::{Read, Seek, SeekFrom};

use thiserror::Error;
use vm_core::{CoreError, VmError};

/// ISO 9660 主卷描述符
#[derive(Debug, Clone)]
pub struct PrimaryVolumeDescriptor {
    pub volume_id: String,
    pub volume_size: u32,
    pub logical_block_size: u16,
    pub path_table_size: u32,
    pub root_directory: DirectoryRecord,
}

/// 目录记录
#[derive(Debug, Clone)]
pub struct DirectoryRecord {
    pub length: u8,
    pub ext_attr_length: u8,
    pub extent_location: u32,
    pub data_length: u32,
    pub flags: u8,
    pub identifier: String,
}

impl DirectoryRecord {
    pub fn is_directory(&self) -> bool {
        self.flags & 0x02 != 0
    }
}

/// ISO 9660 文件系统
pub struct Iso9660<R: Read + Seek> {
    reader: R,
    pvd: Option<PrimaryVolumeDescriptor>,
}

/// ISO 9660 specific errors
#[derive(Debug, Error)]
pub enum IsoError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid ISO 9660 identifier")]
    InvalidIdentifier,

    #[error("Primary volume descriptor not found")]
    PvdNotFound,

    #[error("Directory record too short")]
    DirRecordShort,

    #[error("Not a directory")]
    NotDirectory,

    #[error("PVD not loaded")]
    PvdNotLoaded,

    #[error("{0} is not a directory")]
    NotDirectoryName(String),

    #[error("File not found: {0}")]
    FileNotFound(String),
}

/// Conversion from IsoError to unified VmError
impl From<IsoError> for VmError {
    fn from(err: IsoError) -> Self {
        match err {
            IsoError::Io(io_err) => VmError::Io(io_err.to_string()),
            IsoError::InvalidIdentifier => VmError::Core(CoreError::InvalidParameter {
                name: "iso_identifier".to_string(),
                value: "".to_string(),
                message: "Invalid ISO 9660 identifier".to_string(),
            }),
            IsoError::PvdNotFound => VmError::Core(CoreError::Internal {
                message: "Primary volume descriptor not found".to_string(),
                module: "iso9660".to_string(),
            }),
            IsoError::DirRecordShort => VmError::Core(CoreError::InvalidParameter {
                name: "directory_record".to_string(),
                value: "".to_string(),
                message: "Directory record too short".to_string(),
            }),
            IsoError::NotDirectory => VmError::Core(CoreError::InvalidParameter {
                name: "path".to_string(),
                value: "".to_string(),
                message: "Not a directory".to_string(),
            }),
            IsoError::PvdNotLoaded => VmError::Core(CoreError::InvalidState {
                message: "PVD not loaded".to_string(),
                current: "no_pvd".to_string(),
                expected: "pvd_loaded".to_string(),
            }),
            IsoError::NotDirectoryName(name) => VmError::Core(CoreError::InvalidParameter {
                name: "path".to_string(),
                value: name.clone(),
                message: format!("{} is not a directory", name),
            }),
            IsoError::FileNotFound(path) => VmError::Core(CoreError::Internal {
                message: format!("file not found: {}", path),
                module: "vm-boot::iso9660".to_string(),
            }),
        }
    }
}

impl<R: Read + Seek> Iso9660<R> {
    pub fn new(reader: R) -> Result<Self, IsoError> {
        let mut fs = Self { reader, pvd: None };
        fs.parse_volume_descriptors()?;
        Ok(fs)
    }

    fn parse_volume_descriptors(&mut self) -> Result<(), IsoError> {
        const SECTOR_SIZE: u64 = 2048;
        const VD_START: u64 = 16 * SECTOR_SIZE;
        self.reader.seek(SeekFrom::Start(VD_START))?;

        loop {
            let mut header = [0u8; 7];
            self.reader.read_exact(&mut header)?;
            let vd_type = header[0];
            let identifier = &header[1..6];

            if identifier != b"CD001" {
                return Err(IsoError::InvalidIdentifier);
            }

            match vd_type {
                1 => {
                    self.pvd = Some(self.parse_primary_volume_descriptor()?);
                }
                255 => {
                    break;
                }
                _ => {
                    self.reader.seek(SeekFrom::Current(2048 - 7))?;
                }
            }
        }

        if self.pvd.is_none() {
            return Err(IsoError::PvdNotFound);
        }
        Ok(())
    }

    fn parse_primary_volume_descriptor(&mut self) -> Result<PrimaryVolumeDescriptor, IsoError> {
        let mut data = [0u8; 2048 - 7];
        self.reader.read_exact(&mut data)?;

        let volume_id = String::from_utf8_lossy(&data[33..65]).trim().to_string();
        let volume_size = u32::from_le_bytes([data[73], data[74], data[75], data[76]]);
        let logical_block_size = u16::from_le_bytes([data[121], data[122]]);
        let path_table_size = u32::from_le_bytes([data[125], data[126], data[127], data[128]]);
        let root_directory = self.parse_directory_record(&data[149..183])?;

        Ok(PrimaryVolumeDescriptor {
            volume_id,
            volume_size,
            logical_block_size,
            path_table_size,
            root_directory,
        })
    }

    fn parse_directory_record(&self, data: &[u8]) -> Result<DirectoryRecord, IsoError> {
        if data.len() < 33 {
            return Err(IsoError::DirRecordShort);
        }
        let length = data[0];
        let ext_attr_length = data[1];
        let extent_location = u32::from_le_bytes([data[2], data[3], data[4], data[5]]);
        let data_length = u32::from_le_bytes([data[10], data[11], data[12], data[13]]);
        let flags = data[25];
        let id_length = data[32] as usize;
        let identifier = if id_length > 0 && data.len() >= 33 + id_length {
            String::from_utf8_lossy(&data[33..33 + id_length]).to_string()
        } else {
            String::new()
        };
        Ok(DirectoryRecord {
            length,
            ext_attr_length,
            extent_location,
            data_length,
            flags,
            identifier,
        })
    }

    pub fn read_directory(
        &mut self,
        dir: &DirectoryRecord,
    ) -> Result<Vec<DirectoryRecord>, IsoError> {
        if !dir.is_directory() {
            return Err(IsoError::NotDirectory);
        }
        let pvd = self.pvd.as_ref().ok_or(IsoError::PvdNotLoaded)?;
        let block_size = pvd.logical_block_size as u64;
        let offset = dir.extent_location as u64 * block_size;
        self.reader.seek(SeekFrom::Start(offset))?;

        let mut data = vec![0u8; dir.data_length as usize];
        self.reader.read_exact(&mut data)?;
        self.reader.read_exact(&mut data)?;

        let mut entries = Vec::new();
        let mut pos = 0;
        while pos < data.len() {
            let length = data[pos] as usize;
            if length == 0 {
                break;
            }
            if pos + length > data.len() {
                break;
            }
            let record = self.parse_directory_record(&data[pos..pos + length])?;
            if !record.identifier.is_empty()
                && record.identifier != "\x00"
                && record.identifier != "\x01"
            {
                entries.push(record);
            }
            pos += length;
        }
        Ok(entries)
    }

    pub fn read_file(&mut self, file: &DirectoryRecord) -> Result<Vec<u8>, IsoError> {
        if file.is_directory() {
            return Err(IsoError::NotDirectory);
        }
        let pvd = self.pvd.as_ref().ok_or(IsoError::PvdNotLoaded)?;
        let block_size = pvd.logical_block_size as u64;
        let offset = file.extent_location as u64 * block_size;
        self.reader.seek(SeekFrom::Start(offset))?;
        let mut data = vec![0u8; file.data_length as usize];
        self.reader.read_exact(&mut data)?;
        Ok(data)
    }

    pub fn find_file(&mut self, path: &str) -> Result<DirectoryRecord, IsoError> {
        let pvd = self.pvd.as_ref().ok_or(IsoError::PvdNotLoaded)?;
        let mut current_dir = pvd.root_directory.clone();
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        for (i, part) in parts.iter().enumerate() {
            let entries = self.read_directory(&current_dir)?;
            let found = entries
                .iter()
                .find(|e| e.identifier.to_uppercase() == part.to_uppercase());
            match found {
                Some(entry) => {
                    if i == parts.len() - 1 {
                        return Ok(entry.clone());
                    } else if entry.is_directory() {
                        current_dir = entry.clone();
                    } else {
                        return Err(IsoError::NotDirectoryName((*part).into()));
                    }
                }
                None => {
                    return Err(IsoError::FileNotFound((*part).into()));
                }
            }
        }
        Ok(current_dir)
    }

    pub fn primary_volume_descriptor(&self) -> Option<&PrimaryVolumeDescriptor> {
        self.pvd.as_ref()
    }
}
