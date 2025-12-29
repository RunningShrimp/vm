//! 依赖注入模块
//!
//! 本模块提供了依赖注入框架的公共接口和统一导出。

pub use super::di_builder::{
    ContainerBuilder, ContainerBuilderFactory, ContainerConfig,
    ContainerOptions,
};
pub use super::di_container::{ServiceContainer, ScopedServiceProvider, ContainerStats, ServiceProviderExt};
pub use super::di_injector::{
    DependencyInjector, DIConstruct, DIInject, DIMethodInject, InjectionPoint, InjectionResult,
    InjectionStrategy, InjectorStats, MethodInfo, PropertyInfo, TypeInfo, TypeInfoBuilder,
};
pub use super::di_registry::{
    RegistryConfig, RegistryStats, ServiceConfig, ServiceRegistry, ServiceRegistryBuilder,
};
pub use super::di_resolver::{
    DependencyGraph, DependencyNode, DependencyResolver, ResolutionStrategy, ResolverStats,
};
pub use super::di_service_descriptor::{
    DIError, GenericServiceDescriptor, Scope, ScopeManager, ServiceDescriptor, ServiceInstance,
    ServiceLifetime, ServiceProvider,
};

// 重新导出常用类型和函数
use std::any::TypeId;
use std::sync::Arc;

/// 依赖注入框架的便捷API
pub mod prelude {
    pub use super::{
        ContainerBuilder, ServiceContainer, ServiceProvider, ServiceLifetime, DIError,
        register_singleton, register_transient, register_scoped, register_instance, register_factory,
    };
}

/// 便捷函数：注册单例服务
pub fn register_singleton<T: 'static + Send + Sync>(
    builder: ContainerBuilder,
) -> ContainerBuilder {
    builder.register_singleton::<T>()
}

/// 便捷函数：注册瞬态服务
pub fn register_transient<T: 'static + Send + Sync>(
    builder: ContainerBuilder,
) -> ContainerBuilder {
    builder.register_transient::<T>()
}

/// 便捷函数：注册作用域服务
pub fn register_scoped<T: 'static + Send + Sync>(
    builder: ContainerBuilder,
) -> ContainerBuilder {
    builder.register_scoped::<T>()
}

/// 便捷函数：注册服务实例
pub fn register_instance<T: 'static + Send + Sync>(
    builder: ContainerBuilder,
    instance: T,
) -> ContainerBuilder {
    builder.register_instance(instance)
}

/// 便捷函数：注册工厂函数
pub fn register_factory<T: 'static + Send + Sync, F>(
    builder: ContainerBuilder,
    factory: F,
) -> ContainerBuilder
where
    F: Fn(&dyn ServiceProvider) -> Result<T, DIError> + Send + Sync + 'static,
{
    builder.register_factory(factory)
}

/// 便捷函数：创建默认容器
pub fn create_default_container() -> Result<ServiceContainer, DIError> {
    ContainerBuilder::new().build()
}

/// 便捷函数：创建高性能容器
pub fn create_high_performance_container() -> Result<ServiceContainer, DIError> {
    ContainerBuilderFactory::create_high_performance().build()
}

/// 便捷函数：创建调试容器
pub fn create_debug_container() -> Result<ServiceContainer, DIError> {
    ContainerBuilderFactory::create_debug().build()
}

/// 便捷函数：创建测试容器
pub fn create_test_container() -> Result<ServiceContainer, DIError> {
    ContainerBuilderFactory::create_test().build()
}

/// 服务定位器模式实现
pub struct ServiceLocator {
    container: Arc<ServiceContainer>,
}

impl ServiceLocator {
    /// 创建新的服务定位器
    pub fn new(container: Arc<ServiceContainer>) -> Self {
        Self { container }
    }
    
    /// 获取服务
    pub fn get<T: 'static + Send + Sync>(&self) -> Result<Arc<T>, DIError> {
        self.container.get_required_service::<T>()
    }
    
    /// 尝试获取服务
    pub fn try_get<T: 'static + Send + Sync>(&self) -> Option<Arc<T>> {
        match self.container.get_service::<T>() {
            Ok(service) => service,
            Err(_) => None,
        }
    }
    
    /// 获取容器引用
    pub fn container(&self) -> &ServiceContainer {
        &self.container
    }
}

/// 全局服务定位器（用于兼容单例模式）- 使用 OnceLock 来避免可变静态变量的问题
use std::sync::OnceLock;

static GLOBAL_SERVICE_LOCATOR: OnceLock<Arc<ServiceLocator>> = OnceLock::new();

