//! Performance optimization module
//!
//! Consolidates JIT compilation, async I/O, and frontend generation features
//! under a single "performance" feature flag.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use vm_core::{GuestAddr, GuestArch};
use vm_engine_jit::{AdaptiveThresholdConfig, AdaptiveThresholdStats, CodePtr};

use crate::execution::{ExecutionContext, IrqPolicy, TrapHandler};

/// Performance statistics (consolidates JIT stats and async metrics)
#[derive(Clone, Debug)]
pub struct PerformanceStats {
    /// Execution count (from JIT)
    pub execution_count: u64,
    /// Cold threshold
    pub cold_threshold: u64,
    /// Hot threshold
    pub hot_threshold: u64,
    /// Adaptive compilation enabled
    pub enable_adaptive: bool,
    /// JIT code pool size
    pub code_pool_size: usize,
}

/// Performance configuration (consolidates JIT and async settings)
#[derive(Clone, Debug)]
pub struct PerformanceConfig {
    /// JIT cold threshold
    pub cold_threshold: u64,
    /// JIT hot threshold
    pub hot_threshold: u64,
    /// Enable adaptive compilation
    pub enable_adaptive: bool,
    /// Enable shared code pool
    pub enable_shared_pool: bool,
    /// Enable async I/O
    pub enable_async: bool,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            cold_threshold: 100,
            hot_threshold: 1000,
            enable_adaptive: true,
            enable_shared_pool: false,
            enable_async: true,
        }
    }
}

/// Performance context - holds all performance-related state
pub struct PerformanceContext {
    /// JIT code pool
    code_pool: Option<Arc<Mutex<HashMap<GuestAddr, CodePtr>>>>,
    /// Adaptive JIT statistics
    adaptive_snapshot: Arc<Mutex<Option<AdaptiveThresholdStats>>>,
    /// Adaptive JIT configuration
    adaptive_config: Option<AdaptiveThresholdConfig>,
    /// Performance configuration
    config: PerformanceConfig,
}

impl PerformanceContext {
    /// Create a new performance context
    pub fn new() -> Self {
        Self {
            code_pool: None,
            adaptive_snapshot: Arc::new(Mutex::new(None)),
            adaptive_config: None,
            config: PerformanceConfig::default(),
        }
    }

    /// Get performance statistics
    pub fn get_stats(&self) -> Option<PerformanceStats> {
        let snapshot = self.adaptive_snapshot.lock().ok().and_then(|g| g.clone());
        let config = self.adaptive_config.clone();

        match (snapshot, config) {
            (Some(stats), Some(cfg)) => Some(PerformanceStats {
                execution_count: stats.execution_count,
                cold_threshold: cfg.cold_threshold,
                hot_threshold: cfg.hot_threshold,
                enable_adaptive: cfg.enable_adaptive,
                code_pool_size: self.code_pool.as_ref().map(|p| p.lock().ok().map(|g| g.len()).unwrap_or(0)).unwrap_or(0),
            }),
            _ => None,
        }
    }

    /// Set performance configuration
    pub fn set_config(&mut self, config: PerformanceConfig) {
        self.config = config.clone();

        // Update JIT config
        self.adaptive_config = Some(AdaptiveThresholdConfig {
            cold_threshold: config.cold_threshold,
            hot_threshold: config.hot_threshold,
            enable_adaptive: config.enable_adaptive,
            ..Default::default()
        });

        // Update code pool
        if config.enable_shared_pool {
            if self.code_pool.is_none() {
                self.code_pool = Some(Arc::new(Mutex::new(HashMap::new())));
            }
        } else {
            self.code_pool = None;
        }
    }

