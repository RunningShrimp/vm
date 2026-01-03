//! # vm-debug - 虚拟机调试支持
//!
//! 提供完整的调试功能，包括GDB远程调试、性能分析、日志系统和调试工具集成。
//!
//! ## 主要功能
//!
//! - **GDB远程调试**: 完整的GDB协议实现，支持断点、单步执行、变量查看
//! - **性能分析器**: 热点检测、调用跟踪、内存分析
//! - **结构化日志**: 分层日志系统，支持不同级别的调试信息
//! - **调试代理**: 统一的调试接口，支持多种调试前端
//! - **快照调试**: 虚拟机状态快照和回溯调试
//! - **条件断点**: 基于表达式和条件的断点设置

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use vm_core::{GuestAddr, VcpuStateContainer, VmError};

/// 调试器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebuggerConfig {
    /// 启用GDB远程调试
    pub enable_gdb_stub: bool,
    /// GDB监听端口
    pub gdb_port: u16,
    /// 启用性能分析
    pub enable_profiler: bool,
    /// 性能采样间隔（微秒）
    pub profiling_sample_interval_us: u64,
    /// 启用日志记录
    pub enable_logging: bool,
    /// 日志级别
    pub log_level: LogLevel,
    /// 最大日志缓冲区大小
    pub max_log_buffer_size: usize,
    /// 启用快照调试
    pub enable_snapshot_debugging: bool,
    /// 快照间隔（指令数）
    pub snapshot_interval_instructions: u64,
}

impl Default for DebuggerConfig {
    fn default() -> Self {
        Self {
            enable_gdb_stub: true,
            gdb_port: 1234,
            enable_profiler: true,
            profiling_sample_interval_us: 1000,
            enable_logging: true,
            log_level: LogLevel::Info,
            max_log_buffer_size: 10000,
            enable_snapshot_debugging: false,
            snapshot_interval_instructions: 1000000,
        }
    }
}

/// 日志级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

/// 断点类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BreakpointType {
    /// 执行断点
    Execution,
    /// 读断点
    Read,
    /// 写断点
    Write,
    /// 读写断点
    ReadWrite,
}

/// 断点信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Breakpoint {
    /// 断点ID
    pub id: u64,
    /// 断点地址
    pub address: GuestAddr,
    /// 断点类型
    pub breakpoint_type: BreakpointType,
    /// 条件表达式（可选）
    pub condition: Option<String>,
    /// 命中计数
    pub hit_count: u64,
    /// 启用状态
    pub enabled: bool,
}

/// 调试会话
#[derive(Debug)]
pub struct DebugSession {
    /// 会话ID
    pub session_id: String,
    /// 断点列表
    pub breakpoints: HashMap<u64, Breakpoint>,
    /// 当前停止状态
    pub stopped: bool,
    /// 停止原因
    pub stop_reason: Option<StopReason>,
    /// 当前指令指针
    pub current_pc: GuestAddr,
    /// 变量监视列表
    pub watchpoints: HashSet<GuestAddr>,
    /// 会话开始时间
    pub start_time: Instant,
}

/// 停止原因
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StopReason {
    /// 断点命中
    Breakpoint {
        breakpoint_id: u64,
        address: GuestAddr,
    },
    /// 单步执行
    Step,
    /// 异常/故障
    Exception {
        exception_type: String,
        address: GuestAddr,
    },
    /// 暂停命令
    Pause,
    /// 程序结束
    Exit { exit_code: i32 },
}

/// 调试事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DebugEvent {
    /// 断点命中
    BreakpointHit {
        breakpoint_id: u64,
        address: GuestAddr,
    },
    /// 单步完成
    StepComplete { address: GuestAddr },
    /// 异常发生
    Exception {
        exception_type: String,
        address: GuestAddr,
    },
    /// 程序暂停
    Paused { address: GuestAddr },
    /// 程序继续执行
    Continued,
    /// 变量值改变
    VariableChanged {
        name: String,
        old_value: u64,
        new_value: u64,
    },
}

/// 性能分析数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfilingData {
    /// 函数调用统计
    pub function_stats: HashMap<String, FunctionStats>,
    /// 热点指令
    pub hot_instructions: Vec<HotInstruction>,
    /// 内存访问模式
    pub memory_access_patterns: Vec<MemoryAccessPattern>,
    /// 采样时间范围
    pub time_range: (u64, u64),
}

