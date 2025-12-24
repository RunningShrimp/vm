use std::collections::HashMap;
use std::time::{Duration, Instant};
use vm_ir::{IRBlock, IROp, RegId};
use crate::common::{OptimizationStats, CacheStats};

pub struct AdaptiveOptimizer {
    config: AdaptiveOptimizerConfig,
    execution_history: HashMap<RegId, ExecutionHistory>,
    block_profiles: HashMap<u64, BlockProfile>,
    hot_threshold: u64,
    cold_threshold: u64,
    stats: OptimizationStats,
}

#[derive(Debug, Clone)]
pub struct AdaptiveOptimizerConfig {
    pub initial_hot_threshold: u64,
    pub initial_cold_threshold: u64,
    pub adjustment_factor: f64,
    pub min_threshold: u64,
    pub max_threshold: u64,
    pub sampling_rate: u32,
    pub profile_window_size: usize,
    pub enable_adaptive_recompilation: bool,
}

impl Default for AdaptiveOptimizerConfig {
    fn default() -> Self {
        Self {
            initial_hot_threshold: 100,
            initial_cold_threshold: 10,
            adjustment_factor: 0.1,
            min_threshold: 5,
            max_threshold: 1000,
            sampling_rate: 100,
            profile_window_size: 1000,
            enable_adaptive_recompilation: true,
        }
    }
}

impl AdaptiveOptimizerConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_hot_threshold(mut self, threshold: u64) -> Self {
        self.initial_hot_threshold = threshold;
        self
    }

    pub fn with_cold_threshold(mut self, threshold: u64) -> Self {
        self.initial_cold_threshold = threshold;
        self
    }

    pub fn with_adjustment_factor(mut self, factor: f64) -> Self {
        self.adjustment_factor = factor.clamp(0.01, 1.0);
        self
    }

    pub fn with_enable_adaptive_recompilation(mut self, enable: bool) -> Self {
        self.enable_adaptive_recompilation = enable;
        self
    }
}

#[derive(Debug, Clone)]
pub struct ExecutionHistory {
    pub execution_count: u64,
    pub total_time: Duration,
    pub avg_time: Duration,
    pub last_execution: Option<Instant>,
    pub optimization_level: OptimizationLevel,
}

