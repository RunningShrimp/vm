//! 服务容器核心实现
//!
//! 本模块实现了依赖注入框架的核心服务容器，负责服务的注册、解析和生命周期管理。

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use super::di_service_descriptor::{
    DIError, ServiceDescriptor, ServiceInstance, ServiceLifetime, Scope, ScopeManager,
    ServiceProvider,
};
use crate::CoreError;

/// 服务容器核心实现
#[derive(Clone)]
pub struct ServiceContainer {
    /// 已注册的服务描述符
    services: Arc<RwLock<HashMap<TypeId, Arc<Box<dyn ServiceDescriptor>>>>>,

    /// 单例服务实例缓存
    singleton_instances: Arc<RwLock<HashMap<TypeId, ServiceInstance>>>,

    /// 作用域管理器
    scope_manager: Arc<RwLock<ScopeManager>>,

    /// 当前解析路径（用于循环依赖检测）
    resolving: Arc<RwLock<Vec<TypeId>>>,
}

// Helper methods for lock operations with proper error handling
impl ServiceContainer {
    /// Acquire read lock on services with error handling
    fn lock_services_read(&self) -> Result<std::sync::RwLockReadGuard<'_, HashMap<TypeId, Arc<Box<dyn ServiceDescriptor>>>>, CoreError> {
        self.services.read().map_err(|e| CoreError::Concurrency {
            message: format!("Failed to acquire read lock on services: {}", e),
            operation: "lock_services_read".to_string(),
        })
    }

    /// Acquire write lock on services with error handling
    fn lock_services_write(&self) -> Result<std::sync::RwLockWriteGuard<'_, HashMap<TypeId, Arc<Box<dyn ServiceDescriptor>>>>, CoreError> {
        self.services.write().map_err(|e| CoreError::Concurrency {
            message: format!("Failed to acquire write lock on services: {}", e),
            operation: "lock_services_write".to_string(),
        })
    }

    /// Acquire read lock on singleton_instances with error handling
    fn lock_singletons_read(&self) -> Result<std::sync::RwLockReadGuard<'_, HashMap<TypeId, ServiceInstance>>, CoreError> {
        self.singleton_instances.read().map_err(|e| CoreError::Concurrency {
            message: format!("Failed to acquire read lock on singleton instances: {}", e),
            operation: "lock_singletons_read".to_string(),
        })
    }

    /// Acquire write lock on singleton_instances with error handling
    fn lock_singletons_write(&self) -> Result<std::sync::RwLockWriteGuard<'_, HashMap<TypeId, ServiceInstance>>, CoreError> {
        self.singleton_instances.write().map_err(|e| CoreError::Concurrency {
            message: format!("Failed to acquire write lock on singleton instances: {}", e),
            operation: "lock_singletons_write".to_string(),
        })
    }

    /// Acquire read lock on scope_manager with error handling
    fn lock_scope_manager_read(&self) -> Result<std::sync::RwLockReadGuard<'_, ScopeManager>, CoreError> {
        self.scope_manager.read().map_err(|e| CoreError::Concurrency {
            message: format!("Failed to acquire read lock on scope manager: {}", e),
            operation: "lock_scope_manager_read".to_string(),
        })
    }

    /// Acquire write lock on scope_manager with error handling
    fn lock_scope_manager_write(&self) -> Result<std::sync::RwLockWriteGuard<'_, ScopeManager>, CoreError> {
        self.scope_manager.write().map_err(|e| CoreError::Concurrency {
            message: format!("Failed to acquire write lock on scope manager: {}", e),
            operation: "lock_scope_manager_write".to_string(),
        })
    }

    /// Acquire read lock on resolving with error handling
    fn lock_resolving_read(&self) -> Result<std::sync::RwLockReadGuard<'_, Vec<TypeId>>, CoreError> {
        self.resolving.read().map_err(|e| CoreError::Concurrency {
            message: format!("Failed to acquire read lock on resolving: {}", e),
            operation: "lock_resolving_read".to_string(),
        })
    }

    /// Acquire write lock on resolving with error handling
    fn lock_resolving_write(&self) -> Result<std::sync::RwLockWriteGuard<'_, Vec<TypeId>>, CoreError> {
        self.resolving.write().map_err(|e| CoreError::Concurrency {
            message: format!("Failed to acquire write lock on resolving: {}", e),
            operation: "lock_resolving_write".to_string(),
        })
    }
}