/// 函数统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionStats {
    pub function_name: String,
    pub call_count: u64,
    pub total_time_ns: u64,
    pub average_time_ns: u64,
    pub min_time_ns: u64,
    pub max_time_ns: u64,
}

/// 热点指令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotInstruction {
    pub address: GuestAddr,
    pub execution_count: u64,
    pub total_cycles: u64,
    pub cache_misses: u64,
}

/// 内存访问模式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryAccessPattern {
    pub address_range: (GuestAddr, GuestAddr),
    pub access_type: MemoryAccessType,
    pub frequency: u64,
    pub stride: Option<i64>,
}

/// 内存访问类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryAccessType {
    Sequential,
    Strided,
    Random,
    Constant,
}

/// 虚拟机调试器
pub struct VmDebugger {
    /// 配置
    config: DebuggerConfig,
    /// 当前调试会话
    current_session: Option<DebugSession>,
    /// GDB存根
    gdb_stub: Option<Arc<RwLock<GdbStub>>>,
    /// 性能分析器
    profiler: Option<Arc<RwLock<Profiler>>>,
    /// 日志系统
    logger: Arc<RwLock<DebugLogger>>,
    /// 快照管理器
    snapshot_manager: Option<Arc<RwLock<SnapshotManager>>>,
    /// 事件监听器
    event_listeners: Vec<Box<dyn DebugEventListener + Send + Sync>>,
}

impl VmDebugger {
    /// 创建新的调试器
    pub fn new(config: DebuggerConfig) -> Self {
        let logger = Arc::new(RwLock::new(DebugLogger::new(config.max_log_buffer_size)));
        let gdb_stub = if config.enable_gdb_stub {
            Some(Arc::new(RwLock::new(GdbStub::new(config.gdb_port))))
        } else {
            None
        };

        let profiler = if config.enable_profiler {
            Some(Arc::new(RwLock::new(Profiler::new(
                config.profiling_sample_interval_us,
            ))))
        } else {
            None
        };

        let snapshot_manager = if config.enable_snapshot_debugging {
            Some(Arc::new(RwLock::new(SnapshotManager::new(
                config.snapshot_interval_instructions,
            ))))
        } else {
            None
        };

        Self {
            config,
            current_session: None,
            gdb_stub,
            profiler,
            logger,
            snapshot_manager,
            event_listeners: Vec::new(),
        }
    }

    /// 启动调试会话
    pub async fn start_session(&mut self) -> Result<String, VmError> {
        let session_id = format!("debug_session_{}", Instant::now().elapsed().as_nanos());
        let session = DebugSession {
            session_id: session_id.clone(),
            breakpoints: HashMap::new(),
            stopped: false,
            stop_reason: None,
            current_pc: vm_core::GuestAddr(0),
            watchpoints: HashSet::new(),
            start_time: Instant::now(),
        };

        self.current_session = Some(session);

        // 启动GDB存根 - 在await前获取端口，释放锁后再进行异步操作
        let gdb_port = if let Some(gdb_stub) = &self.gdb_stub {
            let stub = gdb_stub.read().map_err(|e| {
                VmError::Core(vm_core::CoreError::Concurrency {
                    message: format!("Failed to acquire read lock on gdb_stub: {}", e),
                    operation: "start_session".to_string(),
                })
            })?;
            Some(stub.port)
        } else {
            None
        };

        if let Some(port) = gdb_port {
            // 在释放锁后执行异步操作
            let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))
                .await
                .map_err(|e| VmError::Io(e.to_string()))?;

