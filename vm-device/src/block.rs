//! VirtIO Block Device Implementation
//!
//! 实现符合 VirtIO 规范的块设备，支持读写操作

use vm_core::{MMU, GuestAddr};
use vm_mem::SoftMmu;
use crate::mmu_util::MmuUtil;
use std::path::Path;
use tokio::sync::mpsc;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use std::sync::{Arc, Mutex};
use std::io::SeekFrom;

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

/// 异步 IO 请求
#[derive(Debug)]
pub enum BlockIoRequest {
    Read { sector: u64, count: u32, req_id: u64 },
    Write { sector: u64, data: Vec<u8>, req_id: u64 },
    Flush { req_id: u64 },
}

/// 异步 IO 响应
#[derive(Debug)]
pub enum BlockIoResponse {
    ReadOk { data: Vec<u8>, req_id: u64 },
    WriteOk { req_id: u64 },
    FlushOk { req_id: u64 },
    Error { req_id: u64, msg: String },
}

/// VirtIO Block Device
pub struct VirtioBlock {
    tx: mpsc::Sender<BlockIoRequest>,
    rx: Arc<Mutex<mpsc::Receiver<BlockIoResponse>>>,
    /// 设备容量（扇区数）
    capacity: u64,
    /// 扇区大小（字节）
    sector_size: u32,
    /// 是否只读
    read_only: bool,
    /// 下一个请求ID
    next_req_id: u64,
}

impl VirtioBlock {
    /// 创建新的 VirtIO Block 设备
    pub fn new() -> Self {
        let (tx, _) = mpsc::channel(1);
        let (_, rx) = mpsc::channel(1);
        Self {
            tx,
            rx: Arc::new(Mutex::new(rx)),
            capacity: 0,
            sector_size: 512,
            read_only: false,
            next_req_id: 0,
        }
    }

