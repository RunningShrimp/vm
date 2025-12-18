//! JIT引擎错误处理
//!
//! 本模块使用统一的vm-error框架来处理JIT引擎错误，减少代码重复并提高一致性。

use std::collections::HashMap;
use std::time::Instant;
use vm_error::{VmError, VmResult, JitError, ErrorContext, ErrorContextExt, utils};

/// JIT引擎错误类型别名
pub type JITResult<T> = VmResult<T>;

/// JIT引擎错误处理器
pub struct JITErrorHandler {
    error_count: u64,
    last_error_time: Option<Instant>,
    error_history: Vec<VmError>,
    max_history_size: usize,
}

impl JITErrorHandler {
    /// 创建新的错误处理器
    pub fn new(max_history_size: usize) -> Self {
        Self {
            error_count: 0,
            last_error_time: None,
            error_history: Vec::new(),
            max_history_size,
        }
    }
    
    /// 处理错误
    pub fn handle_error(&mut self, error: &VmError) {
        self.error_count += 1;
        self.last_error_time = Some(Instant::now());
        
        // 添加到历史记录
        self.error_history.push(error.clone());
        
        // 限制历史记录大小
        if self.error_history.len() > self.max_history_size {
            self.error_history.remove(0);
        }
        
        // 记录错误
        let context = ErrorContext::new("error_handling", "jit_engine")
            .with_info("error_count", self.error_count.to_string());
        utils::log_error(error, &context);
    }
    
    /// 获取错误统计
    pub fn get_error_stats(&self) -> JITErrorStats {
        let mut error_counts = HashMap::new();
        let mut error_types = HashMap::new();
        
        for error in &self.error_history {
            let count = error_counts.entry(error.to_string()).or_insert(0);
            *count += 1;
            
            let error_type = match error {
                VmError::JitCompilation { .. } => "JIT Compilation",
                VmError::Memory { .. } => "Memory",
                VmError::Translation { .. } => "Translation",
                VmError::Core { .. } => "Core",
                VmError::Device { .. } => "Device",
                VmError::Configuration { .. } => "Configuration",
                VmError::Network { .. } => "Network",
                VmError::Io { .. } => "I/O",
                VmError::Generic { .. } => "Generic",
            };
            
            let type_count = error_types.entry(error_type.to_string()).or_insert(0);
            *type_count += 1;
        }
        
        JITErrorStats {
            total_errors: self.error_count,
            last_error_time: self.last_error_time,
            error_counts,
            error_types,
            most_common_error: self.find_most_common_error(),
        }
    }
    
    /// 查找最常见的错误
    fn find_most_common_error(&self) -> Option<String> {
        if self.error_history.is_empty() {
            return None;
        }
        
        let mut error_counts = HashMap::new();
        
        for error in &self.error_history {
            let count = error_counts.entry(error.to_string()).or_insert(0);
            *count += 1;
        }
        
        error_counts
            .iter()
            .max_by_key(|(_, count)| **count)
            .map(|(error, _)| error.clone())
    }
    
    /// 重置错误统计
    pub fn reset(&mut self) {
        self.error_count = 0;
        self.last_error_time = None;
        self.error_history.clear();
    }
}

/// JIT错误统计
#[derive(Debug, Clone)]
pub struct JITErrorStats {
    /// 总错误数
    pub total_errors: u64,
    /// 最后错误时间
    pub last_error_time: Option<Instant>,
    /// 错误计数
    pub error_counts: HashMap<String, u64>,
    /// 错误类型统计
    pub error_types: HashMap<String, u64>,
    /// 最常见错误
    pub most_common_error: Option<String>,
}

impl Default for JITErrorStats {
    fn default() -> Self {
        Self {
            total_errors: 0,
            last_error_time: None,
            error_counts: HashMap::new(),
            error_types: HashMap::new(),
            most_common_error: None,
        }
    }
}