/// 全局服务定位器API
pub mod global {
    use super::*;
    
    /// 初始化全局服务定位器
    pub fn init(container: Arc<ServiceContainer>) {
        let _ = GLOBAL_SERVICE_LOCATOR.set(Arc::new(ServiceLocator::new(container)));
    }
    
    /// 获取全局服务
    pub fn get<T: 'static + Send + Sync>() -> Result<Arc<T>, DIError> {
        if let Some(locator) = GLOBAL_SERVICE_LOCATOR.get() {
            locator.get::<T>()
        } else {
            Err(DIError::ServiceNotRegistered(TypeId::of::<T>()))
        }
    }
    
    /// 尝试获取全局服务
    pub fn try_get<T: 'static + Send + Sync>() -> Option<Arc<T>> {
        if let Some(locator) = GLOBAL_SERVICE_LOCATOR.get() {
            locator.try_get::<T>()
        } else {
            None
        }
    }
    
    /// 检查全局服务是否已初始化
    pub fn is_initialized() -> bool {
        GLOBAL_SERVICE_LOCATOR.get().is_some()
    }
    
    /// 重置全局服务定位器（主要用于测试）
    pub fn reset() {
        // OnceLock 不支持重置，所以这个方法暂时不可用
        // 在实际使用中，可能需要更复杂的机制
    }
}

/// 依赖注入配置构建器
pub struct DIConfigBuilder {
    config: DIConfig,
}

/// 依赖注入配置
#[derive(Debug, Clone)]
pub struct DIConfig {
    /// 容器配置
    pub container_config: ContainerConfig,
    
    /// 是否启用全局服务定位器
    pub enable_global_locator: bool,
    
    /// 自动注册的服务类型
    pub auto_register_types: Vec<TypeId>,
    
    /// 配置文件路径
    pub config_file: Option<String>,
}

impl Default for DIConfig {
    fn default() -> Self {
        Self {
            container_config: ContainerConfig::default(),
            enable_global_locator: false,
            auto_register_types: Vec::new(),
            config_file: None,
        }
    }
}

impl DIConfigBuilder {
    /// 创建新的配置构建器
    pub fn new() -> Self {
        Self {
            config: DIConfig::default(),
        }
    }
    
    /// 设置容器配置
    pub fn with_container_config(mut self, config: ContainerConfig) -> Self {
        self.config.container_config = config;
        self
    }
    
    /// 启用全局服务定位器
    pub fn with_global_locator(mut self, enable: bool) -> Self {
        self.config.enable_global_locator = enable;
        self
    }
    
    /// 添加自动注册类型
    pub fn with_auto_register_type<T: 'static + Send + Sync>(mut self) -> Self {
        self.config.auto_register_types.push(TypeId::of::<T>());
        self
    }
    
    /// 设置配置文件路径
    pub fn with_config_file(mut self, path: &str) -> Self {
        self.config.config_file = Some(path.to_string());
        self
    }
    
    /// 构建配置
    pub fn build(self) -> DIConfig {
        self.config
    }
}

impl Default for DIConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// 依赖注入应用程序引导器
pub struct DIBootstrap {
    config: DIConfig,
}

impl DIBootstrap {
    /// 创建新的引导器
    pub fn new(config: DIConfig) -> Self {
        Self { config }
    }
    
    /// 引导应用程序
    pub fn bootstrap(self) -> Result<ServiceContainer, DIError> {
        // 创建容器构建器
        let builder = ContainerBuilderFactory::from_config(self.config.container_config);
        
        // 自动注册服务类型
        let builder = builder;
        for _type_id in self.config.auto_register_types {
            // 这里需要根据类型ID注册服务
            // 由于Rust的类型限制，需要更复杂的实现
        }
        
        // 构建容器
        let container = builder.build()?;
        
        // 初始化全局服务定位器
        if self.config.enable_global_locator {
            global::init(Arc::new(container.clone()));
        }
        
        Ok(container)
    }
    
    /// 从配置文件引导
    pub fn bootstrap_from_config_file(path: &str) -> Result<ServiceContainer, DIError> {
        // 在实际实现中，这里应该读取配置文件
        // 目前使用默认配置
        let config = DIConfigBuilder::new()
            .with_config_file(path)
            .build();
        
        Self::new(config).bootstrap()
    }
}

/// 依赖注入框架信息
#[derive(Debug, Clone)]
pub struct DIInfo {
    /// 框架版本
    pub version: &'static str,
    
    /// 支持的功能
    pub features: Vec<&'static str>,
    
