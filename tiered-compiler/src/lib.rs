//! Tiered Compilation System
//!
//! Implements 4-tier compilation strategy:
//! - Tier 0: Interpretation (no compilation)
//! - Tier 1: Fast JIT (<100 μs compilation time)
//! - Tier 2: Balanced JIT (300-500 μs, current default)
//! - Tier 3: Optimized JIT (>1ms, aggressive optimization)

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use parking_lot::RwLock;

/// Result type for compilation operations
pub type CompileResult = Result<CompiledCode, CompileError>;

/// Compilation error types
#[derive(Debug, Clone)]
pub enum CompileError {
    /// Block not found
    BlockNotFound { block_id: u64 },
    /// Compilation timeout
    Timeout { block_id: u64 },
    /// Invalid IR
    InvalidIr { block_id: u64, reason: String },
    /// Out of memory
    OutOfMemory { required: usize },
}

/// Compilation tier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CompilationTier {
    Tier0 = 0, // Interpretation
    Tier1 = 1, // Fast JIT
    Tier2 = 2, // Balanced
    Tier3 = 3, // Optimized
}

/// Compiled code representation
#[derive(Debug, Clone)]
pub struct CompiledCode {
    /// Block ID
    pub block_id: u64,
    /// Generated code (simplified)
    pub code: Vec<u8>,
    /// Compilation tier
    pub tier: CompilationTier,
    /// Compilation time in microseconds
    pub compile_time_us: u64,
    /// Code size in bytes
    pub code_size: usize,
}

/// Block execution statistics
#[derive(Debug, Clone, Default)]
pub struct BlockStats {
    /// Number of times executed
    pub execution_count: u64,
    /// Total execution time in microseconds
    pub total_exec_us: u64,
    /// Current tier
    pub current_tier: Option<CompilationTier>,
    /// Time spent in tier
    pub time_in_tier_us: u64,
}

impl BlockStats {
    /// Average execution time per run
    pub fn avg_exec_time_us(&self) -> f64 {
        if self.execution_count == 0 {
            0.0
        } else {
            self.total_exec_us as f64 / self.execution_count as f64
        }
    }

    /// Estimated benefit of upgrading tier (higher = more benefit)
    pub fn upgrade_benefit(&self) -> f64 {
        // Benefit increases with:
        // 1. Execution count (more runs = worth optimizing)
        // 2. Average exec time (slower = more room for improvement)
        let hotness = (self.execution_count as f64).ln().max(1.0);
        let slowness = self.avg_exec_time_us().max(1.0);
        hotness * slowness
    }
}

/// Tier upgrade policy
#[derive(Debug, Clone)]
pub struct UpgradePolicy {
    /// Min executions before Tier 1 upgrade
    pub tier0_to_tier1_threshold: u64,
    /// Min executions before Tier 2 upgrade
    pub tier1_to_tier2_threshold: u64,
    /// Min executions before Tier 3 upgrade
    pub tier2_to_tier3_threshold: u64,
    /// Min time in current tier (microseconds) before upgrade
    pub min_time_in_tier_us: u64,
}

impl Default for UpgradePolicy {
    fn default() -> Self {
        Self {
            tier0_to_tier1_threshold: 10,   // Hot after 10 executions
            tier1_to_tier2_threshold: 100,  // Very hot after 100 executions
            tier2_to_tier3_threshold: 1000, // Ultra hot after 1000 executions
            min_time_in_tier_us: 100_000,   // At least 100ms in each tier
        }
    }
}

/// Tier-aware compiler state
pub struct TieredCompilerState {
    /// Block statistics
    stats: Arc<RwLock<HashMap<u64, BlockStats>>>,
    /// Code cache
    cache: Arc<RwLock<HashMap<u64, CompiledCode>>>,
    /// Upgrade policy
    pub policy: UpgradePolicy,
}

impl TieredCompilerState {
    /// Create new compiler state
    pub fn new(policy: UpgradePolicy) -> Self {
        Self {
            stats: Arc::new(RwLock::new(HashMap::new())),
            cache: Arc::new(RwLock::new(HashMap::new())),
            policy,
        }
    }

