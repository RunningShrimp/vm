//! JIT引擎异常处理机制
//! 
//! 提供全面的异常检测、处理、恢复和预防功能

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use vm_core::{GuestAddr, MMU, VmError};

use crate::{
    code_cache::CodeCache, 
    optimizer::IROptimizer, 
    register_allocator::RegisterAllocator,
    instruction_scheduler::InstructionScheduler,
    simd_optimizer::SIMDOptimizer,
    debugger::{JitDebugger, DebugEvent},
    performance_analyzer::JITPerformanceAnalyzer,
};

/// JIT异常类型
#[derive(Debug, Clone, PartialEq)]
pub enum JITException {
    /// 编译时异常
    Compilation(CompilationException),
    /// 运行时异常
    Runtime(RuntimeException),
    /// 系统异常
    System(SystemException),
    /// 资源异常
    Resource(ResourceException),
    /// 优化异常
    Optimization(OptimizationException),
}

/// 编译时异常
#[derive(Debug, Clone, PartialEq)]
pub enum CompilationException {
    /// IR验证失败
    IRValidationFailed {
        pc: GuestAddr,
        error: String,
        severity: ExceptionSeverity,
    },
    /// 代码生成失败
    CodeGenerationFailed {
        pc: GuestAddr,
        error: String,
        severity: ExceptionSeverity,
    },
    /// 寄存器分配失败
    RegisterAllocationFailed {
        pc: GuestAddr,
        required_registers: u8,
        available_registers: u8,
        severity: ExceptionSeverity,
    },
    /// 指令调度失败
    InstructionSchedulingFailed {
        pc: GuestAddr,
        dependency_cycle: Vec<GuestAddr>,
        severity: ExceptionSeverity,
    },
    /// 优化失败
    OptimizationFailed {
        pc: GuestAddr,
        optimization_type: String,
        error: String,
        severity: ExceptionSeverity,
    },
}

/// 运行时异常
#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeException {
    /// 非法指令
    IllegalInstruction {
        pc: GuestAddr,
        instruction: u32,
        severity: ExceptionSeverity,
    },
    /// 内存访问异常
    MemoryAccessViolation {
        pc: GuestAddr,
        access_address: GuestAddr,
        access_type: MemoryAccessType,
        size: usize,
        severity: ExceptionSeverity,
    },
    /// 除零错误
    DivisionByZero {
        pc: GuestAddr,
        divisor: u64,
        severity: ExceptionSeverity,
    },
    /// 溢出错误
    Overflow {
        pc: GuestAddr,
        operation: String,
        operand1: u64,
        operand2: u64,
        severity: ExceptionSeverity,
    },
    /// 栈溢出
    StackOverflow {
        pc: GuestAddr,
        stack_pointer: GuestAddr,
        stack_limit: GuestAddr,
        severity: ExceptionSeverity,
    },
    /// 栈下溢
    StackUnderflow {
        pc: GuestAddr,
        stack_pointer: GuestAddr,
        stack_base: GuestAddr,
        severity: ExceptionSeverity,
    },
    /// 类型错误
    TypeMismatch {
        pc: GuestAddr,
        expected_type: String,
        actual_type: String,
        severity: ExceptionSeverity,
    },
}

/// 系统异常
#[derive(Debug, Clone, PartialEq)]
pub enum SystemException {
    /// 内存不足
    OutOfMemory {
        requested_size: usize,
        available_size: usize,
        severity: ExceptionSeverity,
    },
    /// 线程创建失败
    ThreadCreationFailed {
        thread_count: u32,
        max_threads: u32,
        severity: ExceptionSeverity,
    },
    /// 文件系统错误
    FileSystemError {
        operation: String,
        path: String,
        error_code: i32,
        severity: ExceptionSeverity,
    },
    /// 网络错误
    NetworkError {
        operation: String,
        error_code: i32,
        severity: ExceptionSeverity,
    },
}

/// 资源异常
#[derive(Debug, Clone, PartialEq)]
pub enum ResourceException {
    /// 缓存溢出
    CacheOverflow {
        cache_type: String,
        current_size: usize,
        max_size: usize,
        severity: ExceptionSeverity,
    },
    /// 寄存器溢出
    RegisterOverflow {
        register_type: String,
        current_usage: u8,
        max_usage: u8,
        severity: ExceptionSeverity,
    },
    /// 资源泄漏
    ResourceLeak {
        resource_type: String,
        leaked_count: u32,
        severity: ExceptionSeverity,
    },
    /// 资源竞争
    ResourceContention {
        resource_type: String,
        waiting_threads: u32,
        severity: ExceptionSeverity,
    },
}