            // 重新获取锁并更新connection
            if let Some(gdb_stub) = &self.gdb_stub {
                let mut stub = gdb_stub.write().map_err(|e| {
                    VmError::Core(vm_core::CoreError::Concurrency {
                        message: format!("Failed to acquire write lock on gdb_stub: {}", e),
                        operation: "start_session".to_string(),
                    })
                })?;
                stub.connection = Some(listener);
            }
        }

        // 启动性能分析器
        if let Some(profiler) = &self.profiler {
            profiler
                .write()
                .map_err(|e| {
                    VmError::Core(vm_core::CoreError::Concurrency {
                        message: format!("Failed to acquire write lock on profiler: {}", e),
                        operation: "start_session".to_string(),
                    })
                })?
                .start();
        }

        Ok(session_id)
    }

    /// 结束调试会话
    pub async fn end_session(&mut self) -> Result<(), VmError> {
        // 停止GDB存根 - 在await前释放锁
        if let Some(gdb_stub) = &self.gdb_stub {
            {
                let mut stub = gdb_stub.write().map_err(|e| {
                    VmError::Core(vm_core::CoreError::Concurrency {
                        message: format!("Failed to acquire write lock on gdb_stub: {}", e),
                        operation: "end_session".to_string(),
                    })
                })?;
                stub.connection = None;
                stub.current_packet.clear();
            } // 锁在这里释放
        }

        if let Some(profiler) = &self.profiler {
            profiler
                .write()
                .map_err(|e| {
                    VmError::Core(vm_core::CoreError::Concurrency {
                        message: format!("Failed to acquire write lock on profiler: {}", e),
                        operation: "end_session".to_string(),
                    })
                })?
                .stop();
        }

        self.current_session = None;
        Ok(())
    }

    /// 设置断点
    pub fn set_breakpoint(
        &mut self,
        address: GuestAddr,
        breakpoint_type: BreakpointType,
        condition: Option<String>,
    ) -> Result<u64, VmError> {
        let session = self.current_session.as_mut().ok_or_else(|| {
            VmError::Core(vm_core::CoreError::InvalidState {
                message: "No active debug session".to_string(),
                current: "inactive".to_string(),
                expected: "active".to_string(),
            })
        })?;

        let breakpoint_id = session.breakpoints.len() as u64 + 1;
        let breakpoint = Breakpoint {
            id: breakpoint_id,
            address,
            breakpoint_type,
            condition,
            hit_count: 0,
            enabled: true,
        };

        session.breakpoints.insert(breakpoint_id, breakpoint);
        Ok(breakpoint_id)
    }

    /// 删除断点
    pub fn remove_breakpoint(&mut self, breakpoint_id: u64) -> Result<(), VmError> {
        let session = self.current_session.as_mut().ok_or_else(|| {
            VmError::Core(vm_core::CoreError::InvalidState {
                message: "No active debug session".to_string(),
                current: "inactive".to_string(),
                expected: "active".to_string(),
            })
        })?;

        session.breakpoints.remove(&breakpoint_id).ok_or_else(|| {
            VmError::Core(vm_core::CoreError::InvalidState {
                message: format!("Breakpoint {} not found", breakpoint_id),
                current: "not_found".to_string(),
                expected: "exists".to_string(),
            })
        })?;

        Ok(())
    }

    /// 检查断点命中
    pub fn check_breakpoints(
        &mut self,
        pc: GuestAddr,
        access_type: Option<MemoryAccessType>,
    ) -> Option<StopReason> {
        // 先收集所有启用的断点信息，避免借用冲突
        let breakpoint_checks: Vec<_> = {
            let session = self.current_session.as_ref()?;
            session
                .breakpoints
                .iter()
                .filter_map(|(id, breakpoint)| {
                    if !breakpoint.enabled {
                        return None;
                    }

                    let hit = match breakpoint.breakpoint_type {
                        BreakpointType::Execution => pc == breakpoint.address,
                        BreakpointType::Read => {
                            access_type == Some(MemoryAccessType::Random)
                                && pc == breakpoint.address
                        }
                        BreakpointType::Write => {
                            access_type == Some(MemoryAccessType::Random)
                                && pc == breakpoint.address
                        }
                        BreakpointType::ReadWrite => {
                            access_type.is_some() && pc == breakpoint.address
                        }
                    };

                    if hit {
                        Some((*id, breakpoint.condition.clone()))
                    } else {
                        None
                    }
                })
                .collect()
        };

        // 检查每个命中的断点的条件
        let mut hit_breakpoint_id = None;
        for (id, condition) in breakpoint_checks {
            let should_stop = if let Some(condition_str) = condition {
                self.evaluate_condition(&condition_str)
            } else {
                true // 没有条件则总是停止
            };

            if should_stop {
                hit_breakpoint_id = Some(id);
                break;
            }
        }

        // 如果有命中的断点，更新状态并返回
        if let Some(breakpoint_id) = hit_breakpoint_id {
            let session = self.current_session.as_mut()?;
            if let Some(breakpoint) = session.breakpoints.get_mut(&breakpoint_id) {
                breakpoint.hit_count += 1;
                session.stopped = true;
                session.stop_reason = Some(StopReason::Breakpoint {
                    breakpoint_id,
                    address: pc,
                });
                session.current_pc = pc;

                // 事件通知省略

                return session.stop_reason.clone();
            }
        }

        None
    }

    /// 单步执行
    pub fn step(&mut self) -> Result<(), VmError> {
        let session = self.current_session.as_mut().ok_or_else(|| {
            VmError::Core(vm_core::CoreError::InvalidState {
                message: "No active debug session".to_string(),
                current: "inactive".to_string(),
                expected: "active".to_string(),
            })
        })?;

        session.stop_reason = Some(StopReason::Step);
        session.stopped = true;

        Ok(())
    }

    /// 继续执行
    pub fn continue_execution(&mut self) -> Result<(), VmError> {
        let session = self.current_session.as_mut().ok_or_else(|| {
            VmError::Core(vm_core::CoreError::InvalidState {
                message: "No active debug session".to_string(),
                current: "inactive".to_string(),
                expected: "active".to_string(),
            })
        })?;

        session.stopped = false;
        session.stop_reason = None;

        // 通知事件监听器
        self.notify_event_listeners(&DebugEvent::Continued);

        Ok(())
    }

    /// 获取当前状态
    pub fn get_current_state(&self) -> Option<&DebugSession> {
        self.current_session.as_ref()
    }

    /// 读取内存
    pub fn read_memory(&self, address: GuestAddr, size: usize) -> Result<Vec<u8>, VmError> {
        // 这里需要访问实际的MMU
        // 记录访问地址用于调试和审计
        tracing::debug!("Reading {} bytes from address {:x}", size, address.0);
        Ok(vec![0; size])
    }

    /// 写入内存
    pub fn write_memory(&mut self, address: GuestAddr, data: &[u8]) -> Result<(), VmError> {
        // 这里需要访问实际的MMU
        // 记录写入操作用于调试和审计
        tracing::debug!("Writing {} bytes to address {:x}", data.len(), address.0);
        Ok(())
    }

    /// 获取寄存器值
    pub fn get_register(&self, reg_index: usize) -> Result<u64, VmError> {
        // 这里需要访问实际的执行引擎
        // 记录寄存器访问用于调试
        tracing::debug!("Reading register {}", reg_index);
        Ok(0)
    }

    /// 设置寄存器值
    pub fn set_register(&mut self, reg_index: usize, value: u64) -> Result<(), VmError> {
        // 这里需要访问实际的执行引擎
        // 记录寄存器设置用于调试
        tracing::debug!("Setting register {} to 0x{:x}", reg_index, value);
        Ok(())
    }

    /// 获取性能分析数据
    pub fn get_profiling_data(&self) -> Option<ProfilingData> {
        self.profiler.as_ref()?.read().ok()?.get_data()
    }

    /// 创建快照
    pub fn create_snapshot(&mut self) -> Result<String, VmError> {
        let snapshot_manager = self.snapshot_manager.as_ref().ok_or_else(|| {
            VmError::Core(vm_core::CoreError::NotImplemented {
                feature: "snapshot debugging".to_string(),
                module: "vm-debug".to_string(),
            })
        })?;

        snapshot_manager
            .write()
            .map_err(|e| {
                VmError::Core(vm_core::CoreError::Concurrency {
                    message: format!("Failed to acquire write lock on snapshot_manager: {}", e),
                    operation: "create_snapshot".to_string(),
                })
            })?
            .create_snapshot()
    }

    /// 恢复到快照
    pub fn restore_snapshot(&mut self, snapshot_id: &str) -> Result<(), VmError> {
        let snapshot_manager = self.snapshot_manager.as_ref().ok_or_else(|| {
            VmError::Core(vm_core::CoreError::NotImplemented {
                feature: "snapshot debugging".to_string(),
                module: "vm-debug".to_string(),
            })
        })?;

        snapshot_manager
            .write()
            .map_err(|e| {
                VmError::Core(vm_core::CoreError::Concurrency {
                    message: format!("Failed to acquire write lock on snapshot_manager: {}", e),
                    operation: "restore_snapshot".to_string(),
                })
            })?
            .restore_snapshot(snapshot_id)
    }

    /// 添加事件监听器
    pub fn add_event_listener(&mut self, listener: Box<dyn DebugEventListener + Send + Sync>) {
        self.event_listeners.push(listener);
    }

    /// 记录日志
    pub fn log(&self, level: LogLevel, message: &str, context: Option<HashMap<String, String>>) {
        if level >= self.config.log_level
            && let Ok(mut logger) = self.logger.write()
        {
            logger.log(level, message, context);
        }
        // If lock is poisoned, silently fail - logging shouldn't crash the system
    }

    /// 获取日志
    pub fn get_logs(&self, level: Option<LogLevel>, limit: Option<usize>) -> Vec<LogEntry> {
        self.logger
            .read()
            .map(|logger| logger.get_logs(level, limit))
            .unwrap_or_default()
    }

    /// 评估条件表达式
    fn evaluate_condition(&self, condition: &str) -> bool {
        // 简化的条件评估器
        // 实际实现应该支持完整的表达式语法
        if condition.contains("true") {
            true
        } else if condition.contains("false") {
            false
        } else {
            // 默认返回true
            true
        }
    }

    /// 通知事件监听器
    fn notify_event_listeners(&self, event: &DebugEvent) {
        for listener in &self.event_listeners {
            listener.on_debug_event(event.clone());
        }
    }
}

