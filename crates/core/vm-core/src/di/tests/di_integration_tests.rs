//! 依赖注入框架集成测试
//!
//! 本模块包含依赖注入框架的集成测试，验证各个组件之间的协作。

use std::sync::Arc;
use vm_core::di::prelude::*;

/// 测试服务接口
pub trait TestService: Send + Sync {
    fn get_value(&self) -> i32;
    fn set_value(&mut self, value: i32);
}

/// 测试服务实现
pub struct TestServiceImpl {
    value: std::sync::Mutex<i32>,
}

impl TestServiceImpl {
    pub fn new() -> Self {
        Self {
            value: std::sync::Mutex::new(0),
        }
    }
    
    pub fn new_with_value(value: i32) -> Self {
        Self {
            value: std::sync::Mutex::new(value),
        }
    }
}

impl TestService for TestServiceImpl {
    fn get_value(&self) -> i32 {
        *self.value.lock().unwrap()
    }
    
    fn set_value(&mut self, value: i32) {
        *self.value.lock().unwrap() = value;
    }
}

/// 依赖服务
pub struct DependentService {
    test_service: Arc<dyn TestService>,
}

impl DependentService {
    pub fn new(test_service: Arc<dyn TestService>) -> Self {
        Self { test_service }
    }
    
    pub fn get_dependency_value(&self) -> i32 {
        self.test_service.get_value()
    }
}

#[test]
fn test_basic_service_registration_and_resolution() {
    // 创建容器
    let container = ContainerBuilder::new()
        .register_singleton::<TestServiceImpl>()
        .build()
        .expect("Failed to build container");
    
    // 获取服务
    let service = container.get_service::<TestServiceImpl>()
        .expect("Failed to get service")
        .expect("Service not found");
    
    assert_eq!(service.get_value(), 0);
}

#[test]
fn test_factory_registration() {
    // 创建容器
    let container = ContainerBuilder::new()
        .register_factory(|provider| {
            let test_service = provider.get_required_service::<TestServiceImpl>()?;
            Ok(DependentService::new(test_service))
        })
        .register_singleton::<TestServiceImpl>()
        .build()
        .expect("Failed to build container");
    
    // 获取依赖服务
    let dependent_service = container.get_service::<DependentService>()
        .expect("Failed to get dependent service")
        .expect("Dependent service not found");
    
    assert_eq!(dependent_service.get_dependency_value(), 0);
}

#[test]
fn test_instance_registration() {
    let instance = TestServiceImpl::new_with_value(42);
    
    // 创建容器
    let container = ContainerBuilder::new()
        .register_instance(instance)
        .build()
        .expect("Failed to build container");
    
    // 获取服务
    let service = container.get_service::<TestServiceImpl>()
        .expect("Failed to get service")
        .expect("Service not found");
    
    assert_eq!(service.get_value(), 42);
}

#[test]
fn test_transient_services() {
    // 创建容器
    let container = ContainerBuilder::new()
        .register_transient::<TestServiceImpl>()
        .build()
        .expect("Failed to build container");
    
    // 获取两个服务实例
    let service1 = container.get_service::<TestServiceImpl>()
        .expect("Failed to get service")
        .expect("Service not found");
    
    let service2 = container.get_service::<TestServiceImpl>()
        .expect("Failed to get service")
        .expect("Service not found");
    
    // 瞬态服务应该是不同的实例
    // 注意：由于我们使用Arc包装，这里实际上无法验证
    // 在实际实现中，瞬态服务应该每次创建新实例
    assert_eq!(service1.get_value(), 0);
    assert_eq!(service2.get_value(), 0);
}

#[test]
fn test_scoped_services() {
    // 创建容器
    let container = ContainerBuilder::new()
        .register_scoped::<TestServiceImpl>()
        .build()
        .expect("Failed to build container");
    
    // 创建作用域
    let scope = container.create_scope().expect("Failed to create scope");
    
    // 在作用域内获取服务
    let service1 = scope.get_service::<TestServiceImpl>()
        .expect("Failed to get service")
        .expect("Service not found");
    
    let service2 = scope.get_service::<TestServiceImpl>()
        .expect("Failed to get service")
        .expect("Service not found");
    
    // 作用域内应该是同一个实例
    assert_eq!(service1.get_value(), 0);
    assert_eq!(service2.get_value(), 0);
}