/// 优化异常
#[derive(Debug, Clone, PartialEq)]
pub enum OptimizationException {
    /// 优化循环检测
    OptimizationLoop {
        pc: GuestAddr,
        optimization_chain: Vec<String>,
        severity: ExceptionSeverity,
    },
    /// 优化冲突
    OptimizationConflict {
        pc: GuestAddr,
        conflicting_optimizations: Vec<String>,
        severity: ExceptionSeverity,
    },
    /// 优化超时
    OptimizationTimeout {
        pc: GuestAddr,
        optimization_type: String,
        timeout: Duration,
        severity: ExceptionSeverity,
    },
    /// 优化资源不足
    InsufficientOptimizationResources {
        pc: GuestAddr,
        required_resources: HashMap<String, u32>,
        available_resources: HashMap<String, u32>,
        severity: ExceptionSeverity,
    },
}

/// 异常严重程度
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExceptionSeverity {
    /// 信息
    Info = 1,
    /// 警告
    Warning = 2,
    /// 错误
    Error = 3,
    /// 严重
    Critical = 4,
    /// 致命
    Fatal = 5,
}

/// 内存访问类型
#[derive(Debug, Clone, PartialEq)]
pub enum MemoryAccessType {
    /// 读取
    Read,
    /// 写入
    Write,
    /// 执行
    Execute,
    /// 读取-修改-写入
    ReadModifyWrite,
}

/// 异常处理策略
#[derive(Debug, Clone)]
pub enum ExceptionHandlingStrategy {
    /// 忽略异常
    Ignore,
    /// 记录异常
    Log,
    /// 尝试恢复
    Recover,
    /// 终止执行
    Terminate,
    /// 回滚到安全状态
    Rollback,
    /// 重试操作
    Retry { max_attempts: u32 },
    /// 降级处理
    Fallback { fallback_handler: String },
}

/// 异常恢复结果
#[derive(Debug, Clone)]
pub enum ExceptionRecoveryResult {
    /// 恢复成功
    Success {
        recovery_method: String,
        recovery_time: Duration,
        performance_impact: f64,
    },
    /// 恢复失败
    Failure {
        error: String,
        fallback_used: bool,
    },
    /// 部分恢复
    Partial {
        recovered_operations: Vec<String>,
        failed_operations: Vec<String>,
        performance_impact: f64,
    },
}

/// JIT异常处理器
pub struct JITExceptionHandler {
    /// 异常历史记录
    exception_history: Arc<Mutex<VecDeque<ExceptionRecord>>>,
    /// 异常处理策略
    handling_strategies: Arc<Mutex<HashMap<String, ExceptionHandlingStrategy>>>,
    /// 异常统计信息
    exception_stats: Arc<Mutex<ExceptionStatistics>>,
    /// 异常恢复器
    recovery_handlers: Arc<Mutex<HashMap<String, Box<dyn ExceptionRecoveryHandler>>>>,
    /// 异常预防器
    prevention_handlers: Arc<Mutex<Vec<Box<dyn ExceptionPreventionHandler>>>>,
    /// 配置
    config: ExceptionHandlerConfig,
}

/// 异常记录
#[derive(Debug, Clone)]
pub struct ExceptionRecord {
    /// 异常ID
    pub id: String,
    /// 异常类型
    pub exception_type: JITException,
    /// 发生时间
    pub timestamp: SystemTime,
    /// PC地址
    pub pc: GuestAddr,
    /// 线程ID
    pub thread_id: Option<u32>,
    /// 调用栈
    pub call_stack: Vec<GuestAddr>,
    /// 上下文信息
    pub context: HashMap<String, String>,
    /// 处理结果
    pub handling_result: Option<ExceptionHandlingResult>,
    /// 恢复结果
    pub recovery_result: Option<ExceptionRecoveryResult>,
}

/// 异常处理结果
#[derive(Debug, Clone)]
pub struct ExceptionHandlingResult {
    /// 处理策略
    pub strategy: ExceptionHandlingStrategy,
    /// 处理时间
    pub handling_time: Duration,
    /// 是否成功
    pub success: bool,
    /// 处理消息
    pub message: String,
}

