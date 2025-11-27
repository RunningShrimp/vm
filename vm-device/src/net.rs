//! virtio-net 网络设备实现
//!
//! 支持 smoltcp (NAT) 和 TAP/TUN (桥接) 两种后端

#[cfg(unix)]
use libc;

use vm_core::MmioDevice;
use std::sync::{Arc, Mutex};
use thiserror::Error;

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
}

impl VirtioNet {
    /// 创建新的网络设备
    pub fn new() -> Self {
        Self {
            config: VirtioNetConfig::default(),
            backend_type: NetworkBackend::Nat,
            #[cfg(feature = "smoltcp")]
            smoltcp_backend: None,
            #[cfg(target_os = "linux")]
            tap_backend: None,
            status: 0,
            queue_sel: 0,
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
    pub fn with_tap(mut self, tap_name: &str) -> Result<Self, NetError> {
        self.backend_type = NetworkBackend::Tap;
        self.tap_backend = Some(TapBackend::new(tap_name)?);
        Ok(self)
    }

    /// 发送数据包
    pub fn send_packet(&mut self, data: &[u8]) -> Result<(), NetError> {
        match self.backend_type {
            #[cfg(feature = "smoltcp")]
            NetworkBackend::Nat => {
                if let Some(backend) = &mut self.smoltcp_backend {
                    backend.send(data)
                } else {
                    Err(NetError::BackendNotInitialized("smoltcp".into()))
                }
            }
            #[cfg(target_os = "linux")]
            NetworkBackend::Tap => {
                if let Some(backend) = &mut self.tap_backend {
                    backend.send(data)
                } else {
                    Err(NetError::BackendNotInitialized("tap".into()))
                }
            }
            #[cfg(not(any(feature = "smoltcp", target_os = "linux")))]
            _ => Err(NetError::NotAvailable),
        }
    }

    /// 接收数据包
    pub fn recv_packet(&mut self) -> Result<Vec<u8>, NetError> {
        match self.backend_type {
            #[cfg(feature = "smoltcp")]
            NetworkBackend::Nat => {
                if let Some(backend) = &mut self.smoltcp_backend {
                    backend.recv()
                } else {
                    Err(NetError::BackendNotInitialized("smoltcp".into()))
                }
            }
            #[cfg(target_os = "linux")]
            NetworkBackend::Tap => {
                if let Some(backend) = &mut self.tap_backend {
                    backend.recv()
                } else {
                    Err(NetError::BackendNotInitialized("tap".into()))
                }
            }
            #[cfg(not(any(feature = "smoltcp", target_os = "linux")))]
            _ => Err(NetError::NotAvailable),
        }
    }
}

#[derive(Debug, Error)]
pub enum NetError {
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
    fn read(&self, offset: u64, _size: u8) -> u64 {
        match offset {
            0x00 => 0x74726976, // Magic "virt"
            0x04 => 2,          // Version
            0x08 => 1,          // Device ID (Net)
            0x0C => 0x554D4551, // Vendor ID
            0x10 => 0x00000021, // Device features (low): VIRTIO_NET_F_MAC | VIRTIO_NET_F_STATUS
            0x14 => 0x00000001, // Device features (high): VIRTIO_F_VERSION_1
            0x70 => self.status as u64, // Status
            // 配置空间 (0x100+)
            0x100 => u32::from_le_bytes([
                self.config.mac[0],
                self.config.mac[1],
                self.config.mac[2],
                self.config.mac[3],
            ]) as u64,
            0x104 => u16::from_le_bytes([
                self.config.mac[4],
                self.config.mac[5],
            ]) as u64,
            0x106 => self.config.status as u64,
            0x108 => self.config.max_virtqueue_pairs as u64,
            0x10A => self.config.mtu as u64,
            _ => 0,
        }
    }

    fn write(&mut self, offset: u64, val: u64, _size: u8) {
        match offset {
            0x30 => self.queue_sel = val as u32, // Queue select
            0x50 => {
                // Queue notify
                log::debug!("VirtioNet: Queue {} notified", val);
                // 这里应该处理队列请求
            }
            0x70 => self.status = val as u32, // Status
            _ => {
                log::trace!("VirtioNet write: offset={:#x} val={:#x}", offset, val);
            }
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

    pub fn send(&mut self, data: &[u8]) -> Result<(), NetError> {
        self.tx_buffer.push(data.to_vec());
        log::debug!("smoltcp: Sent {} bytes", data.len());
        Ok(())
    }

    pub fn recv(&mut self) -> Result<Vec<u8>, NetError> {
        self.rx_buffer.pop().ok_or(NetError::Tap("No data available".into()))
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
    pub fn new(name: &str) -> Result<Self, NetError> {
        use std::ffi::CString;
        use std::os::unix::io::AsRawFd;

        // 打开 /dev/net/tun
        let path = CString::new("/dev/net/tun").expect("Failed to create device path");
        let fd = unsafe {
            libc::open(
                path.as_ptr(),
                libc::O_RDWR | libc::O_NONBLOCK,
            )
        };

        if fd < 0 {
            return Err(NetError::Io(std::io::Error::last_os_error()));
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
        let ret = unsafe {
            libc::ioctl(fd, TUNSETIFF as _, &ifr as *const _ as *const libc::c_void)
        };

        if ret < 0 {
            unsafe { libc::close(fd); }
            return Err(NetError::Tap("Failed to configure TAP device".into()));
        }

        Ok(Self {
            name: name.to_string(),
            fd,
        })
    }

    pub fn send(&mut self, data: &[u8]) -> Result<(), NetError> {
        let ret = unsafe {
            libc::write(
                self.fd,
                data.as_ptr() as *const libc::c_void,
                data.len(),
            )
        };

        if ret < 0 {
            Err(NetError::Io(std::io::Error::last_os_error()))
        } else {
            log::debug!("TAP: Sent {} bytes", ret);
            Ok(())
        }
    }

    pub fn recv(&mut self) -> Result<Vec<u8>, NetError> {
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
                return Err(NetError::Tap("No data available".into()));
            }
            return Err(NetError::Tap(format!("Failed to read from TAP device: errno {}", errno)));
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
        
        assert_eq!(net.read(0x00, 4), 0x74726976); // Magic
        assert_eq!(net.read(0x08, 4), 1); // Device ID
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
