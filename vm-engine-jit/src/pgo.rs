//! JIT 配置文件引导优化 (Profile-Guided Optimization, PGO)
//!
//! 基于运行时性能配置文件数据优化 JIT 编译决策。
//!
//! ## 核心概念
//!
//! PGO 通过收集程序运行时的实际执行数据来优化编译：
//! - 哪些代码路径最热（hot paths）
//! - 哪些分支更可能被采用
//! - 哪些函数应该内联
//! - 寄存器分配优化
//!
//! ## 实现策略
//!
//! 1. **数据收集**: 在运行时收集执行频率、分支预测等数据
//! 2. **数据持久化**: 将配置文件数据保存到磁盘
//! 3. **优化应用**: 使用收集的数据指导编译优化

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use vm_ir::IRBlock;

use crate::ExecutionError;

/// PGO 配置文件数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileData {
    /// 块执行频率统计
    pub block_execution_count: HashMap<usize, u64>,

    /// 分支预测数据
    pub branch_predictions: HashMap<usize, BranchStats>,

    /// 函数调用统计
    pub function_calls: HashMap<String, CallStats>,

    /// 内存访问模式
    pub memory_patterns: HashMap<usize, MemoryPattern>,

    /// 总执行次数
    pub total_executions: u64,

    /// 配置文件生成时间
    pub profile_timestamp: u64,

    /// 块级性能配置文件
    pub block_profiles: HashMap<usize, BlockProfile>,
}

/// 块级性能配置文件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockProfile {
    /// 块ID
    pub block_id: usize,

    /// 执行次数
    pub execution_count: u64,

    /// 平均执行时间（纳秒）
    pub avg_duration_ns: u64,

    /// 指令数量
    pub instruction_count: usize,

    /// 分支数量
    pub branch_count: usize,

    /// 内存访问次数
    pub memory_access_count: usize,

    /// 调用者列表
    pub callers: Vec<usize>,

    /// 被调用者列表
    pub callees: Vec<usize>,
}

impl Default for ProfileData {
    fn default() -> Self {
        Self {
            block_execution_count: HashMap::new(),
            branch_predictions: HashMap::new(),
            function_calls: HashMap::new(),
            memory_patterns: HashMap::new(),
            total_executions: 0,
            profile_timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            block_profiles: HashMap::new(),
        }
    }
}

/// 分支统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchStats {
    /// 总执行次数
    pub total_taken: u64,

    /// 采用（taken）次数
    pub taken_count: u64,

    /// 不采用（not taken）次数
    pub not_taken_count: u64,

    /// 采用率（0.0-1.0）
    pub taken_rate: f64,
}

impl BranchStats {
    pub fn new() -> Self {
        Self {
            total_taken: 0,
            taken_count: 0,
            not_taken_count: 0,
            taken_rate: 0.0,
        }
    }

    /// 更新分支统计
    pub fn update(&mut self, taken: bool) {
        self.total_taken += 1;
        if taken {
            self.taken_count += 1;
        } else {
            self.not_taken_count += 1;
        }
        self.taken_rate = self.taken_count as f64 / self.total_taken as f64;
    }

    /// 预测分支方向
    pub fn predict(&self) -> bool {
        self.taken_rate >= 0.5
    }
}

/// 函数调用统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallStats {
    /// 调用次数
    pub call_count: u64,

    /// 平均执行时间（微秒）
    pub avg_duration_us: u64,

    /// 是否是热函数（调用次数 > 阈值）
    pub is_hot: bool,

    /// 是否适合内联
    pub should_inline: bool,
}

impl CallStats {
    pub fn new() -> Self {
        Self {
            call_count: 0,
            avg_duration_us: 0,
            is_hot: false,
            should_inline: false,
        }
    }

    /// 更新调用统计
    pub fn update(&mut self, duration: Duration) {
        self.call_count += 1;
        let duration_us = duration.as_micros() as u64;

        // 更新平均执行时间（简单移动平均）
        if self.avg_duration_us == 0 {
            self.avg_duration_us = duration_us;
        } else {
            self.avg_duration_us = (self.avg_duration_us * 9 + duration_us) / 10;
        }

        // 热函数阈值：调用次数 > 100
        self.is_hot = self.call_count > 100;

        // 内联决策：调用频繁且执行时间短
        self.should_inline = self.is_hot && self.avg_duration_us < 1000;
    }
}

