//! 设备管理接口定义

use crate::{Configurable, DeviceId, DeviceStatus, DeviceType, Observable, VmComponent, VmError};

/// 设备配置
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeviceConfig {
    /// 设备ID
    pub device_id: DeviceId,
    /// 设备类型
    pub device_type: DeviceType,
    /// 设备名称
    pub name: String,
    /// 基础地址
    pub base_address: u64,
    /// 地址空间大小
    pub size: u64,
    /// 中断向量
    pub interrupt_vector: Option<u32>,
    /// 启用DMA
    pub enable_dma: bool,
    /// 自定义配置
    pub custom_config: std::collections::HashMap<String, serde_json::Value>,
}

impl Default for DeviceConfig {
    fn default() -> Self {
        Self {
            device_id: 0,
            device_type: DeviceType::Custom(0),
            name: String::new(),
            base_address: 0,
            size: 0,
            interrupt_vector: None,
            enable_dma: false,
            custom_config: std::collections::HashMap::new(),
        }
    }
}

/// I/O区域描述
#[derive(Debug, Clone)]
pub struct IoRegion {
    pub base: u64,
    pub size: u64,
    pub readable: bool,
    pub writable: bool,
}

/// 设备状态
#[derive(Debug, Clone)]
pub struct DeviceState {
    pub device_id: DeviceId,
    pub device_type: DeviceType,
    pub status: DeviceStatus,
    pub last_access: Option<std::time::SystemTime>,
    pub access_count: u64,
}

/// 统一的设备接口
pub trait Device: VmComponent {
    type IoRegion;

    /// 获取设备ID
    fn device_id(&self) -> DeviceId;

    /// 获取设备类型
    fn device_type(&self) -> DeviceType;

    /// 获取I/O区域
    fn io_regions(&self) -> &[Self::IoRegion];

    /// 处理I/O读操作
    fn handle_read(&mut self, offset: u64, size: usize) -> Result<u64, VmError>;

    /// 处理I/O写操作
    fn handle_write(&mut self, offset: u64, value: u64, size: usize) -> Result<(), VmError>;

    /// 处理中断
    fn handle_interrupt(&mut self, vector: u32) -> Result<(), VmError>;

    /// 获取设备状态
    fn device_status(&self) -> DeviceStatus;
}

/// 设备管理器接口
pub trait DeviceManager: VmComponent {
    type Device: Device;

    /// 注册设备
    fn register_device(&mut self, device: Box<Self::Device>) -> Result<DeviceId, VmError>;

    /// 注销设备
    fn unregister_device(&mut self, device_id: DeviceId) -> Result<Box<Self::Device>, VmError>;

    /// 查找设备
    fn find_device(&self, device_id: DeviceId) -> Option<&Self::Device>;

    /// 查找设备（可变）
    fn find_device_mut(&mut self, device_id: DeviceId) -> Option<&mut Self::Device>;

    /// 列出所有设备
    fn list_devices(&self) -> Vec<&Self::Device>;

    /// 路由I/O操作
    fn route_io_read(
        &mut self,
        device_id: DeviceId,
        offset: u64,
        size: usize,
    ) -> Result<u64, VmError>;
    fn route_io_write(
        &mut self,
        device_id: DeviceId,
        offset: u64,
        value: u64,
        size: usize,
    ) -> Result<(), VmError>;
}

/// 设备总线接口
pub trait DeviceBus {
    /// 映射设备到总线地址
    fn map_device(&mut self, device_id: DeviceId, base_addr: u64, size: u64)
    -> Result<(), VmError>;

    /// 取消设备映射
    fn unmap_device(&mut self, device_id: DeviceId) -> Result<(), VmError>;

    /// 地址到设备的翻译
    fn translate_address(&self, addr: u64) -> Option<(DeviceId, u64)>;
}

/// 虚拟设备基类
pub struct VirtualDevice {
    config: DeviceConfig,
    state: DeviceState,
    io_regions: Vec<IoRegion>,
}

impl VirtualDevice {
    pub fn new(config: DeviceConfig) -> Self {
        let state = DeviceState {
            device_id: config.device_id,
            device_type: config.device_type.clone(),
            status: DeviceStatus::Uninitialized,
            last_access: None,
            access_count: 0,
        };

        Self {
            config,
            state,
            io_regions: Vec::new(),
        }
    }

