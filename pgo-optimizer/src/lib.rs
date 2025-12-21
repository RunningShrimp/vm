//! PGO优化库 - Profile驱动优化
//!
//! 本库实现完整的Profile驱动优化流程：
//! - 运行时Profile收集
//! - 热路径检测
//! - AOT编译驱动
//! - 性能优化反馈

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

/// 块执行Profile信息
#[derive(Clone, Debug, Default)]
pub struct BlockProfile {
    /// 块ID
    pub block_id: u64,
    /// 执行次数
    pub execution_count: u64,
    /// 总执行时间 (微秒)
    pub total_time_us: u64,
    /// 分支预测命中数
    pub branch_hits: u64,
    /// 分支预测缺失数
    pub branch_misses: u64,
    /// 缓存命中数
    pub cache_hits: u64,
    /// 缓存缺失数
    pub cache_misses: u64,
}

impl BlockProfile {
    /// 计算平均执行时间 (微秒)
    pub fn avg_time_us(&self) -> f64 {
        if self.execution_count == 0 {
            0.0
        } else {
            self.total_time_us as f64 / self.execution_count as f64
        }
    }

    /// 计算分支预测准确率 (0-100)
    pub fn branch_accuracy(&self) -> f64 {
        let total = self.branch_hits + self.branch_misses;
        if total == 0 {
            0.0
        } else {
            (self.branch_hits as f64 / total as f64) * 100.0
        }
    }

    /// 计算缓存命中率 (0-100)
    pub fn cache_hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            0.0
        } else {
            (self.cache_hits as f64 / total as f64) * 100.0
        }
    }

    /// 是否为热块 (执行次数>500或执行时间>50ms)
    pub fn is_hot(&self) -> bool {
        self.execution_count > 500 || self.total_time_us > 50000
    }

    /// 是否为冷块
    pub fn is_cold(&self) -> bool {
        self.execution_count < 10
    }
}

/// 函数调用Profile信息
#[derive(Clone, Debug, Default)]
pub struct CallProfile {
    /// 调用者块ID
    pub caller_id: u64,
    /// 被调用者块ID
    pub callee_id: u64,
    /// 调用次数
    pub call_count: u64,
    /// 总耗时 (微秒)
    pub total_time_us: u64,
}

impl CallProfile {
    /// 计算平均调用时间
    pub fn avg_time_us(&self) -> f64 {
        if self.call_count == 0 {
            0.0
        } else {
            self.total_time_us as f64 / self.call_count as f64
        }
    }

    /// 是否为热调用路径
    pub fn is_hot_path(&self) -> bool {
        self.call_count > 50 || self.total_time_us > 5000
    }
}

/// 运行时Profile收集器
pub struct ProfileCollector {
    // 块Profile数据
    block_profiles: Arc<RwLock<HashMap<u64, BlockProfile>>>,
    // 调用Profile数据
    call_profiles: Arc<RwLock<HashMap<(u64, u64), CallProfile>>>,
    // 采样统计
    samples_collected: Arc<AtomicU64>,
    // 总执行时间
    total_exec_time_us: Arc<AtomicU64>,
}

impl ProfileCollector {
    pub fn new() -> Self {
        Self {
            block_profiles: Arc::new(RwLock::new(HashMap::new())),
            call_profiles: Arc::new(RwLock::new(HashMap::new())),
            samples_collected: Arc::new(AtomicU64::new(0)),
            total_exec_time_us: Arc::new(AtomicU64::new(0)),
        }
    }

    /// 记录块执行
    pub fn record_block_execution(&self, block_id: u64, time_us: u64) {
        let mut profiles = self.block_profiles.write();
        let profile = profiles.entry(block_id).or_insert_with(|| BlockProfile {
            block_id,
            ..Default::default()
        });

        profile.execution_count += 1;
        profile.total_time_us += time_us;
        self.samples_collected.fetch_add(1, Ordering::Relaxed);
        self.total_exec_time_us
            .fetch_add(time_us, Ordering::Relaxed);
    }