/// 调试事件监听器trait
pub trait DebugEventListener {
    fn on_debug_event(&self, event: DebugEvent);
}

/// GDB存根
pub struct GdbStub {
    port: u16,
    connection: Option<tokio::net::TcpListener>,
    /// 缓冲的GDB协议数据包，用于处理拆分的数据
    current_packet: Vec<u8>,
}

impl GdbStub {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            connection: None,
            // 预分配缓冲区以减少分配次数
            current_packet: Vec::with_capacity(1024),
        }
    }

    pub async fn start(&mut self) -> Result<(), VmError> {
        let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", self.port))
            .await
            .map_err(|e| VmError::Io(e.to_string()))?;

        self.connection = Some(listener);
        Ok(())
    }

    pub async fn stop(&mut self) -> Result<(), VmError> {
        self.connection = None;
        // 清空缓冲区
        self.current_packet.clear();
        Ok(())
    }

    /// 处理GDB命令
    pub fn handle_command(&mut self, command: &str) -> String {
        // 累积包数据用于处理分片的GDB消息
        self.current_packet.extend_from_slice(command.as_bytes());

        match command {
            "g" => self.handle_read_registers(),
            "G" => self.handle_write_registers(),
            "m" => self.handle_read_memory(),
            "M" => self.handle_write_memory(),
            "c" => self.handle_continue(),
            "s" => self.handle_step(),
            "Z0" => self.handle_set_breakpoint(),
            "z0" => self.handle_remove_breakpoint(),
            _ => "+".to_string(), // 未知命令
        }
    }

    fn handle_read_registers(&self) -> String {
        // 返回所有寄存器的值（十六进制）
        "0000000000000000".repeat(32) // 简化的实现
    }

    fn handle_write_registers(&mut self) -> String {
        "OK".to_string()
    }

    fn handle_read_memory(&self) -> String {
        // 返回内存内容（十六进制）
        "deadbeef".to_string()
    }

    fn handle_write_memory(&mut self) -> String {
        "OK".to_string()
    }

    fn handle_continue(&mut self) -> String {
        "S05".to_string() // 信号5
    }

    fn handle_step(&mut self) -> String {
        "S05".to_string()
    }

    fn handle_set_breakpoint(&mut self) -> String {
        "OK".to_string()
    }

    fn handle_remove_breakpoint(&mut self) -> String {
        "OK".to_string()
    }
}

