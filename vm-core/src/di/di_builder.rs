//! 容器构建器
//!
//! 本模块实现了服务容器构建器，提供流式API来配置和构建服务容器。

use std::any::TypeId;

use super::di_container::ServiceContainer;
use super::di_service_descriptor::ServiceProvider;
use super::di_registry::ServiceRegistry;
use super::di_resolver::{DependencyResolver, ResolutionStrategy};
use super::di_service_descriptor::{
    DIError, ServiceDescriptor, ServiceLifetime,
};

/// 容器构建器
pub struct ContainerBuilder {
    /// 服务注册表
    registry: ServiceRegistry,
    
    /// 依赖解析策略
    resolution_strategy: ResolutionStrategy,
    
    /// 是否启用循环依赖检测
    enable_circular_dependency_detection: bool,
    
    /// 是否启用延迟初始化
    enable_lazy_initialization: bool,
    
    /// 预热服务列表
    warmup_services: Vec<TypeId>,
    
    /// 配置选项
    options: ContainerOptions,
}

/// 容器配置选项
#[derive(Debug, Clone)]
pub struct ContainerOptions {
    /// 是否启用性能监控
    pub enable_performance_monitoring: bool,
    
    /// 是否启用调试模式
    pub enable_debug_mode: bool,
    
    /// 最大并发解析数
    pub max_concurrent_resolutions: usize,
    
    /// 解析超时时间（毫秒）
    pub resolution_timeout_ms: u64,
    
    /// 是否启用服务验证
    pub enable_service_validation: bool,
}

impl Default for ContainerOptions {
    fn default() -> Self {
        Self {
            enable_performance_monitoring: false,
            enable_debug_mode: false,
            max_concurrent_resolutions: 10,
            resolution_timeout_ms: 5000,
            enable_service_validation: true,
        }
    }
}

impl ContainerBuilder {
    /// 创建新的容器构建器
    pub fn new() -> Self {
        Self {
            registry: ServiceRegistry::new(),
            resolution_strategy: ResolutionStrategy::TopologicalSort,
            enable_circular_dependency_detection: true,
            enable_lazy_initialization: true,
            warmup_services: Vec::new(),
            options: ContainerOptions::default(),
        }
    }
    
    /// 从现有注册表创建构建器
    pub fn from_registry(registry: ServiceRegistry) -> Self {
        Self {
            registry,
            resolution_strategy: ResolutionStrategy::TopologicalSort,
            enable_circular_dependency_detection: true,
            enable_lazy_initialization: true,
            warmup_services: Vec::new(),
            options: ContainerOptions::default(),
        }
    }
    
