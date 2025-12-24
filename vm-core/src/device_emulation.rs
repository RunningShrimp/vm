//! 跨平台设备模拟
//!
//! 提供网络、磁盘和 GPU 设备的模拟接口

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use serde::{Deserialize, Serialize};

/// 设备类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeviceType {
    /// 网络设备
    Network,
    /// 磁盘设备
    Disk,
    /// GPU 设备
    Gpu,
    /// 输入设备（键盘、鼠标）
    Input,
    /// 输出设备（显示、音频）
    Output,
    /// 串口设备
    Serial,
    /// 并口设备
    Parallel,
    /// USB 设备
    Usb,
}

/// 设备接口
pub trait Device: Send + Sync {
    fn device_type(&self) -> DeviceType;

    fn device_id(&self) -> &str;

    fn read(&mut self, offset: u64, size: usize) -> Result<Vec<u8>, DeviceError>;

    fn write(&mut self, offset: u64, data: &[u8]) -> Result<(), DeviceError>;

    fn reset(&mut self);

    fn io_notify(&mut self, event: IoEvent);
}

/// 设备错误
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceError {
    NotReady,
    Busy,
    InvalidOffset { offset: u64 },
    InvalidSize { size: usize },
    AccessViolation,
    HardwareError { message: String },
}

/// I/O 事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IoEvent {
    /// 读取完成
    ReadComplete { bytes: usize },
    /// 写入完成
    WriteComplete { bytes: usize },
    /// 错误
    Error { error: DeviceError },
    /// 中断
    Interrupt { vector: u32 },
    /// 状态变化
    StatusChange { status: DeviceStatus },
}

/// 设备状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceStatus {
    /// 未初始化
    Uninitialized,
    /// 空闲
    Idle,
    /// 忙碌
    Busy,
    /// 错误
    Error,
    /// 就绪
    Ready,
}

/// 网络设备配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// MAC 地址
    pub mac_address: [u8; 6],
    /// MTU（最大传输单元）
    pub mtu: u16,
    /// 是否启用混杂模式
    pub promiscuous_mode: bool,
    /// 队列大小
    pub queue_size: usize,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            mac_address: [0x52, 0x54, 0x00, 0x12, 0x34, 0x56],
            mtu: 1500,
            promiscuous_mode: false,
            queue_size: 256,
        }
    }
}

/// 网络包
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPacket {
    /// 源 MAC 地址
    pub src_mac: [u8; 6],
    /// 目的 MAC 地址
    pub dst_mac: [u8; 6],
    /// 数据
    pub data: Vec<u8>,
    /// 时间戳
    pub timestamp: u64,
}

/// 虚拟网络设备
pub struct VirtualNetworkDevice {
    config: NetworkConfig,
    status: DeviceStatus,
    rx_queue: VecDeque<NetworkPacket>,
    tx_queue: VecDeque<NetworkPacket>,
    stats: NetworkStats,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkStats {
    pub rx_packets: u64,
    pub tx_packets: u64,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_errors: u64,
    pub tx_errors: u64,
    pub rx_dropped: u64,
}

impl VirtualNetworkDevice {
    pub fn new(config: NetworkConfig) -> Self {
        Self {
            config,
            status: DeviceStatus::Uninitialized,
            rx_queue: VecDeque::new(),
            tx_queue: VecDeque::new(),
            stats: NetworkStats::default(),
        }
    }

    pub fn initialize(&mut self) {
        self.status = DeviceStatus::Idle;
    }

    pub fn receive_packet(&mut self, packet: NetworkPacket) {
        if self.rx_queue.len() >= self.config.queue_size {
            self.stats.rx_dropped += 1;
            return;
        }
        let data_len = packet.data.len() as u64;
        self.rx_queue.push_back(packet);
        self.stats.rx_packets += 1;
        self.stats.rx_bytes += data_len;
    }

    pub fn send_packet(&mut self, packet: NetworkPacket) -> Result<(), DeviceError> {
        if self.tx_queue.len() >= self.config.queue_size {
            return Err(DeviceError::Busy);
        }
        let data_len = packet.data.len() as u64;
        self.tx_queue.push_back(packet);
        self.stats.tx_packets += 1;
        self.stats.tx_bytes += data_len;
        Ok(())
    }

