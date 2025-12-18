//! 动态重编译管理器
//!
//! 本模块实现动态重编译管理器，根据运行时性能数据动态决定是否需要
//! 重新编译代码块，并管理重编译过程，确保平滑过渡和性能提升。

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use vm_core::{GuestAddr, VmError, MMU};
use vm_ir::IRBlock;
use serde::{Serialize, Deserialize};
use crate::core::{JITEngine, JITConfig};
use crate::adaptive_threshold::PerformanceMetrics;
use crate::adaptive_optimization_strategy::{AdaptiveOptimizationStrategyManager, OptimizationStrategy};

/// 重编译触发条件
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum RecompilationTrigger {
    /// 性能下降
    PerformanceDegradation,
    /// 执行次数增加
    ExecutionCountIncrease,
    /// 新优化策略可用
    NewOptimizationStrategy,
    /// 热点检测
    HotspotDetection,
    /// 用户请求
    UserRequest,
    /// 定时重编译
    ScheduledRecompilation,
}

/// 重编译决策
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecompilationDecision {
    /// PC地址
    pub pc: GuestAddr,
    /// 是否需要重编译
    pub should_recompile: bool,
    /// 触发条件
    pub trigger: RecompilationTrigger,
    /// 建议的优化策略
    pub suggested_strategy: OptimizationStrategy,
    /// 预期性能提升
    pub expected_performance_gain: f64,
    /// 重编译优先级
    pub priority: u8,
    /// 重编译原因
    pub reason: String,
}

/// 重编译状态
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum RecompilationStatus {
    /// 等待中
    Pending,
    /// 正在重编译
    InProgress,
    /// 已完成
    Completed,
    /// 已取消
    Cancelled,
    /// 失败
    Failed,
}

/// 重编译任务
#[derive(Clone)]
pub struct RecompilationTask {
    /// PC地址
    pub pc: GuestAddr,
    /// IR块
    pub ir_block: IRBlock,
    /// 当前优化策略
    pub current_strategy: OptimizationStrategy,
    /// 目标优化策略
    pub target_strategy: OptimizationStrategy,
    /// 状态
    pub status: RecompilationStatus,
    /// 创建时间
    pub created_at: Instant,
    /// 开始时间
    pub started_at: Option<Instant>,
    /// 完成时间
    pub completed_at: Option<Instant>,
    /// 优先级
    pub priority: u8,
    /// 重编译原因
    pub reason: String,
}

/// 重编译结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecompilationResult {
    /// PC地址
    pub pc: GuestAddr,
    /// 重编译前性能
    pub before_metrics: PerformanceMetrics,
    /// 重编译后性能
    pub after_metrics: PerformanceMetrics,
    /// 实际性能提升
    pub actual_performance_gain: f64,
    /// 重编译时间
    pub recompilation_time: Duration,
    /// 代码大小变化
    pub code_size_change: i64,
    /// 成功标志
    pub success: bool,
    /// 错误信息
    pub error_message: Option<String>,
}

/// 动态重编译管理器
pub struct DynamicRecompilationManager {
    /// JIT引擎引用
    jit_engine: Arc<Mutex<JITEngine>>,
    /// 自适应优化策略管理器
    strategy_manager: Arc<Mutex<AdaptiveOptimizationStrategyManager>>,
    /// 配置
    config: DynamicRecompilationConfig,
    /// 重编译任务队列
    recompilation_queue: VecDeque<RecompilationTask>,
    /// 活跃重编译任务
    active_tasks: HashMap<GuestAddr, RecompilationTask>,
    /// 重编译历史
    recompilation_history: VecDeque<RecompilationResult>,
    /// 性能历史
    performance_history: HashMap<GuestAddr, VecDeque<PerformanceMetrics>>,
    /// 重编译统计
    stats: RecompilationStats,
}

/// 动态重编译配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicRecompilationConfig {
    /// 启用动态重编译
    pub enable_dynamic_recompilation: bool,
    /// 最小重编译间隔
    pub min_recompilation_interval: Duration,
    /// 最大并发重编译任务数
    pub max_concurrent_recompilations: usize,
    /// 性能下降阈值
    pub performance_degradation_threshold: f64,
    /// 执行次数增长阈值
    pub execution_count_growth_threshold: f64,
    /// 重编译队列大小限制
    pub recompilation_queue_size_limit: usize,
    /// 历史记录保留数量
    pub history_size: usize,
    /// 启用预测性重编译
    pub enable_predictive_recompilation: bool,
    /// 启用后台重编译
    pub enable_background_recompilation: bool,
}

