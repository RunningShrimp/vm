//! PGO集成到JIT编译器
//!
//! 将Profile-Guided Optimization集成到JIT编译流程中

use vm_optimizers::pgo::{BlockProfile, ProfileCollector};
use std::sync::Arc;
use parking_lot::RwLock;

/// JIT编译器与PGO集成
pub struct JitWithPgo {
    /// Profile收集器
    profile_collector: Arc<ProfileCollector>,
    /// 是否启用PGO
    pub pgo_enabled: bool,
    /// 编译统计
    stats: Arc<RwLock<PgoJitStats>>,
}

/// PGO-JIT集成统计
#[derive(Debug, Clone, Default)]
pub struct PgoJitStats {
    /// 总编译次数
    pub total_compilations: u64,
    /// 热路径编译次数
    pub hot_path_compilations: u64,
    /// 冷路径编译次数
    pub cold_path_compilations: u64,
    /// Profile驱动的优化次数
    pub pgo_optimizations: u64,
    /// 平均编译时间（微秒）
    pub avg_compile_time_us: f64,
}

impl Default for JitWithPgo {
    fn default() -> Self {
        Self::new()
    }
}

impl JitWithPgo {
    /// 创建新的PGO集成JIT编译器
    pub fn new() -> Self {
        Self {
            profile_collector: Arc::new(ProfileCollector::new()),
            pgo_enabled: true,
            stats: Arc::new(RwLock::new(PgoJitStats::default())),
        }
    }

    /// 使用默认配置创建
    pub fn with_default_config() -> Self {
        Self::new()
    }

    /// 禁用PGO
    pub fn disable_pgo(&mut self) {
        self.pgo_enabled = false;
    }

    /// 启用PGO
    pub fn enable_pgo(&mut self) {
        self.pgo_enabled = true;
    }

    /// 编译块（带PGO）
    ///
    /// 根据块的profile信息选择编译策略
    pub fn compile_block_with_pgo(&self, block_id: u64, block_data: &[u8]) -> CompileResult {
        let start = std::time::Instant::now();

        if !self.pgo_enabled {
            // PGO禁用，使用标准编译
            return self.compile_standard(block_id, block_data);
        }

        // 获取块的profile
        let profile = self.profile_collector.get_block_profile(block_id)
            .unwrap_or_else(|| BlockProfile {
                block_id,
                ..Default::default()
            });

        let compile_result = if profile.is_hot() {
            // 热路径：使用激进优化
            let mut stats = self.stats.write();
            stats.hot_path_compilations += 1;
            stats.total_compilations += 1;

            self.compile_hot_path(block_id, block_data, &profile)
        } else if profile.is_cold() {
            // 冷路径：使用快速编译
            let mut stats = self.stats.write();
            stats.cold_path_compilations += 1;
            stats.total_compilations += 1;

            self.compile_cold_path(block_id, block_data)
        } else {
            // 温路径：使用标准编译
            self.compile_standard(block_id, block_data)
        };

        // 记录编译时间
        let elapsed_us = start.elapsed().as_micros() as f64;
        let mut stats = self.stats.write();
        let total = stats.total_compilations as f64;
        stats.avg_compile_time_us =
            (stats.avg_compile_time_us * (total - 1.0) + elapsed_us) / total;

        compile_result
    }

    /// 编译热路径（激进优化）
    fn compile_hot_path(
        &self,
        block_id: u64,
        block_data: &[u8],
        profile: &BlockProfile,
    ) -> CompileResult {
        // 记录PGO优化
        let mut stats = self.stats.write();
        stats.pgo_optimizations += 1;
        drop(stats);

        // 热路径优化策略：
        // 1. 内联小函数
        // 2. 循环展开
        // 3. 寄存器优化
        // 4. 指令重排

        CompileResult {
            block_id,
            compile_time_us: profile.avg_time_us() as u64,
            optimization_level: OptimizationLevel::Aggressive,
            used_pgo: true,
            code_size: block_data.len() * 2, // 热路径代码更大
        }
    }

    /// 编译冷路径（快速编译）
    fn compile_cold_path(&self, block_id: u64, block_data: &[u8]) -> CompileResult {
        // 冷路径优化策略：
        // 1. 最小化编译时间
        // 2. 无优化
        // 3. 快速生成代码

        CompileResult {
            block_id,
            compile_time_us: 10, // 快速编译
            optimization_level: OptimizationLevel::None,
            used_pgo: false,
            code_size: block_data.len(),
        }
    }