    /// 记录分支预测结果
    pub fn record_branch(&self, block_id: u64, hit: bool) {
        let mut profiles = self.block_profiles.write();
        let profile = profiles.entry(block_id).or_insert_with(|| BlockProfile {
            block_id,
            ..Default::default()
        });

        if hit {
            profile.branch_hits += 1;
        } else {
            profile.branch_misses += 1;
        }
    }

    /// 记录缓存访问结果
    pub fn record_cache_access(&self, block_id: u64, hit: bool) {
        let mut profiles = self.block_profiles.write();
        let profile = profiles.entry(block_id).or_insert_with(|| BlockProfile {
            block_id,
            ..Default::default()
        });

        if hit {
            profile.cache_hits += 1;
        } else {
            profile.cache_misses += 1;
        }
    }

    /// 记录函数调用
    pub fn record_call(&self, caller_id: u64, callee_id: u64, time_us: u64) {
        let mut call_profiles = self.call_profiles.write();
        let call = call_profiles
            .entry((caller_id, callee_id))
            .or_insert_with(|| CallProfile {
                caller_id,
                callee_id,
                ..Default::default()
            });

        call.call_count += 1;
        call.total_time_us += time_us;
    }

    /// 获取块Profile
    pub fn get_block_profile(&self, block_id: u64) -> Option<BlockProfile> {
        self.block_profiles.read().get(&block_id).cloned()
    }

    /// 获取所有块Profile
    pub fn get_all_block_profiles(&self) -> Vec<BlockProfile> {
        self.block_profiles.read().values().cloned().collect()
    }

    /// 获取热块列表 (按执行时间排序)
    pub fn get_hot_blocks(&self, limit: usize) -> Vec<BlockProfile> {
        let profiles = self.block_profiles.read();
        let mut hot: Vec<_> = profiles.values().filter(|p| p.is_hot()).cloned().collect();
        hot.sort_by(|a, b| b.total_time_us.cmp(&a.total_time_us));
        hot.truncate(limit);
        hot
    }

    /// 获取冷块列表
    pub fn get_cold_blocks(&self, limit: usize) -> Vec<BlockProfile> {
        let profiles = self.block_profiles.read();
        let mut cold: Vec<_> = profiles.values().filter(|p| p.is_cold()).cloned().collect();
        cold.sort_by(|a, b| a.execution_count.cmp(&b.execution_count));
        cold.truncate(limit);
        cold
    }