    /// 注册单例服务
    pub fn register_singleton<T: 'static + Send + Sync>(self) -> Self {
        self.registry.register_singleton::<T>().unwrap();
        self
    }
    
    /// 注册瞬态服务
    pub fn register_transient<T: 'static + Send + Sync>(self) -> Self {
        self.registry.register_transient::<T>().unwrap();
        self
    }
    
    /// 注册作用域服务
    pub fn register_scoped<T: 'static + Send + Sync>(self) -> Self {
        self.registry.register_scoped::<T>().unwrap();
        self
    }
    
    /// 注册服务实例
    pub fn register_instance<T: 'static + Send + Sync>(self, instance: T) -> Self {
        self.registry.register_instance(instance).unwrap();
        self
    }
    
    /// 注册工厂函数
    pub fn register_factory<T: 'static + Send + Sync, F>(self, factory: F) -> Self
    where
        F: Fn(&dyn ServiceProvider) -> Result<T, DIError> + Send + Sync + 'static,
    {
        self.registry.register_factory(factory).unwrap();
        self
    }
    
    /// 注册带依赖的服务
    pub fn register_with_dependencies<T: 'static + Send + Sync>(
        self,
        lifetime: ServiceLifetime,
        dependencies: Vec<TypeId>,
    ) -> Self {
        self.registry.register_with_dependencies::<T>(lifetime, dependencies).unwrap();
        self
    }
    
    /// 注册命名服务
    pub fn register_named<T: 'static + Send + Sync>(
        self,
        name: &str,
        lifetime: ServiceLifetime,
    ) -> Self {
        self.registry.register_named::<T>(name, lifetime).unwrap();
        self
    }
    
    /// 将服务添加到分组
    pub fn add_to_group<T: 'static + Send + Sync>(self, group: &str) -> Self {
        self.registry.add_to_group::<T>(group).unwrap();
        self
    }
    
    /// 设置依赖解析策略
    pub fn with_resolution_strategy(mut self, strategy: ResolutionStrategy) -> Self {
        self.resolution_strategy = strategy;
        self
    }
    
    /// 启用/禁用循环依赖检测
    pub fn with_circular_dependency_detection(mut self, enable: bool) -> Self {
        self.enable_circular_dependency_detection = enable;
        self
    }
    
    /// 启用/禁用延迟初始化
    pub fn with_lazy_initialization(mut self, enable: bool) -> Self {
        self.enable_lazy_initialization = enable;
        self
    }
    
    /// 添加预热服务
    pub fn with_warmup_service<T: 'static + Send + Sync>(mut self) -> Self {
        self.warmup_services.push(TypeId::of::<T>());
        self
    }
    
    /// 添加预热服务列表
    pub fn with_warmup_services(mut self, services: Vec<TypeId>) -> Self {
        self.warmup_services.extend(services);
        self
    }
    
    /// 设置容器选项
    pub fn with_options(mut self, options: ContainerOptions) -> Self {
        self.options = options;
        self
    }
    
    /// 启用性能监控
    pub fn with_performance_monitoring(mut self, enable: bool) -> Self {
        self.options.enable_performance_monitoring = enable;
        self
    }
    
    /// 启用调试模式
    pub fn with_debug_mode(mut self, enable: bool) -> Self {
        self.options.enable_debug_mode = enable;
        self
    }
    
    /// 设置最大并发解析数
    pub fn with_max_concurrent_resolutions(mut self, max: usize) -> Self {
        self.options.max_concurrent_resolutions = max;
        self
    }
    
    /// 设置解析超时时间
    pub fn with_resolution_timeout(mut self, timeout_ms: u64) -> Self {
        self.options.resolution_timeout_ms = timeout_ms;
        self
    }
    
    /// 启用/禁用服务验证
    pub fn with_service_validation(mut self, enable: bool) -> Self {
        self.options.enable_service_validation = enable;
        self
    }
    
    /// 构建服务容器
    pub fn build(self) -> Result<ServiceContainer, DIError> {
        // 创建容器
        let container = ServiceContainer::new();
        
        // 创建依赖解析器
        let resolver = DependencyResolver::new(self.resolution_strategy);
        
        // 注册所有服务到容器
        self.register_services_to_container(&container, &resolver)?;
        
        // 验证服务配置
        if self.options.enable_service_validation {
            self.validate_services(&container)?;
        }
        
        // 预热服务
        if !self.warmup_services.is_empty() {
            container.warm_up(self.warmup_services)?;
        }
        
        Ok(container)
    }
    
    /// 注册服务到容器
    fn register_services_to_container(
        &self,
        container: &ServiceContainer,
        resolver: &DependencyResolver,
    ) -> Result<(), DIError> {
        let registered_services = self.registry.registered_services();
        
        for type_id in registered_services {
            // 获取服务描述符
            let descriptor = self.create_service_descriptor(type_id)?;
            
            // 添加到解析器
            resolver.add_service_descriptor(descriptor.as_ref())?;
            
            // 注册到容器
            container.register_descriptor(descriptor)?;
        }
        
        Ok(())
    }
    
    /// 创建服务描述符
    fn create_service_descriptor(&self, _type_id: TypeId) -> Result<Box<dyn ServiceDescriptor>, DIError> {
        // 这里需要根据类型ID创建适当的服务描述符
        // 由于Rust的类型系统限制，这里需要更复杂的实现
        // 目前返回一个默认描述符作为占位符
        
        // 在实际实现中，这里应该使用类型注册表或其他机制
        // 来获取类型信息并创建相应的描述符
        
        Err(DIError::ServiceCreationFailed(
            "Service descriptor creation not fully implemented".to_string(),
        ))
    }
    
    /// 验证服务配置
    fn validate_services(&self, container: &ServiceContainer) -> Result<(), DIError> {
        let registered_services = self.registry.registered_services();
        
        for type_id in registered_services {
            // 检查服务是否可以解析
            if let Err(e) = container.get_service_by_id(type_id) {
                return Err(DIError::InvalidServiceConfiguration(
                    format!("Service validation failed for {:?}: {}", type_id, e),
                ));
            }
        }
        
        Ok(())
    }
}

