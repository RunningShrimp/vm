//! 依赖注入器
//!
//! 本模块实现了依赖注入的核心逻辑，支持构造函数注入、属性注入和方法注入。

use std::any::TypeId;
use std::collections::HashMap;
use std::sync::Arc;

use super::di_service_descriptor::{DIError, ServiceProvider};

/// 依赖注入器
pub struct DependencyInjector {
    /// 注入策略
    strategy: InjectionStrategy,
    
    /// 类型信息缓存
    type_info_cache: Arc<std::sync::RwLock<HashMap<TypeId, TypeInfo>>>,
}

/// 注入策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InjectionStrategy {
    /// 构造函数注入
    Constructor,
    /// 属性注入
    Property,
    /// 方法注入
    Method,
    /// 自动检测
    Auto,
}

/// 类型信息
#[derive(Debug, Clone)]
pub struct TypeInfo {
    /// 类型ID
    pub type_id: TypeId,
    /// 构造函数参数类型
    pub constructor_params: Vec<TypeId>,
    /// 可注入的属性
    pub injectable_properties: Vec<PropertyInfo>,
    /// 可注入的方法
    pub injectable_methods: Vec<MethodInfo>,
}

/// 属性信息
#[derive(Debug, Clone)]
pub struct PropertyInfo {
    /// 属性名称
    pub name: String,
    /// 属性类型
    pub property_type: TypeId,
    /// 是否可选
    pub optional: bool,
}

/// 方法信息
#[derive(Debug, Clone)]
pub struct MethodInfo {
    /// 方法名称
    pub name: String,
    /// 参数类型
    pub param_types: Vec<TypeId>,
    /// 参数名称
    pub param_names: Vec<String>,
}

/// 注入点
#[derive(Debug, Clone)]
pub enum InjectionPoint {
    /// 构造函数参数
    ConstructorParam { index: usize, param_type: TypeId },
    /// 属性
    Property { name: String, property_type: TypeId, optional: bool },
    /// 方法参数
    MethodParam { method: String, index: usize, param_type: TypeId },
}

/// 注入结果
#[derive(Debug)]
pub struct InjectionResult {
    /// 成功注入的服务数量
    pub injected_count: usize,
    /// 注入失败的点
    pub failed_injections: Vec<InjectionPoint>,
}

impl DependencyInjector {
    /// 创建新的依赖注入器
    pub fn new(strategy: InjectionStrategy) -> Self {
        Self {
            strategy,
            type_info_cache: Arc::new(std::sync::RwLock::new(HashMap::new())),
        }
    }

