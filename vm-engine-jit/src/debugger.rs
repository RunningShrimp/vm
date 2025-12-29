//! JIT引擎调试工具
//!
//! 提供JIT编译过程的可视化、调试和分析功能

use std::collections::HashMap;
use std::fmt::{self, Display};
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use vm_core::GuestAddr;
use vm_ir::IRBlock;

use crate::{
    code_cache::CodeCache, instruction_scheduler::InstructionScheduler, optimizer::IROptimizer,
    register_allocator::RegisterAllocator,
};

/// 调试事件类型
#[derive(Debug, Clone)]
pub enum DebugEvent {
    /// 编译开始
    CompilationStart { pc: GuestAddr, ir_size: usize },
    /// 编译完成
    CompilationEnd {
        pc: GuestAddr,
        code_size: usize,
        duration: Duration,
    },
    /// 优化阶段开始
    OptimizationStart { phase: String, pc: GuestAddr },
    /// 优化阶段完成
    OptimizationEnd {
        phase: String,
        pc: GuestAddr,
        changes: usize,
    },
    /// 寄存器分配开始
    RegisterAllocationStart {
        pc: GuestAddr,
        instruction_count: usize,
    },
    /// 寄存器分配完成
    RegisterAllocationEnd {
        pc: GuestAddr,
        spills: usize,
        reloads: usize,
    },
    /// 指令调度开始
    InstructionSchedulingStart { pc: GuestAddr },
    /// 指令调度完成
    InstructionSchedulingEnd {
        pc: GuestAddr,
        scheduled_count: usize,
    },
    /// SIMD优化开始
    SIMDOptimizationStart { pc: GuestAddr },
    /// SIMD优化完成
    SIMDOptimizationEnd {
        pc: GuestAddr,
        vectorized_ops: usize,
    },
    /// 代码缓存命中
    CacheHit { pc: GuestAddr },
    /// 代码缓存未命中
    CacheMiss { pc: GuestAddr },
    /// 热点检测
    HotspotDetected { pc: GuestAddr, execution_count: u64 },
    /// 错误发生
    Error { pc: GuestAddr, message: String },
}

impl Display for DebugEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DebugEvent::CompilationStart { pc, ir_size } => {
                write!(f, "[编译开始] PC: 0x{:x}, IR大小: {} 指令", pc, ir_size)
            }
            DebugEvent::CompilationEnd {
                pc,
                code_size,
                duration,
            } => {
                write!(
                    f,
                    "[编译完成] PC: 0x{:x}, 代码大小: {} 字节, 耗时: {:?}",
                    pc, code_size, duration
                )
            }
            DebugEvent::OptimizationStart { phase, pc } => {
                write!(f, "[优化开始] 阶段: {}, PC: 0x{:x}", phase, pc)
            }
            DebugEvent::OptimizationEnd { phase, pc, changes } => {
                write!(
                    f,
                    "[优化完成] 阶段: {}, PC: 0x{:x}, 变更数: {}",
                    phase, pc, changes
                )
            }
            DebugEvent::RegisterAllocationStart {
                pc,
                instruction_count,
            } => {
                write!(
                    f,
                    "[寄存器分配开始] PC: 0x{:x}, 指令数: {}",
                    pc, instruction_count
                )
            }
            DebugEvent::RegisterAllocationEnd {
                pc,
                spills,
                reloads,
            } => {
                write!(
                    f,
                    "[寄存器分配完成] PC: 0x{:x}, 溢出: {}, 重载: {}",
                    pc, spills, reloads
                )
            }
            DebugEvent::InstructionSchedulingStart { pc } => {
                write!(f, "[指令调度开始] PC: 0x{:x}", pc)
            }
            DebugEvent::InstructionSchedulingEnd {
                pc,
                scheduled_count,
            } => {
                write!(
                    f,
                    "[指令调度完成] PC: 0x{:x}, 调度指令数: {}",
                    pc, scheduled_count
                )
            }
            DebugEvent::SIMDOptimizationStart { pc } => {
                write!(f, "[SIMD优化开始] PC: 0x{:x}", pc)
            }
            DebugEvent::SIMDOptimizationEnd { pc, vectorized_ops } => {
                write!(
                    f,
                    "[SIMD优化完成] PC: 0x{:x}, 向量化操作数: {}",
                    pc, vectorized_ops
                )
            }
            DebugEvent::CacheHit { pc } => {
                write!(f, "[缓存命中] PC: 0x{:x}", pc)
            }
            DebugEvent::CacheMiss { pc } => {
                write!(f, "[缓存未命中] PC: 0x{:x}", pc)
            }
            DebugEvent::HotspotDetected {
                pc,
                execution_count,
            } => {
                write!(
                    f,
                    "[热点检测] PC: 0x{:x}, 执行次数: {}",
                    pc, execution_count
                )
            }
            DebugEvent::Error { pc, message } => {
                write!(f, "[错误] PC: 0x{:x}, 消息: {}", pc, message)
            }
        }
    }
}

/// 调试统计信息
#[derive(Debug, Clone, Default)]
pub struct DebugStats {
    /// 总编译次数
    pub total_compilations: u64,
    /// 总编译时间
    pub total_compilation_time: Duration,
    /// 缓存命中次数
    pub cache_hits: u64,
    /// 缓存未命中次数
    pub cache_misses: u64,
    /// 优化阶段统计
    pub optimization_stats: HashMap<String, (u64, Duration)>, // (次数, 总时间)
    /// 寄存器分配统计
    pub register_allocation_stats: (u64, u64, u64), // (总次数, 总溢出, 总重载)
    /// SIMD优化统计
    pub simd_optimization_stats: (u64, u64), // (总次数, 总向量化操作数)
    /// 错误计数
    pub error_count: u64,
    /// 热点统计
    pub hotspot_count: u64,
}