impl ServiceContainer {
    /// 创建新的服务容器
    pub fn new() -> Self {
        Self {
            services: Arc::new(RwLock::new(HashMap::new())),
            singleton_instances: Arc::new(RwLock::new(HashMap::new())),
            scope_manager: Arc::new(RwLock::new(ScopeManager::new())),
            resolving: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    /// 注册服务描述符
    pub fn register_descriptor(&self, descriptor: Box<dyn ServiceDescriptor>) -> Result<(), DIError> {
        let type_id = descriptor.service_type();
        let mut services = self.lock_services_write()
            .map_err(|e| DIError::ServiceCreationFailed(format!("Lock acquisition failed: {}", e)))?;

        // 检查是否已注册
        if services.contains_key(&type_id) {
            return Err(DIError::InvalidServiceConfiguration(
                format!("Service already registered: {:?}", type_id),
            ));
        }

        services.insert(type_id, Arc::new(descriptor));
        Ok(())
    }
    
    /// 根据类型ID获取服务
    pub fn get_service_by_id(&self, type_id: TypeId) -> Result<Option<Arc<dyn Any + Send + Sync>>, DIError> {
        // 检查循环依赖
        {
            let mut resolving = self.lock_resolving_write()
                .map_err(|e| DIError::ServiceCreationFailed(format!("Lock acquisition failed: {}", e)))?;
            if let Some(pos) = resolving.iter().position(|&t| t == type_id) {
                let cycle = resolving[pos..].to_vec();
                return Err(DIError::CircularDependency(cycle));
            }
            resolving.push(type_id);
        }

        let result = self.get_service_internal(type_id);

        // 清理解析路径
        {
            let mut resolving = self.lock_resolving_write()
                .map_err(|e| DIError::ServiceCreationFailed(format!("Lock acquisition failed: {}", e)))?;
            resolving.pop();
        }

        result
    }
    
    /// 内部服务获取实现
    fn get_service_internal(&self, type_id: TypeId) -> Result<Option<Arc<dyn Any + Send + Sync>>, DIError> {
        // 获取服务描述符，避免克隆问题
        let services = self.lock_services_read()
            .map_err(|e| DIError::ServiceCreationFailed(format!("Lock acquisition failed: {}", e)))?;
        let descriptor = services.get(&type_id);

        let descriptor = match descriptor {
            Some(desc) => desc,
            None => return Ok(None),
        };

        // 根据生命周期获取实例
        match descriptor.lifetime() {
            ServiceLifetime::Singleton => self.get_singleton_instance(type_id, descriptor),
            ServiceLifetime::Transient => self.create_transient_instance(descriptor),
            ServiceLifetime::Scoped => self.get_scoped_instance(type_id, descriptor),
        }
    }
    
    /// 获取单例实例
    fn get_singleton_instance(
        &self,
        type_id: TypeId,
        descriptor: &Box<dyn ServiceDescriptor>,
    ) -> Result<Option<Arc<dyn Any + Send + Sync>>, DIError> {
        // 检查缓存
        {
            let instances = self.lock_singletons_read()
                .map_err(|e| DIError::ServiceCreationFailed(format!("Lock acquisition failed: {}", e)))?;
            if let Some(instance) = instances.get(&type_id) {
                // 将Any转换为Arc<dyn Any>
                if let Some(arc_any) = instance.as_any().downcast_ref::<Arc<dyn Any + Send + Sync>>() {
                    return Ok(Some(Arc::clone(arc_any)));
                }

                // 如果不是Arc包装的，需要转换
                // 这里需要更复杂的处理，暂时返回错误
                return Err(DIError::ServiceCreationFailed(
                    "Cannot convert singleton instance to Arc".to_string(),
                ));
            }
        }

        // 创建新实例
        let instance = descriptor.create_instance(self)?;
        let arc_instance: Arc<dyn Any + Send + Sync> = Arc::from(instance);

        // 缓存实例
        {
            let mut instances = self.lock_singletons_write()
                .map_err(|e| DIError::ServiceCreationFailed(format!("Lock acquisition failed: {}", e)))?;
            instances.insert(
                type_id,
                ServiceInstance::new(Box::new(arc_instance.clone()), ServiceLifetime::Singleton),
            );
        }

        Ok(Some(arc_instance))
    }
    
    /// 创建瞬态实例
    fn create_transient_instance(
        &self,
        descriptor: &Box<dyn ServiceDescriptor>,
    ) -> Result<Option<Arc<dyn Any + Send + Sync>>, DIError> {
        let instance = descriptor.create_instance(self)?;
        let arc_instance: Arc<dyn Any + Send + Sync> = Arc::from(instance);
        Ok(Some(arc_instance))
    }
    
    /// 获取作用域实例
    fn get_scoped_instance(
        &self,
        type_id: TypeId,
        descriptor: &Box<dyn ServiceDescriptor>,
    ) -> Result<Option<Arc<dyn Any + Send + Sync>>, DIError> {
        // 获取当前作用域
        let scope = {
            let scope_manager = self.lock_scope_manager_read()
                .map_err(|e| DIError::ServiceCreationFailed(format!("Lock acquisition failed: {}", e)))?;
            scope_manager.current_scope().cloned()
        };

        let scope = match scope {
            Some(s) => s,
            None => {
                // 如果没有作用域，当作瞬态处理
                return self.create_transient_instance(descriptor);
            }
        };

        // 检查作用域中是否已有实例
        if let Some(instance) = scope.get_service(type_id) {
            // 转换为Arc
            if let Some(arc_any) = instance.as_any().downcast_ref::<Arc<dyn Any + Send + Sync>>() {
                return Ok(Some(Arc::clone(arc_any)));
            }
        }

        // 创建新实例
        let instance = descriptor.create_instance(self)?;
        let arc_instance: Arc<dyn Any + Send + Sync> = Arc::from(instance);

        // 注册到作用域
        scope.register_service(
            type_id,
            ServiceInstance::new(Box::new(arc_instance.clone()), ServiceLifetime::Scoped),
        );

        Ok(Some(arc_instance))
    }
    
    /// 创建新作用域
    pub fn create_scope(&self) -> Result<Scope, DIError> {
        let mut scope_manager = self.lock_scope_manager_write()
            .map_err(|e| DIError::ServiceCreationFailed(format!("Lock acquisition failed: {}", e)))?;
        Ok(scope_manager.create_scope())
    }

    /// 销毁作用域
    pub fn destroy_scope(&self, scope_id: u64) {
        if let Ok(mut scope_manager) = self.lock_scope_manager_write() {
            scope_manager.destroy_scope(scope_id);
        }
        // Silently fail if lock cannot be acquired
    }
    
    /// 检查服务是否已注册
    pub fn is_registered(&self, type_id: TypeId) -> bool {
        match self.lock_services_read() {
            Ok(services) => services.contains_key(&type_id),
            Err(_) => false,
        }
    }

    /// 获取所有已注册的服务类型
    pub fn registered_services(&self) -> Vec<TypeId> {
        match self.lock_services_read() {
            Ok(services) => services.keys().cloned().collect(),
            Err(_) => Vec::new(),
        }
    }

    /// 清除所有服务（主要用于测试）
    pub fn clear(&self) {
        if let (Ok(mut services), Ok(mut instances), Ok(mut scope_manager)) = (
            self.lock_services_write(),
            self.lock_singletons_write(),
            self.lock_scope_manager_write(),
        ) {
            services.clear();
            instances.clear();

            // 清除所有作用域
            let scopes = scope_manager.current_scope().cloned();
            if let Some(scope) = scopes {
                scope_manager.destroy_scope(scope.id());
            }
        }
        // Silently fail if locks cannot be acquired
    }
    
    /// 预热服务（提前创建单例实例）
    pub fn warm_up(&self, service_types: Vec<TypeId>) -> Result<(), DIError> {
        for type_id in service_types {
            if self.is_registered(type_id) {
                self.get_service_by_id(type_id)?;
            }
        }
        Ok(())
    }
    
    /// 获取容器统计信息
    pub fn stats(&self) -> ContainerStats {
        match (self.lock_services_read(), self.lock_singletons_read(), self.lock_scope_manager_read()) {
            (Ok(services), Ok(instances), Ok(scope_manager)) => {
                ContainerStats {
                    registered_services: services.len(),
                    singleton_instances: instances.len(),
                    active_scopes: if scope_manager.current_scope().is_some() { 1 } else { 0 },
                }
            }
            _ => ContainerStats {
                registered_services: 0,
                singleton_instances: 0,
                active_scopes: 0,
            },
        }
    }
}

/// 容器统计信息
#[derive(Debug, Clone)]
pub struct ContainerStats {
    /// 已注册的服务数量
    pub registered_services: usize,
    /// 单例实例数量
    pub singleton_instances: usize,
    /// 活动作用域数量
    pub active_scopes: usize,
}

impl ServiceProvider for ServiceContainer {
    fn create_scope(&self) -> Result<Box<dyn ServiceProvider>, DIError> {
        let scope = self.create_scope()?;
        Ok(Box::new(ScopedServiceProvider::new(self, scope)))
    }
    
    fn get_service_by_id(&self, type_id: TypeId) -> Result<Option<Arc<dyn Any + Send + Sync>>, DIError> {
        self.get_service_by_id(type_id)
    }
}

/// ServiceProvider扩展trait，提供更便捷的方法
pub trait ServiceProviderExt {
    /// 获取必需的服务（如果不存在则返回错误）
    fn get_required_service<T: 'static + Send + Sync>(&self) -> Result<Arc<T>, DIError>;
    
    /// 获取可选的服务
    fn get_service<T: 'static + Send + Sync>(&self) -> Result<Option<Arc<T>>, DIError>;
}

impl ServiceProviderExt for ServiceContainer {
    fn get_required_service<T: 'static + Send + Sync>(&self) -> Result<Arc<T>, DIError> {
        let type_id = TypeId::of::<T>();
        match self.get_service_by_id(type_id)? {
            Some(service) => {
                service.downcast::<T>()
                    .map_err(|_| DIError::ServiceCreationFailed(
                        format!("Failed to downcast service to {}", std::any::type_name::<T>())
                    ))
            }
            None => Err(DIError::ServiceNotRegistered(type_id)),
        }
    }
    
    fn get_service<T: 'static + Send + Sync>(&self) -> Result<Option<Arc<T>>, DIError> {
        let type_id = TypeId::of::<T>();
        match self.get_service_by_id(type_id)? {
            Some(service) => {
                Ok(service.downcast::<T>().ok())
            }
            None => Ok(None),
        }
    }
}

