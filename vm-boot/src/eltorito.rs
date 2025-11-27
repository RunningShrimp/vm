//! El Torito 引导规范实现
//!
//! 支持从 ISO 镜像引导 BIOS 和 UEFI 系统

use std::io::{Read, Seek, SeekFrom};
use thiserror::Error;
use thiserror::Error;

/// El Torito 引导目录
#[derive(Debug, Clone)]
pub struct BootCatalog {
    /// 验证条目
    pub validation_entry: ValidationEntry,
    /// 初始/默认条目
    pub initial_entry: BootEntry,
    /// 附加引导条目
    pub additional_entries: Vec<BootEntry>,
}

/// 验证条目
#[derive(Debug, Clone)]
pub struct ValidationEntry {
    /// 平台 ID
    pub platform_id: u8,
    /// 制造商字符串
    pub manufacturer: String,
    /// 校验和
    pub checksum: u16,
}

/// 引导条目
#[derive(Debug, Clone)]
pub struct BootEntry {
    /// 引导指示器
    pub boot_indicator: u8,
    /// 引导媒体类型
    pub boot_media_type: BootMediaType,
    /// 加载段
    pub load_segment: u16,
    /// 系统类型
    pub system_type: u8,
    /// 扇区数
    pub sector_count: u16,
    /// 虚拟磁盘起始扇区
    pub load_rba: u32,
}

/// 引导媒体类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BootMediaType {
    /// 无模拟
    NoEmulation = 0,
    /// 1.2 MB 软盘
    Floppy12 = 1,
    /// 1.44 MB 软盘
    Floppy144 = 2,
    /// 2.88 MB 软盘
    Floppy288 = 3,
    /// 硬盘
    HardDisk = 4,
}

impl BootMediaType {
    fn from_u8(value: u8) -> Self {
        match value & 0x0F {
            0 => BootMediaType::NoEmulation,
            1 => BootMediaType::Floppy12,
            2 => BootMediaType::Floppy144,
            3 => BootMediaType::Floppy288,
            4 => BootMediaType::HardDisk,
            _ => BootMediaType::NoEmulation,
        }
    }
}

/// El Torito 引导解析器
pub struct ElTorito<R: Read + Seek> {
    reader: R,
    catalog: Option<BootCatalog>,
}