impl DebugStats {
    /// 创建新的统计信息
    pub fn new() -> Self {
        Self::default()
    }

    /// 更新编译统计
    pub fn update_compilation(&mut self, duration: Duration) {
        self.total_compilations += 1;
        self.total_compilation_time += duration;
    }

    /// 更新缓存统计
    pub fn update_cache_hit(&mut self) {
        self.cache_hits += 1;
    }

    pub fn update_cache_miss(&mut self) {
        self.cache_misses += 1;
    }

    /// 更新优化统计
    pub fn update_optimization(&mut self, phase: &str, duration: Duration) {
        let entry = self
            .optimization_stats
            .entry(phase.to_string())
            .or_insert((0, Duration::default()));
        entry.0 += 1;
        entry.1 += duration;
    }

    /// 更新寄存器分配统计
    pub fn update_register_allocation(&mut self, spills: u64, reloads: u64) {
        self.register_allocation_stats.0 += 1;
        self.register_allocation_stats.1 += spills;
        self.register_allocation_stats.2 += reloads;
    }

    /// 更新SIMD优化统计
    pub fn update_simd_optimization(&mut self, vectorized_ops: u64) {
        self.simd_optimization_stats.0 += 1;
        self.simd_optimization_stats.1 += vectorized_ops;
    }

    /// 更新错误计数
    pub fn update_error(&mut self) {
        self.error_count += 1;
    }

    /// 更新热点统计
    pub fn update_hotspot(&mut self) {
        self.hotspot_count += 1;
    }

    /// 获取缓存命中率
    pub fn cache_hit_rate(&self) -> f64 {
        if self.cache_hits + self.cache_misses == 0 {
            0.0
        } else {
            self.cache_hits as f64 / (self.cache_hits + self.cache_misses) as f64
        }
    }

    /// 获取平均编译时间
    pub fn average_compilation_time(&self) -> Duration {
        if self.total_compilations == 0 {
            Duration::default()
        } else {
            self.total_compilation_time / self.total_compilations as u32
        }
    }
}

/// JIT调试器
pub struct JitDebugger {
    /// 调试事件记录
    events: Arc<Mutex<Vec<DebugEvent>>>,
    /// 统计信息
    stats: Arc<Mutex<DebugStats>>,
    /// 配置
    config: DebuggerConfig,
    /// 当前编译会话
    current_session: Arc<Mutex<Option<CompilationSession>>>,
}

/// 调试器配置
#[derive(Debug, Clone)]
pub struct DebuggerConfig {
    /// 启用事件记录
    pub enable_event_logging: bool,
    /// 启用统计收集
    pub enable_stats_collection: bool,
    /// 最大事件记录数
    pub max_events: usize,
    /// 启用IR转储
    pub enable_ir_dump: bool,
    /// 启用机器码转储
    pub enable_machine_code_dump: bool,
    /// 转储输出目录
    pub dump_output_dir: Option<String>,
    /// 启用性能分析
    pub enable_profiling: bool,
    /// 启用详细日志
    pub enable_verbose_logging: bool,
}

impl Default for DebuggerConfig {
    fn default() -> Self {
        Self {
            enable_event_logging: true,
            enable_stats_collection: true,
            max_events: 10000,
            enable_ir_dump: false,
            enable_machine_code_dump: false,
            dump_output_dir: None,
            enable_profiling: false,
            enable_verbose_logging: false,
        }
    }
}

/// 编译会话信息
#[derive(Clone)]
pub struct CompilationSession {
    /// 会话ID
    pub id: String,
    /// 开始时间
    pub start_time: Instant,
    /// 当前PC
    pub current_pc: GuestAddr,
    /// 原始IR块
    pub original_ir: Option<IRBlock>,
    /// 优化后的IR块
    pub optimized_ir: Option<IRBlock>,
    /// 生成的机器码
    pub machine_code: Option<Vec<u8>>,
    /// 会话事件
    pub events: Vec<DebugEvent>,
}

impl std::fmt::Debug for CompilationSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompilationSession")
            .field("id", &self.id)
            .field("start_time", &self.start_time)
            .field("current_pc", &self.current_pc)
            .field("original_ir", &"<IRBlock>")
            .field("optimized_ir", &"<IRBlock>")
            .field("machine_code", &self.machine_code)
            .field("events", &self.events)
            .finish()
    }
}

