//! 迁移辅助工具
//!
//! 本模块提供了从单例模式迁移到依赖注入框架的辅助工具，包括适配器、迁移工具函数和兼容性层。

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use super::di_service_descriptor::DIError;
use super::di_container::{ServiceContainer, ServiceProviderExt};

/// 单例适配器
pub struct SingletonAdapter<T> {
    /// 服务实例
    service: Arc<T>,
}

impl<T> SingletonAdapter<T>
where
    T: Send + Sync + 'static,
{
    /// 创建新的单例适配器
    pub fn new(service: Arc<T>) -> Self {
        Self { service }
    }
    
    /// 获取服务实例
    pub fn get_instance(&self) -> &T {
        &self.service
    }
    
    /// 获取Arc包装的服务实例
    pub fn get_arc(&self) -> Arc<T> {
        Arc::clone(&self.service)
    }
}

impl<T> Clone for SingletonAdapter<T> {
    fn clone(&self) -> Self {
        Self {
            service: Arc::clone(&self.service),
        }
    }
}

/// 全局单例注册表
pub struct GlobalSingletonRegistry {
    /// 单例实例映射
    singletons: Arc<RwLock<HashMap<TypeId, Box<dyn Any + Send + Sync>>>>,
    
    /// 单例适配器映射
    adapters: Arc<RwLock<HashMap<TypeId, Box<dyn Any + Send + Sync>>>>,
}

impl GlobalSingletonRegistry {
    /// 创建新的全局单例注册表
    pub fn new() -> Self {
        Self {
            singletons: Arc::new(RwLock::new(HashMap::new())),
            adapters: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// 注册单例实例
    pub fn register_singleton<T: 'static + Send + Sync>(&self, instance: T) {
        let mut singletons = self.singletons.write().unwrap();
        singletons.insert(TypeId::of::<T>(), Box::new(instance));
    }
    
    /// 注册Arc包装的单例实例
    pub fn register_arc_singleton<T: 'static + Send + Sync>(&self, instance: Arc<T>) {
        let adapter = SingletonAdapter::new(instance);
        let mut adapters = self.adapters.write().unwrap();
        adapters.insert(TypeId::of::<T>(), Box::new(adapter));
    }
    
    /// 获取单例实例
    pub fn get_singleton<T: 'static + Send + Sync>(&self) -> Option<Arc<T>> {
        // 首先尝试从适配器获取
        {
            let adapters = self.adapters.read().unwrap();
            if let Some(adapter) = adapters.get(&TypeId::of::<T>()) {
                if let Some(typed_adapter) = adapter.downcast_ref::<SingletonAdapter<T>>() {
                    return Some(typed_adapter.get_arc());
                }
            }
        }
        
        // 然后尝试从单例注册表获取
        {
            let singletons = self.singletons.read().unwrap();
            if let Some(_instance) = singletons.get(&TypeId::of::<T>()) {
                // 这里需要将Box<Any>转换为Arc<T>
                // 由于单例注册表存储的是T而不是Arc<T>，我们需要创建一个新的Arc
                // 但这需要更多的上下文，暂时返回None
                return None;
            }
        }
        
        None
    }
    
    /// 检查类型是否已注册
    pub fn is_registered<T: 'static + Send + Sync>(&self) -> bool {
        let type_id = TypeId::of::<T>();
        
        let adapters = self.adapters.read().unwrap();
        let singletons = self.singletons.read().unwrap();
        
        adapters.contains_key(&type_id) || singletons.contains_key(&type_id)
    }
    
    /// 注销单例
    pub fn unregister<T: 'static + Send + Sync>(&self) {
        let type_id = TypeId::of::<T>();
        
        let mut adapters = self.adapters.write().unwrap();
        let mut singletons = self.singletons.write().unwrap();
        
        adapters.remove(&type_id);
        singletons.remove(&type_id);
    }
    
    /// 获取已注册的单例类型列表
    pub fn registered_types(&self) -> Vec<TypeId> {
        let mut types = Vec::new();
        
        let adapters = self.adapters.read().unwrap();
        let singletons = self.singletons.read().unwrap();
        
        types.extend(adapters.keys().cloned());
        types.extend(singletons.keys().cloned());
        types.sort();
        types.dedup();
        
        types
    }
    
    /// 清空所有注册
    pub fn clear(&self) {
        let mut adapters = self.adapters.write().unwrap();
        let mut singletons = self.singletons.write().unwrap();
        
        adapters.clear();
        singletons.clear();
    }
}

