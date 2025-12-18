//! 执行限界上下文
//!
//! 本模块定义了JIT执行相关的领域模型，包括执行环境、执行策略和执行结果。

use std::collections::HashMap;
use std::sync::Arc;
use vm_core::{GuestAddr, MMU, ExecResult, ExecStatus, ExecStats};
use crate::common::{JITResult, JITErrorBuilder, ExecutionStats};
use vm_error::VmError;

/// 执行限界上下文
pub struct ExecutionContext {
    /// 执行ID
    pub execution_id: ExecutionId,
    /// 执行环境
    pub environment: ExecutionEnvironment,
    /// 执行策略
    pub strategy: ExecutionStrategy,
    /// 执行状态
    pub status: ExecutionStatus,
    /// 执行结果
    pub result: Option<ExecutionResult>,
    /// 执行统计
    pub stats: ExecutionStats,
}

/// 执行ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExecutionId(u64);

impl ExecutionId {
    /// 创建新的执行ID
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, Ordering::SeqCst))
    }
    
    /// 获取ID值
    pub fn value(&self) -> u64 {
        self.0
    }
}

/// 执行环境
#[derive(Debug, Clone)]
pub struct ExecutionEnvironment {
    /// MMU实例
    pub mmu: Arc<dyn MMU>,
    /// 寄存器状态
    pub registers: HashMap<String, u64>,
    /// 内存映射
    pub memory_map: MemoryMap,
    /// 执行模式
    pub mode: ExecutionMode,
    /// 安全级别
    pub security_level: SecurityLevel,
}

/// 内存映射
#[derive(Debug, Clone)]
pub struct MemoryMap {
    /// 代码段
    pub code_segments: Vec<MemorySegment>,
    /// 数据段
    pub data_segments: Vec<MemorySegment>,
    /// 栈段
    pub stack_segment: Option<MemorySegment>,
    /// 堆段
    pub heap_segment: Option<MemorySegment>,
}

/// 内存段
#[derive(Debug, Clone)]
pub struct MemorySegment {
    /// 段名称
    pub name: String,
    /// 起始地址
    pub start_addr: GuestAddr,
    /// 大小
    pub size: usize,
    /// 权限
    pub permissions: MemoryPermissions,
    /// 是否已映射
    pub is_mapped: bool,
}

/// 内存权限
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryPermissions {
    /// 可读
    pub readable: bool,
    /// 可写
    pub writable: bool,
    /// 可执行
    pub executable: bool,
}

impl MemoryPermissions {
    /// 创建新的内存权限
    pub fn new(readable: bool, writable: bool, executable: bool) -> Self {
        Self {
            readable,
            writable,
            executable,
        }
    }
    
    /// 只读权限
    pub fn read_only() -> Self {
        Self::new(true, false, false)
    }
    
    /// 读写权限
    pub fn read_write() -> Self {
        Self::new(true, true, false)
    }
    
    /// 可执行权限
    pub fn executable() -> Self {
        Self::new(true, false, true)
    }
    
    /// 读写可执行权限
    pub fn read_write_execute() -> Self {
        Self::new(true, true, true)
    }
}

/// 执行模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    /// 解释执行
    Interpreted,
    /// JIT编译执行
    JITCompiled,
    /// 硬件加速执行
    HardwareAccelerated,
    /// 混合执行
    Hybrid,
}

/// 安全级别
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityLevel {
    /// 无安全检查
    None,
    /// 基础安全检查
    Basic,
    /// 标准安全检查
    Standard,
    /// 严格安全检查
    Strict,
}

/// 执行策略
#[derive(Debug, Clone)]
pub struct ExecutionStrategy {
    /// 执行类型
    pub execution_type: ExecutionType,
    /// 优化级别
    pub optimization_level: OptimizationLevel,
    /// 超时设置
    pub timeout: Option<std::time::Duration>,
    /// 重试策略
    pub retry_policy: RetryPolicy,
    /// 资源限制
    pub resource_limits: ResourceLimits,
}

/// 执行类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionType {
    /// 同步执行
    Synchronous,
    /// 异步执行
    Asynchronous,
    /// 流水线执行
    Pipelined,
    /// 并行执行
    Parallel,
}

/// 优化级别
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizationLevel {
    /// 无优化
    None,
    /// 基础优化
    Basic,
    /// 平衡优化
    Balanced,
    /// 最大优化
    Max,
}

/// 重试策略
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// 最大重试次数
    pub max_attempts: u32,
    /// 重试间隔
    pub retry_interval: std::time::Duration,
    /// 退避策略
    pub backoff_strategy: BackoffStrategy,
}

/// 退避策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackoffStrategy {
    /// 固定间隔
    Fixed,
    /// 线性退避
    Linear,
    /// 指数退避
    Exponential,
}

/// 资源限制
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// 最大内存使用量（字节）
    pub max_memory_bytes: Option<u64>,
    /// 最大执行时间
    pub max_execution_time: Option<std::time::Duration>,
    /// 最大指令数
    pub max_instructions: Option<u64>,
    /// 最大CPU使用率
    pub max_cpu_usage: Option<f64>,
}

