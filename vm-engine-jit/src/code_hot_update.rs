//! 代码热更新管理器
//!
//! 本模块实现代码热更新管理器，支持在运行时替换已编译的代码块，
//! 确保平滑过渡和一致性，同时最小化性能影响。

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use vm_core::{GuestAddr, VmError, MMU};
use vm_ir::IRBlock;
use serde::{Serialize, Deserialize};
use crate::core::{JITEngine, JITConfig};
use crate::adaptive_threshold::PerformanceMetrics;
use crate::dynamic_recompilation::{RecompilationResult, DynamicRecompilationManager};

/// 热更新状态
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum HotUpdateStatus {
    /// 未开始
    NotStarted,
    /// 准备中
    Preparing,
    /// 准备就绪
    Ready,
    /// 正在更新
    Updating,
    /// 已完成
    Completed,
    /// 已回滚
    RolledBack,
    /// 失败
    Failed,
}

/// 热更新策略
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum HotUpdateStrategy {
    /// 立即替换
    Immediate,
    /// 延迟替换
    Deferred,
    /// 基于计数器替换
    CounterBased,
    /// 基于时间点替换
    TimeBased,
    /// 基于安全点替换
    SafepointBased,
}

/// 热更新任务
#[derive(Debug, Clone)]
pub struct HotUpdateTask {
    /// PC地址
    pub pc: GuestAddr,
    /// 旧代码
    pub old_code: Vec<u8>,
    /// 新代码
    pub new_code: Vec<u8>,
    /// 状态
    pub status: HotUpdateStatus,
    /// 策略
    pub strategy: HotUpdateStrategy,
    /// 创建时间
    pub created_at: Instant,
    /// 开始时间
    pub started_at: Option<Instant>,
    /// 完成时间
    pub completed_at: Option<Instant>,
    /// 优先级
    pub priority: u8,
    /// 重试次数
    pub retry_count: u8,
    /// 最大重试次数
    pub max_retries: u8,
    /// 更新原因
    pub reason: String,
    /// 回滚数据
    pub rollback_data: Option<RollbackData>,
}

/// 回滚数据
#[derive(Debug, Clone)]
pub struct RollbackData {
    /// 原始代码
    pub original_code: Vec<u8>,
    /// 回滚原因
    pub rollback_reason: String,
    /// 回滚时间
    pub rollback_time: Instant,
    /// 性能指标
    pub performance_metrics: PerformanceMetrics,
}

/// 热更新结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotUpdateResult {
    /// PC地址
    pub pc: GuestAddr,
    /// 是否成功
    pub success: bool,
    /// 更新时间
    pub update_time: Duration,
    /// 性能影响
    pub performance_impact: f64,
    /// 错误信息
    pub error_message: Option<String>,
    /// 是否回滚
    pub rolled_back: bool,
}

/// 代码热更新管理器
pub struct CodeHotUpdateManager {
    /// JIT引擎引用
    jit_engine: Arc<Mutex<JITEngine>>,
    /// 动态重编译管理器
    recompilation_manager: Arc<Mutex<DynamicRecompilationManager>>,
    /// 配置
    config: HotUpdateConfig,
    /// 热更新任务队列
    update_queue: VecDeque<HotUpdateTask>,
    /// 活跃更新任务
    active_updates: HashMap<GuestAddr, HotUpdateTask>,
    /// 更新历史
    update_history: VecDeque<HotUpdateResult>,
    /// 代码版本管理
    code_versions: HashMap<GuestAddr, CodeVersionInfo>,
    /// 执行计数器
    execution_counters: HashMap<GuestAddr, u64>,
    /// 安全点管理器
    safepoint_manager: SafepointManager,
    /// 更新统计
    stats: HotUpdateStats,
}

/// 热更新配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotUpdateConfig {
    /// 启用热更新
    pub enable_hot_update: bool,
    /// 默认更新策略
    pub default_strategy: HotUpdateStrategy,
    /// 最大并发更新数
    pub max_concurrent_updates: usize,
    /// 更新队列大小限制
    pub update_queue_size_limit: usize,
    /// 历史记录保留数量
    pub history_size: usize,
    /// 最大重试次数
    pub max_retries: u8,
    /// 更新超时时间
    pub update_timeout: Duration,
    /// 启用自动回滚
    pub enable_auto_rollback: bool,
    /// 性能下降阈值
    pub performance_degradation_threshold: f64,
    /// 回滚观察时间
    rollback_observation_time: Duration,
    /// 启用安全点检查
    pub enable_safepoint_check: bool,
    /// 安全点检查间隔
    pub safepoint_check_interval: Duration,
}

