//! Profile-Guided Optimization (PGO) 支持
//!
//! 提供运行时profile数据收集、序列化和基于profile的优化决策

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use vm_core::GuestAddr;

/// Profile数据类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProfileDataType {
    /// 代码块执行频率
    BlockExecutionFrequency,
    /// 分支预测信息
    BranchPrediction,
    /// 内存访问模式
    MemoryAccessPattern,
    /// 函数调用频率
    FunctionCallFrequency,
    /// 循环迭代次数
    LoopIterationCount,
    /// 指令执行频率
    InstructionFrequency,
}

/// 代码块Profile数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockProfile {
    /// 代码块地址
    pub pc: GuestAddr,
    /// 执行次数
    pub execution_count: u64,
    /// 平均执行时间（纳秒）
    pub avg_execution_time_ns: u64,
    /// 总执行时间（纳秒）
    pub total_execution_time_ns: u64,
    /// 最后执行时间
    pub last_execution_time: Option<u64>, // Unix时间戳（秒）
    /// 调用者集合（哪些代码块调用了这个块）
    pub callers: HashSet<GuestAddr>,
    /// 被调用者集合（这个块调用了哪些代码块）
    pub callees: HashSet<GuestAddr>,
}

/// 分支Profile数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchProfile {
    /// 分支指令地址
    pub pc: GuestAddr,
    /// 总分支次数
    pub total_branches: u64,
    /// 跳转次数（taken）
    pub taken_count: u64,
    /// 不跳转次数（not taken）
    pub not_taken_count: u64,
    /// 跳转目标地址 -> 跳转次数
    pub target_counts: HashMap<GuestAddr, u64>,
    /// 跳转概率（taken / total）
    pub taken_probability: f64,
}

/// 内存访问Profile数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryAccessProfile {
    /// 代码块地址
    pub pc: GuestAddr,
    /// 内存访问次数
    pub access_count: u64,
    /// 访问的地址范围（最小地址）
    pub min_address: u64,
    /// 访问的地址范围（最大地址）
    pub max_address: u64,
    /// 访问模式（顺序/随机）
    pub access_pattern: AccessPattern,
    /// 缓存命中率（如果可用）
    pub cache_hit_rate: Option<f64>,
}

/// 内存访问模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccessPattern {
    /// 顺序访问
    Sequential,
    /// 随机访问
    Random,
    /// 混合模式
    Mixed,
}

/// 函数调用Profile数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCallProfile {
    /// 函数入口地址
    pub entry_pc: GuestAddr,
    /// 调用次数
    pub call_count: u64,
    /// 平均执行时间（纳秒）
    pub avg_execution_time_ns: u64,
    /// 调用者集合
    pub callers: HashSet<GuestAddr>,
    /// 递归深度统计
    pub recursion_depth_stats: HashMap<u32, u64>,
}

/// 循环Profile数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopProfile {
    /// 循环入口地址
    pub entry_pc: GuestAddr,
    /// 循环迭代次数
    pub iteration_count: u64,
    /// 平均迭代次数
    pub avg_iterations: f64,
    /// 最大迭代次数
    pub max_iterations: u64,
    /// 最小迭代次数
    pub min_iterations: u64,
    /// 循环体执行时间（纳秒）
    pub body_execution_time_ns: u64,
}

/// 完整的Profile数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileData {
    /// 收集开始时间
    pub start_time: u64, // Unix时间戳（秒）
    /// 收集结束时间
    pub end_time: Option<u64>, // Unix时间戳（秒）
    /// 代码块Profile数据
    pub block_profiles: HashMap<GuestAddr, BlockProfile>,
    /// 分支Profile数据
    pub branch_profiles: HashMap<GuestAddr, BranchProfile>,
    /// 内存访问Profile数据
    pub memory_profiles: HashMap<GuestAddr, MemoryAccessProfile>,
    /// 函数调用Profile数据
    pub function_profiles: HashMap<GuestAddr, FunctionCallProfile>,
    /// 循环Profile数据
    pub loop_profiles: HashMap<GuestAddr, LoopProfile>,
    /// 元数据
    pub metadata: HashMap<String, String>,
}

