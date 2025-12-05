//! 内存事件处理器
//!
//! 处理内存相关的领域事件，将内存操作集成到事件驱动架构。

use log::{debug, info, warn};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use vm_core::{
    GuestAddr, VmError, VmResult,
    domain_event_bus::{DomainEventBus, EventHandler, SimpleEventHandler},
    domain_events::{DomainEvent, DomainEventEnum, MemoryEvent},
};

/// 内存事件统计
#[derive(Debug, Clone, Default)]
pub struct MemoryEventStats {
    /// 分配的内存总数（字节）
    pub total_allocated: u64,
    /// 释放的内存总数（字节）
    pub total_freed: u64,
    /// 页错误次数
    pub page_fault_count: u64,
    /// 内存映射次数
    pub mapping_count: u64,
    /// 当前分配的内存（字节）
    pub current_allocated: u64,
}

/// 内存事件处理器
///
/// 监听内存相关事件并执行相应的操作。
pub struct MemoryEventHandler {
    /// VM ID
    vm_id: String,
    /// 事件总线
    event_bus: Arc<DomainEventBus>,
    /// 统计信息
    stats: Arc<Mutex<MemoryEventStats>>,
    /// 订阅ID（用于取消订阅）
    subscription_ids: Arc<Mutex<Vec<vm_core::domain_event_bus::EventSubscriptionId>>>,
}

