//! 运行时控制器 - DDD实体层
//!
//! 负责VM运行时的核心控制逻辑

use crate::runtime::{RuntimeEvent, RuntimeState};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use vm_core::{GuestAddr, VmError};

/// 运行时控制器 - 贫血模型实体
#[derive(Debug)]
pub struct RuntimeControllerEntity {
    /// 运行时状态
    pub state: RuntimeState,
    /// 内存布局信息
    pub memory_layout: MemoryLayout,
    /// 设备映射
    pub device_map: HashMap<GuestAddr, DeviceInfo>,
    /// 性能统计
    pub stats: RuntimeStats,
}

/// 内存布局信息
#[derive(Debug, Clone)]
pub struct MemoryLayout {
    /// RAM起始地址
    pub ram_start: GuestAddr,
    /// RAM大小
    pub ram_size: u64,
    /// ROM起始地址
    pub rom_start: GuestAddr,
    /// ROM大小
    pub rom_size: u64,
}

/// 设备信息
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceInfo {
    /// 设备名称
    pub name: String,
    /// 设备类型
    pub device_type: String,
    /// 中断号
    pub irq: Option<u32>,
}

/// 运行时统计信息
#[derive(Debug, Clone, Default)]
pub struct RuntimeStats {
    /// 运行时间（毫秒）
    pub uptime_ms: u64,
    /// 处理的事件数量
    pub events_processed: u64,
    /// 内存使用量
    pub memory_used: u64,
    /// CPU使用率
    pub cpu_usage_percent: f32,
}

/// 运行时控制器服务 - DDD服务层
pub struct RuntimeControllerService {
    /// 实体引用
    entity: Arc<Mutex<RuntimeControllerEntity>>,
}

impl RuntimeControllerService {
    /// 创建新的运行时控制器服务
    pub fn new(ram_size: u64, rom_size: u64) -> Self {
        let entity = RuntimeControllerEntity {
            state: RuntimeState::Stopped,
            memory_layout: MemoryLayout {
                ram_start: GuestAddr(0x0),
                ram_size,
                rom_start: GuestAddr(ram_size),
                rom_size,
            },
            device_map: HashMap::new(),
            stats: RuntimeStats::default(),
        };

        Self {
            entity: Arc::new(Mutex::new(entity)),
        }
    }

    /// 获取运行时状态
    pub fn get_state(&self) -> RuntimeState {
        self.entity.lock().unwrap().state
    }

    /// 设置运行时状态
    pub fn set_state(&self, state: RuntimeState) {
        self.entity.lock().unwrap().state = state;
    }

    /// 注册设备
    pub fn register_device(&self, addr: GuestAddr, device: DeviceInfo) -> Result<(), VmError> {
        let mut entity = self.entity.lock().unwrap();
        entity.device_map.insert(addr, device);
        Ok(())
    }

    /// 注销设备
    pub fn unregister_device(&self, addr: GuestAddr) -> Result<(), VmError> {
        let mut entity = self.entity.lock().unwrap();
        entity.device_map.remove(&addr);
        Ok(())
    }

    /// 获取设备信息
    pub fn get_device(&self, addr: GuestAddr) -> Option<DeviceInfo> {
        self.entity.lock().unwrap().device_map.get(&addr).cloned()
    }

    /// 处理运行时事件
    pub fn process_event(&self, event: RuntimeEvent) -> Result<(), VmError> {
        let mut entity = self.entity.lock().unwrap();

        // 更新统计信息
        entity.stats.events_processed += 1;

        // 根据事件类型更新状态
        match event {
            RuntimeEvent::Started => {
                entity.state = RuntimeState::Running;
            }
            RuntimeEvent::Stopped => {
                entity.state = RuntimeState::Stopped;
            }
            RuntimeEvent::Paused => {
                entity.state = RuntimeState::Paused;
            }
            RuntimeEvent::Resumed => {
                entity.state = RuntimeState::Running;
            }
            RuntimeEvent::Error(_) => {
                entity.state = RuntimeState::ShuttingDown;
            }
            _ => {
                // 其他事件不改变状态
            }
        }

        Ok(())
    }

    /// 更新性能统计
    pub fn update_stats(&self, uptime_ms: u64, memory_used: u64, cpu_usage: f32) {
        let mut entity = self.entity.lock().unwrap();
        entity.stats.uptime_ms = uptime_ms;
        entity.stats.memory_used = memory_used;
        entity.stats.cpu_usage_percent = cpu_usage;
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> RuntimeStats {
        self.entity.lock().unwrap().stats.clone()
    }

    /// 获取内存布局
    pub fn get_memory_layout(&self) -> MemoryLayout {
        self.entity.lock().unwrap().memory_layout.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_controller_service() {
        let service = RuntimeControllerService::new(1024 * 1024 * 1024, 64 * 1024 * 1024); // 1GB RAM, 64MB ROM

        // 检查初始状态
        assert!(matches!(service.get_state(), RuntimeState::Initialized));

        // 注册设备
        let device = DeviceInfo {
            name: "test_device".to_string(),
            device_type: "test".to_string(),
            irq: Some(42),
        };
        assert!(
            service
                .register_device(GuestAddr(0x1000), device.clone())
                .is_ok()
        );

        // 获取设备
        let retrieved = service.get_device(GuestAddr(0x1000));
        assert_eq!(retrieved, Some(device));

        // 处理事件
        assert!(service.process_event(RuntimeEvent::Started).is_ok());
        assert!(matches!(service.get_state(), RuntimeState::Running));

        // 更新统计
        service.update_stats(1000, 512 * 1024 * 1024, 45.5);
        let stats = service.get_stats();
        assert_eq!(stats.uptime_ms, 1000);
        assert_eq!(stats.memory_used, 512 * 1024 * 1024);
        assert_eq!(stats.cpu_usage_percent, 45.5);
    }
}