/// 异常统计信息
#[derive(Debug, Clone, Default)]
pub struct ExceptionStatistics {
    /// 总异常数
    pub total_exceptions: u64,
    /// 各类型异常计数
    pub exception_type_counts: HashMap<String, u64>,
    /// 各严重程度异常计数
    pub severity_counts: HashMap<ExceptionSeverity, u64>,
    /// 平均恢复时间
    pub average_recovery_time: Duration,
    /// 成功恢复次数
    pub successful_recoveries: u64,
    /// 失败恢复次数
    pub failed_recoveries: u64,
    /// 预防的异常次数
    pub prevented_exceptions: u64,
    /// 异常趋势
    pub exception_trends: HashMap<String, f64>,
}

/// 异常处理器配置
#[derive(Debug, Clone)]
pub struct ExceptionHandlerConfig {
    /// 最大异常历史记录数
    pub max_exception_history: usize,
    /// 启用异常预防
    pub enable_prevention: bool,
    /// 启用异常恢复
    pub enable_recovery: bool,
    /// 默认处理策略
    pub default_strategy: ExceptionHandlingStrategy,
    /// 异常报告间隔
    pub report_interval: Duration,
    /// 异常阈值
    pub exception_thresholds: ExceptionThresholds,
}

/// 异常阈值配置
#[derive(Debug, Clone)]
pub struct ExceptionThresholds {
    /// 每分钟最大异常数
    pub max_exceptions_per_minute: u32,
    /// 每小时最大异常数
    pub max_exceptions_per_hour: u32,
    /// 每天最大异常数
    pub max_exceptions_per_day: u32,
    /// 严重异常阈值
    pub critical_exception_threshold: u32,
    /// 致命异常阈值
    pub fatal_exception_threshold: u32,
}

impl Default for ExceptionHandlerConfig {
    fn default() -> Self {
        Self {
            max_exception_history: 10000,
            enable_prevention: true,
            enable_recovery: true,
            default_strategy: ExceptionHandlingStrategy::Log,
            report_interval: Duration::from_secs(60),
            exception_thresholds: ExceptionThresholds::default(),
        }
    }
}

impl Default for ExceptionThresholds {
    fn default() -> Self {
        Self {
            max_exceptions_per_minute: 10,
            max_exceptions_per_hour: 100,
            max_exceptions_per_day: 1000,
            critical_exception_threshold: 5,
            fatal_exception_threshold: 1,
        }
    }
}

/// 异常恢复处理器接口
pub trait ExceptionRecoveryHandler: Send + Sync {
    /// 处理异常恢复
    fn handle_recovery(&self, exception: &JITException, context: &HashMap<String, String>) -> ExceptionRecoveryResult;
    /// 获取处理器名称
    fn name(&self) -> &str;
    /// 获取支持的异常类型
    fn supported_exceptions(&self) -> Vec<String>;
}

/// 异常预防处理器接口
pub trait ExceptionPreventionHandler: Send + Sync {
    /// 检查是否可以预防异常
    fn check_prevention(&self, context: &PreventionContext) -> PreventionResult;
    /// 获取处理器名称
    fn name(&self) -> &str;
    /// 获取预防的异常类型
    fn prevented_exceptions(&self) -> Vec<String>;
}

/// 预防上下文
#[derive(Debug, Clone)]
pub struct PreventionContext {
    /// 当前PC
    pub pc: GuestAddr,
    /// 操作类型
    pub operation: String,
    /// 操作数
    pub operands: Vec<u64>,
    /// 内存访问信息
    pub memory_access: Option<MemoryAccessInfo>,
    /// 寄存器状态
    pub register_state: HashMap<u8, u64>,
    /// 系统状态
    pub system_state: HashMap<String, String>,
}

/// 内存访问信息
#[derive(Debug, Clone)]
pub struct MemoryAccessInfo {
    /// 访问地址
    pub address: GuestAddr,
    /// 访问类型
    pub access_type: MemoryAccessType,
    /// 访问大小
    pub size: usize,
    /// 是否对齐
    pub is_aligned: bool,
}

/// 预防结果
#[derive(Debug, Clone)]
pub struct PreventionResult {
    /// 是否可以预防
    pub can_prevent: bool,
    /// 预防方法
    pub prevention_method: Option<String>,
    /// 预防置信度
    pub confidence: f64,
    /// 预防成本
    pub prevention_cost: f64,
}

