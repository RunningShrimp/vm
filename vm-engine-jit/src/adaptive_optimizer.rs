//! 自适应优化器
//!
//! 实现了根据运行时性能动态调整优化策略的自适应优化器。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use vm_core::GuestAddr;
use vm_ir::IRBlock;
use crate::optimizer::IROptimizer;
use crate::simd_optimizer::SIMDOptimizer;
use crate::register_allocator::RegisterAllocator;
use crate::instruction_scheduler::InstructionScheduler;
use crate::codegen::CodeGenerator;
use crate::compiler::CompiledIRBlock;

/// 自适应优化配置
#[derive(Debug, Clone)]
pub struct AdaptiveOptimizationConfig {
    /// 性能采样间隔（毫秒）
    pub sampling_interval_ms: u64,
    /// 性能历史窗口大小
    pub history_window_size: usize,
    /// 优化调整阈值（性能变化百分比）
    pub adjustment_threshold: f64,
    /// 最大优化级别
    pub max_optimization_level: u8,
    /// 最小优化级别
    pub min_optimization_level: u8,
    /// 启用SIMD自适应
    pub enable_simd_adaptation: bool,
    /// 启用调度自适应
    pub enable_scheduling_adaptation: bool,
    /// 启用寄存器分配自适应
    pub enable_register_adaptation: bool,
}

impl Default for AdaptiveOptimizationConfig {
    fn default() -> Self {
        Self {
            sampling_interval_ms: 100,
            history_window_size: 1000,
            adjustment_threshold: 0.1, // 10%
            max_optimization_level: 3,
            min_optimization_level: 0,
            enable_simd_adaptation: true,
            enable_scheduling_adaptation: true,
            enable_register_adaptation: true,
        }
    }
}

/// 性能指标
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    /// 执行时间（纳秒）
    pub execution_time_ns: u64,
    /// 指令执行数量
    pub instruction_count: u64,
    /// 缓存命中次数
    pub cache_hits: u64,
    /// 缓存未命中次数
    pub cache_misses: u64,
    /// 内存访问次数
    pub memory_accesses: u64,
    /// 分支预测失败次数
    pub branch_mispredictions: u64,
    /// 采样时间戳
    pub timestamp: Instant,
}

impl PerformanceMetrics {
    /// 计算每条指令的平均执行时间
    pub fn avg_instruction_time(&self) -> f64 {
        if self.instruction_count == 0 {
            return 0.0;
        }
        self.execution_time_ns as f64 / self.instruction_count as f64
    }
    
    /// 计算缓存命中率
    pub fn cache_hit_rate(&self) -> f64 {
        let total_accesses = self.cache_hits + self.cache_misses;
        if total_accesses == 0 {
            return 0.0;
        }
        self.cache_hits as f64 / total_accesses as f64
    }
    
    /// 计算每千条指令的内存访问次数
    pub fn memory_access_per_kilo_insn(&self) -> f64 {
        if self.instruction_count == 0 {
            return 0.0;
        }
        (self.memory_accesses as f64 * 1000.0) / self.instruction_count as f64
    }
    
    /// 计算分支预测准确率
    pub fn branch_prediction_accuracy(&self) -> f64 {
        let total_branches = self.cache_hits + self.branch_mispredictions;
        if total_branches == 0 {
            return 1.0;
        }
        self.cache_hits as f64 / total_branches as f64
    }
}

/// 优化策略
#[derive(Debug, Clone, PartialEq)]
pub enum OptimizationStrategy {
    /// 最低延迟优化
    MinimizeLatency,
    /// 最大吞吐量优化
    MaximizeThroughput,
    /// 最小内存占用优化
    MinimizeMemoryUsage,
    /// 平衡优化
    Balanced,
    /// 功耗优化
    MinimizePowerConsumption,
}

/// 自适应优化器
pub struct AdaptiveOptimizer {
    /// 配置
    config: AdaptiveOptimizationConfig,
    /// 当前优化策略
    current_strategy: OptimizationStrategy,
    /// 当前优化级别
    current_optimization_level: u8,
    /// 性能历史记录
    performance_history: Arc<Mutex<Vec<PerformanceMetrics>>>,
    /// 策略性能映射
    strategy_performance: Arc<Mutex<HashMap<OptimizationStrategy, Vec<PerformanceMetrics>>>>,
    /// 优化级别性能映射
    level_performance: Arc<Mutex<HashMap<u8, Vec<PerformanceMetrics>>>>,
    /// 最后采样时间
    last_sample_time: Arc<Mutex<Instant>>,
    /// 当前IR块
    current_block: Arc<Mutex<Option<IRBlock>>>,
    /// 优化器组件
    optimizer: Box<dyn IROptimizer>,
    simd_optimizer: Box<dyn SIMDOptimizer>,
    register_allocator: Box<dyn RegisterAllocator>,
    instruction_scheduler: Box<dyn InstructionScheduler>,
    code_generator: Box<dyn CodeGenerator>,
}

