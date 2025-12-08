//! 服务描述符和生命周期管理
//!
//! 本模块定义了服务描述符的接口和实现，以及服务的生命周期管理策略。

use std::any::{Any, TypeId};
use std::fmt;
use std::sync::Arc;


/// 依赖注入错误类型
#[derive(Debug)]
pub enum DIError {
    /// 服务未注册
    ServiceNotRegistered(TypeId),
    /// 循环依赖
    CircularDependency(Vec<TypeId>),
    /// 服务创建失败
    ServiceCreationFailed(String),
    /// 依赖解析失败
    DependencyResolutionFailed(String),
    /// 无效的服务配置
    InvalidServiceConfiguration(String),
}

impl fmt::Display for DIError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DIError::ServiceNotRegistered(type_id) => {
                write!(f, "Service not registered: {:?}", type_id)
            }
            DIError::CircularDependency(types) => {
                write!(f, "Circular dependency detected: {:?}", types)
            }
            DIError::ServiceCreationFailed(msg) => {
                write!(f, "Service creation failed: {}", msg)
            }
            DIError::DependencyResolutionFailed(msg) => {
                write!(f, "Dependency resolution failed: {}", msg)
            }
            DIError::InvalidServiceConfiguration(msg) => {
                write!(f, "Invalid service configuration: {}", msg)
            }
        }
    }
}

impl std::error::Error for DIError {}

/// 服务生命周期类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceLifetime {
    /// 单例：整个容器生命周期内只创建一次
    Singleton,
    /// 瞬态：每次请求都创建新实例
    Transient,
    /// 作用域：在特定作用域内单例
    Scoped,
}

/// 服务描述符 trait
pub trait ServiceDescriptor: Send + Sync {
    /// 获取服务类型ID
    fn service_type(&self) -> TypeId;
    
    /// 获取服务生命周期
    fn lifetime(&self) -> ServiceLifetime;
    
    /// 创建服务实例
    fn create_instance(&self, container: &dyn ServiceProvider) -> Result<Box<dyn Any + Send + Sync>, DIError>;
    
    /// 获取依赖的服务类型
    fn dependencies(&self) -> Vec<TypeId>;
    
    /// 获取服务名称（用于调试）
    fn service_name(&self) -> &'static str;
}

/// 服务提供者 trait（非泛型版本）
pub trait ServiceProvider: Send + Sync {
    /// 创建作用域
    fn create_scope(&self) -> Result<Box<dyn ServiceProvider>, DIError>;
    
    /// 根据类型ID获取服务
    fn get_service_by_id(&self, type_id: TypeId) -> Result<Option<Arc<dyn Any + Send + Sync>>, DIError>;
}


/// 泛型服务描述符实现
pub struct GenericServiceDescriptor<T: 'static + Send + Sync> {
    lifetime: ServiceLifetime,
    factory: Option<Arc<dyn Fn(&dyn ServiceProvider) -> Result<T, DIError> + Send + Sync + 'static>>,
    instance: Option<Arc<T>>,
    dependencies: Vec<TypeId>,
    name: &'static str,
}

impl<T: 'static + Send + Sync> GenericServiceDescriptor<T> {
    /// 创建新的服务描述符
    pub fn new(lifetime: ServiceLifetime) -> Self {
        Self {
            lifetime,
            factory: None,
            instance: None,
            dependencies: Vec::new(),
            name: std::any::type_name::<T>(),
        }
    }
    
    /// 设置工厂函数
    pub fn with_factory<F>(mut self, factory: F) -> Self
    where
        F: Fn(&dyn ServiceProvider) -> Result<T, DIError> + Send + Sync + 'static,
    {
        self.factory = Some(Arc::new(factory));
        self
    }
    
    /// 设置依赖项
    pub fn with_dependencies(mut self, dependencies: Vec<TypeId>) -> Self {
        self.dependencies = dependencies;
        self
    }
    
    /// 设置实例（仅用于单例）
    pub fn with_instance(mut self, instance: T) -> Self {
        self.instance = Some(Arc::new(instance));
        self
    }
}