    pub fn get_stats(&self) -> &NetworkStats {
        &self.stats
    }
}

impl Device for VirtualNetworkDevice {
    fn device_type(&self) -> DeviceType {
        DeviceType::Network
    }

    fn device_id(&self) -> &str {
        "virtio-net"
    }

    fn read(&mut self, offset: u64, size: usize) -> Result<Vec<u8>, DeviceError> {
        match offset {
            0x00 => Ok(vec![self.status as u8]),
            0x01..=0x10 => {
                let mut data = vec![0u8; size];
                for i in 0..size.min(6) {
                    if offset + i as u64 >= 0x02 && (offset + i as u64) < 0x08 {
                        data[i] = self.config.mac_address[i];
                    }
                }
                Ok(data)
            }
            _ => Err(DeviceError::InvalidOffset { offset }),
        }
    }

    fn write(&mut self, offset: u64, data: &[u8]) -> Result<(), DeviceError> {
        match offset {
            0x02..=0x07 => {
                for (i, &byte) in data.iter().enumerate() {
                    if (offset + i as u64) < 0x08 {
                        self.config.mac_address[(offset - 0x02) as usize + i] = byte;
                    }
                }
                Ok(())
            }
            _ => Err(DeviceError::InvalidOffset { offset }),
        }
    }

    fn reset(&mut self) {
        self.rx_queue.clear();
        self.tx_queue.clear();
        self.stats = NetworkStats::default();
        self.status = DeviceStatus::Idle;
    }

    fn io_notify(&mut self, event: IoEvent) {
        match event {
            IoEvent::Interrupt { .. } => {
                let _ = self.tx_queue.pop_front();
                self.status = DeviceStatus::Busy;
            }
            IoEvent::StatusChange { status } => {
                self.status = status;
            }
            _ => {}
        }
    }
}

/// 磁盘设备配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskConfig {
    /// 容量（字节）
    pub capacity: u64,
    /// 扇区大小（字节）
    pub sector_size: u32,
    /// 只读模式
    pub read_only: bool,
    /// 是否可移除
    pub removable: bool,
}

impl Default for DiskConfig {
    fn default() -> Self {
        Self {
            capacity: 1 * 1024 * 1024 * 1024, // 1 GB
            sector_size: 512,
            read_only: false,
            removable: false,
        }
    }
}

/// 虚拟磁盘设备
pub struct VirtualDiskDevice {
    config: DiskConfig,
    status: DeviceStatus,
    data: Vec<u8>,
    stats: DiskStats,
    last_io_time: Option<Instant>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiskStats {
    pub read_requests: u64,
    pub write_requests: u64,
    pub read_bytes: u64,
    pub write_bytes: u64,
    pub read_time_ms: u64,
    pub write_time_ms: u64,
    pub errors: u64,
}

impl VirtualDiskDevice {
    pub fn new(config: DiskConfig) -> Self {
        Self {
            config: config.clone(),
            status: DeviceStatus::Uninitialized,
            data: vec![0u8; config.capacity as usize],
            stats: DiskStats::default(),
            last_io_time: None,
        }
    }

    pub fn initialize(&mut self) {
        self.status = DeviceStatus::Idle;
    }

    pub fn format(&mut self) {
        self.data = vec![0u8; self.config.capacity as usize];
    }

    pub fn get_stats(&self) -> &DiskStats {
        &self.stats
    }
}

impl Device for VirtualDiskDevice {
    fn device_type(&self) -> DeviceType {
        DeviceType::Disk
    }

    fn device_id(&self) -> &str {
        "virtio-blk"
    }

    fn read(&mut self, offset: u64, size: usize) -> Result<Vec<u8>, DeviceError> {
        if self.status == DeviceStatus::Busy {
            return Err(DeviceError::Busy);
        }

        let start = offset as usize;
        let end = (start + size).min(self.data.len());

        if offset as usize >= self.data.len() {
            return Err(DeviceError::InvalidOffset { offset });
        }

        self.status = DeviceStatus::Busy;
        let start_time = Instant::now();

        let data = self.data[start..end].to_vec();

        self.stats.read_requests += 1;
        self.stats.read_bytes += size as u64;
        self.last_io_time = Some(start_time);

        Ok(data)
    }

