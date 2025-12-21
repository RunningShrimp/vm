//! 操作系统级别支持
//!
//! 提供系统调用模拟、设备I/O、中断处理等操作系统级别的功能。
//!
//! ## 当前能力状态（阶段1）
//!
//! **⚠️ 注意：当前实现仅提供最小化的系统调用支持，不足以运行完整的操作系统。**
//!
//! ### 已实现的系统调用
//! - `write` (syscall 1): 支持 stdout/stderr 输出
//! - `exit` (syscall 60): 进程退出
//!
//! ### 未实现的关键功能（阶段2目标）
//! - **引导链**: 镜像加载、页表初始化、特权态设置
//! - **异常处理**: 异常向量表、异常分发、异常返回
//! - **中断处理**: 中断控制器、中断注入、中断路由
//! - **设备支持**: 最小设备集（console/timer/block/net，优先 virtio）
//! - **系统调用**: 完整的系统调用集（文件I/O、进程管理、内存管理等）
//!
//! ## 阶段2最小OS能力清单
//!
//! 为了支持真正的OS级跨架构执行，需要实现以下最小能力集：
//!
//! 1. **引导链**
//!    - ELF/PE/内核镜像加载
//!    - 初始页表建立（x86_64/AArch64/RISC-V64）
//!    - 特权态寄存器初始化
//!    - 入口点跳转
//!
//! 2. **异常/中断模型**
//!    - 异常向量表配置
//!    - 计时器中断注入
//!    - 外设中断注入
//!    - 异常陷入与返回
//!
//! 3. **设备模型**
//!    - VirtIO console（基本I/O）
//!    - VirtIO timer（时间管理）
//!    - VirtIO block（存储）
//!    - VirtIO net（网络，可选）
//!    - 统一设备注册与 MMIO/PIO 映射
//!    - IRQ 分发机制
//!
//! 4. **系统调用扩展**
//!    - 文件I/O（open/read/write/close）
//!    - 进程管理（fork/exec/wait）
//!    - 内存管理（mmap/munmap/brk）
//!    - 信号处理（signal/kill）

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
    // 文件描述符表和下一个文件描述符已移除，因为目前未使用
}

impl Default for LinuxSyscallHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl LinuxSyscallHandler {
    pub fn new() -> Self {
        Self {}
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
                let buf_addr = GuestAddr(args[1]);
                let count = args[2] as usize;

                // 从MMU读取数据
                // 1. 将虚拟地址转换为物理地址
                let phys_addr =
                    mmu.translate(buf_addr, vm_core::AccessType::Read)
                        .map_err(|e| {
                            VmError::Core(vm_core::CoreError::Internal {
                                message: format!(
                                    "Failed to translate address {:#x}: {:?}",
                                    buf_addr, e
                                ),
                                module: "os_support".to_string(),
                            })
                        })?;

                // 2. 批量读取数据
                let mut buf = vec![0u8; count];
                mmu.read_bulk(phys_addr.into(), &mut buf).map_err(|e| {
                    VmError::Core(vm_core::CoreError::Internal {
                        message: format!(
                            "Failed to read {} bytes from address {:#x}: {:?}",
                            count, phys_addr.0, e
                        ),
                        module: "os_support".to_string(),
                    })
                })?;

                // 写入到标准输出
                if fd == 1 || fd == 2 {
                    let output = String::from_utf8_lossy(&buf);
                    tracing::info!(target: "guest_output", "{}", output);
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
