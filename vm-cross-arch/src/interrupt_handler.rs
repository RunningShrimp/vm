//! 中断和异常处理
//!
//! 提供跨架构的中断和异常处理支持：
//! - 异常向量表管理
//! - 中断注入
//! - 异常分发和返回

use std::collections::HashMap;
use vm_core::{GuestArch, GuestAddr, VmError};
use tracing::{debug, trace, warn};

/// 异常类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExceptionType {
    /// 指令访问异常
    InstructionAccessFault,
    /// 数据访问异常
    LoadAccessFault,
    /// 存储访问异常
    StoreAccessFault,
    /// 指令地址未对齐
    InstructionAddressMisaligned,
    /// 数据地址未对齐
    LoadAddressMisaligned,
    /// 存储地址未对齐
    StoreAddressMisaligned,
    /// 非法指令
    IllegalInstruction,
    /// 断点
    Breakpoint,
    /// 环境调用（系统调用）
    EnvironmentCall,
    /// 指令页错误
    InstructionPageFault,
    /// 加载页错误
    LoadPageFault,
    /// 存储页错误
    StorePageFault,
    /// 其他异常
    Other(u32),
}

/// 中断类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InterruptType {
    /// 软件中断
    Software(u32),
    /// 定时器中断
    Timer,
    /// 外部中断
    External(u32),
    /// 其他中断
    Other(u32),
}

/// 异常/中断上下文
#[derive(Debug, Clone)]
pub struct ExceptionContext {
    /// 异常类型
    pub exception_type: ExceptionType,
    /// 异常地址
    pub fault_addr: GuestAddr,
    /// 异常值（架构特定）
    pub fault_value: u64,
    /// 指令地址
    pub instruction_addr: GuestAddr,
    /// 异常原因码（架构特定）
    pub cause_code: u32,
}

/// 异常向量表
pub struct ExceptionVectorTable {
    /// 异常处理函数映射
    handlers: HashMap<ExceptionType, GuestAddr>,
    /// 默认异常处理函数
    default_handler: Option<GuestAddr>,
}

impl ExceptionVectorTable {
    /// 创建新的异常向量表
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
            default_handler: None,
        }
    }

    /// 注册异常处理函数
    pub fn register_handler(&mut self, exception_type: ExceptionType, handler_addr: GuestAddr) {
        self.handlers.insert(exception_type, handler_addr);
        debug!("Registered handler for {:?} at 0x{:x}", exception_type, handler_addr.0);
    }

    /// 设置默认异常处理函数
    pub fn set_default_handler(&mut self, handler_addr: GuestAddr) {
        self.default_handler = Some(handler_addr);
    }

    /// 获取异常处理函数地址
    pub fn get_handler(&self, exception_type: &ExceptionType) -> Option<GuestAddr> {
        self.handlers.get(exception_type).copied().or(self.default_handler)
    }
}

impl Default for ExceptionVectorTable {
    fn default() -> Self {
        Self::new()
    }
}

/// 中断控制器
pub struct InterruptController {
    /// 待处理的中断队列
    pending_interrupts: Vec<InterruptType>,
    /// 中断使能状态
    interrupts_enabled: bool,
    /// 中断掩码
    interrupt_mask: u64,
}

impl InterruptController {
    /// 创建新的中断控制器
    pub fn new() -> Self {
        Self {
            pending_interrupts: Vec::new(),
            interrupts_enabled: true,
            interrupt_mask: 0,
        }
    }

    /// 注入中断
    pub fn inject_interrupt(&mut self, interrupt: InterruptType) {
        if self.interrupts_enabled {
            self.pending_interrupts.push(interrupt);
            trace!("Injected interrupt: {:?}", interrupt);
        } else {
            warn!("Interrupt disabled, ignoring: {:?}", interrupt);
        }
    }

    /// 获取下一个待处理的中断
    pub fn pop_interrupt(&mut self) -> Option<InterruptType> {
        self.pending_interrupts.pop()
    }

    /// 启用中断
    pub fn enable_interrupts(&mut self) {
        self.interrupts_enabled = true;
    }

    /// 禁用中断
    pub fn disable_interrupts(&mut self) {
        self.interrupts_enabled = false;
    }

    /// 检查是否有待处理的中断
    pub fn has_pending_interrupts(&self) -> bool {
        !self.pending_interrupts.is_empty()
    }
}

impl Default for InterruptController {
    fn default() -> Self {
        Self::new()
    }
}

/// 异常处理器
pub struct ExceptionHandler {
    /// 异常向量表
    vector_table: ExceptionVectorTable,
    /// 当前架构
    arch: GuestArch,
}

