//! 服务注册表
//!
//! 本模块实现了服务注册表，提供服务注册、查询和管理功能。

use std::any::TypeId;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use super::di_service_descriptor::{
    DIError, GenericServiceDescriptor, ServiceDescriptor, ServiceLifetime, ServiceProvider,
};

/// 服务注册表
pub struct ServiceRegistry {
    /// 已注册的服务描述符
    services: Arc<RwLock<HashMap<TypeId, Box<dyn ServiceDescriptor>>>>,
    
    /// 服务名称映射（用于按名称查找）
    service_names: Arc<RwLock<HashMap<String, TypeId>>>,
    
    /// 服务类型映射（用于按类型查找）
    service_types: Arc<RwLock<HashMap<TypeId, String>>>,
    
    /// 服务分组
    service_groups: Arc<RwLock<HashMap<String, Vec<TypeId>>>>,
}

impl ServiceRegistry {
    /// 创建新的服务注册表
    pub fn new() -> Self {
        Self {
            services: Arc::new(RwLock::new(HashMap::new())),
            service_names: Arc::new(RwLock::new(HashMap::new())),
            service_types: Arc::new(RwLock::new(HashMap::new())),
            service_groups: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// 注册单例服务
    pub fn register_singleton<T: 'static + Send + Sync>(&self) -> Result<(), DIError> {
        let descriptor = GenericServiceDescriptor::<T>::new(ServiceLifetime::Singleton);
        self.register_descriptor(Box::new(descriptor))
    }
    
    /// 注册瞬态服务
    pub fn register_transient<T: 'static + Send + Sync>(&self) -> Result<(), DIError> {
        let descriptor = GenericServiceDescriptor::<T>::new(ServiceLifetime::Transient);
        self.register_descriptor(Box::new(descriptor))
    }
    
