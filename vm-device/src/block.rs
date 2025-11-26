//! VirtIO Block Device Implementation
//!
//! 实现符合 VirtIO 规范的块设备，支持读写操作

use vm_core::{MMU, GuestAddr};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

/// VirtIO Block Device 特性标志
pub mod features {
    pub const VIRTIO_BLK_F_SIZE_MAX: u32 = 1;
    pub const VIRTIO_BLK_F_SEG_MAX: u32 = 2;
    pub const VIRTIO_BLK_F_GEOMETRY: u32 = 4;
    pub const VIRTIO_BLK_F_RO: u32 = 5;
    pub const VIRTIO_BLK_F_BLK_SIZE: u32 = 6;
    pub const VIRTIO_BLK_F_FLUSH: u32 = 9;
    pub const VIRTIO_BLK_F_TOPOLOGY: u32 = 10;
    pub const VIRTIO_BLK_F_CONFIG_WCE: u32 = 11;
}

/// VirtIO Block 请求类型
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockRequestType {
    In = 0,      // 读取
    Out = 1,     // 写入
    Flush = 4,   // 刷新
    GetId = 8,   // 获取设备ID
}

impl BlockRequestType {
    pub fn from_u32(val: u32) -> Option<Self> {
        match val {
            0 => Some(BlockRequestType::In),
            1 => Some(BlockRequestType::Out),
            4 => Some(BlockRequestType::Flush),
            8 => Some(BlockRequestType::GetId),
            _ => None,
        }
    }
}

/// VirtIO Block 请求状态
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockStatus {
    Ok = 0,
    IoErr = 1,
    Unsupported = 2,
}

/// VirtIO Block 请求头
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BlockRequestHeader {
    pub req_type: u32,
    pub reserved: u32,
    pub sector: u64,
}

/// VirtIO Block Device
pub struct VirtioBlock {
    /// 后端文件
    file: Option<File>,
    /// 设备容量（扇区数）
    capacity: u64,
    /// 扇区大小（字节）
    sector_size: u32,
    /// 是否只读
    read_only: bool,
}

impl VirtioBlock {
    /// 创建新的 VirtIO Block 设备
    pub fn new() -> Self {
        Self {
            file: None,
            capacity: 0,
            sector_size: 512,
            read_only: false,
        }
    }

    /// 从文件路径打开块设备
    pub fn open<P: AsRef<Path>>(path: P, read_only: bool) -> std::io::Result<Self> {
        let file = if read_only {
            File::open(path.as_ref())?
        } else {
            std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(path.as_ref())?
        };

        let metadata = file.metadata()?;
        let capacity = metadata.len() / 512; // 扇区数

        Ok(Self {
            file: Some(file),
            capacity,
            sector_size: 512,
            read_only,
        })
    }

    /// 获取设备容量（扇区数）
    pub fn capacity(&self) -> u64 {
        self.capacity
    }

    /// 获取扇区大小
    pub fn sector_size(&self) -> u32 {
        self.sector_size
    }

    /// 获取设备特性
    pub fn features(&self) -> u32 {
        let mut features = 0u32;
        features |= 1 << features::VIRTIO_BLK_F_BLK_SIZE;
        features |= 1 << features::VIRTIO_BLK_F_FLUSH;
        
        if self.read_only {
            features |= 1 << features::VIRTIO_BLK_F_RO;
        }
        
        features
    }

    /// 处理块设备请求
    pub fn handle_request(
        &mut self,
        mmu: &mut dyn MMU,
        req_addr: GuestAddr,
        data_addr: GuestAddr,
        data_len: u32,
        status_addr: GuestAddr,
    ) -> BlockStatus {
        // 读取请求头
        let req_type = match mmu.read(req_addr, 4) {
            Ok(v) => v as u32,
            Err(_) => return BlockStatus::IoErr,
        };

        let sector = match mmu.read(req_addr + 8, 8) {
            Ok(v) => v,
            Err(_) => return BlockStatus::IoErr,
        };

        let req_type = match BlockRequestType::from_u32(req_type) {
            Some(t) => t,
            None => return BlockStatus::Unsupported,
        };

        let status = match req_type {
            BlockRequestType::In => self.handle_read(mmu, sector, data_addr, data_len),
            BlockRequestType::Out => self.handle_write(mmu, sector, data_addr, data_len),
            BlockRequestType::Flush => self.handle_flush(),
            BlockRequestType::GetId => BlockStatus::Unsupported,
        };

        // 写入状态
        let _ = mmu.write(status_addr, status as u64, 1);
        status
    }

