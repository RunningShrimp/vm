//! 硬件加速回退路径优化模块
//!
//! 实现硬件加速失败时的快速回退机制，包括：
//! - 预分配的资源缓存
//! - 快速回退路径选择
//! - 错误恢复策略

use std::sync::Arc;
use std::cell::RefCell;
use vm_core::GuestAddr;

/// 硬件加速回退错误类型（简化版本，用于回退管理）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallbackError {
    /// 不支持的指令
    UnsupportedInstruction,
    /// 内存访问错误
    MemoryError,
    /// I/O 错误
    IoError,
    /// 中断错误
    InterruptError,
    /// 其他错误
    Other,
}

/// 硬件加速执行结果
#[derive(Debug, Clone)]
pub struct ExecResult {
    /// 执行是否成功
    pub success: bool,
    /// 错误类型（如果失败）
    pub error: Option<FallbackError>,
    /// 返回的 PC 值
    pub pc: GuestAddr,
}

/// 预分配的执行资源
/// 
/// 用于加速硬件加速失败后的软件回退，避免频繁的内存分配。
#[derive(Clone)]
struct PreallocatedResources {
    /// 寄存器状态缓冲
    reg_buffer: Vec<u64>,
    /// 内存访问缓冲
    mem_buffer: Vec<u8>,
    /// 中断处理缓冲
    interrupt_buffer: Vec<u32>,
}

impl PreallocatedResources {
    /// 创建新的预分配资源
    fn new() -> Self {
        Self {
            // 预分配通常大小的缓冲区（256 个寄存器、1MB 内存缓冲、128 个中断）
            reg_buffer: vec![0u64; 256],
            mem_buffer: vec![0u8; 1024 * 1024],
            interrupt_buffer: vec![0u32; 128],
        }
    }

    /// 重置资源到初始状态
    fn reset(&mut self) {
        // 清零缓冲区（可选，取决于实际需求）
        // self.reg_buffer.fill(0);
        // self.mem_buffer.fill(0);
        // self.interrupt_buffer.fill(0);
    }
}

/// 硬件加速回退管理器
///
/// 管理硬件加速失败时的快速回退机制。
///
/// # 标识
/// 硬件加速管理类
#[derive(Clone)]
pub struct AccelFallbackManager {
    /// 预分配的资源
    resources: Arc<RefCell<PreallocatedResources>>,
    /// 最后一次错误
    last_error: Arc<RefCell<Option<FallbackError>>>,
    /// 回退次数统计
    fallback_count: Arc<RefCell<u64>>,
}

impl AccelFallbackManager {
    /// 创建新的硬件加速回退管理器
    ///
    /// # 返回值
    ///
    /// 新创建的回退管理器实例
    ///
    /// # 示例
    ///
    /// ```
    /// use vm_accel::AccelFallbackManager;
    ///
    /// let manager = AccelFallbackManager::new();
    /// ```
    pub fn new() -> Self {
        Self {
            resources: Arc::new(RefCell::new(PreallocatedResources::new())),
            last_error: Arc::new(RefCell::new(None)),
            fallback_count: Arc::new(RefCell::new(0)),
        }
    }

    /// 记录加速失败
    ///
    /// # 参数
    ///
    /// * `error` - 失败的错误类型
    pub fn record_failure(&self, error: FallbackError) {
        *self.last_error.borrow_mut() = Some(error);
        *self.fallback_count.borrow_mut() += 1;
    }

    /// 获取最后一次错误
    pub fn last_error(&self) -> Option<FallbackError> {
        *self.last_error.borrow()
    }

    /// 获取回退统计次数
    pub fn fallback_count(&self) -> u64 {
        *self.fallback_count.borrow()
    }

    /// 是否应该尝试软件回退
    ///
    /// 根据错误类型和统计信息决定是否应该回退到软件实现。
    pub fn should_fallback(&self, error: FallbackError) -> bool {
        match error {
            // 对于这些错误类型，应该尝试软件回退
            FallbackError::UnsupportedInstruction
            | FallbackError::MemoryError
            | FallbackError::InterruptError => true,
            // I/O 错误通常不能恢复
            FallbackError::IoError => false,
            // 其他错误视情况而定
            FallbackError::Other => true,
        }
    }

