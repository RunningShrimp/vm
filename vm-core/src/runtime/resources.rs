// 资源管理（Resource Management）
//
// 本模块提供VM的资源管理服务：
// - 资源分配
// - 资源释放
// - 资源统计

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

/// 资源类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceType {
    /// CPU资源
    Cpu = 0,
    /// 内存资源
    Memory = 1,
    /// 磁盘资源
    Disk = 2,
    /// 网络资源
    Network = 3,
    /// GPU资源
    Gpu = 4,
}

/// 资源描述符
#[derive(Debug, Clone)]
pub struct ResourceDescriptor {
    /// 资源ID
    pub resource_id: u64,
    /// 资源类型
    pub resource_type: ResourceType,
    /// 总数量
    pub total: u64,
    /// 已分配数量
    pub allocated: u64,
    /// 可用数量
    pub available: u64,
}

impl ResourceDescriptor {
    /// 创建新的资源描述符
    pub fn new(resource_id: u64, resource_type: ResourceType, total: u64) -> Self {
        Self {
            resource_id,
            resource_type,
            total,
            allocated: 0,
            available: total,
        }
    }

    /// 获取利用率
    pub fn utilization(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.allocated as f64 / self.total as f64
        }
    }

    /// 是否有可用资源
    pub fn is_available(&self) -> bool {
        self.available > 0
    }
}

/// 资源请求
#[derive(Debug, Clone)]
pub struct ResourceRequest {
    /// 资源ID
    pub resource_id: u64,
    /// 资源类型
    pub resource_type: ResourceType,
    /// 请求数量
    pub amount: u64,
    /// 请求时间戳
    pub timestamp: u64,
}

impl ResourceRequest {
    /// 创建新的资源请求
    pub fn new(resource_id: u64, resource_type: ResourceType, amount: u64) -> Self {
        Self {
            resource_id,
            resource_type,
            amount,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_else(|_| std::time::Duration::from_secs(0))
                .as_secs(),
        }
    }
}

/// 资源管理器
pub struct ResourceManager {
    /// 资源描述符
    pub resources: Arc<Mutex<HashMap<u64, ResourceDescriptor>>>,
    /// 资源请求历史
    pub request_history: Arc<Mutex<Vec<ResourceRequest>>>,
    /// 分配计数
    pub allocation_count: Arc<Mutex<u64>>,
}

impl ResourceManager {
    /// 创建新的资源管理器
    pub fn new() -> Self {
        Self {
            resources: Arc::new(Mutex::new(HashMap::new())),
            request_history: Arc::new(Mutex::new(Vec::new())),
            allocation_count: Arc::new(Mutex::new(0)),
        }
    }

    /// Helper to acquire resources lock with error handling
    fn lock_resources(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, HashMap<u64, ResourceDescriptor>>, String> {
        self.resources
            .lock()
            .map_err(|e| format!("Resources lock is poisoned: {:?}", e))
    }

    /// Helper to acquire allocation_count lock with error handling
    fn lock_allocation_count(&self) -> Result<std::sync::MutexGuard<'_, u64>, String> {
        self.allocation_count
            .lock()
            .map_err(|e| format!("Allocation count lock is poisoned: {:?}", e))
    }

    /// 添加资源
    ///
    /// # 参数
    /// - `descriptor`: 资源描述符
    ///
    /// # 示例
    /// ```ignore
    /// let descriptor = ResourceDescriptor::new(0x100, ResourceType::Memory, 1024);
    /// manager.add_resource(descriptor);
    /// ```
    pub fn add_resource(&self, descriptor: ResourceDescriptor) {
        if let Ok(mut resources) = self.lock_resources() {
            resources.insert(descriptor.resource_id, descriptor);
        }
    }

    /// 分配资源
    ///
    /// # 参数
    /// - `request`: 资源请求
    ///
    /// # 返回
    /// - `Ok(amount)`: 分配成功
    /// - `Err(msg)`: 分配失败
    ///
    /// # 示例
    /// ```ignore
    /// let request = ResourceRequest::new(0x100, ResourceType::Memory, 512);
    /// match manager.allocate(request) {
    ///     Ok(allocated) => println!("Allocated: {}", allocated),
    ///     Err(e) => println!("Allocation failed: {}", e),
    /// }
    /// ```
    pub fn allocate(&self, resource_id: u64, amount: u64) -> Result<u64, String> {
        let mut resources = self.lock_resources()?;

        // 获取资源描述符
        let descriptor = resources
            .get_mut(&resource_id)
            .ok_or_else(|| format!("Resource {} not found", resource_id))?;

        // 检查是否有足够资源
        if descriptor.available < amount {
            return Err(format!(
                "Insufficient resources: available={}, requested={}",
                descriptor.available, amount
            ));
        }

        // 分配资源
        descriptor.available -= amount;
        descriptor.allocated += amount;

        // 更新分配计数
        {
            let mut count = self.lock_allocation_count()?;
            *count += 1;
        }

        Ok(amount)
    }

