//! 事件驱动架构实现
//!
//! 提供统一的事件发布订阅机制，支持组件间的松耦合通信。

use crate::{SubscriptionId, VmError, VmEvent, VmEventListener};
use std::collections::HashMap;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};

/// 事件发布器
pub struct EventPublisher {
    subscribers: HashMap<String, Vec<VmEventListener>>,
    next_id: AtomicU64,
}

impl Default for EventPublisher {
    fn default() -> Self {
        Self::new()
    }
}

impl EventPublisher {
    pub fn new() -> Self {
        Self {
            subscribers: HashMap::new(),
            next_id: AtomicU64::new(1),
        }
    }

    /// 发布事件
    pub fn publish(&self, event: &VmEvent) {
        let event_type = match event {
            VmEvent::ComponentStarted(_) => "component_started",
            VmEvent::ComponentStopped(_) => "component_stopped",
            VmEvent::ExecutionCompleted(_) => "execution_completed",
            VmEvent::MemoryAccess { .. } => "memory_access",
            VmEvent::DeviceInterrupt(_) => "device_interrupt",
            VmEvent::ErrorOccurred(_) => "error_occurred",
        };

        if let Some(callbacks) = self.subscribers.get(event_type) {
            for callback in callbacks {
                callback(event);
            }
        }
    }

    /// 订阅事件
    pub fn subscribe<F>(&mut self, event_type: &str, callback: F) -> SubscriptionId
    where
        F: Fn(&VmEvent) + Send + Sync + 'static,
    {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.subscribers
            .entry(event_type.to_string())
            .or_default()
            .push(Box::new(callback));
        id
    }

    /// 取消订阅
    pub fn unsubscribe(&mut self, event_type: &str, _id: SubscriptionId) -> Result<(), VmError> {
        if self.subscribers.contains_key(event_type) {
            // Note: In a real implementation, we'd need to track IDs per callback
            // This is a simplified version
            Ok(())
        } else {
            Err(VmError::Core(vm_core::CoreError::Internal {
                message: format!("Event type '{}' not found", event_type),
                module: "event_publisher".to_string(),
            }))
        }
    }
}

/// 事件总线 - 全局事件管理器
pub struct EventBus {
    publisher: Arc<Mutex<EventPublisher>>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            publisher: Arc::new(Mutex::new(EventPublisher::new())),
        }
    }

    /// 获取全局事件总线实例
    pub fn global() -> &'static Self {
        static INSTANCE: std::sync::OnceLock<EventBus> = std::sync::OnceLock::new();
        INSTANCE.get_or_init(EventBus::new)
    }

    /// 发布事件
    pub fn publish(&self, event: VmEvent) {
        if let Ok(publisher) = self.publisher.lock() {
            publisher.publish(&event);
        }
    }

    /// 订阅事件
    pub fn subscribe<F>(&self, event_type: &str, callback: F) -> Result<SubscriptionId, VmError>
    where
        F: Fn(&VmEvent) + Send + Sync + 'static,
    {
        if let Ok(mut publisher) = self.publisher.lock() {
            Ok(publisher.subscribe(event_type, callback))
        } else {
            Err(VmError::Core(vm_core::CoreError::Concurrency {
                message: "Failed to acquire event publisher lock".to_string(),
                operation: "subscribe".to_string(),
            }))
        }
    }

    /// 取消订阅
    pub fn unsubscribe(&self, event_type: &str, id: SubscriptionId) -> Result<(), VmError> {
        if let Ok(mut publisher) = self.publisher.lock() {
            publisher.unsubscribe(event_type, id)
        } else {
            Err(VmError::Core(vm_core::CoreError::Concurrency {
                message: "Failed to acquire event publisher lock".to_string(),
                operation: "unsubscribe".to_string(),
            }))
        }
    }
}

/// 组件注册表 - 管理组件生命周期
pub struct ComponentRegistry {
    components: HashMap<String, Box<dyn VmComponentProxy>>,
    event_bus: Arc<EventBus>,
}

impl ComponentRegistry {
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self {
            components: HashMap::new(),
            event_bus,
        }
    }

    /// 注册组件
    pub fn register<C>(&mut self, name: String, component: C) -> Result<(), VmError>
    where
        C: VmComponentProxy + 'static,
    {
        if self.components.contains_key(&name) {
            return Err(VmError::Core(vm_core::CoreError::Internal {
                message: format!("Component '{}' already registered", name),
                module: "component_registry".to_string(),
            }));
        }

        self.components.insert(name.clone(), Box::new(component));
        self.event_bus.publish(VmEvent::ComponentStarted(name));
        Ok(())
    }

    /// 注销组件
    pub fn unregister(&mut self, name: &str) -> Result<(), VmError> {
        if self.components.remove(name).is_some() {
            self.event_bus
                .publish(VmEvent::ComponentStopped(name.to_string()));
            Ok(())
        } else {
            Err(VmError::Core(vm_core::CoreError::Internal {
                message: format!("Component '{}' not found", name),
                module: "component_registry".to_string(),
            }))
        }
    }

    /// 获取组件
    pub fn get(&self, name: &str) -> Option<&dyn VmComponentProxy> {
        self.components.get(name).map(|c| c.as_ref())
    }

    /// 获取可变组件
    pub fn get_mut(&mut self, name: &str) -> Option<&mut (dyn VmComponentProxy + '_)> {
        if let Some(c) = self.components.get_mut(name) {
            Some(&mut **c)
        } else {
            None
        }
    }

    /// 列出所有组件
    pub fn list_components(&self) -> Vec<&str> {
        self.components.keys().map(|s| s.as_str()).collect()
    }
}

/// 组件代理trait - 用于统一组件接口
pub trait VmComponentProxy {
    /// 获取组件名称
    fn name(&self) -> &str;

    /// 获取组件状态
    fn status(&self) -> crate::ComponentStatus;

    /// 启动组件
    fn start(&mut self) -> Result<(), VmError>;

    /// 停止组件
    fn stop(&mut self) -> Result<(), VmError>;

    /// 处理事件
    fn handle_event(&mut self, event: &VmEvent) -> Result<(), VmError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_event_publisher() {
        let mut publisher = EventPublisher::new();
        let counter = Arc::new(AtomicUsize::new(0));

        let counter_clone = counter.clone();
        publisher.subscribe("component_started", move |_| {
            counter_clone.fetch_add(1, Ordering::Relaxed);
        });

        let event = VmEvent::ComponentStarted("test".to_string());
        publisher.publish(&event);

        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_event_bus() {
        let bus = EventBus::new();
        let counter = Arc::new(AtomicUsize::new(0));

        let counter_clone = counter.clone();
        bus.subscribe("component_started", move |_| {
            counter_clone.fetch_add(1, Ordering::Relaxed);
        })
        .expect("Failed to subscribe to event");

        bus.publish(VmEvent::ComponentStarted("test".to_string()));

        // Give a moment for async processing
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }
}