/// JIT错误恢复策略
#[derive(Debug, Clone, Copy)]
pub enum JITErrorRecoveryStrategy {
    /// 立即失败
    FailFast,
    /// 重试指定次数
    Retry { max_attempts: u32, delay_ms: u64 },
    /// 降级处理
    Fallback,
    /// 忽略错误继续
    Ignore,
}

/// JIT错误恢复管理器
pub struct JITErrorRecoveryManager {
    strategy: JITErrorRecoveryStrategy,
    retry_count: u32,
}

impl JITErrorRecoveryManager {
    /// 创建新的错误恢复管理器
    pub fn new(strategy: JITErrorRecoveryStrategy) -> Self {
        Self {
            strategy,
            retry_count: 0,
        }
    }
    
    /// 处理错误并决定恢复策略
    pub fn handle_error(&mut self, error: &VmError) -> JITRecoveryAction {
        match self.strategy {
            JITErrorRecoveryStrategy::FailFast => JITRecoveryAction::Fail,
            JITErrorRecoveryStrategy::Retry { max_attempts, delay_ms } => {
                if self.retry_count < max_attempts && utils::is_recoverable(error) {
                    self.retry_count += 1;
                    JITRecoveryAction::Retry { delay_ms }
                } else {
                    JITRecoveryAction::Fail
                }
            }
            JITErrorRecoveryStrategy::Fallback => JITRecoveryAction::Fallback,
            JITErrorRecoveryStrategy::Ignore => JITRecoveryAction::Ignore,
        }
    }
    
    /// 重置重试计数
    pub fn reset_retry_count(&mut self) {
        self.retry_count = 0;
    }
    
    /// 获取当前重试次数
    pub fn retry_count(&self) -> u32 {
        self.retry_count
    }
}

/// JIT恢复动作
#[derive(Debug, Clone, Copy)]
pub enum JITRecoveryAction {
    /// 失败
    Fail,
    /// 重试
    Retry { delay_ms: u64 },
    /// 降级
    Fallback,
    /// 忽略
    Ignore,
}

/// JIT错误构建器，用于创建特定类型的JIT错误
pub struct JITErrorBuilder;

impl JITErrorBuilder {
    /// 创建配置错误
    pub fn config(message: impl Into<String>) -> VmError {
        VmError::Configuration {
            source: vm_error::ConfigError::InvalidValue("config".to_string(), message.into()),
            message: "Configuration error in JIT engine".to_string(),
        }
    }
    
    /// 创建编译错误
    pub fn compilation(message: impl Into<String>) -> VmError {
        VmError::JitCompilation {
            source: JitError::CodeGenerationFailed(message.into()),
            message: "Compilation error in JIT engine".to_string(),
        }
    }
    
    /// 创建优化错误
    pub fn optimization(message: impl Into<String>) -> VmError {
        VmError::JitCompilation {
            source: JitError::RuntimeError(message.into()),
            message: "Optimization error in JIT engine".to_string(),
        }
    }
    
    /// 创建代码生成错误
    pub fn code_generation(message: impl Into<String>) -> VmError {
        VmError::JitCompilation {
            source: JitError::CodeGenerationFailed(message.into()),
            message: "Code generation error in JIT engine".to_string(),
        }
    }
    
    /// 创建缓存错误
    pub fn cache(message: impl Into<String>) -> VmError {
        VmError::JitCompilation {
            source: JitError::RuntimeError(message.into()),
            message: "Cache error in JIT engine".to_string(),
        }
    }
    
    /// 创建内存错误
    pub fn memory(message: impl Into<String>) -> VmError {
        VmError::Memory {
            source: vm_error::MemoryError::AllocationFailed(message.into()),
            message: "Memory error in JIT engine".to_string(),
        }
    }
    
    /// 创建硬件加速错误
    pub fn hardware_acceleration(message: impl Into<String>) -> VmError {
        VmError::Device {
            source: vm_error::DeviceError::InitializationFailed(message.into()),
            message: "Hardware acceleration error in JIT engine".to_string(),
        }
    }
    