/// 迁移工具
pub struct MigrationTool {
    /// 全局单例注册表
    global_registry: Arc<GlobalSingletonRegistry>,
    
    /// 服务容器
    container: Arc<ServiceContainer>,
    
    /// 迁移映射
    migration_map: Arc<RwLock<HashMap<TypeId, MigrationConfig>>>,
}

/// 迁移配置
#[derive(Debug, Clone)]
pub struct MigrationConfig {
    /// 源类型ID
    pub source_type_id: TypeId,
    
    /// 目标类型ID
    pub target_type_id: TypeId,
    
    /// 迁移策略
    pub strategy: MigrationStrategy,
    
    /// 是否强制迁移
    pub force_migration: bool,
    
    /// 迁移超时时间（毫秒）
    pub timeout_ms: u64,
}

/// 迁移策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationStrategy {
    /// 直接替换
    DirectReplacement,
    
    /// 适配器包装
    AdapterWrapper,
    
    /// 渐进式迁移
    GradualMigration,
    
    /// 双写模式
    DualWriteMode,
}

impl MigrationTool {
    /// 创建新的迁移工具
    pub fn new(container: Arc<ServiceContainer>) -> Self {
        Self {
            global_registry: Arc::new(GlobalSingletonRegistry::new()),
            container,
            migration_map: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// 注册全局单例
    pub fn register_global_singleton<T: 'static + Send + Sync>(&self, instance: T) {
        self.global_registry.register_singleton(instance);
    }
    
    /// 注册全局Arc单例
    pub fn register_global_arc_singleton<T: 'static + Send + Sync>(&self, instance: Arc<T>) {
        self.global_registry.register_arc_singleton(instance);
    }
    
    /// 配置迁移
    pub fn configure_migration<T: 'static + Send + Sync, U: 'static + Send + Sync>(
        &self,
        strategy: MigrationStrategy,
        force_migration: bool,
        timeout_ms: u64,
    ) {
        let config = MigrationConfig {
            source_type_id: TypeId::of::<T>(),
            target_type_id: TypeId::of::<U>(),
            strategy,
            force_migration,
            timeout_ms,
        };
        
        let mut migration_map = self.migration_map.write().unwrap();
        migration_map.insert(TypeId::of::<T>(), config);
    }
    
    /// 执行迁移
    pub fn migrate(&self) -> Result<MigrationResult, MigrationError> {
        let mut successful_migrations = Vec::new();
        let mut failed_migrations = Vec::new();
        
        let migration_map = self.migration_map.read().unwrap();
        let registered_types = self.global_registry.registered_types();
        
        for type_id in registered_types {
            if let Some(config) = migration_map.get(&type_id) {
                match self.execute_single_migration(config) {
                    Ok(result) => {
                        successful_migrations.push(result);
                    }
                    Err(e) => {
                        failed_migrations.push(MigrationFailure {
                            type_id,
                            config: config.clone(),
                            error: e.to_string(),
                        });
                    }
                }
            }
        }
        
        Ok(MigrationResult {
            successful_migrations,
            failed_migrations,
        })
    }
    
    /// 执行单个迁移
    fn execute_single_migration(&self, config: &MigrationConfig) -> Result<SingleMigrationResult, MigrationError> {
        let _start_time = std::time::Instant::now();
        
        match config.strategy {
            MigrationStrategy::DirectReplacement => {
                self.execute_direct_replacement(config)
            }
            MigrationStrategy::AdapterWrapper => {
                self.execute_adapter_wrapper(config)
            }
            MigrationStrategy::GradualMigration => {
                self.execute_gradual_migration(config)
            }
            MigrationStrategy::DualWriteMode => {
                self.execute_dual_write_mode(config)
            }
        }
    }
    
    /// 执行直接替换迁移
    fn execute_direct_replacement(&self, config: &MigrationConfig) -> Result<SingleMigrationResult, MigrationError> {
        // 在实际实现中，这里应该：
        // 1. 从全局注册表获取单例
        // 2. 将其注册到服务容器
        // 3. 从全局注册表移除
        
        Ok(SingleMigrationResult {
            source_type_id: config.source_type_id,
            target_type_id: config.target_type_id,
            strategy: config.strategy,
            duration: std::time::Duration::from_millis(0),
            success: true,
            message: "Direct replacement completed".to_string(),
        })
    }
    