/// 内存访问模式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryPattern {
    /// 访问类型统计
    pub access_types: HashMap<MemoryAccessType, u64>,

    /// 平均访问大小
    pub avg_access_size: usize,

    /// 是否是顺序访问
    pub is_sequential: bool,

    /// 是否是对齐访问
    pub is_aligned: bool,
}

/// 内存访问类型
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MemoryAccessType {
    Load,
    Store,
    Atomic,
}

/// PGO 优化器
///
/// 收集和应用配置文件数据以优化 JIT 编译。
pub struct PGOGuidedOptimizer {
    /// 配置文件数据
    profile_data: Arc<Mutex<ProfileData>>,

    /// 配置文件文件路径
    profile_path: Option<PathBuf>,

    /// 热点阈值（块执行次数超过此值被认为是热点）
    hot_threshold: u64,

    /// 是否启用实时收集
    enable_collection: bool,

    /// 优化统计
    stats: Arc<Mutex<OptimizationStats>>,
}

/// 优化统计
#[derive(Debug, Default)]
struct OptimizationStats {
    /// 优化的块数量
    optimized_blocks: usize,

    /// 内联的函数数量
    inlined_functions: usize,

    /// 优化的分支数量
    optimized_branches: usize,

    /// 总优化时间
    total_optimization_time: Duration,
}

impl PGOGuidedOptimizer {
    /// 创建新的 PGO 优化器
    pub fn new() -> Self {
        Self {
            profile_data: Arc::new(Mutex::new(ProfileData::default())),
            profile_path: None,
            hot_threshold: 100,
            enable_collection: true,
            stats: Arc::new(Mutex::new(OptimizationStats::default())),
        }
    }

    /// 从文件加载配置文件
    pub fn load_profile<P: AsRef<Path>>(&mut self, path: P) -> Result<(), ExecutionError> {
        let path = path.as_ref();
        log::info!("Loading PGO profile from: {:?}", path);

        let content = std::fs::read_to_string(path).map_err(|e| ExecutionError::JitError {
            message: format!("Failed to read profile file: {}", e),
            function_addr: None,
        })?;

        let profile_data: ProfileData =
            serde_json::from_str(&content).map_err(|e| ExecutionError::JitError {
                message: format!("Failed to parse profile data: {}", e),
                function_addr: None,
            })?;

        *self.profile_data.lock() = profile_data;
        self.profile_path = Some(path.to_path_buf());

        log::info!("PGO profile loaded successfully");
        Ok(())
    }