impl<T: 'static + Send + Sync> ServiceDescriptor for GenericServiceDescriptor<T> {
    fn service_type(&self) -> TypeId {
        TypeId::of::<T>()
    }
    
    fn lifetime(&self) -> ServiceLifetime {
        self.lifetime
    }
    
    fn create_instance(&self, container: &dyn ServiceProvider) -> Result<Box<dyn Any + Send + Sync>, DIError> {
        if let Some(ref instance) = self.instance {
            return Ok(Box::new(Arc::clone(instance)));
        }
        
        if let Some(ref factory) = self.factory {
            let instance = factory(container)?;
            return Ok(Box::new(Arc::new(instance)));
        }
        
        // 默认构造函数
        // 这里需要使用反射或其他机制来创建实例
        // 由于Rust没有内置反射，我们使用默认构造函数
        Err(DIError::ServiceCreationFailed(
            "No factory or instance provided".to_string(),
        ))
    }
    
    fn dependencies(&self) -> Vec<TypeId> {
        self.dependencies.clone()
    }
    
    fn service_name(&self) -> &'static str {
        self.name
    }
}

/// 服务实例包装器
pub struct ServiceInstance {
    instance: Box<dyn Any + Send + Sync>,
    lifetime: ServiceLifetime,
}

impl Clone for ServiceInstance {
    fn clone(&self) -> Self {
        panic!("ServiceInstance cannot be cloned");
    }
}

impl ServiceInstance {
    /// 创建新的服务实例
    pub fn new(instance: Box<dyn Any + Send + Sync>, lifetime: ServiceLifetime) -> Self {
        Self { instance, lifetime }
    }
    
    /// 获取实例的引用
    pub fn as_any(&self) -> &dyn Any {
        self.instance.as_ref()
    }
    
    /// 获取实例的可变引用
    pub fn as_any_mut(&mut self) -> &mut dyn Any {
        self.instance.as_mut()
    }
    
    /// 获取服务生命周期
    pub fn lifetime(&self) -> ServiceLifetime {
        self.lifetime
    }
}

/// 作用域管理器
pub struct ScopeManager {
    scopes: Vec<Scope>,
}

impl ScopeManager {
    /// 创建新的作用域管理器
    pub fn new() -> Self {
        Self {
            scopes: Vec::new(),
        }
    }
    
    /// 创建新作用域
    pub fn create_scope(&mut self) -> Scope {
        let scope = Scope::new();
        self.scopes.push(scope.clone());
        scope
    }
    
    /// 销毁作用域
    pub fn destroy_scope(&mut self, scope_id: u64) {
        self.scopes.retain(|s| s.id() != scope_id);
    }
    
    /// 获取当前作用域
    pub fn current_scope(&self) -> Option<&Scope> {
        self.scopes.last()
    }
}

/// 作用域
#[derive(Clone)]
pub struct Scope {
    id: u64,
    services: Arc<std::sync::RwLock<std::collections::HashMap<TypeId, ServiceInstance>>>,
}

impl Scope {
    /// 创建新作用域
    pub fn new() -> Self {
        static NEXT_SCOPE_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        
        Self {
            id: NEXT_SCOPE_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst),
            services: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        }
    }
    
    /// 获取作用域ID
    pub fn id(&self) -> u64 {
        self.id
    }
    
    /// 注册服务
    pub fn register_service(&self, type_id: TypeId, instance: ServiceInstance) {
        let mut services = self.services.write().unwrap();
        services.insert(type_id, instance);
    }
    
    /// 获取服务
    pub fn get_service(&self, type_id: TypeId) -> Option<ServiceInstance> {
        let services = self.services.read().unwrap();
        services.get(&type_id).cloned()
    }
    
    /// 检查服务是否存在
    pub fn has_service(&self, type_id: TypeId) -> bool {
        let services = self.services.read().unwrap();
        services.contains_key(&type_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_service_lifetime() {
        assert_eq!(ServiceLifetime::Singleton, ServiceLifetime::Singleton);
        assert_ne!(ServiceLifetime::Singleton, ServiceLifetime::Transient);
    }
    
    #[test]
    fn test_generic_service_descriptor() {
        let descriptor = GenericServiceDescriptor::<String>::new(ServiceLifetime::Singleton);
        assert_eq!(descriptor.lifetime(), ServiceLifetime::Singleton);
        assert_eq!(descriptor.service_type(), TypeId::of::<String>());
    }
    
    #[test]
    fn test_scope() {
        let scope = Scope::new();
        assert!(scope.id() > 0);
        assert!(!scope.has_service(TypeId::of::<String>()));
    }
    
    #[test]
    fn test_scope_manager() {
        let mut manager = ScopeManager::new();
        let scope = manager.create_scope();
        assert!(manager.current_scope().is_some());
        assert_eq!(manager.current_scope().unwrap().id(), scope.id());
        
        manager.destroy_scope(scope.id());
        assert!(manager.current_scope().is_none());
    }
}