impl Default for ProfileData {
    fn default() -> Self {
        Self {
            start_time: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            end_time: None,
            block_profiles: HashMap::new(),
            branch_profiles: HashMap::new(),
            memory_profiles: HashMap::new(),
            function_profiles: HashMap::new(),
            loop_profiles: HashMap::new(),
            metadata: HashMap::new(),
        }
    }
}

/// Profile收集器
pub struct ProfileCollector {
    /// Profile数据
    profile_data: Arc<Mutex<ProfileData>>,
    /// 是否启用收集
    enabled: Arc<Mutex<bool>>,
    /// 收集间隔（秒）
    collection_interval: Duration,
    /// 最后收集时间
    last_collection_time: Arc<Mutex<Instant>>,
}

impl ProfileCollector {
    /// 创建新的Profile收集器
    pub fn new(collection_interval: Duration) -> Self {
        Self {
            profile_data: Arc::new(Mutex::new(ProfileData::default())),
            enabled: Arc::new(Mutex::new(true)),
            collection_interval,
            last_collection_time: Arc::new(Mutex::new(Instant::now())),
        }
    }

    /// 启用/禁用收集
    pub fn set_enabled(&self, enabled: bool) {
        *self.enabled.lock().unwrap() = enabled;
    }

    /// 记录代码块执行
    pub fn record_block_execution(&self, pc: GuestAddr, execution_time_ns: u64) {
        if !*self.enabled.lock().unwrap() {
            return;
        }

        let mut profile = self.profile_data.lock().unwrap();
        let block_profile = profile.block_profiles.entry(pc).or_insert_with(|| BlockProfile {
            pc,
            execution_count: 0,
            avg_execution_time_ns: 0,
            total_execution_time_ns: 0,
            last_execution_time: None,
            callers: HashSet::new(),
            callees: HashSet::new(),
        });

        block_profile.execution_count += 1;
        block_profile.total_execution_time_ns += execution_time_ns;
        block_profile.avg_execution_time_ns =
            block_profile.total_execution_time_ns / block_profile.execution_count;
        block_profile.last_execution_time = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        );
    }

    /// 记录代码块调用关系
    pub fn record_block_call(&self, caller_pc: GuestAddr, callee_pc: GuestAddr) {
        if !*self.enabled.lock().unwrap() {
            return;
        }

        let mut profile = self.profile_data.lock().unwrap();

        // 更新调用者
        let caller_profile = profile.block_profiles.entry(caller_pc).or_insert_with(|| BlockProfile {
            pc: caller_pc,
            execution_count: 0,
            avg_execution_time_ns: 0,
            total_execution_time_ns: 0,
            last_execution_time: None,
            callers: HashSet::new(),
            callees: HashSet::new(),
        });
        caller_profile.callees.insert(callee_pc);

        // 更新被调用者
        let callee_profile = profile.block_profiles.entry(callee_pc).or_insert_with(|| BlockProfile {
            pc: callee_pc,
            execution_count: 0,
            avg_execution_time_ns: 0,
            total_execution_time_ns: 0,
            last_execution_time: None,
            callers: HashSet::new(),
            callees: HashSet::new(),
        });
        callee_profile.callers.insert(caller_pc);
    }

    /// 记录分支预测
    pub fn record_branch(&self, pc: GuestAddr, target: GuestAddr, taken: bool) {
        if !*self.enabled.lock().unwrap() {
            return;
        }

        let mut profile = self.profile_data.lock().unwrap();
        let branch_profile = profile.branch_profiles.entry(pc).or_insert_with(|| BranchProfile {
            pc,
            total_branches: 0,
            taken_count: 0,
            not_taken_count: 0,
            target_counts: HashMap::new(),
            taken_probability: 0.0,
        });

        branch_profile.total_branches += 1;
        if taken {
            branch_profile.taken_count += 1;
            *branch_profile.target_counts.entry(target).or_insert(0) += 1;
        } else {
            branch_profile.not_taken_count += 1;
        }

        branch_profile.taken_probability =
            branch_profile.taken_count as f64 / branch_profile.total_branches as f64;
    }

    /// 记录内存访问
    pub fn record_memory_access(&self, pc: GuestAddr, address: u64) {
        if !*self.enabled.lock().unwrap() {
            return;
        }

        let mut profile = self.profile_data.lock().unwrap();
        let mem_profile = profile.memory_profiles.entry(pc).or_insert_with(|| MemoryAccessProfile {
            pc,
            access_count: 0,
            min_address: address,
            max_address: address,
            access_pattern: AccessPattern::Sequential,
            cache_hit_rate: None,
        });

        mem_profile.access_count += 1;
        mem_profile.min_address = mem_profile.min_address.min(address);
        mem_profile.max_address = mem_profile.max_address.max(address);

        // 简单的访问模式检测（基于地址连续性）
        if mem_profile.access_count > 1 {
            let address_range = mem_profile.max_address - mem_profile.min_address;
            let expected_sequential = mem_profile.access_count as u64 * 64; // 假设64字节步长
            if address_range > expected_sequential * 2 {
                mem_profile.access_pattern = AccessPattern::Random;
            } else if address_range > expected_sequential {
                mem_profile.access_pattern = AccessPattern::Mixed;
            }
        }
    }

    /// 记录函数调用
    pub fn record_function_call(&self, entry_pc: GuestAddr, caller_pc: Option<GuestAddr>, execution_time_ns: u64) {
        if !*self.enabled.lock().unwrap() {
            return;
        }

        let mut profile = self.profile_data.lock().unwrap();
        let func_profile = profile.function_profiles.entry(entry_pc).or_insert_with(|| FunctionCallProfile {
            entry_pc,
            call_count: 0,
            avg_execution_time_ns: 0,
            callers: HashSet::new(),
            recursion_depth_stats: HashMap::new(),
        });

        func_profile.call_count += 1;
        func_profile.avg_execution_time_ns =
            (func_profile.avg_execution_time_ns * (func_profile.call_count - 1) as u64 + execution_time_ns)
                / func_profile.call_count as u64;

        if let Some(caller) = caller_pc {
            func_profile.callers.insert(caller);
        }
    }

    /// 记录循环迭代
    pub fn record_loop_iteration(&self, entry_pc: GuestAddr, iteration_count: u64, body_time_ns: u64) {
        if !*self.enabled.lock().unwrap() {
            return;
        }

        let mut profile = self.profile_data.lock().unwrap();
        let loop_profile = profile.loop_profiles.entry(entry_pc).or_insert_with(|| LoopProfile {
            entry_pc,
            iteration_count: 0,
            avg_iterations: 0.0,
            max_iterations: iteration_count,
            min_iterations: iteration_count,
            body_execution_time_ns: 0,
        });

        loop_profile.iteration_count += iteration_count;
        loop_profile.avg_iterations =
            loop_profile.iteration_count as f64 / (loop_profile.iteration_count / iteration_count) as f64;
        loop_profile.max_iterations = loop_profile.max_iterations.max(iteration_count);
        loop_profile.min_iterations = loop_profile.min_iterations.min(iteration_count);
        loop_profile.body_execution_time_ns += body_time_ns;
    }

    /// 获取Profile数据
    pub fn get_profile_data(&self) -> ProfileData {
        self.profile_data.lock().unwrap().clone()
    }

    /// 重置Profile数据
    pub fn reset(&self) {
        let mut profile = self.profile_data.lock().unwrap();
        *profile = ProfileData::default();
    }

    /// 序列化Profile数据到文件
    pub fn serialize_to_file<P: AsRef<std::path::Path>>(&self, path: P) -> Result<(), String> {
        let profile = self.get_profile_data();
        let json = serde_json::to_string_pretty(&profile)
            .map_err(|e| format!("Failed to serialize profile: {}", e))?;
        std::fs::write(path, json)
            .map_err(|e| format!("Failed to write profile file: {}", e))?;
        Ok(())
    }

    /// 从文件反序列化Profile数据
    pub fn deserialize_from_file<P: AsRef<std::path::Path>>(path: P) -> Result<ProfileData, String> {
        let json = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read profile file: {}", e))?;
        let profile: ProfileData = serde_json::from_str(&json)
            .map_err(|e| format!("Failed to deserialize profile: {}", e))?;
        Ok(profile)
    }

    /// 合并Profile数据
    pub fn merge_profile_data(&self, other: &ProfileData) {
        let mut profile = self.profile_data.lock().unwrap();

        // 合并代码块Profile
        for (pc, other_block) in &other.block_profiles {
            let block = profile.block_profiles.entry(*pc).or_insert_with(|| BlockProfile {
                pc: *pc,
                execution_count: 0,
                avg_execution_time_ns: 0,
                total_execution_time_ns: 0,
                last_execution_time: None,
                callers: HashSet::new(),
                callees: HashSet::new(),
            });

            let total_count = block.execution_count + other_block.execution_count;
            if total_count > 0 {
                block.avg_execution_time_ns = (block.total_execution_time_ns + other_block.total_execution_time_ns)
                    / total_count;
            }
            block.execution_count = total_count;
            block.total_execution_time_ns += other_block.total_execution_time_ns;
            block.callers.extend(&other_block.callers);
            block.callees.extend(&other_block.callees);
        }

        // 合并分支Profile
        for (pc, other_branch) in &other.branch_profiles {
            let branch = profile.branch_profiles.entry(*pc).or_insert_with(|| BranchProfile {
                pc: *pc,
                total_branches: 0,
                taken_count: 0,
                not_taken_count: 0,
                target_counts: HashMap::new(),
                taken_probability: 0.0,
            });

            branch.total_branches += other_branch.total_branches;
            branch.taken_count += other_branch.taken_count;
            branch.not_taken_count += other_branch.not_taken_count;
            for (target, count) in &other_branch.target_counts {
                *branch.target_counts.entry(*target).or_insert(0) += count;
            }
            branch.taken_probability =
                branch.taken_count as f64 / branch.total_branches as f64;
        }

        // 合并其他Profile数据（类似方式）
        // ... 简化实现
    }
}

