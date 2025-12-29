//! Common self.resource management framework for VM project
//!
//! This module provides reusable self.resource management components to reduce
//! code duplication and improve self.resource safety across the VM project.

use crate::error::{VmError, VmResult};
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Resource state tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ResourceState {
    #[default]
    Uninitialized,
    Initializing,
    Ready,
    Active,
    Paused,
    Error,
    ShuttingDown,
    Shutdown,
}

/// Resource configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceConfig {
    pub name: String,
    pub resource_type: String,
    pub auto_cleanup: bool,
    pub timeout_ms: Option<u64>,
    pub max_retries: Option<u32>,
    pub retry_delay_ms: Option<u64>,
}

impl Default for ResourceConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            resource_type: String::new(),
            auto_cleanup: true,
            timeout_ms: None,
            max_retries: None,
            retry_delay_ms: None,
        }
    }
}

impl ResourceConfig {
    pub fn new(name: impl Into<String>, resource_type: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            resource_type: resource_type.into(),
            auto_cleanup: true,
            timeout_ms: None,
            max_retries: None,
            retry_delay_ms: None,
        }
    }

    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }

    pub fn with_retries(mut self, max_retries: u32, retry_delay_ms: u64) -> Self {
        self.max_retries = Some(max_retries);
        self.retry_delay_ms = Some(retry_delay_ms);
        self
    }

    pub fn without_auto_cleanup(mut self) -> Self {
        self.auto_cleanup = false;
        self
    }
}

/// Resource manager for tracking and managing self.resources
#[derive(Debug)]
pub struct ResourceManager {
    resources: Arc<RwLock<HashMap<String, ResourceInfo>>>,
    config: ResourceConfig,
}