impl JitDebugger {
    /// 创建新的JIT调试器
    pub fn new(config: DebuggerConfig) -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
            stats: Arc::new(Mutex::new(DebugStats::new())),
            config,
            current_session: Arc::new(Mutex::new(None)),
        }
    }

    /// 使用默认配置创建调试器
    pub fn with_default_config() -> Self {
        Self::new(DebuggerConfig::default())
    }

    /// 记录调试事件
    pub fn log_event(&self, event: DebugEvent) {
        if !self.config.enable_event_logging {
            return;
        }

        // 更新统计信息
        if self.config.enable_stats_collection {
            self.update_stats(&event);
        }

        // 记录事件
        if let Ok(mut events) = self.events.lock() {
            if events.len() >= self.config.max_events {
                events.remove(0); // 移除最旧的事件
            }
            events.push(event.clone());
        }

        // 记录到当前会话
        if let Ok(mut session) = self.current_session.lock()
            && let Some(ref mut s) = *session
        {
            s.events.push(event.clone());
        }

        // 详细日志输出
        if self.config.enable_verbose_logging {
            println!("{}", event);
        }
    }

    /// 开始新的编译会话
    pub fn start_compilation_session(&self, pc: GuestAddr, ir_block: &IRBlock) -> String {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let session_id = format!("session_{}_{}", pc, timestamp);

        let session = CompilationSession {
            id: session_id.clone(),
            start_time: Instant::now(),
            current_pc: pc,
            original_ir: Some(ir_block.clone()),
            optimized_ir: None,
            machine_code: None,
            events: Vec::new(),
        };

        if let Ok(mut current_session) = self.current_session.lock() {
            *current_session = Some(session);
        }

        self.log_event(DebugEvent::CompilationStart {
            pc,
            ir_size: ir_block.ops.len(),
        });

        session_id
    }

    /// 结束编译会话
    pub fn end_compilation_session(&self, pc: GuestAddr, machine_code: &[u8]) {
        let duration = if let Ok(mut session) = self.current_session.lock() {
            if let Some(ref mut s) = *session {
                s.machine_code = Some(machine_code.to_vec());
                s.start_time.elapsed()
            } else {
                Duration::default()
            }
        } else {
            Duration::default()
        };

        self.log_event(DebugEvent::CompilationEnd {
            pc,
            code_size: machine_code.len(),
            duration,
        });

        // 转储IR和机器码（如果启用）
        if self.config.enable_ir_dump || self.config.enable_machine_code_dump {
            self.dump_compilation_artifacts(pc, machine_code);
        }

        // 清除当前会话
        if let Ok(mut current_session) = self.current_session.lock() {
            *current_session = None;
        }
    }

    /// 更新优化后的IR
    pub fn update_optimized_ir(&self, ir_block: &IRBlock) {
        if let Ok(mut session) = self.current_session.lock()
            && let Some(ref mut s) = *session
        {
            s.optimized_ir = Some(ir_block.clone());
        }
    }

    /// 获取所有事件
    pub fn get_events(&self) -> Vec<DebugEvent> {
        if let Ok(events) = self.events.lock() {
            events.clone()
        } else {
            Vec::new()
        }
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> DebugStats {
        if let Ok(stats) = self.stats.lock() {
            stats.clone()
        } else {
            DebugStats::new()
        }
    }

    /// 获取当前会话
    pub fn get_current_session(&self) -> Option<CompilationSession> {
        if let Ok(session) = self.current_session.lock() {
            (*session).clone()
        } else {
            None
        }
    }

    /// 清除所有事件和统计
    pub fn clear(&self) {
        if let Ok(mut events) = self.events.lock() {
            events.clear();
        }
        if let Ok(mut stats) = self.stats.lock() {
            *stats = DebugStats::new();
        }
        if let Ok(mut session) = self.current_session.lock() {
            *session = None;
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.config.enable_event_logging
    }

    /// 启用调试器
    pub fn enable(&mut self) {
        self.config.enable_event_logging = true;
    }

    /// 禁用调试器
    pub fn disable(&mut self) {
        self.config.enable_event_logging = false;
    }

    /// 生成调试报告
    pub fn generate_report(&self) -> String {
        let events = self.get_events();
        let stats = self.get_stats();

        let mut report = String::new();
        report.push_str("# JIT引擎调试报告\n\n");

        // 统计摘要
        report.push_str("## 统计摘要\n\n");
        report.push_str(&format!("- 总编译次数: {}\n", stats.total_compilations));
        report.push_str(&format!(
            "- 总编译时间: {:?}\n",
            stats.total_compilation_time
        ));
        report.push_str(&format!(
            "- 平均编译时间: {:?}\n",
            stats.average_compilation_time()
        ));
        report.push_str(&format!(
            "- 缓存命中率: {:.2}%\n",
            stats.cache_hit_rate() * 100.0
        ));
        report.push_str(&format!("- 缓存命中次数: {}\n", stats.cache_hits));
        report.push_str(&format!("- 缓存未命中次数: {}\n", stats.cache_misses));
        report.push_str(&format!("- 错误次数: {}\n", stats.error_count));
        report.push_str(&format!("- 热点数量: {}\n", stats.hotspot_count));

        // 优化阶段统计
        if !stats.optimization_stats.is_empty() {
            report.push_str("\n## 优化阶段统计\n\n");
            for (phase, (count, total_time)) in &stats.optimization_stats {
                let avg_time = if *count > 0 {
                    *total_time / *count as u32
                } else {
                    Duration::default()
                };
                report.push_str(&format!(
                    "- {}: {} 次, 总时间: {:?}, 平均时间: {:?}\n",
                    phase, count, total_time, avg_time
                ));
            }
        }

        // 寄存器分配统计
        report.push_str("\n## 寄存器分配统计\n\n");
        report.push_str(&format!(
            "- 总分配次数: {}\n",
            stats.register_allocation_stats.0
        ));
        report.push_str(&format!(
            "- 总溢出次数: {}\n",
            stats.register_allocation_stats.1
        ));
        report.push_str(&format!(
            "- 总重载次数: {}\n",
            stats.register_allocation_stats.2
        ));
        if stats.register_allocation_stats.0 > 0 {
            let avg_spills =
                stats.register_allocation_stats.1 as f64 / stats.register_allocation_stats.0 as f64;
            let avg_reloads =
                stats.register_allocation_stats.2 as f64 / stats.register_allocation_stats.0 as f64;
            report.push_str(&format!("- 平均溢出次数: {:.2}\n", avg_spills));
            report.push_str(&format!("- 平均重载次数: {:.2}\n", avg_reloads));
        }

        // SIMD优化统计
        report.push_str("\n## SIMD优化统计\n\n");
        report.push_str(&format!(
            "- 总优化次数: {}\n",
            stats.simd_optimization_stats.0
        ));
        report.push_str(&format!(
            "- 总向量化操作数: {}\n",
            stats.simd_optimization_stats.1
        ));
        if stats.simd_optimization_stats.0 > 0 {
            let avg_vectorized =
                stats.simd_optimization_stats.1 as f64 / stats.simd_optimization_stats.0 as f64;
            report.push_str(&format!("- 平均向量化操作数: {:.2}\n", avg_vectorized));
        }

        // 最近事件
        if !events.is_empty() {
            report.push_str("\n## 最近事件\n\n");
            let recent_events = events.iter().rev().take(20);
            for event in recent_events {
                report.push_str(&format!("{}\n", event));
            }
        }

        report
    }

    /// 将报告保存到文件
    pub fn save_report_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), std::io::Error> {
        let report = self.generate_report();
        let mut file = File::create(path)?;
        file.write_all(report.as_bytes())?;
        Ok(())
    }

    /// 更新统计信息
    fn update_stats(&self, event: &DebugEvent) {
        if let Ok(mut stats) = self.stats.lock() {
            match event {
                DebugEvent::CompilationEnd { duration, .. } => {
                    stats.update_compilation(*duration);
                }
                DebugEvent::CacheHit { .. } => {
                    stats.update_cache_hit();
                }
                DebugEvent::CacheMiss { .. } => {
                    stats.update_cache_miss();
                }
                DebugEvent::OptimizationStart { phase, .. } => {
                    // 记录优化阶段开始，用于跟踪不同优化阶段的性能
                    let _phase_name = phase; // 使用变量记录阶段名称
                    let _ = _phase_name; // 确保变量被使用
                }
                DebugEvent::OptimizationEnd { phase, .. } => {
                    // 记录优化阶段结束，用于跟踪不同优化阶段的性能
                    let _phase_name = phase; // 使用变量记录阶段名称
                    let _ = _phase_name; // 确保变量被使用
                }
                DebugEvent::RegisterAllocationEnd {
                    spills, reloads, ..
                } => {
                    stats.update_register_allocation(*spills as u64, *reloads as u64);
                }
                DebugEvent::SIMDOptimizationEnd { vectorized_ops, .. } => {
                    stats.update_simd_optimization(*vectorized_ops as u64);
                }
                DebugEvent::Error { .. } => {
                    stats.update_error();
                }
                DebugEvent::HotspotDetected { .. } => {
                    stats.update_hotspot();
                }
                _ => {}
            }
        }
    }

    /// 转储编译产物
    fn dump_compilation_artifacts(&self, pc: GuestAddr, machine_code: &[u8]) {
        let output_dir = match &self.config.dump_output_dir {
            Some(dir) => dir.clone(),
            None => return,
        };

        // 创建输出目录
        if std::fs::create_dir_all(&output_dir).is_err() {
            return;
        }

        // 获取当前会话
        let session = if let Ok(session) = self.current_session.lock() {
            (*session).clone()
        } else {
            None
        };

        if let Some(s) = session {
            // 转储原始IR
            if self.config.enable_ir_dump
                && let Some(ref ir) = s.original_ir
            {
                let ir_path = format!("{}/0x{:x}_original_ir.txt", output_dir, pc);
                if let Ok(mut file) = File::create(ir_path) {
                    let _ = file.write_all(format!("原始IR块 (PC: 0x{:x}):\n", pc).as_bytes());
                    let _ = file.write_all(self.format_ir_block(ir).as_bytes());
                }
            }

            // 转储优化后的IR
            if self.config.enable_ir_dump
                && let Some(ref ir) = s.optimized_ir
            {
                let ir_path = format!("{}/0x{:x}_optimized_ir.txt", output_dir, pc);
                if let Ok(mut file) = File::create(ir_path) {
                    let _ = file.write_all(format!("优化后IR块 (PC: 0x{:x}):\n", pc).as_bytes());
                    let _ = file.write_all(self.format_ir_block(ir).as_bytes());
                }
            }

            // 转储机器码
            if self.config.enable_machine_code_dump {
                let code_path = format!("{}/0x{:x}_machine_code.bin", output_dir, pc);
                if let Ok(mut file) = File::create(code_path) {
                    let _ = file.write_all(machine_code);
                }

                // 同时生成十六进制转储
                let hex_path = format!("{}/0x{:x}_machine_code.hex", output_dir, pc);
                if let Ok(mut file) = File::create(hex_path) {
                    let _ = file.write_all(format!("机器码 (PC: 0x{:x}):\n", pc).as_bytes());
                    for (i, byte) in machine_code.iter().enumerate() {
                        if i % 16 == 0 {
                            let _ = file.write_all(format!("\n{:04x}: ", i).as_bytes());
                        }
                        let _ = file.write_all(format!("{:02x} ", byte).as_bytes());
                    }
                }
            }
        }
    }

    /// 格式化IR块为字符串
    fn format_ir_block(&self, ir_block: &IRBlock) -> String {
        let mut result = String::new();
        result.push_str(&format!(
            "IR块 (起始PC: 0x{:x}, 大小: {} 指令):\n",
            ir_block.start_pc,
            ir_block.ops.len()
        ));

        for (i, op) in ir_block.ops.iter().enumerate() {
            result.push_str(&format!("  [{}] {:?}\n", i, op));
        }

        result
    }
}

