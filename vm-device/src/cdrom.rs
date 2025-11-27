//! CDROM 设备模拟
//!
//! 支持将 ISO 镜像文件作为虚拟 CD-ROM 设备挂载

use vm_core::MmioDevice;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::sync::{Arc, Mutex};
use thiserror::Error;

/// CDROM 设备状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CdromStatus {
    /// 空闲
    Idle,
    /// 读取中
    Reading,
    /// 错误
    Error,
}

/// CDROM 设备
pub struct CdromDevice {
    /// ISO 镜像文件
    image: Option<Arc<Mutex<File>>>,
    /// 设备状态
    status: CdromStatus,
    /// 当前扇区
    current_sector: u32,
    /// 扇区缓冲区
    sector_buffer: Vec<u8>,
    /// 设备容量（扇区数）
    capacity: u32,
}

impl CdromDevice {
    /// 创建新的 CDROM 设备
    pub fn new() -> Self {
        Self {
            image: None,
            status: CdromStatus::Idle,
            current_sector: 0,
            sector_buffer: vec![0; 2048], // ISO 9660 扇区大小
            capacity: 0,
        }
    }

    /// 挂载 ISO 镜像
    pub fn mount(&mut self, iso_path: &str) -> Result<(), CdromError> {
        let file = File::open(iso_path)?;

        // 获取文件大小
        let metadata = file.metadata()?;
        
        let file_size = metadata.len();
        self.capacity = (file_size / 2048) as u32;

        self.image = Some(Arc::new(Mutex::new(file)));
        self.status = CdromStatus::Idle;

        log::info!("Mounted ISO: {} ({} sectors)", iso_path, self.capacity);
        Ok(())
    }

    /// 卸载 ISO 镜像
    pub fn unmount(&mut self) {
        self.image = None;
        self.status = CdromStatus::Idle;
        self.capacity = 0;
        log::info!("Unmounted ISO");
    }

    /// 读取扇区
    pub fn read_sector(&mut self, sector: u32) -> Result<&[u8], CdromError> {
        let image = self.image.as_ref()
            .ok_or(CdromError::NotMounted)?;

        if sector >= self.capacity {
            return Err(CdromError::SectorOutOfRange);
        }

        let mut file = image.lock().unwrap();
        
        let offset = sector as u64 * 2048;
        file.seek(SeekFrom::Start(offset))?;

        file.read_exact(&mut self.sector_buffer)?;

        self.current_sector = sector;
        self.status = CdromStatus::Idle;

        Ok(&self.sector_buffer)
    }

    /// 获取容量
    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    /// 获取状态
    pub fn status(&self) -> CdromStatus {
        self.status
    }

    /// 是否已挂载
    pub fn is_mounted(&self) -> bool {
        self.image.is_some()
    }
}

impl MmioDevice for CdromDevice {
    fn read(&self, offset: u64, _size: u8) -> u64 {
        match offset {
            0x00 => self.capacity as u64,
            0x04 => self.current_sector as u64,
            0x08 => self.status as u64,
            0x0C => self.is_mounted() as u64,
            _ => 0,
        }
    }

    fn write(&mut self, offset: u64, val: u64, _size: u8) {
        match offset {
            0x04 => {
                // 设置当前扇区
                self.current_sector = val as u32;
            }
            0x10 => {
                // 触发读取
                if val == 1 {
                    self.status = CdromStatus::Reading;
                    if let Err(e) = self.read_sector(self.current_sector) {
                        log::error!("CDROM read error: {}", e);
                        self.status = CdromStatus::Error;
                    }
                }
            }
            _ => {
                log::trace!("CDROM write: offset={:#x} val={:#x}", offset, val);
            }
        }
    }
}

/// VirtIO CDROM 设备（基于 virtio-block）
pub struct VirtioCdrom {
    /// 底层 CDROM 设备
    cdrom: CdromDevice,
    /// VirtIO 队列选择器
    queue_sel: u32,
    /// 设备状态
    status: u32,
}

impl VirtioCdrom {
    /// 创建新的 VirtIO CDROM 设备
    pub fn new() -> Self {
        Self {
            cdrom: CdromDevice::new(),
            queue_sel: 0,
            status: 0,
        }
    }

    /// 挂载 ISO 镜像
    pub fn mount(&mut self, iso_path: &str) -> Result<(), CdromError> {
        self.cdrom.mount(iso_path)
    }

    /// 卸载 ISO 镜像
    pub fn unmount(&mut self) {
        self.cdrom.unmount();
    }

    /// 读取扇区
    pub fn read_sector(&mut self, sector: u32) -> Result<&[u8], CdromError> {
        self.cdrom.read_sector(sector)
    }

    /// 获取容量
    pub fn capacity(&self) -> u32 {
        self.cdrom.capacity()
    }

#[derive(Debug, Error)]
pub enum CdromError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("No ISO mounted")]
    NotMounted,
    #[error("Sector out of range")]
    SectorOutOfRange,
}
}

impl MmioDevice for VirtioCdrom {
    fn read(&self, offset: u64, _size: u8) -> u64 {
        match offset {
            0x00 => 0x74726976, // Magic "virt"
            0x04 => 2,          // Version
            0x08 => 2,          // Device ID (Block)
            0x0C => 0x554D4551, // Vendor ID
            0x10 => 0x00000021, // Device features (low): RO | BLK_SIZE
            0x14 => 0x00000001, // Device features (high): VIRTIO_F_VERSION_1
            0x70 => self.status as u64, // Status
            // 配置空间 (0x100+)
            0x100 => self.cdrom.capacity() as u64, // Capacity
            0x108 => 2048,      // Block size (ISO 9660)
            0x10C => 1,         // Read-only flag
            _ => 0,
        }
    }

    fn write(&mut self, offset: u64, val: u64, _size: u8) {
        match offset {
            0x30 => self.queue_sel = val as u32, // Queue select
            0x50 => {
                // Queue notify
                log::debug!("VirtioCdrom: Queue {} notified", val);
                // 这里应该处理队列请求
            }
            0x70 => self.status = val as u32, // Status
            _ => {
                log::trace!("VirtioCdrom write: offset={:#x} val={:#x}", offset, val);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cdrom_creation() {
        let cdrom = CdromDevice::new();
        
        assert_eq!(cdrom.capacity(), 0);
        assert_eq!(cdrom.status(), CdromStatus::Idle);
        assert!(!cdrom.is_mounted());
    }

    #[test]
    fn test_virtio_cdrom_creation() {
        let cdrom = VirtioCdrom::new();
        
        assert_eq!(cdrom.read(0x00, 4), 0x74726976); // Magic
        assert_eq!(cdrom.read(0x08, 4), 2); // Device ID
        assert_eq!(cdrom.capacity(), 0);
    }
}