    /// 释放资源
    ///
    /// # 参数
    /// - `request`: 资源请求
    ///
    /// # 返回
    /// - `Ok(())`: 释放成功
    /// - `Err(msg)`: 释放失败
    ///
    /// # 示例
    /// ```ignore
    /// let request = ResourceRequest::new(0x100, ResourceType::Memory, 512);
    /// manager.release(request)?;
    /// ```
    pub fn release(&self, resource_id: u64, amount: u64) -> Result<(), String> {
        let mut resources = self.lock_resources()?;

        // 获取资源描述符
        let descriptor = resources
            .get_mut(&resource_id)
            .ok_or_else(|| format!("Resource {} not found", resource_id))?;

        // 检查是否有足够已分配的资源
        if descriptor.allocated < amount {
            return Err(format!(
                "Insufficient allocated resources: allocated={}, requested={}",
                descriptor.allocated, amount
            ));
        }

        // 释放资源
        descriptor.allocated -= amount;
        descriptor.available += amount;

        Ok(())
    }

    /// 获取资源信息
    ///
    /// # 参数
    /// - `resource_id`: 资源ID
    ///
    /// # 返回
    /// - `Some(descriptor)`: 资源存在
    /// - `None`: 资源不存在
    pub fn get_resource(&self, resource_id: u64) -> Option<ResourceDescriptor> {
        self.lock_resources().ok()?.get(&resource_id).cloned()
    }

    /// 获取所有资源信息
    pub fn get_all_resources(&self) -> Vec<ResourceDescriptor> {
        match self.lock_resources() {
            Ok(resources) => resources.values().cloned().collect(),
            Err(_) => Vec::new(),
        }
    }

    /// 清空所有资源
    pub fn clear_resources(&self) {
        if let Ok(mut resources) = self.lock_resources() {
            resources.clear();
        }
        if let Ok(mut count) = self.lock_allocation_count() {
            *count = 0;
        }
    }

    /// 获取分配计数
    pub fn get_allocation_count(&self) -> u64 {
        match self.lock_allocation_count() {
            Ok(count) => *count,
            Err(_) => 0,
        }
    }
}

impl Default for ResourceManager {
    fn default() -> Self {
        Self::new()
    }
}