    /// Create execution context for synchronous execution
    pub fn create_execution_context(
        &self,
        run_flag: Arc<std::sync::atomic::AtomicBool>,
        pause_flag: Arc<std::sync::atomic::AtomicBool>,
        guest_arch: GuestArch,
        trap_handler: Option<TrapHandler>,
        irq_policy: Option<IrqPolicy>,
    ) -> ExecutionContext {
        let mut ctx = ExecutionContext::new(guest_arch);
        ctx.run_flag = run_flag;
        ctx.pause_flag = pause_flag;
        ctx.trap_handler = trap_handler;
        ctx.irq_policy = irq_policy;

        // Add JIT state if performance features are enabled
        #[cfg(feature = "performance")]
        {
            use crate::execution::jit_execution::JitExecutionState;
            ctx.jit_state = Some(JitExecutionState {
                code_pool: self.code_pool.clone(),
                adaptive_snapshot: Arc::clone(&self.adaptive_snapshot),
                adaptive_config: self.adaptive_config.clone(),
            });
        }

        ctx
    }

    /// Create execution context for asynchronous execution
    pub fn create_async_execution_context(
        &self,
        run_flag: Arc<std::sync::atomic::AtomicBool>,
        pause_flag: Arc<std::sync::atomic::AtomicBool>,
        guest_arch: GuestArch,
        trap_handler: Option<TrapHandler>,
        irq_policy: Option<IrqPolicy>,
        coroutine_scheduler: Option<Arc<Mutex<vm_runtime::CoroutineScheduler>>>,
    ) -> ExecutionContext {
        let mut ctx = ExecutionContext::new(guest_arch);
        ctx.run_flag = run_flag;
        ctx.pause_flag = pause_flag;
        ctx.trap_handler = trap_handler;
        ctx.irq_policy = irq_policy;

        // Add JIT state if performance features are enabled
        #[cfg(feature = "performance")]
        {
            use crate::execution::jit_execution::JitExecutionState;
            ctx.jit_state = Some(JitExecutionState {
                code_pool: self.code_pool.clone(),
                adaptive_snapshot: Arc::clone(&self.adaptive_snapshot),
                adaptive_config: self.adaptive_config.clone(),
            });
        }

        // Add coroutine scheduler if provided
        #[cfg(feature = "performance")]
        {
            use crate::execution::async_execution::CoroutineExecutionState;
            if let Some(scheduler) = coroutine_scheduler {
                ctx.coroutine_state = Some(CoroutineExecutionState::with_scheduler(scheduler));
            }
        }

        ctx
    }

    /// Get JIT hot snapshot
    pub fn hot_snapshot(&self) -> Option<(AdaptiveThresholdConfig, AdaptiveThresholdStats)> {
        let snapshot = self.adaptive_snapshot.lock().ok().and_then(|g| g.clone());
        match (self.adaptive_config.clone(), snapshot) {
            (Some(cfg), Some(stats)) => Some((cfg, stats)),
            _ => None,
        }
    }

    /// Export JIT hot snapshot as JSON
    pub fn export_hot_snapshot_json(&self) -> Option<String> {
        self.hot_snapshot().map(|(cfg, stats)| {
            format!(
                "{{\"cold_threshold\":{},\"hot_threshold\":{},\"enable_adaptive\":{},\"execution_count\":{}}}",
                cfg.cold_threshold, cfg.hot_threshold, cfg.enable_adaptive, stats.execution_count
            )
        })
    }

    /// Get shared code pool
    pub fn get_code_pool(&self) -> Option<Arc<Mutex<HashMap<GuestAddr, CodePtr>>>> {
        self.code_pool.as_ref().cloned()
    }

    /// Set shared code pool
    pub fn set_shared_pool(&mut self, enable: bool) {
        if enable {
            if self.code_pool.is_none() {
                self.code_pool = Some(Arc::new(Mutex::new(HashMap::new())));
            }
        } else {
            self.code_pool = None;
        }
        self.config.enable_shared_pool = enable;
    }

    /// Get adaptive snapshot
    pub fn get_adaptive_snapshot(&self) -> Arc<Mutex<Option<AdaptiveThresholdStats>>> {
        Arc::clone(&self.adaptive_snapshot)
    }

    /// Get adaptive config
    pub fn get_adaptive_config(&self) -> Option<AdaptiveThresholdConfig> {
        self.adaptive_config.clone()
    }
}

impl Default for PerformanceContext {
    fn default() -> Self {
        Self::new()
    }
}