/// 基于Profile的优化建议
#[derive(Debug, Clone)]
pub struct OptimizationSuggestion {
    /// 代码块地址
    pub pc: GuestAddr,
    /// 建议的优化类型
    pub optimization_type: OptimizationType,
    /// 优先级（0-100）
    pub priority: u8,
    /// 预期性能提升（百分比）
    pub expected_improvement: f64,
    /// 理由
    pub reason: String,
}

/// 优化类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizationType {
    /// 内联函数
    InlineFunction,
    /// 循环展开
    UnrollLoop,
    /// 分支预测优化
    OptimizeBranch,
    /// 内存预取
    PrefetchMemory,
    /// 寄存器分配优化
    OptimizeRegisterAllocation,
    /// 指令调度优化
    OptimizeInstructionScheduling,
}

/// Profile分析器
/// 执行路径分析器
/// 
/// 分析代码块调用关系，识别热点路径，用于代码预取
pub struct ExecutionPathAnalyzer {
    /// 路径频率映射 (路径 -> 频率)
    path_frequencies: Arc<Mutex<HashMap<Vec<GuestAddr>, u64>>>,
    /// 代码块到路径的映射
    block_to_paths: Arc<Mutex<HashMap<GuestAddr, Vec<Vec<GuestAddr>>>>>,
}