    /// Record block execution
    pub fn record_execution(&self, block_id: u64, exec_time_us: u64) {
        let mut stats = self.stats.write();
        let block_stat = stats.entry(block_id).or_insert_with(|| BlockStats {
            current_tier: Some(CompilationTier::Tier0),
            ..Default::default()
        });

        block_stat.execution_count += 1;
        block_stat.total_exec_us += exec_time_us;
        if let Some(_tier) = block_stat.current_tier {
            block_stat.time_in_tier_us += exec_time_us;
        }
    }

    /// Get block statistics
    pub fn get_stats(&self, block_id: u64) -> Option<BlockStats> {
        self.stats.read().get(&block_id).cloned()
    }

    /// Check if block should upgrade tier
    pub fn should_upgrade(&self, block_id: u64) -> Option<CompilationTier> {
        let stats = self.stats.read();
        let block_stat = stats.get(&block_id)?;

        let current_tier = block_stat.current_tier?;
        let exec_count = block_stat.execution_count;
        let time_in_tier = block_stat.time_in_tier_us;

        // Check if minimum time in tier requirement is met
        if time_in_tier < self.policy.min_time_in_tier_us {
            return None;
        }

        // Determine next tier
        match current_tier {
            CompilationTier::Tier0 => {
                if exec_count >= self.policy.tier0_to_tier1_threshold {
                    Some(CompilationTier::Tier1)
                } else {
                    None
                }
            }
            CompilationTier::Tier1 => {
                if exec_count >= self.policy.tier1_to_tier2_threshold {
                    Some(CompilationTier::Tier2)
                } else {
                    None
                }
            }
            CompilationTier::Tier2 => {
                if exec_count >= self.policy.tier2_to_tier3_threshold {
                    Some(CompilationTier::Tier3)
                } else {
                    None
                }
            }
            CompilationTier::Tier3 => None,
        }
    }

    /// Update tier for block
    pub fn update_tier(&self, block_id: u64, tier: CompilationTier) {
        let mut stats = self.stats.write();
        if let Some(block_stat) = stats.get_mut(&block_id) {
            block_stat.current_tier = Some(tier);
            block_stat.time_in_tier_us = 0; // Reset time counter
        }
    }

    /// Cache compiled code
    pub fn cache_code(&self, code: CompiledCode) {
        self.cache.write().insert(code.block_id, code);
    }

    /// Get cached code
    pub fn get_cached_code(&self, block_id: u64) -> Option<CompiledCode> {
        self.cache.read().get(&block_id).cloned()
    }

    /// Clear cache
    pub fn clear_cache(&self) {
        self.cache.write().clear();
    }
}

/// Tier 0: Interpretation (no compilation)
pub struct Tier0Interpreter;

impl Tier0Interpreter {
    /// Interpret block (placeholder)
    pub fn interpret_block(block_id: u64) -> CompileResult {
        Ok(CompiledCode {
            block_id,
            code: vec![0x90; 10], // NOP sled
            tier: CompilationTier::Tier0,
            compile_time_us: 0,
            code_size: 10,
        })
    }
}

/// Tier 1: Fast JIT (<100 μs)
pub struct Tier1FastJit;

impl Tier1FastJit {
    /// Quick compile without optimization
    /// - Skip register allocation analysis
    /// - No instruction scheduling
    /// - Minimal peephole optimization
    pub fn compile_block(block_id: u64) -> CompileResult {
        let start = Instant::now();

        // Simulate fast compilation (10-100 μs)
        let compile_time = std::cmp::min(50 + (block_id % 50), 100);
        std::thread::sleep(std::time::Duration::from_micros(compile_time));

        Ok(CompiledCode {
            block_id,
            code: vec![0x55, 0x48, 0x89, 0xe5], // Simplified prologue
            tier: CompilationTier::Tier1,
            compile_time_us: start.elapsed().as_micros() as u64,
            code_size: 256,
        })
    }
}

/// Tier 2: Balanced JIT (300-500 μs)
pub struct Tier2BalancedJit;

impl Tier2BalancedJit {
    /// Standard compilation with moderate optimization
    /// - Register allocation
    /// - Basic instruction scheduling
    /// - Standard peephole optimization
    pub fn compile_block(block_id: u64) -> CompileResult {
        let start = Instant::now();

        // Simulate balanced compilation (300-500 μs)
        let compile_time = 300 + (block_id % 200);
        std::thread::sleep(std::time::Duration::from_micros(compile_time));

        Ok(CompiledCode {
            block_id,
            code: vec![
                0x55, 0x48, 0x89, 0xe5, // Prologue
                0x48, 0x83, 0xec, 0x20, // Stack allocation
            ],
            tier: CompilationTier::Tier2,
            compile_time_us: start.elapsed().as_micros() as u64,
            code_size: 512,
        })
    }
}

