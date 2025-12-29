//! ISO 9660文件系统支持
//!
//! 提供ISO 9660文件系统的挂载和管理功能
//! 从 vm-boot/iso9660.rs 迁移而来

use std::path::Path;
use vm_core::VmError;

/// ISO 9660 目录项
#[derive(Debug, Clone)]
pub struct IsoDirectory {
    /// 目录名称
    pub name: String,
    /// 目录位置（ISO中的偏移）
    pub location: u32,
    /// 子目录/文件列表
    pub entries: Vec<IsoEntry>,
}

/// ISO 9660 文件/目录条目
#[derive(Debug, Clone)]
pub enum IsoEntry {
    /// 目录
    Directory(IsoDirectory),
    /// 文件
    File {
        /// 文件名
        name: String,
        /// 文件位置（ISO中的偏移）
        location: u32,
        /// 文件大小（字节）
        size: u32,
        /// 文件数据起始位置
        data_location: u32,
    },
}

/// ISO 9660 管理器特征
pub trait Iso9660: Send + Sync {
    /// 挂载ISO镜像
    fn mount(&mut self, path: &Path) -> Result<(), VmError>;

    /// 卸载ISO镜像
    fn unmount(&mut self) -> Result<(), VmError>;

    /// 读取根目录
    fn read_root_dir(&self) -> Result<IsoDirectory, VmError>;

    /// 读取文件内容
    fn read_file(&self, path: &str) -> Result<Vec<u8>, VmError>;

    /// 列出目录内容
    fn list_directory(&self, path: &str) -> Result<Vec<IsoEntry>, VmError>;
}

/// ISO 9660 卷信息
#[derive(Debug, Clone)]
pub struct IsoVolumeInfo {
    /// 卷标识符
    pub volume_identifier: String,
    /// 系统标识符
    pub system_identifier: String,
    /// 卷空间大小（字节）
    pub volume_space_size: u64,
    /// 卷集大小（字节）
    pub volume_set_size: u64,
}

/// 简化的 ISO 9660 管理器实现
pub struct SimpleIso9660 {
    mounted: bool,
    volume_info: Option<IsoVolumeInfo>,
    iso_data: Option<Vec<u8>>,
    root_dir: Option<IsoDirectory>,
}

impl SimpleIso9660 {
    /// 创建新的 ISO 9660 管理器
    pub fn new() -> Self {
        Self {
            mounted: false,
            volume_info: None,
            iso_data: None,
            root_dir: None,
        }
    }

    /// 解析 ISO 9660 卷描述符
    fn parse_volume_descriptor(data: &[u8]) -> Result<IsoVolumeInfo, VmError> {
        // ISO 9660 卷描述符从 16 个扇区开始 (0x8000)
        const VOLUME_DESCRIPTOR_START: usize = 0x8000;

        if data.len() < VOLUME_DESCRIPTOR_START + 2048 {
            return Err(VmError::Io("ISO file too small".to_string()));
        }

        let vd_data = &data[VOLUME_DESCRIPTOR_START..VOLUME_DESCRIPTOR_START + 2048];

        // 检查卷描述符类型 (标准卷描述符 = 1)
        if vd_data[0] != 1 {
            return Err(VmError::Io("Invalid volume descriptor".to_string()));
        }

        // 提取卷标识符 (偏移 40, 长度 32)
        let volume_identifier = String::from_utf8_lossy(&vd_data[40..72])
            .trim_end_matches(' ')
            .to_string();

        // 提取系统标识符 (偏移 8, 长度 32)
        let system_identifier = String::from_utf8_lossy(&vd_data[8..40])
            .trim_end_matches(' ')
            .to_string();

        // 提取卷空间大小 (偏移 80, 长度 8, little-endian)
        let volume_space_size = u64::from_le_bytes(vd_data[80..88].try_into().unwrap_or([0u8; 8]));

        // 提取卷集大小 (偏移 120, 长度 4, little-endian)
        let volume_set_size_bytes: [u8; 4] = vd_data[120..124].try_into().unwrap_or([0u8; 4]);
        let volume_set_size = u32::from_le_bytes(volume_set_size_bytes) as u64;

        Ok(IsoVolumeInfo {
            volume_identifier,
            system_identifier,
            volume_space_size,
            volume_set_size,
        })
    }