    /// 处理读请求
    fn handle_read(
        &mut self,
        mmu: &mut dyn MMU,
        sector: u64,
        data_addr: GuestAddr,
        data_len: u32,
    ) -> BlockStatus {
        let file = match &mut self.file {
            Some(f) => f,
            None => return BlockStatus::IoErr,
        };

        // 计算字节偏移
        let offset = sector * (self.sector_size as u64);
        
        if let Err(_) = file.seek(SeekFrom::Start(offset)) {
            return BlockStatus::IoErr;
        }

        // 读取数据到临时缓冲区
        let mut buffer = vec![0u8; data_len as usize];
        match file.read_exact(&mut buffer) {
            Ok(_) => {},
            Err(_) => return BlockStatus::IoErr,
        }

        // 写入到 Guest 内存
        for (i, &byte) in buffer.iter().enumerate() {
            if let Err(_) = mmu.write(data_addr + i as u64, byte as u64, 1) {
                return BlockStatus::IoErr;
            }
        }

        BlockStatus::Ok
    }

    /// 处理写请求
    fn handle_write(
        &mut self,
        mmu: &dyn MMU,
        sector: u64,
        data_addr: GuestAddr,
        data_len: u32,
    ) -> BlockStatus {
        if self.read_only {
            return BlockStatus::IoErr;
        }

        let file = match &mut self.file {
            Some(f) => f,
            None => return BlockStatus::IoErr,
        };

        // 计算字节偏移
        let offset = sector * (self.sector_size as u64);
        
        if let Err(_) = file.seek(SeekFrom::Start(offset)) {
            return BlockStatus::IoErr;
        }

        // 从 Guest 内存读取数据
        let mut buffer = vec![0u8; data_len as usize];
        for i in 0..data_len as usize {
            match mmu.read(data_addr + i as u64, 1) {
                Ok(v) => buffer[i] = v as u8,
                Err(_) => return BlockStatus::IoErr,
            }
        }

        // 写入到文件
        match file.write_all(&buffer) {
            Ok(_) => BlockStatus::Ok,
            Err(_) => BlockStatus::IoErr,
        }
    }

    /// 处理刷新请求
    fn handle_flush(&mut self) -> BlockStatus {
        if let Some(file) = &mut self.file {
            match file.sync_all() {
                Ok(_) => BlockStatus::Ok,
                Err(_) => BlockStatus::IoErr,
            }
        } else {
            BlockStatus::IoErr
        }
    }
}

/// VirtIO Block MMIO 设备
pub struct VirtioBlockMmio {
    /// 块设备实例
    pub device: VirtioBlock,
    /// 当前选中的队列索引
    selected_queue: u32,
    /// 队列大小
    queue_size: u32,
    /// 描述符表地址
    desc_addr: GuestAddr,
    /// Available Ring 地址
    avail_addr: GuestAddr,
    /// Used Ring 地址
    used_addr: GuestAddr,
    /// 设备状态
    device_status: u32,
    /// 驱动特性
    driver_features: u32,
    /// 中断状态
    interrupt_status: u32,
    /// Used Ring 索引
    used_idx: u16,
}

impl VirtioBlockMmio {
    pub fn new(device: VirtioBlock) -> Self {
        Self {
            device,
            selected_queue: 0,
            queue_size: 128,
            desc_addr: 0,
            avail_addr: 0,
            used_addr: 0,
            device_status: 0,
            driver_features: 0,
            interrupt_status: 0,
            used_idx: 0,
        }
    }

    /// 处理队列通知
    pub fn handle_queue_notify(&mut self, mmu: &mut dyn MMU, _queue_idx: u32) {
        if self.avail_addr == 0 || self.desc_addr == 0 || self.used_addr == 0 {
            return;
        }

        // 读取 available ring 的 idx
        let avail_idx = match mmu.read(self.avail_addr + 2, 2) {
            Ok(v) => v as u16,
            Err(_) => return,
        };

        // 处理所有待处理的请求
        while self.used_idx != avail_idx {
            let ring_idx = self.used_idx % self.queue_size as u16;
            let desc_idx_addr = self.avail_addr + 4 + (ring_idx as u64 * 2);
            
            let desc_idx = match mmu.read(desc_idx_addr, 2) {
                Ok(v) => v as u16,
                Err(_) => break,
            };

            // 处理描述符链
            self.process_descriptor_chain(mmu, desc_idx);

            self.used_idx = self.used_idx.wrapping_add(1);
        }

        // 设置中断
        self.interrupt_status |= 0x1;
    }