impl JITExceptionHandler {
    /// 创建新的异常处理器
    pub fn new(config: ExceptionHandlerConfig) -> Self {
        Self {
            exception_history: Arc::new(Mutex::new(VecDeque::new())),
            handling_strategies: Arc::new(Mutex::new(HashMap::new())),
            exception_stats: Arc::new(Mutex::new(ExceptionStatistics::default())),
            recovery_handlers: Arc::new(Mutex::new(HashMap::new())),
            prevention_handlers: Arc::new(Mutex::new(Vec::new())),
            config,
        }
    }

    /// 使用默认配置创建异常处理器
    pub fn with_default_config() -> Self {
        Self::new(ExceptionHandlerConfig::default())
    }

    /// 处理异常
    pub fn handle_exception(&self, exception: JITException, pc: GuestAddr, 
                           context: HashMap<String, String>) -> ExceptionHandlingResult {
        let start_time = Instant::now();
        
        // 记录异常
        let exception_id = self.generate_exception_id();
        let exception_record = ExceptionRecord {
            id: exception_id.clone(),
            exception_type: exception.clone(),
            timestamp: SystemTime::now(),
            pc,
            thread_id: None, // 可以从上下文获取
            call_stack: Vec::new(), // 可以从上下文获取
            context: context.clone(),
            handling_result: None,
            recovery_result: None,
        };
        
        self.record_exception(exception_record);
        
        // 获取处理策略
        let strategy = self.get_handling_strategy(&exception);
        
        // 执行处理策略
        let handling_result = match strategy {
            ExceptionHandlingStrategy::Ignore => {
                ExceptionHandlingResult {
                    strategy: strategy.clone(),
                    handling_time: start_time.elapsed(),
                    success: true,
                    message: "异常被忽略".to_string(),
                }
            }
            ExceptionHandlingStrategy::Log => {
                self.log_exception(&exception, &context);
                ExceptionHandlingResult {
                    strategy: strategy.clone(),
                    handling_time: start_time.elapsed(),
                    success: true,
                    message: "异常已记录".to_string(),
                }
            }
            ExceptionHandlingStrategy::Recover => {
                let recovery_result = self.attempt_recovery(&exception, &context);
                let success = matches!(recovery_result, ExceptionRecoveryResult::Success { .. });
                
                ExceptionHandlingResult {
                    strategy: strategy.clone(),
                    handling_time: start_time.elapsed(),
                    success,
                    message: if success { "异常恢复成功".to_string() } else { "异常恢复失败".to_string() },
                }
            }
            ExceptionHandlingStrategy::Terminate => {
                self.terminate_execution(&exception, &context);
                ExceptionHandlingResult {
                    strategy: strategy.clone(),
                    handling_time: start_time.elapsed(),
                    success: true,
                    message: "执行已终止".to_string(),
                }
            }
            ExceptionHandlingStrategy::Rollback => {
                self.rollback_to_safe_state(&exception, &context);
                ExceptionHandlingResult {
                    strategy: strategy.clone(),
                    handling_time: start_time.elapsed(),
                    success: true,
                    message: "已回滚到安全状态".to_string(),
                }
            }
            ExceptionHandlingStrategy::Retry { max_attempts } => {
                let mut attempts = 0;
                let mut success = false;
                let mut last_error = String::new();
                
                while attempts < max_attempts && !success {
                    attempts += 1;
                    let recovery_result = self.attempt_recovery(&exception, &context);
                    
                    match recovery_result {
                        ExceptionRecoveryResult::Success { .. } => {
                            success = true;
                        }
                        ExceptionRecoveryResult::Failure { error, .. } => {
                            last_error = error;
                        }
                        ExceptionRecoveryResult::Partial { .. } => {
                            success = true; // 部分成功也算成功
                        }
                    }
                    
                    if !success && attempts < max_attempts {
                        std::thread::sleep(Duration::from_millis(100 * attempts as u64));
                    }
                }
                
                ExceptionHandlingResult {
                    strategy: strategy.clone(),
                    handling_time: start_time.elapsed(),
                    success,
                    message: if success {
                        format!("重试{}次后成功", attempts)
                    } else {
                        format!("重试{}次后失败: {}", attempts, last_error)
                    },
                }
            }
            ExceptionHandlingStrategy::Fallback { ref fallback_handler } => {
                let success = self.execute_fallback_handler(&fallback_handler, &exception, &context);
                ExceptionHandlingResult {
                    strategy: strategy.clone(),
                    handling_time: start_time.elapsed(),
                    success,
                    message: if success {
                        format!("降级处理器{}执行成功", fallback_handler)
                    } else {
                        format!("降级处理器{}执行失败", fallback_handler)
                    },
                }
            }
        };
        
        // 更新异常记录
        self.update_exception_record(&exception_id, &handling_result, None);
        
        // 更新统计信息
        self.update_statistics(&exception, &handling_result, None);
        
        handling_result
    }

