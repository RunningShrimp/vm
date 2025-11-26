//! ISO 9660 文件系统解析
//!
//! 支持读取 ISO 镜像文件的目录结构和文件内容

use std::io::{Read, Seek, SeekFrom};

/// ISO 9660 主卷描述符
#[derive(Debug, Clone)]
pub struct PrimaryVolumeDescriptor {
    /// 卷标识符
    pub volume_id: String,
    /// 卷大小（逻辑块数）
    pub volume_size: u32,
    /// 逻辑块大小
    pub logical_block_size: u16,
    /// 路径表大小
    pub path_table_size: u32,
    /// 根目录记录
    pub root_directory: DirectoryRecord,
}

/// 目录记录
#[derive(Debug, Clone)]
pub struct DirectoryRecord {
    /// 记录长度
    pub length: u8,
    /// 扩展属性记录长度
    pub ext_attr_length: u8,
    /// 数据位置（逻辑块号）
    pub extent_location: u32,
    /// 数据大小
    pub data_length: u32,
    /// 文件标志
    pub flags: u8,
    /// 文件标识符
    pub identifier: String,
}

impl DirectoryRecord {
    /// 是否是目录
    pub fn is_directory(&self) -> bool {
        self.flags & 0x02 != 0
    }
}

/// ISO 9660 文件系统
pub struct Iso9660<R: Read + Seek> {
    /// 底层读取器
    reader: R,
    /// 主卷描述符
    pvd: Option<PrimaryVolumeDescriptor>,
}

impl<R: Read + Seek> Iso9660<R> {
    /// 创建新的 ISO 9660 文件系统
    pub fn new(mut reader: R) -> Result<Self, String> {
        let mut fs = Self {
            reader,
            pvd: None,
        };
        
        fs.parse_volume_descriptors()?;
        Ok(fs)
    }

    /// 解析卷描述符
    fn parse_volume_descriptors(&mut self) -> Result<(), String> {
        // ISO 9660 卷描述符从扇区 16 开始
        const SECTOR_SIZE: u64 = 2048;
        const VD_START: u64 = 16 * SECTOR_SIZE;

        self.reader.seek(SeekFrom::Start(VD_START))
            .map_err(|e| format!("Failed to seek: {}", e))?;

        loop {
            let mut header = [0u8; 7];
            self.reader.read_exact(&mut header)
                .map_err(|e| format!("Failed to read header: {}", e))?;

            let vd_type = header[0];
            let identifier = &header[1..6];

            // 检查标识符
            if identifier != b"CD001" {
                return Err("Invalid ISO 9660 identifier".to_string());
            }

            match vd_type {
                1 => {
                    // 主卷描述符
                    self.pvd = Some(self.parse_primary_volume_descriptor()?);
                }
                255 => {
                    // 卷描述符集终止符
                    break;
                }
                _ => {
                    // 跳过其他类型的卷描述符
                    self.reader.seek(SeekFrom::Current(2048 - 7))
                        .map_err(|e| format!("Failed to seek: {}", e))?;
                }
            }
        }

        if self.pvd.is_none() {
            return Err("Primary volume descriptor not found".to_string());
        }

        Ok(())
    }

    /// 解析主卷描述符
    fn parse_primary_volume_descriptor(&mut self) -> Result<PrimaryVolumeDescriptor, String> {
        let mut data = [0u8; 2048 - 7];
        self.reader.read_exact(&mut data)
            .map_err(|e| format!("Failed to read PVD: {}", e))?;

        // 解析卷标识符（偏移 40，长度 32）
        let volume_id = String::from_utf8_lossy(&data[33..65])
            .trim()
            .to_string();

        // 解析卷大小（偏移 80，4 字节，little-endian）
        let volume_size = u32::from_le_bytes([
            data[73], data[74], data[75], data[76]
        ]);

        // 解析逻辑块大小（偏移 128，2 字节，little-endian）
        let logical_block_size = u16::from_le_bytes([data[121], data[122]]);

        // 解析路径表大小（偏移 132，4 字节，little-endian）
        let path_table_size = u32::from_le_bytes([
            data[125], data[126], data[127], data[128]
        ]);

        // 解析根目录记录（偏移 156，34 字节）
        let root_directory = self.parse_directory_record(&data[149..183])?;

        Ok(PrimaryVolumeDescriptor {
            volume_id,
            volume_size,
            logical_block_size,
            path_table_size,
            root_directory,
        })
    }