    fn write(&mut self, offset: u64, data: &[u8]) -> Result<(), DeviceError> {
        if self.config.read_only {
            return Err(DeviceError::AccessViolation);
        }

        if self.status == DeviceStatus::Busy {
            return Err(DeviceError::Busy);
        }

        let start = offset as usize;
        let end = (start + data.len()).min(self.data.len());

        if offset as usize >= self.data.len() {
            return Err(DeviceError::InvalidOffset { offset });
        }

        self.status = DeviceStatus::Busy;
        let start_time = Instant::now();

        self.data[start..end].copy_from_slice(data);

        self.stats.write_requests += 1;
        self.stats.write_bytes += data.len() as u64;
        self.last_io_time = Some(start_time);

        Ok(())
    }

    fn reset(&mut self) {
        self.status = DeviceStatus::Idle;
        self.last_io_time = None;
    }

    fn io_notify(&mut self, event: IoEvent) {
        match event {
            IoEvent::ReadComplete { .. } | IoEvent::WriteComplete { .. } => {
                self.status = DeviceStatus::Idle;
            }
            IoEvent::Error { .. } => {
                self.status = DeviceStatus::Error;
                self.stats.errors += 1;
            }
            _ => {}
        }
    }
}

/// GPU 设备配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuConfig {
    /// 显存大小（字节）
    pub vram_size: u64,
    /// 分辨率宽度
    pub width: u32,
    /// 分辨率高度
    pub height: u32,
    /// 色深（位）
    pub color_depth: u8,
}

impl Default for GpuConfig {
    fn default() -> Self {
        Self {
            vram_size: 16 * 1024 * 1024, // 16 MB
            width: 1024,
            height: 768,
            color_depth: 32,
        }
    }
}

/// 虚拟 GPU 设备
pub struct VirtualGpuDevice {
    config: GpuConfig,
    status: DeviceStatus,
    vram: Vec<u8>,
    framebuffer: Vec<u8>,
    cursor_position: (u32, u32),
    stats: GpuStats,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GpuStats {
    pub frames_rendered: u64,
    pub pixels_written: u64,
    pub vram_usage: u64,
}

impl VirtualGpuDevice {
    pub fn new(config: GpuConfig) -> Self {
        let bytes_per_pixel = (config.color_depth / 8) as u32;
        Self {
            config: config.clone(),
            status: DeviceStatus::Uninitialized,
            vram: vec![0u8; config.vram_size as usize],
            framebuffer: vec![0u8; (config.width * config.height * bytes_per_pixel) as usize],
            cursor_position: (0, 0),
            stats: GpuStats::default(),
        }
    }

    pub fn initialize(&mut self) {
        self.status = DeviceStatus::Idle;
    }

    pub fn set_mode(&mut self, width: u32, height: u32, color_depth: u8) {
        self.config.width = width;
        self.config.height = height;
        self.config.color_depth = color_depth;
        let bytes_per_pixel = (color_depth / 8) as u32;
        self.framebuffer = vec![0u8; (width * height * bytes_per_pixel) as usize];
    }

    pub fn write_pixel(&mut self, x: u32, y: u32, color: u32) {
        if x >= self.config.width || y >= self.config.height {
            return;
        }

        let bytes_per_pixel = (self.config.color_depth / 8) as usize;
        let offset = (y * self.config.width + x) as usize * bytes_per_pixel;

        let color_bytes = color.to_le_bytes();
        for i in 0..bytes_per_pixel {
            if offset + i < self.framebuffer.len() {
                self.framebuffer[offset + i] = color_bytes[i];
            }
        }

        self.stats.pixels_written += 1;
    }

    pub fn set_cursor(&mut self, x: u32, y: u32) {
        self.cursor_position = (x, y);
    }

    pub fn get_framebuffer(&self) -> &[u8] {
        &self.framebuffer
    }

    pub fn get_stats(&self) -> &GpuStats {
        &self.stats
    }
}

impl Device for VirtualGpuDevice {
    fn device_type(&self) -> DeviceType {
        DeviceType::Gpu
    }

    fn device_id(&self) -> &str {
        "virtio-gpu"
    }

    fn read(&mut self, offset: u64, size: usize) -> Result<Vec<u8>, DeviceError> {
        match offset {
            0x00 => Ok(vec![self.status as u8]),
            0x10..=0x1F => {
                let mut data = vec![0u8; size];
                for i in 0..size {
                    if (offset + i as u64) < self.vram.len() as u64 {
                        data[i] = self.vram[(offset + i as u64) as usize];
                    }
                }
                Ok(data)
            }
            _ => Err(DeviceError::InvalidOffset { offset }),
        }
    }