    /// 注册作用域服务
    pub fn register_scoped<T: 'static + Send + Sync>(&self) -> Result<(), DIError> {
        let descriptor = GenericServiceDescriptor::<T>::new(ServiceLifetime::Scoped);
        self.register_descriptor(Box::new(descriptor))
    }
    
    /// 注册服务实例
    pub fn register_instance<T: 'static + Send + Sync>(&self, instance: T) -> Result<(), DIError> {
        let descriptor = GenericServiceDescriptor::<T>::new(ServiceLifetime::Singleton)
            .with_instance(instance);
        self.register_descriptor(Box::new(descriptor))
    }
    
    /// 注册工厂函数
    pub fn register_factory<T: 'static + Send + Sync, F>(&self, factory: F) -> Result<(), DIError>
    where
        F: Fn(&dyn ServiceProvider) -> Result<T, DIError> + Send + Sync + 'static,
    {
        let descriptor = GenericServiceDescriptor::<T>::new(ServiceLifetime::Transient)
            .with_factory(factory);
        self.register_descriptor(Box::new(descriptor))
    }
    
    /// 注册带依赖的服务
    pub fn register_with_dependencies<T: 'static + Send + Sync>(
        &self,
        lifetime: ServiceLifetime,
        dependencies: Vec<TypeId>,
    ) -> Result<(), DIError> {
        let descriptor = GenericServiceDescriptor::<T>::new(lifetime)
            .with_dependencies(dependencies);
        self.register_descriptor(Box::new(descriptor))
    }
    
    /// 注册服务描述符
    pub fn register_descriptor(&self, descriptor: Box<dyn ServiceDescriptor>) -> Result<(), DIError> {
        let type_id = descriptor.service_type();
        let service_name = descriptor.service_name().to_string();
        
        // 检查是否已注册
        {
            let services = self.services.read().unwrap();
            if services.contains_key(&type_id) {
                return Err(DIError::InvalidServiceConfiguration(
                    format!("Service already registered: {:?}", type_id),
                ));
            }
        }
        
        // 注册服务
        {
            let mut services = self.services.write().unwrap();
            services.insert(type_id, descriptor);
        }
        
        // 更新名称映射
        {
            let mut service_names = self.service_names.write().unwrap();
            let mut service_types = self.service_types.write().unwrap();
            
            service_names.insert(service_name.clone(), type_id);
            service_types.insert(type_id, service_name);
        }
        
        Ok(())
    }
    
    /// 注册命名服务
    pub fn register_named<T: 'static + Send + Sync>(
        &self,
        name: &str,
        lifetime: ServiceLifetime,
    ) -> Result<(), DIError> {
        let type_id = TypeId::of::<T>();
        
        // 检查名称是否已被使用
        {
            let service_names = self.service_names.read().unwrap();
            if service_names.contains_key(name) {
                return Err(DIError::InvalidServiceConfiguration(
                    format!("Service name already in use: {}", name),
                ));
            }
        }
        
        let descriptor = GenericServiceDescriptor::<T>::new(lifetime);
        
        // 注册服务
        {
            let mut services = self.services.write().unwrap();
            services.insert(type_id, Box::new(descriptor));
        }
        
        // 更新名称映射
        {
            let mut service_names = self.service_names.write().unwrap();
            let mut service_types = self.service_types.write().unwrap();
            
            service_names.insert(name.to_string(), type_id);
            service_types.insert(type_id, name.to_string());
        }
        
        Ok(())
    }
    
    /// 将服务添加到分组
    pub fn add_to_group<T: 'static + Send + Sync>(&self, group: &str) -> Result<(), DIError> {
        let type_id = TypeId::of::<T>();
        
        // 检查服务是否已注册
        {
            let services = self.services.read().unwrap();
            if !services.contains_key(&type_id) {
                return Err(DIError::ServiceNotRegistered(type_id));
            }
        }
        
        // 添加到分组
        {
            let mut service_groups = self.service_groups.write().unwrap();
            let group_services = service_groups.entry(group.to_string()).or_insert_with(Vec::new);
            
            if !group_services.contains(&type_id) {
                group_services.push(type_id);
            }
        }
        
        Ok(())
    }
    
    /// 获取服务描述符
    pub fn get_descriptor(&self, type_id: TypeId) -> Option<Box<dyn ServiceDescriptor>> {
        let services = self.services.read().unwrap();
        if services.contains_key(&type_id) {
            // 这里需要克隆描述符，但ServiceDescriptor trait不能直接克隆
            // 在实际实现中，可能需要使用Arc或其他共享机制
            // 目前返回None作为占位符
            None
        } else {
            None
        }
    }
    
    /// 根据名称获取服务类型
    pub fn get_type_by_name(&self, name: &str) -> Option<TypeId> {
        let service_names = self.service_names.read().unwrap();
        service_names.get(name).cloned()
    }
    
    /// 根据类型获取服务名称
    pub fn get_name_by_type(&self, type_id: TypeId) -> Option<String> {
        let service_types = self.service_types.read().unwrap();
        service_types.get(&type_id).cloned()
    }
    
    /// 获取分组中的所有服务
    pub fn get_services_in_group(&self, group: &str) -> Vec<TypeId> {
        let service_groups = self.service_groups.read().unwrap();
        service_groups.get(group).cloned().unwrap_or_default()
    }
    
    /// 检查服务是否已注册
    pub fn is_registered(&self, type_id: TypeId) -> bool {
        let services = self.services.read().unwrap();
        services.contains_key(&type_id)
    }
    
    /// 检查命名服务是否已注册
    pub fn is_registered_name(&self, name: &str) -> bool {
        let service_names = self.service_names.read().unwrap();
        service_names.contains_key(name)
    }
    
    /// 获取所有已注册的服务类型
    pub fn registered_services(&self) -> Vec<TypeId> {
        let services = self.services.read().unwrap();
        services.keys().cloned().collect()
    }
    
    /// 获取所有服务名称
    pub fn service_names(&self) -> Vec<String> {
        let service_names = self.service_names.read().unwrap();
        service_names.keys().cloned().collect()
    }
    
    /// 获取所有分组名称
    pub fn service_groups(&self) -> Vec<String> {
        let service_groups = self.service_groups.read().unwrap();
        service_groups.keys().cloned().collect()
    }
    
    /// 注销服务
    pub fn unregister(&self, type_id: TypeId) -> Result<(), DIError> {
        // 检查服务是否存在
        {
            let services = self.services.read().unwrap();
            if !services.contains_key(&type_id) {
                return Err(DIError::ServiceNotRegistered(type_id));
            }
        }
        
        // 获取服务名称
        let service_name = {
            let service_types = self.service_types.read().unwrap();
            service_types.get(&type_id).cloned()
        };
        
        // 移除服务
        {
            let mut services = self.services.write().unwrap();
            services.remove(&type_id);
        }
        
        // 移除名称映射
        if let Some(name) = service_name {
            let mut service_names = self.service_names.write().unwrap();
            let mut service_types = self.service_types.write().unwrap();
            
            service_names.remove(&name);
            service_types.remove(&type_id);
        }
        
        // 从所有分组中移除
        {
            let mut service_groups = self.service_groups.write().unwrap();
            for (_, group_services) in service_groups.iter_mut() {
                group_services.retain(|&t| t != type_id);
            }
        }
        
        Ok(())
    }
    
    /// 注销命名服务
    pub fn unregister_name(&self, name: &str) -> Result<(), DIError> {
        let type_id = self.get_type_by_name(name)
            .ok_or_else(|| DIError::InvalidServiceConfiguration(
                format!("Service not found with name: {}", name),
            ))?;
        
        self.unregister(type_id)
    }
    
    /// 清空注册表
    pub fn clear(&self) {
        let mut services = self.services.write().unwrap();
        let mut service_names = self.service_names.write().unwrap();
        let mut service_types = self.service_types.write().unwrap();
        let mut service_groups = self.service_groups.write().unwrap();
        
        services.clear();
        service_names.clear();
        service_types.clear();
        service_groups.clear();
    }
    
    /// 获取注册表统计信息
    pub fn stats(&self) -> RegistryStats {
        let services = self.services.read().unwrap();
        let service_names = self.service_names.read().unwrap();
        let service_groups = self.service_groups.read().unwrap();
        
        let total_groups = service_groups.len();
        let mut total_grouped_services = 0;
        
        for (_, group_services) in service_groups.iter() {
            total_grouped_services += group_services.len();
        }
        
        RegistryStats {
            total_services: services.len(),
            named_services: service_names.len(),
            total_groups,
            total_grouped_services,
        }
    }
    
    /// 导出注册表配置
    pub fn export_config(&self) -> RegistryConfig {
        let services = self.services.read().unwrap();
        let service_types = self.service_types.read().unwrap();
        let service_groups = self.service_groups.read().unwrap();
        
        let mut service_configs = Vec::new();
        
        for (type_id, descriptor) in services.iter() {
            let name = service_types.get(type_id).cloned().unwrap_or_default();
            
            service_configs.push(ServiceConfig {
                type_id: *type_id,
                name,
                lifetime: descriptor.lifetime(),
                dependencies: descriptor.dependencies(),
            });
        }
        
        RegistryConfig {
            services: service_configs,
            groups: service_groups.clone(),
        }
    }
    
    /// 导入注册表配置
    pub fn import_config(&self, config: RegistryConfig) -> Result<(), DIError> {
        // 清空现有配置
        self.clear();
        
        // 导入服务配置
        for _service_config in config.services {
            // 这里需要根据配置创建服务描述符
            // 由于GenericServiceDescriptor需要具体类型，这里只是示例
            // 实际实现中可能需要更复杂的机制
        }
        
        // 导入分组配置
        {
            let mut service_groups = self.service_groups.write().unwrap();
            *service_groups = config.groups;
        }
        
        Ok(())
    }
}

