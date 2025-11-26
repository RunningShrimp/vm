//! WHPX I/O 和 MMIO 处理
//!
//! 实现 Windows Hypervisor Platform 的 I/O 和 MMIO 拦截与处理

use vm_core::GuestAddr;

/// I/O 端口访问类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IoAccessType {
    In,
    Out,
}

/// I/O 端口访问
#[derive(Debug, Clone)]
pub struct IoAccess {
    pub port: u16,
    pub access_type: IoAccessType,
    pub size: u8,  // 1, 2, 4
    pub data: u32,
}

/// MMIO 访问类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MmioAccessType {
    Read,
    Write,
}

/// MMIO 访问
#[derive(Debug, Clone)]
pub struct MmioAccess {
    pub addr: GuestAddr,
    pub access_type: MmioAccessType,
    pub size: u8,  // 1, 2, 4, 8
    pub data: u64,
}

/// I/O 处理器
pub struct IoHandler {
    // I/O 端口映射
    io_handlers: Vec<(u16, u16, Box<dyn Fn(&IoAccess) -> Option<u32>>)>,
}

impl IoHandler {
    pub fn new() -> Self {
        Self {
            io_handlers: Vec::new(),
        }
    }

    /// 注册 I/O 端口处理器
    pub fn register_io_handler<F>(&mut self, port_start: u16, port_end: u16, handler: F)
    where
        F: Fn(&IoAccess) -> Option<u32> + 'static,
    {
        self.io_handlers.push((port_start, port_end, Box::new(handler)));
    }

    /// 处理 I/O 端口访问
    pub fn handle_io(&self, access: &IoAccess) -> Option<u32> {
        for (start, end, handler) in &self.io_handlers {
            if access.port >= *start && access.port <= *end {
                return handler(access);
            }
        }
        None
    }
}

/// MMIO 处理器
pub struct MmioHandler {
    // MMIO 地址范围映射
    mmio_handlers: Vec<(GuestAddr, GuestAddr, Box<dyn Fn(&MmioAccess) -> Option<u64>>)>,
}

impl MmioHandler {
    pub fn new() -> Self {
        Self {
            mmio_handlers: Vec::new(),
        }
    }

    /// 注册 MMIO 处理器
    pub fn register_mmio_handler<F>(&mut self, addr_start: GuestAddr, addr_end: GuestAddr, handler: F)
    where
        F: Fn(&MmioAccess) -> Option<u64> + 'static,
    {
        self.mmio_handlers.push((addr_start, addr_end, Box::new(handler)));
    }

    /// 处理 MMIO 访问
    pub fn handle_mmio(&self, access: &MmioAccess) -> Option<u64> {
        for (start, end, handler) in &self.mmio_handlers {
            if access.addr >= *start && access.addr <= *end {
                return handler(access);
            }
        }
        None
    }
}

/// WHPX 退出处理器
pub struct WhpxExitHandler {
    io_handler: IoHandler,
    mmio_handler: MmioHandler,
}

impl WhpxExitHandler {
    pub fn new() -> Self {
        Self {
            io_handler: IoHandler::new(),
            mmio_handler: MmioHandler::new(),
        }
    }

    /// 获取 I/O 处理器
    pub fn io_handler_mut(&mut self) -> &mut IoHandler {
        &mut self.io_handler
    }

    /// 获取 MMIO 处理器
    pub fn mmio_handler_mut(&mut self) -> &mut MmioHandler {
        &mut self.mmio_handler
    }

    /// 处理 VM 退出
    #[cfg(all(target_os = "windows", feature = "whpx"))]
    pub fn handle_exit(&self, exit_context: &WHV_RUN_VP_EXIT_CONTEXT) -> Result<(), String> {
        use windows_sys::Win32::System::Hypervisor::*;

        match exit_context.ExitReason {
            WHvRunVpExitReasonX64IoPortAccess => {
                // 处理 I/O 端口访问
                let io_info = unsafe { &exit_context.Anonymous.IoPortAccess };
                
                let access = IoAccess {
                    port: io_info.PortNumber,
                    access_type: if io_info.AccessInfo.IsWrite() != 0 {
                        IoAccessType::Out
                    } else {
                        IoAccessType::In
                    },
                    size: io_info.AccessInfo.AccessSize() as u8,
                    data: 0, // 对于 IN 操作，这里是输入数据
                };

                if let Some(result) = self.io_handler.handle_io(&access) {
                    // 将结果写回到寄存器（简化示例）
                    println!("I/O port access handled: port={:#x}, result={:#x}", access.port, result);
                    Ok(())
                } else {
                    Err(format!("Unhandled I/O port access: {:#x}", access.port))
                }
            }
            WHvRunVpExitReasonMemoryAccess => {
                // 处理 MMIO 访问
                let mem_info = unsafe { &exit_context.Anonymous.MemoryAccess };
                
                let access = MmioAccess {
                    addr: mem_info.Gpa,
                    access_type: if mem_info.AccessInfo.AccessType() == WHV_MEMORY_ACCESS_TYPE_WRITE {
                        MmioAccessType::Write
                    } else {
                        MmioAccessType::Read
                    },
                    size: mem_info.AccessInfo.AccessSize() as u8,
                    data: 0, // 对于读操作，这里是读取的数据
                };

                if let Some(result) = self.mmio_handler.handle_mmio(&access) {
                    // 将结果写回到寄存器（简化示例）
                    println!("MMIO access handled: addr={:#x}, result={:#x}", access.addr, result);
                    Ok(())
                } else {
                    Err(format!("Unhandled MMIO access: {:#x}", access.addr))
                }
            }
            _ => {
                Err(format!("Unhandled exit reason: {}", exit_context.ExitReason))
            }
        }
    }

    #[cfg(not(all(target_os = "windows", feature = "whpx")))]
    pub fn handle_exit(&self, _exit_context: &()) -> Result<(), String> {
        Err("WHPX not available on this platform".to_string())
    }
}

#[cfg(all(target_os = "windows", feature = "whpx"))]
use windows_sys::Win32::System::Hypervisor::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_io_handler() {
        let mut handler = IoHandler::new();
        
        // 注册一个简单的 I/O 处理器
        handler.register_io_handler(0x3F8, 0x3FF, |access| {
            // 模拟串口
            if access.access_type == IoAccessType::In {
                Some(0x60) // 返回一个虚拟值
            } else {
                Some(0)
            }
        });

        let access = IoAccess {
            port: 0x3F8,
            access_type: IoAccessType::In,
            size: 1,
            data: 0,
        };

        assert_eq!(handler.handle_io(&access), Some(0x60));
    }

    #[test]
    fn test_mmio_handler() {
        let mut handler = MmioHandler::new();
        
        // 注册一个简单的 MMIO 处理器
        handler.register_mmio_handler(0x10000000, 0x10000FFF, |access| {
            // 模拟设备寄存器
            if access.access_type == MmioAccessType::Read {
                Some(0x12345678)
            } else {
                Some(0)
            }
        });

        let access = MmioAccess {
            addr: 0x10000000,
            access_type: MmioAccessType::Read,
            size: 4,
            data: 0,
        };

        assert_eq!(handler.handle_mmio(&access), Some(0x12345678));
    }
}
