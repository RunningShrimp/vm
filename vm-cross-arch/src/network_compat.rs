//! 网络栈兼容层
//!
//! 提供跨架构网络栈兼容性，包括：
//! - 套接字地址转换（处理不同架构的地址格式）
//! - 网络字节序转换
//! - 套接字选项映射
//! - 网络协议抽象

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use vm_core::{GuestArch, GuestAddr, VmError};
use tracing::{debug, trace, warn};

/// 套接字域（地址族）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SocketDomain {
    /// Unix域套接字
    Unix = 1,
    /// IPv4
    Inet = 2,
    /// IPv6
    Inet6 = 10,
    /// Netlink
    Netlink = 16,
}

/// 套接字类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SocketType {
    /// 流套接字（TCP）
    Stream = 1,
    /// 数据报套接字（UDP）
    Datagram = 2,
    /// 原始套接字
    Raw = 3,
    /// 可靠数据报套接字
    Rdm = 4,
    /// 顺序包套接字
    SeqPacket = 5,
    /// DCCP套接字
    Dccp = 6,
}

/// 套接字协议
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SocketProtocol {
    /// IP协议
    Ip = 0,
    /// TCP协议
    Tcp = 6,
    /// UDP协议
    Udp = 17,
    /// ICMP协议
    Icmp = 1,
    /// ICMPv6协议
    Icmpv6 = 58,
}

/// 套接字选项级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SocketOptionLevel {
    /// 套接字级别
    Socket = 1,
    /// TCP级别
    Tcp = 6,
    /// IP级别
    Ip = 0,
    /// IPv6级别
    Ipv6 = 41,
}

/// 套接字选项名称
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SocketOption {
    /// SO_REUSEADDR
    ReuseAddr = 2,
    /// SO_KEEPALIVE
    KeepAlive = 9,
    /// SO_BROADCAST
    Broadcast = 6,
    /// SO_LINGER
    Linger = 13,
    /// SO_SNDBUF
    SendBuf = 7,
    /// SO_RCVBUF
    RecvBuf = 8,
    /// SO_ERROR
    Error = 4,
    /// SO_TYPE
    Type = 3,
    /// TCP_NODELAY
    TcpNoDelay = 1,
    /// TCP_KEEPIDLE
    TcpKeepIdle = 4,
    /// TCP_KEEPINTVL
    TcpKeepIntvl = 5,
    /// TCP_KEEPCNT
    TcpKeepCnt = 6,
}

/// 套接字地址（跨架构兼容）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SocketAddress {
    /// Unix域套接字地址
    Unix(String),
    /// IPv4地址
    Inet(SocketAddr),
    /// IPv6地址
    Inet6(SocketAddr),
    /// Netlink地址
    Netlink(u32),
}

impl SocketAddress {
    /// 从 guest 内存中的 sockaddr 结构解析地址
    pub fn from_guest_memory(
        guest_arch: GuestArch,
        addr: GuestAddr,
        addr_len: u32,
    ) -> Result<Self, VmError> {
        // 这里需要从 MMU 读取内存
        // 简化实现：返回占位符
        Err(VmError::Core(vm_core::CoreError::NotImplemented {
            feature: "SocketAddress::from_guest_memory".to_string(),
            module: "NetworkCompatibilityLayer".to_string(),
        }))
    }

    /// 转换为 host SocketAddr
    pub fn to_host_addr(&self) -> Option<SocketAddr> {
        match self {
            SocketAddress::Inet(addr) | SocketAddress::Inet6(addr) => Some(*addr),
            _ => None,
        }
    }
}

/// 套接字信息
#[derive(Debug, Clone)]
pub struct SocketInfo {
    pub fd: i32,
    pub domain: SocketDomain,
    pub socket_type: SocketType,
    pub protocol: SocketProtocol,
    pub local_addr: Option<SocketAddress>,
    pub remote_addr: Option<SocketAddress>,
    pub options: HashMap<(SocketOptionLevel, SocketOption), Vec<u8>>,
}

/// 网络栈操作 trait
pub trait NetworkStackOperations: Send + Sync {
    /// 创建套接字
    fn socket(&mut self, domain: SocketDomain, socket_type: SocketType, protocol: SocketProtocol) -> Result<i32, VmError>;

    /// 绑定套接字
    fn bind(&mut self, fd: i32, addr: SocketAddress) -> Result<(), VmError>;

    /// 监听套接字
    fn listen(&mut self, fd: i32, backlog: i32) -> Result<(), VmError>;

    /// 接受连接
    fn accept(&mut self, fd: i32) -> Result<(i32, SocketAddress), VmError>;