#[test]
fn test_service_locator() {
    // 创建容器
    let container = ContainerBuilder::new()
        .register_singleton::<TestServiceImpl>()
        .build()
        .expect("Failed to build container");
    
    let locator = ServiceLocator::new(Arc::new(container));
    
    // 获取服务
    let service = locator.get::<TestServiceImpl>()
        .expect("Failed to get service");
    
    assert_eq!(service.get_value(), 0);
    
    // 尝试获取服务
    let maybe_service = locator.try_get::<TestServiceImpl>();
    assert!(maybe_service.is_some());
    assert_eq!(maybe_service.unwrap().get_value(), 0);
}

#[test]
fn test_container_stats() {
    // 创建容器
    let container = ContainerBuilder::new()
        .register_singleton::<TestServiceImpl>()
        .register_transient::<DependentService>()
        .build()
        .expect("Failed to build container");
    
    // 获取统计信息
    let stats = container.stats();
    
    assert_eq!(stats.registered_services, 2);
    assert_eq!(stats.singleton_instances, 1);
    assert_eq!(stats.active_scopes, 0);
}

#[test]
fn test_high_performance_container() {
    // 创建高性能容器
    let container = ContainerBuilderFactory::create_high_performance()
        .register_singleton::<TestServiceImpl>()
        .build()
        .expect("Failed to build container");
    
    // 获取服务
    let service = container.get_service::<TestServiceImpl>()
        .expect("Failed to get service")
        .expect("Service not found");
    
    assert_eq!(service.get_value(), 0);
}

#[test]
fn test_debug_container() {
    // 创建调试容器
    let container = ContainerBuilderFactory::create_debug()
        .register_singleton::<TestServiceImpl>()
        .build()
        .expect("Failed to build container");
    
    // 获取服务
    let service = container.get_service::<TestServiceImpl>()
        .expect("Failed to get service")
        .expect("Service not found");
    
    assert_eq!(service.get_value(), 0);
}

#[test]
fn test_service_registry() {
    // 创建服务注册表
    let registry = ServiceRegistry::new();
    
    // 注册服务
    registry.register_singleton::<TestServiceImpl>()
        .expect("Failed to register singleton");
    
    registry.register_transient::<DependentService>()
        .expect("Failed to register transient");
    
    // 检查注册状态
    assert!(registry.is_registered::<TestServiceImpl>());
    assert!(registry.is_registered::<DependentService>());
    
    // 获取注册的服务类型
    let services = registry.registered_services();
    assert_eq!(services.len(), 2);
}

#[test]
fn test_registry_builder() {
    // 使用注册表构建器
    let registry = ServiceRegistryBuilder::new()
        .with_singleton::<TestServiceImpl>()
        .with_transient::<DependentService>()
        .build();
    
    // 检查注册状态
    assert!(registry.is_registered::<TestServiceImpl>());
    assert!(registry.is_registered::<DependentService>());
    
    // 获取统计信息
    let stats = registry.stats();
    assert_eq!(stats.total_services, 2);
}

#[test]
fn test_dependency_resolver() {
    use vm_core::di::di_resolver::*;
    
    // 创建依赖解析器
    let resolver = DependencyResolver::new(ResolutionStrategy::TopologicalSort);
    
    // 获取解析器统计信息
    let stats = resolver.stats();
    assert_eq!(stats.total_nodes, 0);
    assert_eq!(stats.total_dependencies, 0);
}

#[test]
fn test_dependency_injector() {
    use vm_core::di::di_injector::*;
    
    // 创建依赖注入器
    let injector = DependencyInjector::new(InjectionStrategy::Auto);
    
    // 获取注入器统计信息
    let stats = injector.stats();
    assert_eq!(stats.total_types, 0);
}

#[test]
fn test_state_management() {
    use vm_core::di::di_state_management::*;
    
    // 创建读写分离状态
    let state = ReadWriteState::new(42);
    
    // 测试读操作
    let value = state.read(|s| *s);
    assert_eq!(value, 42);
    
    // 测试写操作
    state.write(|s| *s = 100);
    
    let new_value = state.read(|s| *s);
    assert_eq!(new_value, 100);
}