impl<R: Read + Seek> ElTorito<R> {
    /// 创建新的 El Torito 解析器
    pub fn new(reader: R) -> Result<Self, EltoritoError> {
    pub fn new(reader: R) -> Result<Self, EltoritoError> {
        let mut parser = Self {
            reader,
            catalog: None,
        };
        
        parser.parse_boot_catalog()?;
        Ok(parser)
    }

    /// 查找引导卷描述符
    fn find_boot_volume_descriptor(&mut self) -> Result<u32, EltoritoError> {
    fn find_boot_volume_descriptor(&mut self) -> Result<u32, EltoritoError> {
        const SECTOR_SIZE: u64 = 2048;
        const VD_START: u64 = 16 * SECTOR_SIZE;

        self.reader.seek(SeekFrom::Start(VD_START))?;
        self.reader.seek(SeekFrom::Start(VD_START))?;

        loop {
            let mut header = [0u8; 7];
            self.reader.read_exact(&mut header)?;
            self.reader.read_exact(&mut header)?;

            let vd_type = header[0];
            let identifier = &header[1..6];

            if identifier != b"CD001" { return Err(EltoritoError::InvalidIdentifier); }

            if vd_type == 0 {
                // 引导记录
                let mut boot_data = [0u8; 2048 - 7];
                self.reader.read_exact(&mut boot_data)?;
                self.reader.read_exact(&mut boot_data)?;

                // 检查 El Torito 标识符（偏移 0，32 字节）
                if &boot_data[0..23] == b"EL TORITO SPECIFICATION" {
                    // 引导目录位置（偏移 71，4 字节，little-endian）
                    let catalog_sector = u32::from_le_bytes([
                        boot_data[64], boot_data[65], boot_data[66], boot_data[67]
                    ]);
                    return Ok(catalog_sector);
                }
            } else if vd_type == 255 {
                // 卷描述符集终止符
                break;
            } else {
                // 跳过其他类型的卷描述符
                self.reader.seek(SeekFrom::Current(2048 - 7))?;
                self.reader.seek(SeekFrom::Current(2048 - 7))?;
            }
        }

        Err(EltoritoError::BootDescriptorNotFound)
        Err(EltoritoError::BootDescriptorNotFound)
    }

    /// 解析引导目录
    fn parse_boot_catalog(&mut self) -> Result<(), EltoritoError> {
    fn parse_boot_catalog(&mut self) -> Result<(), EltoritoError> {
        let catalog_sector = self.find_boot_volume_descriptor()?;

        const SECTOR_SIZE: u64 = 2048;
        let offset = catalog_sector as u64 * SECTOR_SIZE;

        self.reader.seek(SeekFrom::Start(offset))?;
        self.reader.seek(SeekFrom::Start(offset))?;

        // 读取验证条目
        let mut validation_data = [0u8; 32];
        self.reader.read_exact(&mut validation_data)?;
        self.reader.read_exact(&mut validation_data)?;

        let validation_entry = self.parse_validation_entry(&validation_data)?;

        // 读取初始/默认条目
        let mut initial_data = [0u8; 32];
        self.reader.read_exact(&mut initial_data)?;
        self.reader.read_exact(&mut initial_data)?;

        let initial_entry = self.parse_boot_entry(&initial_data)?;

        self.catalog = Some(BootCatalog {
            validation_entry,
            initial_entry,
            additional_entries: Vec::new(),
        });

        Ok(())
    }

    /// 解析验证条目
    fn parse_validation_entry(&self, data: &[u8; 32]) -> Result<ValidationEntry, EltoritoError> {
    fn parse_validation_entry(&self, data: &[u8; 32]) -> Result<ValidationEntry, EltoritoError> {
        // 头部 ID（偏移 0）
        if data[0] != 0x01 { return Err(EltoritoError::InvalidValidationHeader); }

        // 平台 ID（偏移 1）
        let platform_id = data[1];

        // 制造商字符串（偏移 4，24 字节）
        let manufacturer = String::from_utf8_lossy(&data[4..28])
            .trim()
            .to_string();

        // 校验和（偏移 28，2 字节）
        let checksum = u16::from_le_bytes([data[28], data[29]]);

        // 密钥字节（偏移 30，2 字节）应该是 0x55AA
        let key = u16::from_le_bytes([data[30], data[31]]);
        if key != 0x55AA { return Err(EltoritoError::InvalidValidationKey); }

        Ok(ValidationEntry {
            platform_id,
            manufacturer,
            checksum,
        })
    }

    /// 解析引导条目
    fn parse_boot_entry(&self, data: &[u8; 32]) -> Result<BootEntry, EltoritoError> {
    fn parse_boot_entry(&self, data: &[u8; 32]) -> Result<BootEntry, EltoritoError> {
        // 引导指示器（偏移 0）
        let boot_indicator = data[0];

        // 引导媒体类型（偏移 1）
        let boot_media_type = BootMediaType::from_u8(data[1]);

        // 加载段（偏移 2，2 字节）
        let load_segment = u16::from_le_bytes([data[2], data[3]]);

        // 系统类型（偏移 4）
        let system_type = data[4];

        // 扇区数（偏移 6，2 字节）
        let sector_count = u16::from_le_bytes([data[6], data[7]]);

        // 虚拟磁盘起始扇区（偏移 8，4 字节）
        let load_rba = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);

        Ok(BootEntry {
            boot_indicator,
            boot_media_type,
            load_segment,
            system_type,
            sector_count,
            load_rba,
        })
    }

    /// 获取引导目录
    pub fn boot_catalog(&self) -> Option<&BootCatalog> {
        self.catalog.as_ref()
    }

    /// 读取引导镜像
    pub fn read_boot_image(&mut self, entry: &BootEntry) -> Result<Vec<u8>, EltoritoError> {
    pub fn read_boot_image(&mut self, entry: &BootEntry) -> Result<Vec<u8>, EltoritoError> {
        const SECTOR_SIZE: u64 = 2048;
        let offset = entry.load_rba as u64 * SECTOR_SIZE;

        self.reader.seek(SeekFrom::Start(offset))?;
        self.reader.seek(SeekFrom::Start(offset))?;

        // 计算要读取的大小
        let size = if entry.boot_media_type == BootMediaType::NoEmulation {
            // 无模拟模式：使用扇区数
            if entry.sector_count == 0 {
                // 如果扇区数为 0，默认读取 512 字节（一个引导扇区）
                512
            } else {
                entry.sector_count as usize * 512
            }
        } else {
            // 模拟模式：根据媒体类型确定大小
            match entry.boot_media_type {
                BootMediaType::Floppy12 => 1_228_800,  // 1.2 MB
                BootMediaType::Floppy144 => 1_474_560, // 1.44 MB
                BootMediaType::Floppy288 => 2_949_120, // 2.88 MB
                BootMediaType::HardDisk => {
                    // 硬盘模式：读取第一个扇区以获取分区表
                    512
                }
                _ => 512,
            }
        };

        let mut data = vec![0u8; size];
        self.reader.read_exact(&mut data)?;
        self.reader.read_exact(&mut data)?;

        Ok(data)
    }
}

#[derive(Debug, Error)]
pub enum EltoritoError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid ISO 9660 identifier")]
    InvalidIdentifier,
    #[error("Boot volume descriptor not found")]
    BootDescriptorNotFound,
    #[error("Invalid validation entry header")]
    InvalidValidationHeader,
    #[error("Invalid validation entry key")]
    InvalidValidationKey,
}

#[derive(Debug, Error)]
pub enum EltoritoError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid ISO 9660 identifier")]
    InvalidIdentifier,
    #[error("Boot volume descriptor not found")]
    BootDescriptorNotFound,
    #[error("Invalid validation entry header")]
    InvalidValidationHeader,
    #[error("Invalid validation entry key")]
    InvalidValidationKey,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boot_media_type() {
        assert_eq!(BootMediaType::from_u8(0), BootMediaType::NoEmulation);
        assert_eq!(BootMediaType::from_u8(1), BootMediaType::Floppy12);
        assert_eq!(BootMediaType::from_u8(2), BootMediaType::Floppy144);
        assert_eq!(BootMediaType::from_u8(4), BootMediaType::HardDisk);
    }
}
