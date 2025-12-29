//! virtio-net 网络设备实现
//!
//! 支持 smoltcp (NAT) 和 TAP/TUN (桥接) 两种后端

use thiserror::Error;
use vm_core::{PlatformError, VmError};

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

// ============================================================================
// smoltcp backend implementation
// ============================================================================
#[cfg(feature = "smoltcp")]
mod smoltcp_backend {
    use super::*;
    use crate::virtio::Queue;
    use vm_core::{MMU, MmioDevice, PlatformError, VmError, VmResult};

    pub struct SmoltcpBackend {
        mac: [u8; 6],
        rx_buffer: Vec<Vec<u8>>,
        tx_buffer: Vec<Vec<u8>>,
    }

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

        pub fn poll(&mut self) {
            // smoltcp poll implementation
        }
    }

    pub struct VirtioNet {
        config: VirtioNetConfig,
        backend: Option<SmoltcpBackend>,
        queues: Vec<Queue>,
        interrupt_status: u32,
        status: u32,
        queue_sel: u32,
        driver_features: u32,
        driver_features_sel: u32,
        device_features_sel: u32,
    }

    impl VirtioNet {
        pub fn new() -> Self {
            let mut queues = Vec::new();
            for _ in 0..2 {
                queues.push(Queue::new(256));
            }

            Self {
                config: VirtioNetConfig::default(),
                backend: None,
                queues,
                interrupt_status: 0,
                status: 0,
                queue_sel: 0,
                driver_features: 0,
                driver_features_sel: 0,
                device_features_sel: 0,
            }
        }

        pub fn with_backend(mut self, mac: [u8; 6]) -> Self {
            self.backend = Some(SmoltcpBackend::new(mac));
            self
        }
    }

    impl MmioDevice for VirtioNet {
        fn read(&self, offset: u64, _size: u8) -> VmResult<u64> {
            Ok(match offset {
                0x00 => 0x74726976, // Magic
                0x04 => 2,          // Version
                0x08 => 1,          // Device ID
                0x0C => 0x554D4551, // Vendor ID
                0x10 => 0x00000021, // Features
                0x14 => 0x00000001,
                0x34 => self
                    .queues
                    .get(self.queue_sel as usize)
                    .map(|q| q.size as u64)
                    .unwrap_or(0),
                0x44 => 1, // QueueReady
                0x60 => self.interrupt_status as u64,
                0x70 => self.status as u64,
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
                0x30 => self.queue_sel = val as u32,
                0x38 => {
                    if (self.queue_sel as usize) < self.queues.len() {
                        self.queues[self.queue_sel as usize].size = val as u16;
                    }
                }
                0x50 => {
                    log::debug!("VirtioNet: Queue {} notified", val as usize);
                }
                0x64 => {
                    self.interrupt_status &= !(val as u32);
                }
                0x70 => self.status = val as u32,
                0x80 => {
                    if (self.queue_sel as usize) < self.queues.len() {
                        self.queues[self.queue_sel as usize].desc_addr = val;
                    }
                }
                0x90 => {
                    if (self.queue_sel as usize) < self.queues.len() {
                        self.queues[self.queue_sel as usize].avail_addr = val;
                    }
                }
                0xA0 => {
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

    impl crate::virtio::VirtioDevice for VirtioNet {
        fn device_id(&self) -> u32 {
            1
        }
        fn num_queues(&self) -> usize {
            self.queues.len()
        }
        fn get_queue(&mut self, index: usize) -> &mut crate::virtio::Queue {
            unsafe { std::mem::transmute(&mut self.queues[index]) }
        }
        fn process_queues(&mut self, _mmu: &mut dyn MMU) {}
    }
}

// ============================================================================
// TAP backend implementation (Linux only)
// ============================================================================
#[cfg(all(target_os = "linux", not(feature = "smoltcp")))]
mod tap_backend {
    use super::*;
    use crate::mmu_util::MmuUtil;
    use crate::virtio::Queue;
    use std::ffi::CString;
    use vm_core::{MMU, MmioDevice, PlatformError, VmError, VmResult};

    pub struct TapBackend {
        name: String,
        fd: i32,
    }

    impl TapBackend {
        pub fn new(name: &str) -> Result<Self, VmError> {
            let path = CString::new("/dev/net/tun").unwrap();
            let fd = unsafe { libc::open(path.as_ptr(), libc::O_RDWR | libc::O_NONBLOCK) };

            if fd < 0 {
                return Err(VmError::Platform(PlatformError::IoError(
                    std::io::Error::last_os_error(),
                )));
            }

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
                    std::io::Error::new(std::io::ErrorKind::Other, "Failed to configure TAP"),
                )));
            }

            Ok(Self {
                name: name.to_string(),
                fd,
            })
        }

        pub fn send(&mut self, data: &[u8]) -> Result<(), VmError> {
            let ret =
                unsafe { libc::write(self.fd, data.as_ptr() as *const libc::c_void, data.len()) };
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
                        format!("TAP read error: {}", errno),
                    ),
                )));
            }

            buffer.truncate(ret as usize);
            log::debug!("TAP: Received {} bytes", ret);
            Ok(buffer)
        }
    }

    impl Drop for TapBackend {
        fn drop(&mut self) {
            unsafe {
                libc::close(self.fd);
            }
        }
    }

    pub struct VirtioNet {
        config: VirtioNetConfig,
        backend: Option<TapBackend>,
        queues: Vec<Queue>,
        interrupt_status: u32,
        status: u32,
        queue_sel: u32,
        driver_features: u32,
        driver_features_sel: u32,
        device_features_sel: u32,
    }

    impl VirtioNet {
        pub fn new() -> Self {
            let mut queues = Vec::new();
            for _ in 0..2 {
                queues.push(Queue::new(256));
            }

            Self {
                config: VirtioNetConfig::default(),
                backend: None,
                queues,
                interrupt_status: 0,
                status: 0,
                queue_sel: 0,
                driver_features: 0,
                driver_features_sel: 0,
                device_features_sel: 0,
            }
        }

        pub fn with_tap(mut self, tap_name: &str) -> Result<Self, VmError> {
            self.backend = Some(TapBackend::new(tap_name)?);
            Ok(self)
        }
    }

    impl MmioDevice for VirtioNet {
        fn read(&self, offset: u64, _size: u8) -> VmResult<u64> {
            Ok(match offset {
                0x00 => 0x74726976, // Magic
                0x04 => 2,          // Version
                0x08 => 1,          // Device ID
                0x0C => 0x554D4551, // Vendor ID
                0x10 => 0x00000021, // Features
                0x14 => 0x00000001,
                0x34 => self
                    .queues
                    .get(self.queue_sel as usize)
                    .map(|q| q.size as u64)
                    .unwrap_or(0),
                0x44 => 1, // QueueReady
                0x60 => self.interrupt_status as u64,
                0x70 => self.status as u64,
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
                0x30 => self.queue_sel = val as u32,
                0x38 => {
                    if (self.queue_sel as usize) < self.queues.len() {
                        self.queues[self.queue_sel as usize].size = val as u16;
                    }
                }
                0x50 => {
                    log::debug!("VirtioNet: Queue {} notified", val as usize);
                }
                0x64 => {
                    self.interrupt_status &= !(val as u32);
                }
                0x70 => self.status = val as u32,
                0x80 => {
                    if (self.queue_sel as usize) < self.queues.len() {
                        self.queues[self.queue_sel as usize].desc_addr = val;
                    }
                }
                0x90 => {
                    if (self.queue_sel as usize) < self.queues.len() {
                        self.queues[self.queue_sel as usize].avail_addr = val;
                    }
                }
                0xA0 => {
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

    impl crate::virtio::VirtioDevice for VirtioNet {
        fn device_id(&self) -> u32 {
            1
        }
        fn num_queues(&self) -> usize {
            self.queues.len()
        }
        fn get_queue(&mut self, index: usize) -> &mut crate::virtio::Queue {
            unsafe { std::mem::transmute(&mut self.queues[index]) }
        }
        fn process_queues(&mut self, _mmu: &mut dyn MMU) {}
    }
}

// ============================================================================
// Combined backend (both smoltcp and TAP available)
// ============================================================================
#[cfg(all(feature = "smoltcp", target_os = "linux"))]
mod combined_backend {
    use super::*;
    use crate::mmu_util::MmuUtil;
    use crate::virtio::Queue;
    use std::ffi::CString;
    use vm_core::{MMU, MmioDevice, PlatformError, VmError, VmResult};

    pub struct SmoltcpBackend {
        mac: [u8; 6],
        rx_buffer: Vec<Vec<u8>>,
        tx_buffer: Vec<Vec<u8>>,
    }

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
    }

    pub struct TapBackend {
        name: String,
        fd: i32,
    }

    impl TapBackend {
        pub fn new(name: &str) -> Result<Self, VmError> {
            let path = CString::new("/dev/net/tun").unwrap();
            let fd = unsafe { libc::open(path.as_ptr(), libc::O_RDWR | libc::O_NONBLOCK) };

            if fd < 0 {
                return Err(VmError::Platform(PlatformError::IoError(
                    std::io::Error::last_os_error(),
                )));
            }

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
                    std::io::Error::new(std::io::ErrorKind::Other, "Failed to configure TAP"),
                )));
            }

            Ok(Self {
                name: name.to_string(),
                fd,
            })
        }

        pub fn send(&mut self, data: &[u8]) -> Result<(), VmError> {
            let ret =
                unsafe { libc::write(self.fd, data.as_ptr() as *const libc::c_void, data.len()) };
            if ret < 0 {
                Err(VmError::Platform(PlatformError::IoError(
                    std::io::Error::last_os_error(),
                )))
            } else {
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
                    std::io::Error::new(std::io::ErrorKind::Other, format!("TAP error: {}", errno)),
                )));
            }

            buffer.truncate(ret as usize);
            Ok(buffer)
        }
    }

    impl Drop for TapBackend {
        fn drop(&mut self) {
            unsafe {
                libc::close(self.fd);
            }
        }
    }

    pub struct VirtioNet {
        config: VirtioNetConfig,
        backend_type: NetworkBackend,
        smoltcp_backend: Option<SmoltcpBackend>,
        tap_backend: Option<TapBackend>,
        queues: Vec<Queue>,
        interrupt_status: u32,
        status: u32,
        queue_sel: u32,
        driver_features: u32,
        driver_features_sel: u32,
        device_features_sel: u32,
    }

    impl VirtioNet {
        pub fn new() -> Self {
            let mut queues = Vec::new();
            for _ in 0..2 {
                queues.push(Queue::new(256));
            }

            Self {
                config: VirtioNetConfig::default(),
                backend_type: NetworkBackend::Nat,
                smoltcp_backend: None,
                tap_backend: None,
                queues,
                interrupt_status: 0,
                status: 0,
                queue_sel: 0,
                driver_features: 0,
                driver_features_sel: 0,
                device_features_sel: 0,
            }
        }

        pub fn with_smoltcp(mut self) -> Self {
            self.backend_type = NetworkBackend::Nat;
            self.smoltcp_backend = Some(SmoltcpBackend::new(self.config.mac));
            self
        }

        pub fn with_tap(mut self, tap_name: &str) -> Result<Self, VmError> {
            self.backend_type = NetworkBackend::Tap;
            self.tap_backend = Some(TapBackend::new(tap_name)?);
            Ok(self)
        }
    }

    impl MmioDevice for VirtioNet {
        fn read(&self, offset: u64, _size: u8) -> VmResult<u64> {
            Ok(match offset {
                0x00 => 0x74726976, // Magic
                0x04 => 2,          // Version
                0x08 => 1,          // Device ID
                0x0C => 0x554D4551, // Vendor ID
                0x10 => 0x00000021, // Features
                0x14 => 0x00000001,
                0x34 => self
                    .queues
                    .get(self.queue_sel as usize)
                    .map(|q| q.size as u64)
                    .unwrap_or(0),
                0x44 => 1, // QueueReady
                0x60 => self.interrupt_status as u64,
                0x70 => self.status as u64,
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
                0x30 => self.queue_sel = val as u32,
                0x38 => {
                    if (self.queue_sel as usize) < self.queues.len() {
                        self.queues[self.queue_sel as usize].size = val as u16;
                    }
                }
                0x50 => {
                    log::debug!("VirtioNet: Queue {} notified", val as usize);
                }
                0x64 => {
                    self.interrupt_status &= !(val as u32);
                }
                0x70 => self.status = val as u32,
                0x80 => {
                    if (self.queue_sel as usize) < self.queues.len() {
                        self.queues[self.queue_sel as usize].desc_addr = val;
                    }
                }
                0x90 => {
                    if (self.queue_sel as usize) < self.queues.len() {
                        self.queues[self.queue_sel as usize].avail_addr = val;
                    }
                }
                0xA0 => {
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

    impl crate::virtio::VirtioDevice for VirtioNet {
        fn device_id(&self) -> u32 {
            1
        }
        fn num_queues(&self) -> usize {
            self.queues.len()
        }
        fn get_queue(&mut self, index: usize) -> &mut crate::virtio::Queue {
            unsafe { std::mem::transmute(&mut self.queues[index]) }
        }
        fn process_queues(&mut self, _mmu: &mut dyn MMU) {}
    }
}

// ============================================================================
// Public re-exports
// ============================================================================

// smoltcp-only exports
#[cfg(all(feature = "smoltcp", not(target_os = "linux")))]
pub use smoltcp_backend::{SmoltcpBackend, VirtioNet};

// TAP-only exports
#[cfg(all(target_os = "linux", not(feature = "smoltcp")))]
pub use tap_backend::{TapBackend, VirtioNet};

// combined exports
#[cfg(all(feature = "smoltcp", target_os = "linux"))]
pub use combined_backend::{SmoltcpBackend, TapBackend, VirtioNet};

// ============================================================================
// Error types
// ============================================================================

/// 网络设备错误类型别名
pub type NetError = VmError;

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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_virtio_net_config() {
        let config = VirtioNetConfig::default();
        assert_eq!(config.mac, [0x52, 0x54, 0x00, 0x12, 0x34, 0x56]);
        assert_eq!(config.status, 1);
        assert_eq!(config.mtu, 1500);
    }

    #[cfg(feature = "smoltcp")]
    #[test]
    fn test_smoltcp_backend() {
        use smoltcp_backend::SmoltcpBackend;
        let mut backend = SmoltcpBackend::new([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]);

        let data = b"Hello, network!";
        backend.send(data).expect("Failed to send");

        assert_eq!(backend.tx_buffer.len(), 1);
    }
}