    /// 连接到远程地址
    fn connect(&mut self, fd: i32, addr: SocketAddress) -> Result<(), VmError>;

    /// 发送数据
    fn send(&mut self, fd: i32, buf: &[u8], flags: i32) -> Result<usize, VmError>;

    /// 接收数据
    fn recv(&mut self, fd: i32, buf: &mut [u8], flags: i32) -> Result<usize, VmError>;

    /// 发送数据到指定地址
    fn sendto(&mut self, fd: i32, buf: &[u8], flags: i32, addr: SocketAddress) -> Result<usize, VmError>;

    /// 从指定地址接收数据
    fn recvfrom(&mut self, fd: i32, buf: &mut [u8], flags: i32) -> Result<(usize, SocketAddress), VmError>;

    /// 关闭套接字
    fn close(&mut self, fd: i32) -> Result<(), VmError>;

    /// 设置套接字选项
    fn setsockopt(&mut self, fd: i32, level: SocketOptionLevel, opt: SocketOption, value: &[u8]) -> Result<(), VmError>;

    /// 获取套接字选项
    fn getsockopt(&mut self, fd: i32, level: SocketOptionLevel, opt: SocketOption) -> Result<Vec<u8>, VmError>;
}

/// 网络栈兼容层
pub struct NetworkCompatibilityLayer {
    /// Guest 架构
    guest_arch: GuestArch,
    /// 套接字表
    sockets: Arc<Mutex<HashMap<i32, SocketInfo>>>,
    /// 下一个可用的文件描述符
    next_fd: Arc<Mutex<i32>>,
    /// 网络栈操作实现
    net_ops: Arc<Mutex<Box<dyn NetworkStackOperations>>>,
}

impl NetworkCompatibilityLayer {
    /// 创建新的网络栈兼容层
    pub fn new(guest_arch: GuestArch, net_ops: Box<dyn NetworkStackOperations>) -> Self {
        Self {
            guest_arch,
            sockets: Arc::new(Mutex::new(HashMap::new())),
            next_fd: Arc::new(Mutex::new(3)), // 0, 1, 2 are stdin, stdout, stderr
            net_ops: Arc::new(Mutex::new(net_ops)),
        }
    }