    /// 保存配置文件到文件
    pub fn save_profile<P: AsRef<Path>>(&self, path: P) -> Result<(), ExecutionError> {
        let path = path.as_ref();
        log::info!("Saving PGO profile to: {:?}", path);

        let profile_data = self.profile_data.lock();

        // 创建父目录
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ExecutionError::JitError {
                message: format!("Failed to create profile directory: {}", e),
                function_addr: None,
            })?;
        }

        let content =
            serde_json::to_string_pretty(&*profile_data).map_err(|e| ExecutionError::JitError {
                message: format!("Failed to serialize profile data: {}", e),
                function_addr: None,
            })?;

        std::fs::write(path, content).map_err(|e| ExecutionError::JitError {
            message: format!("Failed to write profile file: {}", e),
            function_addr: None,
        })?;

        log::info!("PGO profile saved successfully");
        Ok(())
    }

    /// 记录块执行
    pub fn record_block_execution(&self, block_id: usize) {
        if !self.enable_collection {
            return;
        }

        let mut profile = self.profile_data.lock();
        *profile.block_execution_count.entry(block_id).or_insert(0) += 1;
        profile.total_executions += 1;
    }

    /// 记录分支执行
    pub fn record_branch(&self, block_id: usize, taken: bool) {
        if !self.enable_collection {
            return;
        }

        let mut profile = self.profile_data.lock();
        let stats = profile
            .branch_predictions
            .entry(block_id)
            .or_insert_with(BranchStats::new);
        stats.update(taken);
    }

    /// 记录函数调用
    pub fn record_function_call(&self, function_name: &str, duration: Duration) {
        if !self.enable_collection {
            return;
        }

        let mut profile = self.profile_data.lock();
        let stats = profile
            .function_calls
            .entry(function_name.to_string())
            .or_insert_with(CallStats::new);
        stats.update(duration);
    }

    /// 检查块是否是热点
    pub fn is_hot_block(&self, block_id: usize) -> bool {
        let profile = self.profile_data.lock();
        profile
            .block_execution_count
            .get(&block_id)
            .copied()
            .unwrap_or(0)
            > self.hot_threshold
    }

    /// 获取块的执行频率
    pub fn get_block_frequency(&self, block_id: usize) -> u64 {
        let profile = self.profile_data.lock();
        profile
            .block_execution_count
            .get(&block_id)
            .copied()
            .unwrap_or(0)
    }

    /// 预测分支方向
    pub fn predict_branch(&self, block_id: usize) -> Option<bool> {
        let profile = self.profile_data.lock();
        profile
            .branch_predictions
            .get(&block_id)
            .map(|stats| stats.predict())
    }

    /// 检查函数是否应该内联
    pub fn should_inline_function(&self, function_name: &str) -> bool {
        let profile = self.profile_data.lock();
        profile
            .function_calls
            .get(function_name)
            .map(|stats| stats.should_inline)
            .unwrap_or(false)
    }

    /// 基于配置文件优化 IR 块
    pub fn optimize_with_pgo(&self, block: &IRBlock) -> Result<PGOOptimizedBlock, ExecutionError> {
        let start = Instant::now();

        log::debug!("Optimizing block with PGO: {:?}", block.start_pc);

        let block_id = block.start_pc.0 as usize;
        let is_hot = self.is_hot_block(block_id);
        let frequency = self.get_block_frequency(block_id);

        // 应用 PGO 优化
        let mut optimizations = vec![
            PGOOptimization::HotPath(is_hot),
            PGOOptimization::FrequencyHint(frequency),
        ];

        // 如果块有分支，添加分支预测提示
        if let Some(branch_prediction) = self.predict_branch(block_id) {
            optimizations.push(PGOOptimization::BranchPrediction(branch_prediction));
        }

        let elapsed = start.elapsed();

        // 更新统计
        let mut stats = self.stats.lock();
        stats.optimized_blocks += 1;
        stats.total_optimization_time += elapsed;

        log::debug!("PGO optimization completed in {:?}", elapsed);

        Ok(PGOOptimizedBlock {
            block_id,
            optimizations,
            is_hot,
            frequency,
        })
    }

    /// 获取优化统计
    pub fn get_stats(&self) -> OptimizationStatistics {
        let profile = self.profile_data.lock();
        let stats = self.stats.lock();

        OptimizationStatistics {
            total_blocks_tracked: profile.block_execution_count.len(),
            hot_blocks_count: profile
                .block_execution_count
                .values()
                .filter(|&&count| count > self.hot_threshold)
                .count(),
            total_branches_tracked: profile.branch_predictions.len(),
            optimized_blocks: stats.optimized_blocks,
            inlined_functions: stats.inlined_functions,
            optimized_branches: stats.optimized_branches,
            total_optimization_time: stats.total_optimization_time,
        }
    }

    /// 重置配置文件数据
    pub fn reset_profile(&self) {
        let mut profile = self.profile_data.lock();
        *profile = ProfileData::default();
        log::info!("PGO profile data reset");
    }

    /// 合并另一个配置文件
    pub fn merge_profile(&self, other: ProfileData) {
        let mut profile = self.profile_data.lock();

        // 合并块执行计数
        for (block_id, count) in other.block_execution_count {
            *profile.block_execution_count.entry(block_id).or_insert(0) += count;
        }

        // 合并分支预测
        for (block_id, other_stats) in other.branch_predictions {
            let stats = profile
                .branch_predictions
                .entry(block_id)
                .or_insert_with(BranchStats::new);
            stats.taken_count += other_stats.taken_count;
            stats.not_taken_count += other_stats.not_taken_count;
            stats.total_taken += other_stats.total_taken;
            stats.taken_rate = stats.taken_count as f64 / stats.total_taken as f64;
        }

        // 合并函数调用
        for (func_name, other_stats) in other.function_calls {
            let stats = profile
                .function_calls
                .entry(func_name)
                .or_insert_with(CallStats::new);
            stats.call_count += other_stats.call_count;
            // 重新计算平均时间
            if stats.avg_duration_us == 0 {
                stats.avg_duration_us = other_stats.avg_duration_us;
            } else {
                stats.avg_duration_us = (stats.avg_duration_us + other_stats.avg_duration_us) / 2;
            }
        }

        profile.total_executions += other.total_executions;

        log::info!("PGO profile merged successfully");
    }
}