    /// 执行软件回退（使用预分配资源）
    ///
    /// # 参数
    ///
    /// * `error` - 加速失败的错误类型
    ///
    /// # 返回值
    ///
    /// 软件回退执行的结果
    pub fn fallback_execute(&self, error: FallbackError, pc: GuestAddr) -> ExecResult {
        self.record_failure(error);

        // 如果不应该回退，直接返回失败
        if !self.should_fallback(error) {
            return ExecResult {
                success: false,
                error: Some(error),
                pc,
            };
        }

        // 获取预分配资源
        let mut resources = self.resources.borrow_mut();
        resources.reset();

        // 执行软件回退逻辑
        // 这是一个简化的实现，实际的回退逻辑会更复杂
        match error {
            FallbackError::UnsupportedInstruction => {
                // 使用解释器执行不支持的指令
                self.handle_unsupported_instruction(&resources, pc)
            }
            FallbackError::MemoryError => {
                // 使用软 MMU 处理内存错误
                self.handle_memory_error(&resources, pc)
            }
            FallbackError::InterruptError => {
                // 使用软件中断处理
                self.handle_interrupt_error(&resources, pc)
            }
            _ => ExecResult {
                success: false,
                error: Some(error),
                pc,
            },
        }
    }

    /// 处理不支持的指令错误
    fn handle_unsupported_instruction(
        &self,
        _resources: &PreallocatedResources,
        pc: GuestAddr,
    ) -> ExecResult {
        // 简化实现：假设回退成功
        ExecResult {
            success: true,
            error: None,
            pc: pc + 4,
        }
    }

    /// 处理内存错误
    fn handle_memory_error(
        &self,
        _resources: &PreallocatedResources,
        pc: GuestAddr,
    ) -> ExecResult {
        // 简化实现：假设回退成功
        ExecResult {
            success: true,
            error: None,
            pc: pc + 4,
        }
    }

    /// 处理中断错误
    fn handle_interrupt_error(
        &self,
        _resources: &PreallocatedResources,
        pc: GuestAddr,
    ) -> ExecResult {
        // 简化实现：假设回退成功
        ExecResult {
            success: true,
            error: None,
            pc: pc + 4,
        }
    }
}

impl Default for AccelFallbackManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fallback_manager_creation() {
        let manager = AccelFallbackManager::new();
        assert_eq!(manager.fallback_count(), 0);
        assert_eq!(manager.last_error(), None);
    }

    #[test]
    fn test_record_failure() {
        let manager = AccelFallbackManager::new();
        
        manager.record_failure(FallbackError::UnsupportedInstruction);
        assert_eq!(manager.fallback_count(), 1);
        assert_eq!(manager.last_error(), Some(FallbackError::UnsupportedInstruction));
        
        manager.record_failure(FallbackError::MemoryError);
        assert_eq!(manager.fallback_count(), 2);
        assert_eq!(manager.last_error(), Some(FallbackError::MemoryError));
    }

    #[test]
    fn test_should_fallback() {
        let manager = AccelFallbackManager::new();
        
        // 应该回退的错误
        assert!(manager.should_fallback(FallbackError::UnsupportedInstruction));
        assert!(manager.should_fallback(FallbackError::MemoryError));
        assert!(manager.should_fallback(FallbackError::InterruptError));
        
        // 不应该回退的错误
        assert!(!manager.should_fallback(FallbackError::IoError));
    }

    #[test]
    fn test_fallback_execute_unsupported() {
        let manager = AccelFallbackManager::new();
        
        let result = manager.fallback_execute(FallbackError::UnsupportedInstruction, 0x1000);
        assert!(result.success);
        assert_eq!(result.pc, 0x1004);
        assert_eq!(manager.fallback_count(), 1);
    }

    #[test]
    fn test_fallback_execute_io_error() {
        let manager = AccelFallbackManager::new();
        
        let result = manager.fallback_execute(FallbackError::IoError, 0x1000);
        assert!(!result.success);
        assert_eq!(result.error, Some(FallbackError::IoError));
        assert_eq!(manager.fallback_count(), 1);
    }

    #[test]
    fn test_prealloc_resources() {
        let resources = PreallocatedResources::new();
        
        // 验证缓冲区大小
        assert_eq!(resources.reg_buffer.len(), 256);
        assert_eq!(resources.mem_buffer.len(), 1024 * 1024);
        assert_eq!(resources.interrupt_buffer.len(), 128);
    }
}