    /// 创建套接字
    pub fn socket(&self, domain: u32, socket_type: u32, protocol: u32) -> Result<i32, VmError> {
        let domain = Self::parse_domain(domain)?;
        let socket_type = Self::parse_socket_type(socket_type)?;
        let protocol = Self::parse_protocol(protocol)?;

        let mut net_ops = self.net_ops.lock().map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to lock network operations: {:?}", e),
                module: "NetworkCompatibilityLayer".to_string(),
            })
        })?;

        let fd = net_ops.socket(domain, socket_type, protocol)?;

        // 记录套接字信息
        let mut sockets = self.sockets.lock().map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to lock sockets: {:?}", e),
                module: "NetworkCompatibilityLayer".to_string(),
            })
        })?;

        let socket_info = SocketInfo {
            fd,
            domain,
            socket_type,
            protocol,
            local_addr: None,
            remote_addr: None,
            options: HashMap::new(),
        };

        sockets.insert(fd, socket_info);
        debug!("Created socket: fd={}, domain={:?}, type={:?}, protocol={:?}", fd, domain, socket_type, protocol);

        Ok(fd)
    }

    /// 绑定套接字
    pub fn bind(&self, fd: i32, addr: GuestAddr, addr_len: u32) -> Result<(), VmError> {
        let addr = SocketAddress::from_guest_memory(self.guest_arch, addr, addr_len)?;

        let mut net_ops = self.net_ops.lock().map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to lock network operations: {:?}", e),
                module: "NetworkCompatibilityLayer".to_string(),
            })
        })?;

        net_ops.bind(fd, addr.clone())?;

        // 更新套接字信息
        let mut sockets = self.sockets.lock().map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to lock sockets: {:?}", e),
                module: "NetworkCompatibilityLayer".to_string(),
            })
        })?;

        if let Some(socket_info) = sockets.get_mut(&fd) {
            socket_info.local_addr = Some(addr);
        }

        Ok(())
    }

    /// 监听套接字
    pub fn listen(&self, fd: i32, backlog: i32) -> Result<(), VmError> {
        let mut net_ops = self.net_ops.lock().map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to lock network operations: {:?}", e),
                module: "NetworkCompatibilityLayer".to_string(),
            })
        })?;

        net_ops.listen(fd, backlog)
    }

    /// 接受连接
    pub fn accept(&self, fd: i32, addr: GuestAddr, addr_len: &mut u32) -> Result<i32, VmError> {
        let mut net_ops = self.net_ops.lock().map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to lock network operations: {:?}", e),
                module: "NetworkCompatibilityLayer".to_string(),
            })
        })?;

        let (new_fd, remote_addr) = net_ops.accept(fd)?;

        // 记录新套接字信息
        let mut sockets = self.sockets.lock().map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to lock sockets: {:?}", e),
                module: "NetworkCompatibilityLayer".to_string(),
            })
        })?;

        if let Some(parent_socket) = sockets.get(&fd) {
            let socket_info = SocketInfo {
                fd: new_fd,
                domain: parent_socket.domain,
                socket_type: parent_socket.socket_type,
                protocol: parent_socket.protocol,
                local_addr: parent_socket.local_addr.clone(),
                remote_addr: Some(remote_addr),
                options: parent_socket.options.clone(),
            };

            sockets.insert(new_fd, socket_info);
        }

        Ok(new_fd)
    }

    /// 连接到远程地址
    pub fn connect(&self, fd: i32, addr: GuestAddr, addr_len: u32) -> Result<(), VmError> {
        let addr = SocketAddress::from_guest_memory(self.guest_arch, addr, addr_len)?;

        let mut net_ops = self.net_ops.lock().map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to lock network operations: {:?}", e),
                module: "NetworkCompatibilityLayer".to_string(),
            })
        })?;

        net_ops.connect(fd, addr.clone())?;

        // 更新套接字信息
        let mut sockets = self.sockets.lock().map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to lock sockets: {:?}", e),
                module: "NetworkCompatibilityLayer".to_string(),
            })
        })?;

        if let Some(socket_info) = sockets.get_mut(&fd) {
            socket_info.remote_addr = Some(addr);
        }

        Ok(())
    }

    /// 发送数据
    pub fn send(&self, fd: i32, buf: &[u8], flags: i32) -> Result<usize, VmError> {
        let mut net_ops = self.net_ops.lock().map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to lock network operations: {:?}", e),
                module: "NetworkCompatibilityLayer".to_string(),
            })
        })?;

        net_ops.send(fd, buf, flags)
    }

    /// 接收数据
    pub fn recv(&self, fd: i32, buf: &mut [u8], flags: i32) -> Result<usize, VmError> {
        let mut net_ops = self.net_ops.lock().map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to lock network operations: {:?}", e),
                module: "NetworkCompatibilityLayer".to_string(),
            })
        })?;

        net_ops.recv(fd, buf, flags)
    }

    /// 发送数据到指定地址
    pub fn sendto(&self, fd: i32, buf: &[u8], flags: i32, addr: GuestAddr, addr_len: u32) -> Result<usize, VmError> {
        let addr = SocketAddress::from_guest_memory(self.guest_arch, addr, addr_len)?;

        let mut net_ops = self.net_ops.lock().map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to lock network operations: {:?}", e),
                module: "NetworkCompatibilityLayer".to_string(),
            })
        })?;

        net_ops.sendto(fd, buf, flags, addr)
    }

    /// 从指定地址接收数据
    pub fn recvfrom(&self, fd: i32, buf: &mut [u8], flags: i32, addr: GuestAddr, addr_len: &mut u32) -> Result<usize, VmError> {
        let mut net_ops = self.net_ops.lock().map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to lock network operations: {:?}", e),
                module: "NetworkCompatibilityLayer".to_string(),
            })
        })?;

        let (size, remote_addr) = net_ops.recvfrom(fd, buf, flags)?;
        // TODO: 将 remote_addr 写入 guest 内存
        Ok(size)
    }

    /// 关闭套接字
    pub fn close(&self, fd: i32) -> Result<(), VmError> {
        let mut net_ops = self.net_ops.lock().map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to lock network operations: {:?}", e),
                module: "NetworkCompatibilityLayer".to_string(),
            })
        })?;

        net_ops.close(fd)?;

        // 从套接字表中移除
        let mut sockets = self.sockets.lock().map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to lock sockets: {:?}", e),
                module: "NetworkCompatibilityLayer".to_string(),
            })
        })?;

        sockets.remove(&fd);
        debug!("Closed socket: fd={}", fd);

        Ok(())
    }

    /// 设置套接字选项
    pub fn setsockopt(&self, fd: i32, level: u32, opt: u32, value: &[u8]) -> Result<(), VmError> {
        let level = Self::parse_option_level(level)?;
        let opt = Self::parse_option(opt)?;

        let mut net_ops = self.net_ops.lock().map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to lock network operations: {:?}", e),
                module: "NetworkCompatibilityLayer".to_string(),
            })
        })?;

        net_ops.setsockopt(fd, level, opt, value)?;

        // 更新套接字选项
        let mut sockets = self.sockets.lock().map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to lock sockets: {:?}", e),
                module: "NetworkCompatibilityLayer".to_string(),
            })
        })?;

        if let Some(socket_info) = sockets.get_mut(&fd) {
            socket_info.options.insert((level, opt), value.to_vec());
        }

        Ok(())
    }

    /// 获取套接字选项
    pub fn getsockopt(&self, fd: i32, level: u32, opt: u32) -> Result<Vec<u8>, VmError> {
        let level = Self::parse_option_level(level)?;
        let opt = Self::parse_option(opt)?;

        let mut net_ops = self.net_ops.lock().map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to lock network operations: {:?}", e),
                module: "NetworkCompatibilityLayer".to_string(),
            })
        })?;

        net_ops.getsockopt(fd, level, opt)
    }

    /// 解析套接字域
    fn parse_domain(domain: u32) -> Result<SocketDomain, VmError> {
        match domain {
            1 => Ok(SocketDomain::Unix),
            2 => Ok(SocketDomain::Inet),
            10 => Ok(SocketDomain::Inet6),
            16 => Ok(SocketDomain::Netlink),
            _ => Err(VmError::Core(vm_core::CoreError::InvalidArgument {
                argument: "domain".to_string(),
                value: domain.to_string(),
                reason: "Invalid socket domain".to_string(),
            })),
        }
    }

    /// 解析套接字类型
    fn parse_socket_type(socket_type: u32) -> Result<SocketType, VmError> {
        match socket_type {
            1 => Ok(SocketType::Stream),
            2 => Ok(SocketType::Datagram),
            3 => Ok(SocketType::Raw),
            4 => Ok(SocketType::Rdm),
            5 => Ok(SocketType::SeqPacket),
            6 => Ok(SocketType::Dccp),
            _ => Err(VmError::Core(vm_core::CoreError::InvalidArgument {
                argument: "socket_type".to_string(),
                value: socket_type.to_string(),
                reason: "Invalid socket type".to_string(),
            })),
        }
    }

    /// 解析套接字协议
    fn parse_protocol(protocol: u32) -> Result<SocketProtocol, VmError> {
        match protocol {
            0 => Ok(SocketProtocol::Ip),
            6 => Ok(SocketProtocol::Tcp),
            17 => Ok(SocketProtocol::Udp),
            1 => Ok(SocketProtocol::Icmp),
            58 => Ok(SocketProtocol::Icmpv6),
            _ => Ok(SocketProtocol::Ip), // 默认使用IP协议
        }
    }

    /// 解析套接字选项级别
    fn parse_option_level(level: u32) -> Result<SocketOptionLevel, VmError> {
        match level {
            1 => Ok(SocketOptionLevel::Socket),
            6 => Ok(SocketOptionLevel::Tcp),
            0 => Ok(SocketOptionLevel::Ip),
            41 => Ok(SocketOptionLevel::Ipv6),
            _ => Err(VmError::Core(vm_core::CoreError::InvalidArgument {
                argument: "level".to_string(),
                value: level.to_string(),
                reason: "Invalid socket option level".to_string(),
            })),
        }
    }

    /// 解析套接字选项
    fn parse_option(opt: u32) -> Result<SocketOption, VmError> {
        match opt {
            2 => Ok(SocketOption::ReuseAddr),
            9 => Ok(SocketOption::KeepAlive),
            6 => Ok(SocketOption::Broadcast),
            13 => Ok(SocketOption::Linger),
            7 => Ok(SocketOption::SendBuf),
            8 => Ok(SocketOption::RecvBuf),
            4 => Ok(SocketOption::Error),
            3 => Ok(SocketOption::Type),
            1 => Ok(SocketOption::TcpNoDelay),
            4 => Ok(SocketOption::TcpKeepIdle),
            5 => Ok(SocketOption::TcpKeepIntvl),
            6 => Ok(SocketOption::TcpKeepCnt),
            _ => Err(VmError::Core(vm_core::CoreError::InvalidArgument {
                argument: "opt".to_string(),
                value: opt.to_string(),
                reason: "Invalid socket option".to_string(),
            })),
        }
    }
}