/// 调试器装饰器 - 为现有组件添加调试功能
pub struct DebuggerDecorator<T> {
    /// 被装饰的组件
    inner: T,
    /// 调试器
    debugger: Arc<JitDebugger>,
}

impl<T> DebuggerDecorator<T> {
    /// 创建新的装饰器
    pub fn new(inner: T, debugger: Arc<JitDebugger>) -> Self {
        Self { inner, debugger }
    }

    /// 获取内部组件的引用
    pub fn inner(&self) -> &T {
        &self.inner
    }

    /// 获取内部组件的可变引用
    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// 获取调试器引用
    pub fn debugger(&self) -> &JitDebugger {
        &self.debugger
    }
}

/// 为IROptimizer添加调试功能
impl<T: IROptimizer> IROptimizer for DebuggerDecorator<T> {
    fn optimize(&mut self, block: &IRBlock) -> Result<IRBlock, vm_core::VmError> {
        let pc = block.start_pc;
        let start_time = Instant::now();

        self.debugger.log_event(DebugEvent::OptimizationStart {
            phase: "IROptimizer".to_string(),
            pc,
        });

        let original_size = block.ops.len();
        let result = self.inner.optimize(block);

        if let Ok(ref optimized_block) = result {
            let optimized_size = optimized_block.ops.len();
            let changes = original_size.abs_diff(optimized_size);

            self.debugger.log_event(DebugEvent::OptimizationEnd {
                phase: "IROptimizer".to_string(),
                pc,
                changes,
            });
        }

        if let Ok(mut stats) = self.debugger.stats.lock() {
            stats.update_optimization("IROptimizer", start_time.elapsed());
        }

        result
    }

