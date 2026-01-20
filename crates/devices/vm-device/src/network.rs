//! 网络设备模拟
//!
//! 支持 virtio-net 和多种网络后端

#[cfg(unix)]
use libc;

use std::sync::{Arc, Mutex};

/// MAC 地址
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacAddress(pub [u8; 6]);

impl MacAddress {
    /// 创建新的 MAC 地址
    pub fn new(bytes: [u8; 6]) -> Self {
        Self(bytes)
    }

    /// 生成随机 MAC 地址（本地管理地址）
    pub fn random() -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Operation failed")
            .as_nanos() as u64;
        
        let mut bytes = [0u8; 6];
        bytes[0] = 0x52; // 本地管理地址
        bytes[1] = 0x54; // QEMU 风格
        bytes[2] = ((timestamp >> 24) & 0xFF) as u8;
        bytes[3] = ((timestamp >> 16) & 0xFF) as u8;
        bytes[4] = ((timestamp >> 8) & 0xFF) as u8;
        bytes[5] = (timestamp & 0xFF) as u8;
        
        Self(bytes)
    }

    /// 转换为字符串
    pub fn to_string(&self) -> String {
        format!(
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5]
        )
    }
}

/// 网络数据包
#[derive(Debug, Clone)]
pub struct NetworkPacket {
    pub data: Vec<u8>,
}

impl NetworkPacket {
    /// 创建新的数据包
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }

    /// 获取数据包长度
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// 获取目标 MAC 地址
    pub fn dst_mac(&self) -> Option<MacAddress> {
        if self.data.len() >= 6 {
            let mut bytes = [0u8; 6];
            bytes.copy_from_slice(&self.data[0..6]);
            Some(MacAddress(bytes))
        } else {
            None
        }
    }

    /// 获取源 MAC 地址
    pub fn src_mac(&self) -> Option<MacAddress> {
        if self.data.len() >= 12 {
            let mut bytes = [0u8; 6];
            bytes.copy_from_slice(&self.data[6..12]);
            Some(MacAddress(bytes))
        } else {
            None
        }
    }

    /// 获取以太网类型
    pub fn ethertype(&self) -> Option<u16> {
        if self.data.len() >= 14 {
            Some(u16::from_be_bytes([self.data[12], self.data[13]]))
        } else {
            None
        }
    }
}

/// virtio-net 设备
pub struct VirtioNet {
    /// MAC 地址
    mac: MacAddress,
    /// MTU (Maximum Transmission Unit)
    mtu: u16,
    /// 接收队列
    rx_queue: Vec<NetworkPacket>,
    /// 发送队列
    tx_queue: Vec<NetworkPacket>,
    /// 网络后端
    backend: Option<Box<dyn NetworkBackend>>,
    /// 统计信息
    stats: NetworkStats,
}

/// 网络统计信息
#[derive(Debug, Clone, Default)]
pub struct NetworkStats {
    pub rx_packets: u64,
    pub tx_packets: u64,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_errors: u64,
    pub tx_errors: u64,
    pub rx_dropped: u64,
    pub tx_dropped: u64,
}

impl VirtioNet {
    /// 创建新的 virtio-net 设备
    pub fn new() -> Self {
        Self {
            mac: MacAddress::random(),
            mtu: 1500,
            rx_queue: Vec::new(),
            tx_queue: Vec::new(),
            backend: None,
            stats: NetworkStats::default(),
        }
    }

    /// 设置 MAC 地址
    pub fn set_mac(&mut self, mac: MacAddress) {
        self.mac = mac;
        log::info!("virtio-net MAC address set to {}", mac.to_string());
    }

    /// 获取 MAC 地址
    pub fn get_mac(&self) -> MacAddress {
        self.mac
    }

    /// 设置 MTU
    pub fn set_mtu(&mut self, mtu: u16) {
        self.mtu = mtu;
        log::info!("virtio-net MTU set to {}", mtu);
    }

    /// 设置网络后端
    pub fn set_backend(&mut self, backend: Box<dyn NetworkBackend>) {
        self.backend = Some(backend);
    }

