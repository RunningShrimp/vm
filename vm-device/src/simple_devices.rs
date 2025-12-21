//! 简化设备实现
//!
//! 提供简化版的VirtIO设备实现，适用于快速原型和测试场景。
//! 这些实现提供了基本功能，但减少了复杂性和依赖。
//!
//! ## 使用方式
//!
//! 启用 "simple-devices" 特性来使用简化版本：
//! ```toml
//! [dependencies]
//! vm-device = { path = "../vm-device", features = ["simple-devices"] }
//! ```

use parking_lot::RwLock;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// 网络数据包
#[derive(Clone, Debug)]
pub struct NetworkPacket {
    pub data: Vec<u8>,
    pub timestamp: u64,
}

/// VirtIO-Net网络设备（简化版）
pub struct SimpleVirtioNetDevice {
    // 接收队列
    rx_queue: Arc<RwLock<VecDeque<NetworkPacket>>>,
    // 发送队列
    tx_queue: Arc<RwLock<VecDeque<NetworkPacket>>>,
    // 中断计数
    interrupt_count: Arc<AtomicU64>,
    // 统计信息
    rx_packets: Arc<AtomicU64>,
    tx_packets: Arc<AtomicU64>,
    rx_bytes: Arc<AtomicU64>,
    tx_bytes: Arc<AtomicU64>,
    // MAC地址
    mac_addr: [u8; 6],
    // 设备启用状态
    enabled: Arc<RwLock<bool>>,
}

impl SimpleVirtioNetDevice {
    pub fn new(mac_addr: [u8; 6]) -> Self {
        Self {
            rx_queue: Arc::new(RwLock::new(VecDeque::new())),
            tx_queue: Arc::new(RwLock::new(VecDeque::new())),
            interrupt_count: Arc::new(AtomicU64::new(0)),
            rx_packets: Arc::new(AtomicU64::new(0)),
            tx_packets: Arc::new(AtomicU64::new(0)),
            rx_bytes: Arc::new(AtomicU64::new(0)),
            tx_bytes: Arc::new(AtomicU64::new(0)),
            mac_addr,
            enabled: Arc::new(RwLock::new(false)),
        }
    }

    /// 启用设备
    pub fn enable(&self) {
        *self.enabled.write() = true;
    }

    /// 禁用设备
    pub fn disable(&self) {
        *self.enabled.write() = false;
    }

    /// 发送网络数据包
    pub fn send_packet(&self, data: Vec<u8>) -> bool {
        if !*self.enabled.read() {
            return false;
        }

        let packet = NetworkPacket {
            data: data.clone(),
            timestamp: 0,
        };

        self.tx_queue.write().push_back(packet);
        self.tx_packets.fetch_add(1, Ordering::Relaxed);
        self.tx_bytes
            .fetch_add(data.len() as u64, Ordering::Relaxed);
        self.interrupt_count.fetch_add(1, Ordering::Relaxed);
        true
    }

    /// 接收网络数据包
    pub fn receive_packet(&self, data: Vec<u8>) -> bool {
        if !*self.enabled.read() {
            return false;
        }

        let packet = NetworkPacket {
            data: data.clone(),
            timestamp: 0,
        };

        self.rx_queue.write().push_back(packet);
        self.rx_packets.fetch_add(1, Ordering::Relaxed);
        self.rx_bytes
            .fetch_add(data.len() as u64, Ordering::Relaxed);
        self.interrupt_count.fetch_add(1, Ordering::Relaxed);
        true
    }

    /// 取出一个发送队列中的数据包
    pub fn dequeue_tx(&self) -> Option<NetworkPacket> {
        self.tx_queue.write().pop_front()
    }

    /// 取出一个接收队列中的数据包
    pub fn dequeue_rx(&self) -> Option<NetworkPacket> {
        self.rx_queue.write().pop_front()
    }