impl AdaptiveOptimizer {
    /// 创建新的自适应优化器
    pub fn new(
        config: AdaptiveOptimizationConfig,
        optimizer: Box<dyn IROptimizer>,
        simd_optimizer: Box<dyn SIMDOptimizer>,
        register_allocator: Box<dyn RegisterAllocator>,
        instruction_scheduler: Box<dyn InstructionScheduler>,
        code_generator: Box<dyn CodeGenerator>,
    ) -> Self {
        Self {
            config,
            current_strategy: OptimizationStrategy::Balanced,
            current_optimization_level: 2,
            performance_history: Arc::new(Mutex::new(Vec::new())),
            strategy_performance: Arc::new(Mutex::new(HashMap::new())),
            level_performance: Arc::new(Mutex::new(HashMap::new())),
            last_sample_time: Arc::new(Mutex::new(Instant::now())),
            current_block: Arc::new(Mutex::new(None)),
            optimizer,
            simd_optimizer,
            register_allocator,
            instruction_scheduler,
            code_generator,
        }
    }
    
    /// 设置当前IR块
    pub fn set_current_block(&self, block: IRBlock) {
        *self.current_block.lock().unwrap() = Some(block);
    }
    
    /// 采样性能指标
    pub fn sample_performance(&self, metrics: PerformanceMetrics) {
        let mut history = self.performance_history.lock().unwrap();
        history.push(metrics.clone());
        
        // 保持历史窗口大小
        if history.len() > self.config.history_window_size {
            history.remove(0);
        }
        
        // 更新最后采样时间
        *self.last_sample_time.lock().unwrap() = metrics.timestamp;
        
        // 检查是否需要调整优化策略
        if history.len() >= 10 { // 至少需要10个样本
            self.adjust_optimization_strategy();
        }
    }
    
    /// 调整优化策略
    fn adjust_optimization_strategy(&self) {
        let history = self.performance_history.lock().unwrap();
        if history.len() < 10 {
            return;
        }
        
        // 计算最近性能指标
        let recent_metrics = &history[history.len() - 10..];
        let avg_instruction_time = recent_metrics.iter()
            .map(|m| m.avg_instruction_time())
            .sum::<f64>() / recent_metrics.len() as f64;
        
        let avg_cache_hit_rate = recent_metrics.iter()
            .map(|m| m.cache_hit_rate())
            .sum::<f64>() / recent_metrics.len() as f64;
        
        let avg_memory_access = recent_metrics.iter()
            .map(|m| m.memory_access_per_kilo_insn())
            .sum::<f64>() / recent_metrics.len() as f64;
        
        // 根据性能特征选择最佳策略
        let new_strategy = self.select_optimal_strategy(avg_instruction_time, avg_cache_hit_rate, avg_memory_access);
        
        // 如果策略发生变化，应用新策略
        if new_strategy != self.current_strategy {
            self.apply_optimization_strategy(new_strategy);
            self.current_strategy = new_strategy;
        }
        
        // 调整优化级别
        self.adjust_optimization_level(avg_instruction_time);
    }
    
    /// 选择最优策略
    fn select_optimal_strategy(&self, 
                             avg_instruction_time: f64, 
                             avg_cache_hit_rate: f64, 
                             avg_memory_access: f64) -> OptimizationStrategy {
        // 策略选择逻辑
        if avg_instruction_time > 100.0 && avg_cache_hit_rate < 0.8 {
            // 高延迟且缓存命中率低，优化内存访问
            OptimizationStrategy::MinimizeMemoryUsage
        } else if avg_memory_access > 500.0 {
            // 内存访问频繁，优化内存布局
            OptimizationStrategy::MinimizeMemoryUsage
        } else if avg_instruction_time < 10.0 && avg_cache_hit_rate > 0.95 {
            // 性能已经很好，追求更低功耗
            OptimizationStrategy::MinimizePowerConsumption
        } else if avg_cache_hit_rate > 0.9 {
            // 缓存命中率高，优化吞吐量
            OptimizationStrategy::MaximizeThroughput
        } else {
            // 默认平衡策略
            OptimizationStrategy::Balanced
        }
    }
    
    /// 应用优化策略
    fn apply_optimization_strategy(&self, strategy: OptimizationStrategy) {
        match strategy {
            OptimizationStrategy::MinimizeLatency => {
                // 降低优化级别，减少编译开销
                self.current_optimization_level = self.config.min_optimization_level;
                // 禁用SIMD优化（可能增加延迟）
                // 这里需要与具体优化器组件交互
            }
            OptimizationStrategy::MaximizeThroughput => {
                // 提高优化级别，启用所有优化
                self.current_optimization_level = self.config.max_optimization_level;
                // 启用SIMD和指令调度
            }
            OptimizationStrategy::MinimizeMemoryUsage => {
                // 中等优化级别，专注于内存优化
                self.current_optimization_level = (self.config.min_optimization_level + self.config.max_optimization_level) / 2;
                // 启用内存相关的优化
            }
            OptimizationStrategy::Balanced => {
                // 默认平衡设置
                self.current_optimization_level = 2;
            }
            OptimizationStrategy::MinimizePowerConsumption => {
                // 降低优化级别，减少功耗
                self.current_optimization_level = self.config.min_optimization_level + 1;
            }
        }
    }
    