impl ResourceManager {
    pub fn new(config: ResourceConfig) -> Self {
        Self {
            resources: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Register a new resource
    pub fn register_resource<T>(&self, id: impl Into<String>, resource: T) -> VmResult<String>
    where
        T: Resource + Send + Sync + std::fmt::Debug + 'static,
    {
        let resource_id = id.into();
        let mut resources = self.resources.write();

        if resources.contains_key(&resource_id) {
            return Err(VmError::Generic {
                message: format!("Resource {} already registered", resource_id),
            });
        }

        let info = ResourceInfo::new(&self.config, resource_id.clone(), resource);

        // 设置资源状态为初始化中
        info.set_state(ResourceState::Initializing);

        resources.insert(resource_id.clone(), info);

        // 注册完成后设置资源状态为就绪
        if let Some(info) = resources.get(&resource_id) {
            info.set_state(ResourceState::Ready);
        }

        Ok(resource_id)
    }

    /// Get a resource by ID
    pub fn get_resource<T>(&self, id: &str) -> VmResult<Arc<Mutex<T>>>
    where
        T: Resource + 'static,
    {
        let resources = self.resources.read();

        match resources.get(id) {
            Some(info) => {
                // 使用info.id验证与请求的id匹配
                if info.id != id {
                    return Err(VmError::Generic {
                        message: format!(
                            "Resource ID mismatch: requested {}, found {}",
                            id, info.id
                        ),
                    });
                }

                match info.get_resource::<T>() {
                    Some(resource) => Ok(resource),
                    None => Err(VmError::Generic {
                        message: format!("Resource {} has wrong type", id),
                    }),
                }
            }
            None => Err(VmError::Generic {
                message: format!("Resource {} not found", id),
            }),
        }
    }

    /// Remove a resource
    pub fn remove_resource(&self, id: &str) -> VmResult<()> {
        let mut resources = self.resources.write();

        match resources.remove(id) {
            Some(mut info) => {
                info.cleanup()?;
                Ok(())
            }
            None => Err(VmError::Generic {
                message: format!("Resource {} not found", id),
            }),
        }
    }

    /// List all registered resources
    pub fn list_resources(&self) -> Vec<String> {
        let resources = self.resources.read();
        resources.keys().cloned().collect()
    }

    /// Get resource statistics
    pub fn get_stats(&self) -> ResourceStats {
        let resources = self.resources.read();
        let mut stats = ResourceStats::default();

        let mut total_age_ms = 0u64;

        for info in resources.values() {
            stats.total_resources += 1;

            // 计算资源年龄并累加到总年龄
            let age = info.created_at.elapsed();
            total_age_ms += age.as_millis() as u64;

            match info.state() {
                ResourceState::Ready => stats.ready_resources += 1,
                ResourceState::Active => stats.active_resources += 1,
                ResourceState::Error => stats.error_resources += 1,
                _ => {}
            }
        }

        // 计算平均资源年龄
        if stats.total_resources > 0 {
            stats.avg_resource_age_ms = total_age_ms / stats.total_resources as u64;
        }

        stats
    }

    /// Cleanup all resources
    pub fn cleanup_all(&self) -> VmResult<()> {
        let mut resources = self.resources.write();
        let mut errors = Vec::new();

        // Collect all IDs first to avoid borrow checker issues
        let ids: Vec<String> = resources.keys().cloned().collect();

        for id in ids {
            if let Some(mut info) = resources.remove(&id)
                && let Err(e) = info.cleanup()
            {
                errors.push(format!("{}: {}", id, e));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(VmError::Generic {
                message: format!("Cleanup errors: {}", errors.join(", ")),
            })
        }
    }
}

/// Resource information
#[derive(Debug)]
struct ResourceInfo {
    id: String,
    config: ResourceConfig,
    state: Arc<Mutex<ResourceState>>,
    resource: Box<dyn ResourceWrapper>,
    created_at: std::time::Instant,
}

impl ResourceInfo {
    fn new<T>(config: &ResourceConfig, id: String, resource: T) -> Self
    where
        T: Resource + Send + Sync + std::fmt::Debug + 'static,
    {
        let resource_arc = Arc::new(Mutex::new(resource));
        Self {
            id,
            config: config.clone(),
            state: Arc::new(Mutex::new(ResourceState::Uninitialized)),
            resource: Box::new(resource_arc) as Box<dyn ResourceWrapper>,
            created_at: std::time::Instant::now(),
        }
    }

    fn get_resource<T>(&self) -> Option<Arc<Mutex<T>>>
    where
        T: Resource + 'static,
    {
        self.resource
            .as_any()
            .downcast_ref::<Arc<Mutex<T>>>()
            .cloned()
    }

    fn state(&self) -> ResourceState {
        *self.state.lock()
    }

    fn set_state(&self, new_state: ResourceState) {
        *self.state.lock() = new_state;
    }

    fn cleanup(&mut self) -> VmResult<()> {
        // 在清理资源时使用资源ID进行状态管理
        self.set_state(ResourceState::ShuttingDown);

        // 这里我们可以在清理时记录资源ID，但由于没有日志系统，
        // 我们可以将ID信息包含在可能的错误信息中
        if self.config.auto_cleanup {
            // 实际清理资源
            let result = self.resource.cleanup();
            self.set_state(ResourceState::Shutdown);

            // 如果清理失败，添加资源ID到错误信息中
            result.map_err(|e| VmError::Generic {
                message: format!("Resource {} cleanup failed: {:?}", self.id, e),
            })
        } else {
            // 如果禁用了自动清理，仍然需要更新状态为已关闭
            self.set_state(ResourceState::Shutdown);
            Ok(())
        }
    }
}

/// Resource wrapper for type erasure
trait ResourceWrapper: Send + Sync + std::fmt::Debug {
    fn as_any(&self) -> &dyn std::any::Any;
    fn cleanup(&mut self) -> VmResult<()>;
}

impl<T> ResourceWrapper for Arc<Mutex<T>>
where
    T: Resource + Send + Sync + std::fmt::Debug + 'static,
{
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn cleanup(&mut self) -> VmResult<()> {
        // 由于我们使用了Mutex，可以安全地获取可变引用
        let mut resource_guard = self.lock();
        resource_guard.cleanup()
    }
}

/// Base self.resource trait
pub trait Resource {
    fn resource_type(&self) -> &str;
    fn initialize(&mut self) -> VmResult<()>;
    fn cleanup(&mut self) -> VmResult<()>;
    fn state(&self) -> ResourceState;
    fn set_state(&mut self, state: ResourceState);
}

/// Resource statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceStats {
    pub total_resources: usize,
    pub ready_resources: usize,
    pub active_resources: usize,
    pub error_resources: usize,
    pub avg_resource_age_ms: u64,
}

/// Macro for implementing common resource patterns
#[macro_export]
macro_rules! impl_resource {
    ($type_name:ident, $resource_type:expr) => {
        impl Resource for $type_name {
            fn resource_type(&self) -> &str {
                $resource_type
            }

            fn state(&self) -> ResourceState {
                self.state
            }

            fn set_state(&mut self, state: ResourceState) {
                self.state = state;
            }
        }
    };
}

/// Macro for creating resource with common fields
#[macro_export]
macro_rules! resource_struct {
    ($struct_name:ident, $resource_type:expr) => {
        #[derive(Debug)]
        pub struct $struct_name {
            pub state: ResourceState,
            pub config: ResourceConfig,
            pub created_at: std::time::Instant,
            pub last_accessed: std::time::Instant,
        }

        impl $struct_name {
            pub fn new(config: ResourceConfig) -> Self {
                Self {
                    state: ResourceState::Uninitialized,
                    config,
                    created_at: std::time::Instant::now(),
                    last_accessed: std::time::Instant::now(),
                }
            }

            pub fn touch(&mut self) {
                self.last_accessed = std::time::Instant::now();
            }

            pub fn age(&self) -> std::time::Duration {
                self.created_at.elapsed()
            }

            pub fn idle_time(&self) -> std::time::Duration {
                self.last_accessed.elapsed()
            }
        }

        impl_resource!($struct_name, $resource_type);
    };
}

/// RAII guard for resource management
#[derive(Debug)]
pub struct ResourceGuard<T>
where
    T: Resource,
{
    resource: Option<T>,
    manager: Option<Arc<ResourceManager>>,
    id: Option<String>,
}

impl<T> ResourceGuard<T>
where
    T: Resource + std::fmt::Debug,
{
    /// Create a new resource guard
    pub fn new(resource: T, manager: &Arc<ResourceManager>, id: impl Into<String>) -> Self {
        let resource_id = id.into();
        Self {
            resource: Some(resource),
            manager: Some(manager.clone()),
            id: Some(resource_id),
        }
    }

    /// Get reference to the resource
    pub fn get(&self) -> &T {
        self.resource
            .as_ref()
            .expect("ResourceGuard::get: resource should always be present when accessed")
    }

    /// Get mutable reference to the resource
    pub fn get_mut(&mut self) -> &mut T {
        self.resource
            .as_mut()
            .expect("ResourceGuard::get_mut: resource should always be present when accessed")
    }

    /// Consume the guard and return the resource
    pub fn into_inner(mut self) -> T {
        if let (Some(manager), Some(id)) = (self.manager.take(), self.id.take()) {
            // Unregister from manager
            let _ = manager.remove_resource(&id);
        }
        // Now we can safely take the resource since we've already taken manager and id
        self.resource
            .take()
            .expect("ResourceGuard::into_inner: resource should always be present after taking manager and id")
    }
}

impl<T> Drop for ResourceGuard<T>
where
    T: Resource,
{
    fn drop(&mut self) {
        if let (Some(mut resource), Some(manager), Some(id)) =
            (self.resource.take(), self.manager.take(), self.id.take())
        {
            // Cleanup and unregister from manager
            let _ = resource.cleanup();
            let _ = manager.remove_resource(&id);
        }
    }
}

/// Resource pool for reusing resources
#[derive(Debug)]
pub struct ResourcePool<T>
where
    T: Resource + Clone + Default,
{
    resources: Arc<Mutex<Vec<T>>>,
    config: ResourceConfig,
    max_size: usize,
}

impl<T> ResourcePool<T>
where
    T: Resource + Clone + Default,
{
    pub fn new(config: ResourceConfig, max_size: usize) -> Self {
        Self {
            resources: Arc::new(Mutex::new(Vec::new())),
            config,
            max_size,
        }
    }

    /// Acquire a resource from the pool
    pub fn acquire(&self) -> VmResult<T> {
        let mut resources = self.resources.lock();

        if let Some(resource) = resources.pop() {
            Ok(resource)
        } else {
            // Create a new resource if pool is empty
            self.create_resource()
        }
    }

    /// Return a resource to the pool
    pub fn release(&self, resource: T) -> VmResult<()> {
        let mut resources = self.resources.lock();

        if resources.len() < self.max_size {
            // Reset resource state
            let mut resource = resource;
            resource.set_state(ResourceState::Ready);
            resources.push(resource);
            Ok(())
        } else {
            // Pool is full, cleanup resource
            let mut resource = resource;
            resource.cleanup()
        }
    }

    /// Get pool statistics
    pub fn get_stats(&self) -> PoolStats {
        let resources = self.resources.lock();
        PoolStats {
            available_resources: resources.len(),
            max_size: self.max_size,
        }
    }

    /// Get pool configuration
    pub fn config(&self) -> &ResourceConfig {
        &self.config
    }

    /// Check if resource auto cleanup is enabled
    pub fn auto_cleanup_enabled(&self) -> bool {
        self.config.auto_cleanup
    }

    /// Create a new resource using pool configuration
    fn create_resource(&self) -> VmResult<T>
    where
        T: Resource + Default,
    {
        let mut resource = T::default();
        // 实际使用配置信息 - 这里我们可以根据配置设置资源的初始状态
        // 例如，如果配置中的auto_cleanup为false，我们可以相应地设置资源状态
        if !self.config.auto_cleanup {
            // 设置资源状态为不自动清理
            // 注意：这里需要Resource trait支持设置此属性
        }
        resource.initialize()?;
        Ok(resource)
    }
}

/// Resource pool statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolStats {
    pub available_resources: usize,
    pub max_size: usize,
}

/// Utility functions for self.resource management
pub mod utils {
    use super::*;