impl Default for ExecutionHistory {
    fn default() -> Self {
        Self {
            execution_count: 0,
            total_time: Duration::ZERO,
            avg_time: Duration::ZERO,
            last_execution: None,
            optimization_level: OptimizationLevel::None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum OptimizationLevel {
    None,
    Basic,
    Aggressive,
    Maximum,
}

#[derive(Debug, Clone)]
pub struct BlockProfile {
    pub block_id: u64,
    pub execution_count: u64,
    pub total_time_ns: u64,
    pub avg_time_ns: u64,
    pub current_optimization: OptimizationLevel,
    pub recommended_optimization: OptimizationLevel,
    pub recompilation_count: u64,
}

impl BlockProfile {
    pub fn new(block_id: u64) -> Self {
        Self {
            block_id,
            execution_count: 0,
            total_time_ns: 0,
            avg_time_ns: 0,
            current_optimization: OptimizationLevel::None,
            recommended_optimization: OptimizationLevel::None,
            recompilation_count: 0,
        }
    }

    pub fn record_execution(&mut self, time_ns: u64) {
        self.execution_count += 1;
        self.total_time_ns += time_ns;
        self.avg_time_ns = self.total_time_ns / self.execution_count;
    }

    pub fn needs_recompilation(&self, config: &AdaptiveOptimizerConfig) -> bool {
        if self.execution_count < config.sampling_rate as u64 {
            return false;
        }
        
        self.current_optimization != self.recommended_optimization
    }
}

impl AdaptiveOptimizer {
    pub fn new(config: AdaptiveOptimizerConfig) -> Self {
        Self {
            hot_threshold: config.initial_hot_threshold,
            cold_threshold: config.initial_cold_threshold,
            config,
            execution_history: HashMap::new(),
            block_profiles: HashMap::new(),
            stats: OptimizationStats::default(),
        }
    }

    pub fn optimize_block(&mut self, block: &IRBlock) -> OptimizationLevel {
        let block_id = block.start_pc.0;
        let sampling_rate = self.config.sampling_rate as u64;
        let hot_threshold = self.hot_threshold;
        let cold_threshold = self.cold_threshold;
        
        let profile = self.block_profiles.entry(block_id)
            .or_insert_with(|| BlockProfile::new(block_id));
        
        let recommended_level = if profile.execution_count < sampling_rate {
            OptimizationLevel::Basic
        } else if profile.execution_count >= hot_threshold {
            OptimizationLevel::Maximum
        } else if profile.execution_count >= hot_threshold / 2 {
            OptimizationLevel::Aggressive
        } else if profile.execution_count <= cold_threshold {
            OptimizationLevel::None
        } else {
            OptimizationLevel::Basic
        };
        
        let needs_recompile = recommended_level != profile.current_optimization;
        if needs_recompile && self.config.enable_adaptive_recompilation {
            profile.current_optimization = recommended_level.clone();
            profile.recompilation_count += 1;
            self.stats.blocks_optimized += 1;
        }
        profile.recommended_optimization = recommended_level.clone();
        recommended_level
    }

    pub fn record_execution(&mut self, block_id: u64, execution_time_ns: u64) {
        let profile = self.block_profiles.entry(block_id)
            .or_insert_with(|| BlockProfile::new(block_id));
        profile.record_execution(execution_time_ns);
        
        let execution_count = profile.execution_count;
        
        if execution_count >= self.config.sampling_rate as u64 {
            if execution_count as f64 > self.hot_threshold as f64 * 2.0 {
                let new_hot_threshold = (self.hot_threshold as f64 * (1.0 - self.config.adjustment_factor)) as u64;
                self.hot_threshold = new_hot_threshold.max(self.config.min_threshold);
            }

            if execution_count as f64 > self.cold_threshold as f64 * 2.0 {
                let new_cold_threshold = (self.cold_threshold as f64 * (1.0 + self.config.adjustment_factor)) as u64;
                self.cold_threshold = new_cold_threshold.min(self.config.max_threshold);
            }
        }
    }

    pub fn get_block_profile(&self, block_id: u64) -> Option<&BlockProfile> {
        self.block_profiles.get(&block_id)
    }

    pub fn get_current_thresholds(&self) -> (u64, u64) {
        (self.hot_threshold, self.cold_threshold)
    }

    pub fn get_stats(&self) -> &OptimizationStats {
        &self.stats
    }

    pub fn reset_stats(&mut self) {
        self.stats = OptimizationStats::default();
    }

    pub fn analyze_hotspots(&self) -> Vec<HotspotInfo> {
        let mut hotspots = Vec::new();
        
        for profile in self.block_profiles.values() {
            if profile.execution_count >= self.hot_threshold {
                hotspots.push(HotspotInfo {
                    block_id: profile.block_id,
                    execution_count: profile.execution_count,
                    avg_time_ns: profile.avg_time_ns,
                    optimization_level: profile.current_optimization.clone(),
                });
            }
        }

        hotspots.sort_by(|a, b| {
            b.execution_count.cmp(&a.execution_count)
                .then_with(|| a.avg_time_ns.cmp(&b.avg_time_ns))
        });

        hotspots
    }
}

#[derive(Debug, Clone)]
pub struct HotspotInfo {
    pub block_id: u64,
    pub execution_count: u64,
    pub avg_time_ns: u64,
    pub optimization_level: OptimizationLevel,
}

pub struct DefaultAdaptiveOptimizer {
    inner: AdaptiveOptimizer,
}

impl DefaultAdaptiveOptimizer {
    pub fn new() -> Self {
        let config = AdaptiveOptimizerConfig::default();
        Self {
            inner: AdaptiveOptimizer::new(config),
        }
    }

    pub fn optimize(&mut self, block: &IRBlock) -> OptimizationLevel {
        self.inner.optimize_block(block)
    }

    pub fn record_execution(&mut self, block_id: u64, execution_time_ns: u64) {
        self.inner.record_execution(block_id, execution_time_ns);
    }

    pub fn get_hotspots(&self) -> Vec<HotspotInfo> {
        self.inner.analyze_hotspots()
    }

    pub fn get_current_thresholds(&self) -> (u64, u64) {
        self.inner.get_current_thresholds()
    }
}

impl Default for DefaultAdaptiveOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adaptive_optimizer_creation() {
        let config = AdaptiveOptimizerConfig::new();
        let optimizer = AdaptiveOptimizer::new(config);
        assert_eq!(optimizer.hot_threshold, 100);
        assert_eq!(optimizer.cold_threshold, 10);
    }

    #[test]
    fn test_optimization_levels() {
        let config = AdaptiveOptimizerConfig::default();
        let mut optimizer = AdaptiveOptimizer::new(config);
        
        let block = IRBlock {
            start_pc: vm_core::GuestAddr(0x1000),
            ops: vec![IROp::MovImm { dst: 0, imm: 42 }],
            term: vm_ir::Terminator::Ret,
        };

        for _ in 0..50 {
            optimizer.record_execution(0x1000, 100);
        }
        let level = optimizer.optimize_block(&block);
        assert_eq!(level, OptimizationLevel::Basic);
    }

    #[test]
    fn test_threshold_adjustment() {
        let config = AdaptiveOptimizerConfig::new()
            .with_hot_threshold(100)
            .with_cold_threshold(10);
        let mut optimizer = AdaptiveOptimizer::new(config);

        optimizer.record_execution(0x1000, 100);
        optimizer.record_execution(0x1000, 100);
        
        let (hot, cold) = optimizer.get_current_thresholds();
        assert_eq!(hot, 100);
        assert_eq!(cold, 10);
    }

    #[test]
    fn test_hotspot_detection() {
        let config = AdaptiveOptimizerConfig::default();
        let mut optimizer = AdaptiveOptimizer::new(config);
        
        optimizer.record_execution(0x1000, 100);
        for _ in 0..150 {
            optimizer.record_execution(0x2000, 50);
        }

        let hotspots = optimizer.analyze_hotspots();
        assert!(!hotspots.is_empty());
        assert!(hotspots[0].execution_count >= 100);
    }
}
