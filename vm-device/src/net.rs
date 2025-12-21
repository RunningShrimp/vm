//! virtio-net 网络设备实现
//!
//! 支持 smoltcp (NAT) 和 TAP/TUN (桥接) 两种后端

use crate::mmu_util::MmuUtil;
use crate::virtio::Queue;
use thiserror::Error;
use vm_core::{MMU, MmioDevice, PlatformError, VmError, VmResult};

/// 网络后端类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkBackend {
    /// NAT 模式（使用 smoltcp）
    Nat,
    /// 桥接模式（使用 TAP/TUN）
    Tap,
}

/// VirtIO Net 配置
pub struct VirtioNetConfig {
    /// MAC 地址
    pub mac: [u8; 6],
    /// 状态
    pub status: u16,
    /// 最大队列对数
    pub max_virtqueue_pairs: u16,
    /// MTU
    pub mtu: u16,
}

impl Default for VirtioNetConfig {
    fn default() -> Self {
        Self {
            mac: [0x52, 0x54, 0x00, 0x12, 0x34, 0x56],
            status: 1, // VIRTIO_NET_S_LINK_UP
            max_virtqueue_pairs: 1,
            mtu: 1500,
        }
    }
}

/// VirtIO Net 设备
pub struct VirtioNet {
    /// 配置
    config: VirtioNetConfig,
    /// 后端类型
    backend_type: NetworkBackend,
    /// smoltcp 后端
    #[cfg(feature = "smoltcp")]
    smoltcp_backend: Option<SmoltcpBackend>,
    /// TAP 后端
    #[cfg(target_os = "linux")]
    tap_backend: Option<TapBackend>,
    /// 设备状态
    status: u32,
    /// 队列选择器
    queue_sel: u32,
    /// 队列列表 (0: RX, 1: TX)
    queues: Vec<Queue>,
    /// 中断状态
    interrupt_status: u32,
    /// 驱动特性
    driver_features: u32,
    /// 驱动特性选择
    driver_features_sel: u32,
    /// 设备特性选择
    device_features_sel: u32,
}

impl Default for VirtioNet {
    fn default() -> Self {
        Self::new()
    }
}

impl VirtioNet {
    /// 创建新的网络设备
    pub fn new() -> Self {
        let mut queues = Vec::new();
        // 默认 2 个队列: RX (0), TX (1)
        for _ in 0..2 {
            queues.push(Queue::new(256));
        }

        Self {
            config: VirtioNetConfig::default(),
            backend_type: NetworkBackend::Nat,
            #[cfg(feature = "smoltcp")]
            smoltcp_backend: None,
            #[cfg(target_os = "linux")]
            tap_backend: None,
            status: 0,
            queue_sel: 0,
            queues,
            interrupt_status: 0,
            driver_features: 0,
            driver_features_sel: 0,
            device_features_sel: 0,
        }
    }

    /// 使用 smoltcp 后端
    #[cfg(feature = "smoltcp")]
    pub fn with_smoltcp(mut self) -> Self {
        self.backend_type = NetworkBackend::Nat;
        self.smoltcp_backend = Some(SmoltcpBackend::new(self.config.mac));
        self
    }

    /// 使用 TAP 后端
    #[cfg(target_os = "linux")]
    pub fn with_tap(mut self, tap_name: &str) -> Result<Self, VmError> {
        self.backend_type = NetworkBackend::Tap;
        self.tap_backend = Some(TapBackend::new(tap_name)?);
        Ok(self)
    }

    /// 发送数据包
    pub fn send_packet(&mut self, data: &[u8]) -> Result<(), VmError> {
        match self.backend_type {
            #[cfg(feature = "smoltcp")]
            NetworkBackend::Nat => {
                if let Some(backend) = &mut self.smoltcp_backend {
                    backend.send(data)
                } else {
                    Err(VmError::Platform(PlatformError::InitializationFailed(
                        "smoltcp backend not initialized".into(),
                    )))
                }
            }
            #[cfg(not(feature = "smoltcp"))]
            NetworkBackend::Nat => Err(VmError::Platform(PlatformError::HardwareUnavailable(
                "smoltcp backend not available".to_string(),
            ))),

            #[cfg(target_os = "linux")]
            NetworkBackend::Tap => {
                if let Some(backend) = &mut self.tap_backend {
                    backend.send(data)
                } else {
                    Err(VmError::Platform(PlatformError::InitializationFailed(
                        "tap backend not initialized".into(),
                    )))
                }
            }
            #[cfg(not(target_os = "linux"))]
            NetworkBackend::Tap => Err(VmError::Platform(PlatformError::HardwareUnavailable(
                "tap backend not available".to_string(),
            ))),
        }
    }

