use gc_optimizer::{OptimizedGc, WriteBarrierType, GcStats, GcResult};
use std::sync::Arc;
use parking_lot::RwLock;

/// GC Runtime - 集成gc-optimizer到VM运行时
///
/// 提供垃圾收集的运行时接口，与VM执行引擎集成
pub struct GcRuntime {
    /// 优化GC实例
    gc: Arc<OptimizedGc>,
    /// GC配置
    config: GcConfig,
}

/// GC配置
#[derive(Debug, Clone)]
pub struct GcConfig {
    /// 工作线程数
    pub num_workers: usize,
    /// 目标暂停时间（微秒）
    pub target_pause_us: u64,
    /// 写屏障类型
    pub barrier_type: WriteBarrierType,
}

impl Default for GcConfig {
    fn default() -> Self {
        Self {
            num_workers: num_cpus::get(),
            target_pause_us: 10_000, // 10ms目标
            barrier_type: WriteBarrierType::Atomic,
        }
    }
}

impl GcRuntime {
    /// 创建新的GC运行时
    pub fn new(config: GcConfig) -> Self {
        let gc = Arc::new(OptimizedGc::new(
            config.num_workers,
            config.target_pause_us,
            config.barrier_type,
        ));

        Self { gc, config }
    }

    /// 使用默认配置创建
    pub fn with_default() -> Self {
        Self::new(GcConfig::default())
    }

    /// 记录对象修改（写屏障）
    ///
    /// # Arguments
    /// * `addr` - 被修改对象的地址
    pub fn record_write(&self, addr: u64) {
        self.gc.record_write(addr);
    }

    /// 执行次要GC（Minor GC）
    ///
    /// 用于回收新生代对象
    ///
    /// # Arguments
    /// * `bytes_collected` - 收集的字节数
    ///
    /// # Returns
    /// GC统计信息
    pub fn collect_minor(&self, bytes_collected: u64) -> GcResult {
        self.gc.collect_minor(bytes_collected)
    }

    /// 执行主要GC（Major GC）
    ///
    /// 用于完整堆回收
    ///
    /// # Arguments
    /// * `bytes_collected` - 收集的字节数
    ///
    /// # Returns
    /// GC统计信息
    pub fn collect_major(&self, bytes_collected: u64) -> GcResult {
        self.gc.collect_major(bytes_collected)
    }

    /// 获取GC统计信息
    ///
    /// # Returns
    /// 当前GC统计信息
    pub fn get_stats(&self) -> GcStats {
        self.gc.get_stats()
    }

    /// 获取写屏障开销
    ///
    /// # Returns
    /// 写屏障开销（微秒）
    pub fn get_barrier_overhead_us(&self) -> u64 {
        self.gc.get_barrier_overhead_us()
    }

    /// 获取GC配置
    pub fn config(&self) -> &GcConfig {
        &self.config
    }
}

/// GC集成状态
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

/// GC集成管理器
///
/// 管理GC与VM执行引擎的集成状态
pub struct GcIntegrationManager {
    /// GC运行时
    gc_runtime: Arc<GcRuntime>,
    /// 集成状态
    state: Arc<RwLock<GcIntegrationState>>,
}

impl GcIntegrationManager {
    /// 创建新的GC集成管理器
    pub fn new(gc_runtime: Arc<GcRuntime>) -> Self {
        Self {
            gc_runtime,
            state: Arc::new(RwLock::new(GcIntegrationState {
                enabled: true,
                ..Default::default()
            })),
        }
    }

    /// 是否启用GC
    pub fn is_enabled(&self) -> bool {
        self.state.read().enabled
    }

    /// 启用GC
    pub fn enable(&self) {
        self.state.write().enabled = true;
    }

    /// 禁用GC
    pub fn disable(&self) {
        self.state.write().enabled = false;
    }

    /// 记录分配
    pub fn record_allocation(&self) {
        let mut state = self.state.write();
        state.total_allocations += 1;
    }

    /// 记录GC收集
    pub fn record_collection(&self) {
        let mut state = self.state.write();
        state.total_collections += 1;
        state.last_gc_timestamp = Some(std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64);
    }

    /// 获取集成状态
    pub fn get_state(&self) -> GcIntegrationState {
        self.state.read().clone()
    }

    /// 获取GC运行时
    pub fn gc_runtime(&self) -> &Arc<GcRuntime> {
        &self.gc_runtime
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gc_runtime_creation() {
        let gc_runtime = GcRuntime::with_default();
        let stats = gc_runtime.get_stats();

        assert_eq!(stats.minor_collections, 0);
        assert_eq!(stats.major_collections, 0);
    }

    #[test]
    fn test_gc_runtime_minor_collection() {
        let gc_runtime = GcRuntime::with_default();

        let result = gc_runtime.collect_minor(1024);
        assert!(result.is_ok());

        let stats = gc_runtime.get_stats();
        assert_eq!(stats.minor_collections, 1);
    }

    #[test]
    fn test_gc_runtime_major_collection() {
        let gc_runtime = GcRuntime::with_default();

        let result = gc_runtime.collect_major(4096);
        assert!(result.is_ok());

        let stats = gc_runtime.get_stats();
        assert_eq!(stats.major_collections, 1);
    }

    #[test]
    fn test_gc_write_barrier() {
        let gc_runtime = GcRuntime::with_default();

        for i in 0..100 {
            gc_runtime.record_write(i * 16);
        }

        let overhead = gc_runtime.get_barrier_overhead_us();
        assert!(overhead > 0);
        assert!(overhead < 100);
    }

    #[test]
    fn test_gc_integration_manager() {
        let gc_runtime = Arc::new(GcRuntime::with_default());
        let manager = GcIntegrationManager::new(gc_runtime);

        assert!(manager.is_enabled());

        manager.disable();
        assert!(!manager.is_enabled());

        manager.enable();
        assert!(manager.is_enabled());
    }

    #[test]
    fn test_gc_integration_state() {
        let gc_runtime = Arc::new(GcRuntime::with_default());
        let manager = GcIntegrationManager::new(gc_runtime);

        manager.record_allocation();
        manager.record_allocation();

        let state = manager.get_state();
        assert_eq!(state.total_allocations, 2);
        assert!(state.enabled);
    }
}