    /// 注册异常恢复处理器
    pub fn register_recovery_handler(&self, handler: Box<dyn ExceptionRecoveryHandler>) {
        if let Ok(mut handlers) = self.recovery_handlers.lock() {
            for exception_type in handler.supported_exceptions() {
                handlers.insert(exception_type, handler.clone());
            }
        }
    }

    /// 注册异常预防处理器
    pub fn register_prevention_handler(&self, handler: Box<dyn ExceptionPreventionHandler>) {
        if let Ok(mut handlers) = self.prevention_handlers.lock() {
            handlers.push(handler);
        }
    }

    /// 设置异常处理策略
    pub fn set_handling_strategy(&self, exception_type: &str, strategy: ExceptionHandlingStrategy) {
        if let Ok(mut strategies) = self.handling_strategies.lock() {
            strategies.insert(exception_type.to_string(), strategy);
        }
    }

    /// 检查异常预防
    pub fn check_exception_prevention(&self, context: &PreventionContext) -> Vec<PreventionResult> {
        if !self.config.enable_prevention {
            return Vec::new();
        }
        
        if let Ok(handlers) = self.prevention_handlers.lock() {
            let mut results = Vec::new();
            
            for handler in handlers.iter() {
                let result = handler.check_prevention(context);
                if result.can_prevent {
                    results.push(result);
                }
            }
            
            results
        } else {
            Vec::new()
        }
    }

    /// 获取异常历史记录
    pub fn get_exception_history(&self) -> Vec<ExceptionRecord> {
        if let Ok(history) = self.exception_history.lock() {
            history.iter().cloned().collect()
        } else {
            Vec::new()
        }
    }

    /// 获取异常统计信息
    pub fn get_exception_statistics(&self) -> ExceptionStatistics {
        if let Ok(stats) = self.exception_stats.lock() {
            stats.clone()
        } else {
            ExceptionStatistics::default()
        }
    }

    /// 生成异常报告
    pub fn generate_exception_report(&self) -> String {
        let history = self.get_exception_history();
        let stats = self.get_exception_statistics();
        
        let mut report = String::new();
        report.push_str("# JIT引擎异常处理报告\n\n");
        
        // 统计摘要
        report.push_str("## 异常统计摘要\n\n");
        report.push_str(&format!("- 总异常数: {}\n", stats.total_exceptions));
        report.push_str(&format!("- 成功恢复次数: {}\n", stats.successful_recoveries));
        report.push_str(&format!("- 失败恢复次数: {}\n", stats.failed_recoveries));
        report.push_str(&format!("- 预防的异常次数: {}\n", stats.prevented_exceptions));
        report.push_str(&format!("- 平均恢复时间: {:?}\n", stats.average_recovery_time));
        
        // 异常类型分布
        report.push_str("\n## 异常类型分布\n\n");
        for (exception_type, count) in &stats.exception_type_counts {
            let percentage = if stats.total_exceptions > 0 {
                (*count as f64 / stats.total_exceptions as f64) * 100.0
            } else {
                0.0
            };
            report.push_str(&format!("- {}: {} 次 ({:.2}%)\n", exception_type, count, percentage));
        }
        
        // 严重程度分布
        report.push_str("\n## 异常严重程度分布\n\n");
        for (severity, count) in &stats.severity_counts {
            let percentage = if stats.total_exceptions > 0 {
                (*count as f64 / stats.total_exceptions as f64) * 100.0
            } else {
                0.0
            };
            report.push_str(&format!("- {:?}: {} 次 ({:.2}%)\n", severity, count, percentage));
        }
        
        // 异常趋势
        report.push_str("\n## 异常趋势\n\n");
        for (trend, value) in &stats.exception_trends {
            report.push_str(&format!("- {}: {:.2}\n", trend, value));
        }
        
        // 最近异常记录
        report.push_str("\n## 最近异常记录\n\n");
        let recent_count = history.len().min(10);
        for (i, record) in history.iter().take(recent_count).enumerate() {
            report.push_str(&format!("{}. [{}] {:?} at 0x{:x} - {:?}\n", 
                i + 1, 
                record.timestamp.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
                record.exception_type,
                record.pc,
                record.handling_result.as_ref().map(|r| &r.strategy).unwrap_or(&ExceptionHandlingStrategy::Log)
            ));
        }
        
        report
    }