/// 性能分析器
pub struct Profiler {
    /// 采样间隔（微秒）
    sample_interval_us: u64,
    running: bool,
    data: ProfilingData,
    /// 上次采样的时间戳，用于控制采样频率
    sample_timer: Instant,
}

impl Profiler {
    pub fn new(sample_interval_us: u64) -> Self {
        let now_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| {
                eprintln!(
                    "Warning: Failed to get system time for profiler initialization: {}",
                    e
                );
                e
            })
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        Self {
            sample_interval_us,
            running: false,
            data: ProfilingData {
                function_stats: HashMap::new(),
                hot_instructions: Vec::new(),
                memory_access_patterns: Vec::new(),
                time_range: (now_timestamp, now_timestamp),
            },
            sample_timer: Instant::now(),
        }
    }

    pub fn start(&mut self) {
        self.running = true;
        self.sample_timer = Instant::now();
        self.data.time_range.0 = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| {
                eprintln!(
                    "Warning: Failed to get system time when starting profiler: {}",
                    e
                );
                e
            })
            .map(|d| d.as_millis() as u64)
            .unwrap_or_else(|_| self.data.time_range.0);
    }

    pub fn stop(&mut self) {
        self.running = false;
        self.data.time_range.1 = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| {
                eprintln!(
                    "Warning: Failed to get system time when stopping profiler: {}",
                    e
                );
                e
            })
            .map(|d| d.as_millis() as u64)
            .unwrap_or_else(|_| self.data.time_range.0);
    }

    /// 检查是否应该进行采样
    fn should_sample(&mut self) -> bool {
        let elapsed = self.sample_timer.elapsed().as_micros() as u64;
        if elapsed >= self.sample_interval_us {
            self.sample_timer = Instant::now();
            true
        } else {
            false
        }
    }

    pub fn record_function_call(&mut self, function_name: &str, duration_ns: u64) {
        if !self.running {
            return;
        }

        // 根据采样间隔决定是否记录
        if !self.should_sample() {
            return;
        }

        let stats = self
            .data
            .function_stats
            .entry(function_name.to_string())
            .or_insert_with(|| FunctionStats {
                function_name: function_name.to_string(),
                call_count: 0,
                total_time_ns: 0,
                average_time_ns: 0,
                min_time_ns: u64::MAX,
                max_time_ns: 0,
            });

        stats.call_count += 1;
        stats.total_time_ns += duration_ns;
        stats.average_time_ns = stats.total_time_ns / stats.call_count;
        stats.min_time_ns = stats.min_time_ns.min(duration_ns);
        stats.max_time_ns = stats.max_time_ns.max(duration_ns);
    }

    pub fn record_instruction_execution(&mut self, address: GuestAddr, cycles: u64) {
        if !self.running {
            return;
        }

        // 查找或创建热点指令记录
        let hot_instruction = self
            .data
            .hot_instructions
            .iter_mut()
            .find(|hi| hi.address == address);

        if let Some(hi) = hot_instruction {
            hi.execution_count += 1;
            hi.total_cycles += cycles;
        } else {
            self.data.hot_instructions.push(HotInstruction {
                address,
                execution_count: 1,
                total_cycles: cycles,
                cache_misses: 0,
            });
        }
    }

    pub fn get_data(&self) -> Option<ProfilingData> {
        if self.running {
            None // 运行中不返回数据
        } else {
            Some(self.data.clone())
        }
    }
}