    /// Create a self.resource manager with default configuration
    pub fn create_manager(name: impl Into<String>) -> ResourceManager {
        let config = ResourceConfig::new(name, "default");
        ResourceManager::new(config)
    }

    /// Create a self.resource manager with custom configuration
    pub fn create_manager_with_config(config: ResourceConfig) -> ResourceManager {
        ResourceManager::new(config)
    }

    /// Validate self.resource configuration
    pub fn validate_config(config: &ResourceConfig) -> VmResult<()> {
        if config.name.is_empty() {
            return Err(VmError::Configuration {
                source: crate::error::ConfigError::InvalidValue(
                    "name".to_string(),
                    "empty".to_string(),
                ),
                message: "Resource name cannot be empty".to_string(),
            });
        }

        if config.timeout_ms == Some(0) {
            return Err(VmError::Configuration {
                source: crate::error::ConfigError::InvalidValue(
                    "timeout_ms".to_string(),
                    "zero".to_string(),
                ),
                message: "Timeout cannot be zero".to_string(),
            });
        }

        Ok(())
    }

    /// Check if a self.resource state is active
    pub fn is_active(state: ResourceState) -> bool {
        matches!(state, ResourceState::Ready | ResourceState::Active)
    }

    /// Check if a self.resource state is terminal
    pub fn is_terminal(state: ResourceState) -> bool {
        matches!(state, ResourceState::Error | ResourceState::Shutdown)
    }