    /// 接收数据包
    pub fn recv_packet(&mut self) -> Result<Vec<u8>, VmError> {
        match self.backend_type {
            #[cfg(feature = "smoltcp")]
            NetworkBackend::Nat => {
                if let Some(backend) = &mut self.smoltcp_backend {
                    backend.recv()
                } else {
                    Err(VmError::Platform(PlatformError::InitializationFailed(
                        "smoltcp backend not initialized".into(),
                    )))
                }
            }
            #[cfg(not(feature = "smoltcp"))]
            NetworkBackend::Nat => Err(VmError::Platform(PlatformError::HardwareUnavailable(
                "smoltcp backend not available".to_string(),
            ))),

            #[cfg(target_os = "linux")]
            NetworkBackend::Tap => {
                if let Some(backend) = &mut self.tap_backend {
                    backend.recv()
                } else {
                    Err(VmError::Platform(PlatformError::InitializationFailed(
                        "tap backend not initialized".into(),
                    )))
                }
            }
            #[cfg(not(target_os = "linux"))]
            NetworkBackend::Tap => Err(VmError::Platform(PlatformError::HardwareUnavailable(
                "tap backend not available".to_string(),
            ))),
        }
    }

    /// 处理 TX 队列 (Queue 1)
    fn process_tx_queue(&mut self, mmu: &mut dyn MMU) {
        if self.queues.len() <= 1 {
            return;
        }

        // 借用检查 workaround: 先获取需要的信息，再处理
        // 因为 pop 需要 &mut self.queues[1] 和 &dyn MmuUtil
        // 而 send_packet 需要 &mut self

        let mut packets = Vec::new();

        {
            let queue = &mut self.queues[1];
            while let Some(chain) = queue.pop(mmu) {
                // 解析描述符链
                // VirtIO Net TX Header (12 bytes) + Packet Data
                // 简化：假设 Header 和 Data 在同一个或连续的描述符中

                let mut packet_data = Vec::new();
                for desc in chain.descs {
                    if (desc.flags & 2) != 0 {
                        continue;
                    } // Skip write-only descriptors (shouldn't be in TX)

                    let mut buf = vec![0u8; desc.len as usize];
                    if MmuUtil::read_slice(mmu, desc.addr, &mut buf).is_ok() {
                        packet_data.extend_from_slice(&buf);
                    }
                }

                // 去掉 VirtIO Net Header (12 bytes if VIRTIO_NET_F_MRG_RXBUF is not negotiated, or 10 bytes legacy)
                // 现代 VirtIO 通常是 12 字节 (num_buffers at offset 10)
                let header_len = 12;
                if packet_data.len() > header_len {
                    packets.push((chain.head_index, packet_data[header_len..].to_vec()));
                } else {
                    // Empty packet? Just return descriptor
                    packets.push((chain.head_index, Vec::new()));
                }
            }
        }

        // 发送数据包并更新 Used Ring
        for (head_index, data) in packets {
            if !data.is_empty() {
                let _ = self.send_packet(&data);
            }
            // TX 完成，写入 Used Ring
            // len = 0 for TX
            self.queues[1].add_used(mmu, head_index, 0);
        }

        // 触发中断
        self.interrupt_status |= 1;
    }