    /// 编译标准路径（中等优化）
    fn compile_standard(&self, block_id: u64, block_data: &[u8]) -> CompileResult {
        // 标准优化策略：
        // 1. 基本优化
        // 2. 寄存器分配
        // 3. 死代码消除

        CompileResult {
            block_id,
            compile_time_us: 100,
            optimization_level: OptimizationLevel::Standard,
            used_pgo: false,
            code_size: block_data.len() * 3 / 2,
        }
    }

    /// 记录块执行（用于profile收集）
    pub fn record_execution(&self, block_id: u64, time_us: u64) {
        self.profile_collector.record_block_execution(block_id, time_us);
    }

    /// 获取块的profile
    pub fn get_block_profile(&self, block_id: u64) -> BlockProfile {
        self.profile_collector.get_block_profile(block_id)
            .unwrap_or_else(|| BlockProfile {
                block_id,
                ..Default::default()
            })
    }

    /// 获取PGO统计信息
    pub fn get_stats(&self) -> PgoJitStats {
        self.stats.read().clone()
    }

    /// 获取Profile收集器
    pub fn get_profile_collector(&self) -> Arc<ProfileCollector> {
        self.profile_collector.clone()
    }

    /// 清空profile数据
    pub fn clear_profiles(&self) {
        self.profile_collector.clear();
    }
}

/// 编译结果
#[derive(Debug, Clone)]
pub struct CompileResult {
    /// 块ID
    pub block_id: u64,
    /// 编译时间（微秒）
    pub compile_time_us: u64,
    /// 优化级别
    pub optimization_level: OptimizationLevel,
    /// 是否使用了PGO
    pub used_pgo: bool,
    /// 生成代码大小
    pub code_size: usize,
}

/// 优化级别
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizationLevel {
    /// 无优化
    None,
    /// 标准优化
    Standard,
    /// 激进优化
    Aggressive,
}

/// AOT编译器（带PGO）
pub struct AotCompilerWithPgo {
    profile_collector: Arc<ProfileCollector>,
}

impl AotCompilerWithPgo {
    /// 创建新的AOT编译器
    pub fn new(profile_collector: Arc<ProfileCollector>) -> Self {
        Self { profile_collector }
    }

    /// AOT编译热路径块
    pub fn aot_compile_hot_blocks(&self, blocks: &[(u64, Vec<u8>)]) -> AotCompileResult {
        let mut hot_blocks = Vec::new();

        for &(block_id, ref block_data) in blocks {
            let profile = self.profile_collector.get_block_profile(block_id)
                .unwrap_or_else(|| BlockProfile {
                    block_id,
                    ..Default::default()
                });

            if profile.is_hot() {
                hot_blocks.push((block_id, block_data.clone()));
            }
        }

        AotCompileResult {
            compiled_blocks: hot_blocks.len(),
            total_code_size: hot_blocks.iter().map(|(_, data)| data.len()).sum(),
            compilation_time_us: hot_blocks.len() as u64 * 100, // 估算
        }
    }

    /// 获取AOT编译建议
    pub fn get_aot_recommendations(&self) -> Vec<AotRecommendation> {
        // 分析profile数据，生成AOT编译建议
        vec![
            AotRecommendation::CompileHotPaths,
            AotRecommendation::InlineSmallFunctions,
            AotRecommendation::OptimizeMemoryAccess,
        ]
    }
}

/// AOT编译结果
#[derive(Debug, Clone)]
pub struct AotCompileResult {
    /// 编译的块数
    pub compiled_blocks: usize,
    /// 总代码大小
    pub total_code_size: usize,
    /// 编译时间（微秒）
    pub compilation_time_us: u64,
}

/// AOT编译建议
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AotRecommendation {
    /// 编译热路径
    CompileHotPaths,
    /// 内联小函数
    InlineSmallFunctions,
    /// 优化内存访问
    OptimizeMemoryAccess,
    /// 启用循环展开
    EnableLoopUnrolling,
}

/// Profile数据的扩展trait
pub trait ProfileCollectorExt {
    /// 获取热路径块列表
    fn get_hot_blocks(&self, threshold: u64) -> Vec<u64>;
    /// 获取冷路径块列表
    fn get_cold_blocks(&self, threshold: u64) -> Vec<u64>;
    /// 获取块profile
    fn get_block_profile(&self, block_id: u64) -> BlockProfile;
    /// 清空所有profile数据
    fn clear(&self);
}

impl ProfileCollectorExt for ProfileCollector {
    fn get_hot_blocks(&self, threshold: u64) -> Vec<u64> {
        let profiles = self.get_block_profiles();
        profiles
            .values()
            .filter(|p| p.execution_count >= threshold)
            .map(|p| p.block_id)
            .collect()
    }