impl ExceptionHandler {
    /// 创建新的异常处理器
    pub fn new(arch: GuestArch) -> Self {
        let mut handler = Self {
            vector_table: ExceptionVectorTable::new(),
            arch,
        };
        
        // 初始化默认异常处理
        handler.init_default_handlers();
        handler
    }

    /// 初始化默认异常处理
    fn init_default_handlers(&mut self) {
        // TODO: 根据架构设置默认异常向量表
        // 当前实现：使用占位地址
        let default_handler = GuestAddr(0x1000);
        self.vector_table.set_default_handler(default_handler);
    }

    /// 处理异常
    pub fn handle_exception(
        &self,
        exception: ExceptionContext,
    ) -> Result<GuestAddr, VmError> {
        trace!(
            "Handling exception: {:?} at 0x{:x}",
            exception.exception_type,
            exception.fault_addr.0
        );

        // 查找异常处理函数
        let handler_addr = self
            .vector_table
            .get_handler(&exception.exception_type)
            .ok_or_else(|| {
                VmError::Core(vm_core::CoreError::InvalidState {
                    message: format!(
                        "No handler for exception {:?}",
                        exception.exception_type
                    ),
                    current: "Exception handling".to_string(),
                    expected: "Registered handler".to_string(),
                })
            })?;

        debug!(
            "Dispatching exception {:?} to handler at 0x{:x}",
            exception.exception_type, handler_addr.0
        );

        Ok(handler_addr)
    }

    /// 注册异常处理函数
    pub fn register_handler(&mut self, exception_type: ExceptionType, handler_addr: GuestAddr) {
        self.vector_table.register_handler(exception_type, handler_addr);
    }
}

/// 跨架构异常/中断处理管理器
pub struct CrossArchInterruptManager {
    /// 异常处理器
    exception_handler: ExceptionHandler,
    /// 中断控制器
    interrupt_controller: InterruptController,
    /// Guest 架构
    guest_arch: GuestArch,
}

impl CrossArchInterruptManager {
    /// 创建新的跨架构中断管理器
    pub fn new(guest_arch: GuestArch) -> Self {
        Self {
            exception_handler: ExceptionHandler::new(guest_arch),
            interrupt_controller: InterruptController::new(),
            guest_arch,
        }
    }

    /// 处理异常
    pub fn handle_exception(
        &self,
        exception: ExceptionContext,
    ) -> Result<GuestAddr, VmError> {
        self.exception_handler.handle_exception(exception)
    }

    /// 注入中断
    pub fn inject_interrupt(&mut self, interrupt: InterruptType) {
        self.interrupt_controller.inject_interrupt(interrupt);
    }

    /// 检查并处理待处理的中断
    pub fn process_pending_interrupts(&mut self) -> Vec<InterruptType> {
        let mut interrupts = Vec::new();
        while let Some(interrupt) = self.interrupt_controller.pop_interrupt() {
            interrupts.push(interrupt);
        }
        interrupts
    }

    /// 启用中断
    pub fn enable_interrupts(&mut self) {
        self.interrupt_controller.enable_interrupts();
    }

    /// 禁用中断
    pub fn disable_interrupts(&mut self) {
        self.interrupt_controller.disable_interrupts();
    }

    /// 注册异常处理函数
    pub fn register_exception_handler(
        &mut self,
        exception_type: ExceptionType,
        handler_addr: GuestAddr,
    ) {
        self.exception_handler.register_handler(exception_type, handler_addr);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exception_vector_table() {
        let mut table = ExceptionVectorTable::new();
        let handler_addr = GuestAddr(0x2000);
        
        table.register_handler(ExceptionType::IllegalInstruction, handler_addr);
        
        let handler = table.get_handler(&ExceptionType::IllegalInstruction);
        assert_eq!(handler, Some(handler_addr));
    }

    #[test]
    fn test_interrupt_controller() {
        let mut controller = InterruptController::new();
        
        controller.inject_interrupt(InterruptType::Timer);
        assert!(controller.has_pending_interrupts());
        
        let interrupt = controller.pop_interrupt();
        assert_eq!(interrupt, Some(InterruptType::Timer));
    }

    #[test]
    fn test_exception_handler() {
        let handler = ExceptionHandler::new(GuestArch::X86_64);
        
        let exception = ExceptionContext {
            exception_type: ExceptionType::IllegalInstruction,
            fault_addr: GuestAddr(0x1000),
            fault_value: 0,
            instruction_addr: GuestAddr(0x1000),
            cause_code: 2,
        };
        
        // 应该使用默认处理器
        let handler_addr = handler.handle_exception(exception).unwrap();
        assert_eq!(handler_addr.0, 0x1000);
    }
}