/// 默认网络栈操作实现
pub struct DefaultNetworkStackOperations;

impl NetworkStackOperations for DefaultNetworkStackOperations {
    fn socket(&mut self, _domain: SocketDomain, _socket_type: SocketType, _protocol: SocketProtocol) -> Result<i32, VmError> {
        Err(VmError::Core(vm_core::CoreError::NotImplemented {
            feature: "DefaultNetworkStackOperations::socket".to_string(),
            module: "NetworkCompatibilityLayer".to_string(),
        }))
    }

    fn bind(&mut self, _fd: i32, _addr: SocketAddress) -> Result<(), VmError> {
        Err(VmError::Core(vm_core::CoreError::NotImplemented {
            feature: "DefaultNetworkStackOperations::bind".to_string(),
            module: "NetworkCompatibilityLayer".to_string(),
        }))
    }

    fn listen(&mut self, _fd: i32, _backlog: i32) -> Result<(), VmError> {
        Err(VmError::Core(vm_core::CoreError::NotImplemented {
            feature: "DefaultNetworkStackOperations::listen".to_string(),
            module: "NetworkCompatibilityLayer".to_string(),
        }))
    }

    fn accept(&mut self, _fd: i32) -> Result<(i32, SocketAddress), VmError> {
        Err(VmError::Core(vm_core::CoreError::NotImplemented {
            feature: "DefaultNetworkStackOperations::accept".to_string(),
            module: "NetworkCompatibilityLayer".to_string(),
        }))
    }