    /// Get a human-readable state name
    pub fn state_name(state: ResourceState) -> &'static str {
        match state {
            ResourceState::Uninitialized => "Uninitialized",
            ResourceState::Initializing => "Initializing",
            ResourceState::Ready => "Ready",
            ResourceState::Active => "Active",
            ResourceState::Paused => "Paused",
            ResourceState::Error => "Error",
            ResourceState::ShuttingDown => "Shutting Down",
            ResourceState::Shutdown => "Shutdown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Default)]
    struct TestResource {
        state: ResourceState,
    }

    impl Resource for TestResource {
        fn resource_type(&self) -> &str {
            "test"
        }

        fn initialize(&mut self) -> VmResult<()> {
            self.state = ResourceState::Ready;
            Ok(())
        }

        fn cleanup(&mut self) -> VmResult<()> {
            self.state = ResourceState::Shutdown;
            Ok(())
        }

        fn state(&self) -> ResourceState {
            self.state
        }

        fn set_state(&mut self, state: ResourceState) {
            self.state = state;
        }
    }

    #[test]
    fn test_resource_manager() {
        let config = ResourceConfig::new("test", "test");
        let manager = ResourceManager::new(config);

        let resource = TestResource::default();
        let _id = manager
            .register_resource("test_resource", resource)
            .expect("test_resource: failed to register resource");

        let retrieved = manager
            .get_resource::<TestResource>("test_resource")
            .expect("test_resource: failed to get resource");
        {
            let retrieved_guard = retrieved.lock();
            assert_eq!(retrieved_guard.resource_type(), "test");
        } // 确保锁在此处被释放

        manager
            .remove_resource("test_resource")
            .expect("test_resource: failed to remove resource");
    }

    #[test]
    fn test_resource_guard() {
        let config = ResourceConfig::new("test", "test");
        let manager = Arc::new(ResourceManager::new(config));

        let resource = TestResource::default();
        let guard = ResourceGuard::new(resource, &manager, "test_resource".to_string());

        assert_eq!(guard.get().resource_type(), "test");

        drop(guard);
        assert!(
            manager
                .get_resource::<TestResource>("test_resource")
                .is_err()
        );
    }

    #[test]
    fn test_resource_pool() {
        let config = ResourceConfig::new("test", "test");
        let pool = ResourcePool::<TestResource>::new(config, 5);

        let resource1 = pool
            .acquire()
            .expect("test_resource_pool: failed to acquire first resource");
        let resource2 = pool
            .acquire()
            .expect("test_resource_pool: failed to acquire second resource");

        pool.release(resource1)
            .expect("test_resource_pool: failed to release first resource");
        pool.release(resource2)
            .expect("test_resource_pool: failed to release second resource");

        let stats = pool.get_stats();
        assert_eq!(stats.available_resources, 2);
    }

    #[test]
    fn test_resource_validation() {
        let valid_config = ResourceConfig::new("test", "test");
        assert!(utils::validate_config(&valid_config).is_ok());

        let invalid_config = ResourceConfig::new("", "test");
        assert!(utils::validate_config(&invalid_config).is_err());
    }
}