    fn write(&mut self, offset: u64, data: &[u8]) -> Result<(), DeviceError> {
        match offset {
            0x10..=0x1F => {
                for (i, &byte) in data.iter().enumerate() {
                    let addr = (offset + i as u64) as usize;
                    if addr < self.vram.len() {
                        self.vram[addr] = byte;
                    }
                }
                self.stats.vram_usage = self.vram.iter().filter(|&&b| b != 0).count() as u64;
                Ok(())
            }
            _ => Err(DeviceError::InvalidOffset { offset }),
        }
    }

    fn reset(&mut self) {
        self.framebuffer.fill(0);
        self.cursor_position = (0, 0);
        self.status = DeviceStatus::Idle;
    }

    fn io_notify(&mut self, event: IoEvent) {
        match event {
            IoEvent::StatusChange { status } => {
                self.status = status;
            }
            _ => {}
        }
    }
}

/// 设备管理器
pub struct DeviceManager {
    devices: Vec<Arc<Mutex<Box<dyn Device>>>>,
    next_device_id: u32,
}

impl DeviceManager {
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
            next_device_id: 0,
        }
    }

    pub fn add_device(&mut self, device: Box<dyn Device>) -> u32 {
        let id = self.next_device_id;
        self.devices.push(Arc::new(Mutex::new(device)));
        self.next_device_id += 1;
        id
    }

    pub fn get_device(&self, device_id: &str) -> Option<Arc<Mutex<Box<dyn Device>>>> {
        for device in &self.devices {
            let d = device.lock().unwrap();
            if d.device_id() == device_id {
                drop(d);
                return Some(Arc::clone(device));
            }
        }
        None
    }

    pub fn get_devices_by_type(&self, device_type: DeviceType) -> Vec<Arc<Mutex<Box<dyn Device>>>> {
        self.devices
            .iter()
            .filter(|d| {
                let device = d.lock().unwrap();
                device.device_type() == device_type
            })
            .cloned()
            .collect()
    }

    pub fn reset_all(&mut self) {
        for device in &self.devices {
            let mut d = device.lock().unwrap();
            d.reset();
        }
    }

    pub fn notify_all(&mut self, event: IoEvent) {
        for device in &self.devices {
            let mut d = device.lock().unwrap();
            d.io_notify(event.clone());
        }
    }
}

impl Default for DeviceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_device() {
        let config = NetworkConfig::default();
        let mut device = VirtualNetworkDevice::new(config);

        device.initialize();

        let packet = NetworkPacket {
            src_mac: [0x52, 0x54, 0x00, 0x12, 0x34, 0x56],
            dst_mac: [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
            data: vec![0xDE, 0xAD, 0xBE, 0xEF],
            timestamp: 0,
        };

        device.receive_packet(packet.clone());

        assert_eq!(device.stats.rx_packets, 1);
        assert_eq!(device.stats.rx_bytes, 4);
    }

    #[test]
    fn test_disk_device() {
        let config = DiskConfig::default();
        let mut device = VirtualDiskDevice::new(config);

        device.initialize();

        let data = vec![0xAB, 0xCD, 0xEF];
        device.write(0, &data).unwrap();

        let read_data = device.read(0, 3).unwrap();
        assert_eq!(read_data, data);
    }

    #[test]
    fn test_gpu_device() {
        let config = GpuConfig::default();
        let mut device = VirtualGpuDevice::new(config);

        device.initialize();
        device.write_pixel(100, 200, 0xFF0000);

        let framebuffer = device.get_framebuffer();
        assert!(framebuffer.len() > 0);
    }

    #[test]
    fn test_device_manager() {
        let mut manager = DeviceManager::new();

        let net_device = Box::new(VirtualNetworkDevice::new(NetworkConfig::default())) as Box<dyn Device>;
        let disk_device = Box::new(VirtualDiskDevice::new(DiskConfig::default())) as Box<dyn Device>;

        manager.add_device(net_device);
        manager.add_device(disk_device);

        assert_eq!(manager.get_devices_by_type(DeviceType::Network).len(), 1);
        assert_eq!(manager.get_devices_by_type(DeviceType::Disk).len(), 1);
    }
}