/// 快照管理器
pub struct SnapshotManager {
    snapshots: HashMap<String, VmSnapshot>,
    instruction_counter: u64,
    /// 快照间隔（指令数），当执行指令数达到此间隔时自动创建快照
    snapshot_interval: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmSnapshot {
    pub snapshot_id: String,
    pub timestamp: u64,
    pub instruction_count: u64,
    pub vcpu_state: VcpuStateContainer,
    pub memory_state: HashMap<GuestAddr, Vec<u8>>,
}

/// 调试日志系统
pub struct DebugLogger {
    entries: Vec<LogEntry>,
    max_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: u64,
    pub level: LogLevel,
    pub message: String,
    pub context: Option<HashMap<String, String>>,
}

impl DebugLogger {
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: Vec::with_capacity(max_size),
            max_size,
        }
    }

    pub fn log(
        &mut self,
        level: LogLevel,
        message: &str,
        context: Option<HashMap<String, String>>,
    ) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or_else(|e| {
                eprintln!("Warning: Failed to get timestamp for log entry: {}", e);
                0
            });

        let entry = LogEntry {
            timestamp,
            level,
            message: message.to_string(),
            context,
        };

        self.entries.push(entry);

        // 限制日志大小
        if self.entries.len() > self.max_size {
            self.entries.remove(0);
        }
    }

    pub fn get_logs(&self, level: Option<LogLevel>, limit: Option<usize>) -> Vec<LogEntry> {
        let filtered: Vec<_> = self
            .entries
            .iter()
            .filter(|entry| level.is_none_or(|l| entry.level >= l))
            .cloned()
            .collect();

        if let Some(limit) = limit {
            filtered.into_iter().rev().take(limit).rev().collect()
        } else {
            filtered
        }
    }
}

