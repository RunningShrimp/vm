//! # Disk Image Creation Module
//!
//! 提供虚拟磁盘镜像创建功能,支持多种格式和大小

use std::fs::File;
use std::io::{self, Write, Seek, SeekFrom};
use std::path::Path;

/// 磁盘镜像格式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskFormat {
    /// RAW格式 - 纯二进制,无元数据
    Raw,
    /// QCOW2格式 - QEMU Copy On Write (未实现)
    Qcow2,
    /// VMDK格式 - VMware (未实现)
    Vmdk,
    /// VDI格式 - VirtualBox (未实现)
    Vdi,
}

/// 磁盘镜像创建器
pub struct DiskImageCreator {
    /// 文件路径
    path: String,
    /// 磁盘大小(字节)
    size: u64,
    /// 磁盘格式
    format: DiskFormat,
    /// 扇区大小(通常512字节)
    sector_size: u32,
    /// 是否预分配空间
    preallocate: bool,
}

impl DiskImageCreator {
    /// 创建新的磁盘镜像创建器
    ///
    /// # 参数
    /// - `path`: 磁盘镜像文件路径
    /// - `size_gb`: 磁盘大小(GB)
    pub fn new<P: AsRef<Path>>(path: P, size_gb: u64) -> Self {
        Self {
            path: path.as_ref().to_string_lossy().to_string(),
            size: size_gb * 1024 * 1024 * 1024, // 转换为字节
            format: DiskFormat::Raw,
            sector_size: 512,
            preallocate: false,
        }
    }

    /// 设置磁盘格式
    pub fn format(mut self, format: DiskFormat) -> Self {
        self.format = format;
        self
    }

    /// 设置扇区大小
    pub fn sector_size(mut self, size: u32) -> Self {
        self.sector_size = size;
        self
    }

    /// 设置是否预分配空间
    pub fn preallocate(mut self, preallocate: bool) -> Self {
        self.preallocate = preallocate;
        self
    }

    /// 创建磁盘镜像
    ///
    /// # 返回
    /// 成功返回磁盘信息(扇区数),失败返回错误
    pub fn create(&self) -> Result<DiskInfo, String> {
        // 验证参数
        if self.size == 0 {
            return Err("Disk size cannot be zero".to_string());
        }

        if self.size % 512 != 0 {
            return Err("Disk size must be multiple of 512 bytes".to_string());
        }

        match self.format {
            DiskFormat::Raw => self.create_raw(),
            _ => Err(format!("Format {:?} not implemented yet", self.format)),
        }
    }

    /// 创建RAW格式磁盘镜像
    fn create_raw(&self) -> Result<DiskInfo, String> {
        let path = Path::new(&self.path);

        // 如果文件已存在,先删除
        if path.exists() {
            std::fs::remove_file(path)
                .map_err(|e| format!("Failed to remove existing file: {}", e))?;
        }

        // 创建父目录
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create parent directory: {}", e))?;
            }
        }

        // 创建文件
        let mut file = File::create(path)
            .map_err(|e| format!("Failed to create disk image: {}", e))?;

        // 设置文件大小
        let sector_count = self.size / self.sector_size as u64;

        if self.preallocate {
            // 预分配空间 - 写入零
            self.preallocate_space(&mut file)?;
        } else {
            // 稀疏文件 - 只设置文件大小
            file.set_len(self.size)
                .map_err(|e| format!("Failed to set file length: {}", e))?;
        }

        // 同步到磁盘
        file.sync_all()
            .map_err(|e| format!("Failed to sync disk image: {}", e))?;

        Ok(DiskInfo {
            path: self.path.clone(),
            size: self.size,
            sector_count,
            sector_size: self.sector_size,
            format: self.format,
        })
    }

    /// 预分配磁盘空间(写入零)
    fn preallocate_space(&self, file: &mut File) -> Result<(), String> {
        const BUFFER_SIZE: usize = 1024 * 1024; // 1MB缓冲区
        let buffer = vec![0u8; BUFFER_SIZE];
        let total_written = 0;

        while total_written < self.size {
            let to_write = std::cmp::min(BUFFER_SIZE as u64, self.size - total_written);
            file.write_all(&buffer[..to_write as usize])
                .map_err(|e| format!("Failed to write zeroes: {}", e))?;
        }

        Ok(())
    }

    /// 检查磁盘镜像是否存在
    pub fn exists(&self) -> bool {
        Path::new(&self.path).exists()
    }

    /// 获取磁盘镜像信息(如果已存在)
    pub fn info(&self) -> Result<DiskInfo, String> {
        let path = Path::new(&self.path);

        if !path.exists() {
            return Err("Disk image does not exist".to_string());
        }

        let metadata = std::fs::metadata(path)
            .map_err(|e| format!("Failed to get file metadata: {}", e))?;

        let size = metadata.len();
        let sector_count = size / self.sector_size as u64;

        Ok(DiskInfo {
            path: self.path.clone(),
            size,
            sector_count,
            sector_size: self.sector_size,
            format: self.format,
        })
    }
}

/// 磁盘信息
#[derive(Debug, Clone)]
pub struct DiskInfo {
    /// 文件路径
    pub path: String,
    /// 文件大小(字节)
    pub size: u64,
    /// 扇区数量
    pub sector_count: u64,
    /// 扇区大小(字节)
    pub sector_size: u32,
    /// 磁盘格式
    pub format: DiskFormat,
}

impl DiskInfo {
    /// 获取磁盘大小(GB)
    pub fn size_gb(&self) -> f64 {
        self.size as f64 / (1024.0 * 1024.0 * 1024.0)
    }

    /// 获取磁盘大小(MB)
    pub fn size_mb(&self) -> f64 {
        self.size as f64 / (1024.0 * 1024.0)
    }
}

/// 快速创建20GB RAW磁盘镜像
///
/// # 参数
/// - `path`: 磁盘镜像文件路径
///
/// # 返回
/// 成功返回磁盘信息,失败返回错误
pub fn create_disk_20gb<P: AsRef<Path>>(path: P) -> Result<DiskInfo, String> {
    DiskImageCreator::new(path, 20)
        .format(DiskFormat::Raw)
        .sector_size(512)
        .preallocate(false)
        .create()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_create_small_disk() {
        let test_path = "/tmp/test_disk.img";
        let result = DiskImageCreator::new(test_path, 1)
            .format(DiskFormat::Raw)
            .create();

        assert!(result.is_ok());
        let info = result.unwrap();
        assert_eq!(info.size, 1024 * 1024 * 1024);
        assert_eq!(info.sector_count, 2 * 1024 * 1024);

        // 清理
        let _ = fs::remove_file(test_path);
    }

    #[test]
    fn test_disk_info() {
        let test_path = "/tmp/test_disk_info.img";
        let _ = DiskImageCreator::new(test_path, 1)
            .format(DiskFormat::Raw)
            .create();

        let info = DiskImageCreator::new(test_path, 1).info().unwrap();
        assert_eq!(info.size_gb(), 1.0);

        // 清理
        let _ = fs::remove_file(test_path);
    }
}