impl Default for HotUpdateConfig {
    fn default() -> Self {
        Self {
            enable_hot_update: true,
            default_strategy: HotUpdateStrategy::SafepointBased,
            max_concurrent_updates: 2,
            update_queue_size_limit: 50,
            history_size: 500,
            max_retries: 3,
            update_timeout: Duration::from_secs(30),
            enable_auto_rollback: true,
            performance_degradation_threshold: 0.3, // 30%性能下降
            rollback_observation_time: Duration::from_secs(10),
            enable_safepoint_check: true,
            safepoint_check_interval: Duration::from_millis(100),
        }
    }
}

/// 代码版本信息
#[derive(Debug, Clone)]
struct CodeVersionInfo {
    /// 当前版本号
    pub current_version: u32,
    /// 代码数据
    pub code_data: HashMap<u32, Vec<u8>>,
    /// 版本创建时间
    pub version_timestamps: HashMap<u32, Instant>,
    /// 版本性能指标
    pub version_metrics: HashMap<u32, PerformanceMetrics>,
}

/// 安全点管理器
#[derive(Debug)]
struct SafepointManager {
    /// 当前是否在安全点
    pub at_safepoint: bool,
    /// 上次检查时间
    pub last_check: Instant,
    /// 检查间隔
    pub check_interval: Duration,
    /// 等待更新的PC列表
    pub pending_updates: Vec<GuestAddr>,
}

/// 热更新统计
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HotUpdateStats {
    /// 总更新次数
    pub total_updates: u64,
    /// 成功更新次数
    pub successful_updates: u64,
    /// 失败更新次数
    pub failed_updates: u64,
    /// 回滚次数
    pub rollback_count: u64,
    /// 平均更新时间
    pub average_update_time: Duration,
    /// 总更新时间
    pub total_update_time: Duration,
    /// 平均性能影响
    pub average_performance_impact: f64,
}

impl CodeHotUpdateManager {
    /// 创建新的代码热更新管理器
    pub fn new(
        jit_engine: Arc<Mutex<JITEngine>>,
        recompilation_manager: Arc<Mutex<DynamicRecompilationManager>>,
        config: HotUpdateConfig,
    ) -> Self {
        Self {
            jit_engine,
            recompilation_manager,
            config: config.clone(),
            update_queue: VecDeque::with_capacity(config.update_queue_size_limit),
            active_updates: HashMap::new(),
            update_history: VecDeque::with_capacity(config.history_size),
            code_versions: HashMap::new(),
            execution_counters: HashMap::new(),
            safepoint_manager: SafepointManager {
                at_safepoint: false,
                last_check: Instant::now(),
                check_interval: config.safepoint_check_interval,
                pending_updates: Vec::new(),
            },
            stats: HotUpdateStats::default(),
        }
    }

    /// 提交热更新任务
    pub fn submit_hot_update(
        &mut self,
        pc: GuestAddr,
        old_code: Vec<u8>,
        new_code: Vec<u8>,
        reason: String,
    ) -> Result<(), VmError> {
        // 检查是否已有活跃更新
        if self.active_updates.contains_key(&pc) {
            return Err(VmError::Core(vm_core::CoreError::InvalidState {
                message: "Hot update already in progress".to_string(),
                current: "Update exists".to_string(),
                expected: "No update".to_string(),
            }));
        }
        
        // 创建热更新任务
        let task = HotUpdateTask {
            pc,
            old_code,
            new_code,
            status: HotUpdateStatus::NotStarted,
            strategy: self.config.default_strategy,
            created_at: Instant::now(),
            started_at: None,
            completed_at: None,
            priority: 5, // 默认优先级
            retry_count: 0,
            max_retries: self.config.max_retries,
            reason,
            rollback_data: None,
        };
        
        // 添加到队列
        self.update_queue.push_back(task);
        
        // 限制队列大小
        if self.update_queue.len() > self.config.update_queue_size_limit {
            self.update_queue.pop_front();
        }
        
        // 尝试启动更新任务
        self.try_start_update_tasks()?;
        
        Ok(())
    }