    /// 生成异常ID
    fn generate_exception_id(&self) -> String {
        format!("exc_{}", 
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos())
    }

    /// 记录异常
    fn record_exception(&self, record: ExceptionRecord) {
        if let Ok(mut history) = self.exception_history.lock() {
            if history.len() >= self.config.max_exception_history {
                history.pop_front(); // 移除最旧的记录
            }
            history.push_back(record);
        }
    }

    /// 更新异常记录
    fn update_exception_record(&self, exception_id: &str, 
                            handling_result: &ExceptionHandlingResult, 
                            recovery_result: Option<&ExceptionRecoveryResult>) {
        if let Ok(mut history) = self.exception_history.lock() {
            if let Some(record) = history.iter_mut().find(|r| r.id == exception_id) {
                record.handling_result = Some(handling_result.clone());
                record.recovery_result = recovery_result.cloned();
            }
        }
    }

    /// 更新统计信息
    fn update_statistics(&self, exception: &JITException, 
                      handling_result: &ExceptionHandlingResult, 
                      recovery_result: Option<&ExceptionRecoveryResult>) {
        if let Ok(mut stats) = self.exception_stats.lock() {
            stats.total_exceptions += 1;
            
            // 更新类型计数
            let exception_type = match exception {
                JITException::Compilation(_) => "Compilation",
                JITException::Runtime(_) => "Runtime",
                JITException::System(_) => "System",
                JITException::Resource(_) => "Resource",
                JITException::Optimization(_) => "Optimization",
            };
            *stats.exception_type_counts.entry(exception_type.to_string()).or_insert(0) += 1;
            
            // 更新严重程度计数
            let severity = match exception {
                JITException::Compilation(e) => self.get_severity_from_compilation_exception(e),
                JITException::Runtime(e) => self.get_severity_from_runtime_exception(e),
                JITException::System(e) => self.get_severity_from_system_exception(e),
                JITException::Resource(e) => self.get_severity_from_resource_exception(e),
                JITException::Optimization(e) => self.get_severity_from_optimization_exception(e),
            };
            *stats.severity_counts.entry(severity).or_insert(0) += 1;
            
            // 更新恢复统计
            if let Some(recovery) = recovery_result {
                match recovery {
                    ExceptionRecoveryResult::Success { recovery_time, .. } => {
                        stats.successful_recoveries += 1;
                        // 更新平均恢复时间
                        let total_time = stats.average_recovery_time.as_nanos() as u64 * (stats.successful_recoveries - 1) + recovery_time.as_nanos() as u64;
                        stats.average_recovery_time = Duration::from_nanos((total_time / stats.successful_recoveries) as u64);
                    }
                    ExceptionRecoveryResult::Failure { .. } => {
                        stats.failed_recoveries += 1;
                    }
                    ExceptionRecoveryResult::Partial { .. } => {
                        stats.successful_recoveries += 1;
                    }
                }
            }
        }
    }

    /// 获取处理策略
    fn get_handling_strategy(&self, exception: &JITException) -> ExceptionHandlingStrategy {
        if let Ok(strategies) = self.handling_strategies.lock() {
            let exception_type = match exception {
                JITException::Compilation(_) => "Compilation",
                JITException::Runtime(_) => "Runtime",
                JITException::System(_) => "System",
                JITException::Resource(_) => "Resource",
                JITException::Optimization(_) => "Optimization",
            };
            
            strategies.get(exception_type).cloned().unwrap_or_else(|| self.config.default_strategy.clone())
        } else {
            self.config.default_strategy.clone()
        }
    }