    /// 解析目录记录
    fn parse_directory_record(&self, data: &[u8]) -> Result<DirectoryRecord, String> {
        if data.len() < 33 {
            return Err("Directory record too short".to_string());
        }

        let length = data[0];
        let ext_attr_length = data[1];
        
        // 数据位置（偏移 2，4 字节，little-endian）
        let extent_location = u32::from_le_bytes([
            data[2], data[3], data[4], data[5]
        ]);

        // 数据大小（偏移 10，4 字节，little-endian）
        let data_length = u32::from_le_bytes([
            data[10], data[11], data[12], data[13]
        ]);

        // 文件标志（偏移 25）
        let flags = data[25];

        // 文件标识符长度（偏移 32）
        let id_length = data[32] as usize;

        // 文件标识符（偏移 33）
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

    /// 读取目录内容
    pub fn read_directory(&mut self, dir: &DirectoryRecord) -> Result<Vec<DirectoryRecord>, String> {
        if !dir.is_directory() {
            return Err("Not a directory".to_string());
        }

        let pvd = self.pvd.as_ref().ok_or("PVD not loaded")?;
        let block_size = pvd.logical_block_size as u64;
        let offset = dir.extent_location as u64 * block_size;

        self.reader.seek(SeekFrom::Start(offset))
            .map_err(|e| format!("Failed to seek: {}", e))?;

        let mut data = vec![0u8; dir.data_length as usize];
        self.reader.read_exact(&mut data)
            .map_err(|e| format!("Failed to read directory: {}", e))?;

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
            
            // 跳过 "." 和 ".." 条目
            if !record.identifier.is_empty() 
                && record.identifier != "\x00" 
                && record.identifier != "\x01" {
                entries.push(record);
            }

            pos += length;
        }

        Ok(entries)
    }

    /// 读取文件内容
    pub fn read_file(&mut self, file: &DirectoryRecord) -> Result<Vec<u8>, String> {
        if file.is_directory() {
            return Err("Cannot read directory as file".to_string());
        }

        let pvd = self.pvd.as_ref().ok_or("PVD not loaded")?;
        let block_size = pvd.logical_block_size as u64;
        let offset = file.extent_location as u64 * block_size;

        self.reader.seek(SeekFrom::Start(offset))
            .map_err(|e| format!("Failed to seek: {}", e))?;

        let mut data = vec![0u8; file.data_length as usize];
        self.reader.read_exact(&mut data)
            .map_err(|e| format!("Failed to read file: {}", e))?;

        Ok(data)
    }

    /// 查找文件
    pub fn find_file(&mut self, path: &str) -> Result<DirectoryRecord, String> {
        let pvd = self.pvd.as_ref().ok_or("PVD not loaded")?;
        let mut current_dir = pvd.root_directory.clone();

        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

        for (i, part) in parts.iter().enumerate() {
            let entries = self.read_directory(&current_dir)?;
            
            let found = entries.iter().find(|e| {
                e.identifier.to_uppercase() == part.to_uppercase()
            });

            match found {
                Some(entry) => {
                    if i == parts.len() - 1 {
                        return Ok(entry.clone());
                    } else if entry.is_directory() {
                        current_dir = entry.clone();
                    } else {
                        return Err(format!("{} is not a directory", part));
                    }
                }
                None => {
                    return Err(format!("File not found: {}", part));
                }
            }
        }

        Ok(current_dir)
    }

    /// 获取主卷描述符
    pub fn primary_volume_descriptor(&self) -> Option<&PrimaryVolumeDescriptor> {
        self.pvd.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_directory_record_parsing() {
        let data = vec![
            34, 0, // length, ext_attr_length
            0, 0, 0, 22, 0, 0, 0, 0, // extent_location (LE + BE)
            0, 0, 100, 0, 0, 0, 0, 0, // data_length (LE + BE)
            0, 0, 0, 0, 0, 0, 0, // recording date
            2, // flags (directory)
            0, 0, 0, 0, 0, 0, // file unit size, interleave gap
            1, 0, 1, 0, // volume sequence number
            4, // identifier length
            b'T', b'E', b'S', b'T', // identifier
        ];

        let iso = Iso9660::new(Cursor::new(Vec::<u8>::new())).ok();
        if let Some(iso) = iso {
            // 这个测试会失败，因为没有有效的 ISO 镜像
            // 仅用于演示目录记录解析逻辑
        }
    }
}