impl ServiceProviderExt for Arc<ServiceContainer> {
    fn get_required_service<T: 'static + Send + Sync>(&self) -> Result<Arc<T>, DIError> {
        self.as_ref().get_required_service::<T>()
    }
    
    fn get_service<T: 'static + Send + Sync>(&self) -> Result<Option<Arc<T>>, DIError> {
        self.as_ref().get_service::<T>()
    }
}

/// 作用域服务提供者
pub struct ScopedServiceProvider {
    /// 父容器
    container: Arc<ServiceContainer>,
    /// 作用域
    scope: Scope,
}

impl ScopedServiceProvider {
    /// 创建新的作用域服务提供者
    pub fn new(container: &ServiceContainer, scope: Scope) -> Self {
        Self {
            container: Arc::new(container.clone()),
            scope,
        }
    }
    
    /// 获取作用域ID
    pub fn scope_id(&self) -> u64 {
        self.scope.id()
    }
}

impl ServiceProvider for ScopedServiceProvider {
    fn create_scope(&self) -> Result<Box<dyn ServiceProvider>, DIError> {
        let scope = self.container.create_scope()?;
        Ok(Box::new(ScopedServiceProvider::new(self.container.as_ref(), scope)))
    }

    fn get_service_by_id(&self, type_id: TypeId) -> Result<Option<Arc<dyn Any + Send + Sync>>, DIError> {
        // First try to find the instance in the current scope
        if let Some(inst) = self.scope.get_service(type_id) {
            if let Some(arc_any) = inst.as_any().downcast_ref::<Arc<dyn Any + Send + Sync>>() {
                return Ok(Some(Arc::clone(arc_any)));
            }
        }

        // Fallback: ask the parent container
        self.container.get_service_by_id(type_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::di_service_descriptor::GenericServiceDescriptor;
    
    #[test]
    fn test_container_creation() {
        let container = ServiceContainer::new();
        let stats = container.stats();
        assert_eq!(stats.registered_services, 0);
        assert_eq!(stats.singleton_instances, 0);
        assert_eq!(stats.active_scopes, 0);
    }
    
    #[test]
    fn test_service_registration() {
        let container = ServiceContainer::new();
        let descriptor = GenericServiceDescriptor::<String>::new(ServiceLifetime::Singleton);
        
        assert!(container.register_descriptor(Box::new(descriptor)).is_ok());
        assert!(container.is_registered::<String>());
    }
    
    #[test]
    fn test_duplicate_registration() {
        let container = ServiceContainer::new();
        let descriptor1 = GenericServiceDescriptor::<String>::new(ServiceLifetime::Singleton);
        let descriptor2 = GenericServiceDescriptor::<String>::new(ServiceLifetime::Singleton);
        
        assert!(container.register_descriptor(Box::new(descriptor1)).is_ok());
        assert!(container.register_descriptor(Box::new(descriptor2)).is_err());
    }
    
    #[test]
    fn test_scope_creation() {
        let container = ServiceContainer::new();
        let scope = container.create_scope().unwrap();
        assert!(scope.id() > 0);
        
        let stats = container.stats();
        assert_eq!(stats.active_scopes, 1);
        
        container.destroy_scope(scope.id());
        let stats = container.stats();
        assert_eq!(stats.active_scopes, 0);
    }
    
    #[test]
    fn test_service_not_registered() {
        let container = ServiceContainer::new();
        assert!(!container.is_registered::<String>());
        
        let result: Result<Option<Arc<String>>, DIError> = container.get_service::<String>();
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }
}