    /// 处理热更新队列
    pub fn process_update_queue(&mut self) -> Result<(), VmError> {
        // 检查安全点
        self.check_safepoint()?;
        
        // 启动新的更新任务
        self.try_start_update_tasks()?;
        
        // 检查活跃任务状态
        self.check_active_updates()?;
        
        // 清理已完成的任务
        self.cleanup_completed_updates()?;
        
        // 检查自动回滚
        self.check_auto_rollback()?;
        
        Ok(())
    }

    /// 尝试启动更新任务
    fn try_start_update_tasks(&mut self) -> Result<(), VmError> {
        // 检查是否达到并发限制
        if self.active_updates.len() >= self.config.max_concurrent_updates {
            return Ok(());
        }
        
        // 按优先级排序队列
        let mut tasks: Vec<_> = self.update_queue.drain(..).collect();
        tasks.sort_by(|a, b| b.priority.cmp(&a.priority));
        
        // 启动高优先级任务
        let slots_available = self.config.max_concurrent_updates - self.active_updates.len();
        let tasks_to_start = tasks.into_iter().take(slots_available);
        
        for task in tasks_to_start {
            self.start_hot_update(task)?;
        }
        
        // 将剩余任务放回队列
        let remaining_tasks: Vec<_> = self.update_queue.drain(..).collect();
        for task in remaining_tasks {
            self.update_queue.push_back(task);
        }
        
        Ok(())
    }

    /// 启动热更新
    fn start_hot_update(&mut self, mut task: HotUpdateTask) -> Result<(), VmError> {
        // 更新任务状态
        task.status = HotUpdateStatus::Preparing;
        task.started_at = Some(Instant::now());
        
        // 准备更新
        self.prepare_update(&mut task)?;
        
        // 添加到活跃任务
        self.active_updates.insert(task.pc, task.clone());
        
        // 根据策略执行更新
        match task.strategy {
            HotUpdateStrategy::Immediate => {
                self.perform_immediate_update(task)?;
            }
            HotUpdateStrategy::Deferred => {
                task.status = HotUpdateStatus::Ready;
                // 延迟更新将在后续处理中执行
            }
            HotUpdateStrategy::CounterBased => {
                self.prepare_counter_based_update(task)?;
            }
            HotUpdateStrategy::TimeBased => {
                self.prepare_time_based_update(task)?;
            }
            HotUpdateStrategy::SafepointBased => {
                self.prepare_safepoint_based_update(task)?;
            }
        }
        
        Ok(())
    }

    /// 准备更新
    fn prepare_update(&mut self, task: &mut HotUpdateTask) -> Result<(), VmError> {
        // 创建代码版本信息
        let version_info = self.code_versions.entry(task.pc).or_insert_with(|| CodeVersionInfo {
            current_version: 0,
            code_data: HashMap::new(),
            version_timestamps: HashMap::new(),
            version_metrics: HashMap::new(),
        });
        
        // 保存旧代码版本
        let old_version = version_info.current_version;
        version_info.code_data.insert(old_version, task.old_code.clone());
        version_info.version_timestamps.insert(old_version, Instant::now());
        
        // 准备新代码版本
        let new_version = old_version + 1;
        version_info.code_data.insert(new_version, task.new_code.clone());
        version_info.version_timestamps.insert(new_version, Instant::now());
        
        // 创建回滚数据
        task.rollback_data = Some(RollbackData {
            original_code: task.old_code.clone(),
            rollback_reason: "Hot update rollback".to_string(),
            rollback_time: Instant::now(),
            performance_metrics: PerformanceMetrics::default(),
        });
        
        Ok(())
    }

    /// 执行立即更新
    fn perform_immediate_update(&mut self, mut task: HotUpdateTask) -> Result<(), VmError> {
        task.status = HotUpdateStatus::Updating;
        
        // 执行代码替换
        let start_time = Instant::now();
        
        // 更新代码缓存
        {
            let mut jit_engine = self.jit_engine.lock()
                .map_err(|_| VmError::LockPoisoned("JITEngine mutex poisoned".into()))?;
            // 简化实现：实际需要更新代码缓存
            // jit_engine.update_code_cache(task.pc, &task.new_code)?;
        }
        
        let update_time = start_time.elapsed();
        
        // 更新版本信息
        if let Some(version_info) = self.code_versions.get_mut(&task.pc) {
            version_info.current_version += 1;
        }
        
        // 完成更新
        task.status = HotUpdateStatus::Completed;
        task.completed_at = Some(Instant::now());
        
        // 创建更新结果
        let result = HotUpdateResult {
            pc: task.pc,
            success: true,
            update_time,
            performance_impact: 0.0, // 简化实现
            error_message: None,
            rolled_back: false,
        };
        
        // 添加到历史记录
        self.update_history.push_back(result.clone());
        
        // 限制历史记录大小
        if self.update_history.len() > self.config.history_size {
            self.update_history.pop_front();
        }
        
        // 更新统计
        self.update_stats(&result);
        
        Ok(())
    }