    /// 发送数据包
    pub fn send_packet(&mut self, packet: NetworkPacket) -> Result<(), NetworkError> {
        if packet.len() > self.mtu as usize {
            return Err(NetworkError::PacketTooLarge);
        }

        self.stats.tx_packets += 1;
        self.stats.tx_bytes += packet.len() as u64;

        if let Some(backend) = &mut self.backend {
            backend.send(&packet.data)?;
        } else {
            self.tx_queue.push(packet);
        }

        Ok(())
    }

    /// 接收数据包
    pub fn recv_packet(&mut self) -> Option<NetworkPacket> {
        if let Some(backend) = &mut self.backend {
            let mut buf = vec![0u8; self.mtu as usize];
            match backend.recv(&mut buf) {
                Ok(len) if len > 0 => {
                    buf.truncate(len);
                    let packet = NetworkPacket::new(buf);
                    self.stats.rx_packets += 1;
                    self.stats.rx_bytes += len as u64;
                    return Some(packet);
                }
                _ => {}
            }
        }

        self.rx_queue.pop()
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> &NetworkStats {
        &self.stats
    }

    /// 重置统计信息
    pub fn reset_stats(&mut self) {
        self.stats = NetworkStats::default();
    }
}

impl Default for VirtioNet {
    fn default() -> Self {
        Self::new()
    }
}

/// 网络错误类型
#[derive(Debug)]
pub enum NetworkError {
    PacketTooLarge,
    BackendError(String),
    IoError(std::io::Error),
}

impl std::fmt::Display for NetworkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NetworkError::PacketTooLarge => write!(f, "Packet too large"),
            NetworkError::BackendError(msg) => write!(f, "Backend error: {}", msg),
            NetworkError::IoError(e) => write!(f, "I/O error: {}", e),
        }
    }
}

impl std::error::Error for NetworkError {}

impl From<std::io::Error> for NetworkError {
    fn from(e: std::io::Error) -> Self {
        NetworkError::IoError(e)
    }
}

/// 网络后端 trait
pub trait NetworkBackend: Send + Sync {
    /// 发送数据包
    fn send(&mut self, data: &[u8]) -> Result<(), NetworkError>;
    
    /// 接收数据包
    fn recv(&mut self, buf: &mut [u8]) -> Result<usize, NetworkError>;
    
    /// 获取后端名称
    fn name(&self) -> &str;
}

/// 用户模式网络后端 (SLIRP)
pub struct UserModeNetwork {
    /// 网关 IP
    gateway: [u8; 4],
    /// DNS 服务器 IP
    dns: [u8; 4],
    /// DHCP 起始 IP
    dhcp_start: [u8; 4],
}

impl UserModeNetwork {
    /// 创建新的用户模式网络
    pub fn new() -> Self {
        Self {
            gateway: [10, 0, 2, 2],
            dns: [10, 0, 2, 3],
            dhcp_start: [10, 0, 2, 15],
        }
    }
}

impl Default for UserModeNetwork {
    fn default() -> Self {
        Self::new()
    }
}

impl NetworkBackend for UserModeNetwork {
    fn send(&mut self, data: &[u8]) -> Result<(), NetworkError> {
        // 用户模式网络：简化实现，仅记录
        log::debug!("UserMode: Sent {} bytes", data.len());
        Ok(())
    }

    fn recv(&mut self, _buf: &mut [u8]) -> Result<usize, NetworkError> {
        // 用户模式网络：简化实现
        Ok(0)
    }

    fn name(&self) -> &str {
        "user"
    }
}

/// TAP/TUN 网络后端
pub struct TapTunNetwork {
    #[cfg(unix)]
    fd: Option<i32>,
    #[cfg(not(unix))]
    _phantom: std::marker::PhantomData<()>,
    device_name: String,
}