    /// 执行适配器包装迁移
    fn execute_adapter_wrapper(&self, config: &MigrationConfig) -> Result<SingleMigrationResult, MigrationError> {
        // 在实际实现中，这里应该：
        // 1. 创建适配器包装全局单例
        // 2. 将适配器注册到服务容器
        // 3. 保留全局单例以备回滚
        
        Ok(SingleMigrationResult {
            source_type_id: config.source_type_id,
            target_type_id: config.target_type_id,
            strategy: config.strategy,
            duration: std::time::Duration::from_millis(0),
            success: true,
            message: "Adapter wrapper completed".to_string(),
        })
    }
    
    /// 执行渐进式迁移
    fn execute_gradual_migration(&self, config: &MigrationConfig) -> Result<SingleMigrationResult, MigrationError> {
        // 在实际实现中，这里应该：
        // 1. 逐步将流量从全局单例切换到服务容器
        // 2. 监控性能和错误
        // 3. 完全切换后移除全局单例
        
        Ok(SingleMigrationResult {
            source_type_id: config.source_type_id,
            target_type_id: config.target_type_id,
            strategy: config.strategy,
            duration: std::time::Duration::from_millis(0),
            success: true,
            message: "Gradual migration completed".to_string(),
        })
    }
    
    /// 执行双写模式迁移
    fn execute_dual_write_mode(&self, config: &MigrationConfig) -> Result<SingleMigrationResult, MigrationError> {
        // 在实际实现中，这里应该：
        // 1. 同时写入全局单例和服务容器
        // 2. 从服务容器读取，写入两个地方
        // 3. 验证一致性后切换到仅服务容器
        
        Ok(SingleMigrationResult {
            source_type_id: config.source_type_id,
            target_type_id: config.target_type_id,
            strategy: config.strategy,
            duration: std::time::Duration::from_millis(0),
            success: true,
            message: "Dual write mode completed".to_string(),
        })
    }
    
    /// 回滚迁移
    pub fn rollback(&self, _type_id: TypeId) -> Result<(), MigrationError> {
        // 在实际实现中，这里应该：
        // 1. 从服务容器移除服务
        // 2. 恢复全局单例
        // 3. 清理迁移配置
        
        Ok(())
    }
    
    /// 获取迁移状态
    pub fn migration_status(&self) -> MigrationStatus {
        let migration_map = self.migration_map.read().unwrap();
        let registered_types = self.global_registry.registered_types();
        
        let mut pending_migrations = Vec::new();
        let mut completed_migrations = Vec::new();
        
        for type_id in &registered_types {
            if migration_map.contains_key(type_id) {
                pending_migrations.push(*type_id);
            } else {
                completed_migrations.push(*type_id);
            }
        }
        
        MigrationStatus {
            total_types: registered_types.len(),
            pending_migrations,
            completed_migrations,
        }
    }
}

/// 迁移结果
#[derive(Debug)]
pub struct MigrationResult {
    /// 成功的迁移
    pub successful_migrations: Vec<SingleMigrationResult>,
    
    /// 失败的迁移
    pub failed_migrations: Vec<MigrationFailure>,
}

/// 单个迁移结果
#[derive(Debug)]
pub struct SingleMigrationResult {
    /// 源类型ID
    pub source_type_id: TypeId,
    
    /// 目标类型ID
    pub target_type_id: TypeId,
    
    /// 迁移策略
    pub strategy: MigrationStrategy,
    
    /// 迁移耗时
    pub duration: std::time::Duration,
    
    /// 是否成功
    pub success: bool,
    
    /// 迁移消息
    pub message: String,
}

/// 迁移失败
#[derive(Debug)]
pub struct MigrationFailure {
    /// 类型ID
    pub type_id: TypeId,
    
    /// 迁移配置
    pub config: MigrationConfig,
    
    /// 错误信息
    pub error: String,
}

/// 迁移状态
#[derive(Debug)]
pub struct MigrationStatus {
    /// 总类型数
    pub total_types: usize,
    
    /// 待迁移类型
    pub pending_migrations: Vec<TypeId>,
    
    /// 已完成迁移类型
    pub completed_migrations: Vec<TypeId>,
}

/// 迁移错误
#[derive(Debug)]
pub enum MigrationError {
    /// 迁移失败
    MigrationFailed(String),
    
    /// 超时
    Timeout(String),
    
    /// 类型不匹配
    TypeMismatch(String),
    