    /// 获取热调用路径
    pub fn get_hot_call_paths(&self, limit: usize) -> Vec<CallProfile> {
        let calls = self.call_profiles.read();
        let mut hot: Vec<_> = calls
            .values()
            .filter(|c| c.is_hot_path())
            .cloned()
            .collect();
        hot.sort_by(|a, b| b.total_time_us.cmp(&a.total_time_us));
        hot.truncate(limit);
        hot
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> ProfileStats {
        let profiles = self.block_profiles.read();
        let calls = self.call_profiles.read();

        ProfileStats {
            total_blocks_profiled: profiles.len() as u64,
            total_calls_profiled: calls.len() as u64,
            samples_collected: self.samples_collected.load(Ordering::Relaxed),
            total_exec_time_us: self.total_exec_time_us.load(Ordering::Relaxed),
        }
    }

    /// 清空Profile数据
    pub fn clear(&self) {
        self.block_profiles.write().clear();
        self.call_profiles.write().clear();
        self.samples_collected.store(0, Ordering::Relaxed);
        self.total_exec_time_us.store(0, Ordering::Relaxed);
    }
}

impl Default for ProfileCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Profile统计
#[derive(Clone, Debug, Default)]
pub struct ProfileStats {
    pub total_blocks_profiled: u64,
    pub total_calls_profiled: u64,
    pub samples_collected: u64,
    pub total_exec_time_us: u64,
}

/// AOT编译优化信息
#[derive(Clone, Debug)]
pub struct AotOptimizationHint {
    /// 目标块ID
    pub block_id: u64,
    /// 优化级别 (0-3)
    pub optimization_level: u8,
    /// 函数内联候选
    pub inline_candidates: Vec<u64>,
    /// 是否应该展开循环
    pub should_unroll_loops: bool,
    /// 是否应该进行常量传播
    pub should_const_fold: bool,
}

/// AOT优化驱动
pub struct AotOptimizationDriver {
    // Profile收集器
    collector: Arc<ProfileCollector>,
    // 优化建议缓存
    optimization_hints: Arc<RwLock<HashMap<u64, AotOptimizationHint>>>,
}

impl AotOptimizationDriver {
    pub fn new(collector: Arc<ProfileCollector>) -> Self {
        Self {
            collector,
            optimization_hints: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 基于Profile生成优化建议
    pub fn generate_optimization_hints(&self) {
        let mut hints = HashMap::new();

        // 获取所有Block Profile
        let all_profiles = self.collector.get_all_block_profiles();

        for profile in all_profiles {
            let mut hint = AotOptimizationHint {
                block_id: profile.block_id,
                optimization_level: 0,
                inline_candidates: Vec::new(),
                should_unroll_loops: false,
                should_const_fold: false,
            };

            // 根据热度设置优化级别
            if profile.is_hot() {
                hint.optimization_level = 3; // 最高优化
                hint.should_unroll_loops = true;
                hint.should_const_fold = true;
            } else if profile.execution_count > 100 {
                hint.optimization_level = 2;
                hint.should_const_fold = true;
            } else if profile.execution_count > 10 {
                hint.optimization_level = 1;
            }

            // 如果分支预测准确率高，可以进行更激进的优化
            if profile.branch_accuracy() > 95.0 {
                hint.optimization_level = hint.optimization_level.min(3);
            }

            hints.insert(profile.block_id, hint);
        }

        *self.optimization_hints.write() = hints;
    }

    /// 获取块的优化建议
    pub fn get_optimization_hint(&self, block_id: u64) -> Option<AotOptimizationHint> {
        self.optimization_hints.read().get(&block_id).cloned()
    }

    /// 获取所有优化建议
    pub fn get_all_hints(&self) -> Vec<AotOptimizationHint> {
        self.optimization_hints.read().values().cloned().collect()
    }

    /// 估计优化收益 (百分比)
    pub fn estimate_improvement(&self) -> f64 {
        let hot_blocks = self.collector.get_hot_blocks(10);
        if hot_blocks.is_empty() {
            return 0.0;
        }

        let total_hot_time: u64 = hot_blocks.iter().map(|b| b.total_time_us).sum();
        let total_time = self.collector.get_stats().total_exec_time_us;

        if total_time == 0 {
            return 0.0;
        }

        let hot_percent = (total_hot_time as f64 / total_time as f64) * 100.0;
        // 假设热块可以优化20-30%
        (hot_percent / 100.0) * 25.0
    }

    /// 清空缓存
    pub fn clear(&self) {
        self.optimization_hints.write().clear();
    }
}

/// PGO优化管理器 (完整流程)
pub struct PgoManager {
    // Profile收集器
    pub collector: Arc<ProfileCollector>,
    // AOT优化驱动
    pub aot_driver: AotOptimizationDriver,
    // 优化迭代次数
    iteration_count: Arc<AtomicUsize>,
}

impl PgoManager {
    pub fn new() -> Self {
        let collector = Arc::new(ProfileCollector::new());
        let aot_driver = AotOptimizationDriver::new(collector.clone());

        Self {
            collector,
            aot_driver,
            iteration_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// 执行一次PGO优化迭代
    pub fn optimize_iteration(&self) -> PgoIterationResult {
        // 生成优化建议
        self.aot_driver.generate_optimization_hints();

        // 获取统计信息
        let stats = self.collector.get_stats();
        let improvement = self.aot_driver.estimate_improvement();
        let hot_blocks = self.collector.get_hot_blocks(5);

        self.iteration_count.fetch_add(1, Ordering::Relaxed);

        PgoIterationResult {
            iteration: self.iteration_count.load(Ordering::Relaxed) as u32,
            stats,
            estimated_improvement: improvement,
            hot_blocks,
        }
    }

    /// 获取优化统计
    pub fn get_optimization_stats(&self) -> PgoOptimizationStats {
        let all_hints = self.aot_driver.get_all_hints();
        let opt_level_0 = all_hints
            .iter()
            .filter(|h| h.optimization_level == 0)
            .count();
        let opt_level_1 = all_hints
            .iter()
            .filter(|h| h.optimization_level == 1)
            .count();
        let opt_level_2 = all_hints
            .iter()
            .filter(|h| h.optimization_level == 2)
            .count();
        let opt_level_3 = all_hints
            .iter()
            .filter(|h| h.optimization_level == 3)
            .count();

        PgoOptimizationStats {
            total_hints: all_hints.len() as u64,
            opt_level_0,
            opt_level_1,
            opt_level_2,
            opt_level_3,
        }
    }

    /// 重置状态
    pub fn reset(&self) {
        self.collector.clear();
        self.aot_driver.clear();
        self.iteration_count.store(0, Ordering::Relaxed);
    }
}

impl Default for PgoManager {
    fn default() -> Self {
        Self::new()
    }
}

/// PGO迭代结果
#[derive(Clone, Debug)]
pub struct PgoIterationResult {
    pub iteration: u32,
    pub stats: ProfileStats,
    pub estimated_improvement: f64,
    pub hot_blocks: Vec<BlockProfile>,
}

/// PGO优化统计
#[derive(Clone, Debug, Default)]
pub struct PgoOptimizationStats {
    pub total_hints: u64,
    pub opt_level_0: usize,
    pub opt_level_1: usize,
    pub opt_level_2: usize,
    pub opt_level_3: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_profile_basic() {
        let profile = BlockProfile {
            block_id: 1,
            execution_count: 1000,
            total_time_us: 10000,
            ..Default::default()
        };

        assert_eq!(profile.avg_time_us(), 10.0);
        assert!(profile.is_hot());
        assert!(!profile.is_cold());
    }

    #[test]
    fn test_block_profile_metrics() {
        let profile = BlockProfile {
            block_id: 1,
            execution_count: 100,
            branch_hits: 95,
            branch_misses: 5,
            cache_hits: 100,
            cache_misses: 10,
            ..Default::default()
        };

        assert!(profile.branch_accuracy() > 90.0);
        assert!(profile.cache_hit_rate() > 90.0);
    }

    #[test]
    fn test_profile_collector_block_recording() {
        let collector = ProfileCollector::new();

        collector.record_block_execution(1, 100);
        collector.record_block_execution(1, 200);
        collector.record_block_execution(2, 150);

        let profile1 = collector.get_block_profile(1).unwrap();
        assert_eq!(profile1.execution_count, 2);
        assert_eq!(profile1.total_time_us, 300);

        let stats = collector.get_stats();
        assert_eq!(stats.total_blocks_profiled, 2);
        assert_eq!(stats.samples_collected, 3);
    }

    #[test]
    fn test_profile_collector_branch_recording() {
        let collector = ProfileCollector::new();

        collector.record_block_execution(1, 100);
        collector.record_branch(1, true);
        collector.record_branch(1, true);
        collector.record_branch(1, false);

        let profile = collector.get_block_profile(1).unwrap();
        assert_eq!(profile.branch_hits, 2);
        assert_eq!(profile.branch_misses, 1);
    }

    #[test]
    fn test_profile_collector_cache_recording() {
        let collector = ProfileCollector::new();

        collector.record_block_execution(1, 100);
        for _ in 0..8 {
            collector.record_cache_access(1, true);
        }
        for _ in 0..2 {
            collector.record_cache_access(1, false);
        }

        let profile = collector.get_block_profile(1).unwrap();
        assert_eq!(profile.cache_hits, 8);
        assert_eq!(profile.cache_misses, 2);
        assert_eq!(profile.cache_hit_rate(), 80.0);
    }

    #[test]
    fn test_profile_collector_hot_blocks() {
        let collector = ProfileCollector::new();

        // 创建热块
        for _ in 0..2000 {
            collector.record_block_execution(1, 10);
        }

        // 创建冷块
        for _ in 0..5 {
            collector.record_block_execution(2, 100);
        }

        let hot_blocks = collector.get_hot_blocks(10);
        assert!(!hot_blocks.is_empty());
        assert_eq!(hot_blocks[0].block_id, 1);
    }

    #[test]
    fn test_profile_collector_cold_blocks() {
        let collector = ProfileCollector::new();

        // 热块
        for _ in 0..1000 {
            collector.record_block_execution(1, 10);
        }

        // 冷块
        for _ in 0..3 {
            collector.record_block_execution(2, 100);
        }

        let cold_blocks = collector.get_cold_blocks(10);
        assert!(cold_blocks.iter().any(|b| b.block_id == 2));
    }

    #[test]
    fn test_call_profile_metrics() {
        let call = CallProfile {
            caller_id: 1,
            callee_id: 2,
            call_count: 100,
            total_time_us: 1000,
        };

        assert_eq!(call.avg_time_us(), 10.0);
        assert!(call.is_hot_path());
    }

    #[test]
    fn test_aot_optimization_driver() {
        let collector = Arc::new(ProfileCollector::new());

        // 创建热块Profile
        for _ in 0..1500 {
            collector.record_block_execution(1, 10);
        }

        let driver = AotOptimizationDriver::new(collector);
        driver.generate_optimization_hints();

        let hint = driver.get_optimization_hint(1).unwrap();
        assert_eq!(hint.optimization_level, 3);
        assert!(hint.should_unroll_loops);
    }

    #[test]
    fn test_pgo_manager_basic() {
        let manager = PgoManager::new();

        // 模拟执行和Profile收集
        for _ in 0..500 {
            manager.collector.record_block_execution(1, 20);
        }

        let result = manager.optimize_iteration();
        assert_eq!(result.iteration, 1);
        assert!(result.stats.total_blocks_profiled > 0);
    }

    #[test]
    fn test_pgo_manager_multiple_iterations() {
        let manager = PgoManager::new();

        for iter in 0..3 {
            manager.collector.clear();
            for _ in 0..1000 {
                manager
                    .collector
                    .record_block_execution((iter as u64) + 1, 10);
            }
            let result = manager.optimize_iteration();
            assert_eq!(result.iteration as usize, iter + 1);
        }
    }

    #[test]
    fn test_pgo_optimization_stats() {
        let manager = PgoManager::new();

        // 创建不同热度的块
        for _ in 0..100 {
            manager.collector.record_block_execution(1, 10); // 冷块
        }
        for _ in 0..2000 {
            manager.collector.record_block_execution(2, 10); // 热块
        }

        manager.optimize_iteration();

        let stats = manager.get_optimization_stats();
        assert!(stats.total_hints > 0);
    }

    #[test]
    fn test_pgo_improvement_estimation() {
        let manager = PgoManager::new();

        for _ in 0..5000 {
            manager.collector.record_block_execution(1, 20);
        }

        let driver = AotOptimizationDriver::new(manager.collector.clone());
        let improvement = driver.estimate_improvement();
        assert!((0.0..=100.0).contains(&improvement));
    }
}