impl MemoryEventHandler {
    /// 创建新的内存事件处理器
    pub fn new(vm_id: String, event_bus: Arc<DomainEventBus>) -> Self {
        Self {
            vm_id,
            event_bus,
            stats: Arc::new(Mutex::new(MemoryEventStats::default())),
            subscription_ids: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// 获取统计信息
    pub fn stats(&self) -> MemoryEventStats {
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

        // 订阅内存分配事件
        let handler_allocated = SimpleEventHandler::new({
            let vm_id = self.vm_id.clone();
            let stats = Arc::clone(&self.stats);
            move |event: &dyn DomainEvent| -> VmResult<()> {
                if event.event_type() == "memory.allocated" {
                    if let Ok(DomainEventEnum::Memory(MemoryEvent::MemoryAllocated { size, .. })) =
                        Self::extract_memory_event(event)
                    {
                        let mut s = stats.lock().unwrap();
                        s.total_allocated += size;
                        s.current_allocated += size;
                        info!("Memory allocated: {} bytes, total: {} bytes", size, s.current_allocated);
                    }
                }
                Ok(())
            }
        });
        let id = self.event_bus
            .subscribe("memory.allocated", Box::new(handler_allocated), None)?;
        ids.push(id);

        // 订阅内存释放事件
        let handler_freed = SimpleEventHandler::new({
            let vm_id = self.vm_id.clone();
            let stats = Arc::clone(&self.stats);
            move |event: &dyn DomainEvent| -> VmResult<()> {
                if event.event_type() == "memory.freed" {
                    if let Ok(DomainEventEnum::Memory(MemoryEvent::MemoryFreed { size, .. })) =
                        Self::extract_memory_event(event)
                    {
                        let mut s = stats.lock().unwrap();
                        s.total_freed += size;
                        s.current_allocated = s.current_allocated.saturating_sub(size);
                        info!("Memory freed: {} bytes, current: {} bytes", size, s.current_allocated);
                    }
                }
                Ok(())
            }
        });
        let id = self.event_bus
            .subscribe("memory.freed", Box::new(handler_freed), None)?;
        ids.push(id);

        // 订阅内存映射事件
        let handler_mapped = SimpleEventHandler::new({
            let vm_id = self.vm_id.clone();
            let stats = Arc::clone(&self.stats);
            move |event: &dyn DomainEvent| -> VmResult<()> {
                if event.event_type() == "memory.mapped" {
                    if let Ok(DomainEventEnum::Memory(MemoryEvent::MemoryMapped { guest_addr, size, .. })) =
                        Self::extract_memory_event(event)
                    {
                        let mut s = stats.lock().unwrap();
                        s.mapping_count += 1;
                        debug!("Memory mapped: guest_addr=0x{:x}, size={} bytes", guest_addr, size);
                    }
                }
                Ok(())
            }
        });
        let id = self.event_bus
            .subscribe("memory.mapped", Box::new(handler_mapped), None)?;
        ids.push(id);

        // 订阅页错误事件
        let handler_page_fault = SimpleEventHandler::new({
            let vm_id = self.vm_id.clone();
            let stats = Arc::clone(&self.stats);
            move |event: &dyn DomainEvent| -> VmResult<()> {
                if event.event_type() == "memory.page_fault" {
                    if let Ok(DomainEventEnum::Memory(MemoryEvent::PageFault { addr, is_write, .. })) =
                        Self::extract_memory_event(event)
                    {
                        let mut s = stats.lock().unwrap();
                        s.page_fault_count += 1;
                        warn!("Page fault: addr=0x{:x}, is_write={}", addr, is_write);
                    }
                }
                Ok(())
            }
        });
        let id = self.event_bus
            .subscribe("memory.page_fault", Box::new(handler_page_fault), None)?;
        ids.push(id);

        // 订阅内存取消映射事件
        let handler_unmapped = SimpleEventHandler::new({
            let vm_id = self.vm_id.clone();
            let stats = Arc::clone(&self.stats);
            move |event: &dyn DomainEvent| -> VmResult<()> {
                if event.event_type() == "memory.unmapped" {
                    if let Ok(DomainEventEnum::Memory(MemoryEvent::MemoryUnmapped { guest_addr, size, .. })) =
                        Self::extract_memory_event(event)
                    {
                        debug!("Memory unmapped: guest_addr=0x{:x}, size={} bytes", guest_addr, size);
                    }
                }
                Ok(())
            }
        });
        let id = self.event_bus
            .subscribe("memory.unmapped", Box::new(handler_unmapped), None)?;
        ids.push(id);

        Ok(())
    }

    /// 从领域事件中提取内存事件（辅助方法）
    fn extract_memory_event(event: &dyn DomainEvent) -> Result<DomainEventEnum, VmError> {
        // 这是一个简化实现，实际应该使用类型匹配
        // 由于DomainEvent trait的限制，这里需要根据事件类型重新构造
        Err(VmError::Core(vm_core::CoreError::Internal {
            message: "Cannot extract memory event from trait object".to_string(),
            module: "memory_event_handler".to_string(),
        }))
    }
}

/// 发布内存分配事件
pub fn publish_memory_allocated(
    event_bus: &Arc<DomainEventBus>,
    vm_id: &str,
    addr: GuestAddr,
    size: u64,
) -> VmResult<()> {
    let event = DomainEventEnum::Memory(MemoryEvent::MemoryAllocated {
        vm_id: vm_id.to_string(),
        addr,
        size,
        occurred_at: SystemTime::now(),
    });
    event_bus.publish(event)
}

/// 发布内存映射事件
pub fn publish_memory_mapped(
    event_bus: &Arc<DomainEventBus>,
    vm_id: &str,
    guest_addr: GuestAddr,
    host_addr: u64,
    size: u64,
) -> VmResult<()> {
    let event = DomainEventEnum::Memory(MemoryEvent::MemoryMapped {
        vm_id: vm_id.to_string(),
        guest_addr,
        host_addr,
        size,
        occurred_at: SystemTime::now(),
    });
    event_bus.publish(event)
}

/// 发布页错误事件
pub fn publish_page_fault(
    event_bus: &Arc<DomainEventBus>,
    vm_id: &str,
    addr: GuestAddr,
    is_write: bool,
) -> VmResult<()> {
    let event = DomainEventEnum::Memory(MemoryEvent::PageFault {
        vm_id: vm_id.to_string(),
        addr,
        is_write,
        occurred_at: SystemTime::now(),
    });
    event_bus.publish(event)
}