    /// 获取MAC地址
    pub fn get_mac_addr(&self) -> [u8; 6] {
        self.mac_addr
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> NetworkStats {
        NetworkStats {
            rx_packets: self.rx_packets.load(Ordering::Relaxed),
            tx_packets: self.tx_packets.load(Ordering::Relaxed),
            rx_bytes: self.rx_bytes.load(Ordering::Relaxed),
            tx_bytes: self.tx_bytes.load(Ordering::Relaxed),
            interrupts: self.interrupt_count.load(Ordering::Relaxed),
        }
    }

    /// 获取队列深度
    pub fn queue_depth(&self) -> (usize, usize) {
        let rx = self.rx_queue.read().len();
        let tx = self.tx_queue.read().len();
        (rx, tx)
    }
}

/// 网络设备统计
#[derive(Debug, Clone, Default)]
pub struct NetworkStats {
    pub rx_packets: u64,
    pub tx_packets: u64,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub interrupts: u64,
}

/// VirtIO-Block块设备（简化版）
pub struct SimpleVirtioBlockDevice {
    // 块数据存储 (简化为Vec)
    blocks: Arc<RwLock<Vec<u8>>>,
    // I/O请求队列
    io_queue: Arc<RwLock<VecDeque<BlockIORequest>>>,
    // 统计信息
    read_ops: Arc<AtomicU64>,
    write_ops: Arc<AtomicU64>,
    read_bytes: Arc<AtomicU64>,
    write_bytes: Arc<AtomicU64>,
    // DMA操作计数
    dma_ops: Arc<AtomicU64>,
    // 设备启用状态
    enabled: Arc<RwLock<bool>>,
    // 块大小
    block_size: usize,
}

/// 块设备I/O请求
#[derive(Clone, Debug)]
pub struct BlockIORequest {
    pub request_type: BlockIOType,
    pub block_offset: u64,
    pub block_count: u64,
    pub data: Vec<u8>,
}

/// I/O操作类型
#[derive(Clone, Debug, PartialEq)]
pub enum BlockIOType {
    Read,
    Write,
    Flush,
}

impl SimpleVirtioBlockDevice {
    pub fn new(size_mb: usize) -> Self {
        Self {
            blocks: Arc::new(RwLock::new(vec![0u8; size_mb * 1024 * 1024])),
            io_queue: Arc::new(RwLock::new(VecDeque::new())),
            read_ops: Arc::new(AtomicU64::new(0)),
            write_ops: Arc::new(AtomicU64::new(0)),
            read_bytes: Arc::new(AtomicU64::new(0)),
            write_bytes: Arc::new(AtomicU64::new(0)),
            dma_ops: Arc::new(AtomicU64::new(0)),
            enabled: Arc::new(RwLock::new(false)),
            block_size: 4096,
        }
    }

    /// 启用设备
    pub fn enable(&self) {
        *self.enabled.write() = true;
    }

    /// 禁用设备
    pub fn disable(&self) {
        *self.enabled.write() = false;
    }

    /// 读取块
    pub fn read_blocks(&self, block_offset: u64, block_count: u64) -> Option<Vec<u8>> {
        if !*self.enabled.read() {
            return None;
        }

        let blocks = self.blocks.read();
        let byte_offset = (block_offset * self.block_size as u64) as usize;
        let byte_count = (block_count * self.block_size as u64) as usize;

        if byte_offset + byte_count > blocks.len() {
            return None;
        }

        let data = blocks[byte_offset..byte_offset + byte_count].to_vec();
        self.read_ops.fetch_add(1, Ordering::Relaxed);
        self.read_bytes
            .fetch_add(byte_count as u64, Ordering::Relaxed);
        self.dma_ops.fetch_add(1, Ordering::Relaxed);

        Some(data)
    }

    /// 写入块
    pub fn write_blocks(&self, block_offset: u64, data: Vec<u8>) -> bool {
        if !*self.enabled.read() {
            return false;
        }

        let byte_count = data.len();
        let block_count = byte_count.div_ceil(self.block_size);
        let mut blocks = self.blocks.write();
        let byte_offset = (block_offset * self.block_size as u64) as usize;

        if byte_offset + byte_count > blocks.len() {
            return false;
        }

        blocks[byte_offset..byte_offset + byte_count].copy_from_slice(&data);
        self.write_ops.fetch_add(1, Ordering::Relaxed);
        self.write_bytes
            .fetch_add(byte_count as u64, Ordering::Relaxed);
        self.dma_ops
            .fetch_add(block_count as u64, Ordering::Relaxed);

        true
    }

    /// 排入I/O请求队列
    pub fn queue_io_request(&self, req: BlockIORequest) -> bool {
        if !*self.enabled.read() {
            return false;
        }

        self.io_queue.write().push_back(req);
        true
    }

    /// 取出I/O请求
    pub fn dequeue_io_request(&self) -> Option<BlockIORequest> {
        self.io_queue.write().pop_front()
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> BlockDeviceStats {
        BlockDeviceStats {
            read_ops: self.read_ops.load(Ordering::Relaxed),
            write_ops: self.write_ops.load(Ordering::Relaxed),
            read_bytes: self.read_bytes.load(Ordering::Relaxed),
            write_bytes: self.write_bytes.load(Ordering::Relaxed),
            dma_ops: self.dma_ops.load(Ordering::Relaxed),
        }
    }

    /// 设备容量 (字节)
    pub fn capacity(&self) -> usize {
        self.blocks.read().len()
    }
}

/// 块设备统计
#[derive(Debug, Clone, Default)]
pub struct BlockDeviceStats {
    pub read_ops: u64,
    pub write_ops: u64,
    pub read_bytes: u64,
    pub write_bytes: u64,
    pub dma_ops: u64,
}

/// 像素缓冲 (简化RGBA格式)
#[derive(Clone, Debug)]
pub struct Framebuffer {
    pub width: u32,
    pub height: u32,
    pub pitch: u32,
    pub pixels: Vec<u32>, // RGBA8888
}

impl Framebuffer {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pitch: width * 4,
            pixels: vec![0u32; (width * height) as usize],
        }
    }