    /// 准备基于计数器的更新
    fn prepare_counter_based_update(&mut self, task: HotUpdateTask) -> Result<(), VmError> {
        // 设置执行计数器阈值
        let current_count = self.execution_counters.get(&task.pc).copied().unwrap_or(0);
        let threshold = current_count + 1000; // 每1000次执行后更新
        
        // 简化实现：直接执行更新
        self.perform_immediate_update(task)
    }

    /// 准备基于时间的更新
    fn prepare_time_based_update(&mut self, task: HotUpdateTask) -> Result<(), VmError> {
        // 设置更新时间点
        let update_time = Instant::now() + Duration::from_secs(5); // 5秒后更新
        
        // 简化实现：直接执行更新
        self.perform_immediate_update(task)
    }

    /// 准备基于安全点的更新
    fn prepare_safepoint_based_update(&mut self, mut task: HotUpdateTask) -> Result<(), VmError> {
        // 添加到等待列表
        self.safepoint_manager.pending_updates.push(task.pc);
        
        // 如果当前在安全点，立即执行更新
        if self.safepoint_manager.at_safepoint {
            self.perform_immediate_update(task)
        } else {
            // 等待安全点
            task.status = HotUpdateStatus::Ready;
            Ok(())
        }
    }

    /// 检查安全点
    fn check_safepoint(&mut self) -> Result<(), VmError> {
        let now = Instant::now();
        
        // 检查是否到了检查时间
        if now.duration_since(self.safepoint_manager.last_check) < self.safepoint_manager.check_interval {
            return Ok(());
        }
        
        self.safepoint_manager.last_check = now;
        
        // 简化实现：假设当前在安全点
        self.safepoint_manager.at_safepoint = true;
        
        // 处理等待的更新
        if self.safepoint_manager.at_safepoint && !self.safepoint_manager.pending_updates.is_empty() {
            let pending_updates: Vec<_> = self.safepoint_manager.pending_updates.drain(..).collect();
            
            for pc in pending_updates {
                if let Some(task) = self.active_updates.get(&pc) {
                    self.perform_immediate_update(task.clone())?;
                }
            }
        }
        
        Ok(())
    }

    /// 检查活跃更新状态
    fn check_active_updates(&mut self) -> Result<(), VmError> {
        let now = Instant::now();
        let mut completed_updates = Vec::new();
        
        for (pc, task) in &self.active_updates {
            // 检查超时
            if let Some(started_at) = task.started_at {
                if now.duration_since(started_at) > self.config.update_timeout {
                    completed_updates.push(*pc);
                }
            }
        }
        
        // 处理超时的更新
        for pc in completed_updates {
            if let Some(mut task) = self.active_updates.remove(&pc) {
                task.status = HotUpdateStatus::Failed;
                
                // 创建失败结果
                let result = HotUpdateResult {
                    pc,
                    success: false,
                    update_time: self.config.update_timeout,
                    performance_impact: 0.0,
                    error_message: Some("Update timeout".to_string()),
                    rolled_back: false,
                };
                
                // 添加到历史记录
                self.update_history.push_back(result.clone());
                
                // 更新统计
                self.update_stats(&result);
                
                // 检查是否需要重试
                if task.retry_count < task.max_retries {
                    task.retry_count += 1;
                    task.status = HotUpdateStatus::NotStarted;
                    task.started_at = None;
                    self.update_queue.push_back(task);
                }
            }
        }
        
        Ok(())
    }