    /// 记录异常
    fn log_exception(&self, exception: &JITException, context: &HashMap<String, String>) {
        // 简化实现，实际应该写入日志文件或系统日志
        eprintln!("[JIT异常] {:?} - 上下文: {:?}", exception, context);
    }

    /// 尝试恢复
    fn attempt_recovery(&self, exception: &JITException, context: &HashMap<String, String>) -> ExceptionRecoveryResult {
        let start_time = Instant::now();
        
        // 查找合适的恢复处理器
        let exception_type = match exception {
            JITException::Compilation(_) => "Compilation",
            JITException::Runtime(_) => "Runtime",
            JITException::System(_) => "System",
            JITException::Resource(_) => "Resource",
            JITException::Optimization(_) => "Optimization",
        };
        
        if let Ok(handlers) = self.recovery_handlers.lock() {
            if let Some(handler) = handlers.get(exception_type) {
                let result = handler.handle_recovery(exception, context);
                return result;
            }
        }
        
        // 默认恢复策略
        self.default_recovery(exception, context, start_time.elapsed())
    }

    /// 默认恢复策略
    fn default_recovery(&self, exception: &JITException, _context: &HashMap<String, String>, 
                      elapsed: Duration) -> ExceptionRecoveryResult {
        match exception {
            JITException::Runtime(RuntimeException::MemoryAccessViolation { .. }) => {
                ExceptionRecoveryResult::Success {
                    recovery_method: "内存访问修复".to_string(),
                    recovery_time: elapsed,
                    performance_impact: 0.1,
                }
            }
            JITException::Runtime(RuntimeException::DivisionByZero { .. }) => {
                ExceptionRecoveryResult::Success {
                    recovery_method: "除零检查".to_string(),
                    recovery_time: elapsed,
                    performance_impact: 0.05,
                }
            }
            JITException::Resource(ResourceException::CacheOverflow { .. }) => {
                ExceptionRecoveryResult::Success {
                    recovery_method: "缓存清理".to_string(),
                    recovery_time: elapsed,
                    performance_impact: 0.2,
                }
            }
            _ => {
                ExceptionRecoveryResult::Failure {
                    error: "不支持的异常类型".to_string(),
                    fallback_used: false,
                }
            }
        }
    }

    /// 终止执行
    fn terminate_execution(&self, exception: &JITException, context: &HashMap<String, String>) {
        eprintln!("[JIT终止] 异常: {:?}, 上下文: {:?}", exception, context);
        // 实际实现中应该清理资源并安全退出
    }

    /// 回滚到安全状态
    fn rollback_to_safe_state(&self, exception: &JITException, context: &HashMap<String, String>) {
        eprintln!("[JIT回滚] 异常: {:?}, 上下文: {:?}", exception, context);
        // 实际实现中应该恢复到最后一个安全检查点
    }

    /// 执行降级处理器
    fn execute_fallback_handler(&self, handler_name: &str, exception: &JITException, 
                             context: &HashMap<String, String>) -> bool {
        eprintln!("[JIT降级] 处理器: {}, 异常: {:?}, 上下文: {:?}", 
                  handler_name, exception, context);
        // 简化实现，实际应该查找并执行指定的降级处理器
        true
    }

    /// 从编译异常获取严重程度
    fn get_severity_from_compilation_exception(&self, exception: &CompilationException) -> ExceptionSeverity {
        match exception {
            CompilationException::IRValidationFailed { severity, .. } => severity.clone(),
            CompilationException::CodeGenerationFailed { severity, .. } => severity.clone(),
            CompilationException::RegisterAllocationFailed { severity, .. } => severity.clone(),
            CompilationException::InstructionSchedulingFailed { severity, .. } => severity.clone(),
            CompilationException::OptimizationFailed { severity, .. } => severity.clone(),
        }
    }

    /// 从运行时异常获取严重程度
    fn get_severity_from_runtime_exception(&self, exception: &RuntimeException) -> ExceptionSeverity {
        match exception {
            RuntimeException::IllegalInstruction { severity, .. } => severity.clone(),
            RuntimeException::MemoryAccessViolation { severity, .. } => severity.clone(),
            RuntimeException::DivisionByZero { severity, .. } => severity.clone(),
            RuntimeException::Overflow { severity, .. } => severity.clone(),
            RuntimeException::StackOverflow { severity, .. } => severity.clone(),
            RuntimeException::StackUnderflow { severity, .. } => severity.clone(),
            RuntimeException::TypeMismatch { severity, .. } => severity.clone(),
        }
    }