/// 执行状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionStatus {
    /// 等待执行
    Pending,
    /// 执行中
    Running,
    /// 执行成功
    Completed,
    /// 执行失败
    Failed,
    /// 已暂停
    Paused,
    /// 已取消
    Cancelled,
}

/// 执行结果
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// 执行ID
    pub execution_id: ExecutionId,
    /// 执行状态
    pub status: ExecStatus,
    /// 执行统计
    pub stats: ExecStats,
    /// 执行时间
    pub execution_time: std::time::Duration,
    /// 内存使用峰值
    pub peak_memory_usage: u64,
    /// 指令执行数
    pub instructions_executed: u64,
    /// 异常信息
    pub exceptions: Vec<ExecutionException>,
}

/// 执行异常
#[derive(Debug, Clone)]
pub struct ExecutionException {
    /// 异常类型
    pub exception_type: ExceptionType,
    /// 异常地址
    pub address: GuestAddr,
    /// 异常代码
    pub code: u32,
    /// 异常消息
    pub message: String,
    /// 异常时间
    pub timestamp: std::time::Instant,
}

/// 异常类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExceptionType {
    /// 内存访问异常
    MemoryAccess,
    /// 指令异常
    Instruction,
    /// 算术异常
    Arithmetic,
    /// 系统调用异常
    SystemCall,
    /// 中断异常
    Interrupt,
    /// 页面错误异常
    PageFault,
    /// 保护异常
    Protection,
}

impl ExecutionContext {
    /// 创建新的执行上下文
    pub fn new(environment: ExecutionEnvironment, strategy: ExecutionStrategy) -> Self {
        Self {
            execution_id: ExecutionId::new(),
            environment,
            strategy,
            status: ExecutionStatus::Pending,
            result: None,
            stats: ExecutionStats::new(),
        }
    }
    
    /// 开始执行
    pub fn start_execution(&mut self) {
        self.status = ExecutionStatus::Running;
        self.stats.record_execution_start();
    }
    
    /// 完成执行
    pub fn complete_execution(&mut self, result: ExecutionResult) {
        self.status = ExecutionStatus::Completed;
        self.result = Some(result.clone());
        self.stats.record_successful_execution(result.execution_time);
    }
    
    /// 执行失败
    pub fn fail_execution(&mut self, error: VmError, execution_time: std::time::Duration) {
        self.status = ExecutionStatus::Failed;
        self.stats.record_failed_execution(execution_time);
    }
    
    /// 暂停执行
    pub fn pause_execution(&mut self) {
        self.status = ExecutionStatus::Paused;
    }
    
    /// 恢复执行
    pub fn resume_execution(&mut self) {
        self.status = ExecutionStatus::Running;
    }
    
    /// 取消执行
    pub fn cancel_execution(&mut self) {
        self.status = ExecutionStatus::Cancelled;
    }
    
    /// 获取执行进度
    pub fn get_progress(&self) -> ExecutionProgress {
        match self.status {
            ExecutionStatus::Pending => ExecutionProgress::new(0.0),
            ExecutionStatus::Running => {
                // 基于执行时间估算进度
                if let Some(max_time) = self.strategy.resource_limits.max_execution_time {
                    if let Some(start_time) = self.stats.last_update_time {
                        let elapsed = start_time.elapsed();
                        let progress = (elapsed.as_secs_f64() / max_time.as_secs_f64()).min(1.0);
                        ExecutionProgress::new(progress)
                    } else {
                        ExecutionProgress::new(0.0)
                    }
                } else {
                    ExecutionProgress::new(0.5)
                }
            }
            ExecutionStatus::Completed => ExecutionProgress::new(1.0),
            ExecutionStatus::Failed | ExecutionStatus::Paused | ExecutionStatus::Cancelled => ExecutionProgress::finished(),
        }
    }
}

/// 执行进度
#[derive(Debug, Clone)]
pub struct ExecutionProgress {
    /// 进度百分比（0.0-1.0）
    pub percentage: f64,
    /// 是否已完成
    pub is_finished: bool,
}

impl ExecutionProgress {
    /// 创建新的进度
    pub fn new(percentage: f64) -> Self {
        Self {
            percentage: percentage.clamp(0.0, 1.0),
            is_finished: percentage >= 1.0,
        }
    }
    
    /// 创建已完成的进度
    pub fn finished() -> Self {
        Self {
            percentage: 1.0,
            is_finished: true,
        }
    }
}

/// 执行领域服务
pub struct ExecutionService {
    /// 执行器工厂
    executor_factory: Box<dyn ExecutorFactory>,
    /// 资源管理器
    resource_manager: Arc<dyn ResourceManager>,
}

impl ExecutionService {
    /// 创建新的执行服务
    pub fn new(
        executor_factory: Box<dyn ExecutorFactory>,
        resource_manager: Arc<dyn ResourceManager>,
    ) -> Self {
        Self {
            executor_factory,
            resource_manager,
        }
    }
    