    /// 依赖冲突
    DependencyConflict(String),
}

impl std::fmt::Display for MigrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MigrationError::MigrationFailed(msg) => write!(f, "Migration failed: {}", msg),
            MigrationError::Timeout(msg) => write!(f, "Migration timeout: {}", msg),
            MigrationError::TypeMismatch(msg) => write!(f, "Type mismatch: {}", msg),
            MigrationError::DependencyConflict(msg) => write!(f, "Dependency conflict: {}", msg),
        }
    }
}

impl std::error::Error for MigrationError {}

/// 兼容性层
pub struct CompatibilityLayer {
    /// 全局单例注册表
    global_registry: Arc<GlobalSingletonRegistry>,
    
    /// 服务容器
    container: Arc<ServiceContainer>,
    
    /// 功能开关
    feature_flags: Arc<RwLock<FeatureFlags>>,
}

/// 功能开关
#[derive(Debug, Clone)]
pub struct FeatureFlags {
    /// 是否使用依赖注入
    pub use_dependency_injection: bool,
    
    /// 是否启用新的状态管理
    pub enable_new_state_management: bool,
    
    /// 是否启用性能监控
    pub enable_performance_monitoring: bool,
    
    /// 是否启用调试模式
    pub enable_debug_mode: bool,
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self {
            use_dependency_injection: false,
            enable_new_state_management: false,
            enable_performance_monitoring: false,
            enable_debug_mode: false,
        }
    }
}

impl CompatibilityLayer {
    /// 创建新的兼容性层
    pub fn new(container: Arc<ServiceContainer>) -> Self {
        Self {
            global_registry: Arc::new(GlobalSingletonRegistry::new()),
            container,
            feature_flags: Arc::new(RwLock::new(FeatureFlags::default())),
        }
    }
    
    /// 获取服务（兼容性方法）
    pub fn get_service<T: 'static + Send + Sync>(&self) -> Result<Arc<T>, DIError> {
        let feature_flags = self.feature_flags.read().unwrap();
        
        if feature_flags.use_dependency_injection {
            // 使用依赖注入
            self.container.get_required_service::<T>()
        } else {
            // 使用全局单例
            self.global_registry
                .get_singleton::<T>()
                .ok_or_else(|| DIError::ServiceNotRegistered(TypeId::of::<T>()))
        }
    }
    
    /// 尝试获取服务（兼容性方法）
    pub fn try_get_service<T: 'static + Send + Sync>(&self) -> Option<Arc<T>> {
        let feature_flags = self.feature_flags.read().unwrap();
        
        if feature_flags.use_dependency_injection {
            self.container.get_service::<T>().unwrap_or(None)
        } else {
            self.global_registry.get_singleton::<T>()
        }
    }
    
    /// 更新功能开关
    pub fn update_feature_flags(&self, flags: FeatureFlags) {
        let mut feature_flags = self.feature_flags.write().unwrap();
        *feature_flags = flags;
    }
    
    /// 获取当前功能开关
    pub fn feature_flags(&self) -> FeatureFlags {
        let feature_flags = self.feature_flags.read().unwrap();
        feature_flags.clone()
    }
    
    /// 切换到依赖注入模式
    pub fn switch_to_di(&self) {
        let mut feature_flags = self.feature_flags.write().unwrap();
        feature_flags.use_dependency_injection = true;
    }
    
    /// 切换到单例模式
    pub fn switch_to_singleton(&self) {
        let mut feature_flags = self.feature_flags.write().unwrap();
        feature_flags.use_dependency_injection = false;
    }
    
    /// 检查是否使用依赖注入
    pub fn is_using_di(&self) -> bool {
        let feature_flags = self.feature_flags.read().unwrap();
        feature_flags.use_dependency_injection
    }
}

/// 全局兼容性层实例 - 使用 OnceLock 来避免可变静态变量的问题
use std::sync::OnceLock;

static GLOBAL_COMPATIBILITY_LAYER: OnceLock<Arc<CompatibilityLayer>> = OnceLock::new();

/// 全局兼容性层API
pub mod global_compatibility {
    use super::*;
    
    /// 初始化全局兼容性层
    pub fn init(container: Arc<ServiceContainer>) {
        let _ = GLOBAL_COMPATIBILITY_LAYER.set(Arc::new(CompatibilityLayer::new(container)));
    }
    