/// 注册表统计信息
#[derive(Debug, Clone)]
pub struct RegistryStats {
    /// 总服务数
    pub total_services: usize,
    /// 命名服务数
    pub named_services: usize,
    /// 总分组数
    pub total_groups: usize,
    /// 分组中的服务总数
    pub total_grouped_services: usize,
}

/// 服务配置
#[derive(Debug, Clone)]
pub struct ServiceConfig {
    /// 服务类型ID
    pub type_id: TypeId,
    /// 服务名称
    pub name: String,
    /// 服务生命周期
    pub lifetime: ServiceLifetime,
    /// 依赖的服务类型
    pub dependencies: Vec<TypeId>,
}

/// 注册表配置
#[derive(Debug, Clone)]
pub struct RegistryConfig {
    /// 服务配置列表
    pub services: Vec<ServiceConfig>,
    /// 服务分组
    pub groups: HashMap<String, Vec<TypeId>>,
}

/// 服务注册构建器
pub struct ServiceRegistryBuilder {
    registry: ServiceRegistry,
}

impl ServiceRegistryBuilder {
    /// 创建新的注册表构建器
    pub fn new() -> Self {
        Self {
            registry: ServiceRegistry::new(),
        }
    }
    
    /// 注册单例服务
    pub fn with_singleton<T: 'static + Send + Sync>(self) -> Self {
        self.registry.register_singleton::<T>().unwrap();
        self
    }
    
    /// 注册瞬态服务
    pub fn with_transient<T: 'static + Send + Sync>(self) -> Self {
        self.registry.register_transient::<T>().unwrap();
        self
    }
    
    /// 注册作用域服务
    pub fn with_scoped<T: 'static + Send + Sync>(self) -> Self {
        self.registry.register_scoped::<T>().unwrap();
        self
    }
    
    /// 注册服务实例
    pub fn with_instance<T: 'static + Send + Sync>(self, instance: T) -> Self {
        self.registry.register_instance(instance).unwrap();
        self
    }
    
    /// 注册工厂函数
    pub fn with_factory<T: 'static + Send + Sync, F>(self, factory: F) -> Self
    where
        F: Fn(&dyn ServiceProvider) -> Result<T, DIError> + Send + Sync + 'static,
    {
        self.registry.register_factory(factory).unwrap();
        self
    }
    
    /// 添加到分组
    pub fn with_group<T: 'static + Send + Sync>(self, group: &str) -> Self {
        self.registry.add_to_group::<T>(group).unwrap();
        self
    }
    
    /// 构建注册表
    pub fn build(self) -> ServiceRegistry {
        self.registry
    }
}