    /// 执行代码块
    pub fn execute(&self, environment: ExecutionEnvironment, strategy: ExecutionStrategy) -> JITResult<ExecutionResult> {
        let mut context = ExecutionContext::new(environment, strategy);
        
        // 开始执行
        context.start_execution();
        
        // 创建执行器
        let executor = self.executor_factory.create_executor(&strategy.execution_type);
        
        // 检查资源限制
        self.resource_manager.check_limits(&strategy.resource_limits)?;
        
        // 执行代码
        let result = executor.execute(&mut context)?;
        
        // 完成执行
        context.complete_execution(result.clone());
        
        Ok(result)
    }
}

/// 执行器工厂特征
pub trait ExecutorFactory: Send + Sync {
    /// 创建执行器
    fn create_executor(&self, execution_type: &ExecutionType) -> Box<dyn Executor>;
}

/// 执行器特征
pub trait Executor: Send + Sync {
    /// 执行
    fn execute(&self, context: &mut ExecutionContext) -> JITResult<ExecutionResult>;
}

/// 资源管理器特征
pub trait ResourceManager: Send + Sync {
    /// 检查资源限制
    fn check_limits(&self, limits: &ResourceLimits) -> JITResult<()>;
    
    /// 分配资源
    fn allocate_resources(&self, request: &ResourceRequest) -> JITResult<ResourceAllocation>;
    
    /// 释放资源
    fn release_resources(&self, allocation: &ResourceAllocation);
}

/// 资源请求
#[derive(Debug, Clone)]
pub struct ResourceRequest {
    /// 内存请求量
    pub memory_bytes: u64,
    /// CPU请求量
    pub cpu_units: u32,
    /// 请求优先级
    pub priority: ResourcePriority,
}

/// 资源优先级
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourcePriority {
    /// 低优先级
    Low,
    /// 中等优先级
    Medium,
    /// 高优先级
    High,
    /// 关键优先级
    Critical,
}

/// 资源分配
#[derive(Debug, Clone)]
pub struct ResourceAllocation {
    /// 分配ID
    pub allocation_id: String,
    /// 分配的内存量
    pub memory_bytes: u64,
    /// 分配的CPU单元
    pub cpu_units: u32,
    /// 分配时间
    pub allocated_at: std::time::Instant,
    /// 过期时间
    pub expires_at: Option<std::time::Instant>,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_execution_context() {
        let environment = ExecutionEnvironment {
            mmu: Arc::new(crate::test_utils::MockMMU::new()),
            registers: HashMap::new(),
            memory_map: MemoryMap {
                code_segments: Vec::new(),
                data_segments: Vec::new(),
                stack_segment: None,
                heap_segment: None,
            },
            mode: ExecutionMode::JITCompiled,
            security_level: SecurityLevel::Standard,
        };
        
        let strategy = ExecutionStrategy {
            execution_type: ExecutionType::Synchronous,
            optimization_level: OptimizationLevel::Balanced,
            timeout: Some(std::time::Duration::from_secs(10)),
            retry_policy: RetryPolicy {
                max_attempts: 3,
                retry_interval: std::time::Duration::from_millis(100),
                backoff_strategy: BackoffStrategy::Exponential,
            },
            resource_limits: ResourceLimits {
                max_memory_bytes: Some(1024 * 1024),
                max_execution_time: Some(std::time::Duration::from_secs(30)),
                max_instructions: Some(1000000),
                max_cpu_usage: Some(0.8),
            },
        };
        
        let mut context = ExecutionContext::new(environment, strategy);
        
        assert_eq!(context.status, ExecutionStatus::Pending);
        
        context.start_execution();
        assert_eq!(context.status, ExecutionStatus::Running);
        
        context.complete_execution(ExecutionResult {
            execution_id: context.execution_id,
            status: ExecStatus::Ok,
            stats: ExecStats::default(),
            execution_time: std::time::Duration::from_millis(100),
            peak_memory_usage: 512 * 1024,
            instructions_executed: 1000,
            exceptions: Vec::new(),
        });
        
        assert_eq!(context.status, ExecutionStatus::Completed);
    }
    
    #[test]
    fn test_memory_permissions() {
        let perms = MemoryPermissions::read_write_execute();
        assert!(perms.readable);
        assert!(perms.writable);
        assert!(perms.executable);
        
        let ro_perms = MemoryPermissions::read_only();
        assert!(ro_perms.readable);
        assert!(!ro_perms.writable);
        assert!(!ro_perms.executable);
    }
    
    #[test]
    fn test_execution_progress() {
        let progress = ExecutionProgress::new(0.75);
        assert_eq!(progress.percentage, 0.75);
        assert!(!progress.is_finished);
        
        let finished = ExecutionProgress::finished();
        assert_eq!(finished.percentage, 1.0);
        assert!(finished.is_finished);
    }
    
    #[test]
    fn test_execution_id() {
        let id1 = ExecutionId::new();
        let id2 = ExecutionId::new();
        
        assert_ne!(id1.value(), id2.value());
        assert!(id1.value() > 0);
        assert!(id2.value() > id1.value());
    }
}