    /// Helper method to acquire read lock
    fn lock_read(&self) -> Result<std::sync::RwLockReadGuard<'_, HashMap<TypeId, TypeInfo>>, DIError> {
        self.type_info_cache.read().map_err(|e| {
            DIError::ServiceCreationFailed(format!("Failed to acquire read lock: {}", e))
        })
    }

    /// Helper method to acquire write lock
    fn lock_write(&self) -> Result<std::sync::RwLockWriteGuard<'_, HashMap<TypeId, TypeInfo>>, DIError> {
        self.type_info_cache.write().map_err(|e| {
            DIError::ServiceCreationFailed(format!("Failed to acquire write lock: {}", e))
        })
    }
    
    /// 注入依赖到目标对象
    pub fn inject_into<T: 'static + Send + Sync>(
        &self,
        target: &mut T,
        provider: &dyn ServiceProvider,
    ) -> Result<InjectionResult, DIError> {
        let type_id = TypeId::of::<T>();
        let type_info = self.get_type_info(type_id)?;
        
        let mut injected_count = 0;
        let mut failed_injections = Vec::new();
        
        match self.strategy {
            InjectionStrategy::Constructor => {
                // 构造函数注入在对象创建时完成，这里不做处理
                Ok(InjectionResult {
                    injected_count: 0,
                    failed_injections: Vec::new(),
                })
            }
            InjectionStrategy::Property => {
                self.inject_properties(target, &type_info, provider, &mut injected_count, &mut failed_injections)
            }
            InjectionStrategy::Method => {
                self.inject_methods(target, &type_info, provider, &mut injected_count, &mut failed_injections)
            }
            InjectionStrategy::Auto => {
                // 尝试属性注入，然后方法注入
                let property_result = self.inject_properties(target, &type_info, provider, &mut injected_count, &mut failed_injections)?;
                let method_result = self.inject_methods(target, &type_info, provider, &mut injected_count, &mut failed_injections)?;
                
                Ok(InjectionResult {
                    injected_count: property_result.injected_count + method_result.injected_count,
                    failed_injections: property_result.failed_injections,
                })
            }
        }
    }
    
    /// 创建新实例并注入依赖
    pub fn create_with_injection<T: 'static + Send + Sync>(
        &self,
        provider: &dyn ServiceProvider,
    ) -> Result<T, DIError> {
        let type_id = TypeId::of::<T>();
        let type_info = self.get_type_info(type_id)?;
        
        match self.strategy {
            InjectionStrategy::Constructor | InjectionStrategy::Auto => {
                self.create_with_constructor_injection::<T>(&type_info, provider)
            }
            InjectionStrategy::Property | InjectionStrategy::Method => {
                // 先创建默认实例，再进行属性或方法注入
                let mut instance = self.create_default_instance::<T>()?;
                self.inject_into(&mut instance, provider)?;
                Ok(instance)
            }
        }
    }
    
    /// 获取类型信息
    fn get_type_info(&self, type_id: TypeId) -> Result<TypeInfo, DIError> {
        {
            let cache = self.lock_read()?;
            if let Some(info) = cache.get(&type_id) {
                return Ok(info.clone());
            }
        }

        // 这里应该使用反射来获取类型信息
        // 由于Rust没有内置反射，我们使用默认实现
        let info = self.extract_type_info(type_id)?;

        {
            let mut cache = self.lock_write()?;
            cache.insert(type_id, info.clone());
        }

        Ok(info)
    }
    
    /// 提取类型信息
    fn extract_type_info(&self, type_id: TypeId) -> Result<TypeInfo, DIError> {
        // 在实际实现中，这里应该使用反射或编译时代码生成
        // 目前返回默认信息
        Ok(TypeInfo {
            type_id,
            constructor_params: Vec::new(),
            injectable_properties: Vec::new(),
            injectable_methods: Vec::new(),
        })
    }
    
    /// 创建默认实例
    fn create_default_instance<T: 'static + Send + Sync>(&self) -> Result<T, DIError> {
        // 在实际实现中，这里应该使用默认构造函数
        // 目前返回错误，表示需要显式构造
        Err(DIError::ServiceCreationFailed(
            "Cannot create default instance without constructor information".to_string(),
        ))
    }
    
    /// 构造函数注入
    fn create_with_constructor_injection<T: 'static + Send + Sync>(
        &self,
        type_info: &TypeInfo,
        _provider: &dyn ServiceProvider,
    ) -> Result<T, DIError> {
        // 在实际实现中，这里应该根据构造函数参数创建实例
        // 目前返回错误，表示需要更多信息
        if type_info.constructor_params.is_empty() {
            return self.create_default_instance::<T>();
        }
        
        Err(DIError::ServiceCreationFailed(
            "Constructor injection not fully implemented".to_string(),
        ))
    }
    
    /// 属性注入
    fn inject_properties<T: 'static + Send + Sync>(
        &self,
        _target: &mut T,
        type_info: &TypeInfo,
        provider: &dyn ServiceProvider,
        injected_count: &mut usize,
        failed_injections: &mut Vec<InjectionPoint>,
    ) -> Result<InjectionResult, DIError> {
        for property in &type_info.injectable_properties {
            match self.inject_property(property, provider) {
                Ok(_) => *injected_count += 1,
                Err(_) => {
                    failed_injections.push(InjectionPoint::Property {
                        name: property.name.clone(),
                        property_type: property.property_type,
                        optional: property.optional,
                    });
                }
            }
        }
        
        Ok(InjectionResult {
            injected_count: *injected_count,
            failed_injections: failed_injections.clone(),
        })
    }
    
    /// 注入单个属性
    fn inject_property(
        &self,
        property: &PropertyInfo,
        provider: &dyn ServiceProvider,
    ) -> Result<(), DIError> {
        // 在实际实现中，这里应该设置对象的属性值
        // 目前只是尝试获取依赖
        let _service = provider.get_service_by_id(property.property_type)?;
        Ok(())
    }
    
    /// 方法注入
    fn inject_methods<T: 'static + Send + Sync>(
        &self,
        _target: &mut T,
        type_info: &TypeInfo,
        provider: &dyn ServiceProvider,
        injected_count: &mut usize,
        failed_injections: &mut Vec<InjectionPoint>,
    ) -> Result<InjectionResult, DIError> {
        for method in &type_info.injectable_methods {
            match self.inject_method(method, provider) {
                Ok(_) => *injected_count += 1,
                Err(_) => {
                    for (index, &param_type) in method.param_types.iter().enumerate() {
                        failed_injections.push(InjectionPoint::MethodParam {
                            method: method.name.clone(),
                            index,
                            param_type,
                        });
                    }
                }
            }
        }
        
        Ok(InjectionResult {
            injected_count: *injected_count,
            failed_injections: failed_injections.clone(),
        })
    }
    
    /// 注入单个方法
    fn inject_method(
        &self,
        method: &MethodInfo,
        provider: &dyn ServiceProvider,
    ) -> Result<(), DIError> {
        // 在实际实现中，这里应该调用对象的方法
        // 目前只是尝试获取所有依赖
        for &param_type in &method.param_types {
            let _service = provider.get_service_by_id(param_type)?;
        }
        Ok(())
    }
    
    /// 注册类型信息
    pub fn register_type_info(&self, type_info: TypeInfo) {
        if let Ok(mut cache) = self.lock_write() {
            cache.insert(type_info.type_id, type_info);
        }
    }

    /// 清除类型信息缓存
    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.lock_write() {
            cache.clear();
        }
    }
    
    /// 获取注入器统计信息
    pub fn stats(&self) -> InjectorStats {
        let cache = match self.lock_read() {
            Ok(cache) => cache,
            Err(_) => {
                return InjectorStats {
                    total_types: 0,
                    total_properties: 0,
                    total_methods: 0,
                    average_properties: 0.0,
                    average_methods: 0.0,
                }
            }
        };

        let total_types = cache.len();
        let total_properties = cache.values().map(|info| info.injectable_properties.len()).sum();
        let total_methods = cache.values().map(|info| info.injectable_methods.len()).sum();

        InjectorStats {
            total_types,
            total_properties,
            total_methods,
            average_properties: if total_types == 0 {
                0.0
            } else {
                total_properties as f64 / total_types as f64
            },
            average_methods: if total_types == 0 {
                0.0
            } else {
                total_methods as f64 / total_types as f64
            },
        }
    }
}