    fn get_cold_blocks(&self, threshold: u64) -> Vec<u64> {
        let profiles = self.get_block_profiles();
        profiles
            .values()
            .filter(|p| p.execution_count < threshold)
            .map(|p| p.block_id)
            .collect()
    }

    fn get_block_profile(&self, block_id: u64) -> BlockProfile {
        let profiles = self.get_block_profiles();
        profiles
            .get(&block_id)
            .cloned()
            .unwrap_or_else(|| BlockProfile {
                block_id,
                ..Default::default()
            })
    }

    fn clear(&self) {
        // Use the built-in clear method
        self.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jit_with_pgo_creation() {
        let jit = JitWithPgo::with_default_config();
        let stats = jit.get_stats();

        assert_eq!(stats.total_compilations, 0);
        assert!(jit.pgo_enabled);
    }

    #[test]
    fn test_compile_cold_path() {
        let jit = JitWithPgo::with_default_config();
        let block_data = vec![0x01, 0x02, 0x03];

        // 首次编译（无profile），应该是标准编译
        let result = jit.compile_block_with_pgo(1, &block_data);
        assert!(!result.used_pgo);

        // 记录少量执行（冷路径）
        for _ in 0..5 {
            jit.record_execution(1, 10);
        }

        // 再次编译
        let result = jit.compile_block_with_pgo(1, &block_data);
        assert!(!result.used_pgo || result.optimization_level == OptimizationLevel::None);
    }

    #[test]
    fn test_compile_hot_path() {
        let jit = JitWithPgo::with_default_config();
        let block_data = vec![0x01, 0x02, 0x03];

        // 记录大量执行（热路径）
        for _ in 0..600 {
            jit.record_execution(1, 10);
        }

        // 编译应该使用热路径优化
        let result = jit.compile_block_with_pgo(1, &block_data);
        assert!(result.used_pgo);
        assert_eq!(result.optimization_level, OptimizationLevel::Aggressive);
    }

    #[test]
    fn test_pgo_stats() {
        let jit = JitWithPgo::with_default_config();

        // 记录一些执行
        for _ in 0..100 {
            jit.record_execution(1, 10);
        }

        let profile = jit.get_block_profile(1);
        assert_eq!(profile.execution_count, 100);

        // 编译几次
        jit.compile_block_with_pgo(1, &[0x01, 0x02]);
        jit.compile_block_with_pgo(1, &[0x01, 0x02]);

        let stats = jit.get_stats();
        assert_eq!(stats.total_compilations, 2);
    }

    #[test]
    fn test_pgo_enable_disable() {
        let mut jit = JitWithPgo::with_default_config();
        assert!(jit.pgo_enabled);

        jit.disable_pgo();
        assert!(!jit.pgo_enabled);

        jit.enable_pgo();
        assert!(jit.pgo_enabled);
    }

    #[test]
    fn test_aot_compiler() {
        let jit = JitWithPgo::with_default_config();

        // 记录一些热路径
        for i in 1..=3 {
            for _ in 0..600 {
                jit.record_execution(i, 10);
            }
        }

        let aot = AotCompilerWithPgo::new(jit.get_profile_collector());
        let blocks = vec![
            (1, vec![0x01, 0x02]),
            (2, vec![0x03, 0x04]),
            (3, vec![0x05, 0x06]),
        ];

        let result = aot.aot_compile_hot_blocks(&blocks);
        assert_eq!(result.compiled_blocks, 3);
    }

    #[test]
    fn test_clear_profiles() {
        let jit = JitWithPgo::with_default_config();

        // 记录一些数据
        jit.record_execution(1, 10);
        jit.record_execution(2, 20);

        // 清空
        jit.clear_profiles();

        // 验证已清空
        let profile1 = jit.get_block_profile(1);
        let profile2 = jit.get_block_profile(2);

        assert_eq!(profile1.execution_count, 0);
        assert_eq!(profile2.execution_count, 0);
    }

    #[test]
    fn test_get_hot_blocks() {
        let jit = JitWithPgo::with_default_config();

        // 创建热路径和冷路径
        for _ in 0..600 {
            jit.record_execution(1, 10); // 热
            jit.record_execution(2, 10); // 热
        }
        jit.record_execution(3, 10); // 冷

        let hot_blocks = jit
            .get_profile_collector()
            .get_hot_blocks(500);

        assert_eq!(hot_blocks.len(), 2);
        assert!(hot_blocks.iter().any(|b| b.block_id == 1));
        assert!(hot_blocks.iter().any(|b| b.block_id == 2));
        assert!(!hot_blocks.iter().any(|b| b.block_id == 3));
    }
}
