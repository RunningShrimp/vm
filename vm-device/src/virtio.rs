use vm_core::MmioDevice;
use crate::io::{IoScheduler, IoRequest, IoOpcode, ZeroCopyIo};
use std::sync::{Arc, Mutex};
use std::fs::File;
use std::io::{Read, Write, Seek, SeekFrom};

/// VirtIO Block 设备配置
pub struct VirtioBlockConfig {
    /// 容量（字节）
    pub capacity: u64,
    /// 块大小
    pub blk_size: u32,
    /// 最大段数
    pub seg_max: u32,
    /// 是否只读
    pub read_only: bool,
}

impl Default for VirtioBlockConfig {
    fn default() -> Self {
        Self {
            capacity: 0,
            blk_size: 512,
            seg_max: 128,
            read_only: false,
        }
    }
}

/// VirtIO Block 设备
pub struct VirtioBlock {
    /// 配置
    config: VirtioBlockConfig,
    /// 后端文件
    backend: Option<Arc<Mutex<File>>>,
    /// I/O 调度器
    io_scheduler: Option<Arc<Mutex<IoScheduler>>>,
    /// 设备状态
    status: u32,
    /// 队列选择器
    queue_sel: u32,
}

impl VirtioBlock {
    /// 创建内存后端的块设备
    pub fn new(size: usize) -> Self {
        Self {
            config: VirtioBlockConfig {
                capacity: size as u64,
                ..Default::default()
            },
            backend: None,
            io_scheduler: None,
            status: 0,
            queue_sel: 0,
        }
    }

    /// 创建文件后端的块设备
    pub fn with_file(file: File) -> std::io::Result<Self> {
        let capacity = file.metadata()?.len();
        Ok(Self {
            config: VirtioBlockConfig {
                capacity,
                ..Default::default()
            },
            backend: Some(Arc::new(Mutex::new(file))),
            io_scheduler: None,
            status: 0,
            queue_sel: 0,
        })
    }

    /// 设置 I/O 调度器
    pub fn set_io_scheduler(&mut self, scheduler: Arc<Mutex<IoScheduler>>) {
        self.io_scheduler = Some(scheduler);
    }

    /// 处理读请求
    pub fn handle_read(&mut self, offset: u64, length: usize) -> Result<Vec<u8>, String> {
        if let Some(backend) = &self.backend {
            let mut file = backend.lock().unwrap();
            file.seek(SeekFrom::Start(offset))
                .map_err(|e| format!("Seek failed: {}", e))?;
            
            let mut buffer = vec![0u8; length];
            file.read_exact(&mut buffer)
                .map_err(|e| format!("Read failed: {}", e))?;
            
            Ok(buffer)
        } else {
            Err("No backend configured".to_string())
        }
    }

    /// 处理写请求
    pub fn handle_write(&mut self, offset: u64, data: &[u8]) -> Result<(), String> {
        if self.config.read_only {
            return Err("Device is read-only".to_string());
        }

        if let Some(backend) = &self.backend {
            let mut file = backend.lock().unwrap();
            file.seek(SeekFrom::Start(offset))
                .map_err(|e| format!("Seek failed: {}", e))?;
            
            file.write_all(data)
                .map_err(|e| format!("Write failed: {}", e))?;
            
            Ok(())
        } else {
            Err("No backend configured".to_string())
        }
    }

    /// 处理刷新请求
    pub fn handle_flush(&mut self) -> Result<(), String> {
        if let Some(backend) = &self.backend {
            let mut file = backend.lock().unwrap();
            file.sync_all()
                .map_err(|e| format!("Flush failed: {}", e))?;
            Ok(())
        } else {
            Ok(())
        }
    }
}

impl MmioDevice for VirtioBlock {
    fn read(&self, offset: u64, _size: u8) -> u64 {
        match offset {
            0x00 => 0x74726976, // Magic "virt"
            0x04 => 2,          // Version
            0x08 => 2,          // Device ID (Block)
            0x0C => 0x554D4551, // Vendor ID
            0x10 => 0x00000001, // Device features (low)
            0x14 => 0x00000000, // Device features (high)
            0x70 => self.status as u64, // Status
            // 配置空间 (0x100+)
            0x100 => self.config.capacity & 0xFFFFFFFF,
            0x104 => (self.config.capacity >> 32) & 0xFFFFFFFF,
            0x108 => self.config.blk_size as u64,
            0x10C => self.config.seg_max as u64,
            _ => 0,
        }
    }

    fn write(&mut self, offset: u64, val: u64, _size: u8) {
        match offset {
            0x30 => self.queue_sel = val as u32, // Queue select
            0x50 => {
                // Queue notify
                log::debug!("VirtioBlock: Queue {} notified", val);
                // 这里应该处理队列请求
            }
            0x70 => self.status = val as u32, // Status
            _ => {
                log::trace!("VirtioBlock write: offset={:#x} val={:#x}", offset, val);
            }
        }
    }
}

pub struct VirtioNet {
    pub mac: [u8; 6],
}

impl VirtioNet {
    pub fn new() -> Self {
        Self {
            mac: [0x52, 0x54, 0x00, 0x12, 0x34, 0x56],
        }
    }
}

impl MmioDevice for VirtioNet {
    fn read(&self, offset: u64, _size: u8) -> u64 {
        match offset {
            0x00 => 0x74726976, // Magic "virt"
            0x04 => 2,          // Version
            0x08 => 1,          // Device ID (Net)
            0x0C => 0x554D4551, // Vendor ID
            _ => 0,
        }
    }

    fn write(&mut self, offset: u64, val: u64, _size: u8) {
        println!("VirtioNet write: offset={:#x} val={:#x}", offset, val);
    }
}