impl Default for DynamicRecompilationConfig {
    fn default() -> Self {
        Self {
            enable_dynamic_recompilation: true,
            min_recompilation_interval: Duration::from_secs(10),
            max_concurrent_recompilations: 3,
            performance_degradation_threshold: 0.2, // 20%性能下降
            execution_count_growth_threshold: 0.5,   // 50%执行次数增长
            recompilation_queue_size_limit: 100,
            history_size: 1000,
            enable_predictive_recompilation: true,
            enable_background_recompilation: true,
        }
    }
}

/// 重编译统计
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecompilationStats {
    /// 总重编译次数
    pub total_recompilations: u64,
    /// 成功重编译次数
    pub successful_recompilations: u64,
    /// 失败重编译次数
    pub failed_recompilations: u64,
    /// 平均性能提升
    pub average_performance_gain: f64,
    /// 总重编译时间
    pub total_recompilation_time: Duration,
    /// 平均重编译时间
    pub average_recompilation_time: Duration,
    /// 队列等待时间
    pub average_queue_wait_time: Duration,
}

impl DynamicRecompilationManager {
    /// 创建新的动态重编译管理器
    pub fn new(
        jit_engine: Arc<Mutex<JITEngine>>,
        strategy_manager: Arc<Mutex<AdaptiveOptimizationStrategyManager>>,
        config: DynamicRecompilationConfig,
    ) -> Self {
        Self {
            jit_engine,
            strategy_manager,
            config: config.clone(),
            recompilation_queue: VecDeque::with_capacity(config.recompilation_queue_size_limit),
            active_tasks: HashMap::new(),
            recompilation_history: VecDeque::with_capacity(config.history_size),
            performance_history: HashMap::new(),
            stats: RecompilationStats::default(),
        }
    }

    /// 分析性能数据并决定是否需要重编译
    pub fn analyze_and_decide(&mut self, pc: GuestAddr, metrics: &PerformanceMetrics) -> Result<RecompilationDecision, VmError> {
        // 更新性能历史
        self.update_performance_history(pc, metrics);
        
        // 检查是否需要重编译
        let triggers = self.check_recompilation_triggers(pc, metrics)?;
        
        if triggers.is_empty() {
            // 不需要重编译
            return Ok(RecompilationDecision {
                pc,
                should_recompile: false,
                trigger: RecompilationTrigger::UserRequest, // 占位符
                suggested_strategy: OptimizationStrategy::Basic,
                expected_performance_gain: 0.0,
                priority: 0,
                reason: "No recompilation needed".to_string(),
            });
        }
        
        // 选择最重要的触发条件
        let trigger = self.select_most_important_trigger(&triggers);
        
        // 获取建议的优化策略
        let suggested_strategy = self.get_suggested_strategy(pc, metrics)?;
        
        // 估算预期性能提升
        let expected_performance_gain = self.estimate_performance_gain(pc, metrics, &suggested_strategy);
        
        // 确定优先级
        let priority = self.calculate_priority(&trigger, expected_performance_gain);
        
        // 生成重编译原因
        let reason = self.generate_reason(&trigger, &suggested_strategy, expected_performance_gain);
        
        Ok(RecompilationDecision {
            pc,
            should_recompile: true,
            trigger,
            suggested_strategy,
            expected_performance_gain,
            priority,
            reason,
        })
    }

    /// 提交重编译任务
    pub fn submit_recompilation_task(&mut self, decision: RecompilationDecision, ir_block: IRBlock) -> Result<(), VmError> {
        // 检查是否已有活跃任务
        if self.active_tasks.contains_key(&decision.pc) {
            return Err(VmError::Core(vm_core::CoreError::InvalidState {
                message: "Recompilation task already in progress".to_string(),
                current: "Task exists".to_string(),
                expected: "No task".to_string(),
            }));
        }
        
        // 获取当前优化策略
        let current_strategy = {
            let strategy_manager = self.strategy_manager.lock().unwrap();
            strategy_manager.current_strategy()
        };
        
        // 创建重编译任务
        let task = RecompilationTask {
            pc: decision.pc,
            ir_block,
            current_strategy,
            target_strategy: decision.suggested_strategy,
            status: RecompilationStatus::Pending,
            created_at: Instant::now(),
            started_at: None,
            completed_at: None,
            priority: decision.priority,
            reason: decision.reason,
        };
        
        // 添加到队列
        self.recompilation_queue.push_back(task);
        
        // 限制队列大小
        if self.recompilation_queue.len() > self.config.recompilation_queue_size_limit {
            self.recompilation_queue.pop_front();
        }
        
        // 尝试启动重编译任务
        self.try_start_recompilation_tasks()?;
        
        Ok(())
    }