    /// 处理描述符链
    fn process_descriptor_chain(&mut self, mmu: &mut dyn MMU, desc_idx: u16) {
        const DESC_SIZE: u64 = 16;
        
        let desc_base = self.desc_addr + (desc_idx as u64 * DESC_SIZE);
        
        // 读取第一个描述符（请求头）
        let req_addr = match mmu.read(desc_base, 8) {
            Ok(v) => v,
            Err(_) => return,
        };
        
        let _req_len = match mmu.read(desc_base + 8, 4) {
            Ok(v) => v as u32,
            Err(_) => return,
        };
        
        let req_flags = match mmu.read(desc_base + 12, 2) {
            Ok(v) => v as u16,
            Err(_) => return,
        };
        
        let next_desc = match mmu.read(desc_base + 14, 2) {
            Ok(v) => v as u16,
            Err(_) => return,
        };

        // 如果有下一个描述符，读取数据描述符
        if req_flags & 0x1 != 0 {
            let data_desc_base = self.desc_addr + (next_desc as u64 * DESC_SIZE);
            
            let data_addr = match mmu.read(data_desc_base, 8) {
                Ok(v) => v,
                Err(_) => return,
            };
            
            let data_len = match mmu.read(data_desc_base + 8, 4) {
                Ok(v) => v as u32,
                Err(_) => return,
            };
            
            let data_flags = match mmu.read(data_desc_base + 12, 2) {
                Ok(v) => v as u16,
                Err(_) => return,
            };
            
            let status_desc = match mmu.read(data_desc_base + 14, 2) {
                Ok(v) => v as u16,
                Err(_) => return,
            };

            // 读取状态描述符
            if data_flags & 0x1 != 0 {
                let status_desc_base = self.desc_addr + (status_desc as u64 * DESC_SIZE);
                let status_addr = match mmu.read(status_desc_base, 8) {
                    Ok(v) => v,
                    Err(_) => return,
                };

                // 处理请求
                self.device.handle_request(mmu, req_addr, data_addr, data_len, status_addr);
            }
        }

        // 更新 used ring
        let used_ring_idx = self.used_idx % self.queue_size as u16;
        let used_elem_addr = self.used_addr + 4 + (used_ring_idx as u64 * 8);
        
        let _ = mmu.write(used_elem_addr, desc_idx as u64, 4);
        let _ = mmu.write(used_elem_addr + 4, 0, 4); // len
        
        // 更新 used idx
        let _ = mmu.write(self.used_addr + 2, self.used_idx as u64, 2);
    }

    /// MMIO 读操作
    pub fn read(&self, offset: u64, _size: u8) -> u64 {
        match offset {
            0x000 => 0x74726976, // Magic: "virt"
            0x004 => 0x2,        // Version
            0x008 => 0x2,        // Device ID: block device
            0x00C => 0x554d4551, // Vendor ID: "QEMU"
            0x010 => self.device.features() as u64,
            0x034 => self.queue_size as u64,
            0x044 => self.device_status as u64,
            0x060 => self.interrupt_status as u64,
            0x100 => self.device.capacity(),
            0x108 => self.device.sector_size() as u64,
            _ => 0,
        }
    }

    /// MMIO 写操作
    pub fn write(&mut self, offset: u64, val: u64, _size: u8) {
        match offset {
            0x014 => self.driver_features = val as u32,
            0x030 => self.selected_queue = val as u32,
            0x038 => self.queue_size = val as u32,
            0x044 => self.device_status = val as u32,
            0x050 => {
                // Queue notify
                self.interrupt_status = 0; // 清除中断
            },
            0x064 => self.interrupt_status &= !(val as u32),
            0x080 => self.desc_addr = val,
            0x090 => self.avail_addr = val,
            0x0A0 => self.used_addr = val,
            _ => {}
        }
    }
}

impl vm_core::MmioDevice for VirtioBlockMmio {
    fn read(&self, offset: u64, size: u8) -> u64 {
        self.read(offset, size)
    }

    fn write(&mut self, offset: u64, val: u64, size: u8) {
        self.write(offset, val, size);
    }
}