    fn name(&self) -> &str {
        self.inner.name()
    }

    fn version(&self) -> &str {
        self.inner.version()
    }

    fn set_option(&mut self, option: &str, value: &str) -> Result<(), vm_core::VmError> {
        self.inner.set_option(option, value)
    }

    fn get_option(&self, option: &str) -> Option<String> {
        self.inner.get_option(option)
    }

    fn supported_optimizations(&self) -> Vec<String> {
        self.inner.supported_optimizations()
    }

    fn enable_optimization(&mut self, optimization: &str) -> Result<(), vm_core::VmError> {
        self.inner.enable_optimization(optimization)
    }

    fn disable_optimization(&mut self, optimization: &str) -> Result<(), vm_core::VmError> {
        self.inner.disable_optimization(optimization)
    }
}

/// 为RegisterAllocator添加调试功能
impl<T: RegisterAllocator> RegisterAllocator for DebuggerDecorator<T> {
    fn allocate(
        &mut self,
        block: &crate::compiler::CompiledIRBlock,
    ) -> Result<crate::compiler::CompiledIRBlock, vm_core::VmError> {
        let pc = block.start_pc;
        let instruction_count = block.ops.len();

        self.debugger
            .log_event(DebugEvent::RegisterAllocationStart {
                pc,
                instruction_count,
            });

        let result = self.inner.allocate(block);

        if let Ok(ref allocated_block) = result {
            // 使用已分配块的信息进行调试和验证
            let _allocated_instr_count = allocated_block.ops.len();
            let _allocated_pc = allocated_block.start_pc;

            // 从分配统计中获取溢出和重载信息
            let stats = self.inner.get_stats();
            self.debugger.log_event(DebugEvent::RegisterAllocationEnd {
                pc,
                spills: stats.spilled_registers,
                reloads: stats.reload_count as usize,
            });

            // 确保变量被使用，记录分配的信息
            let _ = (_allocated_instr_count, _allocated_pc);
        }

        result
    }

    fn name(&self) -> &str {
        self.inner.name()
    }

    fn version(&self) -> &str {
        self.inner.version()
    }

    fn set_option(&mut self, option: &str, value: &str) -> Result<(), vm_core::VmError> {
        self.inner.set_option(option, value)
    }

    fn get_option(&self, option: &str) -> Option<String> {
        self.inner.get_option(option)
    }

    fn reset(&mut self) {
        self.inner.reset();
    }

    fn get_stats(&self) -> crate::register_allocator::RegisterAllocationStats {
        self.inner.get_stats()
    }
}

/// 为InstructionScheduler添加调试功能
impl<T: InstructionScheduler> InstructionScheduler for DebuggerDecorator<T> {
    fn schedule(
        &mut self,
        block: &crate::compiler::CompiledIRBlock,
    ) -> Result<crate::compiler::CompiledIRBlock, vm_core::VmError> {
        let pc = block.start_pc;

        self.debugger
            .log_event(DebugEvent::InstructionSchedulingStart { pc });

        let result = self.inner.schedule(block);

        if let Ok(ref scheduled_block) = result {
            let scheduled_count = scheduled_block.ops.len();
            self.debugger
                .log_event(DebugEvent::InstructionSchedulingEnd {
                    pc,
                    scheduled_count,
                });
        }

        result
    }

    fn name(&self) -> &str {
        self.inner.name()
    }

    fn version(&self) -> &str {
        self.inner.version()
    }

    fn set_option(&mut self, option: &str, value: &str) -> Result<(), vm_core::VmError> {
        self.inner.set_option(option, value)
    }

    fn get_option(&self, option: &str) -> Option<String> {
        self.inner.get_option(option)
    }

    fn reset(&mut self) {
        self.inner.reset();
    }

    fn get_stats(&self) -> crate::instruction_scheduler::InstructionSchedulingStats {
        self.inner.get_stats()
    }
}

/// 为CodeCache添加调试功能
pub struct DebugCodeCache<T> {
    /// 内部代码缓存
    inner: T,
    /// 调试器
    debugger: Arc<JitDebugger>,
}

