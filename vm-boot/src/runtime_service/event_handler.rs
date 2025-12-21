//! 事件处理器服务 - DDD服务层
//!
//! 负责处理VM运行时事件的分发和处理

use crate::runtime::{RuntimeEvent, RuntimeEventListener};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use vm_core::VmError;

/// 事件处理器服务
pub struct EventHandlerService<L: RuntimeEventListener + Send + Sync> {
    /// 事件监听器
    listeners: Arc<Mutex<HashMap<String, Box<L>>>>,
    /// 事件队列
    event_queue: Arc<Mutex<Vec<RuntimeEvent>>>,
    /// 处理的统计信息
    processed_count: Arc<Mutex<u64>>,
}

impl<L: RuntimeEventListener + Clone + Send + Sync + 'static> Default for EventHandlerService<L> {
    fn default() -> Self {
        Self::new()
    }
}

impl<L: RuntimeEventListener + Clone + Send + Sync + 'static> EventHandlerService<L> {
    /// 创建新的事件处理器服务
    pub fn new() -> Self {
        Self {
            listeners: Arc::new(Mutex::new(HashMap::new())),
            event_queue: Arc::new(Mutex::new(Vec::new())),
            processed_count: Arc::new(Mutex::new(0)),
        }
    }

    /// 注册事件监听器
    pub fn register_listener(&self, name: String, listener: L) -> Result<(), VmError> {
        let mut listeners = self.listeners.lock().unwrap();
        listeners.insert(name, Box::new(listener));
        Ok(())
    }

    /// 注销事件监听器
    pub fn unregister_listener(&self, name: &str) -> Result<(), VmError> {
        let mut listeners = self.listeners.lock().unwrap();
        listeners.remove(name);
        Ok(())
    }

    /// 队列事件
    pub fn queue_event(&self, event: RuntimeEvent) {
        let mut queue = self.event_queue.lock().unwrap();
        queue.push(event);
    }

    /// 处理所有排队的事件
    pub fn process_queued_events(&self) -> Result<(), VmError> {
        let events = {
            let mut queue = self.event_queue.lock().unwrap();
            std::mem::take(&mut *queue)
        };

        let mut listeners = self.listeners.lock().unwrap();

        for event in events {
            // 通知所有监听器（需要可变访问）
            for (_name, listener) in listeners.iter_mut() {
                listener.on_event(event.clone());
            }

            // 更新处理计数
            let mut count = self.processed_count.lock().unwrap();
            *count += 1;
        }

        Ok(())
    }

    /// 同步处理事件（立即分发）
    pub fn process_event(&self, event: RuntimeEvent) -> Result<(), VmError> {
        let mut listeners = self.listeners.lock().unwrap();

        for (_name, listener) in listeners.iter_mut() {
            listener.on_event(event.clone());
        }

        let mut count = self.processed_count.lock().unwrap();
        *count += 1;

        Ok(())
    }

    /// 获取监听器数量
    pub fn listener_count(&self) -> usize {
        self.listeners.lock().unwrap().len()
    }

    /// 获取队列中的事件数量
    pub fn queued_event_count(&self) -> usize {
        self.event_queue.lock().unwrap().len()
    }

    /// 获取已处理的事件总数
    pub fn processed_event_count(&self) -> u64 {
        *self.processed_count.lock().unwrap()
    }

    /// 清空事件队列
    pub fn clear_queue(&self) {
        let mut queue = self.event_queue.lock().unwrap();
        queue.clear();
    }
}

/// 标准事件监听器实现
#[derive(Clone)]
pub struct StandardEventListener {
    events_received: Arc<Mutex<Vec<RuntimeEvent>>>,
}

impl Default for StandardEventListener {
    fn default() -> Self {
        Self::new()
    }
}

impl StandardEventListener {
    pub fn new() -> Self {
        Self {
            events_received: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn get_received_events(&self) -> Vec<RuntimeEvent> {
        self.events_received.lock().unwrap().clone()
    }

    pub fn event_count(&self) -> usize {
        self.events_received.lock().unwrap().len()
    }
}

impl RuntimeEventListener for StandardEventListener {
    fn on_event(&mut self, event: RuntimeEvent) {
        let mut events = self.events_received.lock().unwrap();
        events.push(event);
    }
}

/// 过滤事件监听器（只处理特定类型的事件）
pub struct FilteredEventListener<F> {
    filter: F,
    events_received: Arc<Mutex<Vec<RuntimeEvent>>>,
}

impl<F> FilteredEventListener<F>
where
    F: Fn(&RuntimeEvent) -> bool + Send + Sync,
{
    pub fn new(filter: F) -> Self {
        Self {
            filter,
            events_received: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn get_received_events(&self) -> Vec<RuntimeEvent> {
        self.events_received.lock().unwrap().clone()
    }
}

impl<F> RuntimeEventListener for FilteredEventListener<F>
where
    F: Fn(&RuntimeEvent) -> bool + Send + Sync,
{
    fn on_event(&mut self, event: RuntimeEvent) {
        if (self.filter)(&event) {
            let mut events = self.events_received.lock().unwrap();
            events.push(event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_handler_service() {
        let service = EventHandlerService::new();

        // 注册监听器
        let listener = StandardEventListener::new();
        assert!(
            service
                .register_listener("test".to_string(), listener.clone())
                .is_ok()
        );

        // 队列事件
        service.queue_event(RuntimeEvent::Started);
        service.queue_event(RuntimeEvent::Timer);

        assert_eq!(service.queued_event_count(), 2);
        assert_eq!(service.listener_count(), 1);

        // 处理事件
        assert!(service.process_queued_events().is_ok());
        assert_eq!(service.queued_event_count(), 0);
        assert_eq!(service.processed_event_count(), 2);
        assert_eq!(listener.event_count(), 2);
    }

    #[test]
    fn test_filtered_event_listener() {
        let filter = |event: &RuntimeEvent| matches!(event, RuntimeEvent::Timer);
        let mut listener = FilteredEventListener::new(filter);

        // 发送不同类型的事件
        listener.on_event(RuntimeEvent::Started);
        listener.on_event(RuntimeEvent::Timer);
        listener.on_event(RuntimeEvent::Io);
        listener.on_event(RuntimeEvent::Timer);

        let events = listener.get_received_events();
        assert_eq!(events.len(), 2);
        assert!(events.iter().all(|e| matches!(e, RuntimeEvent::Timer)));
    }

    #[test]
    fn test_multiple_listeners() {
        let service = EventHandlerService::new();

        let listener1 = StandardEventListener::new();
        let listener2 = StandardEventListener::new();

        service
            .register_listener("listener1".to_string(), listener1.clone())
            .unwrap();
        service
            .register_listener("listener2".to_string(), listener2.clone())
            .unwrap();

        // 处理事件
        service.process_event(RuntimeEvent::Started).unwrap();

        assert_eq!(listener1.event_count(), 1);
        assert_eq!(listener2.event_count(), 1);
        assert_eq!(service.processed_event_count(), 1);
    }
}