    /// 调整优化级别
    fn adjust_optimization_level(&self, avg_instruction_time: f64) {
        let mut level_performance = self.level_performance.lock().unwrap();
        
        // 记录当前级别的性能
        let current_metrics = PerformanceMetrics {
            execution_time_ns: (avg_instruction_time * 1000.0) as u64,
            instruction_count: 1000,
            cache_hits: 0,
            cache_misses: 0,
            memory_accesses: 0,
            branch_mispredictions: 0,
            timestamp: Instant::now(),
        };
        
        level_performance.entry(self.current_optimization_level)
            .or_insert_with(Vec::new)
            .push(current_metrics);
        
        // 保持历史记录大小
        for (_, metrics) in level_performance.iter_mut() {
            if metrics.len() > 100 {
                metrics.remove(0);
            }
        }
        
        // 分析各优化级别的性能
        let mut best_level = self.current_optimization_level;
        let mut best_performance = avg_instruction_time;
        
        for (&level, metrics) in level_performance.iter() {
            if metrics.len() >= 5 {
                let level_avg = metrics.iter()
                    .map(|m| m.avg_instruction_time())
                    .sum::<f64>() / metrics.len() as f64;
                
                if level_avg < best_performance * (1.0 - self.config.adjustment_threshold) {
                    best_level = level;
                    best_performance = level_avg;
                }
            }
        }
        
        // 如果找到更好的优化级别，则调整
        if best_level != self.current_optimization_level {
            self.current_optimization_level = best_level;
        }
    }
    
    /// 获取当前优化策略
    pub fn current_strategy(&self) -> OptimizationStrategy {
        self.current_strategy.clone()
    }
    
    /// 获取当前优化级别
    pub fn current_optimization_level(&self) -> u8 {
        self.current_optimization_level
    }
    
    /// 获取性能历史
    pub fn performance_history(&self) -> Vec<PerformanceMetrics> {
        self.performance_history.lock().unwrap().clone()
    }
    
    /// 获取策略性能统计
    pub fn strategy_performance(&self) -> HashMap<OptimizationStrategy, Vec<PerformanceMetrics>> {
        self.strategy_performance.lock().unwrap().clone()
    }
    
    /// 优化IR块
    pub fn optimize(&self, block: &IRBlock) -> Result<IRBlock, vm_core::VmError> {
        // 根据当前优化级别调整优化器参数
        self.configure_optimizer_for_level();
        
        // 执行基础优化
        let mut optimized_block = self.optimizer.optimize(block)?;
        
        // 根据策略执行特定优化
        match self.current_strategy {
            OptimizationStrategy::MinimizeLatency => {
                // 执行延迟优化
                self.optimize_for_latency(&mut optimized_block)?;
            }
            OptimizationStrategy::MaximizeThroughput => {
                // 执行吞吐量优化
                self.optimize_for_throughput(&mut optimized_block)?;
            }
            OptimizationStrategy::MinimizeMemoryUsage => {
                // 执行内存优化
                self.optimize_for_memory(&mut optimized_block)?;
            }
            OptimizationStrategy::Balanced => {
                // 执行平衡优化
                self.optimize_for_balance(&mut optimized_block)?;
            }
            OptimizationStrategy::MinimizePowerConsumption => {
                // 执行功耗优化
                self.optimize_for_power(&mut optimized_block)?;
            }
        }
        
        Ok(optimized_block)
    }
    
    /// 配置优化器参数
    fn configure_optimizer_for_level(&self) {
        // 这里需要与具体优化器实现交互
        // 根据优化级别调整优化器参数
    }
    
    /// 延迟优化
    fn optimize_for_latency(&self, block: &mut IRBlock) -> Result<(), vm_core::VmError> {
        // 实现延迟特定的优化
        // 例如：减少指令依赖、优化关键路径等
        Ok(())
    }
    
    /// 吞吐量优化
    fn optimize_for_throughput(&self, block: &mut IRBlock) -> Result<(), vm_core::VmError> {
        // 实现吞吐量特定的优化
        // 例如：指令级并行、循环展开等
        Ok(())
    }
    
    /// 内存优化
    fn optimize_for_memory(&self, block: &mut IRBlock) -> Result<(), vm_core::VmError> {
        // 实现内存特定的优化
        // 例如：内存访问模式优化、缓存友好布局等
        Ok(())
    }
    
    /// 平衡优化
    fn optimize_for_balance(&self, block: &mut IRBlock) -> Result<(), vm_core::VmError> {
        // 实现平衡优化
        // 在延迟、吞吐量、内存使用之间取得平衡
        Ok(())
    }
    
    /// 功耗优化
    fn optimize_for_power(&self, block: &mut IRBlock) -> Result<(), vm_core::VmError> {
        // 实现功耗特定的优化
        // 例如：减少指令数量、降低频率等
        Ok(())
    }
}