    fn connect(&mut self, _fd: i32, _addr: SocketAddress) -> Result<(), VmError> {
        Err(VmError::Core(vm_core::CoreError::NotImplemented {
            feature: "DefaultNetworkStackOperations::connect".to_string(),
            module: "NetworkCompatibilityLayer".to_string(),
        }))
    }

    fn send(&mut self, _fd: i32, _buf: &[u8], _flags: i32) -> Result<usize, VmError> {
        Err(VmError::Core(vm_core::CoreError::NotImplemented {
            feature: "DefaultNetworkStackOperations::send".to_string(),
            module: "NetworkCompatibilityLayer".to_string(),
        }))
    }

    fn recv(&mut self, _fd: i32, _buf: &mut [u8], _flags: i32) -> Result<usize, VmError> {
        Err(VmError::Core(vm_core::CoreError::NotImplemented {
            feature: "DefaultNetworkStackOperations::recv".to_string(),
            module: "NetworkCompatibilityLayer".to_string(),
        }))
    }

    fn sendto(&mut self, _fd: i32, _buf: &[u8], _flags: i32, _addr: SocketAddress) -> Result<usize, VmError> {
        Err(VmError::Core(vm_core::CoreError::NotImplemented {
            feature: "DefaultNetworkStackOperations::sendto".to_string(),
            module: "NetworkCompatibilityLayer".to_string(),
        }))
    }

    fn recvfrom(&mut self, _fd: i32, _buf: &mut [u8], _flags: i32) -> Result<(usize, SocketAddress), VmError> {
        Err(VmError::Core(vm_core::CoreError::NotImplemented {
            feature: "DefaultNetworkStackOperations::recvfrom".to_string(),
            module: "NetworkCompatibilityLayer".to_string(),
        }))
    }

    fn close(&mut self, _fd: i32) -> Result<(), VmError> {
        Ok(())
    }

    fn setsockopt(&mut self, _fd: i32, _level: SocketOptionLevel, _opt: SocketOption, _value: &[u8]) -> Result<(), VmError> {
        Err(VmError::Core(vm_core::CoreError::NotImplemented {
            feature: "DefaultNetworkStackOperations::setsockopt".to_string(),
            module: "NetworkCompatibilityLayer".to_string(),
        }))
    }

    fn getsockopt(&mut self, _fd: i32, _level: SocketOptionLevel, _opt: SocketOption) -> Result<Vec<u8>, VmError> {
        Err(VmError::Core(vm_core::CoreError::NotImplemented {
            feature: "DefaultNetworkStackOperations::getsockopt".to_string(),
            module: "NetworkCompatibilityLayer".to_string(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_socket_domain_parsing() {
        assert_eq!(NetworkCompatibilityLayer::parse_domain(2).unwrap(), SocketDomain::Inet);
        assert_eq!(NetworkCompatibilityLayer::parse_domain(10).unwrap(), SocketDomain::Inet6);
    }

    #[test]
    fn test_socket_type_parsing() {
        assert_eq!(NetworkCompatibilityLayer::parse_socket_type(1).unwrap(), SocketType::Stream);
        assert_eq!(NetworkCompatibilityLayer::parse_socket_type(2).unwrap(), SocketType::Datagram);
    }

    #[test]
    fn test_socket_protocol_parsing() {
        assert_eq!(NetworkCompatibilityLayer::parse_protocol(6).unwrap(), SocketProtocol::Tcp);
        assert_eq!(NetworkCompatibilityLayer::parse_protocol(17).unwrap(), SocketProtocol::Udp);
    }
}