    /// 处理 RX 队列 (Queue 0)
    fn process_rx_queue(&mut self, mmu: &mut dyn MMU) {
        if self.queues.is_empty() {
            return;
        }

        // 借用检查 workaround: 先获取需要的信息，再处理
        // 因为 pop 需要 &mut self.queues[0] 和 &dyn MmuUtil
        // 而 recv_packet 需要 &mut self

        let mut packets = Vec::new();
        let mut chains = Vec::new();

        // 先尝试获取一个数据包
        if let Ok(packet) = self.recv_packet() {
            packets.push(packet);
        }

        // 然后尝试获取一个空闲的描述符
        {
            let queue = &mut self.queues[0];
            if let Some(chain) = queue.pop(mmu) {
                chains.push(chain);
            }
        }

        // 处理数据包和描述符
        if let (Some(packet), Some(chain)) = (packets.pop(), chains.pop()) {
            // 将数据包发送给客人
            let mut bytes_written = 0;

            for desc in chain.descs {
                if (desc.flags & 1) == 0 {
                    continue; // Skip read-only descriptors
                }

                let available_space = desc.len as usize;
                let remaining_packet = &packet[bytes_written..];
                let write_size = std::cmp::min(available_space, remaining_packet.len());

                if write_size > 0
                    && let Ok(_) =
                        MmuUtil::write_slice(mmu, desc.addr, &remaining_packet[..write_size])
                {
                    bytes_written += write_size;
                }

                if bytes_written >= packet.len() {
                    break;
                }
            }

            // 更新 Used Ring
            self.queues[0].add_used(mmu, chain.head_index, bytes_written as u32);

            // 触发中断
            self.interrupt_status |= 1;
        } else if !packets.is_empty() {
            // 没有可用的描述符，将数据包放回接收缓冲区
            // 这里需要后端支持重新入队
            log::debug!("RX queue full, dropping packet");
        }
    }
}

/// 网络设备错误类型别名
pub type NetError = VmError;

/// 从传统错误转换为统一错误
impl From<NetLegacyError> for VmError {
    fn from(err: NetLegacyError) -> Self {
        match err {
            NetLegacyError::BackendNotInitialized(msg) => {
                VmError::Platform(PlatformError::InitializationFailed(msg))
            }
            NetLegacyError::NotAvailable => VmError::Platform(PlatformError::HardwareUnavailable(
                "No network backend available".to_string(),
            )),
            NetLegacyError::Io(e) => VmError::Platform(PlatformError::IoError(e.to_string())),
            NetLegacyError::Tap(msg) => VmError::Platform(PlatformError::IoError(
                std::io::Error::other(msg).to_string(),
            )),
        }
    }
}

/// 传统的网络错误类型（保留用于向后兼容）
#[derive(Debug, Error)]
pub enum NetLegacyError {
    #[error("Backend not initialized: {0}")]
    BackendNotInitialized(String),
    #[error("No network backend available")]
    NotAvailable,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Tap error: {0}")]
    Tap(String),
}

impl MmioDevice for VirtioNet {
    fn read(&self, offset: u64, _size: u8) -> VmResult<u64> {
        Ok(match offset {
            0x00 => 0x74726976, // Magic "virt"
            0x04 => 2,          // Version
            0x08 => 1,          // Device ID (Net)
            0x0C => 0x554D4551, // Vendor ID
            0x10 => 0x00000021, // Device features (low): VIRTIO_NET_F_MAC | VIRTIO_NET_F_STATUS
            0x14 => 0x00000001, // Device features (high): VIRTIO_F_VERSION_1
            0x34 => {
                // QueueNumMax
                if (self.queue_sel as usize) < self.queues.len() {
                    self.queues[self.queue_sel as usize].size as u64
                } else {
                    0
                }
            }
            0x44 => 1, // QueueReady (Always ready for now)
            0x60 => self.interrupt_status as u64,
            0x70 => self.status as u64, // Status
            // 配置空间 (0x100+)
            0x100 => u32::from_le_bytes([
                self.config.mac[0],
                self.config.mac[1],
                self.config.mac[2],
                self.config.mac[3],
            ]) as u64,
            0x104 => u16::from_le_bytes([self.config.mac[4], self.config.mac[5]]) as u64,
            0x106 => self.config.status as u64,
            0x108 => self.config.max_virtqueue_pairs as u64,
            0x10A => self.config.mtu as u64,
            _ => 0,
        })
    }