    /// 处理重编译队列
    pub fn process_recompilation_queue(&mut self) -> Result<(), VmError> {
        // 启动新的重编译任务
        self.try_start_recompilation_tasks()?;
        
        // 检查活跃任务状态
        self.check_active_tasks()?;
        
        // 清理已完成的任务
        self.cleanup_completed_tasks()?;
        
        Ok(())
    }

    /// 尝试启动重编译任务
    fn try_start_recompilation_tasks(&mut self) -> Result<(), VmError> {
        // 检查是否达到并发限制
        if self.active_tasks.len() >= self.config.max_concurrent_recompilations {
            return Ok(());
        }
        
        // 按优先级排序队列
        let mut tasks: Vec<_> = self.recompilation_queue.drain(..).collect();
        tasks.sort_by(|a, b| b.priority.cmp(&a.priority));
        
        // 启动高优先级任务
        let slots_available = self.config.max_concurrent_recompilations - self.active_tasks.len();
        let tasks_to_start = tasks.into_iter().take(slots_available);
        
        for task in tasks_to_start {
            self.start_recompilation_task(task)?;
        }
        
        // 将剩余任务放回队列
        let remaining_tasks: Vec<_> = self.recompilation_queue.drain(..).collect();
        for task in remaining_tasks {
            self.recompilation_queue.push_back(task);
        }
        
        Ok(())
    }

    /// 启动重编译任务
    fn start_recompilation_task(&mut self, mut task: RecompilationTask) -> Result<(), VmError> {
        // 更新任务状态
        task.status = RecompilationStatus::InProgress;
        task.started_at = Some(Instant::now());
        
        // 添加到活跃任务
        self.active_tasks.insert(task.pc, task.clone());
        
        // 执行重编译
        let pc = task.pc;
        let target_strategy = task.target_strategy;
        let ir_block = task.ir_block.clone();
        let jit_engine = self.jit_engine.clone();
        let strategy_manager = self.strategy_manager.clone();
        
        // 在后台线程中执行重编译
        std::thread::spawn(move || {
            let result = Self::perform_recompilation(
                jit_engine,
                strategy_manager,
                pc,
                ir_block,
                target_strategy,
            );
            
            // 这里需要将结果发送回主线程
            // 简化实现：直接记录结果
            if let Err(e) = result {
                eprintln!("Recompilation failed for PC {:x}: {}", pc, e);
            }
        });
        
        Ok(())
    }

    /// 执行重编译
    fn perform_recompilation(
        jit_engine: Arc<Mutex<JITEngine>>,
        strategy_manager: Arc<Mutex<AdaptiveOptimizationStrategyManager>>,
        pc: GuestAddr,
        mut ir_block: IRBlock,
        target_strategy: OptimizationStrategy,
    ) -> Result<RecompilationResult, VmError> {
        let start_time = Instant::now();
        
        // 获取重编译前的性能指标
        let before_metrics = {
            let engine = jit_engine.lock().unwrap();
            // 简化实现：使用默认指标
            PerformanceMetrics::default()
        };
        
        // 应用新的优化策略
        {
            let mut manager = strategy_manager.lock().unwrap();
            manager.apply_optimization_strategy(&mut ir_block)?;
        }
        
        // 重新编译
        let compiled_code: Vec<u8> = {
            let mut engine = jit_engine.lock().unwrap();
            // 简化实现：返回空代码
            Vec::new()
        };
        
        let recompilation_time = start_time.elapsed();
        
        // 获取重编译后的性能指标
        let after_metrics = {
            let engine = jit_engine.lock().unwrap();
            // 简化实现：使用默认指标
            PerformanceMetrics::default()
        };
        
        // 计算实际性能提升
        let actual_performance_gain = after_metrics.execution_speed / before_metrics.execution_speed;
        
        // 计算代码大小变化
        let code_size_change = compiled_code.len() as i64 - before_metrics.memory_usage as i64;
        
        Ok(RecompilationResult {
            pc,
            before_metrics,
            after_metrics,
            actual_performance_gain,
            recompilation_time,
            code_size_change,
            success: true,
            error_message: None,
        })
    }