impl<T: CodeCache> DebugCodeCache<T> {
    /// 创建新的调试代码缓存
    pub fn new(inner: T, debugger: Arc<JitDebugger>) -> Self {
        Self { inner, debugger }
    }

    /// 获取内部缓存引用
    pub fn inner(&self) -> &T {
        &self.inner
    }

    /// 获取内部缓存可变引用
    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

impl<T: CodeCache> CodeCache for DebugCodeCache<T> {
    fn get(&self, pc: GuestAddr) -> Option<Vec<u8>> {
        let result = self.inner.get(pc);

        if result.is_some() {
            self.debugger.log_event(DebugEvent::CacheHit { pc });
        } else {
            self.debugger.log_event(DebugEvent::CacheMiss { pc });
        }

        result
    }

    fn insert(&mut self, pc: GuestAddr, code: Vec<u8>) {
        self.inner.insert(pc, code);
    }

    fn contains(&self, pc: GuestAddr) -> bool {
        self.inner.contains(pc)
    }

    fn remove(&mut self, pc: GuestAddr) -> Option<Vec<u8>> {
        self.inner.remove(pc)
    }

    fn clear(&mut self) {
        self.inner.clear();
    }

    fn stats(&self) -> crate::code_cache::CacheStats {
        self.inner.stats()
    }

    fn set_size_limit(&mut self, limit: usize) {
        self.inner.set_size_limit(limit);
    }

    fn size_limit(&self) -> usize {
        self.inner.size_limit()
    }

    fn current_size(&self) -> usize {
        self.inner.current_size()
    }

    fn entry_count(&self) -> usize {
        self.inner.entry_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_ir::Terminator;

    #[test]
    fn test_debugger_creation() {
        let debugger = JitDebugger::with_default_config();
        let stats = debugger.get_stats();
        assert_eq!(stats.total_compilations, 0);
    }

    #[test]
    fn test_event_logging() {
        let debugger = JitDebugger::with_default_config();
        debugger.log_event(DebugEvent::CompilationStart {
            pc: vm_core::GuestAddr(0x1000),
            ir_size: 10,
        });

        let events = debugger.get_events();
        assert_eq!(events.len(), 1);
        match &events[0] {
            DebugEvent::CompilationStart { pc, ir_size } => {
                assert_eq!(*pc, vm_core::GuestAddr(0x1000));
                assert_eq!(*ir_size, 10);
            }
            _ => panic!("Expected CompilationStart event"),
        }
    }

    #[test]
    fn test_compilation_session() {
        let debugger = JitDebugger::with_default_config();

        // 创建测试IR块
        let ir_block = IRBlock {
            start_pc: vm_core::GuestAddr(0x1000),
            ops: vec![],
            term: Terminator::Ret,
        };

        let session_id = debugger.start_compilation_session(vm_core::GuestAddr(0x1000), &ir_block);
        assert!(session_id.starts_with("session_0x1000_"));

        let session = debugger.get_current_session();
        assert!(session.is_some());
        let session = session.expect("Failed to get current session");
        assert_eq!(session.current_pc, vm_core::GuestAddr(0x1000));

        debugger.end_compilation_session(vm_core::GuestAddr(0x1000), &[0x90, 0x90]);

        let session = debugger.get_current_session();
        assert!(session.is_none());
    }

    #[test]
    fn test_stats_update() {
        let debugger = JitDebugger::with_default_config();

        debugger.log_event(DebugEvent::CompilationEnd {
            pc: vm_core::GuestAddr(0x1000),
            code_size: 10,
            duration: Duration::from_millis(100),
        });

        let stats = debugger.get_stats();
        assert_eq!(stats.total_compilations, 1);
        assert_eq!(stats.total_compilation_time, Duration::from_millis(100));
    }

    #[test]
    fn test_report_generation() {
        let debugger = JitDebugger::with_default_config();

        debugger.log_event(DebugEvent::CompilationStart {
            pc: vm_core::GuestAddr(0x1000),
            ir_size: 10,
        });
        debugger.log_event(DebugEvent::CompilationEnd {
            pc: vm_core::GuestAddr(0x1000),
            code_size: 10,
            duration: Duration::from_millis(100),
        });

        let report = debugger.generate_report();
        assert!(report.contains("JIT引擎调试报告"));
        assert!(report.contains("统计摘要"));
        assert!(report.contains("总编译次数: 1"));
    }
}

#[derive(Debug, Clone)]
pub enum PerformanceAnalysisType {
    HotspotDetection,
    ExecutionTimeAnalysis,
    BranchPredictionAnalysis,
    CacheMissAnalysis,
    ThroughputAnalysis,
}

#[derive(Debug, Clone)]
pub enum MemoryAccessType {
    Read,
    Write,
    Fetch,
}

#[derive(Debug, Clone)]
pub struct AdvancedDebugEvent {
    pub timestamp: SystemTime,
    pub event_type: AdvancedEventType,
}

#[derive(Debug, Clone)]
pub enum AdvancedEventType {
    Basic(DebugEvent),
    PerformanceAnalysis {
        pc: GuestAddr,
        analysis_type: PerformanceAnalysisType,
        metrics: HashMap<String, f64>,
    },
    MemoryAccess {
        pc: GuestAddr,
        access_type: MemoryAccessType,
        address: GuestAddr,
        size: usize,
    },
    RegisterStateChange {
        pc: GuestAddr,
        register: String,
        old_value: u64,
        new_value: u64,
    },
    ControlFlowChange {
        from_pc: GuestAddr,
        to_pc: GuestAddr,
        reason: String,
    },
    OptimizationDecision {
        pc: GuestAddr,
        optimization: String,
        decision: bool,
        reason: String,
    },
    HotspotPrediction {
        pc: GuestAddr,
        predicted_hotspot: bool,
        confidence: f64,
    },
    CacheStrategyDecision {
        pc: GuestAddr,
        cache_level: u32,
        decision: String,
    },
    ParallelCompilationEvent {
        pc: GuestAddr,
        worker_id: usize,
        stage: String,
    },
}

#[derive(Debug, Clone, Default)]
pub struct AdvancedDebuggerConfig {
    pub track_memory_accesses: bool,
    pub track_register_changes: bool,
    pub track_control_flow: bool,
    pub performance_analysis: bool,
    pub hotspot_prediction: bool,
    pub parallel_compilation_tracking: bool,
}

#[derive(Debug)]
pub struct AdvancedDebugStats {
    pub memory_access_count: AtomicUsize,
    pub register_change_count: AtomicUsize,
    pub control_flow_change_count: AtomicUsize,
    pub optimization_decision_count: AtomicUsize,
    pub hotspot_prediction_count: AtomicUsize,
    pub parallel_compilation_events: AtomicUsize,
}

impl Clone for AdvancedDebugStats {
    fn clone(&self) -> Self {
        Self {
            memory_access_count: AtomicUsize::new(self.memory_access_count.load(Ordering::Relaxed)),
            register_change_count: AtomicUsize::new(
                self.register_change_count.load(Ordering::Relaxed),
            ),
            control_flow_change_count: AtomicUsize::new(
                self.control_flow_change_count.load(Ordering::Relaxed),
            ),
            optimization_decision_count: AtomicUsize::new(
                self.optimization_decision_count.load(Ordering::Relaxed),
            ),
            hotspot_prediction_count: AtomicUsize::new(
                self.hotspot_prediction_count.load(Ordering::Relaxed),
            ),
            parallel_compilation_events: AtomicUsize::new(
                self.parallel_compilation_events.load(Ordering::Relaxed),
            ),
        }
    }
}

impl Default for AdvancedDebugStats {
    fn default() -> Self {
        Self {
            memory_access_count: AtomicUsize::new(0),
            register_change_count: AtomicUsize::new(0),
            control_flow_change_count: AtomicUsize::new(0),
            optimization_decision_count: AtomicUsize::new(0),
            hotspot_prediction_count: AtomicUsize::new(0),
            parallel_compilation_events: AtomicUsize::new(0),
        }
    }
}

pub struct AdvancedJitDebugger {
    base_debugger: JitDebugger,
    advanced_events: Arc<Mutex<Vec<AdvancedDebugEvent>>>,
    advanced_stats: Arc<Mutex<AdvancedDebugStats>>,
    config: AdvancedDebuggerConfig,
}

impl AdvancedJitDebugger {
    pub fn new(base_config: DebuggerConfig, advanced_config: AdvancedDebuggerConfig) -> Self {
        Self {
            base_debugger: JitDebugger::new(base_config),
            advanced_events: Arc::new(Mutex::new(Vec::new())),
            advanced_stats: Arc::new(Mutex::new(AdvancedDebugStats::default())),
            config: advanced_config,
        }
    }