    fn write(&mut self, offset: u64, val: u64, _size: u8) -> VmResult<()> {
        match offset {
            0x14 => self.device_features_sel = val as u32,
            0x20 => self.driver_features = val as u32,
            0x24 => self.driver_features_sel = val as u32,
            0x30 => self.queue_sel = val as u32, // Queue select
            0x38 => {
                // QueueNum
                if (self.queue_sel as usize) < self.queues.len() {
                    self.queues[self.queue_sel as usize].size = val as u16;
                }
            }
            0x50 => {
                // Queue notify
                let queue_idx = val as usize;
                log::debug!("VirtioNet: Queue {} notified", queue_idx);
                // 队列通知由虚拟机主循环处理，调用 process_queues
            }
            0x64 => {
                // InterruptACK
                self.interrupt_status &= !(val as u32);
            }
            0x70 => self.status = val as u32, // Status
            0x80 => {
                // QueueDescLow
                if (self.queue_sel as usize) < self.queues.len() {
                    self.queues[self.queue_sel as usize].desc_addr = val;
                }
            }
            0x90 => {
                // QueueDriverLow (Avail)
                if (self.queue_sel as usize) < self.queues.len() {
                    self.queues[self.queue_sel as usize].avail_addr = val;
                }
            }
            0xA0 => {
                // QueueDeviceLow (Used)
                if (self.queue_sel as usize) < self.queues.len() {
                    self.queues[self.queue_sel as usize].used_addr = val;
                }
            }
            _ => {
                log::trace!("VirtioNet write: offset={:#x} val={:#x}", offset, val);
            }
        }
        Ok(())
    }
}

/// 为 VirtioNet 实现 VirtioDevice trait
impl crate::virtio::VirtioDevice for VirtioNet {
    fn device_id(&self) -> u32 {
        1 // VIRTIO_DEVICE_ID_NET
    }

    fn num_queues(&self) -> usize {
        self.queues.len()
    }

    fn get_queue(&mut self, index: usize) -> &mut crate::virtio::Queue {
        // 需要转换内部队列类型到 virtio::Queue
        // 这里我们假设内部队列和 virtio::Queue 结构兼容
        let queue = &mut self.queues[index];
        unsafe { std::mem::transmute(queue) }
    }

    /// 处理所有队列
    fn process_queues(&mut self, mmu: &mut dyn MMU) {
        // 处理 TX 队列 (Queue 1)
        if self.queues.len() > 1 {
            self.process_tx_queue(mmu);
        }

        // 处理 RX 队列 (Queue 0)
        if !self.queues.is_empty() {
            self.process_rx_queue(mmu);
        }
    }
}

/// smoltcp 网络后端
#[cfg(feature = "smoltcp")]
pub struct SmoltcpBackend {
    /// MAC 地址
    mac: [u8; 6],
    /// 接收缓冲区
    rx_buffer: Vec<Vec<u8>>,
    /// 发送缓冲区
    tx_buffer: Vec<Vec<u8>>,
}

#[cfg(feature = "smoltcp")]
impl SmoltcpBackend {
    pub fn new(mac: [u8; 6]) -> Self {
        Self {
            mac,
            rx_buffer: Vec::new(),
            tx_buffer: Vec::new(),
        }
    }

    pub fn send(&mut self, data: &[u8]) -> Result<(), VmError> {
        self.tx_buffer.push(data.to_vec());
        log::debug!(
            "smoltcp: MAC {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} sent {} bytes",
            self.mac[0],
            self.mac[1],
            self.mac[2],
            self.mac[3],
            self.mac[4],
            self.mac[5],
            data.len()
        );
        Ok(())
    }

    pub fn recv(&mut self) -> Result<Vec<u8>, VmError> {
        self.rx_buffer.pop().ok_or_else(|| {
            VmError::Platform(PlatformError::IoError(
                std::io::Error::new(std::io::ErrorKind::WouldBlock, "No data available")
                    .to_string(),
            ))
        })
    }

    /// 处理网络栈
    pub fn poll(&mut self) {
        // 这里应该调用 smoltcp 的 poll 函数
        // 处理 ARP、IP、TCP/UDP 等协议栈
        // 简化实现
    }
}