/// 注入器统计信息
#[derive(Debug, Clone)]
pub struct InjectorStats {
    /// 总类型数
    pub total_types: usize,
    /// 总属性数
    pub total_properties: usize,
    /// 总方法数
    pub total_methods: usize,
    /// 平均属性数
    pub average_properties: f64,
    /// 平均方法数
    pub average_methods: f64,
}

/// 依赖注入宏辅助trait
pub trait DIConstruct {
    /// 获取构造函数参数类型
    fn constructor_param_types() -> Vec<TypeId>;
}

/// 属性注入标记trait
pub trait DIInject {
    /// 获取可注入的属性信息
    fn injectable_properties() -> Vec<PropertyInfo>;
}

/// 方法注入标记trait
pub trait DIMethodInject {
    /// 获取可注入的方法信息
    fn injectable_methods() -> Vec<MethodInfo>;
}

/// 手动类型信息构建器
pub struct TypeInfoBuilder {
    type_id: TypeId,
    constructor_params: Vec<TypeId>,
    injectable_properties: Vec<PropertyInfo>,
    injectable_methods: Vec<MethodInfo>,
}

impl TypeInfoBuilder {
    /// 创建新的类型信息构建器
    pub fn new<T: 'static + Send + Sync>() -> Self {
        Self {
            type_id: TypeId::of::<T>(),
            constructor_params: Vec::new(),
            injectable_properties: Vec::new(),
            injectable_methods: Vec::new(),
        }
    }
    
    /// 添加构造函数参数
    pub fn with_constructor_param<T: 'static + Send + Sync>(mut self) -> Self {
        self.constructor_params.push(TypeId::of::<T>());
        self
    }
    
    /// 添加可注入属性
    pub fn with_property<T: 'static + Send + Sync>(mut self, name: &str, optional: bool) -> Self {
        self.injectable_properties.push(PropertyInfo {
            name: name.to_string(),
            property_type: TypeId::of::<T>(),
            optional,
        });
        self
    }

    /// 添加可注入方法
    pub fn with_method(mut self, name: &str, param_types: Vec<TypeId>) -> Self {
        let param_names = (0..param_types.len())
            .map(|i| format!("param{}", i))
            .collect();
        
        self.injectable_methods.push(MethodInfo {
            name: name.to_string(),
            param_types,
            param_names,
        });
        self
    }

    /// 构建类型信息
    pub fn build(self) -> TypeInfo {
        TypeInfo {
            type_id: self.type_id,
            constructor_params: self.constructor_params,
            injectable_properties: self.injectable_properties,
            injectable_methods: self.injectable_methods,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::di_service_descriptor::GenericServiceDescriptor;
    
    struct TestService {
        value: String,
    }
    
    impl TestService {
        fn new(value: String) -> Self {
            Self { value }
        }
    }
    
    #[test]
    fn test_dependency_injector_creation() {
        let injector = DependencyInjector::new(InjectionStrategy::Auto);
        let stats = injector.stats();
        assert_eq!(stats.total_types, 0);
    }
    
    #[test]
    fn test_type_info_builder() {
        let type_info = TypeInfoBuilder::new::<TestService>()
            .with_constructor_param::<String>()
            .with_property::<String>("value", false)
            .build();
        
        assert_eq!(type_info.type_id, TypeId::of::<TestService>());
        assert_eq!(type_info.constructor_params.len(), 1);
        assert_eq!(type_info.injectable_properties.len(), 1);
    }
    
    #[test]
    fn test_register_type_info() {
        let injector = DependencyInjector::new(InjectionStrategy::Auto);
        let type_info = TypeInfoBuilder::new::<TestService>().build();
        
        injector.register_type_info(type_info);
        
        let stats = injector.stats();
        assert_eq!(stats.total_types, 1);
    }
    
    #[test]
    fn test_clear_cache() {
        let injector = DependencyInjector::new(InjectionStrategy::Auto);
        let type_info = TypeInfoBuilder::new::<TestService>().build();
        
        injector.register_type_info(type_info);
        assert_eq!(injector.stats().total_types, 1);
        
        injector.clear_cache();
        assert_eq!(injector.stats().total_types, 0);
    }
}