impl ExecutionPathAnalyzer {
    pub fn new() -> Self {
        Self {
            path_frequencies: Arc::new(Mutex::new(HashMap::new())),
            block_to_paths: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 记录执行路径
    /// 
    /// 记录从caller到callee的调用关系，构建执行路径
    pub fn record_path(&self, caller_pc: GuestAddr, callee_pc: GuestAddr) {
        let mut paths = self.block_to_paths.lock().unwrap();
        let mut frequencies = self.path_frequencies.lock().unwrap();
        
        // 构建路径
        let path = vec![caller_pc, callee_pc];
        
        // 更新路径频率
        *frequencies.entry(path.clone()).or_insert(0) += 1;
        
        // 更新代码块到路径的映射
        paths.entry(caller_pc).or_insert_with(Vec::new).push(path);
    }

    /// 识别热点路径
    /// 
    /// 返回最频繁的执行路径，用于预编译
    pub fn identify_hot_paths(&self, limit: usize) -> Vec<(Vec<GuestAddr>, u64)> {
        let frequencies = self.path_frequencies.lock().unwrap();
        let mut paths: Vec<_> = frequencies.iter()
            .map(|(path, &freq)| (path.clone(), freq))
            .collect();
        
        // 按频率排序
        paths.sort_by(|a, b| b.1.cmp(&a.1));
        
        paths.into_iter().take(limit).collect()
    }

    /// 预测下一个可能执行的代码块
    /// 
    /// 基于当前代码块和执行历史，预测下一个可能执行的代码块
    pub fn predict_next_blocks(&self, current_pc: GuestAddr, limit: usize) -> Vec<GuestAddr> {
        let paths = self.block_to_paths.lock().unwrap();
        let frequencies = self.path_frequencies.lock().unwrap();
        
        let mut candidates: HashMap<GuestAddr, u64> = HashMap::new();
        
        // 查找包含当前PC的路径
        if let Some(paths_for_block) = paths.get(&current_pc) {
            for path in paths_for_block {
                if let Some(freq) = frequencies.get(path) {
                    // 找到路径中current_pc的下一个块
                    if let Some(pos) = path.iter().position(|&pc| pc == current_pc) {
                        if pos + 1 < path.len() {
                            let next_pc = path[pos + 1];
                            *candidates.entry(next_pc).or_insert(0) += freq;
                        }
                    }
                }
            }
        }
        
        // 按频率排序
        let mut sorted: Vec<_> = candidates.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        
        sorted.into_iter().take(limit).map(|(pc, _)| pc).collect()
    }
}

impl Default for ExecutionPathAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ProfileAnalyzer;

impl ProfileAnalyzer {
    /// 分析Profile数据并生成优化建议
    pub fn analyze(&self, profile: &ProfileData) -> Vec<OptimizationSuggestion> {
        let mut suggestions = Vec::new();

        // 分析热点代码块
        for (pc, block_profile) in &profile.block_profiles {
            if block_profile.execution_count > 1000 {
                suggestions.push(OptimizationSuggestion {
                    pc: *pc,
                    optimization_type: OptimizationType::OptimizeRegisterAllocation,
                    priority: 80,
                    expected_improvement: 10.0,
                    reason: format!("Hot block with {} executions", block_profile.execution_count),
                });
            }
        }

        // 分析分支预测
        for (pc, branch_profile) in &profile.branch_profiles {
            if branch_profile.total_branches > 100 {
                // 如果分支几乎总是跳转或几乎总是不跳转，可以优化
                if branch_profile.taken_probability > 0.9 || branch_profile.taken_probability < 0.1 {
                    suggestions.push(OptimizationSuggestion {
                        pc: *pc,
                        optimization_type: OptimizationType::OptimizeBranch,
                        priority: 70,
                        expected_improvement: 5.0,
                        reason: format!(
                            "Highly predictable branch ({}% taken)",
                            branch_profile.taken_probability * 100.0
                        ),
                    });
                }
            }
        }

        // 分析循环
        for (pc, loop_profile) in &profile.loop_profiles {
            if loop_profile.avg_iterations > 10.0 && loop_profile.avg_iterations < 100.0 {
                suggestions.push(OptimizationSuggestion {
                    pc: *pc,
                    optimization_type: OptimizationType::UnrollLoop,
                    priority: 60,
                    expected_improvement: 15.0,
                    reason: format!("Loop with {} average iterations", loop_profile.avg_iterations),
                });
            }
        }

        // 分析函数调用
        for (pc, func_profile) in &profile.function_profiles {
            if func_profile.call_count > 100 && func_profile.callers.len() == 1 {
                // 单调用者函数，适合内联
                suggestions.push(OptimizationSuggestion {
                    pc: *pc,
                    optimization_type: OptimizationType::InlineFunction,
                    priority: 75,
                    expected_improvement: 8.0,
                    reason: format!("Function called {} times from single caller", func_profile.call_count),
                });
            }
        }

        // 分析内存访问模式
        for (pc, mem_profile) in &profile.memory_profiles {
            if mem_profile.access_pattern == AccessPattern::Sequential && mem_profile.access_count > 1000 {
                suggestions.push(OptimizationSuggestion {
                    pc: *pc,
                    optimization_type: OptimizationType::PrefetchMemory,
                    priority: 65,
                    expected_improvement: 12.0,
                    reason: "Sequential memory access pattern detected".to_string(),
                });
            }
        }

        // 按优先级排序
        suggestions.sort_by(|a, b| b.priority.cmp(&a.priority));

        suggestions
    }

    /// 获取热点代码块列表
    pub fn get_hot_blocks(&self, profile: &ProfileData, threshold: u64) -> Vec<GuestAddr> {
        profile
            .block_profiles
            .iter()
            .filter(|(_, block)| block.execution_count >= threshold)
            .map(|(pc, _)| *pc)
            .collect()
    }

    /// 获取冷代码块列表
    pub fn get_cold_blocks(&self, profile: &ProfileData, threshold: u64) -> Vec<GuestAddr> {
        profile
            .block_profiles
            .iter()
            .filter(|(_, block)| block.execution_count < threshold)
            .map(|(pc, _)| *pc)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_collector() {
        let collector = ProfileCollector::new(Duration::from_secs(1));

        // 记录代码块执行
        collector.record_block_execution(0x1000, 1000);
        collector.record_block_execution(0x1000, 2000);

        let profile = collector.get_profile_data();
        assert_eq!(profile.block_profiles.len(), 1);
        assert_eq!(profile.block_profiles[&0x1000].execution_count, 2);
        assert_eq!(profile.block_profiles[&0x1000].avg_execution_time_ns, 1500);
    }

    #[test]
    fn test_branch_profile() {
        let collector = ProfileCollector::new(Duration::from_secs(1));

        // 记录分支
        for _ in 0..90 {
            collector.record_branch(0x2000, 0x3000, true);
        }
        for _ in 0..10 {
            collector.record_branch(0x2000, 0x2000, false);
        }

        let profile = collector.get_profile_data();
        let branch = &profile.branch_profiles[&0x2000];
        assert_eq!(branch.total_branches, 100);
        assert_eq!(branch.taken_count, 90);
        assert!((branch.taken_probability - 0.9).abs() < 0.01);
    }

    #[test]
    fn test_profile_serialization() {
        let collector = ProfileCollector::new(Duration::from_secs(1));
        collector.record_block_execution(0x1000, 1000);

        // 序列化
        let temp_file = std::env::temp_dir().join("test_profile.json");
        collector.serialize_to_file(&temp_file).unwrap();

        // 反序列化
        let loaded = ProfileCollector::deserialize_from_file(&temp_file).unwrap();
        assert_eq!(loaded.block_profiles.len(), 1);
        assert_eq!(loaded.block_profiles[&0x1000].execution_count, 1);

        // 清理
        let _ = std::fs::remove_file(&temp_file);
    }

    #[test]
    fn test_profile_analyzer() {
        let collector = ProfileCollector::new(Duration::from_secs(1));

        // 创建热点代码块
        for _ in 0..2000 {
            collector.record_block_execution(0x1000, 1000);
        }

        let profile = collector.get_profile_data();
        let analyzer = ProfileAnalyzer;
        let suggestions = analyzer.analyze(&profile);

        assert!(!suggestions.is_empty());
        assert!(suggestions.iter().any(|s| s.pc == 0x1000));
    }
}