    /// 解析目录结构
    fn parse_directory_structure(&self) -> Result<IsoDirectory, VmError> {
        let _data = self
            .iso_data
            .as_ref()
            .ok_or_else(|| VmError::Io("ISO not mounted".to_string()))?;

        // 简化的根目录解析
        // 实际实现需要解析 Path Table 和 Directory Records
        Ok(IsoDirectory {
            name: "/".to_string(),
            location: 0,
            entries: vec![],
        })
    }

    /// 挂载ISO镜像
    pub fn mount(&mut self, path: &Path) -> Result<(), VmError> {
        log::info!("Mounting ISO: {:?}", path);

        // 1. 验证 ISO 文件是否存在
        if !path.exists() {
            return Err(VmError::Io(format!("ISO file not found: {:?}", path)));
        }

        // 2. 读取 ISO 文件内容
        use std::fs;
        use std::io::Read;

        let mut file = fs::File::open(path)
            .map_err(|e| VmError::Io(format!("Failed to open ISO file: {}", e)))?;

        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)
            .map_err(|e| VmError::Io(format!("Failed to read ISO file: {}", e)))?;

        // 3. 验证 ISO 文件格式（检查 CD001 标识符）
        if buffer.len() < 0x8001 + 5 {
            return Err(VmError::Io("ISO file too small".to_string()));
        }

        // 检查卷描述符中的 "CD001" 标识符
        let cd001_signature = &buffer[0x8001..0x8006];
        if cd001_signature != b"CD001" {
            return Err(VmError::Io("Invalid ISO 9660 format".to_string()));
        }

        // 4. 解析主卷描述符
        let volume_info = Self::parse_volume_descriptor(&buffer)?;

        log::info!(
            "ISO volume: {} ({})",
            volume_info.volume_identifier,
            volume_info.system_identifier
        );
        log::info!("Volume size: {} bytes", volume_info.volume_space_size);

        // 5. 保存 ISO 数据和卷信息
        self.iso_data = Some(buffer);
        self.volume_info = Some(volume_info);

        // 6. 解析根目录
        self.root_dir = Some(self.parse_directory_structure()?);

        self.mounted = true;
        log::info!("ISO mounted successfully");
        Ok(())
    }

    /// 卸载ISO镜像
    pub fn unmount(&mut self) -> Result<(), VmError> {
        log::info!("Unmounting ISO");

        if !self.mounted {
            return Err(VmError::Io("No ISO mounted".to_string()));
        }

        self.mounted = false;
        self.volume_info = None;
        self.iso_data = None;
        self.root_dir = None;
        log::info!("ISO unmounted successfully");
        Ok(())
    }

    /// 读取根目录
    pub fn read_root_dir(&self) -> Result<IsoDirectory, VmError> {
        if !self.mounted {
            return Err(VmError::Io("No ISO mounted".to_string()));
        }

        self.root_dir
            .clone()
            .ok_or_else(|| VmError::Io("Root directory not available".to_string()))
    }

    /// 读取文件内容
    pub fn read_file(&self, path: &str) -> Result<Vec<u8>, VmError> {
        if !self.mounted {
            return Err(VmError::Io("No ISO mounted".to_string()));
        }

        let _data = self
            .iso_data
            .as_ref()
            .ok_or_else(|| VmError::Io("ISO data not available".to_string()))?;

        log::info!("Reading file from ISO: {}", path);

        // 简化实现：在实际实现中需要：
        // 1. 解析路径找到对应的目录记录
        // 2. 获取文件的位置和大小
        // 3. 从 ISO 数据中读取文件内容

        // 这里返回空数据作为占位符
        Ok(vec![])
    }

    /// 列出目录内容
    pub fn list_directory(&self, path: &str) -> Result<Vec<IsoEntry>, VmError> {
        if !self.mounted {
            return Err(VmError::Io("No ISO mounted".to_string()));
        }

        log::info!("Listing directory from ISO: {}", path);

        // 简化实现：在实际实现中需要：
        // 1. 解析路径找到对应的目录
        // 2. 读取目录记录
        // 3. 返回目录中的所有条目

        if path == "/" || path.is_empty() {
            // 返回根目录
            if let Some(ref root_dir) = self.root_dir {
                return Ok(root_dir.entries.clone());
            }
        }

        Ok(vec![])
    }
}

impl Default for SimpleIso9660 {
    fn default() -> Self {
        Self::new()
    }
}
