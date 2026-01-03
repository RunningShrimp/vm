//! GC Runtime for VM Boot
//!
//! Re-exports GC functionality from vm-runtime and vm-gc.
//! This module provides boot-time GC configuration and initialization.

use std::sync::Arc;

/// Re-export GC types from vm-core::runtime
pub use vm_core::runtime::{
    WriteBarrierType,
    gc::{GcRuntime, GcRuntimeStats},
};

/// Re-export additional GC types from vm-gc
pub use vm_gc::{
    BaseIncrementalGc as IncrementalGc, IncrementalPhase, IncrementalProgress, OptimizedGc,
};

/// Type aliases for backwards compatibility
pub type ConcurrentGC = vm_gc::ConcurrentGC;
pub type GCColor = vm_gc::GCColor;
pub type GCStats = vm_gc::GcStats;
pub type GcResult = std::result::Result<(), vm_core::VmError>;
pub type GcError = vm_core::VmError;
pub type ParallelMarker = vm_gc::ParallelMarker;
pub type AdaptiveQuota = vm_gc::AdaptiveQuota;
pub type AllocStats = vm_gc::AllocStats;

/// Type alias for backwards compatibility
pub type GcConfig = BootGcConfig;

/// Boot-time GC configuration
#[derive(Debug, Clone)]
pub struct BootGcConfig {
    /// Number of GC worker threads
    pub num_workers: usize,
    /// Target pause time in microseconds
    pub target_pause_us: u64,
    /// Write barrier type
    pub barrier_type: WriteBarrierType,
    /// Enable incremental GC
    pub enable_incremental: bool,
}

impl Default for BootGcConfig {
    fn default() -> Self {
        Self {
            num_workers: num_cpus::get(),
            target_pause_us: 10_000, // 10ms target
            barrier_type: WriteBarrierType::Atomic,
            enable_incremental: true,
        }
    }
}

impl BootGcConfig {
    /// Create configuration optimized for production
    pub fn for_production() -> Self {
        Self {
            num_workers: num_cpus::get(),
            target_pause_us: 10_000,
            barrier_type: WriteBarrierType::Atomic,
            enable_incremental: true,
        }
    }

    /// Create configuration optimized for development
    pub fn for_development() -> Self {
        Self {
            num_workers: 2,
            target_pause_us: 50_000, // More lenient for development
            barrier_type: WriteBarrierType::Atomic,
            enable_incremental: true,
        }
    }

    /// Create configuration optimized for testing
    pub fn for_testing() -> Self {
        Self {
            num_workers: 1,
            target_pause_us: 100_000, // Very lenient for testing
            barrier_type: WriteBarrierType::Atomic,
            enable_incremental: false, // Disable for simpler testing
        }
    }

    /// Create GcRuntime from this configuration
    pub fn create_runtime(&self) -> GcRuntime {
        GcRuntime::new(self.num_workers, self.target_pause_us, self.barrier_type)
    }
}

/// GC集成状态 (backwards compatibility stub)
#[derive(Debug, Clone, Default)]
pub struct GcIntegrationState {
    /// 是否已启用
    pub enabled: bool,
    /// 总分配次数
    pub total_allocations: u64,
    /// 总回收次数
    pub total_collections: u64,
    /// 最后GC时间戳
    pub last_gc_timestamp: Option<u64>,
}

/// GC集成管理器 (backwards compatibility stub)
///
/// Note: This is a simplified stub for backwards compatibility.
/// The actual integration is now handled by vm-runtime::gc::GcRuntime.
pub struct GcIntegrationManager {
    gc_runtime: Arc<GcRuntime>,
    state: Arc<parking_lot::RwLock<GcIntegrationState>>,
}

impl GcIntegrationManager {
    pub fn new(gc_runtime: Arc<GcRuntime>) -> Self {
        Self {
            gc_runtime,
            state: Arc::new(parking_lot::RwLock::new(GcIntegrationState {
                enabled: true,
                ..Default::default()
            })),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.state.read().enabled
    }

    pub fn enable(&self) {
        self.state.write().enabled = true;
    }

    pub fn disable(&self) {
        self.state.write().enabled = false;
    }

    pub fn record_allocation(&self) {
        let mut state = self.state.write();
        state.total_allocations += 1;
    }

    pub fn record_collection(&self) {
        let mut state = self.state.write();
        state.total_collections += 1;
        state.last_gc_timestamp = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_micros() as u64,
        );
    }

    pub fn get_state(&self) -> GcIntegrationState {
        self.state.read().clone()
    }

    pub fn gc_runtime(&self) -> &Arc<GcRuntime> {
        &self.gc_runtime
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boot_gc_config_default() {
        let config = BootGcConfig::default();
        assert!(config.num_workers > 0);
        assert!(config.enable_incremental);
    }

    #[test]
    fn test_boot_gc_config_production() {
        let config = BootGcConfig::for_production();
        assert!(config.num_workers > 0);
        assert!(config.enable_incremental);
        assert_eq!(config.target_pause_us, 10_000);
    }

    #[test]
    fn test_boot_gc_config_development() {
        let config = BootGcConfig::for_development();
        assert_eq!(config.num_workers, 2);
        assert!(config.enable_incremental);
        assert_eq!(config.target_pause_us, 50_000);
    }

    #[test]
    fn test_boot_gc_config_testing() {
        let config = BootGcConfig::for_testing();
        assert_eq!(config.num_workers, 1);
        assert!(!config.enable_incremental);
    }

    #[test]
    fn test_create_runtime_from_config() {
        let config = BootGcConfig::for_testing();
        let runtime = config.create_runtime();
        assert!(runtime.is_enabled());
    }

    #[test]
    fn test_gc_integration_manager() {
        let config = BootGcConfig::for_testing();
        let runtime = Arc::new(config.create_runtime());
        let manager = GcIntegrationManager::new(runtime);

        assert!(manager.is_enabled());

        manager.disable();
        assert!(!manager.is_enabled());

        manager.enable();
        assert!(manager.is_enabled());

        manager.record_allocation();
        manager.record_allocation();

        let state = manager.get_state();
        assert_eq!(state.total_allocations, 2);
        assert!(state.enabled);
    }

    #[test]
    fn test_gc_integration_state() {
        let config = BootGcConfig::for_testing();
        let runtime = Arc::new(config.create_runtime());
        let manager = GcIntegrationManager::new(runtime);

        manager.record_allocation();
        manager.record_allocation();

        let state = manager.get_state();
        assert_eq!(state.total_allocations, 2);
        assert!(state.enabled);
    }
}
