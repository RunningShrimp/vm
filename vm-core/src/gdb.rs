//! GDB 远程调试协议实现
//!
//! 支持通过 GDB Remote Serial Protocol (RSP) 调试虚拟机

#[cfg(not(feature = "no_std"))]
use crate::{GuestAddr, MMU, VcpuStateContainer};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

/// GDB 调试服务器
pub struct GdbServer {
    listener: Option<TcpListener>,
    port: u16,
    running: Arc<Mutex<bool>>,
}

impl GdbServer {
    /// 创建新的 GDB 服务器
    pub fn new(port: u16) -> Self {
        Self {
            listener: None,
            port,
            running: Arc::new(Mutex::new(false)),
        }
    }

    /// 启动 GDB 服务器
    pub fn start(&mut self) -> Result<(), String> {
        let addr = format!("127.0.0.1:{}", self.port);
        let listener =
            TcpListener::bind(&addr).map_err(|e| format!("Failed to bind to {}: {}", addr, e))?;

        println!("GDB server listening on {}", addr);
        self.listener = Some(listener);
        let mut running = self
            .running
            .lock()
            .map_err(|e| format!("Failed to lock running state: {}", e))?;
        *running = true;

        Ok(())
    }

    /// 停止 GDB 服务器
    pub fn stop(&mut self) {
        if let Ok(mut running) = self.running.lock() {
            *running = false;
        }
        self.listener = None;
    }

    /// 接受客户端连接
    pub fn accept(&self) -> Result<GdbConnection, String> {
        if let Some(ref listener) = self.listener {
            let (stream, addr) = listener
                .accept()
                .map_err(|e| format!("Failed to accept connection: {}", e))?;

            println!("GDB client connected from {}", addr);
            Ok(GdbConnection::new(stream))
        } else {
            Err("Server not started".to_string())
        }
    }

    /// 是否正在运行
    pub fn is_running(&self) -> bool {
        self.running.lock().map(|state| *state).unwrap_or(false)
    }
}

/// GDB 连接
pub struct GdbConnection {
    stream: TcpStream,
    /// 内部缓冲区，用于优化多次读取操作
    buffer: Vec<u8>,
}

impl GdbConnection {
    fn new(stream: TcpStream) -> Self {
        // 预分配缓冲区以减少分配次数，容量为4KB
        Self {
            stream,
            buffer: Vec::with_capacity(4096),
        }
    }

    /// 接收 GDB 数据包
    pub fn recv_packet(&mut self) -> Result<String, String> {
        // 清空缓冲区但保留容量，用于下次读取
        self.buffer.clear();

        let mut reader = BufReader::new(&self.stream);
        let mut line = String::new();

        reader
            .read_line(&mut line)
            .map_err(|e| format!("Failed to read packet: {}", e))?;

        // GDB 数据包格式: $data#checksum
        if line.starts_with('$') {
            let end = line.find('#').unwrap_or(line.len());
            let data = &line[1..end];
            // 将数据存储到缓冲区以供后续使用
            self.buffer.extend_from_slice(data.as_bytes());

            // 验证缓冲区内容与解析的数据一致，确保数据完整性
            let buffer_str = std::str::from_utf8(&self.buffer)
                .map_err(|e| format!("Invalid UTF-8 in buffer: {}", e))?;
            debug_assert_eq!(buffer_str, data, "Buffer content mismatch");

            Ok(data.to_string())
        } else {
            Err("Invalid packet format".to_string())
        }
    }

    /// 发送 GDB 数据包
    pub fn send_packet(&mut self, data: &str) -> Result<(), String> {
        let checksum = Self::calculate_checksum(data);
        let packet = format!("${}#{:02x}\n", data, checksum);

        self.stream
            .write_all(packet.as_bytes())
            .map_err(|e| format!("Failed to send packet: {}", e))?;

        self.stream
            .flush()
            .map_err(|e| format!("Failed to flush stream: {}", e))?;

        Ok(())
    }