    /// 清理已完成的更新
    fn cleanup_completed_updates(&mut self) -> Result<(), VmError> {
        let mut completed_updates = Vec::new();
        
        for (pc, task) in &self.active_updates {
            if task.status == HotUpdateStatus::Completed || task.status == HotUpdateStatus::Failed {
                completed_updates.push(*pc);
            }
        }
        
        for pc in completed_updates {
            self.active_updates.remove(&pc);
        }
        
        Ok(())
    }

    /// 检查自动回滚
    fn check_auto_rollback(&mut self) -> Result<(), VmError> {
        if !self.config.enable_auto_rollback {
            return Ok(());
        }
        
        let now = Instant::now();
        let mut rollbacks = Vec::new();
        
        // 检查最近的更新
        for result in self.update_history.iter().rev().take(10) {
            if !result.success {
                continue;
            }
            
            // 检查是否在观察期内
            if let Some(version_info) = self.code_versions.get(&result.pc) {
                if let Some(current_version) = version_info.code_data.get(&version_info.current_version) {
                    if let Some(timestamp) = version_info.version_timestamps.get(&version_info.current_version) {
                        if now.duration_since(*timestamp) < self.config.rollback_observation_time {
                            // 检查性能下降
                            if result.performance_impact < -self.config.performance_degradation_threshold {
                                rollbacks.push(result.pc);
                            }
                        }
                    }
                }
            }
        }
        
        // 执行回滚
        for pc in rollbacks {
            self.perform_rollback(pc)?;
        }
        
        Ok(())
    }

    /// 执行回滚
    fn perform_rollback(&mut self, pc: GuestAddr) -> Result<(), VmError> {
        if let Some(version_info) = self.code_versions.get_mut(&pc) {
            if version_info.current_version > 0 {
                // 回滚到上一个版本
                let old_version = version_info.current_version;
                let new_version = old_version - 1;
                
                if let Some(old_code) = version_info.code_data.get(&new_version) {
                    // 更新代码缓存
                    {
                        let mut jit_engine = self.jit_engine.lock()
                            .map_err(|_| VmError::LockPoisoned("JITEngine mutex poisoned".into()))?;
                        // 简化实现：实际需要更新代码缓存
                        // jit_engine.update_code_cache(pc, old_code)?;
                    }
                    
                    // 更新版本信息
                    version_info.current_version = new_version;
                    
                    // 更新统计
                    self.stats.rollback_count += 1;
                    
                    // 创建回滚结果
                    let result = HotUpdateResult {
                        pc,
                        success: true,
                        update_time: Duration::from_millis(10),
                        performance_impact: 0.0,
                        error_message: None,
                        rolled_back: true,
                    };
                    
                    // 添加到历史记录
                    self.update_history.push_back(result);
                }
            }
        }
        
        Ok(())
    }

    /// 更新执行计数器
    pub fn update_execution_counter(&mut self, pc: GuestAddr, count: u64) {
        let counter = self.execution_counters.entry(pc).or_insert(0);
        *counter += count;
    }

    /// 更新统计
    fn update_stats(&mut self, result: &HotUpdateResult) {
        self.stats.total_updates += 1;
        
        if result.success {
            self.stats.successful_updates += 1;
        } else {
            self.stats.failed_updates += 1;
        }
        
        // 更新时间统计
        self.stats.total_update_time += result.update_time;
        self.stats.average_update_time = self.stats.total_update_time / self.stats.total_updates as u32;
        
        // 更新性能影响统计
        let total_impact = self.stats.average_performance_impact * (self.stats.total_updates - 1) as f64
            + result.performance_impact;
        self.stats.average_performance_impact = total_impact / self.stats.total_updates as f64;
    }

    /// 获取热更新统计
    pub fn stats(&self) -> &HotUpdateStats {
        &self.stats
    }

    /// 获取更新历史
    pub fn update_history(&self) -> &VecDeque<HotUpdateResult> {
        &self.update_history
    }

    /// 获取活跃更新数量
    pub fn active_update_count(&self) -> usize {
        self.active_updates.len()
    }

    /// 获取队列更新数量
    pub fn queued_update_count(&self) -> usize {
        self.update_queue.len()
    }

    /// 设置安全点状态
    pub fn set_safepoint(&mut self, at_safepoint: bool) {
        self.safepoint_manager.at_safepoint = at_safepoint;
    }

    /// 获取代码版本信息
    pub fn get_code_version_info(&self, pc: GuestAddr) -> Option<&CodeVersionInfo> {
        self.code_versions.get(&pc)
    }
}