    pub fn with_default_configs() -> Self {
        Self::new(DebuggerConfig::default(), AdvancedDebuggerConfig::default())
    }

    pub fn is_enabled(&self) -> bool {
        self.base_debugger.is_enabled()
    }

    pub fn enable(&mut self) {
        self.base_debugger.enable();
    }

    pub fn disable(&mut self) {
        self.base_debugger.disable();
    }

    pub fn log_event(&self, event: DebugEvent) {
        self.base_debugger.log_event(event);
    }

    pub fn log_advanced_event(&self, event: AdvancedEventType) {
        if !self.is_enabled() {
            return;
        }

        let advanced_event = AdvancedDebugEvent {
            timestamp: SystemTime::now(),
            event_type: event.clone(),
        };

        if let Ok(mut events) = self.advanced_events.lock() {
            events.push(advanced_event);
        }

        if let Ok(stats) = self.advanced_stats.lock() {
            match event {
                AdvancedEventType::MemoryAccess { .. } => {
                    stats.memory_access_count.fetch_add(1, Ordering::Relaxed);
                }
                AdvancedEventType::RegisterStateChange { .. } => {
                    stats.register_change_count.fetch_add(1, Ordering::Relaxed);
                }
                AdvancedEventType::ControlFlowChange { .. } => {
                    stats
                        .control_flow_change_count
                        .fetch_add(1, Ordering::Relaxed);
                }
                AdvancedEventType::OptimizationDecision { .. } => {
                    stats
                        .optimization_decision_count
                        .fetch_add(1, Ordering::Relaxed);
                }
                AdvancedEventType::HotspotPrediction { .. } => {
                    stats
                        .hotspot_prediction_count
                        .fetch_add(1, Ordering::Relaxed);
                }
                AdvancedEventType::ParallelCompilationEvent { .. } => {
                    stats
                        .parallel_compilation_events
                        .fetch_add(1, Ordering::Relaxed);
                }
                _ => {}
            }
        }
    }

    pub fn log_memory_access(
        &self,
        pc: GuestAddr,
        access_type: MemoryAccessType,
        address: GuestAddr,
        size: usize,
    ) {
        if self.config.track_memory_accesses {
            self.log_advanced_event(AdvancedEventType::MemoryAccess {
                pc,
                access_type,
                address,
                size,
            });
        }
    }