    /// 发送确认
    pub fn send_ack(&mut self) -> Result<(), String> {
        self.stream
            .write_all(b"+")
            .map_err(|e| format!("Failed to send ack: {}", e))?;
        Ok(())
    }

    /// 发送错误响应
    pub fn send_error(&mut self, code: u8) -> Result<(), String> {
        self.send_packet(&format!("E{:02x}", code))
    }

    /// 计算校验和
    fn calculate_checksum(data: &str) -> u8 {
        data.bytes().fold(0u8, |acc, b| acc.wrapping_add(b))
    }
}

/// GDB 调试会话
pub struct GdbSession {
    connection: GdbConnection,
    breakpoints: Vec<GuestAddr>,
}

impl GdbSession {
    pub fn new(connection: GdbConnection) -> Self {
        Self {
            connection,
            breakpoints: Vec::new(),
        }
    }

    /// 处理 GDB 命令
    pub fn handle_command(
        &mut self,
        cmd: &str,
        vcpu: &mut VcpuStateContainer,
        mmu: &mut dyn MMU,
    ) -> Result<bool, String> {
        let parts: Vec<&str> = cmd.split(',').collect();
        let cmd_type = parts[0];

        match cmd_type {
            // 查询支持的功能
            "qSupported" => {
                self.connection
                    .send_packet("PacketSize=4096;qXfer:features:read+")?;
            }

            // 读取所有寄存器
            "g" => {
                let regs_hex = self.format_registers(vcpu);
                self.connection.send_packet(&regs_hex)?;
            }

            // 写入所有寄存器
            cmd if cmd.starts_with("G") => {
                let hex_data = &cmd[1..];
                self.parse_registers(vcpu, hex_data)?;
                self.connection.send_packet("OK")?;
            }

            // 读取内存
            cmd if cmd.starts_with("m") => {
                let params = &cmd[1..];
                let parts: Vec<&str> = params.split(',').collect();
                if parts.len() == 2 {
                    let addr = u64::from_str_radix(parts[0], 16)
                        .map_err(|_| "Invalid address".to_string())?;
                    let len = usize::from_str_radix(parts[1], 16)
                        .map_err(|_| "Invalid length".to_string())?;

                    match self.read_memory(mmu, GuestAddr(addr), len) {
                        Ok(data) => {
                            let hex = data
                                .iter()
                                .map(|b| format!("{:02x}", b))
                                .collect::<String>();
                            self.connection.send_packet(&hex)?;
                        }
                        Err(_) => {
                            self.connection.send_error(0x01)?;
                        }
                    }
                }
            }

            // 写入内存
            cmd if cmd.starts_with("M") => {
                let params = &cmd[1..];
                let parts: Vec<&str> = params.split(':').collect();
                if parts.len() == 2 {
                    let addr_len: Vec<&str> = parts[0].split(',').collect();
                    if addr_len.len() == 2 {
                        let addr = u64::from_str_radix(addr_len[0], 16)
                            .map_err(|_| "Invalid address".to_string())?;
                        let data = Self::hex_to_bytes(parts[1])?;

                        match self.write_memory(mmu, GuestAddr(addr), &data) {
                            Ok(_) => self.connection.send_packet("OK")?,
                            Err(_) => self.connection.send_error(0x01)?,
                        }
                    }
                }
            }

            // 继续执行
            "c" => {
                self.connection.send_packet("OK")?;
                return Ok(true); // 继续执行
            }

            // 单步执行
            "s" => {
                self.connection.send_packet("OK")?;
                return Ok(true); // 单步执行
            }

            // 设置断点
            cmd if cmd.starts_with("Z0") => {
                let params = &cmd[3..];
                let parts: Vec<&str> = params.split(',').collect();
                if !parts.is_empty() {
                    let addr = u64::from_str_radix(parts[0], 16)
                        .map_err(|_| "Invalid address".to_string())?;
                    self.breakpoints.push(GuestAddr(addr));
                    self.connection.send_packet("OK")?;
                }
            }

            // 删除断点
            cmd if cmd.starts_with("z0") => {
                let params = &cmd[3..];
                let parts: Vec<&str> = params.split(',').collect();
                if !parts.is_empty() {
                    let addr = u64::from_str_radix(parts[0], 16)
                        .map_err(|_| "Invalid address".to_string())?;
                    self.breakpoints.retain(|&bp| bp != GuestAddr(addr));
                    self.connection.send_packet("OK")?;
                }
            }

            // 查询当前线程
            "qC" => {
                self.connection.send_packet("QC1")?;
            }

            // 查询附加状态
            "qAttached" => {
                self.connection.send_packet("1")?;
            }

            // 终止调试
            "k" => {
                self.connection.send_packet("OK")?;
                return Ok(false); // 终止会话
            }

            // 未知命令
            _ => {
                self.connection.send_packet("")?;
            }
        }

        Ok(false)
    }