    /// 构建信息
    pub build_info: BuildInfo,
}

/// 构建信息
#[derive(Debug, Clone)]
pub struct BuildInfo {
    /// 构建时间
    pub build_time: &'static str,
    
    /// 编译器版本
    pub rustc_version: &'static str,
    
    /// 目标架构
    pub target_arch: &'static str,
}

impl DIInfo {
    /// 获取框架信息
    pub fn get() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION"),
            features: vec![
                "service-container",
                "dependency-injection",
                "lifetime-management",
                "circular-dependency-detection",
                "performance-monitoring",
                "lazy-initialization",
            ],
            build_info: BuildInfo {
                build_time: option_env!("BUILD_TIME").unwrap_or_else(|| "unknown"),
                rustc_version: option_env!("RUSTC_VERSION").unwrap_or_else(|| "unknown"),
                target_arch: option_env!("TARGET_ARCH").unwrap_or_else(|| "unknown"),
            },
        }
    }
}

/// 依赖注入框架初始化宏
#[macro_export]
macro_rules! init_di_framework {
    () => {
        use vm_core::di::prelude::*;
        
        // 创建默认容器
        let container = match create_default_container() {
            Ok(container) => container,
            Err(e) => panic!("Failed to create DI container: {}", e),
        };
        
        // 初始化全局服务定位器
        vm_core::di::global::init(std::sync::Arc::new(container));
    };
    
    (debug) => {
        use vm_core::di::prelude::*;
        
        // 创建调试容器
        let container = match create_debug_container() {
            Ok(container) => container,
            Err(e) => panic!("Failed to create DI container: {}", e),
        };
        
        // 初始化全局服务定位器
        vm_core::di::global::init(std::sync::Arc::new(container));
    };
    
    (performance) => {
        use vm_core::di::prelude::*;
        
        // 创建高性能容器
        let container = match create_high_performance_container() {
            Ok(container) => container,
            Err(e) => panic!("Failed to create DI container: {}", e),
        };
        
        // 初始化全局服务定位器
        vm_core::di::global::init(std::sync::Arc::new(container));
    };
}

/// 服务注册宏
#[macro_export]
macro_rules! register_service {
    (singleton, $type:ty) => {
        vm_core::di::register_singleton::<$type>
    };
    
    (transient, $type:ty) => {
        vm_core::di::register_transient::<$type>
    };
    
    (scoped, $type:ty) => {
        vm_core::di::register_scoped::<$type>
    };
    
    (instance, $type:ty, $value:expr) => {
        vm_core::di::register_instance::<$type>
    };
    
    (factory, $type:ty, $factory:expr) => {
        vm_core::di::register_factory::<$type, _>
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_convenience_functions() {
        let container = create_default_container().unwrap();
        assert!(container.is_registered::<String>());
        
        let high_perf_container = create_high_performance_container().unwrap();
        assert!(high_perf_container.is_registered::<String>());
        
        let debug_container = create_debug_container().unwrap();
        assert!(debug_container.is_registered::<String>());
        
        let test_container = create_test_container().unwrap();
        assert!(test_container.is_registered::<String>());
    }
    
    #[test]
    fn test_service_locator() {
        let container = Arc::new(create_default_container().unwrap());
        let locator = ServiceLocator::new(container);
        
        // 测试服务定位器功能
        // 注意：这里需要先注册服务才能获取
    }
    
    #[test]
    fn test_global_locator() {
        let container = Arc::new(create_default_container().unwrap());
        global::init(container);
        
        assert!(global::is_initialized());
        
        // 重置全局状态
        global::reset();
        assert!(!global::is_initialized());
    }
    
    #[test]
    fn test_di_config_builder() {
        let config = DIConfigBuilder::new()
            .with_global_locator(true)
            .with_auto_register_type::<String>()
            .with_config_file("test_config.json")
            .build();
        
        assert!(config.enable_global_locator);
        assert_eq!(config.config_file, Some("test_config.json".to_string()));
        assert!(config.auto_register_types.contains(&TypeId::of::<String>()));
    }
    
    #[test]
    fn test_di_bootstrap() {
        let config = DIConfigBuilder::new().with_global_locator(true).build();
        let container = DIBootstrap::new(config).bootstrap().unwrap();
        
        assert!(global::is_initialized());
        
        // 重置全局状态
        global::reset();
    }
    
    #[test]
    fn test_di_info() {
        let info = DIInfo::get();
        assert!(!info.version.is_empty());
        assert!(!info.features.is_empty());
        assert!(!info.build_info.build_time.is_empty());
    }
}