impl Default for PGOGuidedOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

/// PGO 优化后的块
#[derive(Debug, Clone)]
pub struct PGOOptimizedBlock {
    pub block_id: usize,
    pub optimizations: Vec<PGOOptimization>,
    pub is_hot: bool,
    pub frequency: u64,
}

/// PGO 优化类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PGOOptimization {
    /// 热路径优化
    HotPath(bool),

    /// 执行频率提示
    FrequencyHint(u64),

    /// 分支预测提示
    BranchPrediction(bool),

    /// 函数内联
    Inline,

    /// 循环展开
    LoopUnroll(usize),
}

/// 优化统计信息
#[derive(Debug, Clone)]
pub struct OptimizationStatistics {
    pub total_blocks_tracked: usize,
    pub hot_blocks_count: usize,
    pub total_branches_tracked: usize,
    pub optimized_blocks: usize,
    pub inlined_functions: usize,
    pub optimized_branches: usize,
    pub total_optimization_time: Duration,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_pgo_optimizer_creation() {
        let optimizer = PGOGuidedOptimizer::new();
        assert!(optimizer.is_hot_block(0) == false);
        assert_eq!(optimizer.get_block_frequency(0), 0);
    }

    #[test]
    fn test_block_execution_recording() {
        let optimizer = PGOGuidedOptimizer::new();

        // 记录执行
        for _ in 0..150 {
            optimizer.record_block_execution(1);
        }

        assert_eq!(optimizer.get_block_frequency(1), 150);
        assert!(optimizer.is_hot_block(1)); // 超过阈值100
        assert!(!optimizer.is_hot_block(2)); // 未执行
    }

    #[test]
    fn test_branch_prediction() {
        let optimizer = PGOGuidedOptimizer::new();

        // 记录分支：80% 采用
        for _ in 0..80 {
            optimizer.record_branch(1, true);
        }
        for _ in 0..20 {
            optimizer.record_branch(1, false);
        }

        // 预测应该倾向于 true
        let prediction = optimizer.predict_branch(1);
        assert_eq!(prediction, Some(true));
    }

    #[test]
    fn test_function_call_recording() {
        let optimizer = PGOGuidedOptimizer::new();

        // 记录多次快速调用
        for _ in 0..150 {
            optimizer.record_function_call("hot_function", Duration::from_micros(100));
        }

        // 应该是热函数且适合内联
        let profile = optimizer.profile_data.lock();
        let stats = profile.function_calls.get("hot_function").unwrap();
        assert!(stats.is_hot);
        assert!(stats.should_inline);
    }