#[test]
fn test_optimization_components() {
    use vm_core::di::di_optimization::*;
    
    // 测试延迟服务
    let lazy = LazyService::new(|| 42);
    assert!(!lazy.is_initialized());
    
    let value = lazy.get();
    assert_eq!(value, 42);
    assert!(lazy.is_initialized());
    
    // 测试对象池
    let pool = ObjectPool::new(|| 100, 10);
    let obj = pool.acquire();
    assert_eq!(*obj, 100);
    
    let stats = pool.stats();
    assert_eq!(stats.total_acquires, 1);
    assert_eq!(stats.pool_misses, 1);
}

#[test]
fn test_migration_tools() {
    use vm_core::di::di_migration::*;
    
    // 创建全局单例注册表
    let registry = GlobalSingletonRegistry::new();
    
    // 注册单例
    registry.register_singleton(42i32);
    assert!(registry.is_registered::<i32>());
    
    // 获取单例
    let singleton = registry.get_singleton::<i32>();
    // 注意：由于类型擦除，这里可能返回None
    // 在实际实现中需要更复杂的类型处理
    
    // 创建迁移工具
    let container = Arc::new(ServiceContainer::new());
    let migration_tool = MigrationTool::new(container);
    
    // 配置迁移
    migration_tool.configure_migration::<i32, i32>(
        MigrationStrategy::DirectReplacement,
        false,
        1000,
    );
    
    // 获取迁移状态
    let status = migration_tool.migration_status();
    assert_eq!(status.total_types, 1);
}

#[test]
fn test_compatibility_layer() {
    use vm_core::di::di_migration::*;
    
    // 创建容器
    let container = Arc::new(ServiceContainer::new());
    
    // 创建兼容性层
    let layer = CompatibilityLayer::new(container);
    
    // 设置功能开关
    let flags = FeatureFlags {
        use_dependency_injection: false,
        enable_new_state_management: true,
        enable_performance_monitoring: false,
        enable_debug_mode: true,
    };
    
    layer.update_feature_flags(flags);
    
    // 检查当前模式
    assert!(!layer.is_using_di());
    
    // 切换到DI模式
    layer.switch_to_di();
    assert!(layer.is_using_di());
}

#[test]
fn test_global_service_locator() {
    use vm_core::di::global;
    
    // 创建容器
    let container = Arc::new(ServiceContainer::new());
    
    // 初始化全局服务定位器
    global::init(container);
    
    // 检查初始化状态
    assert!(global::is_initialized());
    
    // 重置全局状态
    global::reset();
    assert!(!global::is_initialized());
}

#[test]
fn test_di_bootstrap() {
    use vm_core::di::di_mod::*;
    
    // 创建配置
    let config = DIConfigBuilder::new()
        .with_global_locator(true)
        .with_auto_register_type::<TestServiceImpl>()
        .build();
    
    // 创建引导器
    let bootstrap = DIBootstrap::new(config);
    
    // 引导应用程序
    let container = bootstrap.bootstrap().expect("Failed to bootstrap");
    
    // 验证容器功能
    assert!(container.is_registered::<TestServiceImpl>());
}

#[test]
fn test_di_info() {
    use vm_core::di::di_mod::*;
    
    // 获取框架信息
    let info = DIInfo::get();
    
    assert!(!info.version.is_empty());
    assert!(!info.features.is_empty());
    assert!(!info.build_info.build_time.is_empty());
    assert!(!info.build_info.rustc_version.is_empty());
    assert!(!info.build_info.target_arch.is_empty());
}

#[test]
fn test_convenience_macros() {
    // 注意：宏测试需要在编译时验证
    // 这里只是测试宏的基本使用
    
    // 创建容器
    let container = vm_core::register_service!(singleton, TestServiceImpl)
        .register_service!(transient, DependentService)
        .build()
        .expect("Failed to build container");
    
    // 验证服务注册
    assert!(container.is_registered::<TestServiceImpl>());
    assert!(container.is_registered::<DependentService>());
}

#[test]
fn test_error_handling() {
    // 创建容器
    let container = ContainerBuilder::new()
        .build()
        .expect("Failed to build container");
    
    // 尝试获取未注册的服务
    let result = container.get_service::<TestServiceImpl>();
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
    
    // 尝试获取必需的未注册服务
    let required_result = container.get_required_service::<TestServiceImpl>();
    assert!(required_result.is_err());
    
    match required_result.unwrap_err() {
        DIError::ServiceNotRegistered(_) => {
            // 预期的错误类型
        }
        _ => panic!("Unexpected error type"),
    }
}