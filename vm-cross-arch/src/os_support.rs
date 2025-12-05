//! 操作系统级别支持
//!
//! 提供系统调用模拟、设备I/O、中断处理等操作系统级别的功能

use std::collections::HashMap;
use vm_core::{GuestAddr, MMU, VmError};

/// 系统调用处理器
pub trait SyscallHandler: Send + Sync {
    /// 处理系统调用
    fn handle_syscall(
        &mut self,
        syscall_num: u64,
        args: &[u64],
        mmu: &mut dyn MMU,
    ) -> Result<u64, VmError>;
}

/// Linux系统调用处理器
pub struct LinuxSyscallHandler {
    /// 文件描述符表
    fd_table: HashMap<u32, FileDescriptor>,
    /// 下一个文件描述符
    next_fd: u32,
}

impl LinuxSyscallHandler {
    pub fn new() -> Self {
        let mut fd_table = HashMap::new();
        // 标准输入、输出、错误
        fd_table.insert(0, FileDescriptor::Stdin);
        fd_table.insert(1, FileDescriptor::Stdout);
        fd_table.insert(2, FileDescriptor::Stderr);

        Self {
            fd_table,
            next_fd: 3,
        }
    }
}

impl SyscallHandler for LinuxSyscallHandler {
    fn handle_syscall(
        &mut self,
        syscall_num: u64,
        args: &[u64],
        mmu: &mut dyn MMU,
    ) -> Result<u64, VmError> {
        match syscall_num {
            // write系统调用
            1 => {
                let fd = args[0] as u32;
                let buf_addr = args[1] as GuestAddr;
                let count = args[2] as usize;

                // 从MMU读取数据
                // 1. 将虚拟地址转换为物理地址
                let phys_addr = mmu.translate(buf_addr, vm_core::AccessType::Read)
                    .map_err(|e| VmError::Core(vm_core::CoreError::Internal {
                        message: format!("Failed to translate address {:#x}: {:?}", buf_addr, e),
                        module: "os_support".to_string(),
                    }))?;

                // 2. 批量读取数据
                let mut buf = vec![0u8; count];
                mmu.read_bulk(phys_addr, &mut buf)
                    .map_err(|e| VmError::Core(vm_core::CoreError::Internal {
                        message: format!("Failed to read {} bytes from address {:#x}: {:?}", count, phys_addr, e),
                        module: "os_support".to_string(),
                    }))?;

                // 写入到标准输出
                if fd == 1 || fd == 2 {
                    print!("{}", String::from_utf8_lossy(&buf));
                    Ok(count as u64)
                } else {
                    Err(VmError::Device(
                        vm_core::DeviceError::UnsupportedOperation {
                            device_type: "Syscall".to_string(),
                            operation: format!("write fd {}", fd),
                        },
                    ))
                }
            }
            // exit系统调用
            60 => {
                let exit_code = args[0] as i32;
                tracing::info!("Process exited with code: {}", exit_code);
                Ok(0)
            }
            // 其他系统调用
            _ => {
                tracing::warn!("Unhandled syscall: {}", syscall_num);
                Ok(0)
            }
        }
    }
}

/// 文件描述符
#[derive(Debug, Clone)]
enum FileDescriptor {
    Stdin,
    Stdout,
    Stderr,
    File(String),
    Socket(u32),
}

/// 设备模拟器
pub trait DeviceEmulator: Send + Sync {
    /// 读取设备
    fn read(&mut self, offset: u64, size: usize) -> Result<Vec<u8>, VmError>;

    /// 写入设备
    fn write(&mut self, offset: u64, data: &[u8]) -> Result<(), VmError>;

    /// 获取设备类型
    fn device_type(&self) -> DeviceType;
}

/// 设备类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceType {
    VirtioBlk,
    VirtioNet,
    VirtioConsole,
    Rtc,
    Uart,
}

/// 中断控制器
pub struct InterruptController {
    /// 中断向量表
    interrupt_handlers: HashMap<u32, Box<dyn Fn() -> Result<(), VmError> + Send + Sync>>,
    /// 待处理的中断
    pending_interrupts: Vec<u32>,
}

impl InterruptController {
    pub fn new() -> Self {
        Self {
            interrupt_handlers: HashMap::new(),
            pending_interrupts: Vec::new(),
        }
    }

    /// 注册中断处理器
    pub fn register_handler(
        &mut self,
        irq: u32,
        handler: Box<dyn Fn() -> Result<(), VmError> + Send + Sync>,
    ) {
        self.interrupt_handlers.insert(irq, handler);
    }

    /// 触发中断
    pub fn trigger_interrupt(&mut self, irq: u32) {
        self.pending_interrupts.push(irq);
    }

    /// 处理待处理的中断
    pub fn process_interrupts(&mut self) -> Result<(), VmError> {
        while let Some(irq) = self.pending_interrupts.pop() {
            if let Some(handler) = self.interrupt_handlers.get(&irq) {
                handler()?;
            }
        }
        Ok(())
    }
}

impl Default for InterruptController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linux_syscall_handler() {
        use vm_mem::SoftMmu;
        let mut handler = LinuxSyscallHandler::new();
        let mut mmu = SoftMmu::new(1024, false);
        // 测试exit系统调用
        let result = handler.handle_syscall(60, &[0], &mut mmu);
        assert!(result.is_ok());
    }

    #[test]
    fn test_interrupt_controller() {
        let mut controller = InterruptController::new();
        controller.trigger_interrupt(1);
        assert_eq!(controller.pending_interrupts.len(), 1);
    }
}
