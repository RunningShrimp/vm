//! 依赖注入配置模块
//!
//! 配置所有基础设施层实现的依赖注入，将领域层接口与基础设施层实现连接起来。

use std::collections::HashMap;
use std::sync::Arc;

use vm_core::domain::{CacheManager, OptimizationStrategy, RegisterAllocator, TlbManager};
use vm_core::domain_event_bus::DomainEventBus;
use vm_core::domain_services::{
    CacheManagementDomainService, OptimizationPipelineDomainService,
    RegisterAllocationDomainService, TlbManagementDomainService,
};
use vm_engine::jit::cache::GenericCacheManager;
use vm_engine::jit::optimizer_strategy::OptimizationStrategyImpl;
use vm_engine::jit::register_allocator_adapter::{AllocationStrategy, RegisterAllocatorAdapter};
use vm_mem::tlb::management::multilevel::MultiLevelTlbManager;

/// 缓存管理器类型别名（简化复杂类型）
pub type CacheManagerRef = Arc<std::sync::Mutex<dyn CacheManager<u64, Vec<u8>>>>;

/// 服务容器
///
/// 管理所有领域服务的依赖注入配置。
pub struct ServiceContainer {
    /// TLB 管理器
    pub tlb_manager: Arc<std::sync::Mutex<dyn TlbManager>>,
    /// 缓存管理器（按 tier 组织）
    pub cache_managers: HashMap<String, CacheManagerRef>,
    /// 优化策略
    pub optimization_strategy: Arc<dyn OptimizationStrategy>,
    /// 寄存器分配器
    pub register_allocator: Arc<std::sync::Mutex<dyn RegisterAllocator>>,
    /// 事件总线
    pub event_bus: Arc<DomainEventBus>,
}

impl ServiceContainer {
    /// 创建新的服务容器并初始化所有基础设施层实现
    pub fn new() -> Self {
        // 创建事件总线
        let event_bus = Arc::new(DomainEventBus::new());

        // 创建 TLB 管理器
        let tlb_manager: Arc<std::sync::Mutex<dyn TlbManager>> =
            Arc::new(std::sync::Mutex::new(MultiLevelTlbManager::new()));

        // 创建缓存管理器（L1, L2, L3）
        let mut cache_managers: HashMap<String, CacheManagerRef> = HashMap::new();

        // L1 缓存（32KB）
        let l1_cache: CacheManagerRef =
            Arc::new(std::sync::Mutex::new(GenericCacheManager::with_policy(
                32 * 1024,
                vm_engine::jit::cache::manager::CacheReplacementPolicy::LRU,
            )));
        cache_managers.insert("L1".to_string(), l1_cache);

        // L2 缓存（256KB）
        let l2_cache: CacheManagerRef =
            Arc::new(std::sync::Mutex::new(GenericCacheManager::with_policy(
                256 * 1024,
                vm_engine::jit::cache::manager::CacheReplacementPolicy::LRU,
            )));
        cache_managers.insert("L2".to_string(), l2_cache);

        // L3 缓存（2MB）
        let l3_cache: CacheManagerRef =
            Arc::new(std::sync::Mutex::new(GenericCacheManager::with_policy(
                2 * 1024 * 1024,
                vm_engine::jit::cache::manager::CacheReplacementPolicy::LFU,
            )));
        cache_managers.insert("L3".to_string(), l3_cache);

        // 创建优化策略（默认优化级别 2）
        let optimization_strategy: Arc<dyn OptimizationStrategy> =
            Arc::new(OptimizationStrategyImpl::new(2));

        // 创建寄存器分配器（使用混合策略）
        let register_allocator: Arc<std::sync::Mutex<dyn RegisterAllocator>> = Arc::new(
            std::sync::Mutex::new(RegisterAllocatorAdapter::new(AllocationStrategy::Hybrid)),
        );

        Self {
            tlb_manager,
            cache_managers,
            optimization_strategy,
            register_allocator,
            event_bus,
        }
    }

    /// 创建 TLB 管理领域服务
    pub fn create_tlb_management_service(&self) -> TlbManagementDomainService {
        TlbManagementDomainService::new(self.event_bus.clone(), self.tlb_manager.clone())
    }

    /// 创建缓存管理领域服务
    pub fn create_cache_management_service(&self) -> CacheManagementDomainService {
        use vm_core::domain_services::cache_management_service::CacheManagementConfig;

        let config = CacheManagementConfig::default();
        CacheManagementDomainService::new(config, self.cache_managers.clone())
    }

    /// 创建优化管道领域服务
    pub fn create_optimization_pipeline_service(&self) -> OptimizationPipelineDomainService {
        OptimizationPipelineDomainService::new(
            self.optimization_strategy.clone(),
            Some(self.event_bus.clone()),
        )
    }

    /// 创建寄存器分配领域服务
    pub fn create_register_allocation_service(&self) -> RegisterAllocationDomainService {
        use vm_core::domain_services::register_allocation_service::RegisterAllocationConfig;

        let config = RegisterAllocationConfig::default();
        RegisterAllocationDomainService::new(
            self.register_allocator.clone(),
            config,
            Some(self.event_bus.clone()),
        )
    }

    /// 获取TLB管理器（形成逻辑闭环）
    pub fn tlb_manager(&self) -> &Arc<std::sync::Mutex<dyn TlbManager>> {
        &self.tlb_manager
    }

    /// 获取缓存管理器（形成逻辑闭环）
    pub fn cache_managers(&self) -> &HashMap<String, CacheManagerRef> {
        &self.cache_managers
    }

    /// 获取优化策略（形成逻辑闭环）
    pub fn optimization_strategy(&self) -> &Arc<dyn OptimizationStrategy> {
        &self.optimization_strategy
    }

    /// 获取寄存器分配器（形成逻辑闭环）
    pub fn register_allocator(&self) -> &Arc<std::sync::Mutex<dyn RegisterAllocator>> {
        &self.register_allocator
    }

    /// 获取事件总线（形成逻辑闭环）
    pub fn event_bus(&self) -> &Arc<DomainEventBus> {
        &self.event_bus
    }
}

impl Default for ServiceContainer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_service_container_creation() {
        let container = ServiceContainer::new();
        assert_eq!(container.cache_managers.len(), 3);
    }

    #[tokio::test]
    async fn test_create_tlb_management_service() {
        let container = ServiceContainer::new();
        let _service = container.create_tlb_management_service();
        // Service created successfully
        assert!(true);
    }

    #[tokio::test]
    async fn test_create_cache_management_service() {
        let container = ServiceContainer::new();
        let _service = container.create_cache_management_service();
        // Service created successfully
        assert!(true);
    }
}