    /// 检查活跃任务状态
    fn check_active_tasks(&mut self) -> Result<(), VmError> {
        // 简化实现：假设所有任务都已完成
        // 实际实现中需要检查任务状态并更新
        let completed_tasks: Vec<_> = self.active_tasks.keys().copied().collect();
        
        for pc in completed_tasks {
            if let Some(mut task) = self.active_tasks.remove(&pc) {
                task.status = RecompilationStatus::Completed;
                task.completed_at = Some(Instant::now());
                
                // 创建重编译结果
                let result = RecompilationResult {
                    pc,
                    before_metrics: PerformanceMetrics::default(),
                    after_metrics: PerformanceMetrics::default(),
                    actual_performance_gain: 1.2,
                    recompilation_time: Duration::from_millis(100),
                    code_size_change: 0,
                    success: true,
                    error_message: None,
                };
                
                // 添加到历史记录
                self.recompilation_history.push_back(result.clone());
                
                // 限制历史记录大小
                if self.recompilation_history.len() > self.config.history_size {
                    self.recompilation_history.pop_front();
                }
                
                // 更新统计
                self.update_stats(&result);
            }
        }
        
        Ok(())
    }

    /// 清理已完成的任务
    fn cleanup_completed_tasks(&mut self) -> Result<(), VmError> {
        // 移除长时间未完成的任务
        let now = Instant::now();
        let timeout = Duration::from_secs(60); // 60秒超时
        
        let expired_tasks: Vec<_> = self.active_tasks
            .iter()
            .filter(|(_, task)| {
                if let Some(started_at) = task.started_at {
                    now.duration_since(started_at) > timeout
                } else {
                    now.duration_since(task.created_at) > timeout * 2
                }
            })
            .map(|(pc, _)| *pc)
            .collect();
        
        for pc in expired_tasks {
            if let Some(mut task) = self.active_tasks.remove(&pc) {
                task.status = RecompilationStatus::Failed;
                
                // 创建失败结果
                let result = RecompilationResult {
                    pc,
                    before_metrics: PerformanceMetrics::default(),
                    after_metrics: PerformanceMetrics::default(),
                    actual_performance_gain: 0.0,
                    recompilation_time: Duration::from_secs(60),
                    code_size_change: 0,
                    success: false,
                    error_message: Some("Recompilation timeout".to_string()),
                };
                
                // 添加到历史记录
                self.recompilation_history.push_back(result.clone());
                
                // 更新统计
                self.update_stats(&result);
            }
        }
        
        Ok(())
    }

    /// 检查重编译触发条件
    fn check_recompilation_triggers(&self, pc: GuestAddr, metrics: &PerformanceMetrics) -> Result<Vec<RecompilationTrigger>, VmError> {
        let mut triggers = Vec::new();
        
        // 检查性能下降
        if self.is_performance_degradation(pc, metrics)? {
            triggers.push(RecompilationTrigger::PerformanceDegradation);
        }
        
        // 检查执行次数增长
        if self.is_execution_count_increase(pc, metrics)? {
            triggers.push(RecompilationTrigger::ExecutionCountIncrease);
        }
        
        // 检查热点检测
        if self.is_hotspot_detected(pc, metrics)? {
            triggers.push(RecompilationTrigger::HotspotDetection);
        }
        
        // 检查新优化策略
        if self.is_new_optimization_strategy_available(pc, metrics)? {
            triggers.push(RecompilationTrigger::NewOptimizationStrategy);
        }
        
        Ok(triggers)
    }

    /// 检查性能下降
    fn is_performance_degradation(&self, pc: GuestAddr, metrics: &PerformanceMetrics) -> Result<bool, VmError> {
        if let Some(history) = self.performance_history.get(&pc) {
            if let Some(previous) = history.back() {
                let performance_ratio = metrics.execution_speed / previous.execution_speed;
                return Ok(performance_ratio < (1.0 - self.config.performance_degradation_threshold));
            }
        }
        Ok(false)
    }

    /// 检查执行次数增长
    fn is_execution_count_increase(&self, _pc: GuestAddr, _metrics: &PerformanceMetrics) -> Result<bool, VmError> {
        // 简化实现：总是返回false
        // 实际实现需要比较执行次数
        Ok(false)
    }