impl Default for ContainerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// 容器构建器工厂
pub struct ContainerBuilderFactory;

impl ContainerBuilderFactory {
    /// 创建默认构建器
    pub fn create() -> ContainerBuilder {
        ContainerBuilder::new()
    }
    
    /// 创建高性能构建器
    pub fn create_high_performance() -> ContainerBuilder {
        ContainerBuilder::new()
            .with_resolution_strategy(ResolutionStrategy::DepthFirst)
            .with_lazy_initialization(true)
            .with_performance_monitoring(true)
            .with_max_concurrent_resolutions(20)
            .with_resolution_timeout(1000)
    }
    
    /// 创建调试构建器
    pub fn create_debug() -> ContainerBuilder {
        ContainerBuilder::new()
            .with_circular_dependency_detection(true)
            .with_debug_mode(true)
            .with_service_validation(true)
            .with_resolution_timeout(10000)
    }
    
    /// 创建测试构建器
    pub fn create_test() -> ContainerBuilder {
        ContainerBuilder::new()
            .with_lazy_initialization(false)
            .with_service_validation(true)
            .with_debug_mode(true)
    }
    
    /// 从配置创建构建器
    pub fn from_config(config: ContainerConfig) -> ContainerBuilder {
        let mut builder = ContainerBuilder::new();
        
        // 应用配置
        builder = builder.with_resolution_strategy(config.resolution_strategy);
        builder = builder.with_circular_dependency_detection(config.enable_circular_dependency_detection);
        builder = builder.with_lazy_initialization(config.enable_lazy_initialization);
        builder = builder.with_warmup_services(config.warmup_services);
        builder = builder.with_options(config.options);
        
        builder
    }
}

/// 容器配置
#[derive(Debug, Clone)]
pub struct ContainerConfig {
    /// 依赖解析策略
    pub resolution_strategy: ResolutionStrategy,
    
    /// 是否启用循环依赖检测
    pub enable_circular_dependency_detection: bool,
    
    /// 是否启用延迟初始化
    pub enable_lazy_initialization: bool,
    
    /// 预热服务列表
    pub warmup_services: Vec<TypeId>,
    
    /// 容器选项
    pub options: ContainerOptions,
}

impl Default for ContainerConfig {
    fn default() -> Self {
        Self {
            resolution_strategy: ResolutionStrategy::TopologicalSort,
            enable_circular_dependency_detection: true,
            enable_lazy_initialization: true,
            warmup_services: Vec::new(),
            options: ContainerOptions::default(),
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_container_builder_creation() {
        let builder = ContainerBuilder::new();
        assert!(builder.build().is_ok());
    }
    
    #[test]
    fn test_register_services() {
        let container = ContainerBuilder::new()
            .register_singleton::<String>()
            .register_transient::<i32>()
            .build()
            .unwrap();
        
        assert!(container.is_registered::<String>());
        assert!(container.is_registered::<i32>());
    }
    
    #[test]
    fn test_with_options() {
        let options = ContainerOptions {
            enable_performance_monitoring: true,
            enable_debug_mode: true,
            max_concurrent_resolutions: 5,
            resolution_timeout_ms: 2000,
            enable_service_validation: false,
        };
        
        let builder = ContainerBuilder::new().with_options(options);
        // 构建应该成功，但具体选项验证需要更复杂的实现
        assert!(builder.build().is_ok());
    }
    
    #[test]
    fn test_factory_methods() {
        let high_perf_builder = ContainerBuilderFactory::create_high_performance();
        let debug_builder = ContainerBuilderFactory::create_debug();
        let test_builder = ContainerBuilderFactory::create_test();
        
        // 所有构建器都应该能够构建容器
        assert!(high_perf_builder.build().is_ok());
        assert!(debug_builder.build().is_ok());
        assert!(test_builder.build().is_ok());
    }
    
}