    #[test]
    fn test_profile_save_load() {
        let optimizer1 = PGOGuidedOptimizer::new();

        // 记录一些数据
        for _ in 0..150 {
            optimizer1.record_block_execution(1);
            optimizer1.record_branch(1, true);
        }
        optimizer1.record_function_call("test_func", Duration::from_micros(50));

        // 保存到临时文件
        let temp_dir = std::env::temp_dir();
        let profile_path = temp_dir.join("test_profile.json");

        let result = optimizer1.save_profile(&profile_path);
        assert!(result.is_ok());

        // 创建新优化器并加载
        let mut optimizer2 = PGOGuidedOptimizer::new();
        let result = optimizer2.load_profile(&profile_path);
        assert!(result.is_ok());

        // 验证数据加载正确
        assert_eq!(optimizer2.get_block_frequency(1), 150);
        assert_eq!(optimizer2.predict_branch(1), Some(true));

        // 清理
        let _ = std::fs::remove_file(&profile_path);
    }
}

/// Profile collector for runtime profiling data
pub struct ProfileCollector {
    collection_interval: Duration,
    profile_data: Arc<Mutex<ProfileData>>,
    start_time: Instant,
}

impl ProfileCollector {
    pub fn new(collection_interval: Duration) -> Self {
        Self {
            collection_interval,
            profile_data: Arc::new(Mutex::new(ProfileData::default())),
            start_time: Instant::now(),
        }
    }

    /// Record a block call relationship (caller -> callee)
    pub fn record_block_call(&self, caller: vm_core::GuestAddr, callee: vm_core::GuestAddr) {
        let mut profile = self.profile_data.lock();
        let caller_id = caller.0 as usize;
        let callee_id = callee.0 as usize;

        // Update caller's callees list
        if let Some(caller_profile) = profile.block_profiles.get_mut(&caller_id) {
            if !caller_profile.callees.contains(&callee_id) {
                caller_profile.callees.push(callee_id);
            }
        }

        // Update callee's callers list
        if let Some(callee_profile) = profile.block_profiles.get_mut(&callee_id) {
            if !callee_profile.callers.contains(&caller_id) {
                callee_profile.callers.push(caller_id);
            }
        }
    }

    /// Record branch execution with target and direction
    pub fn record_branch(&self, pc: vm_core::GuestAddr, _target: vm_core::GuestAddr, taken: bool) {
        let mut profile = self.profile_data.lock();
        let block_id = pc.0 as usize;

        // Update branch statistics
        let stats = profile
            .branch_predictions
            .entry(block_id)
            .or_insert_with(BranchStats::new);
        stats.update(taken);

        // Update block profile branch count
        if let Some(block_profile) = profile.block_profiles.get_mut(&block_id) {
            block_profile.branch_count += 1;
        }
    }

    /// Record block execution with timing information
    pub fn record_block_execution(&self, pc: vm_core::GuestAddr, duration_ns: u64) {
        let mut profile = self.profile_data.lock();
        let block_id = pc.0 as usize;

        // Update execution count
        *profile.block_execution_count.entry(block_id).or_insert(0) += 1;
        profile.total_executions += 1;

        // Update block profile
        let block_profile =
            profile
                .block_profiles
                .entry(block_id)
                .or_insert_with(|| BlockProfile {
                    block_id,
                    execution_count: 0,
                    avg_duration_ns: 0,
                    instruction_count: 0,
                    branch_count: 0,
                    memory_access_count: 0,
                    callers: Vec::new(),
                    callees: Vec::new(),
                });

        block_profile.execution_count += 1;
        if block_profile.avg_duration_ns == 0 {
            block_profile.avg_duration_ns = duration_ns;
        } else {
            block_profile.avg_duration_ns = (block_profile.avg_duration_ns * 9 + duration_ns) / 10;
        }
    }