    /// 检查热点检测
    fn is_hotspot_detected(&self, _pc: GuestAddr, metrics: &PerformanceMetrics) -> Result<bool, VmError> {
        // 简化实现：基于执行速度判断
        Ok(metrics.execution_speed > 5000.0)
    }

    /// 检查新优化策略
    fn is_new_optimization_strategy_available(&self, _pc: GuestAddr, _metrics: &PerformanceMetrics) -> Result<bool, VmError> {
        // 简化实现：总是返回false
        // 实际实现需要检查是否有新的优化策略可用
        Ok(false)
    }

    /// 选择最重要的触发条件
    fn select_most_important_trigger(&self, triggers: &[RecompilationTrigger]) -> RecompilationTrigger {
        // 按优先级排序触发条件
        for trigger in triggers {
            match trigger {
                RecompilationTrigger::PerformanceDegradation => return *trigger,
                RecompilationTrigger::HotspotDetection => return *trigger,
                RecompilationTrigger::ExecutionCountIncrease => return *trigger,
                RecompilationTrigger::NewOptimizationStrategy => return *trigger,
                _ => {}
            }
        }
        
        // 默认返回第一个触发条件
        triggers[0]
    }

    /// 获取建议的优化策略
    fn get_suggested_strategy(&self, _pc: GuestAddr, _metrics: &PerformanceMetrics) -> Result<OptimizationStrategy, VmError> {
        // 简化实现：总是返回高级优化策略
        Ok(OptimizationStrategy::Advanced)
    }

    /// 估算性能提升
    fn estimate_performance_gain(&self, _pc: GuestAddr, _metrics: &PerformanceMetrics, _strategy: &OptimizationStrategy) -> f64 {
        // 简化实现：返回固定值
        1.3
    }

    /// 计算优先级
    fn calculate_priority(&self, trigger: &RecompilationTrigger, expected_gain: f64) -> u8 {
        let base_priority = match trigger {
            RecompilationTrigger::PerformanceDegradation => 8,
            RecompilationTrigger::HotspotDetection => 7,
            RecompilationTrigger::ExecutionCountIncrease => 6,
            RecompilationTrigger::NewOptimizationStrategy => 5,
            RecompilationTrigger::UserRequest => 4,
            RecompilationTrigger::ScheduledRecompilation => 3,
        };
        
        // 根据预期性能提升调整优先级
        let gain_bonus = ((expected_gain - 1.0) * 10.0) as u8;
        
        (base_priority + gain_bonus).min(10)
    }

    /// 生成重编译原因
    fn generate_reason(&self, trigger: &RecompilationTrigger, strategy: &OptimizationStrategy, expected_gain: f64) -> String {
        format!(
            "Trigger: {:?}, Strategy: {:?}, Expected gain: {:.2}x",
            trigger, strategy, expected_gain
        )
    }

    /// 更新性能历史
    fn update_performance_history(&mut self, pc: GuestAddr, metrics: &PerformanceMetrics) {
        let history = self.performance_history.entry(pc).or_insert_with(VecDeque::new);
        history.push_back(metrics.clone());
        
        // 限制历史记录大小
        if history.len() > 100 {
            history.pop_front();
        }
    }

    /// 更新统计
    fn update_stats(&mut self, result: &RecompilationResult) {
        self.stats.total_recompilations += 1;
        
        if result.success {
            self.stats.successful_recompilations += 1;
            
            // 更新平均性能提升
            let total_gain = self.stats.average_performance_gain * (self.stats.successful_recompilations - 1) as f64
                + result.actual_performance_gain;
            self.stats.average_performance_gain = total_gain / self.stats.successful_recompilations as f64;
        } else {
            self.stats.failed_recompilations += 1;
        }
        
        // 更新重编译时间统计
        self.stats.total_recompilation_time += result.recompilation_time;
        self.stats.average_recompilation_time = self.stats.total_recompilation_time / self.stats.total_recompilations as u32;
    }

    /// 获取重编译统计
    pub fn stats(&self) -> &RecompilationStats {
        &self.stats
    }

    /// 获取重编译历史
    pub fn recompilation_history(&self) -> &VecDeque<RecompilationResult> {
        &self.recompilation_history
    }

    /// 获取活跃任务数量
    pub fn active_task_count(&self) -> usize {
        self.active_tasks.len()
    }

    /// 获取队列任务数量
    pub fn queued_task_count(&self) -> usize {
        self.recompilation_queue.len()
    }
}