    /// 创建性能分析错误
    pub fn performance_analysis(message: impl Into<String>) -> VmError {
        VmError::Core {
            source: vm_error::CoreError::UnsupportedOperation(message.into()),
            message: "Performance analysis error in JIT engine".to_string(),
        }
    }
    
    /// 创建资源不足错误
    pub fn resource_exhausted(resource: impl Into<String>) -> VmError {
        VmError::Core {
            source: vm_error::CoreError::ResourceNotAvailable(resource.into()),
            message: "Resource exhausted in JIT engine".to_string(),
        }
    }
    
    /// 创建超时错误
    pub fn timeout(operation: impl Into<String>) -> VmError {
        VmError::Core {
            source: vm_error::CoreError::Timeout(operation.into()),
            message: "Timeout in JIT engine".to_string(),
        }
    }
    
    /// 创建序列化错误
    pub fn serialization(message: impl Into<String>) -> VmError {
        VmError::Generic {
            message: format!("Serialization error in JIT engine: {}", message.into()),
        }
    }
    
    /// 创建并发错误
    pub fn concurrency(message: impl Into<String>) -> VmError {
        VmError::Core {
            source: vm_error::CoreError::ResourceInUse(message.into()),
            message: "Concurrency error in JIT engine".to_string(),
        }
    }
    
    /// 创建未知错误
    pub fn unknown(message: impl Into<String>) -> VmError {
        VmError::Generic {
            message: format!("Unknown error in JIT engine: {}", message.into()),
        }
    }
}

/// 扩展VmError以添加JIT特定的上下文
pub trait JITErrorContext<T> {
    /// 添加JIT上下文信息
    fn with_jit_context(self, operation: &str, component: &str) -> VmResult<T>;
}

impl<T> JITErrorContext<T> for VmResult<T> {
    fn with_jit_context(self, operation: &str, component: &str) -> VmResult<T> {
        self.with_context(operation, component)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_jit_error_creation() {
        let error = JITErrorBuilder::config("Invalid configuration");
        assert!(matches!(error, VmError::Configuration { .. }));
        
        let error = JITErrorBuilder::compilation("Syntax error");
        assert!(matches!(error, VmError::JitCompilation { .. }));
    }
    
    #[test]
    fn test_jit_error_handler() {
        let mut handler = JITErrorHandler::new(10);
        
        let error1 = JITErrorBuilder::cache("Cache miss");
        let error2 = JITErrorBuilder::memory("Out of memory");
        
        handler.handle_error(&error1);
        handler.handle_error(&error2);
        
        let stats = handler.get_error_stats();
        assert_eq!(stats.total_errors, 2);
        assert!(stats.last_error_time.is_some());
        assert_eq!(stats.error_types.get("JIT Compilation"), Some(&1));
        assert_eq!(stats.error_types.get("Memory"), Some(&1));
    }
    
    #[test]
    fn test_jit_error_recovery() {
        let mut recovery = JITErrorRecoveryManager::new(
            JITErrorRecoveryStrategy::Retry { max_attempts: 3, delay_ms: 100 }
        );
        
        let recoverable_error = JITErrorBuilder::timeout("Operation timeout");
        let action = recovery.handle_error(&recoverable_error);
        
        assert!(matches!(action, JITRecoveryAction::Retry { delay_ms: 100 }));
        assert_eq!(recovery.retry_count(), 1);
        
        // 测试重试次数限制
        recovery.retry_count = 3;
        let action = recovery.handle_error(&recoverable_error);
        assert!(matches!(action, JITRecoveryAction::Fail));
    }
    
    #[test]
    fn test_jit_error_context() {
        let result: JITResult<()> = Err(JITErrorBuilder::compilation("Syntax error"));
        let enhanced_result = result.with_jit_context("IR compilation", "JIT compiler");
        
        assert!(enhanced_result.is_err());
    }
}