/// Tier 3: Optimized JIT (>1 ms)
pub struct Tier3OptimizedJit;

impl Tier3OptimizedJit {
    /// Aggressive optimization
    /// - Global register allocation
    /// - Loop invariant code motion (LICM)
    /// - Function inlining
    /// - Advanced scheduling
    pub fn compile_block(block_id: u64) -> CompileResult {
        let start = Instant::now();

        // Simulate aggressive compilation (1-3 ms)
        let compile_time = 1000 + (block_id % 2000);
        std::thread::sleep(std::time::Duration::from_micros(compile_time));

        Ok(CompiledCode {
            block_id,
            code: vec![
                0x55, 0x48, 0x89, 0xe5, // Prologue
                0x48, 0x83, 0xec, 0x20, // Stack allocation
                0x90, 0x90, 0x90, 0x90, // Optimization space
            ],
            tier: CompilationTier::Tier3,
            compile_time_us: start.elapsed().as_micros() as u64,
            code_size: 1024,
        })
    }
}

/// Main tiered compilation orchestrator
pub struct TieredCompiler {
    state: Arc<TieredCompilerState>,
}

impl TieredCompiler {
    /// Create new tiered compiler
    pub fn new(policy: UpgradePolicy) -> Self {
        Self {
            state: Arc::new(TieredCompilerState::new(policy)),
        }
    }

    /// Compile block at appropriate tier
    pub fn compile(&self, block_id: u64) -> CompileResult {
        // Get current tier
        let stats = self.state.stats.read().get(&block_id).cloned();
        let tier = stats
            .and_then(|s| s.current_tier)
            .unwrap_or(CompilationTier::Tier0);

        // Compile at appropriate tier
        let code = match tier {
            CompilationTier::Tier0 => Tier0Interpreter::interpret_block(block_id)?,
            CompilationTier::Tier1 => Tier1FastJit::compile_block(block_id)?,
            CompilationTier::Tier2 => Tier2BalancedJit::compile_block(block_id)?,
            CompilationTier::Tier3 => Tier3OptimizedJit::compile_block(block_id)?,
        };

        // Cache result
        self.state.cache_code(code.clone());
        Ok(code)
    }

    /// Execute block and record statistics
    pub fn execute_block(&self, block_id: u64, exec_time_us: u64) -> CompileResult {
        // Record execution
        self.state.record_execution(block_id, exec_time_us);

        // Check for tier upgrade
        if let Some(new_tier) = self.state.should_upgrade(block_id) {
            self.state.update_tier(block_id, new_tier);
        }

        // Recompile if tier changed
        if let Some(cached) = self.state.get_cached_code(block_id)
            && let Some(current_stats) = self.state.get_stats(block_id)
            && current_stats.current_tier != Some(cached.tier)
        {
            return self.compile(block_id);
        }

        // Return cached or compile
        if let Some(cached) = self.state.get_cached_code(block_id) {
            Ok(cached)
        } else {
            self.compile(block_id)
        }
    }

    /// Get compilation statistics
    pub fn get_stats(&self, block_id: u64) -> Option<BlockStats> {
        self.state.get_stats(block_id)
    }