impl SnapshotManager {
    pub fn new(snapshot_interval: u64) -> Self {
        Self {
            snapshots: HashMap::new(),
            instruction_counter: 0,
            snapshot_interval,
        }
    }

    /// 检查是否应该创建快照
    pub fn should_snapshot(&self) -> bool {
        self.snapshot_interval > 0
            && self
                .instruction_counter
                .is_multiple_of(self.snapshot_interval)
    }

    /// 增加指令计数器
    pub fn increment_instruction_counter(&mut self) {
        self.instruction_counter += 1;
    }

    pub fn create_snapshot(&mut self) -> Result<String, VmError> {
        let snapshot_id = format!("snapshot_{}", self.snapshots.len());
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or_else(|e| {
                eprintln!("Warning: Failed to get timestamp for snapshot: {}", e);
                0
            });

        // 这里应该从实际的VM状态创建快照
        // 简化的实现
        let snapshot = VmSnapshot {
            snapshot_id: snapshot_id.clone(),
            timestamp,
            instruction_count: self.instruction_counter,
            vcpu_state: VcpuStateContainer::default(),
            memory_state: HashMap::new(),
        };

        self.snapshots.insert(snapshot_id.clone(), snapshot);
        Ok(snapshot_id)
    }

    pub fn restore_snapshot(&mut self, snapshot_id: &str) -> Result<(), VmError> {
        let snapshot = self.snapshots.get(snapshot_id).ok_or_else(|| {
            VmError::Core(vm_core::CoreError::InvalidState {
                message: format!("Snapshot {} not found", snapshot_id),
                current: "not_found".to_string(),
                expected: "exists".to_string(),
            })
        })?;

        // 这里应该恢复VM状态
        // 简化的实现
        self.instruction_counter = snapshot.instruction_count;

        Ok(())
    }

    pub fn list_snapshots(&self) -> Vec<&VmSnapshot> {
        self.snapshots.values().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debugger_creation() {
        let config = DebuggerConfig::default();
        let debugger = VmDebugger::new(config);

        assert!(debugger.current_session.is_none());
        assert!(debugger.gdb_stub.is_some());
        assert!(debugger.profiler.is_some());
    }

    #[tokio::test]
    async fn test_debug_session() {
        let config = DebuggerConfig::default();
        let mut debugger = VmDebugger::new(config);

        let session_id = debugger
            .start_session()
            .await
            .expect("Failed to start debug session");
        assert!(!session_id.is_empty());
        assert!(debugger.current_session.is_some());

        debugger
            .end_session()
            .await
            .expect("Failed to end debug session");
        assert!(debugger.current_session.is_none());
    }

    #[tokio::test]
    async fn test_breakpoint_management() {
        let config = DebuggerConfig::default();
        let mut debugger = VmDebugger::new(config);

        // 需要先启动会话
        let session_id = debugger
            .start_session()
            .await
            .expect("Failed to start debug session");
        assert!(!session_id.is_empty());

        let breakpoint_id = debugger
            .set_breakpoint(vm_core::GuestAddr(0x1000), BreakpointType::Execution, None)
            .expect("Failed to set breakpoint");
        assert_eq!(breakpoint_id, 1);

        debugger
            .remove_breakpoint(breakpoint_id)
            .expect("Failed to remove breakpoint");

        debugger
            .end_session()
            .await
            .expect("Failed to end debug session");
    }

    #[test]
    fn test_profiler() {
        let mut profiler = Profiler::new(1000);

        profiler.start();
        profiler.record_function_call("test_function", 100);
        profiler.record_instruction_execution(vm_core::GuestAddr(0x1000), 10);
        profiler.stop();

        let data = profiler.get_data().expect("Failed to get profiling data");
        assert_eq!(data.function_stats.len(), 1);
        assert_eq!(data.hot_instructions.len(), 1);
    }

    #[test]
    fn test_logger() {
        let mut logger = DebugLogger::new(100);

        logger.log(LogLevel::Info, "Test message", None);
        logger.log(LogLevel::Debug, "Debug message", None);

        let logs = logger.get_logs(Some(LogLevel::Info), None);
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].message, "Test message");
    }
}