    /// Record function call with caller information and execution time
    pub fn record_function_call(
        &self,
        target: vm_core::GuestAddr,
        caller: Option<vm_core::GuestAddr>,
        execution_time_ns: u64,
    ) {
        let mut profile = self.profile_data.lock();
        let target_id = target.0 as usize;
        let function_name = format!("func_{:#x}", target.0);

        // Update function call statistics
        let stats = profile
            .function_calls
            .entry(function_name.clone())
            .or_insert_with(CallStats::new);
        stats.call_count += 1;

        // Update average execution time
        let duration_us = execution_time_ns / 1000; // Convert to microseconds
        if stats.avg_duration_us == 0 {
            stats.avg_duration_us = duration_us;
        } else {
            stats.avg_duration_us = (stats.avg_duration_us * 9 + duration_us) / 10;
        }

        // Update hot function status
        stats.is_hot = stats.call_count > 100;
        stats.should_inline = stats.is_hot && stats.avg_duration_us < 1000;

        // Update caller-callee relationship if provided
        if let Some(caller_addr) = caller {
            let caller_id = caller_addr.0 as usize;

            // Update caller's callees list
            if let Some(caller_profile) = profile.block_profiles.get_mut(&caller_id) {
                if !caller_profile.callees.contains(&target_id) {
                    caller_profile.callees.push(target_id);
                }
            }

            // Update target's callers list
            if let Some(target_profile) = profile.block_profiles.get_mut(&target_id) {
                if !target_profile.callers.contains(&caller_id) {
                    target_profile.callers.push(caller_id);
                }
            }
        }
    }

    pub fn collect_block_profile(&self, block_id: usize, duration: Duration) {
        let mut profile = self.profile_data.lock();
        let block_profile =
            profile
                .block_profiles
                .entry(block_id)
                .or_insert_with(|| BlockProfile {
                    block_id,
                    execution_count: 0,
                    avg_duration_ns: 0,
                    instruction_count: 0,
                    branch_count: 0,
                    memory_access_count: 0,
                    callers: Vec::new(),
                    callees: Vec::new(),
                });

        block_profile.execution_count += 1;
        let duration_ns = duration.as_nanos() as u64;
        if block_profile.avg_duration_ns == 0 {
            block_profile.avg_duration_ns = duration_ns;
        } else {
            block_profile.avg_duration_ns = (block_profile.avg_duration_ns * 9 + duration_ns) / 10;
        }
    }

    pub fn get_profile_data(&self) -> ProfileData {
        self.profile_data.lock().clone()
    }

    pub fn serialize_to_file(&self, path: &std::path::Path) -> Result<(), String> {
        let profile = self.get_profile_data();
        let json = serde_json::to_string_pretty(&profile)
            .map_err(|e| format!("Failed to serialize profile: {}", e))?;
        std::fs::write(path, json).map_err(|e| format!("Failed to write profile to file: {}", e))
    }
}

#[cfg(test)]
mod tests_optimization_stats {
    use super::*;
    #[test]
    fn test_optimization_stats() {
        let optimizer = PGOGuidedOptimizer::new();

        // 记录一些数据
        for i in 1..=5 {
            for _ in 0..(100 + i * 50) {
                optimizer.record_block_execution(i);
            }
        }

        let stats = optimizer.get_stats();
        assert_eq!(stats.total_blocks_tracked, 5);

        // 块1执行150次，块5执行350次，都超过阈值100
        assert_eq!(stats.hot_blocks_count, 5);
    }

    #[test]
    fn test_profile_merge() {
        let optimizer1 = PGOGuidedOptimizer::new();
        let optimizer2 = PGOGuidedOptimizer::new();

        // optimizer1 记录块1
        for _ in 0..100 {
            optimizer1.record_block_execution(1);
        }

        // optimizer2 记录块1和块2
        for _ in 0..50 {
            optimizer2.record_block_execution(1);
        }
        for _ in 0..100 {
            optimizer2.record_block_execution(2);
        }

        // 创建要合并的配置文件
        let profile2 = optimizer2.profile_data.lock().clone();

        // 合并到 optimizer1
        optimizer1.merge_profile(profile2);

        // 验证合并结果
        assert_eq!(optimizer1.get_block_frequency(1), 150); // 100 + 50
        assert_eq!(optimizer1.get_block_frequency(2), 100); // 仅来自 optimizer2
    }

    #[test]
    fn test_profile_reset() {
        let optimizer = PGOGuidedOptimizer::new();

        // 记录一些数据
        for _ in 0..100 {
            optimizer.record_block_execution(1);
        }

        assert_eq!(optimizer.get_block_frequency(1), 100);

        // 重置
        optimizer.reset_profile();

        // 验证数据已清除
        assert_eq!(optimizer.get_block_frequency(1), 0);
    }
}