    /// 清空帧缓冲 (填充为黑色)
    pub fn clear(&mut self) {
        self.pixels.fill(0);
    }

    /// 画单个像素
    pub fn put_pixel(&mut self, x: u32, y: u32, color: u32) {
        if x < self.width && y < self.height {
            self.pixels[(y * self.width + x) as usize] = color;
        }
    }

    /// 画矩形
    pub fn draw_rect(&mut self, x: u32, y: u32, w: u32, h: u32, color: u32) {
        for yy in y..y + h.min(self.height - y) {
            for xx in x..x + w.min(self.width - x) {
                self.put_pixel(xx, yy, color);
            }
        }
    }

    /// 获取像素数据
    pub fn get_pixels(&self) -> &[u32] {
        &self.pixels
    }
}

/// VirtIO-GPU设备（简化版）
pub struct SimpleVirtioGpuDevice {
    // 帧缓冲
    framebuffer: Arc<RwLock<Framebuffer>>,
    // 渲染命令队列
    cmd_queue: Arc<RwLock<VecDeque<GpuCommand>>>,
    // 统计
    render_calls: Arc<AtomicU64>,
    pixel_updates: Arc<AtomicU64>,
    enabled: Arc<RwLock<bool>>,
}

/// GPU命令
#[derive(Clone, Debug)]
pub enum GpuCommand {
    Clear(u32), // 颜色
    DrawRect {
        x: u32,
        y: u32,
        w: u32,
        h: u32,
        color: u32,
    },
    DrawPixel {
        x: u32,
        y: u32,
        color: u32,
    },
}

impl SimpleVirtioGpuDevice {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            framebuffer: Arc::new(RwLock::new(Framebuffer::new(width, height))),
            cmd_queue: Arc::new(RwLock::new(VecDeque::new())),
            render_calls: Arc::new(AtomicU64::new(0)),
            pixel_updates: Arc::new(AtomicU64::new(0)),
            enabled: Arc::new(RwLock::new(false)),
        }
    }

    /// 启用设备
    pub fn enable(&self) {
        *self.enabled.write() = true;
    }

    /// 禁用设备
    pub fn disable(&self) {
        *self.enabled.write() = false;
    }

    /// 提交渲染命令
    pub fn submit_command(&self, cmd: GpuCommand) -> bool {
        if !*self.enabled.read() {
            return false;
        }

        self.cmd_queue.write().push_back(cmd);
        self.render_calls.fetch_add(1, Ordering::Relaxed);
        true
    }

    /// 处理一个命令 (简单渲染)
    pub fn process_command(&self) -> bool {
        if let Some(cmd) = self.cmd_queue.write().pop_front() {
            let mut fb = self.framebuffer.write();
            match cmd {
                GpuCommand::Clear(color) => {
                    fb.pixels.fill(color);
                    self.pixel_updates
                        .fetch_add(fb.pixels.len() as u64, Ordering::Relaxed);
                }
                GpuCommand::DrawRect { x, y, w, h, color } => {
                    fb.draw_rect(x, y, w, h, color);
                    self.pixel_updates
                        .fetch_add((w * h) as u64, Ordering::Relaxed);
                }
                GpuCommand::DrawPixel { x, y, color } => {
                    fb.put_pixel(x, y, color);
                    self.pixel_updates.fetch_add(1, Ordering::Relaxed);
                }
            }
            return true;
        }
        false
    }

    /// 处理所有待处理的命令
    pub fn flush_commands(&self) {
        while self.process_command() {}
    }

    /// 获取帧缓冲快照
    pub fn get_framebuffer_copy(&self) -> Framebuffer {
        let fb = self.framebuffer.read();
        Framebuffer {
            width: fb.width,
            height: fb.height,
            pitch: fb.pitch,
            pixels: fb.pixels.clone(),
        }
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> GpuStats {
        GpuStats {
            render_calls: self.render_calls.load(Ordering::Relaxed),
            pixel_updates: self.pixel_updates.load(Ordering::Relaxed),
        }
    }
}

/// GPU设备统计
#[derive(Debug, Clone, Default)]
pub struct GpuStats {
    pub render_calls: u64,
    pub pixel_updates: u64,
}