/// TAP/TUN 网络后端
#[cfg(target_os = "linux")]
pub struct TapBackend {
    /// TAP 设备名称
    name: String,
    /// 文件描述符
    fd: i32,
}

#[cfg(target_os = "linux")]
impl TapBackend {
    pub fn new(name: &str) -> Result<Self, VmError> {
        use std::ffi::CString;
        use std::os::unix::io::AsRawFd;

        // 打开 /dev/net/tun
        let path = CString::new("/dev/net/tun").expect("Failed to create device path");
        let fd = unsafe { libc::open(path.as_ptr(), libc::O_RDWR | libc::O_NONBLOCK) };

        if fd < 0 {
            return Err(VmError::Platform(PlatformError::IoError(
                std::io::Error::last_os_error(),
            )));
        }

        // 配置 TAP 设备
        #[repr(C)]
        struct ifreq {
            ifr_name: [u8; libc::IF_NAMESIZE],
            ifr_flags: i16,
            _padding: [u8; 22],
        }

        let mut ifr = ifreq {
            ifr_name: [0; libc::IF_NAMESIZE],
            ifr_flags: libc::IFF_TAP as i16 | libc::IFF_NO_PI as i16,
            _padding: [0; 22],
        };

        let name_bytes = name.as_bytes();
        let len = name_bytes.len().min(libc::IF_NAMESIZE - 1);
        ifr.ifr_name[..len].copy_from_slice(&name_bytes[..len]);

        const TUNSETIFF: u64 = 0x400454ca;
        let ret =
            unsafe { libc::ioctl(fd, TUNSETIFF as _, &ifr as *const _ as *const libc::c_void) };

        if ret < 0 {
            unsafe {
                libc::close(fd);
            }
            return Err(VmError::Platform(PlatformError::IoError(
                std::io::Error::new(std::io::ErrorKind::Other, "Failed to configure TAP device"),
            )));
        }

        Ok(Self {
            name: name.to_string(),
            fd,
        })
    }

    pub fn send(&mut self, data: &[u8]) -> Result<(), VmError> {
        let ret = unsafe { libc::write(self.fd, data.as_ptr() as *const libc::c_void, data.len()) };

        if ret < 0 {
            Err(VmError::Platform(PlatformError::IoError(
                std::io::Error::last_os_error(),
            )))
        } else {
            log::debug!("TAP: Sent {} bytes", ret);
            Ok(())
        }
    }

    pub fn recv(&mut self) -> Result<Vec<u8>, VmError> {
        let mut buffer = vec![0u8; 2048];
        let ret = unsafe {
            libc::read(
                self.fd,
                buffer.as_mut_ptr() as *mut libc::c_void,
                buffer.len(),
            )
        };

        if ret < 0 {
            let errno = unsafe { *libc::__errno_location() };
            if errno == libc::EAGAIN || errno == libc::EWOULDBLOCK {
                return Err(VmError::Platform(PlatformError::IoError(
                    std::io::Error::new(std::io::ErrorKind::WouldBlock, "No data available"),
                )));
            }
            return Err(VmError::Platform(PlatformError::IoError(
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to read from TAP device: errno {}", errno),
                ),
            )));
        }

        buffer.truncate(ret as usize);
        log::debug!("TAP: Received {} bytes", ret);
        Ok(buffer)
    }
}

#[cfg(target_os = "linux")]
impl Drop for TapBackend {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_virtio_net_creation() {
        let net = VirtioNet::new();

        assert_eq!(net.read(0x00, 4).unwrap(), 0x74726976); // Magic
        assert_eq!(net.read(0x08, 4).unwrap(), 1); // Device ID
        assert_eq!(net.config.mac, [0x52, 0x54, 0x00, 0x12, 0x34, 0x56]);
    }

    #[cfg(feature = "smoltcp")]
    #[test]
    fn test_smoltcp_backend() {
        let mut backend = SmoltcpBackend::new([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]);

        let data = b"Hello, network!";
        backend.send(data).expect("Failed to send network data");

        assert_eq!(backend.tx_buffer.len(), 1);
    }
}
