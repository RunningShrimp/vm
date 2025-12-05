//! VirtIO Block Device Implementation
//!
//! 本模块仅包含数据容器和enum定义。
//! 所有业务逻辑已移至 block_service.rs 的 BlockDeviceService

use crate::mmu_util::MmuUtil;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::sync::mpsc;
use vm_core::{GuestAddr, MMU, PlatformError, VmError};
use vm_mem::SoftMmu;

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
    In = 0,
    Out = 1,
    Flush = 4,
    GetId = 8,
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

/// VirtIO 设备事件位枚举（用于 cause_evt 路由）
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtioEvent {
    Notify = 0,
    Wake = 16,
    IndexMatch = 32,
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
    Read {
        sector: u64,
        count: u32,
        req_id: u64,
    },
    Write {
        sector: u64,
        data: Vec<u8>,
        req_id: u64,
    },
    Flush {
        req_id: u64,
    },
}

/// 异步 IO 响应
#[derive(Debug)]
pub enum BlockIoResponse {
    ReadOk { data: Vec<u8>, req_id: u64 },
    WriteOk { req_id: u64 },
    FlushOk { req_id: u64 },
    Error { req_id: u64, msg: String },
}

/// VirtIO Block Device - 数据容器（贫血模型）
#[derive(Clone)]
pub struct VirtioBlock {
    /// 设备容量（扇区数）
    pub capacity: u64,
    /// 扇区大小（通常为512）
    pub sector_size: u32,
    /// 是否只读
    pub read_only: bool,
}

impl Default for VirtioBlock {
    fn default() -> Self {
        Self {
            capacity: 0,
            sector_size: 512,
            read_only: false,
        }
    }
}

impl VirtioBlock {
    /// 创建新的VirtioBlock数据容器
    pub fn new(capacity: u64, sector_size: u32, read_only: bool) -> Self {
        Self {
            capacity,
            sector_size,
            read_only,
        }
    }
}

/// VirtIO Block MMIO 设备 - 数据容器（贫血模型）
#[derive(Clone)]
pub struct VirtioBlockMmio {
    /// 当前选中的队列索引
    pub selected_queue: u32,
    /// 队列大小
    pub queue_size: u32,
    /// 描述符表地址
    pub desc_addr: GuestAddr,
    /// Available Ring 地址
    pub avail_addr: GuestAddr,
    /// Used Ring 地址
    pub used_addr: GuestAddr,
    /// 设备状态
    pub device_status: u32,
    /// 驱动特性
    pub driver_features: u32,
    /// 中断状态
    pub interrupt_status: u32,
    /// Used Ring 索引
    pub used_idx: u16,
    /// 事件原因寄存器（扩展）
    pub cause_evt: u64,
}

impl Default for VirtioBlockMmio {
    fn default() -> Self {
        Self {
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
}

impl VirtioBlockMmio {
    /// 创建新的MMIO设备容器
    pub fn new() -> Self {
        Self::default()
    }

    /// 根据容量创建新的MMIO设备容器
    pub fn new_with_capacity(_capacity_sectors: u64) -> Self {
        Self::default()
    }
}

/// VirtIO Block MMIO读操作
pub fn mmio_read(mmio: &VirtioBlockMmio, _offset: u64, _size: u8) -> u64 {
    0 // 业务逻辑应在服务层处理
}

/// VirtIO Block MMIO写操作
pub fn mmio_write(mmio: &mut VirtioBlockMmio, _offset: u64, _val: u64, _size: u8) {
    // 业务逻辑应在服务层处理
}