    /// Clear all caches
    pub fn clear_cache(&self) {
        self.state.clear_cache();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier0_interpretation() {
        let compiler = TieredCompiler::new(UpgradePolicy::default());
        let code = compiler.compile(1).unwrap();

        assert_eq!(code.tier, CompilationTier::Tier0);
        assert_eq!(code.code_size, 10);
    }

    #[test]
    fn test_tier1_fast_jit() {
        let code = Tier1FastJit::compile_block(1).unwrap();

        assert_eq!(code.tier, CompilationTier::Tier1);
        assert!(code.compile_time_us <= 100);
        assert_eq!(code.code_size, 256);
    }

    #[test]
    fn test_tier2_balanced_jit() {
        let code = Tier2BalancedJit::compile_block(1).unwrap();

        assert_eq!(code.tier, CompilationTier::Tier2);
        assert!(code.compile_time_us >= 300);
        assert!(code.compile_time_us <= 500);
        assert_eq!(code.code_size, 512);
    }

    #[test]
    fn test_tier3_optimized_jit() {
        let code = Tier3OptimizedJit::compile_block(1).unwrap();

        assert_eq!(code.tier, CompilationTier::Tier3);
        assert!(code.compile_time_us >= 1000);
        assert_eq!(code.code_size, 1024);
    }

    #[test]
    fn test_block_stats() {
        let policy = UpgradePolicy::default();
        let state = TieredCompilerState::new(policy);

        // Execute block multiple times
        for i in 0..15 {
            state.record_execution(1, 50 + i * 10);
        }

        let stats = state.get_stats(1).unwrap();
        assert_eq!(stats.execution_count, 15);
        assert!(stats.upgrade_benefit() > 0.0);
    }

    #[test]
    fn test_tier_upgrade_policy() {
        let policy = UpgradePolicy::default();
        let state = TieredCompilerState::new(policy);

        // Record enough executions to trigger Tier 0->1 upgrade
        // Need: 10 executions + 100ms total time
        for _ in 0..15 {
            state.record_execution(1, 10_000); // 10ms each
        }

        // Should upgrade to Tier 1 (15 executions > 10 threshold, 150ms > 100ms min)
        let upgrade = state.should_upgrade(1);
        assert_eq!(upgrade, Some(CompilationTier::Tier1));
    }

    #[test]
    fn test_tier_upgrade_chain() {
        let policy = UpgradePolicy {
            tier0_to_tier1_threshold: 5,
            tier1_to_tier2_threshold: 10,
            tier2_to_tier3_threshold: 15,
            min_time_in_tier_us: 1000,
        };
        let state = TieredCompilerState::new(policy);

        // Tier 0 -> 1
        for _ in 0..10 {
            state.record_execution(1, 500);
        }
        assert_eq!(state.should_upgrade(1), Some(CompilationTier::Tier1));

        // Update to Tier 1
        state.update_tier(1, CompilationTier::Tier1);
        assert_eq!(
            state.get_stats(1).unwrap().current_tier,
            Some(CompilationTier::Tier1)
        );
    }

    #[test]
    fn test_code_caching() {
        let compiler = TieredCompiler::new(UpgradePolicy::default());

        // Compile and cache
        let code1 = compiler.compile(1).unwrap();
        compiler.state.cache_code(code1.clone());

        // Retrieve from cache
        let cached = compiler.state.get_cached_code(1).unwrap();
        assert_eq!(cached.block_id, 1);
        assert_eq!(cached.tier, CompilationTier::Tier0);

        // Clear cache
        compiler.clear_cache();
        assert!(compiler.state.get_cached_code(1).is_none());
    }

    #[test]
    fn test_tiered_compiler_workflow() {
        let compiler = TieredCompiler::new(UpgradePolicy {
            tier0_to_tier1_threshold: 5,
            tier1_to_tier2_threshold: 10,
            tier2_to_tier3_threshold: 15,
            min_time_in_tier_us: 100,
        });

        // Initial compilation at Tier 0
        let code = compiler.compile(42).unwrap();
        assert_eq!(code.tier, CompilationTier::Tier0);

        // Execute and track statistics
        for i in 0..20 {
            let _ = compiler.execute_block(42, 100 + i * 10);
        }

        // Verify hotness tracking
        let stats = compiler.get_stats(42).unwrap();
        assert!(stats.execution_count >= 20);
    }

    #[test]
    fn test_multiple_blocks() {
        let compiler = TieredCompiler::new(UpgradePolicy::default());

        // Compile multiple blocks to initialize stats
        for block_id in 1..=10 {
            let code = compiler.compile(block_id).unwrap();
            assert_eq!(code.block_id, block_id);
            assert_eq!(code.tier, CompilationTier::Tier0);
            // Record an execution to initialize stats
            compiler.state.record_execution(block_id, 100);
        }

        // Verify all blocks have stats
        for block_id in 1..=10 {
            assert!(compiler.get_stats(block_id).is_some());
        }
    }

    #[test]
    fn test_upgrade_benefit_calculation() {
        let state = TieredCompilerState::new(UpgradePolicy::default());

        // Record high-value block (many executions, slow)
        for _ in 0..100 {
            state.record_execution(1, 500);
        }

        let stats = state.get_stats(1).unwrap();
        assert!(stats.upgrade_benefit() > 100.0); // High benefit
    }
}