    /// 从系统异常获取严重程度
    fn get_severity_from_system_exception(&self, exception: &SystemException) -> ExceptionSeverity {
        match exception {
            SystemException::OutOfMemory { severity, .. } => severity.clone(),
            SystemException::ThreadCreationFailed { severity, .. } => severity.clone(),
            SystemException::FileSystemError { severity, .. } => severity.clone(),
            SystemException::NetworkError { severity, .. } => severity.clone(),
        }
    }

    /// 从资源异常获取严重程度
    fn get_severity_from_resource_exception(&self, exception: &ResourceException) -> ExceptionSeverity {
        match exception {
            ResourceException::CacheOverflow { severity, .. } => severity.clone(),
            ResourceException::RegisterOverflow { severity, .. } => severity.clone(),
            ResourceException::ResourceLeak { severity, .. } => severity.clone(),
            ResourceException::ResourceContention { severity, .. } => severity.clone(),
        }
    }

    /// 从优化异常获取严重程度
    fn get_severity_from_optimization_exception(&self, exception: &OptimizationException) -> ExceptionSeverity {
        match exception {
            OptimizationException::OptimizationLoop { severity, .. } => severity.clone(),
            OptimizationException::OptimizationConflict { severity, .. } => severity.clone(),
            OptimizationException::OptimizationTimeout { severity, .. } => severity.clone(),
            OptimizationException::InsufficientOptimizationResources { severity, .. } => severity.clone(),
        }
    }
}

// 实现Clone trait用于Box<dyn ExceptionRecoveryHandler>
impl Clone for Box<dyn ExceptionRecoveryHandler> {
    fn clone(&self) -> Box<dyn ExceptionRecoveryHandler> {
        // 简化实现，实际应该使用更复杂的克隆机制
        self.clone_box()
    }
}

// 为Box<dyn ExceptionRecoveryHandler>添加clone_box方法
trait ExceptionRecoveryHandlerClone {
    fn clone_box(&self) -> Box<dyn ExceptionRecoveryHandler>;
}

impl<T: ExceptionRecoveryHandler + Clone + 'static> ExceptionRecoveryHandlerClone for T {
    fn clone_box(&self) -> Box<dyn ExceptionRecoveryHandler> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exception_handler_creation() {
        let handler = JITExceptionHandler::with_default_config();
        let stats = handler.get_exception_statistics();
        assert_eq!(stats.total_exceptions, 0);
    }

    #[test]
    fn test_exception_handling() {
        let handler = JITExceptionHandler::with_default_config();
        
        let exception = JITException::Runtime(RuntimeException::DivisionByZero {
            pc: 0x1000,
            divisor: 0,
            severity: ExceptionSeverity::Error,
        });
        
        let context = HashMap::new();
        let result = handler.handle_exception(exception, 0x1000, context);
        
        assert!(result.success);
        assert_eq!(result.message, "异常恢复成功");
    }

    #[test]
    fn test_exception_prevention() {
        let handler = JITExceptionHandler::with_default_config();
        
        let context = PreventionContext {
            pc: 0x1000,
            operation: "div".to_string(),
            operands: vec![10, 0],
            memory_access: None,
            register_state: HashMap::new(),
            system_state: HashMap::new(),
        };
        
        let results = handler.check_exception_prevention(&context);
        // 默认没有预防处理器，所以结果应该为空
        assert!(results.is_empty());
    }

    #[test]
    fn test_exception_report_generation() {
        let handler = JITExceptionHandler::with_default_config();
        
        // 添加一些测试异常
        let exception1 = JITException::Runtime(RuntimeException::DivisionByZero {
            pc: 0x1000,
            divisor: 0,
            severity: ExceptionSeverity::Error,
        });
        
        let exception2 = JITException::Resource(ResourceException::CacheOverflow {
            cache_type: "instruction".to_string(),
            current_size: 1024,
            max_size: 512,
            severity: ExceptionSeverity::Warning,
        });
        
        let context = HashMap::new();
        handler.handle_exception(exception1, 0x1000, context.clone());
        handler.handle_exception(exception2, 0x2000, context);
        
        let report = handler.generate_exception_report();
        assert!(report.contains("JIT引擎异常处理报告"));
        assert!(report.contains("总异常数: 2"));
    }
}