impl TapTunNetwork {
    /// 创建新的 TAP/TUN 网络
    #[cfg(unix)]
    pub fn new(device_name: &str) -> Result<Self, NetworkError> {
        use std::ffi::CString;
        use std::os::unix::io::RawFd;

        let dev_path = CString::new("/dev/net/tun").expect("Failed to create TUN device path");
        let fd = unsafe { libc::open(dev_path.as_ptr(), libc::O_RDWR) };
        
        if fd < 0 {
            return Err(NetworkError::BackendError("Failed to open /dev/net/tun".to_string()));
        }

        // 配置 TAP 设备
        #[repr(C)]
        struct ifreq {
            ifr_name: [u8; 16],
            ifr_flags: i16,
            _padding: [u8; 22],
        }

        let mut ifr = ifreq {
            ifr_name: [0; 16],
            ifr_flags: 0x0002, // IFF_TAP
            _padding: [0; 22],
        };

        let name_bytes = device_name.as_bytes();
        let copy_len = name_bytes.len().min(15);
        ifr.ifr_name[..copy_len].copy_from_slice(&name_bytes[..copy_len]);

        const TUNSETIFF: u64 = 0x400454ca;
        let ret = unsafe { libc::ioctl(fd, TUNSETIFF, &ifr) };
        
        if ret < 0 {
            unsafe { libc::close(fd); }
            return Err(NetworkError::BackendError("Failed to configure TAP device".to_string()));
        }

        log::info!("Created TAP device: {}", device_name);

        Ok(Self {
            fd: Some(fd),
            device_name: device_name.to_string(),
        })
    }

    #[cfg(not(unix))]
    pub fn new(_device_name: &str) -> Result<Self, NetworkError> {
        Err(NetworkError::BackendError("TAP/TUN not supported on this platform".to_string()))
    }
}

impl NetworkBackend for TapTunNetwork {
    #[cfg(unix)]
    fn send(&mut self, data: &[u8]) -> Result<(), NetworkError> {
        if let Some(fd) = self.fd {
            let ret = unsafe { libc::write(fd, data.as_ptr() as *const _, data.len()) };
            if ret < 0 {
                return Err(NetworkError::BackendError("Write failed".to_string()));
            }
        }
        Ok(())
    }

    #[cfg(not(unix))]
    fn send(&mut self, _data: &[u8]) -> Result<(), NetworkError> {
        Err(NetworkError::BackendError("Not supported".to_string()))
    }

    #[cfg(unix)]
    fn recv(&mut self, buf: &mut [u8]) -> Result<usize, NetworkError> {
        if let Some(fd) = self.fd {
            let ret = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut _, buf.len()) };
            if ret < 0 {
                return Ok(0);
            }
            return Ok(ret as usize);
        }
        Ok(0)
    }

    #[cfg(not(unix))]
    fn recv(&mut self, _buf: &mut [u8]) -> Result<usize, NetworkError> {
        Ok(0)
    }

    fn name(&self) -> &str {
        &self.device_name
    }
}

impl Drop for TapTunNetwork {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Some(fd) = self.fd {
            unsafe { libc::close(fd); }
            log::info!("Closed TAP device: {}", self.device_name);
        }
    }
}

/// 共享网络设备
pub type SharedVirtioNet = Arc<Mutex<VirtioNet>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mac_address() {
        let mac = MacAddress::random();
        println!("Random MAC: {}", mac.to_string());
        assert_eq!(mac.0[0], 0x52);
        assert_eq!(mac.0[1], 0x54);
    }

    #[test]
    fn test_virtio_net() {
        let mut net = VirtioNet::new();
        println!("MAC: {}", net.get_mac().to_string());
        
        let packet = NetworkPacket::new(vec![0xFF; 64]);
        assert!(net.send_packet(packet).is_ok());
        
        let stats = net.get_stats();
        assert_eq!(stats.tx_packets, 1);
        assert_eq!(stats.tx_bytes, 64);
    }

    #[test]
    fn test_user_mode_network() {
        let mut backend = UserModeNetwork::new();
        assert_eq!(backend.name(), "user");
        
        let data = vec![0u8; 100];
        assert!(backend.send(&data).is_ok());
    }
}