    /// 从文件路径打开块设备 (异步)
    pub async fn open<P: AsRef<Path>>(path: P, read_only: bool) -> std::io::Result<Self> {
        let file = if read_only {
            File::open(path.as_ref()).await?
        } else {
            tokio::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(path.as_ref())
                .await?
        };

        let metadata = file.metadata().await?;
        let capacity = metadata.len() / 512; // 扇区数
        let sector_size = 512;

        let (req_tx, mut req_rx) = mpsc::channel::<BlockIoRequest>(100);
        let (resp_tx, resp_rx) = mpsc::channel::<BlockIoResponse>(100);
        
        let mut backend_file = file;

        tokio::spawn(async move {
            while let Some(req) = req_rx.recv().await {
                match req {
                    BlockIoRequest::Read { sector, count, req_id } => {
                        let offset = sector * 512;
                        if let Err(_) = backend_file.seek(std::io::SeekFrom::Start(offset)).await {
                            let _ = resp_tx.send(BlockIoResponse::Error { req_id, msg: "Seek failed".into() }).await;
                            continue;
                        }
                        let mut buf = vec![0u8; count as usize];
                        match backend_file.read_exact(&mut buf).await {
                            Ok(_) => {
                                let _ = resp_tx.send(BlockIoResponse::ReadOk { data: buf, req_id }).await;
                            }
                            Err(e) => {
                                let _ = resp_tx.send(BlockIoResponse::Error { req_id, msg: e.to_string() }).await;
                            }
                        }
                    }
                    BlockIoRequest::Write { sector, data, req_id } => {
                        if read_only {
                            let _ = resp_tx.send(BlockIoResponse::Error { req_id, msg: "Read only".into() }).await;
                            continue;
                        }
                        let offset = sector * 512;
                        if let Err(_) = backend_file.seek(std::io::SeekFrom::Start(offset)).await {
                            let _ = resp_tx.send(BlockIoResponse::Error { req_id, msg: "Seek failed".into() }).await;
                            continue;
                        }
                        match backend_file.write_all(&data).await {
                            Ok(_) => {
                                let _ = resp_tx.send(BlockIoResponse::WriteOk { req_id }).await;
                            }
                            Err(e) => {
                                let _ = resp_tx.send(BlockIoResponse::Error { req_id, msg: e.to_string() }).await;
                            }
                        }
                    }
                    BlockIoRequest::Flush { req_id } => {
                        match backend_file.sync_all().await {
                            Ok(_) => {
                                let _ = resp_tx.send(BlockIoResponse::FlushOk { req_id }).await;
                            }
                            Err(e) => {
                                let _ = resp_tx.send(BlockIoResponse::Error { req_id, msg: e.to_string() }).await;
                            }
                        }
                    }
                }
            }
        });

        Ok(Self {
            tx: req_tx,
            rx: Arc::new(Mutex::new(resp_rx)),
            capacity,
            sector_size,
            read_only,
            next_req_id: 0,
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
        let req_id = self.next_req_id;
        self.next_req_id += 1;

        if let Err(_) = self.tx.blocking_send(BlockIoRequest::Read {
            sector,
            count: data_len,
            req_id,
        }) {
            return BlockStatus::IoErr;
        }

        let mut rx = self.rx.lock().unwrap();
        match rx.blocking_recv() {
            Some(BlockIoResponse::ReadOk { data, req_id: resp_id }) => {
                if resp_id != req_id {
                    return BlockStatus::IoErr;
                }
                if let Some(sm) = mmu.as_any_mut().downcast_mut::<SoftMmu>() {
                    if let Some(slice) = sm.guest_slice_mut(data_addr, data_len as usize) {
                        slice.copy_from_slice(&data);
                    } else {
                        return BlockStatus::IoErr;
                    }
                } else if let Err(_) = MmuUtil::write_slice(mmu, data_addr, &data) {
                    return BlockStatus::IoErr;
                }
                BlockStatus::Ok
            }
            _ => BlockStatus::IoErr,
        }
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

        let mut buffer = vec![0u8; data_len as usize];
        if let Some(sm) = mmu.as_any().downcast_ref::<SoftMmu>() {
            if let Some(slice) = sm.guest_slice(data_addr, data_len as usize) {
                buffer.copy_from_slice(slice);
            } else {
                return BlockStatus::IoErr;
            }
        } else if let Err(_) = MmuUtil::read_slice(mmu, data_addr, &mut buffer) {
            return BlockStatus::IoErr;
        }

        let req_id = self.next_req_id;
        self.next_req_id += 1;

        if let Err(_) = self.tx.blocking_send(BlockIoRequest::Write {
            sector,
            data: buffer,
            req_id,
        }) {
            return BlockStatus::IoErr;
        }

        let mut rx = self.rx.lock().unwrap();
        match rx.blocking_recv() {
            Some(BlockIoResponse::WriteOk { req_id: resp_id }) => {
                if resp_id != req_id {
                    return BlockStatus::IoErr;
                }
                BlockStatus::Ok
            }
            _ => BlockStatus::IoErr,
        }
    }

    /// 处理刷新请求
    fn handle_flush(&mut self) -> BlockStatus {
        let req_id = self.next_req_id;
        self.next_req_id += 1;

        if let Err(_) = self.tx.blocking_send(BlockIoRequest::Flush { req_id }) {
            return BlockStatus::IoErr;
        }

        let mut rx = self.rx.lock().unwrap();
        match rx.blocking_recv() {
            Some(BlockIoResponse::FlushOk { req_id: resp_id }) => {
                if resp_id != req_id {
                    return BlockStatus::IoErr;
                }
                BlockStatus::Ok
            }
            _ => BlockStatus::IoErr,
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
        /// 事件原因寄存器（扩展）
        cause_evt: u64,
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
            cause_evt: 0,
        }
    }

    pub fn new_with_capacity(capacity_sectors: u64) -> Self {
        let (tx, _) = mpsc::channel(1);
        let (_, rx) = mpsc::channel(1);
        let dev = VirtioBlock { 
            tx,
            rx: Arc::new(Mutex::new(rx)),
            capacity: capacity_sectors, 
            sector_size: 512, 
            read_only: false,
            next_req_id: 0,
        };
        Self::new(dev)
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

        // 处理所有待处理的请求（批量，不在每次迭代写 used_idx）
        let mut processed = 0u16;
        while self.used_idx.wrapping_add(processed) != avail_idx {
            let ring_idx = (self.used_idx.wrapping_add(processed)) % self.queue_size as u16;
            let desc_idx_addr = self.avail_addr + 4 + (ring_idx as u64 * 2);
            
            let desc_idx = match mmu.read(desc_idx_addr, 2) {
                Ok(v) => v as u16,
                Err(_) => break,
            };

            // 处理描述符链
            self.process_descriptor_chain(mmu, desc_idx);
            processed = processed.wrapping_add(1);
        }

        if processed != 0 {
            self.used_idx = self.used_idx.wrapping_add(processed);
            let _ = mmu.write(self.used_addr + 2, self.used_idx as u64, 2);
        }

        // 设置中断
        self.interrupt_status |= 0x1;
        let q = self.selected_queue as u64;
        self.cause_evt |= 1u64 << q;           // notify
        self.cause_evt |= 1u64 << (32 + q);    // idx match
        self.cause_evt |= 1u64 << (16 + q);    // wake
    }

    /// 处理描述符链
    fn process_descriptor_chain(&mut self, mmu: &mut dyn MMU, desc_idx: u16) {
        const DESC_SIZE: u64 = 16;
        
        // 遍历描述符链，累计长度并找到状态段
        let mut cur = desc_idx;
        let mut total_len = 0u32;
        let mut status_addr: Option<u64> = None;
        let mut budget = self.queue_size.max(8);
        loop {
            if budget == 0 { break; }
            budget -= 1;
            let base = self.desc_addr + (cur as u64 * DESC_SIZE);
            let addr = match mmu.read(base + 0, 8) { Ok(v) => v, Err(_) => break };
            let len = match mmu.read(base + 8, 4) { Ok(v) => v as u32, Err(_) => break };
            let flags = match mmu.read(base + 12, 2) { Ok(v) => v as u16, Err(_) => break };
            let next = match mmu.read(base + 14, 2) { Ok(v) => v as u16, Err(_) => 0 };

            // 约定：最后一个（可写）单字节段作为状态
            if len == 1 && (flags & 0x2) != 0 {
                status_addr = Some(addr);
            } else {
                total_len = total_len.wrapping_add(len);
            }

            if (flags & 0x1) != 0 { cur = next; } else { break; }
        }

        if let Some(sa) = status_addr { let _ = mmu.write(sa, BlockStatus::Ok as u64, 1); }

        // 更新 used ring
        let used_ring_idx = self.used_idx % self.queue_size as u16;
        let used_elem_addr = self.used_addr + 4 + (used_ring_idx as u64 * 8);
        
        let _ = mmu.write(used_elem_addr, desc_idx as u64, 4);
        let _ = mmu.write(used_elem_addr + 4, total_len as u64, 4);
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
            0x030 => self.interrupt_status as u64,
            0x048 => self.cause_evt,
            0x100 => self.device.capacity(),
            0x108 => self.device.sector_size() as u64,
            _ => 0,
        }
    }

    /// MMIO 写操作
    pub fn write(&mut self, offset: u64, val: u64, _size: u8) {
        match offset {
            0x00 => self.queue_size = val as u32,
            0x08 => self.desc_addr = val,
            0x10 => self.avail_addr = val,
            0x18 => self.used_addr = val,
            0x20 => { self.interrupt_status = 0; }
            0x2C => { self.selected_queue = val as u32; self.cause_evt |= 1u64 << (16 + self.selected_queue as u64); }
            0x34 => { self.interrupt_status = 0; self.cause_evt = 0; }
            0x014 => self.driver_features = val as u32,
            0x030 => self.selected_queue = val as u32,
            0x038 => self.queue_size = val as u32,
            0x044 => self.device_status = val as u32,
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

    fn notify(&mut self, mmu: &mut dyn vm_core::MMU, _offset: u64) {
        self.handle_queue_notify(mmu, self.selected_queue);
    }
}