/// ============================================================================
/// 测试模块
/// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_type_equality() {
        assert_eq!(ResourceType::Cpu, ResourceType::Cpu);
        assert_eq!(ResourceType::Memory, ResourceType::Memory);
        assert_ne!(ResourceType::Cpu, ResourceType::Memory);
    }

    #[test]
    fn test_resource_type_clone() {
        let rtype = ResourceType::Cpu;
        let cloned = rtype;
        assert_eq!(rtype, cloned);
    }

    #[test]
    fn test_resource_descriptor_new() {
        let descriptor = ResourceDescriptor::new(0x100, ResourceType::Memory, 1024);

        assert_eq!(descriptor.resource_id, 0x100);
        assert_eq!(descriptor.resource_type, ResourceType::Memory);
        assert_eq!(descriptor.total, 1024);
        assert_eq!(descriptor.allocated, 0);
        assert_eq!(descriptor.available, 1024);
    }

    #[test]
    fn test_resource_descriptor_utilization() {
        let mut descriptor = ResourceDescriptor::new(0x100, ResourceType::Memory, 1024);

        // Initial utilization should be 0
        assert_eq!(descriptor.utilization(), 0.0);

        // Allocate 512 bytes
        descriptor.allocated = 512;
        descriptor.available = 512;

        assert!((descriptor.utilization() - 0.5).abs() < 0.001);

        // Fully allocated
        descriptor.allocated = 1024;
        descriptor.available = 0;

        assert_eq!(descriptor.utilization(), 1.0);
    }

    #[test]
    fn test_resource_descriptor_utilization_zero_total() {
        let descriptor = ResourceDescriptor::new(0x100, ResourceType::Memory, 0);
        assert_eq!(descriptor.utilization(), 0.0);
    }

    #[test]
    fn test_resource_descriptor_is_available() {
        let mut descriptor = ResourceDescriptor::new(0x100, ResourceType::Memory, 1024);

        // Initially available
        assert!(descriptor.is_available());

        // Half allocated
        descriptor.allocated = 512;
        descriptor.available = 512;
        assert!(descriptor.is_available());

        // Fully allocated
        descriptor.allocated = 1024;
        descriptor.available = 0;
        assert!(!descriptor.is_available());
    }

    #[test]
    fn test_resource_descriptor_clone() {
        let descriptor = ResourceDescriptor::new(0x100, ResourceType::Memory, 1024);

        let cloned = descriptor.clone();
        assert_eq!(cloned.resource_id, descriptor.resource_id);
        assert_eq!(cloned.resource_type, descriptor.resource_type);
        assert_eq!(cloned.total, descriptor.total);
    }

    #[test]
    fn test_resource_request_new() {
        let request = ResourceRequest::new(0x100, ResourceType::Memory, 512);

        assert_eq!(request.resource_id, 0x100);
        assert_eq!(request.resource_type, ResourceType::Memory);
        assert_eq!(request.amount, 512);
        assert!(request.timestamp > 0);
    }

    #[test]
    fn test_resource_request_clone() {
        let request = ResourceRequest::new(0x100, ResourceType::Memory, 512);

        let cloned = request.clone();
        assert_eq!(cloned.resource_id, request.resource_id);
        assert_eq!(cloned.resource_type, request.resource_type);
        assert_eq!(cloned.amount, request.amount);
        assert_eq!(cloned.timestamp, request.timestamp);
    }

    #[test]
    fn test_resource_manager_new() {
        let manager = ResourceManager::new();

        // Check initial state
        let resources = manager.resources.lock().unwrap();
        assert_eq!(resources.len(), 0);
        drop(resources);

        let history = manager.request_history.lock().unwrap();
        assert_eq!(history.len(), 0);
        drop(history);

        let count = manager.allocation_count.lock().unwrap();
        assert_eq!(*count, 0);
    }

    #[test]
    fn test_resource_manager_default() {
        let manager = ResourceManager::default();

        let resources = manager.resources.lock().unwrap();
        assert_eq!(resources.len(), 0);
    }

    #[test]
    fn test_add_resource() {
        let manager = ResourceManager::new();
        let descriptor = ResourceDescriptor::new(0x100, ResourceType::Memory, 1024);

        manager.add_resource(descriptor);

        let resources = manager.resources.lock().unwrap();
        assert_eq!(resources.len(), 1);
        assert!(resources.contains_key(&0x100));
    }

    #[test]
    fn test_allocate_success() {
        let manager = ResourceManager::new();
        let descriptor = ResourceDescriptor::new(0x100, ResourceType::Memory, 1024);

        manager.add_resource(descriptor);

        // Allocate 512 bytes
        let result = manager.allocate(0x100, 512);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 512);

        // Check state
        let resource = manager.get_resource(0x100).unwrap();
        assert_eq!(resource.allocated, 512);
        assert_eq!(resource.available, 512);

        // Check allocation count
        assert_eq!(manager.get_allocation_count(), 1);
    }

    #[test]
    fn test_allocate_insufficient_resources() {
        let manager = ResourceManager::new();
        let descriptor = ResourceDescriptor::new(0x100, ResourceType::Memory, 1024);

        manager.add_resource(descriptor);

        // Try to allocate more than available
        let result = manager.allocate(0x100, 2048);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Insufficient resources"));

        // Check state unchanged
        let resource = manager.get_resource(0x100).unwrap();
        assert_eq!(resource.allocated, 0);
        assert_eq!(resource.available, 1024);
    }

    #[test]
    fn test_allocate_resource_not_found() {
        let manager = ResourceManager::new();

        // Try to allocate from non-existent resource
        let result = manager.allocate(0x100, 512);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_release_success() {
        let manager = ResourceManager::new();
        let descriptor = ResourceDescriptor::new(0x100, ResourceType::Memory, 1024);

        manager.add_resource(descriptor);

        // Allocate first
        manager.allocate(0x100, 512).unwrap();

        // Release 256 bytes
        let result = manager.release(0x100, 256);
        assert!(result.is_ok());

        // Check state
        let resource = manager.get_resource(0x100).unwrap();
        assert_eq!(resource.allocated, 256);
        assert_eq!(resource.available, 768);
    }

    #[test]
    fn test_release_insufficient_allocated() {
        let manager = ResourceManager::new();
        let descriptor = ResourceDescriptor::new(0x100, ResourceType::Memory, 1024);

        manager.add_resource(descriptor);

        // Allocate 512 bytes
        manager.allocate(0x100, 512).unwrap();

        // Try to release more than allocated
        let result = manager.release(0x100, 1024);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Insufficient allocated resources")
        );
    }

    #[test]
    fn test_release_resource_not_found() {
        let manager = ResourceManager::new();

        // Try to release from non-existent resource
        let result = manager.release(0x100, 512);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_get_resource_exists() {
        let manager = ResourceManager::new();
        let descriptor = ResourceDescriptor::new(0x100, ResourceType::Memory, 1024);

        manager.add_resource(descriptor.clone());

        let resource = manager.get_resource(0x100);
        assert!(resource.is_some());
        assert_eq!(resource.unwrap().resource_id, 0x100);
    }

    #[test]
    fn test_get_resource_not_exists() {
        let manager = ResourceManager::new();

        let resource = manager.get_resource(0x100);
        assert!(resource.is_none());
    }

    #[test]
    fn test_get_all_resources() {
        let manager = ResourceManager::new();

        // Add multiple resources
        manager.add_resource(ResourceDescriptor::new(0x100, ResourceType::Memory, 1024));
        manager.add_resource(ResourceDescriptor::new(0x101, ResourceType::Cpu, 4));
        manager.add_resource(ResourceDescriptor::new(0x102, ResourceType::Disk, 1024));

        let resources = manager.get_all_resources();
        assert_eq!(resources.len(), 3);
    }

    #[test]
    fn test_get_all_resources_empty() {
        let manager = ResourceManager::new();

        let resources = manager.get_all_resources();
        assert_eq!(resources.len(), 0);
    }

    #[test]
    fn test_clear_resources() {
        let manager = ResourceManager::new();

        // Add resources and allocate
        manager.add_resource(ResourceDescriptor::new(0x100, ResourceType::Memory, 1024));
        manager.allocate(0x100, 512).unwrap();

        // Clear
        manager.clear_resources();

        // Check all cleared
        let resources = manager.get_all_resources();
        assert_eq!(resources.len(), 0);

        assert_eq!(manager.get_allocation_count(), 0);
    }

    #[test]
    fn test_get_allocation_count() {
        let manager = ResourceManager::new();
        let descriptor = ResourceDescriptor::new(0x100, ResourceType::Memory, 1024);

        manager.add_resource(descriptor);

        // Initial count
        assert_eq!(manager.get_allocation_count(), 0);

        // After allocations
        manager.allocate(0x100, 256).unwrap();
        assert_eq!(manager.get_allocation_count(), 1);

        manager.allocate(0x100, 256).unwrap();
        assert_eq!(manager.get_allocation_count(), 2);
    }

    #[test]
    fn test_multiple_allocations_and_releases() {
        let manager = ResourceManager::new();
        let descriptor = ResourceDescriptor::new(0x100, ResourceType::Memory, 1024);

        manager.add_resource(descriptor);

        // Multiple allocations
        manager.allocate(0x100, 256).unwrap();
        manager.allocate(0x100, 256).unwrap();
        manager.allocate(0x100, 256).unwrap();

        let resource = manager.get_resource(0x100).unwrap();
        assert_eq!(resource.allocated, 768);
        assert_eq!(resource.available, 256);

        // Multiple releases
        manager.release(0x100, 128).unwrap();
        manager.release(0x100, 128).unwrap();

        let resource = manager.get_resource(0x100).unwrap();
        assert_eq!(resource.allocated, 512);
        assert_eq!(resource.available, 512);
    }

    #[test]
    fn test_all_resource_types() {
        let manager = ResourceManager::new();

        manager.add_resource(ResourceDescriptor::new(0x100, ResourceType::Cpu, 4));
        manager.add_resource(ResourceDescriptor::new(0x101, ResourceType::Memory, 1024));
        manager.add_resource(ResourceDescriptor::new(0x102, ResourceType::Disk, 1024));
        manager.add_resource(ResourceDescriptor::new(0x103, ResourceType::Network, 100));
        manager.add_resource(ResourceDescriptor::new(0x104, ResourceType::Gpu, 1));

        let resources = manager.get_all_resources();
        assert_eq!(resources.len(), 5);

        // Verify each type
        let cpu = resources
            .iter()
            .find(|r| r.resource_type == ResourceType::Cpu);
        assert!(cpu.is_some());
        assert_eq!(cpu.unwrap().total, 4);

        let memory = resources
            .iter()
            .find(|r| r.resource_type == ResourceType::Memory);
        assert!(memory.is_some());
        assert_eq!(memory.unwrap().total, 1024);
    }

    #[test]
    fn test_allocate_all_available() {
        let manager = ResourceManager::new();
        let descriptor = ResourceDescriptor::new(0x100, ResourceType::Memory, 1024);

        manager.add_resource(descriptor);

        // Allocate all
        let result = manager.allocate(0x100, 1024);
        assert!(result.is_ok());

        let resource = manager.get_resource(0x100).unwrap();
        assert_eq!(resource.available, 0);
        assert_eq!(resource.allocated, 1024);
        assert!(!resource.is_available());
    }

    #[test]
    fn test_release_all_allocated() {
        let manager = ResourceManager::new();
        let descriptor = ResourceDescriptor::new(0x100, ResourceType::Memory, 1024);

        manager.add_resource(descriptor);

        // Allocate and release all
        manager.allocate(0x100, 1024).unwrap();
        manager.release(0x100, 1024).unwrap();

        let resource = manager.get_resource(0x100).unwrap();
        assert_eq!(resource.available, 1024);
        assert_eq!(resource.allocated, 0);
        assert!(resource.is_available());
    }
}