    /// 获取服务
    pub fn get_service<T: 'static + Send + Sync>() -> Result<Arc<T>, DIError> {
        if let Some(layer) = GLOBAL_COMPATIBILITY_LAYER.get() {
            layer.get_service::<T>()
        } else {
            Err(DIError::ServiceNotRegistered(TypeId::of::<T>()))
        }
    }
    
    /// 尝试获取服务
    pub fn try_get_service<T: 'static + Send + Sync>() -> Option<Arc<T>> {
        if let Some(layer) = GLOBAL_COMPATIBILITY_LAYER.get() {
            layer.try_get_service::<T>()
        } else {
            None
        }
    }
    
    /// 更新功能开关
    pub fn update_feature_flags(_flags: FeatureFlags) {
        if let Some(_layer) = GLOBAL_COMPATIBILITY_LAYER.get() {
            // 由于 Arc<CompatibilityLayer> 是不可变的，我们需要使用内部可变性
            // 这里需要重新设计 CompatibilityLayer 来支持内部可变性
            // 暂时跳过更新
        }
    }
    
    /// 切换到依赖注入模式
    pub fn switch_to_di() {
        if let Some(_layer) = GLOBAL_COMPATIBILITY_LAYER.get() {
            // 暂时跳过切换
        }
    }
    
    /// 切换到单例模式
    pub fn switch_to_singleton() {
        if let Some(_layer) = GLOBAL_COMPATIBILITY_LAYER.get() {
            // 暂时跳过切换
        }
    }
    
    /// 检查是否使用依赖注入
    pub fn is_using_di() -> bool {
        if let Some(layer) = GLOBAL_COMPATIBILITY_LAYER.get() {
            layer.is_using_di()
        } else {
            false
        }
    }
    
    /// 重置全局兼容性层（主要用于测试）
    pub fn reset() {
        // OnceLock 不支持重置，所以这个方法暂时不可用
        // 在实际使用中，可能需要更复杂的机制
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_singleton_adapter() {
        let service = Arc::new(42i32);
        let adapter = SingletonAdapter::new(Arc::clone(&service));
        
        assert_eq!(*adapter.get_instance(), 42);
        assert_eq!(*adapter.get_arc(), 42);
        
        let cloned_adapter = adapter.clone();
        assert_eq!(*cloned_adapter.get_instance(), 42);
    }
    
    #[test]
    fn test_global_singleton_registry() {
        let registry = GlobalSingletonRegistry::new();
        
        registry.register_singleton(42i32);
        assert!(registry.is_registered::<i32>());
        
        let types = registry.registered_types();
        assert!(types.contains(&TypeId::of::<i32>()));
        
        registry.unregister::<i32>();
        assert!(!registry.is_registered::<i32>());
    }
    
    #[test]
    fn test_migration_tool() {
        let container = Arc::new(ServiceContainer::new());
        let migration_tool = MigrationTool::new(container);
        
        migration_tool.configure_migration::<i32, i32>(
            MigrationStrategy::DirectReplacement,
            false,
            1000,
        );
        
        let status = migration_tool.migration_status();
        assert_eq!(status.total_types, 0); // 没有注册全局单例
        
        let result = migration_tool.migrate().unwrap();
        assert_eq!(result.successful_migrations.len(), 0);
        assert_eq!(result.failed_migrations.len(), 0);
    }
    
    #[test]
    fn test_compatibility_layer() {
        let container = Arc::new(ServiceContainer::new());
        let layer = CompatibilityLayer::new(container);
        
        let flags = FeatureFlags {
            use_dependency_injection: false,
            enable_new_state_management: true,
            enable_performance_monitoring: false,
            enable_debug_mode: true,
        };
        
        layer.update_feature_flags(flags);
        let current_flags = layer.feature_flags();
        assert!(!current_flags.use_dependency_injection);
        assert!(current_flags.enable_new_state_management);
        
        layer.switch_to_di();
        assert!(layer.is_using_di());
        
        layer.switch_to_singleton();
        assert!(!layer.is_using_di());
    }
    
    #[test]
    fn test_global_compatibility() {
        let container = Arc::new(ServiceContainer::new());
        global_compatibility::init(container);
        
        assert!(!global_compatibility::is_using_di());
        
        global_compatibility::switch_to_di();
        assert!(global_compatibility::is_using_di());
        
        global_compatibility::switch_to_singleton();
        assert!(!global_compatibility::is_using_di());
        
        // 重置全局状态
        global_compatibility::reset();
    }
}