    pub fn add_io_region(&mut self, region: IoRegion) {
        self.io_regions.push(region);
    }

    pub fn config(&self) -> &DeviceConfig {
        &self.config
    }

    pub fn state(&self) -> &DeviceState {
        &self.state
    }

    pub fn update_access(&mut self) {
        self.state.last_access = Some(std::time::SystemTime::now());
        self.state.access_count += 1;
    }
}

impl VmComponent for VirtualDevice {
    type Config = DeviceConfig;
    type Error = VmError;

    fn init(config: Self::Config) -> Result<Self, Self::Error> {
        let mut device = Self::new(config);
        device.state.status = DeviceStatus::Initialized;
        Ok(device)
    }

    fn start(&mut self) -> Result<(), Self::Error> {
        self.state.status = DeviceStatus::Running;
        Ok(())
    }

    fn stop(&mut self) -> Result<(), Self::Error> {
        self.state.status = DeviceStatus::Stopped;
        Ok(())
    }

    fn status(&self) -> crate::ComponentStatus {
        match self.state.status {
            DeviceStatus::Uninitialized => crate::ComponentStatus::Uninitialized,
            DeviceStatus::Initialized => crate::ComponentStatus::Initialized,
            DeviceStatus::Running => crate::ComponentStatus::Running,
            DeviceStatus::Stopped => crate::ComponentStatus::Stopped,
            DeviceStatus::Error(_) => crate::ComponentStatus::Error,
        }
    }

    fn name(&self) -> &str {
        &self.config.name
    }
}

impl Configurable for VirtualDevice {
    type Config = DeviceConfig;

    fn update_config(&mut self, config: &Self::Config) -> Result<(), VmError> {
        self.config = config.clone();
        Ok(())
    }

    fn get_config(&self) -> &Self::Config {
        &self.config
    }

    fn validate_config(config: &Self::Config) -> Result<(), VmError> {
        if config.size == 0 {
            return Err(VmError::Core(vm_core::CoreError::Config {
                message: "Device size must be greater than 0".to_string(),
                path: Some("size".to_string()),
            }));
        }
        Ok(())
    }
}

impl Observable for VirtualDevice {
    type State = DeviceState;
    type Event = crate::VmEvent;

    fn get_state(&self) -> &Self::State {
        &self.state
    }

    fn subscribe(
        &mut self,
        _callback: Box<dyn Fn(&Self::State, &Self::Event) + Send + Sync>,
    ) -> crate::SubscriptionId {
        // 简化实现
        0
    }

    fn unsubscribe(&mut self, _id: crate::SubscriptionId) -> Result<(), VmError> {
        Ok(())
    }
}

impl Device for VirtualDevice {
    type IoRegion = IoRegion;

    fn device_id(&self) -> DeviceId {
        self.config.device_id
    }

    fn device_type(&self) -> DeviceType {
        self.config.device_type.clone()
    }

    fn io_regions(&self) -> &[Self::IoRegion] {
        &self.io_regions
    }

    fn handle_read(&mut self, offset: u64, size: usize) -> Result<u64, VmError> {
        self.update_access();
        // 默认实现：返回0
        if offset >= self.config.size {
            return Err(VmError::Device(vm_core::DeviceError::IoFailed {
                device_type: format!("{:?}", self.device_type()),
                operation: "read".to_string(),
                message: format!("Offset {} out of range", offset),
            }));
        }
        Ok(0)
    }

    fn handle_write(&mut self, offset: u64, _value: u64, size: usize) -> Result<(), VmError> {
        self.update_access();
        if offset >= self.config.size {
            return Err(VmError::Device(vm_core::DeviceError::IoFailed {
                device_type: format!("{:?}", self.device_type()),
                operation: "write".to_string(),
                message: format!("Offset {} out of range", offset),
            }));
        }
        Ok(())
    }

    fn handle_interrupt(&mut self, _vector: u32) -> Result<(), VmError> {
        // 默认实现：忽略中断
        Ok(())
    }

    fn device_status(&self) -> DeviceStatus {
        self.state.status.clone()
    }
}
