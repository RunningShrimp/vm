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