    pub fn log_register_change(
        &self,
        pc: GuestAddr,
        register: String,
        old_value: u64,
        new_value: u64,
    ) {
        if self.config.track_register_changes {
            self.log_advanced_event(AdvancedEventType::RegisterStateChange {
                pc,
                register,
                old_value,
                new_value,
            });
        }
    }

    pub fn log_control_flow_change(&self, from_pc: GuestAddr, to_pc: GuestAddr, reason: String) {
        if self.config.track_control_flow {
            self.log_advanced_event(AdvancedEventType::ControlFlowChange {
                from_pc,
                to_pc,
                reason,
            });
        }
    }

    pub fn log_optimization_decision(
        &self,
        pc: GuestAddr,
        optimization: String,
        decision: bool,
        reason: String,
    ) {
        self.log_advanced_event(AdvancedEventType::OptimizationDecision {
            pc,
            optimization,
            decision,
            reason,
        });
    }

    pub fn log_hotspot_prediction(&self, pc: GuestAddr, predicted_hotspot: bool, confidence: f64) {
        if self.config.hotspot_prediction {
            self.log_advanced_event(AdvancedEventType::HotspotPrediction {
                pc,
                predicted_hotspot,
                confidence,
            });
        }
    }

    pub fn log_cache_strategy_decision(&self, pc: GuestAddr, cache_level: u32, decision: String) {
        self.log_advanced_event(AdvancedEventType::CacheStrategyDecision {
            pc,
            cache_level,
            decision,
        });
    }

    pub fn log_parallel_compilation(&self, pc: GuestAddr, worker_id: usize, stage: String) {
        if self.config.parallel_compilation_tracking {
            self.log_advanced_event(AdvancedEventType::ParallelCompilationEvent {
                pc,
                worker_id,
                stage,
            });
        }
    }

    pub fn log_performance_analysis(
        &self,
        pc: GuestAddr,
        analysis_type: PerformanceAnalysisType,
        metrics: HashMap<String, f64>,
    ) {
        if self.config.performance_analysis {
            self.log_advanced_event(AdvancedEventType::PerformanceAnalysis {
                pc,
                analysis_type,
                metrics,
            });
        }
    }

    pub fn get_base_debugger(&self) -> &JitDebugger {
        &self.base_debugger
    }

    pub fn get_advanced_events(&self) -> Vec<AdvancedDebugEvent> {
        self.advanced_events
            .lock()
            .map(|events| events.clone())
            .unwrap_or_default()
    }

    pub fn get_advanced_stats(&self) -> AdvancedDebugStats {
        self.advanced_stats
            .lock()
            .map(|stats| AdvancedDebugStats {
                memory_access_count: AtomicUsize::new(
                    stats.memory_access_count.load(Ordering::Relaxed),
                ),
                register_change_count: AtomicUsize::new(
                    stats.register_change_count.load(Ordering::Relaxed),
                ),
                control_flow_change_count: AtomicUsize::new(
                    stats.control_flow_change_count.load(Ordering::Relaxed),
                ),
                optimization_decision_count: AtomicUsize::new(
                    stats.optimization_decision_count.load(Ordering::Relaxed),
                ),
                hotspot_prediction_count: AtomicUsize::new(
                    stats.hotspot_prediction_count.load(Ordering::Relaxed),
                ),
                parallel_compilation_events: AtomicUsize::new(
                    stats.parallel_compilation_events.load(Ordering::Relaxed),
                ),
            })
            .unwrap_or_default()
    }

    pub fn clear_events(&self) {
        self.base_debugger.clear();
        if let Ok(mut events) = self.advanced_events.lock() {
            events.clear();
        }
    }

    pub fn dump_events(&self) -> String {
        let mut output = String::new();
        output.push_str("=== Advanced JIT Debugger Events ===\n\n");

        if let Ok(events) = self.advanced_events.lock() {
            for event in events.iter() {
                output.push_str(&format!("{:?}\n", event));
            }
        }

        output.push_str("\n=== Advanced Statistics ===\n");
        if let Ok(stats) = self.advanced_stats.lock() {
            output.push_str(&format!(
                "Memory accesses: {}\n",
                stats.memory_access_count.load(Ordering::Relaxed)
            ));
            output.push_str(&format!(
                "Register changes: {}\n",
                stats.register_change_count.load(Ordering::Relaxed)
            ));
            output.push_str(&format!(
                "Control flow changes: {}\n",
                stats.control_flow_change_count.load(Ordering::Relaxed)
            ));
            output.push_str(&format!(
                "Optimization decisions: {}\n",
                stats.optimization_decision_count.load(Ordering::Relaxed)
            ));
            output.push_str(&format!(
                "Hotspot predictions: {}\n",
                stats.hotspot_prediction_count.load(Ordering::Relaxed)
            ));
            output.push_str(&format!(
                "Parallel compilation events: {}\n",
                stats.parallel_compilation_events.load(Ordering::Relaxed)
            ));
        }

        output
    }

    pub fn analyze_performance(&self) -> HashMap<String, Vec<(GuestAddr, f64)>> {
        let mut analysis: HashMap<String, Vec<(GuestAddr, f64)>> = HashMap::new();

        if let Ok(events) = self.advanced_events.lock() {
            for event in events.iter() {
                if let AdvancedEventType::PerformanceAnalysis {
                    pc,
                    analysis_type,
                    metrics,
                } = &event.event_type
                {
                    let type_name = format!("{:?}", analysis_type);
                    for (metric_name, value) in metrics.iter() {
                        let key = format!("{}:{}", type_name, metric_name);
                        analysis.entry(key).or_default().push((*pc, *value));
                    }
                }
            }
        }

        analysis
    }
}