impl Default for ServiceRegistryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_registry_creation() {
        let registry = ServiceRegistry::new();
        let stats = registry.stats();
        assert_eq!(stats.total_services, 0);
        assert_eq!(stats.named_services, 0);
        assert_eq!(stats.total_groups, 0);
    }
    
    #[test]
    fn test_register_singleton() {
        let registry = ServiceRegistry::new();
        assert!(registry.register_singleton::<String>().is_ok());
        assert!(registry.is_registered::<String>());
        
        let stats = registry.stats();
        assert_eq!(stats.total_services, 1);
    }
    
    #[test]
    fn test_register_transient() {
        let registry = ServiceRegistry::new();
        assert!(registry.register_transient::<String>().is_ok());
        assert!(registry.is_registered::<String>());
    }
    
    #[test]
    fn test_register_scoped() {
        let registry = ServiceRegistry::new();
        assert!(registry.register_scoped::<String>().is_ok());
        assert!(registry.is_registered::<String>());
    }
    
    #[test]
    fn test_register_instance() {
        let registry = ServiceRegistry::new();
        let instance = String::from("test");
        assert!(registry.register_instance(instance).is_ok());
        assert!(registry.is_registered::<String>());
    }
    
    #[test]
    fn test_register_named() {
        let registry = ServiceRegistry::new();
        assert!(registry.register_named::<String>("test_string", ServiceLifetime::Singleton).is_ok());
        assert!(registry.is_registered::<String>());
        assert!(registry.is_registered_name("test_string"));
        
        let type_id = registry.get_type_by_name("test_string").unwrap();
        assert_eq!(type_id, TypeId::of::<String>());
        
        let name = registry.get_name_by_type(TypeId::of::<String>()).unwrap();
        assert_eq!(name, "test_string");
    }
    
    #[test]
    fn test_service_groups() {
        let registry = ServiceRegistry::new();
        registry.register_singleton::<String>().unwrap();
        registry.add_to_group::<String>("test_group").unwrap();
        
        let groups = registry.service_groups();
        assert_eq!(groups.len(), 1);
        assert!(groups.contains(&"test_group".to_string()));
        
        let services = registry.get_services_in_group("test_group");
        assert_eq!(services.len(), 1);
        assert_eq!(services[0], TypeId::of::<String>());
    }
    
    #[test]
    fn test_unregister() {
        let registry = ServiceRegistry::new();
        registry.register_named::<String>("test_string", ServiceLifetime::Singleton).unwrap();
        
        assert!(registry.is_registered::<String>());
        assert!(registry.is_registered_name("test_string"));
        
        registry.unregister_name("test_string").unwrap();
        
        assert!(!registry.is_registered::<String>());
        assert!(!registry.is_registered_name("test_string"));
    }
    
    #[test]
    fn test_registry_builder() {
        let registry = ServiceRegistryBuilder::new()
            .with_singleton::<String>()
            .with_transient::<i32>()
            .with_group::<String>("test_group")
            .build();
        
        assert!(registry.is_registered::<String>());
        assert!(registry.is_registered::<i32>());
        
        let services = registry.get_services_in_group("test_group");
        assert_eq!(services.len(), 1);
        assert_eq!(services[0], TypeId::of::<String>());
    }
}