/// 输入设备 (键盘和鼠标)（简化版）
pub struct SimpleInputDevice {
    // 键盘事件队列
    key_queue: Arc<RwLock<VecDeque<KeyEvent>>>,
    // 鼠标事件队列
    mouse_queue: Arc<RwLock<VecDeque<MouseEvent>>>,
    // 统计
    key_events: Arc<AtomicU64>,
    mouse_events: Arc<AtomicU64>,
    enabled: Arc<RwLock<bool>>,
}

/// 键盘事件
#[derive(Clone, Debug)]
pub struct KeyEvent {
    pub keycode: u32,
    pub pressed: bool,
}

/// 鼠标事件
#[derive(Clone, Debug)]
pub struct MouseEvent {
    pub x: i32,
    pub y: i32,
    pub buttons: u32, // 位字段: bit0=左, bit1=右, bit2=中
}

impl SimpleInputDevice {
    pub fn new() -> Self {
        Self {
            key_queue: Arc::new(RwLock::new(VecDeque::new())),
            mouse_queue: Arc::new(RwLock::new(VecDeque::new())),
            key_events: Arc::new(AtomicU64::new(0)),
            mouse_events: Arc::new(AtomicU64::new(0)),
            enabled: Arc::new(RwLock::new(false)),
        }
    }

    /// 启用设备
    pub fn enable(&self) {
        *self.enabled.write() = true;
    }

    /// 禁用设备
    pub fn disable(&self) {
        *self.enabled.write() = false;
    }

    /// 注入键盘事件
    pub fn inject_key_event(&self, keycode: u32, pressed: bool) -> bool {
        if !*self.enabled.read() {
            return false;
        }

        self.key_queue
            .write()
            .push_back(KeyEvent { keycode, pressed });
        self.key_events.fetch_add(1, Ordering::Relaxed);
        true
    }

    /// 注入鼠标事件
    pub fn inject_mouse_event(&self, x: i32, y: i32, buttons: u32) -> bool {
        if !*self.enabled.read() {
            return false;
        }

        self.mouse_queue
            .write()
            .push_back(MouseEvent { x, y, buttons });
        self.mouse_events.fetch_add(1, Ordering::Relaxed);
        true
    }

    /// 取出键盘事件
    pub fn dequeue_key_event(&self) -> Option<KeyEvent> {
        self.key_queue.write().pop_front()
    }

    /// 取出鼠标事件
    pub fn dequeue_mouse_event(&self) -> Option<MouseEvent> {
        self.mouse_queue.write().pop_front()
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> InputStats {
        InputStats {
            key_events: self.key_events.load(Ordering::Relaxed),
            mouse_events: self.mouse_events.load(Ordering::Relaxed),
        }
    }
}

impl Default for SimpleInputDevice {
    fn default() -> Self {
        Self::new()
    }
}

/// 输入设备统计
#[derive(Debug, Clone, Default)]
pub struct InputStats {
    pub key_events: u64,
    pub mouse_events: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_virtio_net_basic() {
        let net = SimpleVirtioNetDevice::new([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]);
        net.enable();

        let data = vec![1, 2, 3, 4, 5];
        assert!(net.send_packet(data.clone()));

        let stats = net.get_stats();
        assert_eq!(stats.tx_packets, 1);
        assert_eq!(stats.tx_bytes, 5);
    }

    #[test]
    fn test_simple_virtio_block_read_write() {
        let block = SimpleVirtioBlockDevice::new(10); // 10MB
        block.enable();

        let data = vec![42u8; 4096];
        assert!(block.write_blocks(0, data.clone()));

        let read_data = block.read_blocks(0, 1).unwrap();
        assert_eq!(read_data, data);

        let stats = block.get_stats();
        assert_eq!(stats.read_ops, 1);
        assert_eq!(stats.write_ops, 1);
    }

    #[test]
    fn test_simple_virtio_gpu_framebuffer() {
        let gpu = SimpleVirtioGpuDevice::new(800, 600);
        gpu.enable();

        let cmd = GpuCommand::Clear(0xFF000000); // 黑色
        assert!(gpu.submit_command(cmd));
        gpu.flush_commands();

        let fb = gpu.get_framebuffer_copy();
        assert_eq!(fb.width, 800);
        assert_eq!(fb.height, 600);
        assert!(fb.pixels.iter().all(|&p| p == 0xFF000000));
    }

    #[test]
    fn test_simple_input_device_keyboard() {
        let input = SimpleInputDevice::new();
        input.enable();

        assert!(input.inject_key_event(65, true)); // 'A' 按下
        assert!(input.inject_key_event(65, false)); // 'A' 释放

        let stats = input.get_stats();
        assert_eq!(stats.key_events, 2);
    }
}
