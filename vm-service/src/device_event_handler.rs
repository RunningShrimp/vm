//! 设备事件处理器
//!
//! 处理设备相关的领域事件，将设备操作集成到事件驱动架构。

use log::{debug, info};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use vm_core::{
    VmError, VmResult,
    domain_event_bus::{DomainEventBus, EventHandler, SimpleEventHandler},
    domain_events::{DeviceEvent, DomainEvent, DomainEventEnum},
};

/// 设备事件统计
#[derive(Debug, Clone, Default)]
pub struct DeviceEventStats {
    /// 添加的设备数
    pub devices_added: u64,
    /// 移除的设备数
    pub devices_removed: u64,
    /// 设备中断总数
    pub total_interrupts: u64,
    /// I/O完成的总字节数
    pub total_io_bytes: u64,
    /// 每个设备的中断次数
    pub device_interrupt_counts: HashMap<String, u64>,
}

/// 设备事件处理器
///
/// 监听设备相关事件并执行相应的操作。
pub struct DeviceEventHandler {
    /// VM ID
    vm_id: String,
    /// 事件总线
    event_bus: Arc<DomainEventBus>,
    /// 统计信息
    stats: Arc<Mutex<DeviceEventStats>>,
    /// 订阅ID（用于取消订阅）
    subscription_ids: Arc<Mutex<Vec<vm_core::domain_event_bus::EventSubscriptionId>>>,
}

impl DeviceEventHandler {
    /// 创建新的设备事件处理器
    pub fn new(vm_id: String, event_bus: Arc<DomainEventBus>) -> Self {
        Self {
            vm_id,
            event_bus,
            stats: Arc::new(Mutex::new(DeviceEventStats::default())),
            subscription_ids: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// 获取统计信息
    pub fn stats(&self) -> DeviceEventStats {
        self.stats.lock().unwrap().clone()
    }

    /// 取消所有订阅
    pub fn unregister_handlers(&self) -> VmResult<()> {
        let ids = self.subscription_ids.lock().unwrap();
        for id in ids.iter() {
            let _ = self.event_bus.unsubscribe_by_id(*id);
        }
        Ok(())
    }

    /// 注册事件处理器
    pub fn register_handlers(&self) -> VmResult<()> {
        let mut ids = self.subscription_ids.lock().unwrap();

        // 订阅设备添加事件
        let handler_added = SimpleEventHandler::new({
            let stats = Arc::clone(&self.stats);
            move |event: &dyn DomainEvent| -> VmResult<()> {
                if event.event_type() == "device.added" {
                    let mut s = stats.lock().unwrap();
                    s.devices_added += 1;
                    info!("Device added: total={}", s.devices_added);
                }
                Ok(())
            }
        });
        let id = self.event_bus
            .subscribe("device.added", Box::new(handler_added), None)?;
        ids.push(id);

        // 订阅设备移除事件
        let handler_removed = SimpleEventHandler::new({
            let stats = Arc::clone(&self.stats);
            move |event: &dyn DomainEvent| -> VmResult<()> {
                if event.event_type() == "device.removed" {
                    let mut s = stats.lock().unwrap();
                    s.devices_removed += 1;
                    info!("Device removed: total={}", s.devices_removed);
                }
                Ok(())
            }
        });
        let id = self.event_bus
            .subscribe("device.removed", Box::new(handler_removed), None)?;
        ids.push(id);

        // 订阅设备中断事件
        let handler_interrupt = SimpleEventHandler::new({
            let stats = Arc::clone(&self.stats);
            move |event: &dyn DomainEvent| -> VmResult<()> {
                if event.event_type() == "device.interrupt" {
                    let mut s = stats.lock().unwrap();
                    s.total_interrupts += 1;
                    debug!("Device interrupt: total={}", s.total_interrupts);
                    // 注意：这里无法直接提取设备ID和IRQ，因为DomainEvent trait的限制
                }
                Ok(())
            }
        });
        let id = self.event_bus
            .subscribe("device.interrupt", Box::new(handler_interrupt), None)?;
        ids.push(id);

        // 订阅设备I/O完成事件
        let handler_io_completed = SimpleEventHandler::new({
            let stats = Arc::clone(&self.stats);
            move |event: &dyn DomainEvent| -> VmResult<()> {
                if event.event_type() == "device.io_completed" {
                    let mut s = stats.lock().unwrap();
                    // 注意：这里无法直接提取字节数，因为DomainEvent trait的限制
                    debug!("Device I/O completed");
                }
                Ok(())
            }
        });
        let id = self.event_bus
            .subscribe("device.io_completed", Box::new(handler_io_completed), None)?;
        ids.push(id);

        Ok(())
    }
}

/// 发布设备添加事件
pub fn publish_device_added(
    event_bus: &Arc<DomainEventBus>,
    vm_id: &str,
    device_id: &str,
    device_type: &str,
) -> VmResult<()> {
    let event = DomainEventEnum::Device(DeviceEvent::DeviceAdded {
        vm_id: vm_id.to_string(),
        device_id: device_id.to_string(),
        device_type: device_type.to_string(),
        occurred_at: SystemTime::now(),
    });
    event_bus.publish(event)
}

/// 发布设备移除事件
pub fn publish_device_removed(
    event_bus: &Arc<DomainEventBus>,
    vm_id: &str,
    device_id: &str,
) -> VmResult<()> {
    let event = DomainEventEnum::Device(DeviceEvent::DeviceRemoved {
        vm_id: vm_id.to_string(),
        device_id: device_id.to_string(),
        occurred_at: SystemTime::now(),
    });
    event_bus.publish(event)
}

/// 发布设备中断事件
pub fn publish_device_interrupt(
    event_bus: &Arc<DomainEventBus>,
    vm_id: &str,
    device_id: &str,
    irq: u32,
) -> VmResult<()> {
    let event = DomainEventEnum::Device(DeviceEvent::DeviceInterrupt {
        vm_id: vm_id.to_string(),
        device_id: device_id.to_string(),
        irq,
        occurred_at: SystemTime::now(),
    });
    event_bus.publish(event)
}

/// 发布设备I/O完成事件
pub fn publish_device_io_completed(
    event_bus: &Arc<DomainEventBus>,
    vm_id: &str,
    device_id: &str,
    bytes_transferred: usize,
) -> VmResult<()> {
    let event = DomainEventEnum::Device(DeviceEvent::DeviceIoCompleted {
        vm_id: vm_id.to_string(),
        device_id: device_id.to_string(),
        bytes_transferred,
        occurred_at: SystemTime::now(),
    });
    event_bus.publish(event)
}