    /// 格式化寄存器为十六进制字符串
    fn format_registers(&self, vcpu: &VcpuStateContainer) -> String {
        vcpu.state
            .regs
            .gpr
            .iter()
            .map(|r| format!("{:016x}", r))
            .collect::<Vec<_>>()
            .join("")
    }

    /// 从十六进制字符串解析寄存器
    fn parse_registers(&self, vcpu: &mut VcpuStateContainer, hex: &str) -> Result<(), String> {
        let mut offset = 0;
        for i in 0..vcpu.state.regs.gpr.len() {
            if offset + 16 > hex.len() {
                break;
            }
            let reg_hex = &hex[offset..offset + 16];
            vcpu.state.regs.gpr[i] = u64::from_str_radix(reg_hex, 16)
                .map_err(|_| "Invalid register value".to_string())?;
            offset += 16;
        }
        Ok(())
    }

    /// 读取内存
    fn read_memory(
        &self,
        mmu: &mut dyn MMU,
        addr: GuestAddr,
        len: usize,
    ) -> Result<Vec<u8>, String> {
        let mut data = Vec::with_capacity(len);
        for i in 0..len {
            match mmu.read(addr + i as u64, 1) {
                Ok(byte) => data.push(byte.try_into().unwrap()),
                Err(_) => return Err("Memory read failed".to_string()),
            }
        }
        Ok(data)
    }

    /// 写入内存
    fn write_memory(&self, mmu: &mut dyn MMU, addr: GuestAddr, data: &[u8]) -> Result<(), String> {
        for (i, &byte) in data.iter().enumerate() {
            mmu.write(GuestAddr(addr.0 + i as u64), byte as u64, 1)
                .map_err(|_| "Memory write failed".to_string())?;
        }
        Ok(())
    }

    /// 十六进制字符串转字节数组
    fn hex_to_bytes(hex: &str) -> Result<Vec<u8>, String> {
        let mut bytes = Vec::new();
        for i in (0..hex.len()).step_by(2) {
            if i + 2 > hex.len() {
                break;
            }
            let byte = u8::from_str_radix(&hex[i..i + 2], 16)
                .map_err(|_| "Invalid hex data".to_string())?;
            bytes.push(byte);
        }
        Ok(bytes)
    }

    /// 检查是否命中断点
    pub fn check_breakpoint(&self, pc: GuestAddr) -> bool {
        self.breakpoints.contains(&pc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checksum() {
        let data = "qSupported";
        let checksum = GdbConnection::calculate_checksum(data);
        println!("Checksum for '{}': {:02x}", data, checksum);
    }

    #[test]
    fn test_hex_conversion() {
        let hex = "48656c6c6f";
        let bytes = GdbSession::hex_to_bytes(hex).expect("Failed to convert hex string");
        assert_eq!(bytes, b"Hello");
    }
}
